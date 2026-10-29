//! SPEC §9.3 — the `sprite::` module.

use std::rc::Rc;

use rhai::{Dynamic, Module, NativeCallContext};

use crate::api::args::{self, RhaiResult};
use crate::api::impure;
use crate::runtime::Memory;

/// Register the `sprite::` module.
pub fn module(memory: &Rc<Memory>) -> Module {
    let mut m = Module::new();

    let mem = memory.clone();
    impure!(m, "draw", move |ctx: NativeCallContext,
                             id: Dynamic,
                             x: Dynamic,
                             y: Dynamic|
          -> RhaiResult<Dynamic> {
        let id = args::to_id(&ctx, &id, "sprite id")?;
        let x = args::to_int(&ctx, &x, "x")?;
        let y = args::to_int(&ctx, &y, "y")?;
        mem.sheet
            .borrow()
            .draw(&mut mem.framebuffer.borrow_mut(), id, x, y, false, false);
        Ok(args::unit())
    });

    let mem = memory.clone();
    impure!(m, "draw", move |ctx: NativeCallContext,
                             id: Dynamic,
                             x: Dynamic,
                             y: Dynamic,
                             flip_x: bool,
                             flip_y: bool|
          -> RhaiResult<Dynamic> {
        let id = args::to_id(&ctx, &id, "sprite id")?;
        let x = args::to_int(&ctx, &x, "x")?;
        let y = args::to_int(&ctx, &y, "y")?;
        mem.sheet
            .borrow()
            .draw(&mut mem.framebuffer.borrow_mut(), id, x, y, flip_x, flip_y);
        Ok(args::unit())
    });

    let mem = memory.clone();
    impure!(m, "draw_region", move |ctx: NativeCallContext,
                                    sx: Dynamic,
                                    sy: Dynamic,
                                    w: Dynamic,
                                    h: Dynamic,
                                    dx: Dynamic,
                                    dy: Dynamic,
                                    flip_x: bool,
                                    flip_y: bool|
          -> RhaiResult<Dynamic> {
        let sx = args::to_int(&ctx, &sx, "sx")?;
        let sy = args::to_int(&ctx, &sy, "sy")?;
        let w = args::to_int(&ctx, &w, "w")?;
        let h = args::to_int(&ctx, &h, "h")?;
        let dx = args::to_int(&ctx, &dx, "dx")?;
        let dy = args::to_int(&ctx, &dy, "dy")?;
        mem.sheet
            .borrow()
            .draw_region(
                &mut mem.framebuffer.borrow_mut(),
                sx,
                sy,
                w,
                h,
                dx,
                dy,
                flip_x,
                flip_y,
            )
            .map_err(|e| args::error(&ctx, e.to_string()))?;
        Ok(args::unit())
    });

    let mem = memory.clone();
    impure!(m, "get_pixel", move |ctx: NativeCallContext,
                                  id: Dynamic,
                                  px: Dynamic,
                                  py: Dynamic|
          -> RhaiResult<i64> {
        let id = args::to_id(&ctx, &id, "sprite id")?;
        let px = args::to_tile_pixel(&ctx, &px, "px")?;
        let py = args::to_tile_pixel(&ctx, &py, "py")?;
        Ok(mem.sheet.borrow().get_pixel(id, px, py) as i64)
    });

    let mem = memory.clone();
    impure!(m, "set_pixel", move |ctx: NativeCallContext,
                                  id: Dynamic,
                                  px: Dynamic,
                                  py: Dynamic,
                                  color: Dynamic|
          -> RhaiResult<Dynamic> {
        let id = args::to_id(&ctx, &id, "sprite id")?;
        let px = args::to_tile_pixel(&ctx, &px, "px")?;
        let py = args::to_tile_pixel(&ctx, &py, "py")?;
        let color = args::to_color(&ctx, &color, "color")?;
        mem.sheet.borrow_mut().set_pixel(id, px, py, color);
        Ok(args::unit())
    });

    m
}
