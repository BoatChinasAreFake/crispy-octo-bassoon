//! Procedural sound effects and music. Like the textures, no audio files ship:
//! every sound is synthesised into an in-memory WAV at startup.

use crate::block::*;
use crate::noise::Rng;
use macroquad::audio::{load_sound_from_bytes, play_sound, stop_sound, PlaySoundParams, Sound};
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
            crate::entity::MobKind::Mooer => Sfx::Moo,
            crate::entity::MobKind::Rattler => Sfx::Rattle,
            crate::entity::MobKind::Bloop => Sfx::Bloop,
            crate::entity::MobKind::Woofer => Sfx::Woof,
            crate::entity::MobKind::Hmmer => Sfx::Hmm,
            crate::entity::MobKind::Grumbler => Sfx::Oink,
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

/// A calm, meandering pentatonic tune. Sounds vaguely like a certain composer, if you squint.
fn synth_music(seed: u64) -> Vec<f32> {
    let mut rng = Rng::new(seed);
    let len = 48.0;
    let mut v = vec![0.0f32; samples(len)];
    let scale = [0, 2, 4, 7, 9];
    let root = 196.0; // G3
    let note = |deg: i32| -> f32 {
        let oct = deg.div_euclid(5);
        let st = scale[deg.rem_euclid(5) as usize] + oct * 12;
        root * 2f32.powf(st as f32 / 12.0)
    };
    let mut t = 0.8;
    let mut deg = 5;
    while t < len - 5.0 {
        let f = note(deg);
        tone(&mut v, t, 4.0, f, f, 1.3, 0.5, &[1.0, 0.35, 0.12, 0.05]);
        if rng.chance(0.3) {
            // A soft chord underneath.
            for d in [deg - 5, deg - 3] {
                let g = note(d);
                tone(&mut v, t, 5.0, g, g, 0.8, 0.18, &[1.0, 0.2]);
            }
        }
        deg = (deg + rng.int(-2, 2)).clamp(2, 11);
        t += [0.6, 0.9, 1.2, 1.8, 2.4][rng.int(0, 4) as usize];
    }
    // Cheap echo for a sense of space.
    let d = samples(0.43);
    for i in d..v.len() {
        v[i] += v[i - d] * 0.35;
    }
    let n = v.len();
    let fade = samples(4.0);
    for i in 0..fade {
        v[n - 1 - i] *= i as f32 / fade as f32;
    }
    finish(v, 0.8)
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
    std::fs::write(dir.join("music.wav"), wav(&synth_music(7)))?;
    Ok(n + 1)
}

// ---------------------------------------------------------------- playback

const VARIANTS: usize = 4;

pub struct Audio {
    bank: Vec<(Sfx, Vec<Sound>)>,
    music: Option<Sound>,
    music_playing: f32,
    music_timer: f32,
    rng: Rng,
    pub volume: f32,
    pub music_on: bool,
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
        let music = if AUDIO_DEAD.load(Ordering::Relaxed) { None } else { load_sound_from_bytes(&wav(&synth_music(7))).await.ok() };
        Audio { bank, music, music_playing: 0.0, music_timer: 25.0, rng, volume: 0.8, music_on: true }
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
            Sfx::Groan | Sfx::Oink | Sfx::Baa | Sfx::Moo | Sfx::Cluck => 0.4,
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

    /// Occasionally drift some music in, like a certain other block game.
    pub fn update_music(&mut self, dt: f32, in_game: bool) {
        let Some(m) = &self.music else { return };
        if self.music_playing > 0.0 {
            self.music_playing -= dt;
            if !self.music_on || !self.alive() {
                stop_sound(m);
                self.music_playing = 0.0;
            }
            return;
        }
        if !in_game || !self.music_on || !self.alive() {
            return;
        }
        self.music_timer -= dt;
        if self.music_timer <= 0.0 {
            play_sound(m, PlaySoundParams { looped: false, volume: 0.45 * self.volume });
            self.music_playing = 48.0;
            self.music_timer = self.rng.range(150.0, 300.0);
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
}
