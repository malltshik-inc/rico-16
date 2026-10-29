// Void Reckoning - a port of the PICO-8 game by gruber & krajzeg.
//
// Chapters 1 and 2, both in this cart. The engine and both chapters live in
// `game.rhai`: the original is one global namespace, and Rhai modules cannot
// share module-level state or import each other in a cycle, so one module with
// everything in the reserved global `state` is the faithful translation.

import "game" as game;

fn on_init() {
    game::boot();
}

fn on_update(dt) {
    game::update();
}

fn on_draw() {
    game::draw();
}
