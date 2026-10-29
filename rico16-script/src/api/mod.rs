//! SPEC §9.3, §9.6 — the registered script surface.
//!
//! One file per module, one `module` function each, no scattered `register_fn`
//! calls (AGENTS.md). Every console function mutates cart memory, so all are
//! registered impure: Rhai must never constant-fold a draw call away.

pub mod args;

mod audio;
mod gfx;
mod input;
mod rnd;
mod sprite;
mod system;
mod tilemap;

use std::collections::BTreeMap;
use std::rc::Rc;

use rhai::{Engine, Module};

use crate::runtime::Memory;

/// Register a module function marked **impure**.
macro_rules! impure {
    ($module:ident, $name:literal, $func:expr) => {
        rhai::FuncRegistration::new($name)
            .with_purity(false)
            .set_into_module(&mut $module, $func);
    };
}
pub(crate) use impure;

/// SPEC §9.6 — register every standard module as an ambient global and return
/// them keyed by name so the runtime can also serve `import`.
pub fn register(engine: &mut Engine, memory: &Rc<Memory>) -> BTreeMap<String, Module> {
    let modules = [
        ("gfx", gfx::module(memory)),
        ("sprite", sprite::module(memory)),
        ("tilemap", tilemap::module(memory)),
        ("input", input::module(memory)),
        ("system", system::module(memory)),
        ("audio", audio::module(memory)),
        ("rnd", rnd::module(memory)),
        ("Button", input::button_module()),
    ];

    let mut by_name = BTreeMap::new();
    for (name, module) in modules {
        engine.register_static_module(name, module.clone().into());
        by_name.insert(name.to_string(), module);
    }
    by_name
}
