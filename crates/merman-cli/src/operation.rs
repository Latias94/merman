use crate::error::CliError;
use crate::invocation::ResolvedInvocation;
use merman::{OperationControl, OperationPhase};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

static NEXT_OPERATION_ID: AtomicU64 = AtomicU64::new(1);
static ACTIVE_OPERATIONS: OnceLock<Mutex<OperationRegistry>> = OnceLock::new();
static SIGINT_HANDLER: OnceLock<Result<(), String>> = OnceLock::new();
const SIGINT_EXIT_CODE: i32 = 130;

/// Owns one CLI render operation from local preflight through publication.
pub(crate) struct HostOperation {
    control: OperationControl,
    _signal_registration: SignalRegistration,
}

impl HostOperation {
    pub(crate) fn begin_for(invocation: &ResolvedInvocation) -> Result<Option<Self>, CliError> {
        let timeout = match invocation {
            ResolvedInvocation::Render(args) => Some(args.common.operation_timeout),
            #[cfg(feature = "markdown")]
            ResolvedInvocation::Batch(args) => Some(args.common.operation_timeout),
            #[cfg(feature = "svg")]
            ResolvedInvocation::Mmdc(args) => Some(args.common.operation_timeout),
            _ => None,
        };
        timeout.map(Self::begin).transpose()
    }

    pub(crate) fn begin(timeout: Option<Duration>) -> Result<Self, CliError> {
        let control = timeout.map_or_else(OperationControl::new, |timeout| {
            OperationControl::new().with_deadline(timeout)
        });
        let signal_registration = SignalRegistration::register(control.clone());
        ensure_sigint_handler()?;
        let operation = Self {
            _signal_registration: signal_registration,
            control,
        };
        operation.checkpoint(OperationPhase::Admission)?;
        Ok(operation)
    }

    pub(crate) fn control(&self) -> &OperationControl {
        &self.control
    }

    pub(crate) fn checkpoint(&self, phase: OperationPhase) -> Result<(), CliError> {
        checkpoint(&self.control, phase)
    }
}

pub(crate) fn checkpoint(
    control: &OperationControl,
    phase: OperationPhase,
) -> Result<(), CliError> {
    control
        .checkpoint_at(phase)
        .map_err(|error| CliError::Render(merman::RenderError::Cancelled(error)))
}

fn ensure_sigint_handler() -> Result<(), CliError> {
    SIGINT_HANDLER
        .get_or_init(|| {
            ctrlc::set_handler(cancel_active_operations)
                .map_err(|error| format!("failed to install cooperative SIGINT handler: {error}"))
        })
        .as_ref()
        .map_err(|message| CliError::Io(std::io::Error::other(message.clone())))
        .copied()
}

fn active_operations() -> &'static Mutex<OperationRegistry> {
    ACTIVE_OPERATIONS.get_or_init(|| Mutex::new(OperationRegistry::default()))
}

fn cancel_active_operations() {
    let action = active_operations()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .interrupt();
    if action == InterruptAction::Terminate {
        std::process::exit(SIGINT_EXIT_CODE);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InterruptAction {
    Cancel,
    Terminate,
}

#[derive(Default)]
struct OperationRegistry {
    controls: BTreeMap<u64, OperationControl>,
}

impl OperationRegistry {
    fn insert(&mut self, id: u64, control: OperationControl) {
        let previous = self.controls.insert(id, control);
        debug_assert!(previous.is_none(), "operation registration id collision");
    }

    fn remove(&mut self, id: u64) {
        let removed = self.controls.remove(&id);
        debug_assert!(
            removed.is_some(),
            "operation registration was already removed"
        );
    }

    fn interrupt(&self) -> InterruptAction {
        if self.controls.is_empty() || self.controls.values().all(OperationControl::is_cancelled) {
            return InterruptAction::Terminate;
        }
        for control in self.controls.values() {
            control.cancel();
        }
        InterruptAction::Cancel
    }
}

struct SignalRegistration {
    id: u64,
}

impl SignalRegistration {
    fn register(control: OperationControl) -> Self {
        let id = NEXT_OPERATION_ID.fetch_add(1, Ordering::Relaxed);
        active_operations()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(id, control);
        Self { id }
    }
}

impl Drop for SignalRegistration {
    fn drop(&mut self) {
        active_operations()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(self.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_cancels_every_active_operation() {
        let first = OperationControl::new();
        let second = OperationControl::new();
        let mut registry = OperationRegistry::default();
        registry.insert(1, first.clone());
        registry.insert(2, second.clone());

        assert_eq!(registry.interrupt(), InterruptAction::Cancel);

        assert!(first.is_cancelled());
        assert!(second.is_cancelled());
        assert_eq!(registry.interrupt(), InterruptAction::Terminate);
        registry.remove(1);
        registry.remove(2);
        assert!(registry.controls.is_empty());
        assert_eq!(registry.interrupt(), InterruptAction::Terminate);
    }

    #[test]
    fn zero_timeout_is_reported_as_a_structured_deadline() {
        let control = OperationControl::new().with_deadline(Duration::ZERO);
        let error = checkpoint(&control, OperationPhase::Admission)
            .expect_err("zero timeout must expire at admission");

        assert!(matches!(
            error,
            CliError::Render(merman::RenderError::Cancelled(merman::OperationCancelled {
                phase: OperationPhase::Admission,
                reason: merman::CancelReason::DeadlineExceeded,
            }))
        ));
    }
}
