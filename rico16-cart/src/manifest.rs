//! SPEC §10.2 — `cart.toml`, the project manifest.
//!
//! Read permissively, write canonically (AGENTS.md): unknown `[cart]` keys and
//! unknown top-level tables survive a round-trip, CRLF/BOM are tolerated, and
//! the canonical writer is deterministic.

use std::collections::BTreeMap;

use toml::Value;

use crate::error::CartError;

/// SPEC §10.2 — the only `format` value v1 accepts.
pub const FORMAT_V1: i64 = 1;

/// SPEC §10.2.1 — the default entry point.
pub const DEFAULT_ENTRY: &str = "src/main.rhai";

/// SPEC §10.2.2 — a sprite sheet PNG placed at a tile coordinate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpritePlacement {
    /// Path to the PNG, relative to the project root.
    pub file: String,
    /// Tile column, 0–15.
    pub x: u8,
    /// Tile row, 0–15.
    pub y: u8,
}

/// SPEC §10.2.1 — the `[cart]` section.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CartMeta {
    /// Cart title, UTF-8 (required).
    pub title: String,
    /// Author name.
    pub author: String,
    /// Semantic version string.
    pub version: String,
    /// Entry point, relative to the project root.
    pub entry: Option<String>,
    /// Output PNG path, relative to the project root.
    pub output: Option<String>,
    /// Unknown `[cart]` keys, preserved in order.
    pub extra: BTreeMap<String, Value>,
}

/// SPEC §10.2 — a parsed `cart.toml`.
#[derive(Debug, Clone, PartialEq)]
pub struct CartToml {
    /// Manifest format version; always `1` for v1.
    pub format: i64,
    /// The `[cart]` section.
    pub cart: CartMeta,
    /// `[[sprite]]` placements, in file order.
    pub sprites: Vec<SpritePlacement>,
    /// `[map]` file path, if present.
    pub map: Option<String>,
    /// `[palette]` file path, if present.
    pub palette: Option<String>,
    /// Unknown top-level tables, preserved.
    pub extra_tables: BTreeMap<String, Value>,
    /// Unknown top-level non-table values, preserved.
    pub extra_values: BTreeMap<String, Value>,
}

impl CartToml {
    /// SPEC §10.2 — parse a manifest, permissively.
    pub fn parse_str(text: &str) -> Result<Self, CartError> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let value: Value = toml::from_str(text).map_err(|e| CartError::Manifest(e.to_string()))?;
        Self::from_value(value)
    }

    fn from_value(value: Value) -> Result<Self, CartError> {
        let Value::Table(mut root) = value else {
            return Err(CartError::Manifest("root must be a table".into()));
        };

        let format = match root.remove("format") {
            Some(Value::Integer(n)) => n,
            Some(_) => return Err(CartError::Manifest("`format` must be an integer".into())),
            None => return Err(CartError::Manifest("missing required key `format`".into())),
        };
        if format != FORMAT_V1 {
            return Err(CartError::Manifest(format!(
                "unsupported format {format}, expected {FORMAT_V1}"
            )));
        }

        let cart = match root.remove("cart") {
            Some(Value::Table(t)) => Self::cart_from_table(t)?,
            Some(_) => return Err(CartError::Manifest("`[cart]` must be a table".into())),
            None => {
                return Err(CartError::Manifest(
                    "missing required `[cart]` table".into(),
                ));
            }
        };

        let sprites = match root.remove("sprite") {
            Some(Value::Array(items)) => Self::sprites_from_array(items)?,
            Some(_) => return Err(CartError::Manifest("`[[sprite]]` must be an array".into())),
            None => Vec::new(),
        };

        let map = Self::file_table(root.remove("map"), "map")?;
        let palette = Self::file_table(root.remove("palette"), "palette")?;

        let mut extra_tables = BTreeMap::new();
        let mut extra_values = BTreeMap::new();
        for (key, value) in root {
            if matches!(value, Value::Table(_) | Value::Array(_)) {
                extra_tables.insert(key, value);
            } else {
                extra_values.insert(key, value);
            }
        }

        Ok(Self {
            format,
            cart,
            sprites,
            map,
            palette,
            extra_tables,
            extra_values,
        })
    }

    fn cart_from_table(mut table: toml::Table) -> Result<CartMeta, CartError> {
        let title = match table.remove("title") {
            Some(Value::String(s)) => s,
            Some(_) => return Err(CartError::Manifest("`cart.title` must be a string".into())),
            None => return Err(CartError::Manifest("missing required `cart.title`".into())),
        };
        let author = take_string(&mut table, "author")?.unwrap_or_default();
        let version = take_string(&mut table, "version")?.unwrap_or_default();
        let entry = take_string(&mut table, "entry")?;
        let output = take_string(&mut table, "output")?;
        Ok(CartMeta {
            title,
            author,
            version,
            entry,
            output,
            extra: table.into_iter().collect(),
        })
    }

    fn sprites_from_array(items: Vec<Value>) -> Result<Vec<SpritePlacement>, CartError> {
        let mut sprites = Vec::with_capacity(items.len());
        for item in items {
            let Value::Table(mut table) = item else {
                return Err(CartError::Manifest(
                    "`[[sprite]]` entry must be a table".into(),
                ));
            };
            let file = match table.remove("file") {
                Some(Value::String(s)) => s,
                _ => return Err(CartError::Manifest("`sprite.file` is required".into())),
            };
            let x = tile_coord(&mut table, "x")?;
            let y = tile_coord(&mut table, "y")?;
            sprites.push(SpritePlacement { file, x, y });
        }
        Ok(sprites)
    }

    fn file_table(value: Option<Value>, name: &str) -> Result<Option<String>, CartError> {
        match value {
            Some(Value::Table(mut table)) => match table.remove("file") {
                Some(Value::String(s)) => Ok(Some(s)),
                _ => Err(CartError::Manifest(format!("`{name}.file` is required"))),
            },
            Some(_) => Err(CartError::Manifest(format!("`[{name}]` must be a table"))),
            None => Ok(None),
        }
    }

    /// SPEC §10.2.1 — the entry point path, defaulting to `src/main.rhai`.
    pub fn entry(&self) -> &str {
        self.cart.entry.as_deref().unwrap_or(DEFAULT_ENTRY)
    }

    /// SPEC §10.2.1 — the output path, defaulting to `build/<title-slug>.png`.
    pub fn output(&self) -> String {
        self.cart
            .output
            .clone()
            .unwrap_or_else(|| format!("build/{}.png", self.title_slug()))
    }

    /// SPEC §10.2.1 — the title slug used for the default output name.
    pub fn title_slug(&self) -> String {
        slugify(&self.cart.title)
    }

    /// SPEC §10.2 — write the manifest canonically.
    pub fn to_canonical_string(&self) -> String {
        let mut root = toml::Table::new();
        root.insert("format".into(), Value::Integer(self.format));

        // Plain top-level values must precede any table header, or the output
        // would attach them to the previous table.
        for (key, value) in &self.extra_values {
            root.insert(key.clone(), value.clone());
        }

        let mut cart = toml::Table::new();
        cart.insert("title".into(), Value::String(self.cart.title.clone()));
        cart.insert("author".into(), Value::String(self.cart.author.clone()));
        cart.insert("version".into(), Value::String(self.cart.version.clone()));
        cart.insert("entry".into(), Value::String(self.entry().to_string()));
        cart.insert("output".into(), Value::String(self.output()));
        for (key, value) in &self.cart.extra {
            cart.insert(key.clone(), value.clone());
        }
        root.insert("cart".into(), Value::Table(cart));

        if !self.sprites.is_empty() {
            let entries = self
                .sprites
                .iter()
                .map(|sprite| {
                    let mut table = toml::Table::new();
                    table.insert("file".into(), Value::String(sprite.file.clone()));
                    table.insert("x".into(), Value::Integer(sprite.x as i64));
                    table.insert("y".into(), Value::Integer(sprite.y as i64));
                    Value::Table(table)
                })
                .collect();
            root.insert("sprite".into(), Value::Array(entries));
        }

        if let Some(map) = &self.map {
            let mut table = toml::Table::new();
            table.insert("file".into(), Value::String(map.clone()));
            root.insert("map".into(), Value::Table(table));
        }
        if let Some(palette) = &self.palette {
            let mut table = toml::Table::new();
            table.insert("file".into(), Value::String(palette.clone()));
            root.insert("palette".into(), Value::Table(table));
        }

        for (key, value) in &self.extra_tables {
            root.insert(key.clone(), value.clone());
        }

        toml::to_string(&Value::Table(root)).expect("canonical manifest always serializes")
    }
}

/// SPEC §10.2.1 — the `<title-slug>` recipe.
pub fn slugify(title: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in title.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches('-');
    let truncated: String = trimmed.chars().take(64).collect();
    let truncated = truncated.trim_end_matches('-');
    if truncated.is_empty() {
        "cart".to_string()
    } else {
        truncated.to_string()
    }
}

fn take_string(table: &mut toml::Table, key: &str) -> Result<Option<String>, CartError> {
    match table.remove(key) {
        Some(Value::String(s)) => Ok(Some(s)),
        Some(_) => Err(CartError::Manifest(format!(
            "`cart.{key}` must be a string"
        ))),
        None => Ok(None),
    }
}

fn tile_coord(table: &mut toml::Table, key: &str) -> Result<u8, CartError> {
    match table.remove(key) {
        Some(Value::Integer(n)) if (0..=15).contains(&n) => Ok(n as u8),
        Some(Value::Integer(n)) => Err(CartError::Manifest(format!(
            "`sprite.{key}` is {n}, must be 0–15"
        ))),
        _ => Err(CartError::Manifest(format!("`sprite.{key}` is required"))),
    }
}
