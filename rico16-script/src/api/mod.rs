//! The registered script surface: one module per file, one `register` each,
//! no scattered `register_fn` calls elsewhere in the crate. `AGENTS.md`.

mod args;
mod audio;
mod free;
mod gfx;
mod input;
mod sprite;
mod system;
mod tilemap;

use std::rc::Rc;

use rhai::Engine;

use crate::runtime::State;

/// Shared mutable state behind the engine's native closures.
pub type StateCell = Rc<State>;

/// Register a module function marked **impure**.
///
/// Rhai's optimizer may constant-fold calls to *pure* functions with constant
/// arguments; a `gfx::clear(0)` folded at compile time would stop clearing per
/// frame. Every console function mutates cart memory, so all of them are
/// impure by construction.
macro_rules! impure_fn {
    ($module:expr, $name:literal, $func:expr) => {
        rhai::FuncRegistration::new($name)
            .with_purity(false)
            .set_into_module(&mut $module, $func)
    };
}
pub(crate) use impure_fn;

/// Register the complete §9.3 surface on `engine`.
///
/// `state` is cloned into every closure; the engine is rebuilt per cart, so
/// each `Runtime` gets a fresh engine sharing nothing with the previous one.
pub fn register(engine: &mut Engine, state: &StateCell) {
    gfx::register(engine, state);
    sprite::register(engine, state);
    tilemap::register(engine, state);
    input::register(engine, state);
    system::register(engine, state);
    audio::register(engine, state);
    free::register(engine, state);
}
