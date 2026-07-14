use merman_core::{Engine, ParseOptions, RenderSemanticModel};
use merman_render::LayoutOptions;
use merman_render::c4::layout_c4_diagram_typed;
use merman_render::environment::{RenderEnvironment, TextMeasurementPhase};

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
    let session = RenderEnvironment::parity().begin_session().unwrap();
    const DEPTH: usize = 1500;
    let source = deep_c4_boundary_chain(DEPTH);

    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected");
    assert_eq!(parsed.meta.diagram_type, "c4");

    let RenderSemanticModel::C4(model) = &parsed.model else {
        panic!("expected C4 render model");
    };
    let options = LayoutOptions::default();
    let measurer = session.text_measurer(TextMeasurementPhase::Layout);
    let c4 = layout_c4_diagram_typed(
        model,
        parsed.meta.effective_config.as_value(),
        &measurer,
        options.viewport_width,
        options.viewport_height,
    )
    .expect("layout should not depend on recursive boundary traversal");

    assert_eq!(c4.boundaries.len(), DEPTH + 1);
    assert_eq!(c4.shapes.len(), 1);
    assert_eq!(c4.shapes[0].alias, "leaf");
}
