//! The ratatui editor of the rico-16 fantasy console (SPEC §16).
//!
//! The terminal only edits and shows text; the game runs in its own window
//! (SPEC §16.4), spawned as a child process.

#![forbid(unsafe_code)]

mod app;
pub mod buffer;
pub mod game;
pub mod highlight;
pub mod tree;

pub use app::{EditorOptions, HELP, HOTKEYS, project_root, run};

/// Anything the editor can fail with.
#[derive(Debug, thiserror::Error)]
pub enum TuiError {
    /// A terminal operation failed.
    #[error("terminal: {0}")]
    Io(#[from] std::io::Error),
}
