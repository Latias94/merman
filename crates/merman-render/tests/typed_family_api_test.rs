use merman_core::{Engine, ParseOptions, ParsedDiagramRender, RenderSemanticModel};
#[cfg(feature = "cytoscape-layout")]
use merman_render::architecture::{
    layout_architecture_diagram_typed, render_architecture_diagram_typed_with_debug,
};
use merman_render::environment::{RenderEnvironment, RenderSession, TextMeasurementPhase};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::{
    c4::layout_c4_diagram_typed,
    state::{layout_state_diagram_v2_typed, render_state_diagram_v2_typed_with_debug},
    xychart::layout_xychart_diagram_typed,
};

fn parse_for_render(source: &str) -> ParsedDiagramRender {
    Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected")
}

fn render_session() -> RenderSession {
    RenderEnvironment::parity().begin_session().unwrap()
}

#[test]
fn c4_exposes_its_typed_layout_entry() {
    let parsed = parse_for_render("C4Context\nSystem(api, \"API\")\n");
    let RenderSemanticModel::C4(model) = &parsed.model else {
        panic!("expected C4 render model");
    };
    let session = render_session();
    let measurer = session.text_measurer(TextMeasurementPhase::Layout);

    let layout = layout_c4_diagram_typed(
        model,
        parsed.meta.effective_config.as_value(),
        &measurer,
        800.0,
        600.0,
    )
    .expect("typed C4 layout");

    assert_eq!(layout.shapes.len(), 1);
    assert_eq!(layout.shapes[0].alias, "api");
}

#[test]
fn xychart_exposes_its_typed_layout_entry() {
    let parsed = parse_for_render("xychart-beta\n  x-axis [A]\n  y-axis 0 --> 10\n  bar [7]\n");
    let RenderSemanticModel::XyChart(model) = &parsed.model else {
        panic!("expected XYChart render model");
    };
    let session = render_session();
    let measurer = session.text_measurer(TextMeasurementPhase::Layout);

    let layout =
        layout_xychart_diagram_typed(model, parsed.meta.effective_config.as_value(), &measurer)
            .expect("typed XYChart layout");

    assert!(layout.width > 0.0);
    assert!(!layout.drawables.is_empty());
}

#[cfg(feature = "cytoscape-layout")]
#[test]
fn architecture_exposes_a_typed_model_svg_entry() {
    let parsed = parse_for_render("architecture-beta\n  service api(server)[API]\n");
    let RenderSemanticModel::Architecture(model) = &parsed.model else {
        panic!("expected Architecture render model");
    };
    let session = render_session();
    let measurer = session.text_measurer(TextMeasurementPhase::Layout);
    let layout = layout_architecture_diagram_typed(
        model,
        parsed.meta.effective_config.as_value(),
        &measurer,
        false,
        session.seed().seed().get(),
    )
    .expect("typed Architecture layout");
    let options = SvgRenderOptions {
        diagram_id: Some("typed-architecture".to_string()),
        ..Default::default()
    };

    let svg = render_architecture_diagram_typed_with_debug(
        &layout,
        model,
        &parsed.meta.effective_config,
        &session,
        &options,
        &SvgDebugOptions::default(),
    )
    .expect("typed Architecture SVG");

    assert!(svg.contains(r#"id="typed-architecture-service-api""#));
}

#[test]
fn state_exposes_a_typed_model_svg_entry() {
    let parsed = parse_for_render("stateDiagram-v2\n[*] --> Active\n");
    let RenderSemanticModel::State(model) = &parsed.model else {
        panic!("expected State render model");
    };
    let session = render_session();
    let measurer = session.text_measurer(TextMeasurementPhase::Layout);
    let layout =
        layout_state_diagram_v2_typed(model, parsed.meta.effective_config.as_value(), &measurer)
            .expect("typed State layout");
    let options = SvgRenderOptions {
        diagram_id: Some("typed-state".to_string()),
        ..Default::default()
    };

    let svg = render_state_diagram_v2_typed_with_debug(
        &layout,
        model,
        parsed.meta.effective_config.as_value(),
        parsed.meta.title.as_deref(),
        &session,
        &options,
        &SvgDebugOptions::default(),
    )
    .expect("typed State SVG");

    assert!(svg.contains(r#"id="typed-state-state-Active-0""#), "{svg}");
}
