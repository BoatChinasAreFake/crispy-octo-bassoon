//! **Composing music by the rules.** The background music and the Music
//! Discs are written here, the way a person would be taught to: in a key,
//! over chord progressions that go somewhere, in phrases that answer each
//! other, with a tune that sits on the harmony.
//!
//! - **Key and chords.** A major or minor key. Chords are built in thirds on
//!   the scale (I, ii, iii, IV, V, vi; in minor the V borrows the raised
//!   leading tone, as in harmonic minor, so it pulls home).
//! - **Phrases.** Tunes come in 8-bar periods: a 4-bar question ending on V
//!   (a half cadence), and a 4-bar answer that starts the same way and comes
//!   home V to I (an authentic cadence). A contrasting B section takes other
//!   chords and ends on V, leading back.
//! - **Melody.** Notes on the strong beats are chord tones; between them, a
//!   note is either another chord tone or a step from the one before
//!   (passing and neighbour notes). Mostly steps; after a leap the line steps
//!   back the other way; no tritone leaps, nothing wider than a sixth. The
//!   motif comes back (repetition) and is answered (variation keeps its
//!   rhythm and contour over new chords).
//! - **Voice leading.** Each chord takes whichever inversion moves least
//!   from the one before; the bass has the root on the downbeat.
//!
//! Times here are in beats; whoever plays a piece turns them into seconds.

use crate::noise::Rng;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Major,
    Minor,
}

impl Mode {
    pub fn steps(self) -> [i32; 7] {
        match self {
            Mode::Major => [0, 2, 4, 5, 7, 9, 11],
            Mode::Minor => [0, 2, 3, 5, 7, 8, 10],
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Key {
    /// The tonic as a MIDI note (60 is middle C).
    pub tonic: i32,
    pub mode: Mode,
}

impl Key {
    /// The MIDI note of scale degree `deg` (0 the tonic, 7 an octave up).
    /// Over a V chord in a minor key the seventh degree is raised.
    pub fn pitch(&self, deg: i32, over: i32) -> i32 {
        let i = deg.rem_euclid(7);
        let mut p = self.tonic + self.mode.steps()[i as usize] + 12 * deg.div_euclid(7);
        if self.mode == Mode::Minor && i == 6 && over.rem_euclid(7) == 4 {
            p += 1;
        }
        p
    }
}

/// Hz of a MIDI note.
pub fn hz(midi: i32) -> f32 {
    440.0 * 2f32.powf((midi - 69) as f32 / 12.0)
}

/// The nearest MIDI note to `f` Hz.
pub fn midi_of(f: f32) -> i32 {
    (69.0 + 12.0 * (f / 440.0).log2()).round() as i32
}

/// Is degree `deg` in the chord built on `root`?
pub fn chord_tone(deg: i32, root: i32, sevenths: bool) -> bool {
    let k = (deg - root).rem_euclid(7);
    k == 0 || k == 2 || k == 4 || (sevenths && k == 6)
}

/// One note: when (beats from the start), how long (beats), which.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Note {
    pub at: f32,
    pub len: f32,
    pub midi: i32,
}

/// What part of the form a bar is in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sec {
    Intro,
    /// The tune.
    A,
    /// The tune again, with passing notes filled in.
    A2,
    /// The contrasting middle.
    B,
    Outro,
}

/// One bar: where it starts (beats), its chord (a scale degree), its section,
/// and whether it ends a phrase (on V: half; on I: full).
#[derive(Clone, Copy, Debug)]
pub struct Bar {
    pub start: f32,
    pub chord: i32,
    pub sec: Sec,
    pub cadence: Option<Cadence>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cadence {
    Half,
    Full,
}

/// What to write.
#[derive(Clone, Debug)]
pub struct Spec {
    pub key: Key,
    /// Beats in a bar (3 or 4).
    pub beats: u32,
    /// The form, in sections (the Intro and Outro run 2 and 4 bars; A, A2 and B, 8).
    pub plan: Vec<Sec>,
    /// The tune's range (MIDI notes).
    pub low: i32,
    pub high: i32,
    /// Keep the tune to the major pentatonic away from the strong beats (no
    /// fourth or seventh degree as passing notes): open and calm.
    pub pentatonic: bool,
    /// How busy the tune is, 0 (long notes) to 1 (lots of short ones).
    pub busy: f32,
    /// Seventh chords (and the seventh counts as a chord tone).
    pub sevenths: bool,
}

/// A written piece.
#[derive(Clone, Debug)]
pub struct Piece {
    pub beats: u32,
    pub bars: Vec<Bar>,
    pub lead: Vec<Note>,
    /// Each bar's chord as voiced (MIDI notes, low to high).
    pub voicings: Vec<Vec<i32>>,
    pub bass: Vec<Note>,
}

impl Piece {
    pub fn total_beats(&self) -> f32 {
        self.bars.len() as f32 * self.beats as f32
    }

    /// The bar a time (in beats) falls in.
    pub fn bar_at(&self, at: f32) -> &Bar {
        let i = ((at / self.beats as f32).floor() as usize).min(self.bars.len() - 1);
        &self.bars[i]
    }

    /// Is this time a strong beat (the downbeat; and beat 3 of four)?
    #[cfg(test)]
    pub fn strong(&self, at: f32) -> bool {
        let e = ((at - self.bar_at(at).start) * 2.0).round() as u32;
        e.is_multiple_of(strong_unit(self.beats))
    }
}

/// Eighths between strong beats.
fn strong_unit(beats: u32) -> u32 {
    if beats.is_multiple_of(2) { beats } else { beats * 2 }
}

/// Bar rhythms, in eighths (a negative is a rest), from calm to busy.
fn rhythms(beats: u32) -> &'static [&'static [i32]] {
    if beats == 3 {
        &[&[6], &[4, 2], &[2, 4], &[2, 2, 2], &[3, 1, 2], &[2, 1, 1, 2], &[1, 1, 2, 2]]
    } else {
        &[&[8], &[4, 4], &[6, 2], &[4, 2, 2], &[2, 2, 4], &[2, 2, 2, 2], &[3, 1, 2, 2], &[2, 1, 1, 4], &[3, 1, 3, 1], &[2, 1, 1, 2, 2], &[1, 1, 2, 2, 2]]
    }
}

fn pick_rhythm(beats: u32, busy: f32, rng: &mut Rng) -> Vec<i32> {
    let all = rhythms(beats);
    let n = all.len() as f32;
    // Centre on the busyness, a little either way; never the single long note mid-phrase.
    let c = (1.0 + busy * (n - 2.0)).round() as i32;
    let i = (c + rng.int(-2, 2)).clamp(1, n as i32 - 1) as usize;
    all[i].to_vec()
}

/// The notes of a bar's tune, in eighths: (at, len, degree).
type BarTune = Vec<(u32, u32, i32)>;

/// What the tune has been doing (to keep its line sensible across bars).
struct Line {
    prev: Option<i32>,
    /// The last interval moved (degrees).
    last_move: i32,
}

struct Writer<'a> {
    spec: &'a Spec,
    lo: i32,
    hi: i32,
}

impl Writer<'_> {
    fn chord_tone(&self, deg: i32, root: i32) -> bool {
        chord_tone(deg, root, self.spec.sevenths)
    }

    /// Choose a note. `strong`: it must be a chord tone. `want`: the way the
    /// line should go (-1, 0, 1). `end`: the degrees (mod 7) it has to be.
    #[allow(clippy::too_many_arguments)]
    fn pick(&self, line: &Line, root: i32, strong: bool, want: i32, end: Option<&[i32]>, lo: i32, hi: i32, rng: &mut Rng) -> i32 {
        let key = self.spec.key;
        let mut best: Option<(f32, i32)> = None;
        for relax in 0..3 {
            for d in lo..=hi {
                let ct = self.chord_tone(d, root);
                if strong && !ct {
                    continue;
                }
                if let Some(e) = end
                    && !e.contains(&d.rem_euclid(7))
                {
                    continue;
                }
                if let Some(p) = line.prev {
                    let mv = d - p;
                    // Off the beat: a chord tone, or a step from the last note.
                    if !strong && !ct && mv.abs() != 1 {
                        continue;
                    }
                    if self.spec.pentatonic && !ct && matches!(d.rem_euclid(7), 3 | 6) {
                        continue;
                    }
                    let semis = (key.pitch(d, root) - key.pitch(p, root)).abs();
                    if mv.abs() > 5 || semis == 6 {
                        continue;
                    }
                    // After a leap, step back the other way.
                    if relax < 2 && line.last_move.abs() >= 3 && !(mv.signum() == -line.last_move.signum() && mv.abs() <= 2) {
                        continue;
                    }
                    let mut cost = mv.abs() as f32 * 0.9;
                    if mv == 0 {
                        cost += 1.6;
                    }
                    if mv.abs() >= 3 {
                        cost += 1.5;
                    }
                    if relax == 0 && want != 0 && mv.signum() != want {
                        cost += 2.5;
                    }
                    cost += rng.range(0.0, 1.4);
                    if best.is_none_or(|b| cost < b.0) {
                        best = Some((cost, d));
                    }
                } else {
                    // The first note: a chord tone near the middle of the range.
                    let cost = (d - (lo + hi) / 2).abs() as f32 + rng.range(0.0, 1.5);
                    if best.is_none_or(|b| cost < b.0) {
                        best = Some((cost, d));
                    }
                }
            }
            if best.is_some() {
                break;
            }
        }
        best.map(|b| b.1).unwrap_or_else(|| {
            // Nothing fits (a very tight spot): the chord's root, near the last note.
            let p = line.prev.unwrap_or((lo + hi) / 2);
            let mut d = root + 7 * ((p - root) as f32 / 7.0).round() as i32;
            while d > hi {
                d -= 7;
            }
            while d < lo {
                d += 7;
            }
            d
        })
    }

    /// A bar of tune over `root`, to `rhythm`, following `contour` (the way
    /// each note moves, if given) and ending on one of `end`.
    #[allow(clippy::too_many_arguments)]
    fn bar(&self, line: &mut Line, root: i32, rhythm: &[i32], contour: Option<&[i32]>, end: Option<&[i32]>, shift: i32, rng: &mut Rng) -> BarTune {
        let unit = strong_unit(self.spec.beats);
        let mut out = Vec::new();
        let mut e = 0u32;
        let sounding: Vec<usize> = rhythm.iter().enumerate().filter(|(_, r)| **r > 0).map(|(i, _)| i).collect();
        let mut k = 0;
        for (i, &r) in rhythm.iter().enumerate() {
            let len = r.unsigned_abs();
            if r > 0 {
                let last = sounding.last() == Some(&i);
                let want = contour.and_then(|c| c.get(k).copied()).unwrap_or(0);
                let d = self.pick(line, root, e.is_multiple_of(unit), want, if last { end } else { None }, self.lo + shift, self.hi + shift, rng);
                if let Some(p) = line.prev {
                    line.last_move = d - p;
                }
                line.prev = Some(d);
                out.push((e, len, d));
                k += 1;
            }
            e += len;
        }
        out
    }
}

/// The way each note in a bar moves from the one before.
fn contour(tune: &BarTune, before: Option<i32>) -> Vec<i32> {
    let mut prev = before;
    tune.iter()
        .map(|&(_, _, d)| {
            let s = prev.map(|p| (d - p).signum()).unwrap_or(0);
            prev = Some(d);
            s
        })
        .collect()
}

/// Fill a third with its passing note (in A2): only where the passing note
/// falls off the strong beats.
fn ornament(tune: &BarTune, next: Option<i32>, unit: u32) -> BarTune {
    let mut out = Vec::new();
    for (i, &(at, len, d)) in tune.iter().enumerate() {
        let to = tune.get(i + 1).map(|t| t.2).or(next);
        let mid = at + len / 2;
        match to {
            Some(t) if (t - d).abs() == 2 && len >= 2 && !mid.is_multiple_of(unit) => {
                out.push((at, len / 2, d));
                out.push((mid, len - len / 2, d + (t - d).signum()));
            }
            _ => out.push((at, len, d)),
        }
    }
    out
}

/// Progressions (scale degrees, a chord a bar): the question (ending on V),
/// and the contrasting middle (ending on V too).
fn progressions(mode: Mode, rng: &mut Rng) -> ([i32; 4], [i32; 4]) {
    let (questions, middles): (&[[i32; 4]], &[[i32; 4]]) = match mode {
        Mode::Major => (&[[0, 5, 3, 4], [0, 3, 1, 4], [0, 2, 3, 4], [0, 3, 0, 4], [0, 4, 5, 4]], &[[5, 3, 1, 4], [5, 2, 3, 4], [3, 0, 1, 4], [3, 4, 2, 5]]),
        Mode::Minor => (&[[0, 5, 3, 4], [0, 6, 5, 4], [0, 2, 3, 4], [0, 3, 5, 4]], &[[5, 2, 6, 4], [3, 0, 5, 4], [2, 6, 5, 4]]),
    };
    let q = questions[rng.int(0, questions.len() as i32 - 1) as usize];
    let mut m = middles[rng.int(0, middles.len() as i32 - 1) as usize];
    // (Every middle ends on V, to lead back to the tune.)
    m[3] = 4;
    (q, m)
}

/// Each chord in whichever inversion moves least from the last one (in `lo..lo+14`).
fn voice(key: Key, root: i32, sevenths: bool, prev: Option<&Vec<i32>>, lo: i32) -> Vec<i32> {
    let degs: Vec<i32> = if sevenths { vec![root, root + 2, root + 4, root + 6] } else { vec![root, root + 2, root + 4] };
    let pcs: Vec<i32> = degs.iter().map(|&d| key.pitch(d, root).rem_euclid(12)).collect();
    let mut best: Option<(i32, Vec<i32>)> = None;
    for inv in 0..pcs.len() {
        for base in [lo, lo + 4] {
            let mut v = Vec::new();
            let mut at = base;
            for j in 0..pcs.len() {
                let pc = pcs[(inv + j) % pcs.len()];
                let mut p = at + (pc - at).rem_euclid(12);
                if j > 0 && p == at {
                    p += 12;
                }
                v.push(p);
                at = p;
            }
            let cost = match prev {
                Some(pv) if pv.len() == v.len() => pv.iter().zip(&v).map(|(a, b)| (a - b).abs()).sum::<i32>(),
                Some(pv) => (pv.iter().sum::<i32>() / pv.len() as i32 - v.iter().sum::<i32>() / v.len() as i32).abs() * v.len() as i32,
                // The first chord: root position, close to the middle.
                None => (v[0] - lo - 2).abs() + if inv == 0 { 0 } else { 6 },
            };
            if best.as_ref().is_none_or(|b| cost < b.0) {
                best = Some((cost, v));
            }
        }
    }
    best.unwrap().1
}

/// Write a piece to `spec`.
pub fn compose(spec: &Spec, rng: &mut Rng) -> Piece {
    let key = spec.key;
    // The tune's range in scale degrees.
    let deg_of = |midi: i32, up: bool| {
        let mut d = (midi - key.tonic) * 7 / 12;
        if up {
            while key.pitch(d, 0) < midi {
                d += 1;
            }
        } else {
            while key.pitch(d, 0) > midi {
                d -= 1;
            }
        }
        d
    };
    let w = Writer { spec, lo: deg_of(spec.low, true), hi: deg_of(spec.high, false) };
    let unit = strong_unit(spec.beats);
    let (q, m) = progressions(key.mode, rng);
    let answer = [q[0], q[1], 4, 0];
    // The A motif (two bars) and the B motif, written the first time they're needed.
    let mut motif_a: Option<(BarTune, BarTune)> = None;
    let mut motif_b: Option<(BarTune, BarTune)> = None;
    let mut line = Line { prev: None, last_move: 0 };
    let mut bars: Vec<Bar> = Vec::new();
    let mut tunes: Vec<BarTune> = Vec::new();
    let half_end: &[i32] = &[1, 4];
    let leading: &[i32] = &[1, 6, 4];
    for &sec in &spec.plan {
        match sec {
            Sec::Intro => {
                for c in [0, q[1]] {
                    bars.push(Bar { start: 0.0, chord: c, sec, cadence: None });
                    tunes.push(Vec::new());
                }
            }
            Sec::A | Sec::A2 => {
                let (b1, b2) = match &motif_a {
                    Some(mt) => mt.clone(),
                    None => {
                        let r1 = pick_rhythm(spec.beats, spec.busy, rng);
                        let r2 = pick_rhythm(spec.beats, spec.busy, rng);
                        let b1 = w.bar(&mut line, q[0], &r1, None, None, 0, rng);
                        let b2 = w.bar(&mut line, q[1], &r2, None, None, 0, rng);
                        motif_a = Some((b1.clone(), b2.clone()));
                        (b1, b2)
                    }
                };
                // The question: the motif, the motif again over the next chord (same
                // rhythm and shape), and a half cadence held on V.
                line.prev = b2.last().map(|t| t.2);
                let r1: Vec<i32> = b1.iter().map(|t| t.1 as i32).collect();
                let c1 = contour(&b1, b2.last().map(|t| t.2));
                let b3 = w.bar(&mut line, q[2], &r1, Some(&c1), None, 0, rng);
                let hold: Vec<i32> = if spec.beats == 3 { vec![4, -2] } else { vec![6, -2] };
                let b4 = w.bar(&mut line, 4, &hold, Some(&[-1]), Some(half_end), 0, rng);
                // The answer: the same start, then home.
                let r2: Vec<i32> = b2.iter().map(|t| t.1 as i32).collect();
                let c2 = contour(&b2, b1.last().map(|t| t.2));
                line.prev = b2.last().map(|t| t.2);
                let b7 = w.bar(&mut line, 4, &r2, Some(&c2), Some(leading), 0, rng);
                // (A breath after it, before whatever comes next.)
                let fin: Vec<i32> = vec![spec.beats as i32 * 2 - 2, -2];
                let b8 = w.bar(&mut line, 0, &fin, None, Some(&[0]), 0, rng);
                let bars_tunes = [b1.clone(), b2.clone(), b3, b4, b1, b2, b7, b8];
                let chords = [q[0], q[1], q[2], 4, answer[0], answer[1], answer[2], answer[3]];
                for (i, t) in bars_tunes.iter().enumerate() {
                    let next = bars_tunes.get(i + 1).and_then(|n| n.first()).map(|n| n.2);
                    let t = if sec == Sec::A2 && i != 3 && i != 7 { ornament(t, next, unit) } else { t.clone() };
                    let cadence = match i {
                        3 => Some(Cadence::Half),
                        7 => Some(Cadence::Full),
                        _ => None,
                    };
                    bars.push(Bar { start: 0.0, chord: chords[i], sec, cadence });
                    tunes.push(t);
                }
            }
            Sec::B => {
                // Higher, and its own motif; four bars, then the same again varied, ending on V.
                let (b1, b2) = match &motif_b {
                    Some(mt) => mt.clone(),
                    None => {
                        let r1 = pick_rhythm(spec.beats, (spec.busy + 0.25).min(1.0), rng);
                        let r2 = pick_rhythm(spec.beats, spec.busy, rng);
                        let b1 = w.bar(&mut line, m[0], &r1, Some(&[1, 1, 1]), None, 1, rng);
                        let b2 = w.bar(&mut line, m[1], &r2, None, None, 1, rng);
                        motif_b = Some((b1.clone(), b2.clone()));
                        (b1, b2)
                    }
                };
                line.prev = b2.last().map(|t| t.2);
                let r1: Vec<i32> = b1.iter().map(|t| t.1 as i32).collect();
                let c1 = contour(&b1, b2.last().map(|t| t.2));
                let b3 = w.bar(&mut line, m[2], &r1, Some(&c1), None, 1, rng);
                let hold: Vec<i32> = if spec.beats == 3 { vec![4, -2] } else { vec![6, -2] };
                let b4 = w.bar(&mut line, 4, &hold, None, Some(half_end), 0, rng);
                // The second half: the first bar's rhythm over each chord in turn (a sequence).
                let mut second = Vec::new();
                for (j, &c) in m.iter().enumerate() {
                    let t = if j == 3 {
                        w.bar(&mut line, 4, &hold, Some(&[1]), Some(half_end), 0, rng)
                    } else {
                        let ct = contour(&b1, line.prev);
                        w.bar(&mut line, c, &r1, Some(&ct), None, 1, rng)
                    };
                    second.push(t);
                }
                for (i, t) in [b1, b2, b3, b4].into_iter().chain(second).enumerate() {
                    bars.push(Bar { start: 0.0, chord: m[i % 4], sec, cadence: (i % 4 == 3).then_some(Cadence::Half) });
                    tunes.push(t);
                }
            }
            Sec::Outro => {
                // IV, V, I: the tune sinks home and holds the tonic.
                let slow: Vec<i32> = vec![spec.beats as i32, spec.beats as i32];
                let ends: [Option<&[i32]>; 4] = [None, Some(leading), Some(&[0, 2]), Some(&[0])];
                for (i, c) in [3, 4, 0, 0].into_iter().enumerate() {
                    let r = if i == 3 { vec![spec.beats as i32 * 2] } else { slow.clone() };
                    let t = w.bar(&mut line, c, &r, Some(&[-1, -1]), ends[i], 0, rng);
                    // (The cadence is V to I; the last bar holds the I.)
                    bars.push(Bar { start: 0.0, chord: c, sec, cadence: (i == 2).then_some(Cadence::Full) });
                    tunes.push(t);
                }
            }
        }
    }
    let bpb = spec.beats as f32;
    for (i, b) in bars.iter_mut().enumerate() {
        b.start = i as f32 * bpb;
    }
    let mut lead = Vec::new();
    for (b, t) in bars.iter().zip(&tunes) {
        for &(at, len, d) in t {
            lead.push(Note { at: b.start + at as f32 * 0.5, len: len as f32 * 0.5, midi: key.pitch(d, b.chord) });
        }
    }
    // Chords, voiced smoothly, around the octave below the tune.
    let mut voicings: Vec<Vec<i32>> = Vec::new();
    let around = (spec.low - 12).clamp(48, 60);
    for b in &bars {
        let v = voice(key, b.chord, spec.sevenths, voicings.last(), around);
        voicings.push(v);
    }
    // The bass: the root on the downbeat; in four, the fifth (or the root) on beat 3.
    let mut bass = Vec::new();
    for b in &bars {
        let root = key.pitch(b.chord, b.chord);
        let low = 36 + (root - 36).rem_euclid(12);
        if spec.beats == 4 && b.cadence != Some(Cadence::Full) {
            bass.push(Note { at: b.start, len: 2.0, midi: low });
            let fifth = key.pitch(b.chord + 4, b.chord);
            let f = 36 + (fifth - 36).rem_euclid(12);
            bass.push(Note { at: b.start + 2.0, len: 2.0, midi: if (f - low).abs() <= 7 { f } else { low } });
        } else {
            bass.push(Note { at: b.start, len: bpb, midi: low });
        }
    }
    Piece { beats: spec.beats, bars, lead, voicings, bass }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn specs() -> Vec<Spec> {
        let s = |tonic, mode, beats, pentatonic, busy, sevenths| Spec { key: Key { tonic, mode }, beats, plan: vec![Sec::Intro, Sec::A, Sec::A2, Sec::B, Sec::A, Sec::Outro], low: 64, high: 84, pentatonic, busy, sevenths };
        vec![s(55, Mode::Major, 4, true, 0.4, false), s(53, Mode::Minor, 4, false, 0.2, false), s(57, Mode::Major, 3, false, 0.7, false), s(53, Mode::Major, 4, false, 0.5, true), s(52, Mode::Minor, 3, false, 0.6, false)]
    }

    /// The scale degree of a MIDI note (allowing the raised leading tone over V in minor).
    fn degree(key: Key, midi: i32, chord: i32) -> Option<i32> {
        (-28..40).find(|&d| key.pitch(d, chord) == midi)
    }

    #[test]
    fn tunes_sit_on_the_harmony_and_move_well() {
        for (n, spec) in specs().into_iter().enumerate() {
            for seed in 0..12u64 {
                let p = compose(&spec, &mut Rng::new(seed * 31 + n as u64));
                assert!(!p.lead.is_empty());
                for (i, note) in p.lead.iter().enumerate() {
                    let bar = p.bar_at(note.at);
                    let deg = degree(spec.key, note.midi, bar.chord).unwrap_or_else(|| panic!("spec {n} seed {seed}: note {} isn't in the key", note.midi));
                    if p.strong(note.at) {
                        assert!(chord_tone(deg, bar.chord, spec.sevenths), "spec {n} seed {seed}: a strong-beat note off the chord");
                    }
                    assert!((spec.low - 2..=spec.high + 4).contains(&note.midi), "in range");
                    let Some(next) = p.lead.get(i + 1) else { continue };
                    // Within a phrase (no rest between): no wide or tritone leaps.
                    if (next.at - (note.at + note.len)).abs() < 1e-3 {
                        let jump = (next.midi - note.midi).abs();
                        assert!(jump <= 9 && jump != 6, "spec {n} seed {seed}: leap of {jump} at beat {}", note.at);
                    }
                }
            }
        }
    }

    #[test]
    fn phrases_cadence_and_the_piece_ends_at_home() {
        for (n, spec) in specs().into_iter().enumerate() {
            let p = compose(&spec, &mut Rng::new(7 + n as u64));
            for (i, b) in p.bars.iter().enumerate() {
                match b.cadence {
                    Some(Cadence::Half) => assert_eq!(b.chord, 4, "a half cadence is on V"),
                    Some(Cadence::Full) => {
                        assert_eq!(b.chord, 0, "a full cadence lands on I");
                        assert!(matches!(p.bars[i - 1].chord, 3 | 4), "approached from V or IV");
                    }
                    None => {}
                }
            }
            let last = p.lead.last().unwrap();
            assert_eq!((last.midi - spec.key.tonic).rem_euclid(12), 0, "the tune ends on the tonic");
            assert_eq!(p.bars.last().unwrap().chord, 0);
            // The answer starts like the question (the period's repetition).
            let a = p.bars.iter().position(|b| b.sec == Sec::A).unwrap();
            let start = |bar: usize| p.lead.iter().filter(|x| p.bar_at(x.at).start == p.bars[bar].start).map(|x| x.midi).collect::<Vec<_>>();
            assert_eq!(start(a), start(a + 4));
        }
    }

    #[test]
    fn chords_move_smoothly_and_minor_dominants_lead_home() {
        for spec in specs() {
            let p = compose(&spec, &mut Rng::new(3));
            for w in p.voicings.windows(2) {
                let moved: i32 = w[0].iter().zip(&w[1]).map(|(a, b)| (a - b).abs()).sum();
                assert!(moved as f32 / w[0].len() as f32 <= 4.0, "voices move {moved} semitones in all");
            }
            for (b, v) in p.bars.iter().zip(&p.voicings) {
                // Each voicing really is that chord.
                for &m in v {
                    let d = degree(spec.key, m, b.chord).expect("chord note in key");
                    assert!(chord_tone(d, b.chord, spec.sevenths));
                }
                if spec.key.mode == Mode::Minor && b.chord == 4 {
                    let leading = (spec.key.tonic + 11).rem_euclid(12);
                    assert!(v.iter().any(|m| m.rem_euclid(12) == leading), "V in minor has the raised leading tone");
                }
            }
        }
    }
}
