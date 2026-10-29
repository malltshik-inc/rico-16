# rico-16 Specification

**Version:** 1.0 (draft)
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
20. [Conformance vectors](#20-conformance-vectors)
21. [Reference implementation (informative)](#21-reference-implementation-informative)
22. [Changelog](#22-changelog)

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

4.2. **Palette index 0 is the transparency key** for all blitting operations (sprite and tilemap draws): a source pixel whose palette index is 0 is not written. It is not transparent on the display: after expansion, index 0 shows `palette[0]`. The default palette MUST have `palette[0]` = opaque black. A tile id of 0 is an ordinary tile, unrelated to this rule (§7.2).

4.3. Primitive drawing writes its color argument literally, including 0. This is the only way to erase: `gfx::rect_filled` with color 0 fills with index 0, which then displays as `palette[0]`. There is no "background color" concept.

4.4. `gfx::palette(index, r, g, b)` takes 8-bit components and quantizes with `round(v * 31 / 255)`, equivalently `(v * 31 + 127) / 255` in integer arithmetic. Out-of-range components MUST raise a script error.

4.5. Palette mutation is runtime state. It is restored on a cart reset ([§14.9](#149-quit-and-reset)). It is never written back to the cart file.

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

### 4.8. Why the palette is shaped this way

4.9. The build process resolves the effective palette into the cart at pack time, so a packed cart is unaffected by later changes to the default palette.

4.10. True color is not supported and MUST NOT be added without a major version bump. It is a deliberate non-goal, not a missing feature.

4.11. The console ships one **named palette preset**, `rgb332`, in addition to [`DEFAULT`](#4-palette-and-color): index `0bRRRGGGBB` is literally that colour, three bits of red, three of green, two of blue. It exists because some carts would rather read a colour off an index than walk a ramp, and because every attempt at direct-colour framebuffers eventually proposes it. It is a preset, not a colour model: the framebuffer stays indexed, so a cart using `rgb332` still gets palette swaps and still gets index 0 as a transparent key. It is not the default because two bits of blue means four blue levels.

`rico16 new <name> --palette rgb332` bakes the preset into the cart, after which it is ordinary palette data and the runtime cannot tell the difference.

4.12. **Default palette values (normative):** The exact 256 RGB555 words are the table in [§4.7](#4-palette-and-color). Any conforming runtime MUST use those words bit-for-bit. The HSV recipe above is informative only: if the recipe and the table disagree, the table wins.

4.13. **rgb332 expansion:** For `rgb332`, the 3-bit red/green and 2-bit blue are expanded to RGB555 by replicating the MSB into the LSBs: `r5 = (r3 << 2) | (r3 >> 1)`, `g5 = (g3 << 2) | (g3 >> 1)`, `b5 = (b2 << 3) | (b2 << 1) | (b2 >> 1)`. Equivalently, left-shift and fill low bits with the high bits.

---

## 5. Timing, the game loop, determinism

5.1. The console runs at **60 updates per second**. `dt` passed to `on_update` is exactly `1/60` seconds, as an f64 (`0.016666666666666666`).

5.2. Frame order:

```
load cart → build script engine → on_init() → [ on_update(dt) → on_draw() → present ] × N
```

`on_init` MUST be called exactly once per cart start. `on_update` MUST be called exactly once before each `on_draw`. Each logical frame performs exactly one `present`; see [§14.2](#142-window-and-presentation) for pacing on displays whose refresh rate is not 60 Hz.

5.3. **There is no catch-up.** If a frame takes longer than 1/60 s of wall clock, the runtime MUST NOT run extra updates to compensate; the cart simply runs slower than real time. Rationale: one update per frame keeps carts reproducible, keeps `dt` constant, and avoids the death spiral where the engine falls further behind because it is busy catching up.

5.4. `system::frames()` is a `u64`, the index of the current logical frame. It is **0** throughout the first `on_update`, the following `on_draw`, and that frame's present. It increments after present, so the next `on_update` observes the new value. It does not advance while the system menu is open, and it does not advance if `on_init` requests quit or reset before any frame.

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

**RNG lifecycle (normative):**

- There is a single RNG stream per cart run. It is initialized from the default seed 0 at cart start (before `on_init`) and persists across frames.
- `system::seed(n)` replaces the stream state immediately, at the point of the call.
- A reset ([§14.9](#149-quit-and-reset)) re-initializes the stream from seed 0.
- The RNG state is runtime state only: it is never written to the cart file, and it is not part of `state` ([§9.7](#97-global-state)).

Changing the algorithm or the default seed is a **major** version change: it breaks every recorded replay and every seeded cart.

5.6. Carts MUST be byte-identically reproducible across platforms for the same seed, inputs and frame count. Therefore:

- **f64 arithmetic only**; no `f32`, no fast-math, no FMA contraction differences.
- **NaN canonicalization:** Any operation producing NaN MUST yield the canonical quiet NaN (bit pattern `0x7FF8000000000000`). Signalling NaNs MUST NOT be produced. Carts MUST NOT rely on NaN payload bits.
- **Math functions.** The following free functions MUST be available to carts and MUST be bitwise-identical to the Rust `libm` crate **0.2.16** (musl-derived) for all inputs: `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `exp`, `ln`, `log10`, `pow`, `cbrt`, `hypot`, `sqrt`. Any other transcendental function the engine exposes MUST also match `libm` 0.2.16. The runtime provides these itself; it MUST NOT rely on a host standard library whose results may differ. The runtime does not have to be written in Rust. It has to match those bits. `libm` 0.2.16 is the reference, not a language requirement.
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

6.2. Arguments typed as integer MUST accept integer values. A float argument is converted to integer by **floor** (rounding toward negative infinity: `0.9 → 0`, `-0.9 → -1`). `NaN`, infinite, or finite-outside-`i64`-range floats where an integer is expected MUST raise a script error. **All NaN values are canonicalized to quiet NaN `0x7FF8000000000000` per §5.6.**

6.3. A color argument is an integer 0–255. Out-of-range values MUST raise a script error, never wrap.

6.4. Drawing is always **overwrite**: the destination index is replaced. There is no blending, no XOR mode, no masking beyond the index-0 key.

6.5. `gfx::text` renders an 8×8 fixed built-in font, advancing 8 PU per glyph, with `\n` moving down 8 PU and returning x to the call's start. The font covers printable ASCII 32–126; other code points (including multi-byte UTF-8, control chars, tabs) render as a placeholder box, **each counting as 1 glyph (8 PU)**. Only **ink** pixels are written; the other pixels of the 8×8 cell are left as they were. `gfx::text_width(str)` returns `first_line.chars().count() * 8` where `first_line` is the substring before the first `\n` (or the whole string if no `\n`). The font lives in the console, not in the cart, and there is no mechanism to replace it — carts cannot break text rendering, and text is one asset format we never have to invent.

**Font bitmap (normative, 8 rows × 8 cols, bit 0 = leftmost pixel):**

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

6.7. Circles are rasterized using the **integer disk** `dx² + dy² ≤ r²` (with `dx`, `dy`, `r` in `i128`) so that identical arguments produce identical pixels on every platform. `gfx::circle_filled(cx, cy, r, color)` fills all pixels satisfying the disk. `gfx::circle(cx, cy, r, color)` draws the boundary: pixels in the disk of radius `r` but not in the disk of radius `r-1`. Radius `-1` is the empty set, so `r = 0` is the single center pixel for both the filled and the outline form. `r < 0` is a no-op for both forms. Radius 1 filled is a 3×3 block (9 pixels); outline is 4 pixels (N, S, E, W). An implementation may draw `filled(r)` and skip pixels that `filled(r-1)` would cover; it must not paint the interior. Work is proportional to the number of screen rows the disk intersects; a disk that misses the screen entirely returns immediately.

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

6.9. `sprite::draw(id, x, y)` and overloads: `id` outside 0–255 raises a script error. `sprite::get_pixel(id, px, py)` and `set_pixel`: `px` or `py` outside 0–15 raises a script error. `sprite::draw_region(sx, sy, w, h, dx, dy, flip_x, flip_y)`: `w` or `h` ≤ 0 is a no-op; a source rectangle that extends outside the 256×256 sheet raises a script error. `gfx::palette(index, r, g, b)`: `index` outside 0–255 raises a script error.

6.9.1. `tilemap::get`/`tilemap::set` clip silently per §7.3: reads outside 0–127 return 0 and writes outside 0–127 are ignored. `tilemap::set` raises a script error if `id` is outside 0–255. `tilemap::draw` reads source cells outside 0–127 as tile id 0, consistent with `tilemap::get`.

---

## 7. Resources and limits

| Resource | Size | Encoding | What happens at the limit |
|---|---|---|---|
| Framebuffer | 76,800 B | 1 byte/px | fixed |
| Sprite sheet | 65,536 B (256×256, 256 tiles of 16×16) | 1 byte/px | fixed |
| Tilemap | 16,384 B (128×128 cells) | 1 byte/cell | fixed; out-of-range access is silent (§7.3) |
| Palette | 512 B | RGB555 × 256 | fixed |
| Cart code | no fixed source limit | UTF-8 Rhai | the PNG capacity in §13 |
| Script operations | 1,000,000 per entry-point call | Rhai 1.27.0 operation counter | script error |
| Script call depth | 64 nested calls | | script error |
| Array length | 65,536 elements | | script error on the call that would exceed it |
| Map length | 65,536 entries | | script error |
| String length | 1,048,576 bytes | UTF-8 | script error |

7.1. Tiles are **16×16 PU**. A 320×240 screen shows exactly **20×15** cells. Tile id = `row * 16 + col` on a 16-column sheet; ids are 0–255.

7.2. The map is **128×128 cells** of 16×16 PU, i.e. a 2048×2048 PU world. Cell values are tile ids 0–255. Every id draws; there is no implicit "empty" tile. Carts that need empty space reserve an id for it.

7.3. `tilemap::get` reads outside 0–127 return 0. `tilemap::set` writes outside 0–127 are ignored. This is deliberate silent clipping so that procedural map code can run off the edges; carts MUST NOT rely on it to detect bugs.

7.4. **There is no separate code limit.** The only limit is the PNG container payload capacity after assets are encoded. The build fails only when the payload exceeds the container, and reports used/capacity.

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

8.2. The sprite sheet and the tilemap are mutable data structures. Their initial contents are stored in the cart; at runtime they persist across frames, are read by blits, and are edited by API. Runtime edits last only for the current run and are lost on reset ([§14.9](#149-quit-and-reset)). They are **not** surfaces: they are never composited, never scrolled by hardware, and have no draw order of their own.

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
- The global variable name `state` ([§9.7](#97-global-state)) is reserved: a cart MUST NOT declare or shadow it.
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
| `system::quit()` | ends the cart after the current frame and exits (see §14.9) |
| `system::quit(message)` | same, printing `message` to stdout (editor: console pane) first |
| `system::reset()` | resets the cart from its original bytes after the current frame (see §14.9) |

**Output**

| Function | Notes |
|---|---|
| `print(value)` | writes `value` as one line to stdout (editor: console pane) |
| `debug(value)` | identical to `print`; for development |

**Audio (stubbed in v1)**

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

The reserved module names are: `gfx`, `sprite`, `tilemap`, `input`, `system`, `audio`, `rnd`, `Button`. A cart MUST NOT define its own module with these names. The global variable `state` is likewise reserved ([§9.7](#97-global-state)).

### 9.7. Global state

The runtime provides one **global variable named `state`**, visible to the entry point and to every module. It is the **only** way to persist data across frames. Its structure is free: it may hold any value — an object map, an array, a number, anything.

- At cart start (before `on_init`), `state` is an empty object map (`#{}`).
- Reading, mutating in place (`state.px = 5`, `state.items.push(x)`), and reassigning the whole variable (`state = [...]`) all persist across frames and are visible from every function, including modules.
- The cart owns `state` completely. The runtime never inspects it; it only clears it on reset ([§14.9](#149-quit-and-reset)). There is no getter, no setter, and no schema.
- The name `state` is reserved: a cart MUST NOT declare or shadow it ([§9.1](#91-naming-rules)).
- `state` is **not** serialized into the cart binary. A reset clears it back to `#{}`. Save data persistence is deferred ([§18.1](#181-deferred-not-in-v1-expected-later-design-leaves-room)).

**Pattern:**

```rhai
fn on_init() {
    state.px = 160;
    state.py = 120;
    state.scene = "title";
    state.enemies = [];
}

fn on_update(dt) {
    if state.scene == "play" {
        state.px += 1; // example
    }
}
```

### 9.8. Engine configuration and sandbox

The runtime MUST configure the script engine so that a cart's behaviour is fully determined by the cart and this document:

- **Available to carts:** the Rhai language, the global `state` variable ([§9.7](#97-global-state)), the standard modules ([§9.6](#96-standard-modules-as-ambient-globals)), the math and string/array/map operations Rhai provides (subject to §5.6), and `print`/`debug` ([§9.3](#93-surface)). `import` resolves only embedded modules and the reserved standard modules ([§11.2](#112-build-time-resolution)).
- **Unavailable to carts:** dynamic evaluation (`eval`), wall-clock or monotonic time, host randomness, filesystem, network, environment, process and OS access, and any module not embedded in the cart. Calling one of these is a script error, never a no-op.
- **Operation limit:** the limit in [§7](#7-resources-and-limits) applies per entry-point call. The counter resets at the start of `on_init` and at the start of each `on_update` and each `on_draw`. Module functions called during a callback draw from the same per-call budget.
- **Call depth:** recursion is limited per the [§7](#7-resources-and-limits) table. Exceeding it is a script error; it MUST NOT crash or hang the host.

A cart MUST NOT depend on any engine behaviour that this document does not fix.

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
- An import path that matches a reserved standard-module name (`gfx`, `sprite`, `tilemap`, `input`, `system`, `audio`, `rnd`, `Button` — see [§9.6](#96-standard-modules-as-ambient-globals)) resolves to the native standard module and is never looked up in `src/`.
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

- Modules have **no persistent state of their own**. Module-level `let`/`const` are not visible inside module functions, and module functions cannot see main's local variables.
- Module functions **can** read and write the global `state` ([§9.7](#97-global-state)), which is the shared channel between main and its modules.
- Transitive imports and standard native modules under an alias work.

### 11.5. Scene pattern

Only `main.rhai` (the entry point) has the runtime entry points. A module's `on_init`/`on_update`/`on_draw` are ordinary functions and are never called automatically. Name scene hooks `update`/`draw` and dispatch from main:

```rhai
// scenes/play.rhai
fn update(dt) {
    if input::held(Button::Left) { state.px -= 2; }
}

fn draw() {
    gfx::clear(1);
    gfx::rect_filled(state.px - 4, state.py - 4, 8, 8, 9);
}

// main.rhai
import "scenes/play" as play;

fn on_init() {
    state.px = 160;
    state.py = 120;
    state.scene = "play";
}

fn on_update(dt) {
    if state.scene == "play" {
        play::update(dt);
    }
}

fn on_draw() {
    if state.scene == "play" {
        play::draw();
    }
}
```

That is the honest pattern: modules are libraries with no state of their own, and the cart's data lives in the single global `state`, which main and modules both use.

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
- If the payload exceeds the 1280×960 capacity, the build fails and reports used/capacity.
- Payload compression is reserved for a future format (a flag in the binary header); v1 embeds the cart uncompressed. This is unrelated to the PNG container's own zlib-compressed `IDAT`, which every PNG requires and which the build MUST always emit ([§13.6](#136-determinism)).

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
Additional keys from `[cart]` are preserved. Unknown keys are preserved. The metadata section is authoritative for cart metadata; other copies (the advisory `tEXt` chunk of [§13.3](#133-advisory-metadata-chunk), a source `cart.toml`) are not consulted by the runtime.

Maximum size depends on the PNG container. The `flags` field is reserved: it MUST be 0 on write, and loaders MUST ignore unknown bits. A palette is always present, because packing always resolves one into the cart.

### 13.2. PNG v1: The image is the label

The PNG is **640×480 (default) or 1280×960 (overflow), 8-bit RGBA, non-interlaced** — a completely ordinary image with no private chunks and no reserved banner. The *entire* image is cover art, and the cart data is hidden underneath it in the low four bits of every **R, G, B** channel. The alpha channel is forced to `0xFF` (fully opaque).

13.2.1. **Visible art.** The high nibble of the R, G and B bytes is the cover art, 4 bits per channel (RGB444, 4096 colors). The low nibble of R, G, B carries payload.

**The low nibbles are visible.** They perturb each channel by up to 15/255, which is one RGB444 step. On a flat field they look like grain. This spec does not pretend otherwise. Author labels as flat pixel art, not as photographs. Alpha is not a data channel: putting the payload in alpha made every pixel 240–255 and the picture see-through. That was wrong.

13.2.2. **Payload.** Reading pixels in row-major order from (0,0), the low nibbles of R, G, B are concatenated into a nibble stream (3 nibbles per pixel), and consecutive pairs of nibbles form payload bytes, first nibble as the high half. That yields **12 bits per pixel** and a total capacity of:
- 640×480: 460,800 bytes (460,784 after 16-byte header)
- 1280×960: 1,843,200 bytes (1,843,184 after 16-byte header)

13.2.3. The payload begins with the blob header below, then the binary cart ([§13.1](#131-binary-cart)). Nibbles after the end of the payload MUST be zero. Because the authored art has no low-nibble information by definition, filling them cannot damage it.

```
offset  field
0       magic "R16P"
4       blob version (u16) = 1
6       PNG size variant (u16): 0 = 640×480, 1 = 1280×960
8       payload length (u32) — length of the binary cart that follows
12      CRC-32 (IEEE) of that payload
```

13.2.4. **Cover art.** `rico16 build` takes `--label <image>` or generates a label when none is given. The generated label is deterministic in content — a dark field, the cart title rendered in the built-in 8×8 font, the author line, and a palette strip of the cart's own colours — so that packing the same cart twice produces the same visible cover art. Its exact geometry is implementation-defined, and byte-identical files are not required ([§13.6](#136-determinism)).

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

`rico16 build` SHOULD also emit a `tEXt` chunk with keyword `rico16` and value `v=1;variant=<0|1>;len=<payload length>;crc=<8 uppercase hex>`. Tools MAY use it to identify carts without decoding pixels. The pixel payload is authoritative; if the chunk and the pixels disagree, the runtime MUST trust the pixels and SHOULD warn.

### 13.4. Reading a cart PNG v1

A loader MUST, in order:

1. verify the PNG is a valid, non-interlaced, 8-bit RGBA image whose pixel dimensions are exactly the dimensions of the declared variant (640×480 for variant 0, 1280×960 for variant 1);
2. verify the blob magic is `R16P` and the blob version is 1;
3. verify the payload length fits the variant's capacity and does not exceed the bytes available after the 16-byte blob header;
4. verify the CRC-32 of the payload;
5. verify that every nibble after the end of the payload is zero;
6. parse the binary cart ([§13.1](#131-binary-cart)).

Any mismatch MUST be a clear error naming the field that failed. A loader MUST NOT attempt to guess or repair a corrupt cart.

**Binary cart validation.** Verify the cart magic `R16C` and the format version. Verify that the three length fields are consistent: the bytes after the 82,452-byte fixed header (magic + format version + flags + the three length fields + sprite sheet + tilemap + palette) MUST equal `code_len + modules_len + metadata_len` exactly, with nothing left over and nothing missing. Verify the module table does not overrun `modules_len`. Verify every palette entry has bit 15 clear. Verify module paths are unique, relative, free of the `.rhai` extension, and sorted lexicographically. Verify the metadata section is valid UTF-8 TOML.

### 13.5. Durability rules

13.5.1. Lossless PNG re-encoding (optimizers, `optipng`, CDNs, most social platforms) preserves carts, because the payload lives in pixel values rather than chunks or trailing bytes. Lossy transcoding — JPEG, lossy WebP, palette reduction, or any resize — destroys them. Document this wherever carts are shared: a cart is an image, and images get re-encoded.

13.5.2. Because the payload is spread across the whole image, a cart PNG MUST NOT be resized, cropped, or colour-reduced by any tool that "prepares images for the web". This is the price of having real cover art, and it is the same price PICO-8 pays.

13.5.3. Alpha is always `0xFF` and carries no payload. Because alpha is constantly 255, a tool that drops the alpha channel and keeps RGB **does not** destroy the cart. A tool that requantizes RGB does. The distribution warning is: do not resize, do not JPEG, do not "optimize" colors. Alpha stripping is survivable.

13.5.4. **File size.** The payload occupies the low nibbles of every pixel and is effectively incompressible, so a cart PNG is much larger than its cover art alone would suggest (roughly the size of the raw RGB data). This is expected: a 640×480 cart is on the order of 1 MB and a 1280×960 cart on the order of 4 MB. Distributors MUST NOT apply "image optimization" to reduce it.

### 13.6. Determinism

The **binary cart payload** MUST be deterministic: the same project source produces the same payload bytes, so the embedded cart (magic, length, CRC) is reproducible and comparable.

The **PNG container bytes** need not be byte-identical. Different encoders, DEFLATE implementations, or compression levels may produce different files for the same payload; this is acceptable because the payload is what runs. Two carts with the same payload behave identically.

To keep payloads comparable, the build MUST:

- emit the payload with the exact bytes defined by [§13.1](#131-binary-cart) and [§13.2](#132-png-v1-the-image-is-the-label);
- sort module table entries by path ([§13.1](#131-binary-cart));
- embed no timestamps, random data, absolute paths, or host-specific ordering in the payload.

The PNG wrapper MUST be a valid **8-bit RGBA, non-interlaced** image of the variant size with alpha `0xFF` and no palette, so the payload survives a lossless round-trip. The encoder is otherwise free: any lossless DEFLATE backend and compression settings are acceptable, and the generated default label only needs to be deterministic enough that a re-pack of the same project is recognisably the same cart (it is **not** required to be byte-identical).

Tools that compare carts MUST compare the payload (magic, length, CRC), never the whole file. In particular, a `build` after `unpack` is **not** required to reproduce the original project, its cover art, or its file bytes: `unpack` produces a canonical project ([§15.4](#154-rico16-unpack-png-output-dir)) whose rebuild carries the same payload.

---

## 14. Runtime and player

### 14.1. Cart loading

The player accepts two inputs:
- A project directory (path containing `cart.toml`) — builds in memory, then runs.
- A packed PNG (v1) — loads directly.

### 14.2. Window and presentation

The player opens a single window, scaled per [§3.7](#3-display-and-framebuffer). Each logical frame is presented exactly once, synchronised to vsync. On displays whose refresh rate is not a multiple of 60, judder is expected and accepted.

**Frame pacing:** The runtime MUST NOT begin a new logical frame sooner than 1/60 s of wall time after the previous one, and MUST NOT present more than once per logical frame. On a display whose refresh interval is shorter than 16.67 ms (for example 144 Hz), the runtime simply waits for the next vsync boundary before starting the next logical frame; the intermediate vsync intervals produce neither an update nor a present. `dt` is always `1/60`. `system::frames()` and `system::time()` advance only on logical frames.

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

Reserved keys, never visible to a cart: `Esc` (opens system menu), `F11` (toggle fullscreen), `Ctrl+R` (reset cart). A cart MUST NOT be able to bind them.

### 14.5. System menu

`Esc` opens the **system menu**, a runtime-provided overlay that pauses the cart. The system menu provides:

- **Resume** — closes the menu, continues execution
- **Restart** — resets the cart (see [§14.9](#149-quit-and-reset))
- **Exit** — quits the cart and closes the window

The system menu is drawn by the runtime using a **separate overlay buffer** (not the cart framebuffer). It does not count against the cart's draw budget. While the menu is open, `on_update` and `on_draw` are not called; logical time does not advance (`system::frames()` and `system::time()` pause).

**Cart-owned pause menus:** Carts that need an in-game pause menu (triggered by a gameplay button) must implement it themselves using `state` and conditional drawing. There is no `Button::Menu` magic button.

### 14.6. Input sampling

Input is sampled once per frame, immediately before `on_update`. Edge state (`pressed`, `released`) compares the current frame's bitmask with the previous frame's; on the first frame the previous bitmask is all zeros. While the system menu is open, the cart's input bitmask is frozen (held buttons remain held, no new pressed/released events are generated).

### 14.7. Focus loss

On window focus loss the runtime MUST clear all held buttons. Key-repeat events MUST NOT be treated as new presses.

### 14.8. Error policy

- **Player mode:** print `<file>:<line>:<col>: <message>` to stderr, close the window, exit with code 1. The process MUST NOT panic.
- **Editor mode:** halt the cart, keep the editor alive, show the message in the console pane. See [§16](#16-editor).

### 14.9. Quit and reset

There are two distinct ways to end a cart run:

- **Quit.** `system::quit()` ends the cart after the current frame and exits the process with code 0. `system::quit(message)` prints `message` to stdout first (in the editor, to the console pane) and exits the same way. A message is not an error and MUST NOT go to stderr.
- **Reset.** `system::reset()` ends the current run after the current frame and immediately starts a new run from the cart's original bytes, without exiting the process. The runtime re-runs `on_init`, resets `system::frames()` to 0, re-initializes the RNG from seed 0 ([§5.5](#5-timing-the-game-loop-determinism)), restores the cart's palette, clears `state` back to `#{}`, and discards all other runtime state. Nothing is preserved across a reset.

The system menu ([§14.5](#145-system-menu)) uses both: **Restart** performs a reset, and **Exit** performs a quit. `system::reset()` is the script-facing equivalent of the editor's hot reload ([§16.3](#163-required-actions)). Reset is not a quit: it MUST NOT exit the process or be reported as an error.

### 14.10. Audio

There is no audio output in v1; `audio::play` is accepted and ignored. The audio device MUST NOT be opened. Audio is provisional and deferred; see [§18.3](#183-scope-notes).

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
- Fails with a clear message if the payload exceeds the forced or overflow capacity.
- Reports used/capacity bytes on success.
- Exits with code 0 on success, non-zero on error with clear message.

### 15.3. `rico16 run <project|png>`

Runs a cart.
- If argument is a directory (contains `cart.toml`): builds in memory, then runs.
- If argument is a `.png` file: loads and runs directly (v1).
- No separate build step needed for project directories.

### 15.4. `rico16 unpack <png> <output-dir>`

Inspects a shared cart PNG.
- Reconstructs canonical full sprite sheet (256×256) as `assets/sprites.png`, tilemap (128×128) as `assets/map.png`, palette (256×1) as `assets/palette.png`.
- Extracts all embedded module sources to `src/` preserving directory structure.
- Writes the entry-point source to the path recorded in the cart metadata `entry` (default `src/main.rhai`), creating parent directories as needed, and sets `entry` in the written `cart.toml` to that path.
- Writes a `cart.toml` with metadata from the cart and **canonical asset placements**:
  ```toml
  format = 1

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
- **Does not** reconstruct the original art layout (strip files) or the cover art; only the canonical compiled form.
- `unpack` is in scope and is the supported way to share a project as a single file; see [§18.3](#183-scope-notes).

### 15.5. `rico16 fmt <project>`

Formats all `.rhai` files in `src/` using a Rhai formatter (deferred; may be removed if no formatter exists).

---

## 16. Editor

### 16.1. Overview

The editor is a ratatui terminal application (crossterm backend). It is a separate mode of the same binary: `rico16 edit <project>`. The editor is a convenience, not a requirement — any IDE or text editor can edit a project. It starts minimal and grows feature by feature; see [§18.3](#183-scope-notes).

### 16.2. Required panes (first milestone)

- Project file tree (src/, assets/, cart.toml)
- Code editor with Rhai syntax highlighting
- Console (script errors, warnings, `print`-style output)
- Palette inspector

### 16.3. Required actions

- Run, stop, restart
- **Build-on-run**: `rico16 run` semantics from the editor
- **Rebuild on save**: when a `.rhai` or asset file is saved, rebuild in background; hot reload the running cart by resetting it (re-run `on_init`; nothing is preserved, per [§14.9](#149-quit-and-reset))

### 16.4. Game view

The game runs in its own native window. A 320×240 indexed framebuffer needs
real pixels, and a terminal cannot show it faithfully; the terminal multiplexes
nothing and only edits and shows text. That window is the **display of record**.

The editor MAY additionally show a live **embedded preview** of the running cart
in a terminal pane, for convenience while editing. The preview is a
best-effort, downsampled, palette-quantized view; its size, resolution and
colour fidelity are implementation-defined and it is not the display of record.
A cart MUST behave identically whether it is previewed in the terminal or run in
a window: the preview MUST drive the same runtime with the same fixed timestep
and the same input, and MUST NOT change what the cart observes.

An editor that offers both MUST make it obvious which one is running, and MUST
make the way to run, stop and get help visible without documentation.

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

A runtime MUST load carts of an equal or older major version. It MUST reject a newer major version with a clear message rather than misinterpreting it. The binary cart format version and the PNG blob version (both `u16`) increment on major changes. Minor spec changes do not change either version.

### 17.5. Testing requirements

Any change to format, limits, RNG or API MUST land with: an updated table in this document, a changelog entry, and a test fixture. Format changes additionally require a parse→serialize round-trip test.

The normative conformance vectors live in [§20](#20-conformance-vectors). A runtime or build that fails any of them is non-conforming. Vectors MUST be extended, never weakened, when behaviour is added.

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
- Save data persistence (beyond `state`, which is in-cart and cleared on reset)
- Compression of the embedded binary-cart payload (distinct from the PNG container's own `IDAT`)
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
- `.r16` text format authoring (not for v1)
- Comment-fenced `__` sections

### 18.3. Scope notes

The items below are **deliberately staged**: the priority is a working engine and playable games first, after which these areas are filled in. Nothing here is rejected.

- **Payload error-correction.** The cart payload carries only a CRC-32, which *detects* corruption but cannot *repair* it ([§13.5](#135-durability-rules)). Redundancy or erasure coding is a later concern.
- **Audio.** Audio is too large and too under-specified to include now. `audio::play` stays a stub ([§14.10](#1410-audio)); a full sound and pattern model will be specified later.
- **Editor.** A graphical editor is not required to make games — any IDE or text editor can edit `cart.toml`, `src/` and `assets/`. The reference editor ([§16](#16-editor)) starts minimal (open a project, edit text, build and run, console) and grows feature by feature. It is a convenience, not a dependency.

Two things are explicitly **in scope** and will be built:

- **`unpack`.** `rico16 unpack` ([§15.4](#154-rico16-unpack-png-output-dir)) is implemented. It is how a project is shared as a single PNG — for example over a chat app — and unpacked back into a working project. It reconstructs the canonical compiled form, not the original art layout or cover art. There is no hosted cart library.
- **Multi-platform support.** Windows, macOS and Linux are all first-class from the first release.

These sections will be revised as the engine and its games mature.

---

## 19. Decision log

| Decision | Choice | Why |
|---|---|---|
| Drawing model | Hybrid: immediate mode over persistent sprite/map data | Keeps the console simple and predictable while still allowing tile worlds |
| Color depth | 8-bit indexed only | Cheaper, enforces identity, palette swaps become free effects |
| True color | Dropped | Not worth the API and memory cost; a console needs a palette |
| Default palette | 256 entries: black, 15 base colours, 15 ramps × 16 | Authors never have to pick 256 colours; ramps make shading mechanical |
| Palette values | Generated HSV ramps, committed as the normative table | Unblocks the runtime; re-curation is a data edit, and packed carts keep their snapshot |
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
| Global state | One reserved global variable `state`, resolved by the engine | Free structure, no get/set boilerplate, clone-free, visible to modules |
| Scene pattern | Modules are libraries with no state of their own; data lives in `state` | No state threading, no hidden lifecycle hooks |
| RNG module | `rnd` module (float, int, choice, shuffle) | Namespaced, deterministic xoshiro256**, `system::seed` control |
| System menu | `Esc` opens runtime overlay; carts build own pause menus | No magic button; carts have full control |
| Restart | A separate reset, not a quit | Restart must reload the cart without killing the process |
| Frame pacing | One present per logical frame; at most 60 logical frames/s | Correct speed on high-refresh displays |
| Versions | One format version (`= 1`) across binary cart, blob, and `cart.toml` | No parallel version numbers to keep in sync |
| Build determinism | The payload must be deterministic; the PNG file bytes need not be | The payload is what runs; container bytes are an encoding detail |
| Language | Rust | One toolchain, no C dependency, pure-Rust windowing |
| Window/presentation | `winit` + software framebuffer (`softbuffer`/`pixels`) | Pure Rust, no GPU/shaders, integer scaling is trivial |
| Platforms | Windows, macOS and Linux from day one | A console should run everywhere |
| Rhai | One pinned version, minimal feature set | Stability and safety over feature breadth |
| Math (first cut) | Platform `std`; `libm` bit-exactness is later | Playable games first; §5.6 hardening is tracked |
| `unpack` | Implemented | Single-file project sharing without a hosted library |
| Editor | Minimal first, grown feature by feature | Not required to make games |
| Milestone scope | Engine and playable games first; audio and payload error-correction later | Playability before polish; see [§18.3](#183-scope-notes) |

---

## 20. Conformance vectors

A runtime or build that fails any of these is non-conforming.

### 20.1. RNG, seed 0

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

### 20.2. Line

Line `(0,0)`–`(4,-2)`, before screen clip: pixels in order `(0,0), (1,-1), (2,-1), (3,-2), (4,-2)`. On screen that is only `(0,0)`.

Line `(0,0)`–`(3,3)`: `(0,0), (1,1), (2,2), (3,3)`.

### 20.3. Disk

Disk of radius 2 centered at the origin:
- Outline pixels (relative): `(±2,0), (0,±2), (±1,±1)` — eight pixels.
- Filled pixel count at radius 0, 1, 2 is 1, 5, 13.

### 20.4. `rgb332`

| Index | RGB555 |
|---|---|
| 0 | `0000` |
| `0b11100000` | `001F` |
| `0b00011100` | `03E0` |
| 3 | `7C00` |
| 255 | `7FFF` |

### 20.5. Palette anchors

| Index | Word |
|---|---|
| 0 | `0000` |
| 5 | `7FFF` |
| 6 | `0C7F` |

Index 6 expands to RGB8 `(255, 24, 24)`.

### 20.6. CRC

`CRC-32("123456789") = CBF43926`.

### 20.7. `dt`

The bit pattern of `dt` is `3F91111111111111`.

### 20.8. Quantizers

`(255 * 31 + 127) / 255 = 31`. `(0 * 31 + 127) / 255 = 0`. `(128 * 31 + 127) / 255 = 16`.

`(255 * 15 + 127) / 255 = 15`. `(0 * 15 + 127) / 255 = 0`.

### 20.9. Global state

With `state` starting as `#{}`:

- After `on_init` runs `state.n = 0` and `on_update(dt)` runs ten times doing `state.n += 1`, `state.n == 10`. Mutation in `on_update` is visible on the next frame.
- `on_draw` may mutate `state` as well; mutations made there are visible on the next frame.
- A module function that does `state.n += 1` mutates the same `state` the entry point sees.
- Reassigning the whole variable (`state = #{ n: 7 }`) inside a function that takes parameters is visible to the next call, and `state` may change type (for example to an array).

---

## 21. Reference implementation (informative)

This section records the choices made for the reference implementation. It is **informative**: it adds no conformance requirements, and another implementation may choose differently as long as it satisfies the normative sections.

- **Language:** Rust.
- **Windowing and presentation:** `winit` for the window and input, with a pure-Rust software framebuffer (`softbuffer` or `pixels`) blitted with integer scaling ([§3.7](#3-display-and-framebuffer)). No GPU shaders.
- **Scripting:** Rhai, pinned to one version (`1.27.0`) and built with the minimum feature set needed to make games, with the sandbox of [§9.8](#98-engine-configuration-and-sandbox) enforced.
- **Math:** the first cut uses the platform's standard floating-point math (`std`). Matching `libm` 0.2.16 bit-for-bit (§5.6) is a later hardening step; until it lands, the first cut is a known, tracked gap against §5.6 and cross-platform bit-identical math is best-effort.
- **PNG:** a simple lossless PNG encoder/decoder. The payload must be deterministic, but the PNG file bytes need not be ([§13.6](#136-determinism)); any lossless DEFLATE backend is acceptable.
- **Platforms:** Windows, macOS and Linux are all first-class from the first release.
- **Distribution:** `unpack` ([§15.4](#154-rico16-unpack-png-output-dir)) is the sharing mechanism — a project travels as one PNG (for example over a chat app) and is unpacked back into a working project. There is no hosted cart library.
- **Editor:** minimal first (open a project, edit text, build and run, console) and grown feature by feature ([§18.3](#183-scope-notes)).

---

## 22. Changelog

- **1.0 (draft), correction** — §6.5: the font bitmap is **bit 0 = leftmost pixel**. The normative bytes are, and always were, the LSB-first public-domain 8×8 font; the prose previously said "bit 7 = leftmost", which renders every glyph mirrored. The bytes are unchanged; only the description was wrong. Implementations that followed the prose literally must flip the bit order.

- **1.0 (draft), amendment** — §16.4: the native window is named the display of record, and the editor may now show an embedded terminal preview. The preview is explicitly best-effort and must not change what a cart observes. The editor is required to make run/stop/help discoverable on screen.

- **1.0 (draft)** — Initial specification. Project-based development (`cart.toml`, `src/`, `assets/`). Real Rhai module imports with embedded sources. Standard modules importable and ambient. PNG v1: 640×480 default, 1280×960 overflow (~378 KB / ~1.76 MB code capacity). RGB-only payload (12 bpp), A=0xFF. No code limit. Explicit asset placement in `cart.toml`. Commands: `new`, `build`, `run`, `unpack`. Scene pattern: only the entry point has `on_init`/`on_update`/`on_draw`; global state via a single reserved `state` variable. System menu on `Esc`. Frame pacing for high-refresh displays. Complete RNG spec (xoshiro256**, SplitMix64 streaming, Lemire rejection, Fisher-Yates). Exact drawing algorithms (Bresenham DDA, integer disk, ink-only text). Default palette and font bytes in the spec. Conformance vectors included. Build determinism is defined for the cart payload, not the PNG file bytes. Reference implementation choices (Rust, `winit` + software framebuffer, pinned Rhai, Windows/macOS/Linux) are recorded in [§21](#21-reference-implementation-informative).
