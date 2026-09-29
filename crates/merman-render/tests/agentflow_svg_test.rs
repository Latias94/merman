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

#[test]
fn agentflow_cyclic_container_references_preserve_visible_nodes() {
    for backend in ["elk", "dagre"] {
        for collapsed in [false, true] {
            let engine = Engine::new().with_site_config(merman_core::MermaidConfig::from_value(
                json!({"layout": backend}),
            ));
            let view = if collapsed {
                "@{ view: collapsed }"
            } else {
                ""
            };
            let source = format!(
                "agentflow-beta\nflow A{view}\n a --> B\nend\nflow B{view}\n b --> A\nend\n"
            );
            let parsed = engine
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap();
            let session = RenderEnvironment::deterministic().begin_session().unwrap();
            let artifact = family::prepare(parsed, &LayoutOptions::default(), session).unwrap();
            let projection = artifact.layout_json().unwrap();
            let nodes = projection["layout"]["AgentflowDiagram"]["nodes"]
                .as_array()
                .unwrap();
            assert!(!nodes.is_empty(), "{backend}, collapsed={collapsed}");
            if collapsed {
                assert_eq!(nodes.len(), 1, "{backend}");
                assert_eq!(nodes[0]["id"], "A", "{backend}");
            } else {
                assert_eq!(
                    projection["semantic"]["diagnostics"][0]["id"],
                    "CONTAINMENT_VIOLATION"
                );
            }
            let svg = artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap();
            assert!(
                svg.svg().contains(r#"data-color-id="color-7""#),
                "{backend}"
            );
        }
    }
}

#[test]
fn agentflow_authored_styles_links_and_animation_reach_the_svg() {
    let input = r#"agentflow-beta LR
classDef hot fill:#f1e2d3,stroke:#123abc
flow worker[Worker]
 a[Task]:::hot e@--> b[Done]
end
style b fill:#abc123
linkStyle default stroke:#314159
linkStyle 0 stroke:#271828,stroke-width:3px
e@{ animate: true, animation: fast }
click a href "https://example.com" "Open task" _blank
"#;
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).unwrap();
    assert!(
        artifact.layout_json().unwrap()["semantic"]
            .get("presentation")
            .is_none()
    );
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    assert!(document.descendants().any(|node| {
        node.attribute("class")
            .is_some_and(|classes| classes.split_whitespace().any(|class| class == "hot"))
    }));
    assert!(document.descendants().any(|node| {
        node.tag_name().name() == "a"
            && node.attributes().any(|attribute| {
                attribute.name() == "href" && attribute.value() == "https://example.com/"
            })
    }));
    assert!(document.descendants().any(|node| {
        node.attribute("style")
            .is_some_and(|style| style.contains("#abc123"))
    }));
    let edge = document
        .descendants()
        .find(|node| node.attribute("data-id") == Some("e"))
        .expect("explicit edge id");
    assert!(
        edge.attribute("style")
            .unwrap_or_default()
            .contains("#271828")
    );
    assert!(
        edge.attribute("class")
            .unwrap_or_default()
            .contains("edge-animation-fast")
    );
}

#[test]
fn agentflow_svg_uses_parser_assigned_dom_ordinals() {
    for (source, expected) in [
        (
            "agentflow-beta\nA\nA\nB --> C --> D\n",
            &[
                "agentflow-A-0",
                "agentflow-B-2",
                "agentflow-C-3",
                "agentflow-D-4",
            ][..],
        ),
        (
            "agentflow-beta\nA[Old]@{shape: task}\nstyle A fill:red\nconnector A[API]\nconnector A[Again]\nA@{instruction: call}\nB\n",
            &["agentflow-A-3", "agentflow-B-5"][..],
        ),
    ] {
        for backend in ["elk", "dagre"] {
            let engine = Engine::new().with_site_config(merman_core::MermaidConfig::from_value(
                json!({"layout": backend}),
            ));
            let parsed = engine
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .unwrap();
            let session = RenderEnvironment::deterministic().begin_session().unwrap();
            let artifact = family::prepare(parsed, &LayoutOptions::default(), session).unwrap();
            let rendered = artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            for id in expected {
                let id = format!("merman-{id}");
                assert!(
                    document
                        .descendants()
                        .any(|node| node.attribute("id") == Some(id.as_str())),
                    "{backend}: missing {id}"
                );
            }
        }
    }
}

#[test]
fn agentflow_hand_drawn_containers_keep_flow_group_paint() {
    for backend in ["elk", "dagre"] {
        let engine = Engine::new().with_site_config(merman_core::MermaidConfig::from_value(
            json!({"layout": backend, "look": "handDrawn", "themeVariables": {"flowContainerStroke": "#123456"}}),
        ));
        let parsed = engine
            .parse_diagram_for_render_model_sync(
                "agentflow-beta\nflow F[Flow]\n A --> B\nend\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let artifact = family::prepare(parsed, &LayoutOptions::default(), session).unwrap();
        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let container = document
            .descendants()
            .find(|node| {
                node.attribute("class").is_some_and(|class| {
                    class
                        .split_whitespace()
                        .any(|class| class == "flow-cluster")
                })
            })
            .unwrap();
        let shape = container.children().find(|node| node.is_element()).unwrap();
        let paths: Vec<_> = shape
            .children()
            .filter(|node| node.has_tag_name("path"))
            .collect();
        assert_eq!(
            paths.len(),
            1,
            "{backend}: transparent container must only paint its border"
        );
        let border = paths[0];
        assert_eq!(border.attribute("fill"), Some("none"));
        assert_eq!(border.attribute("stroke"), Some("#123456"));
        assert_eq!(border.attribute("stroke-width"), Some("0.75"));
    }
}

#[test]
fn agentflow_label_types_reach_html_and_svg_markdown_rendering() {
    let source = r#"agentflow-beta
A["`**bold**`"]
B["**plain**"]
C@{label: "`**literal**`", labelType: text}
"#;
    for html in [false, true] {
        let engine = Engine::new().with_site_config(merman_core::MermaidConfig::from_value(
            json!({"layout":"dagre", "htmlLabels":html}),
        ));
        let parsed = engine
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let artifact = family::prepare(parsed, &LayoutOptions::default(), session).unwrap();
        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let text_content = |node: roxmltree::Node<'_, '_>| {
            node.descendants()
                .filter(|node| node.is_text())
                .filter_map(|node| node.text())
                .collect::<String>()
        };
        let nodes: Vec<_> = document
            .descendants()
            .filter(|node| {
                node.has_tag_name("g")
                    && node.attribute("class").is_some_and(|classes| {
                        classes.split_whitespace().any(|class| class == "node")
                    })
            })
            .collect();
        assert_eq!(nodes.len(), 3);
        for expected in ["bold", "**plain**", "`**literal**`"] {
            let node = nodes
                .iter()
                .copied()
                .find(|node| text_content(*node) == expected)
                .unwrap_or_else(|| {
                    panic!(
                        "missing {expected:?}, htmlLabels={html}: {}",
                        rendered.svg()
                    )
                });
            let bold = node.descendants().any(|node| {
                if html {
                    node.has_tag_name("strong")
                } else {
                    node.has_tag_name("tspan") && node.attribute("font-weight") == Some("bold")
                }
            });
            assert_eq!(bold, expected == "bold", "{expected}, htmlLabels={html}");
        }
    }
}
