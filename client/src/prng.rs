// ============================================================================
// File: client/src/prng.rs
// ============================================================================
// ----------------------------------------------------------------------------
// SEEDED DETERMINISTIC PSEUDO-RANDOM NUMBER GENERATOR (XorShift64)
// ----------------------------------------------------------------------------
// Architectural Note:
// Provides deterministic pseudo-random number generation for procedural
// mesh generation (trees, props, grass, creatures) and client-side variations.
// Guaranteed identical output across all clients and platforms for a given seed.
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct Prng {
    pub state: u64,
}

impl Prng {
    #[inline]
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x517cc1b727220a95 } else { seed },
        }
    }

    #[inline]
    pub fn next(&mut self) -> f64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        (self.state as f64) / (u64::MAX as f64)
    }

    #[inline]
    pub fn range(&mut self, min: f32, max: f32) -> f32 {
        min + (self.next() as f32) * (max - min)
    }

    #[inline]
    pub fn blend_color(&mut self, c1: [f32; 4], c2: [f32; 4], t: f32) -> [f32; 4] {
        let f = t.clamp(0.0, 1.0);
        let inv = 1.0 - f;
        [
            c1[0] * inv + c2[0] * f,
            c1[1] * inv + c2[1] * f,
            c1[2] * inv + c2[2] * f,
            c1[3] * inv + c2[3] * f,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prng_determinism() {
        let mut r1 = Prng::new(42);
        let mut r2 = Prng::new(42);
        for _ in 0..100 {
            assert_eq!(r1.next(), r2.next());
            assert_eq!(r1.range(-5.0, 5.0), r2.range(-5.0, 5.0));
        }
    }

    #[test]
    fn test_prng_zero_seed_fallback() {
        let mut r = Prng::new(0);
        assert_ne!(r.next(), 0.0);
    }
}
