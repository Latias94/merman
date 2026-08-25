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
use unicode_segmentation::UnicodeSegmentation;

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

    pub fn admit_graph_extent(self, width: usize, profile: TerminalWidthProfile) -> Result<()> {
        let Some(max_width) = self.viewport.max_width else {
            return Ok(());
        };
        if width <= max_width || self.viewport.overflow != OverflowPolicy::Error {
            return Ok(());
        }
        match self.viewport.overflow {
            OverflowPolicy::Error => Err(crate::AsciiError::WidthOverflow {
                max_width,
                actual_width: width,
                profile,
            }),
            OverflowPolicy::Fallback | OverflowPolicy::Allow => {
                unreachable!("non-error overflow policies return above")
            }
        }
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

    /// Admits a renderer-owned fallback candidate against the same output dimensions used by
    /// normal finalizers. Fallbacks are plain text today, so encoded bytes equal UTF-8 bytes.
    pub(crate) fn admit_fallback_output(
        self,
        text: &str,
        profile: TerminalWidthProfile,
    ) -> Result<()> {
        let mut document_cells = 0usize;
        let mut max_grapheme_bytes = 0usize;
        for line in text.split('\n') {
            document_cells = document_cells
                .checked_add(crate::text::display_width_with_profile(line, profile))
                .ok_or_else(|| {
                    self.new_resource_context(OperationPhase::Emit)
                        .overflow(crate::resource::AsciiResourceLimitId::MaxDocumentCells)
                })?;
            for grapheme in line.graphemes(true) {
                max_grapheme_bytes = max_grapheme_bytes.max(grapheme.len());
            }
        }
        let resources = self.new_resource_context(OperationPhase::Emit);
        resources.charge_document_cells(document_cells)?;
        resources.check(
            crate::resource::AsciiResourceLimitId::MaxOutputBytes,
            text.len(),
        )?;
        resources.check(
            crate::resource::AsciiResourceLimitId::MaxGraphemeBytes,
            max_grapheme_bytes,
        )?;
        self.checkpoint(OperationPhase::Emit)
    }

    /// Checks caller-owned cancellation at a bounded cadence inside deterministic long loops.
    pub fn checkpoint_loop(self, phase: OperationPhase, iteration: usize) -> Result<()> {
        if iteration.is_multiple_of(COOPERATIVE_CHECKPOINT_INTERVAL) {
            self.checkpoint(phase)?;
        }
        Ok(())
    }
}
