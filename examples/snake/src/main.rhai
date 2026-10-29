// Snake: main only dispatches; the cart's data lives in the global `state`.

import "scenes/play" as play;

fn on_init() {
    play::reset();
}

fn on_update(dt) {
    play::update(dt);
}

fn on_draw() {
    play::draw();
}
