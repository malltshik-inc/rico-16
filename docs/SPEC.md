# rico-16 Specification

**Version:** 0.1 (draft)
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
10. [Cart text format (`.r16`)](#10-cart-text-format-r16)
11. [Cart binary format and PNG packing](#11-cart-binary-format-and-png-packing)
12. [Runtime and player](#12-runtime-and-player)
13. [Editor](#13-editor)
14. [Versioning and compatibility](#14-versioning-and-compatibility)
15. [Deferred features and non-goals](#15-deferred-features-and-non-goals)
16. [Open questions](#16-open-questions)
17. [Decision log](#17-decision-log)
18. [Changelog](#18-changelog)

---

## 1. Scope

rico-16 is a fantasy console: a fixed virtual machine with fixed resources, a fixed palette-driven display and an embedded scripting language. This document defines the observable behaviour of the console — what a cart may rely on and what a conforming runtime must do.

It does **not** define internal implementation, crate layout actualities, or the editor's user interface beyond the requirements in [§13](#13-editor).

The three things that make rico-16 a console rather than a game framework:

1. The display is a fixed 320×240 indexed framebuffer. There is exactly one surface.
2. Resources are strictly bounded and known at load time.
3. The script API is complete enough that a cart never needs a second language, a plugin, or access to the host.

## 2. Conformance language

The key words **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT** and **MAY** are to be interpreted as described in RFC 2119.

A **conforming runtime** is a program that loads a cart and satisfies every MUST in this document. A **conforming cart** is a `.r16` file that satisfies every MUST in [§10](#10-cart-text-format-r16).

PU (pixel units) are integer screen pixels. RGB555 words are 16-bit little-endian unless stated otherwise.

## 3. Display and framebuffer

3.1. The display is **320 PU wide and 240 PU tall**. The origin `(0, 0)` is the top-left pixel, `+x` is right, `+y` is down.

3.2. The framebuffer is a single array of **76,800 bytes**, one byte per pixel, row-major, stride 320. Byte values are palette indices, not colors.

3.3. There is exactly one surface and no compositing. Sprites and the tilemap are *data*, not layers ([§8](#8-drawing-model)).

3.4. The framebuffer is **not** cleared automatically between frames. A cart that does not call `gfx::clear` sees the previous frame's contents.

3.5. At cart start the framebuffer is all zeros (index 0).

3.6. The framebuffer is expanded through the palette to RGBA8 at presentation time. Expansion is exact: `rgba = ((r5 << 3) | (r5 >> 2), ...)`, with alpha 255. There is no gamma correction, no color management, and no dithering applied by the runtime.

3.7. Presentation is **nearest-neighbour** with **integer scaling only**, letterboxed with opaque black. A conforming runtime MUST NOT use bilinear or any other filtering. The default window is 3× (960×720); the actual scale is the largest integer that fits the drawable area, minimum 1.

3.8. Curved-screen CRT effects, scanlines, aspect correction and similar are out of scope for the runtime. Carts that want them draw them.

## 4. Palette and color

4.1. The palette has **256 entries**. Each entry is an **RGB555** word: bits 0–4 red, bits 5–9 green, bits 10–14 blue, bit 15 reserved and MUST be 0. Component values are 0–31. Total palette size is **512 bytes**.

4.2. **Index 0 is the transparency key** for all blitting operations (sprite and tilemap draws). It is not transparent on the display: after expansion, index 0 shows `palette[0]`. The default palette MUST have `palette[0]` = opaque black.

4.3. Primitive drawing writes its color argument literally, including 0. This is the only way to erase: `gfx::rect_filled` with color 0 fills with index 0, which then displays as `palette[0]`. There is no "background color" concept.

4.4. `gfx::palette(index, r, g, b)` takes 8-bit components and quantizes with `round(v * 31 / 255)`, equivalently `(v * 31 + 127) / 255` in integer arithmetic. Out-of-range components MUST raise a script error.

4.5. Palette mutation is runtime state. It resets when the cart restarts. It is never written back to the cart file.

4.6. There is **no alpha blending** anywhere. Fades, shadows and transparency effects are done by (a) mutating the palette, or (b) the index-0 key, or (c) dithering drawn by the cart.

4.7. **A cart author never has to choose 256 colors.** The console ships a default palette, a new cart starts with it, and most carts never touch it. The palette is versioned ([§14](#14-versioning-and-compatibility)) and identical on every platform, and it has a fixed structure so that indices are portable between carts:

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

The table is **committed as data** in the reference implementation and pinned by a test, so the console's colours are a fixed artefact rather than a startup computation. Its first version was generated from an HSV ramp curve (value 0.15→1.0, saturation 0.85→0.55 across each ramp) and is expected to be re-curated by hand as the art direction settles. Re-curating affects new carts and future packs only: a packed cart carries its own resolved palette ([§4.9](#4-palette-and-color)).

4.8. Carts MAY ship their own palette ([§10.6](#106-__palette__)) and MAY mutate entries at runtime ([§4.4](#4-palette-and-color)). Because art stores indices rather than colors, replacing the palette re-colors every pixel for free — this is how day/night cycles, damage flashes, fades and alternate visual themes are done, with no extra art.

4.9. `rico16 pack` resolves the effective palette into the cart at pack time, so a packed cart is unaffected by later changes to the default palette.

4.10. True color is not supported and MUST NOT be added without a major version bump. It is a deliberate non-goal, not a missing feature.

4.11. The console ships one **named palette preset**, `rgb332`, in addition to [`DEFAULT`](#4-palette-and-color): index `0bRRRGGGBB` is literally that colour, three bits of red, three of green, two of blue. It exists because some carts would rather read a colour off an index than walk a ramp, and because every attempt at direct-colour framebuffers eventually proposes it. It is a preset, not a colour model: the framebuffer stays indexed, so a cart using `rgb332` still gets palette swaps and still gets index 0 as a transparent key. It is not the default because two bits of blue means four blue levels.

`rico16 new <name> --palette rgb332` bakes the preset into the cart, after which it is ordinary `__palette__` data and the runtime cannot tell the difference.

## 5. Timing, the game loop, determinism

5.1. The console runs at **60 updates per second**. `dt` passed to `on_update` is exactly `1/60` seconds, as an f64 (`0.016666666666666666`).

5.2. Frame order:

```
load cart → build script engine → on_init() → [ on_update(dt) → on_draw() → present ] × N
```

`on_init` MUST be called exactly once per cart start. `on_update` MUST be called exactly once before each `on_draw`.

5.3. **There is no catch-up.** If a frame takes longer than 1/60 s of wall clock, the runtime MUST NOT run extra updates to compensate; the cart simply runs slower than real time. Rationale: one update per frame keeps carts reproducible, keeps `dt` constant, and avoids the death spiral where the engine falls further behind because it is busy catching up.

5.4. `system::frames()` returns the number of completed frames since cart start, starting at 0 during the first `on_update`. `system::time()` returns `frames * dt` as f64. Neither function may observe wall-clock time.

5.5. **Random numbers are deterministic and part of the spec.** `random()` and `random_int()` use xoshiro256\*\*, seeded via splitmix64 from a single u64. The default seed is 0. `system::seed(n)` reseeds.

- `random()` returns f64 in `[0, 1)` computed as `(x >> 11) * 2^-53` from a 64-bit draw.
- `random_int(lo, hi)` returns a uniform integer in `[lo, hi]`, both ends inclusive, with rejection sampling to avoid modulo bias. `lo > hi` is a script error.

Changing the algorithm or the default seed is a **major** version change: it breaks every recorded replay and every seeded cart.

5.6. Carts MUST be byte-identically reproducible across platforms for the same seed, inputs and frame count. Therefore:

- f64 arithmetic only; no `f32`, no fast-math, no FMA contraction differences.
- No dependence on hash iteration order. Rhai's `Map` is ordered; carts MUST NOT depend on anything else unordered.
- No wall-clock, no locale, no filesystem, no environment reads from a cart.

5.7. A runtime MAY provide an optional fast-forward (frame skip) mode for tooling. It MUST produce the same results as real-time execution.

## 6. Coordinates, clipping, drawing rules

6.1. All geometry is integer pixels. Negative coordinates and coordinates beyond the screen are legal and MUST be clipped without panicking, wrapping or saturating incorrectly. A fully off-screen draw is a no-op.

6.2. Arguments typed as integer MUST accept integer values. A float argument is truncated toward zero. `NaN` or infinite floats where an integer is expected MUST raise a script error.

6.3. A color argument is an integer 0–255. Out-of-range values MUST raise a script error, never wrap.

6.4. Drawing is always **overwrite**: the destination index is replaced. There is no blending, no XOR mode, no masking beyond the index-0 key.

6.5. `gfx::text` renders an 8×8 fixed built-in font, advancing 8 PU per glyph, with `\n` moving down 8 PU and returning x to the call's start. The font covers printable ASCII 32–126; other code points MUST render as a placeholder box, never panic. The font lives in the console, not in the cart, and there is no mechanism to replace it — carts cannot break text rendering, and text is one asset format we never have to invent.

6.6. Primitives use the standard half-open convention: `gfx::rect(x, y, w, h)` covers pixels `x .. x+w` exclusive. `w` or `h` ≤ 0 is a no-op.

6.7. Circles are rasterized symmetrically with a fixed midpoint algorithm so that identical arguments produce identical pixels on every platform.

6.8. Lines are rasterized with a fixed algorithm so that identical arguments produce identical pixels on every platform: the rasterizer steps once per pixel along the major axis (the axis of greatest run) and rounds the minor coordinate to the nearest integer, half away from zero. Both endpoints are drawn when on screen. A line is exactly one pixel thick.

## 7. Resources and limits

| Resource | Size | Encoding | Limit enforcement |
|---|---|---|---|
| Framebuffer | 76,800 B | 1 byte/px | fixed |
| Sprite sheet | 65,536 B (256×256 px, 256 tiles of 16×16) | 1 byte/px | fixed |
| Tilemap | 16,384 B (128×128 cells) | 1 byte/cell | fixed |
| Palette | 512 B | RGB555 × 256 | fixed |
| Cart code | ≤ 65,536 B | UTF-8 Rhai source | hard error |

7.1. Tiles are **16×16 PU**. A 320×240 screen shows exactly **20×15** cells. Tile id = `row * 16 + col` on a 16-column sheet; ids are 0–255.

7.2. The map is **128×128 cells** of 16×16 PU, i.e. a 2048×2048 PU world. Cell values are tile ids 0–255. Every id draws; there is no implicit "empty" tile. Carts that need empty space reserve an id for it.

7.3. `tilemap::get` reads outside 0–127 return 0. `tilemap::set` writes outside 0–127 are ignored. This is deliberate silent clipping so that procedural map code can run off the edges; carts MUST NOT rely on it to detect bugs.

7.4. The code limit is measured as the UTF-8 byte length of the `__code__` payload, excluding the single trailing newline before the next section header. `rico16 pack` and `rico16 run` MUST reject an oversized cart with an error stating actual bytes and the limit. Truncation is forbidden.

7.5. Limits are compile-time constants in the reference implementation. Changing any number in the table above is a spec change and a major version bump.

7.6. There is no token counting. The 64 KiB byte budget is the entire budget.

## 8. Drawing model

8.1. rico-16 is **immediate mode over a persistent data model**. Every frame, the cart draws explicitly; nothing is retained except framebuffer contents.

8.2. The sprite sheet and the tilemap are mutable data structures. They persist across frames, are read by blits, are edited by API, and are saved in the cart. They are **not** surfaces: they are never composited, never scrolled by hardware, and have no draw order of their own.

8.3. `tilemap::draw(...)` blits a rectangle of cells into the framebuffer at a given screen position, inline, in call order, exactly like `sprite::draw`. Layer ordering is therefore just call order. There are no layers.

8.4. There is **no global camera and no transform stack** in v1. Scrolling is arithmetic performed by the cart, or by choosing different source cells and destination offsets per call:

```rhai
// draw the map as seen by a camera at world pixel (cam_x, cam_y)
let cx = floor(cam_x / 16.0);
let cy = floor(cam_y / 16.0);
tilemap::draw(cx, cy, 22, 16, cx * 16.0 - cam_x, cy * 16.0 - cam_y);
```

Note the float division: integer division truncates toward zero, which puts negative camera positions in the wrong cell. 22 cells of horizontal overdraw cover the 20 visible cells at any sub-cell offset.

A camera transform is listed as deferred ([§15](#15-deferred-features-and-non-goals)) because hidden render state is the main source of "why did my sprite move" bugs, and a console should be predictable.

8.5. Sprite flipping is per call, not stored in the sheet. There are no per-sprite flags, no collision bitmasks, no palette-per-sprite banks.

## 9. Script API

### 9.1. Naming rules

- Modules are namespaced: `gfx::`, `sprite::`, `tilemap::`, `input::`, `system::`, `audio::`.
- `snake_case`; no abbreviations (`tilemap`, not `map`; `draw_region`, not `sspr`). `map` is explicitly avoided because Rhai has a built-in `Map` type.
- Draw operations are named after their noun; filled variants take a `_filled` suffix so related calls sort together: `gfx::rect`, `gfx::rect_filled`.
- Do not namespace what Rhai already provides. `sin`, `cos`, `sqrt`, `abs`, string and array operations remain free functions; there is no `math::` module.
- Functions may be overloaded by arity (`sprite::draw` with 3 or 5 arguments), never by type.
- The three entry point names in §9.2 are reserved: a cart must not repurpose `on_init`, `on_update` or `on_draw` as helpers. Other `on_`-prefixed names are ordinary functions and are never called by the runtime.

### 9.2. Entry points

```rhai
fn on_init()      { }   // once, before frame 0
fn on_update(dt)  { }   // once per frame; dt == 1/60 always
fn on_draw()      { }   // once per frame, after on_update
```

All three are optional. A cart with no `on_draw` presents an unmodified framebuffer. A cart with no `on_update` still runs `on_draw` at 60 Hz.

Declaring one of these names with a different parameter count is a load error, not a silently skipped handler.

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
| `gfx::text_width(str)` | returns PU width of the first line |
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

**Audio (stubbed in v1)**

| Function | Notes |
|---|---|
| `audio::play(id)` | accepted, no output, no effect |

**Free functions**

`random()`, `random_int(lo, hi)` per [§5.5](#55-random-numbers-are-deterministic-and-part-of-the-spec).

### 9.4. Errors

9.4.1. Every script error MUST be reported with the cart file name, line and column, plus the Rhai message. Reporting a bare `Runtime error` is a spec violation.

9.4.2. An error raised inside a callback aborts the rest of that callback. The runtime then follows [§12.6](#12-runtime-and-player).

9.4.3. Unknown module, function or `Button` value is a script error, not a no-op.

### 9.5. Call budget

Measured host-call overhead is roughly 0.5–2 µs per Rhai function call, so a cart should stay under a few thousand draw calls per frame at 60 Hz. This is guidance, not a limit; the runtime SHOULD warn once per second on stdout when frames consistently overrun. Bulk operations (`tilemap::draw`, `draw_region`) exist to keep carts inside the budget.

## 10. Cart text format (`.r16`)

### 10.1. Encoding and structure

10.1.1. `.r16` is UTF-8 text. A leading BOM MUST be accepted and stripped. LF and CRLF are both accepted on read; LF is always written.

10.1.2. A cart is a sequence of sections. A section begins with a line matching exactly `__[a-z0-9_]+__` at column 0 and ends before the next such line or at end of file.

10.1.3. Sections MAY appear in any order. Duplicate sections MUST be rejected. Unknown sections MUST be preserved verbatim so that newer carts survive an older tool.

10.1.4. A cart therefore MUST NOT contain a line consisting solely of `__name__` inside code. There is no escape mechanism; document the restriction and let the runtime reject the cart instead of silently splitting it.

10.1.5. Every section is optional on read. A missing `__gfx__` or `__map__` is zero-filled; a missing `__palette__` means the console default palette. A file with no sections and no content is a default cart; a file with content but no sections is not a cart and MUST be rejected.

### 10.2. `__meta__`

`key=value`, one per line, UTF-8, values MAY contain spaces. Keys: `title`, `author`, `version`, `description`. Unknown keys are preserved. Missing keys default to empty strings. Values MUST NOT contain newlines. On write, known keys come first in the order listed, then unknown keys in their original order.

### 10.3. `__code__`

The Rhai source, verbatim, including blank lines and comments. Leading and trailing whitespace of the section is preserved except for one trailing newline before the next header.

### 10.4. `__gfx__`

Exactly **256 rows** of **512 hex digits** (2 digits per pixel, row-major, top to bottom). A shorter or longer sheet is an error. Hex digits are case-insensitive on read and uppercase on write. Example row starts `00010203…`.

### 10.5. `__map__`

Exactly **128 rows** of **256 hex digits** (2 digits per cell, 128 cells per row). Hex digits are case-insensitive on read and uppercase on write. A cell naming a tile id that has no pixels simply draws that region of the sheet.

### 10.6. `__palette__`

Optional. Exactly **16 rows** of **16 four-digit hex words** (256 RGB555 values in total), separated by single spaces and uppercase on write. Bit 15 MUST be 0. If the section is absent, the cart uses the console default palette.

### 10.7. Canonical form

A cart is in canonical form when all of the following hold:

- LF line endings, no BOM, one trailing newline at end of file, no trailing whitespace on any line.
- `__meta__` is present if and only if the cart has at least one metadata key. Known keys appear in the order `title`, `author`, `version`, `description`; unknown keys follow, in their original order.
- `__code__` is always present, even when empty.
- `__gfx__` is present if and only if the sprite sheet contains a non-zero pixel.
- `__map__` is present if and only if the tilemap contains a non-zero cell.
- `__palette__` is present if and only if the cart defines its own palette.
- Sections are ordered `__meta__`, `__code__`, `__gfx__`, `__map__`, `__palette__`, followed by any sections this version does not understand, in their original order and byte for byte.
- Hex is uppercase; `__palette__` uses single spaces.

Omitting empty art is what keeps a fresh cart a couple of lines instead of a hundred kilobytes of zeroes; it also means canonical form is computed from content, not from how the file was originally written.

`parse → serialize` MUST be byte-identical for a canonical cart, and serializing MUST be idempotent for any input that parses. Both are required tests ([§14.5](#14-versioning-and-compatibility)).

### 10.8. Size

A maximal cart is roughly 230 KiB of text (code ≤ 64 KiB, sprite sheet ≈ 131 KiB of hex, map ≈ 32 KiB). The text form is a development format; the PNG form is the distribution format.

## 11. Cart binary format and PNG packing

### 11.1. Binary cart

All integers little-endian.

```
offset  size   field
0       4      magic "R16C"
4       2      format version (u16) = 1
6       2      flags (u16), bit 0 = palette present
8       4      code length in bytes (u32)
12      65536  sprite sheet (256×256, 1 byte/px)
65548   16384  tilemap (128×128, 1 byte/cell)
81932   512    palette (256 × RGB555)
82444   N      code, UTF-8 Rhai source
```

Maximum size: `82444 + 65536 = 147,980` bytes. `N` MUST be ≤ 65,536. In format version 1, `flags` bit 0 (palette present) MUST be 1, because packing always resolves a palette into the cart.

### 11.2. The image is the label

The PNG is **320×240, 8-bit RGBA, non-interlaced** — a completely ordinary image with no private chunks and no reserved banner. The *entire* image is cover art, and the cart data is hidden underneath it in the low four bits of every channel. This is the mechanism PICO-8 uses, and it works because pixel art carries fewer than 8 significant bits per channel to begin with.

11.2.1. **Visible art.** The high nibble of the R, G and B bytes is the cover art, 4 bits per channel (RGB444, 4096 colors). The high nibble of the A byte MUST be `0xF`, so the image is fully opaque. A viewer that ignores the low nibbles — that is, every viewer — sees exactly the art that was authored: no noise, no dithering artefacts, no visible payload. The cost is colour resolution, not looks: cover art is 12-bit, which for a fantasy console is generous.

11.2.2. **Payload.** Reading pixels in row-major order from (0,0), the low nibbles of R, G, B and A are concatenated into a nibble stream, and consecutive pairs of nibbles form payload bytes, first nibble as the high half. That yields **16 bits per pixel** and a total capacity of **153,600 bytes**.

11.2.3. The payload begins with the blob header below, then the binary cart ([§11.1](#111-binary-cart)). Nibbles after the end of the payload MUST be zero. Because the authored art has no low-nibble information by definition, filling them cannot damage it.

```
offset  field
0       magic "R16P"
4       blob version (u16) = 1
6       reserved (u16) = 0
8       payload length (u32) — length of the binary cart that follows
12      CRC-32 (IEEE) of that payload
```

Capacity after the 16-byte header is 153,584 bytes, comfortably above the 147,980-byte maximum cart.

11.2.4. **Cover art.** `rico16 pack` takes `--label <image>` or generates a label when none is given. The generated label is deterministic — a dark field, the cart title rendered in the built-in 8×8 font, the author line, and a palette strip of the cart's own colours — so that packing the same cart twice produces the same PNG.

11.2.5. An imported label of any size is resampled to 320×240 (cover fit, centred, cropped) and quantized to RGB444. Resampling is explicitly allowed here: the nearest-neighbour rule in [§3.7](#3-display-and-framebuffer) governs the game display, not the cover. A 320×240 source is pixel-perfect by definition.

11.2.6. **Labels live only in the PNG.** The `.r16` text cart has no label section, mirroring the fact that the label is a property of the published artefact, not of the source. Re-labelling a cart therefore means repacking, and never modifies the cart's contents.

### 11.3. Advisory metadata chunk

`rico16 pack` SHOULD also emit a `tEXt` chunk with keyword `rico16` and value `v=1;len=<payload length>;crc=<8 uppercase hex>`. Tools MAY use it to identify carts without decoding pixels. The pixel payload is authoritative; if the chunk and the pixels disagree, the runtime MUST trust the pixels and SHOULD warn.

### 11.4. Reading a cart PNG

A loader MUST: verify the magic, verify `12 ≤ payload length ≤ 147,980`, verify that the payload fits the capacity, verify the CRC-32, then parse. Any mismatch MUST be a clear error naming the field that failed. A loader MUST NOT attempt to guess or repair a corrupt cart.

### 11.5. Durability rules

11.5.1. Lossless PNG re-encoding (optimizers, `optipng`, CDNs, most social platforms) preserves carts, because the payload lives in pixel values rather than chunks or trailing bytes. Lossy transcoding — JPEG, lossy WebP, palette reduction, or any resize — destroys them. Document this wherever carts are shared: a cart is an image, and images get re-encoded.

11.5.2. Because the payload is spread across the whole image, a cart PNG MUST NOT be resized, cropped, or colour-reduced by any tool that "prepares images for the web". This is the price of having real cover art, and it is the same price PICO-8 pays.

### 11.6. Determinism

`rico16 pack` SHOULD be deterministic: the same cart text, the same label input and the same encoder settings produce a byte-identical PNG. The generated default label MUST be deterministic. Reproducible builds are a stated goal.

## 12. Runtime and player

12.1. The player opens a single window, scaled per [§3.7](#3-display-and-framebuffer), presenting once per update with vsync. On displays whose refresh rate is not a multiple of 60, judder is expected and accepted.

12.2. Default key map, matched on **physical scan codes** so that AZERTY and QWERTZ layouts keep the same physical arrangement:

| Button | Physical key |
|---|---|
| Up / Down / Left / Right | arrow keys |
| `Button::Z` | `KeyZ` position |
| `Button::X` | `KeyX` position |
| `Button::C` | `KeyC` position |
| `Button::V` | `KeyV` position |

Carts refer to logical buttons, never to keys, so a remap changes nothing in cart code.

12.3. Reserved keys, never visible to a cart: `Esc` (quit), `F11` (toggle fullscreen), `Ctrl+R` (restart cart). A cart MUST NOT be able to bind them.

12.4. Input is sampled once per frame, immediately before `on_update`. Edge state (`pressed`, `released`) compares the current frame's bitmask with the previous frame's.

12.5. On window focus loss the runtime MUST clear all held buttons. Key-repeat events MUST NOT be treated as new presses.

12.6. Error policy:

- **Player mode:** print `<file>:<line>:<col>: <message>` to stderr, close the window, exit with code 1. The process MUST NOT panic.
- **Editor mode:** halt the cart, keep the editor alive, show the message in the console pane. See [§13](#13-editor).

12.7. `system::quit()` ends the cart after the current frame and exits with code 0. `system::quit(message)` prints `message` to stdout first (in the editor, to the console pane) and exits the same way. A message is not an error and MUST NOT go to stderr.

12.8. There is no audio output in v1; `audio::play` is accepted and ignored. The audio device MUST NOT be opened.

## 13. Editor

13.1. The editor is a ratatui terminal application (crossterm backend). It is a separate mode of the same binary: `rico16 edit <cart>`.

13.2. Required panes in the first milestone: cart/ file list, code editor with Rhai syntax highlighting, console (script errors, warnings, `print`-style output), palette inspector.

13.3. Required actions: run, stop, restart, hot reload on save. Hot reload restarts the cart and re-runs `on_init`; it MUST NOT attempt to preserve script globals.

13.4. The game runs in its own native window, not inside the terminal. The terminal multiplexes nothing; it only edits and shows text.

13.5. Saving writes canonical form ([§10.7](#107-canonical-form)).

13.6. The editor MUST NOT lose edits on a failed parse: a cart that does not parse is a normal state (the user is mid-edit), and the console pane reports what is wrong.

13.7. Sprite, map and audio editors are explicitly out of the first milestone ([§15](#15-deferred-features-and-non-goals)).

## 14. Versioning and compatibility

14.1. This document's version is `MAJOR.MINOR`. The cart format carries its own version number in the binary header and the PNG blob header.

14.2. **Major** changes (break carts): framebuffer size, palette size, tile size, map size, code limit, RNG algorithm or default seed, drawing semantics, removing or renaming API functions, PNG layout.

14.3. **Minor** changes (safe): adding API functions, adding optional cart sections, adding optional metadata keys, clarifying wording without changing behaviour.

14.4. A runtime MUST load carts of an equal or older major version. It MUST reject a newer major version with a clear message rather than misinterpreting it.

14.5. Any change to format, limits, RNG or API MUST land with: an updated table in this document, a changelog entry below, and a test fixture. Format changes additionally require a parse→serialize round-trip test.

## 15. Deferred features and non-goals

Deferred means "not in v1, expected later, design leaves room": SFX synthesis and a pattern/tracker UI; TUI sprite and map editors; sprite/map/audio asset import; WASM web player; gamepads; a second player; bulk palette operations; a camera/transform stack; arbitrary sprite scaling and rotation; token counting; save data persistence.

Non-goals means "will not happen": true color, alpha blending, hardware-layer (VRAM/OAM) emulation, PICO-8 cart compatibility, short PICO-8-style function names, cart-supplied fonts, networking, and 3D.

## 16. Open questions

Unresolved, and MUST be settled before the milestone that needs it:

16.1. Whether a bulk palette operation is ever needed. `gfx::palette` × 256 costs roughly 0.2–0.3 ms per frame, which fits a 16.6 ms budget, so v1 ships without one. Revisit only with a measurement from a real cart that fades every frame, and then as a minor version addition.

Settled during the design phase, recorded here so they are not reopened:

- The default palette table is generated, committed as data and pinned by a test ([§4.7](#4-palette-and-color)). Re-curating it is art direction, not an open design question, and it cannot break a packed cart.
- RGB332 is a named palette preset, not a framebuffer colour model ([§4.11](#4-palette-and-color)). An indexed frame costs the same memory and keeps palette effects.
- Label art is the whole PNG at RGB444, with the payload in the low nibbles ([§11.2](#112-the-image-is-the-label)).
- No `tilemap::draw_world`. Two ways to address the same map is a bug generator; the conversion is four documented lines ([§8.4](#8-drawing-model)) that a cart can keep in a helper, and adding it later is a non-breaking minor change.
- No frame skipping. It does not help a slow `on_update`, and it breaks the one-frame-one-update guarantee ([§5.3](#5-timing-the-game-loop-determinism)).
- The font is the built-in ASCII 8×8, permanently ([§6.5](#6-coordinates-clipping-drawing-rules)).
- `system::quit(message)` prints the message and exits 0 ([§9.3](#93-surface)).

## 17. Decision log

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
| Input players | 1 exposed, 2 tracked | Adding a second player later is not a breaking change |
| Cart text format | Sectioned text, 2 hex digits/px | Diffable and reviewable; PNG is the distribution format |
| Cart label | The PNG itself, RGB444 + payload nibbles | Real cover art with zero wasted pixels; same idea as PICO-8 |
| Label source | `--label <image>` or a generated default | Always a presentable cart, never a required step |
| PNG packing | Valid PNG, payload in pixels + `tEXt` hint | Carts preview as images everywhere; no appended-blob hack |
| Timestep | Fixed, no catch-up | Determinism and no death spiral |
| RNG | Seeded xoshiro256\*\* in the spec | Replays and tests need stable randomness |
| Font | Built-in ASCII 8×8, permanent | Carts cannot break text; one less asset format |
| Line rasterisation | Major-axis stepping, minor rounded half-away-from-zero | Deterministic pixels on every platform, one-pixel-thick lines (§6.8) |
| `system::quit` | Optional message, exit 0 | Tool-friendly, and good for "you win" screens in the editor |
| Editor | TUI, runtime first | Games must be playable before art tools matter |
| Audio | Stubbed in v1 | Largest hidden cost; do it after the console works |

## 18. Changelog

- **0.1 (rev 4)** — rasterized geometry is fully deterministic: lines specified ([§6.8](#6-coordinates-clipping-drawing-rules)) to step once per major-axis pixel with the minor coordinate rounded half-away-from-zero, complementing the existing fixed midpoint circles (§6.7); all primitives clip to the screen before rasterising so a wild coordinate cannot become a long-running loop; radials are clamped to a constant large enough for any screen and small enough to terminate. The built-in font is `font8x8_basic` (Daniel Hepper, public domain, based on the IBM VGA 8×8 font); glyphs cover printable ASCII 32–126 and everything else renders as a placeholder box (§6.5). Framebuffer, input edges and the fixed 60 Hz player loop are implemented per §3, §5, §12: input is sampled once per frame immediately before the update, focus loss clears held state without synthesizing releases (§12.5), and the player schedules one update per frame with no catch-up (§5.3).
- **0.1 (rev 3)** — default palette generated and committed as data with its structure pinned by tests; `rgb332` added as a named palette preset and removed from open questions.
- **0.1 (rev 2)** — label art re-designed as full-image RGB444 with the payload in the low nibbles, replacing the 120-row banner; palette structure fixed at 15 base colours plus 15 ramps; `system::quit(message)`; `draw_world`, frame skipping, bulk palette operations and cart-supplied fonts settled (rejected or deferred); `draw_world` removed from open questions.
- **0.1** — initial specification. Display, palette, timing, drawing model, script API, cart text format, binary format, PNG packing, runtime and editor requirements.
