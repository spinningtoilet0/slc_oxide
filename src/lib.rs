//! Rust port of the slc replay format for Geometry Dash.
//!
//! Provides a compact and fast replay format to use
//! for bots and converters. Silicate's official format.

pub mod input;
pub mod meta;
pub mod replay;
pub mod v2;
pub mod v3;

#[allow(deprecated)]
pub use input::PlayerData;
pub use input::{Input, InputData, PlayerInput};
pub use meta::Meta;
pub use replay::{Replay, ReplayError};
