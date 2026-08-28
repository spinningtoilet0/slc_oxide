pub mod action;
pub mod opaque;

use std::io::{Read, Seek, Write};

use thiserror::Error;

pub use crate::v3::atom::{action::ActionAtom, opaque::OpaqueAtom};

#[derive(Debug, Clone)]
pub enum Atom {
    Opaque(OpaqueAtom),
    Action(ActionAtom),
}

#[derive(Debug, Error)]
pub enum AtomError {
    /// This is only thrown when an [OpaqueAtom] is over the size limit of 2^56 bytes (this isn't really possible)
    #[error("Too large")]
    TooLarge,
    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),
}

impl Atom {
    pub const MAX_SIZE: u64 = 0x00FF_FFFF_FFFF_FFFF;

    fn read<R: Read>(reader: &mut R) -> Result<Self, std::io::Error> {
        let mut buf4 = [0u8; 4];
        reader.read_exact(&mut buf4)?;

        let id = u32::from_le_bytes(buf4);

        let mut buf8 = [0u8; 8];

        reader.read_exact(&mut buf8)?;
        let flags_and_size = u64::from_le_bytes(buf8);

        let flags = (flags_and_size >> 56) as u8;
        let size = flags_and_size & 0x00FF_FFFF_FFFF_FFFF;

        Ok(match id {
            ActionAtom::ID => Atom::Action(ActionAtom::read(reader, flags, size)?),
            _ => Atom::Opaque(OpaqueAtom::read(reader, id, flags, size)?),
        })
    }

    fn write<W: Write>(&mut self, writer: &mut W) -> Result<(), AtomError> {
        match self {
            Atom::Opaque(opaque_atom) => opaque_atom.write(writer),
            Atom::Action(action_atom) => action_atom.write(writer),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct AtomRegistry {
    pub atoms: Vec<Atom>,
}

impl AtomRegistry {
    pub fn read<R: Read + Seek>(reader: &mut R) -> Result<Self, std::io::Error> {
        let current_pos = reader.stream_position()?;
        let end_pos = reader.seek(std::io::SeekFrom::End(-1))?;
        reader.seek(std::io::SeekFrom::Start(current_pos))?;

        let mut atoms = Vec::new();

        while reader.stream_position()? < end_pos {
            atoms.push(Atom::read(reader)?);
        }

        Ok(Self { atoms })
    }

    pub fn write<W: Write>(&mut self, writer: &mut W) -> Result<(), AtomError> {
        for atom in &mut self.atoms {
            atom.write(writer)?;
        }

        Ok(())
    }
}
