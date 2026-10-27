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
pub(super) fn render(display_list: &ratex_types::DisplayList) -> Option<String> {
    let svg = ratex_svg::render_to_svg(
        display_list,
        &ratex_svg::SvgOptions {
            font_size: 1.0,
            padding: 0.0,
            stroke_width: 0.04,
            embed_glyphs: true,
            font_dir: String::new(),
        },
    );
    own_primitive_paint(&svg)
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
