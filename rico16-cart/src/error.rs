//! Error types for `rico16-cart`.
//!
//! Every user-facing message is lowercase, has no trailing period and no
//! "Error:" prefix (AGENTS.md).

use std::path::PathBuf;

/// Anything that can go wrong reading, validating, packing or unpacking a cart.
#[derive(Debug, thiserror::Error)]
pub enum CartError {
    /// A file could not be read or written.
    #[error("cannot access {path}: {source}")]
    Io {
        /// The path involved.
        path: PathBuf,
        /// The underlying OS error.
        #[source]
        source: std::io::Error,
    },

    /// `cart.toml` is structurally wrong or violates SPEC §10.2.
    #[error("cart.toml: {0}")]
    Manifest(String),

    /// A PNG could not be decoded or encoded.
    #[error("{path}: {message}")]
    Png {
        /// The PNG involved.
        path: PathBuf,
        /// What went wrong.
        message: String,
    },

    /// An asset violated the placement or colour rules of SPEC §10.2.2–§10.2.4.
    #[error("{0}")]
    Asset(String),

    /// The binary cart is malformed (SPEC §13.1, §13.4).
    #[error("{0}")]
    Format(String),

    /// The payload does not fit the chosen PNG variant (SPEC §7.7, §12.5).
    #[error("payload is {used} bytes, capacity is {capacity}")]
    Capacity {
        /// Payload bytes required.
        used: usize,
        /// Payload bytes available.
        capacity: usize,
    },
}

impl CartError {
    /// An I/O error tagged with the path it happened on.
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        CartError::Io {
            path: path.into(),
            source,
        }
    }
}
