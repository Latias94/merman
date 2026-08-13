//! Target-neutral lifecycle primitives for CPU-bound Merman operations.
//!
//! Parsing, analysis, SVG, ASCII, and export all need the same operation-scoped cancellation and
//! deadline semantics. Their layout and output budgets remain adapter-owned.

use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
#[cfg(any(
    not(all(target_arch = "wasm32", target_os = "unknown")),
    feature = "operation-deadlines",
    feature = "system-timing",
    test
))]
use std::time::Duration;

#[cfg(not(all(
    target_arch = "wasm32",
    target_os = "unknown",
    any(feature = "operation-deadlines", feature = "system-timing")
)))]
use std::time::Instant;
#[cfg(all(
    target_arch = "wasm32",
    target_os = "unknown",
    any(feature = "operation-deadlines", feature = "system-timing")
))]
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
    #[cfg(any(test, feature = "test-support"))]
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
            #[cfg(any(test, feature = "test-support"))]
            successful_checkpoints_before_cancellation: AtomicU64::new(u64::MAX),
        }
    }

    fn latch_reason(&self, reason: CancelReason) -> CancelReason {
        let encoded = encode_reason(reason);
        match self.terminal_reason.compare_exchange(
            NO_TERMINATION,
            encoded,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => reason,
            Err(terminal) => decode_reason(terminal),
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
    default_phase: OperationPhase,
}

impl Default for OperationControl {
    fn default() -> Self {
        Self::new()
    }
}

impl OperationControl {
    /// Creates active control using the platform monotonic clock.
    pub fn new() -> Self {
        Self::with_clock(default_clock())
    }

    /// Creates active control with a supplied monotonic clock.
    ///
    /// This remains crate-private; public callers use [`Self::with_deadline`]. Tests use the hook
    /// to advance a fake monotonic clock without sleeping.
    fn with_clock(clock: Clock) -> Self {
        Self {
            state: Arc::new(OperationState::new(None, clock)),
            default_phase: OperationPhase::Unknown,
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
            default_phase: self.default_phase,
        }
    }

    /// Creates a shared view whose unnamed checkpoints report the supplied operation phase.
    pub fn for_phase(&self, phase: OperationPhase) -> Self {
        Self {
            state: Arc::clone(&self.state),
            default_phase: phase,
        }
    }

    /// Sets a relative monotonic deadline if one is not already configured.
    #[cfg(any(
        not(all(target_arch = "wasm32", target_os = "unknown")),
        feature = "operation-deadlines",
        feature = "system-timing"
    ))]
    pub fn with_deadline(self, timeout: Duration) -> Self {
        self.set_deadline(timeout);
        self
    }

    /// Sets a relative monotonic deadline, returning whether this call installed it.
    #[cfg(any(
        not(all(target_arch = "wasm32", target_os = "unknown")),
        feature = "operation-deadlines",
        feature = "system-timing"
    ))]
    pub fn set_deadline(&self, timeout: Duration) -> bool {
        let now = (self.state.clock)();
        self.state
            .deadline
            .set(deadline_after(now, timeout))
            .is_ok()
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
        self.checkpoint_at(self.default_phase)
    }

    /// Checks cancellation/deadline at a named phase.
    pub fn checkpoint_at(&self, phase: OperationPhase) -> Result<(), OperationCancelled> {
        #[cfg(any(test, feature = "test-support"))]
        if self.consume_scheduled_checkpoint() {
            self.cancel();
        }

        if let Some(reason) = self.observe_terminal_reason() {
            return Err(OperationCancelled { phase, reason });
        }
        Ok(())
    }

    fn observe_terminal_reason(&self) -> Option<CancelReason> {
        let (reason, inherited) = self.resolve_terminal_reason()?;
        Some(if inherited {
            self.state.latch_reason(reason)
        } else {
            reason
        })
    }

    fn resolve_terminal_reason(&self) -> Option<(CancelReason, bool)> {
        let mut state = Some(self.state.as_ref());
        let mut inherited = false;
        while let Some(current) = state {
            let terminal = current.terminal_reason.load(Ordering::Acquire);
            if terminal != NO_TERMINATION {
                return Some((decode_reason(terminal), inherited));
            }

            if current.cancelled.load(Ordering::Acquire) {
                return Some((current.latch_reason(CancelReason::Requested), inherited));
            }

            if current
                .deadline
                .get()
                .is_some_and(|deadline| (current.clock)() >= *deadline)
            {
                return Some((
                    current.latch_reason(CancelReason::DeadlineExceeded),
                    inherited,
                ));
            }

            state = current.parent.as_deref();
            inherited = true;
        }
        None
    }

    #[cfg(any(test, feature = "test-support"))]
    fn consume_scheduled_checkpoint(&self) -> bool {
        self.state
            .successful_checkpoints_before_cancellation
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |remaining| {
                (remaining != u64::MAX).then(|| remaining.saturating_sub(1))
            })
            .is_ok_and(|remaining| remaining == 0)
    }

    /// Schedules deterministic cancellation for tests after the requested successful checkpoints.
    #[cfg(any(test, feature = "test-support"))]
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

const fn encode_reason(reason: CancelReason) -> u8 {
    match reason {
        CancelReason::Requested => REQUESTED_TERMINATION,
        CancelReason::DeadlineExceeded => DEADLINE_TERMINATION,
    }
}

#[cfg(any(
    not(all(target_arch = "wasm32", target_os = "unknown")),
    feature = "operation-deadlines",
    feature = "system-timing"
))]
fn deadline_after(now: Instant, mut timeout: Duration) -> Instant {
    loop {
        if let Some(deadline) = now.checked_add(timeout) {
            return deadline;
        }
        timeout /= 2;
    }
}

fn default_clock() -> Clock {
    #[cfg(not(all(
        target_arch = "wasm32",
        target_os = "unknown",
        not(any(feature = "operation-deadlines", feature = "system-timing"))
    )))]
    {
        Arc::new(Instant::now)
    }

    #[cfg(all(
        target_arch = "wasm32",
        target_os = "unknown",
        not(any(feature = "operation-deadlines", feature = "system-timing"))
    ))]
    {
        Arc::new(|| unreachable!("deadline clock is unavailable in this artifact"))
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
    use std::sync::Barrier;
    use std::sync::atomic::AtomicU64;
    use std::thread;

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
    fn phase_views_share_state_and_label_unnamed_checkpoints() {
        let control = OperationControl::new();
        let parse = control.for_phase(OperationPhase::Parse);

        control.cancel();

        let error = parse.checkpoint().unwrap_err();
        assert_eq!(error.phase, OperationPhase::Parse);
        assert_eq!(error.reason, CancelReason::Requested);
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
    fn child_latches_the_first_reason_observed_across_its_parent_chain() {
        let now = Arc::new(AtomicU64::new(0));
        let source = Arc::clone(&now);
        let base = Instant::now();
        let clock = Arc::new(move || base + Duration::from_millis(source.load(Ordering::Relaxed)));
        let parent = OperationControl::with_clock(clock);
        let child = parent.child().with_deadline(Duration::from_millis(1));

        parent.cancel();
        assert_eq!(
            child
                .checkpoint_at(OperationPhase::Parse)
                .unwrap_err()
                .reason,
            CancelReason::Requested
        );
        now.store(2, Ordering::Relaxed);
        assert_eq!(
            child
                .checkpoint_at(OperationPhase::Layout)
                .unwrap_err()
                .reason,
            CancelReason::Requested
        );
    }

    #[test]
    fn child_keeps_its_requested_reason_after_parent_deadline_expires() {
        let now = Arc::new(AtomicU64::new(0));
        let source = Arc::clone(&now);
        let base = Instant::now();
        let clock = Arc::new(move || base + Duration::from_millis(source.load(Ordering::Relaxed)));
        let parent = OperationControl::with_clock(clock).with_deadline(Duration::from_millis(1));
        let child = parent.child();

        child.cancel();
        assert_eq!(
            child
                .checkpoint_at(OperationPhase::Parse)
                .unwrap_err()
                .reason,
            CancelReason::Requested
        );
        now.store(2, Ordering::Relaxed);
        assert_eq!(
            child
                .checkpoint_at(OperationPhase::Layout)
                .unwrap_err()
                .reason,
            CancelReason::Requested
        );
    }

    #[test]
    fn child_latches_an_expired_parent_deadline() {
        let now = Arc::new(AtomicU64::new(0));
        let source = Arc::clone(&now);
        let base = Instant::now();
        let clock = Arc::new(move || base + Duration::from_millis(source.load(Ordering::Relaxed)));
        let parent = OperationControl::with_clock(clock).with_deadline(Duration::from_millis(1));
        let child = parent.child();

        now.store(2, Ordering::Relaxed);
        assert_eq!(
            child
                .checkpoint_at(OperationPhase::Parse)
                .unwrap_err()
                .reason,
            CancelReason::DeadlineExceeded
        );
        parent.cancel();
        assert_eq!(
            child
                .checkpoint_at(OperationPhase::Layout)
                .unwrap_err()
                .reason,
            CancelReason::DeadlineExceeded
        );
    }

    #[test]
    fn oversized_deadline_clamps_without_panicking_or_expiring_immediately() {
        let control = OperationControl::new().with_deadline(Duration::MAX);
        assert!(control.state.deadline.get().is_some());
        assert_eq!(control.checkpoint(), Ok(()));
    }

    #[test]
    fn concurrent_checkpoints_never_lose_a_sticky_cancellation() {
        const THREADS: usize = 16;
        for _ in 0..32 {
            let control = OperationControl::new();
            control.cancel();
            let barrier = Arc::new(Barrier::new(THREADS));
            let handles = (0..THREADS)
                .map(|_| {
                    let control = control.clone();
                    let barrier = Arc::clone(&barrier);
                    thread::spawn(move || {
                        barrier.wait();
                        control.checkpoint_at(OperationPhase::Layout)
                    })
                })
                .collect::<Vec<_>>();

            for handle in handles {
                let cancelled = handle
                    .join()
                    .expect("checkpoint thread should not panic")
                    .expect_err("every concurrent observer must see cancellation");
                assert_eq!(cancelled.reason, CancelReason::Requested);
            }
        }
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

    #[test]
    fn ledger_concurrent_charges_never_exceed_the_limit() {
        const THREADS: usize = 32;
        const LIMIT: u64 = 7;
        let control = OperationControl::new();
        let ledger = Arc::new(OperationLedger::new("concurrent_work", Some(LIMIT)));
        let barrier = Arc::new(Barrier::new(THREADS));
        let handles = (0..THREADS)
            .map(|_| {
                let ledger = Arc::clone(&ledger);
                let barrier = Arc::clone(&barrier);
                let control = control.clone();
                thread::spawn(move || {
                    barrier.wait();
                    ledger.charge(&control, OperationPhase::Layout, 1)
                })
            })
            .collect::<Vec<_>>();

        let mut accepted = 0_u64;
        for handle in handles {
            match handle.join().expect("ledger thread should not panic") {
                Ok(_) => accepted += 1,
                Err(OperationLedgerError::ResourceLimitExceeded(error)) => {
                    assert_eq!(error.consumed, LIMIT);
                    assert_eq!(error.requested, 1);
                }
                Err(other) => panic!("unexpected ledger rejection: {other:?}"),
            }
        }

        assert_eq!(accepted, LIMIT);
        assert_eq!(ledger.consumed(), LIMIT);
    }
}
