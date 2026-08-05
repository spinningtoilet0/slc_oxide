//! Rust port of the slc replay format for Geometry Dash.
//!
//! Provides a compact and fast replay format to use
//! for bots and converters. Silicate's official format.

pub mod action;
pub mod replay;
pub mod v2;
pub mod v3;

pub use action::{Action, ActionData};
pub use replay::{GenericReplay, Replay, ReplayError};
