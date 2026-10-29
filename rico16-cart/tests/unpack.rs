//! SPEC §15.4, §13.6 — `unpack` reconstructs a canonical project whose rebuild
//! carries the same payload.

use std::fs;
use std::path::PathBuf;

use rico16_cart::build::{BuildOptions, assemble_project, build_project};
use rico16_cart::png::{encode_png, unpack};
use rico16_cart::unpack_project;

struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("rico16-unpack-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn write(&self, rel: &str, bytes: &[u8]) {
        let path = self.root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn solid(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
    rgba.iter()
        .copied()
        .cycle()
        .take((w * h) as usize * 4)
        .collect()
}

/// SPEC §15.4 — unpack then rebuild produces the same binary payload.
#[test]
fn unpack_rebuild_round_trip() {
    let project = Scratch::new("project");
    project.write(
        "cart.toml",
        b"format = 1\n\n[cart]\ntitle = \"Round Trip\"\nauthor = \"me\"\nversion = \"1.2.3\"\n",
    );
    project.write(
        "src/main.rhai",
        b"import \"lib/grid\" as grid;\nfn on_init() {}\n",
    );
    project.write("src/lib/grid.rhai", b"fn cell() { 42 }\n");
    let hero_colour = [
        rico16_core::palette::expand5(rico16_core::palette::quantize5(3)),
        rico16_core::palette::expand5(rico16_core::palette::quantize5(5)),
        rico16_core::palette::expand5(rico16_core::palette::quantize5(7)),
        255,
    ];
    project.write(
        "assets/hero.png",
        &encode_png(16, 16, &{
            let mut px = solid(16, 16, [0, 0, 0, 0]);
            px[0..4].copy_from_slice(&hero_colour);
            px
        })
        .unwrap(),
    );
    project.write("cart.toml", b"format = 1\n\n[cart]\ntitle = \"Round Trip\"\nauthor = \"me\"\nversion = \"1.2.3\"\n\n[[sprite]]\nfile = \"assets/hero.png\"\nx = 2\ny = 3\n\n[map]\nfile = \"assets/map.png\"\n\n[palette]\nfile = \"assets/palette.png\"\n");
    project.write(
        "assets/map.png",
        &encode_png(128, 128, &{
            let mut px = solid(128, 128, [0, 0, 0, 255]);
            for (i, p) in px.chunks_exact_mut(4).enumerate() {
                let v = (i % 7) as u8;
                p[0] = v;
                p[1] = v;
                p[2] = v;
            }
            px
        })
        .unwrap(),
    );
    project.write(
        "assets/palette.png",
        &encode_png(16, 16, &{
            let mut px = solid(16, 16, [0, 0, 0, 255]);
            for (i, p) in px.chunks_exact_mut(4).enumerate() {
                p[0] = (i * 3) as u8;
                p[1] = (i * 5) as u8;
                p[2] = (i * 7) as u8;
            }
            px
        })
        .unwrap(),
    );

    let original = assemble_project(&project.root).unwrap();
    let payload = original.encode().unwrap();

    // Pack it, then reconstruct the project and rebuild.
    let built = build_project(&project.root, &BuildOptions::default()).unwrap();
    let (from_png, _) = unpack(&built.png).unwrap();
    assert_eq!(from_png.encode().unwrap(), payload);

    let files = unpack_project(&built.png).unwrap();
    assert!(files.get("cart.toml").is_some());
    assert!(files.get("src/main.rhai").is_some());
    assert!(files.get("src/lib/grid.rhai").is_some());
    assert!(files.get("assets/sprites.png").is_some());
    assert!(files.get("assets/map.png").is_some());
    assert!(files.get("assets/palette.png").is_some());

    let rebuilt = Scratch::new("rebuilt");
    for file in &files.files {
        rebuilt.write(&file.path, &file.bytes);
    }
    let rebuilt_cart = assemble_project(&rebuilt.root).unwrap();
    assert_eq!(
        rebuilt_cart.encode().unwrap(),
        payload,
        "a rebuild of the unpacked project carries the same payload"
    );
}

/// SPEC §15.4 — the written cart.toml is canonical and carries the metadata.
#[test]
fn unpack_writes_canonical_manifest() {
    let project = Scratch::new("manifest");
    project.write("cart.toml", b"format = 1\n\n[cart]\ntitle = \"My Game\"\n");
    project.write("src/main.rhai", b"");
    let built = build_project(&project.root, &BuildOptions::default()).unwrap();

    let files = unpack_project(&built.png).unwrap();
    let cart_toml = String::from_utf8(files.get("cart.toml").unwrap().bytes.clone()).unwrap();
    assert!(cart_toml.contains("title = \"My Game\""), "{cart_toml}");
    assert!(
        cart_toml.contains("entry = \"src/main.rhai\""),
        "{cart_toml}"
    );
    assert!(
        cart_toml.contains("output = \"build/my-game.png\""),
        "{cart_toml}"
    );
    assert!(
        cart_toml.contains("file = \"assets/sprites.png\""),
        "{cart_toml}"
    );
}
