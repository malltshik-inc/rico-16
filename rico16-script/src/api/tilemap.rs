//! The `tilemap::` module. SPEC §9.3, §7.2, §8.3.

use rhai::{Dynamic, Engine, Module, NativeCallContext};

use rico16_core::map as core_map;

use crate::api::impure_fn;
use crate::api::args::{to_coord, to_index, unit, RhaiResult};
use crate::api::StateCell;

/// Register the `tilemap::` module on `engine`.
pub fn register(engine: &mut Engine, state: &StateCell) {
    let mut module = Module::new();

    let s = state.clone();
    impure_fn!(
        module,
        "draw",
        move |ctx: NativeCallContext,
              cx: Dynamic,
              cy: Dynamic,
              cw: Dynamic,
              ch: Dynamic,
              dx: Dynamic,
              dy: Dynamic|
              -> RhaiResult {
            let (cx, cy, cw, ch, dx, dy) = (
                to_coord(&ctx, &cx, "cx")?,
                to_coord(&ctx, &cy, "cy")?,
                to_coord(&ctx, &cw, "cw")?,
                to_coord(&ctx, &ch, "ch")?,
                to_coord(&ctx, &dx, "dx")?,
                to_coord(&ctx, &dy, "dy")?,
            );
            let framebuffer = &mut s.framebuffer.borrow_mut();
            let sheet = s.sheet.borrow();
            let map = s.map.borrow();
            core_map::draw(framebuffer, &sheet, &map, cx, cy, cw, ch, dx, dy);
            Ok(unit())
        }
    );

    let s = state.clone();
    impure_fn!(
        module,
        "get",
        move |ctx: NativeCallContext, cx: Dynamic, cy: Dynamic| -> RhaiResult {
            let (cx, cy) = (
                to_coord(&ctx, &cx, "cx")?,
                to_coord(&ctx, &cy, "cy")?,
            );
            let map = s.map.borrow();
            Ok(Dynamic::from(map.get(cx, cy) as i64))
        }
    );

    let s = state.clone();
    impure_fn!(
        module,
        "set",
        move |ctx: NativeCallContext, cx: Dynamic, cy: Dynamic, id: Dynamic| -> RhaiResult {
            let (cx, cy, id) = (
                to_coord(&ctx, &cx, "cx")?,
                to_coord(&ctx, &cy, "cy")?,
                to_index(&ctx, &id, "id")?,
            );
            let mut map = s.map.borrow_mut();
            map.set(cx, cy, id);
            Ok(unit())
        }
    );

    engine.register_static_module("tilemap", module.into());
}
