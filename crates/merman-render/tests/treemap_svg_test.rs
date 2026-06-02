use merman_core::{Engine, ParseOptions};
use merman_render::model::LayoutDiagram;
use merman_render::svg::{SvgRenderOptions, render_layouted_svg, render_treemap_diagram_svg};
use merman_render::{LayoutOptions, layout_parsed};
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn attr_f64(tag: &str, name: &str) -> Option<f64> {
    let needle = format!(r#"{name}=""#);
    let i = tag.find(&needle)? + needle.len();
    let rest = &tag[i..];
    let end = rest.find('"')?;
    rest[..end].parse::<f64>().ok()
}

fn text_tag_by_text<'a>(svg: &'a str, text: &str) -> &'a str {
    let needle = format!(">{text}</text>");
    let end = svg.find(&needle).expect("expected text tag") + needle.len();
    let start = svg[..end].rfind("<text").expect("expected text tag start");
    &svg[start..end]
}

fn text_tag_by_class_and_text<'a>(svg: &'a str, class_name: &str, text: &str) -> &'a str {
    let needle = format!(">{text}</text>");
    let mut offset = 0;
    while let Some(rel_end) = svg[offset..].find(&needle) {
        let end = offset + rel_end + needle.len();
        let start = svg[..end].rfind("<text").expect("expected text tag start");
        let tag = &svg[start..end];
        if tag.contains(&format!(r#"class="{class_name}""#)) {
            return tag;
        }
        offset = end;
    }
    panic!("expected text tag with class {class_name} and text {text}");
}

fn contains_default_text_fill(tag: &str) -> bool {
    tag.contains("fill:#333")
        || tag.contains("fill: #333")
        || tag.contains("fill:rgb(51, 51, 51)")
        || tag.contains("fill: rgb(51, 51, 51)")
}

fn render_treemap_svg_and_config_from_fixture(fixture: &str) -> (String, serde_json::Value) {
    let path = workspace_root()
        .join("fixtures")
        .join("treemap")
        .join(fixture);
    let text = std::fs::read_to_string(&path).expect("fixture");
    render_treemap_svg_and_config_from_source(&text)
}

fn render_treemap_svg_and_config_from_source(text: &str) -> (String, serde_json::Value) {
    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::TreemapDiagram(layout) = &out.layout else {
        panic!("expected TreemapDiagram layout");
    };

    let svg = render_treemap_diagram_svg(
        layout,
        &out.semantic,
        &out.meta.effective_config,
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    (svg, out.meta.effective_config.clone())
}

fn render_treemap_svg_from_fixture(fixture: &str) -> String {
    render_treemap_svg_and_config_from_fixture(fixture).0
}

fn render_treemap_svg_from_source(text: &str) -> String {
    render_treemap_svg_and_config_from_source(text).0
}

#[test]
fn treemap_leaf_label_font_size_matches_mermaid_cli_baselines() {
    let svg = render_treemap_svg_from_fixture("upstream_treemap_docs_basic_spec.mmd");

    let needle = ">Item A1</text>";
    let end = svg.find(needle).expect("expected Item A1 label");
    let tag = text_tag_by_text(&svg, "Item A1");

    assert!(tag.contains(r#"class="treemapLabel""#));
    assert!(
        tag.contains("font-size: 34px"),
        "expected label font-size to stay at 34px"
    );

    let rest = &svg[(end + needle.len())..];
    let value_class = rest
        .find(r#"class="treemapValue""#)
        .expect("expected value tag");
    let value_start = rest[..value_class]
        .rfind("<text")
        .expect("expected value tag start");
    let value_end_rel = rest[value_start..]
        .find("</text>")
        .expect("expected value end");
    let value_tag = &rest[value_start..(value_start + value_end_rel + "</text>".len())];
    let y = attr_f64(value_tag, "y").expect("expected y attr");
    assert!((y - 174.0).abs() < 0.0001, "expected value y to be 174");
}

#[test]
fn treemap_hierarchical_accessories_label_matches_upstream_font_size() {
    let svg = render_treemap_svg_from_fixture("upstream_treemap_docs_hierarchical_spec.mmd");
    let tag = text_tag_by_text(&svg, "Accessories");

    assert!(
        tag.contains("font-size: 16px"),
        "expected Accessories label font-size to stay at 16px"
    );
}

#[test]
fn treemap_dark_complex_example_matches_upstream_label_color_and_font_size() {
    let (svg, effective_config) = render_treemap_svg_and_config_from_fixture(
        "upstream_cypress_treemap_spec_9_should_handle_a_complex_example_with_multiple_features_016.mmd",
    );
    let theme = effective_config
        .get("theme")
        .and_then(|v| v.as_str())
        .unwrap_or("<missing>");
    let label_text_color = effective_config
        .pointer("/themeVariables/labelTextColor")
        .and_then(|v| v.as_str())
        .unwrap_or("<missing>");
    let scale_label_color = effective_config
        .pointer("/themeVariables/scaleLabelColor")
        .and_then(|v| v.as_str())
        .unwrap_or("<missing>");

    let engineering_tag = text_tag_by_text(&svg, "Engineering");
    assert!(
        engineering_tag.contains("fill:lightgrey") || engineering_tag.contains("fill: lightgrey"),
        "expected Engineering section label to use lightgrey like upstream, got {engineering_tag}; theme={theme}; labelTextColor={label_text_color}; scaleLabelColor={scale_label_color}"
    );

    let frontend_tag = text_tag_by_text(&svg, "Frontend");
    assert!(
        frontend_tag.contains("fill:lightgrey") || frontend_tag.contains("fill: lightgrey"),
        "expected Frontend leaf label to use lightgrey like upstream, got {frontend_tag}"
    );

    let digital_tag = text_tag_by_text(&svg, "Digital");
    assert!(
        digital_tag.contains("font-size: 36px"),
        "expected Digital label font-size to stay at 36px, got {digital_tag}"
    );
}

#[test]
fn treemap_single_leaf_label_uses_readable_fill_over_transparent_cell() {
    let svg = render_treemap_svg_from_source(
        r#"treemap
"Item" : 123.45
"#,
    );

    assert!(
        svg.contains(r#"class="treemapLeaf" fill="transparent""#),
        "expected single top-level leaf to preserve Mermaid's transparent cell fill: {svg}"
    );

    let label_tag = text_tag_by_text(&svg, "Item");
    assert!(
        contains_default_text_fill(label_tag),
        "single-leaf label must remain visible on the white root background: {label_tag}"
    );
    assert!(
        !label_tag.contains("fill:#ffffff")
            && !label_tag.contains("fill: #ffffff")
            && !label_tag.contains("fill: rgb(255, 255, 255)"),
        "single-leaf label should not keep upstream's white-on-transparent fill: {label_tag}"
    );

    let value_tag = text_tag_by_class_and_text(&svg, "treemapValue", "123.45");
    assert!(
        contains_default_text_fill(value_tag),
        "single-leaf value must remain visible on the white root background: {value_tag}"
    );
}

#[test]
fn treemap_classdef_bare_label_style_token_renders_error_like_mermaid_parser() {
    let source = r#"treemap
classDef c fill:#ff0000, stroke:rgb(1\,2\,3), color;
"Root":::c
  "Leaf": 1000.00:::c
"#;

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(
        source,
        ParseOptions {
            suppress_errors: true,
        },
    ))
    .expect("parse returns suppressed error")
    .expect("diagram detected");

    assert_eq!(parsed.meta.diagram_type, "error");

    let layout_options = LayoutOptions::default();
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let svg = render_layouted_svg(
        &out,
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    assert!(
        svg.contains(r#"aria-roledescription="error""#) && svg.contains("Syntax error in text"),
        "expected Mermaid parser-compatible error SVG for invalid classDef style token: {svg}"
    );
}
