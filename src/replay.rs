use std::io::{Read, Seek, Write};

use thiserror::Error;

use crate::{
    input::{Input, InputData},
    meta::Meta,
    v2, v3,
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
pub struct Replay<M: Meta + Clone> {
    pub tps: f64,
    pub meta: M,

    pub inputs: Vec<Input>,
}

#[derive(Debug, Error)]
pub enum ReplayError {
    #[error("Unknown format")]
    UnknownFormat,
    #[error("V3 error: {0}")]
    V2Error(#[from] crate::v2::replay::ReplayError),
    #[error("V3 error: {0}")]
    V3Error(#[from] crate::v3::replay::ReplayError),
    #[error("V3 atom error: {0}")]
    V3AtomError(#[from] crate::v3::atom::AtomError),
    #[error("Invalid player button for v3 conversion: {0}")]
    InvalidV3PlayerButton(u8),
    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),
}

impl<M: Meta + Clone> Replay<M> {
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

        if header_buf[0..4] == v2::replay::Replay::<()>::HEADER {
            Self::read_v2(reader)
        } else if header_buf[0..8] == v3::replay::Replay::HEADER {
            Self::read_v3(reader)
        } else {
            Err(ReplayError::UnknownFormat)
        }
    }

    fn read_v2<R: Read>(reader: &mut R) -> Result<Self, ReplayError> {
        let replay = v2::replay::Replay::read(reader)?;

        Ok(Self {
            tps: replay.tps,
            meta: replay.meta,
            inputs: replay.inputs,
        })
    }

    fn read_v3<R: Read + Seek>(reader: &mut R) -> Result<Self, ReplayError> {
        use crate::v3::ActionType;
        use crate::v3::atom::AtomVariant;

        let v3_replay = v3::Replay::read(reader)?;

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
        let replay = v2::replay::Replay {
            tps: self.tps,
            meta: self.meta.clone(), // yeah cloning is not nice but it makes the api nicer idc
            inputs: self.inputs.clone(),
        };

        replay.write(writer)?;

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
