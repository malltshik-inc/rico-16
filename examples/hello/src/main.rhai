// The smallest interesting cart: a bouncing square that follows the d-pad.

fn on_init() {
    state.x = 160;
    state.y = 120;
    state.dx = 1;
    state.dy = 1;
}

fn on_update(dt) {
    if input::held(Button::Left) { state.dx = -1; }
    if input::held(Button::Right) { state.dx = 1; }
    if input::held(Button::Up) { state.dy = -1; }
    if input::held(Button::Down) { state.dy = 1; }

    state.x += state.dx * 2;
    state.y += state.dy * 2;

    if state.x <= 0 || state.x >= 312 { state.dx = -state.dx; }
    if state.y <= 0 || state.y >= 232 { state.dy = -state.dy; }
}

fn on_draw() {
    gfx::clear(1);
    gfx::rect_filled(state.x, state.y, 8, 8, 6);
    gfx::text("arrows to steer", 8, 224, 5);
}
