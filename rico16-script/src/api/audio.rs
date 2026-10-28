//! The `audio::` module. SPEC §9.3, §12.8.
//!
//! There is no audio device in v1; `audio::play` is accepted and ignored.
//! The argument still has to be a number so a cart type error is caught.

use rhai::{Dynamic, Engine, Module, NativeCallContext};

use crate::api::impure_fn;
use crate::api::args::{to_int, unit, RhaiResult};
use crate::api::StateCell;

/// Register the `audio::` module on `engine`.
pub fn register(engine: &mut Engine, state: &StateCell) {
    let mut module = Module::new();

    let _state = state.clone();
    impure_fn!(
        module,
        "play",
        move |ctx: NativeCallContext, id: Dynamic| -> RhaiResult {
            let _id = to_int(&ctx, &id, "id")?;
            Ok(unit())
        }
    );

    engine.register_static_module("audio", module.into());
}
