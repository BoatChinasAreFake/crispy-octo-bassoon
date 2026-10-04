//! Game controllers (with the `gamepad` feature). The layout is fixed:
//!
//! | Stick / button   | Does                         |
//! |------------------|------------------------------|
//! | Left stick       | walk (press it in: sprint)   |
//! | Right stick      | look                         |
//! | A / Cross        | jump                         |
//! | B / Circle       | sneak; closes screens        |
//! | X / Square       | throw item                   |
//! | Y / Triangle     | inventory                    |
//! | Right trigger    | mine / attack                |
//! | Left trigger     | place / use                  |
//! | Bumpers          | hotbar left / right          |
//! | Start            | pause                        |
//! | Select / Back    | third person                 |

/// One frame of controller input (all zero when there's no controller).
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct PadFrame {
    /// Left stick: x right, y forward.
    pub walk: [f32; 2],
    /// Right stick: x right, y up.
    pub look: [f32; 2],
    pub jump: bool,
    pub jump_pressed: bool,
    pub sneak: bool,
    pub sprint: bool,
    pub attack: bool,
    pub attack_pressed: bool,
    pub use_held: bool,
    pub use_pressed: bool,
    pub drop: bool,
    pub inventory: bool,
    pub back: bool,
    pub pause: bool,
    pub perspective: bool,
    /// -1 or +1 to move along the hotbar.
    pub hotbar: i32,
}

/// Sticks rest a little off centre; ignore that, then rescale so small tilts still count.
#[cfg_attr(not(feature = "gamepad"), allow(dead_code))]
pub fn dead_zone(v: f32) -> f32 {
    const DEAD: f32 = 0.2;
    if v.abs() < DEAD { 0.0 } else { v.signum() * (v.abs() - DEAD) / (1.0 - DEAD) }
}

#[cfg(feature = "gamepad")]
mod imp {
    use super::{dead_zone, PadFrame};
    use gilrs::{Axis, Button, Gilrs};

    const BUTTONS: [Button; 14] = [
        Button::South,
        Button::East,
        Button::West,
        Button::North,
        Button::LeftTrigger,
        Button::RightTrigger,
        Button::LeftTrigger2,
        Button::RightTrigger2,
        Button::Start,
        Button::Select,
        Button::LeftThumb,
        Button::RightThumb,
        Button::DPadLeft,
        Button::DPadRight,
    ];

    pub struct Pad {
        gilrs: Option<Gilrs>,
        was: [bool; BUTTONS.len()],
        pub name: Option<String>,
    }

    impl Pad {
        pub fn new() -> Pad {
            // No controller support on this machine is fine: keyboard and mouse still work.
            Pad { gilrs: Gilrs::new().ok(), was: [false; BUTTONS.len()], name: None }
        }

        pub fn poll(&mut self) -> PadFrame {
            let Some(g) = &mut self.gilrs else { return PadFrame::default() };
            while g.next_event().is_some() {}
            let Some((_, pad)) = g.gamepads().find(|(_, p)| p.is_connected()) else {
                self.name = None;
                self.was = [false; BUTTONS.len()];
                return PadFrame::default();
            };
            self.name = Some(pad.name().to_string());
            let now = BUTTONS.map(|b| pad.is_pressed(b));
            let down = |b: Button| BUTTONS.iter().position(|&x| x == b).is_some_and(|i| now[i]);
            let hit = |b: Button| BUTTONS.iter().position(|&x| x == b).is_some_and(|i| now[i] && !self.was[i]);
            // Triggers are analogue on most pads; count a firm squeeze.
            let trigger = |b: Button| down(b) || pad.button_data(b).is_some_and(|d| d.value() > 0.5);
            let frame = PadFrame {
                walk: [dead_zone(pad.value(Axis::LeftStickX)), dead_zone(pad.value(Axis::LeftStickY))],
                look: [dead_zone(pad.value(Axis::RightStickX)), dead_zone(pad.value(Axis::RightStickY))],
                jump: down(Button::South),
                jump_pressed: hit(Button::South),
                sneak: down(Button::East),
                sprint: down(Button::LeftThumb),
                attack: trigger(Button::RightTrigger2),
                attack_pressed: hit(Button::RightTrigger2),
                use_held: trigger(Button::LeftTrigger2),
                use_pressed: hit(Button::LeftTrigger2),
                drop: hit(Button::West),
                inventory: hit(Button::North),
                back: hit(Button::East),
                pause: hit(Button::Start),
                perspective: hit(Button::Select),
                hotbar: (hit(Button::RightTrigger) || hit(Button::DPadRight)) as i32 - (hit(Button::LeftTrigger) || hit(Button::DPadLeft)) as i32,
            };
            self.was = now;
            frame
        }
    }
}

#[cfg(not(feature = "gamepad"))]
mod imp {
    use super::PadFrame;

    pub struct Pad {
        pub name: Option<String>,
    }

    impl Pad {
        pub fn new() -> Pad {
            Pad { name: None }
        }
        pub fn poll(&mut self) -> PadFrame {
            PadFrame::default()
        }
    }
}

pub use imp::Pad;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dead_zone_ignores_drift_and_keeps_full_tilt() {
        assert_eq!(dead_zone(0.1), 0.0);
        assert_eq!(dead_zone(-0.19), 0.0);
        assert_eq!(dead_zone(1.0), 1.0);
        assert_eq!(dead_zone(-1.0), -1.0);
        assert!((dead_zone(0.6) - 0.5).abs() < 1e-6);
    }
}
