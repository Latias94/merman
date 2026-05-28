use crate::Result;
use regex::Regex;
use std::borrow::Cow;
use std::sync::OnceLock;

use crate::svg::pipeline::{SvgPostprocessContext, SvgPostprocessor};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CssOverridePolicy {
    #[default]
    Preserve,
    StripExistingImportant,
}

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
        _ctx: &SvgPostprocessContext<'_>,
    ) -> Result<Cow<'a, str>> {
        match self.policy {
            CssOverridePolicy::Preserve => Ok(svg),
            CssOverridePolicy::StripExistingImportant => {
                Ok(Cow::Owned(strip_css_important(svg.as_ref())))
            }
        }
    }
}

pub(crate) fn strip_css_important(svg: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"(?i)\s*!important\b").expect("valid important regex"));
    re.replace_all(svg, "").into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svg::pipeline::SvgPipeline;

    #[test]
    fn css_override_strips_important_only_when_requested() {
        let svg = r#"<svg><style>.node{fill:red !important;}</style></svg>"#;

        let preserve = SvgPipeline::parity()
            .with_postprocessor(CssOverridePostprocessor::new(CssOverridePolicy::Preserve))
            .process_to_string(svg)
            .unwrap();
        let strip = SvgPipeline::parity()
            .with_postprocessor(CssOverridePostprocessor::strip_existing_important())
            .process_to_string(svg)
            .unwrap();

        assert!(preserve.contains("!important"));
        assert!(!strip.contains("!important"));
    }
}
