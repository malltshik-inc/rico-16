//! The fixed resources of the console.
//!
//! These are the numbers in `docs/SPEC.md` §7. They exist in exactly one place
//! in the code so that a "quiet" change cannot happen: changing one of them is
//! a major version bump and requires updating the specification.

/// Screen width in pixels. SPEC §3.1.
pub const SCREEN_WIDTH: usize = 320;

/// Screen height in pixels. SPEC §3.1.
pub const SCREEN_HEIGHT: usize = 240;

/// Number of pixels, and therefore bytes, in the framebuffer. SPEC §3.2.
pub const FRAMEBUFFER_LEN: usize = SCREEN_WIDTH * SCREEN_HEIGHT;

/// Tile edge in pixels. SPEC §7.1.
pub const TILE_SIZE: usize = 16;

/// Whole tiles visible across the screen. SPEC §7.1.
pub const TILES_ACROSS: usize = SCREEN_WIDTH / TILE_SIZE;

/// Whole tiles visible down the screen. SPEC §7.1.
pub const TILES_DOWN: usize = SCREEN_HEIGHT / TILE_SIZE;

/// Sprite sheet width in pixels. SPEC §7.
pub const SHEET_WIDTH: usize = 256;

/// Sprite sheet height in pixels. SPEC §7.
pub const SHEET_HEIGHT: usize = 256;

/// Sprite sheet size in bytes. SPEC §7.
pub const SHEET_LEN: usize = SHEET_WIDTH * SHEET_HEIGHT;

/// Number of 16×16 tiles in the sheet. SPEC §7.1.
pub const SHEET_TILES: usize = (SHEET_WIDTH / TILE_SIZE) * (SHEET_HEIGHT / TILE_SIZE);

/// Tilemap width in cells. SPEC §7.2.
pub const MAP_WIDTH: usize = 128;

/// Tilemap height in cells. SPEC §7.2.
pub const MAP_HEIGHT: usize = 128;

/// Tilemap size in bytes; one byte per cell. SPEC §7.2.
pub const MAP_LEN: usize = MAP_WIDTH * MAP_HEIGHT;

/// Palette entries. SPEC §4.1.
pub const PALETTE_LEN: usize = 256;

/// Palette size in bytes. SPEC §4.1.
pub const PALETTE_BYTES: usize = PALETTE_LEN * 2;

/// Maximum bytes of Rhai source per cart. SPEC §7.4.
pub const CODE_LIMIT: usize = 65_536;

/// Updates per second. SPEC §5.1.
pub const FPS: u32 = 60;

/// Frame period in nanoseconds, used to schedule the loop. SPEC §5.1.
///
/// `dt` is the exact f64 `1/60`; this integer is only a scheduling hint, so a
/// sub-nanosecond truncation is harmless.
pub const FRAME_TIME_NANOS: u64 = 1_000_000_000 / FPS as u64;

/// Timestep handed to `on_update`, always exactly this value. SPEC §5.1.
pub const DT: f64 = 1.0 / 60.0;

// The map stores tile ids in a single byte, so the sheet must not hold more
// tiles than a byte can name (SPEC §7.2).
const _: () = assert!(SHEET_TILES <= 256);
const _: () = assert!(TILES_ACROSS * TILE_SIZE == SCREEN_WIDTH);
const _: () = assert!(TILES_DOWN * TILE_SIZE == SCREEN_HEIGHT);
const _: () = assert!(FRAMEBUFFER_LEN == 76_800);
const _: () = assert!(SHEET_LEN == 65_536);
const _: () = assert!(MAP_LEN == 16_384);
const _: () = assert!(PALETTE_BYTES == 512);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_is_exactly_20_by_15_tiles() {
        assert_eq!(TILES_ACROSS, 20);
        assert_eq!(TILES_DOWN, 15);
    }

    #[test]
    fn sheet_holds_256_tiles() {
        assert_eq!(SHEET_TILES, 256);
    }
}
