//! The Rhai source written by `rico16 new`.
//!
//! Keep it runnable-but-boring: it should exercise the API surface a beginner
//! needs and nothing else.

pub const CODE: &str = r#"// rico-16 cart
// esc quits, f11 toggles fullscreen

fn on_init() {
    system::seed(1);
}

fn on_update(dt) {
    // dt is always 1/60; update state here
}

fn on_draw() {
    gfx::clear(0);
    gfx::text("HELLO, RICO-16", 8, 8, 5);
}
"#;
