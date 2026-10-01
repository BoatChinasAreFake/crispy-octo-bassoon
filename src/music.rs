//! Note blocks and jukeboxes.
//!
//! - A **Note Block (Plinky)** (eight planks and Zappy Dust) plays a note when
//!   you hit it or when Zappy power reaches it, and right-clicking tunes it
//!   up a step (25 steps, then round again). The block underneath picks the
//!   instrument (see songs.rs), and it needs air above to be heard. The
//!   pitch lives in the block itself, so it saves and syncs like any block.
//! - A **Jukebox (Plays Your Jams)** (eight planks and a Dimond) plays the
//!   Music Disc you put in it to everyone nearby, louder the closer you are;
//!   right-click again to take the disc out. Discs turn up in dungeon and
//!   ruin chests, Hushed Cities, fortresses (and one from the Snouts).

use crate::block::*;
use crate::game::Game;
use crate::songs::{Instrument, DISCS, PITCHES};
use crate::sound::Sfx;
use macroquad::math::{IVec3, Vec3};

/// How far away a jukebox can be heard.
pub const JUKEBOX_RANGE: f32 = 48.0;

pub fn is_note_block(id: Id) -> bool {
    (NOTE_BLOCK..NOTE_BLOCK + PITCHES as Id).contains(&id)
}

pub fn pitch_of(id: Id) -> u8 {
    id.saturating_sub(NOTE_BLOCK).min(PITCHES as Id - 1) as u8
}

pub fn is_jukebox(id: Id) -> bool {
    (JUKEBOX..JUKEBOX_DISC_FIRST + DISCS.len() as Id).contains(&id)
}

/// Which disc a jukebox is playing (None: empty, or not a jukebox).
pub fn disc_in(id: Id) -> Option<u8> {
    (JUKEBOX_DISC_FIRST..JUKEBOX_DISC_FIRST + DISCS.len() as Id).contains(&id).then(|| (id - JUKEBOX_DISC_FIRST) as u8)
}

pub fn is_disc(item: Id) -> bool {
    (DISC_FIRST..DISC_FIRST + DISCS.len() as Id).contains(&item)
}

pub fn disc_title(n: u8) -> &'static str {
    DISCS.get(n as usize).map(|d| d.1).unwrap_or("?")
}

impl Game {
    /// Play the note block at `pos` (at `pitch`, which may be newer than the world here).
    pub fn sound_note(&mut self, pos: IVec3, pitch: u8) {
        if self.world.get_v(pos + IVec3::Y) != AIR {
            return;
        }
        let inst = Instrument::under(self.world.get_v(pos - IVec3::Y));
        let centre = pos.as_vec3() + Vec3::splat(0.5);
        self.sfx(Sfx::Note(inst.index(), pitch), Some(centre));
        self.note_particle(centre + Vec3::Y * 0.7, pitch);
        self.vibrate(centre, None);
    }

    /// A little coloured note floats up (green low, through to red high).
    pub fn note_particle(&mut self, at: Vec3, pitch: u8) {
        if self.dedicated {
            return;
        }
        let cell = (pitch as u32 * 15 / (PITCHES as u32 - 1)) as f32;
        self.particles.push(crate::entity::Particle {
            pos: at,
            vel: Vec3::new(0.0, 1.2, 0.0),
            life: 0.8,
            tile: crate::texture::T_NOTE_PARTICLE,
            uv: [(cell % 4.0) * 0.25, (cell / 4.0).floor() * 0.25],
            size: 0.3,
            gravity: 0.0,
        });
    }

    /// Right-click: one step up (and play it). Joined players' hosts play it for everyone.
    pub fn tune_note_block(&mut self, pos: IVec3) {
        let id = self.world.get_v(pos);
        let next = (pitch_of(id) + 1) % PITCHES;
        self.world.set_v(pos, NOTE_BLOCK + next as Id);
        self.player.swing = 1.0;
        if !self.is_client() {
            self.sound_note(pos, next);
        }
        self.held_name = 0.0;
        self.msg(format!("Note {} of {} ({}).", next + 1, PITCHES, Instrument::under(self.world.get_v(pos - IVec3::Y)).name()));
        self.advance("plinky");
    }

    /// Zappy power arriving plays the note (once per pulse).
    pub fn note_power(&mut self, pos: IVec3, powered: bool) {
        if powered == self.powered_notes.contains(&pos) {
            return;
        }
        if powered {
            self.powered_notes.insert(pos);
            let pitch = pitch_of(self.world.get_v(pos));
            self.sound_note(pos, pitch);
        } else {
            self.powered_notes.remove(&pos);
        }
    }

    /// Right-click a jukebox: take its disc out, or put the held one in.
    /// True if something happened.
    pub fn use_jukebox(&mut self, pos: IVec3, held: Id) -> bool {
        let id = self.world.get_v(pos);
        if let Some(d) = disc_in(id) {
            self.world.set_v(pos, JUKEBOX);
            // Joined players' hosts hand the disc back (see ledger.rs).
            if !self.is_client() {
                self.pop_drop(pos.as_vec3() + Vec3::new(0.5, 1.1, 0.5), DISC_FIRST + d as Id, 1);
            }
            self.player.swing = 1.0;
            return true;
        }
        if id == JUKEBOX && is_disc(held) {
            let d = (held - DISC_FIRST) as u8;
            self.world.set_v(pos, JUKEBOX_DISC_FIRST + d as Id);
            if !self.creative {
                self.inv.consume_held();
            }
            self.player.swing = 1.0;
            self.msg(format!("Now playing: {}", disc_title(d)));
            self.advance("now_playing");
            return true;
        }
        false
    }

    /// A broken jukebox lets go of its disc (where the world lives).
    pub fn jukebox_broken(&mut self, pos: IVec3, old: Id) {
        if let Some(d) = disc_in(old) {
            self.pop_drop(pos.as_vec3() + Vec3::splat(0.5), DISC_FIRST + d as Id, 1);
        }
    }

    /// The closest jukebox with a disc in it that can be heard from here.
    pub fn nearest_jukebox(&self) -> Option<(IVec3, u8)> {
        let ear = self.player.eye();
        self.world
            .jukeboxes
            .iter()
            .filter_map(|&p| disc_in(self.world.get_v(p)).map(|d| (p, d)))
            .filter(|(p, _)| (p.as_vec3() + Vec3::splat(0.5)).distance(ear) < JUKEBOX_RANGE)
            .min_by(|a, b| a.0.as_vec3().distance(ear).total_cmp(&b.0.as_vec3().distance(ear)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_blocks_tune_round_and_jukeboxes_swap_discs() {
        let mut g = crate::game::tests::arena(81);
        let p = IVec3::new(2, 50, 2);
        g.world.set_v(p - IVec3::Y, PLANKS);
        g.world.set_v(p, NOTE_BLOCK);
        for _ in 0..PITCHES {
            g.tune_note_block(p);
        }
        assert_eq!(g.world.get_v(p), NOTE_BLOCK, "25 steps comes back round");
        g.tune_note_block(p);
        assert_eq!(pitch_of(g.world.get_v(p)), 1);
        assert!(g.sounds.iter().any(|(s, _)| *s == Sfx::Note(Instrument::Bass.index(), 1)));
        // Covered up, it's silent.
        g.sounds.clear();
        g.world.set_v(p + IVec3::Y, STONE);
        g.sound_note(p, 3);
        assert!(g.sounds.is_empty());

        let j = IVec3::new(-2, 50, 2);
        g.world.set_v(j, JUKEBOX);
        g.inv.slots[g.inv.selected] = Some((DISC_FIRST + 4, 1));
        assert!(g.use_jukebox(j, DISC_FIRST + 4));
        assert_eq!(disc_in(g.world.get_v(j)), Some(4));
        assert_eq!(g.inv.count(DISC_FIRST + 4), 0);
        assert_eq!(g.nearest_jukebox(), Some((j, 4)));
        assert!(g.use_jukebox(j, AIR));
        assert_eq!(g.world.get_v(j), JUKEBOX);
        assert!(g.drops.iter().any(|d| d.item == DISC_FIRST + 4), "the disc pops out");
        assert_eq!(g.nearest_jukebox(), None);
    }
}
