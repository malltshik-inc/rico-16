//! The M1 test pattern: every primitive, both palette presets, live input.
//!
//! This is not a cart and does not go through Rhai. It exercises the core the
//! way a cart will — one clear, a batch of draw calls, then present — so that
//! the window, the 60 Hz loop and the input edges can be tested before the
//! script engine exists.
//!
//! Everything here is a pure function of frame number and input. No wall clock,
//! no randomness: the same sequence of frames renders the same bytes.

use rico16_core::blit::{self, Blit};
use rico16_core::framebuffer::Framebuffer;
use rico16_core::gfx;
use rico16_core::input::{Button, Input};
use rico16_core::limits::SCREEN_WIDTH;
use rico16_core::palette::{self, Palette};

/// Sprite width and height; tiles are 16×16. SPEC §7.1.
const SPRITE_SIZE: i32 = 16;

/// Pixels per frame when an arrow is held.
const SPRITE_SPEED: i32 = 2;

/// Top of the play area, below the header.
const PLAY_TOP: i32 = 34;

/// Top of the instruction band. The sprite stays above it.
const BAND_TOP: i32 = 196;

/// The sheet is two frames side by side, not a full 256×256 sheet: the demo
/// only needs `draw_region`, and the keyed path is the same code.
const SHEET_WIDTH: usize = 32;
const SHEET_HEIGHT: usize = 16;

/// A blob with open eyes.
const FRAME_OPEN: [&str; 16] = [
    "................",
    "......kkkk......",
    "....kkbbbbkk....",
    "...kbbbbbbbbk...",
    "..kbbbbbbbbbbk..",
    "..kbbbbbbbbbbk..",
    ".kbbwwbbbbwwbbk.",
    ".kbbwkbbbbwkbbk.",
    ".kbbbbbbbbbbbbk.",
    ".kbbbbbbbbbbbbk.",
    ".kbbbbbbbbbbbbk.",
    "..kbbbbbbbbbbk..",
    "..kbbbbbbbbbbk..",
    "...kbbbbbbbbk...",
    "....kkbbbbkk....",
    "......kkkk......",
];

/// The same blob blinking: two rows of eyes replaced by a closed line.
const FRAME_BLINK: [&str; 16] = [
    "................",
    "......kkkk......",
    "....kkbbbbkk....",
    "...kbbbbbbbbk...",
    "..kbbbbbbbbbbk..",
    "..kbbbbbbbbbbk..",
    ".kbbbbbbbbbbbbk.",
    ".kbbkkbbbbkkbbk.",
    ".kbbbbbbbbbbbbk.",
    ".kbbbbbbbbbbbbk.",
    ".kbbbbbbbbbbbbk.",
    "..kbbbbbbbbbbk..",
    "..kbbbbbbbbbbk..",
    "...kbbbbbbbbk...",
    "....kkbbbbkk....",
    "......kkkk......",
];

/// Live demo state. Reset by Ctrl+R, exactly like restarting a cart. SPEC §12.3.
#[derive(Clone, Debug)]
pub struct Demo {
    frame: u64,
    x: i32,
    y: i32,
    flip_x: bool,
    rgb332: bool,
    grid: bool,
    flash: u8,
    sheet: Vec<u8>,
}

impl Demo {
    /// A fresh demo at its starting state.
    pub fn new() -> Self {
        Self {
            frame: 0,
            x: 150,
            y: 140,
            flip_x: false,
            rgb332: false,
            grid: false,
            flash: 0,
            sheet: build_sheet(),
        }
    }

    /// Restart, as `Ctrl+R` will restart a cart: everything back to frame 0.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Advance one frame. Called once per `on_update`. SPEC §5.2.
    pub fn update(&mut self, input: &Input) {
        if input.held(Button::Left) {
            self.x -= SPRITE_SPEED;
        }
        if input.held(Button::Right) {
            self.x += SPRITE_SPEED;
        }
        if input.held(Button::Up) {
            self.y -= SPRITE_SPEED;
        }
        if input.held(Button::Down) {
            self.y += SPRITE_SPEED;
        }
        self.x = self.x.clamp(0, SCREEN_WIDTH as i32 - SPRITE_SIZE);
        self.y = self.y.clamp(PLAY_TOP, BAND_TOP - SPRITE_SIZE);

        if input.pressed(Button::X) {
            self.flip_x = !self.flip_x;
        }
        if input.pressed(Button::C) {
            // The two shipped palettes, swapped at runtime. This is the whole
            // point of an indexed framebuffer: no art changes when it happens.
            self.rgb332 = !self.rgb332;
        }
        if input.pressed(Button::V) {
            self.grid = !self.grid;
        }
        if input.pressed(Button::Z) {
            self.flash = 6;
        } else {
            self.flash = self.flash.saturating_sub(1);
        }

        self.frame += 1;
    }

    /// Draw the frame and resolve the palette. SPEC §5.2, §9.3.
    pub fn render(&self, framebuffer: &mut Framebuffer, palette: &mut Palette, input: &Input) {
        if self.rgb332 {
            *palette = Palette::from_entries(palette::rgb332());
        } else {
            palette.reset();
        }
        if self.flash > 0 {
            // A palette-only flash: the background entry becomes hot pink for
            // six frames and every pixel using it follows, for free.
            palette.set_rgb8(1, 255, 72, 120);
        }

        framebuffer.clear(1);

        if self.grid {
            self.draw_grid(framebuffer);
        }

        // Header.
        gfx::rect_filled(framebuffer, 0, 0, SCREEN_WIDTH as i32, 34, 0);
        gfx::text(framebuffer, "RICO-16", 8, 6, 5);
        gfx::text(framebuffer, "M1  FRAMEBUFFER  INPUT  60 HZ", 72, 6, 3);
        gfx::text(framebuffer, &format!("FRAME {:05}", self.frame), 8, 18, 4);
        gfx::text(framebuffer, "DT 1/60   NO CATCH-UP", 104, 18, 2);

        // All 256 palette entries, 32 × 8 cells of 8 px, at (32, 38).
        for index in 0..256u16 {
            let x = 32 + (index % 32) as i32 * 8;
            let y = 38 + (index / 32) as i32 * 8;
            gfx::rect_filled(framebuffer, x, y, 8, 8, index as u8);
        }

        // One of each primitive.
        gfx::rect_filled(framebuffer, 8, 110, 64, 56, 6);
        gfx::rect(framebuffer, 8, 110, 64, 56, 5);
        gfx::circle(framebuffer, 110, 138, 26, 12);
        gfx::circle_filled(framebuffer, 170, 138, 26, 9);
        gfx::line(framebuffer, 210, 110, 310, 166, 8);
        gfx::line(framebuffer, 310, 110, 210, 166, 14);

        // Deliberate clipping probes: shapes half off every edge.
        gfx::rect_filled(framebuffer, -8, 168, 20, 20, 13);
        gfx::circle(framebuffer, 316, 140, 18, 15);
        gfx::text(framebuffer, "CLIPPED", -40, 178, 5);

        // The sprite blinks every two seconds; the frame counter is the clock.
        let sheet_x = if self.frame / 40 % 4 == 0 { 16 } else { 0 };
        let blit = Blit::new(sheet_x, 0, 16, 16, self.x, self.y).flipped(self.flip_x, false);
        blit::region(framebuffer, &self.sheet, SHEET_WIDTH, SHEET_HEIGHT, blit);

        // Instruction band.
        gfx::rect_filled(framebuffer, 0, BAND_TOP, SCREEN_WIDTH as i32, 44, 0);
        gfx::text(framebuffer, "ARROWS MOVE   X FLIP   Z FLASH", 8, 200, 4);
        gfx::text(framebuffer, "C PALETTE   V GRID", 8, 212, 4);

        let labels = ["L", "R", "U", "D", "Z", "X", "C", "V"];
        for (index, label) in labels.iter().enumerate() {
            let color = if input.held(Button::ALL[index]) { 5 } else { 2 };
            gfx::text(framebuffer, label, 8 + index as i32 * 16, 224, color);
        }

        let theme = if self.rgb332 {
            "PAL RGB332"
        } else {
            "PAL RAMP"
        };
        gfx::text(
            framebuffer,
            &format!("{:03},{:03} {theme}", self.x, self.y),
            160,
            224,
            3,
        );
    }

    fn draw_grid(&self, framebuffer: &mut Framebuffer) {
        for x in (16..SCREEN_WIDTH as i32).step_by(16) {
            gfx::line(framebuffer, x, PLAY_TOP, x, BAND_TOP, 16);
        }
        for y in (PLAY_TOP..BAND_TOP).step_by(16) {
            gfx::line(framebuffer, 0, y, SCREEN_WIDTH as i32, y, 16);
        }
    }
}

impl Default for Demo {
    fn default() -> Self {
        Self::new()
    }
}

fn build_sheet() -> Vec<u8> {
    let mut sheet = vec![0u8; SHEET_WIDTH * SHEET_HEIGHT];
    paint(FRAME_OPEN, &mut sheet, 0);
    paint(FRAME_BLINK, &mut sheet, 16);
    sheet
}

fn paint(rows: [&str; 16], sheet: &mut [u8], offset_x: usize) {
    for (y, row) in rows.iter().enumerate() {
        debug_assert_eq!(row.len(), 16, "art rows are exactly one tile wide");
        for (x, pixel) in row.chars().enumerate() {
            sheet[y * SHEET_WIDTH + offset_x + x] = match pixel {
                '.' => 0,
                // Index 3, not the background's index 1: the ring must read
                // against the background it is drawn on.
                'k' => 3,
                'b' => 104,
                'w' => 5,
                _ => unreachable!("unknown art pixel"),
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rico16_core::limits::FRAMEBUFFER_LEN;

    struct Render {
        framebuffer: Framebuffer,
        palette: Palette,
        demo: Demo,
        input: Input,
    }

    impl Render {
        fn new() -> Self {
            Self {
                framebuffer: Framebuffer::new(),
                palette: Palette::new(),
                demo: Demo::new(),
                input: Input::new(),
            }
        }

        fn step(&mut self, mask: u8) {
            self.input.update(mask);
            self.demo.update(&self.input);
            self.demo
                .render(&mut self.framebuffer, &mut self.palette, &self.input);
        }

        fn hash(&self) -> u64 {
            // FNV-1a over the indexed framebuffer. Stable and cheap.
            let mut hash = 0xcbf2_9ce4_8422_2325;
            for byte in self.framebuffer.indices() {
                hash ^= *byte as u64;
                hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
            }
            hash
        }
    }

    #[test]
    fn sixty_frames_render_identically_twice() {
        let mut first = Render::new();
        let mut second = Render::new();
        for frame in 0..60 {
            let mask = if frame % 3 == 0 {
                Button::Right.bit()
            } else {
                0
            };
            first.step(mask);
            second.step(mask);
        }
        assert_eq!(first.hash(), second.hash());
        assert_eq!(first.framebuffer.indices(), second.framebuffer.indices());
    }

    #[test]
    fn holding_right_moves_the_sprite_two_pixels_per_frame() {
        let mut render = Render::new();
        let start = render.demo.x;
        for _ in 0..10 {
            render.step(Button::Right.bit());
        }
        assert_eq!(render.demo.x, start + 20);
        assert_eq!(render.demo.y, 140, "vertical position is untouched");
    }

    #[test]
    fn the_sprite_stays_inside_the_play_area() {
        let mut render = Render::new();
        for _ in 0..500 {
            render.step(Button::Right.bit() | Button::Down.bit());
        }
        assert_eq!(render.demo.x, SCREEN_WIDTH as i32 - SPRITE_SIZE);
        assert_eq!(render.demo.y, BAND_TOP - SPRITE_SIZE);

        for _ in 0..500 {
            render.step(Button::Left.bit() | Button::Up.bit());
        }
        assert_eq!(render.demo.x, 0);
        assert_eq!(render.demo.y, PLAY_TOP);
    }

    #[test]
    fn input_changes_what_is_drawn() {
        let mut idle = Render::new();
        let mut moved = Render::new();
        for _ in 0..10 {
            idle.step(0);
            moved.step(Button::Left.bit());
        }
        assert_ne!(idle.hash(), moved.hash());
    }

    #[test]
    fn palette_swap_recolours_without_touching_the_indices() {
        let mut ramp = Render::new();
        ramp.step(0);
        let indices = ramp.framebuffer.indices().to_vec();

        // Same frame, same position, different palette. Setting the flag by
        // hand instead of pressing C keeps both renders on the same frame
        // counter, so the only variable is the palette.
        let mut rgb = Render::new();
        rgb.demo.rgb332 = true;
        rgb.step(0);

        // The status label names the active palette, so its glyphs differ by
        // design. Everything above it — the whole scene and the 256 swatches —
        // must be index-for-index identical.
        let label_row = 224 * SCREEN_WIDTH;
        assert_eq!(
            rgb.framebuffer.indices()[..label_row],
            indices[..label_row],
            "the scene and the swatches keep their indices; only the palette differs"
        );

        let mut ramp_rgba = vec![0u8; FRAMEBUFFER_LEN * 4];
        let mut rgb_rgba = vec![0u8; FRAMEBUFFER_LEN * 4];
        ramp.framebuffer.expand_into(&ramp.palette, &mut ramp_rgba);
        rgb.framebuffer.expand_into(&rgb.palette, &mut rgb_rgba);
        assert_ne!(ramp_rgba, rgb_rgba, "the presented colours must change");

        assert!(
            (0..=255u8).any(|index| ramp.palette.rgba8(index) != rgb.palette.rgba8(index)),
            "the rgb332 preset must recolour at least one entry"
        );
    }

    #[test]
    fn the_flash_decays_after_six_frames() {
        let mut render = Render::new();
        render.step(Button::Z.bit());
        assert_eq!(render.demo.flash, 6);

        // `set_rgb8` quantizes to RGB555 on the way in (SPEC §4.4), so compare
        // against the stored word rather than the requested 8-bit bytes.
        let flashed = render.palette.rgba8(1);
        let mut expected = palette::Palette::new();
        expected.set_rgb8(1, 255, 72, 120);
        assert_eq!(flashed, expected.rgba8(1));
        assert_ne!(flashed, palette::Palette::new().rgba8(1), "the entry moved");

        for _ in 0..6 {
            render.step(0);
        }
        assert_eq!(render.demo.flash, 0);
        assert_eq!(render.palette.rgba8(1), palette::Palette::new().rgba8(1));
    }

    #[test]
    fn the_sprite_sheet_is_two_well_formed_frames() {
        let sheet = build_sheet();
        assert_eq!(sheet.len(), SHEET_WIDTH * SHEET_HEIGHT);

        for y in 0..16 {
            for x in 0..16 {
                let open = sheet[y * SHEET_WIDTH + x];
                let blink = sheet[y * SHEET_WIDTH + 16 + x];
                for color in [open, blink] {
                    assert!(
                        matches!(color, 0 | 3 | 5 | 104),
                        "unexpected sheet colour {color}"
                    );
                }
            }
        }

        assert_eq!(sheet[0], 0, "the top-left pixel is the transparency key");
        assert_ne!(sheet[6 * SHEET_WIDTH + 4], 0, "frame 0 has an eye");
        assert_eq!(
            sheet[7 * SHEET_WIDTH + 16 + 4],
            3,
            "frame 1 has a closed eye"
        );
    }

    #[test]
    fn reset_restores_the_initial_state() {
        let mut render = Render::new();
        for _ in 0..5 {
            render.step(Button::Right.bit() | Button::C.bit() | Button::Z.bit());
        }
        render.demo.reset();
        assert_eq!(render.demo.frame, 0);
        assert_eq!((render.demo.x, render.demo.y), (150, 140));
        assert!(!render.demo.flip_x);
        assert!(!render.demo.rgb332);
        assert_eq!(render.demo.flash, 0);
    }
}
