//! Per-pixel access to the sprite sheet data model. SPEC §8.2, §9.3.
//!
//! The sheet is 256×256 bytes, addressed as 16×16 tiles in a 16-column grid
//! (§7.1). These helpers are the only place the addressing arithmetic lives;
//! `blit` owns drawing.

use crate::limits::{SHEET_LEN, SHEET_WIDTH, TILE_SIZE};

/// Byte offset of the top-left pixel of `id`. SPEC §7.1.
pub fn offset(id: u8) -> usize {
    let rows = SHEET_WIDTH / TILE_SIZE;
    let id = id as usize;
    (id / rows) * TILE_SIZE * SHEET_WIDTH + (id % rows) * TILE_SIZE
}

/// Read one pixel of a tile, or `None` outside the 16×16 tile.
pub fn get_pixel(sheet: &[u8], id: u8, px: usize, py: usize) -> Option<u8> {
    if px >= TILE_SIZE || py >= TILE_SIZE {
        return None;
    }
    debug_assert_eq!(sheet.len(), SHEET_LEN);
    Some(sheet[offset(id) + py * SHEET_WIDTH + px])
}

/// Set one pixel of a tile; out-of-range coordinates are ignored.
pub fn set_pixel(sheet: &mut [u8], id: u8, px: usize, py: usize, color: u8) {
    if px >= TILE_SIZE || py >= TILE_SIZE {
        return;
    }
    debug_assert_eq!(sheet.len(), SHEET_LEN);
    sheet[offset(id) + py * SHEET_WIDTH + px] = color;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet() -> Vec<u8> {
        let mut sheet = vec![0u8; SHEET_LEN];
        // Offset of the (15,15) bottom-right pixel inside a tile.
        let br = (TILE_SIZE - 1) * SHEET_WIDTH + TILE_SIZE - 1;
        // Mark four tiles 0, 15, 16, 255 at their corner pixels.
        sheet[offset(0)] = 1;
        sheet[offset(0) + br] = 2;
        sheet[offset(15)] = 3;
        sheet[offset(15) + br] = 4;
        sheet[offset(16)] = 5;
        sheet[offset(16) + br] = 6;
        sheet[offset(255)] = 7;
        sheet[offset(255) + br] = 8;
        sheet
    }

    #[test]
    fn all_four_landmark_tiles_address_distinct_pixels() {
        let sheet = sheet();
        assert_eq!(get_pixel(&sheet, 0, 0, 0), Some(1));
        assert_eq!(get_pixel(&sheet, 0, 15, 15), Some(2));
        assert_eq!(get_pixel(&sheet, 15, 0, 0), Some(3));
        assert_eq!(get_pixel(&sheet, 16, 0, 0), Some(5));
        assert_eq!(get_pixel(&sheet, 255, 0, 0), Some(7));
        assert_eq!(get_pixel(&sheet, 255, 15, 15), Some(8));
    }

    #[test]
    fn get_pixel_clips_inside_the_tile() {
        let sheet = sheet();
        assert_eq!(get_pixel(&sheet, 0, 16, 0), None);
        assert_eq!(get_pixel(&sheet, 0, 0, 16), None);
        assert_eq!(get_pixel(&sheet, 0, usize::MAX, 0), None);
    }

    #[test]
    fn set_pixel_writes_and_ignores_out_of_range() {
        let mut sheet = sheet();
        set_pixel(&mut sheet, 0, 1, 2, 10);
        assert_eq!(get_pixel(&sheet, 0, 1, 2), Some(10));

        set_pixel(&mut sheet, 0, 16, 0, 99);
        set_pixel(&mut sheet, 0, 0, 16, 99);
        assert_eq!(get_pixel(&sheet, 0, 0, 0), Some(1), "unchanged");
    }

    #[test]
    fn set_pixel_round_trips_through_get_pixel() {
        let mut sheet = vec![0u8; SHEET_LEN];
        set_pixel(&mut sheet, 42, 8, 9, 200);
        assert_eq!(get_pixel(&sheet, 42, 8, 9), Some(200));
        assert_eq!(get_pixel(&sheet, 42, 7, 9), Some(0));
    }
}
