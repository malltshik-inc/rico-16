//! Shared argument coercion and error helpers for the script modules.
//!
//! Rhai resolves numeric arguments automatically, but silently turns a `NaN`
//! or infinite float into `0` when converting to an integer, which SPEC §6.2
//! forbids. Every integer argument therefore arrives as a `Dynamic` and is
//! coerced here, where the value can be checked before it becomes an int.

use rhai::{Dynamic, EvalAltResult, NativeCallContext};

use rico16_core::limits::TILE_SIZE;

/// Result of a fallible native function: Rhai sees `Err` as a script error.
pub type RhaiResult = Result<Dynamic, Box<EvalAltResult>>;

/// The Rhai unit value used as the return of drawing functions.
pub fn unit() -> Dynamic {
    Dynamic::UNIT
}

/// Build a runtime error positioned at the API call site in the cart.
pub fn error(ctx: &NativeCallContext, message: impl AsRef<str>) -> Box<EvalAltResult> {
    Box::new(EvalAltResult::ErrorRuntime(
        message.as_ref().into(),
        ctx.call_position(),
    ))
}

/// Coerce a Rhai value to `i64`: integers pass through, finite floats truncate
/// toward zero, non-numbers and non-finite floats raise. SPEC §6.2.
pub fn to_int(
    ctx: &NativeCallContext,
    value: &Dynamic,
    arg: &str,
) -> Result<i64, Box<EvalAltResult>> {
    if let Ok(n) = value.as_int() {
        return Ok(n);
    }
    if let Ok(f) = value.as_float() {
        if f.is_finite() {
            return Ok(f.trunc() as i64);
        }
        return Err(error(ctx, format!("{arg} is not a finite number")));
    }
    Err(error(
        ctx,
        format!("{arg} must be a whole number, not {}", value.type_name()),
    ))
}

/// A screen coordinate: any `i64`, clamped to `i32` so extreme values are
/// clipped rather than wrapped. Negative values are legal (SPEC §6.1).
pub fn to_coord(
    ctx: &NativeCallContext,
    value: &Dynamic,
    arg: &str,
) -> Result<i32, Box<EvalAltResult>> {
    Ok(to_int(ctx, value, arg)?.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
}

/// An index (color, sprite id, palette entry): 0–255 or an error. SPEC §6.3.
pub fn to_index(
    ctx: &NativeCallContext,
    value: &Dynamic,
    arg: &str,
) -> Result<u8, Box<EvalAltResult>> {
    let n = to_int(ctx, value, arg)?;
    if !(0..=255).contains(&n) {
        return Err(error(
            ctx,
            format!("{arg} out of range {n} (must be 0-255)"),
        ));
    }
    Ok(n as u8)
}

/// A boolean. Booleans only; integers are not silently accepted for a flag.
pub fn to_bool(
    ctx: &NativeCallContext,
    value: &Dynamic,
    arg: &str,
) -> Result<bool, Box<EvalAltResult>> {
    match value.as_bool() {
        Ok(b) => Ok(b),
        Err(type_name) => Err(error(ctx, format!("{arg} must be a boolean, not {type_name}"))),
    }
}

/// A tile-local pixel coordinate: `0..TILE_SIZE` or an error.
pub fn to_tile_pixel(
    ctx: &NativeCallContext,
    value: &Dynamic,
    arg: &str,
) -> Result<usize, Box<EvalAltResult>> {
    let n = to_int(ctx, value, arg)?;
    if !(0..TILE_SIZE as i64).contains(&n) {
        return Err(error(
            ctx,
            format!("{arg} out of range {n} (must be 0-{})", TILE_SIZE - 1),
        ));
    }
    Ok(n as usize)
}
