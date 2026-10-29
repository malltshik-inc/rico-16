//! SPEC §10.2 — `cart.toml` parse/serialize round-trips.

use rico16_cart::CartToml;
use rico16_cart::manifest::slugify;

const SAMPLE: &str = r#"format = 1

[cart]
title = "Snake"
author = "me"
version = "0.1.0"
entry = "src/main.rhai"
output = "build/snake.png"

[[sprite]]
file = "assets/hero.png"
x = 0
y = 0

[[sprite]]
file = "assets/tiles.png"
x = 0
y = 8

[map]
file = "assets/map.png"

[palette]
file = "assets/palette.png"
"#;

/// SPEC §10.2 — a canonical manifest parses and re-serializes byte-identically.
#[test]
fn canonical_round_trip() {
    let parsed = CartToml::parse_str(SAMPLE).expect("parse");
    let written = parsed.to_canonical_string();
    assert_eq!(written, SAMPLE, "canonical writer must be stable");
    let reparsed = CartToml::parse_str(&written).expect("reparse");
    assert_eq!(parsed, reparsed);
}

/// SPEC §10.2.1 — entry and output default when omitted.
#[test]
fn defaults() {
    let text = "format = 1\n\n[cart]\ntitle = \"My Game\"\n";
    let cart = CartToml::parse_str(text).expect("parse");
    assert_eq!(cart.entry(), "src/main.rhai");
    assert_eq!(cart.output(), "build/my-game.png");
    assert!(cart.sprites.is_empty());
    assert_eq!(cart.map, None);
    assert_eq!(cart.palette, None);
}

/// SPEC §10.2.1 — the `<title-slug>` recipe.
#[test]
fn slugs() {
    assert_eq!(slugify("Snake"), "snake");
    assert_eq!(slugify("My  Great -- Game!"), "my-great-game");
    assert_eq!(slugify("--Leading and trailing--"), "leading-and-trailing");
    assert_eq!(slugify(""), "cart");
    assert_eq!(slugify("!!!"), "cart");
    assert_eq!(slugify("Ünïcödé"), "n-c-d");
    assert_eq!(slugify(&"a".repeat(100)).len(), 64);
}

/// AGENTS.md — unknown `[cart]` keys and unknown top-level tables survive.
#[test]
fn unknown_keys_survive() {
    let text = r#"format = 1

[cart]
title = "T"
author = "a"
version = "1"
custom = "keep me"

[tooling]
watcher = true
"#;
    let cart = CartToml::parse_str(text).expect("parse");
    let written = cart.to_canonical_string();
    assert!(written.contains("custom = \"keep me\""), "{written}");
    assert!(written.contains("[tooling]"), "{written}");
    let reparsed = CartToml::parse_str(&written).expect("reparse");
    assert_eq!(
        cart.cart.extra.get("custom").and_then(|v| v.as_str()),
        Some("keep me")
    );
    assert!(reparsed.extra_tables.contains_key("tooling"));
}

/// SPEC §10.2 — a missing or wrong `format` is rejected.
#[test]
fn format_is_required() {
    assert!(CartToml::parse_str("[cart]\ntitle = \"x\"\n").is_err());
    assert!(CartToml::parse_str("format = 2\n\n[cart]\ntitle = \"x\"\n").is_err());
    assert!(CartToml::parse_str("format = \"1\"\n\n[cart]\ntitle = \"x\"\n").is_err());
}

/// SPEC §10.2.1 — a missing title is rejected.
#[test]
fn title_is_required() {
    assert!(CartToml::parse_str("format = 1\n\n[cart]\nauthor = \"x\"\n").is_err());
}

/// SPEC §10.2.2 — sprite tile coordinates are 0–15.
#[test]
fn sprite_coords_are_bounded() {
    let text =
        "format = 1\n\n[cart]\ntitle = \"x\"\n\n[[sprite]]\nfile = \"a.png\"\nx = 16\ny = 0\n";
    assert!(CartToml::parse_str(text).is_err());
}

/// AGENTS.md — a BOM is tolerated on read.
#[test]
fn bom_is_tolerated() {
    let text = "\u{feff}format = 1\n\n[cart]\ntitle = \"x\"\n";
    assert!(CartToml::parse_str(text).is_ok());
}
