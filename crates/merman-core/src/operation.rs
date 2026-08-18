//! Target-neutral lifecycle primitives for CPU-bound Merman operations.
//!
//! Parsing, analysis, SVG, ASCII, and export all need the same operation-scoped cancellation and
//! deadline semantics. Their layout and output budgets remain adapter-owned.

use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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
    terminal: OnceLock<OperationLedgerError>,
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
            .field("terminal", &self.terminal.get())
            .field("deadline", &self.deadline.get())
            .finish_non_exhaustive()
    }
}

impl OperationState {
    fn new(parent: Option<Arc<OperationState>>, clock: Clock) -> Self {
        Self {
            cancelled: AtomicBool::new(false),
            terminal: OnceLock::new(),
            parent,
            deadline: OnceLock::new(),
            clock,
            #[cfg(any(test, feature = "test-support"))]
            successful_checkpoints_before_cancellation: AtomicU64::new(u64::MAX),
        }
    }

    fn latch_terminal(&self, error: OperationLedgerError) -> OperationLedgerError {
        *self.terminal.get_or_init(|| error)
    }
}

/// Cloneable operation-scoped cooperative cancellation and deadline control.
///
/// A synchronous callback cannot be forcefully interrupted. It observes a request when it returns
/// to a checkpoint. Cancellation, deadlines, resource ceilings, and resource arithmetic failures
/// compete for one sticky terminal outcome per operation scope.
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
    ///
    /// This cancellation-only projection is used by parsing and analysis code that cannot produce
    /// resource terminals. If a target adapter has already recorded a non-cancellation terminal,
    /// this method does not replace it with a later cancellation. Target adapters use
    /// [`Self::terminal_checkpoint_at`] to replay the complete terminal value.
    pub fn checkpoint_at(&self, phase: OperationPhase) -> Result<(), OperationCancelled> {
        self.observe_cancellation_at(phase).map_or(Ok(()), Err)
    }

    /// Checks cancellation, deadlines, and any previously observed operation terminal.
    ///
    /// Target adapters call this before formal charges and controlled work. Speculative estimates
    /// that may be discarded may use [`Self::checkpoint_at`], but they must not record a resource
    /// terminal.
    pub fn terminal_checkpoint_at(
        &self,
        phase: OperationPhase,
    ) -> Result<(), OperationLedgerError> {
        if let Some(error) = self.state.terminal.get() {
            return Err(*error);
        }
        let Some(cancellation) = self.observe_cancellation_at(phase) else {
            return self.state.terminal.get().copied().map_or(Ok(()), Err);
        };
        Err(self.latch_terminal_error(OperationLedgerError::Cancelled(cancellation)))
    }

    fn observe_cancellation_at(&self, phase: OperationPhase) -> Option<OperationCancelled> {
        #[cfg(any(test, feature = "test-support"))]
        if self.consume_scheduled_checkpoint() {
            self.cancel();
        }

        self.resolve_cancellation_at(phase)
    }

    fn resolve_cancellation_at(&self, phase: OperationPhase) -> Option<OperationCancelled> {
        let mut state = Some(self.state.as_ref());
        let mut inherited = false;
        while let Some(current) = state {
            let terminal = if let Some(error) = current.terminal.get() {
                match error {
                    OperationLedgerError::Cancelled(error) => Some(*error),
                    OperationLedgerError::ResourceLimitExceeded(_)
                    | OperationLedgerError::ArithmeticOverflow { .. }
                        if !inherited =>
                    {
                        return None;
                    }
                    OperationLedgerError::ResourceLimitExceeded(_)
                    | OperationLedgerError::ArithmeticOverflow { .. }
                        if current.cancelled.load(Ordering::Acquire) =>
                    {
                        Some(OperationCancelled {
                            phase,
                            reason: CancelReason::Requested,
                        })
                    }
                    OperationLedgerError::ResourceLimitExceeded(_)
                    | OperationLedgerError::ArithmeticOverflow { .. }
                        if current
                            .deadline
                            .get()
                            .is_some_and(|deadline| (current.clock)() >= *deadline) =>
                    {
                        Some(OperationCancelled {
                            phase,
                            reason: CancelReason::DeadlineExceeded,
                        })
                    }
                    OperationLedgerError::ResourceLimitExceeded(_)
                    | OperationLedgerError::ArithmeticOverflow { .. } => None,
                }
            } else if current.cancelled.load(Ordering::Acquire) {
                let cancellation = OperationCancelled {
                    phase,
                    reason: CancelReason::Requested,
                };
                match Self::resolve_latched_cancellation(current, inherited, cancellation) {
                    Some(error) => Some(error),
                    None => return None,
                }
            } else if current
                .deadline
                .get()
                .is_some_and(|deadline| (current.clock)() >= *deadline)
            {
                let cancellation = OperationCancelled {
                    phase,
                    reason: CancelReason::DeadlineExceeded,
                };
                match Self::resolve_latched_cancellation(current, inherited, cancellation) {
                    Some(error) => Some(error),
                    None => return None,
                }
            } else {
                None
            };

            if let Some(error) = terminal {
                if !inherited {
                    return Some(error);
                }
                return match self
                    .state
                    .latch_terminal(OperationLedgerError::Cancelled(error))
                {
                    OperationLedgerError::Cancelled(error) => Some(error),
                    OperationLedgerError::ResourceLimitExceeded(_)
                    | OperationLedgerError::ArithmeticOverflow { .. } => None,
                };
            }

            state = current.parent.as_deref();
            inherited = true;
        }
        None
    }

    fn resolve_latched_cancellation(
        state: &OperationState,
        inherited: bool,
        cancellation: OperationCancelled,
    ) -> Option<OperationCancelled> {
        match state.latch_terminal(OperationLedgerError::Cancelled(cancellation)) {
            OperationLedgerError::Cancelled(error) => Some(error),
            OperationLedgerError::ResourceLimitExceeded(_)
            | OperationLedgerError::ArithmeticOverflow { .. }
                if inherited =>
            {
                Some(cancellation)
            }
            OperationLedgerError::ResourceLimitExceeded(_)
            | OperationLedgerError::ArithmeticOverflow { .. } => None,
        }
    }

    fn latch_terminal_error(&self, error: OperationLedgerError) -> OperationLedgerError {
        self.state.latch_terminal(error)
    }

    /// Records a target-owned resource ceiling as this operation's first terminal outcome.
    pub fn terminate_resource_limit(
        &self,
        error: OperationResourceLimitExceeded,
    ) -> OperationLedgerError {
        self.latch_terminal_error(OperationLedgerError::ResourceLimitExceeded(error))
    }

    /// Records target-owned resource accounting overflow as this operation's first terminal outcome.
    pub fn terminate_resource_overflow(
        &self,
        id: &'static str,
        phase: OperationPhase,
    ) -> OperationLedgerError {
        self.latch_terminal_error(OperationLedgerError::ArithmeticOverflow { id, phase })
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
    #[error("operation resource `{id}` arithmetic overflow during {phase}")]
    ArithmeticOverflow {
        id: &'static str,
        phase: OperationPhase,
    },
}

/// A checked operation-scoped work ledger. Target-specific quotas remain adapter-owned.
///
/// Every call for one ledger must use clones or phase views of the same [`OperationControl`]. A
/// child control starts a distinct ledger-terminal scope even though it observes its parent's
/// cancellation and deadline.
#[derive(Debug)]
pub struct OperationLedger {
    id: &'static str,
    limit: Option<u64>,
    consumed: AtomicU64,
}

impl OperationLedger {
    pub const fn new(id: &'static str, limit: Option<u64>) -> Self {
        Self {
            id,
            limit,
            consumed: AtomicU64::new(0),
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
        control.terminal_checkpoint_at(phase)?;

        let mut consumed = self.consumed.load(Ordering::Acquire);
        loop {
            let next = match consumed.checked_add(requested) {
                Some(next) => next,
                None => {
                    return Err(control.terminate_resource_overflow(self.id, phase));
                }
            };
            if self.limit.is_some_and(|limit| next > limit) {
                return Err(
                    control.latch_terminal_error(self.limit_error(phase, requested, consumed))
                );
            }
            match self.consumed.compare_exchange_weak(
                consumed,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(next),
                Err(actual) => {
                    control.terminal_checkpoint_at(phase)?;
                    consumed = actual;
                }
            }
        }
    }

    fn limit_error(
        &self,
        phase: OperationPhase,
        requested: u64,
        consumed: u64,
    ) -> OperationLedgerError {
        OperationLedgerError::ResourceLimitExceeded(OperationResourceLimitExceeded {
            id: self.id,
            phase,
            limit: self.limit.unwrap_or(u64::MAX),
            consumed,
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
        assert_eq!(control.checkpoint().unwrap_err(), error);

        let ledger = OperationLedger::new("work", Some(0));
        assert_eq!(
            ledger
                .charge(&control, OperationPhase::Emit, 1)
                .unwrap_err(),
            OperationLedgerError::Cancelled(error)
        );
        assert_eq!(ledger.consumed(), 0);
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
    fn child_keeps_an_observed_parent_cancellation_when_parent_resource_wins_the_latch() {
        let parent = OperationControl::new();
        let child = parent.child();
        let cancellation = OperationCancelled {
            phase: OperationPhase::Parse,
            reason: CancelReason::Requested,
        };
        let parent_terminal = OperationLedgerError::ArithmeticOverflow {
            id: "parent_work",
            phase: OperationPhase::Layout,
        };

        parent.cancel();
        assert_eq!(
            parent.latch_terminal_error(parent_terminal),
            parent_terminal
        );
        let observed = OperationControl::resolve_latched_cancellation(
            parent.state.as_ref(),
            true,
            cancellation,
        )
        .expect("an ancestor resource terminal must not hide an observed cancellation");
        assert_eq!(observed, cancellation);
        assert_eq!(
            child.latch_terminal_error(OperationLedgerError::Cancelled(observed)),
            OperationLedgerError::Cancelled(cancellation)
        );
        assert_eq!(
            child
                .terminal_checkpoint_at(OperationPhase::Emit)
                .expect_err("the child must replay its cancellation"),
            OperationLedgerError::Cancelled(cancellation)
        );
        assert_eq!(
            parent
                .terminal_checkpoint_at(OperationPhase::Emit)
                .expect_err("the parent must retain its resource terminal"),
            parent_terminal
        );
    }

    #[test]
    fn child_latches_parent_deadline_when_parent_resource_wins_during_clock_read() {
        let now = Arc::new(AtomicU64::new(0));
        let clock_now = Arc::clone(&now);
        let parent_slot = Arc::new(OnceLock::<OperationControl>::new());
        let clock_parent = Arc::clone(&parent_slot);
        let base = Instant::now();
        let parent_terminal = OperationLedgerError::ArithmeticOverflow {
            id: "parent_work",
            phase: OperationPhase::Layout,
        };
        let clock = Arc::new(move || {
            if let Some(parent) = clock_parent.get() {
                parent.latch_terminal_error(parent_terminal);
            }
            base + Duration::from_millis(clock_now.load(Ordering::Relaxed))
        });
        let parent = OperationControl::with_clock(clock).with_deadline(Duration::from_millis(1));
        assert!(parent_slot.set(parent.clone()).is_ok());
        let child = parent.child();

        now.store(2, Ordering::Relaxed);
        let cancellation = child
            .terminal_checkpoint_at(OperationPhase::Parse)
            .expect_err("the child must observe the expired parent deadline");
        assert_eq!(
            cancellation,
            OperationLedgerError::Cancelled(OperationCancelled {
                phase: OperationPhase::Parse,
                reason: CancelReason::DeadlineExceeded,
            })
        );
        assert_eq!(
            child
                .terminal_checkpoint_at(OperationPhase::Emit)
                .expect_err("the child must replay its cancellation"),
            cancellation
        );
        assert_eq!(
            parent
                .terminal_checkpoint_at(OperationPhase::Emit)
                .expect_err("the parent must retain its resource terminal"),
            parent_terminal
        );
    }

    #[test]
    fn oversized_deadline_clamps_without_panicking_or_expiring_immediately() {
        let control = OperationControl::new().with_deadline(Duration::MAX);
        assert!(control.state.deadline.get().is_some());
        assert_eq!(control.checkpoint(), Ok(()));
    }

    #[test]
    fn concurrent_checkpoints_replay_one_complete_cancellation() {
        const THREADS: usize = 16;
        for _ in 0..32 {
            let control = OperationControl::new();
            control.cancel();
            let barrier = Arc::new(Barrier::new(THREADS));
            let handles = (0..THREADS)
                .map(|index| {
                    let control = control.clone();
                    let barrier = Arc::clone(&barrier);
                    thread::spawn(move || {
                        barrier.wait();
                        let phase = if index % 2 == 0 {
                            OperationPhase::Parse
                        } else {
                            OperationPhase::Layout
                        };
                        control.checkpoint_at(phase)
                    })
                })
                .collect::<Vec<_>>();

            let mut first = None;
            for handle in handles {
                let cancelled = handle
                    .join()
                    .expect("checkpoint thread should not panic")
                    .expect_err("every concurrent observer must see cancellation");
                assert_eq!(cancelled.reason, CancelReason::Requested);
                if let Some(first) = first {
                    assert_eq!(cancelled, first);
                } else {
                    first = Some(cancelled);
                }
            }
        }
    }

    #[test]
    fn operation_replays_the_first_resource_rejection_after_later_cancellation() {
        let control = OperationControl::new();
        let ledger = OperationLedger::new("work", Some(2));
        assert_eq!(ledger.charge(&control, OperationPhase::Layout, 2), Ok(2));
        let first = ledger
            .charge(&control, OperationPhase::Layout, 1)
            .unwrap_err();
        assert_eq!(
            first,
            OperationLedgerError::ResourceLimitExceeded(OperationResourceLimitExceeded {
                id: "work",
                phase: OperationPhase::Layout,
                limit: 2,
                consumed: 2,
                requested: 1,
            })
        );
        assert_eq!(ledger.consumed(), 2);

        let clone = control.clone();
        let child = control.child();
        control.cancel();
        assert_eq!(clone.checkpoint_at(OperationPhase::Emit), Ok(()));
        assert_eq!(
            clone
                .terminal_checkpoint_at(OperationPhase::Emit)
                .expect_err("the complete checkpoint must replay the resource terminal"),
            first
        );
        let cancellation = child
            .checkpoint_at(OperationPhase::Postprocess)
            .expect_err("a child has a distinct terminal scope and observes parent cancellation");
        assert_eq!(cancellation.reason, CancelReason::Requested);
        assert_eq!(cancellation.phase, OperationPhase::Postprocess);
        assert_eq!(
            child
                .terminal_checkpoint_at(OperationPhase::Emit)
                .expect_err("the child's complete checkpoint replays its cancellation"),
            OperationLedgerError::Cancelled(cancellation)
        );
        assert_eq!(
            ledger
                .charge(&control, OperationPhase::Emit, u64::MAX)
                .unwrap_err(),
            first
        );
        assert_eq!(ledger.consumed(), 2);
    }

    #[test]
    fn child_control_starts_a_distinct_ledger_terminal_scope() {
        let parent = OperationControl::new();
        let parent_ledger = OperationLedger::new("parent_work", Some(0));
        assert!(matches!(
            parent_ledger.charge(&parent, OperationPhase::Layout, 1),
            Err(OperationLedgerError::ResourceLimitExceeded(_))
        ));

        let child = parent.child();
        let child_ledger = OperationLedger::new("child_work", None);
        assert_eq!(child_ledger.charge(&child, OperationPhase::Emit, 1), Ok(1));
    }

    #[test]
    fn ledger_replays_cancellation_before_later_resource_failure() {
        let control = OperationControl::new();
        let ledger = OperationLedger::new("work", Some(0));
        control.cancel();

        let first = ledger
            .charge(&control, OperationPhase::Parse, 1)
            .unwrap_err();
        assert_eq!(
            first,
            OperationLedgerError::Cancelled(OperationCancelled {
                phase: OperationPhase::Parse,
                reason: CancelReason::Requested,
            })
        );
        assert_eq!(
            ledger
                .charge(&control, OperationPhase::Emit, u64::MAX)
                .unwrap_err(),
            first
        );
        assert_eq!(ledger.consumed(), 0);
    }

    #[test]
    fn ledgers_share_the_operation_terminal_error() {
        let control = OperationControl::new();
        let first_ledger = OperationLedger::new("layout_work", Some(0));
        let second_ledger = OperationLedger::new("output_bytes", None);

        let first = first_ledger
            .charge(&control, OperationPhase::Layout, 1)
            .unwrap_err();
        assert_eq!(
            second_ledger
                .charge(&control, OperationPhase::Emit, 1)
                .unwrap_err(),
            first
        );
        assert_eq!(second_ledger.consumed(), 0);
    }

    #[test]
    fn ledgers_replay_the_first_arithmetic_overflow() {
        let control = OperationControl::new();
        let first_ledger = OperationLedger::new("layout_work", None);
        let second_ledger = OperationLedger::new("output_bytes", None);
        assert_eq!(
            first_ledger.charge(&control, OperationPhase::Layout, u64::MAX),
            Ok(u64::MAX)
        );

        let first = first_ledger
            .charge(&control, OperationPhase::Layout, 1)
            .unwrap_err();
        assert_eq!(
            first,
            OperationLedgerError::ArithmeticOverflow {
                id: "layout_work",
                phase: OperationPhase::Layout,
            }
        );
        assert_eq!(
            second_ledger
                .charge(&control, OperationPhase::Emit, 1)
                .unwrap_err(),
            first
        );
        assert_eq!(second_ledger.consumed(), 0);
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

    #[test]
    fn concurrent_resource_and_cancellation_observers_replay_one_terminal_error() {
        for _ in 0..32 {
            let control = OperationControl::new();
            let ledger = Arc::new(OperationLedger::new("race_work", Some(0)));
            let barrier = Arc::new(Barrier::new(2));

            let resource = {
                let control = control.clone();
                let ledger = Arc::clone(&ledger);
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    ledger.charge(&control, OperationPhase::Layout, 1)
                })
            };
            let cancellation = {
                let control = control.clone();
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    control.cancel();
                    control.terminal_checkpoint_at(OperationPhase::Emit)
                })
            };

            let resource = resource
                .join()
                .expect("resource thread should not panic")
                .unwrap_err();
            let cancellation = cancellation
                .join()
                .expect("cancellation thread should not panic")
                .unwrap_err();
            assert_eq!(resource, cancellation);
            assert_eq!(
                ledger
                    .charge(&control, OperationPhase::Postprocess, 1)
                    .unwrap_err(),
                resource
            );
            assert_eq!(ledger.consumed(), 0);
        }
    }
}
