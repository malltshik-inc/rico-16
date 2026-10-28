//! The player: one window, one texture, sixty updates per second.
//!
//! The present path lives behind [`Presenter`] so that the blit backend can be
//! swapped without touching anything else (`AGENTS.md`, dependency policy).
//! Behaviour is defined by `docs/SPEC.md` §3 and §12.
//!
//! [`run`] loads a `.r16` cart, compiles it and runs it in the window. [`Demo`]
//! is the M1 test pattern, kept as a renderer smoke test.

#![forbid(unsafe_code)]

mod app;
pub mod cart;
pub mod demo;
mod present;

pub use app::{RunError, run};
pub use cart::{CartRunner, Quit};
pub use demo::Demo;
pub use present::{PixelsPresenter, Presenter};

/// Everything that can stop the player before a cart does.
///
/// Messages are lowercase and carry no `Error:` prefix (`AGENTS.md`).
#[derive(Debug, thiserror::Error)]
pub enum PlayerError {
    /// The event loop could not start or run.
    #[error("window: {0}")]
    EventLoop(#[from] winit::error::EventLoopError),

    /// A window could not be created.
    #[error("window: {0}")]
    Window(#[from] winit::error::OsError),

    /// The GPU present path failed.
    #[error("present: {0}")]
    Pixels(#[from] pixels::Error),

    /// A surface or texture could not be resized.
    #[error("present: {0}")]
    Texture(#[from] pixels::TextureError),
}
