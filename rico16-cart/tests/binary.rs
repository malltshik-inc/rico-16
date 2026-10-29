//! SPEC §13.1, §13.4, §20.6 — the binary cart format and CRC.

use rico16_cart::binary::{BinaryCart, Module, metadata_from_manifest};
use rico16_cart::{CartToml, crc32};
use rico16_core::limits::{CART_FIXED_HEADER_LEN, MAP_LEN, PALETTE_LEN, SHEET_LEN};

fn sample() -> BinaryCart {
    let mut palette = [0u16; PALETTE_LEN];
    for (i, word) in palette.iter_mut().enumerate() {
        *word = i as u16;
    }
    BinaryCart {
        code: "fn on_init() {}\n".to_string(),
        modules: vec![
            Module {
                path: "lib/grid".to_string(),
                source: "fn cell() {}\n".to_string(),
            },
            Module {
                path: "scenes/play".to_string(),
                source: "fn update(dt) {}\n".to_string(),
            },
        ],
        metadata: "title = \"t\"\nauthor = \"\"\nversion = \"\"\nentry = \"src/main.rhai\"\n"
            .to_string(),
        sheet: vec![7u8; SHEET_LEN],
        map: vec![3u8; MAP_LEN],
        palette,
    }
}

/// Hand-build raw cart bytes with an arbitrary (possibly invalid) module table,
/// so decode validation can be exercised directly.
fn raw_cart(code: &str, modules: &[(&str, &str)], metadata: &str) -> Vec<u8> {
    let mut table = Vec::new();
    for (path, src) in modules {
        table.extend_from_slice(&(path.len() as u16).to_le_bytes());
        table.extend_from_slice(path.as_bytes());
        table.extend_from_slice(&(src.len() as u32).to_le_bytes());
        table.extend_from_slice(src.as_bytes());
    }
    let mut out = Vec::new();
    out.extend_from_slice(b"R16C");
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&(code.len() as u32).to_le_bytes());
    out.extend_from_slice(&(table.len() as u32).to_le_bytes());
    out.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
    out.extend_from_slice(&[0u8; SHEET_LEN]);
    out.extend_from_slice(&[0u8; MAP_LEN]);
    out.extend_from_slice(&[0u8; PALETTE_LEN * 2]);
    out.extend_from_slice(code.as_bytes());
    out.extend_from_slice(&table);
    out.extend_from_slice(metadata.as_bytes());
    out
}

/// SPEC §20.6 — CRC-32 of `"123456789"`.
#[test]
fn crc_vector() {
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
}

/// SPEC §13.1 — the fixed header fields sit at the documented offsets.
#[test]
fn header_layout() {
    let cart = sample();
    let bytes = cart.encode().expect("encode");
    assert_eq!(&bytes[0..4], b"R16C");
    assert_eq!(
        u16::from_le_bytes([bytes[4], bytes[5]]),
        1,
        "format version"
    );
    assert_eq!(u16::from_le_bytes([bytes[6], bytes[7]]), 0, "flags");
    assert_eq!(
        u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize,
        cart.code.len()
    );
    assert_eq!(
        bytes.len(),
        CART_FIXED_HEADER_LEN + cart.code.len() + module_table_len(&cart) + cart.metadata.len()
    );
    assert_eq!(bytes[20..20 + SHEET_LEN], cart.sheet[..]);
    assert_eq!(
        bytes[20 + SHEET_LEN..20 + SHEET_LEN + MAP_LEN],
        cart.map[..]
    );
}

/// SPEC §13.1 — encode then decode round-trips, with modules sorted.
#[test]
fn binary_round_trip() {
    let cart = sample();
    let bytes = cart.encode().expect("encode");
    let decoded = BinaryCart::decode(&bytes).expect("decode");
    assert_eq!(decoded.code, cart.code);
    assert_eq!(decoded.metadata, cart.metadata);
    assert_eq!(decoded.sheet, cart.sheet);
    assert_eq!(decoded.map, cart.map);
    assert_eq!(decoded.palette, cart.palette);
    assert_eq!(decoded.modules.len(), 2);
    assert_eq!(decoded.modules[0].path, "lib/grid", "sorted by path");
    assert_eq!(decoded.modules[1].path, "scenes/play");
}

/// SPEC §13.6 — the same input encodes to identical bytes.
#[test]
fn payload_is_deterministic() {
    assert_eq!(sample().encode().unwrap(), sample().encode().unwrap());
}

/// SPEC §13.4 — a bad magic is named in the error.
#[test]
fn rejects_bad_magic() {
    let mut bytes = sample().encode().unwrap();
    bytes[0] = b'X';
    let err = BinaryCart::decode(&bytes).unwrap_err().to_string();
    assert!(err.contains("magic"), "{err}");
}

/// SPEC §13.4 — a short cart is rejected rather than read out of bounds.
#[test]
fn rejects_truncated() {
    assert!(BinaryCart::decode(&[0u8; 10]).is_err());
}

/// SPEC §13.4 — the three length fields must account for every remaining byte.
#[test]
fn rejects_length_mismatch() {
    let mut bytes = sample().encode().unwrap();
    let code_len = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    bytes[8..12].copy_from_slice(&(code_len + 1).to_le_bytes());
    let err = BinaryCart::decode(&bytes).unwrap_err().to_string();
    assert!(err.contains("length fields"), "{err}");
}

/// SPEC §13.4 — palette entries must have bit 15 clear.
#[test]
fn rejects_reserved_palette_bit() {
    let mut cart = sample();
    cart.palette[5] = 0x8000;
    let bytes = cart.encode().unwrap();
    let err = BinaryCart::decode(&bytes).unwrap_err().to_string();
    assert!(err.contains("bit 15"), "{err}");
}

/// SPEC §13.4 — module paths must be unique and sorted.
#[test]
fn rejects_unsorted_modules() {
    let bytes = raw_cart("", &[("b", ""), ("a", "")], "title = \"t\"\n");
    let err = BinaryCart::decode(&bytes).unwrap_err().to_string();
    assert!(err.contains("sorted"), "{err}");
}

/// SPEC §13.4 — a `.rhai` extension in a module path is rejected.
#[test]
fn rejects_rhai_extension() {
    let bytes = raw_cart("", &[("lib/grid.rhai", "")], "title = \"t\"\n");
    let err = BinaryCart::decode(&bytes).unwrap_err().to_string();
    assert!(err.contains(".rhai"), "{err}");
}

/// SPEC §13.4 — absolute paths and `..` escapes are rejected.
#[test]
fn rejects_non_relative_paths() {
    for path in ["/etc/passwd", "../escape", "a/../b"] {
        let bytes = raw_cart("", &[(path, "")], "title = \"t\"\n");
        assert!(
            BinaryCart::decode(&bytes).is_err(),
            "path `{path}` should be rejected"
        );
    }
}

/// SPEC §13.4 — metadata must be valid UTF-8 TOML.
#[test]
fn rejects_bad_metadata() {
    let bytes = raw_cart("", &[], "this is not = = toml");
    assert!(BinaryCart::decode(&bytes).is_err());
}

/// SPEC §13.1 — metadata is built from the manifest and is valid TOML.
#[test]
fn metadata_from_manifest_has_required_keys() {
    let manifest = CartToml::parse_str(
        "format = 1\n\n[cart]\ntitle = \"Snake\"\nauthor = \"me\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    let metadata = metadata_from_manifest(&manifest);
    let value: toml::Value = toml::from_str(&metadata).unwrap();
    let table = value.as_table().unwrap();
    assert_eq!(table["title"].as_str(), Some("Snake"));
    assert_eq!(table["author"].as_str(), Some("me"));
    assert_eq!(table["version"].as_str(), Some("0.1.0"));
    assert_eq!(table["entry"].as_str(), Some("src/main.rhai"));
}

fn module_table_len(cart: &BinaryCart) -> usize {
    let mut sorted: Vec<&Module> = cart.modules.iter().collect();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));
    sorted
        .iter()
        .map(|m| 2 + m.path.len() + 4 + m.source.len())
        .sum()
}
