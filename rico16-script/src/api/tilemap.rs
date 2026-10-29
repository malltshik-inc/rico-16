//! SPEC §9.3 — the `tilemap::` module.

use std::rc::Rc;

use rhai::{Dynamic, Module, NativeCallContext};

use crate::api::args::{self, RhaiResult};
use crate::api::impure;
use crate::runtime::Memory;

/// Register the `tilemap::` module.
pub fn module(memory: &Rc<Memory>) -> Module {
    let mut m = Module::new();

    let mem = memory.clone();
    impure!(m, "draw", move |ctx: NativeCallContext,
                             cx: Dynamic,
                             cy: Dynamic,
                             cw: Dynamic,
                             ch: Dynamic,
                             dx: Dynamic,
                             dy: Dynamic|
          -> RhaiResult<Dynamic> {
        let cx = args::to_int(&ctx, &cx, "cx")?;
        let cy = args::to_int(&ctx, &cy, "cy")?;
        let cw = args::to_int(&ctx, &cw, "cw")?;
        let ch = args::to_int(&ctx, &ch, "ch")?;
        let dx = args::to_int(&ctx, &dx, "dx")?;
        let dy = args::to_int(&ctx, &dy, "dy")?;
        let sheet = mem.sheet.borrow();
        let map = mem.map.borrow();
        map.draw(
            &mut mem.framebuffer.borrow_mut(),
            &sheet,
            cx,
            cy,
            cw,
            ch,
            dx,
            dy,
        );
        Ok(args::unit())
    });

    let mem = memory.clone();
    impure!(m, "get", move |cx: i64, cy: i64| -> i64 {
        mem.map.borrow().get(cx, cy) as i64
    });

    let mem = memory.clone();
    impure!(m, "set", move |ctx: NativeCallContext,
                            cx: Dynamic,
                            cy: Dynamic,
                            id: Dynamic|
          -> RhaiResult<Dynamic> {
        let cx = args::to_int(&ctx, &cx, "cx")?;
        let cy = args::to_int(&ctx, &cy, "cy")?;
        let id = args::to_id(&ctx, &id, "tile id")?;
        mem.map.borrow_mut().set(cx, cy, id);
        Ok(args::unit())
    });

    m
}
