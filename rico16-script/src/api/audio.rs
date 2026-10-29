//! SPEC §9.3, §14.10 — the `audio::` module. Accepted and ignored in v1.

use std::rc::Rc;

use rhai::{Dynamic, Module, NativeCallContext};

use crate::api::args::{self, RhaiResult};
use crate::api::impure;
use crate::runtime::Memory;

/// Register the `audio::` module.
pub fn module(_memory: &Rc<Memory>) -> Module {
    let mut m = Module::new();
    impure!(m, "play", move |ctx: NativeCallContext,
                             id: Dynamic|
          -> RhaiResult<Dynamic> {
        // SPEC §6.2 — integer arguments accept floats, floored.
        let _id = args::to_int(&ctx, &id, "id")?;
        Ok(args::unit())
    });
    m
}
