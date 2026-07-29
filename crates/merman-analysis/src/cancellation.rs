#[cfg(test)]
use std::sync::Arc;
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(test)]
const NO_SCHEDULED_CANCELLATION: usize = usize::MAX;

/// A cheap, runtime-independent cancellation signal for CPU-bound analysis.
#[derive(Debug, Clone)]
pub struct AnalysisCancellationToken {
    parse_control: merman_core::ParseControl,
    #[cfg(test)]
    successful_checkpoints_before_cancellation: Arc<AtomicUsize>,
}

impl Default for AnalysisCancellationToken {
    fn default() -> Self {
        Self {
            parse_control: merman_core::ParseControl::new(),
            #[cfg(test)]
            successful_checkpoints_before_cancellation: Arc::new(AtomicUsize::new(
                NO_SCHEDULED_CANCELLATION,
            )),
        }
    }
}

impl AnalysisCancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an independently cancellable child that also observes this token.
    pub fn child(&self) -> Self {
        Self {
            parse_control: self.parse_control.child(),
            #[cfg(test)]
            successful_checkpoints_before_cancellation: Arc::new(AtomicUsize::new(
                NO_SCHEDULED_CANCELLATION,
            )),
        }
    }

    pub fn cancel(&self) {
        self.parse_control.cancel();
    }

    pub fn is_cancelled(&self) -> bool {
        self.parse_control.is_cancelled()
    }

    pub fn checkpoint(&self) -> Result<(), AnalysisCancelled> {
        self.parse_control
            .checkpoint()
            .map_err(|_| AnalysisCancelled)?;

        #[cfg(test)]
        if let Ok(remaining) = self
            .successful_checkpoints_before_cancellation
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |remaining| {
                (remaining != NO_SCHEDULED_CANCELLATION).then(|| remaining.saturating_sub(1))
            })
            && remaining == 0
        {
            self.cancel();
            return Err(AnalysisCancelled);
        }

        Ok(())
    }

    pub(crate) fn parse_control(&self) -> &merman_core::ParseControl {
        &self.parse_control
    }

    #[cfg(test)]
    pub(crate) fn cancel_after_checkpoints(&self, successful_checkpoints: usize) {
        self.successful_checkpoints_before_cancellation
            .store(successful_checkpoints, Ordering::Relaxed);
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
}
