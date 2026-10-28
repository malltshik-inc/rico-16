//! The console palette: 256 RGB555 entries and the conversions around them.
//!
//! Normative reference: `docs/SPEC.md` §4.
//!
//! A cart's art stores palette indices, never colours. That indirection is the
//! whole point of an indexed framebuffer: swapping the table recolours every
//! pixel for free, which is how fades, flashes and alternate themes work.

use crate::limits;

/// Index that blits skip. SPEC §4.2.
pub const TRANSPARENT: u8 = 0;

/// Quantize an 8-bit channel to 5 bits. SPEC §4.4.
pub const fn quantize(channel: u8) -> u16 {
    (channel as u16 * 31 + 127) / 255
}

/// Expand a 5-bit channel to 8 bits. SPEC §3.6.
pub const fn expand(channel: u16) -> u8 {
    let value = (channel & 0x1f) as u8;
    (value << 3) | (value >> 2)
}

/// Build an RGB555 word from 8-bit components. SPEC §4.1, §4.4.
pub const fn from_rgb8(r: u8, g: u8, b: u8) -> u16 {
    (quantize(r) << 10) | (quantize(g) << 5) | quantize(b)
}

/// Split an RGB555 word back into 8-bit components. SPEC §3.6.
pub const fn to_rgb8(word: u16) -> (u8, u8, u8) {
    (
        expand((word >> 10) & 0x1f),
        expand((word >> 5) & 0x1f),
        expand(word & 0x1f),
    )
}

/// The palette every cart starts with. SPEC §4.7.
///
/// Layout: 0 is black, 1–5 are the grey ramp, 6–15 are the ten base hues, and
/// 16–255 are fifteen sixteen-step ramps. Ramps are what make shading
/// mechanical: a cart picks a ramp and walks it.
///
/// The table is committed as data rather than computed at startup, so that the
/// console's colours are a fixed artefact: reviewable in a diff, identical on
/// every platform, and cheap to re-curate by hand. `print_default_table`
/// regenerates it.
pub const DEFAULT: [u16; limits::PALETTE_LEN] = [
    0x0000, 0x0863, 0x1D09, 0x31AE, 0x4E95, 0x77BE, 0x70A6, 0x7E24, //
    0x7F86, 0x4F85, 0x1B09, 0x1314, 0x1B3D, 0x19FD, 0x457B, 0x7138, //
    0x1421, 0x1821, 0x2042, 0x2842, 0x3063, 0x3463, 0x3C84, 0x44A5, //
    0x4CC6, 0x50E7, 0x5908, 0x6129, 0x694A, 0x6D6B, 0x75AD, 0x7DCE, //
    0x1441, 0x1861, 0x2082, 0x28A2, 0x30C3, 0x34E3, 0x3D04, 0x4525, //
    0x4D66, 0x5187, 0x59A8, 0x61C9, 0x6A0A, 0x6E2B, 0x766D, 0x7E8E, //
    0x1461, 0x18A1, 0x20C2, 0x2902, 0x3123, 0x3543, 0x3D84, 0x45A5, //
    0x4DE6, 0x5207, 0x5A48, 0x6269, 0x6AAA, 0x6EEB, 0x770D, 0x7F4E, //
    0x10A1, 0x14C1, 0x1D02, 0x2542, 0x2983, 0x31A3, 0x35E4, 0x3E25, //
    0x4666, 0x4A87, 0x52C8, 0x5709, 0x5F4A, 0x676B, 0x6FAD, 0x73EE, //
    0x08A1, 0x08C1, 0x0D02, 0x1142, 0x1583, 0x19A3, 0x1DE4, 0x2225, //
    0x2666, 0x2A87, 0x2EC8, 0x3709, 0x3B4A, 0x3F6B, 0x47AD, 0x4BEE, //
    0x04A2, 0x04C3, 0x0904, 0x0945, 0x0D86, 0x0DA7, 0x11E8, 0x1629, //
    0x1A6A, 0x1E8B, 0x22CD, 0x270E, 0x2B4F, 0x2F71, 0x37B2, 0x3BF4, //
    0x04A4, 0x04C5, 0x0907, 0x0948, 0x0D8A, 0x0DAB, 0x11ED, 0x162F, //
    0x1A70, 0x1E92, 0x22D3, 0x2715, 0x2B57, 0x2F78, 0x37BA, 0x3BFC, //
    0x0485, 0x04C6, 0x08E8, 0x092A, 0x0D4C, 0x0D8D, 0x11CF, 0x15F1, //
    0x1A33, 0x1E74, 0x2296, 0x26D8, 0x2B1A, 0x2F3B, 0x377D, 0x3BBF, //
    0x0465, 0x0486, 0x08A8, 0x08EA, 0x0D0C, 0x0D2D, 0x116F, 0x1591, //
    0x19B3, 0x1DF4, 0x2216, 0x2658, 0x2A7A, 0x2EBB, 0x36FD, 0x3B1F, //
    0x0445, 0x0466, 0x0888, 0x088A, 0x0CAC, 0x0CCD, 0x10EF, 0x1531, //
    0x1953, 0x1D74, 0x2196, 0x25B8, 0x29FA, 0x2E1B, 0x365D, 0x3A7F, //
    0x0425, 0x0426, 0x0848, 0x0C4A, 0x0C6C, 0x106D, 0x148F, 0x18B1, //
    0x1CD3, 0x20F4, 0x2516, 0x2938, 0x2D5A, 0x317B, 0x39BD, 0x3DDF, //
    0x0825, 0x0C26, 0x1048, 0x144A, 0x186C, 0x1C6D, 0x248F, 0x28B1, //
    0x2CD3, 0x30F4, 0x3516, 0x3D38, 0x415A, 0x497B, 0x4DBD, 0x55DF, //
    0x1025, 0x1426, 0x1C48, 0x204A, 0x286C, 0x2C6D, 0x348F, 0x3CB1, //
    0x40D3, 0x48F4, 0x4D16, 0x5538, 0x5D5A, 0x617B, 0x69BD, 0x71DF, //
    0x1423, 0x1825, 0x2046, 0x2848, 0x3069, 0x346A, 0x3C8C, 0x44AD, //
    0x4CCF, 0x50F0, 0x5912, 0x6133, 0x6955, 0x6D77, 0x75B8, 0x7DDA, //
    0x1422, 0x1823, 0x2043, 0x2844, 0x3065, 0x3466, 0x3C87, 0x44A8, //
    0x4CC9, 0x50EA, 0x590C, 0x612D, 0x694E, 0x6D70, 0x75B1, 0x7DD2, //
];

/// A bit-packed RGB332 palette: index `0bRRRGGGBB` is exactly that colour.
///
/// The same 256-colour budget as [`DEFAULT`], spent differently: three bits of
/// red and green, two of blue, so indices read as colours instead of being
/// walked as ramps. Blue gets four levels, which is why this is a preset and
/// not the default.
fn scale_3_bit(value: u8) -> u8 {
    (value as u16 * 255 / 7) as u8
}

fn scale_2_bit(value: u8) -> u8 {
    (value as u16 * 255 / 3) as u8
}

pub fn rgb332() -> [u16; limits::PALETTE_LEN] {
    let mut table = [0u16; limits::PALETTE_LEN];
    for (index, entry) in table.iter_mut().enumerate() {
        let index = index as u8;
        let red = (index >> 5) & 0x07;
        let green = (index >> 2) & 0x07;
        let blue = index & 0x03;
        *entry = from_rgb8(scale_3_bit(red), scale_3_bit(green), scale_2_bit(blue));
    }
    table
}

/// A live 256-entry palette. SPEC §4.1, §4.5.
///
/// This is runtime state: a cart may mutate entries at any time, and a restart
/// throws the mutations away. `DEFAULT` is never modified.
#[derive(Clone, PartialEq, Eq)]
pub struct Palette {
    entries: [u16; limits::PALETTE_LEN],
}

impl Palette {
    /// The console's default palette. SPEC §4.7.
    pub const fn new() -> Self {
        Self { entries: DEFAULT }
    }

    /// A palette from cart data, already validated as RGB555. SPEC §10.6.
    pub const fn from_entries(entries: [u16; limits::PALETTE_LEN]) -> Self {
        Self { entries }
    }

    /// The raw RGB555 words.
    pub const fn entries(&self) -> &[u16; limits::PALETTE_LEN] {
        &self.entries
    }

    /// One RGB555 word. SPEC §4.1.
    pub const fn entry(&self, index: u8) -> u16 {
        self.entries[index as usize]
    }

    /// Replace one entry. Bit 15 is reserved and forced to 0. SPEC §4.1.
    pub fn set_entry(&mut self, index: u8, word: u16) {
        self.entries[index as usize] = word & 0x7fff;
    }

    /// Replace one entry from 8-bit components, quantized per SPEC §4.4.
    pub fn set_rgb8(&mut self, index: u8, r: u8, g: u8, b: u8) {
        self.entries[index as usize] = from_rgb8(r, g, b);
    }

    /// Restore the console default. SPEC §4.5.
    pub fn reset(&mut self) {
        self.entries = DEFAULT;
    }

    /// The RGBA8 a single index presents as. SPEC §3.6.
    pub fn rgba8(&self, index: u8) -> [u8; 4] {
        let (r, g, b) = to_rgb8(self.entries[index as usize]);
        [r, g, b, 255]
    }

    /// All 256 entries expanded once, for the present path. SPEC §3.6.
    pub fn rgba8_table(&self) -> [[u8; 4]; limits::PALETTE_LEN] {
        let mut table = [[0u8; 4]; limits::PALETTE_LEN];
        for (index, entry) in table.iter_mut().enumerate() {
            *entry = self.rgba8(index as u8);
        }
        table
    }
}

impl Default for Palette {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Palette {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Palette")
            .field("entries", &limits::PALETTE_LEN)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_mutation_is_local_and_resettable() {
        let mut palette = Palette::new();
        let original = palette.entry(7);

        palette.set_rgb8(7, 255, 0, 0);
        assert_eq!(palette.entry(7), from_rgb8(255, 0, 0));
        assert_ne!(palette.entry(7), original);
        assert_eq!(palette.rgba8(7), [255, 0, 0, 255]);

        palette.reset();
        assert_eq!(palette.entry(7), original);
    }

    #[test]
    fn set_entry_clears_the_reserved_bit() {
        let mut palette = Palette::new();
        palette.set_entry(0, 0xffff);
        assert_eq!(palette.entry(0), 0x7fff);
    }

    #[test]
    fn the_expansion_table_has_alpha_255_everywhere() {
        let palette = Palette::new();
        for (index, rgba) in palette.rgba8_table().iter().enumerate() {
            assert_eq!(rgba[3], 255, "index {index}");
            let (r, g, b) = to_rgb8(palette.entry(index as u8));
            assert_eq!(&rgba[..3], &[r, g, b], "index {index}");
        }
    }

    #[test]
    fn quantization_roundtrips_exactly_for_5_bit_values() {
        for channel in 0..32u16 {
            assert_eq!(quantize(expand(channel)), channel);
        }
    }

    #[test]
    fn black_is_black_and_white_is_white() {
        assert_eq!(from_rgb8(0, 0, 0), 0);
        assert_eq!(from_rgb8(255, 255, 255), 0x7fff);
        assert_eq!(to_rgb8(0x7fff), (255, 255, 255));
        assert_eq!(to_rgb8(0x0000), (0, 0, 0));
    }

    #[test]
    fn default_table_has_the_documented_structure() {
        assert_eq!(DEFAULT[TRANSPARENT as usize], 0, "index 0 is black");

        // 1..=5 is a monotonically brightening grey ramp.
        let greys: Vec<u8> = (1..=5).map(|i| to_rgb8(DEFAULT[i]).0).collect();
        assert!(
            greys.windows(2).all(|pair| pair[0] < pair[1]),
            "greys must brighten: {greys:?}"
        );

        // 6..=15 are ten distinct hues, each far from grey.
        for (index, word) in DEFAULT.iter().enumerate().take(16).skip(6) {
            let (r, g, b) = to_rgb8(*word);
            let span = r.max(g).max(b) - r.min(g).min(b);
            assert!(span > 80, "base colour {index} is too grey");
        }

        // 16..=255 are fifteen ramps of sixteen strictly brightening steps.
        // Equal neighbouring words would show up here as equal luminance.
        for ramp in 0..15 {
            let start = 16 + ramp * 16;
            let luminance: Vec<u16> = (0..16)
                .map(|step| {
                    let (r, g, b) = to_rgb8(DEFAULT[start + step]);
                    (r as u16 * 3 + g as u16 * 6 + b as u16) / 10
                })
                .collect();
            assert!(
                luminance.windows(2).all(|pair| pair[0] < pair[1]),
                "ramp {ramp} is not strictly increasing: {luminance:?}"
            );
        }
    }

    #[test]
    fn default_table_starts_dark_and_ends_light() {
        for ramp in 0..15 {
            let start = 16 + ramp * 16;
            let (dark_r, _, _) = to_rgb8(DEFAULT[start]);
            let (light_r, light_g, light_b) = to_rgb8(DEFAULT[start + 15]);
            assert!(dark_r < 80, "ramp {ramp} does not start dark");
            assert!(
                light_r.max(light_g).max(light_b) > 180,
                "ramp {ramp} does not end light"
            );
        }
    }

    /// The committed table is an artefact, not a computation: this pins it so
    /// that an accidental edit is a test failure rather than a silent change
    /// to every cart's colours.
    #[test]
    fn default_table_is_pinned() {
        let checksum: u64 = DEFAULT.iter().map(|word| *word as u64).sum();
        assert_eq!(checksum, 3_261_492);
        assert_eq!(DEFAULT[1], 0x0863);
        assert_eq!(DEFAULT[5], 0x77BE);
        assert_eq!(DEFAULT[15], 0x7138);
        assert_eq!(DEFAULT[255], 0x7DD2);
    }

    #[test]
    // Three-three-two grouping is the point of the test: it is how the index
    // is read as a colour.
    #[allow(clippy::unusual_byte_groupings)]
    fn rgb332_indices_read_as_colours() {
        let table = rgb332();
        assert_eq!(to_rgb8(table[0b000_000_00]), (0, 0, 0));
        assert_eq!(to_rgb8(table[0b111_000_00]), (255, 0, 0));
        assert_eq!(to_rgb8(table[0b000_111_00]), (0, 255, 0));
        assert_eq!(to_rgb8(table[0b000_000_11]), (0, 0, 255));
        assert_eq!(to_rgb8(table[0b111_111_11]), (255, 255, 255));
        // 36 and 85 are the ideal 3-bit and 2-bit steps; RGB555 rounds them to
        // the nearest representable level, which is what a palette can hold.
        assert_eq!(to_rgb8(table[0b001_000_00]), (33, 0, 0));
        assert_eq!(to_rgb8(table[0b000_000_01]), (0, 0, 82));
    }

    /// Regenerates the [`DEFAULT`] table. Not a test: run it by hand when
    /// re-curating, then paste the output over the constant.
    #[test]
    #[ignore = "regenerates the committed DEFAULT table"]
    fn print_default_table() {
        let mut table = [0u16; limits::PALETTE_LEN];

        // 0 is black by definition. 1..=5 is a cool grey ramp.
        const GREYS: [(u8, u8, u8); 5] = [
            (20, 22, 28),
            (58, 62, 72),
            (100, 106, 118),
            (158, 164, 176),
            (235, 240, 248),
        ];
        for (offset, (r, g, b)) in GREYS.iter().enumerate() {
            table[1 + offset] = from_rgb8(*r, *g, *b);
        }

        // 6..=15 are the ten base hues, in the order named in SPEC §4.7.
        const BASE: [(u8, u8, u8); 10] = [
            (232, 40, 48),
            (255, 136, 32),
            (255, 232, 48),
            (160, 232, 40),
            (48, 200, 72),
            (32, 200, 168),
            (48, 208, 240),
            (48, 120, 240),
            (136, 88, 224),
            (232, 72, 200),
        ];
        for (offset, (r, g, b)) in BASE.iter().enumerate() {
            table[6 + offset] = from_rgb8(*r, *g, *b);
        }

        // 16..=255: fifteen ramps of sixteen steps. Each step raises HSV value
        // and lowers saturation, so shadows stay rich and highlights stay
        // chromatic instead of washing out to white.
        const HUES: [f64; 15] = [
            0.0, 22.0, 42.0, 70.0, 105.0, 140.0, 168.0, 188.0, 204.0, 222.0, 244.0, 264.0, 288.0,
            318.0, 344.0,
        ];
        for (ramp, hue) in HUES.iter().enumerate() {
            for step in 0..16 {
                let t = step as f64 / 15.0;
                let value = 0.15 + 0.85 * t;
                let saturation = 0.85 - 0.30 * t;
                let (r, g, b) = hsv_to_rgb(*hue, saturation, value);
                table[16 + ramp * 16 + step] = from_rgb8(r, g, b);
            }
        }

        let mut out = String::new();
        for chunk in table.chunks(8) {
            out.push_str("    ");
            for word in chunk {
                out.push_str(&format!("0x{word:04X}, "));
            }
            out.push('\n');
        }
        println!("{out}");
    }

    fn hsv_to_rgb(hue: f64, saturation: f64, value: f64) -> (u8, u8, u8) {
        let chroma = value * saturation;
        let sector = (hue / 60.0) % 6.0;
        let x = chroma * (1.0 - (sector % 2.0 - 1.0).abs());
        let (r, g, b) = match sector as u32 {
            0 => (chroma, x, 0.0),
            1 => (x, chroma, 0.0),
            2 => (0.0, chroma, x),
            3 => (0.0, x, chroma),
            4 => (x, 0.0, chroma),
            _ => (chroma, 0.0, x),
        };
        let match_value = value - chroma;
        let to8 =
            |component: f64| (((component + match_value) * 255.0).round()).clamp(0.0, 255.0) as u8;
        (to8(r), to8(g), to8(b))
    }
}
