//! The `input::` module and the `Button` namespace. SPEC §9.3, §12.2.
//!
//! Buttons are logical and positional; the player maps physical keys once per
//! frame and the cart only ever names a `Button` constant.

use rhai::{Engine, Module};

use rico16_core::input::Button;

use crate::api::impure_fn;
use crate::api::StateCell;

/// Register the `input::` module and the `Button` constants on `engine`.
pub fn register(engine: &mut Engine, state: &StateCell) {
    register_buttons(engine);

    let mut module = Module::new();

    let s = state.clone();
    impure_fn!(module, "held", move |button: Button| -> bool {
        s.input.borrow().held(button)
    });

    let s = state.clone();
    impure_fn!(module, "pressed", move |button: Button| -> bool {
        s.input.borrow().pressed(button)
    });

    let s = state.clone();
    impure_fn!(module, "released", move |button: Button| -> bool {
        s.input.borrow().released(button)
    });

    engine.register_static_module("input", module.into());
}

/// Register `Button::Left` … `Button::V` as module constants. SPEC §9.3.
///
/// A Rhai module named `Button` holds one constant per variant, an opaque
/// value of the core `Button` type that only the `input::` functions accept.
fn register_buttons(engine: &mut Engine) {
    engine.register_type::<Button>();

    let mut module = Module::new();
    for (name, button) in [
        ("Left", Button::Left),
        ("Right", Button::Right),
        ("Up", Button::Up),
        ("Down", Button::Down),
        ("Z", Button::Z),
        ("X", Button::X),
        ("C", Button::C),
        ("V", Button::V),
    ] {
        module.set_var(name, button);
    }
    engine.register_static_module("Button", module.into());
}
