//! The tilemap data model and its draw operation. SPEC §7.2, §8.3.
//!
//! A map is 128×128 cells of one byte each, row-major. Values are tile ids
//! 0–255; every id is a real tile (no "empty" tile, §7.2). It is *data*, not a
//! surface: nothing scrolls it and it has no draw order of its own.

use crate::blit::{self, Blit};
use crate::framebuffer::Framebuffer;
use crate::limits::{MAP_HEIGHT, MAP_LEN, MAP_WIDTH, SHEET_HEIGHT, SHEET_WIDTH, TILE_SIZE};

/// The 128×128 cell map. SPEC §7.2.
#[derive(Clone, PartialEq, Eq)]
pub struct Tilemap {
    cells: Box<[u8; MAP_LEN]>,
}

impl Tilemap {
    /// An all-zero map: every cell is tile 0.
    pub fn new() -> Self {
        Self {
            cells: Box::new([0; MAP_LEN]),
        }
    }

    /// A map from cart data, copied verbatim.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        assert_eq!(bytes.len(), MAP_LEN, "map data must be 16,384 bytes");
        let mut cells = Box::new([0; MAP_LEN]);
        cells.copy_from_slice(bytes);
        Self { cells }
    }

    /// The raw cell bytes, exactly 16,384 of them.
    pub fn as_bytes(&self) -> &[u8] {
        &*self.cells
    }

    /// The cell at `(cx, cy)`, or 0 outside the map. SPEC §7.3.
    ///
    /// The silent 0 lets procedural code run off the edges without bounds
    /// checks; carts must not rely on it to hide bugs.
    pub fn get(&self, cx: i32, cy: i32) -> u8 {
        if cx < 0 || cy < 0 || cx >= MAP_WIDTH as i32 || cy >= MAP_HEIGHT as i32 {
            return 0;
        }
        self.cells[cy as usize * MAP_WIDTH + cx as usize]
    }

    /// Overwrite the cell at `(cx, cy)`, ignoring positions outside the map
    /// entirely. SPEC §7.3.
    pub fn set(&mut self, cx: i32, cy: i32, id: u8) {
        if cx < 0 || cy < 0 || cx >= MAP_WIDTH as i32 || cy >= MAP_HEIGHT as i32 {
            return;
        }
        self.cells[cy as usize * MAP_WIDTH + cx as usize] = id;
    }

    /// Mutable access for cart loading and tests.
    pub fn cells_mut(&mut self) -> &mut [u8] {
        &mut *self.cells
    }
}

impl Default for Tilemap {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Tilemap {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Tilemap")
            .field("cells", &MAP_LEN)
            .finish()
    }
}

/// Blit a `cw × ch` rectangle of map cells, starting at cell `(cx, cy)`, so its
/// top-left lands at `(dx, dy)` on screen. SPEC §8.3.
///
/// Each cell blits its tile through the sheet in call order; index 0 stays
/// transparent. Cells that run outside the 128×128 map read as tile 0, matching
/// [`Tilemap::get`]. Degenerate or fully off-screen arguments draw nothing.
pub fn draw(
    destination: &mut Framebuffer,
    sheet: &[u8],
    map: &Tilemap,
    cx: i32,
    cy: i32,
    cw: i32,
    ch: i32,
    dx: i32,
    dy: i32,
) {
    if cw <= 0 || ch <= 0 {
        return;
    }
    for row in 0..ch {
        for column in 0..cw {
            let id = map.get(cx + column, cy + row);
            let blit = Blit::new(
                (id as usize % (SHEET_WIDTH / TILE_SIZE) * TILE_SIZE) as i32,
                (id as usize / (SHEET_WIDTH / TILE_SIZE) * TILE_SIZE) as i32,
                TILE_SIZE as i32,
                TILE_SIZE as i32,
                dx + column * TILE_SIZE as i32,
                dy + row * TILE_SIZE as i32,
            );
            blit::region(destination, sheet, SHEET_WIDTH, SHEET_HEIGHT, blit);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank() -> Framebuffer {
        let mut framebuffer = Framebuffer::new();
        framebuffer.clear(0);
        framebuffer
    }

    /// A map seeded with a small prefix of cells, rest zeros.
    fn plain_map(cells: &[u8]) -> Tilemap {
        let mut map = Tilemap::new();
        map.cells_mut()[..cells.len()].copy_from_slice(cells);
        map
    }

    #[test]
    fn a_new_map_is_all_zero() {
        let map = Tilemap::new();
        assert!(map.as_bytes().iter().all(|cell| *cell == 0));
    }

    #[test]
    fn get_and_set_obey_the_bounds_rule() {
        let mut map = Tilemap::new();
        map.set(1, 2, 9);
        assert_eq!(map.get(1, 2), 9);
        assert_eq!(map.get(0, 0), 0);

        // Outside 0..128: get reads 0, set is a no-op (§7.3).
        map.set(-1, 0, 5);
        map.set(0, -1, 5);
        map.set(128, 0, 5);
        map.set(0, 128, 5);
        map.set(i32::MAX, i32::MIN, 5);
        assert_eq!(map.get(-1, 0), 0);
        assert_eq!(map.get(0, 128), 0);
        assert_eq!(map.get(127, 127), 0);
        assert_eq!(map.get(i32::MAX, 0), 0);
    }

    #[test]
    fn the_last_cell_is_addressable() {
        let mut map = Tilemap::new();
        map.set(127, 127, 17);
        assert_eq!(map.get(127, 127), 17);
        map.set(0, 0, 3);
        assert_eq!(map.get(0, 0), 3);
    }

    #[test]
    fn draw_places_cells_in_row_major_order() {
        let mut map = Tilemap::new();
        map.set(0, 0, 0);
        map.set(1, 0, 1);
        map.set(0, 1, 2);
        map.set(1, 1, 3);
        // A single 16×16 sheet whose four corner pixels label the tiles.
        let mut sheet = vec![0u8; SHEET_WIDTH * SHEET_HEIGHT];
        for tile in 0..4u8 {
            let sx = tile as usize % (SHEET_WIDTH / TILE_SIZE) * TILE_SIZE;
            let sy = tile as usize / (SHEET_WIDTH / TILE_SIZE) * TILE_SIZE;
            sheet[sy * SHEET_WIDTH + sx + 7] = tile + 10;
        }

        let mut framebuffer = blank();
        draw(
            &mut framebuffer,
            &sheet,
            &map,
            0,
            0,
            2,
            2,
            10,
            10,
        );

        // Tile 0's marker lands at cell (0,0) → (10,10)+7.
        assert_eq!(framebuffer.get(17, 10), Some(10));
        // Tile 1 at cell (1,0) → (26,10)+7.
        assert_eq!(framebuffer.get(33, 10), Some(11));
        // Tile 2 at cell (0,1) → (10,26)+7.
        assert_eq!(framebuffer.get(17, 26), Some(12));
        // Tile 3 at cell (1,1).
        assert_eq!(framebuffer.get(33, 26), Some(13));
    }

    #[test]
    fn draw_skips_index_zero() {
        let mut map = Tilemap::new();
        // Two cells, both containing tile 0.
        map.set(0, 0, 0);
        map.set(1, 0, 0);
        let mut sheet = vec![0u8; SHEET_WIDTH * SHEET_HEIGHT];

        let mut framebuffer = blank();
        framebuffer.clear(9);
        draw(&mut framebuffer, &sheet, &map, 0, 0, 2, 1, 0, 0);
        // A tile whose content is entirely index 0 never touches the screen.
        assert!(framebuffer.indices().iter().all(|index| *index == 9));

        // The same tile with one visible pixel does draw.
        sheet[5] = 4;
        draw(&mut framebuffer, &sheet, &map, 1, 0, 1, 1, 0, 0);
        assert_eq!(framebuffer.get(5, 0), Some(4));
    }

    #[test]
    fn draw_reads_zero_cells_outside_the_map() {
        let map = Tilemap::new();
        let mut sheet = vec![0u8; SHEET_WIDTH * SHEET_HEIGHT];
        sheet[0] = 4;

        let mut framebuffer = blank();
        draw(&mut framebuffer, &sheet, &map, 126, 126, 4, 4, 0, 0);
        // The out-of-map cells read as tile 0, whose corner marker draws.
        assert_eq!(framebuffer.get(0, 0), Some(4));
    }

    #[test]
    fn degenerate_or_off_screen_draws_draw_nothing() {
        let map = plain_map(&[1]);
        let mut sheet = vec![0u8; SHEET_WIDTH * SHEET_HEIGHT];
        sheet.fill(3);

        for (cx, cy, cw, ch, dx, dy) in [
            (0, 0, 0, 1, 0, 0),
            (0, 0, 1, 0, 0, 0),
            (0, 0, -1, 1, 0, 0),
            (0, 0, 1, -1, 0, 0),
            (0, 0, 1, 1, 400, 0),
            (0, 0, 1, 1, 0, 300),
        ] {
            let mut framebuffer = blank();
            draw(&mut framebuffer, &sheet, &map, cx, cy, cw, ch, dx, dy);
            assert_ne!(
                framebuffer.indices().iter().any(|index| *index != 0),
                true,
                "draw({cx},{cy},{cw},{ch},{dx},{dy}) drew pixels"
            );
        }
    }
}
