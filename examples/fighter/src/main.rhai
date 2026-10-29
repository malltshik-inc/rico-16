// A one-on-one fighter: two characters, an arena, movement and kicks.
//
// The fighters are 32x48 cells of `assets/fighters.png`, drawn with
// sprite::draw_region. Only one player's input exists (SPEC §9.3 exposes one
// player; a second is deferred), so fighter 1 is the CPU.

import "scenes/arena" as arena;

fn on_init() {
    arena::reset();
}

fn on_update(dt) {
    arena::update(dt);
}

fn on_draw() {
    arena::draw();
}
