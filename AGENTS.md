# AGENTS.md

Guidance for anyone — human or AI agent — changing this repository. Read this before writing code.

## Read order

1. [README.md](README.md) — what the project is.
2. [docs/SPEC.md](docs/SPEC.md) — **normative**. Behaviour, numbers and edge cases.
3. This file — how to work here.

`docs/SPEC.md` outranks this file, the README and any comment in the code. If you find a conflict, stop and ask; do not quietly pick a side and do not "fix" the spec to match the code.

## Repository map

| Path | Owns | Must not |
|---|---|---|
| `rico16-core` | Framebuffer, palette, blitting, tilemap data, input state, RNG, constants from SPEC §7 | Touch the filesystem, clock, threads, environment, OS RNG, or any windowing crate |
| `rico16-script` | Rhai engine, module registration, hot reload | Contain rendering logic or resource constants |
| `rico16-cart` | `.r16` parse/serialize, binary encoding, PNG pack/unpack | Know about Rhai execution or windows |
| `rico16-player` | Window, present path, key mapping, main loop | Define behaviour the spec does not describe |
| `rico16-tui` | ratatui editor | Duplicate cart parsing |
| `rico16-cli` | `rico16` binary and subcommand wiring | Hold logic that belongs in a library crate |
| `docs/` | SPEC.md and supporting documents | Contradict SPEC.md |

Dependency direction is strictly downward: `cli` → {`tui`, `player`, `cart`, `script`, `core`}; `tui`/`player` → {`cart`, `script`, `core`}; `script` → `core`; `cart` → `core`. Cycles are a defect.

## Hard guardrails

These are settled design decisions (see SPEC §17). Do not revisit them in a PR; propose a spec change instead.

- **No true color.** Indexed 8-bit only. No RGBA drawing API, no alpha blending.
- **No transparency except index 0 as a blit key** (SPEC §4.2). Primitives write their color literally.
- **No terse PICO-8 names** (`cls`, `spr`, `rectfill`, `sspr`). Namespaced, spelled-out functions only (SPEC §9.1).
- **No PICO-8 compatibility layer**, no `__lua__`, no porting shims. Do not add them "for convenience".
- **No second surface.** No layers, no compositing, no off-screen canvases, no render targets, no z-order.
- **No hidden render state.** No global camera, no transform stack, no scroll registers.
- **Fixed 60 Hz, no catch-up.** `dt` is always `1/60` (SPEC §5.3).
- **The RNG is part of the spec.** Never swap algorithms, never seed from the OS, never add an "unseeded" path (SPEC §5.5).
- **No GPU work beyond one textured quad.** The console is a framebuffer and a palette; shaders are not the place to implement effects.
- **`rico16-core` stays pure.** No `std::fs`, `std::time`, `std::thread`, `std::env`, no `SystemTime`, no `Instant`. It must stay `wasm32`-clean so the web player is possible later. Enforce with a test that greps the crate if needed.

## Where to change a number

Limits (screen size, tile size, sheet size, map size, code budget, PNG layout) exist in exactly one place: the `limits` module in `rico16-core`, each constant carrying a `// SPEC §x.y` comment. SPEC §7 is the human-facing copy. Changing one without the other is a bug. Both are a **major** version change.

## Changing the script API

1. Update the table in SPEC §9.3 first.
2. Implement in `rico16-script/src/api/<module>.rs` — one file per module, one registration function, no scattered `register_fn` calls.
3. Add the overloads explicitly; do not rely on Rhai coercion.
4. Error messages must include the cart name, line and column (SPEC §9.4.1). Bare `Runtime error` is a defect.
5. Add a test that the function exists with the documented arity, and a behavioural test if it draws.

Never expose a function that is not in the spec table. Never rename one without a changelog entry.

## Changing the cart format

- Every format change needs: SPEC update, changelog entry, and a fixture.
- `parse → serialize` must be byte-identical for canonical carts (SPEC §10.7). Keep that test green; it is the cheapest defence against silent corruption.
- Read permissively (CRLF, BOM, either section order), write canonically.
- Unknown sections and unknown metadata keys must survive a round-trip. If a change breaks that, it is wrong.

## Testing rules

- Pixel-exact tests for every drawing primitive, including clipping at all four edges and fully off-screen draws.
- Round-trip tests for `.r16` and for pack/unpack.
- A determinism test: run a fixed cart for N frames twice, hash the framebuffer, compare.
- Fuzz-ish edge cases that must never panic: negative coordinates, `w`/`h` ≤ 0, values at 255, map access at the bounds, empty code section, cart with no entry points.
- No test may depend on wall-clock time, RNG entropy, or the order of directory entries.

Definition of done for any change:

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

All three clean, plus spec/docs updated in the same commit.

## Dependency policy

Ask before adding a crate. The approved set is deliberately small and each one has a job:

| Crate | Used by | Job |
|---|---|---|
| `rhai` | script | scripting |
| `ratatui`, `crossterm` | tui | editor |
| `winit` | player | window and input |
| `pixels` (fallback: raw `wgpu`) | player | one texture, one quad |
| `png` | cart | encode/decode |
| `crc32fast` | cart | blob checksum |
| `thiserror` | all libraries | error types |
| `clap` | cli | argument parsing |

Rules: no `anyhow` in library crates (typed errors); no duplicate-purpose crates; no `unsafe` without a written justification in the PR and a comment on the block; keep `player`-only and `tui`-only crates out of `core`.

The present path is isolated behind a small trait in `rico16-player` so `pixels` can be replaced by raw `wgpu` without touching anything else.

## Style

- Rust 2024 edition, stable toolchain, MSRV 1.85.
- `rustfmt` defaults; no `#[rustfmt::skip]` without a reason in a comment.
- Prefer explicit integer widths in anything that touches the cart format (`u8`, `u16`, `u32`, `u64`). Never `usize` in on-disk structures.
- Public items get a doc comment that states the SPEC clause they implement, e.g. `/// SPEC §6.5 — 8×8 font, printable ASCII.`
- Errors are types, not strings. User-facing text is lowercase, no trailing period, no "Error:" prefix.

## Commits and PRs

- One logical change per commit. Formatting fixes do not ride along with behaviour changes.
- Message: imperative subject line, then *why* in the body if it is not obvious.
- A PR that changes behaviour must reference the SPEC section it affects.
- Do not commit generated artifacts, `target/`, cart binaries used only as scratch, or editor state.

## Working with carts

- Example carts live in `examples/` and must run at HEAD. A broken example is a broken build.
- Do not reformat example carts by hand; use the tool, so canonical form stays canonical.
- Test carts that are deliberately malformed belong in `tests/fixtures/` with a name starting `invalid_`.

## When in doubt

Ask, and prefer the answer that keeps the console boring and predictable. A fantasy console's charm comes from its constraints, not from clever escape hatches. If a feature needs an exception to this document, it probably needs a spec change — and spec changes are decisions, not implementation details.
