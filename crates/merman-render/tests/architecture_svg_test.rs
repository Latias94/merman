use merman_core::{Engine, ParseOptions};
use merman_render::svg::{SvgRenderOptions, render_layout_svg_parts_for_render_model_with_config};
use merman_render::{LayoutOptions, layout_parsed_render_layout_only};
use regex::Regex;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn render_architecture_fixture_with_options(
    fixture_name: &str,
    options: &SvgRenderOptions,
) -> String {
    let path = workspace_root()
        .join("fixtures")
        .join("architecture")
        .join(fixture_name);
    let text = std::fs::read_to_string(&path).expect("read fixture");

    let engine = Engine::new();
    let parsed = engine
        .parse_diagram_for_render_model_sync(&text, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected");
    let layout_options = LayoutOptions::headless_svg_defaults();
    let layout = layout_parsed_render_layout_only(&parsed, &layout_options).expect("layout ok");

    render_layout_svg_parts_for_render_model_with_config(
        &layout,
        &parsed.model,
        &parsed.meta.effective_config,
        parsed.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        options,
    )
    .expect("render SVG")
}

fn render_architecture_fixture(fixture_name: &str) -> String {
    render_architecture_fixture_with_options(
        fixture_name,
        &SvgRenderOptions {
            diagram_id: Some("architecture-crosslinks".to_string()),
            ..Default::default()
        },
    )
}

fn arrow_transform_after_edge(svg: &str, edge_id: &str) -> String {
    let pattern = format!(r#"id="{}"[^>]*/><polygon([^>]*)>"#, regex::escape(edge_id));
    let re = Regex::new(&pattern).expect("valid regex");
    let attrs = re
        .captures(svg)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str())
        .unwrap_or_else(|| panic!("missing arrow polygon after edge {edge_id}"));
    assert!(
        attrs.contains(r#"class="arrow""#),
        "expected polygon after edge {edge_id} to be an arrow, got {attrs}"
    );

    let transform_re = Regex::new(r#"\btransform="([^"]+)""#).expect("valid regex");
    transform_re
        .captures(attrs)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| panic!("missing arrow transform after edge {edge_id}"))
}

fn service_translate(svg: &str, service_id: &str) -> (f64, f64) {
    let pattern = format!(
        r#"id="{}"[^>]*\btransform="translate\(([^,\s]+)[,\s]+([^)]+)\)""#,
        regex::escape(service_id)
    );
    let re = Regex::new(&pattern).expect("valid regex");
    let caps = re
        .captures(svg)
        .unwrap_or_else(|| panic!("missing service transform for {service_id}"));
    let x = caps
        .get(1)
        .and_then(|m| m.as_str().parse::<f64>().ok())
        .unwrap_or_else(|| panic!("invalid service x transform for {service_id}"));
    let y = caps
        .get(2)
        .and_then(|m| m.as_str().parse::<f64>().ok())
        .unwrap_or_else(|| panic!("invalid service y transform for {service_id}"));
    (x, y)
}

fn group_rect(svg: &str, group_id: &str) -> (f64, f64, f64, f64) {
    let pattern = format!(
        r#"id="{}"[^>]*\bx="([^"]+)"[^>]*\by="([^"]+)"[^>]*\bwidth="([^"]+)"[^>]*\bheight="([^"]+)""#,
        regex::escape(group_id)
    );
    let re = Regex::new(&pattern).expect("valid regex");
    let caps = re
        .captures(svg)
        .unwrap_or_else(|| panic!("missing group rect for {group_id}"));
    let parse = |idx: usize, label: &str| {
        caps.get(idx)
            .and_then(|m| m.as_str().parse::<f64>().ok())
            .unwrap_or_else(|| panic!("invalid {label} for {group_id}"))
    };
    (
        parse(1, "x"),
        parse(2, "y"),
        parse(3, "width"),
        parse(4, "height"),
    )
}

fn icon_text_line_clamp(svg: &str, service_id: &str) -> i64 {
    let pattern = format!(
        r#"id="{}"[\s\S]*?-webkit-line-clamp:\s*([0-9]+);"#,
        regex::escape(service_id)
    );
    let re = Regex::new(&pattern).expect("valid regex");
    re.captures(svg)
        .and_then(|caps| caps.get(1))
        .and_then(|m| m.as_str().parse::<i64>().ok())
        .unwrap_or_else(|| panic!("missing iconText line clamp for {service_id}"))
}

fn assert_close(actual: f64, expected: f64, message: &str) {
    let delta = (actual - expected).abs();
    assert!(
        delta <= 1e-6,
        "{message}: expected {expected}, got {actual}, delta {delta}"
    );
}

#[test]
fn architecture_diagonal_arrows_follow_the_actual_edge_segment() {
    let svg = render_architecture_fixture(
        "stress_architecture_batch5_services_outside_groups_crosslinks_078.mmd",
    );

    let diagonal = arrow_transform_after_edge(&svg, "architecture-crosslinks-L_fe_east_api_0");
    assert!(
        diagonal.contains("rotate("),
        "expected diagonal Architecture edge arrow to rotate with the edge segment, got {diagonal}"
    );

    let vertical = arrow_transform_after_edge(&svg, "architecture-crosslinks-L_fe_west_api_0");
    assert!(
        !vertical.contains("rotate("),
        "axis-aligned Architecture arrows should keep the Mermaid-compatible translate-only DOM, got {vertical}"
    );
}

#[test]
fn architecture_group_alignment_follows_source_endpoint_traversal_order() {
    let svg = render_architecture_fixture_with_options(
        "stress_architecture_deep_nesting_013.mmd",
        &SvgRenderOptions {
            diagram_id: Some("architecture-deep".to_string()),
            ..Default::default()
        },
    );

    let lb = service_translate(&svg, "architecture-deep-service-lb");
    let api = service_translate(&svg, "architecture-deep-service-api");
    let cache = service_translate(&svg, "architecture-deep-service-cache");
    let ext = service_translate(&svg, "architecture-deep-service-ext");

    assert_close(
        lb.1,
        api.1,
        "lb/api should share Mermaid's horizontal alignment",
    );
    assert_close(
        lb.0,
        ext.0,
        "lb/ext should share Mermaid's vertical alignment",
    );
    assert_close(
        api.0,
        cache.0,
        "api/cache should share the final core/data vertical alignment",
    );
}

#[test]
fn architecture_group_rect_uses_configured_padding_for_small_icons() {
    let svg = render_architecture_fixture_with_options(
        "stress_architecture_batch6_init_fontsize_icon_size_wrap_093.mmd",
        &SvgRenderOptions {
            diagram_id: Some("architecture-padding".to_string()),
            ..Default::default()
        },
    );

    let left = group_rect(&svg, "architecture-padding-group-left");
    assert!(
        left.2 >= 158.0,
        "custom architecture.padding should expand the group rect beyond the legacy iconSize/2 sizing, got width {}",
        left.2
    );
}

#[test]
fn architecture_long_title_group_rect_stays_within_current_headless_envelope() {
    let svg = render_architecture_fixture_with_options(
        "stress_architecture_batch5_long_titles_and_punct_076.mmd",
        &SvgRenderOptions {
            diagram_id: Some("architecture-batch5-long".to_string()),
            ..Default::default()
        },
    );

    let pipeline = group_rect(&svg, "architecture-batch5-long-group-pipeline");
    assert!(
        pipeline.2 > 470.0 && pipeline.2 < 473.5,
        "unexpected pipeline group width drift for long-title architecture fixture: {}",
        pipeline.2
    );
}

#[test]
fn architecture_icon_text_clamp_uses_architecture_font_size() {
    let svg = render_architecture_fixture_with_options(
        "upstream_architecture_docs_service_icon_text.mmd",
        &SvgRenderOptions {
            diagram_id: Some("architecture-icontext".to_string()),
            ..Default::default()
        },
    );

    let clamp = icon_text_line_clamp(&svg, "architecture-icontext-service-with_icon_text");
    assert_eq!(
        clamp, 4,
        "iconText clamp should follow default architecture.fontSize=16 with iconSize=80"
    );
}
