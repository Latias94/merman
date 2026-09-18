use std::mem::size_of;
use std::sync::{Arc, Mutex, TryLockError};

use crate::sync::lock_recovering_poison;

#[derive(Debug, Clone)]
pub(crate) struct LineIndexBudget {
    inner: Arc<BudgetState>,
}

#[derive(Debug)]
struct BudgetState {
    limit: usize,
    used: Mutex<usize>,
}

impl LineIndexBudget {
    pub(crate) fn new(limit: usize) -> Self {
        Self {
            inner: Arc::new(BudgetState {
                limit,
                used: Mutex::new(0),
            }),
        }
    }

    fn reserve(&self, bytes: usize) -> Option<LineIndexLease> {
        let mut used = lock_recovering_poison(&self.inner.used);
        let next = used.checked_add(bytes)?;
        if next > self.inner.limit {
            return None;
        }
        *used = next;
        Some(LineIndexLease {
            budget: self.clone(),
            bytes,
        })
    }

    #[cfg(test)]
    pub(crate) fn used_bytes(&self) -> usize {
        *lock_recovering_poison(&self.inner.used)
    }
}

#[derive(Debug)]
struct LineIndexLease {
    budget: LineIndexBudget,
    bytes: usize,
}

impl Drop for LineIndexLease {
    fn drop(&mut self) {
        *lock_recovering_poison(&self.budget.inner.used) -= self.bytes;
    }
}

#[derive(Debug)]
pub(crate) struct LineIndex {
    pub(crate) starts: Vec<usize>,
    lease: Option<LineIndexLease>,
}

impl LineIndex {
    pub(crate) fn local(starts: Vec<usize>) -> Arc<Self> {
        Arc::new(Self {
            starts,
            lease: None,
        })
    }

    fn retained_bytes(capacity: usize) -> Option<usize> {
        capacity
            .checked_mul(size_of::<usize>())?
            .checked_add(size_of::<Self>())?
            .checked_add(2 * size_of::<usize>())
    }
}

/// Shared only by clones of the same immutable syntax snapshot.
#[derive(Debug, Default, Clone)]
pub(crate) struct LineIndexCache {
    inner: Arc<Mutex<Option<Arc<LineIndex>>>>,
}

impl LineIndexCache {
    pub(crate) fn get_or_build(
        &self,
        budget: &LineIndexBudget,
        build: impl FnOnce() -> Vec<usize>,
    ) -> Arc<LineIndex> {
        let mut cached = match self.inner.try_lock() {
            Ok(cached) => cached,
            Err(TryLockError::Poisoned(error)) => error.into_inner(),
            Err(TryLockError::WouldBlock) => return LineIndex::local(build()),
        };
        if let Some(index) = cached.as_ref() {
            if index
                .lease
                .as_ref()
                .is_some_and(|lease| Arc::ptr_eq(&lease.budget.inner, &budget.inner))
            {
                return Arc::clone(index);
            }
            // A snapshot cannot borrow another session's retained-memory allowance.
            drop(cached);
            return LineIndex::local(build());
        }

        // Only this request can retain an index; competing requests build locally.
        let starts = build();
        let lease =
            LineIndex::retained_bytes(starts.capacity()).and_then(|bytes| budget.reserve(bytes));
        let index = Arc::new(LineIndex { starts, lease });
        if index.lease.is_some() {
            *cached = Some(Arc::clone(&index));
        }
        index
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;

    fn starts() -> Vec<usize> {
        vec![0, 8, 16]
    }

    fn charge() -> usize {
        LineIndex::retained_bytes(starts().capacity()).unwrap()
    }

    #[test]
    fn exact_budget_threshold_and_fallback() {
        let cache = LineIndexCache::default();
        let too_small = LineIndexBudget::new(charge() - 1);
        let local = cache.get_or_build(&too_small, starts);
        assert_eq!(local.starts, starts());
        assert_eq!(too_small.used_bytes(), 0);
        assert!(local.lease.is_none());

        let exact = LineIndexBudget::new(charge());
        let retained = cache.get_or_build(&exact, starts);
        assert_eq!(exact.used_bytes(), charge());
        let hit = cache.get_or_build(&exact, || panic!("cached index must be reused"));
        assert!(Arc::ptr_eq(&retained, &hit));
    }

    #[test]
    fn accounting_uses_capacity_and_checks_overflow() {
        let mut starts = Vec::with_capacity(64);
        starts.push(0);
        let expected = LineIndex::retained_bytes(starts.capacity()).unwrap();
        let budget = LineIndexBudget::new(expected);
        let cache = LineIndexCache::default();
        let index = cache.get_or_build(&budget, || starts);
        assert_eq!(budget.used_bytes(), expected);
        assert_eq!(index.starts, vec![0]);
        assert_eq!(LineIndex::retained_bytes(usize::MAX), None);

        let huge = LineIndexBudget::new(usize::MAX);
        let lease = huge.reserve(usize::MAX).unwrap();
        assert!(huge.reserve(1).is_none());
        drop(lease);
        assert_eq!(huge.used_bytes(), 0);
    }

    #[test]
    fn separate_sessions_do_not_share_charges_or_cached_indexes() {
        let first = LineIndexBudget::new(charge());
        let second = LineIndexBudget::new(charge());
        let cache = LineIndexCache::default();
        let original = cache.get_or_build(&first, starts);
        let foreign = cache.get_or_build(&second, starts);
        assert!(!Arc::ptr_eq(&original, &foreign));
        assert!(foreign.lease.is_none());
        assert_eq!(first.used_bytes(), charge());
        assert_eq!(second.used_bytes(), 0);
        let independent = LineIndexCache::default().get_or_build(&second, starts);
        assert!(independent.lease.is_some());
        assert_eq!(second.used_bytes(), charge());
        let hit = cache.get_or_build(&first, || panic!("foreign use must preserve cache"));
        assert!(Arc::ptr_eq(&original, &hit));
    }

    #[test]
    fn concurrent_initialization_does_not_block_local_requests() {
        let cache = LineIndexCache::default();
        let budget = LineIndexBudget::new(charge());
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (local_tx, local_rx) = mpsc::channel();
        std::thread::scope(|scope| {
            let cache_ref = &cache;
            let budget_ref = &budget;
            let initializer = scope.spawn(move || {
                cache_ref.get_or_build(budget_ref, || {
                    started_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    starts()
                })
            });
            started_rx.recv().unwrap();
            let contender = scope.spawn(|| {
                let index = cache.get_or_build(&budget, starts);
                local_tx.send(index).unwrap();
            });
            let local = local_rx.recv_timeout(Duration::from_secs(5));
            // Release even when the timeout fails, so a regression cannot hang the test.
            release_tx.send(()).unwrap();
            let retained = initializer.join().unwrap();
            contender.join().unwrap();
            let local = local.expect("contending request must complete before initialization");
            assert!(local.lease.is_none());
            assert_eq!(local.starts, starts());
            assert!(!Arc::ptr_eq(&retained, &local));
            assert_eq!(budget.used_bytes(), charge());
            let hit = cache.get_or_build(&budget, || panic!("initialized index must be reused"));
            assert!(Arc::ptr_eq(&retained, &hit));
        });
    }

    #[test]
    fn old_reader_holds_charge_and_new_snapshot_retries_after_release() {
        let budget = LineIndexBudget::new(charge());
        let old_cache = LineIndexCache::default();
        let old_reader = old_cache.get_or_build(&budget, starts);
        let shared_snapshot = old_cache.clone();
        drop(old_cache);
        drop(shared_snapshot);
        assert_eq!(budget.used_bytes(), charge());

        let new_cache = LineIndexCache::default();
        let local = new_cache.get_or_build(&budget, starts);
        assert!(local.lease.is_none());
        assert_eq!(local.starts, starts());
        drop(old_reader);
        assert_eq!(budget.used_bytes(), 0);
        let retained = new_cache.get_or_build(&budget, starts);
        assert!(retained.lease.is_some());
        assert_eq!(budget.used_bytes(), charge());
        drop(retained);
        drop(new_cache);
        assert_eq!(budget.used_bytes(), 0);
    }

    #[test]
    fn failed_initialization_can_retry_after_poison() {
        let budget = LineIndexBudget::new(charge());
        let cache = LineIndexCache::default();
        let failed =
            std::panic::catch_unwind(|| cache.get_or_build(&budget, || panic!("failed builder")));
        assert!(failed.is_err());
        assert_eq!(budget.used_bytes(), 0);
        let index = cache.get_or_build(&budget, starts);
        assert_eq!(index.starts, starts());
        assert_eq!(budget.used_bytes(), charge());
    }
}
