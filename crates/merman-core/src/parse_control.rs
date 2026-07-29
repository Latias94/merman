use std::sync::Arc;
#[cfg(test)]
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(test)]
const NO_SCHEDULED_CANCELLATION: usize = usize::MAX;

/// Cooperative cancellation control for one parse operation.
///
/// Parsing remains synchronous. Long-running parser stages periodically call
/// [`ParseControl::checkpoint`] and return [`ParseCancelled`] through a channel that is distinct
/// from Mermaid syntax and semantic errors.
#[derive(Debug, Clone)]
pub struct ParseControl {
    state: Arc<ParseControlState>,
    #[cfg(test)]
    successful_checkpoints_before_cancellation: Arc<AtomicUsize>,
}

#[derive(Debug)]
struct ParseControlState {
    cancelled: AtomicBool,
    parent: Option<Arc<ParseControlState>>,
}

impl ParseControl {
    /// Creates an active parse control.
    pub fn new() -> Self {
        Self {
            state: Arc::new(ParseControlState {
                cancelled: AtomicBool::new(false),
                parent: None,
            }),
            #[cfg(test)]
            successful_checkpoints_before_cancellation: Arc::new(AtomicUsize::new(
                NO_SCHEDULED_CANCELLATION,
            )),
        }
    }

    /// Creates an independently cancellable child that also observes this control.
    pub fn child(&self) -> Self {
        Self {
            state: Arc::new(ParseControlState {
                cancelled: AtomicBool::new(false),
                parent: Some(Arc::clone(&self.state)),
            }),
            #[cfg(test)]
            successful_checkpoints_before_cancellation: Arc::new(AtomicUsize::new(
                NO_SCHEDULED_CANCELLATION,
            )),
        }
    }

    /// Requests cancellation for this control and all of its clones.
    pub fn cancel(&self) {
        self.state.cancelled.store(true, Ordering::Release);
    }

    /// Returns whether cancellation was requested locally or by an ancestor control.
    pub fn is_cancelled(&self) -> bool {
        let mut state = self.state.as_ref();
        loop {
            if state.cancelled.load(Ordering::Acquire) {
                return true;
            }
            let Some(parent) = state.parent.as_deref() else {
                return false;
            };
            state = parent;
        }
    }

    /// Stops the current parse at a cooperative boundary when cancellation was requested.
    pub fn checkpoint(&self) -> ParseControlResult<()> {
        if self.is_cancelled() {
            return Err(ParseCancelled);
        }

        #[cfg(test)]
        if let Ok(remaining) = self
            .successful_checkpoints_before_cancellation
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |remaining| {
                (remaining != NO_SCHEDULED_CANCELLATION).then(|| remaining.saturating_sub(1))
            })
            && remaining == 0
        {
            self.cancel();
            return Err(ParseCancelled);
        }

        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn cancel_after_checkpoints(&self, successful_checkpoints: usize) {
        self.successful_checkpoints_before_cancellation
            .store(successful_checkpoints, Ordering::Relaxed);
    }
}

impl Default for ParseControl {
    fn default() -> Self {
        Self::new()
    }
}

/// Signals cooperative parse cancellation without converting it into a Mermaid parse failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("parse operation cancelled")]
pub struct ParseCancelled;

/// Result channel reserved for cooperative parse cancellation.
pub type ParseControlResult<T> = std::result::Result<T, ParseCancelled>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clones_observe_the_same_cancellation_request() {
        let control = ParseControl::new();
        let worker = control.clone();

        assert!(!worker.is_cancelled());
        control.cancel();

        assert!(worker.is_cancelled());
        assert_eq!(worker.checkpoint(), Err(ParseCancelled));
    }

    #[test]
    fn test_schedule_cancels_after_the_requested_successful_checkpoints() {
        let control = ParseControl::new();
        control.cancel_after_checkpoints(2);

        assert_eq!(control.checkpoint(), Ok(()));
        assert_eq!(control.checkpoint(), Ok(()));
        assert_eq!(control.checkpoint(), Err(ParseCancelled));
        assert!(control.is_cancelled());
    }

    #[test]
    fn child_cancellation_is_local_but_parent_cancellation_propagates() {
        let parent = ParseControl::new();
        let child = parent.child();

        child.cancel();
        assert!(child.is_cancelled());
        assert!(!parent.is_cancelled());

        let sibling = parent.child();
        parent.cancel();
        assert!(sibling.is_cancelled());
        assert_eq!(sibling.checkpoint(), Err(ParseCancelled));
    }
}
