//! Operation-scoped state projected into the ASCII backend.
//!
//! The projection deliberately contains only the values the terminal backend needs. Parsing,
//! runtime policy, and cancellation ownership stay in `merman-core`; this crate owns the complete
//! ASCII layout and output resource policy.

use crate::error::{AsciiError, Result};
use crate::resource::{AsciiResourcePolicy, ResourceContext};
use merman_core::{OperationControl, OperationPhase};

const COOPERATIVE_CHECKPOINT_INTERVAL: usize = 64;

/// Narrow operation projection consumed by the model-to-text backend.
#[derive(Debug, Clone, Copy)]
pub(crate) struct AsciiExecution<'a> {
    control: &'a OperationControl,
    resources: &'a AsciiResourcePolicy,
}

impl<'a> AsciiExecution<'a> {
    /// Creates a projection from the caller-owned operation state.
    pub const fn new(control: &'a OperationControl, resources: &'a AsciiResourcePolicy) -> Self {
        Self { control, resources }
    }

    /// Creates a real, never-cancelled execution projection for crate-local unit tests.
    #[cfg(test)]
    pub fn for_test(resources: &'a AsciiResourcePolicy) -> Self {
        static CONTROL: std::sync::OnceLock<OperationControl> = std::sync::OnceLock::new();
        Self::new(CONTROL.get_or_init(OperationControl::new), resources)
    }

    pub const fn resources(self) -> &'a AsciiResourcePolicy {
        self.resources
    }

    /// Creates a new render-wide resource ledger bound to one operation phase.
    pub(crate) fn new_resource_context(self, phase: OperationPhase) -> ResourceContext {
        let resources = ResourceContext::new(*self.resources);
        self.resource_context(&resources, phase)
    }

    pub(crate) fn cloned_control(self) -> OperationControl {
        self.control.clone()
    }

    /// Creates a ledger-sharing resource view for one operation phase.
    pub(crate) fn resource_context(
        self,
        resources: &ResourceContext,
        phase: OperationPhase,
    ) -> ResourceContext {
        debug_assert_eq!(resources.policy(), *self.resources);
        resources.controlled(self.cloned_control(), phase)
    }

    /// Rebinds one shared resource ledger before entering a different operation phase.
    pub(crate) fn rebind_resource_context(
        self,
        resources: &mut ResourceContext,
        phase: OperationPhase,
    ) {
        let rebound = self.resource_context(resources, phase);
        *resources = rebound;
    }

    pub fn checkpoint(self, phase: OperationPhase) -> Result<()> {
        self.control
            .checkpoint_at(phase)
            .map_err(AsciiError::Cancelled)
    }

    /// Checks caller-owned cancellation at a bounded cadence inside deterministic long loops.
    pub fn checkpoint_loop(self, phase: OperationPhase, iteration: usize) -> Result<()> {
        if iteration.is_multiple_of(COOPERATIVE_CHECKPOINT_INTERVAL) {
            self.checkpoint(phase)?;
        }
        Ok(())
    }
}
