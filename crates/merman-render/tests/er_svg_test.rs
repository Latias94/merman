use merman_core::{Engine, ParseOptions};
use merman_render::model::LayoutDiagram;
use merman_render::svg::{SvgRenderOptions, render_er_diagram_debug_svg};
use merman_render::{LayoutOptions, layout_parsed};
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

#[test]
fn er_debug_svg_renders() {
    let path = workspace_root()
        .join("fixtures")
        .join("er")
        .join("basic.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let LayoutDiagram::ErDiagram(layout) = out.layout else {
        panic!("expected ErDiagram layout");
    };

    let svg = render_er_diagram_debug_svg(&layout, &SvgRenderOptions::default());
    assert!(svg.contains("<svg"));
    assert!(svg.contains("edge-label-box") || svg.contains("polyline"));
}
