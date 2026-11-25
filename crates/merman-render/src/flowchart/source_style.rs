//! Owned declarations from the safe Mermaid CSS parser.

use crate::mermaid_style::{CssFontSizeContext, CssValueAnalysis, ParsedStyleDeclaration};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FlowchartStyleDeclaration {
    raw: Arc<str>,
    property: Arc<str>,
    property_css: Arc<str>,
    source_value: Arc<str>,
    value: Arc<str>,
    important: bool,
    analysis: CssValueAnalysis,
}

impl FlowchartStyleDeclaration {
    pub(crate) fn parse(raw: &str) -> Option<Self> {
        let parsed = crate::mermaid_style::parse_style_declaration(raw)?;
        Some(Self::from_parsed(raw, parsed))
    }

    fn from_parsed(raw: &str, parsed: ParsedStyleDeclaration<'_>) -> Self {
        Self {
            raw: Arc::from(raw.trim()),
            property: Arc::from(parsed.property()),
            property_css: Arc::from(parsed.property_css()),
            source_value: Arc::from(parsed.source_value()),
            value: Arc::from(parsed.value()),
            important: parsed.important(),
            analysis: parsed.analysis().clone(),
        }
    }

    pub(crate) fn raw(&self) -> &str {
        &self.raw
    }

    pub(crate) fn property(&self) -> &str {
        &self.property
    }

    /// Returns the validated source spelling suitable for CSS emission.
    pub(crate) fn property_css(&self) -> &str {
        &self.property_css
    }

    /// Returns the source value used by Mermaid-compatible CSS emission.
    pub(crate) fn source_value(&self) -> &str {
        &self.source_value
    }

    pub(crate) fn value(&self) -> &str {
        &self.value
    }

    pub(crate) const fn important(&self) -> bool {
        self.important
    }

    pub(crate) fn svg_number_or_px(&self) -> Option<f64> {
        self.analysis.svg_number_or_px()
    }

    #[cfg(any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow"
    ))]
    pub(crate) fn svg_rect_length_px(&self) -> Result<Option<f64>, ()> {
        self.analysis.svg_rect_length_px(self.value())
    }

    pub(crate) fn resolve_font_size_px(&self, context: CssFontSizeContext) -> Option<f64> {
        self.analysis.resolve_font_size_px(context)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FlowchartSourceFacetStatus {
    Absent,
    Admitted,
    Unverified,
}

impl FlowchartSourceFacetStatus {
    pub(crate) const fn from_parts(declared: bool, admitted: bool) -> Self {
        if admitted {
            Self::Admitted
        } else if declared {
            Self::Unverified
        } else {
            Self::Absent
        }
    }

    pub(crate) const fn is_absent(self) -> bool {
        matches!(self, Self::Absent)
    }
}
