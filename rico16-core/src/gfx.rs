//! Drawing primitives over the indexed framebuffer. SPEC Â§6, Â§9.3.
//!
//! These are the kernels behind the `gfx::` script module. They take and return
//! plain integers so the script layer only has to validate ranges, and they
//! never panic: every coordinate is clipped, every degenerate size is a no-op.
//!
//! Two rules matter throughout:
//!
//! - **Overwrite.** A primitive writes its colour literally, including index 0.
//!   There is no blending and no background colour ([Â§6.4](../docs/SPEC.md)).
//! - **Clip before rasterising.** Lines and circles are clipped to the screen
//!   before walking pixels, so an off-screen draw costs nothing and a wild
//!   coordinate cannot turn into a billion-iteration loop.

use crate::font;
use crate::framebuffer::Framebuffer;
use crate::limits::{SCREEN_HEIGHT, SCREEN_WIDTH};

/// Largest radius the rasteriser will walk. The screen diagonal is 400 px, so
/// a bigger circle is a pathological argument; clamping keeps a bad number
/// from freezing the console instead of merely drawing the wrong arc.
const MAX_RASTER_RADIUS: i32 = 4096;

/// Fill the whole framebuffer with `color`. SPEC Â§9.3.
pub fn clear(framebuffer: &mut Framebuffer, color: u8) {
    framebuffer.clear(color);
}

/// Draw one pixel, clipped. SPEC Â§6.1.
pub fn pixel(framebuffer: &mut Framebuffer, x: i32, y: i32, color: u8) {
    framebuffer.pixel(x, y, color);
}

/// Draw a line with inclusive endpoints. SPEC Â§6.1.
///
/// The rasteriser steps once per pixel along the major axis and rounds the
/// minor coordinate, so the line is exactly one pixel thick and cannot leave a
/// gap. Both endpoints are drawn when they are on screen.
pub fn line(framebuffer: &mut Framebuffer, x0: i32, y0: i32, x1: i32, y1: i32, color: u8) {
    let (x0, y0, x1, y1) = (x0 as i64, y0 as i64, x1 as i64, y1 as i64);
    let dx = x1 - x0;
    let dy = y1 - y0;

    if dx == 0 && dy == 0 {
        framebuffer.pixel(to_i32(x0), to_i32(y0), color);
        return;
    }

    let horizontal = dx.abs() >= dy.abs();
    let (major_start, major_delta, minor_start, minor_delta) = if horizontal {
        (x0, dx, y0, dy)
    } else {
        (y0, dy, x0, dx)
    };

    let steps = major_delta.abs();
    let direction = major_delta.signum();
    let major_limit = if horizontal {
        SCREEN_WIDTH as i64 - 1
    } else {
        SCREEN_HEIGHT as i64 - 1
    };

    // Clip the major axis to 0..=limit. Because the minor axis never moves
    // faster than the major one, this also bounds the work to one screen.
    let (mut first, mut last) = (0i64, steps);
    if direction > 0 {
        first = first.max(-major_start);
        last = last.min(major_limit - major_start);
    } else {
        first = first.max(major_start - major_limit);
        last = last.min(major_start);
    }
    if first > last {
        return;
    }

    for step in first..=last {
        let major = major_start + direction * step;
        // The product of two 32-bit spans needs 64 bits of headroom; i128 is
        // the cheapest way to keep extreme coordinates exact.
        let round = div_round(step as i128 * minor_delta as i128, steps as i128);
        let minor = (minor_start as i128 + round).clamp(i64::MIN as i128, i64::MAX as i128) as i64;
        let (x, y) = if horizontal {
            (major, minor)
        } else {
            (minor, major)
        };
        framebuffer.pixel(to_i32(x), to_i32(y), color);
    }
}

/// Draw a one-pixel outline of a half-open rectangle. SPEC Â§6.6.
pub fn rect(framebuffer: &mut Framebuffer, x: i32, y: i32, w: i32, h: i32, color: u8) {
    if w <= 0 || h <= 0 {
        return;
    }

    let (x0, y0) = (x as i64, y as i64);
    let (x1, y1) = (x0 + w as i64 - 1, y0 + h as i64 - 1);

    // Top and bottom edges; `span` clips the rows and columns.
    span(framebuffer, y0, x0, x1 + 1, color);
    span(framebuffer, y1, x0, x1 + 1, color);

    // Left and right edges, with the row loop clipped first.
    let first_row = y0.max(0);
    let last_row = (y1 + 1).min(SCREEN_HEIGHT as i64);
    for row in first_row..last_row {
        framebuffer.pixel(to_i32(x0), to_i32(row), color);
        framebuffer.pixel(to_i32(x1), to_i32(row), color);
    }
}

/// Draw a filled half-open rectangle. SPEC Â§6.6.
pub fn rect_filled(framebuffer: &mut Framebuffer, x: i32, y: i32, w: i32, h: i32, color: u8) {
    framebuffer.fill_rect(x, y, w, h, color);
}

/// Draw a one-pixel circle outline with a fixed midpoint algorithm. SPEC Â§6.7.
pub fn circle(framebuffer: &mut Framebuffer, cx: i32, cy: i32, r: i32, color: u8) {
    if r < 0 || !circle_is_visible(cx, cy, r) {
        return;
    }
    if r == 0 {
        framebuffer.pixel(cx, cy, color);
        return;
    }

    let r = r.min(MAX_RASTER_RADIUS) as i64;
    let (cx, cy) = (cx as i64, cy as i64);
    let (mut x, mut y) = (r, 0i64);
    let mut decision = 1 - r;

    while x >= y {
        plot_octants(framebuffer, cx, cy, x, y, color);
        y += 1;
        if decision < 0 {
            decision += 2 * y + 1;
        } else {
            x -= 1;
            decision += 2 * (y - x) + 1;
        }
    }
}

/// Draw a filled circle with the same midpoint walk as [`circle`]. SPEC Â§6.7.
pub fn circle_filled(framebuffer: &mut Framebuffer, cx: i32, cy: i32, r: i32, color: u8) {
    if r < 0 || !circle_is_visible(cx, cy, r) {
        return;
    }
    if r == 0 {
        framebuffer.pixel(cx, cy, color);
        return;
    }

    let r = r.min(MAX_RASTER_RADIUS) as i64;
    let (cx, cy) = (cx as i64, cy as i64);
    let (mut x, mut y) = (r, 0i64);
    let mut decision = 1 - r;

    while x >= y {
        span(framebuffer, cy + y, cx - x, cx + x + 1, color);
        span(framebuffer, cy - y, cx - x, cx + x + 1, color);
        span(framebuffer, cy + x, cx - y, cx + y + 1, color);
        span(framebuffer, cy - x, cx - y, cx + y + 1, color);

        y += 1;
        if decision < 0 {
            decision += 2 * y + 1;
        } else {
            x -= 1;
            decision += 2 * (y - x) + 1;
        }
    }
}

/// Draw text in the built-in 8Ã—8 font. SPEC Â§6.5.
///
/// `\n` moves down eight pixels and returns x to the call's origin. `\r` is
/// ignored, so CRLF sources print without stray boxes. Any other code point
/// the font does not cover draws a placeholder box, never a panic.
pub fn text(framebuffer: &mut Framebuffer, text: &str, x: i32, y: i32, color: u8) {
    let start_x = x as i64;
    let mut pen_x = start_x;
    let mut pen_y = y as i64;

    for character in text.chars() {
        match character {
            '\n' => {
                pen_x = start_x;
                pen_y += font::GLYPH_HEIGHT as i64;
            }
            '\r' => {}
            _ => {
                let bitmap = font::glyph(character);
                for (row, bits) in bitmap.iter().enumerate() {
                    for column in 0..font::GLYPH_WIDTH {
                        if bits & (1 << column) != 0 {
                            framebuffer.pixel(
                                to_i32(pen_x + column as i64),
                                to_i32(pen_y + row as i64),
                                color,
                            );
                        }
                    }
                }
                pen_x += font::GLYPH_WIDTH as i64;
            }
        }
    }
}

/// Width of the first line of `text`, in pixels. SPEC Â§9.3.
pub fn text_width(text: &str) -> i32 {
    let columns = text
        .chars()
        .take_while(|character| *character != '\n')
        .count();
    // A string can be longer than i32 pixels in theory; saturate rather than
    // wrap, because the value is only ever used for layout.
    (columns as i64 * font::GLYPH_WIDTH as i64).min(i32::MAX as i64) as i32
}

fn circle_is_visible(cx: i32, cy: i32, r: i32) -> bool {
    let (cx, cy, r) = (cx as i64, cy as i64, r as i64);
    cx + r >= 0 && cx - r < SCREEN_WIDTH as i64 && cy + r >= 0 && cy - r < SCREEN_HEIGHT as i64
}

fn plot_octants(framebuffer: &mut Framebuffer, cx: i64, cy: i64, x: i64, y: i64, color: u8) {
    for (dx, dy) in [
        (x, y),
        (y, x),
        (-x, y),
        (-y, x),
        (x, -y),
        (y, -x),
        (-x, -y),
        (-y, -x),
    ] {
        framebuffer.pixel(to_i32(cx + dx), to_i32(cy + dy), color);
    }
}

/// Clipped horizontal span on a possibly off-screen row. SPEC Â§6.1.
fn span(framebuffer: &mut Framebuffer, y: i64, x0: i64, x1: i64, color: u8) {
    if !(0..SCREEN_HEIGHT as i64).contains(&y) {
        return;
    }
    let x0 = x0.clamp(0, SCREEN_WIDTH as i64) as i32;
    let x1 = x1.clamp(0, SCREEN_WIDTH as i64) as i32;
    framebuffer.fill_span(y as i32, x0, x1, color);
}

/// Round `numerator / denominator` to nearest, half away from zero.
///
/// Deterministic integer arithmetic: no floats, no platform-dependent rounding
/// mode. `denominator` is always positive and non-zero.
fn div_round(numerator: i128, denominator: i128) -> i128 {
    debug_assert!(denominator > 0);
    if numerator >= 0 {
        (numerator + denominator / 2) / denominator
    } else {
        -((-numerator + denominator / 2) / denominator)
    }
}

fn to_i32(value: i64) -> i32 {
    value.clamp(i32::MIN as i64, i32::MAX as i64) as i32
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

    fn lit(framebuffer: &Framebuffer) -> Vec<(i32, i32)> {
        let mut pixels = Vec::new();
        for y in 0..SCREEN_HEIGHT as i32 {
            for x in 0..SCREEN_WIDTH as i32 {
                if framebuffer.get(x, y) != Some(0) {
                    pixels.push((x, y));
                }
            }
        }
        pixels
    }

    fn count(framebuffer: &Framebuffer) -> usize {
        framebuffer
            .indices()
            .iter()
            .filter(|index| **index != 0)
            .count()
    }

    #[test]
    fn clear_writes_color_literally_including_zero() {
        let mut framebuffer = blank();
        clear(&mut framebuffer, 9);
        assert!(framebuffer.indices().iter().all(|index| *index == 9));
        clear(&mut framebuffer, 0);
        assert!(framebuffer.indices().iter().all(|index| *index == 0));
        assert_eq!(framebuffer.indices().len(), FRAMEBUFFER_LEN);
    }

    #[test]
    fn pixel_is_clipped_at_every_edge() {
        let mut framebuffer = blank();
        for (x, y) in [
            (-1, 0),
            (0, -1),
            (SCREEN_WIDTH as i32, 0),
            (0, SCREEN_HEIGHT as i32),
            (-100, -100),
        ] {
            pixel(&mut framebuffer, x, y, 3);
        }
        assert_eq!(count(&framebuffer), 0);

        pixel(&mut framebuffer, 0, 0, 3);
        pixel(
            &mut framebuffer,
            SCREEN_WIDTH as i32 - 1,
            SCREEN_HEIGHT as i32 - 1,
            4,
        );
        assert_eq!(framebuffer.get(0, 0), Some(3));
        assert_eq!(
            framebuffer.get(SCREEN_WIDTH as i32 - 1, SCREEN_HEIGHT as i32 - 1),
            Some(4)
        );
    }

    #[test]
    fn horizontal_line_includes_both_endpoints() {
        let mut framebuffer = blank();
        line(&mut framebuffer, 2, 3, 6, 3, 5);
        let pixels = lit(&framebuffer);
        assert_eq!(pixels, vec![(2, 3), (3, 3), (4, 3), (5, 3), (6, 3)]);
    }

    #[test]
    fn vertical_line_includes_both_endpoints() {
        let mut framebuffer = blank();
        line(&mut framebuffer, 4, 1, 4, 5, 5);
        let pixels = lit(&framebuffer);
        assert_eq!(pixels, vec![(4, 1), (4, 2), (4, 3), (4, 4), (4, 5)]);
    }

    #[test]
    fn diagonal_line_is_contiguous_and_ordered_either_way() {
        let mut forward = blank();
        line(&mut forward, 0, 0, 5, 5, 5);
        assert_eq!(
            lit(&forward),
            vec![(0, 0), (1, 1), (2, 2), (3, 3), (4, 4), (5, 5)]
        );

        let mut backward = blank();
        line(&mut backward, 5, 5, 0, 0, 5);
        assert_eq!(lit(&forward), lit(&backward));
    }

    #[test]
    fn a_shallow_line_is_one_pixel_per_column() {
        let mut framebuffer = blank();
        line(&mut framebuffer, 0, 0, 2, 1, 5);
        assert_eq!(lit(&framebuffer), vec![(0, 0), (1, 1), (2, 1)]);
    }

    #[test]
    fn a_steep_line_is_one_pixel_per_row() {
        let mut framebuffer = blank();
        line(&mut framebuffer, 1, 0, 0, 2, 5);
        let pixels = lit(&framebuffer);
        assert_eq!(pixels.len(), 3);
        assert_eq!(
            pixels.iter().map(|(_, y)| *y).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
    }

    #[test]
    fn a_line_wholly_off_screen_draws_nothing() {
        let mut framebuffer = blank();
        line(&mut framebuffer, -10, -10, -5, -20, 5);
        line(&mut framebuffer, 400, 0, 500, 300, 5);
        line(&mut framebuffer, 0, 300, 300, 400, 5);
        assert_eq!(count(&framebuffer), 0);
    }

    #[test]
    fn a_line_crossing_the_edge_is_clipped_pixel_exactly() {
        let mut framebuffer = blank();
        line(&mut framebuffer, -5, 2, 5, 2, 5);
        assert_eq!(
            lit(&framebuffer),
            (0..=5).map(|x| (x, 2)).collect::<Vec<_>>()
        );

        let mut framebuffer = blank();
        line(&mut framebuffer, 0, 0, 400, 0, 5);
        assert_eq!(
            lit(&framebuffer),
            (0..SCREEN_WIDTH as i32).map(|x| (x, 0)).collect::<Vec<_>>()
        );
    }

    #[test]
    fn extreme_line_coordinates_terminate() {
        // Without clipping this segment would need four billion iterations.
        let mut framebuffer = blank();
        line(&mut framebuffer, i32::MIN, i32::MAX, i32::MAX, i32::MIN, 5);
        assert_eq!(count(&framebuffer), 0);
    }

    #[test]
    fn rect_outline_covers_exactly_the_border() {
        let mut framebuffer = blank();
        rect(&mut framebuffer, 1, 1, 5, 4, 5);

        let expected: Vec<(i32, i32)> = vec![
            (1, 1),
            (2, 1),
            (3, 1),
            (4, 1),
            (5, 1),
            (1, 2),
            (5, 2),
            (1, 3),
            (5, 3),
            (1, 4),
            (2, 4),
            (3, 4),
            (4, 4),
            (5, 4),
        ];
        assert_eq!(lit(&framebuffer), expected);
    }

    #[test]
    fn degenerate_rectangles_draw_nothing() {
        let mut framebuffer = blank();
        for (w, h) in [(0, 5), (5, 0), (-1, 5), (5, -1)] {
            rect(&mut framebuffer, 10, 10, w, h, 5);
            rect_filled(&mut framebuffer, 10, 10, w, h, 5);
        }
        assert_eq!(count(&framebuffer), 0);
    }

    #[test]
    fn one_pixel_rect_is_one_pixel() {
        let mut framebuffer = blank();
        rect(&mut framebuffer, 4, 4, 1, 1, 5);
        assert_eq!(lit(&framebuffer), vec![(4, 4)]);
    }

    #[test]
    fn rect_outline_clips_without_adding_boundary_edges() {
        let mut framebuffer = blank();
        rect(&mut framebuffer, -3, -3, 8, 8, 5);

        // The visible part is the bottom and right edges inside the screen.
        assert_eq!(
            framebuffer.get(0, 0),
            Some(0),
            "no drawn corner at the screen edge"
        );
        assert_eq!(framebuffer.get(4, 0), Some(5));
        assert_eq!(framebuffer.get(0, 4), Some(5));
        assert_eq!(framebuffer.get(4, 4), Some(5));
        assert_eq!(framebuffer.get(4, 5), Some(0));
    }

    #[test]
    fn filled_rect_is_exactly_the_half_open_area() {
        let mut framebuffer = blank();
        rect_filled(&mut framebuffer, 2, 2, 3, 2, 7);
        assert_eq!(
            lit(&framebuffer),
            vec![(2, 2), (3, 2), (4, 2), (2, 3), (3, 3), (4, 3)]
        );
        assert_eq!(framebuffer.get(5, 2), Some(0));
        assert_eq!(framebuffer.get(2, 4), Some(0));
    }

    #[test]
    fn circle_of_radius_zero_is_one_pixel() {
        let mut framebuffer = blank();
        circle(&mut framebuffer, 10, 10, 0, 5);
        circle_filled(&mut framebuffer, 20, 10, 0, 5);
        assert_eq!(lit(&framebuffer), vec![(10, 10), (20, 10)]);
    }

    #[test]
    fn negative_radius_draws_nothing() {
        let mut framebuffer = blank();
        circle(&mut framebuffer, 10, 10, -1, 5);
        circle_filled(&mut framebuffer, 10, 10, -1, 5);
        assert_eq!(count(&framebuffer), 0);
    }

    #[test]
    fn a_circle_wholly_off_screen_draws_nothing() {
        let mut framebuffer = blank();
        circle(&mut framebuffer, -100, -100, 10, 5);
        circle_filled(&mut framebuffer, 500, 500, 10, 5);
        circle(&mut framebuffer, 160, 120, i32::MAX, 5);
        assert_eq!(count(&framebuffer), 0);
    }

    #[test]
    fn circle_is_symmetric_and_matches_the_pinned_radius_three() {
        let mut framebuffer = blank();
        circle(&mut framebuffer, 50, 50, 3, 5);
        let pixels = lit(&framebuffer);

        for (x, y) in &pixels {
            let dx = x - 50;
            let dy = y - 50;
            for (sx, sy) in [
                (dx, dy),
                (-dx, dy),
                (dx, -dy),
                (-dx, -dy),
                (dy, dx),
                (-dy, dx),
            ] {
                assert!(
                    pixels.contains(&(50 + sx, 50 + sy)),
                    "({dx},{dy}) is missing its mirror ({sx},{sy})"
                );
            }
        }

        // Midpoint decision variable, pinned: the canonical 16-pixel circle.
        assert_eq!(pixels.len(), 16);
        assert!(pixels.contains(&(53, 50)));
        assert!(pixels.contains(&(53, 51)));
        assert!(pixels.contains(&(52, 52)));
        assert!(!pixels.contains(&(51, 51)));
    }

    #[test]
    fn filled_circle_contains_the_outline_and_nothing_far_outside() {
        let mut outline = blank();
        circle(&mut outline, 100, 100, 20, 5);

        let mut filled = blank();
        circle_filled(&mut filled, 100, 100, 20, 5);

        for (x, y) in lit(&outline) {
            assert_eq!(
                filled.get(x, y),
                Some(5),
                "outline pixel ({x},{y}) is missing"
            );
        }
        assert_eq!(filled.get(100, 100), Some(5));
        assert_eq!(filled.get(100 + 20, 100 + 20), Some(0));
    }

    #[test]
    fn circles_clip_at_the_screen_edge() {
        let mut framebuffer = blank();
        circle(&mut framebuffer, 0, 0, 5, 5);
        // The arc inside the screen is the bottom-right quarter.
        assert!(lit(&framebuffer).iter().all(|(x, y)| *x >= 0 && *y >= 0));
        assert!(count(&framebuffer) > 0);
    }

    #[test]
    fn text_draws_the_glyph_bits_at_the_origin() {
        let mut framebuffer = blank();
        text(&mut framebuffer, "A", 10, 20, 5);

        // Row 0 of 'A' is 0x0C: columns 2 and 3.
        assert_eq!(framebuffer.get(12, 20), Some(5));
        assert_eq!(framebuffer.get(13, 20), Some(5));
        assert_eq!(framebuffer.get(10, 20), Some(0));
        assert_eq!(framebuffer.get(14, 20), Some(0));

        // Row 4 is 0x3F: columns 0..=5.
        assert_eq!(framebuffer.get(10, 24), Some(5));
        assert_eq!(framebuffer.get(15, 24), Some(5));
        assert_eq!(framebuffer.get(16, 24), Some(0));
    }

    #[test]
    fn text_advances_eight_pixels_and_wraps_on_newline() {
        let mut framebuffer = blank();
        text(&mut framebuffer, "AB", 0, 0, 5);
        assert_eq!(text_width("AB"), 16);
        assert_eq!(text_width("AB\nLONGER"), 16);
        assert_eq!(text_width(""), 0);
        assert_eq!(text_width("\n"), 0);

        // 'B' starts at x=8.
        assert_eq!(framebuffer.get(8, 0), Some(5));

        framebuffer.clear(0);
        text(&mut framebuffer, "A\nB", 10, 10, 5);
        assert_eq!(framebuffer.get(12, 10), Some(5), "first line");
        assert_eq!(framebuffer.get(10, 18), Some(5), "second line returns to x");
        assert_eq!(framebuffer.get(12, 18), Some(5));
    }

    #[test]
    fn carriage_returns_are_ignored() {
        let mut framebuffer = blank();
        text(&mut framebuffer, "AB", 0, 0, 5);
        let expected = lit(&framebuffer);

        let mut crlf = blank();
        text(&mut crlf, "A\rB", 0, 0, 5);
        assert_eq!(lit(&crlf), expected);
    }

    #[test]
    fn unknown_characters_draw_a_placeholder_box() {
        let mut framebuffer = blank();
        text(&mut framebuffer, "Î©", 0, 0, 5);
        assert_eq!(framebuffer.get(1, 0), Some(5));
        assert_eq!(framebuffer.get(6, 0), Some(5));
        assert_eq!(framebuffer.get(0, 0), Some(0));
        assert_eq!(framebuffer.get(1, 1), Some(5));
        assert_eq!(framebuffer.get(3, 1), Some(0));
    }

    #[test]
    fn text_clips_at_every_edge() {
        let mut framebuffer = blank();
        text(&mut framebuffer, "A", -4, -4, 5);
        assert!(lit(&framebuffer).iter().all(|(x, y)| *x >= 0 && *y >= 0));
        assert!(count(&framebuffer) > 0);

        let mut framebuffer = blank();
        text(
            &mut framebuffer,
            "hello",
            SCREEN_WIDTH as i32 - 8,
            SCREEN_HEIGHT as i32 - 8,
            5,
        );
        assert!(count(&framebuffer) > 0);
    }

    #[test]
    fn text_does_not_overflow_on_absurd_origins() {
        let mut framebuffer = blank();
        text(&mut framebuffer, "wide", i32::MAX, i32::MAX, 5);
        text(&mut framebuffer, "wide", i32::MIN, i32::MIN, 5);
        assert_eq!(count(&framebuffer), 0);
    }
}
