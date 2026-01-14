use merman_core::{Engine, ParseOptions};
use merman_render::model::LayoutDiagram;
use merman_render::svg::{SvgRenderOptions, render_state_diagram_v2_debug_svg};
use merman_render::{LayoutOptions, layout_parsed};

#[test]
fn state_debug_svg_includes_cluster_positioning_metadata() {
    let text = "stateDiagram-v2\n[*] --> Active\nstate Active {\n  direction TB\n  Idle --> Idle: LOG\n}\n";
    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let LayoutDiagram::StateDiagramV2(layout) = out.layout else {
        panic!("expected StateDiagramV2 layout");
    };

    let opts = SvgRenderOptions::default();
    let svg = render_state_diagram_v2_debug_svg(&layout, &opts);

    assert!(svg.contains(r#"id="cluster-Active""#));
    assert!(svg.contains(r#"data-diff="#));
    assert!(svg.contains(r#"data-offset-y="#));
}
