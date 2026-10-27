//! Paint ownership for the fixed SVG vocabulary emitted by the compiled RaTeX backend.

use quick_xml::XmlVersion;
use quick_xml::events::BytesStart;
#[cfg(feature = "math")]
use quick_xml::events::Event;
#[cfg(feature = "math")]
use quick_xml::{Reader, Writer};
use std::collections::BTreeMap;
use std::fmt::Write as _;

const PAINT_DEFAULTS: &[(&str, &str)] = &[
    ("fill", "black"),
    ("fill-opacity", "1"),
    ("fill-rule", "nonzero"),
    ("stroke", "none"),
    ("stroke-opacity", "1"),
    ("stroke-width", "1"),
    ("stroke-linecap", "butt"),
    ("stroke-linejoin", "miter"),
    ("stroke-miterlimit", "4"),
    ("stroke-dasharray", "none"),
    ("stroke-dashoffset", "0"),
    ("opacity", "1"),
    ("filter", "none"),
];

const TEXT_DEFAULTS: &[(&str, &str)] = &[
    ("font-family", "serif"),
    ("font-size", "16"),
    ("font-weight", "normal"),
    ("font-style", "normal"),
    ("font-variant", "normal"),
    ("font-stretch", "normal"),
    ("text-anchor", "start"),
    ("dominant-baseline", "alphabetic"),
    ("white-space", "pre"),
];

fn owned_property(name: &str) -> Option<&'static str> {
    PAINT_DEFAULTS
        .iter()
        .chain(TEXT_DEFAULTS)
        .find_map(|&(property, _)| (property == name).then_some(property))
}

fn valid_property(name: &str, value: &str) -> bool {
    let finite = || value.parse::<f64>().ok().filter(|value| value.is_finite());
    match name {
        "fill" | "stroke" => {
            value == "none"
                || ((value.starts_with("rgb(") || value.starts_with("rgba("))
                    && merman_core::theme_color::ThemeColor::parse(value).is_ok())
        }
        "fill-opacity" | "stroke-opacity" | "opacity" => {
            finite().is_some_and(|value| (0.0..=1.0).contains(&value))
        }
        "stroke-width" | "stroke-miterlimit" | "font-size" => {
            finite().is_some_and(|value| value >= 0.0)
        }
        "stroke-dashoffset" => finite().is_some(),
        "stroke-dasharray" => {
            let mut parts = value.split_ascii_whitespace();
            let valid = |part: &str| {
                part.parse::<f64>()
                    .is_ok_and(|value| value.is_finite() && value >= 0.0)
            };
            parts.next().is_some_and(valid)
                && parts.next().is_some_and(valid)
                && parts.next().is_none()
        }
        "fill-rule" => value == "nonzero",
        "stroke-linecap" | "stroke-linejoin" => value == "round",
        "font-family" => {
            !value.contains([':', '!'])
                && crate::mermaid_style::parse_safe_style_decl(&format!("font-family: {value}"))
                    .is_some()
        }
        "font-weight" => matches!(value, "normal" | "bold"),
        "font-style" => matches!(value, "normal" | "italic"),
        "dominant-baseline" => value == "alphabetic",
        _ => false,
    }
}

/// Canonical declarations for a backend primitive, also used by maintenance admission.
///
/// Values come from validated presentation attributes, never from an input style declaration.
pub(crate) fn primitive_paint_style(element: &BytesStart<'_>) -> Option<String> {
    let name = element.name();
    let is_text = name.as_ref() == "text";
    let is_image = name.as_ref() == "image";
    if !matches!(name.as_ref(), "path" | "rect" | "line" | "text" | "image") {
        return None;
    }
    let mut values = BTreeMap::new();
    for attribute in element.attributes().with_checks(true) {
        let attribute = attribute.ok()?;
        let Some(property) = owned_property(attribute.key.as_ref()) else {
            continue;
        };
        let value = attribute.normalized_value(XmlVersion::Implicit1_0).ok()?;
        if !valid_property(property, &value) {
            return None;
        }
        values.insert(property, value.into_owned());
    }
    let mut style = String::new();
    for &(property, default) in
        PAINT_DEFAULTS
            .iter()
            .chain(if is_text { TEXT_DEFAULTS } else { &[] })
    {
        if is_image && !matches!(property, "opacity" | "filter") {
            continue;
        }
        let value = values.get(property).map_or(default, String::as_str);
        // Shape classDefs use !important. Formula paint has already resolved its own foreground.
        if !style.is_empty() {
            style.push(';');
        }
        let _ = write!(style, "{property}:{value}!important");
    }
    Some(style)
}

#[cfg(feature = "math")]
pub(super) struct RenderedFormula {
    pub(super) svg: String,
    pub(super) width_em: f64,
    pub(super) height_em: f64,
}

#[cfg(feature = "math")]
pub(super) fn render(
    display_list: &ratex_types::DisplayList,
    root_width: f64,
) -> Option<RenderedFormula> {
    let options = ratex_svg::SvgOptions {
        font_size: 1.0,
        padding: 0.0,
        stroke_width: 0.04,
        embed_glyphs: true,
        font_dir: String::new(),
    };
    let svg = own_primitive_paint(&ratex_svg::render_to_svg(display_list, &options))?;
    // Missing exact metrics identify candidates, not a claim about the selected font.
    // Render only these candidates to keep structural paths and ordinary KaTeX ink
    // on the upstream display-list bounds policy (including its large-path limits).
    let candidates = ratex_types::DisplayList {
        items: display_list
            .items
            .iter()
            .filter(|item| {
                let ratex_types::DisplayItem::GlyphPath {
                    font, char_code, ..
                } = item
                else {
                    return false;
                };
                let Some(font) = ratex_font::FontId::parse(font) else {
                    return false;
                };
                let character = ratex_font::katex_ttf_glyph_char(font, *char_code);
                ratex_font::get_char_metrics(font, u32::from(character)).is_none()
            })
            .cloned()
            .collect(),
        width: display_list.width,
        height: display_list.height,
        depth: display_list.depth,
    };
    let candidate_svg =
        (!candidates.items.is_empty()).then(|| ratex_svg::render_to_svg(&candidates, &options));
    fit_resolved_ink(
        &svg,
        candidate_svg.as_deref(),
        display_list.width,
        display_list.total_height(),
        root_width < 0.01,
    )
}

/// Reconcile the logical box with the actual self-contained glyphs emitted by RaTeX.
///
/// The backend resolves fallback outlines and PNG strikes after layout. Vertical ink
/// belongs to the formula viewport, just as in RaTeX's display-list bounds recovery.
/// Nonzero horizontal advances retain intentional nested lap clipping; only a
/// near-zero root advance is recovered, following ratex-layout's to_display_list.
#[cfg(feature = "math")]
fn fit_resolved_ink(
    svg: &str,
    ink_svg: Option<&str>,
    logical_width: f64,
    logical_height: f64,
    recover_horizontal: bool,
) -> Option<RenderedFormula> {
    use kurbo::Shape as _;
    let document = roxmltree::Document::parse(ink_svg.unwrap_or("<svg/>")).ok()?;
    let root = document.root_element();
    const UNITS_PER_EM: f64 = 1_000_000.0;
    // Keep already-rounded logical bounds in integer micro-units. Reapplying ceil
    // after an f64 division can spuriously add one unit (for example 0.125008em).
    let mut bounds = kurbo::Rect::new(
        0.0,
        0.0,
        (logical_width.max(0.0) * UNITS_PER_EM).round(),
        (logical_height.max(0.0) * UNITS_PER_EM).round(),
    );
    for element in root.children().filter(|node| node.is_element()) {
        let number = |key, default| -> Option<f64> {
            let value = element
                .attribute(key)
                .map_or(Some(default), |value| value.parse::<f64>().ok())?;
            value.is_finite().then_some(value)
        };
        let ink = match element.tag_name().name() {
            "path" => {
                let path = kurbo::BezPath::from_svg(element.attribute("d")?).ok()?;
                if path.is_empty() {
                    continue;
                }
                path.bounding_box()
            }
            "image" => {
                let x = number("x", 0.0)?;
                let y = number("y", 0.0)?;
                kurbo::Rect::new(x, y, x + number("width", 0.0)?, y + number("height", 0.0)?)
            }
            // Unresolved text uses the host font. It has no renderer-owned outline to
            // measure here; do not invent a second font resolver or synthetic bbox.
            "text" => continue,
            _ => return None,
        };
        if ![ink.x0, ink.y0, ink.x1, ink.y1]
            .into_iter()
            .all(f64::is_finite)
        {
            return None;
        }
        bounds.y0 = bounds.y0.min((ink.y0 * UNITS_PER_EM).floor());
        bounds.y1 = bounds.y1.max((ink.y1 * UNITS_PER_EM).ceil());
        if recover_horizontal {
            bounds.x0 = bounds.x0.min((ink.x0 * UNITS_PER_EM).floor());
            bounds.x1 = bounds.x1.max((ink.x1 * UNITS_PER_EM).ceil());
        }
    }
    let x = bounds.x0 / UNITS_PER_EM;
    let y = bounds.y0 / UNITS_PER_EM;
    let width_em = (bounds.x1 - bounds.x0) / UNITS_PER_EM;
    let height_em = (bounds.y1 - bounds.y0) / UNITS_PER_EM;
    let open_end = svg.find('>')?;
    let body = svg.get(open_end + 1..)?.strip_suffix("</svg>")?;
    Some(RenderedFormula {
        svg: format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{x} {y} {width_em} {height_em}" width="{width_em}em" height="{height_em}em" style="font-size:inherit!important">{body}</svg>"#,
        ),
        width_em,
        height_em,
    })
}

#[cfg(feature = "math")]
fn own_primitive_paint(svg: &str) -> Option<String> {
    let mut reader = Reader::from_str(svg);
    reader.config_mut().check_end_names = true;
    let mut writer = Writer::new(Vec::with_capacity(svg.len()));
    let mut depth = 0usize;
    let mut saw_root = false;
    loop {
        let event = reader.read_event().ok()?;
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(mut tag) | Event::Empty(mut tag) => {
                if depth == 0 {
                    if saw_root || tag.name().as_ref() != "svg" || empty {
                        return None;
                    }
                    saw_root = true;
                } else {
                    if depth != 1 || tag.try_get_attribute("style").ok()?.is_some() {
                        return None;
                    }
                    let style = primitive_paint_style(&tag)?;
                    tag.push_attribute(("style", style.as_str()));
                }
                if empty {
                    writer.write_event(Event::Empty(tag)).ok()?;
                } else {
                    depth += 1;
                    writer.write_event(Event::Start(tag)).ok()?;
                }
            }
            Event::End(tag) => {
                depth = depth.checked_sub(1)?;
                writer.write_event(Event::End(tag)).ok()?;
            }
            Event::Text(text) if depth == 2 => writer.write_event(Event::Text(text)).ok()?,
            Event::GeneralRef(reference) if depth == 2 => {
                writer.write_event(Event::GeneralRef(reference)).ok()?;
            }
            Event::Eof if saw_root && depth == 0 => break,
            _ => return None,
        }
    }
    String::from_utf8(writer.into_inner()).ok()
}

#[cfg(all(test, feature = "math"))]
mod tests {
    use super::*;

    #[test]
    fn primitive_paint_preserves_each_backend_semantic_and_svg_defaults() {
        let input = concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg">"#,
            r#"<path d="M0 0L1 1" fill="rgba(255,0,0,0.5)" stroke="none"/>"#,
            r#"<rect width="2" height="0.04" fill="rgb(0,0,255)" fill-opacity="0.25"/>"#,
            r#"<path d="M0 0L1 1" fill="none" stroke="rgba(0,0,0,1)" stroke-width="0.04" stroke-linecap="round" stroke-linejoin="round"/>"#,
            r#"<line x1="0" x2="2" y1="1" y2="1" stroke="rgb(0,0,0)" stroke-width="0.06" stroke-dasharray="0.18 0.18"/>"#,
            r#"<text x="0" y="1" font-family="KaTeX_Main" font-size="1" font-weight="bold" font-style="italic" fill="rgb(0,0,0)">&lt;</text>"#,
            r#"<image href="data:image/png;base64,AAAA" width="1" height="1" opacity="0.5"/>"#,
            "</svg>",
        );
        let owned = own_primitive_paint(input).expect("fixed backend vocabulary");
        let before = roxmltree::Document::parse(input).unwrap();
        let after = roxmltree::Document::parse(&owned).unwrap();
        let children: Vec<_> = after
            .root_element()
            .children()
            .filter(|n| n.is_element())
            .collect();
        for (original, sealed) in before
            .root_element()
            .children()
            .filter(|n| n.is_element())
            .zip(&children)
        {
            for attribute in original.attributes() {
                assert_eq!(sealed.attribute(attribute.name()), Some(attribute.value()));
            }
        }
        let style = |index: usize| children[index].attribute("style").unwrap();
        assert!(style(0).contains("fill:rgba(255,0,0,0.5)!important"));
        assert!(style(1).contains("fill-opacity:0.25!important"));
        assert!(style(1).contains("stroke:none!important"));
        assert!(style(2).contains("fill:none!important"));
        assert!(style(2).contains("stroke-width:0.04!important"));
        assert!(style(3).contains("stroke-dasharray:0.18 0.18!important"));
        assert!(style(4).contains("font-family:KaTeX_Main!important"));
        assert!(style(4).contains("font-size:1!important"));
        assert!(style(4).contains("font-style:italic!important"));
        assert_eq!(children[4].text(), Some("<"));
        assert_eq!(style(5), "opacity:0.5!important;filter:none!important");
    }

    #[test]
    fn ordinary_katex_and_structural_paths_keep_upstream_display_bounds() {
        let list = ratex_types::DisplayList {
            items: vec![ratex_types::DisplayItem::Path {
                x: 0.0,
                y: 0.0,
                commands: vec![
                    ratex_types::PathCommand::MoveTo {
                        x: 0.0,
                        y: -100000.0,
                    },
                    ratex_types::PathCommand::LineTo {
                        x: 1.0,
                        y: 100000.0,
                    },
                ],
                fill: true,
                color: ratex_types::Color::BLACK,
            }],
            width: 1.0,
            height: 0.5,
            depth: 0.5,
        };
        let rendered = render(&list, 1.0).unwrap();
        assert_eq!((rendered.width_em, rendered.height_em), (1.0, 1.0));
        assert!(rendered.svg.contains(r#"viewBox="0 0 1 1""#));
        let font = ratex_font::FontId::MainBold;
        let mapped = ratex_font::katex_ttf_glyph_char(font, u32::from('𝐀'));
        assert!(ratex_font::get_char_metrics(font, u32::from(mapped)).is_some());
    }

    #[test]
    fn resolved_geometry_recovers_zero_boxes_without_expanding_nested_lap_advances() {
        let svg = r#"<svg><path d="M-0.2 -0.8L1.3 -0.8L1.3 0.2Z"/><image x="0" y="-0.01" width="1" height="1"/></svg>"#;
        let normal = fit_resolved_ink(svg, Some(svg), 1.0, 0.74, false).unwrap();
        let zero = fit_resolved_ink(svg, Some(svg), 0.0, 0.0, true).unwrap();
        assert_eq!(
            normal.width_em, 1.0,
            "nonzero parent advance retains nested lap clipping"
        );
        assert_eq!(
            zero.width_em, 1.5,
            "a zero-width root recovers actual horizontal ink"
        );
        for result in [normal, zero] {
            let document = roxmltree::Document::parse(&result.svg).unwrap();
            let viewport: Vec<f64> = document
                .root_element()
                .attribute("viewBox")
                .unwrap()
                .split_ascii_whitespace()
                .map(|value| value.parse().unwrap())
                .collect();
            assert_eq!(viewport[1], -0.8);
            assert!(viewport[1] + viewport[3] >= 0.99);
            assert_eq!(result.height_em, viewport[3]);
            assert!(
                result.svg.contains(r#"d="M-0.2 -0.8L1.3 -0.8L1.3 0.2Z""#),
                "preserve emitted geometry"
            );
        }
        let exact = fit_resolved_ink("<svg></svg>", None, 0.125008, 1.0, false).unwrap();
        assert_eq!(exact.width_em, 0.125008);
        let empty = fit_resolved_ink("<svg></svg>", None, 0.0, 0.0, true).unwrap();
        assert_eq!(
            (empty.width_em, empty.height_em),
            (0.0, 0.0),
            "phantom/empty content must not gain synthetic ink"
        );
    }

    #[test]
    fn resolved_fallback_viewport_contains_real_ink() {
        for (font, character) in [("CJK-Regular", '中'), ("Emoji-Fallback", '😀')] {
            let list = ratex_types::DisplayList {
                items: vec![ratex_types::DisplayItem::GlyphPath {
                    x: 0.0,
                    y: 0.0,
                    scale: 1.0,
                    font: font.to_owned(),
                    char_code: u32::from(character),
                    color: ratex_types::Color::BLACK,
                }],
                width: 1.0,
                height: 0.0,
                depth: 0.0,
            };
            let rendered = render(&list, list.width).unwrap();
            let svg = rendered.svg;
            let document = roxmltree::Document::parse(&svg).unwrap();
            let root = document.root_element();
            // Font availability belongs to the platform; the synthetic bounds test below
            // remains unconditional when this backend cannot resolve a system glyph.
            if !root
                .children()
                .any(|node| matches!(node.tag_name().name(), "path" | "image"))
            {
                continue;
            }
            let viewport: Vec<f64> = root
                .attribute("viewBox")
                .unwrap()
                .split_ascii_whitespace()
                .map(|number| number.parse().unwrap())
                .collect();
            assert!(viewport[2] > 0.0 && viewport[3] > 0.0, "{font}: {svg}");
        }
    }

    #[test]
    fn backend_adapter_rejects_unexpected_markup_and_invalid_owned_values() {
        for body in [
            "<g><path/></g>",
            "<script/>",
            r#"<path style="fill:red"/>"#,
            r#"<path fill="url(https://example.com/paint)"/>"#,
            r#"<path stroke-width="NaN"/>"#,
            r#"<path fill="rgb(0,0,0)" fill="rgb(255,0,0)"/>"#,
        ] {
            assert!(
                own_primitive_paint(&format!("<svg>{body}</svg>")).is_none(),
                "{body}"
            );
        }
        assert!(own_primitive_paint("<svg></svg><svg></svg>").is_none());
    }
}
