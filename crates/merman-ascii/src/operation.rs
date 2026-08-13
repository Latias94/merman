//! Operation-scoped state projected into the ASCII backend.
//!
//! The projection deliberately contains only the values the terminal backend needs.  Parsing,
//! runtime policy, and cancellation ownership stay in `merman-core`; this crate owns only ASCII
//! layout/output policy and its target-local grid budget.

use crate::error::{AsciiError, Result};
use crate::options::MAX_ASCII_GRID_CELLS_RESOURCE_LIMIT_ID;
use merman_core::{OperationControl, OperationPhase, OperationResourceLimitExceeded};

/// ASCII-specific resource policy projected from the parent operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct AsciiResourcePolicy {
    /// Maximum number of character cells a target-local canvas may allocate.
    ///
    /// `None` is reserved for trusted/unbounded operation profiles.  Presentation options such
    /// as charset, color, and spacing intentionally do not live in this policy.
    pub max_grid_cells: Option<usize>,
}

impl Default for AsciiResourcePolicy {
    fn default() -> Self {
        Self {
            max_grid_cells: Some(250_000),
        }
    }
}

impl AsciiResourcePolicy {
    pub const fn unbounded() -> Self {
        Self {
            max_grid_cells: None,
        }
    }

    pub const fn with_max_grid_cells(max_grid_cells: usize) -> Self {
        Self {
            max_grid_cells: Some(max_grid_cells),
        }
    }

    pub const fn max_grid_cells(self) -> Option<usize> {
        self.max_grid_cells
    }
}

/// Narrow operation projection consumed by the model-to-text backend.
#[derive(Debug, Clone, Copy)]
pub(crate) struct AsciiExecution<'a> {
    control: Option<&'a OperationControl>,
    resources: AsciiResourcePolicy,
}

impl<'a> AsciiExecution<'a> {
    /// Creates a projection from the caller-owned operation state.
    pub const fn new(control: &'a OperationControl, resources: AsciiResourcePolicy) -> Self {
        Self {
            control: Some(control),
            resources,
        }
    }

    /// Creates the execution projection used by the direct typed-model convenience entrypoint.
    pub const fn standalone(resources: AsciiResourcePolicy) -> Self {
        Self {
            control: None,
            resources,
        }
    }

    pub const fn resources(self) -> AsciiResourcePolicy {
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
        let Some(limit) = self.resources.max_grid_cells else {
            return Ok(());
        };
        if actual > limit {
            return Err(AsciiError::ResourceLimitExceeded(
                OperationResourceLimitExceeded {
                    id: MAX_ASCII_GRID_CELLS_RESOURCE_LIMIT_ID,
                    phase: OperationPhase::Layout,
                    limit: limit as u64,
                    consumed: 0,
                    requested: actual as u64,
                },
            ));
        }
        Ok(())
    }
}
