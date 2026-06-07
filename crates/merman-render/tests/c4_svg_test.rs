use merman_core::{Engine, ParseOptions};
use merman_render::model::LayoutDiagram;
use merman_render::{LayoutOptions, layout_parsed_render_layout_only};

fn deep_c4_boundary_chain(depth: usize) -> String {
    let mut input = String::from("C4Context\n");
    for level in 0..depth {
        input.push_str(&format!("Boundary(b{level}, \"B{level}\") {{\n"));
    }
    input.push_str("System(leaf, \"Leaf\")\n");
    for _ in 0..depth {
        input.push_str("}\n");
    }
    input
}

#[test]
fn c4_public_layout_handles_deep_boundary_chain() {
    const DEPTH: usize = 1500;
    let source = deep_c4_boundary_chain(DEPTH);

    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected");
    assert_eq!(parsed.meta.diagram_type, "c4");

    let layout = layout_parsed_render_layout_only(&parsed, &LayoutOptions::default())
        .expect("layout should not depend on recursive boundary traversal");
    let LayoutDiagram::C4Diagram(c4) = &layout else {
        panic!("expected C4Diagram layout");
    };

    assert_eq!(c4.boundaries.len(), DEPTH + 1);
    assert_eq!(c4.shapes.len(), 1);
    assert_eq!(c4.shapes[0].alias, "leaf");
}
