/// A cheap, runtime-independent cancellation signal for CPU-bound analysis.
#[derive(Debug, Clone)]
pub struct AnalysisCancellationToken {
    operation: merman_core::OperationControl,
}

impl Default for AnalysisCancellationToken {
    fn default() -> Self {
        Self {
            operation: merman_core::OperationControl::new()
                .for_phase(merman_core::OperationPhase::Analysis),
        }
    }
}

impl AnalysisCancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    /// Shares a caller-owned operation control with the analysis pipeline.
    ///
    /// Unnamed analysis checkpoints report the analysis phase while cancellation and deadline
    /// state remain shared with the caller.
    pub fn from_operation_control(operation: &merman_core::OperationControl) -> Self {
        Self {
            operation: operation.for_phase(merman_core::OperationPhase::Analysis),
        }
    }

    /// Creates an independently cancellable child that also observes this token.
    pub fn child(&self) -> Self {
        Self {
            operation: self.operation.child(),
        }
    }

    pub fn cancel(&self) {
        self.operation.cancel();
    }

    pub fn is_cancelled(&self) -> bool {
        self.operation.is_cancelled()
    }

    pub fn checkpoint(&self) -> Result<(), AnalysisCancelled> {
        self.operation.checkpoint().map_err(|_| AnalysisCancelled)
    }

    /// Returns the structured terminal condition currently observed by this analysis token.
    #[must_use]
    pub fn operation_cancellation(&self) -> Option<merman_core::OperationCancelled> {
        self.operation.checkpoint().err()
    }

    pub(crate) fn operation_control(&self) -> &merman_core::OperationControl {
        &self.operation
    }

    /// Schedules deterministic cancellation for tests after the requested successful checkpoints.
    #[cfg(test)]
    pub fn cancel_after_checkpoints(&self, successful_checkpoints: usize) {
        self.operation
            .cancel_after_checkpoints(successful_checkpoints);
    }
}

/// Returned when a caller cancels an in-progress analysis generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("analysis cancelled")]
pub struct AnalysisCancelled;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clones_observe_cancellation() {
        let token = AnalysisCancellationToken::new();
        let clone = token.clone();

        token.cancel();

        assert!(clone.is_cancelled());
        assert_eq!(clone.checkpoint(), Err(AnalysisCancelled));
    }

    #[test]
    fn child_cancellation_is_local_but_parent_cancellation_propagates() {
        let parent = AnalysisCancellationToken::new();
        let child = parent.child();

        child.cancel();
        assert!(child.is_cancelled());
        assert!(!parent.is_cancelled());

        let sibling = parent.child();
        parent.cancel();
        assert!(sibling.is_cancelled());
        assert_eq!(sibling.checkpoint(), Err(AnalysisCancelled));
    }

    #[test]
    fn caller_operation_cancellation_propagates_with_analysis_phase() {
        let operation = merman_core::OperationControl::new();
        let token = AnalysisCancellationToken::from_operation_control(&operation);

        operation.cancel();

        assert_eq!(token.checkpoint(), Err(AnalysisCancelled));
        let error = token.operation_cancellation().unwrap();
        assert_eq!(error.phase, merman_core::OperationPhase::Analysis);
        assert_eq!(error.reason, merman_core::CancelReason::Requested);
    }

    #[test]
    fn caller_operation_deadline_propagates_with_analysis_phase() {
        let operation =
            merman_core::OperationControl::new().with_deadline(std::time::Duration::ZERO);
        let token = AnalysisCancellationToken::from_operation_control(&operation);

        assert_eq!(token.checkpoint(), Err(AnalysisCancelled));
        let error = token.operation_cancellation().unwrap();
        assert_eq!(error.phase, merman_core::OperationPhase::Analysis);
        assert_eq!(error.reason, merman_core::CancelReason::DeadlineExceeded);
    }
}
