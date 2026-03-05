use merman_core::{Engine, ParseOptions};
use merman_render::model::LayoutDiagram;
use merman_render::svg::{SvgRenderOptions, render_treemap_diagram_svg};
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

fn render_treemap_svg_from_fixture(fixture: &str) -> String {
    let path = workspace_root()
        .join("fixtures")
        .join("treemap")
        .join(fixture);
    let text = std::fs::read_to_string(&path).expect("fixture");

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::TreemapDiagram(layout) = &out.layout else {
        panic!("expected TreemapDiagram layout");
    };

    render_treemap_diagram_svg(
        layout,
        &out.semantic,
        &out.meta.effective_config,
        &SvgRenderOptions::default(),
    )
    .expect("render svg")
}

#[test]
fn treemap_leaf_label_font_size_matches_mermaid_cli_baselines() {
    let svg = render_treemap_svg_from_fixture("upstream_treemap_docs_basic_spec.mmd");

    let needle = ">Item A1</text>";
    let end = svg.find(needle).expect("expected Item A1 label");
    let start = svg[..end].rfind("<text").expect("expected label tag start");
    let tag = &svg[start..(end + needle.len())];

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
