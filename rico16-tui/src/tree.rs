//! SPEC §16.2 — the project file tree.

use std::path::{Path, PathBuf};

/// One row of the project tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Absolute path.
    pub path: PathBuf,
    /// Display name.
    pub name: String,
    /// Indentation depth.
    pub depth: usize,
    /// Whether this row is a directory.
    pub is_dir: bool,
}

/// SPEC §16.2 — the project tree: `cart.toml`, then `src/` and `assets/`.
///
/// The order is deterministic (directories in a fixed order, files sorted by
/// path) so a test never depends on directory iteration order.
pub fn project_files(root: &Path) -> Vec<Entry> {
    let mut out = Vec::new();

    let manifest = root.join("cart.toml");
    if manifest.is_file() {
        out.push(Entry {
            path: manifest,
            name: "cart.toml".to_string(),
            depth: 0,
            is_dir: false,
        });
    }

    for directory in ["src", "assets"] {
        let dir = root.join(directory);
        if !dir.is_dir() {
            continue;
        }
        out.push(Entry {
            path: dir.clone(),
            name: directory.to_string(),
            depth: 0,
            is_dir: true,
        });
        let mut children = Vec::new();
        collect(&dir, 1, &mut children);
        children.sort_by(|a, b| a.path.cmp(&b.path));
        out.extend(children);
    }

    out
}

fn collect(dir: &Path, depth: usize, out: &mut Vec<Entry>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = read
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| !n.starts_with('.'))
                .unwrap_or(false)
        })
        .collect();
    entries.sort();

    for path in entries {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let is_dir = path.is_dir();
        out.push(Entry {
            path: path.clone(),
            name,
            depth,
            is_dir,
        });
        if is_dir {
            collect(&path, depth + 1, out);
        }
    }
}
