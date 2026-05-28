use crate::Result;
use crate::svg::foreign_object_label_fallback_svg_text;
use std::borrow::Cow;

use super::util::find_tag_end;
use crate::svg::pipeline::{SvgPostprocessContext, SvgPostprocessor};

#[derive(Debug, Clone, Copy, Default)]
pub struct ForeignObjectFallbackPostprocessor;

impl SvgPostprocessor for ForeignObjectFallbackPostprocessor {
    fn name(&self) -> &'static str {
        "foreign-object-fallback"
    }

    fn process<'a>(
        &self,
        svg: Cow<'a, str>,
        _ctx: &SvgPostprocessContext<'_>,
    ) -> Result<Cow<'a, str>> {
        if !svg.contains("<foreignObject") {
            return Ok(svg);
        }
        Ok(Cow::Owned(foreign_object_fallback_svg(&svg)))
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct StripForeignObjectPostprocessor;

impl SvgPostprocessor for StripForeignObjectPostprocessor {
    fn name(&self) -> &'static str {
        "strip-foreign-object"
    }

    fn process<'a>(
        &self,
        svg: Cow<'a, str>,
        _ctx: &SvgPostprocessContext<'_>,
    ) -> Result<Cow<'a, str>> {
        if !svg.contains("<foreignObject") {
            return Ok(svg);
        }
        Ok(Cow::Owned(strip_foreign_objects(&svg)))
    }
}

pub(crate) fn foreign_object_fallback_svg(svg: &str) -> String {
    foreign_object_label_fallback_svg_text(svg)
}

pub(crate) fn strip_foreign_objects(svg: &str) -> String {
    let mut out = String::with_capacity(svg.len());
    let mut cursor = 0;

    while let Some(rel_start) = svg[cursor..].find("<foreignObject") {
        let start = cursor + rel_start;
        out.push_str(&svg[cursor..start]);

        let Some(open_end) = find_tag_end(svg, start) else {
            out.push_str(&svg[start..]);
            return out;
        };

        if svg[start..=open_end].trim_end().ends_with("/>") {
            cursor = open_end + 1;
            continue;
        }

        let close_start = open_end + 1;
        let Some(rel_close) = svg[close_start..].find("</foreignObject>") else {
            cursor = open_end + 1;
            continue;
        };
        cursor = close_start + rel_close + "</foreignObject>".len();
    }

    out.push_str(&svg[cursor..]);
    out
}
