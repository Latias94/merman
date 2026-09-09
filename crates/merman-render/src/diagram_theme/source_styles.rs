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
    owner_id: Arc<str>,
    class_id: Option<Arc<str>>,
    origin: SourceStyleOrigin,
    channel: SourceStyleChannel,
    assignment_ordinal: Option<usize>,
    declaration_ordinal: usize,
}

impl SourceStyleProvenance {
    pub(crate) fn assigned_class(
        owner_id: impl Into<Arc<str>>,
        class_id: impl Into<Arc<str>>,
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
        owner_id: impl Into<Arc<str>>,
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
        class_id: impl Into<Arc<str>>,
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
        owner_id: impl Into<Arc<str>>,
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

    #[cfg(test)]
    pub(crate) fn owner_id(&self) -> &str {
        &self.owner_id
    }

    pub(crate) fn owner_id_arc(&self) -> Arc<str> {
        Arc::clone(&self.owner_id)
    }

    #[cfg(test)]
    pub(crate) fn class_id(&self) -> Option<&str> {
        self.class_id.as_deref()
    }

    pub(crate) fn class_id_arc(&self) -> Option<Arc<str>> {
        self.class_id.as_ref().map(Arc::clone)
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
    raw: Arc<str>,
    property: Arc<str>,
    property_css: Arc<str>,
    source_value: Arc<str>,
    value: Arc<str>,
    important: bool,
    analysis: CssValueAnalysis,
}

impl PreparedSourceStyleDeclaration {
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

    pub(crate) const fn important(&self) -> bool {
        self.important
    }

    pub(crate) fn inherits_property_value(&self) -> bool {
        self.analysis.inherits_property_value()
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
            self.property.as_ref() == property
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

    pub(crate) fn property(&self) -> &str {
        self.prepared.property()
    }

    pub(crate) fn property_css(&self) -> &str {
        self.prepared.property_css()
    }

    pub(crate) fn value(&self) -> &str {
        self.prepared.value()
    }

    #[cfg(test)]
    pub(crate) fn important(&self) -> bool {
        self.prepared.important()
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

    #[cfg(test)]
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
    raw: Arc<str>,
    property: Option<Arc<str>>,
    provenance: SourceStyleProvenance,
    reason: SourceStyleResidualReason,
}

impl SourceStyleResidual {
    pub(crate) fn invalid(raw: &str, provenance: SourceStyleProvenance) -> Self {
        Self::invalid_prepared(Arc::from(raw.trim()), provenance)
    }

    pub(crate) fn invalid_prepared(raw: Arc<str>, provenance: SourceStyleProvenance) -> Self {
        let trimmed = raw.trim();
        let raw = if trimmed.len() == raw.len() {
            raw
        } else {
            Arc::from(trimmed)
        };
        Self {
            raw,
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
            raw: Arc::clone(&declaration.prepared.raw),
            property: Some(Arc::clone(&declaration.prepared.property)),
            provenance: declaration.provenance.clone(),
            reason,
        }
    }

    #[cfg(test)]
    pub(crate) fn raw(&self) -> &str {
        &self.raw
    }

    pub(crate) fn raw_arc(&self) -> Arc<str> {
        Arc::clone(&self.raw)
    }

    #[cfg(test)]
    pub(crate) fn property(&self) -> Option<&str> {
        self.property.as_deref()
    }

    pub(crate) fn property_arc(&self) -> Option<Arc<str>> {
        self.property.as_ref().map(Arc::clone)
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
        assert!(declaration.important());
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
    fn numeric_declaration_requires_a_complete_scalar_value() {
        let owner = SourceStyleProvenance::inline("Ready", SourceStyleChannel::Shape, 0);
        let valid = SourceStyleDeclaration::parse("stroke-width: 20px", owner.clone())
            .expect("valid declaration");
        let invalid = SourceStyleDeclaration::parse("stroke-width: 99px junk", owner)
            .expect("safe declaration with invalid scalar grammar");

        assert_eq!(valid.svg_number_or_px(), Some(20.0));
        assert_eq!(invalid.svg_number_or_px(), None);
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
    }

    #[test]
    fn cloned_source_style_payloads_share_storage() {
        let prepared = Arc::new(
            PreparedSourceStyleDeclaration::parse("fill: red ! IMPORTANT")
                .expect("prepared declaration"),
        );
        let prepared_clone = prepared.as_ref().clone();
        assert!(prepared.important());
        assert!(Arc::ptr_eq(&prepared.raw, &prepared_clone.raw));
        assert!(Arc::ptr_eq(&prepared.property, &prepared_clone.property));
        assert!(Arc::ptr_eq(
            &prepared.property_css,
            &prepared_clone.property_css
        ));
        assert!(Arc::ptr_eq(
            &prepared.source_value,
            &prepared_clone.source_value
        ));
        assert!(Arc::ptr_eq(&prepared.value, &prepared_clone.value));

        let provenance = SourceStyleProvenance::assigned_class(
            "Ready",
            "active",
            SourceStyleChannel::Shape,
            0,
            1,
        );
        let provenance_clone = provenance.clone();
        assert!(Arc::ptr_eq(
            &provenance.owner_id,
            &provenance_clone.owner_id
        ));
        assert!(Arc::ptr_eq(
            provenance.class_id.as_ref().expect("class id"),
            provenance_clone.class_id.as_ref().expect("class id")
        ));

        let declaration = prepared.bind(provenance);
        let residual = SourceStyleResidual::from_declaration(
            &declaration,
            SourceStyleResidualReason::UnsupportedSurface,
        );
        assert_eq!(residual.raw(), "fill: red ! IMPORTANT");
        assert_eq!(residual.property(), Some("fill"));
        assert!(Arc::ptr_eq(&declaration.prepared.raw, &residual.raw));
        assert!(Arc::ptr_eq(
            &declaration.prepared.property,
            residual.property.as_ref().expect("property")
        ));

        let residual_clone = residual.clone();
        assert!(Arc::ptr_eq(&residual.raw, &residual_clone.raw));
        assert!(Arc::ptr_eq(
            residual.property.as_ref().expect("property"),
            residual_clone.property.as_ref().expect("property")
        ));

        let invalid_raw: Arc<str> = Arc::from("filter:url(#unsafe)");
        let invalid = SourceStyleResidual::invalid_prepared(
            Arc::clone(&invalid_raw),
            SourceStyleProvenance::inline("Ready", SourceStyleChannel::Shape, 2),
        );
        assert!(Arc::ptr_eq(&invalid_raw, &invalid.raw));
        assert_eq!(invalid.raw(), "filter:url(#unsafe)");
    }
}
