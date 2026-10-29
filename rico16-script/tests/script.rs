//! SPEC §5, §6, §9, §11, §14 — the script runtime end to end.

use rico16_core::limits::{MAP_LEN, PALETTE_LEN, SHEET_LEN};
use rico16_core::palette::DEFAULT_PALETTE;
use rico16_script::{Action, CartAssets, Runtime};

fn assets(code: &str, modules: &[(&str, &str)]) -> CartAssets {
    CartAssets {
        name: "test.rhai".to_string(),
        code: code.to_string(),
        modules: modules
            .iter()
            .map(|(p, s)| (p.to_string(), s.to_string()))
            .collect(),
        sheet: vec![0u8; SHEET_LEN],
        map: vec![0u8; MAP_LEN],
        palette: DEFAULT_PALETTE,
    }
}

fn runtime(code: &str) -> Runtime {
    Runtime::new(assets(code, &[])).expect("runtime")
}

fn run(code: &str, frames: u64) -> Runtime {
    let mut rt = runtime(code);
    rt.init().expect("init");
    for _ in 0..frames {
        rt.step(0).expect("step");
    }
    rt
}

/// SPEC §9.7, §20.9 — `state` persists across frames and mutates in place.
#[test]
fn state_persists_across_frames() {
    let rt = run(
        "fn on_init() { state.n = 0; }\nfn on_update(dt) { state.n += 1; }\nfn on_draw() { print(state.n); }",
        10,
    );
    let out = rt.take_output();
    assert_eq!(out.len(), 10);
    assert_eq!(out.last().unwrap(), "10");
}

/// SPEC §9.7, §11.4, §20.9 — a module function mutates the same `state`.
#[test]
fn state_is_shared_with_modules() {
    let mut rt = Runtime::new(assets(
        "import \"scenes/play\" as play;\nfn on_init() { state.n = 0; }\nfn on_update(dt) { play::bump(); }\nfn on_draw() { print(state.n); }",
        &[("scenes/play", "fn bump() { state.n += 1; }")],
    ))
    .unwrap();
    rt.init().unwrap();
    for _ in 0..3 {
        rt.step(0).unwrap();
    }
    assert_eq!(rt.take_output().last().unwrap(), "3");
}

/// SPEC §9.7, §20.9 — `state` can be reassigned and change type.
#[test]
fn state_can_be_reassigned() {
    let rt = run(
        "fn on_init() { state = 5; }\nfn on_update(dt) { state = state + 1; }\nfn on_draw() { print(state); }",
        3,
    );
    assert_eq!(rt.take_output(), ["6", "7", "8"]);
}

/// SPEC §9.1 — `state` is reserved.
#[test]
fn state_is_reserved() {
    let mut rt = runtime("let state = 1;\nfn on_init() {}");
    let err = rt.init().unwrap_err();
    assert!(err.message.contains("reserved"), "{err}");
}

/// SPEC §9.2 — entry points are optional.
#[test]
fn entry_points_are_optional() {
    let rt = run("fn on_draw() { print(\"d\"); }", 2);
    assert_eq!(rt.take_output(), ["d", "d"]);

    let rt = run("", 2);
    assert!(rt.take_output().is_empty());
}

/// SPEC §9.2 — a wrong parameter count is a load error, not a silent no-op.
#[test]
fn wrong_entry_point_arity_is_rejected() {
    let err = Runtime::new(assets("fn on_update() {}", &[]))
        .err()
        .expect("should fail");
    assert!(err.message.contains("on_update"), "{err}");
}

/// SPEC §9.4.1 — errors carry the cart name, line and column.
#[test]
fn error_has_position() {
    let mut rt = runtime("fn on_update(dt) {\n    let x = 1;\n    x.nope();\n}");
    rt.init().unwrap();
    let err = rt.step(0).unwrap_err();
    assert_eq!(err.cart, "test.rhai");
    assert_eq!(err.line, 3, "{err}");
    assert!(err.to_string().starts_with("test.rhai:3:"), "{err}");
}

/// SPEC §9.8 — dynamic evaluation is unavailable.
#[test]
fn eval_is_disabled() {
    assert!(Runtime::new(assets("fn on_init() { eval(\"1\"); }", &[])).is_err());
}

/// SPEC §9.8 — the wall clock is unavailable.
#[test]
fn time_functions_are_unavailable() {
    let mut rt = runtime("fn on_update(dt) { timestamp(); }");
    rt.init().unwrap();
    assert!(rt.step(0).is_err());
}

/// SPEC §7, §9.8 — the operation budget aborts a runaway callback.
#[test]
fn operation_limit() {
    let mut rt = runtime("fn on_update(dt) { let i = 0; while i < 100000000 { i += 1; } }");
    rt.init().unwrap();
    let err = rt.step(0).unwrap_err();
    assert!(err.message.to_lowercase().contains("operations"), "{err}");
}

/// SPEC §7 — recursion is bounded.
#[test]
fn call_depth_limit() {
    let mut rt = runtime("fn deep(n) { deep(n + 1) }\nfn on_update(dt) { deep(0); }");
    rt.init().unwrap();
    assert!(rt.step(0).is_err());
}

/// SPEC §9.4.4 — module errors carry the module path.
#[test]
fn module_error_carries_path() {
    let mut rt = Runtime::new(assets(
        "import \"lib/broken\" as broken;\nfn on_update(dt) { broken::boom(); }",
        &[("lib/broken", "fn boom() { nope(); }")],
    ))
    .unwrap();
    rt.init().unwrap();
    let err = rt.step(0).unwrap_err();
    assert!(err.message.contains("lib/broken"), "{err}");
}

/// SPEC §11.4 — transitive imports work.
#[test]
fn transitive_imports() {
    let mut rt = Runtime::new(assets(
        "import \"scenes/play\" as play;\nfn on_draw() { print(play::value()); }",
        &[
            (
                "scenes/play",
                "import \"lib/grid\" as grid;\nfn value() { grid::cell() }",
            ),
            ("lib/grid", "fn cell() { 42 }"),
        ],
    ))
    .unwrap();
    rt.init().unwrap();
    rt.step(0).unwrap();
    assert_eq!(rt.take_output(), ["42"]);
}

/// SPEC §9.6 — standard modules are ambient without import.
#[test]
fn ambient_standard_modules() {
    let mut rt = runtime("fn on_draw() { gfx::clear(3); }");
    rt.init().unwrap();
    rt.step(0).unwrap();
    assert_eq!(rt.memory().framebuffer.borrow().get(0, 0), Some(3));
}

/// SPEC §9.6 — standard modules are also importable.
#[test]
fn importable_standard_modules() {
    let mut rt = runtime("import \"gfx\" as g;\nfn on_draw() { g::clear(4); }");
    rt.init().unwrap();
    rt.step(0).unwrap();
    assert_eq!(rt.memory().framebuffer.borrow().get(0, 0), Some(4));
}

/// SPEC §5.4 — `system::frames()` is 0 on the first frame and counts up.
#[test]
fn frames_counter() {
    let rt = run("fn on_update(dt) { print(system::frames()); }", 3);
    assert_eq!(rt.take_output(), ["0", "1", "2"]);
}

/// SPEC §5.1 — `dt` is exactly 1/60.
#[test]
fn dt_is_exact() {
    let rt = run("fn on_update(dt) { print(dt); }", 1);
    assert_eq!(rt.take_output()[0], (1.0f64 / 60.0).to_string());
}

/// SPEC §20.1 — the RNG is reachable from a cart and matches the vector.
#[test]
fn rng_via_script() {
    let rt = run(
        "fn on_init() { system::seed(0); }\nfn on_draw() { print(rnd::int(1, 6)); }",
        8,
    );
    assert_eq!(rt.take_output(), ["4", "5", "1", "3", "5", "6", "3", "4"]);
}

/// SPEC §5.5 — `rnd::shuffle` is Fisher–Yates from the end.
#[test]
fn rnd_shuffle_via_script() {
    let rt = run(
        "fn on_init() { system::seed(0); }\nfn on_draw() { print(rnd::shuffle([0, 1, 2, 3])); }",
        1,
    );
    assert_eq!(rt.take_output()[0], "[1, 0, 3, 2]");
}

/// SPEC §5.5 — `rnd::choice` of an empty array is `()` and draws nothing.
#[test]
fn rnd_choice_empty() {
    let rt = run(
        "fn on_init() { system::seed(0); }\nfn on_draw() { print(rnd::choice([]) == ()); print(rnd::int(1, 6)); }",
        1,
    );
    assert_eq!(rt.take_output(), ["true", "4"]);
}

/// SPEC §6.2 — float arguments are floored.
#[test]
fn float_arguments_are_floored() {
    let mut rt = runtime("fn on_draw() { gfx::pixel(1.9, -0.1, 5); }");
    rt.init().unwrap();
    rt.step(0).unwrap();
    let fb = rt.memory().framebuffer.borrow();
    assert_eq!(fb.get(1, -1), None, "y floors to -1, off screen");
    // x floors to 1, y floors to -1 -> nothing drawn.
    assert_eq!(fb.get(1, 0), Some(0));
}

/// SPEC §6.3 — out-of-range colours are script errors, never wrapped.
#[test]
fn colour_range_error() {
    let mut rt = runtime("fn on_draw() { gfx::clear(256); }");
    rt.init().unwrap();
    assert!(rt.step(0).is_err());
}

/// SPEC §6.9 — sprite pixel coordinates are 0–15.
#[test]
fn sprite_pixel_range_error() {
    let mut rt = runtime("fn on_draw() { sprite::get_pixel(0, 16, 0); }");
    rt.init().unwrap();
    assert!(rt.step(0).is_err());
}

/// SPEC §5.5 — `rnd::int` with an empty range is an error.
#[test]
fn rnd_empty_range_error() {
    let mut rt = runtime("fn on_draw() { rnd::int(5, 4); }");
    rt.init().unwrap();
    assert!(rt.step(0).is_err());
}

/// SPEC §4.2 — `sprite::draw` skips index 0.
#[test]
fn sprite_draw_skips_index_zero() {
    let mut assets = assets("fn on_draw() { sprite::draw(0, 0, 0); }", &[]);
    // Tile 0 pixel (0,0) is index 7; the rest are 0.
    assets.sheet[0] = 7;
    let mut rt = Runtime::new(assets).unwrap();
    rt.init().unwrap();
    rt.step(0).unwrap();
    let fb = rt.memory().framebuffer.borrow();
    assert_eq!(fb.get(0, 0), Some(7));
    assert_eq!(fb.get(1, 0), Some(0), "index 0 is not written");
}

/// SPEC §4.4, §4.5 — palette mutation, `palette_reset`, and reset restoration.
#[test]
fn palette_mutation_and_reset() {
    let mut rt = runtime("fn on_init() { gfx::palette(1, 255, 0, 0); }");
    rt.init().unwrap();
    assert_eq!(rt.memory().palette.borrow().word(1), 31, "red only");
    assert_eq!(
        rt.memory().palette.borrow().word(1) >> 5,
        0,
        "green cleared"
    );

    // `reset` restores the palette resolved at pack time.
    let mut rt = runtime("fn on_init() {}");
    rt.init().unwrap();
    rt.memory().palette.borrow_mut().set_word(1, 0x1234);
    rt.reset().unwrap();
    assert_eq!(rt.memory().palette.borrow().word(1), DEFAULT_PALETTE[1]);
}

/// SPEC §9.3 — `gfx::palette_reset` restores the cart's palette.
#[test]
fn palette_reset_function() {
    let mut rt = runtime("fn on_init() { gfx::palette(1, 255, 0, 0); gfx::palette_reset(); }");
    rt.init().unwrap();
    assert_eq!(rt.memory().palette.borrow().word(1), DEFAULT_PALETTE[1]);
}

/// SPEC §14.9 — `system::quit(message)`.
#[test]
fn system_quit() {
    let mut rt = runtime("fn on_update(dt) { system::quit(\"bye\"); }");
    rt.init().unwrap();
    rt.step(0).unwrap();
    assert_eq!(rt.take_action(), Action::Quit(Some("bye".to_string())));
}

/// SPEC §14.9 — `system::reset` clears state, frames and the RNG.
#[test]
fn system_reset() {
    let mut rt = runtime(
        "fn on_init() { state.n = 0; }\nfn on_update(dt) { state.n += 1; }\nfn on_draw() { print(system::frames()); print(state.n); }",
    );
    rt.init().unwrap();
    for _ in 0..3 {
        rt.step(0).unwrap();
    }
    rt.reset().unwrap();
    assert_eq!(rt.take_action(), Action::Run);
    rt.take_output();
    rt.step(0).unwrap();
    assert_eq!(rt.take_output(), ["0", "1"], "frames and state were reset");
}

/// SPEC §14.6 — input edges are visible to a cart.
#[test]
fn input_edges() {
    let mut rt = runtime(
        "fn on_update(dt) { if input::pressed(Button::Z) { print(\"down\"); } if input::released(Button::Z) { print(\"up\"); } }",
    );
    rt.init().unwrap();
    rt.step(0x10).unwrap(); // Z bit
    rt.step(0x10).unwrap();
    rt.step(0x00).unwrap();
    assert_eq!(rt.take_output(), ["down", "up"]);
}

/// SPEC §5.6, §9.8 — the same cart produces the same frames.
#[test]
fn determinism() {
    let code = "fn on_init() { system::seed(123); state.x = 160; }\nfn on_update(dt) { state.x += rnd::int(-2, 2); gfx::clear(1); gfx::rect_filled(state.x, 120, 4, 4, 9); }";
    let a = run(code, 60);
    let b = run(code, 60);
    assert_eq!(
        a.memory().framebuffer.borrow().bytes(),
        b.memory().framebuffer.borrow().bytes()
    );
}

/// SPEC §3.6 — `present_into` expands through the live palette.
#[test]
fn present_into() {
    let mut rt = runtime("fn on_draw() { gfx::clear(5); }");
    rt.init().unwrap();
    rt.step(0).unwrap();
    let mut rgba = vec![0u8; 320 * 240 * 4];
    rt.present_into(&mut rgba);
    assert_eq!(&rgba[0..4], &[255, 255, 255, 255], "index 5 is white");
}

/// SPEC §9.3 — `print` and `debug` both reach the host console.
#[test]
fn print_and_debug() {
    let rt = run("fn on_draw() { print(1); debug(2); }", 1);
    let out = rt.take_output();
    assert_eq!(out.len(), 2);
    assert!(out[0].contains('1'));
    assert!(out[1].contains('2'));
}

/// SPEC §7 — a resource of the wrong size is a load error.
#[test]
fn wrong_resource_size() {
    let mut a = assets("", &[]);
    a.sheet = vec![0u8; 10];
    let err = Runtime::new(a).err().expect("should fail");
    assert!(err.message.contains("sprite sheet"), "{err}");
}

/// SPEC §4.1 — the palette resource is the size the console fixes.
#[test]
fn palette_len() {
    assert_eq!(DEFAULT_PALETTE.len(), PALETTE_LEN);
}
