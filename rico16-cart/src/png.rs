//! SPEC §13.2 — the PNG cart: cover art in the high nibbles, payload in the low.
//!
//! The image is an ordinary 8-bit RGBA, non-interlaced PNG. The high nibble of
//! each R, G and B byte is the RGB444 cover art; the low nibble carries the
//! payload. Alpha is always `0xFF`. There are no private chunks: the pixels are
//! the payload (SPEC §13.2).

use std::io::Cursor;

use rico16_core::font;
use rico16_core::limits::{
    BLOB_HEADER_LEN, PALETTE_LEN, PNG_DEFAULT_HEIGHT, PNG_DEFAULT_WIDTH, PNG_OVERFLOW_HEIGHT,
    PNG_OVERFLOW_WIDTH,
};

use crate::binary::BinaryCart;
use crate::crc::crc32;
use crate::error::CartError;

/// SPEC §13.2.3 — the blob magic.
pub const BLOB_MAGIC: &[u8; 4] = b"R16P";
/// SPEC §13.2.3 — the only blob version v1 accepts.
pub const BLOB_VERSION: u16 = 1;

/// A PNG size variant (SPEC §7.7, §13.2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Variant {
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// The blob header's variant id.
    pub id: u16,
}

impl Variant {
    /// SPEC §13.2.3 — `0 = 640×480`.
    pub const DEFAULT: Variant = Variant {
        width: PNG_DEFAULT_WIDTH,
        height: PNG_DEFAULT_HEIGHT,
        id: 0,
    };
    /// SPEC §13.2.3 — `1 = 1280×960`.
    pub const OVERFLOW: Variant = Variant {
        width: PNG_OVERFLOW_WIDTH,
        height: PNG_OVERFLOW_HEIGHT,
        id: 1,
    };

    /// Both variants, smallest first.
    pub const ALL: [Variant; 2] = [Variant::DEFAULT, Variant::OVERFLOW];

    /// SPEC §13.2.2 — total payload capacity in bytes (12 bits per pixel).
    pub fn capacity(self) -> usize {
        (self.width as usize * self.height as usize) * 3 / 2
    }

    /// SPEC §13.2.3 — payload bytes available after the blob header.
    pub fn payload_capacity(self) -> usize {
        self.capacity() - BLOB_HEADER_LEN
    }

    /// Pixel count.
    pub fn pixels(self) -> usize {
        self.width as usize * self.height as usize
    }

    /// The variant with blob id `id`.
    pub fn from_id(id: u16) -> Option<Variant> {
        Variant::ALL.into_iter().find(|v| v.id == id)
    }

    /// The variant with these dimensions.
    pub fn from_dims(width: u32, height: u32) -> Option<Variant> {
        Variant::ALL
            .into_iter()
            .find(|v| v.width == width && v.height == height)
    }
}

/// RGB444 cover art: one `[r, g, b]` triple of high nibbles per pixel.
pub type Art = Vec<[u8; 3]>;

/// SPEC §13.2.2, §13.2.3 — pack a binary cart into a PNG, hiding the payload in
/// the low nibbles of `art`.
pub fn pack(cart_bytes: &[u8], variant: Variant, art: &Art) -> Result<Vec<u8>, CartError> {
    let mut payload = Vec::with_capacity(BLOB_HEADER_LEN + cart_bytes.len());
    payload.extend_from_slice(BLOB_MAGIC);
    payload.extend_from_slice(&BLOB_VERSION.to_le_bytes());
    payload.extend_from_slice(&variant.id.to_le_bytes());
    payload.extend_from_slice(&(cart_bytes.len() as u32).to_le_bytes());
    payload.extend_from_slice(&crc32(cart_bytes).to_le_bytes());
    payload.extend_from_slice(cart_bytes);

    let capacity = variant.capacity();
    if payload.len() > capacity {
        return Err(CartError::Capacity {
            used: payload.len(),
            capacity,
        });
    }

    let pixels = variant.pixels();
    let mut rgba = vec![0u8; pixels * 4];
    let nibble = |n: usize| -> u8 {
        match payload.get(n / 2) {
            Some(byte) if n % 2 == 0 => byte >> 4,
            Some(byte) => byte & 0x0F,
            None => 0,
        }
    };
    for p in 0..pixels {
        let a = art.get(p).copied().unwrap_or([0, 0, 0]);
        rgba[p * 4] = (a[0] << 4) | nibble(3 * p);
        rgba[p * 4 + 1] = (a[1] << 4) | nibble(3 * p + 1);
        rgba[p * 4 + 2] = (a[2] << 4) | nibble(3 * p + 2);
        rgba[p * 4 + 3] = 0xFF;
    }

    encode_png(variant.width, variant.height, &rgba)
}

/// SPEC §12.5 — pack into the default variant, falling back to overflow when
/// the payload does not fit.
pub fn pack_auto(
    cart_bytes: &[u8],
    art_for: impl Fn(Variant) -> Art,
) -> Result<(Vec<u8>, Variant), CartError> {
    let required = BLOB_HEADER_LEN + cart_bytes.len();
    let variant = if required <= Variant::DEFAULT.capacity() {
        Variant::DEFAULT
    } else if required <= Variant::OVERFLOW.capacity() {
        Variant::OVERFLOW
    } else {
        return Err(CartError::Capacity {
            used: required,
            capacity: Variant::OVERFLOW.capacity(),
        });
    };
    let art = art_for(variant);
    Ok((pack(cart_bytes, variant, &art)?, variant))
}

/// SPEC §13.4 — read a cart PNG back into a binary cart.
pub fn unpack(png_bytes: &[u8]) -> Result<(BinaryCart, Variant), CartError> {
    let (width, height, rgba) = decode_png_rgba8_strict(png_bytes)?;
    let variant = Variant::from_dims(width, height).ok_or_else(|| {
        CartError::Format(format!(
            "png is {width}×{height}, must be 640×480 or 1280×960"
        ))
    })?;

    let pixels = variant.pixels();
    let mut nibbles = Vec::with_capacity(pixels * 3);
    for p in 0..pixels {
        nibbles.push(rgba[p * 4] & 0x0F);
        nibbles.push(rgba[p * 4 + 1] & 0x0F);
        nibbles.push(rgba[p * 4 + 2] & 0x0F);
    }
    let mut payload = Vec::with_capacity(nibbles.len() / 2);
    for pair in nibbles.chunks_exact(2) {
        payload.push((pair[0] << 4) | pair[1]);
    }

    if payload.len() < BLOB_HEADER_LEN {
        return Err(CartError::Format(
            "png is too small to hold a payload".into(),
        ));
    }
    if &payload[0..4] != BLOB_MAGIC {
        return Err(CartError::Format("bad blob magic, expected R16P".into()));
    }
    let blob_version = u16::from_le_bytes([payload[4], payload[5]]);
    if blob_version != BLOB_VERSION {
        return Err(CartError::Format(format!(
            "unsupported blob version {blob_version}, expected {BLOB_VERSION}"
        )));
    }
    let variant_id = u16::from_le_bytes([payload[6], payload[7]]);
    if variant_id != variant.id {
        return Err(CartError::Format(format!(
            "blob variant {variant_id} does not match the {width}×{height} image"
        )));
    }
    let len = u32::from_le_bytes([payload[8], payload[9], payload[10], payload[11]]) as usize;
    let expected_crc = u32::from_le_bytes([payload[12], payload[13], payload[14], payload[15]]);

    if BLOB_HEADER_LEN + len > variant.capacity() {
        return Err(CartError::Format(format!(
            "payload length {len} exceeds the variant capacity {}",
            variant.payload_capacity()
        )));
    }
    let cart_bytes = &payload[BLOB_HEADER_LEN..BLOB_HEADER_LEN + len];
    if crc32(cart_bytes) != expected_crc {
        return Err(CartError::Format(format!(
            "payload CRC mismatch: header {expected_crc:08X}"
        )));
    }
    let used_nibbles = (BLOB_HEADER_LEN + len) * 2;
    if nibbles[used_nibbles..].iter().any(|&n| n != 0) {
        return Err(CartError::Format(
            "non-zero nibble after the end of the payload".into(),
        ));
    }

    let cart = BinaryCart::decode(cart_bytes)?;
    Ok((cart, variant))
}

/// SPEC §13.2.4 — a deterministic generated label: dark field, title, author and
/// a strip of the cart's own colours.
pub fn generated_label(
    title: &str,
    author: &str,
    palette: &[u16; PALETTE_LEN],
    variant: Variant,
) -> Art {
    let w = variant.width as i64;
    let h = variant.height as i64;
    let mut art: Art = vec![[0, 0, 2]; variant.pixels()];

    draw_text(&mut art, w, h, title, 24, 32, 3, [15, 15, 15]);
    if !author.is_empty() {
        draw_text(&mut art, w, h, author, 24, 32 + 8 * 3 + 12, 1, [11, 11, 11]);
    }

    // A strip of the cart's own palette along the bottom.
    let sw = (variant.width / PALETTE_LEN as u32).max(1) as i64;
    let strip_w = sw * PALETTE_LEN as i64;
    let x0 = (w - strip_w) / 2;
    let y0 = h - 40;
    for (i, &word) in palette.iter().enumerate() {
        let r = ((word & 0x1F) >> 1) as u8;
        let g = (((word >> 5) & 0x1F) >> 1) as u8;
        let b = (((word >> 10) & 0x1F) >> 1) as u8;
        fill_rect(&mut art, w, h, x0 + i as i64 * sw, y0, sw, 20, [r, g, b]);
    }

    art
}

/// SPEC §13.2.5 — cover-fit a decoded label into the variant size, compositing
/// over black first.
pub fn cover_fit(src_rgba: &[u8], src_w: u32, src_h: u32, variant: Variant) -> Art {
    let dst_w = variant.width as i128;
    let dst_h = variant.height as i128;
    let sw = src_w as i128;
    let sh = src_h as i128;
    let mut art = Vec::with_capacity(variant.pixels());

    for y in 0..dst_h {
        for x in 0..dst_w {
            let (mut sx, mut sy);
            if dst_w * sh >= dst_h * sw {
                sx = ((2 * x + 1) * sw) / (2 * dst_w);
                sy = ((sh * dst_w - dst_h * sw) + (2 * y + 1) * sw) / (2 * dst_w);
            } else {
                sy = ((2 * y + 1) * sh) / (2 * dst_h);
                sx = ((sw * dst_h - dst_w * sh) + (2 * x + 1) * sh) / (2 * dst_h);
            }
            sx = sx.clamp(0, sw - 1);
            sy = sy.clamp(0, sh - 1);

            let at = (sy * sw + sx) as usize * 4;
            let a = src_rgba[at + 3] as i128;
            let r = (src_rgba[at] as i128 * a) / 255;
            let g = (src_rgba[at + 1] as i128 * a) / 255;
            let b = (src_rgba[at + 2] as i128 * a) / 255;
            art.push([quantize4(r as u8), quantize4(g as u8), quantize4(b as u8)]);
        }
    }
    art
}

/// SPEC §13.2 — encode an 8-bit RGBA image as a non-interlaced PNG.
pub fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, CartError> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(Cursor::new(&mut out), width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(png_encode_err)?;
        writer.write_image_data(rgba).map_err(png_encode_err)?;
    }
    Ok(out)
}

/// SPEC §15.4 — encode an 8-bit indexed PNG with an explicit RGB palette.
pub fn encode_indexed_png(
    width: u32,
    height: u32,
    palette_rgb8: &[u8],
    indices: &[u8],
) -> Result<Vec<u8>, CartError> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(Cursor::new(&mut out), width, height);
        encoder.set_color(png::ColorType::Indexed);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_palette(palette_rgb8.to_vec());
        let mut writer = encoder.write_header().map_err(png_encode_err)?;
        writer.write_image_data(indices).map_err(png_encode_err)?;
    }
    Ok(out)
}

/// SPEC §15.4 — encode an 8-bit grayscale PNG.
pub fn encode_gray_png(width: u32, height: u32, samples: &[u8]) -> Result<Vec<u8>, CartError> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(Cursor::new(&mut out), width, height);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(png_encode_err)?;
        writer.write_image_data(samples).map_err(png_encode_err)?;
    }
    Ok(out)
}

/// SPEC §13.4 — decode a cart PNG, which must be 8-bit RGBA and non-interlaced.
pub fn decode_png_rgba8_strict(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), CartError> {
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder.read_info().map_err(|e| png_err("<cart png>", e))?;
    let info = reader.info().clone();
    if info.interlaced {
        return Err(CartError::Format("png must not be interlaced".into()));
    }
    if info.color_type != png::ColorType::Rgba {
        return Err(CartError::Format(format!(
            "png must be RGBA, found {:?}",
            info.color_type
        )));
    }
    if info.bit_depth != png::BitDepth::Eight {
        return Err(CartError::Format(format!(
            "png must be 8-bit, found {:?}",
            info.bit_depth
        )));
    }
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let frame = reader
        .next_frame(&mut buf)
        .map_err(|e| png_err("<cart png>", e))?;
    buf.truncate(frame.buffer_size());
    Ok((info.width, info.height, buf))
}

/// Decode any ordinary 8-bit PNG to straight RGBA8, for labels and assets.
pub fn decode_png_rgba8(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), CartError> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| png_err("<png>", e))?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let frame = reader
        .next_frame(&mut buf)
        .map_err(|e| png_err("<png>", e))?;
    buf.truncate(frame.buffer_size());

    // Use the *output* colour type: the transformations above may have expanded
    // an indexed or low-bit-depth image before we see it.
    let pixels = (frame.width as usize) * (frame.height as usize);
    let rgba = match frame.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => {
            let mut out = Vec::with_capacity(pixels * 4);
            for px in buf.chunks_exact(3) {
                out.extend_from_slice(&[px[0], px[1], px[2], 0xFF]);
            }
            out
        }
        png::ColorType::Grayscale => {
            let mut out = Vec::with_capacity(pixels * 4);
            for &g in &buf {
                out.extend_from_slice(&[g, g, g, 0xFF]);
            }
            out
        }
        png::ColorType::GrayscaleAlpha => {
            let mut out = Vec::with_capacity(pixels * 4);
            for px in buf.chunks_exact(2) {
                out.extend_from_slice(&[px[0], px[0], px[0], px[1]]);
            }
            out
        }
        png::ColorType::Indexed => {
            let palette = reader.info().palette.clone().unwrap_or_default();
            let trns = reader.info().trns.clone().unwrap_or_default();
            let mut out = Vec::with_capacity(pixels * 4);
            for &idx in &buf {
                let i = idx as usize * 3;
                let alpha = trns.get(idx as usize).copied().unwrap_or(0xFF);
                out.extend_from_slice(&[
                    palette.get(i).copied().unwrap_or(0),
                    palette.get(i + 1).copied().unwrap_or(0),
                    palette.get(i + 2).copied().unwrap_or(0),
                    alpha,
                ]);
            }
            out
        }
    };
    Ok((frame.width, frame.height, rgba))
}

/// SPEC §10.2.3 — decode a 128×128 map PNG to tile ids.
///
/// Grayscale: the sample value is the id. Indexed: the palette index is the id.
/// RGB(A): BT.601 luma, clamped to 0–255. Low-bit-depth inputs are rejected
/// rather than guessed at.
pub fn map_tile_ids(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), CartError> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| png_err("<map png>", e))?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let frame = reader
        .next_frame(&mut buf)
        .map_err(|e| png_err("<map png>", e))?;
    buf.truncate(frame.buffer_size());

    if frame.bit_depth != png::BitDepth::Eight {
        return Err(CartError::Asset(format!(
            "map png must be 8-bit, found {:?}",
            frame.bit_depth
        )));
    }

    let ids = match frame.color_type {
        png::ColorType::Grayscale | png::ColorType::Indexed => buf,
        png::ColorType::Rgb => buf
            .chunks_exact(3)
            .map(|px| bt601(px[0], px[1], px[2]))
            .collect(),
        png::ColorType::Rgba => buf
            .chunks_exact(4)
            .map(|px| bt601(px[0], px[1], px[2]))
            .collect(),
        png::ColorType::GrayscaleAlpha => buf.chunks_exact(2).map(|px| px[0]).collect(),
    };
    Ok((frame.width, frame.height, ids))
}

fn bt601(r: u8, g: u8, b: u8) -> u8 {
    (((r as u32) * 299 + (g as u32) * 587 + (b as u32) * 114 + 500) / 1000).min(255) as u8
}

fn quantize4(v: u8) -> u8 {
    ((v as u32 * 15 + 127) / 255) as u8
}
fn png_err(path: &str, e: png::DecodingError) -> CartError {
    CartError::Png {
        path: path.into(),
        message: e.to_string(),
    }
}

fn png_encode_err(e: png::EncodingError) -> CartError {
    CartError::Png {
        path: "<memory>".into(),
        message: e.to_string(),
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_text(
    art: &mut Art,
    w: i64,
    h: i64,
    text: &str,
    x: i64,
    y: i64,
    scale: i64,
    color: [u8; 3],
) {
    let mut cx = x;
    let mut cy = y;
    for ch in text.chars() {
        if ch == '\n' {
            cx = x;
            cy += 8 * scale;
            continue;
        }
        let glyph = font::glyph(ch);
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..8i64 {
                if bits & (0x80 >> col) != 0 {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            let px = cx + col * scale + sx;
                            let py = cy + row as i64 * scale + sy;
                            if px >= 0 && py >= 0 && px < w && py < h {
                                art[py as usize * w as usize + px as usize] = color;
                            }
                        }
                    }
                }
            }
        }
        cx += 8 * scale;
    }
}

#[allow(clippy::too_many_arguments)]
fn fill_rect(art: &mut Art, w: i64, h: i64, x: i64, y: i64, rw: i64, rh: i64, color: [u8; 3]) {
    for py in y..y + rh {
        for px in x..x + rw {
            if px >= 0 && py >= 0 && px < w && py < h {
                art[py as usize * w as usize + px as usize] = color;
            }
        }
    }
}
