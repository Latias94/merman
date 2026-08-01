use std::borrow::Cow;

use super::util::{SvgTagScanner, next_svg_quoted_attr, start_tag_name};
use crate::family::RenderFamilyKind;
use crate::svg::pipeline::SvgPostprocessMetadata;

const QUADRANT_BROWSER_POINT_FILL: &str = "#000000";
const QUADRANT_BROWSER_POINT_STROKE: &str = "none";

pub(crate) fn resolve_resvg_presentation_fallbacks<'a>(
    svg: Cow<'a, str>,
    metadata: &SvgPostprocessMetadata,
) -> Cow<'a, str> {
    if metadata.family_kind() != Some(RenderFamilyKind::QuadrantChart) {
        return svg;
    }

    resolve_quadrant_point_fallbacks(svg)
}

fn resolve_quadrant_point_fallbacks<'a>(svg: Cow<'a, str>) -> Cow<'a, str> {
    // Mermaid 11.16's Quadrant renderer emits circles only for data points. Its invalid
    // presentation attributes are ignored by browsers, leaving black fill and no stroke.
    let mut scanner = SvgTagScanner::new(svg.as_ref());
    let mut rewritten = None::<String>;
    let mut copied_until = 0usize;

    while let Some(tag) = scanner.next() {
        if !is_circle_start_tag(tag.raw()) {
            continue;
        }

        let Some(resolved_tag) = resolve_invalid_circle_presentation(tag.raw()) else {
            continue;
        };
        let out = rewritten.get_or_insert_with(|| String::with_capacity(svg.len()));
        out.push_str(&svg[copied_until..tag.start()]);
        out.push_str(&resolved_tag);
        copied_until = scanner.cursor();
    }

    match rewritten {
        Some(mut out) => {
            out.push_str(&svg[copied_until..]);
            Cow::Owned(out)
        }
        None => svg,
    }
}

fn is_circle_start_tag(tag: &str) -> bool {
    start_tag_name(tag).is_some_and(|name| name.eq_ignore_ascii_case("circle"))
}

fn resolve_invalid_circle_presentation(tag: &str) -> Option<String> {
    let mut out = String::with_capacity(tag.len());
    let mut copied_until = 0usize;
    let mut cursor = 0usize;
    let mut changed = false;

    while let Some(attr) = next_svg_quoted_attr(tag, cursor) {
        let name = &tag[attr.name_start..attr.name_end];
        let value = &tag[attr.value_start..attr.value_end];
        let fallback = if name.eq_ignore_ascii_case("fill") && is_mermaid_missing_amount_hsl(value)
        {
            Some(QUADRANT_BROWSER_POINT_FILL)
        } else if name.eq_ignore_ascii_case("stroke") && is_mermaid_missing_amount_hsl(value) {
            Some(QUADRANT_BROWSER_POINT_STROKE)
        } else {
            None
        };

        if let Some(fallback) = fallback {
            changed = true;
            out.push_str(&tag[copied_until..attr.full_start]);
            out.push(' ');
            out.push_str(name);
            out.push_str(r#"=""#);
            out.push_str(fallback);
            out.push('"');
            copied_until = attr.full_end;
        }
        cursor = attr.full_end;
    }

    if changed {
        out.push_str(&tag[copied_until..]);
        Some(out)
    } else {
        None
    }
}

pub(crate) fn is_mermaid_missing_amount_hsl(value: &str) -> bool {
    let Some(body) = value
        .trim()
        .strip_prefix("hsl(")
        .and_then(|value| value.strip_suffix(')'))
    else {
        return false;
    };
    let mut channels = body.split(',');
    let (Some(hue), Some(saturation), Some(lightness), None) = (
        channels.next(),
        channels.next(),
        channels.next(),
        channels.next(),
    ) else {
        return false;
    };

    is_finite_css_number(hue) && is_finite_css_percentage(saturation) && lightness.trim() == "NaN%"
}

fn is_finite_css_number(value: &str) -> bool {
    value.trim().parse::<f64>().is_ok_and(f64::is_finite)
}

fn is_finite_css_percentage(value: &str) -> bool {
    value
        .trim()
        .strip_suffix('%')
        .is_some_and(is_finite_css_number)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_quadrant_metadata_resolves_browser_initials_for_invalid_point_colors() {
        let svg = Cow::Borrowed(
            r#"<svg><g class="data-points"><g class="data-point"><circle cx="5" cy="6" r="5" fill="hsl(240, 100%, NaN%)" stroke="hsl(240, 100%, NaN%)" stroke-width="0px"/></g></g></svg>"#,
        );
        let metadata =
            SvgPostprocessMetadata::new().with_family_kind(RenderFamilyKind::QuadrantChart);

        let out = resolve_resvg_presentation_fallbacks(svg, &metadata);

        assert!(out.contains(r##"fill="#000000""##), "{out}");
        assert!(out.contains(r#"stroke="none""#), "{out}");
        assert!(!out.contains("NaN"), "{out}");
    }

    #[test]
    fn presentation_fallback_does_not_guess_the_diagram_from_svg_text() {
        let svg = Cow::Borrowed(
            r#"<svg><circle fill="hsl(240, 100%, NaN%)" stroke="hsl(240, 100%, NaN%)"/></svg>"#,
        );
        let metadata = SvgPostprocessMetadata::new().with_diagram_type("quadrantChart");

        let out = resolve_resvg_presentation_fallbacks(svg, &metadata);

        assert!(matches!(out, Cow::Borrowed(_)));
        assert!(out.contains("NaN"));
    }

    #[test]
    fn explicit_quadrant_point_colors_are_not_rewritten() {
        let svg = Cow::Borrowed(r##"<svg><circle fill="#facc15" stroke="#facc15"/></svg>"##);
        let metadata =
            SvgPostprocessMetadata::new().with_family_kind(RenderFamilyKind::QuadrantChart);

        let out = resolve_resvg_presentation_fallbacks(svg, &metadata);

        assert!(matches!(out, Cow::Borrowed(_)));
        assert!(out.contains(r##"fill="#facc15""##));
    }

    #[test]
    fn legal_local_paint_urls_with_similar_words_are_not_rewritten() {
        let svg = Cow::Borrowed(
            r##"<svg><circle fill="url(#undefined)" stroke="url(#nan)"/><circle fill="url(#undefined-gradient)" stroke="url(#nan-stroke)"/></svg>"##,
        );
        let metadata =
            SvgPostprocessMetadata::new().with_family_kind(RenderFamilyKind::QuadrantChart);

        let out = resolve_resvg_presentation_fallbacks(svg, &metadata);

        assert!(matches!(out, Cow::Borrowed(_)));
        assert!(out.contains(r##"fill="url(#undefined)""##));
        assert!(out.contains(r##"stroke="url(#nan)""##));
        assert!(out.contains(r##"fill="url(#undefined-gradient)""##));
        assert!(out.contains(r##"stroke="url(#nan-stroke)""##));
    }

    #[test]
    fn mixed_valid_fill_and_source_backed_invalid_stroke_rewrites_only_stroke() {
        let svg =
            Cow::Borrowed(r##"<svg><circle fill="#facc15" stroke="hsl(240, 100%, NaN%)"/></svg>"##);
        let metadata =
            SvgPostprocessMetadata::new().with_family_kind(RenderFamilyKind::QuadrantChart);

        let out = resolve_resvg_presentation_fallbacks(svg, &metadata);

        assert!(out.contains(r##"fill="#facc15""##), "{out}");
        assert!(out.contains(r#"stroke="none""#), "{out}");
        assert!(!out.contains(r##"fill="#000000""##), "{out}");
    }

    #[test]
    fn unsupported_invalid_paint_shapes_do_not_match_the_source_backed_fallback() {
        for value in [
            "",
            "undefined",
            "Infinity",
            "hsl(NaN, 100%, 50%)",
            "hsl(240, NaN%, 50%)",
            "hsl(240, 100%, NaN%-suffix)",
            "rgb(0, 0, NaN)",
        ] {
            assert!(!is_mermaid_missing_amount_hsl(value), "{value}");
        }
    }
}
