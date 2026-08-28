use std::io::{Read, Seek, Write};

use thiserror::Error;

use crate::{
    action::{Action, ActionData, Player, PlayerAction},
    v2, v3,
};

#[derive(Debug, Error)]
pub enum ReplayError<'a> {
    /// The file header did not match a known replay format.
    #[error("Unknown format")]
    UnknownFormat,
    #[error("V2 error: {0}")]
    V2Error(#[from] v2::ReplayError),
    #[error("V3 error: {0}")]
    V3Error(#[from] v3::ReplayError),
    /// The specified action is unrepresentable in the desired format.
    #[error("Unrepresentable action: {0}")]
    UnrepresentableAction(&'a Action),
    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
pub enum Replay {
    V2(v2::replay::Replay),
    V3(v3::replay::Replay),
}

impl Replay {
    /// Read the replay from a stream.
    ///
    /// This function expects that the start of the stream is the start of the SLC file, and that the end has no extra bytes.
    pub fn read<R: Read + Seek>(reader: &mut R) -> Result<Self, ReplayError<'_>> {
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

    pub fn to_generic_replay(&self) -> GenericReplay {
        match self {
            Replay::V2(v2_replay) => v2_replay.to_generic_replay(),
            Replay::V3(v3_replay) => v3_replay.to_generic_replay(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GenericReplay {
    pub tps: f64,
    pub actions: Vec<Action>,
}

impl GenericReplay {
    /// Add a new action with the specified data to the replay.
    pub fn add_action(&mut self, action: Action) {
        self.actions.push(action);
    }

    /// Writes the replay in the SLC v2 format.
    ///
    /// This function will error if an action in the `actions` Vec
    /// is of type [ActionData::Bugpoint](crate::action::ActionData::Bugpoint)
    pub fn write_v2<W: Write>(
        &self,
        writer: &mut W,
        metadata: &[u8],
    ) -> Result<(), ReplayError<'_>> {
        let mut v2_inputs = Vec::with_capacity(self.actions.len());

        let mut last_frame = 0;

        for action in &self.actions {
            v2_inputs.push(v2::Input {
                frame: action.frame,
                delta: last_frame - action.frame,
                data: match &action.data {
                    ActionData::Player(player_action) => v2::InputData::Player(v2::PlayerInput {
                        hold: player_action.down,
                        player_2: player_action.player == Player::Player2,
                        button: player_action.action.to_v2_button(),
                    }),
                    ActionData::TPS(tps) => v2::InputData::TPS(*tps),
                    // throws away seed information, meaning if you converted a v3 replay with multiple attempts on a randomized level, the v2 replay would break
                    // this should probably be documented
                    // TODO
                    ActionData::Restart { .. } => v2::InputData::Restart,
                    ActionData::RestartFull { .. } => v2::InputData::RestartFull,
                    ActionData::Death { .. } => v2::InputData::Death,
                    ActionData::Bugpoint => {
                        return Err(ReplayError::UnrepresentableAction(action));
                    }
                },
            });

            last_frame = action.frame;
        }

        v2::Replay::write_inner(writer, self.tps, metadata, &v2_inputs)?;

        Ok(())
    }

    /// Write a v3 format replay
    ///
    /// This function expects that the elements in the `actions` Vec are sorted by frame.
    /// See [GenericReplay::reorder_inputs].
    pub fn write_v3<W: Write>(
        &self,
        writer: &mut W,
        metadata: v3::Metadata,
    ) -> Result<(), ReplayError<'_>> {
        let mut v3_actions = Vec::with_capacity(self.actions.len());

        for action in &self.actions {
            v3_actions.push(v3::atom::action::Action::new(
                action.frame,
                match &action.data {
                    ActionData::Player(player_input) => {
                        let holding = player_input.down;
                        let player2 = player_input.player == Player::Player2;

                        match player_input.action {
                            PlayerAction::Jump => {
                                v3::atom::action::ActionData::Jump { holding, player2 }
                            }
                            PlayerAction::Left => {
                                v3::atom::action::ActionData::Left { holding, player2 }
                            }
                            PlayerAction::Right => {
                                v3::atom::action::ActionData::Right { holding, player2 }
                            }
                        }
                    }
                    ActionData::Restart { seed } => {
                        v3::atom::action::ActionData::Restart { seed: *seed }
                    }
                    ActionData::RestartFull { seed } => {
                        v3::atom::action::ActionData::RestartFull { seed: *seed }
                    }
                    ActionData::Death { seed } => {
                        v3::atom::action::ActionData::Death { seed: *seed }
                    }
                    ActionData::TPS(tps) => v3::atom::action::ActionData::TPS(*tps),
                    ActionData::Bugpoint => v3::atom::action::ActionData::Bugpoint,
                },
            ));
        }

        let mut registry = v3::atom::AtomRegistry::default();

        registry
            .atoms
            .push(v3::atom::Atom::Action(v3::atom::ActionAtom {
                flags: 0,
                actions: v3_actions,
            }));

        let mut v3_replay = v3::Replay { metadata, registry };
        v3_replay.write(writer)?;

        Ok(())
    }

    /// Orders elements in the `inputs` array by frame, from least to greatest
    pub fn order_inputs(&mut self) {
        self.actions.sort_by_key(|a| a.frame);
    }
}

impl<'a> From<&'a Replay> for GenericReplay {
    fn from(value: &'a Replay) -> Self {
        value.to_generic_replay()
    }
}

impl From<&v2::Replay> for GenericReplay {
    fn from(value: &v2::Replay) -> Self {
        value.to_generic_replay()
    }
}

impl From<&v3::Replay> for GenericReplay {
    fn from(value: &v3::Replay) -> Self {
        value.to_generic_replay()
    }
}
