//! SPEC §9.3, §14.10 — the `audio::` module. Accepted and ignored in v1.

use std::rc::Rc;

use rhai::{Dynamic, Module};

use crate::api::args;
use crate::api::impure;
use crate::runtime::Memory;

/// Register the `audio::` module.
pub fn module(_memory: &Rc<Memory>) -> Module {
    let mut m = Module::new();
    impure!(m, "play", move |_id: i64| -> Dynamic { args::unit() });
    m
}
