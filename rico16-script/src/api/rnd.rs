//! SPEC §9.3, §5.5 — the `rnd::` module.

use std::rc::Rc;

use rhai::{Array, Dynamic, Module, NativeCallContext};

use crate::api::args::{self, RhaiResult};
use crate::api::impure;
use crate::runtime::Memory;

/// Register the `rnd::` module.
pub fn module(memory: &Rc<Memory>) -> Module {
    let mut m = Module::new();

    let mem = memory.clone();
    impure!(m, "float", move || -> f64 {
        args::canonical_float(mem.rng.borrow_mut().float())
    });

    let mem = memory.clone();
    impure!(m, "int", move |ctx: NativeCallContext,
                            lo: Dynamic,
                            hi: Dynamic|
          -> RhaiResult<i64> {
        let lo = args::to_int(&ctx, &lo, "lo")?;
        let hi = args::to_int(&ctx, &hi, "hi")?;
        mem.rng
            .borrow_mut()
            .int(lo, hi)
            .map_err(|e| args::error(&ctx, e.to_string()))
    });

    let mem = memory.clone();
    impure!(m, "choice", move |arr: Array| -> Dynamic {
        let mut rng = mem.rng.borrow_mut();
        match rng.choice_index(arr.len()) {
            Some(index) => arr[index].clone(),
            None => Dynamic::UNIT,
        }
    });

    let mem = memory.clone();
    impure!(m, "shuffle", move |arr: &mut Array| -> Array {
        mem.rng.borrow_mut().shuffle(arr);
        arr.clone()
    });

    m
}
