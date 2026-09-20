use merman_core::{Engine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::json;

#[test]
fn agentflow_domain_model_projects_through_shared_svg_layout() {
    let input = r#"agentflow-beta LR
flow planner[Planner]
  prompt[Prompt]@{ shape: input, contract: "question" }
  tool[Search]@{ shape: tool }
  prompt --> tool
end
"#;
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("parse agentflow")
        .expect("agentflow render model");
    assert_eq!(parsed.metadata().diagram_type, "agentflow");
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).unwrap();
    assert_eq!(artifact.family_kind().as_str(), "agentflow");
    let layout = artifact.layout_json().unwrap();
    assert_eq!(layout["semantic"]["type"], "agentflow");
    assert_eq!(layout["semantic"]["vertices"][0]["vertexKind"], "input");
    assert_eq!(layout["semantic"]["edges"][0]["edgeSemantic"], "sequence");
    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("agentflow-test".to_string()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .unwrap();
    assert!(
        rendered
            .svg()
            .contains(r#"aria-roledescription="agentflow""#)
    );
    assert!(rendered.svg().contains("prompt"));
    assert!(rendered.svg().contains("tool"));
}

#[test]
fn agentflow_connectors_and_collapsed_flows_keep_visible_routes() {
    for backend in ["elk", "dagre"] {
        let engine = Engine::new().with_site_config(merman_core::MermaidConfig::from_value(
            json!({"layout": backend}),
        ));
        let parsed = engine.parse_diagram_for_render_model_sync(
            "agentflow-beta LR\nflow worker[Worker]\n甲[Input]@{shape: input}\n乙[Task]\n甲 --> 乙\n甲 --> 甲\nend\nworker@{view: collapsed}\nconnector api[API]\n甲 --> api",
            ParseOptions::strict(),
        ).unwrap().unwrap();
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let artifact = family::prepare(parsed, &LayoutOptions::default(), session).unwrap();
        let projection = artifact.layout_json().unwrap();
        let nodes = projection["layout"]["AgentflowDiagram"]["nodes"]
            .as_array()
            .unwrap();
        assert!(
            nodes.iter().any(|node| node["id"] == "worker"),
            "{backend}: missing collapsed flow"
        );
        assert!(
            nodes.iter().any(|node| node["id"] == "api"),
            "{backend}: missing connector"
        );
        assert!(
            !nodes
                .iter()
                .any(|node| node["id"] == "甲" || node["id"] == "乙"),
            "{backend}: hidden descendants escaped collapse"
        );
        let edges = projection["layout"]["AgentflowDiagram"]["edges"]
            .as_array()
            .unwrap();
        assert!(
            edges
                .iter()
                .any(|edge| edge["from"] == "worker" && edge["to"] == "worker"),
            "{backend}: authored self-loop must survive collapse: {edges:?}"
        );
        assert!(
            edges
                .iter()
                .any(|edge| edge["from"] == "worker" && edge["to"] == "api"),
            "{backend}: cross-boundary edge must be redirected: {edges:?}"
        );
        let svg = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        assert!(svg.svg().contains("af-kind-connector"));
        assert!(svg.svg().contains(r#"data-color-id="color-7""#));
    }
}
