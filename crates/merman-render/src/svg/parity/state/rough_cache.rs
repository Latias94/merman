use rustc_hash::FxHashMap;
use std::cell::RefCell;
use std::rc::Rc;

#[cfg(test)]
enum StateRoughTrackedWeak {
    Rc(std::rc::Weak<String>),
    Arc(std::sync::Weak<String>),
}

#[cfg(test)]
impl StateRoughTrackedWeak {
    fn is_live(&self) -> bool {
        match self {
            Self::Rc(value) => value.strong_count() != 0,
            Self::Arc(value) => value.strong_count() != 0,
        }
    }
}

#[cfg(test)]
struct StateRoughTrackedAllocation {
    weak: StateRoughTrackedWeak,
    owned_bytes: usize,
}

#[cfg(test)]
trait StateRoughTrackedStrong {
    fn tracked_allocation(&self) -> StateRoughTrackedAllocation;
}

#[cfg(test)]
impl StateRoughTrackedStrong for std::rc::Rc<String> {
    fn tracked_allocation(&self) -> StateRoughTrackedAllocation {
        StateRoughTrackedAllocation {
            weak: StateRoughTrackedWeak::Rc(std::rc::Rc::downgrade(self)),
            owned_bytes: self.capacity(),
        }
    }
}

#[cfg(test)]
impl StateRoughTrackedStrong for std::sync::Arc<String> {
    fn tracked_allocation(&self) -> StateRoughTrackedAllocation {
        StateRoughTrackedAllocation {
            weak: StateRoughTrackedWeak::Arc(std::sync::Arc::downgrade(self)),
            owned_bytes: self.capacity(),
        }
    }
}

#[cfg(test)]
#[derive(Default)]
struct StateRoughReleaseTrackerInner {
    cache_drop_observed: std::cell::Cell<bool>,
    geometry_witnesses: std::cell::Cell<usize>,
    allocations: RefCell<Vec<StateRoughTrackedAllocation>>,
}

#[cfg(test)]
#[derive(Clone, Default)]
pub(super) struct StateRoughReleaseTracker {
    inner: Rc<StateRoughReleaseTrackerInner>,
}

#[cfg(test)]
impl StateRoughReleaseTracker {
    fn record_circle<T>(&self, value: &T)
    where
        T: StateRoughTrackedStrong,
    {
        self.inner
            .geometry_witnesses
            .set(self.inner.geometry_witnesses.get().saturating_add(1));
        self.inner
            .allocations
            .borrow_mut()
            .push(value.tracked_allocation());
    }

    fn record_paths<T>(&self, fill: &T, stroke: &T)
    where
        T: StateRoughTrackedStrong,
    {
        self.inner
            .geometry_witnesses
            .set(self.inner.geometry_witnesses.get().saturating_add(1));
        let mut allocations = self.inner.allocations.borrow_mut();
        allocations.push(fill.tracked_allocation());
        allocations.push(stroke.tracked_allocation());
    }

    fn mark_cache_dropped(&self) {
        self.inner.cache_drop_observed.set(true);
    }

    pub(super) fn snapshot(&self) -> StateRoughReleaseProof {
        let allocations = self.inner.allocations.borrow();
        let mut witnessed_owned_bytes = 0usize;
        let mut live_allocation_witnesses = 0usize;
        let mut live_owned_bytes = 0usize;
        for allocation in allocations.iter() {
            witnessed_owned_bytes = witnessed_owned_bytes.saturating_add(allocation.owned_bytes);
            if allocation.weak.is_live() {
                live_allocation_witnesses = live_allocation_witnesses.saturating_add(1);
                live_owned_bytes = live_owned_bytes.saturating_add(allocation.owned_bytes);
            }
        }
        StateRoughReleaseProof {
            cache_drop_observed: self.inner.cache_drop_observed.get(),
            geometry_witnesses: self.inner.geometry_witnesses.get(),
            allocation_witnesses: allocations.len(),
            witnessed_owned_bytes,
            live_allocation_witnesses,
            live_owned_bytes,
        }
    }
}

#[cfg(test)]
struct StateRoughCacheReleaseProbe {
    tracker: StateRoughReleaseTracker,
}

#[cfg(test)]
impl Drop for StateRoughCacheReleaseProbe {
    fn drop(&mut self) {
        self.tracker.mark_cache_dropped();
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub(super) struct StateRoughReleaseProof {
    pub(super) cache_drop_observed: bool,
    pub(super) geometry_witnesses: usize,
    pub(super) allocation_witnesses: usize,
    pub(super) witnessed_owned_bytes: usize,
    pub(super) live_allocation_witnesses: usize,
    pub(super) live_owned_bytes: usize,
}

#[derive(Debug, Default, Clone)]
pub(super) struct StateRenderDetails {
    pub(super) root_calls: u32,
    pub(super) clusters: std::time::Duration,
    pub(super) edge_paths: std::time::Duration,
    pub(super) edge_labels: std::time::Duration,
    pub(super) leaf_nodes: std::time::Duration,
    pub(super) leaf_nodes_style_parse: std::time::Duration,
    pub(super) leaf_nodes_roughjs: std::time::Duration,
    pub(super) leaf_roughjs_calls: u32,
    pub(super) leaf_roughjs_unique: std::collections::HashSet<StateRoughCacheKey>,
    pub(super) leaf_nodes_measure: std::time::Duration,
    pub(super) leaf_nodes_label_html: std::time::Duration,
    pub(super) leaf_nodes_emit: std::time::Duration,
    pub(super) nested_roots: std::time::Duration,
    pub(super) self_loop_placeholders: std::time::Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct StateRoughCacheKey {
    pub(super) tag: u8,
    pub(super) a: u64,
    pub(super) b: u64,
    pub(super) seed: roughr::core::RoughJsSeed,
}

#[derive(Clone)]
enum StateRoughGeometry {
    Circle(Rc<String>),
    Paths(Rc<String>, Rc<String>),
}

#[derive(Default)]
pub(super) struct StateRoughCache {
    entries: RefCell<FxHashMap<StateRoughCacheKey, StateRoughGeometry>>,
    // Keep the release probe after `entries`: fields are dropped in declaration order, so its
    // sentinel proves every operation-owned geometry entry has already released.
    #[cfg(test)]
    release_probe: Option<StateRoughCacheReleaseProbe>,
}

impl StateRoughCache {
    #[cfg(test)]
    pub(super) fn with_release_tracker(tracker: StateRoughReleaseTracker) -> Self {
        Self {
            entries: RefCell::new(FxHashMap::default()),
            release_probe: Some(StateRoughCacheReleaseProbe { tracker }),
        }
    }

    pub(super) fn get_circle(&self, key: StateRoughCacheKey) -> Option<Rc<String>> {
        match self.entries.borrow().get(&key) {
            Some(StateRoughGeometry::Circle(value)) => Some(Rc::clone(value)),
            Some(StateRoughGeometry::Paths(..)) => {
                panic!("State Rough cache key reused for circle and path geometry")
            }
            None => None,
        }
    }

    pub(super) fn insert_circle(&self, key: StateRoughCacheKey, value: Rc<String>) {
        #[cfg(test)]
        if let Some(probe) = &self.release_probe {
            probe.tracker.record_circle(&value);
        }
        let previous = self
            .entries
            .borrow_mut()
            .insert(key, StateRoughGeometry::Circle(value));
        debug_assert!(previous.is_none(), "State Rough circle inserted twice");
    }

    pub(super) fn get_paths(&self, key: StateRoughCacheKey) -> Option<(Rc<String>, Rc<String>)> {
        match self.entries.borrow().get(&key) {
            Some(StateRoughGeometry::Paths(fill, stroke)) => {
                Some((Rc::clone(fill), Rc::clone(stroke)))
            }
            Some(StateRoughGeometry::Circle(..)) => {
                panic!("State Rough cache key reused for path and circle geometry")
            }
            None => None,
        }
    }

    pub(super) fn insert_paths(&self, key: StateRoughCacheKey, value: (Rc<String>, Rc<String>)) {
        #[cfg(test)]
        if let Some(probe) = &self.release_probe {
            probe.tracker.record_paths(&value.0, &value.1);
        }
        let previous = self
            .entries
            .borrow_mut()
            .insert(key, StateRoughGeometry::Paths(value.0, value.1));
        debug_assert!(previous.is_none(), "State Rough paths inserted twice");
    }

    #[cfg(test)]
    pub(super) fn footprint(&self) -> (usize, usize) {
        let entries = self.entries.borrow();
        let owned_bytes = entries.values().fold(0usize, |sum, geometry| {
            let bytes = match geometry {
                StateRoughGeometry::Circle(value) => value.capacity(),
                StateRoughGeometry::Paths(fill, stroke) => {
                    fill.capacity().saturating_add(stroke.capacity())
                }
            };
            sum.saturating_add(bytes)
        });
        (entries.len(), owned_bytes)
    }
}

#[cfg(test)]
pub(super) fn state_rough_cache_retained_counts() -> (usize, usize, usize, usize) {
    (0, 0, 0, 0)
}

#[cfg(test)]
pub(super) fn state_rough_cache_clear_for_probe() {}

#[inline]
pub(super) fn detail_guard<'a>(
    timing: super::timing::RenderTiming,
    dst: &'a mut std::time::Duration,
) -> Option<super::timing::TimingGuard<'a>> {
    timing.section(dst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_tracker_distinguishes_live_and_released_rc_and_arc_allocations() {
        let tracker = StateRoughReleaseTracker::default();
        let rc_live = Rc::new(String::with_capacity(11));
        let rc_released = Rc::new(String::with_capacity(13));
        let arc_live = std::sync::Arc::new(String::with_capacity(17));
        let arc_released = std::sync::Arc::new(String::with_capacity(19));
        let witnessed_owned_bytes = rc_live
            .capacity()
            .saturating_add(rc_released.capacity())
            .saturating_add(arc_live.capacity())
            .saturating_add(arc_released.capacity());
        let live_owned_bytes = rc_live.capacity().saturating_add(arc_live.capacity());

        tracker.record_paths(&rc_live, &rc_released);
        tracker.record_paths(&arc_live, &arc_released);
        drop(rc_released);
        drop(arc_released);

        assert!(!tracker.snapshot().cache_drop_observed);
        tracker.mark_cache_dropped();
        assert_eq!(
            tracker.snapshot(),
            StateRoughReleaseProof {
                cache_drop_observed: true,
                geometry_witnesses: 2,
                allocation_witnesses: 4,
                witnessed_owned_bytes,
                live_allocation_witnesses: 2,
                live_owned_bytes,
            }
        );
    }
}
