//! Note blocks and jukeboxes.
//!
//! - A **Note Block (Plinky)** (eight planks and Zappy Dust) plays a note when
//!   you hit it or when Zappy power reaches it, and right-clicking tunes it
//!   up a step (25 steps, then round again). The block underneath picks the
//!   instrument (see below), and it needs air above to be heard. The
//!   pitch lives in the block itself, so it saves and syncs like any block.
//! - A **Jukebox (Plays Your Jams)** (eight planks and a Dimond) plays the
//!   Music Disc you put in it to everyone nearby, louder the closer you are;
//!   right-click again to take the disc out. Discs turn up in dungeon and
//!   ruin chests, Hushed Cities, fortresses (and one from the Snouts).
//!
//! Music for note blocks and jukeboxes, synthesised like every other sound.
//!
//! - **Note blocks** have nine instruments, picked by the block underneath
//!   (as in Minecraft): wood is a double bass, stone a bass drum, sand and
//!   gravel a snare, glass a hi-hat click, gold a bell, ice chimes,
//!   terracotta a flute, wool a guitar, and anything else a harp.
//!   Each plays 25 notes, two octaves from F sharp.
//! - **Music discs** are composed here, a few seconds after one is first
//!   wanted: a little chord progression, a bass line, drums when the style
//!   wants them, and a melody built from a motif that comes back (changed a
//!   bit) the way tunes do. Eight discs, eight styles.

use crate::block::*;
use crate::game::Game;
use crate::sound::Sfx;
use macroquad::math::{IVec3, Vec3};
use crate::noise::Rng;
use crate::sound::{finish, pad, piano, reverb, SR};
use std::f32::consts::TAU;

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

// ---- songs

/// Notes a note block can play (two octaves, F#3 up to F#5 on the harp).
pub const PITCHES: u8 = 25;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Instrument {
    Harp,
    Bass,
    Drum,
    Snare,
    Hat,
    Bell,
    Chime,
    Flute,
    Guitar,
}

pub const INSTRUMENTS: [Instrument; 9] = [Instrument::Harp, Instrument::Bass, Instrument::Drum, Instrument::Snare, Instrument::Hat, Instrument::Bell, Instrument::Chime, Instrument::Flute, Instrument::Guitar];

impl Instrument {
    pub fn index(self) -> u8 {
        INSTRUMENTS.iter().position(|&i| i == self).unwrap_or(0) as u8
    }

    pub fn from_index(i: u8) -> Option<Instrument> {
        INSTRUMENTS.get(i as usize).copied()
    }

    pub fn name(self) -> &'static str {
        match self {
            Instrument::Harp => "Harp",
            Instrument::Bass => "Double Bass",
            Instrument::Drum => "Bass Drum",
            Instrument::Snare => "Snare",
            Instrument::Hat => "Hi-Hat",
            Instrument::Bell => "Bell",
            Instrument::Chime => "Chimes",
            Instrument::Flute => "Flute",
            Instrument::Guitar => "Guitar",
        }
    }

    /// What a note block sounds like sitting on `below`.
    pub fn under(below: Id) -> Instrument {
        let b = block(below);
        if below == GOLD_ORE || below == SCORCH_GOLD_ORE {
            Instrument::Bell
        } else if matches!(below, ICE | SNOW_GRASS) {
            Instrument::Chime
        } else if below == WOOL || (DYED_WOOL..STAINED_GLASS).contains(&below) {
            Instrument::Guitar
        } else if (TERRACOTTA..TERRACOTTA + 4).contains(&below) {
            Instrument::Flute
        } else if matches!(below, SAND | GRAVEL | RED_SAND | EMBERSAND) {
            Instrument::Snare
        } else if below == GLASS || crate::carpentry::is_stained_glass(below) {
            Instrument::Hat
        } else if b.sound == 1 {
            // Planks, logs, anything wooden.
            Instrument::Bass
        } else if b.sound == 0 && below != AIR {
            Instrument::Drum
        } else {
            Instrument::Harp
        }
    }

    /// The instrument's register, relative to the harp.
    fn octave(self) -> f32 {
        match self {
            Instrument::Bass => 0.25,
            Instrument::Guitar => 0.5,
            Instrument::Flute => 2.0,
            Instrument::Bell | Instrument::Chime => 4.0,
            _ => 1.0,
        }
    }
}

/// The pitch of note `pitch` (0..25) on `inst`.
pub fn note_freq(inst: Instrument, pitch: u8) -> f32 {
    185.0 * 2f32.powf(pitch.min(PITCHES - 1) as f32 / 12.0) * inst.octave()
}

fn samples(secs: f32) -> usize {
    (secs * SR as f32) as usize
}

fn t_of(i: usize) -> f32 {
    i as f32 / SR as f32
}

// ---------------------------------------------------------------- voices

/// A plucked string (Karplus-Strong): a burst of noise round a delay line.
#[allow(clippy::too_many_arguments)]
fn pluck(out: &mut [f32], start: f32, len: f32, f: f32, gain: f32, bright: f32, rng: &mut Rng) {
    let s0 = samples(start);
    let period = ((SR as f32 / f.max(20.0)) as usize).max(2);
    let mut buf: Vec<f32> = (0..period).map(|_| rng.range(-1.0, 1.0)).collect();
    // Darker strings start softer.
    for _ in 0..((1.0 - bright) * 3.0) as usize {
        let first = buf[0];
        for i in 0..period {
            let next = if i + 1 < period { buf[i + 1] } else { first };
            buf[i] = (buf[i] + next) * 0.5;
        }
    }
    let n = samples(len);
    let damp = 0.996 + bright * 0.003;
    for i in 0..n {
        if s0 + i >= out.len() {
            break;
        }
        let a = buf[i % period];
        let b = buf[(i + 1) % period];
        buf[i % period] = (a + b) * 0.5 * damp;
        let release = ((len - t_of(i)) / 0.08).clamp(0.0, 1.0);
        out[s0 + i] += a * gain * release;
    }
}

/// A chiptune square-ish wave (odd harmonics), with a quick fall to a sustain.
fn square(out: &mut [f32], start: f32, len: f32, f: f32, gain: f32) {
    let s0 = samples(start);
    let n = samples(len);
    let mut ph = 0.0f32;
    for i in 0..n {
        if s0 + i >= out.len() {
            break;
        }
        let t = t_of(i);
        ph += TAU * f / SR as f32;
        let mut v = 0.0;
        for k in [1.0f32, 3.0, 5.0, 7.0, 9.0] {
            if f * k < 7000.0 {
                v += (ph * k).sin() / k;
            }
        }
        let env = (t / 0.003).min(1.0) * (0.55 + 0.45 * (-t * 8.0).exp()) * ((len - t) / 0.03).clamp(0.0, 1.0);
        out[s0 + i] += v * env * gain;
    }
}

/// A breathy flute: a sine with a little second harmonic, vibrato and air.
fn flute(out: &mut [f32], start: f32, len: f32, f: f32, gain: f32, rng: &mut Rng) {
    let s0 = samples(start);
    let n = samples(len);
    let mut ph = 0.0f32;
    let mut air = 0.0f32;
    for i in 0..n {
        if s0 + i >= out.len() {
            break;
        }
        let t = t_of(i);
        let vib = 1.0 + (t * 5.5 * TAU).sin() * 0.006 * (t / 0.3).min(1.0);
        ph += TAU * f * vib / SR as f32;
        air += 0.2 * (rng.range(-1.0, 1.0) - air);
        let env = (t / 0.06).min(1.0) * ((len - t) / 0.12).clamp(0.0, 1.0);
        out[s0 + i] += (ph.sin() + (ph * 2.0).sin() * 0.18 + air * 0.08) * env * gain;
    }
}

/// A bell: inharmonic partials ringing on.
fn bell(out: &mut [f32], start: f32, len: f32, f: f32, gain: f32) {
    let s0 = samples(start);
    let n = samples(len);
    let parts = [(1.0f32, 1.0f32, 1.6f32), (2.0, 0.6, 2.4), (2.76, 0.45, 3.2), (5.4, 0.25, 5.0), (8.9, 0.1, 8.0)];
    let mut ph = [0.0f32; 5];
    for i in 0..n {
        if s0 + i >= out.len() {
            break;
        }
        let t = t_of(i);
        let mut v = 0.0;
        for (k, &(h, a, d)) in parts.iter().enumerate() {
            ph[k] += TAU * f * h / SR as f32;
            v += ph[k].sin() * a * (-t * d).exp();
        }
        out[s0 + i] += v * (t / 0.002).min(1.0) * ((len - t) / 0.1).clamp(0.0, 1.0) * gain;
    }
}

/// An organ: steady harmonics with a slow wobble.
fn organ(out: &mut [f32], start: f32, len: f32, f: f32, gain: f32) {
    let s0 = samples(start);
    let n = samples(len);
    let mut ph = 0.0f32;
    for i in 0..n {
        if s0 + i >= out.len() {
            break;
        }
        let t = t_of(i);
        ph += TAU * f / SR as f32;
        let v = ph.sin() + (ph * 2.0).sin() * 0.5 + (ph * 3.0).sin() * 0.3 + (ph * 4.0).sin() * 0.15;
        let env = (t / 0.02).min(1.0) * ((len - t) / 0.05).clamp(0.0, 1.0) * (1.0 + (t * 6.0 * TAU).sin() * 0.05);
        out[s0 + i] += v * env * gain * 0.5;
    }
}

/// A kick: a sine diving from `f` down to a thump.
fn kick(out: &mut [f32], start: f32, f: f32, gain: f32) {
    let s0 = samples(start);
    let n = samples(0.35);
    let mut ph = 0.0f32;
    for i in 0..n {
        if s0 + i >= out.len() {
            break;
        }
        let t = t_of(i);
        let fr = 45.0 + (f - 45.0) * (-t * 30.0).exp();
        ph += TAU * fr / SR as f32;
        out[s0 + i] += ph.sin() * (-t * 9.0).exp() * gain;
    }
}

/// Filtered noise with a sharp decay: snares, hats, shakers.
#[allow(clippy::too_many_arguments)]
fn noise_hit(out: &mut [f32], start: f32, len: f32, decay: f32, lo: f32, hi: f32, gain: f32, rng: &mut Rng) {
    let s0 = samples(start);
    let (mut lp, mut hp) = (0.0f32, 0.0f32);
    let (a_hi, a_lo) = (1.0 - (-TAU * hi / SR as f32).exp(), 1.0 - (-TAU * lo / SR as f32).exp());
    for i in 0..samples(len) {
        if s0 + i >= out.len() {
            break;
        }
        let t = t_of(i);
        let x = rng.range(-1.0, 1.0);
        lp += a_hi * (x - lp);
        hp += a_lo * (lp - hp);
        out[s0 + i] += (lp - hp) * (t / 0.001).min(1.0) * (-t * decay).exp() * gain;
    }
}

fn snare(out: &mut [f32], start: f32, gain: f32, rng: &mut Rng) {
    noise_hit(out, start, 0.25, 22.0, 1200.0, 7000.0, gain, rng);
    kick(out, start, 220.0, gain * 0.35);
}

fn hat(out: &mut [f32], start: f32, gain: f32, rng: &mut Rng) {
    noise_hit(out, start, 0.08, 70.0, 7000.0, 11000.0, gain, rng);
}

// ---------------------------------------------------------------- note blocks

/// One note block note (about a second; drums are shorter).
pub fn synth_note(inst: Instrument, pitch: u8) -> Vec<f32> {
    let mut rng = Rng::new(0x707E ^ ((inst.index() as u64) << 8) ^ pitch as u64);
    let f = note_freq(inst, pitch);
    let p = pitch as f32 / (PITCHES - 1) as f32;
    let len = match inst {
        Instrument::Drum | Instrument::Snare | Instrument::Hat => 0.4,
        Instrument::Bell | Instrument::Chime => 1.6,
        _ => 1.1,
    };
    let mut v = vec![0.0f32; samples(len)];
    match inst {
        Instrument::Harp => piano(&mut v, 0.0, len, f, 0.7),
        Instrument::Bass => pluck(&mut v, 0.0, len, f, 0.9, 0.2, &mut rng),
        Instrument::Guitar => pluck(&mut v, 0.0, len, f, 0.8, 0.7, &mut rng),
        Instrument::Drum => kick(&mut v, 0.0, 90.0 + p * 120.0, 1.0),
        Instrument::Snare => noise_hit(&mut v, 0.0, 0.3, 20.0, 900.0 + p * 1500.0, 6000.0 + p * 3000.0, 1.0, &mut rng),
        Instrument::Hat => noise_hit(&mut v, 0.0, 0.1, 60.0, 5000.0 + p * 4000.0, 11000.0, 1.0, &mut rng),
        Instrument::Bell => bell(&mut v, 0.0, len, f, 0.6),
        Instrument::Chime => {
            bell(&mut v, 0.0, len, f, 0.35);
            bell(&mut v, 0.0, len, f * 1.5, 0.15);
        }
        Instrument::Flute => flute(&mut v, 0.0, len * 0.8, f, 0.7, &mut rng),
    }
    finish(v, 0.9)
}

// ---------------------------------------------------------------- discs

/// The eight discs: (key, title).
pub const DISCS: [(&str, &str); 8] = [
    ("music_disc_cat", "Cat, Probably"),
    ("music_disc_blocks", "Blocks Again"),
    ("music_disc_chirp", "Chirpier"),
    ("music_disc_far", "Too Far"),
    ("music_disc_mall", "Mall Muzak"),
    ("music_disc_strad", "Strad-Adjacent"),
    ("music_disc_wait", "Wait For It"),
    ("music_disc_oinkstep", "Oinkstep"),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Voice {
    Piano,
    Pad,
    Pluck,
    Square,
    Flute,
    Bell,
    Organ,
}

/// How a disc goes.
struct Style {
    bpm: f32,
    /// The key's root note (Hz).
    root: f32,
    minor: bool,
    /// Chords, as scale degrees (0 is the root), one per bar.
    prog: [i32; 4],
    /// Beats in a bar (3: a waltz).
    beats: u32,
    lead: Voice,
    chords: Voice,
    bass: Voice,
    /// 0: none, 1: light, 2: a full kit, 3: heavy.
    drums: u8,
    /// Late off-beats (0: straight).
    swing: f32,
    /// Seventh chords and a walking bass.
    jazz: bool,
    /// Drums wait for the second section.
    late_drums: bool,
}

fn style(n: usize) -> Style {
    let s = |bpm, root, minor, prog, beats, lead, chords, bass, drums, swing| Style { bpm, root, minor, prog, beats, lead, chords, bass, drums, swing, jazz: false, late_drums: false };
    match n {
        0 => s(112.0, 261.6, false, [0, 5, 3, 4], 4, Voice::Piano, Voice::Pad, Voice::Pluck, 1, 0.15),
        1 => s(124.0, 196.0, false, [0, 4, 5, 3], 4, Voice::Organ, Voice::Pluck, Voice::Pluck, 2, 0.0),
        2 => s(140.0, 220.0, false, [0, 3, 4, 0], 4, Voice::Square, Voice::Square, Voice::Square, 2, 0.0),
        3 => s(72.0, 146.8, true, [0, 5, 2, 6], 4, Voice::Bell, Voice::Pad, Voice::Pad, 0, 0.0),
        4 => Style { jazz: true, ..s(96.0, 174.6, false, [1, 4, 0, 5], 4, Voice::Flute, Voice::Piano, Voice::Pluck, 1, 0.3) },
        5 => s(132.0, 164.8, true, [0, 3, 4, 0], 3, Voice::Piano, Voice::Piano, Voice::Pluck, 0, 0.0),
        6 => Style { late_drums: true, ..s(90.0, 146.8, false, [0, 4, 5, 3], 4, Voice::Piano, Voice::Pad, Voice::Pluck, 1, 0.0) },
        _ => s(104.0, 220.0, true, [0, 0, 5, 6], 4, Voice::Square, Voice::Organ, Voice::Pluck, 3, 0.1),
    }
}

/// A note in a melody: when (in eighths from the bar's start), how long, and
/// which scale step above the chord's root.
#[derive(Clone, Copy, Debug)]
struct Step {
    at: u32,
    len: u32,
    deg: i32,
}

/// Two bars of tune: mostly chord tones on the beat, passing notes between.
fn motif(rng: &mut Rng, eighths: u32) -> Vec<Vec<Step>> {
    (0..2)
        .map(|bar| {
            let mut out = Vec::new();
            let mut at = 0;
            while at < eighths {
                let len = *[1, 1, 2, 2, 2, 3, 4].get(rng.int(0, 6) as usize).unwrap_or(&2);
                let len = len.min(eighths - at);
                let on_beat = at % 2 == 0;
                let deg = if on_beat { [0, 2, 4, 7][rng.int(0, 3) as usize] } else { rng.int(-1, 5) };
                // The second bar ends on something restful.
                let deg = if bar == 1 && at + len >= eighths { [0, 2, 4][rng.int(0, 2) as usize] } else { deg };
                if rng.chance(0.85) || at == 0 {
                    out.push(Step { at, len, deg });
                }
                at += len;
            }
            out
        })
        .collect()
}

fn play(v: &mut [f32], voice: Voice, t: f32, len: f32, f: f32, gain: f32, rng: &mut Rng) {
    match voice {
        Voice::Piano => piano(v, t, len.max(0.6), f, gain),
        Voice::Pad => pad(v, t, len, f, gain * 0.8),
        Voice::Pluck => pluck(v, t, len.max(0.4), f, gain, 0.5, rng),
        Voice::Square => square(v, t, len, f, gain * 0.45),
        Voice::Flute => flute(v, t, len, f, gain, rng),
        Voice::Bell => bell(v, t, len.max(1.0), f, gain * 0.7),
        Voice::Organ => organ(v, t, len, f, gain),
    }
}

/// Compose and play disc `n` (0..8): about fifty seconds of music.
pub fn synth_disc(n: usize) -> Vec<f32> {
    let st = style(n);
    let mut rng = Rng::new(0xD15C ^ (n as u64 * 7919));
    let beat = 60.0 / st.bpm;
    let eighths = st.beats * 2;
    let bar = beat * st.beats as f32;
    // Intro, A A B A', outro: 20 bars (short bars play the middle twice).
    let mut sections: Vec<(u8, u32)> = vec![(0, 2), (1, 4), (1, 4), (2, 4), (3, 4)];
    if bar * 20.0 < 40.0 {
        sections.extend([(2, 4), (3, 4)]);
    }
    sections.push((4, 2));
    let total_bars: u32 = sections.iter().map(|s| s.1).sum();
    let len = total_bars as f32 * bar + 4.0;
    let mut v = vec![0.0f32; samples(len)];
    let scale: [i32; 7] = if st.minor { [0, 2, 3, 5, 7, 8, 10] } else { [0, 2, 4, 5, 7, 9, 11] };
    let freq = |deg: i32, octave: i32| -> f32 {
        let st_ = scale[deg.rem_euclid(7) as usize] + deg.div_euclid(7) * 12 + octave * 12;
        st.root * 2f32.powf(st_ as f32 / 12.0)
    };
    let a = motif(&mut rng, eighths);
    let b = motif(&mut rng, eighths);
    let swing = |e: u32| if e % 2 == 1 { st.swing * beat * 0.5 } else { 0.0 };
    let mut bar_i = 0u32;
    for (kind, bars) in sections {
        for k in 0..bars {
            let t0 = bar_i as f32 * bar + 0.5;
            let chord = st.prog[(bar_i % 4) as usize];
            let tones: Vec<i32> = if st.jazz { vec![chord, chord + 2, chord + 4, chord + 6] } else { vec![chord, chord + 2, chord + 4] };
            // Chords.
            if st.beats == 3 {
                for b_ in 1..3 {
                    for &d in &tones {
                        play(&mut v, st.chords, t0 + b_ as f32 * beat, beat * 0.9, freq(d, 0), 0.12, &mut rng);
                    }
                }
            } else if st.chords == Voice::Square {
                // Chip arpeggios: the chord tones in quick turns.
                for e in 0..(eighths * 2) {
                    let d = tones[(e as usize) % tones.len()];
                    play(&mut v, st.chords, t0 + e as f32 * beat * 0.25, beat * 0.22, freq(d, 1), 0.08, &mut rng);
                }
            } else {
                for (j, &d) in tones.iter().enumerate() {
                    play(&mut v, st.chords, t0 + j as f32 * 0.02, bar * 0.95, freq(d, 0), 0.11, &mut rng);
                }
            }
            // Bass: the root (walking through the chord when jazzy).
            if st.jazz {
                for b_ in 0..st.beats {
                    let d = tones[(b_ as usize) % tones.len()];
                    play(&mut v, st.bass, t0 + b_ as f32 * beat, beat * 0.9, freq(d, -2), 0.35, &mut rng);
                }
            } else {
                let hits: &[u32] = if st.beats == 3 { &[0] } else { &[0, 2] };
                for &b_ in hits {
                    play(&mut v, st.bass, t0 + b_ as f32 * beat, beat * 1.8, freq(chord, -2), 0.4, &mut rng);
                }
            }
            // Drums (not in the intro or outro).
            let drums_now = st.drums > 0 && (1..4).contains(&kind) && !(st.late_drums && kind == 1);
            if drums_now {
                for e in 0..eighths {
                    let t = t0 + e as f32 * beat * 0.5 + swing(e);
                    let on_beat = e % 2 == 0;
                    let beat_no = e / 2;
                    if on_beat && (beat_no == 0 || (st.drums >= 2 && beat_no == 2)) {
                        kick(&mut v, t, 120.0, 0.6);
                    }
                    if st.drums == 3 && e == 3 {
                        kick(&mut v, t, 120.0, 0.5);
                    }
                    if st.drums >= 2 && on_beat && beat_no % 2 == 1 {
                        snare(&mut v, t, 0.35, &mut rng);
                    }
                    if st.drums == 1 && on_beat && beat_no % 2 == 1 {
                        hat(&mut v, t, 0.25, &mut rng);
                    }
                    if st.drums >= 2 || !on_beat {
                        hat(&mut v, t, 0.12, &mut rng);
                    }
                }
            }
            // The tune.
            let tune = match kind {
                1 => Some(&a),
                2 => Some(&b),
                3 => Some(&a),
                _ => None,
            };
            if let Some(m) = tune {
                for s in &m[(k % 2) as usize] {
                    let mut deg = chord + s.deg;
                    if kind == 3 && rng.chance(0.25) {
                        // A' changes a few notes.
                        deg += if rng.chance(0.5) { 1 } else { -1 };
                    }
                    let t = t0 + s.at as f32 * beat * 0.5 + swing(s.at);
                    play(&mut v, st.lead, t, s.len as f32 * beat * 0.5 * 0.95, freq(deg, 1), 0.32, &mut rng);
                }
            }
            if kind == 4 && k == bars - 1 {
                // The last chord rings out.
                for &d in &tones {
                    play(&mut v, st.chords, t0 + bar, 3.0, freq(d, 0), 0.12, &mut rng);
                }
                play(&mut v, st.lead, t0 + bar, 3.0, freq(chord, 1), 0.25, &mut rng);
            }
            bar_i += 1;
        }
    }
    reverb(&mut v, if n == 3 { 0.7 } else { 0.35 }, if n == 3 { 2.0 } else { 1.2 });
    let total = v.len();
    let fade = samples(2.0);
    for i in 0..fade.min(total) {
        v[total - 1 - i] *= i as f32 / fade as f32;
    }
    finish(v, 0.8)
}

#[cfg(test)]
mod songs_tests {
    use super::*;

    #[test]
    fn instruments_follow_the_block_underneath() {
        assert_eq!(Instrument::under(PLANKS), Instrument::Bass);
        assert_eq!(Instrument::under(STONE), Instrument::Drum);
        assert_eq!(Instrument::under(SAND), Instrument::Snare);
        assert_eq!(Instrument::under(GLASS), Instrument::Hat);
        assert_eq!(Instrument::under(ICE), Instrument::Chime);
        assert_eq!(Instrument::under(GRASS), Instrument::Harp);
        assert!((note_freq(Instrument::Harp, 12) / note_freq(Instrument::Harp, 0) - 2.0).abs() < 1e-3, "twelve steps up is an octave");
        for inst in INSTRUMENTS {
            assert_eq!(Instrument::from_index(inst.index()), Some(inst));
        }
    }

    #[test]
    fn notes_and_discs_make_sound() {
        for inst in INSTRUMENTS {
            let v = synth_note(inst, 12);
            assert!(v.iter().all(|x| x.is_finite()));
            assert!(v.iter().any(|x| x.abs() > 0.1), "{inst:?} is audible");
        }
        for n in [0, 4, 5] {
            let v = synth_disc(n);
            let secs = v.len() as f32 / SR as f32;
            assert!((35.0..80.0).contains(&secs), "disc {n} lasts {secs}s");
            assert!(v.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
            assert!(v.iter().skip(SR as usize * 5).take(SR as usize * 10).any(|x| x.abs() > 0.1), "disc {n} has music in it");
        }
    }
}
