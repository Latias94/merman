use futures::executor::block_on;
use merman_core::{Engine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::model::LayoutDiagram;
use merman_render::svg::{
    SvgDebugOptions, SvgRenderOptions, render_state_diagram_v2_debug_svg_with_debug,
};
use merman_render::{LayoutOptions, layout_parsed};
use std::io::Read;

fn main() {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .expect("read stdin");

    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(&input, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let session = RenderEnvironment::parity()
        .begin_session()
        .expect("begin render session");
    let layouted = layout_parsed(&parsed, &LayoutOptions::default(), &session).expect("layout ok");
    let LayoutDiagram::StateDiagramV2(layout) = layouted.layout else {
        panic!("expected StateDiagramV2 layout");
    };

    let debug = SvgDebugOptions {
        include_edge_id_labels: true,
        ..SvgDebugOptions::default()
    };
    let svg =
        render_state_diagram_v2_debug_svg_with_debug(&layout, &SvgRenderOptions::default(), &debug);
    print!("{svg}");
}
