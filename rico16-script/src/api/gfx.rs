//! The `gfx::` drawing module. SPEC §9.3, §6.
//!
//! Every function validates colors (0–255, never wrapping) and passes
//! coordinates through clipping. Nothing here keeps state; it only calls into
//! the pure kernels in `rico16-core`.

use rhai::{Dynamic, Engine, Module, NativeCallContext};

use rico16_core::gfx as draw;

use crate::api::impure_fn;
use crate::api::args::{to_coord, to_index, unit, RhaiResult};
use crate::api::StateCell;

/// Register the `gfx::` module on `engine`.
pub fn register(engine: &mut Engine, state: &StateCell) {
    let mut module = Module::new();

    let s = state.clone();
    impure_fn!(module, "clear", move |ctx: NativeCallContext, color: Dynamic| -> RhaiResult {
        let color = to_index(&ctx, &color, "color")?;
        s.framebuffer.borrow_mut().clear(color);
        Ok(unit())
    });

    let s = state.clone();
    impure_fn!(module, "pixel", move |ctx: NativeCallContext, x: Dynamic, y: Dynamic, color: Dynamic| -> RhaiResult {
        let (x, y, color) = (
            to_coord(&ctx, &x, "x")?,
            to_coord(&ctx, &y, "y")?,
            to_index(&ctx, &color, "color")?,
        );
        draw::pixel(&mut s.framebuffer.borrow_mut(), x, y, color);
        Ok(unit())
    });

    let s = state.clone();
    impure_fn!(
        module,
        "line",
        move |ctx: NativeCallContext,
              x0: Dynamic,
              y0: Dynamic,
              x1: Dynamic,
              y1: Dynamic,
              color: Dynamic|
              -> RhaiResult {
            let (x0, y0, x1, y1, color) = (
                to_coord(&ctx, &x0, "x0")?,
                to_coord(&ctx, &y0, "y0")?,
                to_coord(&ctx, &x1, "x1")?,
                to_coord(&ctx, &y1, "y1")?,
                to_index(&ctx, &color, "color")?,
            );
            draw::line(&mut s.framebuffer.borrow_mut(), x0, y0, x1, y1, color);
            Ok(unit())
        }
    );

    let s = state.clone();
    impure_fn!(
        module,
        "rect",
        move |ctx: NativeCallContext,
              x: Dynamic,
              y: Dynamic,
              w: Dynamic,
              h: Dynamic,
              color: Dynamic|
              -> RhaiResult {
            let (x, y, w, h, color) = (
                to_coord(&ctx, &x, "x")?,
                to_coord(&ctx, &y, "y")?,
                to_coord(&ctx, &w, "w")?,
                to_coord(&ctx, &h, "h")?,
                to_index(&ctx, &color, "color")?,
            );
            draw::rect(&mut s.framebuffer.borrow_mut(), x, y, w, h, color);
            Ok(unit())
        }
    );

    let s = state.clone();
    impure_fn!(
        module,
        "rect_filled",
        move |ctx: NativeCallContext,
              x: Dynamic,
              y: Dynamic,
              w: Dynamic,
              h: Dynamic,
              color: Dynamic|
              -> RhaiResult {
            let (x, y, w, h, color) = (
                to_coord(&ctx, &x, "x")?,
                to_coord(&ctx, &y, "y")?,
                to_coord(&ctx, &w, "w")?,
                to_coord(&ctx, &h, "h")?,
                to_index(&ctx, &color, "color")?,
            );
            draw::rect_filled(&mut s.framebuffer.borrow_mut(), x, y, w, h, color);
            Ok(unit())
        }
    );

    let s = state.clone();
    impure_fn!(
        module,
        "circle",
        move |ctx: NativeCallContext, x: Dynamic, y: Dynamic, r: Dynamic, color: Dynamic| -> RhaiResult {
            let (x, y, r, color) = (
                to_coord(&ctx, &x, "x")?,
                to_coord(&ctx, &y, "y")?,
                to_coord(&ctx, &r, "r")?,
                to_index(&ctx, &color, "color")?,
            );
            draw::circle(&mut s.framebuffer.borrow_mut(), x, y, r, color);
            Ok(unit())
        }
    );

    let s = state.clone();
    impure_fn!(
        module,
        "circle_filled",
        move |ctx: NativeCallContext, x: Dynamic, y: Dynamic, r: Dynamic, color: Dynamic| -> RhaiResult {
            let (x, y, r, color) = (
                to_coord(&ctx, &x, "x")?,
                to_coord(&ctx, &y, "y")?,
                to_coord(&ctx, &r, "r")?,
                to_index(&ctx, &color, "color")?,
            );
            draw::circle_filled(&mut s.framebuffer.borrow_mut(), x, y, r, color);
            Ok(unit())
        }
    );

    let s = state.clone();
    impure_fn!(
        module,
        "text",
        move |ctx: NativeCallContext, text: &str, x: Dynamic, y: Dynamic, color: Dynamic| -> RhaiResult {
            let (x, y, color) = (
                to_coord(&ctx, &x, "x")?,
                to_coord(&ctx, &y, "y")?,
                to_index(&ctx, &color, "color")?,
            );
            draw::text(&mut s.framebuffer.borrow_mut(), text, x, y, color);
            Ok(unit())
        }
    );

    impure_fn!(module, "text_width", |text: &str| -> i64 {
        draw::text_width(text).into()
    });

    let s = state.clone();
    impure_fn!(
        module,
        "palette",
        move |ctx: NativeCallContext,
              index: Dynamic,
              r: Dynamic,
              g: Dynamic,
              b: Dynamic|
              -> RhaiResult {
            let (index, r, g, b) = (
                to_index(&ctx, &index, "index")?,
                to_index(&ctx, &r, "r")?,
                to_index(&ctx, &g, "g")?,
                to_index(&ctx, &b, "b")?,
            );
            s.palette.borrow_mut().set_rgb8(index, r, g, b);
            Ok(unit())
        }
    );

    let s = state.clone();
    impure_fn!(module, "palette_reset", move || {
        let mut palette = s.palette.borrow_mut();
        *palette = s.cart_palette.clone();
        unit()
    });

    engine.register_static_module("gfx", module.into());
}
