//! Accessibility: subtitles for sounds, and a colour-blind friendly view.
//!
//! - **Subtitles** (Options) caption what you hear, bottom right, with an
//!   arrow toward where it came from: "< Groaner groans". Repeats of the same
//!   sound refresh their line instead of piling up.
//! - **Colour-blind** mode (Options) runs the world through a daltonising
//!   filter in the shader: red-green differences (Zappy Dust on or off, ripe
//!   crops, coloured wool) are shifted into brightness and blue, so they stay
//!   apart for people who can't tell red from green.

use crate::sound::{Mat, Sfx};
use macroquad::math::Vec3;

/// Seconds a caption stays up, and the most shown at once.
pub const CAPTION_SECS: f32 = 3.0;
pub const MAX_CAPTIONS: usize = 6;

/// What a sound is, in words (None: too common to be worth a caption).
pub fn caption(s: Sfx) -> Option<&'static str> {
    Some(match s {
        Sfx::Break(Mat::Glass) => "Glass breaks",
        Sfx::Break(_) => "Block breaks",
        Sfx::Hit(_) | Sfx::Step(_) | Sfx::Place(_) | Sfx::Click | Sfx::Craft => return None,
        Sfx::Hurt => "You're hurt",
        Sfx::MobHurt => "Something's hurt",
        Sfx::Oink => "Oinker oinks",
        Sfx::Hiss => "Hissing!",
        Sfx::Groan => "Groaner groans",
        Sfx::Explode => "Explosion",
        Sfx::Eat => "Eating",
        Sfx::Pop => "Item picked up",
        Sfx::Splash => "Splash",
        Sfx::Thud => "Thud",
        Sfx::Baa => "Fluffer baas",
        Sfx::Warp => "Something teleports",
        Sfx::Fanfare => "Advancement!",
        Sfx::Boing => "Boing",
        Sfx::Cluck => "Cluckster clucks",
        Sfx::Moo => "Mooer moos",
        Sfx::Rattle => "Bones rattle",
        Sfx::Skitter => "Webber skitters",
        Sfx::Bloop => "Bloop",
        Sfx::Twang => "Bow fires",
        Sfx::Thunk => "Arrow hits",
        Sfx::Thunder => "Thunder",
        Sfx::Chime => "Enchanting",
        Sfx::Woof => "Woofer woofs",
        Sfx::Snip => "Shears snip",
        Sfx::Hmm => "Hmmer hmms",
    })
}

/// Which way to point a caption's arrow: -1 left, 0 ahead or close by (or
/// nowhere in particular), 1 right. `yaw` is the listener's.
pub fn side(listener: Vec3, yaw: f32, at: Option<Vec3>) -> i8 {
    let Some(at) = at else { return 0 };
    let d = at - listener;
    if Vec3::new(d.x, 0.0, d.z).length() < 1.5 {
        return 0;
    }
    let right = Vec3::new(yaw.cos(), 0.0, yaw.sin());
    let fwd = Vec3::new(yaw.sin(), 0.0, -yaw.cos());
    let (r, f) = (d.dot(right), d.dot(fwd));
    if f > 0.0 && r.abs() < f * 0.4 {
        0
    } else if r > 0.0 {
        1
    } else {
        -1
    }
}

/// Captions on screen: (text, which side, seconds left).
#[derive(Default)]
pub struct Captions {
    pub lines: Vec<(&'static str, i8, f32)>,
}

impl Captions {
    pub fn add(&mut self, text: &'static str, side: i8) {
        if let Some(l) = self.lines.iter_mut().find(|l| l.0 == text) {
            l.1 = side;
            l.2 = CAPTION_SECS;
            return;
        }
        self.lines.push((text, side, CAPTION_SECS));
        if self.lines.len() > MAX_CAPTIONS {
            self.lines.remove(0);
        }
    }

    pub fn tick(&mut self, dt: f32) {
        for l in self.lines.iter_mut() {
            l.2 -= dt;
        }
        self.lines.retain(|l| l.2 > 0.0);
    }
}

/// The daltonising filter, for the shader (see render.rs).
pub const DALTONIZE_GLSL: &str = r#"
// Simulate deuteranopia in LMS space, then push what was lost into the
// channels that can still be seen (Fidaner, Lin and Ozguven's method).
vec3 daltonize(vec3 c) {
    float L = 17.8824 * c.r + 43.5161 * c.g + 4.11935 * c.b;
    float M = 3.45565 * c.r + 27.1554 * c.g + 3.86714 * c.b;
    float S = 0.0299566 * c.r + 0.184309 * c.g + 1.46709 * c.b;
    float m = 0.494207 * L + 1.24827 * S;
    vec3 sim = vec3(
        0.0809444479 * L - 0.130504409 * m + 0.116721066 * S,
        -0.0102485335 * L + 0.0540193266 * m - 0.113614708 * S,
        -0.000365296938 * L - 0.00412161469 * m + 0.693511405 * S);
    vec3 err = c - sim;
    return clamp(c + vec3(0.0, 0.7 * err.r + err.g, 0.7 * err.r + err.b), 0.0, 1.0);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captions_merge_and_expire() {
        let mut c = Captions::default();
        c.add("Oinker oinks", 1);
        c.add("Oinker oinks", -1);
        assert_eq!(c.lines.len(), 1);
        assert_eq!(c.lines[0].1, -1);
        for _ in 0..10 {
            c.add(caption(Sfx::Explode).unwrap(), 0);
            c.add(caption(Sfx::Moo).unwrap(), 0);
        }
        assert!(c.lines.len() <= MAX_CAPTIONS);
        c.tick(CAPTION_SECS + 0.1);
        assert!(c.lines.is_empty());
        assert_eq!(caption(Sfx::Step(Mat::Stone)), None);
    }

    #[test]
    fn sides() {
        let me = Vec3::ZERO;
        // Facing north (-z): east is on the right.
        assert_eq!(side(me, 0.0, Some(Vec3::new(10.0, 0.0, 0.0))), 1);
        assert_eq!(side(me, 0.0, Some(Vec3::new(-10.0, 0.0, 0.0))), -1);
        assert_eq!(side(me, 0.0, Some(Vec3::new(0.0, 0.0, -10.0))), 0);
        assert_eq!(side(me, 0.0, None), 0);
    }
}
