//! The `sprite::` module. SPEC §9.3, §8.5.
//!
//! Sprites are 16×16 windows into the sheet. Drawing blits with the index-0
//! key and per-call flipping; pixel access reads and writes the sheet data
//! model directly.

use rhai::{Dynamic, Engine, Module, NativeCallContext};

use rico16_core::blit::{self, Blit};
use rico16_core::limits::{SHEET_HEIGHT, SHEET_WIDTH};
use rico16_core::sheet;

use crate::api::impure_fn;
use crate::api::args::{to_bool, to_coord, to_index, to_tile_pixel, unit, RhaiResult};
use crate::api::StateCell;

/// Register the `sprite::` module on `engine`.
pub fn register(engine: &mut Engine, state: &StateCell) {
    let mut module = Module::new();

    let s = state.clone();
    impure_fn!(module, "draw", move |ctx: NativeCallContext, id: Dynamic, x: Dynamic, y: Dynamic| -> RhaiResult {
        let (id, x, y) = (
            to_index(&ctx, &id, "id")?,
            to_coord(&ctx, &x, "x")?,
            to_coord(&ctx, &y, "y")?,
        );
        let framebuffer = &mut s.framebuffer.borrow_mut();
        let sheet = s.sheet.borrow();
        blit::sprite(framebuffer, &sheet, id, x, y, false, false);
        Ok(unit())
    });

    let s = state.clone();
    impure_fn!(
        module,
        "draw",
        move |ctx: NativeCallContext,
              id: Dynamic,
              x: Dynamic,
              y: Dynamic,
              flip_x: Dynamic,
              flip_y: Dynamic|
              -> RhaiResult {
            let (id, x, y, flip_x, flip_y) = (
                to_index(&ctx, &id, "id")?,
                to_coord(&ctx, &x, "x")?,
                to_coord(&ctx, &y, "y")?,
                to_bool(&ctx, &flip_x, "flip_x")?,
                to_bool(&ctx, &flip_y, "flip_y")?,
            );
            let framebuffer = &mut s.framebuffer.borrow_mut();
            let sheet = s.sheet.borrow();
            blit::sprite(framebuffer, &sheet, id, x, y, flip_x, flip_y);
            Ok(unit())
        }
    );

    let s = state.clone();
    impure_fn!(
        module,
        "draw_region",
        move |ctx: NativeCallContext,
              sx: Dynamic,
              sy: Dynamic,
              w: Dynamic,
              h: Dynamic,
              dx: Dynamic,
              dy: Dynamic,
              flip_x: Dynamic,
              flip_y: Dynamic|
              -> RhaiResult {
            let (sx, sy, w, h, dx, dy, flip_x, flip_y) = (
                to_coord(&ctx, &sx, "sx")?,
                to_coord(&ctx, &sy, "sy")?,
                to_coord(&ctx, &w, "w")?,
                to_coord(&ctx, &h, "h")?,
                to_coord(&ctx, &dx, "dx")?,
                to_coord(&ctx, &dy, "dy")?,
                to_bool(&ctx, &flip_x, "flip_x")?,
                to_bool(&ctx, &flip_y, "flip_y")?,
            );
            let blit = Blit::new(sx, sy, w, h, dx, dy).flipped(flip_x, flip_y);
            let framebuffer = &mut s.framebuffer.borrow_mut();
            let sheet = s.sheet.borrow();
            blit::region(framebuffer, &sheet, SHEET_WIDTH, SHEET_HEIGHT, blit);
            Ok(unit())
        }
    );

    let s = state.clone();
    impure_fn!(
        module,
        "get_pixel",
        move |ctx: NativeCallContext, id: Dynamic, px: Dynamic, py: Dynamic| -> RhaiResult {
            let (id, px, py) = (
                to_index(&ctx, &id, "id")?,
                to_tile_pixel(&ctx, &px, "px")?,
                to_tile_pixel(&ctx, &py, "py")?,
            );
            // The coordinates were validated above; `None` is unreachable.
            let value = sheet::get_pixel(&s.sheet.borrow(), id, px, py).unwrap_or(0);
            Ok(Dynamic::from(value as i64))
        }
    );

    let s = state.clone();
    impure_fn!(
        module,
        "set_pixel",
        move |ctx: NativeCallContext,
              id: Dynamic,
              px: Dynamic,
              py: Dynamic,
              color: Dynamic|
              -> RhaiResult {
            let (id, px, py, color) = (
                to_index(&ctx, &id, "id")?,
                to_tile_pixel(&ctx, &px, "px")?,
                to_tile_pixel(&ctx, &py, "py")?,
                to_index(&ctx, &color, "color")?,
            );
            sheet::set_pixel(&mut s.sheet.borrow_mut(), id, px, py, color);
            Ok(unit())
        }
    );

    engine.register_static_module("sprite", module.into());
}
