//! Window, input mapping and the fixed 60 Hz loop. SPEC §12.
//!
//! One window, one texture, one update per frame. The loop does not catch up
//! (§5.3): if a frame overruns, the next one is scheduled from *now*, and the
//! cart simply runs slower than real time.
//!
//! Reserved keys never reach a cart (§12.3): `Esc` closes, `F11` toggles
//! fullscreen, `Ctrl+R` restarts. Everything else is mapped by physical scan
//! code (§12.2), so QWERTZ and AZERTY keep the same physical arrangement.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rico16_core::input::Button;
use rico16_core::limits::{FRAME_TIME_NANOS, SCREEN_HEIGHT, SCREEN_WIDTH};
use rico16_script::Quit;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
use winit::window::{Fullscreen, Window, WindowId};

use crate::PlayerError;
use crate::cart::{CartRunner, CartRunnerError};
use crate::present::PixelsPresenter;

/// Everything that can stop [`run`] before the cart asks to quit.
///
/// Script errors display as `<file>:<line>:<col>: <message>` so the player can
/// print them verbatim (SPEC §12.6).
#[derive(Debug, thiserror::Error)]
pub enum RunError {
    /// The cart file could not be read.
    #[error("{}: {source}", path.display())]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },

    /// The cart could not be loaded, or failed while running.
    #[error(transparent)]
    Cart(#[from] CartRunnerError),

    /// The window or present path failed.
    #[error(transparent)]
    Player(#[from] PlayerError),
}

/// Run the cart at `path` in a window until it quits or the window closes.
///
/// `system::quit(message)` prints the message to stdout and exits successfully
/// (SPEC §12.7); a script error is returned so the caller can print it to
/// stderr and exit with code 1 (SPEC §12.6).
pub fn run(path: &Path) -> Result<(), RunError> {
    let source = std::fs::read_to_string(path).map_err(|source| RunError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let name = path.display().to_string();

    let event_loop = EventLoop::new().map_err(PlayerError::from)?;
    event_loop.set_control_flow(ControlFlow::WaitUntil(
        Instant::now() + Duration::from_nanos(FRAME_TIME_NANOS),
    ));

    let mut app = App::new(name, source);
    event_loop.run_app(&mut app).map_err(PlayerError::from)?;

    match app.failure {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

struct App {
    cart_name: String,
    source: String,
    runner: Option<CartRunner<PixelsPresenter>>,
    raw_mask: u8,
    modifiers: ModifiersState,
    window: Option<Arc<Window>>,
    next_frame: Instant,
    frame_due: bool,
    failure: Option<RunError>,
}

impl App {
    fn new(cart_name: String, source: String) -> Self {
        Self {
            cart_name,
            source,
            runner: None,
            raw_mask: 0,
            modifiers: ModifiersState::empty(),
            window: None,
            next_frame: Instant::now(),
            frame_due: true,
            failure: None,
        }
    }

    fn create_window(&mut self, event_loop: &ActiveEventLoop) -> Result<(), RunError> {
        let attributes = Window::default_attributes()
            .with_title("rico-16")
            .with_inner_size(LogicalSize::new(
                (SCREEN_WIDTH * 3) as f64,
                (SCREEN_HEIGHT * 3) as f64,
            ))
            .with_min_inner_size(LogicalSize::new(SCREEN_WIDTH as f64, SCREEN_HEIGHT as f64))
            .with_resizable(true);

        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .map_err(PlayerError::from)?,
        );
        let presenter = PixelsPresenter::new(window.clone(), window.inner_size())?;
        let runner = CartRunner::from_text(&self.cart_name, &self.source, presenter)?;

        self.window = Some(window);
        self.runner = Some(runner);
        self.next_frame = Instant::now();
        self.frame_due = true;

        if let Some(window) = &self.window {
            window.request_redraw();
        }
        Ok(())
    }

    /// Restart the cart from its source, re-running `on_init`. SPEC §12.3.
    fn restart(&mut self, event_loop: &ActiveEventLoop) {
        let result = match &mut self.runner {
            Some(runner) => runner.restart(&self.source),
            None => return,
        };

        if let Err(error) = result {
            self.failure = Some(RunError::Cart(error));
            event_loop.exit();
            return;
        }

        self.raw_mask = 0;
        self.next_frame = Instant::now();
        self.frame_due = true;
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn toggle_fullscreen(&self) {
        if let Some(window) = &self.window {
            if window.fullscreen().is_some() {
                window.set_fullscreen(None);
            } else {
                window.set_fullscreen(Some(Fullscreen::Borderless(None)));
            }
        }
    }

    fn key(&mut self, event_loop: &ActiveEventLoop, event: KeyEvent) {
        // A key held down repeats; a repeat is not a new press (§12.5).
        if event.repeat {
            return;
        }

        let PhysicalKey::Code(code) = event.physical_key else {
            return;
        };
        let pressed = event.state == ElementState::Pressed;

        // Reserved keys are handled before the cart map and never reach it.
        match code {
            KeyCode::Escape if pressed => {
                event_loop.exit();
                return;
            }
            KeyCode::F11 if pressed => {
                self.toggle_fullscreen();
                return;
            }
            KeyCode::KeyR if pressed && self.modifiers.contains(ModifiersState::CONTROL) => {
                self.restart(event_loop);
                return;
            }
            _ => {}
        }

        if let Some(button) = button_for(code) {
            if pressed {
                self.raw_mask |= button.bit();
            } else {
                self.raw_mask &= !button.bit();
            }
        }
    }

    /// One update, one draw, one present. SPEC §5.2.
    fn draw_frame(&mut self, event_loop: &ActiveEventLoop) {
        // A redraw can be requested for a resize; only a scheduled frame
        // advances the cart, so the update rate stays 60 Hz (§5.1).
        if !self.frame_due {
            return;
        }
        self.frame_due = false;

        match self.advance() {
            Ok(Some(quit)) => {
                if let Some(message) = quit.message {
                    println!("{message}");
                }
                event_loop.exit();
            }
            Ok(None) => {}
            Err(error) => {
                self.failure = Some(RunError::Cart(error));
                event_loop.exit();
            }
        }
    }

    /// Sample input, run one frame and present it.
    fn advance(&mut self) -> Result<Option<Quit>, CartRunnerError> {
        let Some(runner) = &mut self.runner else {
            return Ok(None);
        };
        runner.step(self.raw_mask)?;
        if runner.is_quitting() {
            Ok(runner.take_quit())
        } else {
            Ok(None)
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.runner.is_some() {
            return;
        }
        if let Err(error) = self.create_window(event_loop) {
            self.failure = Some(error);
            event_loop.exit();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                let resized = self
                    .runner
                    .as_mut()
                    .map(|runner| runner.resize(size.width, size.height));
                if let Some(Err(error)) = resized {
                    self.failure = Some(RunError::Player(error));
                    event_loop.exit();
                }
            }
            WindowEvent::Focused(false) => {
                // The window going away must not leave a cart walking (§12.5).
                self.raw_mask = 0;
                if let Some(runner) = &mut self.runner {
                    runner.clear_input();
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }
            WindowEvent::KeyboardInput { event, .. } => self.key(event_loop, event),
            WindowEvent::RedrawRequested => self.draw_frame(event_loop),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.runner.is_none() {
            return;
        }

        let now = Instant::now();
        if now >= self.next_frame {
            self.frame_due = true;
            if let Some(window) = &self.window {
                window.request_redraw();
            }

            self.next_frame += Duration::from_nanos(FRAME_TIME_NANOS);
            if self.next_frame <= now {
                // An overrun: drop the missed frames instead of burning the
                // next second catching up (§5.3).
                self.next_frame = now + Duration::from_nanos(FRAME_TIME_NANOS);
            }
        }

        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
    }
}

/// Logical button for a physical key position. SPEC §12.2.
fn button_for(code: KeyCode) -> Option<Button> {
    match code {
        KeyCode::ArrowLeft => Some(Button::Left),
        KeyCode::ArrowRight => Some(Button::Right),
        KeyCode::ArrowUp => Some(Button::Up),
        KeyCode::ArrowDown => Some(Button::Down),
        KeyCode::KeyZ => Some(Button::Z),
        KeyCode::KeyX => Some(Button::X),
        KeyCode::KeyC => Some(Button::C),
        KeyCode::KeyV => Some(Button::V),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_map_is_the_documented_one() {
        assert_eq!(button_for(KeyCode::ArrowLeft), Some(Button::Left));
        assert_eq!(button_for(KeyCode::ArrowRight), Some(Button::Right));
        assert_eq!(button_for(KeyCode::ArrowUp), Some(Button::Up));
        assert_eq!(button_for(KeyCode::ArrowDown), Some(Button::Down));
        assert_eq!(button_for(KeyCode::KeyZ), Some(Button::Z));
        assert_eq!(button_for(KeyCode::KeyX), Some(Button::X));
        assert_eq!(button_for(KeyCode::KeyC), Some(Button::C));
        assert_eq!(button_for(KeyCode::KeyV), Some(Button::V));
    }

    #[test]
    fn reserved_and_unrelated_keys_never_become_buttons() {
        for code in [
            KeyCode::Escape,
            KeyCode::F11,
            KeyCode::KeyR,
            KeyCode::Space,
            KeyCode::Enter,
            KeyCode::KeyA,
        ] {
            assert_eq!(button_for(code), None, "{code:?} must not reach a cart");
        }
    }
}
