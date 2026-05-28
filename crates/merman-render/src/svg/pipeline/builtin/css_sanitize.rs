use crate::Result;
use regex::{Captures, Regex};
use std::borrow::Cow;
use std::sync::OnceLock;

use super::util::{find_matching_brace, find_tag_end};
use crate::svg::pipeline::{SvgPostprocessContext, SvgPostprocessor};

#[derive(Debug, Clone, Copy, Default)]
pub struct SanitizeCssPostprocessor;

impl SvgPostprocessor for SanitizeCssPostprocessor {
    fn name(&self) -> &'static str {
        "sanitize-css"
    }

    fn process<'a>(
        &self,
        svg: Cow<'a, str>,
        _ctx: &SvgPostprocessContext<'_>,
    ) -> Result<Cow<'a, str>> {
        if !svg.contains("<style") && !svg.contains("style=\"") {
            return Ok(svg);
        }
        Ok(Cow::Owned(sanitize_style_elements(&svg)))
    }
}

pub(crate) fn sanitize_style_elements(svg: &str) -> String {
    let mut out = String::with_capacity(svg.len());
    let mut cursor = 0;

    while let Some(rel_start) = svg[cursor..].find("<style") {
        let start = cursor + rel_start;
        out.push_str(&svg[cursor..start]);

        let Some(open_end) = find_tag_end(svg, start) else {
            out.push_str(&svg[start..]);
            return out;
        };

        let content_start = open_end + 1;
        let Some(rel_close_start) = svg[content_start..].find("</style") else {
            out.push_str(&svg[start..]);
            return out;
        };
        let close_start = content_start + rel_close_start;
        let Some(close_end) = find_tag_end(svg, close_start) else {
            out.push_str(&svg[start..]);
            return out;
        };

        out.push_str(&svg[start..=open_end]);
        out.push_str(&sanitize_css(&svg[content_start..close_start]));
        out.push_str(&svg[close_start..=close_end]);
        cursor = close_end + 1;
    }

    out.push_str(&svg[cursor..]);
    out
}

pub(crate) fn sanitize_css(css: &str) -> String {
    let css = strip_unsupported_css_rules(css);
    let css = strip_animation_declarations(&css);
    strip_css_deg_units(&css)
}

fn strip_unsupported_css_rules(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut cursor = 0;

    while let Some(rel_open) = css[cursor..].find('{') {
        let open = cursor + rel_open;
        let selector = &css[cursor..open];
        let Some(close) = find_matching_brace(css, open) else {
            out.push_str(&css[cursor..]);
            return out;
        };

        let selector_lower = selector.to_ascii_lowercase();
        let unsupported = selector_lower.contains("@keyframes")
            || selector_lower.contains("@-webkit-keyframes")
            || selector_lower.contains(":root");

        if !unsupported {
            out.push_str(&css[cursor..=close]);
        }
        cursor = close + 1;
    }

    out.push_str(&css[cursor..]);
    out
}

fn strip_animation_declarations(css: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?i)(^|[;{])\s*animation(?:-[a-z-]+)?\s*:[^;}]*;?")
            .expect("valid animation declaration regex")
    });

    re.replace_all(css, |caps: &Captures<'_>| caps[1].to_string())
        .into_owned()
}

pub(crate) fn strip_css_deg_units(css: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE
        .get_or_init(|| Regex::new(r"(?i)(-?\d+(?:\.\d+)?)deg\b").expect("valid CSS degree regex"));

    re.replace_all(css, "$1").into_owned()
}
