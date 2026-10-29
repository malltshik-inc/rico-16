//! SPEC §20 — the normative conformance vectors, plus the drawing edge cases
//! AGENTS.md requires. A runtime that fails any of these is non-conforming.

use rico16_core::font;
use rico16_core::framebuffer::Framebuffer;
use rico16_core::gfx;
use rico16_core::limits::{DT, FRAMEBUFFER_LEN, PALETTE_LEN};
use rico16_core::palette::{self, Palette};
use rico16_core::rng::Rng;

/// SPEC §20.1 — the seeded xoshiro256** state and its first draws.
#[test]
fn rng_seed_and_outputs() {
    let rng = Rng::seed(0);
    assert_eq!(
        rng.state(),
        [
            0xE220_A839_7B1D_CDAF,
            0x6E78_9E6A_A1B9_65F4,
            0x06C4_5D18_8009_454F,
            0xF88B_B8A8_724C_81EC,
        ]
    );

    let mut rng = Rng::seed(0);
    let draws: Vec<u64> = (0..4).map(|_| rng.next_u64()).collect();
    assert_eq!(
        draws,
        [
            0x99EC_5F36_CB75_F2B4,
            0xBF6E_1F78_4956_452A,
            0x1A5F_849D_4933_E6E0,
            0x6AA5_94F1_262D_2D2C,
        ]
    );
}

/// SPEC §20.1 — `rnd::float` draws as IEEE-754 bit patterns.
#[test]
fn rng_float_bits() {
    let mut rng = Rng::seed(0);
    let got: Vec<u64> = (0..4).map(|_| rng.float().to_bits()).collect();
    assert_eq!(
        got,
        [
            0x3FE3_3D8B_E6D9_6EBE,
            0x3FE7_EDC3_EF09_2AC8,
            0x3FBA_5F84_9D49_33E0,
            0x3FDA_A965_3C49_8B4A,
        ]
    );
}

/// SPEC §20.1 — `rnd::int(1, 6)` eight times from a fresh seed 0.
#[test]
fn rng_int_range() {
    let mut rng = Rng::seed(0);
    let got: Vec<i64> = (0..8).map(|_| rng.int(1, 6).unwrap()).collect();
    assert_eq!(got, [4, 5, 1, 3, 5, 6, 3, 4]);
}

/// SPEC §20.1 — `rnd::shuffle` of `[0, 1, 2, 3]` from a fresh seed 0.
#[test]
fn rng_shuffle() {
    let mut rng = Rng::seed(0);
    let mut arr = [0, 1, 2, 3];
    rng.shuffle(&mut arr);
    assert_eq!(arr, [1, 0, 3, 2]);
}

/// SPEC §20.1 — an empty `rnd::choice` draws nothing.
#[test]
fn rng_choice_empty_draws_nothing() {
    let mut rng = Rng::seed(0);
    assert_eq!(rng.choice_index(0), None);
    // The stream is untouched: the next int still matches the vector.
    assert_eq!(rng.int(1, 6).unwrap(), 4);
}

/// SPEC §20.2 — line `(0,0)`–`(4,-2)`: only `(0,0)` is on screen.
#[test]
fn line_steep_offscreen() {
    let mut fb = Framebuffer::new();
    gfx::line(&mut fb, 0, 0, 4, -2, 7);
    assert_eq!(fb.get(0, 0), Some(7));
    let lit = count_nonzero(&fb);
    assert_eq!(lit, 1, "only (0,0) is on screen");
}

/// SPEC §20.2 — line `(0,0)`–`(3,3)` draws the four diagonal pixels.
#[test]
fn line_exact_diagonal() {
    let mut fb = Framebuffer::new();
    gfx::line(&mut fb, 0, 0, 3, 3, 7);
    for i in 0..4 {
        assert_eq!(fb.get(i, i), Some(7), "({i},{i})");
    }
    assert_eq!(count_nonzero(&fb), 4);
}

/// SPEC §20.3 — disk counts at radius 0, 1, 2 and the radius-2 outline.
#[test]
fn disk_counts_and_outline() {
    for (r, want) in [(0, 1), (1, 5), (2, 13)] {
        let mut fb = Framebuffer::new();
        gfx::circle_filled(&mut fb, 100, 100, r, 3);
        assert_eq!(count_nonzero(&fb), want, "filled radius {r}");
    }

    let mut fb = Framebuffer::new();
    gfx::circle(&mut fb, 100, 100, 2, 3);
    assert_eq!(count_nonzero(&fb), 8, "outline radius 2");
    for (dx, dy) in [
        (2, 0),
        (-2, 0),
        (0, 2),
        (0, -2),
        (1, 1),
        (-1, 1),
        (1, -1),
        (-1, -1),
    ] {
        assert_eq!(fb.get(100 + dx, 100 + dy), Some(3), "({dx},{dy})");
    }
    assert_eq!(
        fb.get(100, 100),
        Some(0),
        "outline does not paint the interior"
    );
}

/// SPEC §20.4 — the `rgb332` preset.
#[test]
fn rgb332_preset() {
    let p = palette::rgb332();
    assert_eq!(p[0], 0x0000);
    assert_eq!(p[0b1110_0000], 0x001F);
    assert_eq!(p[0b0001_1100], 0x03E0);
    assert_eq!(p[3], 0x7C00);
    assert_eq!(p[255], 0x7FFF);
}

/// SPEC §20.5 — palette anchors and RGB8 expansion.
#[test]
fn palette_anchors() {
    let p = Palette::new();
    assert_eq!(p.word(0), 0x0000);
    assert_eq!(p.word(5), 0x7FFF);
    assert_eq!(p.word(6), 0x0C7F);

    let mut rgba = [0u8; PALETTE_LEN * 4];
    p.expand_rgba8(&mut rgba);
    assert_eq!(&rgba[6 * 4..6 * 4 + 4], &[255, 24, 24, 255]);
}

/// SPEC §20.7 — `dt` is exactly `1/60`.
#[test]
fn dt_bits() {
    assert_eq!(DT.to_bits(), 0x3F91_1111_1111_1111);
}

/// SPEC §20.8 — the quantizers.
#[test]
fn quantizers() {
    assert_eq!(palette::quantize5(255), 31);
    assert_eq!(palette::quantize5(0), 0);
    assert_eq!(palette::quantize5(128), 16);
    let max: u32 = 255;
    let zero: u32 = 0;
    assert_eq!((max * 15 + 127) / 255, 15);
    assert_eq!((zero * 15 + 127) / 255, 0);
}

/// SPEC §6.6 — `rect_filled` is half-open; `w` or `h` ≤ 0 is a no-op.
#[test]
fn rect_filled_bounds() {
    let mut fb = Framebuffer::new();
    gfx::rect_filled(&mut fb, 2, 2, 3, 2, 9);
    assert_eq!(count_nonzero(&fb), 6);
    assert_eq!(fb.get(2, 2), Some(9));
    assert_eq!(fb.get(4, 3), Some(9));
    assert_eq!(fb.get(5, 2), Some(0), "x range is half-open");
    assert_eq!(fb.get(2, 4), Some(0), "y range is half-open");

    let mut fb = Framebuffer::new();
    gfx::rect_filled(&mut fb, 0, 0, 0, 5, 9);
    gfx::rect_filled(&mut fb, 0, 0, 5, -1, 9);
    assert_eq!(count_nonzero(&fb), 0);
}

/// SPEC §6.1 — draws clip at every edge and fully off-screen draws are no-ops.
#[test]
fn clipping_edges() {
    let mut fb = Framebuffer::new();
    gfx::rect_filled(&mut fb, -5, -5, 10, 10, 1);
    assert_eq!(count_nonzero(&fb), 25);

    let mut fb = Framebuffer::new();
    gfx::rect_filled(&mut fb, 318, 238, 10, 10, 1);
    assert_eq!(count_nonzero(&fb), 4);

    let mut fb = Framebuffer::new();
    gfx::rect_filled(&mut fb, -100, -100, 10, 10, 1);
    gfx::rect_filled(&mut fb, 1000, 1000, 10, 10, 1);
    gfx::line(&mut fb, -50, -50, -10, -10, 1);
    gfx::circle_filled(&mut fb, -100, -100, 5, 1);
    assert_eq!(count_nonzero(&fb), 0, "fully off-screen draws are no-ops");
}

/// SPEC §6.5 — the font bitmap is bit 0 = leftmost, so glyphs are not mirrored.
#[test]
fn font_bit_order_is_lsb_first() {
    // 'L' = `0F 06 06 06 46 66 7F 00`: a top bar on the left and a left stem.
    let mut fb = Framebuffer::new();
    gfx::text(&mut fb, "L", 0, 0, 1);

    for col in 0..4 {
        assert_eq!(fb.get(col, 0), Some(1), "top bar covers cols 0..4");
    }
    for col in 4..8 {
        assert_eq!(fb.get(col, 0), Some(0), "top bar does not reach col {col}");
    }
    assert_eq!(fb.get(1, 1), Some(1), "stem is on the left");
    assert_eq!(fb.get(2, 1), Some(1));
    assert_eq!(fb.get(5, 1), Some(0), "stem is not mirrored to the right");
    assert_eq!(fb.get(7, 7), Some(0));
}

/// SPEC §6.5 — `gfx::text_width` and ink-only text, including the placeholder.
#[test]
fn text_and_width() {
    assert_eq!(gfx::text_width("abc"), 24);
    assert_eq!(gfx::text_width("ab\ncd"), 16);
    assert_eq!(gfx::text_width(""), 0);

    let mut fb = Framebuffer::new();
    gfx::text(&mut fb, " ", 0, 0, 1);
    assert_eq!(count_nonzero(&fb), 0, "space has no ink");

    let mut fb = Framebuffer::new();
    gfx::text(&mut fb, "\u{2603}", 0, 0, 1);
    assert_eq!(
        count_nonzero(&fb),
        28,
        "placeholder box: 8 + 6*2 + 8 ink pixels"
    );
}

/// SPEC §3.5, §3.6 — a fresh framebuffer is zeros and expands through the palette.
#[test]
fn framebuffer_start_and_expand() {
    let fb = Framebuffer::new();
    assert!(fb.bytes().iter().all(|&b| b == 0));
    let mut rgba = vec![0u8; FRAMEBUFFER_LEN * 4];
    fb.expand_into(&Palette::new(), &mut rgba);
    assert_eq!(&rgba[0..4], &[0, 0, 0, 255]);
}

/// SPEC §6.5 — the font table covers printable ASCII 32–126.
#[test]
fn font_covers_ascii() {
    assert_eq!(font::FONT.len(), 95);
    assert_eq!(font::glyph('A'), &font::FONT[('A' as u32 - 0x20) as usize]);
    assert_eq!(font::glyph('\u{2603}'), &font::PLACEHOLDER);
}

fn count_nonzero(fb: &Framebuffer) -> usize {
    fb.bytes().iter().filter(|&&b| b != 0).count()
}
