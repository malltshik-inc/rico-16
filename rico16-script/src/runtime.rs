//! The cart runtime: one Rhai engine plus everything a cart sees.
//!
//! Frame order is SPEC §5.2: `on_init` once, then `on_update` and `on_draw`
//! once per frame. Input is sampled once per frame immediately before
//! `on_update` (SPEC §12.4). The framebuffer is never cleared behind the cart's
//! back (§3.4); `system::quit` stops the cart after the current frame (§12.7).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use rhai::{Dynamic, EvalAltResult, ParseError, Scope};
use rico16_core::framebuffer::Framebuffer;
use rico16_core::input::Input;
use rico16_core::limits::{DT, FRAMEBUFFER_LEN, MAP_LEN, PALETTE_LEN, SHEET_LEN};
use rico16_core::map::Tilemap;
use rico16_core::palette::Palette;
use rico16_core::rng::Rng;

use crate::api;
use crate::ScriptError;

/// Everything a loaded cart starts with, built by the host from cart data.
///
/// `rico16-script` deliberately does not depend on the cart crate; the player
/// or CLI constructs this from a parsed `.r16`.
#[derive(Debug, Clone)]
pub struct CartAssets {
    /// Cart file name, used in every error report.
    pub name: String,
    /// Rhai source, already validated to fit the code limit.
    pub code: String,
    /// 65,536 bytes: the 256×256 sprite sheet.
    pub sheet: Vec<u8>,
    /// 16,384 bytes: the 128×128 tilemap.
    pub map: Vec<u8>,
    /// The cart's own palette, or `None` for the console default. SPEC §10.6.
    pub palette: Option<[u16; PALETTE_LEN]>,
}

/// Why the cart asked to stop. SPEC §12.7.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quit {
    /// Message printed to stdout first, if any.
    pub message: Option<String>,
}

/// The mutable memory a cart's scripts can read and write.
///
/// Each resource lives in its own interior cell so that a draw call can hold
/// the framebuffer mutably and the sheet/map immutably at the same time.
/// `cart_palette` is the immutable snapshot `gfx::palette_reset` restores.
pub struct State {
    pub framebuffer: RefCell<Framebuffer>,
    pub palette: RefCell<Palette>,
    pub cart_palette: Palette,
    pub sheet: RefCell<Vec<u8>>,
    pub map: RefCell<Tilemap>,
    pub input: RefCell<Input>,
    pub rng: RefCell<Rng>,
    pub frames: Cell<u64>,
    pub quit: RefCell<Option<Option<String>>>,
}

/// A running cart.
pub struct Runtime {
    engine: rhai::Engine,
    scope: Scope<'static>,
    ast: rhai::AST,
    state: Rc<State>,
    name: String,
}

impl Runtime {
    /// Compile a cart and build the engine around it.
    ///
    /// This fails only when the cart memory is the wrong size or the code does
    /// not parse; the cart itself has not run yet.
    pub fn new(assets: CartAssets) -> Result<Self, ScriptError> {
        if assets.sheet.len() != SHEET_LEN {
            return Err(ScriptError::field(
                &assets.name,
                format!(
                    "sprite sheet is {} bytes, must be exactly {SHEET_LEN}",
                    assets.sheet.len()
                ),
            ));
        }
        if assets.map.len() != MAP_LEN {
            return Err(ScriptError::field(
                &assets.name,
                format!("tilemap is {} bytes, must be exactly {MAP_LEN}", assets.map.len()),
            ));
        }

        let cart_palette = match assets.palette {
            Some(entries) => Palette::from_entries(entries),
            None => Palette::new(),
        };

        let state = Rc::new(State {
            framebuffer: RefCell::new(Framebuffer::new()),
            palette: RefCell::new(cart_palette.clone()),
            cart_palette,
            sheet: RefCell::new(assets.sheet),
            map: RefCell::new(Tilemap::from_bytes(&assets.map)),
            input: RefCell::new(Input::new()),
            rng: RefCell::new(Rng::default()),
            frames: Cell::new(0),
            quit: RefCell::new(None),
        });

        let mut engine = rhai::Engine::new();
        api::register(&mut engine, &state);

        let ast = engine
            .compile(&assets.code)
            .map_err(|parse| ScriptError::from_parse(&assets.name, &parse))?;
        validate_entry_points(&assets.name, &ast)?;

        Ok(Self {
            engine,
            scope: Scope::new(),
            ast,
            state,
            name: assets.name,
        })
    }

    /// Run the cart's top-level statements once, then `on_init`.
    ///
    /// Top-level `let`s and expressions run here, exactly once per cart start,
    /// mirroring how a cart text loads. `on_init` is optional (SPEC §9.2).
    pub fn init(&mut self) -> Result<(), ScriptError> {
        self.engine
            .run_ast_with_scope(&mut self.scope, &self.ast)
            .map_err(|error| self.script_error(&error))?;
        self.call_optional("on_init", &[])?;
        Ok(())
    }

    /// Advance one frame: sample input, `on_update`, `on_draw`.
    ///
    /// After this, [`Runtime::present_into`] reads the finished frame. If the
    /// cart asked to quit mid-frame, the rest of the frame is skipped.
    pub fn step(&mut self, mask: u8) -> Result<(), ScriptError> {
        self.state.input.borrow_mut().update(mask);

        if !self.quit_requested() {
            self.call_optional("on_update", &[Dynamic::from(DT)])?;
        }
        if !self.quit_requested() {
            self.call_optional("on_draw", &[])?;
        }

        let frames = self.state.frames.get();
        self.state.frames.set(frames + 1);
        Ok(())
    }

    /// Expand the finished frame through the live palette. SPEC §3.6.
    ///
    /// `rgba` must be exactly 320×240×4 bytes.
    pub fn present_into(&self, rgba: &mut [u8]) {
        assert_eq!(
            rgba.len(),
            FRAMEBUFFER_LEN * 4,
            "presented frame must be 320x240 RGBA8"
        );
        let framebuffer = self.state.framebuffer.borrow();
        let palette = self.state.palette.borrow();
        framebuffer.expand_into(&palette, rgba);
    }

    /// Did the cart ask to stop? SPEC §12.7.
    pub fn is_quitting(&self) -> bool {
        self.quit_requested()
    }

    /// Forget held buttons and edges without reporting releases. SPEC §12.5.
    pub fn clear_input(&mut self) {
        self.state.input.borrow_mut().clear();
    }

    /// Take the quit request, if any, clearing it.
    pub fn take_quit(&mut self) -> Option<Quit> {
        self.state
            .quit
            .borrow_mut()
            .take()
            .map(|message| Quit { message })
    }

    /// The cartridge's name, as the host set it.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Direct access to the memory behind the cart, for the host.
    pub fn state(&self) -> &State {
        &self.state
    }

    fn quit_requested(&self) -> bool {
        self.state.quit.borrow().is_some()
    }

    /// Call one optional entry point.
    ///
    /// Existence is decided by inspecting the compiled AST instead of by the
    /// error a failed call returns, so an unknown function *inside* a defined
    /// handler is a real script error (§9.4.3) instead of being mistaken for an
    /// absent entry point.
    fn call_optional(&mut self, name: &str, values: &[Dynamic]) -> Result<(), ScriptError> {
        let arity = values.len();
        if !self.handler_present(name, arity) {
            return Ok(());
        }
        let result = match arity {
            0 => self
                .engine
                .call_fn::<Dynamic>(&mut self.scope, &self.ast, name, ()),
            1 => self
                .engine
                .call_fn::<Dynamic>(&mut self.scope, &self.ast, name, (values[0].clone(),)),
            _ => unreachable!("entry points take zero or one argument"),
        };
        result.map(|_| ()).map_err(|error| self.script_error(&error))
    }

    /// Does the compiled cart declare a handler of this name and arity?
    ///
    /// Matched on name plus declared parameter count (`fn on_update(dt)`
    /// declares one parameter). Parameter types are not part of the check:
    /// Rhai script functions cannot declare them, so every parameter is
    /// `Dynamic`.
    fn handler_present(&self, name: &str, arity: usize) -> bool {
        self.ast
            .iter_functions()
            .any(|definition| definition.name == name && definition.params.len() == arity)
    }

    fn script_error(&self, error: &EvalAltResult) -> ScriptError {
        ScriptError::from_rhai(&self.name, error)
    }
}

/// The arity each entry point must declare. SPEC §9.2.
const ENTRY_POINTS: [(&str, usize); 3] = [("on_init", 0), ("on_update", 1), ("on_draw", 0)];

/// Reject a cart whose entry point declares the wrong number of parameters.
///
/// Rhai resolves calls by arity, so `fn on_update()` would never be found by
/// the frame loop. Failing at load time keeps a wrong signature from becoming a
/// silent no-op (SPEC §9.2, §9.4.3). Rhai does not expose the position of a
/// function definition, so the error points at the start of the cart.
fn validate_entry_points(cart: &str, ast: &rhai::AST) -> Result<(), ScriptError> {
    for definition in ast.iter_functions() {
        let Some((name, expected)) = ENTRY_POINTS
            .iter()
            .find(|(name, _)| *name == definition.name)
            .copied()
        else {
            continue;
        };
        let found = definition.params.len();
        if found != expected {
            let wanted = match expected {
                0 => "no parameters",
                1 => "1 parameter (dt)",
                _ => unreachable!("entry points take zero or one parameter"),
            };
            return Err(ScriptError::field(
                cart,
                format!("{name} takes {wanted}, found {found}"),
            ));
        }
    }
    Ok(())
}

impl ScriptError {
    /// Map a Rhai parse error, which already carries a position.
    fn from_parse(cart: &str, error: &ParseError) -> Self {
        let position = error.position();
        Self {
            cart: cart.to_string(),
            line: position.line().unwrap_or(1) as u32,
            col: position.position().map(|col| col as u32).unwrap_or(1),
            message: error.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rico16_core::input::Button;
    use rico16_core::limits::{PALETTE_LEN, SCREEN_WIDTH};

    fn assets(code: &str) -> CartAssets {
        CartAssets {
            name: "test.r16".to_string(),
            code: code.to_string(),
            sheet: vec![0; SHEET_LEN],
            map: vec![0; MAP_LEN],
            palette: None,
        }
    }

    fn run(code: &str, frames: u64, mask: u8) -> Result<Runtime, ScriptError> {
        let mut runtime = Runtime::new(assets(code))?;
        runtime.init()?;
        for _ in 0..frames {
            runtime.step(mask)?;
        }
        Ok(runtime)
    }

    /// The error from running `code`, panicking if the cart ran clean.
    fn expect_error(code: &str) -> ScriptError {
        match run(code, 1, 0) {
            Err(error) => error,
            Ok(_) => panic!("expected a script error for: {code}"),
        }
    }

    fn hash(runtime: &Runtime) -> u64 {
        let framebuffer = runtime.state.framebuffer.borrow();
        let mut hash = 0xcbf2_9ce4_8422_2325;
        for byte in framebuffer.indices() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
        }
        hash
    }

    #[test]
    fn a_cart_with_no_entry_points_still_runs() {
        let mut runtime = run("", 3, 0).unwrap();
        runtime.step(0).unwrap();
        assert_eq!(runtime.state.frames.get(), 4);
    }

    #[test]
    fn top_level_statements_run_once_and_on_init_once() {
        let runtime = run(
            "let inits = 0;\nfn on_init() { inits = inits + 1; }\nfn on_draw() { gfx::clear(3); }",
            4,
            0,
        )
        .unwrap();
        let count: i64 = runtime.scope.get_value("inits").unwrap();
        assert_eq!(count, 1, "on_init ran once");
        assert!(
            runtime
                .state
                .framebuffer
                .borrow()
                .indices()
                .iter()
                .all(|index| *index == 3)
        );
    }

    #[test]
    fn dt_is_exactly_one_sixtieth() {
        let runtime = run(
            "let caught = 0.0;\nfn on_update(dt) { caught = dt; }\nfn on_draw() {}",
            2,
            0,
        )
        .unwrap();
        let caught: f64 = runtime.scope.get_value("caught").unwrap();
        assert_eq!(caught, DT);
        assert_eq!(DT, 0.016_666_666_666_666_666);
    }

    #[test]
    fn frames_and_time_count_completed_frames() {
        let runtime = run(
            "let f = 0;\nlet t = 0.0;\nfn on_update(dt) { f = system::frames(); t = system::time(); }",
            5,
            0,
        )
        .unwrap();
        // During the 5th on_update (frame index 4), frames() == 4.
        let f: i64 = runtime.scope.get_value("f").unwrap();
        let t: f64 = runtime.scope.get_value("t").unwrap();
        assert_eq!(f, 4);
        assert_eq!(t, 4.0 * DT);
    }

    #[test]
    fn a_cart_without_on_draw_presents_unmodified_frames() {
        let mut runtime = run("fn on_update(dt) { gfx::pixel(1, 1, 7); }", 1, 0).unwrap();
        assert_eq!(runtime.state.framebuffer.borrow().get(1, 1), Some(7));
        // A second frame with no on_draw keeps the same memory.
        runtime.step(0).unwrap();
        assert_eq!(runtime.state.framebuffer.borrow().get(1, 1), Some(7));
    }

    #[test]
    fn full_api_surface_is_callable() {
        let code = r#"
fn on_init() {
    gfx::palette(0, 0, 0, 0);
    gfx::palette_reset();
    sprite::set_pixel(0, 0, 0, 200);
    tilemap::set(0, 0, 3);
    system::seed(99);
}
fn on_update(dt) {
    system::time();
    random();
    random_int(1, 6);
    gfx::text_width("hello");
    if input::held(Button::Left) { gfx::pixel(0, 0, 1); }
    if input::pressed(Button::Z) { gfx::pixel(1, 1, 2); }
    if input::released(Button::C) { gfx::pixel(2, 2, 3); }
    audio::play(3);
}
fn on_draw() {
    gfx::clear(1);
    gfx::pixel(3, 3, 255);
    gfx::line(0, 0, 9, 9, 2);
    gfx::rect(10, 10, 5, 5, 3);
    gfx::rect_filled(20, 10, 5, 5, 4);
    gfx::circle(40, 20, 3, 5);
    gfx::circle_filled(50, 20, 3, 6);
    gfx::text("go", 2, 3, 255);
    sprite::draw(0, 5, 5);
    sprite::draw(0, 5, 5, true, false);
    sprite::draw_region(0, 0, 8, 8, 30, 30, false, true);
    let _p = sprite::get_pixel(0, 0, 0);
    tilemap::draw(0, 0, 2, 2, 40, 40);
    let c = tilemap::get(4, 4);
    if c > 0 { gfx::pixel(63, 63, 1); }
}
"#;
        run(code, 2, Button::Left.bit() | Button::Z.bit()).unwrap();
    }

    #[test]
    fn deterministic_carts_hash_identically_twice() {
        let code = r#"
fn on_draw() {
    gfx::clear(0);
    let x = random_int(0, 200);
    let y = random_int(0, 200);
    gfx::rect_filled(x, y, 8, 8, random_int(1, 12));
    gfx::text("rng", x, y, 14);
}
"#;
        let first = run(code, 60, Button::Right.bit()).unwrap();
        let second = run(code, 60, Button::Right.bit()).unwrap();
        assert_eq!(hash(&first), hash(&second));
    }

    #[test]
    fn seeding_changes_the_rng_stream() {
        let default = run(
            "fn on_draw() { gfx::pixel(random_int(0, 319), random_int(0, 239), 1); }",
            5,
            0,
        )
        .unwrap();
        let seeded = run(
            "fn on_init() { system::seed(1234); }\nfn on_draw() { gfx::pixel(random_int(0, 319), random_int(0, 239), 1); }",
            5,
            0,
        )
        .unwrap();
        assert_ne!(hash(&default), hash(&seeded));
        assert_eq!(default.state.frames.get(), 5);
        assert_eq!(seeded.state.frames.get(), 5);
    }

    #[test]
    fn same_seed_means_same_stream() {
        let code = "fn on_init() { system::seed(77); }\nfn on_draw() { let i = random_int(0, 200); gfx::pixel(i, 0, 255); }";
        let first = run(code, 8, 0).unwrap();
        let second = run(code, 8, 0).unwrap();
        assert_eq!(first.framebuffer_snapshot(), second.framebuffer_snapshot());
    }

    #[test]
    fn quit_stops_the_cart_with_the_message() {
        let mut runtime = run("fn on_draw() { system::quit(\"you win\"); }", 1, 0).unwrap();
        assert_eq!(runtime.state.frames.get(), 1);
        let quit = runtime.take_quit();
        assert_eq!(quit, Some(Quit { message: Some("you win".to_string()) }));
        // The quit latch is cleared; the cart is just stopped.
        assert!(!runtime.is_quitting());
    }

    #[test]
    fn quit_without_message_has_none() {
        let mut runtime = run("fn on_draw() { system::quit(); }", 1, 0).unwrap();
        assert_eq!(runtime.take_quit(), Some(Quit { message: None }));
    }

    #[test]
    fn color_out_of_range_is_a_positioned_error() {
        let error = expect_error("fn on_draw() {\n    gfx::pixel(0, 0, 256);\n}");
        assert_eq!(error.cart, "test.r16");
        assert_eq!(error.line, 2);
        assert!(error.col > 1, "column reported: {}", error.col);
        assert!(
            error.message.contains("out of range"),
            "message: {}",
            error.message
        );
    }

    #[test]
    fn nan_float_argument_is_a_positioned_error() {
        let error = expect_error("fn on_draw() { gfx::pixel(1.0 / 0.0, 0, 1); }");
        assert_eq!(error.line, 1);
        assert!(
            error.message.contains("finite"),
            "message: {}",
            error.message
        );
    }

    #[test]
    fn float_arguments_truncate_toward_zero() {
        let runtime = run(
            "fn on_draw() { gfx::pixel(1.9, 2.9, 1); gfx::pixel(-0.9, 5.9, 2); gfx::rect_filled(2.9, 2.9, 1.9, 1.9, 3); }",
            1,
            0,
        )
        .unwrap();
        let framebuffer = runtime.state.framebuffer.borrow();
        assert_eq!(framebuffer.get(1, 2), Some(1), "1.9/2.9 truncate toward zero");
        assert_eq!(framebuffer.get(2, 3), Some(0), "no rounding to 2/3");
        assert_eq!(framebuffer.get(0, 5), Some(2), "-0.9 truncates to 0, not -1");
        assert_eq!(framebuffer.get(-1, 5), None, "-1.9 never drawn off-screen");
        assert_eq!(framebuffer.get(2, 2), Some(3), "2.9 size truncates to 1 wide");
        assert_eq!(framebuffer.get(3, 3), Some(0));
    }

    #[test]
    fn random_int_with_inverted_range_is_an_error() {
        let error = expect_error("fn on_draw() { random_int(5, 2); }");
        assert_eq!(error.line, 1);
        assert!(error.message.contains("random_int"), "message: {}", error.message);
    }

    #[test]
    fn unknown_function_is_an_error_with_position() {
        let error = expect_error("fn on_draw() {\n  totally_not_a_function(1);\n}");
        assert_eq!(error.line, 2);
        assert!(
            error.message.contains("not found") || error.message.to_lowercase().contains("function"),
            "message: {}",
            error.message
        );
        assert_eq!(error.cart, "test.r16");
    }

    #[test]
    fn syntax_errors_report_line_and_column() {
        let error = match Runtime::new(assets("fn on_draw( {")) {
            Err(error) => error,
            Ok(_) => panic!("expected a parse error"),
        };
        assert!(error.line >= 1);
        assert!(error.col >= 1);
        assert!(!error.message.is_empty());
    }

    #[test]
    fn empty_code_section_is_fine() {
        let mut runtime = Runtime::new(assets("")).unwrap();
        runtime.init().unwrap();
        runtime.step(0).unwrap();
        assert!(
            runtime
                .state
                .framebuffer
                .borrow()
                .indices()
                .iter()
                .all(|index| *index == 0)
        );
    }

    #[test]
    fn wrong_entry_point_arity_fails_to_load() {
        for (code, name) in [
            ("fn on_init(x) { }", "on_init"),
            ("fn on_update() { }", "on_update"),
            ("fn on_draw(dt) { }", "on_draw"),
        ] {
            let error = match Runtime::new(assets(code)) {
                Err(error) => error,
                Ok(_) => panic!("expected a load error for: {code}"),
            };
            assert_eq!(error.cart, "test.r16");
            assert_eq!((error.line, error.col), (1, 1), "no definition position");
            assert!(
                error.message.contains(name),
                "message names the handler: {}",
                error.message
            );
            assert!(
                error.message.contains("parameter"),
                "message explains the signature: {}",
                error.message
            );
        }
    }

    #[test]
    fn palette_reset_restores_the_cart_snapshot() {
        let mut assets = assets("fn on_init() { gfx::palette(7, 0, 0, 0); gfx::palette_reset(); }");
        let mut entries = [0u16; PALETTE_LEN];
        entries[7] = 0x7c00; // red
        assets.palette = Some(entries);

        let mut runtime = Runtime::new(assets).unwrap();
        runtime.init().unwrap();
        assert_eq!(
            runtime.state.palette.borrow().rgba8(7),
            [255, 0, 0, 255],
            "reset restored red"
        );
    }

    #[test]
    fn sprite_pixels_are_readable_and_writable() {
        let runtime = run("fn on_draw() { sprite::set_pixel(3, 1, 2, 77); }", 1, 0).unwrap();
        let sheet = runtime.state.sheet.borrow();
        assert_eq!(rico16_core::sheet::get_pixel(&sheet, 3, 1, 2), Some(77));
        drop(sheet);

        let mut runtime =
            Runtime::new(assets(
                "fn on_init() { sprite::set_pixel(0, 0, 0, 5); }\nfn on_draw() { let v = sprite::get_pixel(0, 0, 0); gfx::text(v.to_string(), 0, 0, v); }",
            ))
            .unwrap();
        runtime.init().unwrap();
        runtime.step(0).unwrap();
        assert_eq!(runtime.state.framebuffer.borrow().get(0, 0), Some(5));
    }

    #[test]
    fn input_edges_are_visible_to_carts() {
        let mut runtime = run(
            "let seen = 0;\nlet down = 0;\nlet up = 0;\nfn on_update(dt) { if input::pressed(Button::Right) { seen += 1; } if input::held(Button::Right) { down += 1; } if input::released(Button::Right) { up += 1; } }",
            0,
            0,
        )
        .unwrap();

        let right = Button::Right.bit();
        // Frame 0: nothing held.
        runtime.step(0).unwrap();
        // Frame 1: press.
        runtime.step(right).unwrap();
        // Frame 2: held.
        runtime.step(right).unwrap();
        // Frame 3: release.
        runtime.step(0).unwrap();
        // Frame 4: nothing.
        runtime.step(0).unwrap();

        assert_eq!(runtime.state.frames.get(), 5);
        let seen: i64 = runtime.scope.get_value("seen").unwrap();
        let down: i64 = runtime.scope.get_value("down").unwrap();
        let up: i64 = runtime.scope.get_value("up").unwrap();
        assert_eq!(seen, 1, "pressed fires once");
        assert_eq!(down, 2, "held in frames 1 and 2");
        assert_eq!(up, 1, "released fires once");
    }

    impl Runtime {
        /// Copy of the framebuffer contents for tests.
        fn framebuffer_snapshot(&self) -> Vec<u8> {
            self.state.framebuffer.borrow().indices().to_vec()
        }
    }

    #[test]
    fn world_clips_at_the_screen_edge_through_the_api() {
        let runtime = run(
            "fn on_draw() { gfx::rect_filled(-100, -100, 500, 500, 9); }",
            1,
            0,
        )
        .unwrap();
        let framebuffer = runtime.state.framebuffer.borrow();
        // Everything on screen got painted; nothing outside can exist.
        assert!(framebuffer.indices().iter().all(|index| *index == 9));
        assert_eq!(framebuffer.indices().len(), SCREEN_WIDTH * 240);
    }
}
