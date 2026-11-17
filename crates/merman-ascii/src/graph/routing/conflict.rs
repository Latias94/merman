//! Bounded evidence from rejected reservations, not a certificate of infeasibility.

use super::layout_allocation_failed;
use crate::error::Result;
use crate::resource::ResourceContext;

pub(super) const MAX_BLOCKER_WITNESSES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ReservationKind {
    Route,
    Port,
    Marker,
    Label,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct BlockerWitness {
    pub(super) canonical_edge_index: usize,
    reservations: u8,
}

impl BlockerWitness {
    #[cfg(test)]
    pub(super) fn contains(self, kind: ReservationKind) -> bool {
        self.reservations & (1 << kind as u8) != 0
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct BlockerWitnesses {
    /// Sorted by semantic edge identity, independently of route-vector order.
    pub(super) blockers: Vec<BlockerWitness>,
    pub(super) static_restriction: bool,
    pub(super) capacity_restriction: bool,
    /// More distinct owners were seen than could be retained.
    pub(super) truncated: bool,
}

impl BlockerWitnesses {
    pub(super) fn try_new(resources: &ResourceContext) -> Result<Self> {
        resources.charge_layout_work(MAX_BLOCKER_WITNESSES)?;
        let mut witnesses = Self::default();
        witnesses
            .blockers
            .try_reserve_exact(MAX_BLOCKER_WITNESSES)
            .map_err(|_| layout_allocation_failed())?;
        Ok(witnesses)
    }

    pub(super) fn record(
        &mut self,
        canonical_edge_index: usize,
        kind: ReservationKind,
        resources: &ResourceContext,
    ) -> Result<()> {
        // Admit the bounded lookup and insertion shift before mutating the evidence.
        resources.charge_layout_work(resources.checked_work_add(self.blockers.len(), 1)?)?;
        let reservations = 1 << kind as u8;
        match self
            .blockers
            .binary_search_by_key(&canonical_edge_index, |blocker| {
                blocker.canonical_edge_index
            }) {
            Ok(index) => self.blockers[index].reservations |= reservations,
            Err(index) => {
                if self.blockers.len() == MAX_BLOCKER_WITNESSES {
                    self.truncated = true;
                    if index == MAX_BLOCKER_WITNESSES {
                        return Ok(());
                    }
                    // Retain the smallest semantic identities even if queries arrive in
                    // a different order. Storage never grows beyond the admitted bound.
                    self.blockers.pop();
                }
                self.blockers.insert(
                    index,
                    BlockerWitness {
                        canonical_edge_index,
                        reservations,
                    },
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AsciiError;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};

    #[test]
    fn witnesses_merge_reservations_and_keep_a_deterministic_bounded_set() {
        let resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
        let mut ascending = BlockerWitnesses::try_new(&resources).unwrap();
        let mut descending = BlockerWitnesses::try_new(&resources).unwrap();
        for index in 0..MAX_BLOCKER_WITNESSES + 17 {
            ascending
                .record(index * 7, ReservationKind::Route, &resources)
                .unwrap();
            ascending
                .record(index * 7, ReservationKind::Label, &resources)
                .unwrap();
        }
        for index in (0..MAX_BLOCKER_WITNESSES + 17).rev() {
            descending
                .record(index * 7, ReservationKind::Label, &resources)
                .unwrap();
            descending
                .record(index * 7, ReservationKind::Route, &resources)
                .unwrap();
        }
        assert_eq!(ascending, descending);
        assert!(ascending.truncated);
        assert_eq!(ascending.blockers.len(), MAX_BLOCKER_WITNESSES);
        for (index, blocker) in ascending.blockers.iter().enumerate() {
            assert_eq!(blocker.canonical_edge_index, index * 7);
            assert!(blocker.contains(ReservationKind::Route));
            assert!(blocker.contains(ReservationKind::Label));
            assert!(!blocker.contains(ReservationKind::Marker));
        }
    }

    #[test]
    fn work_failure_precedes_collector_storage_and_mutation() {
        let resources = ResourceContext::new(
            AsciiResourcePolicy::unbounded()
                .with_limit(
                    AsciiResourceLimitId::MaxLayoutWorkUnits,
                    MAX_BLOCKER_WITNESSES - 1,
                )
                .unwrap(),
        );
        assert!(matches!(BlockerWitnesses::try_new(&resources),
            Err(AsciiError::ResourceLimitExceeded(details))
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits));

        let resources = ResourceContext::new(
            AsciiResourcePolicy::unbounded()
                .with_limit(
                    AsciiResourceLimitId::MaxLayoutWorkUnits,
                    MAX_BLOCKER_WITNESSES,
                )
                .unwrap(),
        );
        let mut witnesses = BlockerWitnesses::try_new(&resources).unwrap();
        assert!(matches!(
            witnesses.record(37, ReservationKind::Marker, &resources),
            Err(AsciiError::ResourceLimitExceeded(_))
        ));
        assert!(witnesses.blockers.is_empty());
        assert!(!witnesses.truncated);
    }
}
