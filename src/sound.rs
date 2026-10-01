//! Procedural sound effects and music. Like the textures, no audio files ship:
//! every sound is synthesised into an in-memory WAV at startup.

use crate::block::*;
use crate::noise::Rng;
use macroquad::audio::{load_sound_from_bytes, play_sound, set_sound_volume, stop_sound, PlaySoundParams, Sound};
use macroquad::math::Vec3;
use std::f32::consts::TAU;
use std::sync::atomic::{AtomicBool, Ordering};

const SR: u32 = 22050;

/// Set when the platform audio thread dies (e.g. no sound device); we then stay silent.
pub static AUDIO_DEAD: AtomicBool = AtomicBool::new(false);

/// Linux: quad-snd's ALSA thread panics (before `main` even runs) when there is no
/// sound card, and every later sound call then prints an error. Ask ALSA up front instead.
#[cfg(all(feature = "sound", target_os = "linux"))]
fn has_output_device() -> bool {
    let mut card: std::os::raw::c_int = -1;
    // Only enumerates cards; doesn't open (and so can't fight quad-snd over) a device.
    unsafe { quad_alsa_sys::snd_card_next(&mut card) >= 0 && card >= 0 }
}

#[cfg(not(all(feature = "sound", target_os = "linux")))]
fn has_output_device() -> bool {
    cfg!(feature = "sound")
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mat {
    Stone,
    Wood,
    Grass,
    Sand,
    Glass,
}

pub fn material(block_id: Id) -> Mat {
    match block(block_id).sound {
        0 => Mat::Stone,
        1 => Mat::Wood,
        3 => Mat::Sand,
        4 => Mat::Glass,
        _ => Mat::Grass,
    }
}

impl Sfx {
    /// Wire encoding for multiplayer.
    pub fn to_u8(self) -> u8 {
        all_sfx().iter().position(|s| *s == self).unwrap_or(0) as u8
    }
    pub fn from_u8(v: u8) -> Option<Sfx> {
        all_sfx().get(v as usize).copied()
    }
    /// What a mob sounds like when hit.
    pub fn hurt_of(kind: crate::entity::MobKind) -> Sfx {
        match kind {
            crate::entity::MobKind::Oinker => Sfx::Oink,
            crate::entity::MobKind::Fluffer => Sfx::Baa,
            crate::entity::MobKind::Cluckster => Sfx::Cluck,
            crate::entity::MobKind::Squawker => Sfx::Squawk,
            crate::entity::MobKind::Mooer => Sfx::Moo,
            crate::entity::MobKind::Rattler => Sfx::Rattle,
            crate::entity::MobKind::Bloop => Sfx::Bloop,
            crate::entity::MobKind::Woofer => Sfx::Woof,
            crate::entity::MobKind::Hmmer => Sfx::Hmm,
            crate::entity::MobKind::Grumbler => Sfx::Oink,
            crate::entity::MobKind::Bee => Sfx::Buzz,
            crate::entity::MobKind::Sneaker => Sfx::Yip,
            crate::entity::MobKind::Ribbit => Sfx::Croak,
            crate::entity::MobKind::Rollo => Sfx::Scuttle,
            crate::entity::MobKind::Hush => Sfx::Roar,
            _ => Sfx::MobHurt,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sfx {
    /// Block broken.
    Break(Mat),
    /// Hitting a block while mining it.
    Hit(Mat),
    Place(Mat),
    Step(Mat),
    Hurt,
    MobHurt,
    Oink,
    Hiss,
    Groan,
    Explode,
    Eat,
    Pop,
    Click,
    Splash,
    Craft,
    Thud,
    Baa,
    /// A Starer (or a player with a Stare Pearl) teleporting.
    Warp,
    /// "Advancement Made!" fanfare.
    Fanfare,
    Boing,
    Cluck,
    Moo,
    /// Rattler bones clattering.
    Rattle,
    /// Webber legs.
    Skitter,
    Bloop,
    /// A bow (or a Rattler) firing.
    Twang,
    /// An arrow hitting something.
    Thunk,
    /// Lightning: a crack, then a long rumble.
    Thunder,
    /// An enchanting table doing its thing.
    Chime,
    /// A Woofer.
    Woof,
    /// Shears.
    Snip,
    /// A Hmmer, hmming.
    Hmm,
    /// A Squawker, squawking.
    Squawk,
    /// Bees.
    Buzz,
    /// A brush scraping at suspicious sand (see archaeology.rs).
    Brush,
    /// A sculk sensor clicking (see deepdark.rs).
    Sculk,
    /// A sculk shrieker.
    Shriek,
    /// The Hush, roaring.
    Roar,
    /// The Hush's shush: a blast of sound through walls.
    Shush,
    /// A grindstone.
    Grind,
    /// A Sneaker (fox).
    Yip,
    /// A Ribbit (frog).
    Croak,
    /// A Rollo (armadillo) scuttling or rolling up.
    Scuttle,
}

// ---------------------------------------------------------------- synthesis

struct Lp {
    y: f32,
}
impl Lp {
    fn new() -> Self {
        Lp { y: 0.0 }
    }
    fn run(&mut self, x: f32, fc: f32) -> f32 {
        let a = 1.0 - (-TAU * fc / SR as f32).exp();
        self.y += a * (x - self.y);
        self.y
    }
}

fn samples(secs: f32) -> usize {
    (secs * SR as f32) as usize
}

fn t_of(i: usize) -> f32 {
    i as f32 / SR as f32
}

/// Noise burst through a band of low/high-pass filters with exponential decay.
fn burst(out: &mut [f32], start: f32, len: f32, decay: f32, lo: f32, hi: f32, gain: f32, rng: &mut Rng) {
    let s0 = samples(start);
    let (mut lp, mut hp) = (Lp::new(), Lp::new());
    for i in 0..samples(len) {
        if s0 + i >= out.len() {
            break;
        }
        let t = t_of(i);
        let x = rng.range(-1.0, 1.0);
        let y = lp.run(x, hi);
        let band = y - hp.run(y, lo);
        let env = (t / 0.002).min(1.0) * (-t * decay).exp();
        out[s0 + i] += band * env * gain;
    }
}

fn tone(out: &mut [f32], start: f32, len: f32, f0: f32, f1: f32, decay: f32, gain: f32, harmonics: &[f32]) {
    let s0 = samples(start);
    let mut phase = 0.0f32;
    let n = samples(len);
    for i in 0..n {
        if s0 + i >= out.len() {
            break;
        }
        let t = t_of(i);
        let f = f0 + (f1 - f0) * (i as f32 / n as f32);
        phase += TAU * f / SR as f32;
        let mut v = 0.0;
        for (h, a) in harmonics.iter().enumerate() {
            v += (phase * (h + 1) as f32).sin() * a;
        }
        let env = (t / 0.005).min(1.0) * (-t * decay).exp();
        out[s0 + i] += v * env * gain;
    }
}

/// Bandlimited-ish "voice": a saw run through a formant lowpass.
fn voice(out: &mut [f32], start: f32, len: f32, f0: f32, f1: f32, formant: f32, vibrato: f32, gain: f32, rng: &mut Rng) {
    let s0 = samples(start);
    let n = samples(len);
    let mut phase = 0.0f32;
    let (mut lp, mut lp2) = (Lp::new(), Lp::new());
    for i in 0..n {
        if s0 + i >= out.len() {
            break;
        }
        let t = t_of(i);
        let k = i as f32 / n as f32;
        let f = (f0 + (f1 - f0) * k) * (1.0 + (t * 6.0 * TAU).sin() * vibrato);
        phase = (phase + f / SR as f32) % 1.0;
        let saw = phase * 2.0 - 1.0 + rng.range(-0.15, 0.15);
        let y = lp2.run(lp.run(saw, formant), formant * 1.3);
        let env = (k / 0.08).min(1.0) * ((1.0 - k) / 0.25).min(1.0);
        out[s0 + i] += y * env * gain;
    }
}

fn finish(mut v: Vec<f32>, peak: f32) -> Vec<f32> {
    let m = v.iter().fold(0.0f32, |a, x| a.max(x.abs())).max(1e-6);
    for x in v.iter_mut() {
        *x = (*x / m * peak).tanh();
    }
    v
}

fn wav(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut b = Vec::with_capacity(44 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes()); // PCM
    b.extend_from_slice(&1u16.to_le_bytes()); // mono
    b.extend_from_slice(&SR.to_le_bytes());
    b.extend_from_slice(&(SR * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        b.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32000.0) as i16).to_le_bytes());
    }
    b
}

fn synth_material(m: Mat, kind: u8, rng: &mut Rng) -> Vec<f32> {
    // kind: 0 = break, 1 = hit, 2 = place, 3 = step
    let scale: f32 = [1.0, 0.45, 0.6, 0.4][kind as usize];
    let mut v = vec![0.0; samples(0.4 * scale.max(0.5))];
    let p = rng.range(0.85, 1.15);
    match m {
        Mat::Stone => {
            burst(&mut v, 0.0, 0.12, 45.0, 400.0 * p, 3200.0 * p, 1.0, rng);
            if kind == 0 {
                burst(&mut v, 0.035, 0.12, 35.0, 250.0, 2000.0 * p, 0.8, rng);
                burst(&mut v, 0.08, 0.15, 30.0, 150.0, 1400.0, 0.5, rng);
            }
        }
        Mat::Wood => {
            tone(&mut v, 0.0, 0.15, 190.0 * p, 150.0 * p, 32.0, 0.9, &[1.0, 0.5, 0.25]);
            burst(&mut v, 0.0, 0.1, 50.0, 200.0, 1100.0 * p, 0.6, rng);
            if kind == 0 {
                tone(&mut v, 0.05, 0.2, 150.0 * p, 110.0 * p, 22.0, 0.7, &[1.0, 0.4]);
            }
        }
        Mat::Grass => {
            let grains = if kind == 0 { 7 } else { 3 };
            for g in 0..grains {
                let at = g as f32 * 0.025 + rng.range(0.0, 0.015);
                burst(&mut v, at, 0.07, 55.0, 300.0, 1800.0 * p, rng.range(0.4, 1.0), rng);
            }
        }
        Mat::Sand => {
            let grains = if kind == 0 { 10 } else { 4 };
            for g in 0..grains {
                let at = g as f32 * 0.02 + rng.range(0.0, 0.01);
                burst(&mut v, at, 0.05, 70.0, 900.0, 4200.0 * p, rng.range(0.3, 0.9), rng);
            }
        }
        Mat::Glass => {
            if kind == 0 {
                burst(&mut v, 0.0, 0.08, 60.0, 2000.0, 7000.0, 0.8, rng);
                for _ in 0..7 {
                    let f = rng.range(2200.0, 5200.0);
                    tone(&mut v, rng.range(0.0, 0.06), 0.3, f, f * 0.98, rng.range(12.0, 25.0), 0.35, &[1.0]);
                }
            } else {
                tone(&mut v, 0.0, 0.1, 1800.0 * p, 1700.0, 40.0, 0.5, &[1.0, 0.3]);
                burst(&mut v, 0.0, 0.05, 80.0, 1500.0, 5000.0, 0.5, rng);
            }
        }
    }
    finish(v, 1.4)
}

fn synth(s: Sfx, rng: &mut Rng) -> Vec<f32> {
    match s {
        Sfx::Break(m) => synth_material(m, 0, rng),
        Sfx::Hit(m) => synth_material(m, 1, rng),
        Sfx::Place(m) => synth_material(m, 2, rng),
        Sfx::Step(m) => synth_material(m, 3, rng),
        Sfx::Hurt => {
            // The famous-ish "oof", legally distinct edition.
            let mut v = vec![0.0; samples(0.3)];
            let p = rng.range(0.95, 1.05);
            voice(&mut v, 0.0, 0.26, 250.0 * p, 150.0 * p, 900.0, 0.02, 1.0, rng);
            finish(v, 1.1)
        }
        Sfx::MobHurt => {
            let mut v = vec![0.0; samples(0.25)];
            let p = rng.range(0.9, 1.1);
            voice(&mut v, 0.0, 0.2, 380.0 * p, 260.0 * p, 1300.0, 0.03, 1.0, rng);
            burst(&mut v, 0.0, 0.05, 60.0, 300.0, 2500.0, 0.4, rng);
            finish(v, 1.1)
        }
        Sfx::Oink => {
            let mut v = vec![0.0; samples(0.45)];
            let p = rng.range(0.9, 1.15);
            voice(&mut v, 0.0, 0.14, 150.0 * p, 125.0 * p, 1100.0, 0.08, 1.0, rng);
            voice(&mut v, 0.19, 0.18, 165.0 * p, 115.0 * p, 1000.0, 0.1, 0.9, rng);
            burst(&mut v, 0.0, 0.14, 25.0, 400.0, 1400.0, 0.25, rng);
            burst(&mut v, 0.19, 0.18, 25.0, 400.0, 1400.0, 0.25, rng);
            finish(v, 1.1)
        }
        Sfx::Hiss => {
            let n = samples(1.5);
            let mut v = vec![0.0; n];
            let (mut lp, mut hp) = (Lp::new(), Lp::new());
            for (i, x) in v.iter_mut().enumerate() {
                let k = i as f32 / n as f32;
                let y = lp.run(rng.range(-1.0, 1.0), 7000.0);
                let band = y - hp.run(y, 2500.0);
                *x = band * k.powf(0.7) * ((1.0 - k) / 0.03).min(1.0);
            }
            finish(v, 1.1)
        }
        Sfx::Groan => {
            let mut v = vec![0.0; samples(1.3)];
            let p = rng.range(0.85, 1.1);
            voice(&mut v, 0.0, 1.25, 95.0 * p, 70.0 * p, 520.0, 0.06, 1.0, rng);
            voice(&mut v, 0.0, 1.25, 142.0 * p, 104.0 * p, 700.0, 0.05, 0.4, rng);
            finish(v, 1.1)
        }
        Sfx::Thunder => {
            let n = samples(4.0);
            let mut v = vec![0.0; n];
            let (mut lp, mut lp2, mut crack) = (Lp::new(), Lp::new(), Lp::new());
            let wobble = rng.range(1.5, 3.0);
            for (i, x) in v.iter_mut().enumerate() {
                let t = t_of(i);
                let snap = crack.run(rng.range(-1.0, 1.0), 6000.0) * (-t * 18.0).exp() * 1.4;
                let fc = 120.0 + 500.0 * (-t * 1.5).exp();
                let rumble = lp2.run(lp.run(rng.range(-1.0, 1.0), fc), fc) * 4.0;
                let swell = (0.6 + 0.4 * (t * wobble * TAU).sin()) * (-t * 0.9).exp();
                *x = (snap + rumble * swell) * (t / 0.003).min(1.0);
            }
            finish(v, 2.0)
        }
        Sfx::Chime => {
            let n = samples(1.6);
            let mut v = vec![0.0; n];
            for (k, f) in [880.0f32, 1318.5, 1760.0, 2637.0].into_iter().enumerate() {
                let start = k as f32 * 0.07;
                for (i, x) in v.iter_mut().enumerate() {
                    let t = t_of(i) - start;
                    if t > 0.0 {
                        *x += (t * f * TAU).sin() * (-t * 3.0).exp() * 0.3;
                    }
                }
            }
            finish(v, 1.0)
        }
        Sfx::Explode => {
            let n = samples(2.0);
            let mut v = vec![0.0; n];
            let mut lp = Lp::new();
            let mut lp2 = Lp::new();
            let mut phase = 0.0f32;
            for (i, x) in v.iter_mut().enumerate() {
                let t = t_of(i);
                let fc = 200.0 + 3500.0 * (-t * 4.0).exp();
                let rumble = lp2.run(lp.run(rng.range(-1.0, 1.0), fc), fc * 1.2) * 3.0;
                let f = 30.0 + 35.0 * (-t * 5.0).exp();
                phase += TAU * f / SR as f32;
                let thump = phase.sin() * (-t * 5.0).exp() * 1.2;
                *x = (rumble * (-t * 2.2).exp() + thump) * (t / 0.004).min(1.0);
            }
            finish(v, 2.5)
        }
        Sfx::Eat => {
            let mut v = vec![0.0; samples(0.6)];
            for k in 0..3 {
                let at = k as f32 * 0.17 + rng.range(0.0, 0.02);
                burst(&mut v, at, 0.1, 30.0, 500.0, 2400.0, 1.0, rng);
                burst(&mut v, at + 0.02, 0.06, 50.0, 1200.0, 5000.0, 0.4, rng);
            }
            finish(v, 1.3)
        }
        Sfx::Pop => {
            let mut v = vec![0.0; samples(0.12)];
            let p = rng.range(0.9, 1.2);
            tone(&mut v, 0.0, 0.1, 520.0 * p, 1250.0 * p, 30.0, 1.0, &[1.0, 0.2]);
            finish(v, 1.0)
        }
        Sfx::Click => {
            let mut v = vec![0.0; samples(0.05)];
            tone(&mut v, 0.0, 0.04, 1300.0, 900.0, 90.0, 1.0, &[1.0, 0.3]);
            burst(&mut v, 0.0, 0.01, 300.0, 1000.0, 6000.0, 0.4, rng);
            finish(v, 0.9)
        }
        Sfx::Splash => {
            let mut v = vec![0.0; samples(0.7)];
            burst(&mut v, 0.0, 0.6, 7.0, 300.0, 2600.0, 1.0, rng);
            for _ in 0..6 {
                let f = rng.range(350.0, 800.0);
                tone(&mut v, rng.range(0.05, 0.4), 0.08, f, f * 1.8, 30.0, 0.35, &[1.0]);
            }
            finish(v, 1.3)
        }
        Sfx::Craft => {
            let mut v = vec![0.0; samples(0.3)];
            tone(&mut v, 0.0, 0.12, 523.0, 523.0, 25.0, 1.0, &[1.0, 0.3, 0.1]);
            tone(&mut v, 0.1, 0.18, 784.0, 784.0, 18.0, 1.0, &[1.0, 0.3, 0.1]);
            finish(v, 0.9)
        }
        Sfx::Thud => {
            let mut v = vec![0.0; samples(0.25)];
            tone(&mut v, 0.0, 0.2, 90.0, 40.0, 20.0, 1.0, &[1.0, 0.3]);
            burst(&mut v, 0.0, 0.1, 40.0, 60.0, 500.0, 0.8, rng);
            finish(v, 1.6)
        }
        Sfx::Baa => {
            // Heavy vibrato is what makes it a bleat rather than a moan.
            let mut v = vec![0.0; samples(0.7)];
            let p = rng.range(0.9, 1.15);
            voice(&mut v, 0.0, 0.65, 310.0 * p, 270.0 * p, 1500.0, 0.09, 1.0, rng);
            voice(&mut v, 0.0, 0.65, 620.0 * p, 540.0 * p, 2200.0, 0.09, 0.3, rng);
            finish(v, 1.1)
        }
        Sfx::Warp => {
            let mut v = vec![0.0; samples(0.6)];
            let p = rng.range(0.9, 1.1);
            tone(&mut v, 0.0, 0.5, 900.0 * p, 120.0 * p, 5.0, 0.8, &[1.0, 0.5, 0.3]);
            tone(&mut v, 0.0, 0.5, 1210.0 * p, 160.0 * p, 5.0, 0.4, &[1.0]);
            burst(&mut v, 0.0, 0.3, 10.0, 800.0, 5000.0, 0.3, rng);
            finish(v, 1.0)
        }
        Sfx::Fanfare => {
            // Da-da-da-DAAA. Deeply original.
            let mut v = vec![0.0; samples(1.1)];
            let h = [1.0, 0.45, 0.25, 0.12];
            for (i, f) in [523.0, 659.0, 784.0].iter().enumerate() {
                tone(&mut v, i as f32 * 0.1, 0.18, *f, *f, 14.0, 0.8, &h);
            }
            tone(&mut v, 0.3, 0.8, 1047.0, 1047.0, 3.5, 1.0, &h);
            tone(&mut v, 0.3, 0.8, 784.0, 784.0, 3.5, 0.5, &h);
            finish(v, 0.9)
        }
        Sfx::Boing => {
            let mut v = vec![0.0; samples(0.4)];
            let p = rng.range(0.9, 1.1);
            tone(&mut v, 0.0, 0.38, 180.0 * p, 520.0 * p, 7.0, 1.0, &[1.0, 0.3]);
            finish(v, 1.0)
        }
        Sfx::Cluck => {
            // Three quick, pinched bawks.
            let mut v = vec![0.0; samples(0.45)];
            let p = rng.range(0.9, 1.15);
            for k in 0..3 {
                let at = k as f32 * 0.12 + rng.range(0.0, 0.02);
                voice(&mut v, at, 0.07, 700.0 * p, 520.0 * p, 2400.0, 0.02, 1.0, rng);
            }
            finish(v, 1.1)
        }
        Sfx::Squawk => {
            // Two harsh, high screeches, falling.
            let mut v = vec![0.0; samples(0.5)];
            let p = rng.range(0.9, 1.2);
            for k in 0..2 {
                voice(&mut v, k as f32 * 0.2 + rng.range(0.0, 0.03), 0.14, 1500.0 * p, 900.0 * p, 3600.0, 0.08, 1.0, rng);
            }
            finish(v, 1.0)
        }
        Sfx::Buzz => {
            // A wobbly drone around 220 Hz, rising and falling.
            let mut v = vec![0.0; samples(0.9)];
            let p = rng.range(0.9, 1.1);
            voice(&mut v, 0.0, 0.85, 210.0 * p, 240.0 * p, 1800.0, 0.06, 0.8, rng);
            voice(&mut v, 0.05, 0.8, 235.0 * p, 205.0 * p, 1500.0, 0.05, 0.5, rng);
            finish(v, 0.9)
        }
        Sfx::Brush => {
            // Soft bristly scratches.
            let mut v = vec![0.0; samples(0.35)];
            for k in 0..3 {
                burst(&mut v, k as f32 * 0.1 + rng.range(0.0, 0.02), 0.09, 25.0, 1500.0, 6000.0, 0.5, rng);
            }
            finish(v, 0.8)
        }
        Sfx::Sculk => {
            // A dry click and a hollow ping.
            let mut v = vec![0.0; samples(0.6)];
            burst(&mut v, 0.0, 0.03, 90.0, 800.0, 4000.0, 0.8, rng);
            tone(&mut v, 0.02, 0.5, 520.0, 500.0, 6.0, 0.4, &[1.0, 0.0, 0.3]);
            finish(v, 0.9)
        }
        Sfx::Shriek => {
            // A rising, ghostly wail.
            let mut v = vec![0.0; samples(1.4)];
            voice(&mut v, 0.0, 1.3, 400.0, 1300.0, 3000.0, 0.03, 1.0, rng);
            voice(&mut v, 0.1, 1.2, 600.0, 1700.0, 3500.0, 0.04, 0.6, rng);
            finish(v, 1.0)
        }
        Sfx::Roar => {
            // Deep and long.
            let mut v = vec![0.0; samples(1.6)];
            voice(&mut v, 0.0, 1.5, 70.0, 55.0, 500.0, 0.05, 1.0, rng);
            burst(&mut v, 0.0, 1.4, 1.5, 60.0, 400.0, 0.5, rng);
            finish(v, 1.1)
        }
        Sfx::Shush => {
            // A charging hum, then a whoomp.
            let mut v = vec![0.0; samples(1.2)];
            tone(&mut v, 0.0, 0.7, 120.0, 480.0, 0.5, 0.5, &[1.0, 0.5, 0.25]);
            burst(&mut v, 0.7, 0.5, 6.0, 40.0, 900.0, 1.2, rng);
            tone(&mut v, 0.7, 0.45, 90.0, 40.0, 5.0, 0.9, &[1.0, 0.4]);
            finish(v, 1.1)
        }
        Sfx::Grind => {
            // Stone on metal: gritty, with a squeal.
            let mut v = vec![0.0; samples(0.6)];
            burst(&mut v, 0.0, 0.55, 3.0, 400.0, 3000.0, 0.7, rng);
            tone(&mut v, 0.05, 0.4, 1900.0, 2100.0, 4.0, 0.15, &[1.0]);
            finish(v, 0.9)
        }
        Sfx::Yip => {
            // Two high, sharp yips.
            let mut v = vec![0.0; samples(0.4)];
            let p = rng.range(0.9, 1.15);
            for k in 0..2 {
                voice(&mut v, k as f32 * 0.16, 0.08, 900.0 * p, 1200.0 * p, 3000.0, 0.02, 1.0, rng);
            }
            finish(v, 1.0)
        }
        Sfx::Croak => {
            // A rubbery "rib-bit".
            let mut v = vec![0.0; samples(0.5)];
            let p = rng.range(0.85, 1.15);
            voice(&mut v, 0.0, 0.12, 160.0 * p, 140.0 * p, 700.0, 0.2, 1.0, rng);
            voice(&mut v, 0.2, 0.16, 190.0 * p, 120.0 * p, 800.0, 0.25, 1.0, rng);
            finish(v, 1.0)
        }
        Sfx::Scuttle => {
            // Tiny claws, then a clack as the shell closes.
            let mut v = vec![0.0; samples(0.4)];
            for k in 0..4 {
                burst(&mut v, k as f32 * 0.05, 0.02, 120.0, 2000.0, 7000.0, 0.4, rng);
            }
            burst(&mut v, 0.25, 0.05, 60.0, 300.0, 2500.0, 0.9, rng);
            finish(v, 0.9)
        }
        Sfx::Woof => {
            // Two short barks.
            let mut v = vec![0.0; samples(0.5)];
            let p = rng.range(0.9, 1.15);
            for k in 0..2 {
                voice(&mut v, k as f32 * 0.2, 0.12, 330.0 * p, 220.0 * p, 1600.0, 0.01, 1.0, rng);
            }
            finish(v, 1.1)
        }
        Sfx::Snip => {
            let mut v = vec![0.0; samples(0.3)];
            for k in 0..2 {
                burst(&mut v, k as f32 * 0.12, 0.04, 60.0, 3000.0, 9000.0, 0.8, rng);
                tone(&mut v, k as f32 * 0.12, 0.03, 2400.0, 2000.0, 80.0, 0.3, &[1.0]);
            }
            finish(v, 1.0)
        }
        Sfx::Hmm => {
            // A thoughtful, nasal "hmm".
            let mut v = vec![0.0; samples(0.6)];
            let p = rng.range(0.85, 1.15);
            voice(&mut v, 0.0, 0.5, 180.0 * p, 150.0 * p, 900.0, 0.04, 1.0, rng);
            finish(v, 1.0)
        }
        Sfx::Moo => {
            let mut v = vec![0.0; samples(1.2)];
            let p = rng.range(0.9, 1.1);
            voice(&mut v, 0.0, 1.1, 110.0 * p, 85.0 * p, 450.0, 0.03, 1.0, rng);
            voice(&mut v, 0.0, 1.1, 220.0 * p, 170.0 * p, 700.0, 0.03, 0.35, rng);
            finish(v, 1.1)
        }
        Sfx::Rattle => {
            let mut v = vec![0.0; samples(0.4)];
            for _ in 0..9 {
                let f = rng.range(900.0, 1800.0);
                tone(&mut v, rng.range(0.0, 0.3), 0.05, f, f * 0.9, 60.0, 0.6, &[1.0, 0.5]);
                burst(&mut v, rng.range(0.0, 0.3), 0.03, 90.0, 1200.0, 5000.0, 0.4, rng);
            }
            finish(v, 1.1)
        }
        Sfx::Skitter => {
            let mut v = vec![0.0; samples(0.5)];
            for k in 0..12 {
                burst(&mut v, k as f32 * 0.035 + rng.range(0.0, 0.01), 0.02, 120.0, 2000.0, 7000.0, rng.range(0.3, 0.8), rng);
            }
            finish(v, 1.0)
        }
        Sfx::Bloop => {
            let mut v = vec![0.0; samples(0.3)];
            let p = rng.range(0.85, 1.2);
            tone(&mut v, 0.0, 0.25, 320.0 * p, 140.0 * p, 12.0, 1.0, &[1.0, 0.4]);
            burst(&mut v, 0.0, 0.08, 40.0, 200.0, 1200.0, 0.3, rng);
            finish(v, 1.1)
        }
        Sfx::Twang => {
            let mut v = vec![0.0; samples(0.4)];
            let p = rng.range(0.95, 1.05);
            tone(&mut v, 0.0, 0.35, 190.0 * p, 170.0 * p, 9.0, 1.0, &[1.0, 0.6, 0.4, 0.2]);
            burst(&mut v, 0.0, 0.05, 60.0, 1500.0, 6000.0, 0.5, rng);
            finish(v, 1.0)
        }
        Sfx::Thunk => {
            let mut v = vec![0.0; samples(0.2)];
            tone(&mut v, 0.0, 0.15, 260.0, 180.0, 30.0, 1.0, &[1.0, 0.4]);
            burst(&mut v, 0.0, 0.05, 70.0, 300.0, 2500.0, 0.6, rng);
            finish(v, 1.2)
        }
    }
}

/// A piano-ish note: bright attack, higher partials fading faster, a touch of
/// detune between strings.
fn piano(out: &mut [f32], start: f32, len: f32, f: f32, gain: f32) {
    let s0 = samples(start);
    let n = samples(len);
    let partials = [(1.0, 1.0, 1.0), (2.0, 0.45, 1.8), (3.0, 0.22, 2.6), (4.0, 0.1, 3.4), (5.0, 0.05, 4.2)];
    let (mut p1, mut p2) = ([0.0f32; 5], [0.0f32; 5]);
    for i in 0..n {
        if s0 + i >= out.len() {
            break;
        }
        let t = t_of(i);
        let mut v = 0.0;
        for (k, &(h, a, d)) in partials.iter().enumerate() {
            let fk = f * h * (1.0 + 0.0004 * h * h);
            p1[k] += TAU * fk / SR as f32;
            p2[k] += TAU * fk * 1.0015 / SR as f32;
            v += (p1[k].sin() + p2[k].sin()) * 0.5 * a * (-t * d).exp();
        }
        let attack = (t / 0.004).min(1.0);
        let release = ((len - t) / 0.3).clamp(0.0, 1.0);
        out[s0 + i] += v * attack * release * gain;
    }
}

/// A soft, slowly swelling pad (for chords under the melody).
fn pad(out: &mut [f32], start: f32, len: f32, f: f32, gain: f32) {
    let s0 = samples(start);
    let n = samples(len);
    let mut ph = [0.0f32; 3];
    for i in 0..n {
        if s0 + i >= out.len() {
            break;
        }
        let t = t_of(i);
        let k = i as f32 / n as f32;
        let mut v = 0.0;
        for (j, det) in [0.997f32, 1.0, 1.003].iter().enumerate() {
            ph[j] += TAU * f * det / SR as f32;
            v += ph[j].sin() + (ph[j] * 2.0).sin() * 0.15;
        }
        let env = (k / 0.35).min(1.0) * ((1.0 - k) / 0.4).min(1.0) * (1.0 + (t * 0.7 * TAU).sin() * 0.08);
        out[s0 + i] += v / 3.0 * env * gain;
    }
}

/// A small room/hall: parallel combs into series allpasses (Schroeder), mixed in.
fn reverb(v: &mut [f32], mix: f32, size: f32) {
    let dry = v.to_vec();
    let mut wet = vec![0.0f32; v.len()];
    for (d, g) in [(0.0297, 0.80), (0.0371, 0.78), (0.0411, 0.76), (0.0437, 0.74)] {
        let n = samples(d * size).max(1);
        let mut buf = vec![0.0f32; n];
        let mut lp = 0.0f32;
        for i in 0..v.len() {
            let y = buf[i % n];
            lp += 0.4 * (y - lp);
            buf[i % n] = dry[i] + lp * g;
            wet[i] += y * 0.25;
        }
    }
    for d in [0.005, 0.0017] {
        let n = samples(d * size).max(1);
        let mut buf = vec![0.0f32; n];
        for (i, x) in wet.iter_mut().enumerate() {
            let y = buf[i % n];
            let input = *x + y * 0.5;
            buf[i % n] = input;
            *x = y - input * 0.5;
        }
    }
    for (o, w) in v.iter_mut().zip(wet) {
        *o += w * mix;
    }
}

/// A music track's mood: which scale, how fast, what plays.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mood {
    /// Calm, meandering, pentatonic: the daytime tune.
    Day,
    /// Slow, minor, sparse, with long pads.
    Night,
    /// Rolling major arpeggios, a little brighter.
    Morning,
    /// Low, dark and far apart: caves and the other dimensions.
    Deep,
}

pub const MOODS: [Mood; 4] = [Mood::Day, Mood::Night, Mood::Morning, Mood::Deep];

/// One piece of music for a mood (different seeds, different tunes).
fn synth_music_mood(mood: Mood, seed: u64) -> Vec<f32> {
    let mut rng = Rng::new(seed ^ (mood as u64 * 0x9E37));
    let len = 60.0;
    let mut v = vec![0.0f32; samples(len)];
    let (scale, root, steps, chord_chance, low, high): (&[i32], f32, &[f32], f32, i32, i32) = match mood {
        Mood::Day => (&[0, 2, 4, 7, 9], 196.0, &[0.6, 0.9, 1.2, 1.8, 2.4], 0.3, 2, 11),
        Mood::Night => (&[0, 2, 3, 5, 7, 8, 10], 174.6, &[1.2, 1.8, 2.4, 3.6], 0.45, 3, 12),
        Mood::Morning => (&[0, 2, 4, 5, 7, 9, 11], 220.0, &[0.3, 0.3, 0.6, 0.6, 0.9], 0.2, 4, 14),
        Mood::Deep => (&[0, 1, 3, 7, 8], 98.0, &[2.4, 3.6, 4.8], 0.6, 0, 8),
    };
    let n = scale.len() as i32;
    let note = |deg: i32| -> f32 {
        let st = scale[deg.rem_euclid(n) as usize] + deg.div_euclid(n) * 12;
        root * 2f32.powf(st as f32 / 12.0)
    };
    let mut t = 1.0;
    let mut deg = (low + high) / 2;
    while t < len - 6.0 {
        let f = note(deg);
        match mood {
            Mood::Deep => pad(&mut v, t, 5.0, f, 0.35),
            _ => piano(&mut v, t, 4.0, f, 0.5),
        }
        if rng.chance(chord_chance) {
            // A soft chord underneath: the third and fifth below (in scale steps).
            for d in [deg - n, deg - n + 2, deg - n + 4] {
                pad(&mut v, t, 5.5, note(d), 0.1);
            }
        }
        if mood == Mood::Morning && rng.chance(0.35) {
            // A quick run upward.
            for (k, step) in [1, 2, 4].iter().enumerate() {
                piano(&mut v, t + 0.15 * (k + 1) as f32, 1.5, note(deg + step), 0.28);
            }
        }
        deg = (deg + rng.int(-2, 2)).clamp(low, high);
        t += steps[rng.int(0, steps.len() as i32 - 1) as usize];
    }
    reverb(&mut v, if mood == Mood::Deep { 0.8 } else { 0.45 }, if mood == Mood::Deep { 2.2 } else { 1.4 });
    let total = v.len();
    let fade = samples(5.0);
    for i in 0..fade {
        v[total - 1 - i] *= i as f32 / fade as f32;
    }
    finish(v, 0.8)
}

/// A daytime tune (for the tests).
#[cfg(test)]
fn synth_music(seed: u64) -> Vec<f32> {
    synth_music_mood(Mood::Day, seed)
}

/// Rain on everything: a loop of soft, filtered noise with the odd heavier drop.
fn synth_rain(rng: &mut Rng) -> Vec<f32> {
    let len = 4.0;
    let mut v = vec![0.0f32; samples(len)];
    let (mut lp, mut hp) = (Lp::new(), Lp::new());
    for (i, x) in v.iter_mut().enumerate() {
        let n = rng.range(-1.0, 1.0);
        let y = lp.run(n, 5000.0);
        let band = y - hp.run(y, 500.0);
        let t = t_of(i);
        *x = band * (0.8 + 0.2 * (t * 0.5 * TAU).sin());
    }
    for _ in 0..70 {
        let at = rng.range(0.0, len - 0.05);
        burst(&mut v, at, 0.04, 90.0, 1200.0, 6000.0, rng.range(0.4, 1.1), rng);
    }
    // Cross-fade the ends so the loop has no click.
    let fade = samples(0.25);
    let total = v.len();
    for i in 0..fade {
        let k = i as f32 / fade as f32;
        let a = v[i];
        v[i] = a * k + v[total - fade + i] * (1.0 - k);
    }
    v.truncate(total - fade);
    finish(v, 0.6)
}

/// The dark underground: a low swell with a distant, uneasy drone. Plays now and then in caves.
fn synth_cave(rng: &mut Rng) -> Vec<f32> {
    let len = 5.0;
    let mut v = vec![0.0f32; samples(len)];
    let f = rng.range(45.0, 70.0);
    pad(&mut v, 0.0, len, f, 0.9);
    pad(&mut v, 0.3, len - 0.3, f * rng.range(1.41, 1.5), 0.35);
    burst(&mut v, rng.range(0.5, 2.5), 1.5, 2.0, 60.0, 400.0, 0.5, rng);
    reverb(&mut v, 0.9, 2.5);
    finish(v, 0.7)
}

/// Write every effect (one variant each) and the music as WAV files, for listening
/// without launching the game: `minceraft --export-sounds <dir>`.
pub fn export_wavs(dir: &std::path::Path) -> std::io::Result<usize> {
    std::fs::create_dir_all(dir)?;
    let mut rng = Rng::new(0x50_4E_D5);
    let mut n = 0;
    for s in all_sfx() {
        let name = format!("{s:?}").to_lowercase().replace(['(', ')'], "_").trim_end_matches('_').to_string();
        std::fs::write(dir.join(format!("{name}.wav")), wav(&synth(s, &mut rng)))?;
        n += 1;
    }
    for (i, m) in MOODS.iter().enumerate() {
        std::fs::write(dir.join(format!("music_{}.wav", format!("{m:?}").to_lowercase())), wav(&synth_music_mood(*m, 7 + i as u64)))?;
    }
    std::fs::write(dir.join("rain.wav"), wav(&synth_rain(&mut rng)))?;
    std::fs::write(dir.join("cave.wav"), wav(&synth_cave(&mut rng)))?;
    Ok(n + MOODS.len() + 2)
}

// ---------------------------------------------------------------- playback

const VARIANTS: usize = 4;

pub struct Audio {
    bank: Vec<(Sfx, Vec<Sound>)>,
    /// Music, one track per mood, arriving from a background thread (see `poll`).
    music: Vec<(Mood, Sound)>,
    pending: Option<std::sync::mpsc::Receiver<(Mood, Vec<u8>)>>,
    current: Option<usize>,
    music_playing: f32,
    music_timer: f32,
    rain: Option<Sound>,
    rain_vol: f32,
    rain_playing: bool,
    cave: Option<Sound>,
    cave_timer: f32,
    rng: Rng,
    pub volume: f32,
    pub music_on: bool,
}

/// What the surroundings sound like right now (worked out by the game each frame).
#[derive(Clone, Copy, Debug)]
pub struct Ambience {
    /// Which music suits the moment.
    pub mood: Mood,
    /// How loud the rain is where you are: 0 (none, or snow) to 1 (out in a downpour).
    pub rain: f32,
    /// Deep in the dark underground, where caves make their noises.
    pub cave: bool,
}

impl Default for Ambience {
    fn default() -> Self {
        Ambience { mood: Mood::Day, rain: 0.0, cave: false }
    }
}

pub(crate) fn all_sfx() -> Vec<Sfx> {
    let mut v = Vec::new();
    for m in [Mat::Stone, Mat::Wood, Mat::Grass, Mat::Sand, Mat::Glass] {
        v.extend([Sfx::Break(m), Sfx::Hit(m), Sfx::Place(m), Sfx::Step(m)]);
    }
    v.extend([
        Sfx::Hurt,
        Sfx::MobHurt,
        Sfx::Oink,
        Sfx::Hiss,
        Sfx::Groan,
        Sfx::Explode,
        Sfx::Eat,
        Sfx::Pop,
        Sfx::Click,
        Sfx::Splash,
        Sfx::Craft,
        Sfx::Thud,
        // Appended only: the index is the multiplayer wire encoding.
        Sfx::Baa,
        Sfx::Warp,
        Sfx::Fanfare,
        Sfx::Boing,
        Sfx::Cluck,
        Sfx::Moo,
        Sfx::Rattle,
        Sfx::Skitter,
        Sfx::Bloop,
        Sfx::Twang,
        Sfx::Thunk,
        Sfx::Thunder,
        Sfx::Chime,
        Sfx::Woof,
        Sfx::Snip,
        Sfx::Hmm,
        Sfx::Squawk,
        Sfx::Buzz,
        Sfx::Brush,
        Sfx::Sculk,
        Sfx::Shriek,
        Sfx::Roar,
        Sfx::Shush,
        Sfx::Grind,
        Sfx::Yip,
        Sfx::Croak,
        Sfx::Scuttle,
    ]);
    v
}

impl Audio {
    pub async fn load() -> Audio {
        if !has_output_device() {
            if cfg!(feature = "sound") {
                eprintln!("Minceraft: no sound card found. Continuing in silence.");
            }
            AUDIO_DEAD.store(true, Ordering::Relaxed);
        }
        let mut rng = Rng::new(0x50_4E_D5);
        let mut bank = Vec::new();
        if !AUDIO_DEAD.load(Ordering::Relaxed) {
            for s in all_sfx() {
                // Several variants per effect: quad-snd has no pitch control, so variety is baked in.
                let n = if matches!(s, Sfx::Explode | Sfx::Hiss | Sfx::Click | Sfx::Craft | Sfx::Fanfare | Sfx::Thunder | Sfx::Chime) { 1 } else { VARIANTS };
                let mut sounds = Vec::new();
                for _ in 0..n {
                    if let Ok(snd) = load_sound_from_bytes(&wav(&synth(s, &mut rng))).await {
                        sounds.push(snd);
                    }
                }
                bank.push((s, sounds));
            }
        }
        let dead = AUDIO_DEAD.load(Ordering::Relaxed);
        // The music takes a few seconds to make, so it's made in the background.
        let pending = (!dead).then(|| {
            let (tx, rx) = std::sync::mpsc::channel();
            let _ = std::thread::Builder::new().name("music".into()).spawn(move || {
                for (i, m) in MOODS.iter().enumerate() {
                    if tx.send((*m, wav(&synth_music_mood(*m, 7 + i as u64)))).is_err() {
                        break;
                    }
                }
            });
            rx
        });
        let (rain, cave) = if dead {
            (None, None)
        } else {
            (load_sound_from_bytes(&wav(&synth_rain(&mut rng))).await.ok(), load_sound_from_bytes(&wav(&synth_cave(&mut rng))).await.ok())
        };
        Audio {
            bank,
            music: Vec::new(),
            pending,
            current: None,
            music_playing: 0.0,
            music_timer: 25.0,
            rain,
            rain_vol: 0.0,
            rain_playing: false,
            cave,
            cave_timer: 30.0,
            rng,
            volume: 0.8,
            music_on: true,
        }
    }

    /// Pick up music finished in the background (call once a frame).
    pub async fn poll(&mut self) {
        let Some(rx) = &self.pending else { return };
        let mut ready = Vec::new();
        loop {
            match rx.try_recv() {
                Ok(t) => ready.push(t),
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.pending = None;
                    break;
                }
            }
        }
        for (m, bytes) in ready {
            if let Ok(snd) = load_sound_from_bytes(&bytes).await {
                self.music.push((m, snd));
            }
        }
    }

    fn alive(&self) -> bool {
        !AUDIO_DEAD.load(Ordering::Relaxed) && self.volume > 0.0
    }

    fn base_volume(s: Sfx) -> f32 {
        match s {
            Sfx::Step(_) => 0.28,
            Sfx::Hit(_) => 0.45,
            Sfx::Click => 0.5,
            Sfx::Pop => 0.45,
            Sfx::Explode | Sfx::Thunder => 1.0,
            // Voices are dense; keep them level with the percussive sounds.
            Sfx::Groan | Sfx::Oink | Sfx::Baa | Sfx::Moo | Sfx::Cluck | Sfx::Squawk | Sfx::Yip | Sfx::Croak => 0.4,
            Sfx::Buzz => 0.3,
            Sfx::Fanfare => 0.5,
            Sfx::Hurt | Sfx::MobHurt => 0.5,
            _ => 0.8,
        }
    }

    /// Play an effect; positional sounds fade with distance from the listener.
    pub fn play(&mut self, s: Sfx, at: Option<Vec3>, listener: Vec3) {
        if !self.alive() {
            return;
        }
        let range = if s == Sfx::Explode { 64.0 } else { 24.0 };
        let atten = match at {
            Some(p) => (1.0 - p.distance(listener) / range).clamp(0.0, 1.0).powf(1.5),
            None => 1.0,
        };
        let volume = Self::base_volume(s) * atten * self.volume;
        if volume < 0.01 {
            return;
        }
        let Some((_, sounds)) = self.bank.iter().find(|(k, _)| *k == s) else { return };
        if sounds.is_empty() {
            return;
        }
        let i = self.rng.int(0, sounds.len() as i32 - 1) as usize;
        play_sound(&sounds[i], PlaySoundParams { looped: false, volume });
    }

    /// Occasionally drift some music in, like a certain other block game (a
    /// track to suit the moment), and keep the rain and caves sounding.
    pub fn update_music(&mut self, dt: f32, in_game: bool, amb: Ambience) {
        self.ambience(dt, in_game, amb);
        if self.music_playing > 0.0 {
            self.music_playing -= dt;
            if !self.music_on || !self.alive() {
                if let Some((_, m)) = self.current.and_then(|i| self.music.get(i)) {
                    stop_sound(m);
                }
                self.music_playing = 0.0;
            }
            return;
        }
        if !in_game || !self.music_on || !self.alive() {
            return;
        }
        self.music_timer -= dt;
        if self.music_timer <= 0.0 {
            // Day and morning tunes are interchangeable; don't play the same one twice running.
            let fits = |m: Mood| m == amb.mood || matches!((m, amb.mood), (Mood::Day, Mood::Morning) | (Mood::Morning, Mood::Day));
            let mut choices: Vec<usize> = (0..self.music.len()).filter(|&i| fits(self.music[i].0) && Some(i) != self.current).collect();
            if choices.is_empty() {
                choices = (0..self.music.len()).filter(|&i| fits(self.music[i].0)).collect();
            }
            if choices.is_empty() {
                // Nothing ready yet (or nothing fits): try again shortly.
                self.music_timer = 10.0;
                return;
            }
            let i = choices[self.rng.int(0, choices.len() as i32 - 1) as usize];
            play_sound(&self.music[i].1, PlaySoundParams { looped: false, volume: 0.45 * self.volume });
            self.current = Some(i);
            self.music_playing = 60.0;
            self.music_timer = self.rng.range(150.0, 300.0);
        }
    }

    fn ambience(&mut self, dt: f32, in_game: bool, amb: Ambience) {
        let live = in_game && self.alive();
        // Rain fades in and out rather than switching.
        let target = if live { amb.rain.clamp(0.0, 1.0) * 0.5 * self.volume } else { 0.0 };
        self.rain_vol += (target - self.rain_vol) * (dt * 1.5).min(1.0);
        if let Some(r) = &self.rain {
            if self.rain_vol > 0.005 {
                if !self.rain_playing {
                    play_sound(r, PlaySoundParams { looped: true, volume: self.rain_vol });
                    self.rain_playing = true;
                } else {
                    set_sound_volume(r, self.rain_vol);
                }
            } else if self.rain_playing {
                stop_sound(r);
                self.rain_playing = false;
            }
        }
        // Now and then, something rumbles in the dark.
        if live && amb.cave {
            self.cave_timer -= dt;
            if self.cave_timer <= 0.0 {
                if let Some(c) = &self.cave {
                    play_sound(c, PlaySoundParams { looped: false, volume: 0.5 * self.volume });
                }
                self.cave_timer = self.rng.range(45.0, 120.0);
            }
        } else {
            self.cave_timer = self.cave_timer.max(20.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_effect_synthesises_to_valid_audio() {
        let mut rng = Rng::new(1);
        for s in all_sfx() {
            let v = synth(s, &mut rng);
            assert!(!v.is_empty(), "{s:?} is empty");
            assert!(v.iter().all(|x| x.is_finite() && x.abs() <= 1.0), "{s:?} out of range");
            assert!(v.iter().any(|x| x.abs() > 0.1), "{s:?} is silent");
            let w = wav(&v);
            assert_eq!(&w[..4], b"RIFF");
            assert_eq!(w.len(), 44 + v.len() * 2);
        }
        let m = synth_music(3);
        assert!(m.len() > SR as usize * 40 && m.iter().all(|x| x.is_finite()));
    }

    #[test]
    fn music_moods_rain_and_caves_are_sound() {
        let mut rng = Rng::new(2);
        for m in MOODS {
            let v = synth_music_mood(m, 5);
            assert_eq!(v.len(), samples(60.0), "{m:?}");
            assert!(v.iter().all(|x| x.is_finite() && x.abs() <= 1.0), "{m:?} out of range");
            // Something plays through most of it (no long dead stretches).
            let quiet = v.chunks(samples(5.0)).take(10).filter(|c| c.iter().fold(0.0f32, |a, x| a.max(x.abs())) < 0.02).count();
            assert!(quiet <= 1, "{m:?} has {quiet} silent stretches");
            // Fades out at the end.
            assert!(v[v.len() - 10..].iter().all(|x| x.abs() < 0.01), "{m:?} ends abruptly");
        }
        let rain = synth_rain(&mut rng);
        assert!(rain.len() > SR as usize * 3 && rain.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
        // A loop: wrapping from the end to the start jumps no more than the sound does anyway.
        let biggest = rain.windows(2).fold(0.0f32, |a, w| a.max((w[1] - w[0]).abs()));
        assert!((rain[0] - rain[rain.len() - 1]).abs() <= biggest, "rain loop clicks");
        let rms = (rain.iter().map(|x| x * x).sum::<f32>() / rain.len() as f32).sqrt();
        assert!(rms > 0.05, "rain is too quiet: {rms}");
        let cave = synth_cave(&mut rng);
        assert!(cave.iter().any(|x| x.abs() > 0.2) && cave.iter().all(|x| x.is_finite()));
    }
}
