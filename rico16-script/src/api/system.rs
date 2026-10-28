//! The `system::` module. SPEC §9.3, §5.4, §12.7.

use rhai::{Dynamic, Engine, Module, NativeCallContext};

use rico16_core::limits::DT;
use rico16_core::rng::Rng;

use crate::api::impure_fn;
use crate::api::args::{to_int, unit, RhaiResult};
use crate::api::StateCell;

/// Register the `system::` module on `engine`.
pub fn register(engine: &mut Engine, state: &StateCell) {
    let mut module = Module::new();

    let s = state.clone();
    impure_fn!(module, "frames", move || -> i64 {
        s.frames.get() as i64
    });

    let s = state.clone();
    impure_fn!(module, "time", move || -> f64 {
        s.frames.get() as f64 * DT
    });

    let s = state.clone();
    impure_fn!(
        module,
        "seed",
        move |ctx: NativeCallContext, seed: Dynamic| -> RhaiResult {
            let seed = to_int(&ctx, &seed, "seed")?;
            *s.rng.borrow_mut() = Rng::seed(seed as u64);
            Ok(unit())
        }
    );

    let s = state.clone();
    impure_fn!(module, "quit", move || {
        *s.quit.borrow_mut() = Some(None);
        unit()
    });

    let s = state.clone();
    impure_fn!(module, "quit", move |message: &str| {
        *s.quit.borrow_mut() = Some(Some(message.to_string()));
        unit()
    });

    engine.register_static_module("system", module.into());
}
