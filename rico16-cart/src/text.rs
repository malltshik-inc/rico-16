//! The `.r16` text format: parsing, validation and canonical serialization.
//!
//! Normative reference: `docs/SPEC.md` §10.

use std::fmt::Write as _;

use rico16_core::limits;

/// Number of palette entries per row in the `__palette__` section. SPEC §10.6.
const PALETTE_ROW: usize = 16;

/// Number of rows in the `__palette__` section. SPEC §10.6.
const PALETTE_ROWS: usize = limits::PALETTE_LEN / PALETTE_ROW;

/// Metadata carried by `__meta__`. SPEC §10.2.
///
/// The four known keys are written first, in a fixed order. Unknown keys are
/// kept in their original order and written after them, so that a newer cart
/// survives an older tool without losing information.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Meta {
    pub title: String,
    pub author: String,
    pub version: String,
    pub description: String,
    pub extra: Vec<(String, String)>,
}

impl Meta {
    /// True when there is nothing to write, in which case the section is
    /// omitted from the canonical form. SPEC §10.7.
    pub fn is_empty(&self) -> bool {
        self.title.is_empty()
            && self.author.is_empty()
            && self.version.is_empty()
            && self.description.is_empty()
            && self.extra.is_empty()
    }
}

/// A cart, in the in-memory shape every other part of the console uses.
///
/// `gfx` and `map` are flat byte arrays whose lengths are fixed by the
/// specification; `validate` is the cheap way to re-check that after building
/// one by hand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cart {
    pub meta: Meta,
    pub code: String,
    pub gfx: Vec<u8>,
    pub map: Vec<u8>,
    pub palette: Option<[u16; limits::PALETTE_LEN]>,
    /// Sections this version does not understand, preserved verbatim.
    pub unknown: Vec<(String, String)>,
}

/// Everything that can be wrong with a cart.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    #[error("not a cart: no `__section__` header found")]
    NotACart,

    #[error("line {line}: text before the first `__section__` header")]
    TextBeforeFirstSection { line: usize },

    #[error("duplicate section `__{0}__`")]
    DuplicateSection(String),

    #[error("`__{section}__`: expected {expected} rows, found {found}")]
    RowCount {
        section: &'static str,
        expected: usize,
        found: usize,
    },

    #[error("`__{section}__` row {row}: expected {expected} hex digits, found {found}")]
    RowWidth {
        section: &'static str,
        row: usize,
        expected: usize,
        found: usize,
    },

    #[error("`__{section}__` row {row} column {column}: `{ch}` is not a hex digit")]
    BadHex {
        section: &'static str,
        row: usize,
        column: usize,
        ch: char,
    },

    #[error("`__palette__` row {row}: expected {expected} words, found {found}")]
    PaletteRow {
        row: usize,
        expected: usize,
        found: usize,
    },

    #[error("`__palette__` row {row} word {word}: expected four hex digits")]
    PaletteWord { row: usize, word: usize },

    #[error("`__palette__` entry {index}: reserved bit 15 must be 0")]
    PaletteReservedBit { index: usize },

    #[error("`__meta__` line {line}: expected `key=value`")]
    MetaLine { line: usize },

    #[error("`__meta__` line {line}: `{key}` is not a valid key")]
    MetaKey { line: usize, key: String },

    #[error("code is {actual} bytes, the limit is {limit}")]
    CodeTooLong { actual: usize, limit: usize },

    #[error("{name} is {found} bytes, the console requires {expected}")]
    BadResourceLength {
        name: &'static str,
        expected: usize,
        found: usize,
    },
}

impl Default for Cart {
    fn default() -> Self {
        Self::new("")
    }
}

impl Cart {
    /// An empty cart: all-zero art, no code, no palette — so it uses the
    /// console default palette (SPEC §10.6).
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            meta: Meta {
                title: title.into(),
                ..Meta::default()
            },
            code: String::new(),
            gfx: vec![0; limits::SHEET_LEN],
            map: vec![0; limits::MAP_LEN],
            palette: None,
            unknown: Vec::new(),
        }
    }

    /// Parse the text form. SPEC §10.
    ///
    /// Reads permissively — BOM, CRLF, lowercase hex, any section order — and
    /// rejects structural damage rather than guessing.
    pub fn parse(source: &str) -> Result<Self, ParseError> {
        let source = source.strip_prefix('\u{feff}').unwrap_or(source);
        let text = source.replace("\r\n", "\n");

        let mut lines: Vec<&str> = text.split('\n').collect();
        if text.ends_with('\n') {
            // The final newline terminates the last line; it is not a line of
            // its own and must not end up inside a section payload.
            lines.pop();
        }

        let mut headers: Vec<(usize, &str)> = Vec::new();
        for (index, line) in lines.iter().enumerate() {
            if let Some(name) = header_name(line) {
                headers.push((index, name));
            }
        }

        if headers.is_empty() {
            if text.trim().is_empty() {
                return Ok(Self::default());
            }
            return Err(ParseError::NotACart);
        }

        if let Some((line, _)) = lines
            .iter()
            .take(headers[0].0)
            .enumerate()
            .find(|(_, line)| !line.trim().is_empty())
        {
            return Err(ParseError::TextBeforeFirstSection { line: line + 1 });
        }

        let mut cart = Self::default();
        let mut seen: Vec<&str> = Vec::new();

        for (position, (start, name)) in headers.iter().enumerate() {
            let name = *name;
            if seen.contains(&name) {
                return Err(ParseError::DuplicateSection(name.to_owned()));
            }
            seen.push(name);

            let end = headers
                .get(position + 1)
                .map_or(lines.len(), |(next, _)| *next);
            let payload = lines[start + 1..end].join("\n");

            match name {
                "meta" => cart.meta = parse_meta(&payload)?,
                "code" => cart.code = payload,
                "gfx" => {
                    cart.gfx = parse_hex_block(
                        "gfx",
                        limits::SHEET_HEIGHT,
                        limits::SHEET_WIDTH,
                        &payload,
                    )?;
                }
                "map" => {
                    cart.map =
                        parse_hex_block("map", limits::MAP_HEIGHT, limits::MAP_WIDTH, &payload)?;
                }
                "palette" => cart.palette = Some(parse_palette(&payload)?),
                other => cart.unknown.push((other.to_owned(), payload)),
            }
        }

        cart.validate()?;
        Ok(cart)
    }

    /// Check the invariants the in-memory shape promises. SPEC §7.4.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.code.len() > limits::CODE_LIMIT {
            return Err(ParseError::CodeTooLong {
                actual: self.code.len(),
                limit: limits::CODE_LIMIT,
            });
        }
        if self.gfx.len() != limits::SHEET_LEN {
            return Err(ParseError::BadResourceLength {
                name: "gfx",
                expected: limits::SHEET_LEN,
                found: self.gfx.len(),
            });
        }
        if self.map.len() != limits::MAP_LEN {
            return Err(ParseError::BadResourceLength {
                name: "map",
                expected: limits::MAP_LEN,
                found: self.map.len(),
            });
        }
        Ok(())
    }

    /// Serialize into canonical form. SPEC §10.7.
    ///
    /// Sections that carry no information are omitted, so a fresh cart is a
    /// couple of lines rather than a hundred kilobytes of zeroes.
    pub fn to_text(&self) -> String {
        let mut out = String::new();

        let meta = meta_text(&self.meta);
        if !meta.is_empty() {
            push_section(&mut out, "meta", &meta);
        }
        push_section(&mut out, "code", &self.code);
        if self.gfx.iter().any(|&pixel| pixel != 0) {
            push_section(&mut out, "gfx", &hex_rows(&self.gfx, limits::SHEET_WIDTH));
        }
        if self.map.iter().any(|&cell| cell != 0) {
            push_section(&mut out, "map", &hex_rows(&self.map, limits::MAP_WIDTH));
        }
        if let Some(palette) = &self.palette {
            push_section(&mut out, "palette", &palette_text(palette));
        }
        for (name, payload) in &self.unknown {
            push_section(&mut out, name, payload);
        }

        out
    }
}

/// The name inside a `__name__` header, or `None` for any other line. SPEC §10.1.2.
fn header_name(line: &str) -> Option<&str> {
    let inner = line.strip_prefix("__")?.strip_suffix("__")?;
    if inner.is_empty() {
        return None;
    }
    let valid = inner
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    valid.then_some(inner)
}

fn is_key_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.'
}

fn parse_meta(payload: &str) -> Result<Meta, ParseError> {
    let mut meta = Meta::default();

    for (index, line) in payload.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(ParseError::MetaLine { line: index + 1 });
        };
        let key = key.trim();
        if key.is_empty() || !key.bytes().all(is_key_char) {
            return Err(ParseError::MetaKey {
                line: index + 1,
                key: key.to_owned(),
            });
        }
        let value = value.trim().to_owned();
        match key {
            "title" => meta.title = value,
            "author" => meta.author = value,
            "version" => meta.version = value,
            "description" => meta.description = value,
            _ => meta.extra.push((key.to_owned(), value)),
        }
    }

    Ok(meta)
}

fn meta_text(meta: &Meta) -> String {
    let mut out = String::new();
    for (key, value) in [
        ("title", &meta.title),
        ("author", &meta.author),
        ("version", &meta.version),
        ("description", &meta.description),
    ] {
        if !value.is_empty() {
            let _ = writeln!(out, "{key}={}", value.trim());
        }
    }
    for (key, value) in &meta.extra {
        let _ = writeln!(out, "{key}={}", value.trim());
    }
    if out.ends_with('\n') {
        out.pop();
    }
    out
}

fn parse_hex_block(
    section: &'static str,
    rows: usize,
    pixels_per_row: usize,
    payload: &str,
) -> Result<Vec<u8>, ParseError> {
    let found: Vec<&str> = payload.lines().collect();
    if found.len() != rows {
        return Err(ParseError::RowCount {
            section,
            expected: rows,
            found: found.len(),
        });
    }

    let expected = pixels_per_row * 2;
    let mut out = Vec::with_capacity(rows * pixels_per_row);

    for (index, row) in found.iter().enumerate() {
        let row = row.trim_end_matches('\r');
        // Check ASCII before slicing: a multi-byte character would otherwise
        // let a wrong-width row pass the length test and then panic.
        if let Some(column) = row.bytes().position(|b| !b.is_ascii()) {
            return Err(ParseError::BadHex {
                section,
                row: index + 1,
                column,
                ch: row[column..].chars().next().unwrap_or('?'),
            });
        }
        if row.len() != expected {
            return Err(ParseError::RowWidth {
                section,
                row: index + 1,
                expected,
                found: row.len(),
            });
        }
        for (pair, chunk) in row.as_bytes().chunks_exact(2).enumerate() {
            let high = hex_value(chunk[0]);
            let low = hex_value(chunk[1]);
            match (high, low) {
                (Some(high), Some(low)) => out.push((high << 4) | low),
                _ => {
                    let (offset, byte) = if high.is_none() {
                        (0, chunk[0])
                    } else {
                        (1, chunk[1])
                    };
                    return Err(ParseError::BadHex {
                        section,
                        row: index + 1,
                        column: pair * 2 + offset,
                        ch: byte as char,
                    });
                }
            }
        }
    }

    Ok(out)
}

fn hex_value(byte: u8) -> Option<u8> {
    (byte as char).to_digit(16).map(|digit| digit as u8)
}

fn hex_rows(bytes: &[u8], per_row: usize) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";

    let mut out = String::with_capacity(bytes.len() * 2 + bytes.len() / per_row);
    for (index, chunk) in bytes.chunks(per_row).enumerate() {
        if index > 0 {
            out.push('\n');
        }
        for &byte in chunk {
            out.push(HEX[(byte >> 4) as usize] as char);
            out.push(HEX[(byte & 0x0f) as usize] as char);
        }
    }
    out
}

fn parse_palette(payload: &str) -> Result<[u16; limits::PALETTE_LEN], ParseError> {
    let rows: Vec<&str> = payload.lines().collect();
    if rows.len() != PALETTE_ROWS {
        return Err(ParseError::RowCount {
            section: "palette",
            expected: PALETTE_ROWS,
            found: rows.len(),
        });
    }

    let mut palette = [0u16; limits::PALETTE_LEN];

    for (row_index, row) in rows.iter().enumerate() {
        let words: Vec<&str> = row.trim_end_matches('\r').split_whitespace().collect();
        if words.len() != PALETTE_ROW {
            return Err(ParseError::PaletteRow {
                row: row_index + 1,
                expected: PALETTE_ROW,
                found: words.len(),
            });
        }
        for (word_index, word) in words.iter().enumerate() {
            if word.len() != 4 || !word.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(ParseError::PaletteWord {
                    row: row_index + 1,
                    word: word_index + 1,
                });
            }
            let value = u16::from_str_radix(word, 16).map_err(|_| ParseError::PaletteWord {
                row: row_index + 1,
                word: word_index + 1,
            })?;
            if value & 0x8000 != 0 {
                return Err(ParseError::PaletteReservedBit {
                    index: row_index * PALETTE_ROW + word_index,
                });
            }
            palette[row_index * PALETTE_ROW + word_index] = value;
        }
    }

    Ok(palette)
}

fn palette_text(palette: &[u16; limits::PALETTE_LEN]) -> String {
    let mut out = String::with_capacity(PALETTE_ROWS * (PALETTE_ROW * 5));
    for (row_index, row) in palette.chunks(PALETTE_ROW).enumerate() {
        if row_index > 0 {
            out.push('\n');
        }
        for (index, word) in row.iter().enumerate() {
            if index > 0 {
                out.push(' ');
            }
            let _ = write!(out, "{word:04X}");
        }
    }
    out
}

fn push_section(out: &mut String, name: &str, payload: &str) {
    out.push_str("__");
    out.push_str(name);
    out.push_str("__\n");
    if !payload.is_empty() {
        out.push_str(payload);
        out.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = "__code__\nfn on_draw() { }\n";

    fn gfx_block(pair: &str) -> String {
        let row = pair.repeat(limits::SHEET_WIDTH);
        std::iter::repeat_n(row, limits::SHEET_HEIGHT)
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn parses_a_minimal_cart() {
        let cart = Cart::parse(MINIMAL).unwrap();
        assert_eq!(cart.code, "fn on_draw() { }");
        assert_eq!(cart.gfx, vec![0; limits::SHEET_LEN]);
        assert_eq!(cart.map, vec![0; limits::MAP_LEN]);
        assert!(cart.palette.is_none());
        assert_eq!(cart.to_text(), MINIMAL);
    }

    #[test]
    fn empty_input_is_a_default_cart() {
        assert_eq!(Cart::parse("").unwrap(), Cart::default());
        assert_eq!(Cart::parse("  \n\n").unwrap(), Cart::default());
    }

    #[test]
    fn canonical_roundtrip_is_byte_identical() {
        let mut cart = Cart::new("demo");
        cart.meta.author = "someone".to_owned();
        cart.meta
            .extra
            .push(("genre".to_owned(), "arcade".to_owned()));
        cart.code = "fn on_draw() {\n    gfx::clear(0);\n}\n".to_owned();
        cart.gfx[0] = 0x0f;
        cart.gfx[limits::SHEET_LEN - 1] = 0xa5;
        cart.map[7] = 3;
        cart.palette = Some([0x1234; limits::PALETTE_LEN]);

        let text = cart.to_text();
        let reparsed = Cart::parse(&text).unwrap();
        assert_eq!(reparsed, cart);
        assert_eq!(reparsed.to_text(), text);
    }

    #[test]
    fn serializing_is_idempotent_for_every_input() {
        let inputs = [
            MINIMAL,
            "__meta__\ntitle=t\n__code__\nfn a() {}\n",
            "__map__\n",
            "stray text\n__code__\nfn a() {}\n",
        ];
        // The last two are malformed on purpose; anything that parses must
        // reach a fixed point after one rewrite.
        for input in inputs {
            if let Ok(cart) = Cart::parse(input) {
                let once = cart.to_text();
                let twice = Cart::parse(&once).unwrap().to_text();
                assert_eq!(once, twice, "input: {input:?}");
            }
        }
    }

    #[test]
    fn reads_permissively() {
        let source = "\u{feff}__code__\r\nfn a() {}\r\n";
        let cart = Cart::parse(source).unwrap();
        assert_eq!(cart.code, "fn a() {}");
        assert_eq!(cart.to_text(), "__code__\nfn a() {}\n");
    }

    #[test]
    fn accepts_lowercase_hex_and_writes_uppercase() {
        let source = format!("__gfx__\n{}\n", gfx_block("ab"));
        let cart = Cart::parse(&source).unwrap();
        assert_eq!(cart.gfx[0], 0xab);
        let text = cart.to_text();
        assert!(text.contains("AB"));
        assert!(!text.contains("ab"));
    }

    #[test]
    fn all_zero_art_is_omitted() {
        let source = format!("__code__\n\n__gfx__\n{}\n", gfx_block("00"));
        let cart = Cart::parse(&source).unwrap();
        assert!(!cart.to_text().contains("__gfx__"));
    }

    #[test]
    fn rejects_duplicate_sections() {
        let error = Cart::parse("__code__\na\n__code__\nb\n").unwrap_err();
        assert_eq!(error, ParseError::DuplicateSection("code".to_owned()));
    }

    #[test]
    fn rejects_wrong_row_count() {
        let error = Cart::parse("__gfx__\n00\n").unwrap_err();
        assert!(matches!(
            error,
            ParseError::RowCount {
                section: "gfx",
                expected: 256,
                found: 1
            }
        ));
    }

    #[test]
    fn rejects_wrong_row_width() {
        let mut block = gfx_block("00");
        block.truncate(block.len() - 1);
        let error = Cart::parse(&format!("__gfx__\n{block}\n")).unwrap_err();
        assert!(matches!(error, ParseError::RowWidth { found: 511, .. }));
    }

    #[test]
    fn rejects_non_hex_without_panicking() {
        let row = "0\u{e9}".repeat(limits::SHEET_WIDTH);
        let block = std::iter::repeat_n(row, limits::SHEET_HEIGHT)
            .collect::<Vec<_>>()
            .join("\n");
        let error = Cart::parse(&format!("__gfx__\n{block}\n")).unwrap_err();
        assert!(matches!(error, ParseError::BadHex { column: 1, .. }));
    }

    #[test]
    fn rejects_oversized_code() {
        let code = "a".repeat(limits::CODE_LIMIT + 1);
        let error = Cart::parse(&format!("__code__\n{code}\n")).unwrap_err();
        assert!(matches!(error, ParseError::CodeTooLong { .. }));
    }

    #[test]
    fn rejects_palette_reserved_bit() {
        let row = vec!["0000"; 16].join(" ");
        let mut rows = vec![row.as_str(); 16];
        rows[3] = "8000 0000 0000 0000 0000 0000 0000 0000 0000 0000 0000 0000 0000 0000 0000 0000";
        let error = Cart::parse(&format!("__palette__\n{}\n", rows.join("\n"))).unwrap_err();
        assert_eq!(error, ParseError::PaletteReservedBit { index: 48 });
    }

    #[test]
    fn preserves_unknown_sections_and_meta_keys() {
        let source = "__meta__\ntitle=t\ngenre=arcade\n__code__\nfn a() {}\n__audio__\npattern=1\n";
        let cart = Cart::parse(source).unwrap();
        assert_eq!(cart.meta.extra, [("genre".to_owned(), "arcade".to_owned())]);
        assert_eq!(cart.unknown, [("audio".to_owned(), "pattern=1".to_owned())]);
        let text = cart.to_text();
        assert!(text.contains("genre=arcade"));
        assert!(text.ends_with("__audio__\npattern=1\n"));
        assert_eq!(Cart::parse(&text).unwrap(), cart);
    }

    #[test]
    fn rejects_text_before_the_first_section() {
        let error = Cart::parse("oops\n__code__\n").unwrap_err();
        assert_eq!(error, ParseError::TextBeforeFirstSection { line: 1 });
    }

    #[test]
    fn rejects_a_file_with_no_sections() {
        assert_eq!(Cart::parse("hello\n").unwrap_err(), ParseError::NotACart);
    }

    #[test]
    fn code_keeps_blank_lines_but_not_the_section_terminator() {
        let cart = Cart::parse("__code__\na\n\nb\n").unwrap();
        assert_eq!(cart.code, "a\n\nb");
    }
}
