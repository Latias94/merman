use crate::Result;
use crate::svg::pipeline::{SvgPostprocessContext, SvgPostprocessExecution, SvgPostprocessor};
use merman_render_core::svg_safety::sanitize_style_elements;
use std::borrow::Cow;

#[derive(Debug, Clone, Copy, Default)]
pub struct SanitizeCssPostprocessor;

impl SvgPostprocessor for SanitizeCssPostprocessor {
    fn name(&self) -> &'static str {
        "sanitize-css"
    }

    fn process<'a>(
        &self,
        svg: Cow<'a, str>,
        ctx: &SvgPostprocessContext<'_>,
    ) -> Result<Cow<'a, str>> {
        apply_sanitize_style_elements(svg, ctx.execution())
    }
}

pub(crate) fn apply_sanitize_style_elements<'a>(
    svg: Cow<'a, str>,
    execution: SvgPostprocessExecution<'_>,
) -> Result<Cow<'a, str>> {
    sanitize_style_elements(svg, &mut || execution.checkpoint(), &mut |projected| {
        execution.preflight_svg_byte_count(projected)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_element_sanitizer_checks_real_xml_expansion_against_svg_byte_policy() {
        use crate::resources::{RenderResourcePolicy, ResourceLimitId};
        let svg = format!(
            r#"<svg><style><![CDATA[#root{{font-family:"{}",monospace;}}]]></style></svg>"#,
            "&".repeat(256)
        );
        let environment = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxSvgBytes, svg.len())
                    .unwrap(),
            );
        let session = environment.begin_session().unwrap();
        let error = apply_sanitize_style_elements(
            Cow::Borrowed(&svg),
            SvgPostprocessExecution::new(&session),
        )
        .expect_err("encoded CSS must fit the same operation output ceiling");
        let crate::Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected byte ceiling");
        };
        assert_eq!(limit.limit, ResourceLimitId::MaxSvgBytes.as_str());
        assert!(limit.actual > limit.max);
    }
}
