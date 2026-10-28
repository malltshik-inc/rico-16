# rico-16

A 16-bit fantasy console: **320×240**, **256 indexed colors**, **Rhai** scripting, a **TUI editor**, and carts that pack into a **PNG**.

rico-16 is a self-contained console in the spirit of PICO-8, but built for the 16-bit era: a bigger screen, a wider palette, and a scripting language that doesn't make you learn Lua. Everything a cart needs — code, art, map, palette — lives in one file.

> **Status: M3 complete — carts run end to end, with sprites and tilemap.** `rico16 run cart.r16` loads a cart, compiles it, and runs it at 60 Hz in a 320×240 window. Script errors report file, line and column; `system::quit(message)` prints the message and exits 0. PNG packing (M4), the TUI editor (M5) and the sample-cart set (M6) are still ahead (see [Roadmap](#roadmap)).

## The console

| | |
|---|---|
| Display | 320×240, 8-bit indexed framebuffer, index 0 is the blit transparency key |
| Palette | 256 × RGB555 (32,768 colors), cart-defined with a curated default |
| Frame rate | fixed 60 Hz, no catch-up, seeded deterministic RNG |
| Tiles | 16×16 px, exactly 20×15 cells on screen |
| Sprite sheet | 256×256 px = 256 tiles |
| Map | 128×128 cells (2048×2048 px world) |
| Code | up to 64 KiB of Rhai per cart |
| Input | 4 directions + `Z` `X` `C` `V`, one player |
| Carts | `.r16` (text, diffable) and `.r16.png` (a valid PNG that is also its own cover art) |
| Editor | terminal UI: code, console, hot reload, palette |

Exact numbers, edge cases and normative requirements live in [docs/SPEC.md](docs/SPEC.md). That document wins over everything else in this repository, including this README.

## A cart

```rhai
fn on_init() {
    system::seed(1234);
}

fn on_update(dt) {
    if input::held(Button::Left)  { player_x -= 90.0 * dt; }
    if input::held(Button::Right) { player_x += 90.0 * dt; }
}

fn on_draw() {
    gfx::clear(0);
    tilemap::draw(0, 0, 20, 15, 0, 0);
    sprite::draw(0, player_x, player_y);
    gfx::text("SCORE 0", 8, 8, 7);
}
```

The API is namespaced and spelled out — `gfx::rect_filled`, not `rectfill`. There is deliberately no PICO-8 compatibility layer and no terse-alias set; rico-16 is its own console.

## Carts

A cart is a text file you can diff, merge and review:

```text
__meta__
title=hello
author=nobody

__code__
fn on_init() { }
fn on_update(dt) { }
fn on_draw() { gfx::clear(0); }

__gfx__
00010203...   (256 rows of 2 hex digits per pixel)

__map__
00ff00aa...   (128 rows of 2 hex digits per cell)

__palette__
0000 0842 ...  (16 rows of 16 RGB555 words)
```

`rico16 pack` embeds the compiled cart in the **pixel values** of a 320×240 PNG — the low nibble of every channel, under image data that uses only the high nibble. The result is a completely ordinary PNG whose *whole area* is cover art, so a published cart looks like a real image in file managers, chat clients and web pages. No private chunks, no bytes glued after `IEND`, and `--label cover.png` swaps the art without touching the cart.

The trade is stated plainly: cover art is 12-bit colour, and a cart PNG must never be resized or lossily re-encoded. A cart is an image, and images get re-encoded.

## Editor

`rico16 edit cart.r16` opens a [ratatui](https://github.com/ratatui/ratatui) terminal UI with a cart browser, a code editor with Rhai highlighting, a console pane for errors and prints, a palette inspector, and run/stop with hot reload. The game itself opens in its own window — a terminal cannot do justice to a 320×240 indexed framebuffer.

Pixel-art tools are intentionally *not* in the first milestone. Carts are text, so any editor works until the built-in ones land.

## Repository layout

```
rico16-core     framebuffer, palette, blitting, tile layer, input, RNG
                (pure: no filesystem, no clock, no threads, wasm-clean)
rico16-script   Rhai engine, script API bindings, hot reload
rico16-cart     .r16 text parser/serializer, PNG pack/unpack
rico16-player   window, input mapping, main loop
rico16-tui      ratatui editor
rico16-cli      the `rico16` binary: new / fmt / run (pack, unpack, edit planned)
docs/           SPEC.md and friends
```

## Roadmap

| | |
|---|---|
| M0 | Workspace skeleton, `rico16 new`, cart parser with round-trip tests — **done** |
| M1 | Core framebuffer + drawing ops, player window, input, 60 Hz loop — **done** |
| M2 | Rhai embedded, script API, cart loading, error reporting with line numbers — **done** |
| M3 | Sprites and tilemap: `sprite`, `tilemap`, scrolling via explicit coordinates — **done** |
| M4 | Cart format finalized, PNG pack/unpack, thumbnails, CLI `pack`/`unpack` |
| M5 | TUI editor: edit, run/stop, console, hot reload, palette |
| M6 | Limit enforcement, docs, sample carts |
| later | SFX synth and tracker UI, TUI sprite/map editors, WASM web player, gamepads, second player |

## Building

Requires a stable Rust toolchain (edition 2024, MSRV 1.85). Windows, Linux and macOS are first-class.

```sh
cargo build --workspace
cargo test --workspace
```

Working today:

```sh
cargo run -p rico16-cli -- new hello              # writes hello.r16
cargo run -p rico16-cli -- new hexed --palette rgb332
cargo run -p rico16-cli -- run examples/hello.r16 # play a cart in a 3× window
cargo run -p rico16-cli -- fmt hello.r16          # rewrite in canonical form
cargo run -p rico16-player -- examples/hello.r16  # same, without the CLI
```

In the window: `Esc` closes, `F11` toggles fullscreen, `Ctrl+R` restarts the cart and re-runs `on_init`. Arrows and `Z X C V` map to the eight buttons by physical key position. `system::quit(message)` prints the message and exits 0; a script error prints `<file>:<line>:<col>: <message>` and exits 1. `pack`, `unpack` and `edit` arrive with M4–M5; `examples/hello.r16` is the fixture they will be tested against.

## Documentation

- [docs/SPEC.md](docs/SPEC.md) — the console specification (normative)
- [AGENTS.md](AGENTS.md) — how to work in this repository, for humans and AI agents

## Non-goals

True color, alpha blending, hardware-style layers/OAM/VRAM emulation, PICO-8 cart compatibility, and anything that needs a GPU shader to render a sprite. The console is a framebuffer, a palette and a scripting language.

## License

Not chosen yet. Do not assume a license until one is committed at the repository root.
