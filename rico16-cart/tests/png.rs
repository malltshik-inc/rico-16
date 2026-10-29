//! SPEC §13.2, §13.4 — PNG packing and unpacking.

use rico16_cart::binary::BinaryCart;
use rico16_cart::png::{
    self, Variant, cover_fit, decode_png_rgba8, decode_png_rgba8_strict, encode_png,
    generated_label, pack, unpack,
};
use rico16_core::limits::{PALETTE_LEN, SHEET_LEN};
use rico16_core::palette::DEFAULT_PALETTE;

fn cart_bytes() -> Vec<u8> {
    let cart = BinaryCart {
        code: "fn on_draw() {}\n".to_string(),
        modules: vec![],
        metadata: "title = \"t\"\nauthor = \"a\"\nversion = \"\"\nentry = \"src/main.rhai\"\n"
            .to_string(),
        sheet: vec![0u8; SHEET_LEN],
        map: vec![0u8; 16_384],
        palette: DEFAULT_PALETTE,
    };
    cart.encode().unwrap()
}

/// SPEC §13.2 — pack then unpack returns the same binary cart.
#[test]
fn pack_unpack_round_trip() {
    let bytes = cart_bytes();
    let art = generated_label("Snake", "me", &DEFAULT_PALETTE, Variant::DEFAULT);
    let png = pack(&bytes, Variant::DEFAULT, &art).expect("pack");
    let (cart, variant) = unpack(&png).expect("unpack");
    assert_eq!(variant, Variant::DEFAULT);
    assert_eq!(cart.encode().unwrap(), bytes);
}

/// SPEC §13.2.3 — the blob header sits in the first 16 bytes of the payload.
#[test]
fn blob_header_fields() {
    let bytes = cart_bytes();
    let art = generated_label("t", "", &DEFAULT_PALETTE, Variant::DEFAULT);
    let png = pack(&bytes, Variant::DEFAULT, &art).unwrap();
    let (_, _, rgba) = decode_png_rgba8_strict(&png).unwrap();

    // Reassemble the payload the same way `unpack` does.
    let mut nibbles = Vec::new();
    for p in 0..Variant::DEFAULT.pixels() {
        nibbles.push(rgba[p * 4] & 0x0F);
        nibbles.push(rgba[p * 4 + 1] & 0x0F);
        nibbles.push(rgba[p * 4 + 2] & 0x0F);
    }
    let mut payload = Vec::new();
    for pair in nibbles.chunks_exact(2) {
        payload.push((pair[0] << 4) | pair[1]);
    }
    assert_eq!(&payload[0..4], b"R16P");
    assert_eq!(u16::from_le_bytes([payload[4], payload[5]]), 1);
    assert_eq!(u16::from_le_bytes([payload[6], payload[7]]), 0, "variant 0");
    assert_eq!(
        u32::from_le_bytes([payload[8], payload[9], payload[10], payload[11]]) as usize,
        bytes.len()
    );
    assert_eq!(
        u32::from_le_bytes([payload[12], payload[13], payload[14], payload[15]]),
        rico16_cart::crc32(&bytes)
    );
}

/// SPEC §13.2 — every pixel is opaque and the payload hides in the low nibbles.
#[test]
fn alpha_is_opaque_and_art_is_high_nibbles() {
    let bytes = cart_bytes();
    let art = vec![[9u8, 5, 3]; Variant::DEFAULT.pixels()];
    let png = pack(&bytes, Variant::DEFAULT, &art).unwrap();
    let (w, h, rgba) = decode_png_rgba8_strict(&png).unwrap();
    assert_eq!((w, h), (640, 480));
    for p in 0..Variant::DEFAULT.pixels() {
        assert_eq!(rgba[p * 4 + 3], 0xFF, "alpha always 0xFF");
        assert_eq!(rgba[p * 4] >> 4, 9, "high nibble is cover art");
        assert_eq!(rgba[p * 4 + 1] >> 4, 5);
        assert_eq!(rgba[p * 4 + 2] >> 4, 3);
    }
}

/// SPEC §13.2.3 — nibbles after the payload must be zero, and we detect when
/// they are not.
#[test]
fn rejects_dirty_trailing_nibbles() {
    let bytes = cart_bytes();
    let art = generated_label("t", "", &DEFAULT_PALETTE, Variant::DEFAULT);
    let png = pack(&bytes, Variant::DEFAULT, &art).unwrap();
    let (w, h, mut rgba) = decode_png_rgba8_strict(&png).unwrap();

    // Set a low nibble far past the payload.
    let last = (w as usize * h as usize - 1) * 4;
    rgba[last] |= 0x0F;
    let dirty = encode_png(w, h, &rgba).unwrap();
    let err = unpack(&dirty).unwrap_err().to_string();
    assert!(err.contains("non-zero nibble"), "{err}");
}

/// SPEC §13.4 — a corrupted payload fails the CRC check.
#[test]
fn rejects_crc_mismatch() {
    let bytes = cart_bytes();
    let art = generated_label("t", "", &DEFAULT_PALETTE, Variant::DEFAULT);
    let png = pack(&bytes, Variant::DEFAULT, &art).unwrap();
    let (w, h, mut rgba) = decode_png_rgba8_strict(&png).unwrap();
    // Flip a payload nibble inside the cart body (after the 16-byte blob header).
    let at = 200 * 4;
    rgba[at] ^= 0x01;
    let broken = encode_png(w, h, &rgba).unwrap();
    let err = unpack(&broken).unwrap_err().to_string();
    assert!(err.contains("CRC"), "{err}");
}

/// SPEC §13.4 — a wrong-sized PNG is rejected.
#[test]
fn rejects_wrong_dimensions() {
    let rgba = vec![0u8; 64 * 64 * 4];
    let png = encode_png(64, 64, &rgba).unwrap();
    assert!(unpack(&png).is_err());
}

/// SPEC §13.2.2 — a payload larger than the default capacity needs the overflow
/// variant.
#[test]
fn overflow_variant_selected() {
    let big = vec![0u8; Variant::DEFAULT.capacity() + 1];
    let required = 16 + big.len();
    assert!(required > Variant::DEFAULT.capacity());
    assert!(required <= Variant::OVERFLOW.capacity());
    let art = generated_label("big", "", &DEFAULT_PALETTE, Variant::OVERFLOW);
    let png = pack(&big, Variant::OVERFLOW, &art).unwrap();
    let (w, h, _) = decode_png_rgba8_strict(&png).unwrap();
    assert_eq!((w, h), (1280, 960));
}

/// SPEC §13.2.2 — a payload that cannot fit any variant is a capacity error.
#[test]
fn capacity_error() {
    let huge = vec![0u8; Variant::OVERFLOW.capacity() + 1];
    let art = vec![[0u8; 3]; Variant::OVERFLOW.pixels()];
    let err = pack(&huge, Variant::OVERFLOW, &art)
        .unwrap_err()
        .to_string();
    assert!(err.contains("capacity"), "{err}");
}

/// SPEC §13.2.4 — the generated label is deterministic in content.
#[test]
fn generated_label_is_deterministic() {
    let a = generated_label("Snake", "me", &DEFAULT_PALETTE, Variant::DEFAULT);
    let b = generated_label("Snake", "me", &DEFAULT_PALETTE, Variant::DEFAULT);
    assert_eq!(a, b);
    assert_eq!(a.len(), Variant::DEFAULT.pixels());
}

/// SPEC §13.2.5 — a label that is already the variant size maps to itself.
#[test]
fn cover_fit_identity() {
    let w = Variant::DEFAULT.width;
    let h = Variant::DEFAULT.height;
    let mut rgba = vec![0u8; (w as usize * h as usize) * 4];
    for (i, px) in rgba.chunks_exact_mut(4).enumerate() {
        px[0] = (i % 256) as u8;
        px[1] = 128;
        px[2] = 255;
        px[3] = 255;
    }
    let art = cover_fit(&rgba, w, h, Variant::DEFAULT);
    assert_eq!(art.len(), Variant::DEFAULT.pixels());
    for (i, a) in art.iter().enumerate() {
        let r = (i % 256) as u8;
        assert_eq!(a[0], ((r as u32 * 15 + 127) / 255) as u8);
        assert_eq!(a[1], ((128u32 * 15 + 127) / 255) as u8);
        assert_eq!(a[2], 15);
    }
}

/// SPEC §13.2.5 — the label is composited over black before cover-fit.
#[test]
fn cover_fit_composites_over_black() {
    // A single fully transparent white pixel becomes black.
    let rgba = [255u8, 255, 255, 0];
    let art = cover_fit(&rgba, 1, 1, Variant::DEFAULT);
    assert!(art.iter().all(|a| *a == [0, 0, 0]));
}

/// SPEC §13.2.5 — a non-RGBA label PNG decodes to RGBA8.
#[test]
fn label_png_decodes_to_rgba() {
    let rgba = vec![10u8, 20, 30, 255, 40, 50, 60, 128];
    let png = encode_png(2, 1, &rgba).unwrap();
    let (w, h, out) = decode_png_rgba8(&png).unwrap();
    assert_eq!((w, h), (2, 1));
    assert_eq!(out, rgba);
}

/// SPEC §13.2 — `pack_auto` picks the default variant when the payload fits.
#[test]
fn pack_auto_default() {
    let bytes = cart_bytes();
    let (png, variant) =
        png::pack_auto(&bytes, |v| generated_label("t", "", &DEFAULT_PALETTE, v)).unwrap();
    assert_eq!(variant, Variant::DEFAULT);
    let (cart, _) = unpack(&png).unwrap();
    assert_eq!(cart.encode().unwrap(), bytes);
}

/// A cart with a full 256-colour palette still round-trips.
#[test]
fn palette_survives() {
    let mut palette = [0u16; PALETTE_LEN];
    for (i, w) in palette.iter_mut().enumerate() {
        *w = (i as u16) & 0x7FFF;
    }
    let cart = BinaryCart {
        code: String::new(),
        modules: vec![],
        metadata: "title = \"t\"\n".to_string(),
        sheet: vec![0u8; SHEET_LEN],
        map: vec![0u8; 16_384],
        palette,
    };
    let bytes = cart.encode().unwrap();
    let art = vec![[0u8; 3]; Variant::DEFAULT.pixels()];
    let png = pack(&bytes, Variant::DEFAULT, &art).unwrap();
    let (decoded, _) = unpack(&png).unwrap();
    assert_eq!(decoded.palette, palette);
}
