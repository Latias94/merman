//! Operation-scoped state projected into the ASCII backend.
//!
//! The projection deliberately contains only the values the terminal backend needs. Parsing,
//! runtime policy, and cancellation ownership stay in `merman-core`; this crate owns the complete
//! ASCII layout and output resource policy.

use crate::error::{AsciiError, Result};
use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
use merman_core::{OperationControl, OperationPhase};

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

    /// Creates the execution projection used by the direct typed-model convenience entrypoint.
    pub const fn standalone(resources: &'a AsciiResourcePolicy) -> Self {
        Self {
            control: None,
            resources,
        }
    }

    pub const fn resources(self) -> &'a AsciiResourcePolicy {
        self.resources
    }

    pub fn checkpoint(self, phase: OperationPhase) -> Result<()> {
        match self.control {
            Some(control) => control.checkpoint_at(phase).map_err(AsciiError::Cancelled),
            None => Ok(()),
        }
    }

    /// Checks and admits a target-local canvas allocation before it is materialized.
    pub fn admit_grid(self, actual: usize) -> Result<()> {
        self.checkpoint(OperationPhase::Layout)?;
        self.resources
            .check(AsciiResourceLimitId::MaxGridCells, actual)
    }
}
