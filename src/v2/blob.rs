use std::io::{Read, Write};
use thiserror::Error;

use crate::v2::input::Input;

pub struct Blob {
    pub byte_size: u64,
    pub start: u64,
    pub length: u64,
}

#[derive(Debug, Error)]
pub enum BlobError {
    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),
    #[error("Input error: {0}")]
    InputError(#[from] crate::v2::input::InputError),
}

impl Blob {
    pub fn read<R: Read>(reader: &mut R) -> Result<Self, BlobError> {
        let mut buf = [0u8; 8];

        reader.read_exact(&mut buf)?;
        let byte_size = u64::from_le_bytes(buf);
        reader.read_exact(&mut buf)?;
        let start = u64::from_le_bytes(buf);
        reader.read_exact(&mut buf)?;
        let length = u64::from_le_bytes(buf);

        Ok(Self {
            byte_size,
            start,
            length,
        })
    }

    pub fn write<W: Write>(&self, writer: &mut W) -> Result<(), BlobError> {
        if self.length == 0 {
            return Ok(());
        }

        writer.write_all(&self.byte_size.to_le_bytes())?;
        writer.write_all(&self.start.to_le_bytes())?;
        writer.write_all(&self.length.to_le_bytes())?;

        Ok(())
    }

    pub fn read_inputs<R: Read>(
        &self,
        reader: &mut R,
        inputs: &mut Vec<Input>,
        frame: &mut u64,
    ) -> Result<(), BlobError> {
        for i in (self.start as usize)..((self.start + self.length) as usize) {
            inputs.push(Input::read(reader, *frame, self.byte_size as usize)?);

            *frame = inputs[i].frame;
        }

        Ok(())
    }

    pub fn write_inputs<W: Write>(
        &self,
        writer: &mut W,
        inputs: &[Input],
    ) -> Result<(), BlobError> {
        if self.length == 0 {
            return Ok(());
        }

        inputs
            .iter()
            .skip(self.start as usize)
            .take(self.length as usize)
            .try_for_each(|input| input.write(writer, self.byte_size))?;

        Ok(())
    }
}
