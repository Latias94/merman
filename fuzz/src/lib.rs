#![forbid(unsafe_code)]

use chrono::NaiveDate;
use merman::Engine;
use merman::render::{
    HeadlessRenderer, RenderEnvironment, RenderResourcePolicy, RenderTimeSnapshot, ResourceLimitId,
    finalize_resvg_svg,
};
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
    let mut limits = RenderResourcePolicy::interactive();
    for (id, value) in [
        (ResourceLimitId::MaxSourceBytes, MAX_RENDER_INPUT_BYTES),
        (ResourceLimitId::MaxSvgBytes, 4 * 1024 * 1024),
        (ResourceLimitId::MaxFlowchartNodes, 512),
        (ResourceLimitId::MaxFlowchartEdges, 1_024),
        (ResourceLimitId::MaxFlowchartSubgraphs, 128),
        (ResourceLimitId::MaxClassNodes, 512),
        (ResourceLimitId::MaxClassEdges, 1_024),
        (ResourceLimitId::MaxClassNamespaces, 128),
        (ResourceLimitId::MaxLabelBytes, 256 * 1024),
    ] {
        limits
            .apply_limit(id, value)
            .expect("fuzz resource policy uses overridable positive limits");
    }

    HeadlessRenderer::new()
        .with_fixed_time(
            RenderTimeSnapshot::from_unix_millis(1_735_689_600_000, 0)
                .expect("2025-01-01 UTC is a valid fixed render time"),
        )
        .with_strict_parsing()
        .with_deterministic_text_measurer()
        .with_resource_policy(limits)
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
    let session = RenderEnvironment::parity()
        .begin_session()
        .expect("parity render session must be constructible");
    let normalized = finalize_resvg_svg(svg, &session)
        .unwrap_or_else(|error| panic!("resvg-safe output failed terminal validation: {error}"));
    assert_eq!(
        normalized.as_str(),
        svg,
        "resvg-safe output was not terminally normalized"
    );

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

#[cfg(test)]
mod tests {
    use super::assert_resvg_safe_svg;

    #[test]
    fn css_url_oracle_does_not_treat_function_name_suffixes_as_url_functions() {
        assert_resvg_safe_svg(
            r##"<svg xmlns="http://www.w3.org/2000/svg"><path style="filter:filterBurl(file:///tmp/x);stroke:#033"/></svg>"##,
        );
    }

    #[test]
    #[should_panic(expected = "resvg-safe output was not terminally normalized")]
    fn terminal_oracle_rejects_an_actual_unsafe_css_url() {
        assert_resvg_safe_svg(
            r##"<svg xmlns="http://www.w3.org/2000/svg"><path style="fill:url('file:///tmp/x')"/></svg>"##,
        );
    }
}
