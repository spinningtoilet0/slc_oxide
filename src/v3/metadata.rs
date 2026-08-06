use std::io::{Read, Write};

pub const METADATA_SIZE: usize = 64;

#[derive(Debug, Clone, Copy)]
pub struct Metadata {
    pub tps: f64,
    pub seed: u64,
    pub version: u32,
    pub build: u32,
    pub randomness_algorithm: u32,
}

impl Metadata {
    pub fn new(tps: f64, seed: u64, build: u32) -> Self {
        Self {
            tps,
            seed,
            version: 2,
            build,
            randomness_algorithm: 0,
        }
    }

    pub fn read<R: Read>(reader: &mut R) -> std::io::Result<Self> {
        let mut buf = [0u8; 8];
        reader.read_exact(&mut buf)?;
        let tps = f64::from_le_bytes(buf);

        reader.read_exact(&mut buf)?;
        let seed = u64::from_le_bytes(buf);

        let mut buf4 = [0u8; 4];
        reader.read_exact(&mut buf4)?;
        let version = u32::from_le_bytes(buf4);

        reader.read_exact(&mut buf4)?;
        let build = u32::from_le_bytes(buf4);

        reader.read_exact(&mut buf4)?;
        let randomness_algorithm = u32::from_le_bytes(buf4);

        reader.read_exact(&mut [0u8; 36])?; // pading

        Ok(Self {
            tps,
            seed,
            version,
            build,
            randomness_algorithm,
        })
    }

    pub fn write<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(&self.tps.to_le_bytes())?;
        writer.write_all(&self.seed.to_le_bytes())?;
        writer.write_all(&self.version.to_le_bytes())?;
        writer.write_all(&self.build.to_le_bytes())?;
        writer.write_all(&self.randomness_algorithm.to_le_bytes())?;
        writer.write_all(&[0u8; 36])?; // padding
        Ok(())
    }
}
