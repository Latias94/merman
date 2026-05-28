use crate::Result;
use regex::{Captures, Regex};
use std::borrow::Cow;
use std::sync::OnceLock;

use super::css_sanitize::strip_css_deg_units;
use super::util::find_tag_end;
use crate::svg::pipeline::{SvgPostprocessContext, SvgPostprocessor};

#[derive(Debug, Clone, Copy, Default)]
pub struct SanitizeSvgAttributesPostprocessor;

impl SvgPostprocessor for SanitizeSvgAttributesPostprocessor {
    fn name(&self) -> &'static str {
        "sanitize-svg-attributes"
    }

    fn process<'a>(
        &self,
        svg: Cow<'a, str>,
        _ctx: &SvgPostprocessContext<'_>,
    ) -> Result<Cow<'a, str>> {
        Ok(Cow::Owned(sanitize_element_attributes(&svg)))
    }
}

pub(crate) fn sanitize_element_attributes(svg: &str) -> String {
    let mut out = String::with_capacity(svg.len());
    let mut cursor = 0;

    while let Some(rel_start) = svg[cursor..].find('<') {
        let start = cursor + rel_start;
        out.push_str(&svg[cursor..start]);

        let Some(end) = find_tag_end(svg, start) else {
            out.push_str(&svg[start..]);
            return out;
        };

        let tag = &svg[start..=end];
        out.push_str(&sanitize_tag_attributes(tag));
        cursor = end + 1;
    }

    out.push_str(&svg[cursor..]);
    out
}

fn sanitize_tag_attributes(tag: &str) -> Cow<'_, str> {
    if tag.starts_with("</")
        || tag.starts_with("<!--")
        || tag.starts_with("<!")
        || tag.starts_with("<?")
    {
        return Cow::Borrowed(tag);
    }

    static ATTR_RE: OnceLock<Regex> = OnceLock::new();
    let attr_re = ATTR_RE.get_or_init(|| {
        Regex::new(r#"\s+([A-Za-z_:][-A-Za-z0-9_:.]*)\s*=\s*"([^"]*)""#)
            .expect("valid SVG attribute regex")
    });

    let mut changed = false;
    let result = attr_re
        .replace_all(tag, |caps: &Captures<'_>| {
            let full = &caps[0];
            let name = &caps[1];
            let value = &caps[2];

            if should_drop_attribute(name, value) {
                changed = true;
                return String::new();
            }

            if let Some(value) = normalize_px_attribute(name, value) {
                changed = true;
                return format!(r#" {name}="{value}""#);
            }

            if name.eq_ignore_ascii_case("style") {
                let sanitized = sanitize_style_attribute(value);
                if sanitized.trim().is_empty() {
                    changed = true;
                    return String::new();
                }
                if sanitized != value {
                    changed = true;
                    return format!(r#" style="{sanitized}""#);
                }
            }

            full.to_string()
        })
        .into_owned();

    if changed {
        Cow::Owned(result)
    } else {
        Cow::Borrowed(tag)
    }
}

fn should_drop_attribute(name: &str, value: &str) -> bool {
    if name.eq_ignore_ascii_case("style") {
        return false;
    }

    let normalized = name.to_ascii_lowercase();
    let guarded = matches!(
        normalized.as_str(),
        "fill"
            | "stroke"
            | "width"
            | "height"
            | "x"
            | "y"
            | "x1"
            | "x2"
            | "y1"
            | "y2"
            | "r"
            | "cx"
            | "cy"
            | "rx"
            | "ry"
            | "stroke-width"
            | "transform"
            | "d"
            | "points"
    );

    guarded && is_invalid_svg_value(value)
}

fn normalize_px_attribute(name: &str, value: &str) -> Option<String> {
    let normalized = name.to_ascii_lowercase();
    let guarded = matches!(
        normalized.as_str(),
        "width"
            | "height"
            | "x"
            | "y"
            | "x1"
            | "x2"
            | "y1"
            | "y2"
            | "r"
            | "cx"
            | "cy"
            | "rx"
            | "ry"
            | "stroke-width"
    );
    if !guarded {
        return None;
    }

    let trimmed = value.trim();
    let number = trimmed.strip_suffix("px")?.trim();
    if number.parse::<f64>().is_ok_and(f64::is_finite) {
        Some(number.to_string())
    } else {
        None
    }
}

fn sanitize_style_attribute(value: &str) -> String {
    let mut out = Vec::new();

    for decl in value.split(';') {
        let trimmed = decl.trim();
        if trimmed.is_empty() {
            continue;
        }

        let Some((property, raw_value)) = trimmed.split_once(':') else {
            out.push(strip_css_deg_units(trimmed));
            continue;
        };

        let property = property.trim();
        let value = raw_value.trim();
        if value.is_empty() || is_invalid_svg_value(value) {
            continue;
        }
        if property
            .trim()
            .to_ascii_lowercase()
            .starts_with("animation")
        {
            continue;
        }

        out.push(format!("{property}:{}", strip_css_deg_units(value)));
    }

    out.join(";")
}

fn is_invalid_svg_value(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() {
        return true;
    }

    let lower = value.to_ascii_lowercase();
    lower.contains("nan") || lower.contains("undefined") || lower.contains("infinity")
}
