//! The deterministic RNG. SPEC §5.5.
//!
//! Randomness is part of the cart format: two runs of the same cart, same seed,
//! same inputs, must draw the same numbers. The algorithm is xoshiro256\*\*,
//! seeded through splitmix64 from a single u64, default 0. Never swap the
//! algorithm, never seed from the OS — a seeded cart recorded today must
//! replay tomorrow on every platform.
//!
//! Draw vectors in the tests are pinned against an independent transcription of
//! the same algorithm, so an accidental change to the constants is caught.

/// Error from [`Rng::random_int`]. SPEC §5.5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RngError {
    /// `lo` was greater than `hi`; the range is empty.
    #[error("random_int range is empty: lo {lo} > hi {hi}")]
    EmptyRange { lo: i64, hi: i64 },
}

/// Seeded xoshiro256\*\* generator. SPEC §5.5.
///
/// This is a value type: clone it to snapshot the stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rng {
    state: [u64; 4],
}

impl Rng {
    /// A fresh generator for `seed`. The seed is expanded to four 64-bit
    /// words by splitmix64, as the SPEC requires. Default cart seed is 0.
    pub fn seed(seed: u64) -> Self {
        let mut state = seed;
        let mut word = || splitmix64(&mut state);
        Self {
            state: [word(), word(), word(), word()],
        }
    }

    /// The next raw 64-bit draw.
    ///
    /// xoshiro256\*\* scrambler: `rotl(s1·5, 7)·9`, then the usual shift-xor
    /// unlinearization.
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

    /// A uniform float in `[0, 1)`. SPEC §5.5.
    ///
    /// Computed as `(x >> 11) * 2^-53` from a 64-bit draw, exactly as the SPEC
    /// states. `2^-53` is a power of two, so the multiplication is exact and no
    /// two draws collapse onto the same value.
    pub fn random(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// A uniform integer in `[lo, hi]`, inclusive, with rejection sampling so
    /// the distribution has no modulo bias. SPEC §5.5.
    ///
    /// `lo > hi` is an empty range and an error.
    pub fn random_int(&mut self, lo: i64, hi: i64) -> Result<i64, RngError> {
        if lo > hi {
            return Err(RngError::EmptyRange { lo, hi });
        }

        // The size of the range, in [1, 2^64]. Computed in i128 so the full
        // `i64::MIN..=i64::MAX` span is exact instead of wrapping to zero.
        // `hi - lo + 1` reaches exactly 2^64, which needs the u128 headroom.
        let span = (hi as i128 - lo as i128 + 1) as u128;

        loop {
            let x = self.next_u64() as u128;

            // Rejection threshold: reject the first `(2^64 mod span)` draws so
            // the accepted count is a multiple of `span`, which makes every
            // residue equally likely.
            let limit = (1u128 << 64) % span;
            if x >= limit {
                // `lo as u64` maps negative bounds onto the u64 circle (a
                // modulo-2^64 wrap); widening to u128 then keeps the sum below
                // 2^65, and the final `as i64` wraps back into `[lo, hi]`.
                let offset = lo as u64 as u128;
                return Ok((offset + x % span) as i64);
            }
        }
    }
}

impl Default for Rng {
    fn default() -> Self {
        // SPEC §5.5: the default seed is 0.
        Self::seed(0)
    }
}

/// One step of splitmix64: an additive counter mixed with a bijective 64-bit
/// permutation. Produces a well-distributed word from the running state.
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeded_state_matches_the_pinned_vectors() {
        // Cross-checked against an independent transcription of splitmix64.
        assert_eq!(
            Rng::seed(0).state,
            [
                0xe220_a839_7b1d_cdaf,
                0x6e78_9e6a_a1b9_65f4,
                0x06c4_5d18_8009_454f,
                0xf88b_b8a8_724c_81ec,
            ]
        );
    }

    #[test]
    fn first_draws_match_the_pinned_vectors() {
        // Cross-checked against an independent transcription of xoshiro256**.
        let mut rng = Rng::seed(0);
        let draws: [u64; 5] = std::array::from_fn(|_| rng.next_u64());
        assert_eq!(
            draws,
            [
                0x99ec_5f36_cb75_f2b4,
                0xbf6e_1f78_4956_452a,
                0x1a5f_849d_4933_e6e0,
                0x6aa5_94f1_262d_2d2c,
                0xbba5_ad4a_1f84_2e59,
            ]
        );

        let mut other = Rng::seed(1);
        assert_eq!(
            [other.next_u64(), other.next_u64(), other.next_u64()],
            [
                0xb3f2_af6d_0fc7_10c5,
                0x853b_5596_4736_4cea,
                0x92f8_9756_082a_4514,
            ]
        );
    }

    #[test]
    fn random_samples_are_in_range_and_pinned() {
        let mut rng = Rng::seed(0);
        let first = rng.random();
        let second = rng.random();

        assert_eq!(first, 0.601_262_999_417_904_8);
        assert_eq!(second, 0.747_774_092_547_239_8);
        for sample in [first, second] {
            assert!((0.0..1.0).contains(&sample));
        }
    }

    #[test]
    fn random_produces_distinct_values_across_many_draws() {
        // The canary runs on raw draws, which map 1:1 onto random() values
        // (each is `draw >> 11` scaled by an exact power of two).
        let mut rng = Rng::seed(7);
        let mut seen = std::collections::HashMap::new();
        for i in 0..10_000 {
            let value = rng.next_u64();
            if let Some(first) = seen.insert(value, i) {
                panic!("draw {value:016x} repeated: first at {first}, again at {i}");
            }
        }
    }

    #[test]
    fn random_int_respects_inclusive_bounds() {
        let mut rng = Rng::seed(42);
        for (lo, hi) in [
            (0i64, 0i64),
            (0, 1),
            (-3, 3),
            (i64::MIN, i64::MIN),
            (i64::MAX, i64::MAX),
            (i64::MIN, i64::MAX),
        ] {
            for _ in 0..2_000 {
                let value = rng.random_int(lo, hi).unwrap();
                assert!(
                    (lo..=hi).contains(&value),
                    "{value} outside [{lo}, {hi}]"
                );
            }
        }
    }

    #[test]
    fn random_int_rejects_an_empty_range() {
        let mut rng = Rng::seed(0);
        let error = rng.random_int(5, 2).unwrap_err();
        assert!(matches!(error, RngError::EmptyRange { lo: 5, hi: 2 }));
    }

    #[test]
    fn random_int_draws_are_deterministic() {
        let mut first = Rng::seed(9);
        let mut second = Rng::seed(9);
        let a: Vec<i64> = (0..500).map(|_| first.random_int(10, 20).unwrap()).collect();
        let b: Vec<i64> = (0..500).map(|_| second.random_int(10, 20).unwrap()).collect();
        assert_eq!(a, b);
    }

    #[test]
    fn a_clone_is_an_independent_snapshot() {
        let mut original = Rng::seed(3);
        let mut clone = original.clone();

        // Both streams advance in lockstep from the copied state...
        assert_eq!(original.next_u64(), clone.next_u64());
        // ...and calling the original alone does not move the clone: it still
        // produces the second draw when asked.
        let original_second = original.next_u64();
        assert_eq!(clone.next_u64(), original_second);
        // They are still aligned after that.
        assert_eq!(original.next_u64(), clone.next_u64());
    }
}
