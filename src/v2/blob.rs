use std::io::{Read, Write};

use crate::v2::{ReplayError, input::Input};

pub struct Blob {
    pub byte_size: u64,
    pub start: u64,
    pub length: u64,
}

impl Blob {
    pub fn read<R: Read>(reader: &mut R) -> Result<Self, std::io::Error> {
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

    pub fn write<W: Write>(&self, writer: &mut W) -> Result<(), std::io::Error> {
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
    ) -> Result<(), ReplayError> {
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
    ) -> Result<(), ReplayError> {
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
