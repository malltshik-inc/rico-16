//! The cart format of the rico-16 fantasy console.
//!
//! A cart carries everything a game needs: script, sprite sheet, tilemap and
//! palette. It exists in two forms, defined in `docs/SPEC.md`:
//!
//! - `.r16`, the text form you edit and review (§10);
//! - `.r16.png`, the binary form you publish, whose pixels are both cover art
//!   and payload (§11).
//!
//! This crate knows nothing about Rhai execution or windows.

#![forbid(unsafe_code)]

pub mod text;

pub use text::{Cart, Meta, ParseError};
