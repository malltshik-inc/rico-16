//! Fixed resources and the data model of the rico-16 fantasy console.
//!
//! This crate is deliberately pure: no filesystem, no clock, no threads, no
//! environment, no OS randomness (see `AGENTS.md`). Everything here is
//! deterministic for a given seed and input, which is what makes carts
//! reproducible across platforms.

#![forbid(unsafe_code)]

pub mod blit;
pub mod font;
pub mod framebuffer;
pub mod gfx;
pub mod input;
pub mod limits;
pub mod map;
pub mod palette;
pub mod rng;
pub mod sheet;
