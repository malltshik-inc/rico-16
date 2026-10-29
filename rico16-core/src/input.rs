//! SPEC §9.3, §14.6, §14.7 — the eight logical buttons and their edge state.

/// SPEC §9.3 — the `Button` values, matched on physical key position (SPEC §14.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Button {
    /// D-pad left.
    Left,
    /// D-pad right.
    Right,
    /// D-pad up.
    Up,
    /// D-pad down.
    Down,
    /// The `Z` position.
    Z,
    /// The `X` position.
    X,
    /// The `C` position.
    C,
    /// The `V` position.
    V,
}

impl Button {
    /// Every button, in bit order.
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

    /// The bit this button occupies in the input mask.
    pub const fn bit(self) -> u8 {
        1 << (self as u8)
    }

    /// The button for a bit index 0–7.
    pub const fn from_index(index: u8) -> Option<Button> {
        match index {
            0 => Some(Button::Left),
            1 => Some(Button::Right),
            2 => Some(Button::Up),
            3 => Some(Button::Down),
            4 => Some(Button::Z),
            5 => Some(Button::X),
            6 => Some(Button::C),
            7 => Some(Button::V),
            _ => None,
        }
    }
}

/// SPEC §14.6 — input sampled once per frame. Edge state compares this frame's
/// bitmask with the previous frame's; the first frame's previous mask is zero.
#[derive(Debug, Clone, Default)]
pub struct Input {
    current: u8,
    previous: u8,
}

impl Input {
    /// A fresh input with no buttons held.
    pub fn new() -> Self {
        Self::default()
    }

    /// SPEC §14.6 — sample a new frame's mask. The current mask becomes the
    /// previous one, so `pressed`/`released` describe this transition.
    pub fn update(&mut self, mask: u8) {
        self.previous = self.current;
        self.current = mask;
    }

    /// SPEC §14.7 — forget all held buttons without reporting releases.
    pub fn clear(&mut self) {
        self.current = 0;
        self.previous = 0;
    }

    /// SPEC §9.3 — `input::held`: true while the button is down.
    pub fn held(&self, button: Button) -> bool {
        self.current & button.bit() != 0
    }

    /// SPEC §9.3 — `input::pressed`: true only on the frame it went down.
    pub fn pressed(&self, button: Button) -> bool {
        self.current & !self.previous & button.bit() != 0
    }

    /// SPEC §9.3 — `input::released`: true only on the frame it went up.
    pub fn released(&self, button: Button) -> bool {
        !self.current & self.previous & button.bit() != 0
    }

    /// The current frame's raw mask.
    pub fn mask(&self) -> u8 {
        self.current
    }
}
