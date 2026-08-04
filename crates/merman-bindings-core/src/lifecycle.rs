use std::fmt;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::ThreadId;

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
    closing_thread: Option<ThreadId>,
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
    close_complete: Condvar,
}

impl BindingEngineAdmission {
    #[must_use]
    pub fn new(mode: BindingEngineAdmissionMode) -> Arc<Self> {
        Arc::new(Self {
            mode,
            state: Mutex::new(AdmissionState::default()),
            close_complete: Condvar::new(),
        })
    }

    pub fn enter_operation(
        self: &Arc<Self>,
    ) -> Result<BindingOperationAdmission, BindingEngineAdmissionError> {
        let mut state = self.lock_state();
        if state.closed || state.closing_thread.is_some() {
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
        if state.closed || state.closing_thread.is_some() {
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
    pub fn try_close(self: &Arc<Self>) -> Result<(), BindingEngineAdmissionError> {
        self.try_close_detaching(|| ())
    }

    /// Atomically detaches an owned service graph and destroys it after releasing admission.
    ///
    /// Active operations and callbacks fail immediately without changing state. A concurrent close
    /// waits only for the already-detached graph to finish destruction, then observes `Closed`.
    /// Close re-entry from the thread currently destroying that graph also observes `Closed`
    /// immediately, which prevents callback destructors from deadlocking.
    pub fn try_close_detaching<T>(
        self: &Arc<Self>,
        detach: impl FnOnce() -> T,
    ) -> Result<(), BindingEngineAdmissionError> {
        let current_thread = std::thread::current().id();
        let detached = {
            let mut state = self.lock_state();
            loop {
                if state.closed {
                    return Err(BindingEngineAdmissionError::Closed);
                }
                if let Some(closing_thread) = state.closing_thread {
                    if closing_thread == current_thread {
                        return Err(BindingEngineAdmissionError::Closed);
                    }
                    state = self
                        .close_complete
                        .wait(state)
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    continue;
                }
                if state.callback_active {
                    return Err(BindingEngineAdmissionError::ReentrantCall);
                }
                if state.active_operations != 0 {
                    return Err(BindingEngineAdmissionError::Busy);
                }

                let detached = detach();
                state.closing_thread = Some(current_thread);
                break detached;
            }
        };

        let completion = CloseCompletion {
            admission: Arc::clone(self),
        };
        drop(detached);
        drop(completion);
        Ok(())
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, AdmissionState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

struct CloseCompletion {
    admission: Arc<BindingEngineAdmission>,
}

impl Drop for CloseCompletion {
    fn drop(&mut self) {
        {
            let mut state = self.admission.lock_state();
            debug_assert!(state.closing_thread.is_some());
            state.closing_thread = None;
            state.closed = true;
        }
        self.admission.close_complete.notify_all();
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

    struct BlockingDrop {
        entered: Arc<std::sync::Barrier>,
        release: Arc<std::sync::Barrier>,
    }

    impl Drop for BlockingDrop {
        fn drop(&mut self) {
            self.entered.wait();
            self.release.wait();
        }
    }

    struct ReentrantDrop {
        admission: Arc<BindingEngineAdmission>,
        result: Arc<Mutex<Option<Result<(), BindingEngineAdmissionError>>>>,
    }

    impl Drop for ReentrantDrop {
        fn drop(&mut self) {
            let result = self.admission.try_close();
            *self
                .result
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(result);
        }
    }

    #[test]
    fn callback_free_operations_are_concurrent_across_threads() {
        let admission = BindingEngineAdmission::new(BindingEngineAdmissionMode::Concurrent);
        let release = Arc::new(std::sync::Barrier::new(3));
        let (entered_sender, entered_receiver) = std::sync::mpsc::channel();
        let mut threads = Vec::new();

        for _ in 0..2 {
            let admission = Arc::clone(&admission);
            let release = Arc::clone(&release);
            let entered_sender = entered_sender.clone();
            threads.push(std::thread::spawn(move || {
                let operation = admission.enter_operation().expect("concurrent operation");
                entered_sender.send(()).expect("report operation admission");
                release.wait();
                drop(operation);
            }));
        }
        drop(entered_sender);
        entered_receiver.recv().expect("first operation admission");
        entered_receiver.recv().expect("second operation admission");
        assert_eq!(
            admission.try_close(),
            Err(BindingEngineAdmissionError::Busy)
        );

        release.wait();
        for thread in threads {
            thread.join().expect("operation thread");
        }
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
    fn concurrent_close_waits_until_detached_state_is_destroyed() {
        let admission = BindingEngineAdmission::new(BindingEngineAdmissionMode::Concurrent);
        let entered = Arc::new(std::sync::Barrier::new(2));
        let release = Arc::new(std::sync::Barrier::new(2));
        let first_admission = Arc::clone(&admission);
        let first_entered = Arc::clone(&entered);
        let first_release = Arc::clone(&release);
        let first = std::thread::spawn(move || {
            first_admission.try_close_detaching(|| BlockingDrop {
                entered: first_entered,
                release: first_release,
            })
        });

        entered.wait();
        let second_admission = Arc::clone(&admission);
        let (done_sender, done_receiver) = std::sync::mpsc::channel();
        let second = std::thread::spawn(move || {
            done_sender
                .send(second_admission.try_close())
                .expect("report second close");
        });
        assert!(
            done_receiver
                .recv_timeout(std::time::Duration::from_millis(50))
                .is_err(),
            "a concurrent close must not finish before detached state is destroyed"
        );

        release.wait();
        first.join().expect("first close thread").unwrap();
        assert_eq!(
            done_receiver.recv().expect("second close result"),
            Err(BindingEngineAdmissionError::Closed)
        );
        second.join().expect("second close thread");
    }

    #[test]
    fn detached_state_may_reenter_close_on_the_destroying_thread() {
        let admission = BindingEngineAdmission::new(BindingEngineAdmissionMode::Concurrent);
        let result = Arc::new(Mutex::new(None));

        admission
            .try_close_detaching(|| ReentrantDrop {
                admission: Arc::clone(&admission),
                result: Arc::clone(&result),
            })
            .expect("outer close");

        assert_eq!(
            result
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .take(),
            Some(Err(BindingEngineAdmissionError::Closed))
        );
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
