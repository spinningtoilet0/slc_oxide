use std::{
    fmt::Display,
    io::{Read, Write},
};

use thiserror::Error;

/// A player input.
///
/// This input assumes the following buttons:
/// 1 - Jump
/// 2 - Left
/// 3 - Right
///
/// Buttons match the in-game buttons directly provided in `GJBaseGameLayer::handleButton`.
/// You may safely use them without any further processing.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerInput {
    pub hold: bool,
    pub player_2: bool,
    pub button: u8,
}

/// Backwards compatibility alias for PlayerInput.
#[deprecated(since = "0.2.0", note = "Use `PlayerInput` instead")]
pub type PlayerData = PlayerInput;

/// Data specifying an input's action.
#[derive(Debug, Clone, PartialEq)]
pub enum InputData {
    /// This input does nothing.
    Skip,
    /// This input is an in-game player button push.
    Player(PlayerInput),
    /// This input restarts the level. (`PlayLayer::resetLevel`)
    Restart,
    /// This input restarts the level, fully. (`PlayLayer::fullReset`)
    RestartFull,
    /// This input signals that the player may die on any subsequent frame.
    Death,
    /// This input changes the current tps of the replay.
    TPS(f64),
}

impl Display for InputData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InputData::Skip => write!(f, "skip"),
            Self::Player(p) => write!(
                f,
                "button: {}, hold: {}, p2: {}",
                p.button, p.hold, p.player_2
            ),
            Self::Death => write!(f, "death"),
            Self::Restart => write!(f, "restart"),
            Self::RestartFull => write!(f, "full restart"),
            Self::TPS(tps) => write!(f, "tps: {}", tps),
        }
    }
}

/// A replay input.
///
/// Replay inputs are identified by the frame they're on. Do note
/// that different bots count frames differently (e.g. using GJGameState's `m_currentProgress`).
#[derive(Debug, Clone, PartialEq)]
pub struct Input {
    pub delta: u64,
    pub frame: u64,
    pub data: InputData,
}

impl Display for Input {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "frame: {}, input: {}", self.frame, self.data)
    }
}

// IO

#[derive(Debug, Error)]
pub enum InputError {
    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),
    #[error("Invalid TPS provided")]
    InvalidTPS,
    #[error("Invalid button type")]
    InvalidButton,
}

impl Input {
    pub(crate) fn read<R: Read>(
        reader: &mut R,
        current_frame: u64,
        byte_size: usize,
    ) -> Result<Self, InputError> {
        let mut buf = vec![0u8; byte_size];
        reader.read_exact(&mut buf)?;
        buf.resize(8, 0);

        let state = u64::from_le_bytes(*unsafe { &*buf.as_ptr().cast::<[u8; 8]>() });

        let delta = state >> 5;
        let frame = current_frame + delta;
        let button = (state & 0b11100) >> 2;

        let data = match button {
            0 => InputData::Skip,
            1..=3 => InputData::Player(PlayerInput {
                hold: (state & 1) != 0,
                player_2: (state & 2) != 0,
                button: button as u8,
            }),
            4 => InputData::Restart,
            5 => InputData::RestartFull,
            6 => InputData::Death,
            7 => {
                reader.read_exact(&mut buf)?;
                let tps = f64::from_le_bytes(*unsafe { &*buf.as_ptr().cast::<[u8; 8]>() });

                InputData::TPS(tps)
            }
            _ => return Err(InputError::InvalidButton),
        };

        Ok(Input { delta, frame, data })
    }

    const fn to_state(&self) -> u64 {
        let state: u64 = match self.data {
            InputData::Skip => 0 << 2,
            InputData::Player(PlayerInput {
                button,
                hold,
                player_2,
            }) => ((button as u64) << 2) | hold as u64 | ((player_2 as u64) << 1),
            InputData::Restart => 4 << 2,
            InputData::RestartFull => 5 << 2,
            InputData::Death => 6 << 2,
            InputData::TPS(_) => 7 << 2,
        };

        state | (self.delta << 5)
    }

    pub(crate) const fn required_bytes(&self) -> u8 {
        if let InputData::TPS(_) = self.data {
            return 8;
        }

        let state = self.to_state();
        match state {
            0..0x100 => 1,
            0x100..0x10000 => 2,
            0x10000..0x100000000 => 4,
            _ => 8,
        }
    }

    pub(crate) fn write<W: Write>(&self, writer: &mut W, byte_size: u64) -> Result<(), InputError> {
        writer.write_all(&self.to_state().to_le_bytes()[0..byte_size as usize])?;
        if let InputData::TPS(tps) = self.data {
            writer.write_all(&tps.to_le_bytes())?;
        }

        Ok(())
    }
}
