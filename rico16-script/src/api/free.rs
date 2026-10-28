//! Free (`math`-less) helper functions. SPEC §9.1, §5.5.
//!
//! Rhai already provides `sin`, `cos`, `sqrt`, `abs` and string/array
//! operations, so there is no `math::` module; randomness is the only free
//! surface the console itself adds.

use rhai::{Dynamic, Engine, FuncRegistration, NativeCallContext};

use crate::api::args::{error, to_int};
use crate::api::StateCell;

/// Register `random()` and `random_int(lo, hi)` in the global namespace.
/// SPEC §5.5.
pub fn register(engine: &mut Engine, state: &StateCell) {
    let s = state.clone();
    FuncRegistration::new("random")
        .with_purity(false)
        .register_into_engine(engine, move || -> f64 {
            s.rng.borrow_mut().random()
        });

    let s = state.clone();
    FuncRegistration::new("random_int")
        .with_purity(false)
        .register_into_engine(
            engine,
            move |ctx: NativeCallContext, lo: Dynamic, hi: Dynamic| {
                let (lo, hi) = (
                    to_int(&ctx, &lo, "lo")?,
                    to_int(&ctx, &hi, "hi")?,
                );
                match s.rng.borrow_mut().random_int(lo, hi) {
                    Ok(value) => Ok(Dynamic::from(value)),
                    Err(rng_error) => Err(error(&ctx, rng_error.to_string())),
                }
            },
        );
}
