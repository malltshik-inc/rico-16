//! SPEC §14 — the player: window, input, frame loop and system menu.

use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
use winit::window::{Fullscreen, Window, WindowId};

use rico16_core::limits::{FRAMEBUFFER_LEN, SCREEN_HEIGHT, SCREEN_WIDTH};
use rico16_script::{Action, Runtime};

use crate::CartSource;
use crate::error::PlayerError;
use crate::input::{Reserved, button_for_key, reserved_for_key};
use crate::menu::{MenuAction, SystemMenu, composite};
use crate::pacing::FramePacer;
use crate::present::{PixelsPresenter, Presenter};

/// SPEC §3.7 — the default window size, 3×.
const DEFAULT_SCALE: u32 = 3;

/// SPEC §14.1 — build the cart, open a window and run it to completion.
pub fn run(source: CartSource) -> Result<(), PlayerError> {
    let assets = source.load()?;
    let title = assets.name.clone();
    let mut runtime = Runtime::new(assets)?;
    runtime.init()?;

    let mut player = Player::new(title, runtime);
    let event_loop = EventLoop::new().map_err(|e| PlayerError::Window(e.to_string()))?;
    event_loop
        .run_app(&mut player)
        .map_err(|e| PlayerError::Window(e.to_string()))?;
    Ok(())
}

struct Player {
    title: String,
    runtime: Runtime,
    window: Option<Arc<Window>>,
    presenter: Option<PixelsPresenter>,
    /// The cart's own frame, before any overlay.
    cart_frame: Vec<u8>,
    /// The frame handed to the presenter, with the system menu composited in.
    frame: Vec<u8>,
    overlay: Vec<u8>,
    mask: u8,
    menu: Option<SystemMenu>,
    pacer: FramePacer,
    started: Instant,
    modifiers: ModifiersState,
    fullscreen: bool,
}

impl Player {
    fn new(title: String, runtime: Runtime) -> Self {
        Self {
            title,
            runtime,
            window: None,
            presenter: None,
            cart_frame: vec![0; FRAMEBUFFER_LEN * 4],
            frame: vec![0; FRAMEBUFFER_LEN * 4],
            overlay: vec![0; FRAMEBUFFER_LEN * 4],
            mask: 0,
            menu: None,
            pacer: FramePacer::new(),
            started: Instant::now(),
            modifiers: ModifiersState::empty(),
            fullscreen: false,
        }
    }

    /// SPEC §5.2 — one logical frame: update, draw, then recompose.
    fn run_frame(&mut self) -> Result<(), PlayerError> {
        self.runtime.step(self.mask)?;

        match self.runtime.take_action() {
            Action::Run => {}
            Action::Quit(message) => {
                // SPEC §14.9 — a quit message is not an error.
                if let Some(message) = message {
                    println!("{message}");
                }
                std::process::exit(0);
            }
            Action::Reset => {
                self.runtime.reset()?;
                self.mask = 0;
            }
        }

        for line in self.runtime.take_output() {
            println!("{line}");
        }

        self.runtime.present_into(&mut self.cart_frame);
        self.compose();
        Ok(())
    }

    /// Recompose the presented frame from the cart frame plus any overlay.
    fn compose(&mut self) {
        self.frame.copy_from_slice(&self.cart_frame);
        if let Some(menu) = &self.menu {
            menu.render(&mut self.overlay);
            composite(&mut self.frame, &self.overlay);
        }
    }

    fn request_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn open_menu(&mut self) {
        // SPEC §14.5 — while the menu is open the cart is paused and its input
        // bitmask is frozen.
        self.menu = Some(SystemMenu::new());
        self.runtime.clear_input();
        self.mask = 0;
        self.compose();
        self.request_redraw();
    }

    fn close_menu(&mut self) {
        self.menu = None;
        self.compose();
        self.request_redraw();
    }

    fn toggle_fullscreen(&mut self) {
        let Some(window) = &self.window else {
            return;
        };
        self.fullscreen = !self.fullscreen;
        window.set_fullscreen(if self.fullscreen {
            Some(Fullscreen::Borderless(None))
        } else {
            None
        });
    }

    fn activate(&mut self, event_loop: &ActiveEventLoop, action: MenuAction) {
        match action {
            MenuAction::Resume => self.close_menu(),
            MenuAction::Restart => {
                self.close_menu();
                if let Err(e) = self.runtime.reset() {
                    self.fail(e);
                    event_loop.exit();
                }
                self.mask = 0;
            }
            MenuAction::Exit => event_loop.exit(),
        }
    }

    fn on_key(&mut self, event_loop: &ActiveEventLoop, event: KeyEvent) {
        let pressed = event.state == ElementState::Pressed;
        let PhysicalKey::Code(code) = event.physical_key else {
            return;
        };

        // SPEC §14.4 — reserved keys are never visible to a cart.
        if pressed {
            if let Some(reserved) = reserved_for_key(code, self.modifiers.control_key()) {
                match reserved {
                    Reserved::Menu => {
                        if self.menu.is_some() {
                            self.close_menu();
                        } else {
                            self.open_menu();
                        }
                    }
                    Reserved::Fullscreen => self.toggle_fullscreen(),
                    Reserved::Reset => {
                        if let Err(e) = self.runtime.reset() {
                            self.fail(e);
                            event_loop.exit();
                        }
                        self.mask = 0;
                    }
                }
                return;
            }
        }

        if let Some(menu) = &mut self.menu {
            if pressed {
                match code {
                    KeyCode::ArrowDown => menu.next(),
                    KeyCode::ArrowUp => menu.prev(),
                    KeyCode::Enter | KeyCode::Space => {
                        let action = menu.action();
                        self.activate(event_loop, action);
                        return;
                    }
                    _ => {}
                }
                self.compose();
                self.request_redraw();
            }
            return;
        }

        // SPEC §14.7 — key repeat is not a new press.
        if event.repeat {
            return;
        }
        if let Some(button) = button_for_key(code) {
            if pressed {
                self.mask |= button.bit();
            } else {
                self.mask &= !button.bit();
            }
        }
    }

    /// SPEC §14.8 — player mode: report and exit 1.
    fn fail(&self, error: impl std::fmt::Display) {
        eprintln!("{error}");
    }
}

impl ApplicationHandler for Player {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title(self.title.clone())
            .with_inner_size(winit::dpi::LogicalSize::new(
                SCREEN_WIDTH as u32 * DEFAULT_SCALE,
                SCREEN_HEIGHT as u32 * DEFAULT_SCALE,
            ));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(e) => {
                eprintln!("cannot open a window: {e}");
                event_loop.exit();
                return;
            }
        };
        let size = window.inner_size();
        match PixelsPresenter::new(window.clone(), size.width, size.height) {
            Ok(presenter) => self.presenter = Some(presenter),
            Err(e) => {
                eprintln!("{e}");
                event_loop.exit();
                return;
            }
        }
        self.window = Some(window);
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
                if let Some(presenter) = &mut self.presenter {
                    if let Err(e) = presenter.resize(size.width, size.height) {
                        eprintln!("{e}");
                    }
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }
            WindowEvent::KeyboardInput { event, .. } => self.on_key(event_loop, event),
            WindowEvent::Focused(false) => {
                // SPEC §14.7 — focus loss clears all held buttons.
                self.runtime.clear_input();
                self.mask = 0;
            }
            WindowEvent::RedrawRequested => {
                if let Some(presenter) = &mut self.presenter {
                    if let Err(e) = presenter.present(&self.frame) {
                        eprintln!("{e}");
                        event_loop.exit();
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // SPEC §14.5 — logical time does not advance while the menu is open.
        if self.menu.is_some() {
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        }

        let now = self.started.elapsed();
        if self.pacer.ready(now) {
            self.pacer.advance(now);
            if let Err(e) = self.run_frame() {
                self.fail(e);
                event_loop.exit();
                return;
            }
            self.request_redraw();
        }

        let remaining = self.pacer.deadline().saturating_sub(self.started.elapsed());
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + remaining));
    }
}
