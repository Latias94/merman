use crate::Result;
use crate::svg::pipeline::{SvgPostprocessContext, SvgPostprocessor};
use merman_render_core::svg_validation::SvgIdRebase;
use std::borrow::Cow;

#[derive(Debug, Clone)]
pub(crate) struct RebaseSvgIdsPostprocessor(SvgIdRebase);

impl RebaseSvgIdsPostprocessor {
    pub(crate) fn new(prefix: impl Into<String>) -> Self {
        Self(SvgIdRebase::new(&prefix.into()))
    }
}

impl SvgPostprocessor for RebaseSvgIdsPostprocessor {
    fn name(&self) -> &'static str {
        "rebase-svg-ids"
    }

    fn process<'a>(
        &self,
        svg: Cow<'a, str>,
        ctx: &SvgPostprocessContext<'_>,
    ) -> Result<Cow<'a, str>> {
        self.0.apply(svg, ctx.execution().validation())
    }
}

#[cfg(test)]
mod tests;
