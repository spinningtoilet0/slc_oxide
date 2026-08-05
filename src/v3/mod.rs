//! SLC3 (Version 3) replay format implementation.
//!
//! This module provides support for reading and writing SLC3 format replays,
//! which use an atom-based structure with run-length encoding for efficient storage.

pub mod atom;
pub mod metadata;
pub mod replay;
pub mod section;

pub use atom::{Action, ActionType};
pub use metadata::Metadata;
pub use replay::Replay;
