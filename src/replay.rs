use std::io::{Read, Seek, Write};

use thiserror::Error;

use crate::{
    v2::{
        self,
        input::{Input, InputData, PlayerInput},
    },
    v3,
};

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
    ///
    /// This function expects that the start of the stream is the start of the SLC file, and that the end has no extra bytes.
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

    pub fn to_generic_replay(self) -> GenericReplay {
        match self {
            Replay::V2(v2_replay) => GenericReplay {
                tps: v2_replay.tps,
                inputs: v2_replay.inputs,
            },
            Replay::V3(v3_replay) => {
                let mut replay = GenericReplay {
                    tps: v3_replay.metadata.tps,
                    inputs: Vec::new(),
                };

                use v3::action::ActionType;
                use v3::atom::AtomVariant;

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
                                    InputData::Player(PlayerInput {
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

                replay
            }
        }
    }
}

// TODO: migrate from v2::input::Input to a common type

pub struct GenericReplay {
    pub tps: f64,
    pub inputs: Vec<Input>,
}

impl GenericReplay {
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

    pub fn write_v2<W: Write>(&self, writer: &mut W, metadata: &[u8]) -> Result<(), ReplayError> {
        v2::replay::Replay::write_inner(writer, self.tps, metadata, &self.inputs)?;

        Ok(())
    }

    pub fn write_v3<W: Write>(
        &self,
        writer: &mut W,
        seed: u64,
        build: u32,
    ) -> Result<(), ReplayError> {
        use crate::v3::atom::AtomVariant;
        use crate::v3::builtin::ActionAtom;
        use crate::v3::{ActionType, Metadata};

        let metadata = Metadata::new(self.tps, seed, build);
        let mut v3_replay = crate::v3::Replay::new(metadata);

        let mut action_atom = ActionAtom::new();

        for input in &self.inputs {
            match &input.data {
                InputData::Player(p) => {
                    let action_type = match p.button {
                        1 => ActionType::Jump,
                        2 => ActionType::Left,
                        3 => ActionType::Right,
                        _ => {
                            return Err(ReplayError::V3Error(
                                v3::replay::ReplayError::InvalidPlayerButton(p.button),
                            ));
                        }
                    };

                    action_atom.add_player_action(input.frame, action_type, p.hold, p.player_2)
                }
                InputData::Restart => {
                    action_atom.add_death_action(input.frame, ActionType::Restart, 0)
                }
                InputData::RestartFull => {
                    action_atom.add_death_action(input.frame, ActionType::RestartFull, 0)
                }
                InputData::Death => action_atom.add_death_action(input.frame, ActionType::Death, 0),
                InputData::TPS(tps) => action_atom.add_tps_action(input.frame, *tps),
                InputData::Skip => Ok(()),
            }
            .map_err(v3::replay::ReplayError::from)?;
        }

        v3_replay.add_atom(AtomVariant::Action(action_atom));
        v3_replay.write(writer)?;

        Ok(())
    }
}
