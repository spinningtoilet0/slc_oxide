use std::io::{Read, Write};

use crate::v3::atom::{Atom, AtomError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpaqueAtom {
    pub id: u32,
    pub flags: u8,
    pub body: Vec<u8>,
}

impl OpaqueAtom {
    pub fn read<R: Read>(
        reader: &mut R,
        id: u32,
        flags: u8,
        size: u64,
    ) -> Result<Self, std::io::Error> {
        let mut body = vec![0u8; size as usize];
        reader.read_exact(&mut body)?;

        Ok(Self { id, flags, body })
    }

    pub fn write<W: Write>(&self, writer: &mut W) -> Result<(), AtomError> {
        if self.body.len() as u64 > Atom::MAX_SIZE {
            return Err(AtomError::TooLarge);
        }

        writer.write_all(&self.id.to_le_bytes())?;

        let flags_and_size = ((self.flags as u64) << 56) | (self.body.len() as u64);

        writer.write_all(&flags_and_size.to_le_bytes())?;
        writer.write_all(&self.body)?;

        Ok(())
    }
}
