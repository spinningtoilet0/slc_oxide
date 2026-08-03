use std::io::{Read, Seek};

use thiserror::Error;

use crate::{v2, v3};

#[derive(Debug, Error)]
pub enum ReplayError {
    #[error("Unknown format")]
    UnknownFormat,
    #[error("V2 error: {0}")]
    V2Error(#[from] v2::replay::ReplayError),
    #[error("V3 error: {0}")]
    V3Error(#[from] v3::replay::ReplayError),
    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),
}

pub enum Replay {
    V2(v2::replay::Replay),
    V3(v3::replay::Replay),
}

impl Replay {
    /// Read the replay from a stream.
    pub fn read<R: Read + Seek>(reader: &mut R) -> Result<Self, ReplayError> {
        let mut header_buf = [0u8; 8];
        reader.read_exact(&mut header_buf)?;
        reader.seek(std::io::SeekFrom::Start(0))?;

        if header_buf[0..4] == v2::replay::Replay::HEADER {
            Ok(Replay::V2(v2::replay::Replay::read(reader)?))
        } else if header_buf[0..8] == v3::replay::Replay::HEADER {
            Ok(Replay::V3(v3::replay::Replay::read(reader)?))
        } else {
            Err(ReplayError::UnknownFormat)
        }
    }
}
