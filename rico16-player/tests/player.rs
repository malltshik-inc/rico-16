//! SPEC §14 — the pure parts of the player: pacing, key mapping and the menu.

use std::time::Duration;

use rico16_core::input::Button;
use rico16_core::limits::{FRAMEBUFFER_LEN, SCREEN_HEIGHT, SCREEN_WIDTH};
use rico16_player::input::{Reserved, button_for_key, reserved_for_key};
use rico16_player::menu::{self, ITEMS, MenuAction, SystemMenu};
use rico16_player::pacing::{FramePacer, frame_interval};
use winit::keyboard::KeyCode;

/// SPEC §5.1, §14.2 — the frame interval is exactly 1/60 s.
#[test]
fn interval_is_one_sixtieth() {
    assert_eq!(frame_interval(), Duration::from_secs_f64(1.0 / 60.0));
}

/// SPEC §14.2 — a logical frame may not start sooner than 1/60 s after the last.
#[test]
fn pacer_gates_frames() {
    let mut pacer = FramePacer::new();
    assert!(pacer.ready(Duration::ZERO), "the first frame is immediate");
    assert_eq!(pacer.deadline(), Duration::ZERO);
    pacer.advance(Duration::ZERO);

    assert!(!pacer.ready(Duration::from_millis(10)));
    assert!(!pacer.ready(Duration::from_millis(16)));
    assert!(pacer.ready(Duration::from_millis(17)));
    assert_eq!(pacer.deadline(), frame_interval());

    pacer.advance(Duration::from_millis(17));
    assert!(!pacer.ready(Duration::from_millis(30)));
    assert!(pacer.ready(Duration::from_millis(34)));
}

/// SPEC §14.3 — the eight buttons map by physical key position.
#[test]
fn key_map() {
    assert_eq!(button_for_key(KeyCode::ArrowLeft), Some(Button::Left));
    assert_eq!(button_for_key(KeyCode::ArrowRight), Some(Button::Right));
    assert_eq!(button_for_key(KeyCode::ArrowUp), Some(Button::Up));
    assert_eq!(button_for_key(KeyCode::ArrowDown), Some(Button::Down));
    assert_eq!(button_for_key(KeyCode::KeyZ), Some(Button::Z));
    assert_eq!(button_for_key(KeyCode::KeyX), Some(Button::X));
    assert_eq!(button_for_key(KeyCode::KeyC), Some(Button::C));
    assert_eq!(button_for_key(KeyCode::KeyV), Some(Button::V));
    assert_eq!(button_for_key(KeyCode::KeyA), None);
}

/// SPEC §14.4 — reserved keys are never visible to a cart.
#[test]
fn reserved_keys() {
    assert_eq!(
        reserved_for_key(KeyCode::Escape, false),
        Some(Reserved::Menu)
    );
    assert_eq!(
        reserved_for_key(KeyCode::F11, false),
        Some(Reserved::Fullscreen)
    );
    assert_eq!(reserved_for_key(KeyCode::KeyR, true), Some(Reserved::Reset));
    assert_eq!(
        reserved_for_key(KeyCode::KeyR, false),
        None,
        "R alone is not reserved"
    );
    assert_eq!(reserved_for_key(KeyCode::KeyZ, true), None);
}

/// SPEC §14.5 — menu navigation wraps and maps to actions.
#[test]
fn menu_navigation() {
    let mut menu = SystemMenu::new();
    assert_eq!(menu.action(), MenuAction::Resume);
    menu.next();
    assert_eq!(menu.action(), MenuAction::Restart);
    menu.next();
    assert_eq!(menu.action(), MenuAction::Exit);
    menu.next();
    assert_eq!(menu.action(), MenuAction::Resume, "wraps forward");
    menu.prev();
    assert_eq!(menu.action(), MenuAction::Exit, "wraps backward");
    assert_eq!(ITEMS.len(), 3);
}

/// SPEC §14.5 — the menu renders into a separate overlay, not the framebuffer.
#[test]
fn menu_renders_into_overlay() {
    let mut overlay = vec![0u8; FRAMEBUFFER_LEN * 4];
    SystemMenu::new().render(&mut overlay);
    assert_eq!(overlay.len(), FRAMEBUFFER_LEN * 4);

    // A dim wash covers the whole screen.
    assert!(overlay[3] > 0, "the overlay dims the paused cart");
    // The panel centre is opaque.
    let centre = (SCREEN_HEIGHT / 2 * SCREEN_WIDTH + SCREEN_WIDTH / 2) * 4;
    assert_eq!(overlay[centre + 3], 255);
}

/// SPEC §6.5 — the menu's text is not mirrored (bit 0 is the leftmost pixel).
#[test]
fn menu_text_is_not_mirrored() {
    let mut overlay = vec![0u8; FRAMEBUFFER_LEN * 4];
    SystemMenu::new().render(&mut overlay);

    let x0 = (SCREEN_WIDTH as i64 - 160) / 2;
    let y0 = (SCREEN_HEIGHT as i64 - 88) / 2;
    let at = |x: i64, y: i64| {
        let i = (y as usize * SCREEN_WIDTH + x as usize) * 4;
        [overlay[i], overlay[i + 1], overlay[i + 2], overlay[i + 3]]
    };

    // 'P' row 0 is 0x3F: the top bar covers the left six columns at scale 2.
    assert_eq!(
        at(x0 + 12, y0 + 12),
        [255, 255, 255, 255],
        "ink on the left"
    );
    assert_eq!(
        at(x0 + 12 + 12, y0 + 12),
        [24, 24, 32, 255],
        "the top bar does not extend to the right"
    );
}

/// SPEC §14.5 — compositing an overlay leaves transparent pixels alone.
#[test]
fn overlay_composite() {
    let mut frame = vec![10u8, 20, 30, 255];
    let overlay = vec![255u8, 0, 0, 255];
    menu::composite(&mut frame, &overlay);
    assert_eq!(frame, [255, 0, 0, 255], "opaque overlay replaces");

    let mut frame = vec![10u8, 20, 30, 255];
    let overlay = vec![255u8, 0, 0, 0];
    menu::composite(&mut frame, &overlay);
    assert_eq!(
        frame,
        [10, 20, 30, 255],
        "transparent overlay leaves the frame"
    );

    let mut frame = vec![0u8, 0, 0, 255];
    let overlay = vec![255u8, 255, 255, 128];
    menu::composite(&mut frame, &overlay);
    assert_eq!(frame[0], 128, "half-alpha blends");
}

/// SPEC §14.1 — a directory is a project, a file is a PNG.
#[test]
fn source_classification() {
    let dir = std::env::temp_dir().join(format!("rico16-player-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    assert!(matches!(
        rico16_player::CartSource::from_path(&dir),
        rico16_player::CartSource::Project(_)
    ));

    let file = dir.join("cart.png");
    std::fs::write(&file, b"x").unwrap();
    assert!(matches!(
        rico16_player::CartSource::from_path(&file),
        rico16_player::CartSource::Png(_)
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

/// SPEC §14.1 — a project directory loads into script assets.
#[test]
fn project_loads_into_assets() {
    let root = std::env::temp_dir().join(format!("rico16-player-load-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("cart.toml"),
        "format = 1\n\n[cart]\ntitle = \"T\"\n",
    )
    .unwrap();
    std::fs::write(root.join("src/main.rhai"), "fn on_init() {}\n").unwrap();

    let source = rico16_player::CartSource::Project(root.clone());
    let assets = source.load().expect("load");
    assert_eq!(assets.name, "main.rhai");
    assert_eq!(assets.code, "fn on_init() {}\n");
    assert_eq!(source.title(), root.file_name().unwrap().to_string_lossy());
    let _ = std::fs::remove_dir_all(&root);
}

/// SPEC §14.1 — a path with no file name (`.`, `..`) still names the project.
#[test]
fn project_without_a_file_name_names_itself() {
    let root = std::env::temp_dir().join(format!("rico16-player-name-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("a/b")).unwrap();

    // `.../a/b/..` has no file name of its own; it resolves to `.../a`.
    let source = rico16_player::CartSource::Project(root.join("a/b/.."));
    assert_eq!(source.title(), "a");

    let _ = std::fs::remove_dir_all(&root);
}

/// AGENTS.md — every example cart must run at HEAD.
#[test]
fn example_carts_run() {
    let examples = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples");
    let mut found = 0;
    for entry in std::fs::read_dir(&examples).expect("examples directory") {
        let dir = entry.unwrap().path();
        if !dir.join("cart.toml").is_file() {
            continue;
        }
        found += 1;
        let source = rico16_player::CartSource::Project(dir.clone());
        let assets = source
            .load()
            .unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        let mut runtime = rico16_script::Runtime::new(assets)
            .unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        runtime
            .init()
            .unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        // Enough frames to reach steady state and to catch errors that only
        // appear after a few updates. Raycasting is slow in a debug build, so
        // this is deliberately not 60.
        for frame in 0..30 {
            runtime
                .step(0)
                .unwrap_or_else(|e| panic!("{} frame {frame}: {e}", dir.display()));
        }
        let mut rgba = vec![0u8; 320 * 240 * 4];
        runtime.present_into(&mut rgba);
        assert!(
            rgba.iter().any(|&b| b != 0),
            "{} drew nothing",
            dir.display()
        );
    }
    assert!(
        found >= 2,
        "expected at least two example carts, found {found}"
    );
}

/// The fighter example: a fighter's sprite follows the way it is facing.
///
/// This was wrong once. The flip was keyed to *which* fighter it was — fighter 1
/// was always mirrored — so a fighter that walked past its opponent kept facing
/// the old way while its hitboxes had already turned around.
#[test]
fn fighter_sprite_flips_with_facing() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/fighter");

    let facing_right = capture_red_fighter(&dir, 0);
    let facing_left = capture_red_fighter(&dir, 250);

    assert!(!facing_right.is_empty(), "the red fighter was not drawn");
    assert_eq!(
        facing_right.len(),
        facing_left.len(),
        "both captures settle to the same pose"
    );

    let mirrored: Vec<Vec<bool>> = facing_right
        .iter()
        .map(|row| row.iter().rev().copied().collect())
        .collect();
    assert_ne!(
        mirrored, facing_right,
        "the idle pose is asymmetric, so this test is able to fail"
    );
    assert_eq!(
        mirrored, facing_left,
        "the sprite did not flip when the fighter turned around"
    );
}

/// Run the fighter cart with Right held for `hold_right` frames, let it settle,
/// and return the red fighter's pixels cropped to their bounding box.
///
/// Only y >= 125 is scanned: the crowd along the horizon also uses the red ramp.
fn capture_red_fighter(dir: &std::path::Path, hold_right: u32) -> Vec<Vec<bool>> {
    let assets = rico16_player::CartSource::Project(dir.to_path_buf())
        .load()
        .expect("load the fighter cart");
    let mut runtime = rico16_script::Runtime::new(assets).expect("runtime");
    runtime.init().expect("init");

    for _ in 0..hold_right {
        runtime.step(0b10).expect("step holding Right");
    }
    for _ in 0..40 {
        runtime.step(0).expect("settle");
    }

    let fb = runtime.memory().framebuffer.borrow();
    let (mut x0, mut x1, mut y0, mut y1) = (i64::MAX, -1i64, i64::MAX, -1i64);
    for y in 125..240 {
        for x in 0..320 {
            if (16..=31).contains(&fb.get(x, y).unwrap_or(0)) {
                x0 = x0.min(x);
                x1 = x1.max(x);
                y0 = y0.min(y);
                y1 = y1.max(y);
            }
        }
    }
    if x1 < 0 {
        return Vec::new();
    }
    (y0..=y1)
        .map(|y| {
            (x0..=x1)
                .map(|x| (16..=31).contains(&fb.get(x, y).unwrap_or(0)))
                .collect()
        })
        .collect()
}
