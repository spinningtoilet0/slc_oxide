use std::io::{Read, Seek, Write};

use thiserror::Error;

use crate::{
    GenericReplay,
    v3::{
        atom::{self, AtomRegistry},
        metadata::Metadata,
    },
};

#[derive(Debug, Clone)]
pub struct Replay {
    pub metadata: Metadata,
    pub registry: AtomRegistry,
}

#[derive(Debug, Error)]
pub enum ReplayError {
    #[error("Invalid header")]
    InvalidHeader,
    #[error("Invalid metadata size")]
    InvalidMetadataSize,
    #[error("Invalid footer")]
    InvalidFooter,
    #[error("Atom error: {0}")]
    AtomError(#[from] atom::AtomError),
    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),
}

impl Replay {
    pub const HEADER: [u8; 8] = *b"SLC3RPLY";
    pub const FOOTER: u8 = 0xCC;

    pub fn read<R: Read + Seek>(reader: &mut R) -> Result<Self, ReplayError> {
        let mut header_buf = [0u8; 8];

        reader.read_exact(&mut header_buf)?;

        if header_buf != Self::HEADER {
            return Err(ReplayError::InvalidHeader);
        }

        let mut meta_size_buf = [0u8; 2];
        reader.read_exact(&mut meta_size_buf)?;
        let meta_size = u16::from_le_bytes(meta_size_buf);

        if meta_size != Metadata::SIZE as u16 {
            return Err(ReplayError::InvalidMetadataSize);
        }

        let metadata = Metadata::read(reader)?;
        let atoms = AtomRegistry::read(reader)?;

        let mut footer_buf = [0u8; 1];
        reader.read_exact(&mut footer_buf)?;

        if footer_buf[0] != Self::FOOTER {
            return Err(ReplayError::InvalidFooter);
        }

        Ok(Self {
            metadata,
            registry: atoms,
        })
    }

    pub fn write<W: Write>(&mut self, writer: &mut W) -> Result<(), ReplayError> {
        writer.write_all(&Self::HEADER)?;

        writer.write_all(&(Metadata::SIZE as u16).to_le_bytes())?;
        self.metadata.write(writer)?;

        self.registry.write(writer)?;

        writer.write_all(&[Self::FOOTER])?;

        Ok(())
    }

    pub fn to_generic_replay(&self) -> GenericReplay {
        todo!()
    }
}
