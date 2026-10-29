# rico-16

A 16-bit fantasy console: **320×240**, **256 indexed colors**, **Rhai** scripting, a **TUI editor**, and carts that pack into a **PNG**.

rico-16 is a self-contained console in the spirit of PICO-8, but built for the 16-bit era: a bigger screen, a wider palette, and a scripting language that doesn't make you learn Lua. Everything a cart needs — code, art, map, palette — lives in one PNG file.

> **Status: spec complete (v1 draft), implementation in progress.** Project-based development with `cart.toml`, `src/`, `assets/`. PNG (640×480 or 1280×960) with a 12-bit RGB payload. Real Rhai module imports with embedded sources. A single reserved global `state` variable for cart data. Full conformance vectors in the spec.

## The console

| | |
|---|---|
| Display | 320×240, 8-bit indexed framebuffer, index 0 is the blit transparency key |
| Palette | 256 × RGB555 (32,768 colors), cart-defined with a curated default |
| Frame rate | fixed 60 Hz, no catch-up, seeded deterministic RNG (xoshiro256**) |
| Tiles | 16×16 px, exactly 20×15 cells on screen |
| Sprite sheet | 256×256 px = 256 tiles |
| Map | 128×128 cells (2048×2048 px world) |
| Code | no fixed limit; PNG capacity only (~378 KB default, ~1.76 MB overflow) |
| Input | 4 directions + `Z` `X` `C` `V`, one player, `Esc` = system menu |
| Carts | project directory (`cart.toml` + `src/` + `assets/`) → packed PNG |
| Editor | terminal UI: project tree, code editor, console, hot reload, palette |

Exact numbers, edge cases and normative requirements live in [docs/SPEC.md](docs/SPEC.md). That document wins over everything else in this repository, including this README.

## A cart

Only `main.rhai` has the runtime entry points. Modules are libraries with no state of their own; the cart's data lives in the single global `state`, which main and modules both use.

```rhai
// src/main.rhai
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

```rhai
// src/scenes/play.rhai
fn update(dt) {
    if input::held(Button::Left) { state.px -= 2; }
}

fn draw() {
    gfx::clear(1);
    gfx::rect_filled(state.px - 4, state.py - 4, 8, 8, 9);
}
```

The API is namespaced and spelled out — `gfx::rect_filled`, not `rectfill`. There is deliberately no PICO-8 compatibility layer and no terse-alias set; rico-16 is its own console.

## Project structure

A cart is a directory you can diff, merge and review:

```
my-game/
  cart.toml
  src/
    main.rhai
    scenes/
      play.rhai
    lib/
      grid.rhai
  assets/
    hero.png
    tiles.png
    map.png
    palette.png
  build/           # output (gitignored)
```

`cart.toml` example:

```toml
format = 1

[cart]
title = "Snake"
author = "me"
version = "0.1.0"

[[sprite]]
file = "assets/hero.png"
x = 0
y = 0

[map]
file = "assets/map.png"

[palette]
file = "assets/palette.png"
```

Build produces a single PNG with the whole cart inside — code, modules, sprite sheet, tilemap, palette, and metadata — hidden in the low nibbles of RGB. The image is a valid 640×480 (or 1280×960) PNG whose whole area is cover art. The alpha channel is always 0xFF.

The trade is stated plainly: cover art is 12-bit colour, and a cart PNG must never be resized or lossily re-encoded. A cart is an image, and images get re-encoded.

## Editor

`rico16 edit <project>` opens a [ratatui](https://github.com/ratatui/ratatui) terminal UI with a project tree, a code editor with Rhai highlighting, a console pane for errors and prints, a palette inspector, and run/stop with hot reload. The game itself opens in its own window — a terminal cannot do justice to a 320×240 indexed framebuffer.

Pixel-art tools are intentionally *not* in the first milestone. Carts are text, so any editor works until the built-in ones land.

## Repository layout

```
rico16-core     framebuffer, palette, blitting, tilemap, input, RNG
                (pure: no filesystem, no clock, no threads, wasm-clean)
rico16-script   Rhai engine, script API bindings, module resolver, hot reload
rico16-cart     cart.toml parser, binary encoding, PNG pack/unpack
rico16-player   window, input mapping, main loop, frame pacing
rico16-tui      ratatui editor
rico16-cli      the `rico16` binary: new / build / run / unpack / edit
docs/           SPEC.md and friends
```

## Commands

```sh
rico16 new my-game                 # scaffold a project
rico16 build my-game               # project → packed PNG
rico16 run my-game                 # build in memory, run
rico16 run my-game.png             # run packed PNG directly
rico16 unpack my-game.png out/     # inspect a shared cart
rico16 edit my-game                # TUI editor
```

In the player window: `Esc` = system menu, `F11` = fullscreen, `Ctrl+R` = restart. Arrows and `Z X C V` map to the eight buttons by physical key position. `system::quit(message)` prints the message and exits 0; a script error prints `<file>:<line>:<col>: <message>` and exits 1.

## Roadmap

Milestones track the spec, not the old `.r16` prototype on `main`. Each lands with its conformance tests green.

| | |
|---|---|
| M0 | Workspace skeleton and `rico16-core`: framebuffer, palette, font, blitting, tilemap, input, RNG |
| M1 | `rico16-cart`: `cart.toml`, binary cart, PNG pack/unpack, deterministic payload |
| M2 | `rico16-script`: Rhai engine, global `state`, script API, module system, error reporting |
| M3 | `rico16-player`: window, present path, input mapping, 60 Hz loop, system menu |
| M4 | `rico16-cli`: `new` / `build` / `run` / `unpack` |
| M5 | `rico16-tui`: editor — file tree, code editor, console, palette, hot reload |
| later | SFX synth and tracker UI, TUI sprite/map editors, WASM web player, gamepads, second player |

## Building

Requires a stable Rust toolchain (edition 2024, MSRV 1.85). Windows, Linux and macOS are first-class.

```sh
cargo build --workspace
cargo test --workspace
```

## Documentation

- [docs/SPEC.md](docs/SPEC.md) — the console specification (normative)
- [AGENTS.md](AGENTS.md) — how to work in this repository, for humans and AI agents

## Non-goals

True color, alpha blending, hardware-style layers/OAM/VRAM emulation, PICO-8 cart compatibility, and anything that needs a GPU shader to render a sprite. The console is a framebuffer, a palette and a scripting language.

## License

Not chosen yet. Do not assume a license until one is committed at the repository root.
