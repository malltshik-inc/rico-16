//! The player of the rico-16 fantasy console: window, present path, input
//! mapping, main loop and frame pacing (SPEC §14).
//!
//! The window and input live here; nothing here defines behaviour the spec does
//! not describe (AGENTS.md).

#![forbid(unsafe_code)]

mod app;
pub mod error;
pub mod input;
pub mod menu;
pub mod pacing;
pub mod present;

use std::path::{Path, PathBuf};

pub use error::PlayerError;

use rico16_cart::{BinaryCart, BuildOptions};
use rico16_script::CartAssets;

/// SPEC §14.1 — what the player was asked to run.
#[derive(Debug, Clone)]
pub enum CartSource {
    /// A project directory containing `cart.toml`: built in memory, then run.
    Project(PathBuf),
    /// A packed PNG (v1): loaded directly.
    Png(PathBuf),
}

impl CartSource {
    /// A source for a path: a directory is a project, anything else a PNG.
    pub fn from_path(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        if path.is_dir() {
            CartSource::Project(path)
        } else {
            CartSource::Png(path)
        }
    }

    /// SPEC §14.1 — build or load the cart into script assets.
    pub fn load(&self) -> Result<CartAssets, PlayerError> {
        match self {
            CartSource::Project(root) => {
                let manifest_path = root.join("cart.toml");
                let text =
                    std::fs::read_to_string(&manifest_path).map_err(|source| PlayerError::Io {
                        path: manifest_path.clone(),
                        source,
                    })?;
                let manifest = rico16_cart::CartToml::parse_str(&text)?;
                let name = file_name_of(manifest.entry());
                let output = rico16_cart::build_project(root, &BuildOptions::default())?;
                Ok(assets_from(&output.cart, name))
            }
            CartSource::Png(path) => {
                let bytes = std::fs::read(path).map_err(|source| PlayerError::Io {
                    path: path.clone(),
                    source,
                })?;
                let (cart, _) = rico16_cart::png::unpack(&bytes)?;
                Ok(assets_from(&cart, file_name_of(&path.to_string_lossy())))
            }
        }
    }

    /// The name used in error reports and the window title.
    pub fn title(&self) -> String {
        let path = match self {
            CartSource::Project(root) => root,
            CartSource::Png(path) => path,
        };
        // `file_name` is `None` for `.` and `..`, so fall back to the resolved
        // directory name: `rico16 run` from inside a project should still say
        // which project it is.
        path.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .or_else(|| {
                std::fs::canonicalize(path)
                    .ok()
                    .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            })
            .unwrap_or_else(|| "cart".to_string())
    }
}

/// SPEC §14.1 — run a cart to completion.
pub fn run(source: CartSource) -> Result<(), PlayerError> {
    app::run(source)
}

fn assets_from(cart: &BinaryCart, name: String) -> CartAssets {
    CartAssets {
        name,
        code: cart.code.clone(),
        modules: cart
            .modules
            .iter()
            .map(|m| (m.path.clone(), m.source.clone()))
            .collect(),
        sheet: cart.sheet.clone(),
        map: cart.map.clone(),
        palette: cart.palette,
    }
}

fn file_name_of(path: &str) -> String {
    path.rsplit(['/', '\\'])
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("cart.rhai")
        .to_string()
}
