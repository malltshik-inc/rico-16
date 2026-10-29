//! SPEC §15.4 — reconstruct a canonical project from a cart PNG.
//!
//! The result is the canonical compiled form: one full sheet, the map, the
//! palette and the extracted sources. It is *not* the original art layout or
//! cover art, and rebuilding it carries the same payload (SPEC §13.6).

use rico16_core::limits::{MAP_SIZE, SHEET_SIZE};
use rico16_core::palette::expand5;

use crate::binary::BinaryCart;
use crate::error::CartError;
use crate::manifest::{CartMeta, CartToml, SpritePlacement};
use crate::png::{self, encode_gray_png, encode_indexed_png, encode_png};

/// One file of a reconstructed project, with a path relative to the root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectFile {
    /// Path relative to the project root, forward slashes.
    pub path: String,
    /// File contents.
    pub bytes: Vec<u8>,
}

/// A reconstructed project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectFiles {
    /// Every file, in a deterministic order.
    pub files: Vec<ProjectFile>,
}

impl ProjectFiles {
    /// The file at `path`, if present.
    pub fn get(&self, path: &str) -> Option<&ProjectFile> {
        self.files.iter().find(|f| f.path == path)
    }
}

/// SPEC §15.4 — read a cart PNG and reconstruct a canonical project.
pub fn unpack_project(png_bytes: &[u8]) -> Result<ProjectFiles, CartError> {
    let (cart, _) = png::unpack(png_bytes)?;
    project_from_cart(&cart)
}

/// SPEC §15.4 — reconstruct a canonical project from a decoded cart.
pub fn project_from_cart(cart: &BinaryCart) -> Result<ProjectFiles, CartError> {
    let metadata: toml::Value = toml::from_str(&cart.metadata)
        .map_err(|e| CartError::Format(format!("metadata is not valid TOML: {e}")))?;
    let field = |key: &str| {
        metadata
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    let entry = {
        let entry = field("entry");
        if entry.is_empty() {
            "src/main.rhai".to_string()
        } else {
            entry
        }
    };

    let manifest = CartToml {
        format: crate::manifest::FORMAT_V1,
        cart: CartMeta {
            title: field("title"),
            author: field("author"),
            version: field("version"),
            entry: Some(entry.clone()),
            output: None,
            extra: Default::default(),
        },
        sprites: vec![SpritePlacement {
            file: "assets/sprites.png".to_string(),
            x: 0,
            y: 0,
        }],
        map: Some("assets/map.png".to_string()),
        palette: Some("assets/palette.png".to_string()),
        extra_tables: Default::default(),
        extra_values: Default::default(),
    };

    let palette_rgb8 = palette_rgb8(&cart.palette);

    let mut files = Vec::new();
    files.push(text("cart.toml", manifest.to_canonical_string()));
    files.push(text(&entry, cart.code.clone()));
    for module in &cart.modules {
        files.push(text(
            &format!("src/{}.rhai", module.path),
            module.source.clone(),
        ));
    }

    // The canonical full sheet, as an indexed PNG over the cart's palette.
    files.push(ProjectFile {
        path: "assets/sprites.png".to_string(),
        bytes: encode_indexed_png(
            SHEET_SIZE as u32,
            SHEET_SIZE as u32,
            &palette_rgb8,
            &cart.sheet,
        )?,
    });

    // The map, as grayscale where the sample is the tile id.
    files.push(ProjectFile {
        path: "assets/map.png".to_string(),
        bytes: encode_gray_png(MAP_SIZE as u32, MAP_SIZE as u32, &cart.map)?,
    });

    // The palette, as 256×1 opaque swatches.
    let mut palette_rgba = Vec::with_capacity(256 * 4);
    for i in 0..256 {
        palette_rgba.extend_from_slice(&[
            palette_rgb8[i * 3],
            palette_rgb8[i * 3 + 1],
            palette_rgb8[i * 3 + 2],
            0xFF,
        ]);
    }
    files.push(ProjectFile {
        path: "assets/palette.png".to_string(),
        bytes: encode_png(256, 1, &palette_rgba)?,
    });

    Ok(ProjectFiles { files })
}

fn text(path: &str, contents: String) -> ProjectFile {
    ProjectFile {
        path: path.to_string(),
        bytes: contents.into_bytes(),
    }
}

fn palette_rgb8(palette: &[u16; 256]) -> Vec<u8> {
    let mut out = Vec::with_capacity(256 * 3);
    for word in palette {
        out.push(expand5(word & 0x1F));
        out.push(expand5((word >> 5) & 0x1F));
        out.push(expand5((word >> 10) & 0x1F));
    }
    out
}
