//! Java `java.util.Random` compatible generator.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/GraphConfigurator.java
//! - OpenJDK `java.util.Random` seed scrambling and `nextInt(int)` semantics.

const MULTIPLIER: u64 = 0x5DEECE66D;
const ADDEND: u64 = 0xB;
const MASK: u64 = (1u64 << 48) - 1;

/// Resolves ELK's `randomSeed` sentinel without giving the layered engine ambient randomness.
///
/// Eclipse ELK interprets `randomSeed = 0` as `new Random()`. A headless caller must choose the
/// authority for that unseeded branch explicitly: reject it or supply an operation-owned fallback
/// key. Nonzero configured seeds always retain their exact signed 32-bit Java semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RandomSeedPolicy {
    /// Reject ELK's unseeded sentinel instead of consulting process state.
    RequireExplicit,
    /// Derive a stable concrete Java seed from a caller-owned operation key and graph path.
    DeterministicFallback { operation_seed: u64 },
}

/// Failure to resolve ELK's upstream unseeded random sentinel at an execution boundary.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RandomSeedError {
    #[error(
        "ELK graph `{graph_path}` uses randomSeed=0; provide an operation-owned deterministic fallback"
    )]
    Unresolved { graph_path: String },
}

impl RandomSeedPolicy {
    pub const fn require_explicit() -> Self {
        Self::RequireExplicit
    }

    pub const fn deterministic(operation_seed: u64) -> Self {
        Self::DeterministicFallback { operation_seed }
    }

    /// Resolves a configured seed for one GraphConfigurator invocation.
    ///
    /// ELK stores `randomSeed` as an `i32`, but Java's `Random` accepts an `i64`. Nonzero
    /// configured values preserve ELK's exact signed `i32 -> i64` conversion. The upstream zero
    /// sentinel is resolved into a distinct operation-owned Java seed without changing the
    /// original configuration value.
    pub fn resolve(
        self,
        configured_seed: i32,
        graph_path: &[&str],
        configuration_invocation: u64,
    ) -> Result<i64, RandomSeedError> {
        if configured_seed != 0 {
            return Ok(i64::from(configured_seed));
        }
        let Self::DeterministicFallback { operation_seed } = self else {
            return Err(RandomSeedError::Unresolved {
                graph_path: graph_path.join("/"),
            });
        };
        Ok(derive_java_seed(
            operation_seed,
            graph_path,
            configuration_invocation,
        ))
    }
}

fn derive_java_seed(
    operation_seed: u64,
    graph_path: &[&str],
    configuration_invocation: u64,
) -> i64 {
    // FNV-1a plus an avalanche step gives a stable, target-independent Java long. This is only
    // used to replace ELK's unseeded sentinel; configured nonzero values pass through untouched.
    // The invocation component mirrors ELK constructing a fresh `new Random()` each time its
    // GraphConfigurator runs, while retaining replayable operation ownership.
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for component in std::iter::once("merman-elk-layered.random-seed.v1")
        .chain(std::iter::once(
            "GraphConfigurator.configureGraphProperties",
        ))
        .chain(graph_path.iter().copied())
    {
        for byte in component.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    for byte in operation_seed
        .to_le_bytes()
        .into_iter()
        .chain(configuration_invocation.to_le_bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash ^= hash >> 30;
    hash = hash.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    hash ^= hash >> 27;
    hash = hash.wrapping_mul(0x94d0_49bb_1331_11eb);
    hash ^= hash >> 31;

    hash as i64
}

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

    pub fn next_bool(&mut self) -> bool {
        self.next_bits(1) != 0
    }

    pub fn next_float(&mut self) -> f32 {
        self.next_bits(24) as f32 / (1u32 << 24) as f32
    }

    pub fn next_double(&mut self) -> f64 {
        let high = i64::from(self.next_bits(26));
        let low = i64::from(self.next_bits(27));
        ((high << 27) + low) as f64 / (1u64 << 53) as f64
    }

    pub fn next_long(&mut self) -> i64 {
        let high = i64::from(self.next_bits(32));
        let low = i64::from(self.next_bits(32));
        (high << 32).wrapping_add(low)
    }

    pub fn set_seed(&mut self, seed: i64) {
        self.seed = ((seed as u64) ^ MULTIPLIER) & MASK;
    }

    fn next_bits(&mut self, bits: u32) -> i32 {
        self.seed = self.seed.wrapping_mul(MULTIPLIER).wrapping_add(ADDEND) & MASK;
        (self.seed >> (48 - bits)) as i32
    }
}

impl Default for JavaRandom {
    fn default() -> Self {
        Self::new(1)
    }
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

    #[test]
    fn scalar_methods_match_java_random_for_seed_one() {
        let mut random = JavaRandom::new(1);

        assert!(random.next_bool());
        assert_eq!(random.next_float().to_bits(), 0x3dcdc4e0);

        let mut random = JavaRandom::new(1);
        assert_eq!(random.next_long(), -4964420948893066024);
    }

    #[test]
    fn explicit_java_seed_zero_is_deterministic() {
        let mut random = JavaRandom::new(0);

        assert_eq!(random.next_int(3), Some(0));
        assert_eq!(random.next_int(3), Some(1));
        assert_eq!(random.next_int(10), Some(9));

        let mut random = JavaRandom::new(0);
        assert_eq!(random.next_long(), -4962768465676381896);
    }

    #[test]
    fn random_seed_policy_preserves_nonzero_java_i32_values() {
        let policy = RandomSeedPolicy::deterministic(0x1234_5678_9abc_def0);
        for configured in [i32::MIN, -1, 1, i32::MAX] {
            assert_eq!(
                policy.resolve(configured, &["root"], 0),
                Ok(i64::from(configured))
            );
        }
    }

    #[test]
    fn random_seed_policy_requires_a_fallback_for_elk_zero() {
        assert_eq!(
            RandomSeedPolicy::require_explicit().resolve(0, &["root"], 0),
            Err(RandomSeedError::Unresolved {
                graph_path: "root".to_string(),
            })
        );
    }

    #[test]
    fn deterministic_fallback_is_stable_and_graph_path_scoped() {
        let policy = RandomSeedPolicy::deterministic(0x1234_5678_9abc_def0);
        let root = policy.resolve(0, &["root"], 0);
        let nested = policy.resolve(0, &["root", "group"], 0);

        assert_eq!(root, policy.resolve(0, &["root"], 0));
        assert_eq!(nested, policy.resolve(0, &["root", "group"], 0));
        assert_ne!(root, nested);
        assert_ne!(root, policy.resolve(0, &["root"], 1));
    }
}
