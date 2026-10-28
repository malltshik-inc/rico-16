//! Typed script errors with the cart name, line and column. SPEC §9.4.1.

use std::fmt;

use rhai::EvalAltResult;

/// A script failure, already positioned inside the cart's source.
///
/// Every error MUST carry the cart name, a line and a column plus the Rhai
/// message (SPEC §9.4.1). Reporting a bare `Runtime error` is a spec violation,
/// so the host prints this type's `Display` directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptError {
    /// Cart file name as given by the host.
    pub cart: String,
    /// 1-based line.
    pub line: u32,
    /// 1-based column.
    pub col: u32,
    /// The Rhai message, e.g. `color out of range 300 (must be 0-255)`.
    pub message: String,
}

impl ScriptError {
    /// An error with no meaningful source position (e.g. a build-time check).
    pub fn field(cart: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            cart: cart.into(),
            line: 1,
            col: 1,
            message: message.into(),
        }
    }

    /// Map a Rhai evaluation error onto the innermost failing expression.
    ///
    /// Rhai wraps script errors in `ErrorInFunctionCall` layers; `unwrap_inner`
    /// drills down to the actual failing call, whose position is where the cart
    /// should look.
    pub fn from_rhai(cart: impl Into<String>, error: &EvalAltResult) -> Self {
        let base = error.unwrap_inner();
        let position = base.position();
        Self {
            cart: cart.into(),
            line: position.line().unwrap_or(1) as u32,
            col: position.position().map(|col| col as u32).unwrap_or(1),
            message: base.to_string(),
        }
    }
}

impl fmt::Display for ScriptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}:{}:{}: {}",
            self.cart, self.line, self.col, self.message
        )
    }
}

impl std::error::Error for ScriptError {}
