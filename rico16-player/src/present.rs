//! SPEC §3.7, §14.2 — the present path.
//!
//! Isolated behind a small trait so `pixels` can be replaced by raw `wgpu`
//! without touching the rest of the player (AGENTS.md).

use std::sync::Arc;

use pixels::{Pixels, SurfaceTexture};
use winit::window::Window;

use rico16_core::limits::{SCREEN_HEIGHT, SCREEN_WIDTH};

use crate::error::PlayerError;

/// A surface the finished 320×240 RGBA frame is presented to.
pub trait Presenter {
    /// SPEC §3.7 — the drawable changed size.
    fn resize(&mut self, width: u32, height: u32) -> Result<(), PlayerError>;
    /// SPEC §14.2 — present exactly one logical frame.
    fn present(&mut self, rgba: &[u8]) -> Result<(), PlayerError>;
}

/// SPEC §3.7, §21 — a `pixels`-backed presenter.
///
/// `pixels` scales the 320×240 texture to the surface at the largest integer
/// ratio that fits, letterboxing the remainder with opaque black, and samples
/// with nearest-neighbour — exactly what SPEC §3.7 requires.
pub struct PixelsPresenter {
    pixels: Pixels<'static>,
}

impl PixelsPresenter {
    /// Build a presenter for `window`, whose drawable is `width`×`height`.
    pub fn new(window: Arc<Window>, width: u32, height: u32) -> Result<Self, PlayerError> {
        let surface = SurfaceTexture::new(width.max(1), height.max(1), window);
        let pixels = Pixels::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32, surface)
            .map_err(|e| PlayerError::Window(e.to_string()))?;
        Ok(Self { pixels })
    }
}

impl Presenter for PixelsPresenter {
    fn resize(&mut self, width: u32, height: u32) -> Result<(), PlayerError> {
        if width == 0 || height == 0 {
            return Ok(());
        }
        self.pixels
            .resize_surface(width, height)
            .map_err(|e| PlayerError::Present(e.to_string()))
    }

    fn present(&mut self, rgba: &[u8]) -> Result<(), PlayerError> {
        let frame = self.pixels.frame_mut();
        if frame.len() != rgba.len() {
            return Err(PlayerError::Present(format!(
                "frame is {} bytes, expected {}",
                rgba.len(),
                frame.len()
            )));
        }
        frame.copy_from_slice(rgba);
        self.pixels
            .render()
            .map_err(|e| PlayerError::Present(e.to_string()))
    }
}
