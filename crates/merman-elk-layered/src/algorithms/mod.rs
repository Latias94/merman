//! Additional source-backed ELK algorithms within the existing EPL-2.0 boundary.

pub mod box_layout;
pub mod force;
pub mod mrtree;
pub mod radial;
pub mod rectpacking;
pub mod spore_overlap;
pub mod stress;

/// Distinct random boundaries used by the additional providers. Stress's Force initializer
/// deliberately shares the Force boundary; SPOrE uses a separate operation-owned stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RandomDomain {
    Force,
    SporeOverlap,
}

/// Resolves the source's configured Java seed without introducing ambient randomness.
pub fn resolve_seed(
    configured: i32,
    operation: Option<crate::OperationSeed>,
    scope: &crate::GraphSeedScope,
    domain: RandomDomain,
) -> Result<i64, crate::RandomSeedError> {
    use crate::random::{RandomSeedAuthority, RandomSeedPhase};
    let authority = operation.map_or_else(
        RandomSeedAuthority::require_explicit,
        RandomSeedAuthority::operation,
    );
    authority.resolve_scope(
        configured,
        scope,
        match domain {
            RandomDomain::Force => RandomSeedPhase::Force,
            RandomDomain::SporeOverlap => RandomSeedPhase::SporeOverlap,
        },
        0,
    )
}

/// A deterministic replacement for SPOrE's ambient `Math.random`. The operation owner selects
/// the seed; this helper only supplies the same bounded Java random stream as the other kernels.
pub fn random_stream(seed: i64) -> impl FnMut() -> f64 {
    let mut random = crate::random::JavaRandom::new(seed);
    move || random.next_double()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_streams_are_scoped_without_reinterpreting_explicit_java_seeds() {
        let scope = crate::GraphSeedScope::root("root");
        let operation = Some(crate::OperationSeed::from_operation_seed(
            std::num::NonZeroU64::new(321).unwrap(),
        ));
        for domain in [RandomDomain::Force, RandomDomain::SporeOverlap] {
            assert_eq!(resolve_seed(-7, operation, &scope, domain), Ok(-7));
            assert!(resolve_seed(0, None, &scope, domain).is_err());
            let first = resolve_seed(0, operation, &scope, domain).unwrap();
            assert_eq!(first, resolve_seed(0, operation, &scope, domain).unwrap());
            assert_ne!(
                first,
                resolve_seed(0, operation, &scope.child("group"), domain).unwrap()
            );
        }
        assert_ne!(
            resolve_seed(0, operation, &scope, RandomDomain::Force).unwrap(),
            resolve_seed(0, operation, &scope, RandomDomain::SporeOverlap).unwrap()
        );
    }
}
