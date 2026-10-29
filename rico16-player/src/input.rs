//! SPEC §14.3, §14.4 — physical key mapping and reserved keys.

use rico16_core::input::Button;
use winit::keyboard::KeyCode;

/// SPEC §14.3 — the default key map, matched on **physical** key position so
/// AZERTY and QWERTZ layouts keep the same arrangement.
pub fn button_for_key(code: KeyCode) -> Option<Button> {
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

/// SPEC §14.4 — a reserved key a cart must never see.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reserved {
    /// `Esc` opens the system menu.
    Menu,
    /// `F11` toggles fullscreen.
    Fullscreen,
    /// `Ctrl+R` resets the cart.
    Reset,
}

/// SPEC §14.4 — is this key reserved by the runtime?
pub fn reserved_for_key(code: KeyCode, ctrl: bool) -> Option<Reserved> {
    match code {
        KeyCode::Escape => Some(Reserved::Menu),
        KeyCode::F11 => Some(Reserved::Fullscreen),
        KeyCode::KeyR if ctrl => Some(Reserved::Reset),
        _ => None,
    }
}
