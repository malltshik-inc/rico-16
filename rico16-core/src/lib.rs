//! The pure core of the rico-16 fantasy console.
//!
//! Everything here is fixed by `docs/SPEC.md`: the framebuffer, the palette,
//! the drawing primitives, the sprite sheet and tilemap data, input state and
//! the RNG. Nothing here touches the filesystem, the clock, threads, the
//! environment or the OS RNG — the crate must stay `wasm32`-clean (AGENTS.md).

#![forbid(unsafe_code)]

pub mod error;
pub mod font;
pub mod framebuffer;
pub mod gfx;
pub mod input;
pub mod limits;
pub mod map;
pub mod palette;
pub mod rng;
pub mod sheet;

pub use error::CoreError;
pub use framebuffer::Framebuffer;
pub use input::{Button, Input};
pub use map::Tilemap;
pub use palette::Palette;
pub use rng::Rng;
pub use sheet::Sheet;
