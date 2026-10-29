//! SPEC §7.2, §7.3, §8.3 — the tilemap: 128×128 cells of tile ids.

use crate::error::CoreError;
use crate::framebuffer::Framebuffer;
use crate::limits::{MAP_LEN, MAP_SIZE, TILE_SIZE};
use crate::sheet::Sheet;

/// SPEC §7.2 — the map, one byte per cell (a tile id 0–255).
#[derive(Clone)]
pub struct Tilemap {
    cells: Box<[u8; MAP_LEN]>,
}

impl Default for Tilemap {
    fn default() -> Self {
        Self::new()
    }
}

impl Tilemap {
    /// A zero-filled map.
    pub fn new() -> Self {
        Self {
            cells: Box::new([0u8; MAP_LEN]),
        }
    }

    /// SPEC §13.1 — a map from the cart's 16,384 bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CoreError> {
        if bytes.len() != MAP_LEN {
            return Err(CoreError::WrongLength {
                what: "tilemap",
                expected: MAP_LEN,
                found: bytes.len(),
            });
        }
        let mut cells = Box::new([0u8; MAP_LEN]);
        cells.copy_from_slice(bytes);
        Ok(Self { cells })
    }

    /// The raw map bytes.
    pub fn bytes(&self) -> &[u8; MAP_LEN] {
        &self.cells
    }

    /// The raw map bytes, mutable.
    pub fn bytes_mut(&mut self) -> &mut [u8; MAP_LEN] {
        &mut self.cells
    }

    /// SPEC §7.3 — `tilemap::get`. Reads outside 0–127 return 0.
    pub fn get(&self, cx: i64, cy: i64) -> u8 {
        if cx < 0 || cy < 0 || cx >= MAP_SIZE as i64 || cy >= MAP_SIZE as i64 {
            return 0;
        }
        self.cells[cy as usize * MAP_SIZE + cx as usize]
    }

    /// SPEC §7.3 — `tilemap::set`. Writes outside 0–127 are ignored.
    pub fn set(&mut self, cx: i64, cy: i64, id: u8) {
        if cx < 0 || cy < 0 || cx >= MAP_SIZE as i64 || cy >= MAP_SIZE as i64 {
            return;
        }
        self.cells[cy as usize * MAP_SIZE + cx as usize] = id;
    }

    /// SPEC §8.3 — `tilemap::draw`: blit a rectangle of cells inline, in call
    /// order. Source cells outside the map read as tile id 0 (SPEC §7.3).
    /// `cw` or `ch` ≤ 0 is a no-op.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        fb: &mut Framebuffer,
        sheet: &Sheet,
        cx: i64,
        cy: i64,
        cw: i64,
        ch: i64,
        dx: i64,
        dy: i64,
    ) {
        if cw <= 0 || ch <= 0 {
            return;
        }
        for j in 0..ch {
            for i in 0..cw {
                let id = self.get(cx + i, cy + j);
                sheet.draw(
                    fb,
                    id,
                    dx + i * TILE_SIZE as i64,
                    dy + j * TILE_SIZE as i64,
                    false,
                    false,
                );
            }
        }
    }
}
