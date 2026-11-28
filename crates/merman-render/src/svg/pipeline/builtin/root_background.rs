use crate::Result;
use crate::svg::pipeline::{SvgPostprocessContext, SvgPostprocessor};
use merman_render_core::svg_css::set_root_background_color;
use std::borrow::Cow;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootBackgroundPostprocessor {
    background_color: String,
}

impl RootBackgroundPostprocessor {
    pub fn new(background_color: impl Into<String>) -> Self {
        Self {
            background_color: background_color.into(),
        }
    }

    pub fn background_color(&self) -> &str {
        &self.background_color
    }
}

impl SvgPostprocessor for RootBackgroundPostprocessor {
    fn name(&self) -> &'static str {
        "root-background"
    }

    fn process<'a>(
        &self,
        svg: Cow<'a, str>,
        ctx: &SvgPostprocessContext<'_>,
    ) -> Result<Cow<'a, str>> {
        set_root_background_color(svg, &self.background_color, ctx.execution().work_meter())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svg::pipeline::SvgPipeline;

    fn render_session() -> crate::environment::RenderSession {
        crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap()
    }

    #[test]
    fn root_background_rewrites_existing_background_color() {
        let svg =
            r#"<svg id="diagram" style="max-width: 400px; background-color: white;"><g/></svg>"#;
        let session = render_session();

        let out = SvgPipeline::parity()
            .with_postprocessor(RootBackgroundPostprocessor::new("#111827"))
            .process_to_string(svg, &session)
            .unwrap();

        assert_eq!(
            out,
            r#"<svg id="diagram" style="max-width: 400px; background-color: #111827;"><g/></svg>"#
        );
    }

    #[test]
    fn root_background_adds_missing_style_property() {
        let svg = r#"<svg id="diagram" width="100%"><g/></svg>"#;
        let session = render_session();

        let out = SvgPipeline::parity()
            .with_postprocessor(RootBackgroundPostprocessor::new("transparent"))
            .process_to_string(svg, &session)
            .unwrap();

        assert_eq!(
            out,
            r#"<svg id="diagram" width="100%" style="background-color: transparent;"><g/></svg>"#
        );
    }

    #[test]
    fn root_background_obeys_the_serialized_svg_byte_ceiling() {
        use crate::resources::{RenderResourcePolicy, ResourceLimitId};

        let expected = r#"<svg style="background-color: white;"/>"#;
        let pipeline =
            SvgPipeline::parity().with_postprocessor(RootBackgroundPostprocessor::new("white"));
        let session = |maximum| {
            crate::environment::RenderEnvironment::deterministic()
                .with_resource_policy(
                    RenderResourcePolicy::unbounded_for_trusted_input()
                        .with_limit(ResourceLimitId::MaxSvgBytes, maximum)
                        .unwrap(),
                )
                .begin_session()
                .unwrap()
        };
        assert_eq!(
            pipeline
                .process_to_string("<svg/>", &session(expected.len()))
                .unwrap(),
            expected
        );
        let error = pipeline
            .process_to_string("<svg/>", &session(expected.len() - 1))
            .unwrap_err();
        let crate::Error::ResourceLimitExceeded(error) = error else {
            panic!("expected a structured byte limit, got {error:?}");
        };
        assert_eq!(error.limit, "max_svg_bytes");
        assert_eq!(error.actual, expected.len());
        assert_eq!(error.max, expected.len() - 1);
    }
}
