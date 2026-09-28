//! Seeded gradient noise and small hashing helpers. No external crates.

/// SplitMix64: tiny, fast, good-enough PRNG for gameplay and world gen.
#[derive(Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }
    /// Uniform float in [0, 1).
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f32()
    }
    pub fn int(&mut self, lo: i32, hi_inclusive: i32) -> i32 {
        lo + (self.next_u32() % ((hi_inclusive - lo + 1) as u32)) as i32
    }
    pub fn chance(&mut self, p: f32) -> bool {
        self.f32() < p
    }
}

/// Stateless hash of integer coordinates -> [0, 1).
pub fn hash3(seed: u32, x: i32, y: i32, z: i32) -> f32 {
    let mut h = (seed as u64).wrapping_mul(0x2545_F491_4F6C_DD1D);
    h ^= (x as u32 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    h = h.rotate_left(29);
    h ^= (y as u32 as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h = h.rotate_left(31);
    h ^= (z as u32 as u64).wrapping_mul(0x1656_67B1_9E37_79F9);
    h = (h ^ (h >> 33)).wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    h = (h ^ (h >> 33)).wrapping_mul(0xC4CE_B9FE_1A85_EC53);
    h ^= h >> 33;
    (h >> 40) as f32 / (1u64 << 24) as f32
}

pub fn hash2(seed: u32, x: i32, z: i32) -> f32 {
    hash3(seed, x, 0x5EED, z)
}

/// Classic improved Perlin noise (Ken Perlin, 2002) with a seeded permutation.
#[derive(Clone)]
pub struct Perlin {
    perm: [u8; 512],
}

#[inline]
fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[inline]
fn grad(hash: u8, x: f32, y: f32, z: f32) -> f32 {
    let h = hash & 15;
    let u = if h < 8 { x } else { y };
    let v = if h < 4 {
        y
    } else if h == 12 || h == 14 {
        x
    } else {
        z
    };
    (if h & 1 == 0 { u } else { -u }) + (if h & 2 == 0 { v } else { -v })
}

impl Perlin {
    pub fn new(seed: u64) -> Self {
        let mut p: Vec<u8> = (0..=255u8).collect();
        let mut rng = Rng::new(seed);
        for i in (1..256usize).rev() {
            let j = (rng.next_u32() as usize) % (i + 1);
            p.swap(i, j);
        }
        let mut perm = [0u8; 512];
        for i in 0..512 {
            perm[i] = p[i & 255];
        }
        Perlin { perm }
    }

    pub fn noise3(&self, x: f32, y: f32, z: f32) -> f32 {
        let (xf, yf, zf) = (x.floor(), y.floor(), z.floor());
        let xi = (xf as i32 & 255) as usize;
        let yi = (yf as i32 & 255) as usize;
        let zi = (zf as i32 & 255) as usize;
        let (x, y, z) = (x - xf, y - yf, z - zf);
        let (u, v, w) = (fade(x), fade(y), fade(z));
        let p = &self.perm;
        let a = p[xi] as usize + yi;
        let aa = p[a] as usize + zi;
        let ab = p[a + 1] as usize + zi;
        let b = p[xi + 1] as usize + yi;
        let ba = p[b] as usize + zi;
        let bb = p[b + 1] as usize + zi;
        lerp(
            lerp(
                lerp(grad(p[aa], x, y, z), grad(p[ba], x - 1.0, y, z), u),
                lerp(grad(p[ab], x, y - 1.0, z), grad(p[bb], x - 1.0, y - 1.0, z), u),
                v,
            ),
            lerp(
                lerp(grad(p[aa + 1], x, y, z - 1.0), grad(p[ba + 1], x - 1.0, y, z - 1.0), u),
                lerp(
                    grad(p[ab + 1], x, y - 1.0, z - 1.0),
                    grad(p[bb + 1], x - 1.0, y - 1.0, z - 1.0),
                    u,
                ),
                v,
            ),
            w,
        )
    }

    pub fn noise2(&self, x: f32, y: f32) -> f32 {
        // A fixed, non-integer z slice avoids the zero plane of 3D Perlin.
        self.noise3(x, y, 0.371)
    }

    /// Fractal Brownian motion, roughly normalised to [-1, 1].
    pub fn fbm2(&self, x: f32, y: f32, octaves: u32) -> f32 {
        let (mut sum, mut amp, mut freq, mut norm) = (0.0, 1.0, 1.0, 0.0);
        for i in 0..octaves {
            sum += self.noise2(x * freq + i as f32 * 17.3, y * freq - i as f32 * 9.1) * amp;
            norm += amp;
            amp *= 0.5;
            freq *= 2.0;
        }
        (sum / norm * 1.6).clamp(-1.0, 1.0)
    }
}
