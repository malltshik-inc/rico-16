//! SPEC §3.2, §3.4, §3.6 — the single indexed framebuffer.
//!
//! One surface, one byte per pixel, row-major, stride 320. There is no
//! compositing and no second surface (SPEC §3.3).

use crate::limits::{FRAMEBUFFER_LEN, SCREEN_HEIGHT, SCREEN_WIDTH};
use crate::palette::{Palette, expand5};

/// SPEC §3.2 — the framebuffer: 76,800 palette indices, row-major.
#[derive(Clone)]
pub struct Framebuffer {
    pixels: Box<[u8; FRAMEBUFFER_LEN]>,
}

impl Default for Framebuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl Framebuffer {
    /// SPEC §3.5 — a new framebuffer is all zeros (index 0).
    pub fn new() -> Self {
        Self {
            pixels: Box::new([0u8; FRAMEBUFFER_LEN]),
        }
    }

    /// SPEC §3.2 — the raw framebuffer bytes.
    pub fn bytes(&self) -> &[u8; FRAMEBUFFER_LEN] {
        &self.pixels
    }

    /// SPEC §3.2 — the raw framebuffer bytes, mutable.
    pub fn bytes_mut(&mut self) -> &mut [u8; FRAMEBUFFER_LEN] {
        &mut self.pixels
    }

    /// SPEC §3.2 — overwrite the whole framebuffer from raw bytes.
    pub fn set_bytes(&mut self, bytes: &[u8; FRAMEBUFFER_LEN]) {
        *self.pixels = *bytes;
    }

    /// SPEC §4.3 — `gfx::clear`: fill every pixel with `color` literally.
    pub fn clear(&mut self, color: u8) {
        self.pixels.fill(color);
    }

    /// SPEC §6.1 — read a pixel. Out-of-bounds reads return `None`.
    pub fn get(&self, x: i64, y: i64) -> Option<u8> {
        if x < 0 || y < 0 || x >= SCREEN_WIDTH as i64 || y >= SCREEN_HEIGHT as i64 {
            return None;
        }
        Some(self.pixels[y as usize * SCREEN_WIDTH + x as usize])
    }

    /// SPEC §6.1 — write a pixel, clipped without panicking. A fully
    /// off-screen write is a no-op.
    pub fn set(&mut self, x: i64, y: i64, color: u8) {
        if x < 0 || y < 0 || x >= SCREEN_WIDTH as i64 || y >= SCREEN_HEIGHT as i64 {
            return;
        }
        self.pixels[y as usize * SCREEN_WIDTH + x as usize] = color;
    }

    /// SPEC §3.6 — expand the framebuffer through `palette` to RGBA8, exact,
    /// alpha 255. `out` must be exactly `FRAMEBUFFER_LEN * 4` bytes.
    pub fn expand_into(&self, palette: &Palette, out: &mut [u8]) {
        debug_assert_eq!(out.len(), FRAMEBUFFER_LEN * 4);
        for (i, &index) in self.pixels.iter().enumerate() {
            let word = palette.word(index as usize);
            out[i * 4] = expand5(word & 0x1F);
            out[i * 4 + 1] = expand5((word >> 5) & 0x1F);
            out[i * 4 + 2] = expand5((word >> 10) & 0x1F);
            out[i * 4 + 3] = 0xFF;
        }
    }
}
