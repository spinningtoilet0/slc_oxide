// this generic input format takes from v2 because (i believe)
// it is the most idiomatic way to represent a replay action

use std::fmt::Display;

#[derive(Debug, PartialEq, Eq, Clone)]
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

#[derive(Debug, PartialEq, Eq, Clone)]
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
    pub fn to_v2_button(&self) -> u8 {
        match self {
            PlayerAction::Jump => 1,
            PlayerAction::Left => 2,
            PlayerAction::Right => 3,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
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
#[derive(Debug, PartialEq, Clone)]
pub enum ActionData {
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
            Self::Death => write!(f, "death"),
            Self::Restart => write!(f, "restart"),
            Self::RestartFull => write!(f, "full restart"),
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
