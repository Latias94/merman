//! Java `java.util.Random` compatible generator.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/GraphConfigurator.java
//! - OpenJDK `java.util.Random` seed scrambling and `nextInt(int)` semantics.

use std::time::{SystemTime, UNIX_EPOCH};

const MULTIPLIER: u64 = 0x5DEECE66D;
const ADDEND: u64 = 0xB;
const MASK: u64 = (1u64 << 48) - 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaRandom {
    seed: u64,
}

impl JavaRandom {
    pub fn new(seed: i64) -> Self {
        Self {
            seed: ((seed as u64) ^ MULTIPLIER) & MASK,
        }
    }

    pub fn from_layout_seed(seed: i32) -> Self {
        if seed == 0 {
            Self::new(unseeded_seed())
        } else {
            Self::new(i64::from(seed))
        }
    }

    pub fn next_int(&mut self, bound: usize) -> Option<usize> {
        let bound = i32::try_from(bound).ok()?;
        if bound <= 0 {
            return None;
        }

        if (bound & -bound) == bound {
            let next = i64::from(self.next_bits(31));
            return Some(((i64::from(bound) * next) >> 31) as usize);
        }

        loop {
            let bits = self.next_bits(31);
            let value = bits % bound;
            if bits.wrapping_sub(value).wrapping_add(bound - 1) >= 0 {
                return Some(value as usize);
            }
        }
    }

    fn next_bits(&mut self, bits: u32) -> i32 {
        self.seed = self.seed.wrapping_mul(MULTIPLIER).wrapping_add(ADDEND) & MASK;
        (self.seed >> (48 - bits)) as i32
    }
}

impl Default for JavaRandom {
    fn default() -> Self {
        Self::from_layout_seed(1)
    }
}

fn unseeded_seed() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as i64)
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_int_matches_java_random_for_seed_one() {
        let mut random = JavaRandom::new(1);

        assert_eq!(random.next_int(3), Some(0));
        assert_eq!(random.next_int(3), Some(1));
        assert_eq!(random.next_int(3), Some(1));
        assert_eq!(random.next_int(10), Some(3));
    }
}
