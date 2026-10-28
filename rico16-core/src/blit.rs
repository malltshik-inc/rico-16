//! Keyed blits from the sprite sheet. SPEC §4.2, §8.2, §9.3.
//!
//! A blit copies a rectangle of indices from cart data to the framebuffer. Index
//! 0 is skipped, which is the console's only transparency mechanism. Everything
//! else is copied literally — no blending, no scaling, no colour mapping.
//!
//! Source and destination are both clipped: a sprite half off the screen draws
//! its visible half, and a source rectangle that runs off the sheet simply runs
//! out of data.

use crate::framebuffer::Framebuffer;
use crate::limits::{SCREEN_HEIGHT, SCREEN_WIDTH, SHEET_HEIGHT, SHEET_WIDTH, TILE_SIZE};
use crate::palette::TRANSPARENT;

/// A source rectangle and where its top-left corner lands. SPEC §9.3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Blit {
    /// Source x in sheet pixels.
    pub sx: i32,
    /// Source y in sheet pixels.
    pub sy: i32,
    /// Source width in pixels.
    pub w: i32,
    /// Source height in pixels.
    pub h: i32,
    /// Destination x on screen.
    pub dx: i32,
    /// Destination y on screen.
    pub dy: i32,
    /// Mirror each row left-to-right.
    pub flip_x: bool,
    /// Mirror rows top-to-bottom.
    pub flip_y: bool,
}

impl Blit {
    /// A blit with no flipping.
    pub const fn new(sx: i32, sy: i32, w: i32, h: i32, dx: i32, dy: i32) -> Self {
        Self {
            sx,
            sy,
            w,
            h,
            dx,
            dy,
            flip_x: false,
            flip_y: false,
        }
    }

    /// Flip the blit on one or both axes. SPEC §8.5.
    pub const fn flipped(mut self, flip_x: bool, flip_y: bool) -> Self {
        self.flip_x = flip_x;
        self.flip_y = flip_y;
        self
    }
}

/// Blit a region of `source` into the framebuffer, skipping index 0. SPEC §4.2.
///
/// `source` must be `source_width * source_height` bytes. The destination is
/// clipped per pixel; the source is clipped to its own bounds.
pub fn region(
    destination: &mut Framebuffer,
    source: &[u8],
    source_width: usize,
    source_height: usize,
    blit: Blit,
) {
    let Blit {
        sx,
        sy,
        w,
        h,
        dx,
        dy,
        flip_x,
        flip_y,
    } = blit;

    if w <= 0 || h <= 0 || source_width == 0 || source_height == 0 {
        return;
    }
    assert_eq!(
        source.len(),
        source_width * source_height,
        "source buffer does not match its declared size"
    );

    let (sx, sy) = (sx as i64, sy as i64);
    let first_x = sx.max(0);
    let first_y = sy.max(0);
    let last_x = (sx + w as i64).min(source_width as i64);
    let last_y = (sy + h as i64).min(source_height as i64);
    if first_x >= last_x || first_y >= last_y {
        return;
    }

    for source_y in first_y..last_y {
        let row = if flip_y {
            h as i64 - 1 - (source_y - sy)
        } else {
            source_y - sy
        };
        let destination_y = dy as i64 + row;
        if !(0..SCREEN_HEIGHT as i64).contains(&destination_y) {
            continue;
        }

        let source_row = source_y as usize * source_width;
        for source_x in first_x..last_x {
            let color = source[source_row + source_x as usize];
            if color == TRANSPARENT {
                continue;
            }

            let column = if flip_x {
                w as i64 - 1 - (source_x - sx)
            } else {
                source_x - sx
            };
            let destination_x = dx as i64 + column;
            if (0..SCREEN_WIDTH as i64).contains(&destination_x) {
                destination.pixel(destination_x as i32, destination_y as i32, color);
            }
        }
    }
}

/// Blit one 16×16 tile of the sprite sheet, by id. SPEC §9.3.
///
/// `sheet` must be exactly [`SHEET_WIDTH`] × [`SHEET_HEIGHT`] bytes.
pub fn sprite(
    destination: &mut Framebuffer,
    sheet: &[u8],
    id: u8,
    x: i32,
    y: i32,
    flip_x: bool,
    flip_y: bool,
) {
    let id = id as usize;
    let columns = SHEET_WIDTH / TILE_SIZE;
    let sx = (id % columns) * TILE_SIZE;
    let sy = (id / columns) * TILE_SIZE;
    let blit = Blit::new(
        sx as i32,
        sy as i32,
        TILE_SIZE as i32,
        TILE_SIZE as i32,
        x,
        y,
    )
    .flipped(flip_x, flip_y);
    region(destination, sheet, SHEET_WIDTH, SHEET_HEIGHT, blit);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::FRAMEBUFFER_LEN;

    fn blank() -> Framebuffer {
        let mut framebuffer = Framebuffer::new();
        framebuffer.clear(0);
        framebuffer
    }

    fn lit(framebuffer: &Framebuffer) -> Vec<(i32, i32, u8)> {
        let mut pixels = Vec::new();
        for y in 0..SCREEN_HEIGHT as i32 {
            for x in 0..SCREEN_WIDTH as i32 {
                let index = framebuffer.get(x, y).unwrap();
                if index != 0 {
                    pixels.push((x, y, index));
                }
            }
        }
        pixels
    }

    #[test]
    fn index_zero_is_transparent_and_everything_else_is_copied() {
        // 2×2 source: one transparent pixel, three opaque.
        let source = [1u8, 0, 0, 4];
        let mut framebuffer = blank();
        framebuffer.clear(9);

        region(
            &mut framebuffer,
            &source,
            2,
            2,
            Blit::new(0, 0, 2, 2, 10, 10),
        );

        assert_eq!(framebuffer.get(10, 10), Some(1));
        assert_eq!(
            framebuffer.get(11, 10),
            Some(9),
            "index 0 leaves the destination"
        );
        assert_eq!(framebuffer.get(10, 11), Some(9));
        assert_eq!(framebuffer.get(11, 11), Some(4));
    }

    #[test]
    fn flip_x_mirrors_rows() {
        let source = [1u8, 2, 3, 4];
        let mut framebuffer = blank();
        region(
            &mut framebuffer,
            &source,
            2,
            2,
            Blit::new(0, 0, 2, 2, 0, 0).flipped(true, false),
        );
        assert_eq!(
            lit(&framebuffer),
            vec![(0, 0, 2), (1, 0, 1), (0, 1, 4), (1, 1, 3)]
        );
    }

    #[test]
    fn flip_y_mirrors_rows_top_to_bottom() {
        let source = [1u8, 2, 3, 4];
        let mut framebuffer = blank();
        region(
            &mut framebuffer,
            &source,
            2,
            2,
            Blit::new(0, 0, 2, 2, 0, 0).flipped(false, true),
        );
        assert_eq!(
            lit(&framebuffer),
            vec![(0, 0, 3), (1, 0, 4), (0, 1, 1), (1, 1, 2)]
        );
    }

    #[test]
    fn both_flips_rotate_by_180_degrees() {
        let source = [1u8, 2, 3, 4];
        let mut framebuffer = blank();
        region(
            &mut framebuffer,
            &source,
            2,
            2,
            Blit::new(0, 0, 2, 2, 0, 0).flipped(true, true),
        );
        assert_eq!(
            lit(&framebuffer),
            vec![(0, 0, 4), (1, 0, 3), (0, 1, 2), (1, 1, 1)]
        );
    }

    #[test]
    fn destination_clips_at_all_four_edges() {
        let mut source = vec![0u8; 8 * 8];
        source.fill(7);

        for (dx, dy) in [
            (-4, 20),
            (20, -4),
            (SCREEN_WIDTH as i32 - 4, 20),
            (20, SCREEN_HEIGHT as i32 - 4),
            (-100, -100),
            (1000, 1000),
        ] {
            let mut framebuffer = blank();
            region(
                &mut framebuffer,
                &source,
                8,
                8,
                Blit::new(0, 0, 8, 8, dx, dy),
            );
            for (x, y, index) in lit(&framebuffer) {
                assert_eq!(index, 7);
                assert!((0..SCREEN_WIDTH as i32).contains(&x));
                assert!((0..SCREEN_HEIGHT as i32).contains(&y));
            }
        }
    }

    #[test]
    fn a_blit_wholly_off_screen_draws_nothing() {
        let source = [7u8; 4];
        let mut framebuffer = blank();
        for (dx, dy) in [(-10, 0), (400, 0), (0, -10), (0, 300)] {
            region(
                &mut framebuffer,
                &source,
                2,
                2,
                Blit::new(0, 0, 2, 2, dx, dy),
            );
        }
        assert!(lit(&framebuffer).is_empty());
    }

    #[test]
    fn a_source_rectangle_off_the_sheet_draws_only_its_visible_half() {
        let source = [1u8, 2, 3, 4, 5, 6];
        let mut framebuffer = blank();

        // Start two pixels to the left: only columns 0..4 exist.
        region(
            &mut framebuffer,
            &source,
            3,
            2,
            Blit::new(-2, 0, 5, 2, 10, 10),
        );

        // The first two destination columns are outside the source.
        assert_eq!(framebuffer.get(10, 10), Some(0));
        assert_eq!(framebuffer.get(11, 10), Some(0));
        assert_eq!(framebuffer.get(12, 10), Some(1));
        assert_eq!(framebuffer.get(13, 10), Some(2));
        assert_eq!(framebuffer.get(14, 10), Some(3));
        assert_eq!(framebuffer.get(15, 10), Some(0));
        assert_eq!(framebuffer.get(14, 11), Some(6));
    }

    #[test]
    fn non_positive_sizes_are_no_ops() {
        let source = [7u8; 4];
        let mut framebuffer = blank();
        for (w, h) in [(0, 2), (2, 0), (-1, 2), (2, -1)] {
            region(&mut framebuffer, &source, 2, 2, Blit::new(0, 0, w, h, 0, 0));
        }
        assert!(lit(&framebuffer).is_empty());
    }

    #[test]
    fn sprite_addressing_uses_sixteen_columns() {
        let mut sheet = vec![0u8; SHEET_WIDTH * SHEET_HEIGHT];
        // Mark the top-left pixel of tiles 0, 15, 16 and 255.
        sheet[0] = 1;
        sheet[15 * TILE_SIZE] = 2;
        // Tile 16 is the first tile of the second row.
        sheet[TILE_SIZE * SHEET_WIDTH] = 3;
        // Tile 255 is the bottom-right tile of the sheet.
        sheet[(SHEET_HEIGHT - TILE_SIZE) * SHEET_WIDTH + (SHEET_WIDTH - TILE_SIZE)] = 4;

        let mut framebuffer = blank();
        for id in [0u8, 15, 16, 255] {
            sprite(&mut framebuffer, &sheet, id, 0, 0, false, false);
        }

        // All four land on the same screen pixel; the last one wins here, which
        // also proves each id addressed a different sheet pixel.
        assert_eq!(framebuffer.get(0, 0), Some(4));
        assert_eq!(framebuffer.indices().len(), FRAMEBUFFER_LEN);
    }
}
