//! SPEC §11.2 — the import scanner and dependency ordering for embedded modules.

use std::collections::{BTreeMap, BTreeSet};

use crate::error::ScriptError;

/// SPEC §11.2 — module names that resolve to native modules, never to sources.
pub const RESERVED_MODULES: [&str; 8] = [
    "gfx", "sprite", "tilemap", "input", "system", "audio", "rnd", "Button",
];

/// SPEC §11.2 — is `name` a reserved standard-module name?
pub fn is_reserved(name: &str) -> bool {
    RESERVED_MODULES.contains(&name)
}

/// SPEC §11.2, §13.1 — normalize an import path to its module-table key.
pub fn normalize(raw: &str) -> String {
    let raw = raw.strip_prefix("src/").unwrap_or(raw);
    raw.strip_suffix(".rhai").unwrap_or(raw).to_string()
}

/// Order `modules` so every module appears after the modules it imports.
///
/// The cart's module table is already acyclic and normalized (SPEC §13.4); this
/// is a defensive ordering for building Rhai modules bottom-up.
pub fn topological_order(modules: &[(String, String)]) -> Result<Vec<String>, ScriptError> {
    let sources: BTreeMap<&str, &str> = modules
        .iter()
        .map(|(path, source)| (path.as_str(), source.as_str()))
        .collect();

    let mut order = Vec::with_capacity(modules.len());
    let mut done: BTreeSet<String> = BTreeSet::new();
    let mut visiting: Vec<String> = Vec::new();

    for (path, _) in modules {
        visit(path, &sources, &mut order, &mut done, &mut visiting)?;
    }
    Ok(order)
}

fn visit(
    path: &str,
    sources: &BTreeMap<&str, &str>,
    order: &mut Vec<String>,
    done: &mut BTreeSet<String>,
    visiting: &mut Vec<String>,
) -> Result<(), ScriptError> {
    if done.contains(path) {
        return Ok(());
    }
    if visiting.iter().any(|p| p == path) {
        return Err(ScriptError::load(
            "<cart>",
            format!("import cycle at `{path}`"),
        ));
    }
    let Some(source) = sources.get(path) else {
        return Err(ScriptError::load(
            "<cart>",
            format!("imported module `{path}` is not in the cart"),
        ));
    };
    visiting.push(path.to_string());
    for raw in import_paths(source)? {
        if is_reserved(&raw) {
            continue;
        }
        let child = normalize(&raw);
        visit(&child, sources, order, done, visiting)?;
    }
    visiting.pop();
    done.insert(path.to_string());
    order.push(path.to_string());
    Ok(())
}

/// SPEC §11.1 — every `import "path"` literal in `source`, in order.
///
/// Skips line and block comments and string literals, and rejects an `import`
/// whose path is not a string literal (SPEC §11.2).
pub fn import_paths(source: &str) -> Result<Vec<String>, ScriptError> {
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
        } else if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            while i < n && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
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
                    return Err(ScriptError::load(
                        "<cart>",
                        "import path must be a string literal",
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
