//! The Rhai scripting layer of the rico-16 fantasy console.
//!
//! Owns the engine, the per-cart memory (framebuffer, palette, sheet, map, RNG,
//! input, frame counter) and the `gfx::` / `sprite::` / `tilemap::` /
//! `input::` / `system::` / `audio::` / `rnd::` modules of SPEC §9. Nothing here
//! draws on its own and nothing reads the filesystem; the host hands in cart
//! memory and takes rendered frames out.

#![forbid(unsafe_code)]

mod api;
pub mod error;
pub mod imports;
pub mod runtime;

pub use error::ScriptError;
pub use runtime::{Action, CartAssets, Memory, Runtime};
