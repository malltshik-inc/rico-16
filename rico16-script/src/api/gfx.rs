//! SPEC §9.3 — the `gfx::` module.

use std::rc::Rc;

use rhai::{Dynamic, Module, NativeCallContext};

use rico16_core::gfx as core_gfx;

use crate::api::args::{self, RhaiResult};
use crate::api::impure;
use crate::runtime::Memory;

/// Register the `gfx::` module.
pub fn module(memory: &Rc<Memory>) -> Module {
    let mut m = Module::new();

    let mem = memory.clone();
    impure!(m, "clear", move |ctx: NativeCallContext,
                              color: Dynamic|
          -> RhaiResult<Dynamic> {
        let color = args::to_color(&ctx, &color, "color")?;
        mem.framebuffer.borrow_mut().clear(color);
        Ok(args::unit())
    });

    let mem = memory.clone();
    impure!(m, "pixel", move |ctx: NativeCallContext,
                              x: Dynamic,
                              y: Dynamic,
                              color: Dynamic|
          -> RhaiResult<Dynamic> {
        let x = args::to_int(&ctx, &x, "x")?;
        let y = args::to_int(&ctx, &y, "y")?;
        let color = args::to_color(&ctx, &color, "color")?;
        core_gfx::pixel(&mut mem.framebuffer.borrow_mut(), x, y, color);
        Ok(args::unit())
    });

    let mem = memory.clone();
    impure!(m, "line", move |ctx: NativeCallContext,
                             x0: Dynamic,
                             y0: Dynamic,
                             x1: Dynamic,
                             y1: Dynamic,
                             color: Dynamic|
          -> RhaiResult<Dynamic> {
        let x0 = args::to_int(&ctx, &x0, "x0")?;
        let y0 = args::to_int(&ctx, &y0, "y0")?;
        let x1 = args::to_int(&ctx, &x1, "x1")?;
        let y1 = args::to_int(&ctx, &y1, "y1")?;
        let color = args::to_color(&ctx, &color, "color")?;
        core_gfx::line(&mut mem.framebuffer.borrow_mut(), x0, y0, x1, y1, color);
        Ok(args::unit())
    });

    let mem = memory.clone();
    impure!(m, "rect", move |ctx: NativeCallContext,
                             x: Dynamic,
                             y: Dynamic,
                             w: Dynamic,
                             h: Dynamic,
                             color: Dynamic|
          -> RhaiResult<Dynamic> {
        let x = args::to_int(&ctx, &x, "x")?;
        let y = args::to_int(&ctx, &y, "y")?;
        let w = args::to_int(&ctx, &w, "w")?;
        let h = args::to_int(&ctx, &h, "h")?;
        let color = args::to_color(&ctx, &color, "color")?;
        core_gfx::rect(&mut mem.framebuffer.borrow_mut(), x, y, w, h, color);
        Ok(args::unit())
    });

    let mem = memory.clone();
    impure!(m, "rect_filled", move |ctx: NativeCallContext,
                                    x: Dynamic,
                                    y: Dynamic,
                                    w: Dynamic,
                                    h: Dynamic,
                                    color: Dynamic|
          -> RhaiResult<Dynamic> {
        let x = args::to_int(&ctx, &x, "x")?;
        let y = args::to_int(&ctx, &y, "y")?;
        let w = args::to_int(&ctx, &w, "w")?;
        let h = args::to_int(&ctx, &h, "h")?;
        let color = args::to_color(&ctx, &color, "color")?;
        core_gfx::rect_filled(&mut mem.framebuffer.borrow_mut(), x, y, w, h, color);
        Ok(args::unit())
    });

    let mem = memory.clone();
    impure!(m, "circle", move |ctx: NativeCallContext,
                               x: Dynamic,
                               y: Dynamic,
                               r: Dynamic,
                               color: Dynamic|
          -> RhaiResult<Dynamic> {
        let x = args::to_int(&ctx, &x, "x")?;
        let y = args::to_int(&ctx, &y, "y")?;
        let r = args::to_int(&ctx, &r, "r")?;
        let color = args::to_color(&ctx, &color, "color")?;
        core_gfx::circle(&mut mem.framebuffer.borrow_mut(), x, y, r, color);
        Ok(args::unit())
    });

    let mem = memory.clone();
    impure!(m, "circle_filled", move |ctx: NativeCallContext,
                                      x: Dynamic,
                                      y: Dynamic,
                                      r: Dynamic,
                                      color: Dynamic|
          -> RhaiResult<Dynamic> {
        let x = args::to_int(&ctx, &x, "x")?;
        let y = args::to_int(&ctx, &y, "y")?;
        let r = args::to_int(&ctx, &r, "r")?;
        let color = args::to_color(&ctx, &color, "color")?;
        core_gfx::circle_filled(&mut mem.framebuffer.borrow_mut(), x, y, r, color);
        Ok(args::unit())
    });

    let mem = memory.clone();
    impure!(m, "text", move |ctx: NativeCallContext,
                             text: &str,
                             x: Dynamic,
                             y: Dynamic,
                             color: Dynamic|
          -> RhaiResult<Dynamic> {
        let x = args::to_int(&ctx, &x, "x")?;
        let y = args::to_int(&ctx, &y, "y")?;
        let color = args::to_color(&ctx, &color, "color")?;
        core_gfx::text(&mut mem.framebuffer.borrow_mut(), text, x, y, color);
        Ok(args::unit())
    });

    impure!(m, "text_width", move |text: &str| -> i64 {
        core_gfx::text_width(text)
    });

    let mem = memory.clone();
    impure!(m, "palette", move |ctx: NativeCallContext,
                                index: Dynamic,
                                r: Dynamic,
                                g: Dynamic,
                                b: Dynamic|
          -> RhaiResult<Dynamic> {
        let index = args::to_palette_index(&ctx, &index)?;
        let r = args::to_component(&ctx, &r, "r")?;
        let g = args::to_component(&ctx, &g, "g")?;
        let b = args::to_component(&ctx, &b, "b")?;
        mem.palette.borrow_mut().set_rgb8(index, r, g, b);
        Ok(args::unit())
    });

    let mem = memory.clone();
    impure!(m, "palette_reset", move || -> Dynamic {
        let cart = mem.cart_palette.clone();
        mem.palette.borrow_mut().copy_from(&cart);
        args::unit()
    });

    m
}
