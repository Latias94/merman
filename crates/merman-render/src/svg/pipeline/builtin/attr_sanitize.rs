use crate::Result;
use crate::svg::pipeline::{SvgPostprocessContext, SvgPostprocessor};
use merman_render_core::svg_safety::sanitize_element_attributes_cow_with_checkpoints;
use std::borrow::Cow;

#[derive(Debug, Clone, Copy, Default)]
pub struct SanitizeSvgAttributesPostprocessor;

impl SvgPostprocessor for SanitizeSvgAttributesPostprocessor {
    fn name(&self) -> &'static str {
        "sanitize-svg-attributes"
    }

    fn process<'a>(
        &self,
        svg: Cow<'a, str>,
        ctx: &SvgPostprocessContext<'_>,
    ) -> Result<Cow<'a, str>> {
        sanitize_element_attributes_cow_with_checkpoints(svg, &mut || ctx.checkpoint())
    }
}
