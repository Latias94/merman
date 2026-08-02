use rustc_hash::FxHashMap;
use std::cell::RefCell;
use std::sync::{Arc, Mutex, OnceLock};

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
    Circle(Arc<String>),
    Paths(Arc<String>, Arc<String>),
}

#[derive(Default)]
pub(super) struct StateRoughCache {
    entries: RefCell<FxHashMap<StateRoughCacheKey, StateRoughGeometry>>,
}

impl StateRoughCache {
    pub(super) fn get_circle(&self, key: StateRoughCacheKey) -> Option<Arc<String>> {
        match self.entries.borrow().get(&key) {
            Some(StateRoughGeometry::Circle(value)) => Some(Arc::clone(value)),
            Some(StateRoughGeometry::Paths(..)) => {
                panic!("State Rough cache key reused for circle and path geometry")
            }
            None => None,
        }
    }

    pub(super) fn insert_circle(&self, key: StateRoughCacheKey, value: Arc<String>) {
        let previous = self
            .entries
            .borrow_mut()
            .insert(key, StateRoughGeometry::Circle(value));
        debug_assert!(previous.is_none(), "State Rough circle inserted twice");
    }

    pub(super) fn get_paths(&self, key: StateRoughCacheKey) -> Option<(Arc<String>, Arc<String>)> {
        match self.entries.borrow().get(&key) {
            Some(StateRoughGeometry::Paths(fill, stroke)) => {
                Some((Arc::clone(fill), Arc::clone(stroke)))
            }
            Some(StateRoughGeometry::Circle(..)) => {
                panic!("State Rough cache key reused for path and circle geometry")
            }
            None => None,
        }
    }

    pub(super) fn insert_paths(&self, key: StateRoughCacheKey, value: (Arc<String>, Arc<String>)) {
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

type StateRoughCircleCache = FxHashMap<StateRoughCacheKey, Arc<String>>;
type StateRoughPathsCache = FxHashMap<StateRoughCacheKey, (Arc<String>, Arc<String>)>;

const STATE_ROUGH_TLS_CACHE_LIMIT: usize = 4096;

pub(super) fn state_global_rough_circle_cache() -> &'static Mutex<StateRoughCircleCache> {
    static CACHE: OnceLock<Mutex<StateRoughCircleCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(FxHashMap::default()))
}

pub(super) fn state_global_rough_paths_cache() -> &'static Mutex<StateRoughPathsCache> {
    static CACHE: OnceLock<Mutex<StateRoughPathsCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(FxHashMap::default()))
}

thread_local! {
    static STATE_TLS_ROUGH_CIRCLE_CACHE: std::cell::RefCell<StateRoughCircleCache> =
        std::cell::RefCell::new(FxHashMap::default());
    static STATE_TLS_ROUGH_PATHS_CACHE: std::cell::RefCell<StateRoughPathsCache> =
        std::cell::RefCell::new(FxHashMap::default());
}

#[inline]
pub(super) fn state_tls_get_circle(key: StateRoughCacheKey) -> Option<Arc<String>> {
    STATE_TLS_ROUGH_CIRCLE_CACHE.with(|cache| cache.borrow().get(&key).cloned())
}

#[inline]
pub(super) fn state_tls_put_circle(key: StateRoughCacheKey, value: Arc<String>) {
    STATE_TLS_ROUGH_CIRCLE_CACHE.with(|cache| {
        let mut map = cache.borrow_mut();
        if map.len() >= STATE_ROUGH_TLS_CACHE_LIMIT {
            // Best-effort bound. This cache only exists to avoid global mutex overhead on
            // repeated renders within the same thread; eviction does not affect correctness.
            map.clear();
        }
        map.insert(key, value);
    });
}

#[inline]
pub(super) fn state_tls_get_paths(key: StateRoughCacheKey) -> Option<(Arc<String>, Arc<String>)> {
    STATE_TLS_ROUGH_PATHS_CACHE.with(|cache| cache.borrow().get(&key).cloned())
}

#[inline]
pub(super) fn state_tls_put_paths(key: StateRoughCacheKey, value: (Arc<String>, Arc<String>)) {
    STATE_TLS_ROUGH_PATHS_CACHE.with(|cache| {
        let mut map = cache.borrow_mut();
        if map.len() >= STATE_ROUGH_TLS_CACHE_LIMIT {
            // Best-effort bound. See `state_tls_put_circle` for rationale.
            map.clear();
        }
        map.insert(key, value);
    });
}

#[cfg(test)]
fn state_rough_circle_owned_bytes(cache: &StateRoughCircleCache) -> usize {
    cache
        .values()
        .fold(0usize, |sum, value| sum.saturating_add(value.capacity()))
}

#[cfg(test)]
fn state_rough_paths_owned_bytes(cache: &StateRoughPathsCache) -> usize {
    cache.values().fold(0usize, |sum, (fill, stroke)| {
        sum.saturating_add(fill.capacity())
            .saturating_add(stroke.capacity())
    })
}

#[cfg(test)]
pub(super) fn state_rough_cache_retained_counts() -> (usize, usize, usize, usize) {
    let global_circle = state_global_rough_circle_cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let global_paths = state_global_rough_paths_cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let global_entries = global_circle.len().saturating_add(global_paths.len());
    let global_owned_bytes = state_rough_circle_owned_bytes(&global_circle)
        .saturating_add(state_rough_paths_owned_bytes(&global_paths));
    drop(global_paths);
    drop(global_circle);

    let (tls_circle_entries, tls_circle_owned_bytes) = STATE_TLS_ROUGH_CIRCLE_CACHE.with(|cache| {
        let cache = cache.borrow();
        (cache.len(), state_rough_circle_owned_bytes(&cache))
    });
    let (tls_paths_entries, tls_paths_owned_bytes) = STATE_TLS_ROUGH_PATHS_CACHE.with(|cache| {
        let cache = cache.borrow();
        (cache.len(), state_rough_paths_owned_bytes(&cache))
    });
    (
        global_entries,
        global_owned_bytes,
        tls_circle_entries.saturating_add(tls_paths_entries),
        tls_circle_owned_bytes.saturating_add(tls_paths_owned_bytes),
    )
}

#[cfg(test)]
pub(super) fn state_rough_cache_clear_for_probe() {
    state_global_rough_circle_cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear();
    state_global_rough_paths_cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear();
    STATE_TLS_ROUGH_CIRCLE_CACHE.with(|cache| cache.borrow_mut().clear());
    STATE_TLS_ROUGH_PATHS_CACHE.with(|cache| cache.borrow_mut().clear());
}

#[inline]
pub(super) fn detail_guard<'a>(
    timing: super::timing::RenderTiming,
    dst: &'a mut std::time::Duration,
) -> Option<super::timing::TimingGuard<'a>> {
    timing.section(dst)
}
