# rico-16

A 16-bit fantasy console: **320×240**, **256 indexed colors**, **Rhai** scripting, and carts that pack into a **PNG**.

rico-16 is a self-contained console in the spirit of PICO-8, but built for the 16-bit era: a bigger screen, a wider palette, and a scripting language that doesn't make you learn Lua. Everything a cart needs — code, art, map, palette — lives in one PNG file.

> **Status: spec complete (v1 draft); the console is implemented.** Project-based development with `cart.toml`, `src/`, `assets/`. PNG (640×480 or 1280×960) with a 12-bit RGB payload. Real Rhai module imports with embedded sources. A single reserved global `state` variable for cart data. Full conformance vectors in the spec.

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

**There is no editor in v1, on purpose.** A cart is plain text and PNGs, so any text editor and any pixel-art tool works, and `rico16 build` and `rico16 run` are the whole workflow.

An editor is a product in its own right — file tree, code editor, console, palette inspector, run/stop, hot reload, plus the awkward problem of showing a 320×240 indexed framebuffer in a terminal that cannot render it faithfully. Building one well would dominate v1's effort while changing nothing about what a cart can do. It is deferred to v2 ([SPEC §16](docs/SPEC.md)); the earlier v1 sketch is withdrawn rather than half-built.

## Repository layout

```
rico16-core     framebuffer, palette, blitting, tilemap, input, RNG
                (pure: no filesystem, no clock, no threads, wasm-clean)
rico16-script   Rhai engine, script API bindings, module resolver
rico16-cart     cart.toml parser, binary encoding, PNG pack/unpack
rico16-player   window, input mapping, main loop, frame pacing
rico16-cli      the `rico16` binary: new / build / run / unpack
docs/           SPEC.md and friends
```

## Commands

```sh
rico16 new my-game                 # scaffold a project
rico16 build                       # build the project in the current directory
rico16 build my-game               # project → packed PNG
rico16 run                         # build and run the current directory
rico16 run my-game                 # build in memory, run
rico16 run my-game.png             # run packed PNG directly
rico16 unpack my-game.png out/     # inspect a shared cart
```

`build` and `run` default to the current directory, so from inside a project the
bare commands are enough:

In the player window: `Esc` = system menu, `F11` = fullscreen, `Ctrl+R` = restart. Arrows and `Z X C V` map to the eight buttons by physical key position. `system::quit(message)` prints the message and exits 0; a script error prints `<file>:<line>:<col>: <message>` and exits 1.

## Examples

Four carts live in `examples/`. A test builds and runs every one of them at HEAD.

| Cart | What it shows |
|---|---|
| `hello` | the smallest interesting cart: `state`, the entry points, a rect and some text |
| `snake` | modules and the scene pattern, input, `rnd`, and data that lives in `state` |
| `raycaster` | a Doom-style renderer: a 128×128 level from `assets/map.png`, a DDA per screen column, walls shaded by distance along the palette ramps, sky and floor gradients, and a minimap |
| `fighter` | a one-on-one fighter: a 40×60 sprite drawn with `sprite::draw_region` (the same art, mirrored with `flip_x` whenever a fighter faces left), twelve shaded poses, attack wind-up and recovery frames with hitboxes, blocking, ground shadows, health bars and a CPU opponent |

```sh
cargo run -p rico16-cli -- run examples/raycaster
cargo run -p rico16-cli -- run examples/fighter
```

`raycaster` is worth reading if you want to see the drawing model stretched: the whole scene is 320 `gfx::rect_filled` calls, one per screen column, and the level is a plain grayscale PNG that `cart.toml` places with `[map]`.

`fighter` is worth reading for the script side. It shows the one mutation pattern that matters in Rhai: arguments are passed by value, so `fn control(f) { f.x += 1 }` silently does nothing. The fighter functions are called *on* the fighter — `state.a.control(state.b)` — so `this` is the map and mutations stick. It also shows why a fighter carries its own copy of the arena bounds: a method called on `state.a` cannot also read `state`, because the runtime sees a re-entrant borrow of the same shared value.

Both carts' assets are plain PNGs, editable with any pixel-art tool. Two layouts are worth knowing:

- `raycaster/assets/map.png` is 128×128 grayscale, and SPEC §10.2.3 reads the sample value as the tile id — so the tile ids *are* the pixels. 0 is open floor and 32/96/160/224 are the four wall types, which is why the file looks like a grey maze rather than a picture.
- `fighter/assets/fighters.png` is a 256×256 sheet of 40×60 cells: pose `p` of fighter `f` sits at `x = (p % 6) * 40`, `y = (f * 2 + p / 6) * 60`. The twelve poses, in order, are idle, breathe, walk0, walk1, crouch, punch wind-up, punch, punch recover, kick wind-up, kick, kick recover, hit. Both fighters are drawn facing right; the cart mirrors one with `flip_x`.

One thing worth knowing before drawing art for this console: SPEC §7 fixes the sprite sheet at 65,536 bytes for **all** art, and that is the real budget. The fighter uses 24 cells of 40×60 = 88% of it for twelve poses each. Larger characters mean fewer frames — 64×96 leaves room for five poses each, which is not a fighting game. Shading is free by comparison: it is the same pixels in better colours, which is why the fighters get five tones per material rather than two.

## Roadmap

Milestones track the spec. Each landed with its conformance tests green.

| | |
|---|---|
| M0 | Workspace skeleton and `rico16-core`: framebuffer, palette, font, blitting, tilemap, input, RNG — **done** |
| M1 | `rico16-cart`: `cart.toml`, binary cart, PNG pack/unpack, deterministic payload — **done** |
| M2 | `rico16-script`: Rhai engine, global `state`, script API, module system, error reporting — **done** |
| M3 | `rico16-player`: window, present path, input mapping, 60 Hz loop, system menu — **done** |
| M4 | `rico16-cli`: `new` / `build` / `run` / `unpack` — **done** |
| v2 | Editor, SFX synth and tracker UI, TUI sprite/map editors, WASM web player, gamepads, second player |

## Building

Requires a stable Rust toolchain (edition 2024, MSRV 1.85). Windows, Linux and macOS are first-class.

```sh
cargo build --workspace
cargo test --workspace
```

Working today:

```sh
cargo run -p rico16-cli -- new hello               # scaffold a project
cargo run -p rico16-cli -- new hexed --palette rgb332
cargo run -p rico16-cli -- build hello             # project → hello/build/hello.png
cargo run -p rico16-cli -- run hello               # build in memory, run
cargo run -p rico16-cli -- run hello/build/hello.png
cargo run -p rico16-cli -- unpack hello/build/hello.png out/
cargo run -p rico16-cli -- run examples/snake      # a playable example
```

Or from inside a project, with no arguments:

```sh
cd examples/snake
cargo run -p rico16-cli -- build                   # writes build/snake.png
cargo run -p rico16-cli -- run                     # builds and runs
```

The example carts in `examples/` build and run at HEAD; a test enforces it.

## Documentation

- [docs/SPEC.md](docs/SPEC.md) — the console specification (normative)
- [AGENTS.md](AGENTS.md) — how to work in this repository, for humans and AI agents

## Non-goals

True color, alpha blending, hardware-style layers/OAM/VRAM emulation, PICO-8 cart compatibility, and anything that needs a GPU shader to render a sprite. The console is a framebuffer, a palette and a scripting language.

## License

Not chosen yet. Do not assume a license until one is committed at the repository root.
