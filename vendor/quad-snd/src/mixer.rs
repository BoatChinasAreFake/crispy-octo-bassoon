use crate::{AudioContext, PlaySoundParams};

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc;

enum AudioMessage {
    AddSound(u32, Vec<f32>),
    Play(u32, u32, bool, f32),
    Stop(u32),
    StopAll(u32),
    SetVolume(u32, f32),
    SetVolumeAll(u32, f32),
    Delete(u32),
}

#[derive(Debug)]
pub struct SoundState {
    sound_id: u32,
    play_id: u32,
    sample: usize,
    data: Arc<[f32]>,
    looped: bool,
    volume: f32,
}

impl SoundState {
    fn get_samples(&mut self, n: usize) -> &[f32] {
        let data = &self.data[self.sample..];

        self.sample += n;

        match data.get(..n) {
            Some(data) => data,
            None => data,
        }
    }

    fn rewind(&mut self) {
        self.sample = 0;
    }
}

pub struct Mixer {
    rx: mpsc::Receiver<AudioMessage>,
    sounds: HashMap<u32, Arc<[f32]>>,
    mixer_state: Vec<SoundState>,
    limiter: Limiter,
}

/// (Minceraft patch.) The mix bus used to be a plain sum, so a few loud sounds
/// at once went past full scale and clipped hard. This is a look-ahead
/// limiter instead. The mix runs 63 frames (about 1.4 ms) late, so peaks are
/// seen coming. The gain each frame needs is held at its lowest across the
/// look-ahead, then averaged over it: a smooth curve that has already come
/// down by the time a peak arrives, and stays under what every frame needs
/// (so nothing is flattened, and nothing goes over). It then recovers gently,
/// over about a quarter of a second.
pub struct Limiter {
    /// The frames waiting to go out.
    frames: Vec<[f32; 2]>,
    /// The gain each recent frame needs, and the held (lowest) gains being averaged.
    need: Vec<f32>,
    held: Vec<f32>,
    held_sum: f32,
    pos: usize,
    gain: f32,
}

impl Limiter {
    /// Where it starts to work: about -1 dBFS.
    pub const CEILING: f32 = 0.89;
    /// The look-ahead, in frames (the delay is one less).
    pub const AHEAD: usize = 64;
    /// Per-frame recovery at 44.1 kHz: about a quarter of a second.
    const RELEASE: f32 = 0.00009;

    pub fn new() -> Limiter {
        let n = Self::AHEAD;
        Limiter { frames: vec![[0.0; 2]; n], need: vec![1.0; n], held: vec![1.0; n], held_sum: n as f32, pos: 0, gain: 1.0 }
    }

    /// Process interleaved stereo in place (it comes out `AHEAD - 1` frames late).
    pub fn run(&mut self, buffer: &mut [f32]) {
        let n = Self::AHEAD;
        for frame in buffer.chunks_exact_mut(2) {
            let peak = frame[0].abs().max(frame[1].abs());
            self.need[self.pos] = if peak > Self::CEILING { Self::CEILING / peak } else { 1.0 };
            let lowest = self.need.iter().fold(1.0f32, |a, g| a.min(*g));
            self.held_sum += lowest - self.held[self.pos];
            self.held[self.pos] = lowest;
            if self.pos == 0 {
                // (Now and then, start the running sum afresh so rounding can't creep in.)
                self.held_sum = self.held.iter().sum();
            }
            let smooth = self.held_sum / n as f32;
            self.frames[self.pos] = [frame[0], frame[1]];
            self.pos = (self.pos + 1) % n;
            // The oldest frame is next in the ring.
            let out = self.frames[self.pos];
            self.gain = (self.gain + (1.0 - self.gain) * Self::RELEASE).min(smooth);
            frame[0] = Self::soft_clip(out[0] * self.gain);
            frame[1] = Self::soft_clip(out[1] * self.gain);
        }
    }

    /// Unchanged up to the ceiling; above it (which the look-ahead prevents),
    /// a smooth curve that stays under 1.
    fn soft_clip(x: f32) -> f32 {
        let a = x.abs();
        if a <= Self::CEILING {
            x
        } else {
            let room = 1.0 - Self::CEILING;
            // (Capped: far over the top, tanh rounds to exactly 1.)
            x.signum() * (Self::CEILING + room * ((a - Self::CEILING) / room).tanh()).min(0.999)
        }
    }
}

pub struct MixerBuilder {
    rx: mpsc::Receiver<AudioMessage>,
}

pub struct MixerControl {
    tx: mpsc::Sender<AudioMessage>,
    sound_id: Cell<u32>,
    play_id: Cell<u32>,
}

pub struct Playback {
    play_id: u32,
}

impl Playback {
    pub fn stop(self, ctx: &AudioContext) {
        ctx.mixer_ctrl.send(AudioMessage::Stop(self.play_id));
    }

    pub fn set_volume(&self, ctx: &AudioContext, volume: f32) {
        ctx.mixer_ctrl
            .send(AudioMessage::SetVolume(self.play_id, volume));
    }
}

impl MixerControl {
    pub fn load(&self, data: &[u8]) -> u32 {
        let sound_id = self.sound_id.get();

        let samples = load_samples_from_file(data).unwrap();

        self.tx
            .send(crate::mixer::AudioMessage::AddSound(sound_id, samples))
            .unwrap_or_else(|_| println!("Audio thread died"));
        self.sound_id.set(sound_id + 1);

        sound_id
    }

    pub fn play(&self, sound_id: u32, params: PlaySoundParams) -> Playback {
        let play_id = self.play_id.get();

        self.send(AudioMessage::Play(
            sound_id,
            play_id,
            params.looped,
            params.volume,
        ));

        self.play_id.set(play_id + 1);

        Playback { play_id }
    }

    pub fn stop(&self, play_id: u32) {
        self.send(AudioMessage::Stop(play_id));
    }

    pub fn stop_all(&self, sound_id: u32) {
        self.send(AudioMessage::StopAll(sound_id));
    }

    pub fn set_volume_all(&self, sound_id: u32, volume: f32) {
        self.send(AudioMessage::SetVolumeAll(sound_id, volume));
    }

    pub fn delete(&self, sound_id: u32) {
        self.send(AudioMessage::Delete(sound_id));
    }

    fn send(&self, message: AudioMessage) {
        self.tx
            .send(message)
            .unwrap_or_else(|_| println!("Audio thread died"))
    }
}

impl MixerBuilder {
    pub fn build(self) -> Mixer {
        Mixer {
            rx: self.rx,
            sounds: HashMap::new(),
            mixer_state: vec![],
            limiter: Limiter::new(),
        }
    }
}

impl Mixer {
    pub fn new() -> (MixerBuilder, MixerControl) {
        let (tx, rx) = mpsc::channel();

        (
            MixerBuilder { rx },
            MixerControl {
                tx,
                sound_id: Cell::new(0),
                play_id: Cell::new(0),
            },
        )
    }

    pub fn fill_audio_buffer(&mut self, buffer: &mut [f32], frames: usize) {
        while let Ok(message) = self.rx.try_recv() {
            match message {
                AudioMessage::AddSound(id, data) => {
                    self.sounds.insert(id, data.into());
                }
                AudioMessage::Play(sound_id, play_id, looped, volume) => {
                    if let Some(data) = self.sounds.get(&sound_id) {
                        self.mixer_state.push(SoundState {
                            sound_id,
                            play_id,
                            sample: 0,
                            data: data.clone(),
                            looped,
                            volume,
                        });
                    }
                }
                AudioMessage::Stop(play_id) => {
                    if let Some(i) = self.mixer_state.iter().position(|s| s.play_id == play_id) {
                        self.mixer_state.swap_remove(i);
                    }
                }
                AudioMessage::StopAll(sound_id) => {
                    for i in (0..self.mixer_state.len()).rev() {
                        if self.mixer_state[i].sound_id == sound_id {
                            self.mixer_state.swap_remove(i);
                        }
                    }
                }
                AudioMessage::SetVolume(play_id, volume) => {
                    if let Some(sound) = self.mixer_state.iter_mut().find(|s| s.play_id == play_id)
                    {
                        sound.volume = volume;
                    }
                }
                AudioMessage::SetVolumeAll(sound_id, volume) => {
                    for sound in self
                        .mixer_state
                        .iter_mut()
                        .filter(|s| s.sound_id == sound_id)
                    {
                        sound.volume = volume;
                    }
                }
                AudioMessage::Delete(sound_id) => {
                    for i in (0..self.mixer_state.len()).rev() {
                        if self.mixer_state[i].sound_id == sound_id {
                            self.mixer_state.swap_remove(i);
                        }
                    }
                    self.sounds.remove(&sound_id);
                }
            }
        }

        // zeroize the buffer
        buffer.fill(0.0);

        // Note: Doing manual iteration so we can remove sounds that finished playing
        let mut i = 0;

        while let Some(sound) = self.mixer_state.get_mut(i) {
            let volume = sound.volume;
            let mut remainder = buffer.len();

            loop {
                let samples = sound.get_samples(remainder);

                for (b, s) in buffer.iter_mut().zip(samples) {
                    *b += s * volume;
                }

                remainder -= samples.len();

                if remainder > 0 && sound.looped {
                    sound.rewind();
                    continue;
                }

                break;
            }

            if remainder > 0 {
                self.mixer_state.swap_remove(i);
            } else {
                i += 1;
            }
        }

        self.limiter.run(buffer);
    }
}

/// Parse ogg/wav/etc and get  resampled to 44100, 2 channel data
pub fn load_samples_from_file(bytes: &[u8]) -> Result<Vec<f32>, ()> {
    let mut audio_stream = {
        let file = std::io::Cursor::new(bytes);
        audrey::Reader::new(file).unwrap()
    };

    let description = audio_stream.description();
    let channels_count = description.channel_count();
    assert!(channels_count == 1 || channels_count == 2);

    let mut frames: Vec<f32> = Vec::with_capacity(4096);
    let mut samples_iterator = audio_stream
        .samples::<f32>()
        .map(std::result::Result::unwrap);

    // audrey's frame docs: "TODO: Should consider changing this behaviour to check the audio file's actual number of channels and automatically convert to F's number of channels while reading".
    // lets fix this TODO here
    if channels_count == 1 {
        frames.extend(samples_iterator.flat_map(|sample| [sample, sample]));
    } else if channels_count == 2 {
        frames.extend(samples_iterator);
    }

    let sample_rate = description.sample_rate();

    // (Minceraft patch.) Linear interpolation rather than nearest-neighbour,
    // which repeated or dropped whole samples and left harsh mirror images.
    // Minceraft hands everything over at 44.1 kHz already, so this is a fallback.
    if sample_rate != 44100 {
        let in_frames = frames.len() / 2;
        let out_frames = ((44100.0 / sample_rate as f64) * in_frames as f64) as usize;
        let step = sample_rate as f64 / 44100.0;
        let mut resampled = vec![0.0; out_frames * 2];
        for (n, sample) in resampled.chunks_exact_mut(2).enumerate() {
            let pos = n as f64 * step;
            let i = pos as usize;
            let t = (pos - i as f64) as f32;
            let j = (i + 1).min(in_frames.saturating_sub(1));
            for c in 0..2 {
                sample[c] = frames[2 * i + c] * (1.0 - t) + frames[2 * j + c] * t;
            }
        }
        return Ok(resampled);
    }

    Ok(frames)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_limiter_keeps_a_hot_mix_under_full_scale_and_leaves_a_quiet_one_alone() {
        let mut l = Limiter::new();
        // Quiet: untouched, just a moment late.
        let mut quiet: Vec<f32> = (0..2000).map(|i| (i as f32 * 0.05).sin() * 0.5).collect();
        let before = quiet.clone();
        l.run(&mut quiet);
        let late = 2 * (Limiter::AHEAD - 1);
        assert_eq!(quiet[late..], before[..2000 - late]);
        // Six loud sounds at once: never over full scale, and no hard flat tops.
        let mut hot: Vec<f32> = (0..44100).map(|i| (i as f32 * 0.03).sin() * 3.5).collect();
        l.run(&mut hot);
        let worst = hot.iter().enumerate().fold((0, 0.0f32), |a, (i, x)| if x.abs() > a.1 { (i, x.abs()) } else { a });
        assert!(worst.1 <= Limiter::CEILING + 1e-5, "the look-ahead never lets it past the ceiling (give or take rounding): {:?}", worst);
        let flat = hot.windows(2).filter(|w| w[0] == w[1] && w[0].abs() > 0.5).count();
        assert_eq!(flat, 0);
        // And it settles to the ceiling, not far below it.
        let late = hot[30000..].iter().fold(0.0f32, |a, x| a.max(x.abs()));
        assert!(late > 0.85 && late <= Limiter::CEILING + 0.01, "{}", late);
    }

    /// A mono WAV of 32-bit floats (how Minceraft hands its sounds over).
    fn float_wav(rate: u32, samples: &[f32]) -> Vec<u8> {
        let len = (samples.len() * 4) as u32;
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36 + len).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        for v in [16u32] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        for v in [3u16, 1] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        for v in [rate, rate * 4] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        for v in [4u16, 32] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(b"data");
        b.extend_from_slice(&len.to_le_bytes());
        for s in samples {
            b.extend_from_slice(&s.to_le_bytes());
        }
        b
    }

    #[test]
    fn float_wavs_at_44k_load_untouched_and_others_resample_smoothly() {
        let tone: Vec<f32> = (0..1000).map(|i| (i as f32 * 0.07).sin() * 1e-3).collect();
        let got = load_samples_from_file(&float_wav(44100, &tone)).unwrap();
        assert_eq!(got.len(), 2000);
        assert!(tone.iter().zip(got.chunks(2)).all(|(a, f)| f[0] == *a && f[1] == *a), "exact, both channels");
        // At 22.05 kHz: twice as long, and the in-between samples are in between.
        let got = load_samples_from_file(&float_wav(22050, &tone)).unwrap();
        assert_eq!(got.len(), 4000);
        let mid = got[2 * 101];
        assert!((mid - (tone[50] + tone[51]) / 2.0).abs() < 1e-9, "{} vs {} / {}", mid, tone[50], tone[51]);
    }
}
