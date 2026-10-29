//! SPEC §3.6, §4 — the 256-entry RGB555 palette.
//!
//! The default table is normative data (SPEC §4.7, §4.12): the words below are
//! copied verbatim from the spec and must be used bit-for-bit. The HSV recipe in
//! SPEC §4.7 is informative only.

use crate::limits::PALETTE_LEN;

/// SPEC §4.7, §4.12 — the default palette, 256 RGB555 words, little-endian on
/// disk. Bit 15 is reserved and zero in every entry.
///
/// Index 0 is black (the transparency key); 1–5 are the grey ramp; 6–15 are the
/// ten base hues; 16–255 are fifteen ramps of sixteen steps.
pub const DEFAULT_PALETTE: [u16; PALETTE_LEN] = [
    0x0000, 0x18C6, 0x2D6B, 0x4210, 0x5AD6, 0x7FFF, 0x0C7F, 0x0DDF, 0x0F5F, 0x0FF1, 0x23E3, 0x63E3,
    0x7F43, 0x7D03, 0x7C71, 0x687F, 0x0425, 0x0426, 0x0848, 0x084A, 0x0C6C, 0x0C6D, 0x108F, 0x14B1,
    0x18D3, 0x1CF4, 0x2116, 0x2538, 0x295A, 0x2D7B, 0x35BD, 0x39DF, 0x0445, 0x0466, 0x0888, 0x08AA,
    0x0CCC, 0x0CED, 0x112F, 0x1551, 0x1973, 0x1D94, 0x21D6, 0x25F8, 0x2A1A, 0x2E5B, 0x367D, 0x3ABF,
    0x0485, 0x04A6, 0x08E8, 0x090A, 0x0D4C, 0x0D6D, 0x11AF, 0x15F1, 0x1A13, 0x1E54, 0x2276, 0x26B8,
    0x2AFA, 0x2F1B, 0x375D, 0x3B9F, 0x04A4, 0x04C5, 0x0907, 0x0948, 0x0D8A, 0x0DAB, 0x11ED, 0x162F,
    0x1A70, 0x1E92, 0x22D3, 0x2715, 0x2B57, 0x2F78, 0x37BA, 0x3BFC, 0x04A2, 0x04C3, 0x0904, 0x0945,
    0x0D86, 0x0DA7, 0x11E9, 0x162A, 0x1A6B, 0x1E8C, 0x22CE, 0x270F, 0x2B50, 0x2F72, 0x37B3, 0x3BF5,
    0x04A1, 0x04C1, 0x0902, 0x0942, 0x0D83, 0x0DA3, 0x11E4, 0x1625, 0x1A66, 0x1E87, 0x22C8, 0x2709,
    0x2B4A, 0x2F6B, 0x37AD, 0x3BEE, 0x08A1, 0x0CC1, 0x1102, 0x1542, 0x1983, 0x1DA3, 0x25E4, 0x2A25,
    0x2E66, 0x3287, 0x3AC8, 0x3F09, 0x434A, 0x4B6B, 0x4FAD, 0x57EE, 0x10A1, 0x14C1, 0x1D02, 0x2142,
    0x2983, 0x2DA3, 0x35E4, 0x3E25, 0x4266, 0x4A87, 0x4EC8, 0x5709, 0x5F4A, 0x636B, 0x6BAD, 0x73EE,
    0x1481, 0x18A1, 0x20E2, 0x2902, 0x3143, 0x3563, 0x3DA4, 0x45E5, 0x4E06, 0x5247, 0x5A68, 0x62A9,
    0x6AEA, 0x6F0B, 0x774D, 0x7F8E, 0x1441, 0x1861, 0x2082, 0x28A2, 0x30C3, 0x34E3, 0x3D24, 0x4545,
    0x4D66, 0x5187, 0x59C8, 0x61E9, 0x6A0A, 0x6E4B, 0x766D, 0x7EAE, 0x1421, 0x1821, 0x2042, 0x2842,
    0x3063, 0x3463, 0x3C84, 0x44A5, 0x4CC6, 0x50E7, 0x5908, 0x6129, 0x694A, 0x6D6B, 0x75AD, 0x7DCE,
    0x1422, 0x1823, 0x2044, 0x2845, 0x3066, 0x3467, 0x3C89, 0x44AA, 0x4CCB, 0x50EC, 0x590E, 0x612F,
    0x6950, 0x6D72, 0x75B3, 0x7DD5, 0x1424, 0x1825, 0x2047, 0x2848, 0x306A, 0x346B, 0x3C8D, 0x44AF,
    0x4CD0, 0x50F2, 0x5913, 0x6135, 0x6957, 0x6D78, 0x75BA, 0x7DDC, 0x1025, 0x1426, 0x1C48, 0x204A,
    0x286C, 0x2C6D, 0x348F, 0x3CB1, 0x40D3, 0x48F4, 0x4D16, 0x5538, 0x5D5A, 0x617B, 0x69BD, 0x71DF,
    0x0825, 0x0C26, 0x1048, 0x144A, 0x186C, 0x1C6D, 0x248F, 0x28B1, 0x2CD3, 0x30F4, 0x3916, 0x3D38,
    0x415A, 0x497B, 0x4DBD, 0x55DF,
];

/// SPEC §4.4 — quantize an 8-bit component to a 5-bit component:
/// `round(v * 31 / 255)`, i.e. `(v * 31 + 127) / 255`.
pub const fn quantize5(v: u8) -> u16 {
    ((v as u32 * 31 + 127) / 255) as u16
}

/// SPEC §3.6 — expand a 5-bit component to 8 bits: `(v << 3) | (v >> 2)`.
pub const fn expand5(v: u16) -> u8 {
    (((v & 0x1F) << 3) | ((v & 0x1F) >> 2)) as u8
}

/// SPEC §4.13 — expand the 3-bit red and green of an `rgb332` index.
const fn expand3(v: u16) -> u16 {
    ((v & 0x7) << 2) | ((v & 0x7) >> 1)
}

/// SPEC §4.13 — expand the 2-bit blue of an `rgb332` index.
const fn expand2(v: u16) -> u16 {
    ((v & 0x3) << 3) | ((v & 0x3) << 1) | ((v & 0x3) >> 1)
}

/// SPEC §4.11, §4.13 — the `rgb332` named palette preset: index
/// `0bRRRGGGBB` is literally that colour, expanded to RGB555.
pub fn rgb332() -> [u16; PALETTE_LEN] {
    let mut out = [0u16; PALETTE_LEN];
    let mut i = 0;
    while i < PALETTE_LEN {
        let idx = i as u16;
        let r = expand3(idx >> 5);
        let g = expand3(idx >> 2);
        let b = expand2(idx);
        out[i] = r | (g << 5) | (b << 10);
        i += 1;
    }
    out
}

/// The live palette. SPEC §4.5 — palette mutation is runtime state, restored on
/// reset and never written back to the cart file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    entries: [u16; PALETTE_LEN],
}

impl Default for Palette {
    fn default() -> Self {
        Self::new()
    }
}

impl Palette {
    /// The console default palette (SPEC §4.7).
    pub const fn new() -> Self {
        Self {
            entries: DEFAULT_PALETTE,
        }
    }

    /// A palette from raw RGB555 words, e.g. one resolved at pack time
    /// (SPEC §4.9).
    pub const fn from_entries(entries: [u16; PALETTE_LEN]) -> Self {
        Self { entries }
    }

    /// SPEC §4.1 — the RGB555 word at `index`.
    pub fn word(&self, index: usize) -> u16 {
        self.entries[index]
    }

    /// SPEC §4.1 — replace the RGB555 word at `index`.
    pub fn set_word(&mut self, index: usize, word: u16) {
        self.entries[index] = word;
    }

    /// SPEC §4.4 — set entry `index` from 8-bit components, quantized.
    pub fn set_rgb8(&mut self, index: usize, r: u8, g: u8, b: u8) {
        self.entries[index] = quantize5(r) | (quantize5(g) << 5) | (quantize5(b) << 10);
    }

    /// SPEC §4.5 — restore the cart's palette after mutation.
    pub fn copy_from(&mut self, other: &Palette) {
        self.entries = other.entries;
    }

    /// SPEC §3.6 — expand the whole palette to RGBA8, alpha 255. `out` must be
    /// exactly `PALETTE_LEN * 4` bytes.
    pub fn expand_rgba8(&self, out: &mut [u8]) {
        debug_assert_eq!(out.len(), PALETTE_LEN * 4);
        for (i, word) in self.entries.iter().enumerate() {
            let r = expand5(word & 0x1F);
            let g = expand5((word >> 5) & 0x1F);
            let b = expand5((word >> 10) & 0x1F);
            out[i * 4] = r;
            out[i * 4 + 1] = g;
            out[i * 4 + 2] = b;
            out[i * 4 + 3] = 0xFF;
        }
    }
}
