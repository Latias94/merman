#![cfg(feature = "layout-elk")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::{LayoutOptions, environment::RenderEnvironment, family};
use std::fmt::Write as _;

#[test]
fn public_flowchart_documentation_fixture_fits_interactive_elk_budget() {
    let source =
        include_str!("../../../fixtures/flowchart/upstream_docs_diagrams_flowchart_code_flow.mmd");
    assert_fits_interactive_elk_budget(source);
}

#[test]
fn flowchart_with_499_incoming_edges_fits_interactive_elk_budget() {
    let mut source = String::from("flowchart TD\n");
    for node in 1..500 {
        writeln!(&mut source, "N{node} --> N0").unwrap();
    }
    assert_fits_interactive_elk_budget(&source);
}

fn assert_fits_interactive_elk_budget(source: &str) {
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(
            serde_json::json!({"layout":"elk"}),
        ),
    );
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session).unwrap();
}

#[allow(dead_code)]
mod common;
