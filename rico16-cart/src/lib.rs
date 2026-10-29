//! The cart format of the rico-16 fantasy console: `cart.toml`, the binary
//! cart and PNG packing (SPEC §10, §13).
//!
//! This crate never knows about Rhai execution or windows; it turns a project
//! into bytes and bytes back into a project (AGENTS.md).

#![forbid(unsafe_code)]

pub mod error;
pub mod manifest;

pub use error::CartError;
pub use manifest::{CartMeta, CartToml, DEFAULT_ENTRY, FORMAT_V1, SpritePlacement};
