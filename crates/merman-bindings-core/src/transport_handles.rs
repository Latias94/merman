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

/// Why an operation control is unavailable for acquisition or retirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationControlHandleError {
    NeverIssued,
    Retired,
}

impl HandleRegistry<OperationControl> {
    /// Clones the live control so that it can be used after unlocking the registry.
    pub fn acquire(&self, handle: u64) -> Result<OperationControl, OperationControlHandleError> {
        self.get(handle)
            .cloned()
            .ok_or_else(|| self.inactive_control_handle(handle))
    }

    /// Removes the registry's control without cancelling acquired clones. The caller owns the
    /// returned control and can drop it after unlocking the registry.
    pub fn retire(&mut self, handle: u64) -> Result<OperationControl, OperationControlHandleError> {
        self.remove(handle)
            .ok_or_else(|| self.inactive_control_handle(handle))
    }

    fn inactive_control_handle(&self, handle: u64) -> OperationControlHandleError {
        if self.was_issued(handle) {
            OperationControlHandleError::Retired
        } else {
            OperationControlHandleError::NeverIssued
        }
    }
}

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
        let acquired = registry.acquire(handle).unwrap();
        drop(registry.retire(handle).unwrap());

        assert!(!acquired.is_cancelled());
        control.cancel();
        assert!(acquired.is_cancelled());
    }

    #[test]
    fn operation_control_handles_distinguish_never_issued_from_retired() {
        let mut registry = OperationControlRegistry::new(2);
        let handle = registry.try_insert(OperationControl::new()).unwrap();
        drop(registry.retire(handle).unwrap());

        for (handle, expected) in [
            (0, OperationControlHandleError::NeverIssued),
            (handle, OperationControlHandleError::Retired),
            (handle + 1, OperationControlHandleError::NeverIssued),
        ] {
            assert_eq!(registry.acquire(handle).unwrap_err(), expected);
            assert_eq!(registry.retire(handle).unwrap_err(), expected);
        }
    }
}
