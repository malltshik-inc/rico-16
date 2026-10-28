//! The present path, behind a trait. SPEC §3.6, §3.7, §12.1.
//!
//! `PixelsPresenter` is the only place that knows about `pixels` and `wgpu`.
//! The player hands it an RGBA8 frame; swapping the backend means writing
//! another implementation of [`Presenter`] and changing nothing else.
//!
//! The console-side contract is strict: the image is 320×240, scaled by an
//! integer with nearest-neighbour sampling and letterboxed with opaque black.
//! `pixels` does exactly that, so the configuration here is deliberately small.

use std::sync::Arc;

use pixels::{Pixels, SurfaceTexture};
use rico16_core::limits::{FRAMEBUFFER_LEN, SCREEN_HEIGHT, SCREEN_WIDTH};
use winit::dpi::PhysicalSize;
use winit::window::Window;

use crate::PlayerError;

/// A sink for finished RGBA8 frames.
pub trait Presenter {
    /// Upload and show one frame. `rgba` is 320×240×4 bytes, alpha always 255.
    fn present(&mut self, rgba: &[u8]) -> Result<(), PlayerError>;

    /// The drawable area changed. Sizes are physical pixels.
    fn resize(&mut self, width: u32, height: u32) -> Result<(), PlayerError>;
}

/// The GPU present path: one texture, one quad. SPEC §3.6.
pub struct PixelsPresenter {
    pixels: Pixels<'static>,
}

impl PixelsPresenter {
    /// Build the GPU pipeline for an existing window.
    pub fn new(window: Arc<Window>, size: PhysicalSize<u32>) -> Result<Self, PlayerError> {
        // A zero-sized surface is legal during some window transitions; the
        // backend would panic on a zero texture size, so keep it at least 1×1.
        let width = size.width.max(1);
        let height = size.height.max(1);

        let surface = SurfaceTexture::new(width, height, window);
        let mut pixels = Pixels::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32, surface)?;

        // Letterbox bars are opaque black, never the window system's default.
        pixels.clear_color(pixels::wgpu::Color::BLACK);

        Ok(Self { pixels })
    }
}

impl Presenter for PixelsPresenter {
    fn present(&mut self, rgba: &[u8]) -> Result<(), PlayerError> {
        assert_eq!(
            rgba.len(),
            FRAMEBUFFER_LEN * 4,
            "presented frame must be 320x240 RGBA8"
        );
        self.pixels.frame_mut().copy_from_slice(rgba);
        self.pixels.render()?;
        Ok(())
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<(), PlayerError> {
        if width == 0 || height == 0 {
            return Ok(());
        }
        self.pixels.resize_surface(width, height)?;
        Ok(())
    }
}
