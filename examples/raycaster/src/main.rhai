// A Doom-style raycaster.
//
// The level is `assets/map.png`, declared as [map] in cart.toml: 128x128
// grayscale, where the sample value is the tile id (SPEC §10.2.3). 0 is open
// floor and 1-4 are wall types. Nothing here is pre-baked into the cart — the
// walls are cast every frame from the map, and the whole scene is 320
// gfx::rect_filled calls plus a handful of bands.

import "scenes/world" as world;

fn on_init() {
    world::reset();
}

fn on_update(dt) {
    world::update(dt);
}

fn on_draw() {
    world::draw();
}
