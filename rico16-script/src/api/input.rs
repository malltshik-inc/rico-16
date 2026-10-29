//! SPEC §9.3, §14.6 — the `input::` module and the `Button` values.

use std::rc::Rc;

use rhai::{Dynamic, Module};

use rico16_core::input::Button;

use crate::api::impure;
use crate::runtime::Memory;

/// SPEC §9.3 — the `Button` namespace of constants.
pub fn button_module() -> Module {
    let mut m = Module::new();
    for button in Button::ALL {
        m.set_var(format!("{button:?}"), Dynamic::from(button));
    }
    m
}

/// Register the `input::` module.
pub fn module(memory: &Rc<Memory>) -> Module {
    let mut m = Module::new();

    let mem = memory.clone();
    impure!(m, "held", move |button: Button| -> bool {
        mem.input.borrow().held(button)
    });

    let mem = memory.clone();
    impure!(m, "pressed", move |button: Button| -> bool {
        mem.input.borrow().pressed(button)
    });

    let mem = memory.clone();
    impure!(m, "released", move |button: Button| -> bool {
        mem.input.borrow().released(button)
    });

    m
}
