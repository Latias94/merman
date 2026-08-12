use crate::mermaid_style::{CssFontSizeContext, CssValueAnalysis, ParsedStyleDeclaration};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum SourceStyleOrigin {
    AssignedClass,
    InlineStyle,
    LabelStyle,
    GeneratedClassCss,
}

impl SourceStyleOrigin {
    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::AssignedClass => "assigned-class",
            Self::InlineStyle => "inline-style",
            Self::LabelStyle => "label-style",
            Self::GeneratedClassCss => "generated-class-css",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum SourceStyleChannel {
    Shape,
    Label,
    Stylesheet,
}

impl SourceStyleChannel {
    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::Shape => "shape",
            Self::Label => "label",
            Self::Stylesheet => "stylesheet",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceStyleProvenance {
    owner_id: String,
    class_id: Option<String>,
    origin: SourceStyleOrigin,
    channel: SourceStyleChannel,
    assignment_ordinal: Option<usize>,
    declaration_ordinal: usize,
}

impl SourceStyleProvenance {
    pub(crate) fn assigned_class(
        owner_id: impl Into<String>,
        class_id: impl Into<String>,
        channel: SourceStyleChannel,
        assignment_ordinal: usize,
        declaration_ordinal: usize,
    ) -> Self {
        Self {
            owner_id: owner_id.into(),
            class_id: Some(class_id.into()),
            origin: SourceStyleOrigin::AssignedClass,
            channel,
            assignment_ordinal: Some(assignment_ordinal),
            declaration_ordinal,
        }
    }

    pub(crate) fn inline(
        owner_id: impl Into<String>,
        channel: SourceStyleChannel,
        declaration_ordinal: usize,
    ) -> Self {
        Self {
            owner_id: owner_id.into(),
            class_id: None,
            origin: SourceStyleOrigin::InlineStyle,
            channel,
            assignment_ordinal: None,
            declaration_ordinal,
        }
    }

    pub(crate) fn generated_class_css(
        class_id: impl Into<String>,
        channel: SourceStyleChannel,
        declaration_ordinal: usize,
    ) -> Self {
        let class_id = class_id.into();
        Self {
            owner_id: class_id.clone(),
            class_id: Some(class_id),
            origin: SourceStyleOrigin::GeneratedClassCss,
            channel,
            assignment_ordinal: None,
            declaration_ordinal,
        }
    }

    pub(crate) fn label_style(
        owner_id: impl Into<String>,
        channel: SourceStyleChannel,
        declaration_ordinal: usize,
    ) -> Self {
        Self {
            owner_id: owner_id.into(),
            class_id: None,
            origin: SourceStyleOrigin::LabelStyle,
            channel,
            assignment_ordinal: None,
            declaration_ordinal,
        }
    }

    pub(crate) fn owner_id(&self) -> &str {
        &self.owner_id
    }

    pub(crate) fn class_id(&self) -> Option<&str> {
        self.class_id.as_deref()
    }

    pub(crate) const fn origin(&self) -> SourceStyleOrigin {
        self.origin
    }

    pub(crate) const fn channel(&self) -> SourceStyleChannel {
        self.channel
    }

    pub(crate) const fn assignment_ordinal(&self) -> Option<usize> {
        self.assignment_ordinal
    }

    pub(crate) const fn declaration_ordinal(&self) -> usize {
        self.declaration_ordinal
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PreparedSourceStyleDeclaration {
    raw: String,
    property: String,
    property_css: String,
    source_value: String,
    value: String,
    analysis: CssValueAnalysis,
}

impl PreparedSourceStyleDeclaration {
    pub(crate) fn parse(raw: &str) -> Option<Self> {
        let parsed = crate::mermaid_style::parse_style_declaration(raw)?;
        Some(Self::from_parsed(raw, parsed))
    }

    fn from_parsed(raw: &str, parsed: ParsedStyleDeclaration<'_>) -> Self {
        Self {
            raw: raw.trim().to_string(),
            property: parsed.property().to_string(),
            property_css: parsed.property_css().to_string(),
            source_value: parsed.source_value().to_string(),
            value: parsed.value().to_string(),
            analysis: parsed.analysis().clone(),
        }
    }

    pub(crate) fn bind(
        self: &Arc<Self>,
        provenance: SourceStyleProvenance,
    ) -> SourceStyleDeclaration {
        SourceStyleDeclaration {
            prepared: Arc::clone(self),
            provenance,
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

    pub(crate) const fn is_single_component_value(&self) -> bool {
        self.analysis.is_single_component()
    }

    pub(crate) fn svg_number_or_px(&self) -> Option<f64> {
        self.analysis.svg_number_or_px()
    }

    pub(crate) fn resolve_font_size_px(&self, context: CssFontSizeContext) -> Option<f64> {
        self.analysis.resolve_font_size_px(context)
    }

    pub(crate) fn property_matches(&self, property: &str) -> bool {
        if self.property.starts_with("--") || property.starts_with("--") {
            self.property == property
        } else {
            self.property.eq_ignore_ascii_case(property)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SourceStyleDeclaration {
    prepared: Arc<PreparedSourceStyleDeclaration>,
    provenance: SourceStyleProvenance,
}

impl SourceStyleDeclaration {
    pub(crate) fn parse(
        raw: &str,
        provenance: SourceStyleProvenance,
    ) -> Result<Self, SourceStyleResidual> {
        let Some(prepared) = PreparedSourceStyleDeclaration::parse(raw) else {
            return Err(SourceStyleResidual::invalid(raw, provenance));
        };
        Ok(Arc::new(prepared).bind(provenance))
    }

    pub(crate) fn raw(&self) -> &str {
        self.prepared.raw()
    }

    pub(crate) fn property(&self) -> &str {
        self.prepared.property()
    }

    pub(crate) fn property_css(&self) -> &str {
        self.prepared.property_css()
    }

    pub(crate) fn value(&self) -> &str {
        self.prepared.value()
    }

    /// Validate a declaration before allowing it to participate in a typed winner.
    ///
    /// Callers should use this gate before updating an emission map or layout value;
    /// an invalid later declaration must not displace an earlier admitted winner.
    pub(crate) fn admit<T>(
        &self,
        validator: impl FnOnce(&PreparedSourceStyleDeclaration) -> Option<T>,
    ) -> Result<T, SourceStyleResidual> {
        validator(&self.prepared).ok_or_else(|| {
            SourceStyleResidual::from_declaration(self, SourceStyleResidualReason::InvalidValue)
        })
    }

    pub(crate) fn is_single_component_value(&self) -> bool {
        self.prepared.is_single_component_value()
    }

    pub(crate) fn svg_number_or_px(&self) -> Option<f64> {
        self.prepared.svg_number_or_px()
    }

    pub(crate) fn resolve_font_size_px(&self, context: CssFontSizeContext) -> Option<f64> {
        self.prepared.resolve_font_size_px(context)
    }

    pub(crate) const fn provenance(&self) -> &SourceStyleProvenance {
        &self.provenance
    }

    pub(crate) fn property_matches(&self, property: &str) -> bool {
        self.prepared.property_matches(property)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceStyleResidualReason {
    InvalidDeclaration,
    UnsupportedProperty,
    InvalidValue,
    UnsupportedSurface,
}

impl SourceStyleResidualReason {
    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::InvalidDeclaration => "invalid-declaration",
            Self::UnsupportedProperty => "unsupported-property",
            Self::InvalidValue => "invalid-value",
            Self::UnsupportedSurface => "unsupported-surface",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceStyleResidual {
    raw: String,
    property: Option<String>,
    provenance: SourceStyleProvenance,
    reason: SourceStyleResidualReason,
}

impl SourceStyleResidual {
    pub(crate) fn invalid(raw: &str, provenance: SourceStyleProvenance) -> Self {
        Self {
            raw: raw.trim().to_string(),
            property: None,
            provenance,
            reason: SourceStyleResidualReason::InvalidDeclaration,
        }
    }

    pub(crate) fn from_declaration(
        declaration: &SourceStyleDeclaration,
        reason: SourceStyleResidualReason,
    ) -> Self {
        Self {
            raw: declaration.raw().to_string(),
            property: Some(declaration.property().to_string()),
            provenance: declaration.provenance.clone(),
            reason,
        }
    }

    pub(crate) fn raw(&self) -> &str {
        &self.raw
    }

    pub(crate) fn property(&self) -> Option<&str> {
        self.property.as_deref()
    }

    pub(crate) const fn provenance(&self) -> &SourceStyleProvenance {
        &self.provenance
    }

    pub(crate) const fn reason(&self) -> SourceStyleResidualReason {
        self.reason
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declaration_normalizes_priority_and_preserves_actionable_provenance() {
        let declaration = SourceStyleDeclaration::parse(
            "FONT-SIZE: 24px !important",
            SourceStyleProvenance::assigned_class(
                "Ready",
                "active",
                SourceStyleChannel::Label,
                2,
                3,
            ),
        )
        .expect("parse source style");

        assert_eq!(declaration.property(), "font-size");
        assert_eq!(declaration.value(), "24px");
        assert!(declaration.is_single_component_value());
        assert_eq!(
            declaration.resolve_font_size_px(CssFontSizeContext::uniform(16.0)),
            Some(24.0)
        );
        assert_eq!(declaration.provenance().owner_id(), "Ready");
        assert_eq!(declaration.provenance().class_id(), Some("active"));
        assert_eq!(declaration.provenance().assignment_ordinal(), Some(2));
        assert_eq!(declaration.provenance().declaration_ordinal(), 3);
    }

    #[test]
    fn custom_property_matching_remains_case_sensitive() {
        let declaration = SourceStyleDeclaration::parse(
            "--BrandColor: #fff",
            SourceStyleProvenance::inline("Ready", SourceStyleChannel::Shape, 0),
        )
        .expect("parse custom property");

        assert!(declaration.property_matches("--BrandColor"));
        assert!(!declaration.property_matches("--brandcolor"));

        let escaped = SourceStyleDeclaration::parse(
            r"--brand\:accent: #fff",
            SourceStyleProvenance::inline("Ready", SourceStyleChannel::Shape, 1),
        )
        .expect("parse escaped custom property");
        assert_eq!(escaped.property(), "--brand:accent");
        assert_eq!(escaped.property_css(), r"--brand\:accent");
        assert!(escaped.property_matches("--brand:accent"));

        let escaped_separator = SourceStyleDeclaration::parse(
            r"--brand\;accent: #fff",
            SourceStyleProvenance::inline("Ready", SourceStyleChannel::Shape, 2),
        )
        .expect("parse escaped custom-property separator");
        assert_eq!(escaped_separator.property(), "--brand;accent");
        assert_eq!(escaped_separator.property_css(), r"--brand\;accent");
    }

    #[test]
    fn invalid_declaration_residual_keeps_owner_and_channel() {
        let residual = SourceStyleDeclaration::parse(
            "fill: !important",
            SourceStyleProvenance::inline("Ready", SourceStyleChannel::Shape, 4),
        )
        .expect_err("empty declaration value should fail");

        assert_eq!(residual.raw(), "fill: !important");
        assert_eq!(residual.property(), None);
        assert_eq!(residual.provenance().owner_id(), "Ready");
        assert_eq!(residual.provenance().channel().id(), "shape");
        assert_eq!(residual.reason().id(), "invalid-declaration");
    }

    #[test]
    fn admission_keeps_invalid_later_declarations_out_of_winner_selection() {
        let owner = SourceStyleProvenance::inline("Ready", SourceStyleChannel::Shape, 0);
        let valid = SourceStyleDeclaration::parse("stroke-width: 20px", owner.clone())
            .expect("valid declaration");
        let invalid = SourceStyleDeclaration::parse("stroke-width: 20px junk", owner)
            .expect("safe declaration with invalid scalar grammar");

        let mut winner = valid
            .admit(|declaration| declaration.svg_number_or_px())
            .expect("valid declaration is admitted");
        if let Ok(value) = invalid.admit(|declaration| declaration.svg_number_or_px()) {
            winner = value;
        }
        assert_eq!(winner, 20.0);
    }

    #[test]
    fn prepared_declaration_rejects_trailing_junk_for_scalar_consumers() {
        let declaration = SourceStyleDeclaration::parse(
            "font-size: 24px trailing",
            SourceStyleProvenance::inline("Ready", SourceStyleChannel::Label, 0),
        )
        .expect("source declaration remains structurally safe");

        assert!(!declaration.is_single_component_value());
        assert_eq!(
            declaration.resolve_font_size_px(CssFontSizeContext::uniform(16.0)),
            None
        );
        let residual = declaration
            .admit(|prepared| prepared.resolve_font_size_px(CssFontSizeContext::uniform(16.0)))
            .expect_err("invalid scalar must produce an admission residual");
        assert_eq!(residual.reason(), SourceStyleResidualReason::InvalidValue);
    }
}
