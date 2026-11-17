//! Operation-scoped state projected into the ASCII backend.
//!
//! The projection deliberately contains only the values the terminal backend needs. Parsing,
//! runtime policy, and cancellation remain caller-owned shared contracts; this crate owns the
//! complete ASCII layout and output resource policy.

use crate::error::Result;
use crate::options::{AsciiGraphPresentation, TerminalWidthProfile};
use crate::output::{AsciiViewportPolicy, OverflowPolicy};
use crate::resource::{AsciiResourcePolicy, ResourceContext, operation_terminal_error};
use merman_model::{OperationControl, OperationPhase};
use std::cell::Cell;
const COOPERATIVE_CHECKPOINT_INTERVAL: usize = 64;

#[derive(Debug)]
pub(crate) struct SearchAllowance {
    expansions: Cell<usize>,
    work: Cell<usize>,
}

impl SearchAllowance {
    const fn new(expansions: usize, work: usize) -> Self {
        Self {
            expansions: Cell::new(expansions),
            work: Cell::new(work),
        }
    }

    pub(crate) fn remaining(&self) -> (usize, usize) {
        (self.expansions.get(), self.work.get())
    }

    pub(crate) fn debit(&self, expansions: usize, work: usize) {
        self.expansions
            .set(self.expansions.get().saturating_sub(expansions));
        self.work.set(self.work.get().saturating_sub(work));
    }

    pub(crate) fn account_scene_work(
        &self,
        start_work: usize,
        initial_allowance: usize,
        resources: &ResourceContext,
    ) {
        let spent = resources.layout_work_used().saturating_sub(start_work);
        let charged = initial_allowance.saturating_sub(self.work.get());
        self.debit(0, spent.saturating_sub(charged));
    }
}

#[derive(Debug)]
pub(crate) struct GraphPlanningBudget {
    pub(crate) feasibility: SearchAllowance,
    pub(crate) allocation: SearchAllowance,
    pub(crate) replacement: SearchAllowance,
    pub(crate) quality: SearchAllowance,
    pub(crate) quality_improvement: SearchAllowance,
    pub(crate) repair_attempts: Cell<usize>,
    pub(crate) reflow_work: Cell<usize>,
}

impl Default for GraphPlanningBudget {
    fn default() -> Self {
        Self {
            feasibility: SearchAllowance::new(usize::MAX, usize::MAX),
            allocation: SearchAllowance::new(4_096, 500_000),
            replacement: SearchAllowance::new(8_192, 1_000_000),
            quality: SearchAllowance::new(8_192, 1_000_000),
            quality_improvement: SearchAllowance::new(4_096, 500_000),
            repair_attempts: Cell::new(4),
            reflow_work: Cell::new(1_000_000),
        }
    }
}

impl GraphPlanningBudget {
    pub(crate) fn for_presentation(presentation: AsciiGraphPresentation) -> Self {
        let mut budget = Self::default();
        if presentation == AsciiGraphPresentation::Adaptive {
            budget.feasibility = SearchAllowance::new(65_536, 3_000_000);
        }
        budget
    }
}

/// Narrow operation projection consumed by the model-to-text backend.
#[derive(Debug, Clone, Copy)]
pub(crate) struct AsciiExecution<'a> {
    control: &'a OperationControl,
    resources: &'a AsciiResourcePolicy,
    viewport: AsciiViewportPolicy,
    optional_layout_candidate: bool,
    trim_canvas_padding: bool,
    render_ledger: Option<&'a ResourceContext>,
    graph_planning: Option<&'a GraphPlanningBudget>,
}

impl<'a> AsciiExecution<'a> {
    /// Creates a projection from the caller-owned operation state.
    pub const fn new(control: &'a OperationControl, resources: &'a AsciiResourcePolicy) -> Self {
        Self {
            control,
            resources,
            viewport: AsciiViewportPolicy::unrestricted(),
            render_ledger: None,
            optional_layout_candidate: false,
            trim_canvas_padding: false,
            graph_planning: None,
        }
    }

    pub const fn with_viewport(mut self, viewport: AsciiViewportPolicy) -> Self {
        self.viewport = viewport;
        self
    }

    pub(crate) const fn max_width(self) -> Option<usize> {
        self.viewport.max_width
    }

    /// A canvas embedded in a reading document must admit its retained rows, rather than a
    /// padded rectangle that the document composer would immediately discard. This is local
    /// materialization policy, independent of the caller's final document trim request.
    #[cfg(any(feature = "diagram-requirement", feature = "diagram-c4"))]
    pub(crate) const fn with_trimmed_canvas(mut self) -> Self {
        self.trim_canvas_padding = true;
        self
    }

    pub(crate) const fn trims_canvas_padding(self) -> bool {
        self.trim_canvas_padding
    }

    pub(crate) const fn with_graph_planning(mut self, budget: &'a GraphPlanningBudget) -> Self {
        if self.graph_planning.is_none() {
            self.graph_planning = Some(budget);
        }
        self
    }

    pub(crate) const fn graph_planning(self) -> Option<&'a GraphPlanningBudget> {
        self.graph_planning
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

    pub(crate) const fn with_optional_layout_candidate(mut self) -> Self {
        self.optional_layout_candidate = true;
        self
    }

    pub(crate) const fn is_optional_layout_candidate(self) -> bool {
        self.optional_layout_candidate
    }

    /// Retains cumulative work across both graph and family transaction boundaries when
    /// selection can continue with another candidate or a complete graph document.
    pub(crate) fn preserves_graph_work_on_error(
        self,
        error: &crate::AsciiError,
        presentation: AsciiGraphPresentation,
    ) -> bool {
        match error {
            crate::AsciiError::PrimaryViewportOverflow { .. } => true,
            crate::AsciiError::RoutingSearchExhausted { .. }
            | crate::AsciiError::PortCapacityExceeded { .. } => {
                presentation == AsciiGraphPresentation::Adaptive
                    || self.is_optional_layout_candidate()
            }
            crate::AsciiError::UnsupportedFeature { .. } => {
                presentation == AsciiGraphPresentation::Strict
                    && self.is_optional_layout_candidate()
            }
            _ => false,
        }
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
