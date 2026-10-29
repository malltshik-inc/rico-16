//! SPEC §3, §4, §5, §7, §13 — the console's fixed resources and PNG layout.
//!
//! Every number the console fixes lives here and nowhere else (AGENTS.md).
//! SPEC §7 is the human-facing copy of the same table; changing one without
//! the other is a bug and a major version change.

/// SPEC §3.1 — screen width in pixel units.
pub const SCREEN_WIDTH: usize = 320;
/// SPEC §3.1 — screen height in pixel units.
pub const SCREEN_HEIGHT: usize = 240;
/// SPEC §3.2 — framebuffer length in bytes, one byte per pixel.
pub const FRAMEBUFFER_LEN: usize = SCREEN_WIDTH * SCREEN_HEIGHT; // 76_800

/// SPEC §7.1 — tile edge in pixel units.
pub const TILE_SIZE: usize = 16;
/// SPEC §7.1 — visible cells across the screen (20×15).
pub const SCREEN_COLS: usize = SCREEN_WIDTH / TILE_SIZE; // 20
/// SPEC §7.1 — visible cells down the screen.
pub const SCREEN_ROWS: usize = SCREEN_HEIGHT / TILE_SIZE; // 15

/// SPEC §7 — sprite sheet edge in pixels.
pub const SHEET_SIZE: usize = 256;
/// SPEC §7 — sprite sheet length in bytes.
pub const SHEET_LEN: usize = SHEET_SIZE * SHEET_SIZE; // 65_536
/// SPEC §7.1 — tiles per sheet row (tile id = row * 16 + col).
pub const TILES_PER_ROW: usize = SHEET_SIZE / TILE_SIZE; // 16
/// SPEC §7.1 — number of tile ids (0–255).
pub const TILE_COUNT: usize = 256;

/// SPEC §7.2 — map edge in cells.
pub const MAP_SIZE: usize = 128;
/// SPEC §7.2 — map length in cells.
pub const MAP_LEN: usize = MAP_SIZE * MAP_SIZE; // 16_384
/// SPEC §7.2 — world size in pixel units (2048×2048).
pub const WORLD_SIZE: usize = MAP_SIZE * TILE_SIZE; // 2048

/// SPEC §4.1 — palette entries.
pub const PALETTE_LEN: usize = 256;
/// SPEC §4.1 — palette size in bytes (RGB555 × 256).
pub const PALETTE_BYTES: usize = PALETTE_LEN * 2; // 512

/// SPEC §5.1 — the fixed timestep passed to `on_update`, exactly `1/60`.
pub const DT: f64 = 1.0 / 60.0;

/// SPEC §7 — Rhai operations allowed per entry-point call.
pub const MAX_OPERATIONS: u64 = 1_000_000;
/// SPEC §7 — maximum nested script calls.
pub const MAX_CALL_DEPTH: usize = 64;
/// SPEC §7 — maximum array length.
pub const MAX_ARRAY_LEN: usize = 65_536;
/// SPEC §7 — maximum map length.
pub const MAX_MAP_LEN: usize = 65_536;
/// SPEC §7 — maximum string length in bytes.
pub const MAX_STRING_LEN: usize = 1_048_576;
/// The Rhai parser's expression depth limit, per source file.
///
/// Not one of the limits SPEC §7 fixes. It exists only to stop a pathological
/// script from overflowing the parser's stack, and the engine's default is low
/// enough to reject ordinary nested loops, so it is raised well clear of real
/// code rather than left as a trap.
pub const MAX_EXPR_DEPTH: usize = 256;

/// SPEC §13.1 — fixed binary cart header length before the code section
/// (magic + version + flags + three length fields + sheet + map + palette).
pub const CART_FIXED_HEADER_LEN: usize = 20 + SHEET_LEN + MAP_LEN + PALETTE_BYTES; // 82_452

/// SPEC §13.2.3 — blob header length.
pub const BLOB_HEADER_LEN: usize = 16;

/// SPEC §7.7 — default PNG variant width.
pub const PNG_DEFAULT_WIDTH: u32 = 640;
/// SPEC §7.7 — default PNG variant height.
pub const PNG_DEFAULT_HEIGHT: u32 = 480;
/// SPEC §7.7 — overflow PNG variant width.
pub const PNG_OVERFLOW_WIDTH: u32 = 1280;
/// SPEC §7.7 — overflow PNG variant height.
pub const PNG_OVERFLOW_HEIGHT: u32 = 960;

/// SPEC §13.2.2 — payload capacity in bytes for the default variant
/// (12 bits per pixel = 3/2 bytes per pixel).
pub const PNG_DEFAULT_CAPACITY: usize =
    (PNG_DEFAULT_WIDTH as usize * PNG_DEFAULT_HEIGHT as usize) * 3 / 2; // 460_800
/// SPEC §13.2.2 — payload capacity in bytes for the overflow variant.
pub const PNG_OVERFLOW_CAPACITY: usize =
    (PNG_OVERFLOW_WIDTH as usize * PNG_OVERFLOW_HEIGHT as usize) * 3 / 2; // 1_843_200
/// SPEC §13.2.3 — payload bytes available after the blob header (default).
pub const PNG_DEFAULT_PAYLOAD: usize = PNG_DEFAULT_CAPACITY - BLOB_HEADER_LEN;
/// SPEC §13.2.3 — payload bytes available after the blob header (overflow).
pub const PNG_OVERFLOW_PAYLOAD: usize = PNG_OVERFLOW_CAPACITY - BLOB_HEADER_LEN;
