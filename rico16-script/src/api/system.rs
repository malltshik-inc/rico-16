//! SPEC §9.3, §5.4, §14.9 — the `system::` module.

use std::rc::Rc;

use rhai::{Dynamic, Module, NativeCallContext};

use rico16_core::limits::DT;
use rico16_core::rng::Rng;

use crate::api::args::{self, RhaiResult};
use crate::api::impure;
use crate::runtime::{Action, Memory};

/// Register the `system::` module.
pub fn module(memory: &Rc<Memory>) -> Module {
    let mut m = Module::new();

    let mem = memory.clone();
    impure!(m, "frames", move || -> i64 { mem.frames.get() as i64 });

    let mem = memory.clone();
    impure!(m, "time", move || -> f64 {
        args::canonical_float(mem.frames.get() as f64 * DT)
    });

    let mem = memory.clone();
    impure!(m, "seed", move |ctx: NativeCallContext,
                             seed: Dynamic|
          -> RhaiResult<Dynamic> {
        let seed = args::to_int(&ctx, &seed, "seed")?;
        *mem.rng.borrow_mut() = Rng::seed(seed as u64);
        Ok(args::unit())
    });

    let mem = memory.clone();
    impure!(m, "quit", move || -> Dynamic {
        *mem.action.borrow_mut() = Action::Quit(None);
        args::unit()
    });

    let mem = memory.clone();
    impure!(m, "quit", move |message: &str| -> Dynamic {
        *mem.action.borrow_mut() = Action::Quit(Some(message.to_string()));
        args::unit()
    });

    let mem = memory.clone();
    impure!(m, "reset", move || -> Dynamic {
        *mem.action.borrow_mut() = Action::Reset;
        args::unit()
    });

    m
}
