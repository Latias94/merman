use crate::Result;
use std::borrow::Cow;

use super::css_sanitize::strip_css_deg_units;
use super::util::{SvgTagScanner, next_svg_quoted_attr};
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
    let mut scanner = SvgTagScanner::new(svg);
    let mut copied_until = 0;

    while let Some(tag) = scanner.next() {
        out.push_str(&svg[copied_until..tag.start()]);

        let raw_tag = tag.raw();
        if is_bad_rect_tag(raw_tag) {
            copied_until = if tag.is_self_closing() {
                scanner.cursor()
            } else {
                svg[scanner.cursor()..]
                    .find("</rect>")
                    .map(|rel_close| scanner.cursor() + rel_close + "</rect>".len())
                    .unwrap_or(scanner.cursor())
            };
            scanner.skip_to(copied_until);
            continue;
        }
        out.push_str(&sanitize_tag_attributes(raw_tag));
        copied_until = scanner.cursor();
    }

    out.push_str(&svg[copied_until..]);
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

    let mut changed = false;
    let mut out = String::new();
    let mut copied_until = 0usize;
    let mut cursor = 0usize;

    while let Some(attr) = next_svg_quoted_attr(tag, cursor) {
        let name = &tag[attr.name_start..attr.name_end];
        let value = &tag[attr.value_start..attr.value_end];

        let replacement = sanitized_attr_replacement(name, value);
        if let AttrReplacement::Unchanged = replacement {
            cursor = attr.full_end;
            continue;
        }

        if !changed {
            out = String::with_capacity(tag.len());
            changed = true;
        }
        out.push_str(&tag[copied_until..attr.full_start]);
        match replacement {
            AttrReplacement::Unchanged => {}
            AttrReplacement::Drop => {}
            AttrReplacement::Replace(replacement) => out.push_str(&replacement),
        }
        copied_until = attr.full_end;
        cursor = attr.full_end;
    }

    if changed {
        out.push_str(&tag[copied_until..]);
        Cow::Owned(out)
    } else {
        Cow::Borrowed(tag)
    }
}

enum AttrReplacement {
    Unchanged,
    Drop,
    Replace(String),
}

fn sanitized_attr_replacement(name: &str, value: &str) -> AttrReplacement {
    if should_drop_attribute(name, value) {
        return AttrReplacement::Drop;
    }

    if let Some(value) = normalize_px_attribute(name, value) {
        return AttrReplacement::Replace(format!(r#" {name}="{value}""#));
    }

    if name.eq_ignore_ascii_case("style") {
        let sanitized = sanitize_style_attribute(value);
        if sanitized.trim().is_empty() {
            return AttrReplacement::Drop;
        }
        if sanitized != value {
            return AttrReplacement::Replace(format!(r#" style="{sanitized}""#));
        }
    }

    AttrReplacement::Unchanged
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
    let mut cursor = 0usize;
    while let Some(attr) = next_svg_quoted_attr(tag, cursor) {
        if tag[attr.name_start..attr.name_end].eq_ignore_ascii_case(name) {
            return Some(tag[attr.value_start..attr.value_end].to_string());
        }
        cursor = attr.full_end;
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

    #[test]
    fn sanitize_element_attributes_scans_double_quoted_attrs_without_regex() {
        let svg = r#"<svg><path data-keep = "ok" x = "10px" stroke="" style="transform: rotate(45deg); animation: dash 1s; stroke: #333;"/></svg>"#;
        let out = sanitize_element_attributes(svg);

        assert!(out.contains(r#"data-keep = "ok""#), "got: {out}");
        assert!(out.contains(r#" x="10""#), "got: {out}");
        assert!(!out.contains(r#"stroke="""#), "got: {out}");
        assert!(
            out.contains(r#"style="transform:rotate(45);stroke:#333""#),
            "got: {out}"
        );
        assert!(!out.contains("animation"), "got: {out}");
    }

    #[test]
    fn sanitize_element_attributes_scans_single_quoted_attrs() {
        let svg = r#"<svg><path x = '10px' style='animation: dash 1s; stroke: #333;'/></svg>"#;
        let out = sanitize_element_attributes(svg);

        assert!(out.contains(r#" x="10""#), "got: {out}");
        assert!(out.contains(r#"style="stroke:#333""#), "got: {out}");
        assert!(!out.contains("animation"), "got: {out}");
    }

    #[test]
    fn sanitize_element_attributes_uses_scanned_attrs_for_bad_rect_detection() {
        let svg = r#"<svg><rect WIDTH = "12" HEIGHT = "8"/><rect width = "NaN" height = "8"><title>bad</title></rect></svg>"#;
        let out = sanitize_element_attributes(svg);

        assert!(
            out.contains(r#"<rect WIDTH = "12" HEIGHT = "8"/>"#),
            "got: {out}"
        );
        assert!(!out.contains("NaN"), "got: {out}");
        assert!(!out.contains("<title>bad</title>"), "got: {out}");
    }
}
