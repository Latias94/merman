use futures::executor::block_on;
use merman_core::{Engine, ParseOptions};
use merman_render::model::LayoutDiagram;
use merman_render::svg::{SvgRenderOptions, render_state_diagram_v2_debug_svg};
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

    let layouted = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let LayoutDiagram::StateDiagramV2(layout) = layouted.layout else {
        panic!("expected StateDiagramV2 layout");
    };

    let mut opts = SvgRenderOptions::default();
    opts.include_edge_id_labels = true;
    let svg = render_state_diagram_v2_debug_svg(&layout, &opts);
    print!("{svg}");
}
