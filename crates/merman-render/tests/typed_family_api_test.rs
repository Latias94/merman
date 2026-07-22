use merman_core::{Engine, ParseOptions, ParsedDiagramRender, RenderSemanticModel};
use merman_render::environment::{RenderEnvironment, RenderSession, TextMeasurementPhase};
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::{
    LayoutOptions, c4::layout_c4_diagram_typed, xychart::layout_xychart_diagram_typed,
};

fn parse_for_render(source: &str) -> ParsedDiagramRender {
    Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected")
}

fn render_session() -> RenderSession {
    RenderEnvironment::deterministic().begin_session().unwrap()
}

#[test]
fn c4_exposes_its_typed_layout_entry() {
    let parsed = parse_for_render("C4Context\nSystem(api, \"API\")\n");
    let RenderSemanticModel::C4(model) = parsed.model() else {
        panic!("expected C4 render model");
    };
    let session = render_session();
    let measurer = session.text_measurer(TextMeasurementPhase::Layout);

    let layout = layout_c4_diagram_typed(
        model,
        parsed.metadata().effective_config.as_value(),
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
    let RenderSemanticModel::XyChart(model) = parsed.model() else {
        panic!("expected XYChart render model");
    };
    let session = render_session();
    let measurer = session.text_measurer(TextMeasurementPhase::Layout);

    let layout = layout_xychart_diagram_typed(
        model,
        parsed.metadata().effective_config.as_value(),
        &measurer,
    )
    .expect("typed XYChart layout");

    assert!(layout.width > 0.0);
    assert!(!layout.drawables.is_empty());
}

#[cfg(feature = "layout-cytoscape")]
#[test]
fn architecture_prepared_artifact_renders_the_typed_family() {
    let parsed = parse_for_render("architecture-beta\n  service api(server)[API]\n");
    let session = render_session();
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare Architecture artifact");
    let options = SvgRenderOptions {
        diagram_id: Some("typed-architecture".to_string()),
        ..Default::default()
    };

    let svg = artifact
        .render_svg(&options, &SvgDebugOptions::default())
        .expect("render Architecture artifact");

    assert!(svg.svg().contains(r#"id="typed-architecture-service-api""#));
}

#[cfg(not(feature = "layout-cytoscape"))]
#[test]
fn architecture_reports_the_missing_cytoscape_layout_capability() {
    let parsed = match Engine::new().parse_diagram_for_render_model_with_type_sync(
        "architecture",
        "architecture-beta\n  service api(server)[API]\n",
        ParseOptions::strict(),
    ) {
        Ok(Some(parsed)) => parsed,
        Err(merman_core::Error::UnsupportedDiagram { .. }) => return,
        result => panic!("unexpected Architecture parse result: {result:?}"),
    };
    let error = match family::prepare(parsed, &LayoutOptions::default(), render_session()) {
        Err(error) => error,
        Ok(_) => panic!("Architecture must be rejected without layout-cytoscape"),
    };

    assert_eq!(
        error.to_string(),
        "compiled renderer lacks capability `layout-cytoscape` required by diagram `architecture`"
    );
}

#[cfg(not(feature = "layout-cytoscape"))]
#[test]
fn mindmap_tidy_tree_renders_without_cytoscape_layout() {
    let parsed = match Engine::new().parse_diagram_for_render_model_with_type_sync(
        "mindmap",
        "---\nconfig:\n  layout: tidy-tree\n---\nmindmap\n  Root\n    Child\n",
        ParseOptions::strict(),
    ) {
        Ok(Some(parsed)) => parsed,
        Err(merman_core::Error::UnsupportedDiagram { .. }) => return,
        result => panic!("unexpected Mindmap parse result: {result:?}"),
    };
    let artifact = family::prepare(parsed, &LayoutOptions::default(), render_session())
        .expect("tidy-tree Mindmap must not require layout-cytoscape");
    assert_eq!(artifact.family_kind().as_str(), "mindmap");
}

#[cfg(not(feature = "layout-elk"))]
#[test]
fn elk_flowchart_reports_the_missing_layout_capability() {
    let parsed = parse_for_render(
        "---\nconfig:\n  layout: elk\n---\nflowchart TD\n  start[Start] --> finish[Finish]\n",
    );
    let error = match family::prepare(parsed, &LayoutOptions::default(), render_session()) {
        Err(error) => error,
        Ok(_) => panic!("ELK Flowchart must be rejected without layout-elk"),
    };

    assert_eq!(
        error.to_string(),
        "compiled renderer lacks capability `layout-elk` required by diagram `flowchart-v2`"
    );
}

#[test]
fn state_prepared_artifact_renders_the_typed_family() {
    let parsed = parse_for_render("stateDiagram-v2\n[*] --> Active\n");
    let session = render_session();
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare State artifact");
    let options = SvgRenderOptions {
        diagram_id: Some("typed-state".to_string()),
        ..Default::default()
    };

    let svg = artifact
        .render_svg(&options, &SvgDebugOptions::default())
        .expect("render State artifact");

    assert!(
        svg.svg().contains(r#"id="typed-state-state-Active-0""#),
        "{}",
        svg.svg()
    );
}
