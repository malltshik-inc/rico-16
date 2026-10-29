//! SPEC §11.2, §12 — the build pipeline end to end.

use std::fs;
use std::path::{Path, PathBuf};

use rico16_cart::CartError;
use rico16_cart::build::{BuildOptions, build_project, import_paths, normalize_module_path};
use rico16_cart::png::{encode_png, unpack};
use rico16_core::limits::{MAP_LEN, SHEET_LEN};

/// A scratch project directory under the OS temp dir, removed on drop.
struct Project {
    root: PathBuf,
}

impl Project {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("rico16-build-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("assets")).unwrap();
        Self { root }
    }

    fn write(&self, rel: &str, contents: &[u8]) -> &Self {
        let path = self.root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
        self
    }

    fn write_text(&self, rel: &str, contents: &str) -> &Self {
        self.write(rel, contents.as_bytes())
    }

    fn write_rgba_png(&self, rel: &str, w: u32, h: u32, rgba: &[u8]) -> &Self {
        self.write(rel, &encode_png(w, h, rgba).unwrap())
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn solid_rgba(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
    rgba.iter()
        .copied()
        .cycle()
        .take((w * h) as usize * 4)
        .collect()
}

const MINIMAL_MANIFEST: &str = "format = 1\n\n[cart]\ntitle = \"Tiny\"\n";

/// SPEC §12.4 — a minimal project with no assets builds and unpacks.
#[test]
fn minimal_project_builds() {
    let project = Project::new("minimal");
    project
        .write_text("cart.toml", MINIMAL_MANIFEST)
        .write_text("src/main.rhai", "fn on_init() {}\n");

    let output = build_project(&project.root, &BuildOptions::default()).unwrap();
    assert_eq!(output.cart.sheet.len(), SHEET_LEN);
    assert_eq!(output.cart.map.len(), MAP_LEN);
    assert_eq!(output.cart.palette, rico16_core::palette::DEFAULT_PALETTE);
    assert!(output.payload_used <= output.capacity);

    let (cart, _) = unpack(&output.png).unwrap();
    assert_eq!(cart.code, "fn on_init() {}\n");
}

/// SPEC §12.1 — a sprite PNG lands in the sheet at its tile coordinate.
#[test]
fn sprite_placement() {
    let project = Project::new("sprite");
    project
        .write_text(
            "cart.toml",
            "format = 1\n\n[cart]\ntitle = \"T\"\n\n[[sprite]]\nfile = \"assets/hero.png\"\nx = 1\ny = 2\n",
        )
        .write_text("src/main.rhai", "")
        // One opaque white pixel; the rest transparent.
        .write_rgba_png("assets/hero.png", 16, 16, &{
            let mut px = solid_rgba(16, 16, [0, 0, 0, 0]);
            px[0..4].copy_from_slice(&[255, 255, 255, 255]);
            px
        });

    let cart = rico16_cart::assemble_project(&project.root).unwrap();
    // Tile (1,2) is tile id 2*16+1 = 33; its top-left pixel is sheet (16, 32).
    assert_eq!(cart.sheet[32 * 256 + 16], 5, "white is palette index 5");
    assert_eq!(cart.sheet[32 * 256 + 17], 0, "transparent becomes index 0");
}

/// SPEC §10.2.2 — an unknown colour is a build error.
#[test]
fn unknown_sprite_colour_is_rejected() {
    let project = Project::new("bad-colour");
    project
        .write_text(
            "cart.toml",
            "format = 1\n\n[cart]\ntitle = \"T\"\n\n[[sprite]]\nfile = \"assets/a.png\"\nx = 0\ny = 0\n",
        )
        .write_text("src/main.rhai", "")
        .write_rgba_png("assets/a.png", 16, 16, &solid_rgba(16, 16, [1, 2, 3, 255]));

    let err = rico16_cart::assemble_project(&project.root)
        .unwrap_err()
        .to_string();
    assert!(err.contains("not in the palette"), "{err}");
}

/// SPEC §10.2.2 — partial alpha is a build error.
#[test]
fn partial_alpha_is_rejected() {
    let project = Project::new("partial-alpha");
    project
        .write_text(
            "cart.toml",
            "format = 1\n\n[cart]\ntitle = \"T\"\n\n[[sprite]]\nfile = \"assets/a.png\"\nx = 0\ny = 0\n",
        )
        .write_text("src/main.rhai", "")
        .write_rgba_png("assets/a.png", 16, 16, &solid_rgba(16, 16, [255, 255, 255, 128]));

    let err = rico16_cart::assemble_project(&project.root)
        .unwrap_err()
        .to_string();
    assert!(err.contains("partial alpha"), "{err}");
}

/// SPEC §10.2.2 — overlapping placements are rejected.
#[test]
fn overlapping_placements_rejected() {
    let project = Project::new("overlap");
    project
        .write_text(
            "cart.toml",
            "format = 1\n\n[cart]\ntitle = \"T\"\n\n[[sprite]]\nfile = \"assets/a.png\"\nx = 0\ny = 0\n\n[[sprite]]\nfile = \"assets/b.png\"\nx = 0\ny = 0\n",
        )
        .write_text("src/main.rhai", "")
        .write_rgba_png("assets/a.png", 16, 16, &solid_rgba(16, 16, [0, 0, 0, 0]))
        .write_rgba_png("assets/b.png", 16, 16, &solid_rgba(16, 16, [0, 0, 0, 0]));

    let err = rico16_cart::assemble_project(&project.root)
        .unwrap_err()
        .to_string();
    assert!(err.contains("overlap"), "{err}");
}

/// SPEC §10.2.2 — a placement that leaves the sheet is rejected.
#[test]
fn out_of_bounds_placement_rejected() {
    let project = Project::new("oob");
    project
        .write_text(
            "cart.toml",
            "format = 1\n\n[cart]\ntitle = \"T\"\n\n[[sprite]]\nfile = \"assets/a.png\"\nx = 15\ny = 0\n",
        )
        .write_text("src/main.rhai", "")
        .write_rgba_png("assets/a.png", 32, 16, &solid_rgba(32, 16, [0, 0, 0, 0]));

    let err = rico16_cart::assemble_project(&project.root)
        .unwrap_err()
        .to_string();
    assert!(err.contains("leaves the sheet"), "{err}");
}

/// SPEC §12.3 — a custom palette PNG resolves into the cart.
#[test]
fn custom_palette() {
    let project = Project::new("palette");
    project
        .write_text(
            "cart.toml",
            "format = 1\n\n[cart]\ntitle = \"T\"\n\n[palette]\nfile = \"assets/palette.png\"\n",
        )
        .write_text("src/main.rhai", "")
        .write_rgba_png("assets/palette.png", 16, 16, &{
            let mut px = vec![0u8; 16 * 16 * 4];
            for (i, p) in px.chunks_exact_mut(4).enumerate() {
                p[0] = (i * 16) as u8;
                p[1] = 0;
                p[2] = 255;
                p[3] = 255;
            }
            px
        });

    let cart = rico16_cart::assemble_project(&project.root).unwrap();
    assert_eq!(cart.palette[0], 31 << 10, "red 0, blue 31");
    // Entry 15: red = 15*16 = 240 -> 29, blue = 31.
    assert_eq!(cart.palette[15], 29 | (31 << 10));
}

/// SPEC §12.2, §10.2.3 — a grayscale map PNG becomes tile ids.
#[test]
fn grayscale_map() {
    let project = Project::new("map");
    project
        .write_text(
            "cart.toml",
            "format = 1\n\n[cart]\ntitle = \"T\"\n\n[map]\nfile = \"assets/map.png\"\n",
        )
        .write_text("src/main.rhai", "")
        .write_rgba_png("assets/map.png", 128, 128, &{
            let mut px = vec![0u8; 128 * 128 * 4];
            for (i, p) in px.chunks_exact_mut(4).enumerate() {
                let v = (i % 256) as u8;
                p[0] = v;
                p[1] = v;
                p[2] = v;
                p[3] = 255;
            }
            px
        });

    let cart = rico16_cart::assemble_project(&project.root).unwrap();
    assert_eq!(cart.map.len(), MAP_LEN);
    // BT.601 on a grey (v,v,v) is v.
    assert_eq!(cart.map[0], 0);
    assert_eq!(cart.map[255], 255);
    assert_eq!(cart.map[256], 0);
}

/// SPEC §11.2 — transitive imports are embedded, sorted, and reserved names are
/// not.
#[test]
fn import_graph() {
    let project = Project::new("imports");
    project
        .write_text("cart.toml", MINIMAL_MANIFEST)
        .write_text(
            "src/main.rhai",
            "import \"scenes/play\" as play;\nimport \"gfx\" as g;\nfn on_init() { play::start(); }\n",
        )
        .write_text("src/scenes/play.rhai", "import \"lib/grid\" as grid;\nfn start() {}\n")
        .write_text("src/lib/grid.rhai", "fn cell() {}\n");

    let cart = rico16_cart::assemble_project(&project.root).unwrap();
    let paths: Vec<&str> = cart.modules.iter().map(|m| m.path.as_str()).collect();
    assert_eq!(
        paths,
        ["lib/grid", "scenes/play"],
        "sorted, reserved excluded"
    );
    assert!(cart.code.contains("import \"gfx\""));
}

/// SPEC §11.2 — a cycle is rejected.
#[test]
fn import_cycle_rejected() {
    let project = Project::new("cycle");
    project
        .write_text("cart.toml", MINIMAL_MANIFEST)
        .write_text("src/main.rhai", "import \"a\" as a;\n")
        .write_text("src/a.rhai", "import \"b\" as b;\n")
        .write_text("src/b.rhai", "import \"a\" as a;\n");

    let err = rico16_cart::assemble_project(&project.root)
        .unwrap_err()
        .to_string();
    assert!(err.contains("cycle"), "{err}");
}

/// SPEC §11.2 — the import scanner skips comments and strings.
#[test]
fn import_scanner_skips_noise() {
    let source = r#"
// import "commented" as c;
/* import "blocked" as b; */
let s = "import \"in a string\" as x;";
import "real/one" as real;
"#;
    assert_eq!(import_paths(source).unwrap(), vec!["real/one"]);
}

/// SPEC §11.2 — non-literal import paths are rejected.
#[test]
fn non_literal_import_rejected() {
    assert!(import_paths("import mod + \"o\" as c;\n").is_err());
}

/// SPEC §11.2, §13.1 — module keys drop `.rhai` and a leading `src/`.
#[test]
fn module_path_normalization() {
    assert_eq!(normalize_module_path("scenes/play").unwrap(), "scenes/play");
    assert_eq!(
        normalize_module_path("scenes/play.rhai").unwrap(),
        "scenes/play"
    );
    assert_eq!(normalize_module_path("src/lib/grid").unwrap(), "lib/grid");
    assert!(normalize_module_path("../escape").is_err());
    assert!(normalize_module_path("/abs").is_err());
    assert!(normalize_module_path("a\\b").is_err());
}

/// SPEC §15.2 — a forced overflow variant produces a 1280×960 PNG.
#[test]
fn forced_overflow_variant() {
    let project = Project::new("overflow");
    project
        .write_text("cart.toml", MINIMAL_MANIFEST)
        .write_text("src/main.rhai", "");

    let options = BuildOptions {
        label: None,
        variant: Some(rico16_cart::Variant::OVERFLOW),
    };
    let output = build_project(&project.root, &options).unwrap();
    assert_eq!(output.variant, rico16_cart::Variant::OVERFLOW);
    assert!(!output.png.is_empty());
    let (_, variant) = unpack(&output.png).unwrap();
    assert_eq!(variant, rico16_cart::Variant::OVERFLOW);
}

/// SPEC §12.4 — a missing entry point is an I/O error naming the path.
#[test]
fn missing_entry_point() {
    let project = Project::new("no-entry");
    project.write_text("cart.toml", MINIMAL_MANIFEST);
    let err = build_project(&project.root, &BuildOptions::default()).unwrap_err();
    match err {
        CartError::Io { path, .. } => assert!(path.ends_with(Path::new("src/main.rhai"))),
        other => panic!("expected an io error, got {other}"),
    }
}
