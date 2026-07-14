use super::builtin::util::extract_root_svg_id;
use super::preset::SvgPipelinePreset;
use crate::environment::{RenderSession, RoutedTextMeasurer, TextMeasurementPhase};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SvgPostprocessMetadata {
    diagram_type: Option<String>,
    diagram_title: Option<String>,
    svg_id: Option<String>,
}

impl SvgPostprocessMetadata {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_svg(svg: &str) -> Self {
        Self {
            svg_id: extract_root_svg_id(svg),
            ..Self::default()
        }
    }

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
    fn metadata_extracts_root_svg_id() {
        let metadata = SvgPostprocessMetadata::from_svg(
            r#"<svg xmlns="http://www.w3.org/2000/svg" id="diagram-1"><g/></svg>"#,
        );

        assert_eq!(metadata.svg_id(), Some("diagram-1"));
    }

    #[test]
    fn metadata_ignores_non_root_ids() {
        let metadata = SvgPostprocessMetadata::from_svg(r#"<g id="nested"></g>"#);

        assert_eq!(metadata.svg_id(), None);
    }
}
