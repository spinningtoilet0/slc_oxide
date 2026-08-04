use std::{
    cmp::Ordering,
    io::{Read, Write},
};

use thiserror::Error;

use crate::{
    replay::GenericReplay,
    v2::{blob::Blob, input::Input},
};

pub struct Replay {
    pub tps: f64,
    pub meta: Vec<u8>,
    pub inputs: Vec<Input>,
}

#[derive(Debug, Error)]
pub enum ReplayError {
    #[error("Header mismatch error")]
    HeaderMismatchError,
    #[error("Footer mismatch error")]
    FooterMismatchError,
    #[error("Blob error: {0}")]
    Blob(#[from] crate::v2::blob::BlobError),
    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),
}

impl Replay {
    pub const HEADER: [u8; 4] = *b"SILL";
    pub const FOOTER: [u8; 3] = *b"EOM";

    pub fn read<R: Read>(reader: &mut R) -> Result<Self, ReplayError> {
        let mut header_buf = [0u8; 4];
        reader.read_exact(&mut header_buf)?;

        if header_buf != Self::HEADER {
            return Err(ReplayError::HeaderMismatchError);
        }

        let mut big_buf = [0u8; 8];
        reader.read_exact(&mut big_buf)?;
        let tps = f64::from_le_bytes(big_buf);

        reader.read_exact(&mut big_buf)?;
        let meta_size = u64::from_le_bytes(big_buf);

        let mut meta = vec![0u8; meta_size as usize];
        reader.read_exact(meta.as_mut_slice())?;

        reader.read_exact(&mut big_buf)?;
        let length = u64::from_le_bytes(big_buf);
        let mut inputs: Vec<Input> = Vec::with_capacity(length as usize);

        reader.read_exact(&mut big_buf)?;
        let blob_count = u64::from_le_bytes(big_buf);

        let mut blobs: Vec<Blob> = Vec::with_capacity(blob_count as usize);
        for _ in 0..blob_count {
            blobs.push(Blob::read(reader)?);
        }

        let mut current_frame = 0;
        for blob in blobs {
            blob.read_inputs(reader, &mut inputs, &mut current_frame)?;
        }

        let mut footer_buf = [0u8; 3];
        reader.read_exact(&mut footer_buf)?;
        if footer_buf != Self::FOOTER {
            return Err(ReplayError::FooterMismatchError);
        }

        Ok(Self { tps, meta, inputs })
    }

    pub fn write<W: Write>(&self, writer: &mut W) -> Result<(), ReplayError> {
        Self::write_inner(writer, self.tps, &self.meta, &self.inputs)?;

        Ok(())
    }

    // theres probably
    pub(crate) fn write_inner<W: Write>(
        writer: &mut W,
        tps: f64,
        meta: &[u8],
        inputs: &[Input],
    ) -> Result<(), ReplayError> {
        writer.write_all(&Self::HEADER)?;

        writer.write_all(&tps.to_le_bytes())?;
        writer.write_all(&(meta.len() as u64).to_le_bytes())?;
        writer.write_all(meta)?;
        writer.write_all(&(inputs.len() as u64).to_le_bytes())?;

        let mut blobs: Vec<Blob> = Vec::new();

        // First blob pass
        inputs.iter().enumerate().for_each(|(i, input)| {
            let byte_size = input.required_bytes();
            if blobs.is_empty() {
                blobs.push(Blob {
                    byte_size: byte_size as u64,
                    start: i as u64,
                    length: 1,
                });
                return;
            }

            let blob = blobs
                .last_mut()
                .expect("Blobs should have an element already");

            match blob.byte_size.cmp(&(byte_size as u64)) {
                Ordering::Less | Ordering::Greater => {
                    blobs.push(Blob {
                        byte_size: byte_size as u64,
                        start: i as u64,
                        length: 1,
                    });
                }
                Ordering::Equal => {
                    blob.length += 1;
                }
            }
        });

        let mut zero_sized_blobs = 0;

        // Second blob pass
        for i in (1..blobs.len()).rev() {
            let [previous, blob] = blobs
                .get_disjoint_mut([i - 1, i])
                .expect("Blob should exist");

            let blob_size = blob.byte_size * blob.length;
            const BLOB_MEM_SIZE: u64 = 24;

            if blob_size < BLOB_MEM_SIZE {
                if blob.byte_size > previous.byte_size
                    && (previous.byte_size * blob.length) < BLOB_MEM_SIZE
                {
                    previous.length += blob.length;
                    previous.byte_size = blob.byte_size;
                    blob.length = 0;
                    zero_sized_blobs += 1;
                    continue;
                } else if blob.byte_size < previous.byte_size
                    && (previous.byte_size * blob.length) < BLOB_MEM_SIZE
                {
                    previous.length += blob.length;
                    blob.length = 0;
                    zero_sized_blobs += 1;
                    continue;
                }
            }

            if blob.byte_size == previous.byte_size {
                previous.length += blob.length;
                blob.length = 0;
                zero_sized_blobs += 1;
            }
        }

        let blob_length: u64 = blobs.len() as u64 - zero_sized_blobs;
        writer.write_all(&blob_length.to_le_bytes())?;

        blobs.iter().try_for_each(|b| b.write(writer))?;
        blobs
            .iter()
            .try_for_each(|b| b.write_inputs(writer, inputs))?;

        writer.write_all(&Self::FOOTER)?;

        Ok(())
    }

    pub fn to_generic_replay(self) -> GenericReplay {
        GenericReplay {
            tps: self.tps,
            inputs: self.inputs,
        }
    }
}
