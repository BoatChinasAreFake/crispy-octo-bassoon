//! **Beds** and sleeping.
//!
//! A bed is still one block (budget cuts), but it's a proper little bed now:
//! a mattress with a red blanket and a pillow, a headboard and a footboard,
//! facing away from whoever put it down.
//!
//! Sleeping takes a few seconds: you lie down in it (in a position nobody
//! would call comfortable: a grown-up in a one-block bed), the screen fades,
//! and if you're still there when it's over the night is skipped and you
//! pop out onto a free spot beside the bed. Jumping, sneaking or getting hurt
//! gets you up early (and the night goes on).

use crate::block::*;
use crate::game::Game;
use macroquad::math::{IVec3, Mat4, Vec3};
use std::f32::consts::{FRAC_PI_2, PI};

/// Seconds in bed before the night is over.
pub const SLEEP_SECS: f32 = 3.5;

/// Any bed, whichever way it faces.
pub fn is_bed(id: Id) -> bool {
    id == BED || (BED_FACING_FIRST..BED_FACING_FIRST + 3).contains(&id)
}

/// Which way the head of the bed is (0 north .. 3 west).
pub fn facing(id: Id) -> u8 {
    if id == BED { 0 } else { (id - BED_FACING_FIRST) as u8 + 1 }
}

/// The bed facing `f`.
pub fn bed(f: u8) -> Id {
    if f.is_multiple_of(4) { BED } else { BED_FACING_FIRST + (f % 4) as Id - 1 }
}

/// Toward the head of a bed facing `f`.
pub fn head_dir(f: u8) -> Vec3 {
    match f % 4 {
        0 => Vec3::NEG_Z,
        1 => Vec3::X,
        2 => Vec3::Z,
        _ => Vec3::NEG_X,
    }
}

/// The bed's boxes: the mattress, the headboard and the footboard.
pub fn boxes(f: u8) -> [([f32; 3], [f32; 3]); 3] {
    let t = 2.0 / 16.0;
    let mattress = ([0.0, 3.0 / 16.0, 0.0], [1.0, 9.0 / 16.0, 1.0]);
    // (lo, hi) along the bed's length, for the head and the foot.
    let (head, foot) = match f % 4 {
        0 | 3 => ((0.0, t), (1.0 - t, 1.0)),
        _ => ((1.0 - t, 1.0), (0.0, t)),
    };
    let board = |(a, b): (f32, f32), h: f32| if f.is_multiple_of(2) { ([0.0, 0.0, a], [1.0, h, b]) } else { ([a, 0.0, 0.0], [b, h, 1.0]) };
    [mattress, board(head, 15.0 / 16.0), board(foot, 11.0 / 16.0)]
}

/// The top of the mattress.
const MATTRESS: f32 = 9.0 / 16.0;

/// Someone asleep (or trying): which bed, and for how long.
#[derive(Clone, Copy, Debug)]
pub struct Sleep {
    pub bed: IVec3,
    pub secs: f32,
}

/// Where a sleeper lies: the root of their model (feet at its origin),
/// stretched along the bed with their head on the pillow (and their feet
/// well off the end), rolled a little to one side.
pub fn pose(bed: IVec3, f: u8) -> Mat4 {
    let head = head_dir(f);
    let yaw = match f % 4 {
        0 => PI,
        1 => FRAC_PI_2,
        2 => 0.0,
        _ => -FRAC_PI_2,
    };
    let feet = bed.as_vec3() + Vec3::new(0.5, MATTRESS + 0.14, 0.5) - head * 0.95;
    Mat4::from_translation(feet) * Mat4::from_rotation_y(yaw) * Mat4::from_rotation_z(0.35) * Mat4::from_rotation_x(FRAC_PI_2)
}

impl Game {
    /// Lie down in the bed at `bed` (the checks have passed).
    pub fn lie_down(&mut self, bed: IVec3) {
        self.sleeping = Some(Sleep { bed, secs: 0.0 });
        self.player.body.vel = Vec3::ZERO;
        self.msg("You lie down. It's a one-block bed, so this is going to be awkward. Zzz...");
    }

    /// While asleep: stay put; fade out; at the end, skip the night (if it's
    /// ours to skip) and get up beside the bed.
    pub fn sleep_tick(&mut self, dt: f32, wake: bool) {
        let Some(mut s) = self.sleeping else { return };
        if !is_bed(self.world.get_v(s.bed)) || self.dead.is_some() {
            self.sleeping = None;
            return;
        }
        let f = facing(self.world.get_v(s.bed));
        // Lying on the mattress (the body box sits on it; the model is drawn lying down).
        self.player.body.pos = s.bed.as_vec3() + Vec3::new(0.5, MATTRESS + 0.01, 0.5);
        self.player.body.vel = Vec3::ZERO;
        self.player.fall_start = self.player.body.pos.y;
        if wake || self.player.hurt > 0.3 {
            self.get_up(s.bed, f);
            self.msg("You got up. The night goes on without you.");
            return;
        }
        s.secs += dt;
        self.sleeping = Some(s);
        if s.secs < SLEEP_SECS {
            return;
        }
        if self.is_client() {
            self.msg("Good morning! (Only the host's bed moves the clock, though.)");
        } else {
            self.finish_night();
        }
        self.get_up(s.bed, f);
    }

    /// Out of bed onto the nearest free spot beside it (or on top, if it's boxed in).
    fn get_up(&mut self, bed: IVec3, f: u8) {
        self.sleeping = None;
        let foot = -head_dir(f).as_ivec3();
        let side = IVec3::new(foot.z, 0, -foot.x);
        let free = |g: &Game, p: IVec3| !is_solid(g.world.get_v(p)) && !is_solid(g.world.get_v(p + IVec3::Y)) && (is_solid(g.world.get_v(p - IVec3::Y)) || is_solid(g.world.get_v(p - IVec3::Y * 2)));
        let spots = [side, -side, foot, foot + side, foot - side, -foot, -foot + side, -foot - side];
        let to = spots.iter().map(|d| bed + *d).find(|p| free(self, *p)).unwrap_or(bed + IVec3::Y);
        self.player.body.pos = to.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
        self.player.body.vel = Vec3::ZERO;
        self.player.fall_start = self.player.body.pos.y;
        // Facing away from the bed.
        let away = to.as_vec3() - bed.as_vec3();
        self.player.yaw = away.x.atan2(-away.z);
    }

    /// How dark the screen is while asleep (0 to 1).
    pub fn sleep_fade(&self) -> f32 {
        self.sleeping.map(|s| (s.secs / SLEEP_SECS).clamp(0.0, 1.0)).unwrap_or(0.0)
    }

    /// The camera while asleep: on the pillow, looking up past your feet.
    pub fn sleep_view(&self) -> Option<(Vec3, Vec3)> {
        let s = self.sleeping?;
        let f = facing(self.world.get_v(s.bed));
        let head = head_dir(f);
        let eye = s.bed.as_vec3() + Vec3::new(0.5, MATTRESS + 0.35, 0.5) + head * 0.3;
        Some((eye, (Vec3::Y * 1.6 - head).normalize()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use macroquad::math::ivec3;

    #[test]
    fn beds_face_the_way_theyre_put_and_sleeping_takes_a_while() {
        for f in 0..4 {
            assert!(is_bed(bed(f)) && facing(bed(f)) == f);
            assert_eq!(placing_item(bed(f)), Some(BED));
            // The headboard is on the head side.
            let [_, hb, fb] = boxes(f);
            let mid = |b: ([f32; 3], [f32; 3])| (Vec3::from_array(b.0) + Vec3::from_array(b.1)) * 0.5 - Vec3::new(0.5, 0.0, 0.5);
            assert!(mid(hb).dot(head_dir(f)) > 0.3 && mid(fb).dot(head_dir(f)) < -0.3, "facing {f}");
        }
        let mut g = crate::game::tests::arena(242);
        let b = ivec3(2, 50, 2);
        g.world.set_v(b, bed(1));
        g.time = 0.7; // night
        g.lie_down(b);
        // A second in: still in bed, still night.
        for _ in 0..20 {
            g.sleep_tick(0.05, false);
        }
        assert!(g.sleeping.is_some() && g.time > 0.6);
        assert!((g.player.body.pos - Vec3::new(2.5, 50.0 + MATTRESS + 0.01, 2.5)).length() < 0.01);
        for _ in 0..60 {
            g.sleep_tick(0.05, false);
        }
        assert!(g.sleeping.is_none() && g.time < 0.1, "morning");
        // Out beside the bed, on the floor.
        let p = g.player.body.pos;
        assert_eq!(p.y, 50.0);
        assert!((p - Vec3::new(2.5, 50.0, 2.5)).length() > 0.9);
        // Getting up early leaves it night.
        g.time = 0.7;
        g.lie_down(b);
        g.sleep_tick(0.05, false);
        g.sleep_tick(0.05, true);
        assert!(g.sleeping.is_none() && g.time > 0.6);
    }
}
