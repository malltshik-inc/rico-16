//! SPEC §16 — the pure parts of the editor: file tree, highlighting, buffer.

use std::path::PathBuf;

use rico16_tui::buffer::TextBuffer;
use rico16_tui::highlight::{Token, highlight};
use rico16_tui::tree::project_files;

struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("rico16-tui-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src/scenes")).unwrap();
        std::fs::create_dir_all(root.join("assets")).unwrap();
        std::fs::write(root.join("cart.toml"), "format = 1\n").unwrap();
        std::fs::write(root.join("src/main.rhai"), "fn on_init() {}\n").unwrap();
        std::fs::write(root.join("src/scenes/play.rhai"), "fn update(dt) {}\n").unwrap();
        std::fs::write(root.join("assets/hero.png"), b"x").unwrap();
        Self { root }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// SPEC §16.2 — the tree lists cart.toml, src/ and assets/, deterministically.
#[test]
fn tree_lists_the_project() {
    let scratch = Scratch::new("tree");
    let files = project_files(&scratch.root);
    let names: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "cart.toml",
            "src",
            "main.rhai",
            "scenes",
            "play.rhai",
            "assets",
            "hero.png",
        ]
    );
    assert!(!files[0].is_dir);
    assert!(files[1].is_dir, "src is a directory");
    assert_eq!(files[2].depth, 1, "main.rhai is one level deep");
    assert_eq!(files[3].depth, 1);
    assert_eq!(files[4].depth, 2, "play.rhai is nested");
}

/// SPEC §16.2 — highlighting classifies the interesting tokens.
#[test]
fn highlighting() {
    let spans = highlight("fn on_init() { let x = 42; } // note");
    let classes: Vec<Token> = spans.iter().map(|(t, _)| *t).collect();
    assert_eq!(classes[0], Token::Keyword, "fn");
    assert_eq!(spans[0].1, "fn");
    assert!(
        spans
            .iter()
            .any(|(t, s)| *t == Token::Keyword && *s == "let")
    );
    assert!(spans.iter().any(|(t, s)| *t == Token::Number && *s == "42"));
    assert!(
        spans
            .iter()
            .any(|(t, s)| *t == Token::Comment && *s == "// note")
    );
    assert!(
        spans
            .iter()
            .any(|(t, s)| *t == Token::Ident && *s == "on_init")
    );
}

/// SPEC §16.2 — string literals and literals are classified.
#[test]
fn highlighting_strings() {
    let spans = highlight(r#"let s = "hello // not a comment"; let b = true;"#);
    assert!(
        spans
            .iter()
            .any(|(t, s)| *t == Token::String && *s == "\"hello // not a comment\"")
    );
    assert!(
        spans
            .iter()
            .any(|(t, s)| *t == Token::Literal && *s == "true")
    );
}

/// SPEC §16.2 — every character of the line is accounted for.
#[test]
fn highlighting_is_lossless() {
    let line = "fn draw() { gfx::clear(1); }";
    let joined: String = highlight(line).into_iter().map(|(_, s)| s).collect();
    assert_eq!(joined, line);
}

/// SPEC §16.2 — the buffer edits like a text editor.
#[test]
fn buffer_editing() {
    let mut buffer = TextBuffer::from_text("let x = 1\n", None);
    assert_eq!(
        buffer.line_count(),
        2,
        "a trailing newline is an empty line"
    );
    assert_eq!(buffer.line(0), "let x = 1");

    buffer.move_end();
    buffer.insert_char(';');
    assert_eq!(buffer.line(0), "let x = 1;");

    buffer.insert_newline();
    buffer.insert_char('y');
    assert_eq!(buffer.line(1), "y");
    assert_eq!(buffer.line_count(), 3);

    buffer.backspace();
    buffer.backspace();
    assert_eq!(buffer.line(1), "");
    assert_eq!(buffer.line_count(), 2);

    buffer.move_up();
    buffer.move_home();
    buffer.delete();
    assert_eq!(buffer.line(0), "et x = 1;");
    assert!(buffer.is_dirty());
}

/// SPEC §16.5 — saving writes the file and clears the dirty flag.
#[test]
fn buffer_save() {
    let scratch = Scratch::new("save");
    let path = scratch.root.join("src/main.rhai");
    let mut buffer = TextBuffer::open(&path).unwrap();
    assert!(!buffer.is_dirty());

    buffer.move_end();
    buffer.insert_char(' ');
    assert!(buffer.is_dirty());
    buffer.save().unwrap();
    assert!(!buffer.is_dirty());
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "fn on_init() {} \n"
    );
}

/// SPEC §16.5 — a buffer with unsaved edits keeps them until saved.
#[test]
fn buffer_keeps_edits() {
    let mut buffer = TextBuffer::from_text("a", None);
    buffer.insert_char('b');
    assert_eq!(buffer.to_text(), "ba");
    assert_eq!(buffer.line(0), "ba");
}

/// SPEC §16.4 — the footer advertises how to run and how to get help.
#[test]
fn footer_advertises_the_keys() {
    let keys: Vec<&str> = rico16_tui::HOTKEYS.iter().map(|(k, _)| *k).collect();
    for key in ["F1", "F5", "F6", "F7", "F8", "Tab"] {
        assert!(keys.contains(&key), "the footer must mention {key}");
    }
    let help: Vec<&str> = rico16_tui::HELP.iter().map(|(k, _)| *k).collect();
    assert!(
        help.iter().any(|k| k.contains("F1")),
        "the help must say how to open it"
    );
}

/// SPEC §16.4 — terminal keys map to the cart's buttons.
#[test]
fn embedded_key_map() {
    use crossterm::event::KeyCode;
    use rico16_core::input::Button;
    use rico16_tui::game::button_for_key;

    assert_eq!(button_for_key(KeyCode::Left), Some(Button::Left));
    assert_eq!(button_for_key(KeyCode::Right), Some(Button::Right));
    assert_eq!(button_for_key(KeyCode::Up), Some(Button::Up));
    assert_eq!(button_for_key(KeyCode::Down), Some(Button::Down));
    assert_eq!(button_for_key(KeyCode::Char('z')), Some(Button::Z));
    assert_eq!(button_for_key(KeyCode::Char('X')), Some(Button::X));
    assert_eq!(button_for_key(KeyCode::Char('c')), Some(Button::C));
    assert_eq!(button_for_key(KeyCode::Char('v')), Some(Button::V));
    assert_eq!(button_for_key(KeyCode::Char('a')), None);
}

/// SPEC §16.4 — the preview draws one cell per two source pixels, upper half
/// in the foreground and lower half in the background.
#[test]
fn embedded_renderer_uses_half_blocks() {
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::Color;
    use rico16_core::limits::{FRAMEBUFFER_LEN, SCREEN_HEIGHT, SCREEN_WIDTH};
    use rico16_tui::game::render_frame;

    let mut rgba = vec![0u8; FRAMEBUFFER_LEN * 4];
    rgba[0..4].copy_from_slice(&[255, 0, 0, 255]); // pixel (0, 0)
    let second = (SCREEN_WIDTH) * 4; // pixel (0, 1)
    rgba[second..second + 4].copy_from_slice(&[0, 0, 255, 255]);

    // A 320x120 cell area is exactly 1:1 with the 320x240 framebuffer.
    let area = Rect::new(0, 0, SCREEN_WIDTH as u16, (SCREEN_HEIGHT / 2) as u16);
    let mut buf = Buffer::empty(area);
    render_frame(&rgba, area, &mut buf);

    let cell = buf.cell((0, 0)).unwrap();
    assert_eq!(cell.symbol(), "\u{2580}");
    assert_eq!(cell.fg, Color::Rgb(255, 0, 0));
    assert_eq!(cell.bg, Color::Rgb(0, 0, 255));
}

/// SPEC §16.4 — the embedded preview runs real carts, headless.
#[test]
fn embedded_game_runs_examples() {
    use rico16_tui::game::{EmbeddedGame, Tick};
    use std::path::Path;

    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples");
    let mut found = 0;
    for entry in std::fs::read_dir(&examples).expect("examples directory") {
        let dir = entry.unwrap().path();
        if !dir.join("cart.toml").is_file() {
            continue;
        }
        found += 1;
        let mut game =
            EmbeddedGame::start(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        for frame in 0..180 {
            match game.step_frame() {
                Tick::Running => {}
                Tick::Quit(_) => break,
                Tick::Error(e) => panic!("{} frame {frame}: {e}", dir.display()),
            }
        }
        assert!(
            game.rgba().iter().any(|&b| b != 0),
            "{} drew nothing",
            dir.display()
        );
    }
    assert!(
        found >= 2,
        "expected at least two example carts, found {found}"
    );
}
