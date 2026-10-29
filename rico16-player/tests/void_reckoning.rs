//! A headless smoke test for the ported `examples/void-reckoning` cart, following
//! the same beats as the original PICO-8 `chapters/ch01/tests/smoke.lua`.
//!
//! The original pokes the game's globals directly. A cart cannot be poked from
//! the host, so the cart exposes a small button-driven surface instead: holding V
//! prints a state line to the console, and holding V+C teleports to the first
//! station. That is enough to drive the same playthrough.

use rico16_player::CartSource;
use rico16_script::Runtime;

const RIGHT: u8 = 1 << 1;
const UP: u8 = 1 << 2;
const DOWN: u8 = 1 << 3;
const Z: u8 = 1 << 4;
const X: u8 = 1 << 5;
const V: u8 = 1 << 7;

struct Game {
    rt: Runtime,
}

#[derive(Debug, Default, PartialEq)]
struct Probe {
    st: i64,
    qs: i64,
    hp: i64,
    shd: i64,
    fuel: i64,
    cr: i64,
    ore: i64,
    mis: i64,
    x: i64,
    y: i64,
    foes: i64,
    stns: i64,
    mg: i64,
    dg: i64,
    mi: i64,
}

impl Game {
    fn new() -> Self {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/void-reckoning");
        let assets = CartSource::Project(dir).load().expect("load the cart");
        let mut rt = Runtime::new(assets).expect("runtime");
        rt.init().expect("init");
        Self { rt }
    }

    fn tick(&mut self, n: u32) {
        for _ in 0..n {
            self.rt.step(0).expect("step");
        }
    }

    /// One frame with the button held, then one frame released.
    fn press(&mut self, button: u8) {
        self.rt.step(button).expect("press");
        self.rt.step(0).expect("release");
    }

    fn hold(&mut self, button: u8, n: u32) {
        for _ in 0..n {
            self.rt.step(button).expect("hold");
        }
    }

    /// Hold V for one frame and parse the state line the cart prints.
    fn probe(&mut self) -> Probe {
        self.rt.take_output();
        self.rt.step(V).expect("probe");
        let out = self.rt.take_output();
        let line = out
            .iter()
            .rev()
            .find(|l| l.starts_with("VR "))
            .unwrap_or_else(|| panic!("no probe line in {out:?}"));
        let mut p = Probe::default();
        for field in line[3..].split_whitespace() {
            let (k, v) = field.split_once('=').expect("key=value");
            let v: i64 = v.parse().expect("integer");
            match k {
                "st" => p.st = v,
                "qs" => p.qs = v,
                "hp" => p.hp = v,
                "shd" => p.shd = v,
                "fuel" => p.fuel = v,
                "cr" => p.cr = v,
                "ore" => p.ore = v,
                "mis" => p.mis = v,
                "x" => p.x = v,
                "y" => p.y = v,
                "foes" => p.foes = v,
                "stns" => p.stns = v,
                "mg" => p.mg = v,
                "dg" => p.dg = v,
                "mi" => p.mi = v,
                other => panic!("unknown probe field {other}"),
            }
        }
        p
    }

    /// Skip whatever is on screen until the flight state is reached.
    fn skip_to_flight(&mut self) {
        for _ in 0..4000 {
            let p = self.probe();
            if p.st == 2 {
                return;
            }
            self.press(X);
        }
        panic!("never reached flight: {:?}", self.probe());
    }
}

/// SPEC §16-ish: the port boots, plays through chapter 1's loop and never errors.
#[test]
fn void_reckoning_smoke() {
    let mut g = Game::new();

    // boots to the title
    let p = g.probe();
    assert_eq!(p.st, 0, "boots to the title");

    // X starts the story; skipping the cutscenes hands over to flight
    g.press(X);
    assert_eq!(g.probe().st, 1, "X on the title starts the story");
    g.skip_to_flight();

    let p = g.probe();
    assert_eq!(
        (p.hp, p.fuel, p.qs),
        (100, 100, 1),
        "fresh ship, first quest stage"
    );
    assert_eq!(p.stns, 2, "chapter 1 authors two stations");

    // arrows fly the ship and thrust burns fuel
    let x0 = p.x;
    g.hold(RIGHT, 30);
    let p = g.probe();
    assert!(p.x > x0 + 10, "arrows fly the ship (x {x0} -> {})", p.x);
    assert!(p.fuel < 100, "thrust burns fuel");

    // X away from any station is the cannon, not a dock
    g.press(X);
    assert_eq!(g.probe().st, 2, "X in open space stays in flight");

    // docking works, and the first visit plays the officer's briefing
    g.hold(V, 3);
    g.step_press(X);
    let p = g.probe();
    assert_eq!(p.st, 4, "X beside the outpost docks");
    assert_eq!(p.dg, 1, "the officer hails on the first visit");
    for _ in 0..400 {
        if g.probe().dg == 0 {
            break;
        }
        g.press(X);
    }
    assert_eq!(g.probe().dg, 0, "the briefing can be skipped");

    // the services menu responds to the arrows and X
    g.press(DOWN);
    assert_eq!(g.probe().mi, 2, "down moves the menu selection");
    g.press(UP);
    assert_eq!(g.probe().mi, 1, "up moves it back");
    g.press(X);
    g.press(X);

    // Z undocks
    g.press(Z);
    assert_eq!(g.probe().st, 2, "Z undocks");

    // fly in every direction: a negative heading used to produce a negative
    // sprite id (Lua's `%` is floored, Rust's is truncated) and spr() walked off
    // the top of the sheet
    g.hold(UP, 40);
    g.hold(UP | 2, 40);
    g.hold(2, 40);
    g.hold(2 | DOWN, 40);
    g.hold(DOWN, 40);
    g.hold(DOWN | 1, 40);
    g.hold(1, 40);
    g.hold(1 | UP, 40);
    assert_eq!(g.probe().st, 2, "still flying after a full turn");

    // and the flight loop keeps running
    g.hold(RIGHT, 60);
    g.hold(X, 50);
    g.press(Z);
    g.tick(200);
    assert_eq!(g.probe().st, 2, "still flying after a soak");
}

impl Game {
    /// A press that also holds V, so the following probe still sees a frame.
    fn step_press(&mut self, button: u8) {
        self.rt.step(button).expect("press");
        self.rt.step(0).expect("release");
    }
}

/// Chapter 2 (XR-7PO): the warp in, then the relic hunt, with its own rules.
#[test]
fn void_reckoning_chapter2() {
    let mut g = Game::new();

    // nine frames of V jumps to chapter 2
    g.hold(V, 9);
    assert_eq!(g.probe().st, 0, "chapter 2 opens on the title");

    // X starts its intro warp, which hands over to the story, then to flight
    g.press(X);
    g.skip_to_flight();

    let p = g.probe();
    assert_eq!(p.stns, 7, "the archive, five containers and the gate");
    assert_eq!(p.ore, 0, "no relics yet");
    assert_eq!(p.foes, 0, "XR-7PO has no ambient patrols");
    assert_eq!(p.cr, 0, "there is no money here");

    // it flies like chapter 1
    let x0 = p.x;
    g.hold(RIGHT, 30);
    assert!(g.probe().x > x0 + 10, "arrows fly the ship");

    // docking a sealed container opens the slicing minigame
    g.hold(V, 6);
    g.step_press(X);
    let p = g.probe();
    assert_eq!(p.mg, 1, "a container opens the slicing minigame");
    assert_eq!(p.dg, 0, "containers do not talk");

    // Z abandons the slice, which trips the alarm and spawns guards
    g.press(Z);
    let p = g.probe();
    assert_eq!(p.mg, 0, "the minigame closes");
    assert_eq!(p.st, 2, "back to flight");
    assert!(
        p.foes > 0,
        "a failed slice brings the guards ({} foes)",
        p.foes
    );

    // and it keeps running
    g.hold(RIGHT, 60);
    g.hold(X, 50);
    g.tick(200);
    assert_eq!(g.probe().st, 2, "still flying after a soak");
}
