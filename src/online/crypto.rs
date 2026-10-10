//! The cryptography behind encrypted multiplayer, written out by hand so the
//! game still needs nothing but the standard library for its networking.
//!
//! - **SHA-256**, **HMAC-SHA-256** and **HKDF** (RFC 2104, 5869) for hashing
//!   and turning a shared secret into keys.
//! - **X25519** (RFC 7748) for the key exchange.
//! - **ChaCha20-Poly1305** (RFC 8439) for the encrypted, tamper-proof stream.
//! - Random bytes from the operating system (`/dev/urandom`, or
//!   `RtlGenRandom` on Windows), stirred with a few other sources.
//!
//! Every piece is checked against the test vectors from its RFC (see the
//! tests below). None of it tries to be fast; a game's traffic is small.

// ------------------------------------------------------------------ SHA-256

/// Plain SHA-256 (FIPS 180-4).
pub fn sha256(data: &[u8]) -> [u8; 32] {
    sha256_parts(&[data])
}

/// SHA-256 of several pieces, as if joined.
pub fn sha256_parts(parts: &[&[u8]]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
        0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
        0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
        0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let total: usize = parts.iter().map(|p| p.len()).sum();
    let mut msg = Vec::with_capacity(total + 72);
    for p in parts {
        msg.extend_from_slice(p);
    }
    let bit_len = (total as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for block in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(block[i * 4..i * 4 + 4].try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (x, v) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *x = x.wrapping_add(v);
        }
    }
    let mut out = [0u8; 32];
    for (i, v) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
    }
    out
}

/// HMAC-SHA-256 (RFC 2104).
pub fn hmac(key: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        k[..32].copy_from_slice(&sha256(key));
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let ipad: Vec<u8> = k.iter().map(|b| b ^ 0x36).collect();
    let opad: Vec<u8> = k.iter().map(|b| b ^ 0x5c).collect();
    let mut inner: Vec<&[u8]> = vec![&ipad];
    inner.extend_from_slice(parts);
    let ih = sha256_parts(&inner);
    sha256_parts(&[&opad, &ih])
}

/// HKDF-SHA-256 (RFC 5869): `len` bytes of key from secret `ikm`.
pub fn hkdf(salt: &[u8], ikm: &[u8], info: &[u8], len: usize) -> Vec<u8> {
    let prk = hmac(salt, &[ikm]);
    let mut out = Vec::with_capacity(len);
    let mut t: Vec<u8> = Vec::new();
    let mut i = 1u8;
    while out.len() < len {
        t = hmac(&prk, &[&t, info, &[i]]).to_vec();
        out.extend_from_slice(&t);
        i += 1;
    }
    out.truncate(len);
    out
}

/// Compare without leaking where two secrets first differ.
pub fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

// ------------------------------------------------------------------ X25519

/// A number mod 2^255 - 19, in five 51-bit limbs.
type Fe = [u64; 5];
const M51: u64 = (1 << 51) - 1;

fn fe_load(b: &[u8; 32]) -> Fe {
    let at = |i: usize| u64::from_le_bytes(b[i..i + 8].try_into().unwrap());
    [at(0) & M51, (at(6) >> 3) & M51, (at(12) >> 6) & M51, (at(19) >> 1) & M51, (at(24) >> 12) & M51]
}

fn fe_carry(mut h: Fe) -> Fe {
    for i in 0..4 {
        h[i + 1] += h[i] >> 51;
        h[i] &= M51;
    }
    h[0] += 19 * (h[4] >> 51);
    h[4] &= M51;
    h[1] += h[0] >> 51;
    h[0] &= M51;
    h
}

fn fe_add(a: &Fe, b: &Fe) -> Fe {
    fe_carry([a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3], a[4] + b[4]])
}

fn fe_sub(a: &Fe, b: &Fe) -> Fe {
    // Add 4p first so nothing goes below zero.
    const P4: Fe = [0x1F_FFFF_FFFF_FFB4, 0x1F_FFFF_FFFF_FFFC, 0x1F_FFFF_FFFF_FFFC, 0x1F_FFFF_FFFF_FFFC, 0x1F_FFFF_FFFF_FFFC];
    fe_carry([a[0] + P4[0] - b[0], a[1] + P4[1] - b[1], a[2] + P4[2] - b[2], a[3] + P4[3] - b[3], a[4] + P4[4] - b[4]])
}

fn fe_mul(a: &Fe, b: &Fe) -> Fe {
    let m = |x: u64, y: u64| x as u128 * y as u128;
    let (b1, b2, b3, b4) = (b[1] * 19, b[2] * 19, b[3] * 19, b[4] * 19);
    let r0 = m(a[0], b[0]) + m(a[1], b4) + m(a[2], b3) + m(a[3], b2) + m(a[4], b1);
    let mut r1 = m(a[0], b[1]) + m(a[1], b[0]) + m(a[2], b4) + m(a[3], b3) + m(a[4], b2);
    let mut r2 = m(a[0], b[2]) + m(a[1], b[1]) + m(a[2], b[0]) + m(a[3], b4) + m(a[4], b3);
    let mut r3 = m(a[0], b[3]) + m(a[1], b[2]) + m(a[2], b[1]) + m(a[3], b[0]) + m(a[4], b4);
    let mut r4 = m(a[0], b[4]) + m(a[1], b[3]) + m(a[2], b[2]) + m(a[3], b[1]) + m(a[4], b[0]);
    r1 += r0 >> 51;
    r2 += r1 >> 51;
    r3 += r2 >> 51;
    r4 += r3 >> 51;
    let mut h0 = (r0 as u64 & M51) + 19 * (r4 >> 51) as u64;
    let mut h1 = r1 as u64 & M51;
    h1 += h0 >> 51;
    h0 &= M51;
    [h0, h1, r2 as u64 & M51, r3 as u64 & M51, r4 as u64 & M51]
}

fn fe_sq(a: &Fe) -> Fe {
    fe_mul(a, a)
}

/// a^(p-2) = 1/a.
fn fe_invert(a: &Fe) -> Fe {
    // p - 2 = 2^255 - 21, little-endian.
    let mut e = [0xffu8; 32];
    e[0] = 0xeb;
    e[31] = 0x7f;
    let mut r: Fe = [1, 0, 0, 0, 0];
    for bit in (0..255).rev() {
        r = fe_sq(&r);
        if (e[bit / 8] >> (bit % 8)) & 1 == 1 {
            r = fe_mul(&r, a);
        }
    }
    r
}

fn fe_store(a: &Fe) -> [u8; 32] {
    let mut h = fe_carry(fe_carry(*a));
    // Subtract p if h >= p.
    let mut q = (h[0] + 19) >> 51;
    for limb in h.iter().skip(1) {
        q = (limb + q) >> 51;
    }
    h[0] += 19 * q;
    for i in 0..4 {
        h[i + 1] += h[i] >> 51;
        h[i] &= M51;
    }
    h[4] &= M51;
    let mut out = [0u8; 32];
    let mut acc: u128 = 0;
    let mut bits = 0;
    let mut at = 0;
    for limb in h {
        acc |= (limb as u128) << bits;
        bits += 51;
        while bits >= 8 && at < 32 {
            out[at] = acc as u8;
            acc >>= 8;
            bits -= 8;
            at += 1;
        }
    }
    if at < 32 {
        out[at] = acc as u8;
    }
    out
}

fn fe_cswap(a: &mut Fe, b: &mut Fe, swap: u64) {
    let mask = 0u64.wrapping_sub(swap);
    for i in 0..5 {
        let t = mask & (a[i] ^ b[i]);
        a[i] ^= t;
        b[i] ^= t;
    }
}

/// X25519: `scalar` times the point with x-coordinate `u`.
pub fn x25519(scalar: &[u8; 32], u: &[u8; 32]) -> [u8; 32] {
    let mut k = *scalar;
    k[0] &= 248;
    k[31] &= 127;
    k[31] |= 64;
    let x1 = fe_load(u);
    let (mut x2, mut z2, mut x3, mut z3): (Fe, Fe, Fe, Fe) = ([1, 0, 0, 0, 0], [0; 5], x1, [1, 0, 0, 0, 0]);
    let a24: Fe = [121665, 0, 0, 0, 0];
    let mut swap = 0u64;
    for t in (0..255).rev() {
        let bit = ((k[t / 8] >> (t % 8)) & 1) as u64;
        swap ^= bit;
        fe_cswap(&mut x2, &mut x3, swap);
        fe_cswap(&mut z2, &mut z3, swap);
        swap = bit;
        let a = fe_add(&x2, &z2);
        let aa = fe_sq(&a);
        let b = fe_sub(&x2, &z2);
        let bb = fe_sq(&b);
        let e = fe_sub(&aa, &bb);
        let c = fe_add(&x3, &z3);
        let d = fe_sub(&x3, &z3);
        let da = fe_mul(&d, &a);
        let cb = fe_mul(&c, &b);
        x3 = fe_sq(&fe_add(&da, &cb));
        z3 = fe_mul(&x1, &fe_sq(&fe_sub(&da, &cb)));
        x2 = fe_mul(&aa, &bb);
        z2 = fe_mul(&e, &fe_add(&aa, &fe_mul(&a24, &e)));
    }
    fe_cswap(&mut x2, &mut x3, swap);
    fe_cswap(&mut z2, &mut z3, swap);
    fe_store(&fe_mul(&x2, &fe_invert(&z2)))
}

/// The public key for a secret one.
pub fn x25519_public(secret: &[u8; 32]) -> [u8; 32] {
    let mut nine = [0u8; 32];
    nine[0] = 9;
    x25519(secret, &nine)
}

/// A key pair: (secret, public).
pub fn keypair() -> ([u8; 32], [u8; 32]) {
    let mut s = [0u8; 32];
    random_bytes(&mut s);
    (s, x25519_public(&s))
}

// ------------------------------------------------------------------ ChaCha20-Poly1305

fn quarter(s: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    s[a] = s[a].wrapping_add(s[b]);
    s[d] = (s[d] ^ s[a]).rotate_left(16);
    s[c] = s[c].wrapping_add(s[d]);
    s[b] = (s[b] ^ s[c]).rotate_left(12);
    s[a] = s[a].wrapping_add(s[b]);
    s[d] = (s[d] ^ s[a]).rotate_left(8);
    s[c] = s[c].wrapping_add(s[d]);
    s[b] = (s[b] ^ s[c]).rotate_left(7);
}

/// One 64-byte ChaCha20 block.
pub fn chacha20_block(key: &[u8; 32], counter: u32, nonce: &[u8; 12]) -> [u8; 64] {
    let word = |b: &[u8], i: usize| u32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap());
    let mut init = [0u32; 16];
    init[..4].copy_from_slice(&[0x61707865, 0x3320646e, 0x79622d32, 0x6b206574]);
    for i in 0..8 {
        init[4 + i] = word(key, i);
    }
    init[12] = counter;
    for i in 0..3 {
        init[13 + i] = word(nonce, i);
    }
    let mut s = init;
    for _ in 0..10 {
        quarter(&mut s, 0, 4, 8, 12);
        quarter(&mut s, 1, 5, 9, 13);
        quarter(&mut s, 2, 6, 10, 14);
        quarter(&mut s, 3, 7, 11, 15);
        quarter(&mut s, 0, 5, 10, 15);
        quarter(&mut s, 1, 6, 11, 12);
        quarter(&mut s, 2, 7, 8, 13);
        quarter(&mut s, 3, 4, 9, 14);
    }
    let mut out = [0u8; 64];
    for i in 0..16 {
        out[i * 4..i * 4 + 4].copy_from_slice(&s[i].wrapping_add(init[i]).to_le_bytes());
    }
    out
}

/// XOR `data` with the ChaCha20 stream from block `counter` on.
pub fn chacha20_xor(key: &[u8; 32], counter: u32, nonce: &[u8; 12], data: &mut [u8]) {
    for (i, chunk) in data.chunks_mut(64).enumerate() {
        let ks = chacha20_block(key, counter.wrapping_add(i as u32), nonce);
        for (b, k) in chunk.iter_mut().zip(ks) {
            *b ^= k;
        }
    }
}

/// Poly1305 (RFC 8439 2.5): a one-time authenticator.
pub fn poly1305(key: &[u8; 32], msg: &[u8]) -> [u8; 16] {
    const M44: u64 = (1 << 44) - 1;
    const M42: u64 = (1 << 42) - 1;
    let le = |b: &[u8]| u64::from_le_bytes(b.try_into().unwrap());
    let (t0, t1) = (le(&key[0..8]), le(&key[8..16]));
    let r0 = t0 & 0xffc0fffffff;
    let r1 = ((t0 >> 44) | (t1 << 20)) & 0xfffffc0ffff;
    let r2 = (t1 >> 24) & 0x00ffffffc0f;
    let (s1, s2) = (r1 * 20, r2 * 20);
    let (mut h0, mut h1, mut h2) = (0u64, 0u64, 0u64);
    let m = |x: u64, y: u64| x as u128 * y as u128;
    for chunk in msg.chunks(16) {
        let mut block = [0u8; 16];
        block[..chunk.len()].copy_from_slice(chunk);
        let hibit = if chunk.len() == 16 {
            1u64 << 40
        } else {
            block[chunk.len()] = 1;
            0
        };
        let (t0, t1) = (le(&block[0..8]), le(&block[8..16]));
        h0 += t0 & M44;
        h1 += ((t0 >> 44) | (t1 << 20)) & M44;
        h2 += ((t1 >> 24) & M42) | hibit;
        let d0 = m(h0, r0) + m(h1, s2) + m(h2, s1);
        let mut d1 = m(h0, r1) + m(h1, r0) + m(h2, s2);
        let mut d2 = m(h0, r2) + m(h1, r1) + m(h2, r0);
        d1 += d0 >> 44;
        h0 = d0 as u64 & M44;
        d2 += d1 >> 44;
        h1 = d1 as u64 & M44;
        let c = (d2 >> 42) as u64;
        h2 = d2 as u64 & M42;
        h0 += c * 5;
        h1 += h0 >> 44;
        h0 &= M44;
    }
    // Fully carry h.
    let mut c = h1 >> 44;
    h1 &= M44;
    h2 += c;
    c = h2 >> 42;
    h2 &= M42;
    h0 += c * 5;
    c = h0 >> 44;
    h0 &= M44;
    h1 += c;
    c = h1 >> 44;
    h1 &= M44;
    h2 += c;
    c = h2 >> 42;
    h2 &= M42;
    h0 += c * 5;
    c = h0 >> 44;
    h0 &= M44;
    h1 += c;
    // h - p, chosen if it doesn't go negative.
    let mut g0 = h0 + 5;
    c = g0 >> 44;
    g0 &= M44;
    let mut g1 = h1 + c;
    c = g1 >> 44;
    g1 &= M44;
    let g2 = (h2 + c).wrapping_sub(1 << 42);
    let keep_g = (g2 >> 63).wrapping_sub(1);
    h0 = (h0 & !keep_g) | (g0 & keep_g);
    h1 = (h1 & !keep_g) | (g1 & keep_g);
    h2 = (h2 & !keep_g) | (g2 & keep_g);
    // Add s.
    let (t0, t1) = (le(&key[16..24]), le(&key[24..32]));
    h0 += t0 & M44;
    c = h0 >> 44;
    h0 &= M44;
    h1 += (((t0 >> 44) | (t1 << 20)) & M44) + c;
    c = h1 >> 44;
    h1 &= M44;
    h2 += ((t1 >> 24) & M42) + c;
    h2 &= M42;
    let lo = h0 | (h1 << 44);
    let hi = (h1 >> 20) | (h2 << 24);
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&lo.to_le_bytes());
    out[8..].copy_from_slice(&hi.to_le_bytes());
    out
}

fn aead_tag(key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], ct: &[u8]) -> [u8; 16] {
    let block = chacha20_block(key, 0, nonce);
    let otk: [u8; 32] = block[..32].try_into().unwrap();
    let pad = |n: usize| vec![0u8; (16 - n % 16) % 16];
    let mut mac = Vec::with_capacity(aad.len() + ct.len() + 48);
    mac.extend_from_slice(aad);
    mac.extend(pad(aad.len()));
    mac.extend_from_slice(ct);
    mac.extend(pad(ct.len()));
    mac.extend_from_slice(&(aad.len() as u64).to_le_bytes());
    mac.extend_from_slice(&(ct.len() as u64).to_le_bytes());
    poly1305(&otk, &mac)
}

/// ChaCha20-Poly1305 (RFC 8439): encrypt `data` in place; returns the tag.
pub fn seal(key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], data: &mut [u8]) -> [u8; 16] {
    chacha20_xor(key, 1, nonce, data);
    aead_tag(key, nonce, aad, data)
}

/// Check the tag and decrypt `data` in place. False (and `data` untouched) if
/// it was tampered with.
pub fn open(key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], data: &mut [u8], tag: &[u8; 16]) -> bool {
    if !same(&aead_tag(key, nonce, aad, data), tag) {
        return false;
    }
    chacha20_xor(key, 1, nonce, data);
    true
}

// ------------------------------------------------------------------ randomness

#[cfg(windows)]
#[link(name = "advapi32")]
unsafe extern "system" {
    #[link_name = "SystemFunction036"]
    fn rtl_gen_random(buf: *mut u8, len: u32) -> u8;
}

/// The operating system's random bytes, if it gives them.
fn os_random(buf: &mut [u8]) -> bool {
    #[cfg(windows)]
    {
        // SAFETY: the buffer is valid for `len` bytes.
        unsafe { rtl_gen_random(buf.as_mut_ptr(), buf.len() as u32) != 0 }
    }
    #[cfg(not(windows))]
    {
        use std::io::Read;
        std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(buf)).is_ok()
    }
}

/// Fill `buf` with unpredictable bytes: the operating system's, stirred with
/// the standard library's own random hash keys, the time and a counter (which
/// alone would still make every key different, if far less secret).
pub fn random_bytes(buf: &mut [u8]) {
    use std::hash::{BuildHasher, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut os = vec![0u8; buf.len()];
    let have_os = os_random(&mut os);
    let mut seed = Vec::new();
    for _ in 0..4 {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
        seed.extend_from_slice(&h.finish().to_le_bytes());
    }
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    seed.extend_from_slice(&t.to_le_bytes());
    seed.extend_from_slice(&(std::process::id() as u64).to_le_bytes());
    seed.extend_from_slice(&(buf.as_ptr() as usize as u64).to_le_bytes());
    seed.push(have_os as u8);
    let key = sha256(&seed);
    let mut stream = vec![0u8; buf.len()];
    chacha20_xor(&key, 0, &[0; 12], &mut stream);
    for ((b, o), s) in buf.iter_mut().zip(os).zip(stream) {
        *b = o ^ s;
    }
}

/// A short fingerprint of a public key, for people to compare: "ab12-cd34-ef56-7890".
pub fn fingerprint(public: &[u8; 32]) -> String {
    let h = sha256(public);
    h[..8].chunks(2).map(|c| format!("{:02x}{:02x}", c[0], c[1])).collect::<Vec<_>>().join("-")
}

pub fn to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn from_hex<const N: usize>(s: &str) -> Option<[u8; N]> {
    let s = s.trim();
    if s.len() != N * 2 || !s.is_ascii() {
        return None;
    }
    let mut out = [0u8; N];
    for (i, o) in out.iter_mut().enumerate() {
        *o = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        let s: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }
    fn h32(s: &str) -> [u8; 32] {
        hex(s).try_into().unwrap()
    }

    #[test]
    fn sha256_and_hmac_match_known_vectors() {
        assert_eq!(sha256(b"abc").to_vec(), hex("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"));
        assert_eq!(sha256(b"").to_vec(), hex("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"));
        // RFC 4231 test case 2.
        assert_eq!(hmac(b"Jefe", &[b"what do ya want ", b"for nothing?"]).to_vec(), hex("5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"));
        // RFC 5869 test case 1.
        let okm = hkdf(&hex("000102030405060708090a0b0c"), &[0x0b; 22], &hex("f0f1f2f3f4f5f6f7f8f9"), 42);
        assert_eq!(okm, hex("3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865"));
    }

    #[test]
    fn x25519_matches_rfc7748() {
        let out = x25519(&h32("a546e36bf0527c9d3b16154b82465edd62144c0ac1fc5a18506a2244ba449ac4"), &h32("e6db6867583030db3594c1a424b15f7c726624ec26b3353b10a903a6d0ab1c4c"));
        assert_eq!(out.to_vec(), hex("c3da55379de9c6908e94ea4df28d084f32eccf03491c71f754b4075577a28552"));
        let a = h32("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a");
        let b = h32("5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb");
        assert_eq!(x25519_public(&a).to_vec(), hex("8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a"));
        assert_eq!(x25519_public(&b).to_vec(), hex("de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f"));
        let shared = hex("4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742");
        assert_eq!(x25519(&a, &x25519_public(&b)).to_vec(), shared);
        assert_eq!(x25519(&b, &x25519_public(&a)).to_vec(), shared);
        // Fresh pairs agree too.
        let ((s1, p1), (s2, p2)) = (keypair(), keypair());
        assert_ne!(p1, p2);
        assert_eq!(x25519(&s1, &p2), x25519(&s2, &p1));
    }

    #[test]
    fn chacha20_poly1305_matches_rfc8439() {
        let key: [u8; 32] = std::array::from_fn(|i| i as u8);
        // 2.3.2: the block function.
        let block = chacha20_block(&key, 1, &hex("000000090000004a00000000").try_into().unwrap());
        assert_eq!(block[..16].to_vec(), hex("10f1e7e4d13b5915500fdd1fa32071c4"));
        // 2.5.2: Poly1305.
        let pk = h32("85d6be7857556d337f4452fe42d506a80103808afb0db2fd4abff6af4149f51b");
        assert_eq!(poly1305(&pk, b"Cryptographic Forum Research Group").to_vec(), hex("a8061dc1305136c6c22b8baf0c0127a9"));
        // 2.8.2: the AEAD.
        let key = h32("808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f");
        let nonce: [u8; 12] = hex("070000004041424344454647").try_into().unwrap();
        let aad = hex("50515253c0c1c2c3c4c5c6c7");
        let text = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";
        let mut data = text.to_vec();
        let tag = seal(&key, &nonce, &aad, &mut data);
        assert_eq!(data[..16].to_vec(), hex("d31a8d34648e60db7b86afbc53ef7ec2"));
        assert_eq!(tag.to_vec(), hex("1ae10b594f09e26a7e902ecbd0600691"));
        assert!(open(&key, &nonce, &aad, &mut data, &tag));
        assert_eq!(&data, text);
        // Any change is caught.
        let mut bad = data.clone();
        seal(&key, &nonce, &aad, &mut bad);
        bad[3] ^= 1;
        assert!(!open(&key, &nonce, &aad, &mut bad, &tag));
    }

    #[test]
    fn random_bytes_differ_and_hex_round_trips() {
        let (mut a, mut b) = ([0u8; 32], [0u8; 32]);
        random_bytes(&mut a);
        random_bytes(&mut b);
        assert_ne!(a, b);
        assert_eq!(from_hex::<32>(&to_hex(&a)), Some(a));
        assert_eq!(from_hex::<32>("zz"), None);
        assert_eq!(fingerprint(&a).len(), 19);
    }
}
