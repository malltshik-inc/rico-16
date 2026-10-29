//! SPEC §9.4 — script errors, reported with the cart name, line and column.

use std::fmt;

/// A cart script error. SPEC §9.4.1 requires the cart file name, line and
/// column; a bare "runtime error" is a spec violation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptError {
    /// The cart file name, as the host supplied it.
    pub cart: String,
    /// 1-based line.
    pub line: u32,
    /// 1-based column.
    pub col: u32,
    /// The Rhai message, including a module path when the error is in a module
    /// (SPEC §9.4.4).
    pub message: String,
}

impl ScriptError {
    /// Build an error with no position, used for load-time checks.
    pub fn load(cart: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            cart: cart.into(),
            line: 0,
            col: 0,
            message: message.into(),
        }
    }
}

impl fmt::Display for ScriptError {
    /// SPEC §14.8 — `<file>:<line>:<col>: <message>`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            write!(f, "{}: {}", self.cart, self.message)
        } else {
            write!(
                f,
                "{}:{}:{}: {}",
                self.cart, self.line, self.col, self.message
            )
        }
    }
}

impl std::error::Error for ScriptError {}
