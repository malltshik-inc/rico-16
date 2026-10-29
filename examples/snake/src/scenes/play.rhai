// Snake's play scene: a library with no state of its own (SPEC §11.4), reading
// and writing the cart's global `state`. Module-level constants are not visible
// inside functions, so the board geometry lives in `state` too.

fn reset() {
    state.cell = 8;
    state.cols = 40;
    state.rows = 30;
    state.body = [ #{ x: 20, y: 15 } ];
    state.dir = #{ x: 1, y: 0 };
    state.food = #{ x: 30, y: 15 };
    state.tick = 0;
    state.score = 0;
    state.dead = false;
    system::seed(1);
}

fn steer() {
    if input::pressed(Button::Left) && state.dir.x == 0 {
        state.dir = #{ x: -1, y: 0 };
    } else if input::pressed(Button::Right) && state.dir.x == 0 {
        state.dir = #{ x: 1, y: 0 };
    } else if input::pressed(Button::Up) && state.dir.y == 0 {
        state.dir = #{ x: 0, y: -1 };
    } else if input::pressed(Button::Down) && state.dir.y == 0 {
        state.dir = #{ x: 0, y: 1 };
    }
}

fn update(dt) {
    if state.dead {
        if input::pressed(Button::Z) {
            reset();
        }
        return;
    }

    steer();

    // The snake moves every eighth frame, so the game runs at 7.5 cells a second.
    state.tick += 1;
    if state.tick < 8 {
        return;
    }
    state.tick = 0;

    let head = state.body[0];
    let next = #{ x: head.x + state.dir.x, y: head.y + state.dir.y };

    if next.x < 0 || next.y < 0 || next.x >= state.cols || next.y >= state.rows {
        state.dead = true;
        return;
    }
    for segment in state.body {
        if segment.x == next.x && segment.y == next.y {
            state.dead = true;
            return;
        }
    }

    state.body.insert(0, next);
    if next.x == state.food.x && next.y == state.food.y {
        state.score += 1;
        state.food = #{ x: rnd::int(0, state.cols - 1), y: rnd::int(0, state.rows - 1) };
    } else {
        state.body.pop();
    }
}

fn draw() {
    gfx::clear(1);

    gfx::rect_filled(
        state.food.x * state.cell,
        state.food.y * state.cell,
        state.cell,
        state.cell,
        6,
    );
    for segment in state.body {
        gfx::rect_filled(
            segment.x * state.cell,
            segment.y * state.cell,
            state.cell,
            state.cell,
            10,
        );
    }

    gfx::text("score " + state.score.to_string(), 4, 4, 5);
    if state.dead {
        gfx::text("game over - press z", 4, 224, 6);
    }
}
