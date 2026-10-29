//! AGENTS.md — `rico16-core` stays pure. It must not touch the filesystem, the
//! clock, threads, the environment or the OS RNG, so it stays `wasm32`-clean.

use std::fs;
use std::path::Path;

const FORBIDDEN: &[&str] = &[
    "std::fs",
    "std::time",
    "std::thread",
    "std::env",
    "SystemTime",
    "Instant",
    "std::process",
    "std::net",
];

#[test]
fn core_is_pure() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut checked = 0;
    for entry in fs::read_dir(&src).expect("read core src") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let text = fs::read_to_string(&path).expect("read source file");
        for token in FORBIDDEN {
            assert!(
                !text.contains(token),
                "{} contains forbidden token `{token}`",
                path.display()
            );
        }
        checked += 1;
    }
    assert!(checked > 0, "no source files were checked");
}
