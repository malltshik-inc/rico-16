//! The cart format of the rico-16 fantasy console: `cart.toml`, the binary
//! cart and PNG packing (SPEC §10, §13).
//!
//! This crate never knows about Rhai execution or windows; it turns a project
//! into bytes and bytes back into a project (AGENTS.md).

#![forbid(unsafe_code)]

pub mod binary;
pub mod build;
pub mod crc;
pub mod error;
pub mod manifest;
pub mod png;

pub use binary::{BinaryCart, FORMAT_VERSION, MAGIC, Module, metadata_from_manifest};
pub use build::{BuildOptions, BuildOutput, assemble_project, build_project};
pub use crc::crc32;
pub use error::CartError;
pub use manifest::{CartMeta, CartToml, DEFAULT_ENTRY, FORMAT_V1, SpritePlacement};
pub use png::{Art, BLOB_MAGIC, BLOB_VERSION, Variant};
