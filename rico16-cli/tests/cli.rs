//! SPEC §15 — the `rico16` command line, driven through the built binary.

use std::path::{Path, PathBuf};
use std::process::Command;

struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("rico16-cli-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn rico16() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rico16"))
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// SPEC §15.1 — `new` scaffolds the documented layout.
#[test]
fn new_scaffolds_a_project() {
    let scratch = Scratch::new("new");
    let status = rico16()
        .args(["new", scratch.path("hello").to_str().unwrap()])
        .status()
        .unwrap();
    assert!(status.success());

    assert!(scratch.path("hello/cart.toml").is_file());
    assert!(scratch.path("hello/src/main.rhai").is_file());
    assert!(scratch.path("hello/assets").is_dir());
    assert!(scratch.path("hello/build").is_dir());
    assert_eq!(read(&scratch.path("hello/.gitignore")), "build/\n");

    let manifest = read(&scratch.path("hello/cart.toml"));
    assert!(manifest.contains("format = 1"));
    assert!(manifest.contains("title = \"hello\""));
    assert!(manifest.contains("entry = \"src/main.rhai\""));
    assert!(manifest.contains("output = \"build/hello.png\""));

    let source = read(&scratch.path("hello/src/main.rhai"));
    assert!(source.contains("fn on_init"));
    assert!(source.contains("fn on_update"));
    assert!(source.contains("fn on_draw"));
}

/// SPEC §4.11, §15.1 — `--palette rgb332` bakes the preset into the cart.
#[test]
fn new_with_rgb332_preset() {
    let scratch = Scratch::new("rgb332");
    let status = rico16()
        .args([
            "new",
            scratch.path("hexed").to_str().unwrap(),
            "--palette",
            "rgb332",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    assert!(scratch.path("hexed/assets/palette.png").is_file());
    let manifest = read(&scratch.path("hexed/cart.toml"));
    assert!(manifest.contains("[palette]"), "{manifest}");
    assert!(
        manifest.contains("file = \"assets/palette.png\""),
        "{manifest}"
    );
}

/// SPEC §15.1 — an unknown preset is refused.
#[test]
fn new_with_unknown_preset_fails() {
    let scratch = Scratch::new("bad-preset");
    let output = rico16()
        .args([
            "new",
            scratch.path("x").to_str().unwrap(),
            "--palette",
            "cga",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown palette preset"));
}

/// SPEC §15.2 — `build` writes a packed PNG and reports used/capacity.
#[test]
fn build_writes_a_png() {
    let scratch = Scratch::new("build");
    assert!(
        rico16()
            .args(["new", scratch.path("hello").to_str().unwrap()])
            .status()
            .unwrap()
            .success()
    );
    let output = rico16()
        .args(["build", scratch.path("hello").to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("payload bytes used"), "{stdout}");

    let png = std::fs::read(scratch.path("hello/build/hello.png")).unwrap();
    assert_eq!(&png[0..8], b"\x89PNG\r\n\x1a\n");
    let (_, variant) = rico16_cart::png::unpack(&png).unwrap();
    assert_eq!(variant, rico16_cart::Variant::DEFAULT);
}

/// SPEC §15.2 — `--variant overflow` forces the larger PNG.
#[test]
fn build_with_forced_variant() {
    let scratch = Scratch::new("overflow");
    assert!(
        rico16()
            .args(["new", scratch.path("big").to_str().unwrap()])
            .status()
            .unwrap()
            .success()
    );
    assert!(
        rico16()
            .args([
                "build",
                scratch.path("big").to_str().unwrap(),
                "--variant",
                "overflow",
            ])
            .status()
            .unwrap()
            .success()
    );
    let png = std::fs::read(scratch.path("big/build/big.png")).unwrap();
    let (_, variant) = rico16_cart::png::unpack(&png).unwrap();
    assert_eq!(variant, rico16_cart::Variant::OVERFLOW);
}

/// SPEC §15.2 — an unknown variant is refused.
#[test]
fn build_with_unknown_variant_fails() {
    let scratch = Scratch::new("bad-variant");
    assert!(
        rico16()
            .args(["new", scratch.path("x").to_str().unwrap()])
            .status()
            .unwrap()
            .success()
    );
    let output = rico16()
        .args([
            "build",
            scratch.path("x").to_str().unwrap(),
            "--variant",
            "huge",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown variant"));
}

/// SPEC §15.4 — `unpack` reconstructs the canonical project.
#[test]
fn unpack_reconstructs_a_project() {
    let scratch = Scratch::new("unpack");
    assert!(
        rico16()
            .args(["new", scratch.path("hello").to_str().unwrap()])
            .status()
            .unwrap()
            .success()
    );
    assert!(
        rico16()
            .args(["build", scratch.path("hello").to_str().unwrap()])
            .status()
            .unwrap()
            .success()
    );
    let out = scratch.path("out");
    let status = rico16()
        .args([
            "unpack",
            scratch.path("hello/build/hello.png").to_str().unwrap(),
            out.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(status.success());

    for file in [
        "cart.toml",
        "src/main.rhai",
        "assets/sprites.png",
        "assets/map.png",
        "assets/palette.png",
    ] {
        assert!(out.join(file).is_file(), "missing {file}");
    }

    // Rebuilding the unpacked project produces the same payload.
    let rebuilt = rico16()
        .args(["build", out.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        rebuilt.status.success(),
        "{}",
        String::from_utf8_lossy(&rebuilt.stderr)
    );
    let original =
        rico16_cart::png::unpack(&std::fs::read(scratch.path("hello/build/hello.png")).unwrap())
            .unwrap()
            .0;
    let again = rico16_cart::png::unpack(&std::fs::read(out.join("build/hello.png")).unwrap())
        .unwrap()
        .0;
    assert_eq!(original.encode().unwrap(), again.encode().unwrap());
}

/// SPEC §15.2 — building a directory with no cart.toml fails clearly.
#[test]
fn build_without_manifest_fails() {
    let scratch = Scratch::new("no-manifest");
    let output = rico16()
        .args(["build", scratch.root.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cart.toml"));
}

/// SPEC §15.2, §15.3 — `build` and `run` default to the current directory.
#[test]
fn build_defaults_to_the_current_directory() {
    let scratch = Scratch::new("default-dir");
    assert!(
        rico16()
            .args(["new", scratch.path("hello").to_str().unwrap()])
            .status()
            .unwrap()
            .success()
    );

    let output = rico16()
        .arg("build")
        .current_dir(scratch.path("hello"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(scratch.path("hello/build/hello.png").is_file());
    assert!(String::from_utf8_lossy(&output.stdout).contains("payload bytes used"));
}

/// SPEC §15.2 — defaulting to `.` still fails clearly when there is no project.
#[test]
fn build_defaults_fail_clearly_without_a_manifest() {
    let scratch = Scratch::new("default-dir-empty");
    let output = rico16()
        .arg("build")
        .current_dir(&scratch.root)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cart.toml"));
}

/// SPEC §15.2 — an explicit argument still wins over the default.
#[test]
fn build_still_accepts_an_explicit_project() {
    let scratch = Scratch::new("explicit-dir");
    assert!(
        rico16()
            .args(["new", scratch.path("hello").to_str().unwrap()])
            .status()
            .unwrap()
            .success()
    );
    let output = rico16()
        .args(["build", scratch.path("hello").to_str().unwrap()])
        .current_dir(&scratch.root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(scratch.path("hello/build/hello.png").is_file());
}

/// AGENTS.md — every example cart must build at HEAD.
#[test]
fn example_carts_build() {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples");
    let scratch = Scratch::new("examples");
    let mut found = 0;
    for entry in std::fs::read_dir(&examples).expect("examples directory") {
        let dir = entry.unwrap().path();
        if !dir.join("cart.toml").is_file() {
            continue;
        }
        found += 1;

        // Build a copy: building writes `build/<slug>.png` next to the
        // manifest, and a test must not leave artifacts in the repository.
        let copy = scratch.root.join(dir.file_name().unwrap());
        copy_tree(&dir, &copy);

        let output = rico16()
            .args(["build", copy.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{} failed to build: {}",
            dir.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert!(
        found >= 2,
        "expected at least two example carts, found {found}"
    );
}

/// Copy a directory tree, skipping any `build/` output.
fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name() == "build" {
            continue;
        }
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}
