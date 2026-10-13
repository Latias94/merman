use merman_core::{Engine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn render_swimlane(source: &str, diagram_id: &str) -> String {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse swimlane")
        .expect("detect swimlane");
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).unwrap();
    artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some(diagram_id.to_string()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .unwrap()
        .svg()
        .to_string()
}

#[test]
fn explicit_swimlane_cluster_ids_are_scoped_to_each_diagram() {
    let source = "swimlane-beta LR\nsubgraph Lane\nA --> B\nend\n";
    for diagram_id in ["first", "second"] {
        let svg = render_swimlane(source, diagram_id);
        assert!(svg.contains(&format!(
            "id=\"{diagram_id}-Lane\" data-id=\"Lane\" data-et=\"cluster\""
        )));
    }
}

#[test]
fn synthetic_swimlane_cluster_keeps_upstream_dom_id_fallback() {
    // Mermaid synthesizes this group after render.ts prefixes existing DOM IDs.
    let svg = render_swimlane("swimlane-beta LR\nA --> B\n", "synthetic");
    assert!(svg.contains(
        "id=\"__swimlane_default__\" data-id=\"__swimlane_default__\" data-et=\"cluster\""
    ));
}

#[test]
fn explicitly_declared_default_swimlane_cluster_is_scoped() {
    let source = "swimlane-beta LR\nsubgraph __swimlane_default__[Named lane]\nA --> B\nend\n";
    let svg = render_swimlane(source, "declared");
    assert!(svg.contains(
        "id=\"declared-__swimlane_default__\" data-id=\"__swimlane_default__\" data-et=\"cluster\""
    ));
}
