//! SPEC §5.5 — the deterministic RNG: xoshiro256** seeded by SplitMix64.
//!
//! This algorithm is part of the spec. Never swap it, never seed it from the
//! OS, never add an "unseeded" path (AGENTS.md).

use crate::error::CoreError;

/// `2^-53`, exact in binary64.
const TWO_POW_M53: f64 = 1.0 / 9_007_199_254_740_992.0;

/// SPEC §5.5 — SplitMix64 streaming step.
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// SPEC §5.5 — seed the four-word state from one `u64`.
fn seed_xoshiro256(seed: u64) -> [u64; 4] {
    let mut sm = seed;
    let s = [
        splitmix64(&mut sm),
        splitmix64(&mut sm),
        splitmix64(&mut sm),
        splitmix64(&mut sm),
    ];
    if s == [0, 0, 0, 0] { [1, 0, 0, 0] } else { s }
}

/// SPEC §5.5 — the single RNG stream a cart run owns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rng {
    state: [u64; 4],
}

impl Default for Rng {
    fn default() -> Self {
        Self::new()
    }
}

impl Rng {
    /// SPEC §5.5 — a stream from the default seed 0.
    pub fn new() -> Self {
        Self::seed(0)
    }

    /// SPEC §5.5 — a stream from `seed`, via canonical SplitMix64 streaming.
    pub fn seed(seed: u64) -> Self {
        Self {
            state: seed_xoshiro256(seed),
        }
    }

    /// The four state words, for conformance tests (SPEC §20.1).
    pub fn state(&self) -> [u64; 4] {
        self.state
    }

    /// SPEC §5.5 — one xoshiro256** step (Vigna 2020).
    pub fn next_u64(&mut self) -> u64 {
        let result = self.state[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.state[1] << 17;
        self.state[2] ^= self.state[0];
        self.state[3] ^= self.state[1];
        self.state[1] ^= self.state[2];
        self.state[0] ^= self.state[3];
        self.state[2] ^= t;
        self.state[3] = self.state[3].rotate_left(45);
        result
    }

    /// SPEC §5.5 — `rnd::float`: `(x >> 11) as f64 * 2^-53`, in `[0, 1)`.
    pub fn float(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * TWO_POW_M53
    }

    /// SPEC §5.5 — `rnd::int(lo, hi)`, both ends inclusive.
    pub fn int(&mut self, lo: i64, hi: i64) -> Result<i64, CoreError> {
        if lo > hi {
            return Err(CoreError::EmptyRange { lo, hi });
        }
        let count = (hi as i128) - (lo as i128) + 1;
        if count == (1i128 << 64) {
            // The full i64 range: one draw, bit-cast.
            return Ok(self.next_u64() as i64);
        }
        let offset = self.bounded(count as u64);
        Ok((lo as i128 + offset as i128) as i64)
    }

    /// SPEC §5.5 — Lemire's nearly-divisionless bounded draw, uniform in
    /// `0..range` (exclusive). `range` must be ≥ 1.
    pub fn bounded(&mut self, range: u64) -> u64 {
        debug_assert!(range >= 1);
        let mut x = self.next_u64();
        let mut m = (x as u128) * (range as u128);
        let mut lo = m as u64;
        if lo < range {
            let threshold = ((1u128 << 64) % (range as u128)) as u64;
            while lo < threshold {
                x = self.next_u64();
                m = (x as u128) * (range as u128);
                lo = m as u64;
            }
        }
        (m >> 64) as u64
    }

    /// SPEC §5.5 — `rnd::choice` index: `None` for an empty array, and no draw.
    pub fn choice_index(&mut self, len: usize) -> Option<usize> {
        if len == 0 {
            return None;
        }
        Some(self.bounded(len as u64) as usize)
    }

    /// SPEC §5.5 — `rnd::shuffle`: Fisher–Yates from the end. Draws nothing
    /// for `n <= 1`.
    pub fn shuffle<T>(&mut self, arr: &mut [T]) {
        let n = arr.len();
        if n <= 1 {
            return;
        }
        for i in (1..n).rev() {
            let j = self.bounded(i as u64 + 1) as usize;
            arr.swap(i, j);
        }
    }
}
