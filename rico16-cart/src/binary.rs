//! SPEC §13.1, §13.4 — the binary cart: assets, code, modules and metadata.

use rico16_core::limits::{CART_FIXED_HEADER_LEN, MAP_LEN, PALETTE_LEN, SHEET_LEN};

use crate::error::CartError;
use crate::manifest::CartToml;

/// SPEC §13.1 — the binary cart magic.
pub const MAGIC: &[u8; 4] = b"R16C";
/// SPEC §13.1 — the only binary cart format version v1 accepts.
pub const FORMAT_VERSION: u16 = 1;

const MAGIC_OFFSET: usize = 0;
const VERSION_OFFSET: usize = 4;
const FLAGS_OFFSET: usize = 6;
const CODE_LEN_OFFSET: usize = 8;
const MODULES_LEN_OFFSET: usize = 12;
const METADATA_LEN_OFFSET: usize = 16;
const SHEET_OFFSET: usize = 20;
const MAP_OFFSET: usize = SHEET_OFFSET + SHEET_LEN;
const PALETTE_OFFSET: usize = MAP_OFFSET + MAP_LEN;

/// SPEC §13.1 — one embedded module: an import path and its Rhai source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    /// Normalized path, forward slashes, no `.rhai`, relative to `src/`.
    pub path: String,
    /// The module's Rhai source.
    pub source: String,
}

/// SPEC §13.1 — a decoded binary cart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryCart {
    /// The entry point's Rhai source (SPEC §13.1: the entry point is *not* in
    /// the module table).
    pub code: String,
    /// Embedded modules, sorted by path.
    pub modules: Vec<Module>,
    /// Cart metadata as TOML (SPEC §13.1).
    pub metadata: String,
    /// The 256×256 sprite sheet.
    pub sheet: Vec<u8>,
    /// The 128×128 tilemap.
    pub map: Vec<u8>,
    /// The resolved palette, 256 RGB555 words.
    pub palette: [u16; PALETTE_LEN],
}

impl BinaryCart {
    /// SPEC §13.1 — encode to the binary cart. Modules are written sorted by
    /// path so the payload is deterministic (SPEC §13.6).
    pub fn encode(&self) -> Result<Vec<u8>, CartError> {
        if self.sheet.len() != SHEET_LEN {
            return Err(CartError::Format(format!(
                "sprite sheet is {} bytes, must be exactly {SHEET_LEN}",
                self.sheet.len()
            )));
        }
        if self.map.len() != MAP_LEN {
            return Err(CartError::Format(format!(
                "tilemap is {} bytes, must be exactly {MAP_LEN}",
                self.map.len()
            )));
        }

        let mut modules: Vec<&Module> = self.modules.iter().collect();
        modules.sort_by(|a, b| a.path.cmp(&b.path));
        validate_modules(&modules)?;

        let mut module_table = Vec::new();
        for module in &modules {
            let path = module.path.as_bytes();
            let source = module.source.as_bytes();
            let path_len = u16::try_from(path.len())
                .map_err(|_| CartError::Format("module path is longer than 65535 bytes".into()))?;
            module_table.extend_from_slice(&path_len.to_le_bytes());
            module_table.extend_from_slice(path);
            let source_len = u32::try_from(source.len())
                .map_err(|_| CartError::Format("module source is longer than 2^32 bytes".into()))?;
            module_table.extend_from_slice(&source_len.to_le_bytes());
            module_table.extend_from_slice(source);
        }

        let code = self.code.as_bytes();
        let metadata = self.metadata.as_bytes();
        let code_len = u32::try_from(code.len())
            .map_err(|_| CartError::Format("code section is longer than 2^32 bytes".into()))?;
        let modules_len = u32::try_from(module_table.len())
            .map_err(|_| CartError::Format("module table is longer than 2^32 bytes".into()))?;
        let metadata_len = u32::try_from(metadata.len())
            .map_err(|_| CartError::Format("metadata is longer than 2^32 bytes".into()))?;

        let mut out = Vec::with_capacity(
            CART_FIXED_HEADER_LEN + code.len() + module_table.len() + metadata.len(),
        );
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&code_len.to_le_bytes());
        out.extend_from_slice(&modules_len.to_le_bytes());
        out.extend_from_slice(&metadata_len.to_le_bytes());
        out.extend_from_slice(&self.sheet);
        out.extend_from_slice(&self.map);
        for word in &self.palette {
            out.extend_from_slice(&word.to_le_bytes());
        }
        out.extend_from_slice(code);
        out.extend_from_slice(&module_table);
        out.extend_from_slice(metadata);
        Ok(out)
    }

    /// SPEC §13.4 — decode and validate a binary cart.
    pub fn decode(bytes: &[u8]) -> Result<Self, CartError> {
        if bytes.len() < CART_FIXED_HEADER_LEN {
            return Err(CartError::Format(format!(
                "cart is {} bytes, shorter than the {CART_FIXED_HEADER_LEN}-byte fixed header",
                bytes.len()
            )));
        }
        if &bytes[MAGIC_OFFSET..MAGIC_OFFSET + 4] != MAGIC {
            return Err(CartError::Format("bad magic, expected R16C".into()));
        }
        let version = u16::from_le_bytes([bytes[VERSION_OFFSET], bytes[VERSION_OFFSET + 1]]);
        if version != FORMAT_VERSION {
            return Err(CartError::Format(format!(
                "unsupported cart format version {version}, expected {FORMAT_VERSION}"
            )));
        }
        // SPEC §13.1: `flags` is reserved; loaders ignore unknown bits.
        let _flags = u16::from_le_bytes([bytes[FLAGS_OFFSET], bytes[FLAGS_OFFSET + 1]]);

        let code_len = read_u32(bytes, CODE_LEN_OFFSET) as usize;
        let modules_len = read_u32(bytes, MODULES_LEN_OFFSET) as usize;
        let metadata_len = read_u32(bytes, METADATA_LEN_OFFSET) as usize;

        let body = &bytes[CART_FIXED_HEADER_LEN..];
        let expected = code_len
            .checked_add(modules_len)
            .and_then(|n| n.checked_add(metadata_len))
            .ok_or_else(|| CartError::Format("length fields overflow".into()))?;
        if body.len() != expected {
            return Err(CartError::Format(format!(
                "length fields total {expected} bytes but {} bytes follow the header",
                body.len()
            )));
        }

        let sheet = bytes[SHEET_OFFSET..MAP_OFFSET].to_vec();
        let map = bytes[MAP_OFFSET..PALETTE_OFFSET].to_vec();

        let mut palette = [0u16; PALETTE_LEN];
        for (i, word) in palette.iter_mut().enumerate() {
            let at = PALETTE_OFFSET + i * 2;
            let value = u16::from_le_bytes([bytes[at], bytes[at + 1]]);
            if value & 0x8000 != 0 {
                return Err(CartError::Format(format!(
                    "palette entry {i} has bit 15 set"
                )));
            }
            *word = value;
        }

        let code = utf8(&body[..code_len], "code section")?.to_string();
        let module_bytes = &body[code_len..code_len + modules_len];
        let metadata = utf8(&body[code_len + modules_len..], "metadata")?.to_string();

        let modules = decode_modules(module_bytes)?;

        toml::from_str::<toml::Value>(&metadata)
            .map_err(|e| CartError::Format(format!("metadata is not valid TOML: {e}")))?;

        Ok(Self {
            code,
            modules,
            metadata,
            sheet,
            map,
            palette,
        })
    }
}

/// SPEC §13.1 — build the metadata TOML from a manifest.
pub fn metadata_from_manifest(manifest: &CartToml) -> String {
    let mut table = toml::Table::new();
    table.insert(
        "title".into(),
        toml::Value::String(manifest.cart.title.clone()),
    );
    table.insert(
        "author".into(),
        toml::Value::String(manifest.cart.author.clone()),
    );
    table.insert(
        "version".into(),
        toml::Value::String(manifest.cart.version.clone()),
    );
    table.insert(
        "entry".into(),
        toml::Value::String(manifest.entry().to_string()),
    );
    for (key, value) in &manifest.cart.extra {
        table.insert(key.clone(), value.clone());
    }
    toml::to_string(&toml::Value::Table(table)).expect("metadata always serializes")
}

fn decode_modules(bytes: &[u8]) -> Result<Vec<Module>, CartError> {
    let mut modules = Vec::new();
    let mut pos = 0;
    while pos < bytes.len() {
        let path_len = read_u16(bytes, pos, "module path length")? as usize;
        pos += 2;
        let path = utf8(span(bytes, pos, path_len, "module path")?, "module path")?.to_string();
        pos += path_len;
        let source_len = read_u32_checked(bytes, pos, "module source length")? as usize;
        pos += 4;
        let source = utf8(
            span(bytes, pos, source_len, "module source")?,
            "module source",
        )?
        .to_string();
        pos += source_len;
        modules.push(Module { path, source });
    }
    let refs: Vec<&Module> = modules.iter().collect();
    validate_modules(&refs)?;
    Ok(modules)
}

/// SPEC §13.4 — module paths are unique, relative, free of `.rhai`, and sorted.
fn validate_modules(modules: &[&Module]) -> Result<(), CartError> {
    for (i, module) in modules.iter().enumerate() {
        let path = &module.path;
        if path.is_empty() {
            return Err(CartError::Format("module path is empty".into()));
        }
        if path.starts_with('/') || path.contains('\\') {
            return Err(CartError::Format(format!(
                "module path `{path}` must be relative with forward slashes"
            )));
        }
        if path.split('/').any(|part| part == ".." || part == ".") {
            return Err(CartError::Format(format!(
                "module path `{path}` must not contain `.` or `..`"
            )));
        }
        if path.ends_with(".rhai") {
            return Err(CartError::Format(format!(
                "module path `{path}` must not carry the `.rhai` extension"
            )));
        }
        if i > 0 && modules[i - 1].path >= *path {
            return Err(CartError::Format(format!(
                "module paths must be unique and sorted; `{path}` is out of order"
            )));
        }
    }
    Ok(())
}

fn utf8<'a>(bytes: &'a [u8], what: &str) -> Result<&'a str, CartError> {
    std::str::from_utf8(bytes).map_err(|_| CartError::Format(format!("{what} is not valid UTF-8")))
}

fn span<'a>(bytes: &'a [u8], pos: usize, len: usize, what: &str) -> Result<&'a [u8], CartError> {
    bytes
        .get(pos..pos + len)
        .ok_or_else(|| CartError::Format(format!("module table truncated in {what}")))
}

fn read_u16(bytes: &[u8], pos: usize, what: &str) -> Result<u16, CartError> {
    let raw = span(bytes, pos, 2, what)?;
    Ok(u16::from_le_bytes([raw[0], raw[1]]))
}

fn read_u32_checked(bytes: &[u8], pos: usize, what: &str) -> Result<u32, CartError> {
    let raw = span(bytes, pos, 4, what)?;
    Ok(u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]))
}

fn read_u32(bytes: &[u8], pos: usize) -> u32 {
    u32::from_le_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]])
}
