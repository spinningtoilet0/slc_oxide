use std::{
    cmp::Ordering,
    io::{Read, Seek, Write},
};

use thiserror::Error;

use crate::{
    blob::Blob,
    input::{Input, InputData},
    meta::Meta,
};

/// An slc replay.
///
/// This replay format is designed to be small, while still efficiently parsing replays.
///
/// You may specify your own custom meta through the `M` generic type. See [`slc_oxide::meta::Meta`] for further details.
///
/// # Examples
/// ```ignore
/// struct ReplayMeta {
///   pub seed: u64
/// }
///
/// let mut replay = Replay::<ReplayMeta>::new(
///   240.0,
///   ReplayMeta {
///     seed: 1234
///   }
/// );
///
/// // OR
///
/// let mut replay = Replay::<()>::new(240.0, ()); // For no meta
///
/// // Set tps by directly changing the value
/// replay.tps = 480.0;
///
/// // Add inputs using the `add_input` function
/// replay.add_input(200, InputData::Player(PlayerData {
///   button: 1,
///   hold: true,
///   player_2: false
/// }));
///
/// // Other input types
/// replay.add_input(400, InputData::Death);
/// replay.add_input(600, InputData::TPS(480.0));
///
/// // Save the replay
/// let file = File::open("replay.slc")?;
/// let bw = BufWriter::new(file); // RECOMMENDED!
/// replay.write(bw)?;
/// ```
pub struct Replay<M: Meta> {
    pub tps: f64,
    pub meta: M,

    pub inputs: Vec<Input>,
}

#[derive(Debug, Error)]
pub enum ReplayError {
    #[error("Header mismatch error")]
    HeaderMismatchError,
    #[error("Meta size mismatch error")]
    MetaSizeMismatchError,
    #[error("Footer mismatch error")]
    FooterMismatchError,
    #[error("Unknown format")]
    UnknownFormat,
    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),
    #[error("Blob error: {0}")]
    Blob(#[from] crate::blob::BlobError),
    #[error("V3 error: {0}")]
    V3Error(#[from] crate::v3::replay::ReplayError),
    #[error("V3 atom error: {0}")]
    V3AtomError(#[from] crate::v3::atom::AtomError),
    #[error("Invalid player button for v3 conversion: {0}")]
    InvalidV3PlayerButton(u8),
}

pub const V2_HEADER: [u8; 4] = *b"SILL";
pub const V2_FOOTER: [u8; 3] = *b"EOM";
pub const V3_HEADER: [u8; 8] = *b"SLC3RPLY";

impl<M: Meta> Replay<M> {
    /// Create a new slc replay with the specified tps and meta.
    pub fn new(tps: f64, meta: M) -> Self {
        Self {
            tps,
            meta,
            inputs: vec![],
        }
    }

    /// Add a new input with the specified data to the replay.
    pub fn add_input(&mut self, frame: u64, data: InputData) {
        if self.inputs.is_empty() {
            self.inputs.push(Input {
                frame,
                delta: frame,
                data,
            });

            return;
        }

        let last_input = self.inputs.last().expect("Input should exist");

        self.inputs.push(Input {
            frame,
            delta: frame - last_input.frame,
            data,
        })
    }

    /// Read the replay from a stream.
    pub fn read<R: Read + Seek>(reader: &mut R) -> Result<Self, ReplayError> {
        let mut header_buf = [0u8; 8];
        reader.read_exact(&mut header_buf)?;
        reader.seek(std::io::SeekFrom::Start(0))?;

        if header_buf[0..4] == V2_HEADER {
            Self::read_v2(reader)
        } else if header_buf[0..8] == V3_HEADER {
            Self::read_v3(reader)
        } else {
            Err(ReplayError::UnknownFormat)
        }
    }

    fn read_v2<R: Read>(reader: &mut R) -> Result<Self, ReplayError> {
        let mut header_buf = [0u8; 4];
        reader.read_exact(&mut header_buf)?;

        if header_buf != V2_HEADER {
            return Err(ReplayError::HeaderMismatchError);
        }

        let mut big_buf = [0u8; 8];
        reader.read_exact(&mut big_buf)?;
        let tps = f64::from_le_bytes(big_buf);

        reader.read_exact(&mut big_buf)?;
        let meta_size = u64::from_le_bytes(big_buf);
        if meta_size != M::size() {
            return Err(ReplayError::MetaSizeMismatchError);
        }

        let mut meta_buf = vec![0u8; M::size() as usize];
        reader.read_exact(meta_buf.as_mut_slice())?;
        let meta = M::from_bytes(meta_buf.as_slice());

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
        if footer_buf != V2_FOOTER {
            return Err(ReplayError::FooterMismatchError);
        }

        Ok(Self { tps, meta, inputs })
    }

    fn read_v3<R: Read + Seek>(reader: &mut R) -> Result<Self, ReplayError> {
        use crate::v3::ActionType;
        use crate::v3::atom::AtomVariant;

        let v3_replay = crate::v3::Replay::read(reader)?;

        let empty_meta = vec![0u8; M::size() as usize];
        let mut replay = Self::new(v3_replay.metadata.tps, M::from_bytes(&empty_meta));

        for atom in &v3_replay.atoms.atoms {
            if let AtomVariant::Action(action_atom) = atom {
                for action in &action_atom.actions {
                    let data = match action.action_type {
                        ActionType::Jump | ActionType::Left | ActionType::Right => {
                            let button = match action.action_type {
                                ActionType::Jump => 1,
                                ActionType::Left => 2,
                                ActionType::Right => 3,
                                _ => 1,
                            };
                            InputData::Player(crate::input::PlayerInput {
                                hold: action.holding,
                                player_2: action.player2,
                                button,
                            })
                        }
                        ActionType::Restart => InputData::Restart,
                        ActionType::RestartFull => InputData::RestartFull,
                        ActionType::Death => InputData::Death,
                        ActionType::TPS => InputData::TPS(action.tps),
                        ActionType::Bugpoint => InputData::Skip,
                        ActionType::Reserved => InputData::Skip,
                    };

                    replay.add_input(action.frame, data);
                }
            }
        }

        Ok(replay)
    }

    /// Write the replay to a stream in v2 format.
    pub fn write<W: Write>(&self, writer: &mut W) -> Result<(), ReplayError> {
        self.write_v2(writer)
    }

    fn write_v2<W: Write>(&self, writer: &mut W) -> Result<(), ReplayError> {
        writer.write_all(&V2_HEADER)?;

        writer.write_all(&self.tps.to_le_bytes())?;
        writer.write_all(&M::size().to_le_bytes())?;
        writer.write_all(&self.meta.to_bytes())?;

        writer.write_all(&(self.inputs.len() as u64).to_le_bytes())?;

        let mut blobs: Vec<Blob> = Vec::new();

        // First blob pass
        self.inputs.iter().enumerate().for_each(|(i, input)| {
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
            .try_for_each(|b| b.write_inputs(writer, self.inputs.as_slice()))?;

        writer.write_all(&V2_FOOTER)?;

        Ok(())
    }

    pub fn write_v3<W: Write>(&self, writer: &mut W) -> Result<(), ReplayError> {
        use crate::v3::atom::AtomVariant;
        use crate::v3::builtin::ActionAtom;
        use crate::v3::{ActionType, Metadata};

        let metadata = Metadata::new(self.tps, 0, 1);
        let mut v3_replay = crate::v3::Replay::new(metadata);

        let mut action_atom = ActionAtom::new();

        for input in &self.inputs {
            match &input.data {
                InputData::Player(p) => {
                    let action_type = match p.button {
                        1 => ActionType::Jump,
                        2 => ActionType::Left,
                        3 => ActionType::Right,
                        _ => return Err(ReplayError::InvalidV3PlayerButton(p.button)),
                    };
                    action_atom.add_player_action(input.frame, action_type, p.hold, p.player_2)?;
                }
                InputData::Restart => {
                    action_atom.add_death_action(input.frame, ActionType::Restart, 0)?;
                }
                InputData::RestartFull => {
                    action_atom.add_death_action(input.frame, ActionType::RestartFull, 0)?;
                }
                InputData::Death => {
                    action_atom.add_death_action(input.frame, ActionType::Death, 0)?;
                }
                InputData::TPS(tps) => {
                    action_atom.add_tps_action(input.frame, *tps)?;
                }
                InputData::Skip => {}
            }
        }

        v3_replay.add_atom(AtomVariant::Action(action_atom));
        v3_replay.write(writer)?;

        Ok(())
    }
}
