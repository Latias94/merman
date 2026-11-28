use crate::Result;
use crate::svg::pipeline::{SvgPostprocessContext, SvgPostprocessor};
use merman_render_core::svg_css::strip_css_important;
use std::borrow::Cow;

pub use merman_render_core::svg_css::CssOverridePolicy;

#[derive(Debug, Clone, Copy, Default)]
pub struct CssOverridePostprocessor {
    policy: CssOverridePolicy,
}

impl CssOverridePostprocessor {
    pub fn new(policy: CssOverridePolicy) -> Self {
        Self { policy }
    }

    pub fn strip_existing_important() -> Self {
        Self::new(CssOverridePolicy::StripExistingImportant)
    }

    pub fn policy(&self) -> CssOverridePolicy {
        self.policy
    }
}

impl SvgPostprocessor for CssOverridePostprocessor {
    fn name(&self) -> &'static str {
        "css-override"
    }

    fn process<'a>(
        &self,
        svg: Cow<'a, str>,
        ctx: &SvgPostprocessContext<'_>,
    ) -> Result<Cow<'a, str>> {
        match self.policy {
            CssOverridePolicy::Preserve => Ok(svg),
            CssOverridePolicy::StripExistingImportant => {
                strip_css_important(svg, ctx.execution().work_meter())
            }
        }
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
    fn css_override_strips_important_only_when_requested() {
        let svg = r#"<svg><style>.node{fill:red !important;}</style></svg>"#;
        let session = render_session();

        let preserve = SvgPipeline::parity()
            .with_postprocessor(CssOverridePostprocessor::new(CssOverridePolicy::Preserve))
            .process_to_string(svg, &session)
            .unwrap();
        let strip = SvgPipeline::parity()
            .with_postprocessor(CssOverridePostprocessor::strip_existing_important())
            .process_to_string(svg, &session)
            .unwrap();

        assert!(preserve.contains("!important"));
        assert!(!strip.contains("!important"));
    }
}
