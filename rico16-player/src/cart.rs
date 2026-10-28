//! Running a parsed cart through the script runtime with a [`Presenter`].
//!
//! `rico16-script` owns the Rhai engine and cart memory; this module owns the
//! present path (one frame per step, letterboxed by the presenter). Filesystem
//! access stays out of `script` and `core`, so it lives here — the player may
//! read files; the console cannot (SPEC §5.6).

use crate::PlayerError;
use crate::present::Presenter;
use rico16_cart::text::{Cart, ParseError};
use rico16_core::limits::{CODE_LIMIT, FRAMEBUFFER_LEN};
pub use rico16_script::Quit;
use rico16_script::{CartAssets, Runtime, ScriptError};

/// A loaded cart running against a presenter.
///
/// Frames: `init()` once, then `step(mask)` + `present()` per frame.
/// `system::quit` latches after `step`; `take_quit` reads and clears it.
pub struct CartRunner<P: Presenter> {
    runtime: Runtime,
    presenter: P,
    rgba: Vec<u8>,
}

impl<P: Presenter> CartRunner<P> {
    /// Parse cart text, validate resources, compile and build the runner.
    pub fn from_text(cart_name: &str, source: &str, presenter: P) -> Result<Self, CartRunnerError> {
        let cart = Self::parse(cart_name, source)?;
        Self::build(cart, cart_name, presenter)
    }

    /// Build from cart data (e.g. after unpacking a `.r16.png`).
    pub fn from_cart(cart: Cart, presenter: P) -> Result<Self, CartRunnerError> {
        let name = cart.meta.title.clone();
        Self::build(cart, &name, presenter)
    }

    /// Shared body of [`Self::from_text`] and [`Self::from_cart`].
    ///
    /// `name` is what script errors report (SPEC §9.4.1): the cart file name
    /// when the host has one, the cart title otherwise.
    fn build(cart: Cart, name: &str, presenter: P) -> Result<Self, CartRunnerError> {
        let runtime = Self::runtime(cart, name)?;
        let mut runner = Self {
            runtime,
            presenter,
            rgba: vec![0; FRAMEBUFFER_LEN * 4],
        };
        runner.init()?;
        Ok(runner)
    }

    /// Compile a parsed cart into a fresh runtime.
    fn runtime(cart: Cart, name: &str) -> Result<Runtime, CartRunnerError> {
        if cart.code.len() > CODE_LIMIT {
            return Err(CartRunnerError::CodeTooLong {
                actual: cart.code.len(),
                limit: CODE_LIMIT,
            });
        }

        let assets = CartAssets {
            name: name.to_string(),
            code: cart.code,
            sheet: cart.gfx,
            map: cart.map,
            palette: cart.palette,
        };

        Runtime::new(assets).map_err(CartRunnerError::Script)
    }

    /// Restart from source, keeping the window and surface. SPEC §13.3.
    ///
    /// Globals are not preserved: the new runtime starts from `on_init` again.
    pub fn restart(&mut self, source: &str) -> Result<(), CartRunnerError> {
        let cart = Self::parse(self.runtime.name(), source)?;
        self.runtime = Self::runtime(cart, self.runtime.name())?;
        self.init()
    }

    /// Parse and validate cart text, naming the cart in any error.
    fn parse(cart_name: &str, source: &str) -> Result<Cart, CartRunnerError> {
        let cart = Cart::parse(source).map_err(|error| CartRunnerError::Cart {
            name: cart_name.to_string(),
            error,
        })?;
        cart.validate().map_err(|error| CartRunnerError::Cart {
            name: cart_name.to_string(),
            error,
        })?;
        Ok(cart)
    }

    /// Run the cart's top-level code and `on_init`. SPEC §5.2.
    pub fn init(&mut self) -> Result<(), CartRunnerError> {
        self.runtime.init().map_err(CartRunnerError::Script)
    }

    /// One frame: update input sample, `on_update(dt)`, `on_draw()`, present.
    pub fn step(&mut self, mask: u8) -> Result<(), CartRunnerError> {
        self.runtime.step(mask).map_err(CartRunnerError::Script)?;
        self.runtime.present_into(&mut self.rgba);
        self.presenter
            .present(&self.rgba)
            .map_err(CartRunnerError::Presenter)?;
        Ok(())
    }

    /// Resize the window surface. The framebuffer stays 320×240. SPEC §3.7.
    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), PlayerError> {
        self.presenter.resize(width, height)
    }

    /// Forget held buttons and edges without reporting releases. SPEC §12.5.
    pub fn clear_input(&mut self) {
        self.runtime.clear_input();
    }

    /// Did `system::quit` fire this frame? SPEC §12.7.
    pub fn is_quitting(&self) -> bool {
        self.runtime.is_quitting()
    }

    /// Take the quit request, clearing it.
    pub fn take_quit(&mut self) -> Option<Quit> {
        self.runtime.take_quit()
    }

    /// The cart's title.
    pub fn name(&self) -> &str {
        self.runtime.name()
    }
}

/// Why a cart could not run. Lowercase messages, no `Error:` prefix
/// (`AGENTS.md`).
#[derive(Debug, thiserror::Error)]
pub enum CartRunnerError {
    #[error("{name}: {error}")]
    Cart {
        name: String,
        #[source]
        error: ParseError,
    },
    #[error("code is {actual} bytes, the limit is {limit}")]
    CodeTooLong { actual: usize, limit: usize },
    #[error(transparent)]
    Script(#[from] ScriptError),
    #[error(transparent)]
    Presenter(#[from] PlayerError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use rico16_cart::text::Meta;
    use rico16_core::input::{Button, Input};

    fn empty_cart() -> Cart {
        Cart {
            meta: Meta::default(),
            code: "fn on_draw() { gfx::pixel(1, 1, 7); }".to_string(),
            gfx: vec![0; rico16_core::limits::SHEET_LEN],
            map: vec![0; rico16_core::limits::MAP_LEN],
            palette: None,
            unknown: Vec::new(),
        }
    }

    #[test]
    fn from_cart_runs_and_presents() {
        let mut runner = CartRunner::from_cart(empty_cart(), StubPresenter::new()).unwrap();
        runner.step(Input::new().mask()).unwrap();
        assert_eq!(runner.runtime.state().frames.get(), 1);
        assert!(!runner.is_quitting());
    }

    #[test]
    fn quit_stops_after_current_frame() {
        let mut runner = CartRunner::from_cart(
            Cart {
                code: "fn on_draw() { system::quit(\"done\"); }".to_string(),
                ..empty_cart()
            },
            StubPresenter::new(),
        )
        .unwrap();
        runner.step(Input::new().mask()).unwrap();
        assert_eq!(runner.runtime.state().frames.get(), 1);
        let quit = runner.take_quit();
        assert_eq!(
            quit,
            Some(Quit {
                message: Some("done".to_string())
            })
        );
    }

    #[test]
    fn oversized_code_is_rejected() {
        let code = "x".repeat(CODE_LIMIT + 1);
        let cart = Cart {
            code,
            ..empty_cart()
        };
        let error = match CartRunner::from_cart(cart, StubPresenter::new()) {
            Ok(_) => panic!("oversized code must be rejected"),
            Err(error) => error,
        };
        assert!(matches!(error, CartRunnerError::CodeTooLong { .. }));
    }

    #[test]
    fn unparseable_cart_is_a_clear_error() {
        let error = match CartRunner::from_text("bad.r16", "not a cart", StubPresenter::new()) {
            Ok(_) => panic!("unparseable text must be rejected"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("no `__section__` header"));
    }

    #[test]
    fn restart_reruns_init_and_starts_from_a_fresh_framebuffer() {
        let mut runner = CartRunner::from_cart(empty_cart(), StubPresenter::new()).unwrap();
        runner.step(Input::new().mask()).unwrap();
        assert_eq!(
            runner.runtime.state().framebuffer.borrow().get(1, 1),
            Some(7)
        );

        runner
            .restart("__code__\nfn on_draw() { gfx::pixel(2, 2, 9); }\n")
            .unwrap();
        assert_eq!(runner.runtime.state().frames.get(), 0, "fresh frame count");
        runner.step(Input::new().mask()).unwrap();

        let framebuffer = runner.runtime.state().framebuffer.borrow();
        assert_eq!(framebuffer.get(2, 2), Some(9));
        assert_eq!(framebuffer.get(1, 1), Some(0), "old drawing is gone");
    }

    #[test]
    fn the_hello_example_runs_headless() {
        let source = include_str!("../../examples/hello.r16");
        let mut runner = CartRunner::from_text("hello.r16", source, StubPresenter::new()).unwrap();
        for _ in 0..60 {
            runner.step(Input::new().mask()).unwrap();
        }
        assert!(!runner.is_quitting());
        assert_eq!(runner.runtime.state().frames.get(), 60);
    }

    /// The top-left cell of the first pixel of `color` below the HUD.
    fn cell_of(runner: &CartRunner<StubPresenter>, color: u8) -> Option<(i64, i64)> {
        let framebuffer = runner.runtime.state().framebuffer.borrow();
        for y in 16..240 {
            for x in 0..320 {
                if framebuffer.get(x, y) == Some(color) {
                    return Some(((x / 8) as i64, ((y - 16) / 8) as i64));
                }
            }
        }
        None
    }

    #[test]
    fn the_snake_example_moves_and_eats() {
        let source = include_str!("../../examples/snake.r16");
        let mut runner = CartRunner::from_text("snake.r16", source, StubPresenter::new()).unwrap();

        // Frame 0 draws without moving: head at (20, 14), body behind it.
        runner.step(0).unwrap();
        assert_eq!(cell_of(&runner, 9), Some((20, 14)), "head");
        // Seed 1987 puts the first food at (30, 9): ten cells right, five up.
        assert_eq!(cell_of(&runner, 8), Some((30, 9)), "first food");

        // Seven frames per cell. Right for ten cells (step 70), turn up on
        // step 71, then five more cells to reach the food on step 105.
        for step in 1..=105 {
            let mask = if step == 71 { Button::Up.bit() } else { 0 };
            runner.step(mask).unwrap();
        }

        assert_eq!(
            cell_of(&runner, 9),
            Some((30, 9)),
            "the head reached the food"
        );
        assert_ne!(cell_of(&runner, 8), Some((30, 9)), "a new food appeared");
        assert!(!runner.is_quitting());
    }

    #[test]
    fn the_snake_example_dies_at_the_wall_and_restarts_on_z() {
        let source = include_str!("../../examples/snake.r16");
        let mut runner = CartRunner::from_text("snake.r16", source, StubPresenter::new()).unwrap();

        // Right from x=20 to the wall at x=40: twenty cells, seven frames each.
        for _ in 0..150 {
            runner.step(0).unwrap();
        }
        assert_eq!(cell_of(&runner, 9), Some((39, 14)), "stopped at the wall");

        runner.step(Button::Z.bit()).unwrap();
        assert_eq!(cell_of(&runner, 9), Some((20, 14)), "back at the start");
        let food = cell_of(&runner, 8).expect("fresh food");
        assert_ne!(food, (20, 14), "food is never under the head");
        assert!(!runner.is_quitting());
    }

    #[test]
    fn the_snake_example_runs_headless() {
        let source = include_str!("../../examples/snake.r16");
        let mut runner = CartRunner::from_text("snake.r16", source, StubPresenter::new()).unwrap();
        // No input: the snake runs right, hits the wall and waits on the
        // game-over screen. Nothing may error, and the cart must not quit.
        for _ in 0..300 {
            runner.step(Input::new().mask()).unwrap();
        }
        assert!(!runner.is_quitting());
        assert_eq!(runner.runtime.state().frames.get(), 300);
    }

    /// A presenter that records the last frame's palette expansion.
    struct StubPresenter;

    impl StubPresenter {
        fn new() -> Self {
            Self
        }
    }

    impl Presenter for StubPresenter {
        fn present(&mut self, _rgba: &[u8]) -> Result<(), PlayerError> {
            Ok(())
        }

        fn resize(&mut self, _width: u32, _height: u32) -> Result<(), PlayerError> {
            Ok(())
        }
    }
}
