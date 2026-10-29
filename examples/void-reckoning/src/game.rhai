// Void Reckoning - engine + both chapters, ported from the PICO-8 original.
//
// Everything lives in the reserved global `state`, because a Rhai module cannot
// have module-level state visible to its functions, and the original is one
// global namespace. Data structures keep their original shape.
//
// Deviations from the original, all forced by the rico-16 spec:
//   * audio        - `sfx`/`music` are no-ops (SPEC §14.10: no audio in v1)
//   * save data    - kept in memory only (SPEC §18.1: persistence is deferred)
//   * custom font  - dropped; the font is built-in and unreplaceable (§6.5)
//   * pscii glyphs - the PICO-8 font's button/arrow glyphs become ASCII
//   * screen       - the 128x128 frame is drawn 1:1, centred, at (96, 56)
//   * rng          - xoshiro256** is normative (SPEC §5.5), so the generated
//                    system layout differs; authored content is identical

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

// PICO-8 has atan2; Rhai's math package does not, so roll it from atan.
fn atan2(y, x) {
    let pi = 3.141592653589793;
    if x > 0.0 { return atan(y / x); }
    if x < 0.0 {
        if y >= 0.0 { return atan(y / x) + pi; }
        return atan(y / x) - pi;
    }
    if y > 0.0 { return pi / 2.0; }
    if y < 0.0 { return -pi / 2.0; }
    0.0
}

// PICO-8 has ceil; Rhai's is `ceiling`.
fn ceil(x) { -floor(-x.to_float()) }

// PICO-8's min/max, named to avoid any collision with the engine's builtins.
fn mn(a, b) { if a < b { a } else { b } }
fn mx(a, b) { if a > b { a } else { b } }

// Lua arrays read as nil past the end and Rhai raises, so every script-driven
// index goes through this. 1-based, like the original.
fn at(arr, i) { let i = i.to_int(); if i >= 1 && i <= arr.len { arr[i - 1] } else { () } }

fn clampi(v, a, b) {
    if v < a { return a; }
    if v > b { return b; }
    v
}

// PICO-8's `sub(s,a,b)` is 1-based and inclusive; Rhai's sub_string is 0-based
// with a length.
fn sub(s, a, b) {
    // the original passes numbers straight through; Rhai wants integers
    let a = a.to_int();
    let b = b.to_int();
    if a > b { return ""; }
    s.sub_string(a - 1, b - a + 1)
}

// overflow-safe hit test: axis-guard BEFORE squaring (the original guards against
// PICO-8's 16.16 fixed point; harmless here and it keeps the port literal)
fn hit2(x1, y1, x2, y2, r) {
    let dx = abs(x1 - x2);
    if dx >= r { return false; }
    let dy = abs(y1 - y2);
    if dy >= r { return false; }
    dx * dx + dy * dy < r * r
}

// exact distance by rotating the delta onto an axis, so nothing is squared
fn dist_trig(x1, y1, x2, y2) {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let a = atan2(dx, dy);
    dx * cos(a) + dy * sin(a)
}

fn arena_dist(x, y) { abs(dist_trig(state.ARX, state.ARY, x, y)) }

fn clamp_arena(x, y, m) {
    let dx = x - state.ARX;
    let dy = y - state.ARY;
    let d = abs(dist_trig(state.ARX, state.ARY, x, y));
    let lim = state.ARR - m;
    if d > lim {
        let s = lim / (d + 0.0001);
        return [state.ARX + dx * s, state.ARY + dy * s];
    }
    [x, y]
}

// word-wrap to lines no wider than w characters, each {t=line, o=start offset in
// the source} so a typewriter can reveal it without the words reflowing
fn wrap(s, w) {
    if s == "" { return [#{t: "", o: 1}]; }
    let out = [];
    let cur = "";
    let lo = 1;
    let pos = 1;
    for word in s.split(" ") {
        let word = word.to_string();
        if cur == "" { cur = word; }
        else if cur.len + 1 + word.len <= w { cur = cur + " " + word; }
        else { out.push(#{t: cur, o: lo}); cur = word; lo = pos; }
        pos += word.len + 1;
    }
    if cur != "" { out.push(#{t: cur, o: lo}); }
    out
}

// force all on-screen text to lowercase: PICO-8 draws lowercase as small-caps,
// which keeps the UI at one font size
fn lc(s) {
    let s = s.to_string();
    let o = "";
    for c in s.chars() {
        o += c.to_lower().to_string();
    }
    o
}

// string pixel width. PICO-8 mixes 4px and 8px glyphs; the console font is a
// uniform 8px, so this is simply len * 8.
fn sw(s) { s.len * 8 }

// heading (turns) -> frame 0..7
fn ship_frame(a) { floor(a * 8.0 + 0.5) % 8 }

fn ang_lerp(a, b, t) {
    let d = (b - a) % 1.0;
    if d > 0.5 { d -= 1.0; }
    (a + d * t) % 1.0
}

// ---------------------------------------------------------------------------
// drawing: the 128x128 frame is centred at (96, 56), and the world pass is
// camera-relative. The original brackets world drawing with camera()/camera();
// here that is a flag.
// ---------------------------------------------------------------------------

fn fx(x) { if state.cam { state.ox + x - state.camx } else { state.ox + x } }
fn fy(y) { if state.cam { state.oy + y - state.camy } else { state.oy + y } }

fn pset(x, y, c) { gfx::pixel(fx(x), fy(y), c) }
fn line(x0, y0, x1, y1, c) { gfx::line(fx(x0), fy(y0), fx(x1), fy(y1), c) }
fn rect(x0, y0, x1, y1, c) { gfx::rect(fx(x0), fy(y0), x1 - x0 + 1, y1 - y0 + 1, c) }
fn rectfill(x0, y0, x1, y1, c) { gfx::rect_filled(fx(x0), fy(y0), x1 - x0 + 1, y1 - y0 + 1, c) }
fn circ(cx, cy, r, c) { gfx::circle(fx(cx), fy(cy), r, c) }
fn circfill(cx, cy, r, c) { gfx::circle_filled(fx(cx), fy(cy), r, c) }
fn pt(s, x, y, c) { gfx::text(lc(s), fx(x), fy(y), c) }

// spr(n,x,y): the PICO-8 sheet is 16 sprites across, 8x8 each. rico-16 tiles are
// 16x16, so sprites are drawn as 8x8 regions of the sheet instead.
fn spr(n, x, y) {
    sprite::draw_region((n % 16) * 8, (n / 16) * 8, 8, 8, fx(x), fy(y), false, false);
}

// sspr without scaling (the original's portraits are the only scaled call, and
// those are pre-scaled into the sheet here)
fn sspr(sx, sy, w, h, dx, dy) {
    sprite::draw_region(sx, sy, w, h, fx(dx), fy(dy), false, false);
}

// audio is a stub in v1: the calls stay so the structure matches the original
fn sfx(_n, _c) {}
fn music(_n) {}
fn set_music(n) { state.mtrack = n; }

// the cart's own palette IS PICO-8's 16 colours (see assets/palette.png), so the
// motion-trail fade is a palette remap - exactly what the original does with
// poke(0x5f54) + sspr, and the same DIM table
fn fade_screen() {
    let i = 0;
    while i < 16 {
        let v = state.DIM[i];
        let t = if state.frame % 2 < 1 { v } else { state.DIM[v] };
        gfx::palette(i, state.PR[t], state.PG[t], state.PB[t]);
        i += 1;
    }
}

// ---------------------------------------------------------------------------
// boot, save (in memory), death
// ---------------------------------------------------------------------------

fn boot() {
    state.ox = 96.0;
    state.oy = 56.0;
    state.cam = false;
    state.vhold = 0;
    state.camx = 0.0;
    state.camy = 0.0;

    // PICO-8's 16 colours, and the fade table
    state.PR = [0, 33, 123, 0, 173, 99, 198, 255, 255, 255, 255, 0, 41, 132, 255, 255];
    state.PG = [0, 41, 33, 132, 82, 90, 198, 239, 0, 165, 239, 231, 173, 115, 115, 206];
    state.PB = [0, 82, 82, 82, 57, 82, 198, 231, 74, 0, 41, 57, 255, 156, 165, 173];
    state.DIM = [0, 0, 0, 1, 1, 1, 5, 6, 4, 4, 9, 3, 2, 2, 2, 9];

    // pscii glyphs the PICO-8 font draws and ours cannot: ASCII stand-ins
    state.BO = "O";
    state.BX = "X";
    state.CUR = ">";
    state.MORE = "v";
    state.ARRL = "<";
    state.ARRR = ">";
    state.CARET = "_";

    state.ARX = 256.0;
    state.ARY = 256.0;
    state.ARR = 380.0;

    state.CHGMAX = 45;
    state.FOE_CAP = 8;
    state.SPAWN_MIN = 1200;
    state.SPAWN_VAR = 600;
    state.WAVE_MIN = 3;
    state.WAVE_VAR = 3;
    state.ROID_CAP = 22;
    state.ROID_RESPAWN = 150;
    state.ROID_HP = 12;
    state.SPR_PLAYER = 0;
    state.SPR_SCOUT = 8;
    state.SPR_RAIDER = 16;
    state.MUS_FLIGHT = 0;
    state.MUS_COMBAT = 6;
    state.MUS_STATION = 10;
    state.JPER = 45;

    // cartdata is in memory only
    state.mem = [0, 0, 0, 0, 0, 0, 0, 0, 0];
    state.mtrack = -1;
    state.mtcombat = false;
    state.frame = 0;
    state.chapter = 1;
    state.hassave = false;
    state.SEED = 1000.0 + state.chapter + 7919.0;
    new_game();
}

fn next_seed() {
    state.mem[6] = state.mem[6] + 1;
    return 1000.0 + state.chapter + state.mem[6] * 7919.0;
}

fn dget(i) { state.mem[i] }
fn dset(i, v) { state.mem[i] = v }

fn new_game() {
    install_chapter();
    init_player();
    load_system();
    init_story();
    state.st = 0;
}

fn init_player() {
    state.p = #{
        x: 64.0, y: 64.0, dx: 0.0, dy: 0.0, a: 0.25,
        hp: 100.0, mhp: 100.0, shd: 50.0, mshd: 50.0, scd: 0,
        fuel: 100.0, mfuel: 100.0, credits: 40.0, mis: 10.0, mmis: 10.0,
        wcd: 0, ore: 0.0, mcargo: 10.0, chg: 0.0, ramt: 0
    };
    state.dth = "";
    state.shk = 0;
    state.got = 0;
    state.spawnt = state.SPAWN_MIN;
    state.msg = "";
    state.msgt = 0;
}

fn reset_game() {
    state.SEED = next_seed();
    new_game();
    state.hassave = false;
    dset(8, 0);
}

fn save_game() {
    dset(0, state.chapter);
    dset(1, state.qs);
    dset(2, if state.ohint { 1 } else { 0 });
    dset(3, state.p.credits);
    dset(4, state.p.ore);
    dset(5, state.p.mis);
    dset(6, state.SEED);
    dset(8, 1);
    state.hassave = true;
}

fn load_game() {
    if dget(8) != 1 || dget(0) != state.chapter { return false; }
    state.qs = dget(1);
    state.ohint = dget(2) == 1;
    state.p.credits = dget(3);
    state.p.ore = dget(4);
    state.p.mis = dget(5);
    state.p.x = state.active.x;
    state.p.y = state.active.y + 18.0;
    true
}

fn recover() {
    if state.chapter == 2 { ch2_recover(); return; }
    let fee = mn(state.p.credits, floor(state.p.credits * 0.25));
    state.p.credits -= fee;
    state.p.ore = 0.0;
    state.p.hp = state.p.mhp;
    state.p.shd = state.p.mshd;
    state.p.fuel = state.p.mfuel;
    state.p.dx = 0.0;
    state.p.dy = 0.0;
    state.p.chg = 0.0;
    state.p.scd = 0;
    state.p.ramt = 0;
    state.p.x = state.active.x;
    state.p.y = state.active.y + 18.0;
    msg_set("RECOVERED -" + fee.to_string() + "$  CARGO LOST");
    save_game();
    state.st = 2;
}

fn msg_set(s) {
    state.msg = s;
    state.msgt = 200;
}

// ---------------------------------------------------------------------------
// the frame
// ---------------------------------------------------------------------------


// ---------------------------------------------------------------------------
// port-only test surface. The original's smoke test reaches into the game's
// globals and sets them; a cart cannot be poked from the host, so the same
// capability is driven by a button chord instead: hold V to print a state line
// to the console, hold V+C to teleport to the first station. Neither does
// anything unless a button is held, so normal play is untouched.
// ---------------------------------------------------------------------------

fn test_hooks() {
    if !input::held(Button::V) { state.vhold = 0; return; }
    state.vhold += 1;
    print("VR st=" + state.st.to_int().to_string()
        + " qs=" + state.qs.to_int().to_string()
        + " hp=" + floor(state.p.hp).to_int().to_string()
        + " shd=" + floor(state.p.shd).to_int().to_string()
        + " fuel=" + floor(state.p.fuel).to_int().to_string()
        + " cr=" + state.p.credits.to_int().to_string()
        + " ore=" + state.p.ore.to_int().to_string()
        + " mis=" + state.p.mis.to_int().to_string()
        + " x=" + floor(state.p.x).to_int().to_string()
        + " y=" + floor(state.p.y).to_int().to_string()
        + " foes=" + state.foes.len.to_string()
        + " stns=" + state.stations.len.to_string()
        + " mg=" + if state.mg == () { "0" } else { "1" }
        + " dg=" + if state.dg == () { "0" } else { "1" }
        + " mi=" + (if state.st == 8 { state.mm.i }
            else if state.st == 4 && state.sm != () { state.sm.i }
            else { 0 }).to_string());
    // One shot per hold, keyed on how long V is held, so the hooks never
    // collide: 3 frames teleports to the first station, 6 to the second (a
    // sealed container in chapter 2), 9 jumps to chapter 2.
    if state.vhold == 3 && state.stations.len > 0 {
        state.p.x = state.stations[0].x;
        state.p.y = state.stations[0].y + 8.0;
    } else if state.vhold == 6 && state.stations.len > 1 {
        state.p.x = state.stations[1].x;
        state.p.y = state.stations[1].y + 8.0;
    } else if state.vhold == 9 {
        start_chapter(2);
    }
}

fn update() {
    state.frame += 1;
    test_hooks();
    if state.msgt > 0 { state.msgt -= 1; }
    if state.st == 0 {
        if input::pressed(Button::X) {
            if state.hassave { state.st = 2; } else { state.st = 1; start_story(); }
        } else if input::pressed(Button::Z) && state.hassave { reset_game(); }
    } else if state.st == 1 {
        update_story();
    } else if state.st == 2 {
        update_flight();
    } else if state.st == 3 {
        update_neg();
    } else if state.st == 4 {
        update_station();
    } else if state.st == 5 {
        update_jump();
    } else if state.st == 8 {
        update_menu();
    } else if state.st == 6 {
        update_fx();
        if state.got > 0 { state.got -= 1; }
        else if input::pressed(Button::Z) || input::pressed(Button::X) { recover(); }
    }
}

fn draw() {
    if state.st != 2 {
        // every state but flight clears; flight keeps the frame so the palette
        // fade can dim it into motion trails
        gfx::palette_reset();
        state.cam = false;
        gfx::clear(0);
    }
    if state.st == 0 { draw_title(); }
    else if state.st == 1 { draw_story(); }
    else if state.st == 2 { draw_flight(); }
    else if state.st == 3 { draw_neg(); }
    else if state.st == 4 { draw_station(); }
    else if state.st == 5 { draw_jump(); }
    else if state.st == 8 { draw_menu(); }
    else if state.st == 6 { draw_gameover(); }
}

// ---------------------------------------------------------------------------
// flight sim: foes, asteroids, drops, particles
// ---------------------------------------------------------------------------

fn update_flight() {
    if input::pressed(Button::C) { open_menu(); return; }
    if state.p.ramt > 0 { state.p.ramt -= 1; }

    if input::pressed(Button::X) {
        let st = nearest_station(20.0);
        if st != () {
            state.p.chg = 0.0;
            if st.kind == "gate" { start_jump(); } else { dock(st); }
            return;
        }
    }

    // snappy 8-way: the nose follows input, quick stop
    let ax = 0.0;
    let ay = 0.0;
    if input::held(Button::Up) { ay -= 1.0; }
    if input::held(Button::Down) { ay += 1.0; }
    if input::held(Button::Left) { ax -= 1.0; }
    if input::held(Button::Right) { ax += 1.0; }
    let tx = 0.0;
    let ty = 0.0;
    if (ax != 0.0 || ay != 0.0) && state.p.fuel > 0.0 {
        let m = sqrt(ax * ax + ay * ay);
        let spd = 1.8;
        tx = ax / m * spd;
        ty = ay / m * spd;
        state.p.a = atan2(ax, ay);
        state.p.fuel = mx(0.0, state.p.fuel - 0.06);
    }
    state.p.dx += (tx - state.p.dx) * 0.35;
    state.p.dy += (ty - state.p.dy) * 0.35;
    state.p.x += state.p.dx;
    state.p.y += state.p.dy;

    // dry out in dead space and you are lost
    if state.p.fuel <= 0.0 && arena_dist(state.p.x, state.p.y) > state.ARR {
        damage_player(30000.0, "ADRIFT IN THE VOID");
        return;
    }

    if state.p.scd > 0 { state.p.scd -= 1; }
    else if state.p.shd < state.p.mshd && state.p.chg <= 0.0 {
        state.p.shd = mn(state.p.mshd, state.p.shd + 0.1);
    }

    if state.p.wcd > 0 { state.p.wcd -= 1; }
    if input::held(Button::X) && state.p.wcd <= 0 && state.p.fuel > 0.0 {
        state.p.chg = mn(state.p.chg + 1.0, state.CHGMAX);
    } else if state.p.chg > 0.0 {
        fire_primary(1.0 + floor(state.p.chg / state.CHGMAX * 2.0));
        state.p.chg = 0.0;
    }

    update_spawn();

    if input::pressed(Button::Z) && state.p.fuel > 0.0 { fire_missile(); }

    update_bullets();
    update_missiles();
    update_foes();
    update_roids();
    update_drops();
    update_fx();
    collide_all();
    update_camera();

    // combat theme while enemies are engaged, with hysteresis
    let dmin = 30000.0;
    for f in state.foes {
        let d = abs(f.x - state.p.x) + abs(f.y - state.p.y);
        if d < dmin { dmin = d; }
    }
    if dmin < 220.0 { state.mtcombat = true; }
    else if dmin > 340.0 { state.mtcombat = false; }
    set_music(if state.mtcombat { state.MUS_COMBAT } else { state.MUS_FLIGHT });
}

fn spawn_foe(kind, x, y) {
    let d = state.foes_def[kind];
    state.foes.push(#{
        kind: kind, x: x, y: y, dx: 0.0, dy: 0.0,
        hp: d.hp, mhp: d.hp, shd: d.shd, mshd: d.shd,
        cd: d.cd, r: d.r
    });
}

fn update_spawn() {
    if arena_dist(state.p.x, state.p.y) > state.ARR { return; }
    if state.foes.len >= state.FOE_CAP { return; }
    state.spawnt -= 1;
    if state.spawnt > 0 { return; }
    state.spawnt = state.SPAWN_MIN + floor(rnd::float() * state.SPAWN_VAR);
    let n = mn(
        state.WAVE_MIN + floor(rnd::float() * state.WAVE_VAR),
        state.FOE_CAP - state.foes.len
    );
    let i = 1;
    while i <= n {
        let a = rnd::float();
        let dist = 150.0 + rnd::float() * 60.0;
        let x = state.p.x + cos(a) * dist;
        let y = state.p.y + sin(a) * dist;
        let c = clamp_arena(x, y, 24.0);
        let idx = (1 + floor(rnd::float() * state.FOE_POOL.len)).to_int();
        spawn_foe(state.FOE_POOL[idx - 1], c[0], c[1]);
        i += 1;
    }
}

fn update_foes() {
    let i = state.foes.len;
    while i >= 1 {
        let f = state.foes[i - 1];
        let d = state.foes_def[f.kind];
        let ddx = state.p.x - f.x;
        let ddy = state.p.y - f.y;
        let adx = abs(ddx);
        let ady = abs(ddy);
        let a = atan2(ddx, ddy);
        if adx < 40.0 && ady < 40.0 {
            state.foes[i - 1].dx += cos(a + 0.25) * 0.05;
            state.foes[i - 1].dy += sin(a + 0.25) * 0.05;
        } else {
            state.foes[i - 1].dx += cos(a) * 0.06;
            state.foes[i - 1].dy += sin(a) * 0.06;
        }
        state.foes[i - 1].dx *= 0.97;
        state.foes[i - 1].dy *= 0.97;
        let sp = sqrt(state.foes[i - 1].dx * state.foes[i - 1].dx + state.foes[i - 1].dy * state.foes[i - 1].dy);
        if sp > d.spd {
            state.foes[i - 1].dx *= d.spd / sp;
            state.foes[i - 1].dy *= d.spd / sp;
        }
        state.foes[i - 1].x += state.foes[i - 1].dx;
        state.foes[i - 1].y += state.foes[i - 1].dy;
        if state.foes[i - 1].shd < state.foes[i - 1].mshd {
            state.foes[i - 1].shd = mn(state.foes[i - 1].mshd, state.foes[i - 1].shd + 0.05);
        }
        state.foes[i - 1].cd -= 1;
        if state.foes[i - 1].cd <= 0 && adx < d.range && ady < d.range {
            state.foes[i - 1].cd = d.cd;
            let n = d.n ?? 1;
            let k = 1;
            while k <= n {
                spawn_bullet(
                    state.foes[i - 1].x + cos(a) * 5.0,
                    state.foes[i - 1].y + sin(a) * 5.0,
                    a + (k - (n + 1) / 2.0) * 0.1,
                    d.bs, d.dmg, 1
                );
                k += 1;
            }
        }
        i -= 1;
    }
}

fn update_roids() {
    let i = 0;
    while i < state.roids.len {
        let r = state.roids[i];
        state.roids[i].x += r.dx;
        state.roids[i].y += r.dy;
        let dx = state.roids[i].x - state.ARX;
        let dy = state.roids[i].y - state.ARY;
        let d = abs(dist_trig(state.ARX, state.ARY, state.roids[i].x, state.roids[i].y));
        let lim = state.ARR - r.r;
        if d > lim {
            let nx = dx / (d + 0.0001);
            let ny = dy / (d + 0.0001);
            state.roids[i].x = state.ARX + nx * lim;
            state.roids[i].y = state.ARY + ny * lim;
            let dot = state.roids[i].dx * nx + state.roids[i].dy * ny;
            state.roids[i].dx = state.roids[i].dx - 2.0 * dot * nx;
            state.roids[i].dy = state.roids[i].dy - 2.0 * dot * ny;
        }
        i += 1;
    }
    // top the mineable rocks back up over time
    let solid = 0;
    for r in state.roids { if r.solid { solid += 1; } }
    if solid < state.ROID_CAP {
        state.roidt -= 1;
        if state.roidt <= 0 { state.roidt = state.ROID_RESPAWN; spawn_roid(); }
    }
}

fn update_drops() {
    let i = state.drops.len;
    while i >= 1 {
        let o = state.drops[i - 1];
        let ddx = state.p.x - o.x;
        let ddy = state.p.y - o.y;
        if abs(ddx) < 40.0 && abs(ddy) < 40.0 {
            let dd = sqrt(ddx * ddx + ddy * ddy) + 0.001;
            if dd < 34.0 && state.p.ore < state.p.mcargo {
                state.drops[i - 1].x += ddx / dd * 1.5;
                state.drops[i - 1].y += ddy / dd * 1.5;
            }
            if dd < 8.0 {
                if state.p.ore < state.p.mcargo {
                    state.p.ore += 1.0;
                    sfx(3, 3);
                    flt(o.x, o.y - 4.0, "+1 ore", 11);
                    state.drops.remove(i - 1);
                } else if state.msgt < 1 {
                    msg_set("CARGO FULL");
                }
            }
        }
        i -= 1;
    }
}

fn update_parts() {
    let i = state.parts.len;
    while i >= 1 {
        let q = state.parts[i - 1];
        state.parts[i - 1].x += q.dx;
        state.parts[i - 1].y += q.dy;
        state.parts[i - 1].t -= 1;
        if state.parts[i - 1].t <= 0 { state.parts.remove(i - 1); }
        i -= 1;
    }
}

fn flt(x, y, s, c) { state.floats.push(#{x: x, y: y, s: s, c: c, t: 50}); }

fn update_floats() {
    let i = state.floats.len;
    while i >= 1 {
        state.floats[i - 1].y -= 0.4;
        state.floats[i - 1].t -= 1;
        if state.floats[i - 1].t < 1 { state.floats.remove(i - 1); }
        i -= 1;
    }
}

fn update_fx() {
    update_parts();
    update_floats();
    let i = state.blasts.len;
    while i >= 1 {
        let b = state.blasts[i - 1];
        state.blasts[i - 1].t -= 1;
        state.blasts[i - 1].r += (b.mr - b.r) * 0.4;
        if state.blasts[i - 1].t <= 0 { state.blasts.remove(i - 1); }
        i -= 1;
    }
}

fn boom(x, y) {
    let i = 1;
    while i <= 12 {
        let a = rnd::float();
        let s = rnd::float() * 2.2;
        state.parts.push(#{
            x: x, y: y, dx: cos(a) * s, dy: sin(a) * s,
            t: 10 + floor(rnd::float() * 9.0), c: 8 + floor(rnd::float() * 3.0), s: 1.0
        });
        i += 1;
    }
    state.blasts.push(#{x: x, y: y, r: 0.0, mr: 11.0, t: 12, c: 10});
}

fn boom_big(x, y) {
    let i = 1;
    while i <= 44 {
        let a = rnd::float();
        let s = rnd::float() * 3.6;
        state.parts.push(#{
            x: x, y: y, dx: cos(a) * s, dy: sin(a) * s,
            t: 14 + floor(rnd::float() * 22.0), c: 8 + floor(rnd::float() * 5.0),
            s: 1.0 + rnd::float() * 2.0
        });
        i += 1;
    }
    state.blasts.push(#{x: x, y: y, r: 0.0, mr: 20.0, t: 16, c: 9});
    state.blasts.push(#{x: x, y: y, r: 0.0, mr: 36.0, t: 22, c: 8});
}

fn hit_foe(j, dmg) {
    if state.chapter == 2 { ch2_hit_foe(j, dmg); return; }
    let f = state.foes[j - 1];
    if f.shd > 0.0 {
        state.foes[j - 1].shd -= dmg;
        if state.foes[j - 1].shd < 0.0 {
            state.foes[j - 1].hp += state.foes[j - 1].shd;
            state.foes[j - 1].shd = 0.0;
        }
    } else {
        state.foes[j - 1].hp -= dmg;
    }
    if state.foes[j - 1].hp <= 0.0 {
        let bo = state.foes_def[f.kind].bounty;
        state.p.credits += bo;
        flt(f.x, f.y - 4.0, "+$" + bo.to_string(), 10);
        if rnd::float() < 0.3 { state.p.mis = mn(state.p.mmis, state.p.mis + 1.0); }
        boom(f.x, f.y);
        sfx(2, 2);
        state.foes.remove(j - 1);
    }
}

fn hit_roid(j, dmg) {
    let r = state.roids[j - 1];
    state.roids[j - 1].hp -= dmg;
    if state.roids[j - 1].hp <= 0.0 {
        state.drops.push(#{x: r.x, y: r.y, kind: 1});
        boom(r.x, r.y);
        state.roids.remove(j - 1);
    }
}

fn damage_player(d, why) {
    state.p.scd = 180;
    sfx(4, 2);
    state.shk = mx(state.shk, 4);
    if state.p.shd >= d {
        state.p.shd -= d;
    } else {
        let r = d - state.p.shd;
        state.p.shd = 0.0;
        state.p.hp -= r;
        if state.p.hp <= 0.0 {
            state.p.hp = 0.0;
            boom_big(state.p.x, state.p.y);
            sfx(2, 2);
            state.shk = 12;
            state.dth = why;
            state.got = 180;
            state.st = 6;
        }
    }
}

fn update_camera() {
    if state.shk > 0 { state.shk -= 1; }
    let jx = 0.0;
    let jy = 0.0;
    if state.shk > 0 {
        jx = (rnd::float() - 0.5) * state.shk;
        jy = (rnd::float() - 0.5) * state.shk;
    }
    state.camx = state.p.x - 64.0 + jx;
    state.camy = state.p.y - 64.0 + jy;
}

// ---------------------------------------------------------------------------
// world: system generation, camera draw
// ---------------------------------------------------------------------------

fn spawn_roid() {
    let a = rnd::float();
    let rr = rnd::float() * (state.ARR - 30.0);
    let x = state.ARX + cos(a) * rr;
    let y = state.ARY + sin(a) * rr;
    if hit2(x, y, state.p.x, state.p.y, 30.0) { return; }
    state.roids.push(#{
        x: x, y: y, dx: (rnd::float() - 0.5) * 0.4, dy: (rnd::float() - 0.5) * 0.4,
        r: 3.0 + rnd::float() * 4.0, hp: state.ROID_HP, c: 7, solid: true
    });
}

fn load_system() {
    system::seed(state.SEED.to_int());
    state.planets = [];
    state.stations = [];
    state.roids = [];
    state.foes = [];
    state.drops = [];
    state.bullets = [];
    state.missiles = [];
    state.parts = [];
    state.blasts = [];
    state.floats = [];
    ch_stations();
    state.active = state.stations[0];
    let i = 1;
    while i <= 4 + floor(rnd::float() * 3.0) {
        let a = rnd::float();
        let rr = rnd::float() * (state.ARR - 50.0);
        let x = state.ARX + cos(a) * rr;
        let y = state.ARY + sin(a) * rr;
        let clear = true;
        for s in state.stations {
            if abs(x - s.x) + abs(y - s.y) <= 60.0 { clear = false; }
        }
        if clear {
            state.planets.push(#{
                x: x, y: y, r: 10.0 + rnd::float() * 12.0,
                c: 1 + floor(rnd::float() * 15.0)
            });
        }
        i += 1;
    }
    i = 1;
    while i <= 36 {
        let solid = rnd::float() < 0.6;
        let c = 7;
        if !solid {
            if rnd::float() < 0.4 { c = 1; } else { c = 5; }
        }
        let a = rnd::float();
        let rr = rnd::float() * (state.ARR - 20.0);
        state.roids.push(#{
            x: state.ARX + cos(a) * rr, y: state.ARY + sin(a) * rr,
            dx: (rnd::float() - 0.5) * 0.4, dy: (rnd::float() - 0.5) * 0.4,
            r: 3.0 + rnd::float() * 4.0, hp: state.ROID_HP, c: c, solid: solid
        });
        i += 1;
    }
    ch_foes();
    state.p.x = state.ARX;
    state.p.y = state.ARY + 80.0;
    state.p.dx = 0.0;
    state.p.dy = 0.0;
    state.spawnt = state.SPAWN_MIN;
    state.roidt = state.ROID_RESPAWN;
    system::seed((state.SEED + 8000.0).to_int());
    ne_stars();
}

fn ne_stars() {
    state.stars = [];
    let cc = [1, 1, 1, 5, 5, 6, 7, 12, 13];
    let i = 1;
    while i <= 90 {
        state.stars.push(#{
            x: rnd::float() * 2048.0, y: rnd::float() * 2048.0,
            z: 1.0 + rnd::float() * 3.0,
            c: cc[floor(rnd::float() * cc.len).to_int()]
        });
        i += 1;
    }
}

fn draw_flight() {
    fade_screen();
    for s in state.stars {
        pset(
            (s.x - state.camx) / s.z % 128.0,
            (s.y - state.camy) / s.z % 128.0,
            s.c
        );
    }
    state.cam = true;
    for r in state.roids { if !r.solid { circfill(r.x, r.y, r.r, r.c); } }
    for pl in state.planets {
        circfill(pl.x, pl.y, pl.r, pl.c);
        circfill(pl.x - pl.r * 0.3, pl.y - pl.r * 0.3, pl.r * 0.55, pl.c + 1);
        circ(pl.x, pl.y, pl.r, 0);
    }
    for r in state.roids {
        if r.solid {
            circfill(r.x, r.y, r.r, r.c);
            circ(r.x, r.y, r.r, 1);
        }
    }
    for o in state.drops {
        let c = o.c ?? (if state.frame % 20 < 10 { 11 } else { 10 });
        rectfill(o.x - 2.0, o.y - 2.0, o.x + 2.0, o.y + 2.0, c);
    }
    draw_parts();
    draw_floats();
    for b in state.bullets {
        if b.own == 0 { pset(b.x, b.y, 7); } else { pset(b.x, b.y, 8); }
    }
    for m in state.missiles { circfill(m.x, m.y, 1, 11); }
    draw_foes();
    draw_stations();
    draw_player();
    state.cam = false;

    let st = nearest_station(20.0);
    if st != () && !(st.sliced ?? false) {
        pc(state.BX + (if st.kind == "gate" { " jump" } else { " " + (st.label ?? "dock") }), 105.0, 11);
    }
    let ad = arena_dist(state.p.x, state.p.y);
    if state.frame % 30 < 20 {
        if ad > state.ARR { pc("dead space area", 30.0, 8); }
        else if state.p.fuel <= 0.0 { pc("no fuel", 44.0, 8); }
    }
    draw_hud();
    draw_radar();
    draw_msg();
}

fn draw_floats() {
    for q in state.floats {
        pt(q.s, q.x - sw(q.s) * 0.5, q.y, if q.t < 20 && state.frame % 4 < 2 { 6 } else { q.c });
    }
}

fn draw_parts() {
    for q in state.parts {
        let c = q.c ?? (if q.t > 6 { 10 } else { 2 });
        pset(q.x, q.y, c);
        if (q.s ?? 1.0) > 1.0 && state.frame % 2 < 1 {
            pset(q.x + 1.0, q.y, c);
            pset(q.x, q.y + 1.0, c);
        }
    }
    for b in state.blasts { circ(b.x, b.y, b.r, b.c); }
}

fn draw_player() {
    let a = state.p.a;
    let k = ship_frame(a);
    spr(state.SPR_PLAYER + k, state.p.x - 4.0, state.p.y - 4.0);
    if abs(state.p.dx) + abs(state.p.dy) > 0.5 && state.frame % 4 < 2 {
        let b = a + 0.5;
        pset(state.p.x + cos(b) * 4.0, state.p.y + sin(b) * 4.0, 9);
    }
    if state.p.scd > 150 && state.frame % 6 < 3 { circ(state.p.x, state.p.y, 7.0, 8); }
    if state.p.chg >= state.CHGMAX { circ(state.p.x, state.p.y, 6.0, 10); }
}

fn draw_foes() {
    for f in state.foes {
        let a = atan2(state.p.x - f.x, state.p.y - f.y);
        spr(state.foes_def[f.kind].spr + ship_frame(a), f.x - 4.0, f.y - 4.0);
    }
}

// ---------------------------------------------------------------------------
// combat: weapons, projectiles, missiles, collision
// ---------------------------------------------------------------------------

fn fire_primary(power) {
    let dmg = 8.0 * power;
    let spd = 4.0 + (power - 1.0) * 0.5;
    state.p.wcd = 8;
    spawn_bullet(
        state.p.x + cos(state.p.a) * 6.0, state.p.y + sin(state.p.a) * 6.0,
        state.p.a, spd, dmg, 0
    );
    if power >= 3.0 {
        spawn_bullet(
            state.p.x + cos(state.p.a + 0.14) * 6.0,
            state.p.y + sin(state.p.a + 0.14) * 6.0,
            state.p.a + 0.08, spd, dmg / 2.0, 0
        );
        spawn_bullet(
            state.p.x + cos(state.p.a - 0.14) * 6.0,
            state.p.y + sin(state.p.a - 0.14) * 6.0,
            state.p.a - 0.08, spd, dmg / 2.0, 0
        );
    }
    sfx(0, 3);
}

fn fire_missile() {
    if state.p.mis <= 0.0 { return; }
    state.p.mis -= 1.0;
    state.missiles.push(#{x: state.p.x, y: state.p.y, a: state.p.a, s: 1.5, life: 300});
    sfx(1, 3);
}

fn spawn_bullet(x, y, a, s, d, own) {
    state.bullets.push(#{
        x: x, y: y, dx: cos(a) * s, dy: sin(a) * s, d: d, own: own, life: 120
    });
}

fn update_bullets() {
    let i = state.bullets.len;
    while i >= 1 {
        let b = state.bullets[i - 1];
        state.bullets[i - 1].x += b.dx;
        state.bullets[i - 1].y += b.dy;
        state.bullets[i - 1].life -= 1;
        if state.bullets[i - 1].life <= 0 { state.bullets.remove(i - 1); }
        i -= 1;
    }
}

fn update_missiles() {
    let i = state.missiles.len;
    while i >= 1 {
        let m = state.missiles[i - 1];
        let best = ();
        let bd = 200.0;
        let j = 1;
        while j <= state.foes.len {
            let f = state.foes[j - 1];
            let d = abs(f.x - m.x) + abs(f.y - m.y);
            if d < bd { bd = d; best = f; }
            j += 1;
        }
        if best != () {
            state.missiles[i - 1].a = ang_lerp(
                m.a, atan2(best.x - m.x, best.y - m.y), 0.1
            );
        }
        state.missiles[i - 1].s = mn(state.missiles[i - 1].s + 0.1, 3.5);
        state.missiles[i - 1].x += cos(state.missiles[i - 1].a) * state.missiles[i - 1].s;
        state.missiles[i - 1].y += sin(state.missiles[i - 1].a) * state.missiles[i - 1].s;
        state.missiles[i - 1].life -= 1;
        if state.missiles[i - 1].life <= 0 { state.missiles.remove(i - 1); }
        i -= 1;
    }
}

fn collide_all() {
    let i = state.bullets.len;
    while i >= 1 {
        let b = state.bullets[i - 1];
        if b.own == 0 {
            let hit = false;
            let j = 1;
            while j <= state.foes.len {
                let f = state.foes[j - 1];
                if hit2(b.x, b.y, f.x, f.y, f.r + 2.0) {
                    hit_foe(j, b.d);
                    hit = true;
                    break;
                }
                j += 1;
            }
            if !hit {
                let j = 1;
                while j <= state.roids.len {
                    let r = state.roids[j - 1];
                    if r.solid && hit2(b.x, b.y, r.x, r.y, r.r) {
                        hit_roid(j, b.d);
                        hit = true;
                        break;
                    }
                    j += 1;
                }
            }
            if hit { state.bullets.remove(i - 1); }
        } else {
            if hit2(b.x, b.y, state.p.x, state.p.y, 5.0) {
                damage_player(b.d, "KILLED BY PIRATES");
                state.bullets.remove(i - 1);
            }
        }
        i -= 1;
    }

    i = state.missiles.len;
    while i >= 1 {
        let m = state.missiles[i - 1];
        let hit = false;
        let j = 1;
        while j <= state.foes.len {
            let f = state.foes[j - 1];
            if hit2(m.x, m.y, f.x, f.y, f.r + 3.0) {
                hit_foe(j, 40.0);
                hit = true;
                break;
            }
            j += 1;
        }
        if !hit {
            let j = 1;
            while j <= state.roids.len {
                let r = state.roids[j - 1];
                if r.solid && hit2(m.x, m.y, r.x, r.y, r.r) {
                    hit_roid(j, 12.0);
                    hit = true;
                    break;
                }
                j += 1;
            }
        }
        if hit { state.missiles.remove(i - 1); }
        i -= 1;
    }

    let j = state.roids.len;
    while j >= 1 {
        let r = state.roids[j - 1];
        if r.solid && hit2(state.p.x, state.p.y, r.x, r.y, r.r + 4.0) {
            let v = sqrt(state.p.dx * state.p.dx + state.p.dy * state.p.dy);
            if v > 0.4 {
                damage_player(ceil(r.r * v * 1.5), "CRASHED INTO AN ASTEROID");
                boom(state.p.x + (r.x - state.p.x) * 0.5, state.p.y + (r.y - state.p.y) * 0.5);
                boom_big(state.p.x, state.p.y);
                sfx(2, 2);
                state.shk = 8;
                state.p.dx = (state.p.x - r.x) * 0.06;
                state.p.dy = (state.p.y - r.y) * 0.06;
                state.roids[j - 1].dx = -r.dx;
                state.roids[j - 1].dy = -r.dy;
            }
        }
        j -= 1;
    }

    let k = 1;
    while k <= state.foes.len {
        let f = state.foes[k - 1];
        if hit2(state.p.x, state.p.y, f.x, f.y, f.r + 4.0) && state.p.ramt <= 0 {
            damage_player(2.0, "RAMMED BY A PIRATE");
            state.p.ramt = 30;
            let a = atan2(state.p.x - f.x, state.p.y - f.y);
            state.p.dx += cos(a) * 1.2;
            state.p.dy += sin(a) * 1.2;
        }
        k += 1;
    }
}

// ---------------------------------------------------------------------------
// ui: text helpers, menus, HUD, radar, overlays
// ---------------------------------------------------------------------------

// PICO-8 centres with a 4px lowercase font in a 128px box; the console font is a
// uniform 8px, so every wrap width in this section is roughly halved to fit.
fn pc(s, y, c) { pt(s, mx(0.0, floor((128.0 - sw(s)) / 2.0)), y, c); }

fn menu_new(t, items) { #{title: t, i: 1, items: items, sub: ""} }

// The station menu. These take no map argument: Rhai passes by value, so a
// function handed `state.sm` would move a copy and the selection would never
// change. Actions report their status line through state.sub.
fn menu_move(d) {
    let n = clampi(state.sm.i + d, 1, state.sm.items.len);
    if n != state.sm.i { state.sm.i = n; sfx(5, 3); }
}

fn menu_tick() {
    if input::pressed(Button::Down) { menu_move(1); }
    if input::pressed(Button::Up) { menu_move(-1); }
    if input::pressed(Button::X) {
        let it = state.sm.items[state.sm.i - 1];
        if it.act != () {
            state.sub = "";
            it.act.call();
            state.sm.sub = state.sub;
        }
    }
}

fn menu_show(m) {
    pt(m.title, 12.0, 8.0, 11);
    let k = 1;
    while k <= m.items.len {
        let it = m.items[k - 1];
        let sel = k == m.i;
        pt(
            (if sel { state.CUR } else { " " }) + k.to_string() + "." + it.label,
            8.0, 26.0 + (k - 1) * 11.0, if sel { 7 } else { 11 }
        );
        k += 1;
    }
    if m.sub != "" {
        let ls = wrap(m.sub, 14);
        let i = 1;
        while i <= mn(2, ls.len) {
            pt(ls[i - 1].t, 8.0, 92.0 + (i - 1) * 8.0, 11);
            i += 1;
        }
    }
}

// the custom title font is not portable (SPEC §6.5), so this is the built-in
// font with a drop shadow - the closest the console allows
fn print_title(s, y, c) {
    let x = floor(64.0 - s.len * 4.0);
    pt(s, x, y + 2.0, 1);
    pt(s, x, y, c);
}

fn draw_title() {
    let i = 1;
    while i <= 60 {
        let x = (i * 53.0 + floor(state.frame * 0.2)) % 128.0;
        pset(x, (i * 37.0) % 128.0, if i % 5 == 0 { 7 } else { 5 });
        i += 1;
    }
    circfill(64.0, 78.0, 44.0, 1);
    circ(64.0, 78.0, 44.0, 6);
    circfill(50.0, 104.0, 12.0, 2);
    circfill(84.0, 60.0, 8.0, 2);
    print_title("void reckoning", 52.0, 7);
    if state.hassave {
        pc(state.BX + " continue    " + state.BO + " new game", 100.0, 11);
    } else if state.frame % 40 < 25 {
        pc(state.BX + " begin", 100.0, 11);
    }
    pc("audio: gruber & krajzeg", 120.0, 5);
}

fn open_menu() {
    state.mm = menu_new("", [
        #{label: "MISSIONS", lines: mission_lines(), ci: 1, act: ()},
        #{label: "INVENTORY", lines: inventory_lines(), ci: 1, act: ()},
        #{label: "EXIT", lines: ["save and quit to the title page."], ci: 1,
          act: || { save_game(); state.st = 0; }}
    ]);
    state.st = 8;
    sfx(5, 3);
}

fn update_menu() {
    let n = state.mm.items.len;
    if input::pressed(Button::Down) {
        let v = clampi(state.mm.i + 1, 1, n);
        if v != state.mm.i { state.mm.i = v; sfx(5, 3); }
    }
    if input::pressed(Button::Up) {
        let v = clampi(state.mm.i - 1, 1, n);
        if v != state.mm.i { state.mm.i = v; sfx(5, 3); }
    }
    let lines = state.mm.items[state.mm.i - 1].lines.len;
    let ci = state.mm.items[state.mm.i - 1].ci ?? 1;
    if lines > 1 && input::pressed(Button::Left) {
        state.mm.items[state.mm.i - 1].ci = (ci + lines - 2) % lines + 1;
    }
    if lines > 1 && input::pressed(Button::Right) {
        state.mm.items[state.mm.i - 1].ci = ci % lines + 1;
    }
    let act = state.mm.items[state.mm.i - 1].act;
    if input::pressed(Button::X) && act != () { act.call(); }
    if input::pressed(Button::Z) || input::pressed(Button::C) { state.st = 2; }
}

fn draw_menu() {
    rectfill(6.0, 6.0, 121.0, 121.0, 1);
    rect(6.0, 6.0, 121.0, 121.0, 11);
    let k = 1;
    while k <= state.mm.items.len {
        let it = state.mm.items[k - 1];
        let sel = k == state.mm.i;
        pt(
            (if sel { state.CUR } else { " " }) + k.to_string() + "." + it.label,
            10.0, 14.0 + (k - 1) * 10.0, if sel { 7 } else { 11 }
        );
        k += 1;
    }
    line(10.0, 42.0, 117.0, 42.0, 5);
    let it = state.mm.items[state.mm.i - 1];
    let ls = wrap(it.lines[it.ci - 1] ?? "", 14);
    let j = 1;
    while j <= ls.len {
        pc(ls[j - 1].t, 54.0 + (j - 1) * 9.0, 7);
        j += 1;
    }
    if it.lines.len > 1 {
        pt(state.ARRL, 8.0, 58.0, 11);
        pt(state.ARRR, 114.0, 58.0, 11);
    }
    pc(state.BX + " select  " + state.BO + " exit", 112.0, 11);
}

fn hbar(l, bx, by, w, v, mv, c) {
    pt(l, bx - 10.0, by - 1.0, 11);
    rectfill(bx, by, bx + w - 1.0, by + 3.0, 2);
    rectfill(bx, by, bx + floor(v / mv * (w - 1.0)), by + 3.0, c);
}

fn misicon(x, y, c) { rectfill(x, y, x + 1.0, y + 3.0, c); }

fn draw_hud() {
    rectfill(0.0, 0.0, 127.0, 13.0, 1);
    hbar("HP", 11.0, 1.0, 42.0, state.p.hp, state.p.mhp, 11);
    hbar("SH", 11.0, 7.0, 42.0, state.p.shd, state.p.mshd, 12);
    hbar("FU", 66.0, 1.0, 42.0, state.p.fuel, state.p.mfuel, 9);
    hbar("CG", 66.0, 7.0, 42.0, state.p.ore, state.p.mcargo, 10);
    let s = "$" + state.p.credits.to_int().to_string();
    pt(s, 127.0 - sw(s), 0.0, 10);
    let mc = if state.p.mis > 0.0 { 11 } else { 8 };
    misicon(111.0, 7.0, mc);
    pt(state.p.mis.to_int().to_string(), 116.0, 7.0, mc);
}

fn rplot(cx, cy, rr, sc, x, y, c) {
    let px = (x - state.ARX) * sc;
    let py = (y - state.ARY) * sc;
    let d = abs(dist_trig(0.0, 0.0, px, py));
    if d > rr - 1.0 {
        let k = (rr - 1.0) / (d + 0.0001);
        px *= k;
        py *= k;
    }
    pset(cx + px, cy + py, c);
}

fn draw_radar() {
    let cx = 106.0;
    let cy = 104.0;
    let rr = 16.0;
    circfill(cx, cy, rr, 1);
    circ(cx, cy, rr, 6);
    let sc = rr / state.ARR;
    for s in state.stations { rplot(cx, cy, rr, sc, s.x, s.y, 10); }
    for f in state.foes { rplot(cx, cy, rr, sc, f.x, f.y, 8); }
    for r in state.roids { if r.solid { rplot(cx, cy, rr, sc, r.x, r.y, 6); } }
    let pc2 = 11;
    if arena_dist(state.p.x, state.p.y) > state.ARR { pc2 = 8; }
    rplot(cx, cy, rr, sc, state.p.x, state.p.y, pc2);
    let tgt = objective();
    if tgt != () {
        let a = atan2(tgt.x - state.p.x, tgt.y - state.p.y);
        let tx = cx + cos(a) * (rr + 4.0);
        let ty = cy + sin(a) * (rr + 4.0);
        let bx = tx - cos(a) * 5.0;
        let by = ty - sin(a) * 5.0;
        line(tx, ty, bx - sin(a) * 3.0, by + cos(a) * 3.0, 10);
        line(tx, ty, bx + sin(a) * 3.0, by - cos(a) * 3.0, 10);
    }
}

fn draw_msg() {
    if state.msgt > 0 {
        rectfill(0.0, 114.0, 127.0, 127.0, 1);
        pc(state.msg, 117.0, 11);
    }
}

fn draw_gameover() {
    state.cam = true;
    draw_parts();
    state.cam = false;
    pc("GAME OVER", 46.0, 11);
    pc(state.dth, 60.0, 11);
    if state.got > 0 {
        pc("STANDBY...", 88.0, 11);
    } else if state.frame % 40 < 25 {
        pc("press " + state.BX + " to recover", 92.0, 11);
    }
}

fn update_jump() {
    state.jt += 1;
    if input::pressed(Button::Z) || input::pressed(Button::X) {
        if state.jt > state.jlines.len * state.JPER { jump_end(); }
        else { state.jt = state.jlines.len * state.JPER + 1; }
    }
}

fn draw_jump() {
    let i = 0;
    while i <= 19 {
        let a = i / 20.0;
        let r = 8.0 + ((i * 13.0 + state.frame * 16.0) % 104.0);
        line(
            64.0 + cos(a) * r, 64.0 + sin(a) * r,
            64.0 + cos(a) * (r + 9.0), 64.0 + sin(a) * (r + 9.0),
            if i % 4 == 0 { 7 } else { 12 }
        );
        i += 1;
    }
    let n = clampi(state.jt / state.JPER + 1, 0, state.jlines.len);
    let k = 1;
    while k <= n {
        let line_s = state.jlines[k - 1];
        pt(line_s, floor((128.0 - sw(line_s)) / 2.0), 12.0 + (k - 1) * 11.0, 7);
        k += 1;
    }
    if state.jt > state.jlines.len * state.JPER && state.frame % 40 < 25 {
        pc(state.BX + " continue", 118.0, 11);
    }
}

// ---------------------------------------------------------------------------
// dialogue: portraits and a typed-out scene runner
// ---------------------------------------------------------------------------

fn dlg_start(id) {
    state.dg = #{
        s: state.talks[id], i: 1, ch: 0, y: 128.0, w: 1,
        lines: (), nm: state.talkname[id] ?? ""
    };
}

fn update_dlg() {
    if state.dg == () { return; }
    let dg = state.dg;
    let st = at(dg.s, dg.i);
    if st == () {
        state.dg.y += 4.0;
        if state.dg.y >= 128.0 { state.dg = (); }
        return;
    }
    if state.dg.y > 80.0 { state.dg.y = mx(80.0, state.dg.y - 4.0); }
    if st.type_of() == "map" {
        if st.w != () { state.dg.w = st.w; }
        state.dg.i += 1;
        state.dg.ch = 0;
        state.dg.lines = ();
        return;
    }
    if state.dg.lines == () { state.dg.lines = wrap(st, 15); }
    let total = st.len;
    if input::pressed(Button::Z) || input::pressed(Button::X) {
        if state.dg.ch < total { state.dg.ch = total; }
        else { state.dg.i += 1; state.dg.ch = 0; state.dg.lines = (); }
        return;
    }
    if state.dg.ch < total {
        state.dg.ch += 2;
        if state.dg.ch > total { state.dg.ch = total; }
        if state.dg.ch % 8 < 2 { sfx(10, 2); }
    }
}

fn portrait(id, x, y) {
    let port = state.PORT[id - 1];
    sspr(port[0], port[1], 16, 16, x, y);
}

fn draw_dlg() {
    if state.dg == () { return; }
    let dg = state.dg;
    if dg.w != 0 {
        portrait(1, 12.0, 8.0);
        portrait(2, 84.0, 8.0);
        pt(state.PORTNAME[0], 12.0, 44.0, if dg.w == 1 { 7 } else { 5 });
        let n2 = if dg.nm != "" { dg.nm } else { state.PORTNAME[1] };
        pt(n2, 115.0 - sw(n2), 44.0, if dg.w == 2 { 7 } else { 5 });
    }
    let y = dg.y;
    rectfill(0.0, y, 127.0, 127.0, 1);
    rect(0.0, y, 127.0, 127.0, 5);
    if dg.lines != () {
        let k = 1;
        while k <= mn(dg.lines.len, 6) {
            let L = dg.lines[k - 1];
            let n = clampi(dg.ch - L.o + 1, 0, L.t.len);
            if n > 0 { pt(sub(L.t, 1, n), 8.0, y + 6.0 + (k - 1) * 8.0, 7); }
            k += 1;
        }
        let cur = at(dg.s, dg.i);
        if cur != () && dg.ch >= cur.len && state.frame % 30 < 18 {
            pt(state.MORE, 119.0, y + 30.0, 11);
        }
    }
}

fn next_spage() {
    state.spg += 1;
    state.spt = 0;
    if state.spg > state.spages.len { state.st = 2; }
}

// ---------------------------------------------------------------------------
// negotiation (engine; no chapter here uses it yet, kept for parity)
// ---------------------------------------------------------------------------

fn neg_start(id) {
    let s = state.negs[id];
    state.ng = #{s: s, i: s.start, k: 1, t: 0};
    state.st = 3;
    sfx(5, 3);
}

fn neg_go(i) {
    let n = state.ng.s.n[i];
    state.ng.i = i;
    state.ng.t = 0;
    state.ng.k = 1;
    if n.res != () { state.nres = n.res; }
    if n.act != () { n.act.call(); }
}

fn update_neg() {
    let n = state.ng.s.n[state.ng.i];
    state.ng.t += 1;
    if n.tm != () && state.ng.t > n.tm { neg_go(n.to); return; }
    if n.o == () {
        if input::pressed(Button::Z) || input::pressed(Button::X) {
            state.ng = ();
            state.st = if state.active != () { 4 } else { 2 };
        }
        return;
    }
    let o = n.o;
    if input::pressed(Button::Down) { state.ng.k = state.ng.k % o.len + 1; sfx(5, 2); }
    if input::pressed(Button::Up) { state.ng.k = (state.ng.k + o.len - 2) % o.len + 1; sfx(5, 2); }
    if input::pressed(Button::X) {
        let q = o[state.ng.k - 1];
        if q.c != () && state.p.credits < q.c { sfx(8, 3); return; }
        if q.c != () { state.p.credits -= q.c; }
        neg_go(q.to);
    }
}

fn draw_neg() {
    let n = state.ng.s.n[state.ng.i];
    rectfill(6.0, 6.0, 121.0, 121.0, 1);
    rect(6.0, 6.0, 121.0, 121.0, 11);
    sspr(state.PORT[0][0], state.PORT[0][1], 16, 16, 10.0, 10.0);
    let w = state.ng.s.who ?? 2;
    sspr(state.PORT[w - 1][0], state.PORT[w - 1][1], 16, 16, 84.0, 10.0);
    pt(state.PORTNAME[0], 10.0, 44.0, 5);
    let nm = state.ng.s.name ?? state.PORTNAME[w - 1];
    pt(nm, 115.0 - sw(nm), 44.0, 7);
    let ls = wrap(n.t, 14);
    let i = 1;
    while i <= mn(4, ls.len) {
        pt(ls[i - 1].t, 8.0, 54.0 + (i - 1) * 8.0, 7);
        i += 1;
    }
    if n.o == () {
        if state.frame % 40 < 25 { pc(state.BX + " continue", 116.0, 11); }
        return;
    }
    i = 1;
    while i <= n.o.len {
        let s = i == state.ng.k;
        pt(
            (if s { state.CUR } else { " " }) + n.o[i - 1].t,
            8.0, 92.0 + (i - 1) * 8.0, if s { 7 } else { 5 }
        );
        i += 1;
    }
}

fn leave_station() {
    state.st = 2;
    if state.ambush {
        state.ambush = false;
        if state.ch_ambush != () { state.ch_ambush.call(); }
    }
}

// ---------------------------------------------------------------------------
// chapter dispatch. The original overrides globals by defining them later; here
// both chapters live in one module, so each hook branches on state.chapter.
// ---------------------------------------------------------------------------

fn ch_stations() { if state.chapter == 1 { ch1_stations(); } else { ch2_stations(); } }
fn ch_foes() { if state.chapter == 1 { ch1_foes(); } else { ch2_foes(); } }
fn objective() { if state.chapter == 1 { ch1_objective(); } else { ch2_objective(); } }
fn draw_stations() { if state.chapter == 1 { ch1_draw_stations(); } else { ch2_draw_stations(); } }
fn mission_lines() { if state.chapter == 1 { ch1_mission_lines(); } else { ch2_mission_lines(); } }
fn inventory_lines() { if state.chapter == 1 { ch1_inventory_lines(); } else { ch2_inventory_lines(); } }
fn init_story() { if state.chapter == 1 { ch1_init_story(); } else { ch2_init_story(); } }
fn start_story() { if state.chapter == 1 { ch1_start_story(); } else { ch2_start_story(); } }
fn update_story() { if state.chapter == 1 { ch1_update_story(); } else { ch2_update_story(); } }
fn draw_story() { if state.chapter == 1 { ch1_draw_story(); } else { ch2_draw_story(); } }
fn start_jump() { if state.chapter == 1 { ch1_start_jump(); } else { ch2_start_jump(); } }
fn nearest_station(r) { if state.chapter == 1 { ch1_nearest_station(r); } else { ch2_nearest_station(r); } }
fn dock(s) { if state.chapter == 1 { ch1_dock(s); } else { ch2_dock(s); } }
fn update_station() { if state.chapter == 1 { ch1_update_station(); } else { ch2_update_station(); } }
fn draw_station() { if state.chapter == 1 { ch1_draw_station(); } else { ch2_draw_station(); } }

// jump_end chains the chapters inside this cart (the original loads the next cart)
fn jump_end() {
    if state.chapter == 1 { start_chapter(2); return; }
    if state.ch2_intro {
        state.ch2_intro = false;
        state.spg = 1;
        state.spt = 0;
        state.st = 1;
        sfx(7, 1);
        return;
    }
    reset_game();
}

fn start_chapter(n) {
    state.chapter = n;
    state.SEED = 1000.0 + state.chapter + (state.mem[6] ?? 0) * 7919.0;
    state.hassave = false;
    dset(8, 0);
    new_game();
}

// ---------------------------------------------------------------------------
// chapter 1 - mine ore, dock at the outpost, sell, buy a hyper-cell, jump out
// ---------------------------------------------------------------------------

fn ch1_setup() {
    state.foes_def = #{
        scout: #{hp: 25.0, shd: 0.0, spd: 1.0, r: 4.0, cd: 30, range: 90.0,
                 dmg: 5.0, bs: 2.2, bounty: 15.0, spr: state.SPR_SCOUT}
    };
    state.FOE_POOL = ["scout"];
    state.ECO = #{oresell: 10.0, cell: 250.0, repair: 2.0, fuel: 1.0, missile: 15.0};
}

fn ch1_stations() {
    state.stations.push(#{x: 150.0, y: 110.0, kind: "outpost"});
    state.stations.push(#{x: 400.0, y: 400.0, kind: "gate"});
}

fn ch1_foes() {
    let i = 1;
    while i <= 2 {
        spawn_foe("scout", 300.0 + rnd::float() * 80.0, 300.0 + rnd::float() * 80.0);
        i += 1;
    }
}

fn ch1_objective() { state.stations[if state.qs >= 3 { 1 } else { 0 }] }

fn ch1_draw_stations() {
    for s in state.stations {
        let x = s.x;
        let y = s.y;
        if s.kind == "outpost" {
            circ(x, y, 13.0, 1);
            circ(x, y, 12.0, 6);
            circ(x, y, 11.0, 6);
            circ(x, y, 4.0, 1);
            circfill(x, y, 2.0, 10);
            line(x - 11.0, y, x - 4.0, y, 6);
            line(x + 4.0, y, x + 11.0, y, 6);
            line(x, y - 11.0, x, y - 4.0, 6);
            line(x, y + 11.0, x, y + 4.0, 6);
            rectfill(x - 19.0, y - 2.0, x - 13.0, y + 2.0, 12);
            rectfill(x + 13.0, y - 2.0, x + 19.0, y + 2.0, 12);
            rectfill(x - 3.0, y - 17.0, x + 3.0, y - 12.0, 6);
            rectfill(x - 1.0, y - 16.0, x + 1.0, y - 13.0, 9);
            pset(x + 11.0, y - 7.0, if state.frame % 30 < 15 { 8 } else { 2 });
            pt("OUTPOST", x - 13.0, y - 26.0, 11);
        } else {
            rectfill(x - 3.0, y - 7.0, x + 2.0, y + 6.0, if state.frame % 40 < 20 { 12 } else { 13 });
            sspr(64.0, 16.0, 16.0, 16.0, x - 8.0, y - 8.0);
            pset(x, y - 8.0, if state.frame % 24 < 12 { 7 } else { 13 });
            pt("GATE", x - 9.0, y - 18.0, 11);
        }
    }
}

fn ch1_nearest_station(range) {
    for s in state.stations {
        if abs(s.x - state.p.x) < range && abs(s.y - state.p.y) < range { return s; }
    }
    ()
}

fn ch1_dock(s) {
    state.active = s;
    let eco = state.ECO;
    state.sm = menu_new("OUTPOST - SERVICES", [
        #{label: "REPAIR HULL (" + eco.repair.to_int().to_string() + "$/HP)",
          act: || {
            let need = ceil(state.p.mhp - state.p.hp);
            if need <= 0.0 { state.sub = "HULL AT MAXIMUM"; return; }
            let units = mn(need, floor(state.p.credits / eco.repair));
            if units <= 0.0 { state.sub = "NOT ENOUGH $"; return; }
            state.p.credits -= units * eco.repair;
            state.p.hp = mn(state.p.mhp, state.p.hp + units);
            state.sub = "+" + units.to_int().to_string() + " HP -"
                + (units * eco.repair).to_int().to_string() + "$";
          }},
        #{label: "REFUEL (" + eco.fuel.to_int().to_string() + "$/UNIT)",
          act: || {
            let need = ceil(state.p.mfuel - state.p.fuel);
            if need <= 0.0 { state.sub = "FUEL FULL"; return; }
            let units = mn(need, floor(state.p.credits / eco.fuel));
            if units <= 0.0 { state.sub = "NOT ENOUGH $"; return; }
            state.p.credits -= units * eco.fuel;
            state.p.fuel = mn(state.p.mfuel, state.p.fuel + units);
            state.sub = "+" + units.to_int().to_string() + " FUEL -"
                + (units * eco.fuel).to_int().to_string() + "$";
          }},
        #{label: "SELL ORE (" + eco.oresell.to_int().to_string() + "$ EACH)",
          act: || {
            if state.p.ore <= 0.0 { state.sub = "NO ORE TO SELL"; return; }
            let g = state.p.ore * eco.oresell;
            state.p.credits += g;
            state.p.ore = 0.0;
            if state.qs == 1 { state.qs = 2; }
            state.sub = "SOLD ORE +" + g.to_int().to_string() + "$";
          }},
        #{label: "BUY MISSILE (" + eco.missile.to_int().to_string() + "$)",
          act: || {
            if state.p.mis >= state.p.mmis { state.sub = "MISSILE BAY FULL"; return; }
            if state.p.credits < eco.missile { state.sub = "NOT ENOUGH $"; return; }
            state.p.credits -= eco.missile;
            state.p.mis += 1.0;
            state.sub = "MISSILE LOADED (" + state.p.mis.to_int().to_string() + "/"
                + state.p.mmis.to_int().to_string() + ")";
          }},
        #{label: "BUY HYPER-CELL (" + eco.cell.to_int().to_string() + "$)",
          act: || {
            if state.qs >= 3 { state.sub = "HYPER-CELL ALREADY FITTED"; return; }
            if state.p.credits >= eco.cell {
                state.p.credits -= eco.cell;
                state.qs = 3;
                state.sub = "HYPER-CELL FITTED. GATE UNLOCKED.";
            } else { state.sub = "NEED " + eco.cell.to_int().to_string() + "$"; }
          }}
    ]);
    sfx(6, 3);
    set_music(state.MUS_STATION);
    state.st = 4;
    if s.kind == "outpost" && !state.ohint {
        state.ohint = true;
        state.qs = 1;
        dlg_start("officer");
    }
    save_game();
}

fn ch1_update_station() {
    if state.dg != () { update_dlg(); return; }
    menu_tick();
    if input::pressed(Button::Z) { state.st = 2; }
}

fn ch1_draw_station() {
    rectfill(6.0, 6.0, 121.0, 121.0, 1);
    rect(6.0, 6.0, 121.0, 121.0, 11);
    if state.dg != () { draw_dlg(); return; }
    pt("$" + state.p.credits.to_int().to_string(), 10.0, 18.0, 10);
    pt("hull " + floor(state.p.hp).to_int().to_string() + "/" + state.p.mhp.to_int().to_string(),
        34.0, 18.0, 11);
    pt("fuel " + floor(state.p.fuel).to_int().to_string(), 84.0, 18.0, 11);
    menu_show(state.sm);
    pc(state.BX + " select  " + state.BO + " undock", 112.0, 11);
}

fn ch1_talks() {
    state.talks = #{
        intro: [
            #{w: 0},
            "IMPERIUM CENTRAL COMMAND",
            "COURIER 1393 - BRIEFING",
            "ASSET: SEALED",
            "DESTINATION: BR-11K0",
            #{w: 2},
            "your training campaign is over, pilot. you're ready for your first mission.",
            "eleven thousand applicants. we chose you.",
            "this is a top-secret delivery run. even i don't know what's in the container.",
            "and to be honest? i don't want to know.",
            #{w: 1},
            "...",
            #{w: 2},
            "deliver it at all costs. may the imperium protect you."
        ],
        officer: [
            #{w: 2},
            "so. an imperium courier, out here. in this system that uniform is a death sentence.",
            "your comms are dead and your hyper-cell is shot. you are not going anywhere without one.",
            "br-11k0? that chart's been scrubbed clean. nobody out here goes there.",
            "the white rocks carry ore. break them, sell it here, and i will fit you a spare cell.",
            "there's a word scratched on the wall in here. i can't read it. neither will you.",
            "then take the jump gate and get out. i never saw you. neither did anyone else.",
            #{w: 1},
            "understood."
        ]
    };
    state.talkname = #{officer: "outpost officer"};
    state.PORT = [[0.0, 16.0], [16.0, 16.0]];
    state.PORTNAME = ["pilot 1393", "handler rooke"];
}

fn ch1_mission_lines() {
    if !state.ohint { return ["no active orders. hail the outpost."]; }
    let m = if state.qs == 1 { "break the white rocks and sell ore at the outpost." }
        else if state.qs == 2 { "buy a hyper-cell at the outpost." }
        else { "fly to the jump gate and jump." };
    ["[main] " + m, "[side] destroy pirates for bounty.", "[side] mine and sell ore."]
}

fn ch1_inventory_lines() {
    ["$" + state.p.credits.to_int().to_string(),
     "ore " + state.p.ore.to_int().to_string() + " / " + state.p.mcargo.to_int().to_string(),
     "missiles " + state.p.mis.to_int().to_string() + " / " + state.p.mmis.to_int().to_string(),
     "cargo: sealed (br-11k0)",
     if state.qs >= 3 { "hyper-cell: fitted" } else { "hyper-cell: none" }]
}

fn ch1_init_story() {
    state.qs = 1;
    state.ohint = false;
    state.spg = 1;
    state.spt = 0;
    state.dg = ();
    state.spages = [
        #{kind: "dlg", id: "intro"},
        #{kind: "fly"},
        #{kind: "worm"},
        #{kind: "term", lines: [
            "> critical: wormhole collision",
            "> critical: navigation data corrupted",
            "> critical: hyper-cell damaged",
            "> nav chart: warning entry erased"]}
    ];
}

fn ch1_start_story() { state.spg = 1; state.spt = 0; }

fn ch1_update_story() {
    let pg = state.spages[state.spg - 1];
    state.spt += 1;
    if pg.kind == "dlg" {
        if state.spt == 1 { dlg_start(pg.id); }
        update_dlg();
        if state.dg == () { next_spage(); }
        return;
    }
    let ok = input::pressed(Button::Z) || input::pressed(Button::X);
    if pg.kind == "fly" {
        if state.spt > 112 || ok { next_spage(); }
    } else if pg.kind == "worm" {
        if state.spt > 150 || ok { next_spage(); }
    } else if pg.kind == "term" {
        let tl = 0;
        for l in pg.lines { tl += l.len; }
        if ok {
            if state.spt * 3 < tl { state.spt = ceil(tl / 3.0); } else { next_spage(); }
        }
    }
}

fn ch1_draw_story() {
    let pg = state.spages[state.spg - 1];
    if pg.kind == "fly" { ch1_draw_fly(); }
    else if pg.kind == "worm" { ch1_draw_wormhole(); }
    else if pg.kind == "term" { ch1_draw_term(pg); }
    else { draw_dlg(); }
}

fn ch1_draw_term(pg) {
    let n = state.spt * 3;
    let y = 30.0;
    let i = 1;
    while i <= pg.lines.len {
        let s = pg.lines[i - 1];
        let shown = sub(s, 1, clampi(n, 0, s.len));
        for k in wrap(shown, 15) {
            pt(k.t, 8.0, y, 11);
            y += 8.0;
        }
        n -= s.len;
        i += 1;
    }
    if state.frame % 30 < 15 { pt(state.CARET, 8.0, y, 11); }
}

fn ch1_draw_fly() {
    let t = state.spt;
    let i = 1;
    while i <= 24 {
        let y = (i * 37.0) % 128.0;
        let x = (i * 53.0 - t * 5.0) % 152.0 - 14.0;
        line(x, y, x + 4.0, y, if i % 4 == 0 { 7 } else { 5 });
        i += 1;
    }
    let wr = 5.0 + mn(t, 84) * 0.29;
    let r = 0;
    while r <= 2 {
        circ(100.0, 64.0, wr * (0.45 + r * 0.28), 1 + r);
        r += 1;
    }
    circfill(100.0, 64.0, wr * 0.34, 13);
    circfill(100.0, 64.0, wr * 0.16, 7);
    spr(state.SPR_PLAYER, 8.0 + mn(t, 102) * 0.9, 50.0);
    if t > 104 { circfill(100.0, 64.0, mn(140.0, (t - 104) * 18.0), 7); }
}

fn ch1_draw_wormhole() {
    let cx = 64.0;
    let cy = 58.0;
    let wc = [2, 12, 13];
    let i = 5;
    while i >= 1 {
        circ(cx, cy, i * 13.0, wc[floor(rnd::float() * wc.len).to_int()]);
        i -= 1;
    }
    let j = 0;
    while j <= 7 {
        let a = state.frame * 0.02 + j / 8.0;
        line(
            cx + cos(a) * 24.0, cy + sin(a) * 24.0,
            cx + cos(a) * 96.0, cy + sin(a) * 96.0,
            wc[floor(rnd::float() * wc.len).to_int()]
        );
        j += 1;
    }
    let sa = state.spt * 0.05;
    spr(state.SPR_PLAYER + ship_frame(sa), cx - 4.0, cy - 4.0);
    if state.spt > 120 { circfill(cx, cy, mn(60.0, (state.spt - 120) * 2.0), 7); }
}

fn ch1_start_jump() {
    if state.qs < 3 { msg_set("GATE LOCKED - FIT A HYPER-CELL"); return; }
    state.st = 5;
    state.jt = 0;
    state.jlines = [
        "the gate folds shut behind you.",
        "the system is gone.",
        "no patrol. no hunter.",
        "this is not your destination -",
        "only the far side of a doorway.",
        "the crate waits.",
        "the coordinates do not.",
        "(to be continued...)"
    ];
    sfx(7, 3);
}

// ---------------------------------------------------------------------------
// chapter 2 - XR-7PO: slice five sealed relics out of a stripped archive
// ---------------------------------------------------------------------------

fn ch2_setup() {
    state.foes_def = #{
        interceptor: #{hp: 30.0, shd: 0.0, spd: 1.15, r: 4.0, cd: 24, range: 105.0,
                      dmg: 5.0, bs: 3.0, spr: state.SPR_SCOUT},
        lancer: #{hp: 60.0, shd: 30.0, spd: 0.95, r: 5.0, cd: 44, range: 130.0,
                  dmg: 7.0, bs: 3.4, n: 3, spr: state.SPR_RAIDER}
    };
    state.RELIC_GOAL = 5.0;
    state.ROID_CAP = 0;
    state.WAVE_MIN = 0;
    state.WAVE_VAR = 1;
    state.SLICE_T = [3400, 2340, 1600, 1050, 600, 400];
    state.P_TERM = [48.0, 16.0];
}

fn ch2_stations() {
    state.stations.push(#{x: 392.0, y: 128.0, kind: "archive"});
    let cx = [120.0, 330.0, 420.0, 300.0, 90.0];
    let cy = [140.0, 90.0, 300.0, 420.0, 300.0];
    let i = 1;
    while i <= state.RELIC_GOAL {
        state.stations.push(#{
            x: cx[i - 1], y: cy[i - 1], kind: "container", idx: i, label: "slice"
        });
        i += 1;
    }
    state.stations.push(#{x: 136.0, y: 392.0, kind: "gate"});
}

fn ch2_foes() {
    let i = state.roids.len;
    while i >= 1 {
        if state.roids[i - 1].solid { state.roids.remove(i - 1); }
        i -= 1;
    }
}

fn ch2_objective() {
    if state.qs >= 4 { return state.stations[6]; }
    if state.qs >= 3 { return state.stations[0]; }
    let i = 2;
    while i <= 6 {
        if !(state.stations[i - 1].sliced ?? false) { return state.stations[i - 1]; }
        i += 1;
    }
    state.stations[0]
}

fn ch2_draw_stations() {
    for s in state.stations {
        let x = s.x;
        let y = s.y;
        if s.kind == "archive" {
            rectfill(x - 16.0, y - 9.0, x + 16.0, y + 9.0, 1);
            rect(x - 16.0, y - 9.0, x + 16.0, y + 9.0, 13);
            rect(x - 2.0, y - 4.0, x + 4.0, y + 4.0, 5);
            pset(x, y - 13.0, if state.frame % 40 < 20 { 8 } else { 2 });
            pt("ARCHIVE", x - 14.0, y - 18.0, 11);
        } else if s.kind == "container" {
            rect(x - 6.0, y - 5.0, x + 6.0, y + 5.0, if s.sliced ?? false { 6 } else { 13 });
            if !(s.sliced ?? false) {
                pset(x, y - 8.0, if state.frame % 20 < 10 {
                    if s.hot ?? false { 8 } else { 12 }
                } else { 1 });
            }
        } else {
            rectfill(x - 3.0, y - 7.0, x + 2.0, y + 6.0, if state.frame % 40 < 20 { 12 } else { 13 });
            sspr(64.0, 16.0, 16.0, 16.0, x - 8.0, y - 8.0);
            pset(x, y - 8.0, if state.frame % 24 < 12 { 7 } else { 13 });
            pt("GATE", x - 9.0, y - 18.0, 11);
        }
    }
}

fn ch2_nearest_station(range) {
    for s in state.stations {
        if abs(s.x - state.p.x) < range && abs(s.y - state.p.y) < range { return s; }
    }
    ()
}

fn ch2_talk(id, p) {
    state.PORT[1] = p;
    dlg_start(id);
}

fn ch2_dock(s) {
    if s.kind == "container" {
        if s.sliced ?? false { return; }
        start_slice(s);
        return;
    }
    state.active = s;
    state.st = 4;
    sfx(6, 3);
    save_game();
    if !state.ohint {
        state.ohint = true;
        ch2_talk("archive", state.P_TERM);
    } else if state.qs >= 3 && state.qs < 4 {
        start_slice(s);
    }
}

fn ch2_update_station() {
    if state.mg != () { update_mg(); return; }
    if state.dg != () {
        update_dlg();
        if state.dg == () && state.qs >= 3 && state.qs < 4 { start_slice(state.active); }
        return;
    }
    if input::pressed(Button::Z) { state.st = 2; }
}

fn ch2_draw_station() {
    if state.mg != () { draw_mg(); return; }
    rectfill(6.0, 6.0, 121.0, 121.0, 1);
    rect(6.0, 6.0, 121.0, 121.0, 11);
    if state.dg != () { draw_dlg(); return; }
    pt("the archive is silent.", 12.0, 26.0, 11);
    pc(state.BO + " undock", 112.0, 11);
}

// XR-7PO's death rule: relics ARE the progress, so keep them and charge nothing
fn ch2_recover() {
    state.p.hp = state.p.mhp;
    state.p.shd = state.p.mshd;
    state.p.fuel = state.p.mfuel;
    state.p.dx = 0.0;
    state.p.dy = 0.0;
    state.p.chg = 0.0;
    state.p.scd = 0;
    state.p.ramt = 0;
    state.p.x = state.active.x;
    state.p.y = state.active.y + 18.0;
    msg_set("RECOVERED - RELICS INTACT");
    save_game();
    state.st = 2;
}

fn ch2_talks() {
    state.talks = #{
        contact: [
            #{w: 0}, "INCOMING SIGNAL",
            #{w: 2}, "courier. surrender your hold.",
            #{w: 1}, "no.",
            #{w: 2}, "fine. you're gonna die."
        ],
        archive: [
            #{w: 0}, "XR-7PO ARCHIVE", "TERMINAL: STILL RUNNING",
            #{w: 2}, "the swarm sealed my relics in crates.",
            "slice them open and bring them here."
        ]
    };
    state.talkname = #{contact: "resistance pilot", archive: "archive terminal"};
    state.PORT = [[0.0, 16.0], [16.0, 16.0]];
    state.PORTNAME = ["pilot 1393", "handler rooke"];
}

fn ch2_mission_lines() {
    if state.qs < 3 {
        return ["[main] slice the relic containers ("
            + state.p.ore.to_int().to_string() + "/"
            + state.RELIC_GOAL.to_int().to_string() + ")."];
    }
    if state.qs == 3 { return ["[main] dock the archive and crack the code."]; }
    ["[main] fly to the gate and jump."]
}

fn ch2_inventory_lines() {
    ["relics " + state.p.ore.to_int().to_string() + " / " + state.RELIC_GOAL.to_int().to_string(),
     "missiles " + state.p.mis.to_int().to_string() + " / " + state.p.mmis.to_int().to_string(),
     "hull " + floor(state.p.hp).to_int().to_string() + " / " + state.p.mhp.to_int().to_string()]
}

fn ch2_init_story() {
    state.qs = 1;
    state.ohint = false;
    state.mg = ();
    state.ch2_intro = false;
    state.spg = 1;
    state.spt = 0;
    state.dg = ();
    state.p.credits = 0.0;
    state.p.mcargo = state.RELIC_GOAL;
    state.spages = [#{kind: "dlg", id: "contact"}];
}

fn ch2_start_story() {
    state.ch2_intro = true;
    state.jlines = [
        "the blockade is behind you.",
        "you fitted a cell and jumped.",
        "the wormhole cast you out here.",
        "xr-7po: a dead archive.",
        "the coordinates are inside it.",
        "you don't know how yet.",
        "survive. recover them. get out."
    ];
    state.jt = 0;
    state.st = 5;
}

fn ch2_update_story() {
    let pg = state.spages[state.spg - 1];
    state.spt += 1;
    if state.spt == 1 { ch2_talk(pg.id, [32.0, 16.0]); }
    update_dlg();
    if state.dg == () { next_spage(); }
}

fn ch2_draw_story() { draw_dlg(); }

fn ch2_start_jump() {
    if state.qs < 4 { msg_set("GATE LOCKED - NO COORDINATES"); return; }
    state.st = 5;
    state.jt = 0;
    state.jlines = [
        "the gate opens onto a road.",
        "the coordinates lead somewhere.",
        "xr-7po is dark behind you.",
        "the crate is still sealed.",
        "(to be continued...)"
    ];
    sfx(7, 3);
}

// --- the slicing minigame: find the 4-char run that appears twice -------------

fn gen_buf() {
    let syms = "#$%&'()*+,-./0123456789:;<=>?@ABCDEF";
    let b = "";
    let i = 1;
    while i <= 120 {
        b += syms.sub_string(floor(rnd::float() * 36.0).to_int(), 1);
        i += 1;
    }
    let c = 1 + floor(rnd::float() * 17.0);
    let rp = floor(rnd::float() * 5.0);
    let rq = rp + 1 + floor(rnd::float() * (5.0 - rp));
    let p = rp * 20 + c;
    let q = rq * 20 + c;
    let k = sub(b, p, p + 3);
    [sub(b, 1, p - 1) + k + sub(b, p + 4, q - 1) + k + sub(b, q + 4, 120), k]
}

fn start_slice(s) {
    let tm = state.SLICE_T[((s.idx ?? 6) - 1).to_int()];
    let bk = gen_buf();
    state.mg = #{s: s, buf: bk[0], key: bk[1], pos: 1, t: tm, tm: tm};
    state.st = 4;
    sfx(10, 2);
}

fn relocate(s) {
    let a = rnd::float();
    let c = clamp_arena(
        state.ARX + cos(a) * (120.0 + rnd::float() * 200.0),
        state.ARY + sin(a) * (120.0 + rnd::float() * 200.0),
        40.0
    );
    state.mg_s_x = c[0];
    state.mg_s_y = c[1];
}

fn fail_slice() {
    let s = state.mg.s;
    state.mg = ();
    if s.kind == "container" {
        let a = rnd::float();
        let c = clamp_arena(
            state.ARX + cos(a) * (120.0 + rnd::float() * 200.0),
            state.ARY + sin(a) * (120.0 + rnd::float() * 200.0),
            40.0
        );
        for i in 0..state.stations.len {
            if state.stations[i].kind == "container" && state.stations[i].idx == s.idx {
                state.stations[i].x = c[0];
                state.stations[i].y = c[1];
            }
        }
    }
    for i in 0..state.stations.len {
        if state.stations[i].kind == "container" && state.stations[i].idx == s.idx {
            state.stations[i].hot = true;
        }
    }
    msg_set("ALARM");
    sfx(7, 2);
    let n = mn(2.0 + (s.idx ?? 4).to_float(), 6.0);
    let i = 1;
    while i <= n {
        let a = rnd::float();
        let kind = if rnd::float() < (s.idx ?? 4).to_float() * 0.14 { "lancer" } else { "interceptor" };
        let c = clamp_arena(
            state.p.x + cos(a) * 100.0, state.p.y + sin(a) * 100.0, 24.0
        );
        spawn_foe(kind, c[0], c[1]);
        i += 1;
    }
    state.st = 2;
}

fn update_mg() {
    state.mg.t -= 1;
    if state.mg.t < 1 { fail_slice(); return; }
    let moves = [-1, 1, -20, 20];
    let i = 0;
    while i < 4 {
        let pressed = if i == 0 { input::pressed(Button::Left) }
            else if i == 1 { input::pressed(Button::Right) }
            else if i == 2 { input::pressed(Button::Up) }
            else { input::pressed(Button::Down) };
        if pressed {
            state.mg.pos = clampi(state.mg.pos + moves[i], 1, 117);
        }
        i += 1;
    }
    if input::pressed(Button::X) {
        if sub(state.mg.buf, state.mg.pos, state.mg.pos + 3) == state.mg.key {
            let s = state.mg.s;
            state.mg = ();
            if s.kind == "container" {
                for k in 0..state.stations.len {
                    if state.stations[k].kind == "container" && state.stations[k].idx == s.idx {
                        state.stations[k].sliced = true;
                    }
                }
                state.p.ore = mn(state.RELIC_GOAL, state.p.ore + 1.0);
                state.p.fuel = mn(state.p.mfuel, state.p.fuel + 20.0);
                flt(s.x, s.y - 10.0, "+relic", 12);
                if state.p.ore >= state.RELIC_GOAL { state.qs = 3; }
            } else {
                state.qs = 4;
                msg_set("COORDINATES RECOVERED");
            }
            sfx(3, 3);
            save_game();
            state.st = 2;
        } else {
            sfx(2, 1);
        }
    } else if input::pressed(Button::Z) {
        fail_slice();
    }
}

fn draw_mg() {
    pt(if state.mg.s.kind == "container" { "RELIC CONTAINER" } else { "ARCHIVE TERMINAL" },
        24.0, 8.0, 11);
    rect(14.0, 20.0, 113.0, 24.0, 5);
    rectfill(15.0, 21.0, 15.0 + floor(state.mg.t / state.mg.tm * 98.0), 23.0,
        if state.mg.t < 90 { 8 } else { 12 });
    let r = 0;
    while r <= 5 {
        pt(sub(state.mg.buf, 1 + r * 20, 20 + r * 20), 24.0, 32.0 + r * 8.0, 5);
        r += 1;
    }
    let x = 24.0 + (state.mg.pos - 1) % 20 * 4.0;
    let y = 32.0 + floor((state.mg.pos - 1) / 20.0) * 8.0;
    pt(sub(state.mg.buf, state.mg.pos, state.mg.pos + 3), x, y, 7);
    pc("ARROWS MOVE  X TAKE  Z LEAVE", 112.0, 11);
}

fn ch2_hit_foe(j, dmg) {
    let f = state.foes[j - 1];
    if f.shd > 0.0 {
        state.foes[j - 1].shd -= dmg;
        if state.foes[j - 1].shd < 0.0 {
            state.foes[j - 1].hp += state.foes[j - 1].shd;
            state.foes[j - 1].shd = 0.0;
        }
    } else {
        state.foes[j - 1].hp -= dmg;
    }
    if state.foes[j - 1].hp < 1.0 {
        boom(f.x, f.y);
        sfx(2, 2);
        let r = floor(rnd::float() * 3.0);
        if r < 1.0 {
            state.p.fuel = mn(state.p.mfuel, state.p.fuel + 30.0);
            flt(f.x, f.y - 6.0, "+fuel", 9);
        } else if r < 2.0 {
            state.p.mis = mn(state.p.mmis, state.p.mis + 2.0);
            flt(f.x, f.y - 6.0, "+missiles", 7);
        } else {
            state.p.hp = mn(state.p.mhp, state.p.hp + 25.0);
            flt(f.x, f.y - 6.0, "+hull", 7);
        }
        state.foes.remove(j - 1);
        if state.foes.len < 1 {
            for k in 0..state.stations.len {
                if state.stations[k].hot ?? false {
                    state.stations[k].hot = false;
                    msg_set("GUARDS DOWN");
                }
            }
        }
    }
}

// chapter content is installed by boot()/start_chapter(); these run once per game
fn install_chapter() {
    if state.chapter == 1 { ch1_setup(); ch1_talks(); }
    else { ch2_setup(); ch2_talks(); }
}
