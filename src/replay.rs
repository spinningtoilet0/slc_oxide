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
    V2Error(#[from] v2::replay::ReplayError),
    #[error("V3 error: {0}")]
    V3Error(#[from] v3::replay::ReplayError),
    /// The specified action is unrepresentable in the desired format.
    #[error("Unrepresentable action: {0}")]
    UnrepresentableAction(&'a Action),
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

    pub fn to_generic_replay(&self) -> Result<GenericReplay, ReplayError<'_>> {
        Ok(match self {
            Replay::V2(v2_replay) => v2_replay.to_generic_replay()?,
            Replay::V3(v3_replay) => v3_replay.to_generic_replay(),
        })
    }
}

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
                    ActionData::Restart => v2::InputData::Restart,
                    ActionData::RestartFull => v2::InputData::RestartFull,
                    ActionData::Death => v2::InputData::Death,
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
        use crate::v3::ActionType;
        use crate::v3::atom::ActionAtom;
        use crate::v3::atom::AtomVariant;

        let mut v3_replay = crate::v3::Replay::new(metadata);

        let mut action_atom = ActionAtom::new();

        for input in &self.actions {
            match &input.data {
                ActionData::Player(p) => {
                    let action_type = match p.action {
                        PlayerAction::Jump => ActionType::Jump,
                        PlayerAction::Left => ActionType::Left,
                        PlayerAction::Right => ActionType::Right,
                    };

                    action_atom.add_player_action(
                        input.frame,
                        action_type,
                        p.down,
                        p.player == Player::Player2,
                    )
                }
                ActionData::Restart => {
                    action_atom.add_death_action(input.frame, ActionType::Restart, 0)
                }
                ActionData::RestartFull => {
                    action_atom.add_death_action(input.frame, ActionType::RestartFull, 0)
                }
                ActionData::Death => {
                    action_atom.add_death_action(input.frame, ActionType::Death, 0)
                }

                ActionData::TPS(tps) => action_atom.add_tps_action(input.frame, *tps),
                ActionData::Bugpoint => action_atom.add_bugpoint_action(input.frame),
            }
            .map_err(v3::replay::ReplayError::from)?;
        }

        v3_replay.add_atom(AtomVariant::Action(action_atom));
        v3_replay.write(writer)?;

        Ok(())
    }

    /// Orders elements in the `inputs` array by frame, from least to greatest
    pub fn order_inputs(&mut self) {
        self.actions.sort_by_key(|a| a.frame);
    }
}

impl TryFrom<&v2::Replay> for GenericReplay {
    type Error = v2::ReplayError;

    fn try_from(value: &v2::Replay) -> Result<Self, v2::ReplayError> {
        value.to_generic_replay()
    }
}

impl From<&v3::Replay> for GenericReplay {
    fn from(value: &v3::Replay) -> Self {
        value.to_generic_replay()
    }
}
