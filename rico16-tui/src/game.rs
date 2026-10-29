//! SPEC §16.4 — the embedded game preview.
//!
//! A 320×240 indexed framebuffer cannot be shown faithfully in a terminal, so
//! the preview is a best-effort, downsampled, palette-quantized view drawn with
//! half-block characters. It is a convenience while editing; the native window
//! remains the display of record, and a cart must behave identically either way.

use std::path::Path;
use std::time::{Duration, Instant};

use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

use rico16_core::input::Button;
use rico16_core::limits::{DT, FRAMEBUFFER_LEN, SCREEN_HEIGHT, SCREEN_WIDTH};
use rico16_script::{Action, CartAssets, Runtime};

/// What happened during one preview tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tick {
    /// The cart is still running.
    Running,
    /// The cart asked to quit, optionally with a message.
    Quit(Option<String>),
    /// The cart raised a script error.
    Error(String),
}

/// SPEC §16.4 — a cart running inside the terminal.
pub struct EmbeddedGame {
    runtime: Runtime,
    rgba: Vec<u8>,
    mask: u8,
    last: Instant,
}

impl EmbeddedGame {
    /// Build and start the cart in `project`.
    pub fn start(project: &Path) -> Result<Self, String> {
        let assets = load_assets(project)?;
        let mut runtime = Runtime::new(assets).map_err(|e| e.to_string())?;
        runtime.init().map_err(|e| e.to_string())?;
        Ok(Self {
            runtime,
            rgba: vec![0; FRAMEBUFFER_LEN * 4],
            mask: 0,
            last: Instant::now(),
        })
    }

    /// SPEC §14.2 — advance one logical frame if `1/60` s has passed.
    pub fn tick(&mut self) -> Tick {
        if self.last.elapsed() < Duration::from_secs_f64(DT) {
            return Tick::Running;
        }
        self.last = Instant::now();
        self.step_frame()
    }

    /// Advance exactly one logical frame, ignoring the pacer. The host drives
    /// this directly when it wants a deterministic number of frames.
    pub fn step_frame(&mut self) -> Tick {
        if let Err(e) = self.runtime.step(self.mask) {
            return Tick::Error(e.to_string());
        }
        match self.runtime.take_action() {
            Action::Run => {}
            Action::Quit(message) => return Tick::Quit(message),
            Action::Reset => {
                if let Err(e) = self.runtime.reset() {
                    return Tick::Error(e.to_string());
                }
                self.mask = 0;
            }
        }
        self.runtime.present_into(&mut self.rgba);
        Tick::Running
    }

    /// Take the cart's `print`/`debug` lines.
    pub fn take_output(&mut self) -> Vec<String> {
        self.runtime.take_output()
    }

    /// SPEC §14.6 — set or clear one button.
    pub fn set_button(&mut self, button: Button, down: bool) {
        if down {
            self.mask |= button.bit();
        } else {
            self.mask &= !button.bit();
        }
    }

    /// SPEC §14.7 — forget all held buttons.
    pub fn clear_input(&mut self) {
        self.mask = 0;
    }

    /// The cart's current frame as RGBA8.
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}

/// SPEC §14.3 — terminal keys to logical buttons, by position.
pub fn button_for_key(code: KeyCode) -> Option<Button> {
    match code {
        KeyCode::Left => Some(Button::Left),
        KeyCode::Right => Some(Button::Right),
        KeyCode::Up => Some(Button::Up),
        KeyCode::Down => Some(Button::Down),
        KeyCode::Char('z') | KeyCode::Char('Z') => Some(Button::Z),
        KeyCode::Char('x') | KeyCode::Char('X') => Some(Button::X),
        KeyCode::Char('c') | KeyCode::Char('C') => Some(Button::C),
        KeyCode::Char('v') | KeyCode::Char('V') => Some(Button::V),
        _ => None,
    }
}

/// SPEC §16.4 — draw the cart's frame into `area` using half-block characters,
/// centred, with nearest-neighbour downsampling.
pub fn render_frame(rgba: &[u8], area: Rect, buf: &mut Buffer) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let avail_w = area.width as usize;
    let avail_h = area.height as usize * 2;
    let scale = (avail_w as f32 / SCREEN_WIDTH as f32).min(avail_h as f32 / SCREEN_HEIGHT as f32);
    if scale <= 0.0 {
        return;
    }

    let shown_w = ((SCREEN_WIDTH as f32 * scale).floor() as usize).clamp(1, avail_w);
    let shown_rows = (((SCREEN_HEIGHT as f32 * scale).floor() as usize).clamp(2, avail_h)) / 2;
    let offset_x = area.x + ((avail_w - shown_w) / 2) as u16;
    let offset_y = area.y + ((area.height as usize - shown_rows) / 2) as u16;

    for row in 0..shown_rows {
        for col in 0..shown_w {
            let top = sample(rgba, col, row * 2, scale);
            let bottom = sample(rgba, col, row * 2 + 1, scale);
            if let Some(cell) = buf.cell_mut((offset_x + col as u16, offset_y + row as u16)) {
                cell.set_char('\u{2580}'); // upper half block
                cell.set_fg(Color::Rgb(top.0, top.1, top.2));
                cell.set_bg(Color::Rgb(bottom.0, bottom.1, bottom.2));
            }
        }
    }
}

fn sample(rgba: &[u8], x: usize, y: usize, scale: f32) -> (u8, u8, u8) {
    let sx = ((x as f32 / scale) as usize).min(SCREEN_WIDTH - 1);
    let sy = ((y as f32 / scale) as usize).min(SCREEN_HEIGHT - 1);
    let at = (sy * SCREEN_WIDTH + sx) * 4;
    (rgba[at], rgba[at + 1], rgba[at + 2])
}

/// SPEC §16.3 — build a project into script assets.
///
/// The editor cannot reuse the player's loader (SPEC §16.4 keeps the crates
/// apart), so the conversion lives here.
fn load_assets(project: &Path) -> Result<CartAssets, String> {
    let manifest_path = project.join("cart.toml");
    let text = std::fs::read_to_string(&manifest_path).map_err(|e| e.to_string())?;
    let manifest = rico16_cart::CartToml::parse_str(&text).map_err(|e| e.to_string())?;
    let name = manifest
        .entry()
        .rsplit('/')
        .next()
        .unwrap_or("main.rhai")
        .to_string();

    let output = rico16_cart::build_project(project, &rico16_cart::BuildOptions::default())
        .map_err(|e| e.to_string())?;
    let cart = output.cart;
    Ok(CartAssets {
        name,
        code: cart.code,
        modules: cart
            .modules
            .into_iter()
            .map(|m| (m.path, m.source))
            .collect(),
        sheet: cart.sheet,
        map: cart.map,
        palette: cart.palette,
    })
}
