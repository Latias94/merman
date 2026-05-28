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
        if is_bad_rect_tag(tag) {
            cursor = if tag.trim_end().ends_with("/>") {
                end + 1
            } else {
                svg[end + 1..]
                    .find("</rect>")
                    .map(|rel_close| end + 1 + rel_close + "</rect>".len())
                    .unwrap_or(end + 1)
            };
            continue;
        }
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

fn is_start_or_empty_tag(tag: &str, expected: &str) -> bool {
    let tag = tag.trim_start();
    if !tag.starts_with('<') || tag.starts_with("</") || tag.starts_with("<!--") {
        return false;
    }

    let name = tag[1..]
        .chars()
        .take_while(|ch| !ch.is_whitespace() && *ch != '/' && *ch != '>')
        .collect::<String>();
    name.eq_ignore_ascii_case(expected)
}

fn attr_value(tag: &str, name: &str) -> Option<String> {
    static ATTR_RE: OnceLock<Regex> = OnceLock::new();
    let attr_re = ATTR_RE.get_or_init(|| {
        Regex::new(r#"\s+([A-Za-z_:][-A-Za-z0-9_:.]*)\s*=\s*"([^"]*)""#)
            .expect("valid SVG attribute regex")
    });

    for caps in attr_re.captures_iter(tag) {
        if caps[1].eq_ignore_ascii_case(name) {
            return Some(caps[2].to_string());
        }
    }
    None
}

fn is_missing_or_invalid_rect_dimension(value: Option<&str>) -> bool {
    let Some(value) = value.map(str::trim) else {
        return true;
    };
    if value.is_empty() {
        return true;
    }
    if let Ok(n) = value.parse::<f64>() {
        return !n.is_finite() || n <= 0.0;
    }
    false
}

fn is_bad_rect_tag(tag: &str) -> bool {
    if !is_start_or_empty_tag(tag, "rect") {
        return false;
    }

    let width = attr_value(tag, "width");
    let height = attr_value(tag, "height");
    is_missing_or_invalid_rect_dimension(width.as_deref())
        || is_missing_or_invalid_rect_dimension(height.as_deref())
}

fn sanitize_style_attribute(value: &str) -> String {
    let mut out = Vec::new();

    for decl in value.split(';') {
        let trimmed = decl.trim();
        if trimmed.is_empty() {
            continue;
        }

        let Some((property, raw_value)) = trimmed.split_once(':') else {
            if is_invalid_svg_value(trimmed) {
                continue;
            }
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

#[cfg(test)]
mod tests {
    use super::sanitize_element_attributes;

    #[test]
    fn sanitize_style_attribute_drops_invalid_bare_declarations() {
        let svg = r#"<svg><path style="undefined; stroke: #333; undefined"/></svg>"#;
        let out = sanitize_element_attributes(svg);

        assert!(!out.contains("undefined"), "got: {out}");
        assert!(out.contains(r#"style="stroke:#333""#), "got: {out}");
    }

    #[test]
    fn sanitize_element_attributes_drops_rects_without_positive_dimensions() {
        let svg = r#"<svg><rect/><rect width="0" height="10"/><rect width="12" height="8"/><g><rect width="NaN" height="10"><title>bad</title></rect></g></svg>"#;
        let out = sanitize_element_attributes(svg);

        assert!(!out.contains("<rect/>"), "got: {out}");
        assert!(!out.contains(r#"width="0""#), "got: {out}");
        assert!(!out.contains("NaN"), "got: {out}");
        assert!(!out.contains("<title>bad</title>"), "got: {out}");
        assert!(
            out.contains(r#"<rect width="12" height="8"/>"#),
            "got: {out}"
        );
    }
}
