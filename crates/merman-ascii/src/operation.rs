//! Operation-scoped state projected into the ASCII backend.
//!
//! The projection deliberately contains only the values the terminal backend needs. Parsing,
//! runtime policy, and cancellation ownership stay in `merman-core`; this crate owns the complete
//! ASCII layout and output resource policy.

use crate::error::{AsciiError, Result};
use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy, ResourceContext};
use merman_core::{OperationControl, OperationPhase};

const COOPERATIVE_CHECKPOINT_INTERVAL: usize = 64;

/// Narrow operation projection consumed by the model-to-text backend.
#[derive(Debug, Clone, Copy)]
pub(crate) struct AsciiExecution<'a> {
    control: Option<&'a OperationControl>,
    resources: &'a AsciiResourcePolicy,
}

impl<'a> AsciiExecution<'a> {
    /// Creates a projection from the caller-owned operation state.
    pub const fn new(control: &'a OperationControl, resources: &'a AsciiResourcePolicy) -> Self {
        Self {
            control: Some(control),
            resources,
        }
    }

    /// Creates an uncontrolled execution projection for crate-local unit tests.
    #[cfg(test)]
    pub const fn standalone(resources: &'a AsciiResourcePolicy) -> Self {
        Self {
            control: None,
            resources,
        }
    }

    pub const fn resources(self) -> &'a AsciiResourcePolicy {
        self.resources
    }

    pub(crate) fn cloned_control(self) -> Option<OperationControl> {
        self.control.cloned()
    }

    /// Creates a ledger-sharing resource view for one operation phase.
    pub(crate) fn resource_context(
        self,
        resources: &ResourceContext,
        phase: OperationPhase,
    ) -> ResourceContext {
        debug_assert_eq!(resources.policy(), *self.resources);
        match self.cloned_control() {
            Some(control) => resources.controlled(control, phase),
            None => resources.clone(),
        }
    }

    pub fn checkpoint(self, phase: OperationPhase) -> Result<()> {
        match self.control {
            Some(control) => control.checkpoint_at(phase).map_err(AsciiError::Cancelled),
            None => Ok(()),
        }
    }

    /// Checks caller-owned cancellation at a bounded cadence inside deterministic long loops.
    pub fn checkpoint_loop(self, phase: OperationPhase, iteration: usize) -> Result<()> {
        if iteration.is_multiple_of(COOPERATIVE_CHECKPOINT_INTERVAL) {
            self.checkpoint(phase)?;
        }
        Ok(())
    }

    /// Checks and admits a target-local canvas allocation before it is materialized.
    pub fn admit_grid(self, actual: usize) -> Result<()> {
        self.checkpoint(OperationPhase::Layout)?;
        self.resources
            .check(AsciiResourceLimitId::MaxGridCells, actual)
    }
}
