//! The indexed framebuffer. SPEC §3.
//!
//! One byte per pixel, row-major, stride 320. Values are palette indices, never
//! colours: the palette is applied once, at presentation time ([`Framebuffer::expand_into`]).
//!
//! The buffer is deliberately not cleared between frames (SPEC §3.4). `new()`
//! zeroes it, and that is the only automatic fill.

use crate::limits::{FRAMEBUFFER_LEN, SCREEN_HEIGHT, SCREEN_WIDTH};
use crate::palette::Palette;

/// A 320×240 surface of palette indices. SPEC §3.2.
#[derive(Clone, PartialEq, Eq)]
pub struct Framebuffer {
    indices: Box<[u8; FRAMEBUFFER_LEN]>,
}

impl Framebuffer {
    /// An all-zero framebuffer: every pixel is index 0. SPEC §3.5.
    pub fn new() -> Self {
        Self {
            indices: Box::new([0; FRAMEBUFFER_LEN]),
        }
    }

    /// Fill every pixel with `color`. SPEC §3.3.
    pub fn clear(&mut self, color: u8) {
        self.indices.fill(color);
    }

    /// The raw index buffer, exactly 76,800 bytes. SPEC §3.2.
    pub fn indices(&self) -> &[u8] {
        &*self.indices
    }

    /// Mutable access for blits and tests.
    pub fn indices_mut(&mut self) -> &mut [u8] {
        &mut *self.indices
    }

    /// Read a pixel, or `None` outside the screen. SPEC §3.1.
    pub fn get(&self, x: i32, y: i32) -> Option<u8> {
        if !in_bounds(x, y) {
            return None;
        }
        Some(self.indices[index(x, y)])
    }

    /// Write one pixel, ignoring positions outside the screen. SPEC §6.1.
    pub fn pixel(&mut self, x: i32, y: i32, color: u8) {
        if in_bounds(x, y) {
            self.indices[index(x, y)] = color;
        }
    }

    /// Fill `x0..x1` on row `y`, half-open and clipped. SPEC §6.1, §6.6.
    ///
    /// Used by the primitives, which have already clipped to the screen; the
    /// clamps here keep the function safe for direct callers too.
    pub fn fill_span(&mut self, y: i32, x0: i32, x1: i32, color: u8) {
        if !(0..SCREEN_HEIGHT as i32).contains(&y) {
            return;
        }
        let x0 = x0.max(0).min(SCREEN_WIDTH as i32) as usize;
        let x1 = x1.max(0).min(SCREEN_WIDTH as i32) as usize;
        if x0 >= x1 {
            return;
        }
        let row = y as usize * SCREEN_WIDTH;
        self.indices[row + x0..row + x1].fill(color);
    }

    /// Fill a half-open rectangle, clipped. SPEC §6.1, §6.6.
    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: u8) {
        if w <= 0 || h <= 0 {
            return;
        }
        // i64 keeps `x + w` from overflowing for extreme arguments.
        let x0 = (x as i64).max(0);
        let y0 = (y as i64).max(0);
        let x1 = (x as i64 + w as i64).min(SCREEN_WIDTH as i64);
        let y1 = (y as i64 + h as i64).min(SCREEN_HEIGHT as i64);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        for row in y0..y1 {
            self.fill_span(row as i32, x0 as i32, x1 as i32, color);
        }
    }

    /// Expand the whole framebuffer through `palette` into RGBA8. SPEC §3.6.
    ///
    /// `rgba` must be exactly `FRAMEBUFFER_LEN * 4` bytes; alpha is always 255.
    /// This is the only place an index becomes a colour.
    pub fn expand_into(&self, palette: &Palette, rgba: &mut [u8]) {
        assert_eq!(
            rgba.len(),
            FRAMEBUFFER_LEN * 4,
            "rgba buffer must hold one RGBA8 pixel per framebuffer pixel"
        );

        // Resolve the 256 entries once per frame instead of per pixel. The
        // table is 1 KiB and replaces up to 300k bit operations.
        let table = palette.rgba8_table();
        for (index, pixel) in self.indices.iter().zip(rgba.chunks_exact_mut(4)) {
            pixel.copy_from_slice(&table[*index as usize]);
        }
    }
}

impl Default for Framebuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Framebuffer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // A 76,800-byte array is not printable; the size is the useful fact.
        formatter
            .debug_struct("Framebuffer")
            .field("width", &SCREEN_WIDTH)
            .field("height", &SCREEN_HEIGHT)
            .finish()
    }
}

fn in_bounds(x: i32, y: i32) -> bool {
    (0..SCREEN_WIDTH as i32).contains(&x) && (0..SCREEN_HEIGHT as i32).contains(&y)
}

fn index(x: i32, y: i32) -> usize {
    y as usize * SCREEN_WIDTH + x as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette;

    #[test]
    fn a_new_framebuffer_is_all_zero() {
        let framebuffer = Framebuffer::new();
        assert_eq!(framebuffer.indices().len(), FRAMEBUFFER_LEN);
        assert!(framebuffer.indices().iter().all(|index| *index == 0));
    }

    #[test]
    fn clear_fills_every_pixel() {
        let mut framebuffer = Framebuffer::new();
        framebuffer.clear(7);
        assert!(framebuffer.indices().iter().all(|index| *index == 7));
    }

    #[test]
    fn pixels_outside_the_screen_are_ignored() {
        let mut framebuffer = Framebuffer::new();
        for (x, y) in [
            (-1, 0),
            (0, -1),
            (SCREEN_WIDTH as i32, 0),
            (0, SCREEN_HEIGHT as i32),
            (i32::MIN, i32::MIN),
            (i32::MAX, i32::MAX),
        ] {
            framebuffer.pixel(x, y, 9);
        }
        assert!(framebuffer.indices().iter().all(|index| *index == 0));
        assert_eq!(framebuffer.get(-1, 0), None);
        assert_eq!(framebuffer.get(0, i32::MAX), None);
    }

    #[test]
    fn the_four_corners_are_addressable() {
        let mut framebuffer = Framebuffer::new();
        framebuffer.pixel(0, 0, 1);
        framebuffer.pixel(SCREEN_WIDTH as i32 - 1, 0, 2);
        framebuffer.pixel(0, SCREEN_HEIGHT as i32 - 1, 3);
        framebuffer.pixel(SCREEN_WIDTH as i32 - 1, SCREEN_HEIGHT as i32 - 1, 4);
        assert_eq!(framebuffer.get(0, 0), Some(1));
        assert_eq!(framebuffer.get(SCREEN_WIDTH as i32 - 1, 0), Some(2));
        assert_eq!(framebuffer.get(0, SCREEN_HEIGHT as i32 - 1), Some(3));
        assert_eq!(
            framebuffer.get(SCREEN_WIDTH as i32 - 1, SCREEN_HEIGHT as i32 - 1),
            Some(4)
        );
    }

    #[test]
    fn fill_rect_clips_at_all_four_edges() {
        let mut framebuffer = Framebuffer::new();
        framebuffer.fill_rect(-10, -10, 20, 20, 5);
        assert_eq!(framebuffer.get(0, 0), Some(5));
        assert_eq!(framebuffer.get(9, 9), Some(5));
        assert_eq!(framebuffer.get(10, 10), Some(0));

        framebuffer.clear(0);
        framebuffer.fill_rect(SCREEN_WIDTH as i32 - 5, SCREEN_HEIGHT as i32 - 5, 20, 20, 6);
        assert_eq!(
            framebuffer.get(SCREEN_WIDTH as i32 - 1, SCREEN_HEIGHT as i32 - 1),
            Some(6)
        );
        assert_eq!(
            framebuffer.get(SCREEN_WIDTH as i32 - 6, SCREEN_HEIGHT as i32 - 1),
            Some(0)
        );
    }

    #[test]
    fn non_positive_rectangles_draw_nothing() {
        let mut framebuffer = Framebuffer::new();
        framebuffer.fill_rect(10, 10, 0, 10, 4);
        framebuffer.fill_rect(10, 10, 10, 0, 4);
        framebuffer.fill_rect(10, 10, -5, 10, 4);
        framebuffer.fill_rect(10, 10, 10, -5, 4);
        assert!(framebuffer.indices().iter().all(|index| *index == 0));
    }

    #[test]
    fn expansion_matches_the_palette_for_every_index() {
        let palette = palette::Palette::new();
        let mut framebuffer = Framebuffer::new();
        for index in 0..=255u8 {
            framebuffer.pixel(index as i32, 0, index);
        }

        let mut rgba = vec![0u8; FRAMEBUFFER_LEN * 4];
        framebuffer.expand_into(&palette, &mut rgba);

        for index in 0..=255u8 {
            let pixel = &rgba[index as usize * 4..index as usize * 4 + 4];
            let (r, g, b) = palette::to_rgb8(palette.entry(index));
            assert_eq!(pixel, [r, g, b, 255], "index {index}");
        }
    }
}
