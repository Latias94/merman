//! Shared XML and resource validation for SVG consumers.

mod id_rebase;
mod resource_closure;
mod static_inline;
mod terminal;
mod text_tracking;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

use crate::resources::{
    OperationWorkMeter, RenderResourcePolicy, ResourceLimitExceeded, ResourceLimitPhase,
};
use crate::{Error, Result};
use merman_model::OperationPhase;

pub use id_rebase::SvgIdRebase;
pub use resource_closure::SvgResourceClosure;
pub use terminal::{
    InlineRasterSources, SvgReferencePlan, TerminalSvgValidation, plan_svg_occurrences_for_receipt,
};
pub use text_tracking::SvgFontSeal;

/// Borrows the operation's existing budget and cancellation ledger for postprocess validation.
#[derive(Clone, Copy)]
pub struct SvgValidation<'a> {
    meter: &'a OperationWorkMeter,
}

impl<'a> SvgValidation<'a> {
    pub const fn new(meter: &'a OperationWorkMeter) -> Self {
        Self { meter }
    }

    fn checkpoint(self) -> Result<()> {
        self.meter
            .checkpoint(OperationPhase::Postprocess)
            .map_err(Into::into)
    }

    const fn resource_policy(self) -> &'a RenderResourcePolicy {
        self.meter.policy_ref()
    }

    fn preflight_svg_byte_count(self, actual: usize) -> Result<()> {
        self.meter
            .preflight_svg_byte_count(
                actual,
                ResourceLimitPhase::SvgPostprocess,
                OperationPhase::Postprocess,
            )
            .map_err(Into::into)
    }

    fn preflight_svg_structure(self, elements: usize, tree_depth: usize) -> Result<()> {
        self.meter
            .preflight_svg_structure(elements, tree_depth, OperationPhase::Postprocess)
            .map_err(Into::into)
    }

    fn preflight_svg_element_count(self, elements: usize) -> Result<()> {
        self.meter
            .preflight_svg_element_count(elements, OperationPhase::Postprocess)
            .map_err(Into::into)
    }

    fn terminate_resource_error(self, error: ResourceLimitExceeded) -> Error {
        self.meter
            .terminate_absolute_resource_error(error, OperationPhase::Postprocess)
            .into()
    }
}

fn is_css_value_attribute(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "style"
            | "background"
            | "background-image"
            | "fill"
            | "stroke"
            | "filter"
            | "clip-path"
            | "mask"
            | "marker"
            | "marker-start"
            | "marker-mid"
            | "marker-end"
            | "cursor"
            | "color-profile"
    )
}

fn is_svg_idref_attribute(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "aria-activedescendant"
            | "aria-controls"
            | "aria-describedby"
            | "aria-details"
            | "aria-errormessage"
            | "aria-flowto"
            | "aria-labelledby"
            | "aria-owns"
    )
}
