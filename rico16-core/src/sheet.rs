//! SPEC §7, §6.9, §8.2 — the sprite sheet: 256×256 bytes, 256 tiles of 16×16.

use crate::error::CoreError;
use crate::framebuffer::Framebuffer;
use crate::limits::{SHEET_LEN, SHEET_SIZE, TILE_SIZE, TILES_PER_ROW};

/// SPEC §7 — the sprite sheet, one byte per pixel.
#[derive(Clone)]
pub struct Sheet {
    pixels: Box<[u8; SHEET_LEN]>,
}

impl Default for Sheet {
    fn default() -> Self {
        Self::new()
    }
}

impl Sheet {
    /// A zero-filled sheet.
    pub fn new() -> Self {
        Self {
            pixels: Box::new([0u8; SHEET_LEN]),
        }
    }

    /// SPEC §13.1 — a sheet from the cart's 65,536 bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CoreError> {
        if bytes.len() != SHEET_LEN {
            return Err(CoreError::WrongLength {
                what: "sprite sheet",
                expected: SHEET_LEN,
                found: bytes.len(),
            });
        }
        let mut pixels = Box::new([0u8; SHEET_LEN]);
        pixels.copy_from_slice(bytes);
        Ok(Self { pixels })
    }

    /// The raw sheet bytes.
    pub fn bytes(&self) -> &[u8; SHEET_LEN] {
        &self.pixels
    }

    /// The raw sheet bytes, mutable.
    pub fn bytes_mut(&mut self) -> &mut [u8; SHEET_LEN] {
        &mut self.pixels
    }

    /// SPEC §6.9 — `sprite::get_pixel`. `px`, `py` must be in 0–15.
    pub fn get_pixel(&self, id: u8, px: u8, py: u8) -> u8 {
        self.pixels[Self::offset(id, px, py)]
    }

    /// SPEC §6.9 — `sprite::set_pixel`. `px`, `py` must be in 0–15.
    pub fn set_pixel(&mut self, id: u8, px: u8, py: u8, color: u8) {
        let at = Self::offset(id, px, py);
        self.pixels[at] = color;
    }

    /// SPEC §6.9, §9.3 — `sprite::draw`, 16×16, clipped, skipping index 0.
    pub fn draw(&self, fb: &mut Framebuffer, id: u8, x: i64, y: i64, flip_x: bool, flip_y: bool) {
        let base_x = (id as usize % TILES_PER_ROW) * TILE_SIZE;
        let base_y = (id as usize / TILES_PER_ROW) * TILE_SIZE;
        let _ = self.draw_region(
            fb,
            base_x as i64,
            base_y as i64,
            TILE_SIZE as i64,
            TILE_SIZE as i64,
            x,
            y,
            flip_x,
            flip_y,
        );
    }

    /// SPEC §6.9, §9.3 — `sprite::draw_region`, source in sheet pixels, no
    /// scaling, index-0 key. `w` or `h` ≤ 0 is a no-op; a source rectangle that
    /// leaves the 256×256 sheet is an error.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_region(
        &self,
        fb: &mut Framebuffer,
        sx: i64,
        sy: i64,
        w: i64,
        h: i64,
        dx: i64,
        dy: i64,
        flip_x: bool,
        flip_y: bool,
    ) -> Result<(), CoreError> {
        if w <= 0 || h <= 0 {
            return Ok(());
        }
        if sx < 0 || sy < 0 || sx + w > SHEET_SIZE as i64 || sy + h > SHEET_SIZE as i64 {
            return Err(CoreError::RegionOutOfSheet);
        }
        for j in 0..h {
            for i in 0..w {
                let src_x = sx + if flip_x { w - 1 - i } else { i };
                let src_y = sy + if flip_y { h - 1 - j } else { j };
                let value = self.pixels[src_y as usize * SHEET_SIZE + src_x as usize];
                if value != 0 {
                    fb.set(dx + i, dy + j, value);
                }
            }
        }
        Ok(())
    }

    fn offset(id: u8, px: u8, py: u8) -> usize {
        debug_assert!(px < TILE_SIZE as u8 && py < TILE_SIZE as u8);
        let x = (id as usize % TILES_PER_ROW) * TILE_SIZE + px as usize;
        let y = (id as usize / TILES_PER_ROW) * TILE_SIZE + py as usize;
        y * SHEET_SIZE + x
    }
}
