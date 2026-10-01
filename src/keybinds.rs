//! Rebindable controls: which key or mouse button does what.
//!
//! Every action has up to two bindings, saved in `settings.txt` as
//! `bind.<action>=<name> <name>` (for example `bind.jump=Space`). Hotbar
//! numbers, Esc and the F keys stay where they are.

use macroquad::input::{is_key_down, is_key_pressed, is_mouse_button_down, is_mouse_button_pressed, KeyCode, MouseButton};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Forward,
    Back,
    Left,
    Right,
    Jump,
    Sneak,
    Sprint,
    Attack,
    Use,
    PickBlock,
    Drop,
    Inventory,
    Chat,
    Command,
    Perspective,
    Screenshot,
}

pub const ACTIONS: [Action; 16] = [
    Action::Forward,
    Action::Back,
    Action::Left,
    Action::Right,
    Action::Jump,
    Action::Sneak,
    Action::Sprint,
    Action::Attack,
    Action::Use,
    Action::PickBlock,
    Action::Drop,
    Action::Inventory,
    Action::Chat,
    Action::Command,
    Action::Perspective,
    Action::Screenshot,
];

impl Action {
    /// The name used in settings.txt.
    pub fn key(self) -> &'static str {
        match self {
            Action::Forward => "forward",
            Action::Back => "back",
            Action::Left => "left",
            Action::Right => "right",
            Action::Jump => "jump",
            Action::Sneak => "sneak",
            Action::Sprint => "sprint",
            Action::Attack => "attack",
            Action::Use => "use",
            Action::PickBlock => "pick_block",
            Action::Drop => "drop",
            Action::Inventory => "inventory",
            Action::Chat => "chat",
            Action::Command => "command",
            Action::Perspective => "perspective",
            Action::Screenshot => "screenshot",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Action::Forward => "Walk Forward",
            Action::Back => "Walk Backward",
            Action::Left => "Strafe Left",
            Action::Right => "Strafe Right",
            Action::Jump => "Jump / Swim Up",
            Action::Sneak => "Sneak",
            Action::Sprint => "Sprint",
            Action::Attack => "Mine / Attack",
            Action::Use => "Place / Use",
            Action::PickBlock => "Pick Block",
            Action::Drop => "Throw Item",
            Action::Inventory => "Inventory",
            Action::Chat => "Chat",
            Action::Command => "Command",
            Action::Perspective => "Third Person",
            Action::Screenshot => "Screenshot",
        }
    }

    fn defaults(self) -> [Option<Bind>; 2] {
        use Bind::{Key, Mouse};
        let k = |a: KeyCode| Some(Key(a));
        match self {
            Action::Forward => [k(KeyCode::W), k(KeyCode::Up)],
            Action::Back => [k(KeyCode::S), k(KeyCode::Down)],
            Action::Left => [k(KeyCode::A), k(KeyCode::Left)],
            Action::Right => [k(KeyCode::D), k(KeyCode::Right)],
            Action::Jump => [k(KeyCode::Space), None],
            Action::Sneak => [k(KeyCode::LeftShift), k(KeyCode::RightShift)],
            Action::Sprint => [k(KeyCode::LeftControl), k(KeyCode::R)],
            Action::Attack => [Some(Mouse(MouseButton::Left)), None],
            Action::Use => [Some(Mouse(MouseButton::Right)), None],
            Action::PickBlock => [Some(Mouse(MouseButton::Middle)), None],
            Action::Drop => [k(KeyCode::Q), None],
            Action::Inventory => [k(KeyCode::E), k(KeyCode::Tab)],
            Action::Chat => [k(KeyCode::T), k(KeyCode::Enter)],
            Action::Command => [k(KeyCode::Slash), None],
            Action::Perspective => [k(KeyCode::F5), None],
            Action::Screenshot => [k(KeyCode::F2), None],
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bind {
    Key(KeyCode),
    Mouse(MouseButton),
}

/// Keys that can be bound, by their saved names.
macro_rules! key_table {
    ($($k:ident),* $(,)?) => {
        const KEYS: &[(&str, KeyCode)] = &[$((stringify!($k), KeyCode::$k)),*];
    };
}

key_table!(
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    Key0, Key1, Key2, Key3, Key4, Key5, Key6, Key7, Key8, Key9,
    Space, Enter, Tab, Backspace, Insert, Delete, Home, End, PageUp, PageDown,
    Up, Down, Left, Right, CapsLock, LeftShift, RightShift, LeftControl, RightControl, LeftAlt, RightAlt,
    Apostrophe, Comma, Minus, Period, Slash, Semicolon, Equal, LeftBracket, Backslash, RightBracket, GraveAccent,
    F1, F2, F4, F5, F6, F7, F8, F9, F10, F12,
    Kp0, Kp1, Kp2, Kp3, Kp4, Kp5, Kp6, Kp7, Kp8, Kp9, KpDecimal, KpDivide, KpMultiply, KpSubtract, KpAdd, KpEnter,
);

const BUTTONS: [(&str, MouseButton); 3] = [("MouseLeft", MouseButton::Left), ("MouseRight", MouseButton::Right), ("MouseMiddle", MouseButton::Middle)];

impl Bind {
    pub fn name(self) -> &'static str {
        match self {
            Bind::Key(k) => KEYS.iter().find(|(_, c)| *c == k).map(|(n, _)| *n).unwrap_or("?"),
            Bind::Mouse(b) => BUTTONS.iter().find(|(_, c)| *c == b).map(|(n, _)| *n).unwrap_or("?"),
        }
    }

    pub fn parse(name: &str) -> Option<Bind> {
        KEYS.iter().find(|(n, _)| *n == name).map(|&(_, k)| Bind::Key(k)).or_else(|| BUTTONS.iter().find(|(n, _)| *n == name).map(|&(_, b)| Bind::Mouse(b)))
    }

    /// Friendlier than the saved name: "Mouse Left", "Key 4", "Left Shift".
    pub fn display(self) -> String {
        let n = self.name();
        let n = n.strip_prefix("Key").filter(|d| d.len() == 1).map(|d| format!("Key {d}")).unwrap_or_else(|| n.to_string());
        let mut out = String::new();
        for (i, c) in n.chars().enumerate() {
            if i > 0 && c.is_ascii_uppercase() && !out.ends_with(' ') {
                out.push(' ');
            }
            out.push(c);
        }
        out
    }

    fn down(self) -> bool {
        match self {
            Bind::Key(k) => is_key_down(k),
            Bind::Mouse(b) => is_mouse_button_down(b),
        }
    }

    fn pressed(self) -> bool {
        match self {
            Bind::Key(k) => is_key_pressed(k),
            Bind::Mouse(b) => is_mouse_button_pressed(b),
        }
    }

    /// Whatever the player just pressed (for the rebinding screen).
    pub fn just_pressed() -> Option<Bind> {
        KEYS.iter().find(|(_, k)| is_key_pressed(*k)).map(|&(_, k)| Bind::Key(k)).or_else(|| BUTTONS.iter().find(|(_, b)| is_mouse_button_pressed(*b)).map(|&(_, b)| Bind::Mouse(b)))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Bindings {
    binds: [[Option<Bind>; 2]; ACTIONS.len()],
}

impl Default for Bindings {
    fn default() -> Self {
        Bindings { binds: ACTIONS.map(|a| a.defaults()) }
    }
}

impl Bindings {
    fn slot(a: Action) -> usize {
        ACTIONS.iter().position(|&b| b == a).unwrap_or(0)
    }

    pub fn get(&self, a: Action) -> [Option<Bind>; 2] {
        self.binds[Self::slot(a)]
    }

    pub fn down(&self, a: Action) -> bool {
        self.get(a).iter().flatten().any(|b| b.down())
    }

    pub fn pressed(&self, a: Action) -> bool {
        self.get(a).iter().flatten().any(|b| b.pressed())
    }

    /// Bind `b` to `a` (primary or secondary). One input does one thing, so it
    /// comes off any other action that had it.
    pub fn set(&mut self, a: Action, secondary: bool, b: Option<Bind>) {
        if let Some(b) = b {
            for pair in self.binds.iter_mut() {
                for slot in pair.iter_mut() {
                    if *slot == Some(b) {
                        *slot = None;
                    }
                }
            }
        }
        self.binds[Self::slot(a)][secondary as usize] = b;
    }

    /// How the bindings of an action read on screen.
    pub fn describe(&self, a: Action) -> String {
        let names: Vec<String> = self.get(a).iter().flatten().map(|b| b.display()).collect();
        if names.is_empty() { "(none)".into() } else { names.join(" or ") }
    }

    /// `bind.<action>=...` lines for settings.txt.
    pub fn to_text(&self) -> String {
        ACTIONS
            .iter()
            .map(|&a| {
                let names: Vec<&str> = self.get(a).iter().flatten().map(|b| b.name()).collect();
                format!("bind.{}={}\n", a.key(), names.join(" "))
            })
            .collect()
    }

    /// Take a `bind.<action>` line from settings.txt. False if it isn't one of ours.
    pub fn read(&mut self, key: &str, value: &str) -> bool {
        let Some(a) = key.strip_prefix("bind.").and_then(|k| ACTIONS.iter().find(|a| a.key() == k)) else { return false };
        let mut found = value.split_whitespace().filter_map(Bind::parse);
        let (first, second) = (found.next(), found.next());
        // An empty line clears an action; unknown names leave the defaults alone.
        if first.is_none() && !value.trim().is_empty() {
            return true;
        }
        self.binds[Self::slot(*a)] = [first, second];
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_default_has_a_name() {
        for a in ACTIONS {
            for b in a.defaults().into_iter().flatten() {
                assert_eq!(Bind::parse(b.name()), Some(b), "{a:?}");
            }
        }
    }

    #[test]
    fn bindings_round_trip_through_text() {
        let mut b = Bindings::default();
        b.set(Action::Jump, true, Some(Bind::Mouse(MouseButton::Middle)));
        b.set(Action::Drop, false, Some(Bind::Key(KeyCode::G)));
        let mut back = Bindings::default();
        for line in b.to_text().lines() {
            let (k, v) = line.split_once('=').unwrap();
            assert!(back.read(k, v));
        }
        assert_eq!(back, b);
        // Taking the middle button for jumping took it off pick block.
        assert_eq!(b.get(Action::PickBlock), [None, None]);
        assert_eq!(b.describe(Action::Jump), "Space or Mouse Middle");
        assert!(!back.read("fov", "70"));
        // Nonsense leaves the binding as it was.
        assert!(back.read("bind.drop", "Banana"));
        assert_eq!(back.get(Action::Drop)[0], Some(Bind::Key(KeyCode::G)));
    }

    #[test]
    fn display_names_read_well() {
        assert_eq!(Bind::Key(KeyCode::LeftShift).display(), "Left Shift");
        assert_eq!(Bind::Key(KeyCode::Key4).display(), "Key 4");
        assert_eq!(Bind::Key(KeyCode::F5).display(), "F5");
    }
}
