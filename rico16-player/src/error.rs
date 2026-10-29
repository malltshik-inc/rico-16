//! Player errors.

use std::path::PathBuf;

/// Anything that can go wrong loading or running a cart in the player.
#[derive(Debug, thiserror::Error)]
pub enum PlayerError {
    /// Building or reading the cart failed.
    #[error("{0}")]
    Cart(#[from] rico16_cart::CartError),

    /// The cart's script failed.
    #[error("{0}")]
    Script(#[from] rico16_script::ScriptError),

    /// A file could not be read.
    #[error("cannot read {path}: {source}")]
    Io {
        /// The path involved.
        path: PathBuf,
        /// The underlying OS error.
        #[source]
        source: std::io::Error,
    },

    /// The window or GPU surface could not be created.
    #[error("window or surface: {0}")]
    Window(String),

    /// Presenting a frame failed.
    #[error("present: {0}")]
    Present(String),
}
