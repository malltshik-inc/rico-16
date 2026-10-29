//! SPEC §6 — drawing primitives.
//!
//! Every primitive is overwrite (SPEC §6.4), takes integer geometry, clips
//! without panicking (SPEC §6.1) and writes its colour literally, including 0
//! (SPEC §4.3).

use crate::font;
use crate::framebuffer::Framebuffer;
use crate::limits::{SCREEN_HEIGHT, SCREEN_WIDTH};

/// SPEC §9.3 — `gfx::pixel`.
pub fn pixel(fb: &mut Framebuffer, x: i64, y: i64, color: u8) {
    fb.set(x, y, color);
}

/// SPEC §6.8 — `gfx::line`, Bresenham's integer DDA.
///
/// Identical arguments produce identical pixels on every platform. The visible
/// range is walked directly rather than clipping the endpoints and restarting
/// the error term, which would plot different pixels.
pub fn line(fb: &mut Framebuffer, x0: i64, y0: i64, x1: i64, y1: i64, color: u8) {
    if x0 == x1 && y0 == y1 {
        fb.set(x0, y0, color);
        return;
    }

    let dx = x1 as i128 - x0 as i128;
    let dy = y1 as i128 - y0 as i128;
    let adx = dx.abs();
    let ady = dy.abs();

    if adx >= ady {
        // X is the major axis, including exact diagonals.
        let sx: i128 = if dx > 0 { 1 } else { -1 };
        let lo = x0.min(x1).max(0);
        let hi = x0.max(x1).min(SCREEN_WIDTH as i64 - 1);
        let mut x = lo;
        while x <= hi {
            let i = (x as i128 - x0 as i128) * sx;
            let y = minor_coord(y0 as i128, dy, i, adx);
            if y >= 0 && y < SCREEN_HEIGHT as i128 {
                fb.set(x, y as i64, color);
            }
            x += 1;
        }
    } else {
        // Y is the major axis.
        let sy: i128 = if dy > 0 { 1 } else { -1 };
        let lo = y0.min(y1).max(0);
        let hi = y0.max(y1).min(SCREEN_HEIGHT as i64 - 1);
        let mut y = lo;
        while y <= hi {
            let i = (y as i128 - y0 as i128) * sy;
            let x = minor_coord(x0 as i128, dx, i, ady);
            if x >= 0 && x < SCREEN_WIDTH as i128 {
                fb.set(x as i64, y, color);
            }
            y += 1;
        }
    }
}

/// SPEC §6.8 — the minor coordinate at major-axis step `i`:
/// `start + round_half_away(delta * i, major)`.
fn minor_coord(start: i128, delta: i128, i: i128, major: i128) -> i128 {
    start + round_half_away(delta * i, major)
}

/// SPEC §6.8 — `round_half_away(n, denom)`, both divisions truncating toward 0.
fn round_half_away(n: i128, denom: i128) -> i128 {
    debug_assert!(denom > 0);
    if n >= 0 {
        (n + denom / 2) / denom
    } else {
        -(((-n) + denom / 2) / denom)
    }
}

/// SPEC §6.6 — `gfx::rect_filled`, half-open `x .. x+w` by `y .. y+h`.
/// `w` or `h` ≤ 0 is a no-op.
pub fn rect_filled(fb: &mut Framebuffer, x: i64, y: i64, w: i64, h: i64, color: u8) {
    if w <= 0 || h <= 0 {
        return;
    }
    let x0 = x.max(0);
    let x1 = (x + w - 1).min(SCREEN_WIDTH as i64 - 1);
    let y0 = y.max(0);
    let y1 = (y + h - 1).min(SCREEN_HEIGHT as i64 - 1);
    let mut cy = y0;
    while cy <= y1 {
        let mut cx = x0;
        while cx <= x1 {
            fb.set(cx, cy, color);
            cx += 1;
        }
        cy += 1;
    }
}

/// SPEC §6.6 — `gfx::rect` outline. `w` or `h` ≤ 0 is a no-op.
pub fn rect(fb: &mut Framebuffer, x: i64, y: i64, w: i64, h: i64, color: u8) {
    if w <= 0 || h <= 0 {
        return;
    }
    let x1 = x + w - 1;
    let y1 = y + h - 1;

    let tx0 = x.max(0);
    let tx1 = x1.min(SCREEN_WIDTH as i64 - 1);
    if y >= 0 && y < SCREEN_HEIGHT as i64 {
        fill_span(fb, y, tx0, tx1, color);
    }
    if y1 >= 0 && y1 < SCREEN_HEIGHT as i64 {
        fill_span(fb, y1, tx0, tx1, color);
    }

    let ty0 = y.max(0);
    let ty1 = y1.min(SCREEN_HEIGHT as i64 - 1);
    if x >= 0 && x < SCREEN_WIDTH as i64 {
        fill_column(fb, x, ty0, ty1, color);
    }
    if x1 >= 0 && x1 < SCREEN_WIDTH as i64 {
        fill_column(fb, x1, ty0, ty1, color);
    }
}

/// SPEC §6.7 — `gfx::circle_filled`: every pixel of the integer disk
/// `dx² + dy² ≤ r²`. `r < 0` is a no-op.
pub fn circle_filled(fb: &mut Framebuffer, cx: i64, cy: i64, r: i64, color: u8) {
    if r < 0 {
        return;
    }
    let r = r as i128;
    let r2 = r * r;
    let top = (cy as i128 - r).max(0);
    let bot = (cy as i128 + r).min(SCREEN_HEIGHT as i128 - 1);
    let mut y = top;
    while y <= bot {
        let dy = y - cy as i128;
        let half = isqrt(r2 - dy * dy);
        let x0 = (cx as i128 - half).max(0);
        let x1 = (cx as i128 + half).min(SCREEN_WIDTH as i128 - 1);
        fill_span_i128(fb, y, x0, x1, color);
        y += 1;
    }
}

/// SPEC §6.7 — `gfx::circle` outline: pixels in the disk of radius `r` but not
/// in the disk of radius `r-1`. `r = 0` is the single centre pixel; `r < 0` is
/// a no-op.
pub fn circle(fb: &mut Framebuffer, cx: i64, cy: i64, r: i64, color: u8) {
    if r < 0 {
        return;
    }
    let r = r as i128;
    let r2 = r * r;
    let inner = r - 1;
    let inner2 = if inner >= 0 { inner * inner } else { -1 };

    let top = (cy as i128 - r).max(0);
    let bot = (cy as i128 + r).min(SCREEN_HEIGHT as i128 - 1);
    let mut y = top;
    while y <= bot {
        let dy = y - cy as i128;
        let outer_half = isqrt(r2 - dy * dy);
        let inner_half = if dy * dy <= inner2 {
            isqrt(inner2 - dy * dy)
        } else {
            -1
        };

        let x_left_lo = (cx as i128 - outer_half).max(0);
        let x_left_hi = (cx as i128 - inner_half - 1).min(SCREEN_WIDTH as i128 - 1);
        if x_left_lo <= x_left_hi {
            fill_span_i128(fb, y, x_left_lo, x_left_hi, color);
        }

        let x_right_lo = (cx as i128 + inner_half + 1).max(0);
        let x_right_hi = (cx as i128 + outer_half).min(SCREEN_WIDTH as i128 - 1);
        if x_right_lo <= x_right_hi {
            fill_span_i128(fb, y, x_right_lo, x_right_hi, color);
        }

        y += 1;
    }
}

/// SPEC §6.5 — `gfx::text`.
pub fn text(fb: &mut Framebuffer, s: &str, x: i64, y: i64, color: u8) {
    font::draw_text(fb, s, x, y, color);
}

/// SPEC §6.5 — `gfx::text_width`.
pub fn text_width(s: &str) -> i64 {
    font::text_width(s)
}

/// Integer square root, exact for any non-negative `i128`.
fn isqrt(n: i128) -> i128 {
    if n <= 0 {
        return 0;
    }
    let mut x = (n as f64).sqrt() as i128;
    while x > 0 && x * x > n {
        x -= 1;
    }
    while (x + 1) * (x + 1) <= n {
        x += 1;
    }
    x
}

fn fill_span(fb: &mut Framebuffer, y: i64, x0: i64, x1: i64, color: u8) {
    let mut x = x0;
    while x <= x1 {
        fb.set(x, y, color);
        x += 1;
    }
}

fn fill_span_i128(fb: &mut Framebuffer, y: i128, x0: i128, x1: i128, color: u8) {
    let mut x = x0;
    while x <= x1 {
        fb.set(x as i64, y as i64, color);
        x += 1;
    }
}

fn fill_column(fb: &mut Framebuffer, x: i64, y0: i64, y1: i64, color: u8) {
    let mut y = y0;
    while y <= y1 {
        fb.set(x, y, color);
        y += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SPEC §20.2 — line `(0,0)`–`(4,-2)` before screen clip, as minor
    /// coordinates at each major-axis step. X is major (`adx = 4 >= ady = 2`).
    #[test]
    fn line_minor_coords_before_clip() {
        let expected = [0, -1, -1, -2, -2];
        for (i, want) in expected.iter().enumerate() {
            let got = minor_coord(0, -2, i as i128, 4);
            assert_eq!(got, *want as i128, "step {i}");
        }
    }

    /// SPEC §20.2 — exact diagonal is X-major and lands on the diagonal.
    #[test]
    fn line_minor_coords_exact_diagonal() {
        let expected = [0, 1, 2, 3];
        for (i, want) in expected.iter().enumerate() {
            assert_eq!(minor_coord(0, 3, i as i128, 3), *want as i128);
        }
    }

    /// SPEC §6.8 — rounding is half away from zero on both signs.
    #[test]
    fn round_half_away_signs() {
        assert_eq!(round_half_away(1, 2), 1);
        assert_eq!(round_half_away(-1, 2), -1);
        assert_eq!(round_half_away(3, 2), 2);
        assert_eq!(round_half_away(-3, 2), -2);
    }

    /// SPEC §6.7 — radius `-1` is empty, radius 0 is the single centre pixel.
    #[test]
    fn circle_radius_edges() {
        let mut fb = Framebuffer::new();
        circle_filled(&mut fb, 10, 10, -1, 5);
        assert_eq!(fb.get(10, 10), Some(0));
        circle_filled(&mut fb, 10, 10, 0, 5);
        assert_eq!(fb.get(10, 10), Some(5));
        assert_eq!(fb.get(9, 10), Some(0));

        let mut fb = Framebuffer::new();
        circle(&mut fb, 10, 10, 0, 7);
        assert_eq!(fb.get(10, 10), Some(7));
    }
}
