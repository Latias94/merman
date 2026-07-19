use super::builtin::util::{extract_quoted_attr, root_svg_tag};
use super::preset::SvgPipelinePreset;
use crate::environment::{RenderSession, RoutedTextMeasurer, TextMeasurementPhase};
use crate::family::RenderFamilyKind;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SvgPostprocessMetadata {
    family_kind: Option<RenderFamilyKind>,
    diagram_type: Option<String>,
    diagram_title: Option<String>,
    svg_id: Option<String>,
}

impl SvgPostprocessMetadata {
    pub fn new() -> Self {
        Self::default()
    }

    /// Recovers descriptive metadata from the root SVG without granting family capabilities.
    ///
    /// Family-specific passes consume an explicitly supplied [`RenderFamilyKind`], never metadata
    /// inferred from SVG text.
    pub fn from_svg(svg: &str) -> Self {
        let root_tag = root_svg_tag(svg);
        let diagram_type = root_tag
            .and_then(|tag| extract_quoted_attr(tag, "aria-roledescription"))
            .map(ToOwned::to_owned);
        Self {
            diagram_type,
            svg_id: root_tag
                .and_then(|tag| extract_quoted_attr(tag, "id"))
                .map(ToOwned::to_owned),
            ..Self::default()
        }
    }

    /// Supplies the closed family identity required by family-specific built-in passes.
    pub fn with_family_kind(mut self, family_kind: RenderFamilyKind) -> Self {
        self.family_kind = Some(family_kind);
        self
    }

    /// Records descriptive diagram metadata without granting family-specific processing.
    pub fn with_diagram_type(mut self, diagram_type: impl Into<String>) -> Self {
        self.diagram_type = Some(diagram_type.into());
        self
    }

    pub fn with_optional_diagram_type(mut self, diagram_type: Option<impl Into<String>>) -> Self {
        self.diagram_type = diagram_type.map(Into::into);
        self
    }

    pub fn with_diagram_title(mut self, diagram_title: impl Into<String>) -> Self {
        self.diagram_title = Some(diagram_title.into());
        self
    }

    pub fn with_optional_diagram_title(mut self, diagram_title: Option<impl Into<String>>) -> Self {
        self.diagram_title = diagram_title.map(Into::into);
        self
    }

    pub fn with_svg_id(mut self, svg_id: impl Into<String>) -> Self {
        self.svg_id = Some(svg_id.into());
        self
    }

    pub fn with_optional_svg_id(mut self, svg_id: Option<impl Into<String>>) -> Self {
        if let Some(svg_id) = svg_id {
            self.svg_id = Some(svg_id.into());
        }
        self
    }

    pub fn diagram_type(&self) -> Option<&str> {
        self.diagram_type.as_deref()
    }

    pub fn family_kind(&self) -> Option<RenderFamilyKind> {
        self.family_kind
    }

    pub fn diagram_title(&self) -> Option<&str> {
        self.diagram_title.as_deref()
    }

    pub fn svg_id(&self) -> Option<&str> {
        self.svg_id.as_deref()
    }
}

#[derive(Clone, Copy)]
pub struct SvgPostprocessContext<'a> {
    preset: SvgPipelinePreset,
    pass_index: usize,
    pass_name: &'a str,
    metadata: &'a SvgPostprocessMetadata,
    session: &'a RenderSession,
}

impl std::fmt::Debug for SvgPostprocessContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SvgPostprocessContext")
            .field("preset", &self.preset)
            .field("pass_index", &self.pass_index)
            .field("pass_name", &self.pass_name)
            .field("metadata", &self.metadata)
            .finish_non_exhaustive()
    }
}

impl<'a> SvgPostprocessContext<'a> {
    pub(crate) fn new(
        preset: SvgPipelinePreset,
        pass_index: usize,
        pass_name: &'a str,
        metadata: &'a SvgPostprocessMetadata,
        session: &'a RenderSession,
    ) -> Self {
        Self {
            preset,
            pass_index,
            pass_name,
            metadata,
            session,
        }
    }

    pub fn preset(&self) -> SvgPipelinePreset {
        self.preset
    }

    pub fn pass_index(&self) -> usize {
        self.pass_index
    }

    pub fn pass_name(&self) -> &'a str {
        self.pass_name
    }

    pub fn diagram_type(&self) -> Option<&'a str> {
        self.metadata.diagram_type()
    }

    pub fn family_kind(&self) -> Option<RenderFamilyKind> {
        self.metadata.family_kind()
    }

    pub fn diagram_title(&self) -> Option<&'a str> {
        self.metadata.diagram_title()
    }

    pub fn svg_id(&self) -> Option<&'a str> {
        self.metadata.svg_id()
    }

    pub fn text_measurer(&self, phase: TextMeasurementPhase) -> RoutedTextMeasurer<'a> {
        self.session.text_measurer(phase)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_extracts_root_svg_id_and_diagram_type() {
        let metadata = SvgPostprocessMetadata::from_svg(
            r#"<svg xmlns="http://www.w3.org/2000/svg" id="diagram-1" aria-roledescription="quadrantChart"><g/></svg>"#,
        );

        assert_eq!(metadata.svg_id(), Some("diagram-1"));
        assert_eq!(metadata.diagram_type(), Some("quadrantChart"));
        assert_eq!(metadata.family_kind(), None);
    }

    #[test]
    fn metadata_ignores_non_root_ids() {
        let metadata = SvgPostprocessMetadata::from_svg(
            r#"<g id="nested" aria-roledescription="quadrantChart"></g>"#,
        );

        assert_eq!(metadata.svg_id(), None);
        assert_eq!(metadata.diagram_type(), None);
        assert_eq!(metadata.family_kind(), None);
    }

    #[test]
    fn metadata_rejects_comment_spoofs_and_nested_svg_roles() {
        for svg in [
            r#"<!-- <svg id="spoof" aria-roledescription="quadrantChart"> --><g/>"#,
            r#"<g><svg id="nested" aria-roledescription="quadrantChart"/></g>"#,
        ] {
            let metadata = SvgPostprocessMetadata::from_svg(svg);

            assert_eq!(metadata.svg_id(), None, "{svg}");
            assert_eq!(metadata.diagram_type(), None, "{svg}");
            assert_eq!(metadata.family_kind(), None, "{svg}");
        }
    }

    #[test]
    fn metadata_accepts_a_root_svg_after_xml_prolog_and_comment() {
        let metadata = SvgPostprocessMetadata::from_svg(
            r#"<?xml version="1.0"?><!-- generated --><svg id="root" aria-roledescription="quadrantChart"/>"#,
        );

        assert_eq!(metadata.svg_id(), Some("root"));
        assert_eq!(metadata.diagram_type(), Some("quadrantChart"));
        assert_eq!(metadata.family_kind(), None);
    }

    #[test]
    fn diagram_type_string_does_not_forge_typed_family_context() {
        let metadata = SvgPostprocessMetadata::new().with_diagram_type("quadrantChart");

        assert_eq!(metadata.diagram_type(), Some("quadrantChart"));
        assert_eq!(metadata.family_kind(), None);
    }
}
