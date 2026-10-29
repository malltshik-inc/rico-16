// The raycaster's world scene. Modules have no state of their own (SPEC §11.4),
// so everything lives in the global `state`; the board's constants are literals
// because module-level `let`/`const` are not visible inside functions.

// ---------------------------------------------------------------------------
// Setup
// ---------------------------------------------------------------------------

fn reset() {
    // The generator prints this: the longest corridor view it could find, so
    // the first frame shows perspective instead of a flat wall.
    state.px = 48.5;
    state.py = 10.0;
    state.angle = 0.0;
    state.fov = 0.66;          // half-width of the camera plane: a 66 degree view
    state.speed = 0.055;
    state.turn = 0.045;
    state.bob = 0.0;
    state.show_map = true;
    state.frames = 0;

    if blocked(state.px, state.py) {
        let open = find_open_cell();
        state.px = open[0];
        state.py = open[1];
    }
}

fn find_open_cell() {
    let y = 1;
    while y < 127 {
        let x = 1;
        while x < 127 {
            if tilemap::get(x, y) == 0 {
                return [x + 0.5, y + 0.5];
            }
            x += 1;
        }
        y += 1;
    }
    [21.5, 21.5]
}

// ---------------------------------------------------------------------------
// Input
// ---------------------------------------------------------------------------

fn update(dt) {
    if input::held(Button::Left) { state.angle -= state.turn; }
    if input::held(Button::Right) { state.angle += state.turn; }

    let dx = cos(state.angle);
    let dy = sin(state.angle);

    let moving = false;
    if input::held(Button::Up) { try_move(dx * state.speed, dy * state.speed); moving = true; }
    if input::held(Button::Down) { try_move(-dx * state.speed, -dy * state.speed); moving = true; }
    if input::held(Button::Z) { try_move(-dy * state.speed, dx * state.speed); moving = true; }
    if input::held(Button::X) { try_move(dy * state.speed, -dx * state.speed); moving = true; }

    // A little head bob while walking, so movement is visible without sprites.
    if moving { state.bob += 0.25; } else { state.bob = 0.0; }

    if input::pressed(Button::C) { state.show_map = !state.show_map; }
    if input::pressed(Button::V) { reset(); }

    state.frames += 1;
}

fn try_move(mx, my) {
    // Step each axis separately so sliding along a wall works.
    if !blocked(state.px + mx * 2.0, state.py) { state.px += mx; }
    if !blocked(state.px, state.py + my * 2.0) { state.py += my; }
}

fn blocked(x, y) {
    if x < 1.0 || y < 1.0 || x > 126.0 || y > 126.0 { return true; }
    tilemap::get(x, y) > 0
}

// ---------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------

fn draw() {
    draw_backdrop();
    cast_walls();
    draw_hud();
    if state.show_map { draw_minimap(); }
}

/// Sky and floor as a handful of horizontal bands: a gradient for two calls
/// per band instead of 76,800 gfx::pixel calls.
fn draw_backdrop() {
    let i = 0;
    while i < 8 {
        // Blue ramp (base 176), lighter towards the top of the sky.
        gfx::rect_filled(0, i * 15, 320, 15, 176 + 8 - i);
        // Brown ramp (base 32), lighter towards the viewer's feet.
        gfx::rect_filled(0, 120 + i * 15, 320, 15, 32 + i);
        i += 1;
    }
}

/// One vertical stripe per screen column. This is the whole renderer.
fn cast_walls() {
    let dirx = cos(state.angle);
    let diry = sin(state.angle);
    // The camera plane is the direction turned 90 degrees, scaled by the fov.
    let planex = -diry * state.fov;
    let planey = dirx * state.fov;

    let x = 0;
    while x < 320 {
        let camera = 2.0 * x / 319.0 - 1.0;
        cast_column(x, dirx + planex * camera, diry + planey * camera);
        x += 1;
    }
}

fn cast_column(x, rdx, rdy) {
    // Classic grid DDA: walk cell to cell until the ray meets a wall.
    let mapx = floor(state.px);
    let mapy = floor(state.py);

    let deltax = if abs(rdx) < 0.0001 { 100000.0 } else { abs(1.0 / rdx) };
    let deltay = if abs(rdy) < 0.0001 { 100000.0 } else { abs(1.0 / rdy) };

    let stepx = if rdx < 0.0 { -1 } else { 1 };
    let stepy = if rdy < 0.0 { -1 } else { 1 };

    let sidedistx = if rdx < 0.0 { (state.px - mapx) * deltax } else { (mapx + 1.0 - state.px) * deltax };
    let sidedisty = if rdy < 0.0 { (state.py - mapy) * deltay } else { (mapy + 1.0 - state.py) * deltay };

    let side = 0;
    let tile = 0;
    let steps = 0;
    while steps < 96 {
        if sidedistx < sidedisty {
            sidedistx += deltax;
            mapx += stepx;
            side = 0;
        } else {
            sidedisty += deltay;
            mapy += stepy;
            side = 1;
        }
        tile = tilemap::get(mapx, mapy);
        if tile > 0 { break; }
        steps += 1;
    }
    if tile <= 0 { return; }

    // Distance to the wall, measured across the ray, not along it: this is what
    // keeps straight walls straight instead of fish-eyed.
    let perp = if side == 0 { sidedistx - deltax } else { sidedisty - deltay };
    if perp < 0.08 { perp = 0.08; }

    let height = floor(240.0 / perp);
    let top = 120.0 - height / 2.0 + sin(state.bob) * 2.0;

    // How far along the wall the ray landed. This is the texture coordinate:
    // `u` runs 0..1 across each wall cell and drives the vertical stripes.
    let hit = if side == 0 { state.py + perp * rdy } else { state.px + perp * rdx };
    let u = hit - floor(hit);

    let shade = wall_shade(perp, side, u);
    let base = ramp_for(tile);

    let y0 = if top < 0.0 { 0.0 } else { top };
    let y1 = if top + height > 240.0 { 240.0 } else { top + height };
    if y1 <= y0 { return; }

    // Two segments per column: the upper part a shade darker, which reads as
    // light coming from above.
    if y1 - y0 > 6.0 {
        let split = y0 + (y1 - y0) * 0.42;
        let upper = if shade >= 3.0 { shade - 3.0 } else { 0.0 };
        gfx::rect_filled(x, y0, 1, split - y0, base + upper);
        gfx::rect_filled(x, split, 1, y1 - split, base + shade);
    } else {
        gfx::rect_filled(x, y0, 1, y1 - y0, base + shade);
    }
}

/// Wall brightness, 0..15 within the tile's ramp.
fn wall_shade(perp, side, u) {
    // Eight vertical stripes per wall cell, alternating light and dark.
    let band = floor(u * 8.0);
    let half = floor(band / 2.0);
    let dark = if band == half * 2.0 { 0.0 } else { 4.0 };

    // A gentle distance falloff: steep enough to read as depth, shallow enough
    // that a wall at the far end of a corridor is still a wall.
    let shade = 14.0 - perp * 0.9 - dark;

    // North/south faces a shade darker, so corners read as corners.
    if side == 1 { shade -= 2.0; }

    if shade < 0.0 { shade = 0.0; }
    if shade > 15.0 { shade = 15.0; }
    shade
}

/// Wall type to palette ramp base. SPEC §4.7: index `16 + ramp * 16 + step`.
///
/// Tile ids come from the map PNG, so they are spread across the byte range
/// (32/96/160/224) rather than 1-4: see the generator.
fn ramp_for(tile) {
    if tile < 64 { return 16; }    // red
    if tile < 128 { return 80; }   // green
    if tile < 192 { return 128; }  // cyan
    208                            // magenta
}

fn draw_hud() {
    // A crosshair, and hints short enough to clear the minimap on the right.
    gfx::rect_filled(159, 118, 2, 2, 5);
    gfx::text("arrows move/turn", 4, 4, 5);
    gfx::text("z x strafe  c map", 4, 14, 5);
}

fn draw_minimap() {
    // A 32x24 cell window of the map, two pixels per cell, top right.
    let scale = 2;
    let cells_x = 32;
    let cells_y = 24;
    let ox = 320 - cells_x * scale - 6;
    let oy = 6;
    let px = floor(state.px);
    let py = floor(state.py);

    gfx::rect_filled(ox - 2, oy - 2, cells_x * scale + 4, cells_y * scale + 4, 0);

    let cy = 0;
    while cy < cells_y {
        let cx = 0;
        while cx < cells_x {
            let tile = tilemap::get(px - 16 + cx, py - 12 + cy);
            if tile > 0 {
                let shade = ramp_for(tile) + 9;
                let sx = ox + cx * scale;
                let sy = oy + cy * scale;
                gfx::rect_filled(sx, sy, scale, scale, shade);
            }
            cx += 1;
        }
        cy += 1;
    }

    gfx::rect_filled(ox + 16 * scale - 1, oy + 12 * scale - 1, 4, 4, 5);
}
