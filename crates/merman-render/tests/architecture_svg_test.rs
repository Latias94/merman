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

fn render_architecture_fixture(fixture_name: &str) -> String {
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
        &SvgRenderOptions {
            diagram_id: Some("architecture-crosslinks".to_string()),
            ..Default::default()
        },
    )
    .expect("render SVG")
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

#[test]
fn architecture_diagonal_arrows_follow_the_actual_edge_segment() {
    let svg = render_architecture_fixture(
        "stress_architecture_batch5_services_outside_groups_crosslinks_078.mmd",
    );

    let diagonal = arrow_transform_after_edge(&svg, "L_fe_east_api_0");
    assert!(
        diagonal.contains("rotate("),
        "expected diagonal Architecture edge arrow to rotate with the edge segment, got {diagonal}"
    );

    let vertical = arrow_transform_after_edge(&svg, "L_fe_west_api_0");
    assert!(
        !vertical.contains("rotate("),
        "axis-aligned Architecture arrows should keep the Mermaid-compatible translate-only DOM, got {vertical}"
    );
}
