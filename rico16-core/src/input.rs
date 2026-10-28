//! Button state and per-frame edges. SPEC §12.2, §12.4.
//!
//! The player collects raw key state; once per frame it calls [`Input::update`]
//! with a bitmask. `held`, `pressed` and `released` are then read by the script
//! layer. Sampling in one place is what makes edge detection independent of how
//! many key events the OS delivered that frame.

/// The eight buttons a cart can see. SPEC §9.3.
///
/// Buttons are logical. Physical keys are the player's business ([§12.2](../docs/SPEC.md)),
/// so a remap changes nothing in cart code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Button {
    Left,
    Right,
    Up,
    Down,
    Z,
    X,
    C,
    V,
}

impl Button {
    /// Every button, in documentation order.
    pub const ALL: [Button; 8] = [
        Button::Left,
        Button::Right,
        Button::Up,
        Button::Down,
        Button::Z,
        Button::X,
        Button::C,
        Button::V,
    ];

    /// The bit this button occupies in a raw mask. SPEC §12.4.
    pub const fn bit(self) -> u8 {
        1 << self as u8
    }

    /// Position in [`Button::ALL`], useful for tables.
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// Held state for the current and previous frame. SPEC §12.4.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Input {
    current: u8,
    previous: u8,
}

impl Input {
    /// No buttons held.
    pub const fn new() -> Self {
        Self {
            current: 0,
            previous: 0,
        }
    }

    /// Sample the frame. Called once per frame, immediately before `on_update`.
    ///
    /// The previous frame becomes history, so `pressed` and `released` describe
    /// exactly the transition between two samples. Key-repeat events never
    /// reach this function, which is why they cannot fake a press (§12.5).
    pub fn update(&mut self, mask: u8) {
        self.previous = self.current;
        self.current = mask;
    }

    /// True while the button is down.
    pub fn held(self, button: Button) -> bool {
        self.current & button.bit() != 0
    }

    /// True only on the frame the button went down.
    pub fn pressed(self, button: Button) -> bool {
        self.current & !self.previous & button.bit() != 0
    }

    /// True only on the frame the button went up.
    pub fn released(self, button: Button) -> bool {
        !self.current & self.previous & button.bit() != 0
    }

    /// The raw mask sampled this frame.
    pub const fn mask(self) -> u8 {
        self.current
    }

    /// Forget everything held, without reporting releases.
    ///
    /// Used on focus loss (§12.5): a cart must not keep walking because the
    /// window went away. Releases are suppressed on purpose — losing focus is
    /// not the player lifting a finger, and firing eight `released` events
    /// would make a cart act on something that did not happen.
    pub fn clear(&mut self) {
        self.current = 0;
        self.previous = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges_are_true_only_on_the_transition_frame() {
        let mut input = Input::new();
        input.update(Button::Z.bit());
        assert!(input.held(Button::Z));
        assert!(input.pressed(Button::Z));
        assert!(!input.released(Button::Z));

        input.update(Button::Z.bit());
        assert!(input.held(Button::Z));
        assert!(!input.pressed(Button::Z));
        assert!(!input.released(Button::Z));

        input.update(0);
        assert!(!input.held(Button::Z));
        assert!(!input.pressed(Button::Z));
        assert!(input.released(Button::Z));

        input.update(0);
        assert!(!input.released(Button::Z));
    }

    #[test]
    fn buttons_do_not_interfere() {
        let mut input = Input::new();
        input.update(Button::Left.bit() | Button::V.bit());
        assert!(input.held(Button::Left));
        assert!(input.held(Button::V));
        assert!(!input.held(Button::Right));
        assert_eq!(input.mask(), Button::Left.bit() | Button::V.bit());

        input.update(Button::Right.bit());
        assert!(input.released(Button::Left));
        assert!(input.pressed(Button::Right));
        assert!(input.released(Button::V));
    }

    #[test]
    fn clear_drops_held_state_without_firing_releases() {
        let mut input = Input::new();
        input.update(Button::Down.bit() | Button::C.bit());
        input.clear();

        assert!(!input.held(Button::Down));
        assert!(!input.held(Button::C));
        assert!(!input.released(Button::Down));
        assert!(!input.pressed(Button::Down));
        assert_eq!(input.mask(), 0);
    }

    #[test]
    fn every_button_has_its_own_bit() {
        let mut mask = 0u8;
        for button in Button::ALL {
            assert_eq!(mask & button.bit(), 0, "{button:?} shares a bit");
            mask |= button.bit();
        }
        assert_eq!(mask, 0xff);
        assert_eq!(Button::Left.index(), 0);
        assert_eq!(Button::V.index(), 7);
    }
}
