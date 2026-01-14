use futures::executor::block_on;
use merman_core::{Engine, ParseOptions};
use merman_render::model::LayoutDiagram;
use merman_render::svg::{SvgRenderOptions, render_flowchart_v2_debug_svg};
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
    let LayoutDiagram::FlowchartV2(layout) = layouted.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let svg = render_flowchart_v2_debug_svg(&layout, &SvgRenderOptions::default());
    print!("{svg}");
}
