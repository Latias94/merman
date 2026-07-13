#![forbid(unsafe_code)]

use chrono::NaiveDate;
use merman::Engine;
use merman::render::{HeadlessRenderer, RenderResourceLimits};
use merman_core::entities::decode_html_entities_to_unicode;
use roxmltree::Document;

pub const MAX_PARSE_INPUT_BYTES: usize = 256 * 1024;
pub const MAX_RENDER_INPUT_BYTES: usize = 32 * 1024;
pub const MAX_SVG_INPUT_BYTES: usize = 256 * 1024;

pub fn bounded_utf8(data: &[u8], max_bytes: usize) -> Option<&str> {
    if data.len() > max_bytes {
        return None;
    }
    std::str::from_utf8(data).ok()
}

pub fn deterministic_engine() -> Engine {
    Engine::new()
        .with_fixed_today(NaiveDate::from_ymd_opt(2025, 1, 1))
        .with_fixed_local_offset_minutes(Some(0))
}

pub fn bounded_renderer() -> HeadlessRenderer {
    let limits = RenderResourceLimits {
        max_source_bytes: Some(MAX_RENDER_INPUT_BYTES),
        max_svg_bytes: Some(4 * 1024 * 1024),
        max_flowchart_nodes: Some(512),
        max_flowchart_edges: Some(1_024),
        max_flowchart_subgraphs: Some(128),
        max_class_nodes: Some(512),
        max_class_edges: Some(1_024),
        max_class_namespaces: Some(128),
        max_label_bytes: Some(256 * 1024),
    };

    HeadlessRenderer::new()
        .with_fixed_today(NaiveDate::from_ymd_opt(2025, 1, 1))
        .with_fixed_local_offset_minutes(Some(0))
        .with_strict_parsing()
        .with_deterministic_text_measurer()
        .with_resource_limits(limits)
        .with_diagram_id("fuzz")
}

pub fn is_well_formed_svg(svg: &str) -> bool {
    Document::parse(svg).is_ok_and(|document| {
        document
            .root_element()
            .tag_name()
            .name()
            .eq_ignore_ascii_case("svg")
    })
}

pub fn assert_resvg_safe_svg(svg: &str) {
    let document = Document::parse(svg)
        .unwrap_or_else(|error| panic!("successful SVG output is not well formed: {error}"));
    let root = document.root_element();
    assert!(
        root.tag_name().name().eq_ignore_ascii_case("svg"),
        "successful SVG output has a non-SVG root"
    );

    for node in document.descendants().filter(roxmltree::Node::is_element) {
        let local_name = node.tag_name().name();
        assert!(
            !is_active_svg_element(local_name),
            "resvg-safe output retained active element <{local_name}>"
        );

        for attribute in node.attributes() {
            let name = attribute.name();
            assert!(
                !is_event_handler_attribute(name),
                "resvg-safe output retained event attribute {name}"
            );
            assert!(
                !is_unsafe_url_reference(name, attribute.value()),
                "resvg-safe output retained an unsafe {name} URL reference"
            );
        }
    }
}

fn is_active_svg_element(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "script" | "iframe" | "object" | "embed" | "foreignobject"
    )
}

fn is_event_handler_attribute(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() > 2
        && name
            .get(..2)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("on"))
        && bytes[2].is_ascii_alphabetic()
}

fn is_unsafe_url_reference(name: &str, value: &str) -> bool {
    let name = name.to_ascii_lowercase();
    if matches!(name.as_str(), "href" | "src") {
        return is_unsafe_url_value(value);
    }
    if name == "style" || is_url_function_attribute(&name) {
        return css_value_contains_unsafe_url_function(value);
    }
    false
}

fn is_url_function_attribute(name: &str) -> bool {
    matches!(
        name,
        "fill"
            | "stroke"
            | "filter"
            | "clip-path"
            | "mask"
            | "marker-start"
            | "marker-mid"
            | "marker-end"
    )
}

fn css_value_contains_unsafe_url_function(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    let mut cursor = 0usize;
    while let Some(rel_start) = lower[cursor..].find("url(") {
        let arg_start = cursor + rel_start + "url(".len();
        let Some(rel_end) = lower[arg_start..].find(')') else {
            return true;
        };
        let arg_end = arg_start + rel_end;
        if is_unsafe_url_value(trim_css_url_argument(&value[arg_start..arg_end])) {
            return true;
        }
        cursor = arg_end + 1;
    }
    false
}

fn trim_css_url_argument(value: &str) -> &str {
    let value = value.trim();
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        if (bytes[0] == b'"' && bytes[value.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[value.len() - 1] == b'\'')
        {
            return &value[1..value.len() - 1];
        }
    }
    value
}

fn is_unsafe_url_value(value: &str) -> bool {
    let normalized = normalize_url_for_scheme_check(value);
    if normalized.is_empty() || normalized.starts_with('#') {
        return false;
    }
    if let Some(data) = normalized.strip_prefix("data:") {
        let media_type = data.split_once(',').map_or(data, |(head, _)| head);
        return !matches!(
            media_type.split(';').next().unwrap_or_default(),
            "image/png" | "image/jpeg" | "image/jpg" | "image/gif" | "image/webp"
        );
    }

    let Some((scheme, _)) = normalized.split_once(':') else {
        return false;
    };
    !matches!(scheme, "http" | "https" | "mailto")
}

fn normalize_url_for_scheme_check(value: &str) -> String {
    let decoded = decode_html_entities_to_unicode(value);
    let mut normalized = String::with_capacity(decoded.len());
    for ch in decoded.trim().chars() {
        if !ch.is_whitespace() && !ch.is_control() {
            normalized.extend(ch.to_lowercase());
        }
    }
    normalized
}
