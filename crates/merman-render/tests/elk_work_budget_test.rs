#![cfg(feature = "layout-elk")]

use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::{LayoutOptions, environment::RenderEnvironment, family};

#[test]
fn public_flowchart_documentation_fixture_fits_interactive_elk_budget() {
    let source =
        include_str!("../../../fixtures/flowchart/upstream_docs_diagrams_flowchart_code_flow.mmd");
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(
        serde_json::json!({"layout":"elk"}),
    ));
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session).unwrap();
}
