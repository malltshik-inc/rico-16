//! SPEC §6.2, §6.3, §5.6 — argument conversion for the script API.
//!
//! Integer arguments accept integers and floats converted by **floor**;
//! `NaN`, infinite and out-of-`i64`-range floats are script errors. Colour
//! arguments must be 0–255 and never wrap.

use rhai::{Dynamic, EvalAltResult, NativeCallContext};

/// The result type every native function returns.
pub type RhaiResult<T> = Result<T, Box<EvalAltResult>>;

/// `-2^63` as an `f64`, exactly representable.
const I64_MIN_F: f64 = -9_223_372_036_854_775_808.0;
/// `2^63` as an `f64`: the first value above `i64::MAX`.
const I64_MAX_EXCLUSIVE_F: f64 = 9_223_372_036_854_775_808.0;

/// SPEC §5.6 — the canonical quiet NaN bit pattern.
pub const CANONICAL_NAN: u64 = 0x7FF8_0000_0000_0000;

/// SPEC §5.6 — map any NaN to the canonical quiet NaN.
pub fn canonical_float(v: f64) -> f64 {
    if v.is_nan() {
        f64::from_bits(CANONICAL_NAN)
    } else {
        v
    }
}

/// A script error at the call site.
pub fn error(ctx: &NativeCallContext, message: impl Into<String>) -> Box<EvalAltResult> {
    Box::new(EvalAltResult::ErrorRuntime(
        Dynamic::from(message.into()),
        ctx.call_position(),
    ))
}

/// `()` as a dynamic.
pub fn unit() -> Dynamic {
    Dynamic::UNIT
}

/// SPEC §6.2 — coerce an argument to an integer: integers pass through, floats
/// are floored, and `NaN`/infinite/out-of-range floats are errors.
pub fn to_int(ctx: &NativeCallContext, value: &Dynamic, what: &str) -> RhaiResult<i64> {
    if let Some(i) = value.clone().try_cast::<i64>() {
        return Ok(i);
    }
    if let Some(f) = value.clone().try_cast::<f64>() {
        if f.is_nan() || f.is_infinite() {
            return Err(error(ctx, format!("{what} must be an integer, got {f}")));
        }
        let floored = f.floor();
        if !(I64_MIN_F..I64_MAX_EXCLUSIVE_F).contains(&floored) {
            return Err(error(ctx, format!("{what} is out of integer range: {f}")));
        }
        return Ok(floored as i64);
    }
    Err(error(ctx, format!("{what} must be an integer")))
}

/// SPEC §6.3 — a colour argument must be an integer 0–255 and never wraps.
pub fn to_color(ctx: &NativeCallContext, value: &Dynamic, what: &str) -> RhaiResult<u8> {
    let n = to_int(ctx, value, what)?;
    if !(0..=255).contains(&n) {
        return Err(error(ctx, format!("{what} must be 0–255, got {n}")));
    }
    Ok(n as u8)
}

/// A sprite id or tile id: an integer 0–255.
pub fn to_id(ctx: &NativeCallContext, value: &Dynamic, what: &str) -> RhaiResult<u8> {
    to_color(ctx, value, what)
}

/// A palette index: an integer 0–255.
pub fn to_palette_index(ctx: &NativeCallContext, value: &Dynamic) -> RhaiResult<usize> {
    Ok(to_color(ctx, value, "palette index")? as usize)
}

/// SPEC §6.9 — a sprite pixel coordinate: an integer 0–15.
pub fn to_tile_pixel(ctx: &NativeCallContext, value: &Dynamic, what: &str) -> RhaiResult<u8> {
    let n = to_int(ctx, value, what)?;
    if !(0..=15).contains(&n) {
        return Err(error(ctx, format!("{what} must be 0–15, got {n}")));
    }
    Ok(n as u8)
}

/// A colour component for `gfx::palette`: an integer 0–255.
pub fn to_component(ctx: &NativeCallContext, value: &Dynamic, what: &str) -> RhaiResult<u8> {
    to_color(ctx, value, what)
}
