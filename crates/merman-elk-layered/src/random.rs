//! Java `java.util.Random` compatible generator.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/GraphConfigurator.java
//! - OpenJDK `java.util.Random` seed scrambling and `nextInt(int)` semantics.

use std::sync::Arc;

const MULTIPLIER: u64 = 0x5DEECE66D;
const ADDEND: u64 = 0xB;
const MASK: u64 = (1u64 << 48) - 1;
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;
const RANDOM_SCOPE_DOMAIN: &str = "merman-elk-layered.random-seed.v2";

/// A nonzero seed captured once by the owner of a layout operation.
///
/// This is deliberately distinct from ELK's configured `randomSeed`: ELK uses `0` in that
/// option as an *unseeded sentinel*, while a Merman operation always owns a nonzero entropy
/// source. The source port can then replace only the sentinel without changing the configured
/// Java `i32` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationSeed(std::num::NonZeroU64);

impl OperationSeed {
    /// Creates the seed token from a render/layout operation's already-captured random seed.
    ///
    /// Callers must create one token per operation and reuse it for every graph belonging to that
    /// operation. Requiring `NonZeroU64` prevents a source `randomSeed = 0` sentinel from being
    /// accidentally repurposed as the operation key.
    pub const fn from_operation_seed(seed: std::num::NonZeroU64) -> Self {
        Self(seed)
    }

    const fn value(self) -> u64 {
        self.0.get()
    }
}

/// A persistent graph path used to derive stable per-scope random streams.
///
/// Child scopes extend the path in constant time and share their immutable prefix. Formatting the
/// full path is deferred until an unresolved zero-seed error actually needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphSeedScope(Option<Arc<GraphSeedScopeNode>>);

#[derive(Debug, PartialEq, Eq)]
struct GraphSeedScopeNode {
    parent: Option<Arc<GraphSeedScopeNode>>,
    component: Box<str>,
    graph_configurator_hash: u64,
}

impl GraphSeedScope {
    pub fn root(component: impl Into<Box<str>>) -> Self {
        let component = component.into();
        let hash = graph_configurator_scope_prefix();
        Self(Some(Arc::new(GraphSeedScopeNode {
            parent: None,
            graph_configurator_hash: hash_component(hash, component.as_ref()),
            component,
        })))
    }

    pub fn child(&self, component: impl Into<Box<str>>) -> Self {
        let component = component.into();
        let parent = self.node();
        Self(Some(Arc::new(GraphSeedScopeNode {
            parent: Some(Arc::clone(
                self.0.as_ref().expect("live graph seed scope has a node"),
            )),
            graph_configurator_hash: hash_component(
                parent.graph_configurator_hash,
                component.as_ref(),
            ),
            component,
        })))
    }

    pub(crate) fn from_components(components: &[&str]) -> Self {
        let mut components = components.iter().copied();
        let root = components
            .next()
            .expect("ELK random seed graph scope must not be empty");
        components.fold(Self::root(root), |scope, component| scope.child(component))
    }

    fn path_string(&self) -> String {
        let mut components = Vec::new();
        let mut current = Some(self.node());
        while let Some(node) = current {
            components.push(node.component.as_ref());
            current = node.parent.as_deref();
        }
        components.reverse();
        components.join("/")
    }

    fn node(&self) -> &GraphSeedScopeNode {
        self.0
            .as_deref()
            .expect("graph seed scope node is only taken during drop")
    }
}

impl Drop for GraphSeedScope {
    fn drop(&mut self) {
        // A uniquely owned Arc parent chain otherwise drops recursively. Budget rejection may
        // release a very deep scope arena at once, so peel unique nodes iteratively; shared
        // prefixes stop immediately and remain owned by their other scope handles.
        let mut current = self.0.take();
        while let Some(node) = current {
            match Arc::try_unwrap(node) {
                Ok(mut node) => current = node.parent.take(),
                Err(shared) => {
                    drop(shared);
                    break;
                }
            }
        }
    }
}

/// Failure to resolve ELK's upstream unseeded random sentinel at an execution boundary.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RandomSeedError {
    #[error(
        "ELK graph `{graph_path}` uses randomSeed=0; execute it through an operation-owned seed"
    )]
    Unresolved { graph_path: String },
}

/// The only source-port boundary that creates ELK's graph-level Java random generator.
///
/// Keeping this phase explicit makes a later port of another Java random boundary unable to
/// silently share the GraphConfigurator stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RandomSeedPhase {
    GraphConfigurator,
}

impl RandomSeedPhase {
    const fn domain_label(self) -> &'static str {
        match self {
            Self::GraphConfigurator => "GraphConfigurator.configureGraphProperties",
        }
    }
}

/// Resolves ELK's `randomSeed` sentinel without ambient process randomness.
///
/// Raw source-port graphs start in `RequireExplicit` mode. Only adapter code that owns an
/// operation can install `Operation`, and even then the configured source value is preserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RandomSeedAuthority {
    RequireExplicit,
    Operation(OperationSeed),
}

impl RandomSeedAuthority {
    pub(crate) const fn require_explicit() -> Self {
        Self::RequireExplicit
    }

    pub(crate) const fn operation(seed: OperationSeed) -> Self {
        Self::Operation(seed)
    }

    /// Resolves a configured seed at a specific source-port random boundary.
    ///
    /// ELK stores `randomSeed` as an `i32`, but Java's `Random` accepts an `i64`. Nonzero
    /// configured values preserve ELK's exact signed `i32 -> i64` conversion. The upstream zero
    /// sentinel is derived from the owning operation, stable graph path, random boundary, and
    /// invocation without truncating the operation seed to `i32`.
    #[cfg(test)]
    pub(crate) fn resolve(
        self,
        configured_seed: i32,
        graph_path: &[&str],
        phase: RandomSeedPhase,
        configuration_invocation: u64,
    ) -> Result<i64, RandomSeedError> {
        if configured_seed != 0 {
            return Ok(i64::from(configured_seed));
        }
        let graph_scope = GraphSeedScope::from_components(graph_path);
        self.resolve_scope(
            configured_seed,
            &graph_scope,
            phase,
            configuration_invocation,
        )
    }

    pub(crate) fn resolve_scope(
        self,
        configured_seed: i32,
        graph_scope: &GraphSeedScope,
        phase: RandomSeedPhase,
        configuration_invocation: u64,
    ) -> Result<i64, RandomSeedError> {
        if configured_seed != 0 {
            return Ok(i64::from(configured_seed));
        }
        let Self::Operation(operation_seed) = self else {
            return Err(RandomSeedError::Unresolved {
                graph_path: graph_scope.path_string(),
            });
        };
        Ok(derive_java_seed_from_scope(
            operation_seed,
            graph_scope,
            phase,
            configuration_invocation,
        ))
    }
}

fn derive_java_seed_from_scope(
    operation_seed: OperationSeed,
    graph_scope: &GraphSeedScope,
    phase: RandomSeedPhase,
    configuration_invocation: u64,
) -> i64 {
    // FNV-1a plus an avalanche step gives a stable, target-independent Java long. This is only
    // used to replace ELK's unseeded sentinel; configured nonzero values pass through untouched.
    // The phase and invocation components mirror ELK constructing fresh Java random streams at
    // distinct source boundaries while retaining replayable operation ownership.
    debug_assert_eq!(phase, RandomSeedPhase::GraphConfigurator);
    let mut hash = graph_scope.node().graph_configurator_hash;
    for byte in operation_seed
        .value()
        .to_le_bytes()
        .into_iter()
        .chain(configuration_invocation.to_le_bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash ^= hash >> 30;
    hash = hash.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    hash ^= hash >> 27;
    hash = hash.wrapping_mul(0x94d0_49bb_1331_11eb);
    hash ^= hash >> 31;

    hash as i64
}

fn graph_configurator_scope_prefix() -> u64 {
    hash_component(
        hash_component(FNV_OFFSET, RANDOM_SCOPE_DOMAIN),
        RandomSeedPhase::GraphConfigurator.domain_label(),
    )
}

fn hash_component(mut hash: u64, component: &str) -> u64 {
    for byte in component.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash ^= 0xff;
    hash.wrapping_mul(FNV_PRIME)
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
    fn operation_seed_authority_preserves_nonzero_java_i32_values() {
        let authority = RandomSeedAuthority::operation(operation_seed());
        for configured in [i32::MIN, -1, 1, i32::MAX] {
            assert_eq!(
                authority.resolve(configured, &["root"], RandomSeedPhase::GraphConfigurator, 0,),
                Ok(i64::from(configured))
            );
        }
    }

    #[test]
    fn raw_authority_rejects_elk_zero() {
        assert_eq!(
            RandomSeedAuthority::require_explicit().resolve(
                0,
                &["root"],
                RandomSeedPhase::GraphConfigurator,
                0,
            ),
            Err(RandomSeedError::Unresolved {
                graph_path: "root".to_string(),
            })
        );
    }

    #[test]
    fn operation_seed_fallback_is_stable_and_graph_path_invocation_scoped() {
        let authority = RandomSeedAuthority::operation(operation_seed());
        let root = authority.resolve(0, &["root"], RandomSeedPhase::GraphConfigurator, 0);
        let nested =
            authority.resolve(0, &["root", "group"], RandomSeedPhase::GraphConfigurator, 0);

        assert_eq!(
            root,
            authority.resolve(0, &["root"], RandomSeedPhase::GraphConfigurator, 0)
        );
        assert_eq!(
            nested,
            authority.resolve(0, &["root", "group"], RandomSeedPhase::GraphConfigurator, 0,)
        );
        assert_ne!(root, nested);
        assert_ne!(
            root,
            authority.resolve(0, &["root"], RandomSeedPhase::GraphConfigurator, 1)
        );
    }

    #[test]
    fn persistent_graph_seed_scope_preserves_slice_seed_and_error_path_semantics() {
        let authority = RandomSeedAuthority::operation(operation_seed());
        let scope = GraphSeedScope::root("root").child("outer").child("inner");
        assert_eq!(
            authority.resolve(
                0,
                &["root", "outer", "inner"],
                RandomSeedPhase::GraphConfigurator,
                0,
            ),
            authority.resolve_scope(0, &scope, RandomSeedPhase::GraphConfigurator, 0)
        );
        assert_eq!(
            RandomSeedAuthority::require_explicit().resolve_scope(
                0,
                &scope,
                RandomSeedPhase::GraphConfigurator,
                0,
            ),
            Err(RandomSeedError::Unresolved {
                graph_path: "root/outer/inner".to_string(),
            })
        );
    }

    #[test]
    fn persistent_graph_seed_scope_drop_is_small_stack_safe() {
        std::thread::Builder::new()
            .name("elk-graph-seed-scope-drop".to_string())
            .stack_size(64 * 1024)
            .spawn(|| {
                let mut scope = GraphSeedScope::root("root");
                for index in 0..50_000 {
                    scope = scope.child(index.to_string());
                }
                drop(scope);
            })
            .unwrap()
            .join()
            .expect("persistent graph seed scopes must drop iteratively");
    }

    fn operation_seed() -> OperationSeed {
        OperationSeed::from_operation_seed(
            std::num::NonZeroU64::new(0x1234_5678_9abc_def0).expect("nonzero operation seed"),
        )
    }
}
