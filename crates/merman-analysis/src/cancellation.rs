use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A cheap, runtime-independent cancellation signal for CPU-bound analysis.
#[derive(Debug, Clone, Default)]
pub struct AnalysisCancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl AnalysisCancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub fn checkpoint(&self) -> Result<(), AnalysisCancelled> {
        if self.is_cancelled() {
            Err(AnalysisCancelled)
        } else {
            Ok(())
        }
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
}
