// this generic input format takes from v2 because (i believe)
// it is the most idiomatic way to represent a replay action

use std::fmt::Display;

use crate::v2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Player {
    Player1,
    Player2,
}

impl Display for Player {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Player::Player1 => write!(f, "p1"),
            Player::Player2 => write!(f, "p2"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerAction {
    Jump,
    Left,
    Right,
}

impl Display for PlayerAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlayerAction::Jump => write!(f, "jump"),
            PlayerAction::Left => write!(f, "left"),
            PlayerAction::Right => write!(f, "right"),
        }
    }
}

impl PlayerAction {
    /// Convert a [PlayerAction] to [v2::PlayerInput](crate::v2::PlayerInput)'s `button` field.
    pub fn to_v2_button(&self) -> v2::Button {
        match self {
            PlayerAction::Jump => v2::Button::Jump,
            PlayerAction::Left => v2::Button::Left,
            PlayerAction::Right => v2::Button::Right,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerInput {
    pub action: PlayerAction,
    /// Indicates whether the action is a "hold" or "release"
    ///
    /// `true` = "hold" \
    /// `false` = "release"
    pub down: bool,
    pub player: Player,
}

/// Data specifying what an action does.
#[derive(Debug, Clone, PartialEq)]
pub enum ActionData {
    /// This input is an in-game player button push.
    Player(PlayerInput),
    /// This input restarts the level. (`PlayLayer::resetLevel`)
    Restart { seed: u64 },
    /// This input restarts the level, fully. (`PlayLayer::fullReset`)
    RestartFull { seed: u64 },
    /// This input signals that the player may die on any subsequent frame.
    Death { seed: u64 },
    /// This input changes the current tps of the replay.
    TPS(f64),
    /// This input indicates a (now removed) bug which allowed you to
    /// place a checkpoint in normal mode. It is kept for compatibility.
    Bugpoint,
}

impl Display for ActionData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Player(p) => write!(
                f,
                "player action: {}, down: {}, player: {}",
                p.action, p.down, p.player
            ),
            Self::Death { seed } => write!(f, "death (seed {})", seed),
            Self::Restart { seed } => write!(f, "restart (seed {})", seed),
            Self::RestartFull { seed } => write!(f, "full restart (seed {})", seed),
            Self::TPS(tps) => write!(f, "tps: {}", tps),
            Self::Bugpoint => write!(f, "bugpoint"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Action {
    pub frame: u64,
    pub data: ActionData,
}

impl Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "frame: {}, input: ({})", self.frame, self.data)
    }
}
