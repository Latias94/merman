//! Target-neutral lifecycle primitives for CPU-bound Merman operations.
//!
//! Parsing, analysis, SVG, ASCII, and export all need the same operation-scoped cancellation and
//! deadline semantics. Their layout and output budgets remain adapter-owned.

use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use web_time::Instant;

const NO_TERMINATION: u8 = 0;
const REQUESTED_TERMINATION: u8 = 1;
const DEADLINE_TERMINATION: u8 = 2;

/// The broad phase in which an operation observes a terminal condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum OperationPhase {
    Admission,
    Parse,
    Semantic,
    Analysis,
    Layout,
    Emit,
    Postprocess,
    Export,
    Unknown,
}

impl OperationPhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Admission => "admission",
            Self::Parse => "parse",
            Self::Semantic => "semantic",
            Self::Analysis => "analysis",
            Self::Layout => "layout",
            Self::Emit => "emit",
            Self::Postprocess => "postprocess",
            Self::Export => "export",
            Self::Unknown => "unknown",
        }
    }
}

impl fmt::Display for OperationPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why an operation stopped at a cooperative checkpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CancelReason {
    Requested,
    DeadlineExceeded,
}

impl CancelReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::DeadlineExceeded => "deadline_exceeded",
        }
    }
}

impl fmt::Display for CancelReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Structured cooperative cancellation returned by an operation checkpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("operation cancelled during {phase}: {reason}")]
pub struct OperationCancelled {
    pub phase: OperationPhase,
    pub reason: CancelReason,
}

/// Result channel for a controlled operation stage.
pub type OperationControlResult<T> = std::result::Result<T, OperationCancelled>;

type Clock = Arc<dyn Fn() -> Instant + Send + Sync + 'static>;

struct OperationState {
    cancelled: AtomicBool,
    terminal_reason: AtomicU8,
    parent: Option<Arc<OperationState>>,
    deadline: OnceLock<Instant>,
    clock: Clock,
    successful_checkpoints_before_cancellation: AtomicU64,
}

impl fmt::Debug for OperationState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OperationState")
            .field("cancelled", &self.cancelled.load(Ordering::Acquire))
            .field(
                "terminal_reason",
                &self.terminal_reason.load(Ordering::Acquire),
            )
            .field("deadline", &self.deadline.get())
            .finish_non_exhaustive()
    }
}

impl OperationState {
    fn new(parent: Option<Arc<OperationState>>, clock: Clock) -> Self {
        Self {
            cancelled: AtomicBool::new(false),
            terminal_reason: AtomicU8::new(NO_TERMINATION),
            parent,
            deadline: OnceLock::new(),
            clock,
            successful_checkpoints_before_cancellation: AtomicU64::new(u64::MAX),
        }
    }
}

/// Cloneable operation-scoped cooperative cancellation and deadline control.
///
/// A synchronous callback cannot be forcefully interrupted. It observes a request when it returns
/// to a checkpoint. Once observed, the terminal reason is sticky.
#[derive(Clone, Debug)]
pub struct OperationControl {
    state: Arc<OperationState>,
}

impl Default for OperationControl {
    fn default() -> Self {
        Self::new()
    }
}

impl OperationControl {
    /// Creates active control using the platform monotonic clock.
    pub fn new() -> Self {
        Self::with_clock(Arc::new(Instant::now))
    }

    /// Creates active control with a supplied monotonic clock.
    ///
    /// This remains crate-private; public callers use [`Self::with_deadline`]. Tests use the hook
    /// to advance a fake monotonic clock without sleeping.
    fn with_clock(clock: Clock) -> Self {
        Self {
            state: Arc::new(OperationState::new(None, clock)),
        }
    }

    /// Creates an independently cancellable child that observes this control's cancellation and
    /// deadline state.
    pub fn child(&self) -> Self {
        Self {
            state: Arc::new(OperationState::new(
                Some(Arc::clone(&self.state)),
                Arc::clone(&self.state.clock),
            )),
        }
    }

    /// Sets a relative monotonic deadline if one is not already configured.
    pub fn with_deadline(self, timeout: Duration) -> Self {
        self.set_deadline(timeout);
        self
    }

    /// Sets a relative monotonic deadline, returning whether this call installed it.
    pub fn set_deadline(&self, timeout: Duration) -> bool {
        self.state
            .deadline
            .set((self.state.clock)() + timeout)
            .is_ok()
    }

    /// Returns the configured absolute monotonic deadline.
    pub fn deadline(&self) -> Option<Instant> {
        self.state.deadline.get().copied()
    }

    /// Requests cancellation for this control and its clones.
    pub fn cancel(&self) {
        self.state.cancelled.store(true, Ordering::Release);
    }

    /// Returns whether this control or an ancestor has received a cancellation request.
    pub fn is_cancelled(&self) -> bool {
        let mut state = Some(self.state.as_ref());
        while let Some(current) = state {
            if current.cancelled.load(Ordering::Acquire) {
                return true;
            }
            state = current.parent.as_deref();
        }
        false
    }

    /// Checks cancellation/deadline at an unspecified phase.
    pub fn checkpoint(&self) -> Result<(), OperationCancelled> {
        self.checkpoint_at(OperationPhase::Unknown)
    }

    /// Checks cancellation/deadline at a named phase.
    pub fn checkpoint_at(&self, phase: OperationPhase) -> Result<(), OperationCancelled> {
        if self.consume_scheduled_checkpoint() {
            self.cancel();
        }

        if let Some(reason) = self.observe_terminal_reason() {
            return Err(OperationCancelled { phase, reason });
        }
        Ok(())
    }

    fn observe_terminal_reason(&self) -> Option<CancelReason> {
        let mut state = Some(self.state.as_ref());
        while let Some(current) = state {
            let terminal = current.terminal_reason.load(Ordering::Acquire);
            if terminal != NO_TERMINATION {
                return Some(decode_reason(terminal));
            }

            if current.cancelled.load(Ordering::Acquire)
                && current
                    .terminal_reason
                    .compare_exchange(
                        NO_TERMINATION,
                        REQUESTED_TERMINATION,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    )
                    .is_ok()
            {
                return Some(CancelReason::Requested);
            }

            if current
                .deadline
                .get()
                .is_some_and(|deadline| (current.clock)() >= *deadline)
                && current
                    .terminal_reason
                    .compare_exchange(
                        NO_TERMINATION,
                        DEADLINE_TERMINATION,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    )
                    .is_ok()
            {
                return Some(CancelReason::DeadlineExceeded);
            }

            state = current.parent.as_deref();
        }
        None
    }

    fn consume_scheduled_checkpoint(&self) -> bool {
        self.state
            .successful_checkpoints_before_cancellation
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |remaining| {
                (remaining != u64::MAX).then(|| remaining.saturating_sub(1))
            })
            .is_ok_and(|remaining| remaining == 0)
    }

    #[doc(hidden)]
    pub fn cancel_after_checkpoints(&self, successful_checkpoints: usize) {
        self.state
            .successful_checkpoints_before_cancellation
            .store(successful_checkpoints as u64, Ordering::Relaxed);
    }
}

fn decode_reason(value: u8) -> CancelReason {
    match value {
        DEADLINE_TERMINATION => CancelReason::DeadlineExceeded,
        _ => CancelReason::Requested,
    }
}

/// Target-neutral description of a checked operation ledger rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error(
    "operation resource limit `{id}` exceeded during {phase}: {consumed} + {requested} > {limit}"
)]
pub struct OperationResourceLimitExceeded {
    pub id: &'static str,
    pub phase: OperationPhase,
    pub limit: u64,
    pub consumed: u64,
    pub requested: u64,
}

/// Failure returned by [`OperationLedger::charge`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum OperationLedgerError {
    #[error(transparent)]
    Cancelled(#[from] OperationCancelled),
    #[error(transparent)]
    ResourceLimitExceeded(#[from] OperationResourceLimitExceeded),
    #[error("operation ledger arithmetic overflow during {phase}")]
    ArithmeticOverflow { phase: OperationPhase },
}

/// A checked operation-owned work ledger. Target-specific quotas remain adapter-owned.
#[derive(Debug)]
pub struct OperationLedger {
    id: &'static str,
    limit: Option<u64>,
    consumed: AtomicU64,
    rejected: AtomicBool,
}

impl OperationLedger {
    pub const fn new(id: &'static str, limit: Option<u64>) -> Self {
        Self {
            id,
            limit,
            consumed: AtomicU64::new(0),
            rejected: AtomicBool::new(false),
        }
    }

    pub fn consumed(&self) -> u64 {
        self.consumed.load(Ordering::Acquire)
    }

    pub fn limit(&self) -> Option<u64> {
        self.limit
    }

    pub fn charge(
        &self,
        control: &OperationControl,
        phase: OperationPhase,
        requested: u64,
    ) -> Result<u64, OperationLedgerError> {
        control.checkpoint_at(phase)?;
        if self.rejected.load(Ordering::Acquire) {
            return Err(self.limit_error(phase, requested));
        }

        let mut consumed = self.consumed.load(Ordering::Acquire);
        loop {
            let next = consumed
                .checked_add(requested)
                .ok_or(OperationLedgerError::ArithmeticOverflow { phase })?;
            if self.limit.is_some_and(|limit| next > limit) {
                self.rejected.store(true, Ordering::Release);
                return Err(self.limit_error(phase, requested));
            }
            match self.consumed.compare_exchange_weak(
                consumed,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(next),
                Err(actual) => consumed = actual,
            }
        }
    }

    fn limit_error(&self, phase: OperationPhase, requested: u64) -> OperationLedgerError {
        OperationLedgerError::ResourceLimitExceeded(OperationResourceLimitExceeded {
            id: self.id,
            phase,
            limit: self.limit.unwrap_or(u64::MAX),
            consumed: self.consumed(),
            requested,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU64;

    #[test]
    fn clones_and_children_observe_shared_cancellation() {
        let control = OperationControl::new();
        let clone = control.clone();
        let child = control.child();
        child.cancel();
        assert!(child.is_cancelled());
        assert!(!control.is_cancelled());
        control.cancel();
        assert!(clone.is_cancelled());
        assert!(child.checkpoint_at(OperationPhase::Parse).is_err());
    }

    #[test]
    fn deadline_is_sticky_and_distinct_from_requested_cancellation() {
        let now = Arc::new(AtomicU64::new(0));
        let source = Arc::clone(&now);
        let base = Instant::now();
        let clock = Arc::new(move || base + Duration::from_millis(source.load(Ordering::Relaxed)));
        let control = OperationControl::with_clock(clock).with_deadline(Duration::from_millis(0));
        let error = control.checkpoint_at(OperationPhase::Layout).unwrap_err();
        assert_eq!(error.reason, CancelReason::DeadlineExceeded);
        assert_eq!(
            control.checkpoint().unwrap_err().reason,
            CancelReason::DeadlineExceeded
        );
    }

    #[test]
    fn ledger_checks_control_before_charging_and_does_not_advance_on_rejection() {
        let control = OperationControl::new();
        let ledger = OperationLedger::new("work", Some(2));
        assert_eq!(ledger.charge(&control, OperationPhase::Layout, 2), Ok(2));
        assert!(matches!(
            ledger.charge(&control, OperationPhase::Layout, 1),
            Err(OperationLedgerError::ResourceLimitExceeded(_))
        ));
        assert_eq!(ledger.consumed(), 2);
        control.cancel();
        assert!(matches!(
            ledger.charge(&control, OperationPhase::Layout, 0),
            Err(OperationLedgerError::Cancelled(_))
        ));
    }
}
