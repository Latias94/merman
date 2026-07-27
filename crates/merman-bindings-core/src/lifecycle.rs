use std::fmt;
use std::sync::{Arc, Mutex};

/// Admission mode selected when a reusable binding engine is constructed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingEngineAdmissionMode {
    /// Operations may run concurrently because no foreign callback is reachable.
    Concurrent,
    /// Operations are serialized because they may synchronously invoke a foreign callback.
    HostCallback,
}

/// A nonblocking failure from reusable-engine operation or close admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingEngineAdmissionError {
    Busy,
    ReentrantCall,
    Closed,
    InvalidCallbackState,
    CounterExhausted,
}

impl fmt::Display for BindingEngineAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Busy => "the reusable engine is busy",
            Self::ReentrantCall => "the reusable engine cannot be re-entered from its callback",
            Self::Closed => "the reusable engine is closed",
            Self::InvalidCallbackState => {
                "a host callback must run inside its admitted callback-enabled operation"
            }
            Self::CounterExhausted => "the reusable engine operation counter is exhausted",
        })
    }
}

impl std::error::Error for BindingEngineAdmissionError {}

impl From<BindingEngineAdmissionError> for crate::BindingError {
    fn from(error: BindingEngineAdmissionError) -> Self {
        match error {
            BindingEngineAdmissionError::Busy => crate::BindingError::busy(error.to_string()),
            BindingEngineAdmissionError::ReentrantCall => {
                crate::BindingError::reentrant_call(error.to_string())
            }
            BindingEngineAdmissionError::Closed => {
                crate::BindingError::new(crate::BindingStatus::InvalidArgument, error.to_string())
            }
            BindingEngineAdmissionError::InvalidCallbackState
            | BindingEngineAdmissionError::CounterExhausted => {
                crate::BindingError::new(crate::BindingStatus::InternalError, error.to_string())
            }
        }
    }
}

#[derive(Debug, Default)]
struct AdmissionState {
    active_operations: usize,
    callback_active: bool,
    closed: bool,
}

/// Shared safe-Rust admission state for reusable binding engines.
///
/// Transport registries retain ownership of their tokens and foreign objects. This coordinator
/// only linearizes operation admission, synchronous callback entry, and nonblocking close.
#[derive(Debug)]
pub struct BindingEngineAdmission {
    mode: BindingEngineAdmissionMode,
    state: Mutex<AdmissionState>,
}

impl BindingEngineAdmission {
    #[must_use]
    pub fn new(mode: BindingEngineAdmissionMode) -> Arc<Self> {
        Arc::new(Self {
            mode,
            state: Mutex::new(AdmissionState::default()),
        })
    }

    pub fn enter_operation(
        self: &Arc<Self>,
    ) -> Result<BindingOperationAdmission, BindingEngineAdmissionError> {
        let mut state = self.lock_state();
        if state.closed {
            return Err(BindingEngineAdmissionError::Closed);
        }
        if state.callback_active {
            return Err(BindingEngineAdmissionError::ReentrantCall);
        }
        if self.mode == BindingEngineAdmissionMode::HostCallback && state.active_operations != 0 {
            return Err(BindingEngineAdmissionError::Busy);
        }
        state.active_operations = state
            .active_operations
            .checked_add(1)
            .ok_or(BindingEngineAdmissionError::CounterExhausted)?;
        drop(state);
        Ok(BindingOperationAdmission {
            admission: Arc::clone(self),
        })
    }

    pub fn enter_callback(
        self: &Arc<Self>,
    ) -> Result<BindingCallbackAdmission, BindingEngineAdmissionError> {
        let mut state = self.lock_state();
        if state.closed {
            return Err(BindingEngineAdmissionError::Closed);
        }
        if self.mode != BindingEngineAdmissionMode::HostCallback || state.active_operations != 1 {
            return Err(BindingEngineAdmissionError::InvalidCallbackState);
        }
        if state.callback_active {
            return Err(BindingEngineAdmissionError::ReentrantCall);
        }
        state.callback_active = true;
        drop(state);
        Ok(BindingCallbackAdmission {
            admission: Arc::clone(self),
        })
    }

    /// Attempts to retire the engine without waiting.
    ///
    /// Success is a quiescence barrier. An operation that obtained a transport-owned reference
    /// before this call but did not enter admission observes `Closed` afterwards.
    pub fn try_close(&self) -> Result<(), BindingEngineAdmissionError> {
        let mut state = self.lock_state();
        if state.closed {
            return Err(BindingEngineAdmissionError::Closed);
        }
        if state.callback_active {
            return Err(BindingEngineAdmissionError::ReentrantCall);
        }
        if state.active_operations != 0 {
            return Err(BindingEngineAdmissionError::Busy);
        }
        state.closed = true;
        Ok(())
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, AdmissionState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[derive(Debug)]
pub struct BindingOperationAdmission {
    admission: Arc<BindingEngineAdmission>,
}

impl Drop for BindingOperationAdmission {
    fn drop(&mut self) {
        let mut state = self.admission.lock_state();
        debug_assert!(
            !state.callback_active,
            "an operation cannot finish while its host callback is active"
        );
        debug_assert!(
            state.active_operations != 0,
            "an operation admission must increment the active count"
        );
        state.active_operations = state.active_operations.saturating_sub(1);
    }
}

#[derive(Debug)]
pub struct BindingCallbackAdmission {
    admission: Arc<BindingEngineAdmission>,
}

impl Drop for BindingCallbackAdmission {
    fn drop(&mut self) {
        let mut state = self.admission.lock_state();
        debug_assert!(
            state.callback_active,
            "a callback admission must mark the callback active"
        );
        state.callback_active = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callback_free_operations_are_concurrent() {
        let admission = BindingEngineAdmission::new(BindingEngineAdmissionMode::Concurrent);

        let first = admission.enter_operation().expect("first operation");
        let second = admission.enter_operation().expect("concurrent operation");
        assert_eq!(
            admission.try_close(),
            Err(BindingEngineAdmissionError::Busy)
        );

        drop((first, second));
        admission.try_close().expect("quiescent close");
    }

    #[test]
    fn callback_enabled_competitor_returns_busy() {
        let admission = BindingEngineAdmission::new(BindingEngineAdmissionMode::HostCallback);
        let operation = admission.enter_operation().expect("first operation");

        assert!(matches!(
            admission.enter_operation(),
            Err(BindingEngineAdmissionError::Busy)
        ));

        drop(operation);
        admission.try_close().expect("quiescent close");
    }

    #[test]
    fn callback_reentry_and_close_are_reentrant_errors() {
        let admission = BindingEngineAdmission::new(BindingEngineAdmissionMode::HostCallback);
        let operation = admission.enter_operation().expect("operation");
        let callback = admission.enter_callback().expect("callback");

        assert!(matches!(
            admission.enter_operation(),
            Err(BindingEngineAdmissionError::ReentrantCall)
        ));
        assert_eq!(
            admission.try_close(),
            Err(BindingEngineAdmissionError::ReentrantCall)
        );

        drop(callback);
        assert_eq!(
            admission.try_close(),
            Err(BindingEngineAdmissionError::Busy)
        );
        drop(operation);
        admission.try_close().expect("quiescent close");
    }

    #[test]
    fn acquired_reference_cannot_enter_after_successful_close() {
        let admission = BindingEngineAdmission::new(BindingEngineAdmissionMode::Concurrent);
        let acquired_before_close = Arc::clone(&admission);

        admission.try_close().expect("quiescent close");

        assert!(matches!(
            acquired_before_close.enter_operation(),
            Err(BindingEngineAdmissionError::Closed)
        ));
    }

    #[test]
    fn callback_entry_requires_one_callback_enabled_operation() {
        let concurrent = BindingEngineAdmission::new(BindingEngineAdmissionMode::Concurrent);
        let concurrent_operation = concurrent.enter_operation().expect("concurrent operation");
        assert!(matches!(
            concurrent.enter_callback(),
            Err(BindingEngineAdmissionError::InvalidCallbackState)
        ));
        drop(concurrent_operation);

        let callback = BindingEngineAdmission::new(BindingEngineAdmissionMode::HostCallback);
        assert!(matches!(
            callback.enter_callback(),
            Err(BindingEngineAdmissionError::InvalidCallbackState)
        ));
    }
}
