//! A small language model for mods: SmolLM2 (a Llama-style transformer),
//! run on the CPU with 8-bit weights, in plain Rust (no new dependencies).
//!
//! - A mod ships nothing big itself: the model is one file, `model.lm`, put
//!   in the mod's folder (`mods/<id>/model.lm`). The mod loader ignores it,
//!   so it's never sent to joining players. Releases offer it as a separate
//!   download, made by `minceraft --pack-model <hf-folder> <out.lm>` from
//!   Hugging Face's `config.json`, `tokenizer.json` and `model.safetensors`.
//! - Weights are stored as 8-bit integers with one scale per row (about 140 MB
//!   for SmolLM2-135M), and stay that way in memory; each row is turned back
//!   into floats as it's used.
//! - Scripts ask with `model_ask(player, system, prompt)`; the answer comes
//!   back later as `on_model_reply(player, text)` (see scripting.rs). It runs
//!   on its own thread, one question at a time, where the world lives.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;

/// Token bytes to ids, and merge pairs to (rank, merged id).
type Vocab = HashMap<Vec<u8>, u32>;
type Merges = HashMap<(u32, u32), (usize, u32)>;
/// Tensors by name: shape and values.
type Tensors = HashMap<String, (Vec<usize>, Vec<f32>)>;

const MAGIC: &[u8; 8] = b"MCLM0001";
/// How many tokens a reply may run to, and how much context there's room for.
pub const MAX_NEW: usize = 80;
pub const MAX_CONTEXT: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Config {
    pub dim: usize,
    pub hidden: usize,
    pub layers: usize,
    pub heads: usize,
    pub kv_heads: usize,
    pub vocab: usize,
    pub rope_theta: f32,
    pub eps: f32,
}

impl Config {
    fn head_dim(&self) -> usize {
        self.dim / self.heads
    }
}

/// A matrix of 8-bit weights, one scale per row.
#[derive(Clone, Debug, PartialEq)]
pub struct Q8 {
    pub rows: usize,
    pub cols: usize,
    pub scale: Vec<f32>,
    pub q: Vec<i8>,
}

impl Q8 {
    pub fn quantize(rows: usize, cols: usize, w: &[f32]) -> Q8 {
        let mut scale = Vec::with_capacity(rows);
        let mut q = Vec::with_capacity(rows * cols);
        for r in w.chunks(cols) {
            let m = r.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            let s = if m > 0.0 { m / 127.0 } else { 1.0 };
            scale.push(s);
            q.extend(r.iter().map(|v| (v / s).round().clamp(-127.0, 127.0) as i8));
        }
        Q8 { rows, cols, scale, q }
    }
    fn row(&self, r: usize, out: &mut [f32]) {
        let s = self.scale[r];
        for (o, &v) in out.iter_mut().zip(&self.q[r * self.cols..(r + 1) * self.cols]) {
            *o = v as f32 * s;
        }
    }
    /// out = W x
    fn matvec(&self, x: &[f32], out: &mut [f32]) {
        let work = |rows: std::ops::Range<usize>, out: &mut [f32]| {
            for (o, r) in out.iter_mut().zip(rows) {
                let w = &self.q[r * self.cols..(r + 1) * self.cols];
                let mut acc = 0.0f32;
                for (a, b) in w.iter().zip(x) {
                    acc += *a as f32 * b;
                }
                *o = acc * self.scale[r];
            }
        };
        // Big matrices are split across a few threads.
        let threads = if self.rows * self.cols > 1 << 18 { std::thread::available_parallelism().map_or(1, |n| n.get().min(8)) } else { 1 };
        if threads <= 1 {
            work(0..self.rows, out);
            return;
        }
        let per = self.rows.div_ceil(threads);
        std::thread::scope(|s| {
            for (i, chunk) in out.chunks_mut(per).enumerate() {
                let start = i * per;
                s.spawn(move || work(start..start + chunk.len(), chunk));
            }
        });
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub attn_norm: Vec<f32>,
    pub wq: Q8,
    pub wk: Q8,
    pub wv: Q8,
    pub wo: Q8,
    pub mlp_norm: Vec<f32>,
    pub gate: Q8,
    pub up: Q8,
    pub down: Q8,
}

/// The model: its shape, tokenizer and weights (the output layer shares the embedding).
#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    pub cfg: Config,
    pub tok: Tokenizer,
    pub embed: Q8,
    pub layers: Vec<Layer>,
    pub norm: Vec<f32>,
}

// ------------------------------------------------------------------ tokenizer

/// Byte-level BPE, as GPT-2 and SmolLM2 use: text split into words, each word
/// into bytes, then pairs merged in rank order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tokenizer {
    /// Each token's bytes.
    pub tokens: Vec<Vec<u8>>,
    /// (left, right) -> merged, in rank order.
    pub merges: Vec<(u32, u32, u32)>,
    /// Special tokens by name (<|im_start|> and the like).
    pub special: Vec<(String, u32)>,
}

impl Tokenizer {
    fn tables(&self) -> (Vocab, Merges) {
        let by_bytes = self.tokens.iter().enumerate().filter(|(_, t)| !t.is_empty()).map(|(i, t)| (t.clone(), i as u32)).collect();
        let ranks = self.merges.iter().enumerate().map(|(r, &(a, b, m))| ((a, b), (r, m))).collect();
        (by_bytes, ranks)
    }

    pub fn special(&self, name: &str) -> Option<u32> {
        self.special.iter().find(|(n, _)| n == name).map(|&(_, id)| id)
    }

    /// Plain text to tokens (special tokens aren't recognised in it).
    pub fn encode(&self, text: &str) -> Vec<u32> {
        let (by_bytes, ranks) = self.tables();
        let mut out = Vec::new();
        for word in pre_split(text) {
            let mut parts: Vec<u32> = word.bytes().filter_map(|b| by_bytes.get(&vec![b]).copied()).collect();
            loop {
                let best = parts.windows(2).enumerate().filter_map(|(i, w)| ranks.get(&(w[0], w[1])).map(|&(r, m)| (r, i, m))).min();
                let Some((_, i, m)) = best else { break };
                parts[i] = m;
                parts.remove(i + 1);
            }
            out.extend(parts);
        }
        out
    }

    pub fn decode(&self, ids: &[u32]) -> String {
        let bytes: Vec<u8> = ids.iter().filter(|&&id| !self.special.iter().any(|(_, s)| *s == id)).flat_map(|&id| self.tokens.get(id as usize).cloned().unwrap_or_default()).collect();
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

/// GPT-2's word split (contractions, letters, single digits, other symbols,
/// runs of spaces), each word taking the space before it.
pub fn pre_split(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let start = i;
        let c = chars[i];
        // Contractions.
        if c == '\'' {
            let rest: String = chars[i + 1..].iter().take(2).collect::<String>().to_ascii_lowercase();
            let n = ["re", "ve", "ll"].iter().find(|s| rest.starts_with(*s)).map(|_| 3).or_else(|| ["s", "t", "m", "d"].iter().find(|s| rest.starts_with(*s)).map(|_| 2));
            if let Some(n) = n {
                i += n;
                out.push(chars[start..i].iter().collect());
                continue;
            }
        }
        // (Digits are split off first, so a space before one stands alone.)
        let lead = c == ' ' && i + 1 < chars.len() && !chars[i + 1].is_whitespace() && !chars[i + 1].is_numeric();
        let j = if lead { i + 1 } else { i };
        let d = chars[j];
        if d.is_alphabetic() {
            i = j;
            while i < chars.len() && chars[i].is_alphabetic() {
                i += 1;
            }
        } else if d.is_numeric() {
            // (Digits one at a time.)
            i = j + 1;
        } else if !d.is_whitespace() {
            i = j;
            while i < chars.len() && !chars[i].is_whitespace() && !chars[i].is_alphanumeric() {
                i += 1;
            }
        } else {
            // A run of spaces, leaving the last one to lead the next word.
            i = j;
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            if i < chars.len() && i - j > 1 && chars[i - 1] == ' ' {
                i -= 1;
            }
        }
        out.push(chars[start..i.max(start + 1)].iter().collect());
        i = i.max(start + 1);
    }
    out
}

// -------------------------------------------------------------------- running

/// One conversation's working memory.
struct State {
    k: Vec<Vec<f32>>,
    v: Vec<Vec<f32>>,
    pos: usize,
}

fn rmsnorm(x: &[f32], w: &[f32], eps: f32, out: &mut [f32]) {
    let ms = x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32;
    let k = 1.0 / (ms + eps).sqrt();
    for ((o, a), b) in out.iter_mut().zip(x).zip(w) {
        *o = a * k * b;
    }
}

/// Rotate one head's query or key by its position (the halves pair up, as in Hugging Face's Llama).
fn rope(v: &mut [f32], pos: usize, theta: f32) {
    let half = v.len() / 2;
    for i in 0..half {
        let freq = theta.powf(-2.0 * i as f32 / v.len() as f32);
        let (s, c) = (pos as f32 * freq).sin_cos();
        let (a, b) = (v[i], v[i + half]);
        v[i] = a * c - b * s;
        v[i + half] = b * c + a * s;
    }
}

impl Model {
    fn state(&self) -> State {
        State { k: vec![Vec::new(); self.cfg.layers], v: vec![Vec::new(); self.cfg.layers], pos: 0 }
    }

    /// Feed one token; the scores for the next one.
    fn step(&self, st: &mut State, token: u32) -> Vec<f32> {
        let c = &self.cfg;
        let (hd, kvd) = (c.head_dim(), c.kv_heads * c.head_dim());
        let mut x = vec![0.0; c.dim];
        self.embed.row(token as usize, &mut x);
        let mut h = vec![0.0; c.dim];
        let (mut q, mut k, mut v) = (vec![0.0; c.dim], vec![0.0; kvd], vec![0.0; kvd]);
        let mut att = vec![0.0; c.dim];
        let mut o = vec![0.0; c.dim];
        let (mut g, mut u) = (vec![0.0; c.hidden], vec![0.0; c.hidden]);
        for (l, w) in self.layers.iter().enumerate() {
            rmsnorm(&x, &w.attn_norm, c.eps, &mut h);
            w.wq.matvec(&h, &mut q);
            w.wk.matvec(&h, &mut k);
            w.wv.matvec(&h, &mut v);
            for head in q.chunks_mut(hd) {
                rope(head, st.pos, c.rope_theta);
            }
            for head in k.chunks_mut(hd) {
                rope(head, st.pos, c.rope_theta);
            }
            st.k[l].extend_from_slice(&k);
            st.v[l].extend_from_slice(&v);
            let n = st.pos + 1;
            let group = c.heads / c.kv_heads;
            for head in 0..c.heads {
                let kv = head / group;
                let qh = &q[head * hd..(head + 1) * hd];
                let mut scores: Vec<f32> = (0..n)
                    .map(|t| {
                        let kt = &st.k[l][t * kvd + kv * hd..t * kvd + (kv + 1) * hd];
                        qh.iter().zip(kt).map(|(a, b)| a * b).sum::<f32>() / (hd as f32).sqrt()
                    })
                    .collect();
                softmax(&mut scores);
                let out = &mut att[head * hd..(head + 1) * hd];
                out.fill(0.0);
                for (t, s) in scores.iter().enumerate() {
                    let vt = &st.v[l][t * kvd + kv * hd..t * kvd + (kv + 1) * hd];
                    for (a, b) in out.iter_mut().zip(vt) {
                        *a += s * b;
                    }
                }
            }
            w.wo.matvec(&att, &mut o);
            for (a, b) in x.iter_mut().zip(&o) {
                *a += b;
            }
            rmsnorm(&x, &w.mlp_norm, c.eps, &mut h);
            w.gate.matvec(&h, &mut g);
            w.up.matvec(&h, &mut u);
            for (a, b) in g.iter_mut().zip(&u) {
                *a = *a / (1.0 + (-*a).exp()) * b;
            }
            w.down.matvec(&g, &mut o);
            for (a, b) in x.iter_mut().zip(&o) {
                *a += b;
            }
        }
        st.pos += 1;
        rmsnorm(&x.clone(), &self.norm, c.eps, &mut x);
        let mut logits = vec![0.0; c.vocab];
        self.embed.matvec(&x, &mut logits);
        logits
    }

    /// The scores after each token of a sequence (for tests).
    #[cfg(test)]
    pub fn logits(&self, ids: &[u32]) -> Vec<Vec<f32>> {
        let mut st = self.state();
        ids.iter().map(|&t| self.step(&mut st, t)).collect()
    }

    /// A chat reply (ChatML, as SmolLM2-Instruct expects): the system's
    /// framing and the player's words in, a few sentences out.
    pub fn reply(&self, system: &str, user: &str, seed: u64) -> String {
        let t = &self.tok;
        let (Some(start), Some(end)) = (t.special("<|im_start|>"), t.special("<|im_end|>")) else { return String::new() };
        let mut ids = Vec::new();
        for (role, text) in [("system", system), ("user", user)] {
            ids.push(start);
            ids.extend(t.encode(&format!("{role}\n{text}")));
            ids.push(end);
            ids.extend(t.encode("\n"));
        }
        ids.push(start);
        ids.extend(t.encode("assistant\n"));
        // Keep the end of an overlong prompt.
        let keep = MAX_CONTEXT - MAX_NEW;
        if ids.len() > keep {
            ids.drain(..ids.len() - keep);
        }
        let mut st = self.state();
        let mut logits = Vec::new();
        for &id in &ids {
            logits = self.step(&mut st, id);
        }
        let mut rng = crate::noise::Rng::new(seed);
        let stop = [Some(end), t.special("<|endoftext|>")];
        let mut out = Vec::new();
        for _ in 0..MAX_NEW {
            let next = sample(&mut logits, 0.7, 40, &mut rng);
            if stop.contains(&Some(next)) {
                break;
            }
            out.push(next);
            logits = self.step(&mut st, next);
        }
        t.decode(&out).trim().to_string()
    }
}

fn softmax(x: &mut [f32]) {
    let m = x.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let mut sum = 0.0;
    for v in x.iter_mut() {
        *v = (*v - m).exp();
        sum += *v;
    }
    for v in x.iter_mut() {
        *v /= sum;
    }
}

/// Pick among the `k` likeliest at temperature `temp`.
fn sample(logits: &mut [f32], temp: f32, k: usize, rng: &mut crate::noise::Rng) -> u32 {
    let mut idx: Vec<usize> = (0..logits.len()).collect();
    idx.sort_by(|&a, &b| logits[b].total_cmp(&logits[a]));
    idx.truncate(k.max(1));
    let mut p: Vec<f32> = idx.iter().map(|&i| logits[i] / temp).collect();
    softmax(&mut p);
    let mut r = rng.f32();
    for (&i, &pi) in idx.iter().zip(&p) {
        r -= pi;
        if r <= 0.0 {
            return i as u32;
        }
    }
    idx[0] as u32
}

// ------------------------------------------------------------- the .lm file

fn put_u32(w: &mut Vec<u8>, n: usize) {
    w.extend((n as u32).to_le_bytes());
}
fn put_f32s(w: &mut Vec<u8>, v: &[f32]) {
    put_u32(w, v.len());
    for x in v {
        w.extend(x.to_le_bytes());
    }
}
fn put_q8(w: &mut Vec<u8>, m: &Q8) {
    put_u32(w, m.rows);
    put_u32(w, m.cols);
    put_f32s(w, &m.scale);
    w.extend(m.q.iter().map(|&v| v as u8));
}

struct Reader<'a>(&'a [u8]);
impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        if self.0.len() < n {
            return Err("the model file is cut short".into());
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a)
    }
    fn u32(&mut self) -> Result<usize, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()) as usize)
    }
    fn f32(&mut self) -> Result<f32, String> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn f32s(&mut self) -> Result<Vec<f32>, String> {
        let n = self.u32()?;
        (0..n).map(|_| self.f32()).collect()
    }
    fn q8(&mut self) -> Result<Q8, String> {
        let (rows, cols) = (self.u32()?, self.u32()?);
        let scale = self.f32s()?;
        let q = self.take(rows.checked_mul(cols).ok_or("bad size")?)?.iter().map(|&v| v as i8).collect();
        Ok(Q8 { rows, cols, scale, q })
    }
}

impl Model {
    pub fn to_bytes(&self) -> Vec<u8> {
        let c = &self.cfg;
        let mut w = MAGIC.to_vec();
        for n in [c.dim, c.hidden, c.layers, c.heads, c.kv_heads, c.vocab] {
            put_u32(&mut w, n);
        }
        w.extend(c.rope_theta.to_le_bytes());
        w.extend(c.eps.to_le_bytes());
        put_u32(&mut w, self.tok.tokens.len());
        for t in &self.tok.tokens {
            put_u32(&mut w, t.len());
            w.extend(t);
        }
        put_u32(&mut w, self.tok.merges.len());
        for &(a, b, m) in &self.tok.merges {
            for n in [a, b, m] {
                put_u32(&mut w, n as usize);
            }
        }
        put_u32(&mut w, self.tok.special.len());
        for (name, id) in &self.tok.special {
            put_u32(&mut w, name.len());
            w.extend(name.as_bytes());
            put_u32(&mut w, *id as usize);
        }
        put_q8(&mut w, &self.embed);
        for l in &self.layers {
            put_f32s(&mut w, &l.attn_norm);
            for m in [&l.wq, &l.wk, &l.wv, &l.wo] {
                put_q8(&mut w, m);
            }
            put_f32s(&mut w, &l.mlp_norm);
            for m in [&l.gate, &l.up, &l.down] {
                put_q8(&mut w, m);
            }
        }
        put_f32s(&mut w, &self.norm);
        w
    }

    pub fn from_bytes(b: &[u8]) -> Result<Model, String> {
        let mut r = Reader(b);
        if r.take(8)? != MAGIC {
            return Err("not a model file (or a newer one)".into());
        }
        let mut n = || r.u32();
        let (dim, hidden, layers, heads, kv_heads, vocab) = (n()?, n()?, n()?, n()?, n()?, n()?);
        let cfg = Config { dim, hidden, layers, heads, kv_heads, vocab, rope_theta: r.f32()?, eps: r.f32()? };
        if heads == 0 || kv_heads == 0 || heads % kv_heads != 0 || dim % heads != 0 || layers > 256 || vocab > 1 << 20 {
            return Err("the model's shape doesn't add up".into());
        }
        let mut tok = Tokenizer::default();
        for _ in 0..r.u32()? {
            let len = r.u32()?;
            tok.tokens.push(r.take(len)?.to_vec());
        }
        for _ in 0..r.u32()? {
            tok.merges.push((r.u32()? as u32, r.u32()? as u32, r.u32()? as u32));
        }
        for _ in 0..r.u32()? {
            let len = r.u32()?;
            let name = String::from_utf8_lossy(r.take(len)?).into_owned();
            tok.special.push((name, r.u32()? as u32));
        }
        let embed = r.q8()?;
        let mut ls = Vec::new();
        for _ in 0..layers {
            let attn_norm = r.f32s()?;
            let (wq, wk, wv, wo) = (r.q8()?, r.q8()?, r.q8()?, r.q8()?);
            let mlp_norm = r.f32s()?;
            let (gate, up, down) = (r.q8()?, r.q8()?, r.q8()?);
            ls.push(Layer { attn_norm, wq, wk, wv, wo, mlp_norm, gate, up, down });
        }
        let norm = r.f32s()?;
        let kvd = kv_heads * (dim / heads);
        let shapes_ok = embed.rows == vocab
            && embed.cols == dim
            && norm.len() == dim
            && ls.iter().all(|l| {
                (l.wq.rows, l.wq.cols, l.wk.rows, l.wv.rows, l.wo.rows, l.wo.cols) == (dim, dim, kvd, kvd, dim, dim)
                    && (l.gate.rows, l.up.rows, l.down.rows, l.down.cols) == (hidden, hidden, dim, hidden)
            });
        if !shapes_ok || tok.tokens.len() > vocab {
            return Err("the model's weights don't match its shape".into());
        }
        Ok(Model { cfg, tok, embed, layers: ls, norm })
    }

    pub fn load(path: &Path) -> Result<Model, String> {
        let mut bytes = Vec::new();
        std::fs::File::open(path).and_then(|mut f| f.read_to_end(&mut bytes)).map_err(|e| format!("{}: {e}", path.display()))?;
        Model::from_bytes(&bytes)
    }
}

// --------------------------------------------- converting from Hugging Face

/// Just enough JSON for config.json, tokenizer.json and a safetensors header.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn parse(s: &str) -> Result<Json, String> {
        let b = s.as_bytes();
        let mut i = 0;
        let v = json_value(b, &mut i)?;
        Ok(v)
    }
    pub fn get(&self, k: &str) -> Option<&Json> {
        match self {
            Json::Obj(kv) => kv.iter().find(|(a, _)| a == k).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn num(&self) -> Option<f64> {
        if let Json::Num(n) = self { Some(*n) } else { None }
    }
    pub fn str(&self) -> Option<&str> {
        if let Json::Str(s) = self { Some(s) } else { None }
    }
}

fn ws(b: &[u8], i: &mut usize) {
    while *i < b.len() && b[*i].is_ascii_whitespace() {
        *i += 1;
    }
}

fn json_value(b: &[u8], i: &mut usize) -> Result<Json, String> {
    ws(b, i);
    let bad = || "bad JSON".to_string();
    match b.get(*i).ok_or_else(bad)? {
        b'{' => {
            *i += 1;
            let mut kv = Vec::new();
            loop {
                ws(b, i);
                if b.get(*i) == Some(&b'}') {
                    *i += 1;
                    return Ok(Json::Obj(kv));
                }
                let Json::Str(k) = json_value(b, i)? else { return Err(bad()) };
                ws(b, i);
                if b.get(*i) != Some(&b':') {
                    return Err(bad());
                }
                *i += 1;
                kv.push((k, json_value(b, i)?));
                ws(b, i);
                if b.get(*i) == Some(&b',') {
                    *i += 1;
                }
            }
        }
        b'[' => {
            *i += 1;
            let mut v = Vec::new();
            loop {
                ws(b, i);
                if b.get(*i) == Some(&b']') {
                    *i += 1;
                    return Ok(Json::Arr(v));
                }
                v.push(json_value(b, i)?);
                ws(b, i);
                if b.get(*i) == Some(&b',') {
                    *i += 1;
                }
            }
        }
        b'"' => {
            *i += 1;
            let mut s = Vec::new();
            while let Some(&c) = b.get(*i) {
                *i += 1;
                match c {
                    b'"' => return Ok(Json::Str(String::from_utf8_lossy(&s).into_owned())),
                    b'\\' => {
                        let e = *b.get(*i).ok_or_else(bad)?;
                        *i += 1;
                        match e {
                            b'n' => s.push(b'\n'),
                            b't' => s.push(b'\t'),
                            b'r' => s.push(b'\r'),
                            b'b' => s.push(8),
                            b'f' => s.push(12),
                            b'u' => {
                                let hex = std::str::from_utf8(b.get(*i..*i + 4).ok_or_else(bad)?).map_err(|_| bad())?;
                                let mut code = u32::from_str_radix(hex, 16).map_err(|_| bad())?;
                                *i += 4;
                                // A surrogate pair.
                                if (0xD800..0xDC00).contains(&code) && b.get(*i..*i + 2) == Some(b"\\u") {
                                    let lo = u32::from_str_radix(std::str::from_utf8(b.get(*i + 2..*i + 6).ok_or_else(bad)?).map_err(|_| bad())?, 16).map_err(|_| bad())?;
                                    code = 0x10000 + ((code - 0xD800) << 10) + (lo - 0xDC00);
                                    *i += 6;
                                }
                                let mut buf = [0u8; 4];
                                s.extend(char::from_u32(code).unwrap_or('\u{FFFD}').encode_utf8(&mut buf).as_bytes());
                            }
                            o => s.push(o),
                        }
                    }
                    o => s.push(o),
                }
            }
            Err(bad())
        }
        b't' => {
            *i += 4;
            Ok(Json::Bool(true))
        }
        b'f' => {
            *i += 5;
            Ok(Json::Bool(false))
        }
        b'n' => {
            *i += 4;
            Ok(Json::Null)
        }
        _ => {
            let start = *i;
            while *i < b.len() && matches!(b[*i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                *i += 1;
            }
            std::str::from_utf8(&b[start..*i]).ok().and_then(|t| t.parse().ok()).map(Json::Num).ok_or_else(bad)
        }
    }
}

/// GPT-2's map from bytes to printable characters, turned round.
fn unicode_to_byte() -> HashMap<char, u8> {
    let mut printable: Vec<u32> = (b'!' as u32..=b'~' as u32).chain(0xA1..=0xAC).chain(0xAE..=0xFF).collect();
    let mut chars: Vec<u32> = printable.clone();
    let mut extra = 0;
    for b in 0..256u32 {
        if !printable.contains(&b) {
            printable.push(b);
            chars.push(256 + extra);
            extra += 1;
        }
    }
    printable.iter().zip(&chars).map(|(&b, &c)| (char::from_u32(c).unwrap(), b as u8)).collect()
}

/// Read a tokenizer.json (byte-level BPE).
pub fn tokenizer_from_json(j: &Json) -> Result<Tokenizer, String> {
    let model = j.get("model").ok_or("tokenizer.json has no model")?;
    let Some(Json::Obj(vocab)) = model.get("vocab") else { return Err("tokenizer.json has no vocab".into()) };
    let back = unicode_to_byte();
    let mut tokens: Vec<Vec<u8>> = Vec::new();
    let mut ids: HashMap<String, u32> = HashMap::new();
    for (piece, id) in vocab {
        let id = id.num().ok_or("bad vocab id")? as usize;
        if tokens.len() <= id {
            tokens.resize(id + 1, Vec::new());
        }
        tokens[id] = piece.chars().map(|c| back.get(&c).copied().unwrap_or(b'?')).collect();
        ids.insert(piece.clone(), id as u32);
    }
    let mut merges = Vec::new();
    if let Some(Json::Arr(list)) = model.get("merges") {
        for m in list {
            let (a, b) = match m {
                Json::Str(s) => s.split_once(' ').map(|(a, b)| (a.to_string(), b.to_string())).ok_or("bad merge")?,
                Json::Arr(p) if p.len() == 2 => (p[0].str().unwrap_or("").to_string(), p[1].str().unwrap_or("").to_string()),
                _ => return Err("bad merge".into()),
            };
            if let (Some(&x), Some(&y), Some(&m)) = (ids.get(&a), ids.get(&b), ids.get(&format!("{a}{b}"))) {
                merges.push((x, y, m));
            }
        }
    }
    let mut special = Vec::new();
    if let Some(Json::Arr(list)) = j.get("added_tokens") {
        for t in list {
            if let (Some(c), Some(id)) = (t.get("content").and_then(Json::str), t.get("id").and_then(Json::num)) {
                special.push((c.to_string(), id as u32));
                if tokens.len() <= id as usize {
                    tokens.resize(id as usize + 1, Vec::new());
                }
            }
        }
    }
    Ok(Tokenizer { tokens, merges, special })
}

/// The tensors in a .safetensors file, as floats (F32, F16 or BF16).
pub fn read_safetensors(bytes: &[u8]) -> Result<Tensors, String> {
    let n = u64::from_le_bytes(bytes.get(..8).ok_or("short file")?.try_into().unwrap()) as usize;
    let header = Json::parse(std::str::from_utf8(bytes.get(8..8 + n).ok_or("short file")?).map_err(|e| e.to_string())?)?;
    let data = &bytes[8 + n..];
    let Json::Obj(entries) = header else { return Err("bad header".into()) };
    let mut out = HashMap::new();
    for (name, t) in entries {
        if name == "__metadata__" {
            continue;
        }
        let dtype = t.get("dtype").and_then(Json::str).ok_or("no dtype")?;
        let shape: Vec<usize> = match t.get("shape") {
            Some(Json::Arr(s)) => s.iter().filter_map(Json::num).map(|v| v as usize).collect(),
            _ => return Err("no shape".into()),
        };
        let off: Vec<usize> = match t.get("data_offsets") {
            Some(Json::Arr(o)) => o.iter().filter_map(Json::num).map(|v| v as usize).collect(),
            _ => return Err("no offsets".into()),
        };
        let raw = data.get(off[0]..off[1]).ok_or("bad offsets")?;
        let vals: Vec<f32> = match dtype {
            "F32" => raw.chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect(),
            "BF16" => raw.chunks_exact(2).map(|c| f32::from_bits((u16::from_le_bytes([c[0], c[1]]) as u32) << 16)).collect(),
            "F16" => raw.chunks_exact(2).map(|c| f16_to_f32(u16::from_le_bytes([c[0], c[1]]))).collect(),
            other => return Err(format!("{name}: can't read {other} weights")),
        };
        out.insert(name, (shape, vals));
    }
    Ok(out)
}

fn f16_to_f32(h: u16) -> f32 {
    let (s, e, m) = ((h >> 15) as u32, ((h >> 10) & 0x1F) as u32, (h & 0x3FF) as u32);
    let bits = match e {
        0 if m == 0 => s << 31,
        0 => {
            // Subnormal.
            let mut e = 127 - 15 + 1;
            let mut m = m;
            while m & 0x400 == 0 {
                m <<= 1;
                e -= 1;
            }
            (s << 31) | (e << 23) | ((m & 0x3FF) << 13)
        }
        31 => (s << 31) | (0xFF << 23) | (m << 13),
        _ => (s << 31) | ((e + 127 - 15) << 23) | (m << 13),
    };
    f32::from_bits(bits)
}

/// Build a model from a Hugging Face folder (config.json, tokenizer.json, model.safetensors).
pub fn convert(dir: &Path) -> Result<Model, String> {
    let read = |f: &str| std::fs::read(dir.join(f)).map_err(|e| format!("{f}: {e}"));
    let cfg_json = Json::parse(&String::from_utf8_lossy(&read("config.json")?))?;
    let num = |k: &str| cfg_json.get(k).and_then(Json::num).ok_or(format!("config.json: no {k}"));
    let cfg = Config {
        dim: num("hidden_size")? as usize,
        hidden: num("intermediate_size")? as usize,
        layers: num("num_hidden_layers")? as usize,
        heads: num("num_attention_heads")? as usize,
        kv_heads: num("num_key_value_heads")? as usize,
        vocab: num("vocab_size")? as usize,
        rope_theta: num("rope_theta").unwrap_or(10000.0) as f32,
        eps: num("rms_norm_eps").unwrap_or(1e-5) as f32,
    };
    let tok = tokenizer_from_json(&Json::parse(&String::from_utf8_lossy(&read("tokenizer.json")?))?)?;
    let mut t = read_safetensors(&read("model.safetensors")?)?;
    let mut q = |name: &str| -> Result<Q8, String> {
        let (shape, w) = t.remove(name).ok_or(format!("no tensor {name}"))?;
        Ok(Q8::quantize(shape[0], shape[1..].iter().product(), &w))
    };
    let embed = q("model.embed_tokens.weight")?;
    let mut layers = Vec::new();
    for l in 0..cfg.layers {
        let p = format!("model.layers.{l}.");
        layers.push(Layer {
            attn_norm: Vec::new(),
            wq: q(&format!("{p}self_attn.q_proj.weight"))?,
            wk: q(&format!("{p}self_attn.k_proj.weight"))?,
            wv: q(&format!("{p}self_attn.v_proj.weight"))?,
            wo: q(&format!("{p}self_attn.o_proj.weight"))?,
            mlp_norm: Vec::new(),
            gate: q(&format!("{p}mlp.gate_proj.weight"))?,
            up: q(&format!("{p}mlp.up_proj.weight"))?,
            down: q(&format!("{p}mlp.down_proj.weight"))?,
        });
    }
    let mut v = |name: &str| t.remove(name).map(|(_, w)| w).ok_or(format!("no tensor {name}"));
    for (l, layer) in layers.iter_mut().enumerate() {
        layer.attn_norm = v(&format!("model.layers.{l}.input_layernorm.weight"))?;
        layer.mlp_norm = v(&format!("model.layers.{l}.post_attention_layernorm.weight"))?;
    }
    let norm = v("model.norm.weight")?;
    let m = Model { cfg, tok, embed, layers, norm };
    // (Check it reads back.)
    Model::from_bytes(&m.to_bytes())
}

/// `--pack-model <hf-folder> <out.lm>`, and `--ask-model <file.lm> <question>` to try one.
pub fn cli(args: &[String]) -> Option<i32> {
    let at = |flag: &str| args.iter().position(|a| a == flag);
    if let Some(i) = at("--pack-model") {
        let (Some(dir), Some(out)) = (args.get(i + 1), args.get(i + 2)) else {
            eprintln!("usage: --pack-model <folder with config.json, tokenizer.json, model.safetensors> <out.lm>");
            return Some(2);
        };
        return Some(match convert(Path::new(dir)).and_then(|m| std::fs::File::create(out).and_then(|mut f| f.write_all(&m.to_bytes())).map_err(|e| e.to_string())) {
            Ok(()) => {
                println!("wrote {out}");
                0
            }
            Err(e) => {
                eprintln!("couldn't pack the model: {e}");
                1
            }
        });
    }
    if let Some(i) = at("--ask-model") {
        let (Some(file), Some(q)) = (args.get(i + 1), args.get(i + 2)) else {
            eprintln!("usage: --ask-model <file.lm> <question>");
            return Some(2);
        };
        return Some(match Model::load(Path::new(file)) {
            Ok(m) => {
                let t = std::time::Instant::now();
                println!("{}", m.reply("You are a helpful assistant. Answer in one short sentence.", q, 1));
                eprintln!("({:.1}s)", t.elapsed().as_secs_f32());
                0
            }
            Err(e) => {
                eprintln!("{e}");
                1
            }
        });
    }
    None
}

// ------------------------------------------------------------------ the worker

/// Where a mod's model lives.
pub fn model_path(mod_id: &str) -> PathBuf {
    crate::mods::mods_dir().join(mod_id).join("model.lm")
}

pub struct Ask {
    pub mod_id: String,
    pub player: String,
    pub system: String,
    pub prompt: String,
}

/// Questions go to a thread that answers them in turn (loading each mod's
/// model the first time it's needed).
pub struct Worker {
    tx: mpsc::Sender<Ask>,
    rx: mpsc::Receiver<(String, String, String)>,
    /// Questions not yet answered.
    pub pending: usize,
}

impl Worker {
    pub fn start() -> Worker {
        let (tx, ask_rx) = mpsc::channel::<Ask>();
        let (reply_tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("model".into())
            .spawn(move || {
                let mut models: HashMap<String, Result<Model, String>> = HashMap::new();
                let mut seed = 1u64;
                while let Ok(a) = ask_rx.recv() {
                    let m = models.entry(a.mod_id.clone()).or_insert_with(|| Model::load(&model_path(&a.mod_id)));
                    let text = match m {
                        Ok(m) => m.reply(&a.system, &a.prompt, seed),
                        Err(e) => format!("(the model couldn't load: {e})"),
                    };
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    if reply_tx.send((a.mod_id, a.player, text)).is_err() {
                        break;
                    }
                }
            })
            .expect("couldn't start the model thread");
        Worker { tx, rx, pending: 0 }
    }

    pub fn ask(&mut self, a: Ask) {
        if self.pending < 8 && self.tx.send(a).is_ok() {
            self.pending += 1;
        }
    }

    /// Answers that are ready: (mod, player, text).
    pub fn replies(&mut self) -> Vec<(String, String, String)> {
        let v: Vec<_> = self.rx.try_iter().collect();
        self.pending = self.pending.saturating_sub(v.len());
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny model with random weights and a real (if small) byte-level tokenizer.
    fn tiny() -> Model {
        let mut rng = crate::noise::Rng::new(9);
        let cfg = Config { dim: 16, hidden: 24, layers: 2, heads: 4, kv_heads: 2, vocab: 300, rope_theta: 10000.0, eps: 1e-5 };
        let mut m = |r: usize, c: usize| Q8::quantize(r, c, &(0..r * c).map(|_| rng.range(-0.5, 0.5)).collect::<Vec<f32>>());
        let kvd = cfg.kv_heads * cfg.dim / cfg.heads;
        let layers = (0..cfg.layers)
            .map(|_| Layer {
                attn_norm: vec![1.0; 16],
                wq: m(16, 16),
                wk: m(kvd, 16),
                wv: m(kvd, 16),
                wo: m(16, 16),
                mlp_norm: vec![1.0; 16],
                gate: m(24, 16),
                up: m(24, 16),
                down: m(16, 24),
            })
            .collect();
        let embed = m(300, 16);
        let mut tokens: Vec<Vec<u8>> = (0..256u32).map(|b| vec![b as u8]).collect();
        tokens.push(b"th".to_vec()); // 256
        tokens.push(b"the".to_vec()); // 257
        tokens.push(b" the".to_vec()); // 258
        tokens.resize(300, Vec::new());
        let merges = vec![(b't' as u32, b'h' as u32, 256), (256, b'e' as u32, 257), (b' ' as u32, 257, 258)];
        let special = vec![("<|im_start|>".to_string(), 297), ("<|im_end|>".to_string(), 298), ("<|endoftext|>".to_string(), 299)];
        Model { cfg, tok: Tokenizer { tokens, merges, special }, embed, layers, norm: vec![1.0; 16] }
    }

    #[test]
    fn words_split_like_gpt2() {
        assert_eq!(pre_split("Hello world, it's 42!"), vec!["Hello", " world", ",", " it", "'s", " ", "4", "2", "!"]);
        assert_eq!(pre_split("a  b"), vec!["a", " ", " b"]);
    }

    #[test]
    fn the_tokenizer_merges_and_comes_back() {
        let m = tiny();
        let ids = m.tok.encode("the cat, the end");
        assert_eq!(&ids[..1], &[257], "\"the\" in one");
        assert!(ids.contains(&258), "\" the\" in one");
        assert_eq!(m.tok.decode(&ids), "the cat, the end");
    }

    /// The fast path (cached keys, grouped heads, threads) matches a slow,
    /// obvious one that recomputes everything at every step.
    #[test]
    fn the_cached_forward_pass_matches_a_naive_one() {
        let m = tiny();
        let ids = [5u32, 257, 40, 258, 7, 7];
        let fast = m.logits(&ids);
        let c = m.cfg;
        let hd = c.dim / c.heads;
        let deq = |q: &Q8| -> Vec<Vec<f32>> {
            (0..q.rows).map(|r| {
                let mut v = vec![0.0; q.cols];
                q.row(r, &mut v);
                v
            }).collect()
        };
        let mv = |w: &Vec<Vec<f32>>, x: &[f32]| -> Vec<f32> { w.iter().map(|r| r.iter().zip(x).map(|(a, b)| a * b).sum()).collect() };
        let norm = |x: &[f32]| -> Vec<f32> {
            let ms = x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32;
            x.iter().map(|v| v / (ms + c.eps).sqrt()).collect()
        };
        let rot = |v: &[f32], pos: usize| -> Vec<f32> {
            let half = v.len() / 2;
            let mut o = v.to_vec();
            for i in 0..half {
                let a = pos as f32 * c.rope_theta.powf(-((2 * i) as f32) / v.len() as f32);
                o[i] = v[i] * a.cos() - v[i + half] * a.sin();
                o[i + half] = v[i + half] * a.cos() + v[i] * a.sin();
            }
            o
        };
        let emb = deq(&m.embed);
        for n in 1..=ids.len() {
            // Every position's hidden state, from scratch.
            let mut xs: Vec<Vec<f32>> = ids[..n].iter().map(|&t| emb[t as usize].clone()).collect();
            for w in &m.layers {
                let (wq, wk, wv, wo) = (deq(&w.wq), deq(&w.wk), deq(&w.wv), deq(&w.wo));
                let hs: Vec<Vec<f32>> = xs.iter().map(|x| norm(x)).collect();
                let qs: Vec<Vec<f32>> = hs.iter().map(|h| mv(&wq, h)).collect();
                let ks: Vec<Vec<f32>> = hs.iter().map(|h| mv(&wk, h)).collect();
                let vs: Vec<Vec<f32>> = hs.iter().map(|h| mv(&wv, h)).collect();
                for p in 0..n {
                    let mut att = vec![0.0; c.dim];
                    for head in 0..c.heads {
                        let kv = head * c.kv_heads / c.heads;
                        let q = rot(&qs[p][head * hd..(head + 1) * hd], p);
                        let mut s: Vec<f32> = (0..=p).map(|t| {
                            let k = rot(&ks[t][kv * hd..(kv + 1) * hd], t);
                            q.iter().zip(&k).map(|(a, b)| a * b).sum::<f32>() / (hd as f32).sqrt()
                        }).collect();
                        softmax(&mut s);
                        for (t, st) in s.iter().enumerate() {
                            for i in 0..hd {
                                att[head * hd + i] += st * vs[t][kv * hd + i];
                            }
                        }
                    }
                    let o = mv(&wo, &att);
                    for i in 0..c.dim {
                        xs[p][i] += o[i];
                    }
                }
                let (g, u, d) = (deq(&w.gate), deq(&w.up), deq(&w.down));
                for x in xs.iter_mut() {
                    let h = norm(x);
                    let (a, b) = (mv(&g, &h), mv(&u, &h));
                    let act: Vec<f32> = a.iter().zip(&b).map(|(a, b)| a / (1.0 + (-a).exp()) * b).collect();
                    let o = mv(&d, &act);
                    for i in 0..c.dim {
                        x[i] += o[i];
                    }
                }
            }
            let slow = mv(&emb, &norm(&xs[n - 1]));
            let worst = slow.iter().zip(&fast[n - 1]).map(|(a, b)| (a - b).abs()).fold(0.0, f32::max);
            assert!(worst < 1e-3, "step {n}: off by {worst}");
        }
    }

    #[test]
    fn the_file_round_trips_and_shrugs_off_bad_data() {
        let m = tiny();
        let bytes = m.to_bytes();
        assert_eq!(Model::from_bytes(&bytes).unwrap(), m);
        for cut in [0, 7, 8, 40, bytes.len() / 2, bytes.len() - 1] {
            assert!(Model::from_bytes(&bytes[..cut]).is_err());
        }
        // And it answers (gibberish, from random weights) without falling over.
        let r = m.reply("You are terse.", "Say the thing.", 3);
        assert!(r.len() < 1000);
    }

    /// The converter reads Hugging Face's files: a tiny model written out as
    /// config.json, tokenizer.json and BF16 safetensors comes back the same.
    #[test]
    fn hugging_face_files_convert() {
        let m = tiny();
        let dir = std::env::temp_dir().join(format!("mclm-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let c = m.cfg;
        std::fs::write(dir.join("config.json"), format!(
            r#"{{"hidden_size": {}, "intermediate_size": {}, "num_hidden_layers": {}, "num_attention_heads": {}, "num_key_value_heads": {}, "vocab_size": {}, "rope_theta": 10000.0, "rms_norm_eps": 1e-05, "tie_word_embeddings": true}}"#,
            c.dim, c.hidden, c.layers, c.heads, c.kv_heads, c.vocab
        )).unwrap();
        // The byte-level vocabulary as GPT-2 writes it.
        let to_char: HashMap<u8, char> = unicode_to_byte().into_iter().map(|(ch, b)| (b, ch)).collect();
        let piece = |t: &[u8]| -> String { t.iter().map(|b| to_char[b]).collect() };
        let esc = |s: &str| s.chars().map(|ch| if ch == '"' || ch == '\\' { format!("\\{ch}") } else if (ch as u32) < 32 { format!("\\u{:04x}", ch as u32) } else { ch.to_string() }).collect::<String>();
        let vocab: Vec<String> = m.tok.tokens.iter().enumerate().filter(|(_, t)| !t.is_empty()).map(|(i, t)| format!("\"{}\": {i}", esc(&piece(t)))).collect();
        let merges: Vec<String> = m.tok.merges.iter().map(|&(a, b, _)| format!("\"{} {}\"", esc(&piece(&m.tok.tokens[a as usize])), esc(&piece(&m.tok.tokens[b as usize])))).collect();
        let added: Vec<String> = m.tok.special.iter().map(|(n, id)| format!(r#"{{"id": {id}, "content": "{n}"}}"#)).collect();
        std::fs::write(dir.join("tokenizer.json"), format!(r#"{{"added_tokens": [{}], "model": {{"type": "BPE", "vocab": {{{}}}, "merges": [{}]}}}}"#, added.join(","), vocab.join(","), merges.join(","))).unwrap();
        // The weights, as BF16.
        let mut tensors: Vec<(String, Vec<usize>, Vec<f32>)> = Vec::new();
        let full = |q: &Q8| -> Vec<f32> { (0..q.rows).flat_map(|r| { let mut v = vec![0.0; q.cols]; q.row(r, &mut v); v }).collect() };
        tensors.push(("model.embed_tokens.weight".into(), vec![m.embed.rows, m.embed.cols], full(&m.embed)));
        for (l, w) in m.layers.iter().enumerate() {
            let p = format!("model.layers.{l}.");
            for (n, q) in [("self_attn.q_proj", &w.wq), ("self_attn.k_proj", &w.wk), ("self_attn.v_proj", &w.wv), ("self_attn.o_proj", &w.wo), ("mlp.gate_proj", &w.gate), ("mlp.up_proj", &w.up), ("mlp.down_proj", &w.down)] {
                tensors.push((format!("{p}{n}.weight"), vec![q.rows, q.cols], full(q)));
            }
            tensors.push((format!("{p}input_layernorm.weight"), vec![c.dim], w.attn_norm.clone()));
            tensors.push((format!("{p}post_attention_layernorm.weight"), vec![c.dim], w.mlp_norm.clone()));
        }
        tensors.push(("model.norm.weight".into(), vec![c.dim], m.norm.clone()));
        let mut data = Vec::new();
        let mut header = Vec::new();
        for (name, shape, vals) in &tensors {
            let start = data.len();
            for v in vals {
                data.extend(((v.to_bits() + 0x8000) >> 16).to_le_bytes()[..2].to_vec());
            }
            header.push(format!(r#""{name}": {{"dtype": "BF16", "shape": {shape:?}, "data_offsets": [{start}, {}]}}"#, data.len()));
        }
        let header = format!("{{{}}}", header.join(","));
        let mut file = (header.len() as u64).to_le_bytes().to_vec();
        file.extend(header.as_bytes());
        file.extend(data);
        std::fs::write(dir.join("model.safetensors"), file).unwrap();
        let got = convert(&dir).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(got.cfg, m.cfg);
        assert_eq!(got.tok.special, m.tok.special);
        assert_eq!(got.tok.encode("the cat, the end"), m.tok.encode("the cat, the end"));
        // Close, given BF16 and requantizing.
        let (a, b) = (got.logits(&[5, 257, 40]), m.logits(&[5, 257, 40]));
        let worst = a[2].iter().zip(&b[2]).map(|(x, y)| (x - y).abs()).fold(0.0, f32::max);
        assert!(worst < 0.1, "off by {worst}");
    }

    #[test]
    fn json_reads_what_hugging_face_writes() {
        let j = Json::parse(r#"{"a": [1, -2.5e3, "xé\n"], "b": {"c": true, "d": null}}"#).unwrap();
        assert_eq!(j.get("a"), Some(&Json::Arr(vec![Json::Num(1.0), Json::Num(-2500.0), Json::Str("xé\n".into())])));
        assert_eq!(j.get("b").and_then(|b| b.get("c")), Some(&Json::Bool(true)));
    }
}
