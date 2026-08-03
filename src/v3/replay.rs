use std::io::{Read, Seek, Write};
use thiserror::Error;

use super::atom::{AtomRegistry, AtomVariant};
use super::metadata::{METADATA_SIZE, Metadata};

/// An SLC3 format replay.
///
/// # Examples
/// ```no_run
/// use slc_oxide::v3::{Replay, Metadata, ActionType};
/// use slc_oxide::v3::atom::AtomVariant;
/// use slc_oxide::v3::builtin::ActionAtom;
/// use std::fs::File;
/// use std::io::BufWriter;
///
/// let metadata = Metadata::new(240.0, 12345, 1);
/// let mut replay = Replay::new(metadata);
///
/// let mut action_atom = ActionAtom::new();
/// action_atom.add_player_action(100, ActionType::Jump, true, false).unwrap();
/// action_atom.add_player_action(102, ActionType::Jump, false, false).unwrap();
///
/// replay.add_atom(AtomVariant::Action(action_atom));
///
/// let file = File::create("replay.slc3").unwrap();
/// let mut writer = BufWriter::new(file);
/// replay.write(&mut writer).unwrap();
/// ```
pub struct Replay {
    pub metadata: Metadata,
    pub atoms: AtomRegistry,
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
    AtomError(#[from] super::atom::AtomError),
    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),
}

impl Replay {
    pub const HEADER: [u8; 8] = *b"SLC3RPLY";
    pub const FOOTER: u8 = 0xCC;

    pub fn new(metadata: Metadata) -> Self {
        Self {
            metadata,
            atoms: AtomRegistry::new(),
        }
    }

    pub fn read<R: Read + Seek>(reader: &mut R) -> Result<Self, ReplayError> {
        let mut header_buf = [0u8; 8];
        reader.read_exact(&mut header_buf)?;

        if header_buf != Self::HEADER {
            return Err(ReplayError::InvalidHeader);
        }

        let mut buf2 = [0u8; 2];
        reader.read_exact(&mut buf2)?;
        let meta_size = u16::from_le_bytes(buf2);

        if meta_size != METADATA_SIZE as u16 {
            return Err(ReplayError::InvalidMetadataSize);
        }

        let metadata = Metadata::read(reader)?;

        let mut atoms = AtomRegistry::new();

        let current_pos = reader.stream_position()?;
        reader.seek(std::io::SeekFrom::End(-1))?;
        let end_pos = reader.stream_position()?;
        reader.seek(std::io::SeekFrom::Start(current_pos))?;

        atoms.read_all(reader, end_pos)?;

        let mut footer_buf = [0u8; 1];
        reader.read_exact(&mut footer_buf)?;

        if footer_buf[0] != Self::FOOTER {
            return Err(ReplayError::InvalidFooter);
        }

        Ok(Self { metadata, atoms })
    }

    pub fn write<W: Write>(&self, writer: &mut W) -> Result<(), ReplayError> {
        writer.write_all(&Self::HEADER)?;

        let meta_size = METADATA_SIZE as u16;
        writer.write_all(&meta_size.to_le_bytes())?;

        self.metadata.write(writer)?;

        self.atoms.write_all(writer)?;

        writer.write_all(&[Self::FOOTER])?;

        Ok(())
    }

    pub fn add_atom(&mut self, atom: AtomVariant) {
        self.atoms.add(atom);
    }
}
