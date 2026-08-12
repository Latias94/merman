use crate::operation::{OperationControl, OperationPhase};

/// Cooperative cancellation control for one parse operation.
///
/// Parsing remains synchronous. Long-running parser stages periodically call
/// [`ParseControl::checkpoint`] and return [`ParseCancelled`] through a channel that is distinct
/// from Mermaid syntax and semantic errors.
#[derive(Debug, Clone)]
pub struct ParseControl {
    operation: OperationControl,
}

impl ParseControl {
    /// Creates an active parse control.
    pub fn new() -> Self {
        Self {
            operation: OperationControl::new(),
        }
    }

    /// Creates an independently cancellable child that also observes this control.
    pub fn child(&self) -> Self {
        Self {
            operation: self.operation.child(),
        }
    }

    /// Requests cancellation for this control and all of its clones.
    pub fn cancel(&self) {
        self.operation.cancel();
    }

    /// Returns whether cancellation was requested locally or by an ancestor control.
    pub fn is_cancelled(&self) -> bool {
        self.operation.is_cancelled()
    }

    /// Stops the current parse at a cooperative boundary when cancellation was requested.
    pub fn checkpoint(&self) -> ParseControlResult<()> {
        self.operation
            .checkpoint_at(OperationPhase::Parse)
            .map_err(|_| ParseCancelled)
    }

    /// Returns the shared target-neutral operation control for adapter integration.
    pub fn operation_control(&self) -> &OperationControl {
        &self.operation
    }

    #[doc(hidden)]
    pub fn cancel_after_checkpoints(&self, successful_checkpoints: usize) {
        self.operation
            .cancel_after_checkpoints(successful_checkpoints);
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
