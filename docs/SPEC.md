# rico-16 Specification

**Version:** 3.0 (draft)
**Status:** normative. If code and this document disagree, the code is wrong unless a changelog entry below says otherwise.

---

## Contents

1. [Scope](#1-scope)
2. [Conformance language](#2-conformance-language)
3. [Display and framebuffer](#3-display-and-framebuffer)
4. [Palette and color](#4-palette-and-color)
5. [Timing, the game loop, determinism](#5-timing-the-game-loop-determinism)
6. [Coordinates, clipping, drawing rules](#6-coordinates-clipping-drawing-rules)
7. [Resources and limits](#7-resources-and-limits)
8. [Drawing model](#8-drawing-model)
9. [Script API](#9-script-api)
10. [Project structure and cart.toml](#10-project-structure-and-carttoml)
11. [Module system](#11-module-system)
12. [Assets and build pipeline](#12-assets-and-build-pipeline)
13. [Cart binary format and PNG packing](#13-cart-binary-format-and-png-packing)
14. [Runtime and player](#14-runtime-and-player)
15. [Commands](#15-commands)
16. [Editor](#16-editor)
17. [Versioning and compatibility](#17-versioning-and-compatibility)
18. [Deferred features and non-goals](#18-deferred-features-and-non-goals)
19. [Decision log](#19-decision-log)
20. [Changelog](#20-changelog)
21. [Conformance vectors](#21-conformance-vectors)

---

## 1. Scope

rico-16 is a fantasy console: a fixed virtual machine with fixed resources, a fixed palette-driven display and an embedded scripting language. This document defines the observable behaviour of the console — what a cart may rely on and what a conforming runtime must do.

It does **not** define internal implementation, crate layout actualities, or the editor's user interface beyond the requirements in [§16](#16-editor).

The three things that make rico-16 a console rather than a game framework:

1. The display is a fixed 320×240 indexed framebuffer. There is exactly one surface.
2. Resources are strictly bounded and known at load time.
3. The script API is complete enough that a cart never needs a second language, a plugin, or access to the host.

---

## 2. Conformance language

The key words **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT** and **MAY** are to be interpreted as described in RFC 2119.

A **conforming runtime** is a program that loads a cart (project or packed PNG) and satisfies every MUST in this document. A **conforming project** is a directory structure with `cart.toml` that satisfies every MUST in [§10](#10-project-structure-and-carttoml).

PU (pixel units) are integer screen pixels. RGB555 words are 16-bit little-endian unless stated otherwise.

---

## 3. Display and framebuffer

3.1. The display is **320 PU wide and 240 PU tall**. The origin `(0, 0)` is the top-left pixel, `+x` is right, `+y` is down.

3.2. The framebuffer is a single array of **76,800 bytes**, one byte per pixel, row-major, stride 320. Byte values are palette indices, not colors.

3.3. There is exactly one surface and no compositing. Sprites and the tilemap are *data*, not layers ([§8](#8-drawing-model)).

3.4. The framebuffer is **not** cleared automatically between frames. A cart that does not call `gfx::clear` sees the previous frame's contents.

3.5. At cart start the framebuffer is all zeros (index 0).

3.6. The framebuffer is expanded through the palette to RGBA8 at presentation time. Expansion is exact: `rgba = ((r5 << 3) | (r5 >> 2), ...)`, with alpha 255. There is no gamma correction, no color management, and no dithering applied by the runtime.

3.7. Presentation is **nearest-neighbour** with **integer scaling only**, letterboxed with opaque black. A conforming runtime MUST NOT use bilinear or any other filtering. The default window is 3× (960×720); the actual scale is the largest integer that fits the drawable area, minimum 1.

3.8. Curved-screen CRT effects, scanlines, aspect correction and similar are out of scope for the runtime. Carts that want them draw them.

---

## 4. Palette and color

4.1. The palette has **256 entries**. Each entry is an **RGB555** word: bits 0–4 red, bits 5–9 green, bits 10–14 blue, bit 15 reserved and MUST be 0. Component values are 0–31. Total palette size is **512 bytes**.

4.2. **Index 0 is the transparency key** for all blitting operations (sprite and tilemap draws). It is not transparent on the display: after expansion, index 0 shows `palette[0]`. The default palette MUST have `palette[0]` = opaque black.

4.3. Primitive drawing writes its color argument literally, including 0. This is the only way to erase: `gfx::rect_filled` with color 0 fills with index 0, which then displays as `palette[0]`. There is no "background color" concept.

4.4. `gfx::palette(index, r, g, b)` takes 8-bit components and quantizes with `round(v * 31 / 255)`, equivalently `(v * 31 + 127) / 255` in integer arithmetic. Out-of-range components MUST raise a script error.

4.5. Palette mutation is runtime state. It resets when the cart restarts. It is never written back to the cart file.

4.6. There is **no alpha blending** anywhere. Fades, shadows and transparency effects are done by (a) mutating the palette, or (b) the index-0 key, or (c) dithering drawn by the cart.

4.7. **A cart author never has to choose 256 colors.** The console ships a default palette, a new cart starts with it, and most carts never touch it. The palette is versioned ([§17](#17-versioning-and-compatibility)) and identical on every platform, and it has a fixed structure so that indices are portable between carts:

- **0** — black, the transparency key.
- **1–15** — the fixed base colours. Their meanings are part of the spec, so that a cart, a tutorial and a generated file agree. Indices 1–5 are the grey ramp; 6–15 are the ten hues, in this order:

  | Index | Colour | Index | Colour |
  |---|---|---|---|
  | 1 | darkest grey | 9 | lime |
  | 2 | dark grey | 10 | green |
  | 3 | mid grey | 11 | teal |
  | 4 | light grey | 12 | cyan |
  | 5 | white | 13 | blue |
  | 6 | red | 14 | violet |
  | 7 | orange | 15 | magenta |
  | 8 | yellow | | |

- **16–255** — fifteen ramps of sixteen steps. Each ramp is one hue, dark to light; value rises and saturation falls slightly along the ramp, so shadows stay rich and highlights stay chromatic instead of washing out to white. Ramps are the shading primitive of pixel art: pick a ramp, pick a step, and you have a coherent colour story in five minutes.

The table is the **normative data**. The recipe that produced it is informative only:

- Index 0 is black.
- Indices 1–5 are a grey ramp at values 0.20, 0.35, 0.50, 0.70, 1.00.
- Indices 6–15 are ten named hues at saturation 0.90, value 1.00, hues in degrees: red 0, orange 24, yellow 50, lime 90, green 130, teal 165, cyan 190, blue 230, violet 270, magenta 310.
- Indices 16–255 are fifteen ramps of sixteen steps. Ramp `i` (`0..=14`) has hue `i * 24` degrees. Step `t = step/15` runs value `0.15 → 1.00` and saturation `0.85 → 0.55`.
- HSV→RGB is the usual sector formula. A channel is `floor(x * 31 + 0.5)` on the real value in `[0, 1]`.

If that recipe and the table disagree, the table wins.

| Index | Name | Index | Name |
|---|---|---|---|
| 0 | black, transparency key | 8 | yellow |
| 1 | darkest grey | 9 | lime |
| 2 | dark grey | 10 | green |
| 3 | mid grey | 11 | teal |
| 4 | light grey | 12 | cyan |
| 5 | white | 13 | blue |
| 6 | red | 14 | violet |
| 7 | orange | 15 | magenta |

Each cell below is the RGB555 word, hex, little-endian on disk.

| Indices | Words |
|---|---|
| 0–7 | `0000` `18C6` `2D6B` `4210` `5AD6` `7FFF` `0C7F` `0DDF` |
| 8–15 | `0F5F` `0FF1` `23E3` `63E3` `7F43` `7D03` `7C71` `687F` |
| 16–23 | `0425` `0426` `0848` `084A` `0C6C` `0C6D` `108F` `14B1` |
| 24–31 | `18D3` `1CF4` `2116` `2538` `295A` `2D7B` `35BD` `39DF` |
| 32–39 | `0445` `0466` `0888` `08AA` `0CCC` `0CED` `112F` `1551` |
| 40–47 | `1973` `1D94` `21D6` `25F8` `2A1A` `2E5B` `367D` `3ABF` |
| 48–55 | `0485` `04A6` `08E8` `090A` `0D4C` `0D6D` `11AF` `15F1` |
| 56–63 | `1A13` `1E54` `2276` `26B8` `2AFA` `2F1B` `375D` `3B9F` |
| 64–71 | `04A4` `04C5` `0907` `0948` `0D8A` `0DAB` `11ED` `162F` |
| 72–79 | `1A70` `1E92` `22D3` `2715` `2B57` `2F78` `37BA` `3BFC` |
| 80–87 | `04A2` `04C3` `0904` `0945` `0D86` `0DA7` `11E9` `162A` |
| 88–95 | `1A6B` `1E8C` `22CE` `270F` `2B50` `2F72` `37B3` `3BF5` |
| 96–103 | `04A1` `04C1` `0902` `0942` `0D83` `0DA3` `11E4` `1625` |
| 104–111 | `1A66` `1E87` `22C8` `2709` `2B4A` `2F6B` `37AD` `3BEE` |
| 112–119 | `08A1` `0CC1` `1102` `1542` `1983` `1DA3` `25E4` `2A25` |
| 120–127 | `2E66` `3287` `3AC8` `3F09` `434A` `4B6B` `4FAD` `57EE` |
| 128–135 | `10A1` `14C1` `1D02` `2142` `2983` `2DA3` `35E4` `3E25` |
| 136–143 | `4266` `4A87` `4EC8` `5709` `5F4A` `636B` `6BAD` `73EE` |
| 144–151 | `1481` `18A1` `20E2` `2902` `3143` `3563` `3DA4` `45E5` |
| 152–159 | `4E06` `5247` `5A68` `62A9` `6AEA` `6F0B` `774D` `7F8E` |
| 160–167 | `1441` `1861` `2082` `28A2` `30C3` `34E3` `3D24` `4545` |
| 168–175 | `4D66` `5187` `59C8` `61E9` `6A0A` `6E4B` `766D` `7EAE` |
| 176–183 | `1421` `1821` `2042` `2842` `3063` `3463` `3C84` `44A5` |
| 184–191 | `4CC6` `50E7` `5908` `6129` `694A` `6D6B` `75AD` `7DCE` |
| 192–199 | `1422` `1823` `2044` `2845` `3066` `3467` `3C89` `44AA` |
| 200–207 | `4CCB` `50EC` `590E` `612F` `6950` `6D72` `75B3` `7DD5` |
| 208–215 | `1424` `1825` `2047` `2848` `306A` `346B` `3C8D` `44AF` |
| 216–223 | `4CD0` `50F2` `5913` `6135` `6957` `6D78` `75BA` `7DDC` |
| 224–231 | `1025` `1426` `1C48` `204A` `286C` `2C6D` `348F` `3CB1` |
| 232–239 | `40D3` `48F4` `4D16` `5538` `5D5A` `617B` `69BD` `71DF` |
| 240–247 | `0825` `0C26` `1048` `144A` `186C` `1C6D` `248F` `28B1` |
| 248–255 | `2CD3` `30F4` `3916` `3D38` `415A` `497B` `4DBD` `55DF` |

Index `16 + i*16 + step` is ramp `i`, step `step` (`0` darkest, `15` lightest).

### 4.7. Why the palette is shaped this way

4.9. The build process resolves the effective palette into the cart at pack time, so a packed cart is unaffected by later changes to the default palette.

4.10. True color is not supported and MUST NOT be added without a major version bump. It is a deliberate non-goal, not a missing feature.

4.11. The console ships one **named palette preset**, `rgb332`, in addition to [`DEFAULT`](#4-palette-and-color): index `0bRRRGGGBB` is literally that colour, three bits of red, three of green, two of blue. It exists because some carts would rather read a colour off an index than walk a ramp, and because every attempt at direct-colour framebuffers eventually proposes it. It is a preset, not a colour model: the framebuffer stays indexed, so a cart using `rgb332` still gets palette swaps and still gets index 0 as a transparent key. It is not the default because two bits of blue means four blue levels.

`rico16 new <name> --palette rgb332` bakes the preset into the cart, after which it is ordinary palette data and the runtime cannot tell the difference.

4.12. **Default palette values (normative):** The exact 256 RGB555 words are defined in the reference implementation (`rico16-core/src/palette.rs`) and pinned by a test. Any conforming runtime MUST produce bitwise-identical palette words. The HSV generation parameters (hue offsets, value range, saturation curve) are documented in the source for reproducibility but the normative values are the committed data.

4.13. **rgb332 expansion:** For `rgb332`, the 3-bit red/green and 2-bit blue are expanded to RGB555 by replicating the MSB into the LSBs: `r5 = (r3 << 2) | (r3 >> 1)`, `g5 = (g3 << 2) | (g3 >> 1)`, `b5 = (b2 << 3) | (b2 << 1) | (b2 >> 1)`. Equivalently, left-shift and fill low bits with the high bits.

---

## 5. Timing, the game loop, determinism

5.1. The console runs at **60 updates per second**. `dt` passed to `on_update` is exactly `1/60` seconds, as an f64 (`0.016666666666666666`).

5.2. Frame order:

```
load cart → build script engine → on_init() → [ on_update(dt) → on_draw() → present ] × N
```

`on_init` MUST be called exactly once per cart start. `on_update` MUST be called exactly once before each `on_draw`.

5.3. **There is no catch-up.** If a frame takes longer than 1/60 s of wall clock, the runtime MUST NOT run extra updates to compensate; the cart simply runs slower than real time. Rationale: one update per frame keeps carts reproducible, keeps `dt` constant, and avoids the death spiral where the engine falls further behind because it is busy catching up.

5.4. `system::frames()` is a `u64`, the index of the current logical frame. It is **0** throughout the first `on_update`, the following `on_draw`, and that frame's present. It increments after present, so the next `on_update` observes the new value. It does not advance while the system menu is open, and it does not advance if `on_init` requests quit before any frame.

`system::time()` is `(frames as f64) * dt` in IEEE-754 round-to-nearest-even. Past `2^53` frames the conversion is no longer integer-exact; carts that need a precise counter use `frames()`.

Neither function reads the wall clock.

5.5. **Random numbers are deterministic and part of the spec.** The `rnd` module uses **xoshiro256** (xoshiro256starstar), seeded via **SplitMix64** from a single u64. The default seed is 0. `system::seed(n)` reseeds.

**Seeding algorithm (normative):**

```rust
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

fn seed_xoshiro256(seed: u64) -> [u64; 4] {
    let mut sm = seed;
    let s = [
        splitmix64(&mut sm),
        splitmix64(&mut sm),
        splitmix64(&mut sm),
        splitmix64(&mut sm),
    ];
    // SplitMix64 streaming does not produce the all-zero state for any
    // reachable seed. The guard makes the guarantee local and obvious.
    if s == [0, 0, 0, 0] { [1, 0, 0, 0] } else { s }
}
```

This is **canonical SplitMix64 streaming** (`sm` is updated each call). It is **not** `splitmix(seed+i)` for `i = 0..4`.

**xoshiro256** step (normative, Vigna 2020, xoshiro256starstar.c 1.0):**

```rust
fn rotl(x: u64, k: u32) -> u64 { x.rotate_left(k) }

fn next_u64(s: &mut [u64; 4]) -> u64 {
    let result = rotl(s[1].wrapping_mul(5), 7).wrapping_mul(9);
    let t = s[1] << 17;
    s[2] ^= s[0];
    s[3] ^= s[1];
    s[1] ^= s[2];
    s[0] ^= s[3];
    s[2] ^= t;
    s[3] = rotl(s[3], 45);
    result
}
```

- `rnd::float()` draws `x = next_u64()` and returns `(x >> 11) as f64 * 2^-53`. `2^-53` is exact in binary64. The result is in `[0, 1)`.

- `rnd::int(lo, hi)` returns a uniform `i64` in `[lo, hi]`, both ends inclusive.
  - `lo > hi` is a script error.
  - Let `count` be the `u128` value ` (hi as i128) - (lo as i128) + 1 `.
  - If `count == 2^64`, draw one `u64` and bit-cast it to `i64`. This is the full `i64` range.
  - Otherwise draw an unbiased offset in `0..count` by **Lemire's nearly-divisionless method** (Lemire, 2019), then return `lo + offset` computed in `i128` and cast back to `i64`:

```
fn bounded(range: u64) -> u64 {
    // range >= 1. Result is uniform in 0..range (exclusive end).
    let mut x = next_u64();
    let mut m = (x as u128) * (range as u128);
    let mut lo = m as u64;
    if lo < range {
        let threshold = ((1u128 << 64) % (range as u128)) as u64;
        while lo < threshold {
            x = next_u64();
            m = (x as u128) * (range as u128);
            lo = m as u64;
        }
    }
    (m >> 64) as u64
}
```

- `rnd::choice(arr)` returns `()` if `arr` is empty and **does not draw**. Otherwise it returns `arr[rnd::int(0, len-1)]`. A one-element array still draws once.

- `rnd::shuffle(arr)` shuffles in place and returns the same array. It is **Fisher–Yates from the end**. For `n <= 1` it draws nothing.

```
for i in (1..n).rev() {          // i = n-1, n-2, ..., 1
    let j = rnd::int(0, i);      // inclusive
    swap(arr[i], arr[j]);
}
```

Changing the algorithm or the default seed is a **major** version change: it breaks every recorded replay and every seeded cart.

5.6. Carts MUST be byte-identically reproducible across platforms for the same seed, inputs and frame count. Therefore:

- **f64 arithmetic only**; no `f32`, no fast-math, no FMA contraction differences.
- **NaN canonicalization:** Any operation producing NaN MUST yield the canonical quiet NaN (bit pattern `0x7FF8000000000000`). Signalling NaNs MUST NOT be produced. Carts MUST NOT rely on NaN payload bits.
- **Math functions** (`sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `sqrt`, `log`, `exp`, `abs`, `floor`, `ceil`, `round`, `min`, `max`, etc.) MUST be bitwise-identical to the Rust `libm` crate **0.2.16** (musl-derived) for `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `exp`, `ln`, `log10`, `pow`, `cbrt`, `hypot`. The runtime does not have to be written in Rust. It has to match those bits. `libm` 0.2.16 is the reference, not a language requirement.
- `floor`, `ceil`, and `trunc` are the IEEE operations (`trunc` toward zero). `round` is half **away from zero**, including on negatives (`round(-1.5) = -2`).
- `abs` is IEEE `abs` (clears the sign bit, including of zeros and infinities; a NaN input becomes the canonical NaN).
- `min` / `max`: if either argument is NaN, the result is the canonical NaN. Otherwise IEEE numeric comparison, with the signed-zero rule above.
- No dependence on hash iteration order. Rhai's `Map` is ordered; carts MUST NOT depend on anything else unordered.
- No wall-clock, no locale, no filesystem, no environment reads from a cart.
- **Signed zero:** `-0.0` and `+0.0` are distinct bit patterns but compare equal. Carts MUST NOT rely on the sign of zero.
- **i64 overflow** in arithmetic raises a script error (Rhai default). Wrapping is not automatic.

5.7. A runtime MAY provide an optional fast-forward (frame skip) mode for tooling. It MUST produce the same results as real-time execution.

---

## 6. Coordinates, clipping, drawing rules

6.1. All geometry is integer pixels. Negative coordinates and coordinates beyond the screen are legal and MUST be clipped without panicking, wrapping or saturating incorrectly. A fully off-screen draw is a no-op.

6.2. Arguments typed as integer MUST accept integer values. A float argument is converted to integer by **floor** (rounding toward negative infinity: `0.9 → 0`, `-0.9 → -1`). `NaN` or infinite floats where an integer is expected MUST raise a script error. **All NaN values are canonicalized to quiet NaN `0x7FF8000000000000` per [§5.6](#56-carts-must-be-byte-identically-reproducible-across-platforms-for-the-same-seed-inputs-and-frame-count).**

6.3. A color argument is an integer 0–255. Out-of-range values MUST raise a script error, never wrap.

6.4. Drawing is always **overwrite**: the destination index is replaced. There is no blending, no XOR mode, no masking beyond the index-0 key.

6.5. `gfx::text` renders an 8×8 fixed built-in font, advancing 8 PU per glyph, with `\n` moving down 8 PU and returning x to the call's start. The font covers printable ASCII 32–126; other code points (including multi-byte UTF-8, control chars, tabs) render as a placeholder box, **each counting as 1 glyph (8 PU)**. Only **ink** pixels are written; the other pixels of the 8×8 cell are left as they were. `gfx::text_width(str)` returns `first_line.chars().count() * 8` where `first_line` is the substring before the first `\n` (or the whole string if no `\n`). The font lives in the console, not in the cart, and there is no mechanism to replace it — carts cannot break text rendering, and text is one asset format we never have to invent.

**Font bitmap (normative, 8 rows × 8 cols, bit 7 = leftmost pixel):**

The placeholder glyph (used for every scalar outside U+0020..=U+007E) is:
```
FF 81 81 81 81 81 81 FF
```

Printable glyphs are the public-domain 8×8 font of Daniel Hepper, which traces to Marcel Sondaar's public-domain transcription of the IBM VGA 8×8 font. The bytes below are normative.

| Code | Char | Rows |
|---|---|---|
| 32 | `space` | `00 00 00 00 00 00 00 00` |
| 33 | `!` | `18 3C 3C 18 18 00 18 00` |
| 34 | `"` | `36 36 00 00 00 00 00 00` |
| 35 | `#` | `36 36 7F 36 7F 36 36 00` |
| 36 | `$` | `0C 3E 03 1E 30 1F 0C 00` |
| 37 | `%` | `00 63 33 18 0C 66 63 00` |
| 38 | `&` | `1C 36 1C 6E 3B 33 6E 00` |
| 39 | `'` | `06 06 03 00 00 00 00 00` |
| 40 | `(` | `18 0C 06 06 06 0C 18 00` |
| 41 | `)` | `06 0C 18 18 18 0C 06 00` |
| 42 | `*` | `00 66 3C FF 3C 66 00 00` |
| 43 | `+` | `00 0C 0C 3F 0C 0C 00 00` |
| 44 | `,` | `00 00 00 00 00 0C 0C 06` |
| 45 | `-` | `00 00 00 3F 00 00 00 00` |
| 46 | `.` | `00 00 00 00 00 0C 0C 00` |
| 47 | `/` | `60 30 18 0C 06 03 01 00` |
| 48 | `0` | `3E 63 73 7B 6F 67 3E 00` |
| 49 | `1` | `0C 0E 0C 0C 0C 0C 3F 00` |
| 50 | `2` | `1E 33 30 1C 06 33 3F 00` |
| 51 | `3` | `1E 33 30 1C 30 33 1E 00` |
| 52 | `4` | `38 3C 36 33 7F 30 78 00` |
| 53 | `5` | `3F 03 1F 30 30 33 1E 00` |
| 54 | `6` | `1C 06 03 1F 33 33 1E 00` |
| 55 | `7` | `3F 33 30 18 0C 0C 0C 00` |
| 56 | `8` | `1E 33 33 1E 33 33 1E 00` |
| 57 | `9` | `1E 33 33 3E 30 18 0E 00` |
| 58 | `:` | `00 0C 0C 00 00 0C 0C 00` |
| 59 | `;` | `00 0C 0C 00 00 0C 0C 06` |
| 60 | `<` | `18 0C 06 03 06 0C 18 00` |
| 61 | `=` | `00 00 3F 00 00 3F 00 00` |
| 62 | `>` | `06 0C 18 30 18 0C 06 00` |
| 63 | `?` | `1E 33 30 18 0C 00 0C 00` |
| 64 | `@` | `3E 63 7B 7B 7B 03 1E 00` |
| 65 | `A` | `0C 1E 33 33 3F 33 33 00` |
| 66 | `B` | `3F 66 66 3E 66 66 3F 00` |
| 67 | `C` | `3C 66 03 03 03 66 3C 00` |
| 68 | `D` | `1F 36 66 66 66 36 1F 00` |
| 69 | `E` | `7F 46 16 1E 16 46 7F 00` |
| 70 | `F` | `7F 46 16 1E 16 06 0F 00` |
| 71 | `G` | `3C 66 03 03 73 66 7C 00` |
| 72 | `H` | `33 33 33 3F 33 33 33 00` |
| 73 | `I` | `1E 0C 0C 0C 0C 0C 1E 00` |
| 74 | `J` | `78 30 30 30 33 33 1E 00` |
| 75 | `K` | `67 66 36 1E 36 66 67 00` |
| 76 | `L` | `0F 06 06 06 46 66 7F 00` |
| 77 | `M` | `63 77 7F 7F 6B 63 63 00` |
| 78 | `N` | `63 67 6F 7B 73 63 63 00` |
| 79 | `O` | `1C 36 63 63 63 36 1C 00` |
| 80 | `P` | `3F 66 66 3E 06 06 0F 00` |
| 81 | `Q` | `1E 33 33 33 3B 1E 38 00` |
| 82 | `R` | `3F 66 66 3E 36 66 67 00` |
| 83 | `S` | `1E 33 07 0E 38 33 1E 00` |
| 84 | `T` | `3F 2D 0C 0C 0C 0C 1E 00` |
| 85 | `U` | `33 33 33 33 33 33 3F 00` |
| 86 | `V` | `33 33 33 33 33 1E 0C 00` |
| 87 | `W` | `63 63 63 6B 7F 77 63 00` |
| 88 | `X` | `63 63 36 1C 1C 36 63 00` |
| 89 | `Y` | `33 33 33 1E 0C 0C 1E 00` |
| 90 | `Z` | `7F 63 31 18 4C 66 7F 00` |
| 91 | `[` | `1E 06 06 06 06 06 1E 00` |
| 92 | `\` | `03 06 0C 18 30 60 40 00` |
| 93 | `]` | `1E 18 18 18 18 18 1E 00` |
| 94 | `^` | `08 1C 36 63 00 00 00 00` |
| 95 | `_` | `00 00 00 00 00 00 00 FF` |
| 96 | `` ` `` | `0C 0C 18 00 00 00 00 00` |
| 97 | `a` | `00 00 1E 30 3E 33 6E 00` |
| 98 | `b` | `07 06 06 3E 66 66 3B 00` |
| 99 | `c` | `00 00 1E 33 03 33 1E 00` |
| 100 | `d` | `38 30 30 3E 33 33 6E 00` |
| 101 | `e` | `00 00 1E 33 3F 03 1E 00` |
| 102 | `f` | `1C 36 06 0F 06 06 0F 00` |
| 103 | `g` | `00 00 6E 33 33 3E 30 1F` |
| 104 | `h` | `07 06 36 6E 66 66 67 00` |
| 105 | `i` | `0C 00 0E 0C 0C 0C 1E 00` |
| 106 | `j` | `30 00 30 30 30 33 33 1E` |
| 107 | `k` | `07 06 66 36 1E 36 67 00` |
| 108 | `l` | `0E 0C 0C 0C 0C 0C 1E 00` |
| 109 | `m` | `00 00 33 7F 7F 6B 63 00` |
| 110 | `n` | `00 00 1F 33 33 33 33 00` |
| 111 | `o` | `00 00 1E 33 33 33 1E 00` |
| 112 | `p` | `00 00 3B 66 66 3E 06 0F` |
| 113 | `q` | `00 00 6E 33 33 3E 30 78` |
| 114 | `r` | `00 00 3B 6E 66 06 0F 00` |
| 115 | `s` | `00 00 3E 03 1E 30 1F 00` |
| 116 | `t` | `08 0C 3E 0C 0C 2C 18 00` |
| 117 | `u` | `00 00 33 33 33 33 6E 00` |
| 118 | `v` | `00 00 33 33 33 1E 0C 00` |
| 119 | `w` | `00 00 63 6B 7F 7F 36 00` |
| 120 | `x` | `00 00 63 36 1C 36 63 00` |
| 121 | `y` | `00 00 33 33 33 3E 30 1F` |
| 122 | `z` | `00 00 3F 19 0C 26 3F 00` |
| 123 | `{` | `38 0C 0C 07 0C 0C 38 00` |
| 124 | `|` | `18 18 18 00 18 18 18 00` |
| 125 | `}` | `07 0C 0C 38 0C 0C 07 00` |
| 126 | `~` | `6E 3B 00 00 00 00 00 00` |

6.6. Primitives use the standard half-open convention: `gfx::rect(x, y, w, h)` covers pixels `x .. x+w` exclusive. `w` or `h` ≤ 0 is a no-op.

6.7. Circles are rasterized using the **integer disk** `dx² + dy² ≤ r²` (with `dx`, `dy`, `r` in `i128`) so that identical arguments produce identical pixels on every platform. `gfx::circle_filled(cx, cy, r, color)` fills all pixels satisfying the disk. `gfx::circle(cx, cy, r, color)` draws the boundary: pixels in the disk of radius `r` but not in the disk of radius `r-1`. Radius `-1` is the empty set, so `r = 0` is the center pixel. `r < 0` is a no-op for filled; `r ≤ 0` is a no-op for outline. Radius 1 filled is a 3×3 block (9 pixels); outline is 4 pixels (N, S, E, W). An implementation may draw `filled(r)` and skip pixels that `filled(r-1)` would cover; it must not paint the interior. Work is proportional to the number of screen rows the disk intersects, not to `r` when the disk misses the screen (returns immediately).

6.8. Lines are rasterized with **Bresenham's integer DDA** so that identical arguments produce identical pixels on every platform. `gfx::line(x0, y0, x1, y1, color)` is one pixel thick. Both endpoints are drawn when on screen. A point (`x0 == x1` and `y0 == y1`) is that one pixel.

Let `dx = x1 - x0`, `dy = y1 - y0` in `i128`. Let `adx = abs(dx)`, `ady = abs(dy)`. **X is the major axis when `adx ≥ ady`**, including exact diagonals. Otherwise Y is major.

Along the major axis the minor coordinate at step `i` (`i` from 0 through the major length, inclusive) is `start + round_half_away(minor_delta * i, major_length)` where

```
round_half_away(n, denom) with denom > 0:
    if n >= 0:  (n + denom/2) / denom      # both divisions truncate toward 0
    else:       -(((-n) + denom/2) / denom)
```

`denom/2` truncates toward 0. The formula is evaluated in `i128`.

Visible-range rule, bit-identical to walking every `i` and discarding off-screen pixels:

- X major, `sx = sign(dx)` (`+1` or `-1`; point case already returned). `x(i) = x0 + i*sx`. Iterate `x` over the intersection of `[min(x0,x1), max(x0,x1)]` with `[0, 319]`. For each such `x`, `i = (x - x0) * sx`, compute `y`, write if `y` is on screen.
- Y major is the same with axes swapped.

Do not clip the endpoints to the screen and then restart the error term. That plots different pixels.

6.9. `sprite::draw(id, x, y)` and overloads: `id` outside 0–255 raises a script error. `sprite::get_pixel(id, px, py)` and `set_pixel`: `px` or `py` outside 0–15 raises a script error. `sprite::draw_region(sx, sy, w, h, dx, dy, flip_x, flip_y)`: source rectangle outside the 256×256 sheet raises a script error. `palette(index, r, g, b)`: `index` outside 0–255 raises a script error.

---

## 7. Resources and limits

| Resource | Size | Encoding | What happens at the limit |
|---|---|---|---|
| Framebuffer | 76,800 B | 1 byte/px | fixed |
| Sprite sheet | 65,536 B (256×256, 256 tiles of 16×16) | 1 byte/px | fixed |
| Tilemap | 16,384 B (128×128 cells) | 1 byte/cell | fixed; out-of-range access is silent (§6.9) |
| Palette | 512 B | RGB555 × 256 | fixed |
| Cart code | no fixed source limit | UTF-8 Rhai | the PNG capacity in §13 |
| Script operations | 1,000,000 per invocation | Rhai 1.27.0 operation counter | script error |
| Array length | 65,536 elements | | script error on the call that would exceed it |
| Map length | 65,536 entries | | script error |
| String length | 1,048,576 bytes | UTF-8 | script error |

7.1. Tiles are **16×16 PU**. A 320×240 screen shows exactly **20×15** cells. Tile id = `row * 16 + col` on a 16-column sheet; ids are 0–255.

7.2. The map is **128×128 cells** of 16×16 PU, i.e. a 2048×2048 PU world. Cell values are tile ids 0–255. Every id draws; there is no implicit "empty" tile. Carts that need empty space reserve an id for it.

7.3. `tilemap::get` reads outside 0–127 return 0. `tilemap::set` writes outside 0–127 are ignored. This is deliberate silent clipping so that procedural map code can run off the edges; carts MUST NOT rely on it to detect bugs.

7.4. **There is no `CODE_LIMIT`.** The only limit is the PNG container payload capacity after assets are encoded. The build fails only when the payload exceeds the container, and reports used/capacity.

7.5. Limits are compile-time constants in the reference implementation. Changing any number in the table above is a spec change and a major version bump.

7.6. There is no token counting.

7.7. **PNG capacity table** (12 bits per pixel, low nibbles of RGB, A=0xFF):

| PNG size | Payload bytes | After 16-byte blob header | Code space with full assets |
|---|---|---|---|
| 640×480 (default) | 460,800 | 460,784 | ≈ 378 KB |
| 1280×960 (overflow) | 1,843,200 | 1,843,184 | ≈ 1.76 MB |

---

## 8. Drawing model

8.1. rico-16 is **immediate mode over a persistent data model**. Every frame, the cart draws explicitly; nothing is retained except framebuffer contents.

8.2. The sprite sheet and the tilemap are mutable data structures. They persist across frames, are read by blits, are edited by API, and are saved in the cart. They are **not** surfaces: they are never composited, never scrolled by hardware, and have no draw order of their own.

8.3. `tilemap::draw(...)` blits a rectangle of cells into the framebuffer at a given screen position, inline, in call order, exactly like `sprite::draw`. Layer ordering is therefore just call order. There are no layers.

8.4. There is **no global camera and no transform stack**. Scrolling is arithmetic performed by the cart, or by choosing different source cells and destination offsets per call:

```rhai
// draw the map as seen by a camera at world pixel (cam_x, cam_y)
let cx = floor(cam_x / 16.0);
let cy = floor(cam_y / 16.0);
tilemap::draw(cx, cy, 22, 16, cx * 16.0 - cam_x, cy * 16.0 - cam_y);
```

Note the float division: integer division truncates toward zero, which puts negative camera positions in the wrong cell. 22 cells of horizontal overdraw cover the 20 visible cells at any sub-cell offset.

A camera transform is listed as deferred ([§18](#18-deferred-features-and-non-goals)) because hidden render state is the main source of "why did my sprite move" bugs, and a console should be predictable.

8.5. Sprite flipping is per call, not stored in the sheet. There are no per-sprite flags, no collision bitmasks, no palette-per-sprite banks.

---

## 9. Script API

### 9.1. Naming rules

- Modules are namespaced: `gfx::`, `sprite::`, `tilemap::`, `input::`, `system::`, `audio::`, `rnd::`.
- `snake_case`; no abbreviations (`tilemap`, not `map`; `draw_region`, not `sspr`). `map` is explicitly avoided because Rhai has a built-in `Map` type.
- Draw operations are named after their noun; filled variants take a `_filled` suffix so related calls sort together: `gfx::rect`, `gfx::rect_filled`.
- Do not namespace what Rhai already provides. `sin`, `cos`, `sqrt`, `abs`, string and array operations remain free functions; there is no `math::` module.
- Functions may be overloaded by arity (`sprite::draw` with 3 or 5 arguments), never by type.
- The three entry point names in §9.2 are reserved: a cart must not repurpose `on_init`, `on_update` or `on_draw` as helpers. Other `on_`-prefixed names are ordinary functions and are never called by the runtime.
- **Methods cannot be imported.** `array.push`, string methods, etc. stay global Rhai built-ins. `import "gfx" as g;` gives you `g::clear(0)`, not `g.clear(0)`.

### 9.2. Entry points

```rhai
fn on_init()      { }   // once, before frame 0
fn on_update(dt)  { }   // once per frame; dt == 1/60 always
fn on_draw()      { }   // once per frame, after on_update
```

All three are optional. A cart with no `on_draw` presents an unmodified framebuffer. A cart with no `on_update` still runs `on_draw` at 60 Hz.

Declaring one of these names with a different parameter count is a load error, not a silently skipped handler.

**Only `main.rhai` (the entry point specified in `cart.toml`) has the runtime entry points.** A module's `on_init`/`on_update`/`on_draw` are ordinary functions and are never called automatically. Scene modules must receive and return state; the main module dispatches to them.

### 9.3. Surface

Signatures below are normative. `color` is an index 0–255. `button` is a `Button` value.

**Graphics**

| Function | Notes |
|---|---|
| `gfx::clear(color)` | fills the whole framebuffer |
| `gfx::pixel(x, y, color)` | |
| `gfx::line(x0, y0, x1, y1, color)` | inclusive endpoints |
| `gfx::rect(x, y, w, h, color)` | outline |
| `gfx::rect_filled(x, y, w, h, color)` | |
| `gfx::circle(x, y, r, color)` | outline |
| `gfx::circle_filled(x, y, r, color)` | |
| `gfx::text(str, x, y, color)` | 8×8 font, `\n` supported |
| `gfx::text_width(str)` | returns PU width of the first line: `first_line.chars().count() * 8` |
| `gfx::palette(index, r, g, b)` | 8-bit components, quantized |
| `gfx::palette_reset()` | restores the cart's palette |

**Sprites**

| Function | Notes |
|---|---|
| `sprite::draw(id, x, y)` | 16×16, clipped, skips index 0 |
| `sprite::draw(id, x, y, flip_x, flip_y)` | overload with flipping |
| `sprite::draw_region(sx, sy, w, h, dx, dy, flip_x, flip_y)` | source in sheet pixels, no scaling |
| `sprite::get_pixel(id, px, py)` | `px`, `py` in 0–15 |
| `sprite::set_pixel(id, px, py, color)` | runtime mutation of the sheet |

**Tilemap**

| Function | Notes |
|---|---|
| `tilemap::draw(cx, cy, cw, ch, dx, dy)` | source cell rect, destination screen position |
| `tilemap::get(cx, cy)` | `u8`, 0 outside bounds |
| `tilemap::set(cx, cy, id)` | no-op outside bounds |

**Input**

| Function | Notes |
|---|---|
| `input::held(button)` | true while down |
| `input::pressed(button)` | true only on the frame it went down |
| `input::released(button)` | true only on the frame it went up |

`Button` values: `Button::Left`, `Button::Right`, `Button::Up`, `Button::Down`, `Button::Z`, `Button::X`, `Button::C`, `Button::V`.

**System**

| Function | Notes |
|---|---|
| `system::frames()` | u64, 0 on the first frame |
| `system::time()` | f64 seconds, `frames * dt` |
| `system::seed(n)` | reseed the RNG |
| `system::quit()` | ends the cart cleanly after the current frame |
| `system::quit(message)` | same, printing `message` to stdout (editor: console pane) first |
| `system::get_state()` | returns a copy of the global state map (see §9.7) |
| `system::set_state(map)` | replaces the global state map with a clone of `map` |

**Audio (stubbed in v3)**

| Function | Notes |
|---|---|
| `audio::play(id)` | accepted, no output, no effect |

**Random (rnd)**

| Function | Notes |
|---|---|
| `rnd::float()` | f64 in `[0, 1)` |
| `rnd::int(lo, hi)` | integer in `[lo, hi]` inclusive, Lemire rejection |
| `rnd::choice(arr)` | random element from array, or `()` if empty |
| `rnd::shuffle(arr)` | Fisher-Yates descending, in-place, returns array |

### 9.4. Errors

9.4.1. Every script error MUST be reported with the cart file name, line and column, plus the Rhai message. Reporting a bare `Runtime error` is a spec violation.

9.4.2. An error raised inside a callback aborts the rest of that callback. The runtime then follows [§14.8](#148-error-policy).

9.4.3. Unknown module, function or `Button` value is a script error, not a no-op.

9.4.4. **Module errors carry the module path** (e.g., `Function not found ... @ 'lib/broken.rhai'`).

### 9.5. Call budget

Measured host-call overhead is roughly 0.5–2 µs per Rhai function call, so a cart should stay under a few thousand draw calls per frame at 60 Hz. This is guidance, not a limit; the runtime SHOULD warn once per second on stdout when frames consistently overrun. Bulk operations (`tilemap::draw`, `draw_region`) exist to keep carts inside the budget.

### 9.6. Standard modules as ambient globals

In addition to being importable (`import "gfx" as g;`), the standard modules are **also available as ambient globals** without import: `gfx::clear(0)`, `sprite::draw(1, 10, 10)`, `input::held(Button::Left)`, etc. This is for ergonomics; the import form is preferred for clarity and to avoid ambiguity.

The reserved module names are: `gfx`, `sprite`, `tilemap`, `input`, `system`, `audio`, `rnd`, `Button`. A cart MUST NOT define its own module with these names.

### 9.7. Global state (cart-wide mutable map)

The runtime provides a single mutable map owned by the cart, accessible only via `system::get_state()` and `system::set_state(map)`. This is the **only** way to persist data across frames in the entry script.

- `system::get_state()` returns a clone of the current state map (or `#{}` if never set).
- `system::set_state(map)` replaces the stored map with a clone of `map`.
- The map is cloned on get/set; mutations to the returned map do not affect the stored state until `set_state` is called.
- At cart start (before `on_init`), the state map is empty (`#{}`).
- The state map is **not** serialized into the cart binary. Restart (§14.10) reloads the cart from its original bytes, so script globals and the state map are reset. Save data persistence is deferred (§18.1).

**Pattern:**

```rhai
fn on_init() {
    let s = system::get_state();
    if !s.contains("px") {
        s["px"] = 160;
        s["py"] = 120;
        s["scene"] = "title";
        system::set_state(s);
    }
}

fn on_update(dt) {
    let s = system::get_state();
    if s["scene"] == "play" {
        s["px"] = s["px"] + 1; // example
    }
    system::set_state(s);
}
```

Modules **cannot** access this map directly. They receive state as function arguments and return updated state (see §11.5).

---

## 10. Project structure and cart.toml

### 10.1. Directory layout

A rico-16 project is a directory with the following structure:

```
project-name/
  cart.toml           # REQUIRED: manifest
  src/
    main.rhai         # DEFAULT entry point (configurable in cart.toml)
    scenes/           # OPTIONAL: scene modules
      title.rhai
      play.rhai
    lib/              # OPTIONAL: library modules
      grid.rhai
  assets/
    hero.png          # OPTIONAL: sprite sheet strips
    tiles.png
    map.png
    palette.png
  build/              # OUTPUT: packed PNG (gitignored)
```

### 10.2. cart.toml

`cart.toml` holds metadata and asset placement. It is **TOML** (requires adding `toml` crate to approved dependencies).

```toml
format = 1

[cart]
title = "Snake"
author = "..."
version = "0.1.0"
# entry = "src/main.rhai"    # optional, defaults to "src/main.rhai"
# output = "build/snake.png" # optional, defaults to "build/<title-slug>.png"

[[sprite]]
file = "assets/hero.png"
x = 0
y = 0

[[sprite]]
file = "assets/tiles.png"
x = 0
y = 8

[map]
file = "assets/map.png"

[palette]
file = "assets/palette.png"
```

**No external dependencies.** There is no `[dependencies]` section and no package manager. All modules are local to the project (`src/`, `src/lib/`, `src/scenes/`, etc.). External Rhai libraries are vendored by copying their `.rhai` files into your project tree and importing them normally:

```rhai
import "lib/external" as ext;
```

#### 10.2.1. `[cart]` section

| Key | Required | Default | Description |
|---|---|---|---|
| `title` | yes | — | Cart title, UTF-8 |
| `author` | no | `""` | Author name |
| `version` | no | `""` | Semantic version string |
| `entry` | no | `src/main.rhai` | Path to entry point Rhai file, relative to project root |
| `output` | no | `build/<title-slug>.png` | Output PNG path, relative to project root |

The `<title-slug>` is the title lowercased (ASCII-only lowercasing; non-ASCII characters replaced with `-`), non-alphanumerics replaced with `-`, consecutive `-` collapsed, leading/trailing `-` stripped, truncated to 64 chars. Empty slug becomes `cart`.

The `format` key at the root is REQUIRED and MUST be `1` for v1 projects. Unknown keys in `[cart]` are preserved. Unknown top-level tables are preserved.

#### 10.2.2. `[[sprite]]` array

Each entry places a sprite sheet PNG at a tile coordinate in the 256×256 sheet (16×16 tiles).

| Key | Required | Description |
|---|---|---|
| `file` | yes | Path to PNG, relative to project root |
| `x` | yes | Tile column (0–15) |
| `y` | yes | Tile row (0–15) |

- The PNG dimensions MUST be multiples of 16×16.
- Each pixel MUST exactly match a palette colour (after palette resolution); alpha 0 becomes index 0; partial alpha or unknown colours is a build error.
- Overlapping placements and out-of-bounds placements are rejected.

#### 10.2.3. `[map]` section

| Key | Required | Description |
|---|---|---|
| `file` | yes | Path to PNG, relative to project root |

- The PNG MUST be 128×128 pixels (one pixel per cell).
- **Decoding:** If the PNG is grayscale, the sample value (0–255) is the tile id. If indexed, the palette index (0–255) is the tile id. If RGB(A), each pixel is converted to grayscale via `Y = (r*299 + g*587 + b*114 + 500) / 1000` (BT.601) and clamped to 0–255. The resulting value is the tile id.
- If absent, the tilemap is zero-filled.

#### 10.2.4. `[palette]` section

| Key | Required | Description |
|---|---|---|
| `file` | yes | Path to PNG, relative to project root |

- The PNG MUST be 256×1 or 16×16 opaque swatches.
- Colours are quantized to RGB555 using the same formula as `gfx::palette` (§4.4).
- If absent, the console default palette is used.

### 10.3. Source assets

Source assets stay as PNGs on disk; the build converts them to the console's raw data. The distribution PNG contains the compiled sheet, map and palette, not the original strip files. `unpack` can reconstruct a canonical full sheet, map and palette for inspection, but not the original art layout.

---

## 11. Module system

### 11.1. Import syntax

The source uses real Rhai imports:

```rhai
import "scenes/title" as title;
import "scenes/play" as play;
import "lib/grid" as grid;
import "gfx" as g;           // standard module
```

### 11.2. Build-time resolution

At build time:

- All imports are resolved **relative to `src/`** (the source root). The importing file's location is irrelevant.
- Append `.rhai` when missing.
- Reject `..` escapes, absolute paths, cycles, and non-literal import paths.
- Embed every reachable module source into the PNG.
- The module table key is the import path **without** `.rhai` and **without** leading `src/` (e.g., `scenes/play`, `lib/grid`).

### 11.3. Runtime module resolver

At runtime:

- A custom `ModuleResolver` serves the embedded modules.
- No filesystem, no network, no runtime file imports.
- Standard modules (`gfx`, `sprite`, `tilemap`, `input`, `system`, `audio`, `rnd`, `Button`) are reserved and also importable.
- Module errors carry the module path.

### 11.4. Module semantics

- Modules are **stateless**. Module-level `let`/`const` are not visible inside module functions, and module functions cannot see main's top-level variables.
- Scene modules must receive and return state explicitly.
- Transitive imports and standard native modules under an alias work.

### 11.5. Scene pattern

Only `main.rhai` (the entry point) has the runtime entry points. A module's `on_init`/`on_update`/`on_draw` are ordinary functions and are never called automatically. Name scene hooks `update`/`draw` and dispatch from main:

```rhai
// scenes/play.rhai
fn update(dt, state) {
    let next = state.clone();
    if input::held(Button::Left) { next["px"] = next["px"] - 2; }
    next
}

fn draw(state) {
    gfx::clear(1);
    gfx::rect_filled(state["px"] - 4, state["py"] - 4, 8, 8, 9);
}

// main.rhai
import "scenes/play" as play;

fn on_init() {
    let s = system::get_state();
    if !s.contains("px") {
        s["px"] = 160;
        s["py"] = 120;
        s["scene"] = "play";
        system::set_state(s);
    }
}

fn on_update(dt) {
    let s = system::get_state();
    if s["scene"] == "play" {
        s = play::update(dt, s);
    }
    system::set_state(s);
}

fn on_draw() {
    let s = system::get_state();
    if s["scene"] == "play" {
        play::draw(s);
    }
}
```

That is the honest pattern: modules are libraries, main owns the game state via `system::get_state`/`set_state`.

---

## 12. Assets and build pipeline

### 12.1. Sprites

- Any PNG whose dimensions are multiples of 16×16.
- Place it explicitly at tile coordinates in `cart.toml` (`[[sprite]]` entries).
- Each pixel must exactly match a palette colour; alpha 0 becomes index 0; partial alpha or unknown colours is a build error.
- A strip exported by a pixel-art tool is just a wide PNG placed at a tile coordinate.

### 12.2. Map

- 128×128 grayscale or indexed PNG where the pixel value is the tile id.
- Placed via `[map]` in `cart.toml`.
- Decoding per §10.2.3.

### 12.3. Palette

- 256×1 or 16×16 opaque swatches, quantized to RGB555.
- Placed via `[palette]` in `cart.toml`.
- If absent, the console default palette is baked in.

### 12.4. Build process

1. Parse `cart.toml`.
2. Load and validate palette (custom or default).
3. Load and validate sprite PNGs, convert to 256×256 sheet using `cart.toml` coordinates.
4. Load and validate map PNG, convert to 128×128 tilemap.
5. Resolve import graph from entry point, embed all reachable module sources.
6. Serialize binary cart (assets + module sources + metadata).
7. Pack into PNG (640×480 default, 1280×960 if payload exceeds 640×480 capacity).
8. Write output PNG.

### 12.5. Capacity and overflow

- Default PNG: 640×480 → ~460 KB payload → ~378 KB for code with full assets.
- Overflow PNG: 1280×960 → ~1.84 MB payload → ~1.76 MB for code with full assets.
- Build automatically uses overflow size if payload exceeds default capacity.
- No language-level token/byte limit. Build fails only when payload exceeds container, and reports used/capacity.
- Compression (zlib) is reserved for future (flag in format); not in v1.

---

## 13. Cart binary format and PNG packing

### 13.1. Binary cart

All integers little-endian.

```
offset  size   field
0       4      magic "R16C"
4       2      format version (u16) = 1
6       2      flags (u16) = 0
8       4      code length in bytes (u32)
12      4      module table length in bytes (u32)
16      4      metadata length in bytes (u32)
20      65536  sprite sheet (256×256, 1 byte/px)
65556   16384  tilemap (128×128, 1 byte/cell)
81940   512    palette (256 × RGB555)
82452   N      code, UTF-8 Rhai source (entry point only)
82452+N   M      module table (see below)
82452+N+M K      metadata (TOML, UTF-8)
```

**Module table format:**
- Each module entry: `u16 path_len`, `path (UTF-8)`, `u32 source_len`, `source (UTF-8)`.
- Paths are normalized with forward slashes, no `.rhai` extension, relative to `src/` (e.g., `scenes/play`, `lib/grid`).
- The entry point (`main.rhai`) is NOT in the module table; it is in the code section.
- Module entries are sorted by path (lexicographic) for deterministic builds.

**Metadata format:** UTF-8 TOML containing at minimum:
```toml
title = "..."
author = "..."
version = "..."
entry = "src/main.rhai"
```
Additional keys from `[cart]` are preserved. Unknown keys are preserved.

Maximum size depends on PNG container. In format version 2, `flags` bit 0 (palette present) MUST be 1, because packing always resolves a palette into the cart. Other flag bits are reserved and MUST be 0 on write; loaders MUST ignore unknown bits.

### 13.2. PNG v1: The image is the label

The PNG is **640×480 (default) or 1280×960 (overflow), 8-bit RGBA, non-interlaced** — a completely ordinary image with no private chunks and no reserved banner. The *entire* image is cover art, and the cart data is hidden underneath it in the low four bits of every **R, G, B** channel. The alpha channel is forced to `0xFF` (fully opaque).

13.2.1. **Visible art.** The high nibble of the R, G and B bytes is the cover art, 4 bits per channel (RGB444, 4096 colors). The low nibble of R, G, B carries payload.

**The low nibbles are visible.** They perturb each channel by up to 15/255, which is one RGB444 step. On a flat field they look like grain. This spec does not pretend otherwise. Author labels as flat pixel art, not as photographs. Alpha is not a data channel: putting the payload in alpha made every pixel 240–255 and the picture see-through. That was wrong.

13.2.2. **Payload.** Reading pixels in row-major order from (0,0), the low nibbles of R, G, B are concatenated into a nibble stream (3 nibbles per pixel), and consecutive pairs of nibbles form payload bytes, first nibble as the high half. That yields **12 bits per pixel** and a total capacity of:
- 640×480: 460,800 bytes (460,784 after 16-byte header)
- 1280×960: 1,843,200 bytes (1,843,184 after 16-byte header)

13.2.3. The payload begins with the blob header below, then the binary cart v3 ([§13.1](#131-binary-cart)). Nibbles after the end of the payload MUST be zero. Because the authored art has no low-nibble information by definition, filling them cannot damage it.

```
offset  field
0       magic "R16P"
4       blob version (u16) = 3
6       PNG size variant (u16): 0 = 640×480, 1 = 1280×960
8       payload length (u32) — length of the binary cart that follows
12      CRC-32 (IEEE) of that payload
```

13.2.4. **Cover art.** `rico16 build` takes `--label <image>` or generates a label when none is given. The generated label is deterministic — a dark field, the cart title rendered in the built-in 8×8 font, the author line, and a palette strip of the cart's own colours — so that packing the same cart twice produces the same PNG.

13.2.5. **Custom label cover-fit (normative).** `--label` is a non-interlaced 8-bit PNG, decoded to straight RGBA8. Composite over black first: each channel becomes `(c * a) / 255`, truncating. Then cover-fit into the variant size by sampling the source pixel that contains the image of each destination pixel's center.

All arithmetic is `i128`, division truncates toward zero. Let `dst_w`, `dst_h` be the variant size and `src_w`, `src_h` the label size.

Width-limited, when `dst_w * src_h ≥ dst_h * src_w` (the source, once scaled to the destination width, is at least as tall as the destination; leftover height is cropped equally off the top and bottom):

```
src_x = ((2*x + 1) * src_w) / (2 * dst_w)
src_y = ((src_h * dst_w - dst_h * src_w) + (2*y + 1) * src_w) / (2 * dst_w)
```

Height-limited, when `dst_w * src_h < dst_h * src_w`:

```
src_y = ((2*y + 1) * src_h) / (2 * dst_h)
src_x = ((src_w * dst_h - dst_w * src_h) + (2*x + 1) * src_h) / (2 * dst_h)
```

Clamp `src_x` into `0 .. src_w-1` and `src_y` into `0 .. src_h-1`. A label that is already the variant size maps `(x, y)` to itself; implementations use these formulas for that case too, not a second resampler. This is not the game scaler (§3.7).

Then quantize each channel with `(v * 15 + 127) / 255` into the high nibble.

13.2.6. **Labels live only in the PNG.** The project source has no label; the label is a property of the published artefact, not of the source. Re-labelling a cart therefore means repacking, and never modifies the cart's contents.

### 13.3. Advisory metadata chunk

`rico16 build` SHOULD also emit a `tEXt` chunk with keyword `rico16` and value `v=3;variant=<0|1>;len=<payload length>;crc=<8 uppercase hex>`. Tools MAY use it to identify carts without decoding pixels. The pixel payload is authoritative; if the chunk and the pixels disagree, the runtime MUST trust the pixels and SHOULD warn.

### 13.4. Reading a cart PNG v1

A loader MUST: verify the magic, verify blob version = 1, verify PNG size variant (0 or 1), verify that the payload fits the capacity for that variant, verify the CRC-32, then parse. Any mismatch MUST be a clear error naming the field that failed. A loader MUST NOT attempt to guess or repair a corrupt cart.

**Additional validation:** Verify palette entry bit 15 is 0. Verify module table does not overrun payload. Verify that the bytes after the fixed header (82,452 bytes: magic + version + flags + 4 lengths + metadata strings + sheet + map + palette) equal `code_len + modules_len + metadata_len` exactly, with nothing left over and nothing missing.

### 13.5. Durability rules

13.5.1. Lossless PNG re-encoding (optimizers, `optipng`, CDNs, most social platforms) preserves carts, because the payload lives in pixel values rather than chunks or trailing bytes. Lossy transcoding — JPEG, lossy WebP, palette reduction, or any resize — destroys them. Document this wherever carts are shared: a cart is an image, and images get re-encoded.

13.5.2. Because the payload is spread across the whole image, a cart PNG MUST NOT be resized, cropped, or colour-reduced by any tool that "prepares images for the web". This is the price of having real cover art, and it is the same price PICO-8 pays.

13.5.3. Alpha is always `0xFF` and carries no payload. Because alpha is constantly 255, a tool that drops the alpha channel and keeps RGB **does not** destroy the cart. A tool that requantizes RGB does. The distribution warning is: do not resize, do not JPEG, do not "optimize" colors. Alpha stripping is survivable.

### 13.6. Determinism

`rico16 build` SHOULD be deterministic: the same project source, the same label input and the same encoder settings produce a byte-identical PNG. The generated default label MUST be deterministic. Module table entries are sorted by path. Reproducible builds are a stated goal.

---

## 14. Runtime and player

### 14.1. Cart loading

The player accepts two inputs:
- A project directory (path containing `cart.toml`) — builds in memory, then runs.
- A packed PNG (v3) — loads directly.

### 14.2. Window and presentation

The player opens a single window, scaled per [§3.7](#3-display-and-framebuffer), presenting once per update with vsync. On displays whose refresh rate is not a multiple of 60, judder is expected and accepted.

**Frame pacing:** The runtime MUST limit updates to at most one per 1/60 s of wall time. If the vsync interval is shorter than 16.67 ms (e.g., 144 Hz display), the runtime MUST skip presents (not updates) to maintain 60 Hz logical rate. `dt` is always `1/60`. `system::frames()` and `system::time()` advance only on logical frames, not on skipped presents.

### 14.3. Default key map

Default key map, matched on **physical scan codes** so that AZERTY and QWERTZ layouts keep the same physical arrangement:

| Button | Physical key |
|---|---|
| Up / Down / Left / Right | arrow keys |
| `Button::Z` | `KeyZ` position |
| `Button::X` | `KeyX` position |
| `Button::C` | `KeyC` position |
| `Button::V` | `KeyV` position |

Carts refer to logical buttons, never to keys, so a remap changes nothing in cart code.

### 14.4. Reserved keys

Reserved keys, never visible to a cart: `Esc` (opens system menu), `F11` (toggle fullscreen), `Ctrl+R` (restart cart). A cart MUST NOT be able to bind them.

### 14.5. System menu

`Esc` opens the **system menu**, a runtime-provided overlay that pauses the cart. The system menu provides:

- **Resume** — closes the menu, continues execution
- **Restart** — restarts the cart (calls `system::quit()` internally, then reloads)
- **Exit** — calls `system::quit()` and closes the window

The system menu is drawn by the runtime using a **separate overlay buffer** (not the cart framebuffer). It does not count against the cart's draw budget. While the menu is open, `on_update` and `on_draw` are not called; logical time does not advance (`system::frames()` and `system::time()` pause).

**Cart-owned pause menus:** Carts that need an in-game pause menu (triggered by a gameplay button) must implement it themselves using `system::get_state`/`set_state` and conditional drawing. There is no `Button::Menu` magic button.

### 14.6. Input sampling

Input is sampled once per frame, immediately before `on_update`. Edge state (`pressed`, `released`) compares the current frame's bitmask with the previous frame's. While the system menu is open, the cart's input bitmask is frozen (held buttons remain held, no new pressed/released events are generated).

### 14.7. Focus loss

On window focus loss the runtime MUST clear all held buttons. Key-repeat events MUST NOT be treated as new presses.

### 14.8. Error policy

- **Player mode:** print `<file>:<line>:<col>: <message>` to stderr, close the window, exit with code 1. The process MUST NOT panic.
- **Editor mode:** halt the cart, keep the editor alive, show the message in the console pane. See [§16](#16-editor).

### 14.9. Quit behavior

`system::quit()` ends the cart after the current frame and exits with code 0. `system::quit(message)` prints `message` to stdout first (in the editor, to the console pane) and exits the same way. A message is not an error and MUST NOT go to stderr.

The system menu's "Restart" action calls `system::quit()` internally, then the runtime reloads the cart and starts a new process (or reinitializes the engine). The system menu's "Exit" action calls `system::quit()` and exits the process.

### 14.10. Audio

There is no audio output in v1; `audio::play` is accepted and ignored. The audio device MUST NOT be opened.

### 14.11. Module resolver integration

The runtime constructs a custom `ModuleResolver` that serves embedded module sources from the loaded cart (PNG or in-memory build). No filesystem access occurs during script execution. Standard modules are registered natively and also available through the resolver.

---

## 15. Commands

The `rico16` binary provides the following subcommands:

### 15.1. `rico16 new <name> [--palette <preset>]`

Creates a new project directory with:
- `cart.toml` with defaults
- `src/main.rhai` with empty entry points
- `assets/` directory
- `build/` directory (gitignored)
- `.gitignore` with `build/`

`--palette rgb332` bakes the `rgb332` preset into `cart.toml` (adds `[palette]` with a generated palette file or inline default).

### 15.2. `rico16 build <project> [--label <image>] [--variant <default|overflow>]`

Builds the project to a packed PNG.
- Reads `cart.toml`, validates, compiles assets, resolves imports, packs.
- `--label` provides custom cover art (resampled to PNG dimensions).
- `--variant` forces PNG size: `default` (640×480) or `overflow` (1280×960). Default is auto: use default, switch to overflow if payload exceeds capacity.
- Reports used/capacity bytes on success.
- Exits with code 0 on success, non-zero on error with clear message.

### 15.3. `rico16 run <project|png>`

Runs a cart.
- If argument is a directory (contains `cart.toml`): builds in memory, then runs.
- If argument is a `.png` file: loads and runs directly (v3).
- No separate build step needed for project directories.

### 15.4. `rico16 unpack <png> <output-dir>`

Inspects a shared cart PNG.
- Reconstructs canonical full sprite sheet (256×256) as `assets/sprites.png`, tilemap (128×128) as `assets/map.png`, palette (256×1) as `assets/palette.png`.
- Extracts all embedded module sources to `src/` preserving directory structure.
- Writes a `cart.toml` with metadata from the cart and **canonical asset placements**:
  ```toml
  format = 3

  [cart]
  title = "..."
  author = "..."
  version = "..."
  entry = "src/main.rhai"
  output = "build/<title-slug>.png"

  [[sprite]]
  file = "assets/sprites.png"
  x = 0
  y = 0

  [map]
  file = "assets/map.png"

  [palette]
  file = "assets/palette.png"
  ```
- **Does not** reconstruct the original art layout (strip files); only the canonical compiled form.

### 15.5. `rico16 fmt <project>`

Formats all `.rhai` files in `src/` using a Rhai formatter (deferred; may be removed if no formatter exists).

---

## 16. Editor

### 16.1. Overview

The editor is a ratatui terminal application (crossterm backend). It is a separate mode of the same binary: `rico16 edit <project>`.

### 16.2. Required panes (first milestone)

- Project file tree (src/, assets/, cart.toml)
- Code editor with Rhai syntax highlighting
- Console (script errors, warnings, `print`-style output)
- Palette inspector

### 16.3. Required actions

- Run, stop, restart
- **Build-on-run**: `rico16 run` semantics from the editor
- **Rebuild on save**: when a `.rhai` or asset file is saved, rebuild in background; hot reload the running cart (restart cart, re-run `on_init`; MUST NOT attempt to preserve script globals)

### 16.4. Game window

The game runs in its own native window, not inside the terminal. The terminal multiplexes nothing; it only edits and shows text.

### 16.5. Save behavior

Saving writes the file to disk. The editor MUST NOT lose edits on a failed parse: a cart that does not parse is a normal state (the user is mid-edit), and the console pane reports what is wrong.

### 16.6. Asset editors

Sprite, map and audio editors are explicitly out of the first milestone ([§18](#18-deferred-features-and-non-goals)).

---

## 17. Versioning and compatibility

### 17.1. Versioning

This document's version is `MAJOR.MINOR`. The cart format carries its own version number in the binary header and the PNG blob header.

### 17.2. Major changes (break carts)

- Framebuffer size, palette size, tile size, map size
- RNG algorithm or default seed
- Drawing semantics
- Removing or renaming API functions
- PNG layout (size variants, blob header, payload encoding)
- Module system semantics
- `cart.toml` schema changes
- Binary cart format version

### 17.3. Minor changes (safe)

- Adding API functions
- Adding optional `cart.toml` keys
- Adding optional metadata keys
- Clarifying wording without changing behaviour
- New standard module functions

### 17.4. Compatibility

A runtime MUST load carts of an equal or older major version. It MUST reject a newer major version with a clear message rather than misinterpreting it. The binary format version (u16) increments on major changes. Minor spec changes do not change the binary format version.

### 17.5. Testing requirements

Any change to format, limits, RNG or API MUST land with: an updated table in this document, a changelog entry below, and a test fixture. Format changes additionally require a parse→serialize round-trip test.

**Conformance test vectors (normative, in `tests/conformance/`):**
- RNG: seed 0 → first 10 `rnd::float()` and `rnd::int(0, 100)` values
- Line: clipped segment from (-10, -10) to (330, 250) → exact pixel set
- Circle: `circle_filled(160, 120, 5)` → exact pixel set
- Palette: all 256 RGB555 words
- Font: 8×8 bitmap for printable ASCII 32–126 + placeholder
- PNG nibble walk: encode/decode round-trip for a known cart

---

## 18. Deferred features and non-goals

### 18.1. Deferred (not in v1, expected later, design leaves room)

- SFX synthesis and a pattern/tracker UI
- TUI sprite and map editors
- Sprite/map/audio asset import (beyond PNG placement)
- WASM web player
- Gamepads
- A second player
- Bulk palette operations
- A camera/transform stack
- Arbitrary sprite scaling and rotation
- Save data persistence (beyond `system::get_state`/`set_state` which is in-cart)
- Zlib compression for PNG payload
- Rhai formatter for `rico16 fmt`

### 18.2. Non-goals (will not happen)

- True color
- Alpha blending
- Hardware-layer (VRAM/OAM) emulation
- PICO-8 cart compatibility
- Short PICO-8-style function names (`cls`, `spr`, `rectfill`, `sspr`)
- Cart-supplied fonts
- Networking
- 3D
- Runtime file imports
- Private PNG chunks as the only payload
- `set_meta()` / `set_gfx()` — metadata is needed before execution, assets as code bloat the payload, and the cart becomes order-dependent executable data. Use `cart.toml`.
- `.r16` text format authoring (not for v3)
- Comment-fenced `__` sections

---

## 19. Decision log

| Decision | Choice | Why |
|---|---|---|
| Drawing model | Hybrid: immediate mode over persistent sprite/map data | Keeps the console simple and predictable while still allowing tile worlds |
| Color depth | 8-bit indexed only | Cheaper, enforces identity, palette swaps become free effects |
| True color | Dropped | Not worth the API and memory cost; a console needs a palette |
| Default palette | 256 entries: black, 15 base colours, 15 ramps × 16 | Authors never have to pick 256 colours; ramps make shading mechanical |
| Palette values | Generated HSV ramps, committed as data, pinned by a test | Unblocks the runtime; re-curation is a data edit, and packed carts keep their snapshot |
| RGB332 | A named palette preset, not the colour model | Same memory as indexed, but keeps palette effects and index-0 transparency |
| Tiles | 16×16 PU | 320×240 divides exactly into 20×15 |
| Sprite sheet | 256×256, 256 tiles | 4× PICO-8's pixel budget; fits the payload with code and map |
| Map cells | `u8` (16 KiB) | Matches 256 tile ids; `u16` would encode values that cannot exist |
| Tilemap scrolling | Stateless `tilemap::draw` | No hidden render state; scrolling is cart arithmetic |
| Camera | None in v1 | Deferred; predictability over convenience |
| `draw_world` | Rejected | Two addressing modes for one map is a bug generator |
| Palettes | Cart can override, console default | Enables fades and custom looks without hard-coding art direction |
| Bulk palette op | Deferred | 256 calls ≈ 0.2–0.3 ms; no speculative API |
| Frame skipping | Rejected | Does not help a slow update; breaks the frame contract |
| Scripting | Rhai | Pure Rust, embeddable, sandboxable, no C toolchain |
| API style | Namespaced, spelled out | Readability and discoverability beat token golf |
| PICO-8 compatibility | None | Never a requirement; avoided inventing constraints and legacy names |
| Entry points | `on_init` / `on_update(dt)` / `on_draw` | Reserved prefix, no collisions with user helpers |
| `dt` | Passed, always 1/60 | Costs nothing, leaves the door open for variable rate |
| Buttons | `Z X C V`, logical `Button` enum | Ergonomic row, position-based enum avoids "press A" ambiguity |
| Button naming | Positional (`::Z`, `::C`, …) | Names double as documentation on the physical keyboard |
| Input players | 1 exposed | API matches single-player; second player deferred |
| Project format | Directory with `cart.toml` + `src/` + `assets/` | Real modules, real assets, real build step |
| Module system | Real Rhai imports with embedded sources | Namespacing, transitive imports, module errors with paths |
| Standard modules | Importable + ambient globals | Ergonomics without sacrificing explicit imports |
| Cart distribution | PNG v1 (640×480 default, 1280×960 overflow) | ~378 KB / ~1.76 MB code capacity; real cover art |
| Payload encoding | Low nibbles of RGB (12 bits/px), A=0xFF | Survives lossless re-encoding; no private chunks; truly opaque |
| Code limit | None (PNG capacity only) | Our own limit, not OS/Rhai limit; reported used/capacity |
| Asset handling | Explicit placement in `cart.toml` | No filename conventions; overlaps/out-of-bounds rejected |
| Scene pattern | Modules are libraries, main owns state via `system::get_state` | Honest pattern; no hidden lifecycle hooks |
| RNG module | `rnd` module (float, int, choice, shuffle) | Namespaced, deterministic xoshiro256**, `system::seed` control |
| System menu | `Esc` opens runtime overlay; carts build own pause menus | No magic button; carts have full control |
| Frame pacing | At most 1 update per 1/60 s wall time | Correct speed on high-refresh displays |

---

## 21. Conformance vectors

A runtime or build that fails any of these is non-conforming.

### 21.1. RNG, seed 0

State after `seed_xoshiro256(0)`, four `u64`s:

```
e220a8397b1dcdaf
6e789e6aa1b965f4
06c45d188009454f
f88bb8a8724c81ec
```

First four `next_u64` outputs:

```
99ec5f36cb75f2b4
bf6e1f784956452a
1a5f849d4933e6e0
6aa594f1262d2d2c
```

Those draws as `rnd::float()`, IEEE bits:

```
3fe33d8be6d96ebe
3fe7edc3ef092ac8
3fba5f849d4933e0
3fdaa9653c498b4a
```

From a fresh seed 0, eight calls to `rnd::int(1, 6)` return `4, 5, 1, 3, 5, 6, 3, 4`.

From a fresh seed 0, `rnd::shuffle` of `[0, 1, 2, 3]` yields `[1, 0, 3, 2]`.

### 21.2. Line

Line `(0,0)`–`(4,-2)`, before screen clip: pixels in order `(0,0), (1,-1), (2,-1), (3,-2), (4,-2)`. On screen that is only `(0,0)`.

Line `(0,0)`–`(3,3)`: `(0,0), (1,1), (2,2), (3,3)`.

### 21.3. Disk

Disk of radius 2 centered at the origin:
- Outline pixels (relative): `(±2,0), (0,±2), (±1,±1)` — eight pixels.
- Filled pixel count at radius 0, 1, 2 is 1, 5, 13.

### 21.4. `rgb332`

| Index | RGB555 |
|---|---|
| 0 | `0000` |
| `0b11100000` | `001F` |
| `0b00011100` | `03E0` |
| 3 | `7C00` |
| 255 | `7FFF` |

### 21.5. Palette anchors

| Index | Word |
|---|---|
| 0 | `0000` |
| 5 | `7FFF` |
| 6 | `0C7F` |

Index 6 expands to RGB8 `(255, 24, 24)`.

### 21.6. CRC

`CRC-32("123456789") = CBF43926`.

### 21.7. `dt`

The bit pattern of `dt` is `3F91111111111111`.

### 21.8. Quantizers

`(255 * 31 + 127) / 255 = 31`. `(0 * 31 + 127) / 255 = 0`. `(128 * 31 + 127) / 255 = 15`.

`(255 * 15 + 127) / 255 = 15`. `(0 * 15 + 127) / 255 = 0`.

---

## 20. Changelog

- **3.0 (draft)** — Project-based development (`cart.toml`, `src/`, `assets/`). Real Rhai module imports with embedded sources. Standard modules importable and ambient. PNG v3: 640×480 default, 1280×960 overflow (~378 KB / ~1.76 MB code capacity). RGB-only payload (12 bpp), A=0xFF. No `CODE_LIMIT`. Explicit asset placement in `cart.toml`. Commands: `new`, `build`, `run`, `unpack`. Scene pattern: only entry point has `on_init`/`on_update`/`on_draw`; global state via `system::get_state`/`set_state`. System menu on `Esc`. Frame pacing for high-refresh displays. Complete RNG spec (xoshiro256**, SplitMix64 streaming, Lemire rejection, Fisher-Yates). Exact drawing algorithms (Bresenham DDA, integer disk, ink-only text). Default palette and font bytes in spec. Conformance vectors included. Removed: `.r16` text format, `__code__`/`__meta__`/`__gfx__` sections, `/* rico16:assets */` comments, `Button::Menu` magic button, `fmt` (deferred).