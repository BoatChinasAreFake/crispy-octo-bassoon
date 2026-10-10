//! Leads: walk an animal about on a rope, and tie it up at a fence.
//!
//! - A **Lead** (four string and a Goo) goes on a farm animal, a pet, a pack
//!   animal or a Snow Golem: right-click it with the lead. It follows you on
//!   the rope, pulled along if it lags too far behind; run off too far and
//!   the lead snaps (and drops).
//! - Right-click a fence with animals on your leads and they're tied to it
//!   (a knot on the post), staying there until you untie them. Right-click a
//!   tied or led animal to take the lead back. Breaking the fence lets them
//!   go and drops the lead.
//!
//! Leads live where the world lives: the host works out where each lead's
//! far end is (`Mob::leash_end`) and sends it with the mobs, so everyone
//! sees the rope. Tied-up animals are saved with their fence.

use crate::animals::Interaction;
use crate::block::*;
use crate::entity::{Mob, MobKind};
use crate::game::Game;
use crate::players::record_key;
use crate::render::{DynGeo, Pass};
use crate::sound::{Mat, Sfx};
use macroquad::math::{IVec3, Mat4, Quat, Vec3};

/// The rope's length: an animal further away than this is pulled along.
pub const LEAD_LEN: f32 = 5.0;
/// Further than this and it snaps.
pub const SNAP: f32 = 12.0;
/// How far round a fence animals on your leads get tied to it.
pub const TIE_RANGE: f32 = 10.0;

/// What the other end of a lead is held by.
#[derive(Clone, Debug, PartialEq)]
pub enum Leash {
    /// A player, by record key.
    Player(String),
    /// A fence post.
    Fence(IVec3),
}

/// Can this go on a lead? (Farm animals, pets, pack animals and Snow Golems:
/// not monsters, not people, not the very big.)
pub fn leashable(m: &Mob) -> bool {
    let k = m.kind;
    (k.passive() || matches!(k, MobKind::Woofer | MobKind::SnowGolem | MobKind::PolarBear | MobKind::Rotsteed | MobKind::Clanker | MobKind::Allay))
        && !matches!(k, MobKind::Turtle | MobKind::Axolotl | MobKind::Ribbit | MobKind::Sneaker | MobKind::Squawker)
        && m.rider == 0
}

/// Where a lead ties onto a fence post.
pub fn knot(p: IVec3) -> Vec3 {
    p.as_vec3() + Vec3::new(0.5, 0.65, 0.5)
}

impl Game {
    /// A right-click on mob `i` by `who` holding `item`, as far as leads go.
    /// None: nothing to do with leads.
    pub fn lead_interact(&mut self, who: &str, i: usize, item: Id) -> Option<Interaction> {
        let m = &mut self.mobs[i];
        let at = m.body.pos + Vec3::Y * m.body.height * 0.7;
        match &m.leash {
            // Yours, or tied up: the lead comes back to you.
            Some(Leash::Player(p)) if p == who => {}
            Some(Leash::Fence(_)) => {}
            Some(Leash::Player(_)) => return None,
            None if item == LEAD && leashable(m) => {
                m.leash = Some(Leash::Player(who.to_string()));
                m.persistent = true;
                m.sitting = false;
                self.sfx(Sfx::Place(Mat::Grass), Some(at));
                self.advance_for(who, "walkies");
                return Some(Interaction::Ate);
            }
            None => return None,
        }
        m.leash = None;
        m.leash_end = None;
        self.sfx(Sfx::Snip, Some(at));
        Some(Interaction::Unleashed)
    }

    /// Tie `who`'s led animals to the fence at `fence`. How many were tied.
    pub fn tie_leads(&mut self, who: &str, fence: IVec3) -> usize {
        if !crate::carpentry::is_fence(self.world.get_v(fence)) {
            return 0;
        }
        let post = knot(fence);
        let mut n = 0;
        for m in self.mobs.iter_mut() {
            if matches!(&m.leash, Some(Leash::Player(p)) if p == who) && m.body.pos.distance(post) < TIE_RANGE {
                m.leash = Some(Leash::Fence(fence));
                n += 1;
            }
        }
        if n > 0 {
            self.sfx(Sfx::Place(Mat::Wood), Some(post));
        }
        n
    }

    /// The local player right-clicked a fence: tie up whatever's on our leads.
    /// True if that's what the click was for.
    pub fn use_fence_for_leads(&mut self, fence: IVec3) -> bool {
        let me = self.player.body.pos;
        if self.is_client() {
            // The host knows whose leads are whose; anything led near us might be ours.
            let any = self.mobs.iter().any(|m| m.leash_end.is_some_and(|e| e.distance(me + Vec3::Y * 1.2) < 1.5));
            if any {
                let held = self.inv.held();
                self.net_send_msg(crate::net::Msg::Interact { x: fence.x, y: fence.y, z: fence.z, item: held });
                self.player.swing = 1.0;
            }
            return any;
        }
        let who = record_key(&self.player_name);
        let n = self.tie_leads(&who, fence);
        if n > 0 {
            self.player.swing = 1.0;
        }
        n > 0
    }

    /// Where the world lives: leads pull, snap and come loose.
    pub fn leads_tick(&mut self, dt: f32) {
        if self.is_client() {
            return;
        }
        let spots = self.player_spots();
        let mut dropped: Vec<Vec3> = Vec::new();
        for m in self.mobs.iter_mut() {
            let Some(leash) = &m.leash else {
                m.leash_end = None;
                continue;
            };
            let end = match leash {
                Leash::Player(who) => spots.iter().find(|s| &s.0 == who).map(|s| s.1 + Vec3::Y * 1.1),
                Leash::Fence(p) => crate::carpentry::is_fence(self.world.get_v(*p)).then(|| knot(*p)),
            };
            let collar = m.body.pos + Vec3::Y * m.body.height * 0.7;
            let Some(end) = end.filter(|e| e.distance(collar) < SNAP) else {
                // Its holder's gone, the fence is gone, or it's too far: it's free.
                m.leash = None;
                m.leash_end = None;
                dropped.push(collar);
                continue;
            };
            m.leash_end = Some(end);
            let d = end - collar;
            let flat = Vec3::new(d.x, 0.0, d.z);
            if flat.length() > LEAD_LEN {
                // Pulled along the rope (and helped up a step).
                let pull = flat.normalize_or_zero() * (flat.length() - LEAD_LEN) * 6.0;
                m.body.vel.x += pull.x * dt;
                m.body.vel.z += pull.z * dt;
                m.yaw = flat.x.atan2(-flat.z);
                if d.y > 0.5 && m.body.on_ground {
                    m.body.vel.y = m.body.vel.y.max(5.0);
                }
            }
            // It goes where it's led, and doesn't wander off a fence.
            m.goal = (flat.length() > 2.5).then_some(end);
        }
        for at in dropped {
            self.pop_drop(at, LEAD, 1);
        }
    }

    /// Draw every lead in sight: a rope sagging from the collar to its end.
    pub fn draw_leads(&self, g: &mut DynGeo, eye: Vec3, range: f32) {
        g.begin(Pass::Opaque, [1.0; 4], false);
        for m in &self.mobs {
            let Some(end) = m.leash_end else { continue };
            let start = m.body.pos + Vec3::Y * m.body.height * 0.7;
            if start.distance(eye) > range {
                continue;
            }
            // While we hold it, it runs from our hand rather than our middle.
            let end = if (end - Vec3::Y * 1.1).distance(self.player.body.pos) < 0.5 && !self.away() {
                let yaw = self.player.yaw;
                self.player.body.pos + Vec3::new(yaw.cos() * 0.35 + yaw.sin() * 0.3, 1.1, yaw.sin() * 0.35 - yaw.cos() * 0.3)
            } else {
                end
            };
            let sky = self.world.sky_shade(start.x.floor() as i32, start.y.floor() as i32, start.z.floor() as i32).max(0.3);
            let span = start.distance(end);
            let sag = (LEAD_LEN - span).max(0.0) * 0.25 + 0.1;
            let steps = 10;
            let point = |t: f32| start.lerp(end, t) - Vec3::Y * sag * 4.0 * t * (1.0 - t);
            for k in 0..steps {
                let (a, b) = (point(k as f32 / steps as f32), point((k + 1) as f32 / steps as f32));
                let d = b - a;
                let rot = Quat::from_rotation_arc(Vec3::Z, d.normalize_or(Vec3::Z));
                let w = 0.05;
                let seg = Mat4::from_translation(a) * Mat4::from_quat(rot) * Mat4::from_translation(Vec3::new(-w / 2.0, -w / 2.0, 0.0)) * Mat4::from_scale(Vec3::new(w, w, d.length().max(0.01)));
                g.cube(&seg, [crate::texture::T_ROPE; 6], sky, [0.4, 0.0, 0.6, 1.0]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::tests::arena;

    #[test]
    fn leads_go_on_pull_tie_and_come_off() {
        let mut g = arena(911);
        let me = record_key(&g.player_name);
        let id = g.alloc_mob(MobKind::Oinker, Vec3::new(2.5, 51.0, 0.5));
        let i = g.mobs.iter().position(|m| m.id == id).unwrap();
        assert_eq!(g.lead_interact(&me, i, AIR), None, "nothing to do without a lead");
        assert_eq!(g.lead_interact(&me, i, LEAD), Some(Interaction::Ate));
        assert_eq!(g.mobs[i].leash, Some(Leash::Player(me.clone())));
        // Walk off: it's pulled along.
        g.player.body.pos = Vec3::new(10.5, 51.0, 0.5);
        g.leads_tick(0.1);
        assert!(g.mobs[i].body.vel.x > 0.0, "pulled toward us");
        assert!(g.mobs[i].leash_end.is_some());
        // Tie it to a fence.
        let fence = IVec3::new(4, 51, 3);
        g.world.set_v(fence, FENCE_FIRST);
        assert_eq!(g.tie_leads(&me, fence), 1);
        assert_eq!(g.mobs[i].leash, Some(Leash::Fence(fence)));
        // Untie it: the lead comes back.
        assert_eq!(g.lead_interact(&me, i, AIR), Some(Interaction::Unleashed));
        assert_eq!(g.mobs[i].leash, None);
        // Monsters don't go on leads.
        let h = g.alloc_mob(MobKind::Hisser, Vec3::new(-3.5, 51.0, 0.5));
        let j = g.mobs.iter().position(|m| m.id == h).unwrap();
        assert_eq!(g.lead_interact(&me, j, LEAD), None);
    }

    #[test]
    fn leads_snap_and_fences_let_go() {
        let mut g = arena(912);
        let me = record_key(&g.player_name);
        let id = g.alloc_mob(MobKind::Mooer, Vec3::new(2.5, 51.0, 0.5));
        let i = g.mobs.iter().position(|m| m.id == id).unwrap();
        g.lead_interact(&me, i, LEAD);
        g.player.body.pos = Vec3::new(40.5, 51.0, 0.5);
        g.leads_tick(0.05);
        assert_eq!(g.mobs[i].leash, None, "snapped");
        assert!(g.drops.iter().any(|d| d.item == LEAD), "the lead drops");
        // Tied to a fence that's then broken: it goes free.
        let fence = IVec3::new(3, 51, 0);
        g.world.set_v(fence, FENCE_FIRST);
        g.mobs[i].leash = Some(Leash::Fence(fence));
        g.leads_tick(0.05);
        assert!(g.mobs[i].leash_end.is_some());
        g.world.set_v(fence, AIR);
        g.leads_tick(0.05);
        assert_eq!(g.mobs[i].leash, None);
    }
}
