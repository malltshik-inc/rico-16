//! The Rhai scripting layer of the rico-16 console.
//!
//! This crate binds a cart's script to the frozen data model in `rico16-core`:
//! it owns the engine, the per-cart memory (framebuffer, palette, sheet, map,
//! RNG, input, frame counter) and the `gfx::` / `sprite::` / `tilemap::` /
//! `input::` / `system::` / `audio::` modules. The API surface is
//! `docs/SPEC.md` §9; every function a cart can name is registered here.
//!
//! Nothing here draws on its own and nothing reads the filesystem; the host
//! hands in cart memory and takes rendered frames out.

#![forbid(unsafe_code)]

mod api;
mod error;
mod runtime;

pub use error::ScriptError;
pub use runtime::{CartAssets, Quit, Runtime};
