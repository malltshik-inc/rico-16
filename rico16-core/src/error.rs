//! Error types for `rico16-core`.
//!
//! Core errors are about the fixed geometry of the console and the two script
//! conditions the spec pins to core data (SPEC §5.5, §6.9). Script-visible
//! errors carry the cart name, line and column at the script layer (SPEC §9.4.1).

use std::fmt;

/// A condition core can detect that the spec requires to be reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreError {
    /// A fixed-size resource was not the exact size the console fixes (SPEC §7).
    WrongLength {
        /// Which resource, for the message.
        what: &'static str,
        /// The size the spec fixes.
        expected: usize,
        /// The size actually supplied.
        found: usize,
    },
    /// SPEC §6.9 — a `sprite::draw_region` source rectangle leaves the sheet.
    RegionOutOfSheet,
    /// SPEC §5.5 — `rnd::int(lo, hi)` with `lo > hi`.
    EmptyRange {
        /// The lower bound as given.
        lo: i64,
        /// The upper bound as given.
        hi: i64,
    },
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::WrongLength {
                what,
                expected,
                found,
            } => write!(f, "{what} is {found} bytes, must be exactly {expected}"),
            CoreError::RegionOutOfSheet => {
                write!(f, "source rectangle extends outside the sprite sheet")
            }
            CoreError::EmptyRange { lo, hi } => {
                write!(f, "rnd::int range is empty: lo {lo} > hi {hi}")
            }
        }
    }
}

impl std::error::Error for CoreError {}
