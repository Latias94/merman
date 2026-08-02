use rustc_hash::FxHashMap;
use std::cell::RefCell;
use std::rc::Rc;

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
}

impl StateRoughCache {
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
