//! Operation-scoped state projected into the ASCII backend.
//!
//! The projection deliberately contains only the values the terminal backend needs. Parsing,
//! runtime policy, and cancellation ownership stay in `merman-core`; this crate owns the complete
//! ASCII layout and output resource policy.

use crate::error::Result;
use crate::options::TerminalWidthProfile;
use crate::output::{AsciiViewportPolicy, OverflowPolicy};
use crate::resource::{AsciiResourcePolicy, ResourceContext, operation_terminal_error};
use merman_core::{OperationControl, OperationPhase};
const COOPERATIVE_CHECKPOINT_INTERVAL: usize = 64;

/// Narrow operation projection consumed by the model-to-text backend.
#[derive(Debug, Clone, Copy)]
pub(crate) struct AsciiExecution<'a> {
    control: &'a OperationControl,
    resources: &'a AsciiResourcePolicy,
    viewport: AsciiViewportPolicy,
    render_ledger: Option<&'a ResourceContext>,
}

impl<'a> AsciiExecution<'a> {
    /// Creates a projection from the caller-owned operation state.
    pub const fn new(control: &'a OperationControl, resources: &'a AsciiResourcePolicy) -> Self {
        Self {
            control,
            resources,
            viewport: AsciiViewportPolicy::unrestricted(),
            render_ledger: None,
        }
    }

    pub const fn with_viewport(mut self, viewport: AsciiViewportPolicy) -> Self {
        self.viewport = viewport;
        self
    }

    /// Binds the execution to the render-wide ledger owned by the top-level request.
    ///
    /// Standalone family tests keep the historical disposable ledger behavior, while a complete
    /// source-to-output request shares cumulative layout/document admissions with any fallback
    /// attempt made after the primary projection.
    pub(crate) const fn with_render_ledger(mut self, ledger: &'a ResourceContext) -> Self {
        self.render_ledger = Some(ledger);
        self
    }

    pub(crate) fn admit_primary_extent(
        self,
        width: usize,
        height: usize,
        profile: TerminalWidthProfile,
    ) -> Result<()> {
        let Some(max_width) = self.viewport.max_width else {
            return Ok(());
        };
        if width <= max_width {
            return Ok(());
        }
        match self.viewport.overflow {
            OverflowPolicy::Error => Err(crate::AsciiError::WidthOverflow {
                max_width,
                actual_width: width,
                profile,
            }),
            OverflowPolicy::Fallback => Err(crate::AsciiError::PrimaryViewportOverflow {
                max_width,
                actual_width: width,
                height,
                profile,
            }),
            OverflowPolicy::Allow => Ok(()),
        }
    }

    pub(crate) fn admit_graph_extent(
        self,
        width: usize,
        height: usize,
        profile: TerminalWidthProfile,
    ) -> Result<()> {
        self.admit_primary_extent(width, height, profile)
    }

    /// Creates a real, never-cancelled execution projection for crate-local unit tests.
    #[cfg(test)]
    pub fn for_test(resources: &'a AsciiResourcePolicy) -> Self {
        // The leaked handle is bounded to the test process and keeps terminal state isolated
        // between independently scheduled unit tests.
        let control = Box::leak(Box::new(OperationControl::new()));
        Self::new(control, resources)
    }

    pub const fn resources(self) -> &'a AsciiResourcePolicy {
        self.resources
    }

    /// Creates a new render-wide resource ledger bound to one operation phase.
    pub(crate) fn new_resource_context(self, phase: OperationPhase) -> ResourceContext {
        if let Some(ledger) = self.render_ledger {
            return self.resource_context(ledger, phase);
        }
        let resources = ResourceContext::new(*self.resources);
        self.resource_context(&resources, phase)
    }

    /// Creates a candidate-local resource view that preserves policy and cancellation without
    /// mutating the render-wide ledger. Final admission commits the measured candidate once.
    pub(crate) fn detached_resource_context(self, phase: OperationPhase) -> ResourceContext {
        let resources = self.render_ledger.map_or_else(
            || ResourceContext::new(*self.resources),
            ResourceContext::detached,
        );
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
            .terminal_checkpoint_at(phase)
            .map_err(operation_terminal_error)
    }

    /// Checks caller-owned cancellation at a bounded cadence inside deterministic long loops.
    pub fn checkpoint_loop(self, phase: OperationPhase, iteration: usize) -> Result<()> {
        if iteration.is_multiple_of(COOPERATIVE_CHECKPOINT_INTERVAL) {
            self.checkpoint(phase)?;
        }
        Ok(())
    }
}
