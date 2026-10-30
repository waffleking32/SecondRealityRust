//! Microsoft C 6.0's `rand()`, the generator DOTS (`DOTS/MAKEFILE:10-16`,
//! `cl /qc /AL`) and HARD link: a 32-bit LCG, modulus 2³², seed 1,
//! `seed = seed * 214013 + 2531011`, returning `(seed >> 16) & 0x7FFF`.
//! `docs/fidelity/minvball.md` §2 has the evidence that it is this generator
//! and not Turbo C's.

/// `rand()` with the library's state.
pub(crate) struct MsRand {
    seed: u32,
}

impl MsRand {
    const MULTIPLIER: u32 = 214_013;
    const INCREMENT: u32 = 2_531_011;

    /// `holdrand = 1L`, the library's initial seed; neither source seeds it.
    pub(crate) fn new() -> MsRand {
        MsRand { seed: 1 }
    }

    /// One `rand()`: 0 through 32,767. The state is a `u32`, so the multiply
    /// wraps at the modulus.
    pub(crate) fn next(&mut self) -> i32 {
        self.seed = self
            .seed
            .wrapping_mul(Self::MULTIPLIER)
            .wrapping_add(Self::INCREMENT);
        ((self.seed >> 16) & 0x7FFF) as i32
    }
}

#[cfg(test)]
mod tests {
    use super::MsRand;

    /// The first ten values of Microsoft C's seed-1 stream.
    #[test]
    fn the_first_values_are_microsoft_cs_sequence() {
        let mut rng = MsRand::new();
        let first: Vec<i32> = (0..10).map(|_| rng.next()).collect();
        assert_eq!(first, [41, 18467, 6334, 26500, 19169, 15724, 11478, 29358, 26962, 24464]);
        let mut wrapped = MsRand { seed: u32::MAX };
        wrapped.next();
        assert_eq!(wrapped.seed, u32::MAX.wrapping_mul(214_013).wrapping_add(2_531_011));
    }
}
