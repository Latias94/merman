//! Monotonic handle primitives shared by native and WASI transports.

use crate::OperationControl;
use std::collections::BTreeMap;

/// Issues nonzero handles in order up to a fixed inclusive limit.
pub struct HandleCounter {
    last_handle: u64,
    max_handle: u64,
}

impl HandleCounter {
    pub const fn new(max_handle: u64) -> Self {
        Self {
            last_handle: 0,
            max_handle,
        }
    }

    /// Consumes the next handle, or returns `None` when the space is exhausted.
    pub fn allocate(&mut self) -> Option<u64> {
        if self.last_handle == self.max_handle {
            return None;
        }
        self.last_handle += 1;
        Some(self.last_handle)
    }

    fn was_issued(&self, handle: u64) -> bool {
        handle != 0 && handle <= self.last_handle
    }
}

/// A counter and its live values. The transport owns synchronization and wire encoding.
pub struct HandleRegistry<T> {
    handles: HandleCounter,
    live: BTreeMap<u64, T>,
}

impl<T> HandleRegistry<T> {
    pub const fn new(max_handle: u64) -> Self {
        Self {
            handles: HandleCounter::new(max_handle),
            live: BTreeMap::new(),
        }
    }

    /// Returns the original value if the handle space is exhausted.
    pub fn try_insert(&mut self, value: T) -> Result<u64, T> {
        let Some(handle) = self.handles.allocate() else {
            return Err(value);
        };
        let previous = self.live.insert(handle, value);
        debug_assert!(previous.is_none());
        Ok(handle)
    }

    pub fn get(&self, handle: u64) -> Option<&T> {
        self.live.get(&handle)
    }

    pub fn get_mut(&mut self, handle: u64) -> Option<&mut T> {
        self.live.get_mut(&handle)
    }

    pub fn remove(&mut self, handle: u64) -> Option<T> {
        self.live.remove(&handle)
    }

    /// Includes retired handles so callers can implement idempotent release.
    pub fn was_issued(&self, handle: u64) -> bool {
        self.handles.was_issued(handle)
    }
}

pub type OperationControlRegistry = HandleRegistry<OperationControl>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_issues_the_max_handle_once() {
        let mut counter = HandleCounter {
            last_handle: u64::MAX - 1,
            max_handle: u64::MAX,
        };
        assert_eq!(counter.allocate(), Some(u64::MAX));
        assert_eq!(counter.allocate(), None);
        assert!(counter.was_issued(u64::MAX));
    }

    #[test]
    fn retired_handles_stay_issued_and_exhaustion_returns_the_value() {
        let mut registry = HandleRegistry::new(2);
        assert_eq!(registry.try_insert("first"), Ok(1));
        assert_eq!(registry.remove(1), Some("first"));
        assert!(registry.was_issued(1));
        assert!(!registry.was_issued(0));
        assert!(!registry.was_issued(2));
        assert_eq!(registry.try_insert("second"), Ok(2));
        assert_eq!(registry.try_insert("rejected"), Err("rejected"));
        assert_eq!(registry.get(2), Some(&"second"));
        assert_eq!(registry.get(1), None);
        assert_eq!(registry.remove(1), None);
        assert!(!registry.was_issued(3));
    }

    #[test]
    fn acquired_control_stays_live_after_the_handle_is_retired() {
        let mut registry = OperationControlRegistry::new(1);
        let control = OperationControl::new();
        let handle = registry.try_insert(control.clone()).unwrap();
        let acquired = registry.get(handle).unwrap().clone();
        drop(registry.remove(handle));

        assert!(registry.get(handle).is_none());
        control.cancel();
        assert!(acquired.is_cancelled());
    }
}
