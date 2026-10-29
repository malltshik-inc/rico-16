//! SPEC §11.2, §12 — the build pipeline: assets, imports, assembly, packing.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use rico16_core::limits::{MAP_LEN, SHEET_LEN, TILE_SIZE};
use rico16_core::palette::{DEFAULT_PALETTE, expand5, quantize5};

use crate::binary::{BinaryCart, Module, metadata_from_manifest};
use crate::error::CartError;
use crate::manifest::CartToml;
use crate::png::{
    Art, Variant, cover_fit, decode_png_rgba8, generated_label, map_tile_ids, pack, pack_auto,
};

/// SPEC §11.2 — module names that resolve to native modules, never to files.
pub const RESERVED_MODULES: [&str; 8] = [
    "gfx", "sprite", "tilemap", "input", "system", "audio", "rnd", "Button",
];

/// Options for [`build_project`].
#[derive(Debug, Clone, Default)]
pub struct BuildOptions {
    /// Custom cover art PNG, if any (SPEC §13.2.5).
    pub label: Option<PathBuf>,
    /// Force a PNG size variant instead of auto-selecting (SPEC §15.2).
    pub variant: Option<Variant>,
}

/// The result of a build.
#[derive(Debug, Clone)]
pub struct BuildOutput {
    /// The assembled binary cart.
    pub cart: BinaryCart,
    /// The packed PNG.
    pub png: Vec<u8>,
    /// The variant that was used.
    pub variant: Variant,
    /// Payload bytes used (blob header plus binary cart).
    pub payload_used: usize,
    /// Payload bytes available in the chosen variant.
    pub capacity: usize,
}

/// SPEC §12.4 — build a project directory into a packed PNG.
pub fn build_project(root: &Path, options: &BuildOptions) -> Result<BuildOutput, CartError> {
    let manifest = read_manifest(root)?;
    let cart = assemble_from(root, &manifest)?;
    let bytes = cart.encode()?;

    let custom: Option<(u32, u32, Vec<u8>)> = match &options.label {
        Some(path) => {
            let raw = fs::read(path).map_err(|e| CartError::io(path, e))?;
            let (w, h, rgba) = decode_png_rgba8(&raw).map_err(|e| png_at(path, e))?;
            Some((w, h, rgba))
        }
        None => None,
    };

    let art_for = |variant: Variant| -> Art {
        match &custom {
            Some((w, h, rgba)) => cover_fit(rgba, *w, *h, variant),
            None => generated_label(
                &manifest.cart.title,
                &manifest.cart.author,
                &cart.palette,
                variant,
            ),
        }
    };

    let (png, variant) = match options.variant {
        Some(variant) => (pack(&bytes, variant, &art_for(variant))?, variant),
        None => pack_auto(&bytes, art_for)?,
    };

    Ok(BuildOutput {
        payload_used: rico16_core::limits::BLOB_HEADER_LEN + bytes.len(),
        capacity: variant.capacity(),
        cart,
        png,
        variant,
    })
}

/// SPEC §12.4 steps 1–6 — assemble the binary cart without packing it.
pub fn assemble_project(root: &Path) -> Result<BinaryCart, CartError> {
    let manifest = read_manifest(root)?;
    assemble_from(root, &manifest)
}

fn read_manifest(root: &Path) -> Result<CartToml, CartError> {
    let path = root.join("cart.toml");
    let text = fs::read_to_string(&path).map_err(|e| CartError::io(&path, e))?;
    CartToml::parse_str(&text)
}

fn assemble_from(root: &Path, manifest: &CartToml) -> Result<BinaryCart, CartError> {
    let palette = load_palette(root, manifest)?;
    let sheet = load_sheet(root, manifest, &palette)?;
    let map = load_map(root, manifest)?;

    let entry = root.join(manifest.entry());
    let code = fs::read_to_string(&entry).map_err(|e| CartError::io(&entry, e))?;
    let source_root = root.join("src");
    let modules = resolve_imports(&code, &source_root)?;

    Ok(BinaryCart {
        code,
        modules,
        metadata: metadata_from_manifest(manifest),
        sheet,
        map,
        palette,
    })
}

/// SPEC §12.3 — resolve the cart's palette, or the console default.
fn load_palette(root: &Path, manifest: &CartToml) -> Result<[u16; 256], CartError> {
    let Some(rel) = &manifest.palette else {
        return Ok(DEFAULT_PALETTE);
    };
    let path = root.join(rel);
    let bytes = fs::read(&path).map_err(|e| CartError::io(&path, e))?;
    let (w, h, rgba) = decode_png_rgba8(&bytes).map_err(|e| png_at(&path, e))?;
    if !((w == 256 && h == 1) || (w == 16 && h == 16)) {
        return Err(CartError::Asset(format!(
            "{rel}: palette png must be 256x1 or 16x16, found {w}x{h}"
        )));
    }
    let mut palette = [0u16; 256];
    for (i, word) in palette.iter_mut().enumerate() {
        let at = i * 4;
        *word =
            quantize5(rgba[at]) | (quantize5(rgba[at + 1]) << 5) | (quantize5(rgba[at + 2]) << 10);
    }
    Ok(palette)
}

/// SPEC §12.1, §10.2.2 — place sprite PNGs into the 256×256 sheet.
fn load_sheet(
    root: &Path,
    manifest: &CartToml,
    palette: &[u16; 256],
) -> Result<Vec<u8>, CartError> {
    let mut sheet = vec![0u8; SHEET_LEN];
    let mut occupied = [false; 256];

    let mut lookup: HashMap<(u8, u8, u8), u8> = HashMap::new();
    for (index, word) in palette.iter().enumerate() {
        let key = (
            expand5(word & 0x1F),
            expand5((word >> 5) & 0x1F),
            expand5((word >> 10) & 0x1F),
        );
        lookup.entry(key).or_insert(index as u8);
    }

    for placement in &manifest.sprites {
        let path = root.join(&placement.file);
        let bytes = fs::read(&path).map_err(|e| CartError::io(&path, e))?;
        let (w, h, rgba) = decode_png_rgba8(&bytes).map_err(|e| png_at(&path, e))?;

        if w == 0 || h == 0 || w % TILE_SIZE as u32 != 0 || h % TILE_SIZE as u32 != 0 {
            return Err(CartError::Asset(format!(
                "{}: sprite png must be a multiple of 16x16, found {w}x{h}",
                placement.file
            )));
        }

        let cols = w / TILE_SIZE as u32;
        let rows = h / TILE_SIZE as u32;
        let tx = placement.x as u32;
        let ty = placement.y as u32;
        if tx + cols > 16 || ty + rows > 16 {
            return Err(CartError::Asset(format!(
                "{}: placement at tile ({}, {}) with {cols}x{rows} tiles leaves the sheet",
                placement.file, placement.x, placement.y
            )));
        }
        for row in 0..rows {
            for col in 0..cols {
                let tile = ((ty + row) * 16 + (tx + col)) as usize;
                if occupied[tile] {
                    return Err(CartError::Asset(format!(
                        "{}: placement overlaps an earlier sprite at tile ({}, {})",
                        placement.file,
                        tx + col,
                        ty + row
                    )));
                }
                occupied[tile] = true;
            }
        }

        for py in 0..h {
            for px in 0..w {
                let at = ((py * w + px) * 4) as usize;
                let (r, g, b, a) = (rgba[at], rgba[at + 1], rgba[at + 2], rgba[at + 3]);
                let index = match a {
                    0 => 0,
                    255 => *lookup.get(&(r, g, b)).ok_or_else(|| {
                        CartError::Asset(format!(
                            "{}: colour #{r:02X}{g:02X}{b:02X} is not in the palette",
                            placement.file
                        ))
                    })?,
                    _ => {
                        return Err(CartError::Asset(format!(
                            "{}: partial alpha {a} is not allowed",
                            placement.file
                        )));
                    }
                };
                let sx = tx as usize * TILE_SIZE + px as usize;
                let sy = ty as usize * TILE_SIZE + py as usize;
                sheet[sy * 256 + sx] = index;
            }
        }
    }
    Ok(sheet)
}

/// SPEC §12.2, §10.2.3 — decode the map PNG into tile ids.
fn load_map(root: &Path, manifest: &CartToml) -> Result<Vec<u8>, CartError> {
    let Some(rel) = &manifest.map else {
        return Ok(vec![0u8; MAP_LEN]);
    };
    let path = root.join(rel);
    let bytes = fs::read(&path).map_err(|e| CartError::io(&path, e))?;
    let (w, h, ids) = map_tile_ids(&bytes).map_err(|e| png_at(&path, e))?;
    if w != 128 || h != 128 {
        return Err(CartError::Asset(format!(
            "{rel}: map png must be 128x128, found {w}x{h}"
        )));
    }
    Ok(ids)
}

/// SPEC §11.2 — resolve the import graph from the entry point, relative to the
/// source root, and embed every reachable module.
pub fn resolve_imports(entry_source: &str, source_root: &Path) -> Result<Vec<Module>, CartError> {
    let mut modules: BTreeMap<String, String> = BTreeMap::new();
    let mut visiting: Vec<String> = Vec::new();
    for raw in import_paths(entry_source)? {
        if is_reserved(&raw) {
            continue;
        }
        let key = normalize_module_path(&raw)?;
        collect_module(&key, source_root, &mut modules, &mut visiting)?;
    }
    Ok(modules
        .into_iter()
        .map(|(path, source)| Module { path, source })
        .collect())
}

fn collect_module(
    path: &str,
    source_root: &Path,
    modules: &mut BTreeMap<String, String>,
    visiting: &mut Vec<String>,
) -> Result<(), CartError> {
    if modules.contains_key(path) {
        return Ok(());
    }
    if visiting.iter().any(|p| p == path) {
        return Err(CartError::Asset(format!("import cycle at `{path}`")));
    }
    visiting.push(path.to_string());

    let file = source_root.join(format!("{path}.rhai"));
    let source = fs::read_to_string(&file).map_err(|e| CartError::io(&file, e))?;
    for raw in import_paths(&source)? {
        if is_reserved(&raw) {
            continue;
        }
        let child = normalize_module_path(&raw)?;
        collect_module(&child, source_root, modules, visiting)?;
    }

    visiting.pop();
    modules.insert(path.to_string(), source);
    Ok(())
}

/// SPEC §11.2 — is `name` a reserved standard-module name?
pub fn is_reserved(name: &str) -> bool {
    RESERVED_MODULES.contains(&name)
}

/// SPEC §11.2, §13.1 — normalize an import path to its module-table key.
pub fn normalize_module_path(raw: &str) -> Result<String, CartError> {
    if raw.contains('\\') {
        return Err(CartError::Asset(format!(
            "import path `{raw}` must use forward slashes"
        )));
    }
    let raw = raw.strip_prefix("src/").unwrap_or(raw);
    let key = raw.strip_suffix(".rhai").unwrap_or(raw);
    if key.is_empty() {
        return Err(CartError::Asset("import path is empty".into()));
    }
    if key.starts_with('/') {
        return Err(CartError::Asset(format!(
            "import path `{raw}` must be relative"
        )));
    }
    if key
        .split('/')
        .any(|part| part == ".." || part == "." || part.is_empty())
    {
        return Err(CartError::Asset(format!(
            "import path `{raw}` must not contain `.` or `..`"
        )));
    }
    Ok(key.to_string())
}

/// SPEC §11.1, §11.2 — every `import "path"` literal in `source`, in order.
///
/// This is a light scanner: it skips line and block comments and string
/// literals, and rejects an `import` whose path is not a string literal.
pub fn import_paths(source: &str) -> Result<Vec<String>, CartError> {
    let chars: Vec<char> = source.chars().collect();
    let n = chars.len();
    let mut i = 0;
    let mut out = Vec::new();

    while i < n {
        let c = chars[i];
        if c == '/' && i + 1 < n && chars[i + 1] == '/' {
            i += 2;
            while i < n && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && i + 1 < n && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < n && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(n);
        } else if c == '"' || c == '`' {
            let (_, next) = read_string(&chars, i);
            i = next;
        } else if is_ident_start(c) {
            let start = i;
            while i < n && is_ident_char(chars[i]) {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            if word == "import" {
                while i < n && chars[i].is_whitespace() {
                    i += 1;
                }
                if i < n && chars[i] == '"' {
                    let (literal, next) = read_string(&chars, i);
                    i = next;
                    out.push(literal);
                } else {
                    return Err(CartError::Asset(
                        "import path must be a string literal".into(),
                    ));
                }
            }
        } else {
            i += 1;
        }
    }
    Ok(out)
}

fn read_string(chars: &[char], start: usize) -> (String, usize) {
    let quote = chars[start];
    let mut out = String::new();
    let mut i = start + 1;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && i + 1 < chars.len() {
            out.push(chars[i + 1]);
            i += 2;
            continue;
        }
        if c == quote {
            return (out, i + 1);
        }
        out.push(c);
        i += 1;
    }
    (out, i)
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn png_at(path: &Path, error: CartError) -> CartError {
    match error {
        CartError::Png { message, .. } => CartError::Png {
            path: path.to_path_buf(),
            message,
        },
        other => other,
    }
}
