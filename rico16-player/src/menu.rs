//! SPEC §14.5 — the runtime system menu.
//!
//! Drawn into a **separate overlay buffer**, not the cart framebuffer, so it
//! never counts against the cart's draw budget and never touches the cart's
//! pixels.

use rico16_core::font;
use rico16_core::limits::{FRAMEBUFFER_LEN, SCREEN_HEIGHT, SCREEN_WIDTH};

/// SPEC §14.5 — the menu entries, in order.
pub const ITEMS: [&str; 3] = ["Resume", "Restart", "Exit"];

/// What activating a menu entry means to the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    /// Close the menu and continue.
    Resume,
    /// Reset the cart (SPEC §14.9).
    Restart,
    /// Quit and close the window.
    Exit,
}

/// SPEC §14.5 — the system menu's selection state.
#[derive(Debug, Clone, Default)]
pub struct SystemMenu {
    /// Index into [`ITEMS`].
    pub selected: usize,
}

impl SystemMenu {
    /// A menu with the first entry selected.
    pub fn new() -> Self {
        Self::default()
    }

    /// Move the selection down, wrapping.
    pub fn next(&mut self) {
        self.selected = (self.selected + 1) % ITEMS.len();
    }

    /// Move the selection up, wrapping.
    pub fn prev(&mut self) {
        self.selected = (self.selected + ITEMS.len() - 1) % ITEMS.len();
    }

    /// The action of the current selection.
    pub fn action(&self) -> MenuAction {
        match self.selected {
            0 => MenuAction::Resume,
            1 => MenuAction::Restart,
            _ => MenuAction::Exit,
        }
    }

    /// SPEC §14.5 — render the menu into `overlay`, an RGBA buffer of the screen
    /// size. Transparent pixels leave the cart frame visible.
    pub fn render(&self, overlay: &mut [u8]) {
        debug_assert_eq!(overlay.len(), FRAMEBUFFER_LEN * 4);
        overlay.fill(0);

        // A dim wash over the paused cart.
        for px in overlay.chunks_exact_mut(4) {
            px.copy_from_slice(&[0, 0, 0, 168]);
        }

        // A centred panel.
        let panel_w = 160;
        let panel_h = 88;
        let x0 = (SCREEN_WIDTH as i64 - panel_w) / 2;
        let y0 = (SCREEN_HEIGHT as i64 - panel_h) / 2;
        fill_rect(overlay, x0, y0, panel_w, panel_h, [24, 24, 32, 255]);
        fill_rect(overlay, x0, y0, panel_w, 2, [255, 255, 255, 255]);
        fill_rect(
            overlay,
            x0,
            y0 + panel_h - 2,
            panel_w,
            2,
            [255, 255, 255, 255],
        );

        draw_text(overlay, "PAUSED", x0 + 12, y0 + 12, 2, [255, 255, 255, 255]);
        for (i, item) in ITEMS.iter().enumerate() {
            let marker = if i == self.selected { "> " } else { "  " };
            let line = format!("{marker}{item}");
            let color = if i == self.selected {
                [255, 210, 0, 255]
            } else {
                [200, 200, 200, 255]
            };
            draw_text(overlay, &line, x0 + 12, y0 + 36 + i as i64 * 12, 1, color);
        }
    }
}

/// SPEC §14.5 — composite an overlay over a frame, in place. `alpha` 0 leaves
/// the frame untouched; 255 replaces it.
pub fn composite(frame: &mut [u8], overlay: &[u8]) {
    for (dst, src) in frame.chunks_exact_mut(4).zip(overlay.chunks_exact(4)) {
        match src[3] {
            0 => {}
            255 => dst.copy_from_slice(src),
            a => {
                let a = a as u32;
                for c in 0..3 {
                    dst[c] = ((src[c] as u32 * a + dst[c] as u32 * (255 - a)) / 255) as u8;
                }
                dst[3] = 255;
            }
        }
    }
}

fn fill_rect(buf: &mut [u8], x: i64, y: i64, w: i64, h: i64, color: [u8; 4]) {
    for py in y..y + h {
        for px in x..x + w {
            if px < 0 || py < 0 || px >= SCREEN_WIDTH as i64 || py >= SCREEN_HEIGHT as i64 {
                continue;
            }
            let at = (py as usize * SCREEN_WIDTH + px as usize) * 4;
            buf[at..at + 4].copy_from_slice(&color);
        }
    }
}

fn draw_text(buf: &mut [u8], text: &str, x: i64, y: i64, scale: i64, color: [u8; 4]) {
    let mut cx = x;
    for ch in text.chars() {
        let glyph = font::glyph(ch);
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..8i64 {
                if bits & (0x80 >> col) == 0 {
                    continue;
                }
                for sy in 0..scale {
                    for sx in 0..scale {
                        let px = cx + col * scale + sx;
                        let py = y + row as i64 * scale + sy;
                        if px < 0
                            || py < 0
                            || px >= SCREEN_WIDTH as i64
                            || py >= SCREEN_HEIGHT as i64
                        {
                            continue;
                        }
                        let at = (py as usize * SCREEN_WIDTH + px as usize) * 4;
                        buf[at..at + 4].copy_from_slice(&color);
                    }
                }
            }
        }
        cx += 8 * scale;
    }
}
