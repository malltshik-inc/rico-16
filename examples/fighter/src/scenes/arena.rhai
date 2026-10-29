// The arena. A module has no state of its own (SPEC §11.4), so the fighters
// live in the cart's global `state`.
//
// Each fighter is a map, and the fighter functions are called *on* it —
// `state.a.control(state.b)` — so `this` is the fighter and mutations stick.
// Rhai passes arguments by value, so a plain `fn control(f) { f.x += 1 }` would
// quietly do nothing.

// --- setup -----------------------------------------------------------------

fn reset() {
    state.floor_y = 192.0;
    state.left = 24.0;
    state.right = 296.0;
    state.frames = 0;
    state.banner = "FIGHT!";
    state.banner_timer = 60;
    state.over = 0;
    state.a = fighter(88.0, 1, state.floor_y, state.left, state.right);
    state.b = fighter(200.0, -1, state.floor_y, state.left, state.right);
    system::seed(11);
}

/// Each fighter carries the arena bounds it needs. It would be tidier to read
/// them from `state`, but a method called on `state.a` cannot also touch
/// `state`: the runtime sees a re-entrant borrow of the same shared value and
/// reports a data race. Passing what a fighter needs keeps its methods
/// self-contained.
fn fighter(x, facing, floor_y, left, right) {
    #{
        x: x,
        y: 0.0,              // height above the floor
        vx: 0.0,
        vy: 0.0,
        on_ground: true,
        facing: facing,      // 1 faces right, -1 faces left
        walk: 0.0,           // animation clock
        attack: 0,           // frames left in the current attack, 0 = none
        kind: 0,             // 1 punch, 2 kick
        landed: false,       // this attack has already connected
        stun: 0,             // frames left unable to act
        block: false,
        health: 100,
        floor_y: floor_y,
        left: left,
        right: right,
    }
}

// --- fighter behaviour -----------------------------------------------------

fn feet() { this.floor_y - this.y }

fn centre() { this.x + 20.0 }

/// Which cell of the sheet to draw. See tools/make_sprites.py.
fn pose() {
    if this.stun > 0 { return 7; }              // hit
    if this.attack > 0 {
        if this.kind == 1 { return 4; }         // punch
        if this.attack > 9 { return 5; }        // kick, wind-up
        return 6;                               // kick, extended
    }
    if !this.on_ground { return 5; }
    if this.block { return 3; }                 // crouch behind the guard
    if abs(this.vx) > 0.2 {
        if this.walk > 1.5 { return 1; }        // walk, stride
        return 2;                               // walk, together
    }
    0                                           // idle
}

fn step() {
    if !this.on_ground {
        this.vy += 0.55;
        this.y -= this.vy;
        if this.y <= 0.0 {
            this.y = 0.0;
            this.vy = 0.0;
            this.on_ground = true;
        }
    }

    this.x += this.vx;
    if this.x < this.left { this.x = this.left; }
    if this.x > this.right - 40.0 { this.x = this.right - 40.0; }

    this.vx *= 0.62;
    if abs(this.vx) < 0.05 { this.vx = 0.0; }

    if this.stun > 0 { this.stun -= 1; }
    if this.attack > 0 { this.attack -= 1; }

    this.walk += 0.3;
    if this.walk > 3.0 { this.walk = 0.0; }
}

fn face(other) {
    if this.centre() < other.centre() { this.facing = 1; } else { this.facing = -1; }
}

fn control(other) {
    if this.stun > 0 { return; }

    this.block = input::held(Button::V);

    if !this.block {
        if input::held(Button::Left) { this.vx = -2.0; }
        if input::held(Button::Right) { this.vx = 2.0; }
    }

    if this.attack == 0 {
        if input::pressed(Button::Z) { this.start(1); }
        if input::pressed(Button::X) { this.start(2); }
        if input::pressed(Button::Up) && this.on_ground {
            this.vy = -8.0;
            this.on_ground = false;
        }
    }

    this.face(other);
}

fn start(kind) {
    this.kind = kind;
    this.attack = if kind == 1 { 12 } else { 20 };
    this.landed = false;
}

/// A deliberately simple opponent: close the gap, then throw something.
fn cpu(other) {
    if this.stun > 0 { return; }

    let gap = abs(this.centre() - other.centre());
    let towards = if this.centre() < other.centre() { 1.0 } else { -1.0 };
    this.block = false;

    if this.attack == 0 {
        if gap > 46.0 {
            this.vx = towards * 1.5;
        } else if gap < 34.0 && rnd::int(0, 70) == 0 {
            this.start(1);
        } else if gap < 52.0 && rnd::int(0, 110) == 0 {
            this.start(2);
        } else if rnd::int(0, 150) == 0 {
            this.block = true;
        }
    }

    this.face(other);
}

// --- hits ------------------------------------------------------------------

/// Is `f`'s attack in its active frames and not yet spent?
fn active(f) {
    if f.attack == 0 || f.landed { return false; }
    if f.kind == 1 { return f.attack <= 9 && f.attack >= 5; }
    f.attack <= 15 && f.attack >= 7
}

/// The attack's box as `[x0, y0, x1, y1]`, in front of `f`.
fn hitbox(f) {
    let reach = if f.kind == 1 { 24.0 } else { 34.0 };
    let high = if f.kind == 1 { 44.0 } else { 34.0 };
    let low = if f.kind == 1 { 26.0 } else { 15.0 };
    let feet = f.floor_y - f.y;

    if f.facing > 0 {
        [f.x + 28.0, feet - high, f.x + 28.0 + reach, feet - low]
    } else {
        [f.x + 12.0 - reach, feet - high, f.x + 12.0, feet - low]
    }
}

/// Does `box` overlap `g`'s body?
fn strikes(box, g) {
    let feet = g.floor_y - g.y;
    box[0] < g.x + 30.0
        && box[2] > g.x + 10.0
        && box[1] < feet
        && box[3] > feet - 58.0
}

/// `[damage, knockback]` for `attacker` hitting `victim`.
fn hurt(victim, attacker) {
    let facing_each_other = (victim.facing > 0 && victim.centre() < attacker.centre())
        || (victim.facing < 0 && victim.centre() > attacker.centre());
    if victim.block && facing_each_other { return [3, 1.5]; }
    if attacker.kind == 1 { return [6, 2.0]; }
    [11, 3.5]
}

fn resolve_hits() {
    if active(state.a) && strikes(hitbox(state.a), state.b) {
        state.a.landed = true;
        let hit = hurt(state.b, state.a);
        state.b.health -= hit[0];
        state.b.stun = 16;
        state.b.vx = state.a.facing * hit[1];
    }
    if active(state.b) && strikes(hitbox(state.b), state.a) {
        state.b.landed = true;
        let hit = hurt(state.a, state.b);
        state.a.health -= hit[0];
        state.a.stun = 16;
        state.a.vx = state.b.facing * hit[1];
    }
}

// --- frame -----------------------------------------------------------------

fn update(dt) {
    state.frames += 1;
    if state.banner_timer > 0 { state.banner_timer -= 1; }

    if state.over > 0 {
        state.over -= 1;
        if state.over == 0 { reset(); }
        return;
    }

    state.a.control(state.b);
    state.b.cpu(state.a);
    state.a.step();
    state.b.step();
    resolve_hits();

    if state.a.health <= 0 || state.b.health <= 0 {
        state.banner = if state.a.health <= 0 { "K.O.  CPU WINS" } else { "K.O.  YOU WIN" };
        state.banner_timer = 150;
        state.over = 150;
    }
}

// --- drawing ---------------------------------------------------------------

fn draw() {
    draw_arena();
    draw_fighter(state.a, 0);
    draw_fighter(state.b, 1);
    draw_hud();
}

fn draw_arena() {
    // Sky above the horizon, floor below, both as gradient bands. The floor is
    // a green ramp and the fighters are red and blue, so neither fighter
    // disappears into the ground.
    let i = 0;
    while i < 8 {
        gfx::rect_filled(0, i * 15, 320, 15, 176 + 8 - i);
        gfx::rect_filled(0, 120 + i * 15, 320, 15, 80 + i);
        i += 1;
    }

    // A crowd: two rows of coloured heads along the horizon.
    let x = 0;
    while x < 320 {
        let colour = 16 + ((x / 8) % 15) * 16 + 7;
        gfx::rect_filled(x, 100, 7, 7, colour);
        gfx::rect_filled(x + 4, 111, 7, 7, colour);
        x += 12;
    }

    // The ring: a line at the floor and two posts.
    gfx::rect_filled(0, state.floor_y, 320, 2, 5);
    gfx::rect_filled(state.left - 4.0, 120, 2, state.floor_y - 120.0, 3);
    gfx::rect_filled(state.right + 2.0, 120, 2, state.floor_y - 120.0, 3);
}

fn draw_fighter(f, which) {
    let pose = f.pose();
    // Fighter 1 is the same art mirrored: sprite::draw_region's flip_x.
    sprite::draw_region(
        (pose % 6) * 40,
        (which * 2 + pose / 6) * 60,
        40,
        60,
        f.x,
        f.floor_y - 60.0 - f.y,
        which == 1,
        false,
    );
}

fn draw_hud() {
    let width = 120;

    let mine = state.a.health;
    if mine < 0 { mine = 0; }
    gfx::rect_filled(8, 8, width, 10, 1);
    gfx::rect_filled(8, 8, width * mine / 100, 10, 89);

    let theirs = state.b.health;
    if theirs < 0 { theirs = 0; }
    gfx::rect_filled(192, 8, width, 10, 1);
    gfx::rect_filled(312 - width * theirs / 100, 8, width * theirs / 100, 10, 25);

    if state.banner_timer > 0 {
        let half = gfx::text_width(state.banner) / 2;
        gfx::text(state.banner, 160 - half, 60, 5);
    }

    gfx::text("arrows move  z punch  x kick", 8, 222, 5);
    gfx::text("up jumps  v blocks", 8, 230, 4);
}
