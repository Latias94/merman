use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramEffectSet, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec,
    EffectBinding, EffectGraph, EffectInput, EffectPrimitive, FontStack, OrdinalPalette, Specified,
    TextStylePatch, ThemeColorValue, ThemePortabilityRequirement, ThemeRule, ThemeRuleSet,
    ThemeStylePatch, ThemeTarget, ThemeTextStyle, TypographySpec,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::json;

#[test]
fn agentflow_eager_hierarchy_markers_match_upstream_definitions() {
    let fixtures = [
        (
            include_str!("../../../fixtures/agentflow/basic.mmd"),
            include_str!("../../../fixtures/upstream-svgs/agentflow/basic.svg"),
        ),
        (
            include_str!("../../../fixtures/agentflow/container-and-shapes.mmd"),
            include_str!("../../../fixtures/upstream-svgs/agentflow/container-and-shapes.svg"),
        ),
    ];
    for backend in ["elk", "dagre"] {
        for (source, upstream) in fixtures {
            let (_, svg) = render_agentflow_config_probe(source, json!({"layout": backend}), false);
            let actual = roxmltree::Document::parse(&svg).unwrap();
            let expected = roxmltree::Document::parse(upstream).unwrap();
            let mut ids = std::collections::BTreeSet::new();
            for id in actual.descendants().filter_map(|node| node.attribute("id")) {
                assert!(ids.insert(id), "{backend}: duplicate SVG id {id}");
            }
            let markers = actual
                .descendants()
                .filter(|node| node.has_tag_name("marker"))
                .collect::<Vec<_>>();
            assert_eq!(markers.len(), 14, "{backend}: eager marker registry");
            let scope = markers[0]
                .attribute("id")
                .unwrap()
                .strip_suffix("-pointEnd")
                .unwrap();
            for suffix in ["hierarchyEnd", "hierarchyStart"] {
                let marker_id = format!("{scope}-{suffix}");
                let marker = markers
                    .iter()
                    .find(|node| node.attribute("id") == Some(marker_id.as_str()))
                    .expect("hierarchy marker shares the document marker namespace");
                let reference = expected
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("marker")
                            && node.attribute("id").is_some_and(|id| id.ends_with(suffix))
                    })
                    .unwrap();
                for attribute in reference.attributes().filter(|attr| attr.name() != "id") {
                    assert_eq!(marker.attribute(attribute.name()), Some(attribute.value()));
                }
                let path = marker.children().find(|node| node.is_element()).unwrap();
                let reference_path = reference.children().find(|node| node.is_element()).unwrap();
                assert!(path.has_tag_name("path"));
                for attribute in reference_path.attributes() {
                    assert_eq!(path.attribute(attribute.name()), Some(attribute.value()));
                }
            }
        }
    }
}

#[test]
fn agentflow_domain_model_projects_through_shared_svg_layout() {
    let input = r#"agentflow-beta LR
flow planner[Planner]
  prompt[Prompt]@{ shape: input, contract: "question" }
  tool[Search]@{ shape: tool }
  prompt --> tool
end
"#;
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("parse agentflow")
        .expect("agentflow render model")
        .into_presentation();
    assert_eq!(parsed.metadata().diagram_type, "agentflow");
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
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
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(
                json!({"layout": backend}),
            ),
        );
        let parsed = engine.parse_diagram_for_render_model_sync(
            "agentflow-beta LR\nflow worker[Worker]\n甲[Input]@{shape: input}\n乙[Task]\n甲 --> 乙\n甲 --> 甲\nend\nworker@{view: collapsed}\nconnector api[API]\n甲 --> api",
            ParseOptions::strict(),
        ).unwrap().unwrap().into_presentation();
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
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
        if backend == "elk" && cfg!(feature = "layout-elk") {
            assert!(
                edges
                    .iter()
                    .any(|edge| edge["from"] == "worker" && edge["to"] == "worker"),
                "{backend}: authored self-loop must survive collapse: {edges:?}"
            );
        } else {
            // Mermaid 12.2 Dagre merges self-loops only for its fixed diagram-type allowlist.
            // Agentflow retains all three helper segments after collapsed-node projection.
            for (id, from, to) in [
                ("worker-cyclic-special-1", "worker", "worker---worker---1"),
                (
                    "worker-cyclic-special-mid",
                    "worker---worker---1",
                    "worker---worker---2",
                ),
                ("worker-cyclic-special-2", "worker---worker---2", "worker"),
            ] {
                assert!(
                    edges
                        .iter()
                        .any(|edge| edge["id"] == id && edge["from"] == from && edge["to"] == to),
                    "{backend}: missing Dagre helper segment {id}: {edges:?}"
                );
            }
        }
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
fn agentflow_nested_collapse_keeps_pinned_dagre_segments_and_label_dom_order() {
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct EdgeSignature {
        id: String,
        start_marker: bool,
        end_marker: bool,
        commands: String,
    }
    fn signature(svg: &str, canonical_ids: bool) -> (Vec<EdgeSignature>, Vec<String>) {
        let document = roxmltree::Document::parse(svg).unwrap();
        let paths = document
            .descendants()
            .filter(|node| node.has_tag_name("path") && node.attribute("data-edge") == Some("true"))
            .map(|node| EdgeSignature {
                id: node.attribute("data-id").unwrap().to_owned(),
                start_marker: node.attribute("marker-start").is_some(),
                end_marker: node.attribute("marker-end").is_some(),
                commands: node
                    .attribute("d")
                    .unwrap()
                    .chars()
                    .filter(|ch| ch.is_ascii_alphabetic())
                    .collect(),
            })
            .collect();
        let label_group = document
            .descendants()
            .find(|node| node.has_tag_name("g") && node.attribute("class") == Some("edgeLabels"))
            .unwrap();
        let labels = label_group
            .children()
            .filter(roxmltree::Node::is_element)
            .map(|node| {
                if node.attribute("class") == Some("edgeLabel") {
                    node.children()
                        .find(|node| node.attribute("data-id").is_some())
                        .unwrap()
                        .attribute("data-id")
                        .unwrap()
                        .to_owned()
                } else {
                    let background = node.children().find(roxmltree::Node::is_element).unwrap();
                    assert!(background.has_tag_name("rect"));
                    assert_eq!(background.attribute("class"), Some("background"));
                    "background".to_owned()
                }
            })
            .collect();
        let mut helper_dom_ids = std::collections::HashSet::new();
        for helper_id in ["inner---inner---1", "inner---inner---2"] {
            let mut matches = document.descendants().filter(|node| {
                if canonical_ids {
                    node.attribute("data-id") == Some(helper_id)
                        && node.attribute("data-et") == Some("edge-label")
                } else {
                    node.attribute("id") == Some(helper_id)
                }
            });
            let helper = matches.next().expect("self-loop helper occurrence");
            assert!(
                matches.next().is_none(),
                "duplicate self-loop helper occurrence"
            );
            let dom_id = helper.attribute("id").unwrap();
            assert!(helper_dom_ids.insert(dom_id));
            if canonical_ids {
                let prefix = format!(
                    "{}-merman-flowchart-document-synthetic-label-",
                    document.root_element().attribute("id").unwrap(),
                );
                let ordinal = dom_id
                    .strip_prefix(&prefix)
                    .expect("helper DOM identity belongs to its canonical document owner");
                assert!(!ordinal.is_empty() && ordinal.bytes().all(|byte| byte.is_ascii_digit()));
            }
            assert_eq!(helper.attribute("class"), Some("label edgeLabel"));
            let rect = helper.children().find(roxmltree::Node::is_element).unwrap();
            assert!(rect.has_tag_name("rect"));
            assert_eq!(rect.attribute("width"), Some("0.1"));
            assert_eq!(rect.attribute("height"), Some("0.1"));
        }
        if canonical_ids {
            let ids = document
                .descendants()
                .filter_map(|node| node.attribute("id"))
                .collect::<Vec<_>>();
            assert_eq!(
                ids.len(),
                ids.iter()
                    .copied()
                    .collect::<std::collections::HashSet<_>>()
                    .len(),
                "duplicate document DOM IDs"
            );
        }
        (paths, labels)
    }
    let input = include_str!("../../../fixtures/agentflow/local_nested_collapse_routes.mmd");
    let upstream =
        include_str!("../../../fixtures/upstream-svgs/agentflow/local_nested_collapse_routes.svg");
    let (layout, svg) = render_agentflow_config_probe(input, json!({}), false);
    let (paths, labels) = signature(&svg, true);
    assert_eq!((paths.clone(), labels.clone()), signature(upstream, false));
    assert_eq!(
        paths
            .iter()
            .map(|path| path.id.as_str())
            .collect::<Vec<_>>(),
        [
            "inner-cyclic-special-1",
            "inner-cyclic-special-mid",
            "inner-cyclic-special-2",
            "enter",
            "exit",
        ]
    );
    assert!(paths.iter().all(|path| !path.start_marker));
    assert_eq!(
        paths.iter().map(|path| path.end_marker).collect::<Vec<_>>(),
        [false, false, true, true, true]
    );
    assert_eq!(
        labels.len(),
        10,
        "each unmerged empty edge inserts one wrapper and background"
    );
    let graph = &layout["AgentflowDiagram"];
    assert_eq!(graph["edges"].as_array().unwrap().len(), 5);
    let nodes = graph["nodes"].as_array().unwrap();
    assert!(!nodes.iter().any(|node| node["id"] == "task"));
    assert!(
        nodes
            .iter()
            .any(|node| node["id"] == "inner" && node["is_cluster"] == false)
    );
    assert!(
        nodes
            .iter()
            .any(|node| node["id"] == "outer" && node["is_cluster"] == true)
    );
}

#[test]
fn agentflow_parallel_dagre_loops_replace_metadata_without_moving_the_first_slots() {
    let source = "agentflow-beta LR\nA[Task]\nB[Next]\nA first@-->|first loop| A\nA ordinary@--> B\nA second@--x|second loop| A\n";
    let (layout, svg) = render_agentflow_config_probe(
        source,
        json!({"layout": "dagre", "htmlLabels": false}),
        false,
    );
    let edges = layout["AgentflowDiagram"]["edges"].as_array().unwrap();
    let expected = [
        "A-cyclic-special-1",
        "A-cyclic-special-mid",
        "A-cyclic-special-2",
        "ordinary",
    ];
    assert_eq!(
        edges
            .iter()
            .map(|edge| edge["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(edges[0]["from"], "A");
    assert_eq!(edges[0]["to"], "A---A---1");
    assert_eq!(edges[1]["from"], "A---A---1");
    assert_eq!(edges[1]["to"], "A---A---2");
    assert_eq!(edges[2]["from"], "A---A---2");
    assert_eq!(edges[2]["to"], "A");
    assert!(edges[1]["label"]["width"].as_f64().unwrap() > 0.0);
    let document = roxmltree::Document::parse(&svg).unwrap();
    let paths: Vec<_> = document
        .descendants()
        .filter(|node| node.has_tag_name("path") && node.attribute("data-edge") == Some("true"))
        .collect();
    assert_eq!(
        paths
            .iter()
            .map(|path| path.attribute("data-id").unwrap())
            .collect::<Vec<_>>(),
        expected
    );
    assert!(
        paths[..3]
            .iter()
            .all(|path| path.attribute("marker-start").is_none())
    );
    assert!(
        paths[..2]
            .iter()
            .all(|path| path.attribute("marker-end").is_none())
    );
    assert!(
        paths[2]
            .attribute("marker-end")
            .unwrap()
            .contains("crossEnd")
    );
    assert!(
        paths[3]
            .attribute("marker-end")
            .unwrap()
            .contains("pointEnd")
    );
    let labels = document
        .descendants()
        .find(|node| node.attribute("class") == Some("edgeLabels"))
        .unwrap();
    assert_eq!(
        labels
            .descendants()
            .filter_map(|node| node.attribute("data-id"))
            .collect::<Vec<_>>(),
        expected
    );
    let middle = labels
        .descendants()
        .find(|node| node.attribute("data-id") == Some("A-cyclic-special-mid"))
        .unwrap();
    let visible: String = middle
        .descendants()
        .filter(roxmltree::Node::is_text)
        .filter_map(|node| node.text())
        .collect();
    assert_eq!(
        visible, "second loop",
        "last loop metadata owns the existing helper slots"
    );
    for id in ["A-cyclic-special-1", "A-cyclic-special-2"] {
        let helper_label = labels
            .descendants()
            .find(|node| node.attribute("data-id") == Some(id))
            .unwrap();
        let visible: String = helper_label
            .descendants()
            .filter(roxmltree::Node::is_text)
            .filter_map(|node| node.text())
            .collect();
        assert!(
            visible.is_empty(),
            "only the middle self-loop segment owns its label"
        );
    }
    assert!(!svg.contains("first loop"));
}

#[test]
fn agentflow_cyclic_container_references_preserve_visible_nodes() {
    for backend in ["elk", "dagre"] {
        for collapsed in [false, true] {
            let engine = MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    json!({"layout": backend}),
                ),
            );
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
                .unwrap()
                .into_presentation();
            let session = RenderEnvironment::deterministic().begin_session().unwrap();
            let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
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
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
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
            &[("A", 0), ("B", 2), ("C", 3), ("D", 4)][..],
        ),
        (
            "agentflow-beta\nA[Old]@{shape: task}\nstyle A fill:red\nconnector A[API]\nconnector A[Again]\nA@{instruction: call}\nB\n",
            &[("A", 3), ("B", 5)][..],
        ),
    ] {
        for backend in ["elk", "dagre"] {
            let engine = MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    json!({"layout": backend}),
                ),
            );
            let parsed = engine
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .unwrap()
                .into_presentation();
            let session = RenderEnvironment::deterministic().begin_session().unwrap();
            let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
            let rendered = artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            for (id, ordinal) in expected {
                let node = document
                    .descendants()
                    .find(|node| {
                        node.attribute("data-id") == Some(*id)
                            && node.attribute("data-et") == Some("node")
                    })
                    .unwrap_or_else(|| panic!("{backend}: missing {id}"));
                assert!(
                    node.attribute("id")
                        .is_some_and(|value| value.ends_with(&format!("-node-{ordinal}"))),
                    "{backend}: {id} must retain parser ordinal {ordinal}: {node:?}"
                );
            }
        }
    }
}

#[test]
fn agentflow_hand_drawn_containers_keep_flow_group_paint() {
    for backend in ["elk", "dagre"] {
        let engine = MermaidEngine::new().with_site_config(merman_core::presentation::SourcePresentationConfig::from_value(
            json!({"layout": backend, "look": "handDrawn", "themeVariables": {"flowContainerStroke": "#123456"}}),
        ));
        let parsed = engine
            .parse_diagram_for_render_model_sync(
                "agentflow-beta\nflow F[Flow]\n A --> B\nend\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap()
            .into_presentation();
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
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
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(
                json!({"layout":"dagre", "htmlLabels":html}),
            ),
        );
        let parsed = engine
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
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

#[test]
fn agentflow_measurement_config_wins_over_flowchart_defaults_and_overrides() {
    for backend in ["elk", "dagre"] {
        for html_labels in [false, true] {
            for flowchart in [json!({}), json!({"minNodeWidth": 10})] {
                let engine = MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(json!({
                        "layout": backend,
                        "htmlLabels": html_labels,
                        "agentflow": { "minNodeWidth": 300 },
                        "flowchart": flowchart
                    })),
                );
                let parsed = engine
                    .parse_diagram_for_render_model_sync(
                        "agentflow-beta\nA[Hi]\n",
                        ParseOptions::strict(),
                    )
                    .unwrap()
                    .unwrap()
                    .into_presentation();
                let session = RenderEnvironment::deterministic().begin_session().unwrap();
                let artifact =
                    family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
                let projection = artifact.layout_json().unwrap();
                let width = projection["layout"]["AgentflowDiagram"]["nodes"][0]["width"]
                    .as_f64()
                    .unwrap();
                assert!(width >= 300.0, "{backend}, html={html_labels}: {width}");
                artifact
                    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                    .unwrap();
            }
        }

        let mut dimensions = Vec::new();
        for wrapping_width in [40, 200] {
            let engine = MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(json!({
                    "layout": backend,
                    "htmlLabels": false,
                    "agentflow": { "minNodeWidth": 0, "wrappingWidth": wrapping_width },
                    "flowchart": { "minNodeWidth": 0, "wrappingWidth": 200 }
                })),
            );
            let parsed = engine
                .parse_diagram_for_render_model_sync(
                    "agentflow-beta\nA[one two three four five six seven eight nine ten]\n",
                    ParseOptions::strict(),
                )
                .unwrap()
                .unwrap()
                .into_presentation();
            let session = RenderEnvironment::deterministic().begin_session().unwrap();
            let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
            let projection = artifact.layout_json().unwrap();
            let node = &projection["layout"]["AgentflowDiagram"]["nodes"][0];
            dimensions.push((
                node["width"].as_f64().unwrap(),
                node["height"].as_f64().unwrap(),
            ));
        }
        assert!(
            dimensions[0].0 < dimensions[1].0,
            "{backend}: {dimensions:?}"
        );
        assert!(
            dimensions[0].1 > dimensions[1].1,
            "{backend}: {dimensions:?}"
        );
    }
}
fn render_agentflow_config_probe(
    source: &str,
    config: serde_json::Value,
    frontmatter: bool,
) -> (serde_json::Value, String) {
    let (engine, source) = if frontmatter {
        (
            MermaidEngine::new(),
            format!("---\nconfig: {config}\n---\n{source}"),
        )
    } else {
        (
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(config),
            ),
            source.to_owned(),
        )
    };
    let parsed = engine
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
    let layout = artifact.layout_json().unwrap()["layout"].clone();
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap()
        .svg()
        .to_owned();
    (layout, svg)
}

#[test]
fn agentflow_viewport_and_spacing_ignore_unrelated_flowchart_settings() {
    for backend in ["dagre", "elk"] {
        for frontmatter in [false, true] {
            let source = "agentflow-beta\nA-->B\nA-->C";
            let base =
                render_agentflow_config_probe(source, json!({"layout": backend}), frontmatter);
            let polluted = render_agentflow_config_probe(
                source,
                json!({
                    "layout": backend,
                    "flowchart": {
                        "nodeSpacing": 200, "rankSpacing": 200,
                        "diagramPadding": 60, "useMaxWidth": false, "titleTopMargin": 100
                    }
                }),
                frontmatter,
            );
            assert_eq!(base, polluted, "{backend}, frontmatter={frontmatter}");
            let family = render_agentflow_config_probe(
                source,
                json!({
                    "layout": backend,
                    "agentflow": {"diagramPadding": 60, "useMaxWidth": false}
                }),
                frontmatter,
            );
            assert_eq!(base.0, family.0, "viewport options cannot move nodes");
            let base_doc = roxmltree::Document::parse(&base.1).unwrap();
            let family_doc = roxmltree::Document::parse(&family.1).unwrap();
            let bounds = |node: roxmltree::Node<'_, '_>| -> Vec<f64> {
                node.attribute("viewBox")
                    .unwrap()
                    .split_whitespace()
                    .map(|value| value.parse().unwrap())
                    .collect()
            };
            let before = bounds(base_doc.root_element());
            let after = bounds(family_doc.root_element());
            assert_eq!(
                after,
                vec![
                    before[0] - 52.0,
                    before[1] - 52.0,
                    before[2] + 104.0,
                    before[3] + 104.0
                ]
            );
            assert_eq!(base_doc.root_element().attribute("width"), Some("100%"));
            assert_ne!(family_doc.root_element().attribute("width"), Some("100%"));
            assert!(family_doc.root_element().attribute("height").is_some());
        }
    }
}

#[test]
fn agentflow_dagre_spacing_obeys_family_values_and_root_precedence() {
    let source = "agentflow-beta\nA-->B\nA-->C";
    let config = |family, root| {
        json!({
            "layout": "dagre", "nodeSpacing": root, "rankSpacing": root,
            "agentflow": {"nodeSpacing": family, "rankSpacing": family}
        })
    };
    let base = render_agentflow_config_probe(source, config(50, 0), false);
    let zero = render_agentflow_config_probe(source, config(0, 0), false);
    assert_eq!(base, zero);
    let wide = render_agentflow_config_probe(source, config(200, 0), false);
    assert_ne!(base.0, wide.0);
    let root = render_agentflow_config_probe(source, config(200, 50), false);
    assert_eq!(base, root);
}

#[cfg(feature = "layout-elk")]
#[test]
fn agentflow_elk_spacing_ignores_root_family_and_flowchart_overrides() {
    let source = "agentflow-beta\nflow Workers\nA-->B\nA-->C\nend\nC-->D";
    for frontmatter in [false, true] {
        let base = render_agentflow_config_probe(source, json!({"layout": "elk"}), frontmatter);
        for overrides in [
            json!({"agentflow": {"nodeSpacing": 200, "rankSpacing": 210}}),
            json!({"flowchart": {"nodeSpacing": 300, "rankSpacing": 310}}),
            json!({"nodeSpacing": 90, "rankSpacing": 110}),
            json!({
                "nodeSpacing": 90, "rankSpacing": 110,
                "agentflow": {"nodeSpacing": 200, "rankSpacing": 210},
                "flowchart": {"nodeSpacing": 300, "rankSpacing": 310}
            }),
        ] {
            let mut config = overrides.clone();
            config["layout"] = json!("elk");
            let actual = render_agentflow_config_probe(source, config, frontmatter);
            assert_eq!(base, actual, "frontmatter={frontmatter}, {overrides}");
        }
    }
}

#[test]
fn agentflow_title_uses_family_margin_and_matching_css_class() {
    for backend in ["dagre", "elk"] {
        for (margin, expected) in [(json!(100), "-100"), (json!(0), "0"), (json!(null), "-25")] {
            let (_, svg) = render_agentflow_config_probe(
                "---\ntitle: Heading\n---\nagentflow-beta\nA",
                json!({
                    "layout": backend,
                    "agentflow": {"titleTopMargin": margin},
                    "flowchart": {"titleTopMargin": 200}
                }),
                false,
            );
            let document = roxmltree::Document::parse(&svg).unwrap();
            let title = document
                .descendants()
                .find(|node| node.attribute("class") == Some("agentflowTitleText"))
                .unwrap();
            assert_eq!(title.attribute("y"), Some(expected), "{backend}, {margin}");
            assert!(svg.contains(".agentflowTitleText{text-anchor:middle;font-size:18px;"));
            assert!(!svg.contains("flowchartTitleText"));
        }
    }
}

fn agentflow_rule(target: ThemeTarget, patch: ThemeStylePatch) -> ThemeRule {
    ThemeRule::new(target, patch).for_family(merman_render::DiagramFamilyId::AGENTFLOW)
}

fn agentflow_fill(target: ThemeTarget, color: &str) -> ThemeRule {
    agentflow_rule(
        target,
        ThemeStylePatch::default().with_fill(CanvasPaint::solid(color).unwrap()),
    )
}

fn compile_agentflow_theme(rules: impl IntoIterator<Item = ThemeRule>) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                rules
                    .into_iter()
                    .fold(ThemeRuleSet::default(), |set, rule| set.with_rule(rule)),
            ),
        )
        .unwrap()
}

fn render_agentflow_theme_probe(
    source: &str,
    theme: Option<&DiagramTheme>,
    config: serde_json::Value,
    requirement: ThemePortabilityRequirement,
) -> merman_render::Result<family::RenderedFamilySvg> {
    render_agentflow_theme_probe_with_environment(
        source,
        theme,
        config,
        requirement,
        RenderEnvironment::deterministic(),
    )
}

fn render_agentflow_theme_probe_with_environment(
    source: &str,
    theme: Option<&DiagramTheme>,
    config: serde_json::Value,
    requirement: ThemePortabilityRequirement,
    environment: RenderEnvironment,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let mut engine = MermaidEngine::new()
        .with_site_config(merman_core::presentation::SourcePresentationConfig::from_value(config));
    if let Some(theme) = theme {
        engine = common::install_parse_compatibility(theme, engine);
    }
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    assert_eq!(parsed.metadata().diagram_type, "agentflow");
    let environment = environment.with_theme_portability_requirement(requirement);
    let session = match theme {
        Some(theme) => environment.begin_session_with_theme(theme),
        None => environment.begin_session(),
    }
    .unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)?;
    assert_eq!(artifact.family_kind().as_str(), "agentflow");
    artifact.render_svg(
        &SvgRenderOptions {
            diagram_id: Some("agentflow-theme-test".to_owned()),
            ..Default::default()
        },
        &SvgDebugOptions::default(),
    )
}

fn assert_agentflow_theme_evidence(
    rendered: family::RenderedFamilySvg,
    applied: usize,
    absent: usize,
    residual: usize,
) {
    let completion = rendered.into_completion();
    assert_eq!(
        completion.report().family_id(),
        merman_render::DiagramFamilyId::AGENTFLOW
    );
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), applied + absent + residual);
    assert_eq!(evidence.accounted_count(), evidence.required_count());
    assert_eq!(evidence.applied_count(), applied);
    assert_eq!(evidence.not_applicable_count(), absent);
    assert_eq!(evidence.theme_residual_count(), residual);
    assert_eq!(evidence.source_residual_count(), 0);
}

fn agentflow_element<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
    id: &str,
    kind: &str,
) -> roxmltree::Node<'a, 'input> {
    document
        .descendants()
        .find(|node| {
            node.attribute("data-id") == Some(id) && node.attribute("data-et") == Some(kind)
        })
        .unwrap_or_else(|| panic!("missing Agentflow {kind} {id}"))
}

const AGENTFLOW_THEME_SOURCE: &str = "---\ntitle: Workflow\n---\nagentflow-beta LR\nflow Worker[Worker]\n A[Prompt]@{shape: input}\n B[Task]\n A -- result --> B\nend\n";

#[test]
fn agentflow_compiled_paints_reach_flowchart_terminals_without_losing_kind_or_identity() {
    let theme = compile_agentflow_theme([
        // Place shared text first so later role-local fills win in authored order.
        agentflow_fill(ThemeTarget::Text, "#313f53"),
        agentflow_rule(
            ThemeTarget::Node,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#ac1374").unwrap())
                .with_stroke(CanvasPaint::solid("#1976a2").unwrap()),
        ),
        agentflow_fill(ThemeTarget::NodeLabel, "#351f71"),
        agentflow_rule(
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#165a37").unwrap()),
        ),
        agentflow_fill(ThemeTarget::EdgeLabelBackground, "#eff3a5"),
        agentflow_rule(
            ThemeTarget::Cluster,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#d4e5f6").unwrap())
                .with_stroke(CanvasPaint::solid("#4f657b").unwrap())
                .with_stroke_width(3.0)
                .unwrap(),
        ),
        agentflow_fill(ThemeTarget::ClusterLabel, "#a35b17"),
        agentflow_fill(ThemeTarget::Title, "#b1724e"),
    ]);
    for backend in ["dagre", "elk"] {
        for look in ["classic", "neo"] {
            for html in [false, true] {
                let rendered = render_agentflow_theme_probe(
                    AGENTFLOW_THEME_SOURCE,
                    Some(&theme),
                    json!({"layout": backend, "look": look, "htmlLabels": html}),
                    ThemePortabilityRequirement::RequirePortable,
                )
                .unwrap();
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                assert_eq!(
                    document.root_element().attribute("class"),
                    Some("agentflow")
                );
                for (id, kind) in [("A", "af-kind-input"), ("B", "af-kind-task")] {
                    let node = agentflow_element(&document, id, "node");
                    assert!(
                        node.attribute("class")
                            .unwrap()
                            .split_whitespace()
                            .any(|c| c == kind)
                    );
                    assert!(node.descendants().any(|shape| {
                        matches!(shape.tag_name().name(), "rect" | "polygon")
                            && shape.attribute("style").is_some_and(|style| {
                                style.contains("fill:#ac1374 !important")
                                    && style.contains("stroke:#1976a2 !important")
                            })
                    }));
                    assert!(
                        node.descendants().any(|label| {
                            label.attribute("style").is_some_and(|style| {
                                style.contains("fill:#351f71 !important")
                                    || style.contains("color:#351f71 !important")
                            })
                        }),
                        "{backend}, {look}, htmlLabels={html}, node={id}"
                    );
                }
                let cluster = agentflow_element(&document, "Worker", "cluster");
                let rect = cluster
                    .children()
                    .find(|node| node.has_tag_name("rect"))
                    .unwrap();
                assert_eq!(rect.attribute("rx"), Some("10"));
                let style = rect.attribute("style").unwrap();
                for declaration in [
                    "fill:#d4e5f6 !important",
                    "stroke:#4f657b !important",
                    "stroke-width:3px !important",
                ] {
                    assert!(style.contains(declaration), "{backend}, {look}: {style}");
                }
                assert!(document.descendants().any(|node| {
                    node.attribute("data-edge") == Some("true")
                        && node
                            .attribute("style")
                            .is_some_and(|style| style.contains("stroke:#165a37 !important"))
                }));
                let css = document
                    .descendants()
                    .find(|node| node.has_tag_name("style"))
                    .unwrap()
                    .text()
                    .unwrap();
                assert!(css.contains(".cluster-label text{fill:#a35b17;}"));
                assert!(css.contains(".cluster-label span{color:#a35b17;}"));
                assert!(css.contains(".edgeLabel .label"));
                assert!(css.contains("fill:#313f53;color:#313f53;"));
                assert!(css.contains(
                    ".agentflowTitleText{text-anchor:middle;font-size:18px;fill:#b1724e;}"
                ));
                assert!(css.contains(".af-kind-input"));
                assert!(css.contains("#eff3a5"));
                assert!(!rendered.svg().contains("flowchartTitleText"));
                assert_agentflow_theme_evidence(rendered, 8, 0, 0);
            }
        }
    }
}

#[test]
fn agentflow_title_rule_owns_document_and_container_titles_ahead_of_text() {
    for cluster_rule in [false, true] {
        for reverse in [false, true] {
            let mut rules = vec![agentflow_fill(ThemeTarget::Title, "#71294f")];
            if cluster_rule {
                rules.insert(0, agentflow_fill(ThemeTarget::ClusterLabel, "#a35b17"));
            }
            rules.push(agentflow_fill(ThemeTarget::Text, "#195a73"));
            if reverse {
                rules.reverse();
            }
            let theme = compile_agentflow_theme(rules);
            for backend in ["dagre", "elk"] {
                for html in [false, true] {
                    let rendered = render_agentflow_theme_probe(
                        AGENTFLOW_THEME_SOURCE,
                        Some(&theme),
                        json!({"layout": backend, "htmlLabels": html}),
                        ThemePortabilityRequirement::RequirePortable,
                    )
                    .unwrap();
                    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                    let cluster = agentflow_element(&document, "Worker", "cluster");
                    let label = cluster
                        .descendants()
                        .find(|node| node.attribute("class") == Some("cluster-label"))
                        .unwrap();
                    assert!(
                        label
                            .descendants()
                            .any(|node| node.text() == Some("Worker"))
                    );
                    assert_eq!(
                        label
                            .descendants()
                            .any(|node| node.has_tag_name("foreignObject")),
                        html
                    );
                    let css = document
                        .descendants()
                        .find(|node| node.has_tag_name("style"))
                        .unwrap()
                        .text()
                        .unwrap();
                    let cluster_color = if cluster_rule { "#a35b17" } else { "#71294f" };
                    assert!(css.contains(&format!(".cluster-label text{{fill:{cluster_color};}}")));
                    assert!(
                        css.contains(&format!(".cluster-label span{{color:{cluster_color};}}"))
                    );
                    assert!(css.contains(
                        ".agentflowTitleText{text-anchor:middle;font-size:18px;fill:#71294f;}"
                    ));
                    let title = document
                        .descendants()
                        .find(|node| node.attribute("class") == Some("agentflowTitleText"))
                        .unwrap();
                    assert_eq!(title.text(), Some("Workflow"));
                    assert_eq!(title.attribute("style"), None);
                    drop(document);
                    assert_agentflow_theme_evidence(rendered, 2 + usize::from(cluster_rule), 0, 0);
                }
            }
        }
    }
}

#[test]
fn agentflow_title_fill_applies_without_a_container_and_preserves_text_fallback() {
    let source = "---\ntitle: Workflow\n---\nagentflow-beta\nA[Alpha]\n";
    let title_only = compile_agentflow_theme([agentflow_fill(ThemeTarget::Title, "#71294f")]);
    let rendered = render_agentflow_theme_probe(
        source,
        Some(&title_only),
        json!({"layout": "dagre", "htmlLabels": false}),
        ThemePortabilityRequirement::RequirePortable,
    )
    .unwrap();
    assert!(
        rendered
            .svg()
            .contains(".agentflowTitleText{text-anchor:middle;font-size:18px;fill:#71294f;}")
    );
    assert_agentflow_theme_evidence(rendered, 1, 0, 0);
    for title_rule in [false, true] {
        for configured in [false, true] {
            let mut rules = vec![agentflow_fill(ThemeTarget::Text, "#195a73")];
            if title_rule {
                rules.push(agentflow_fill(ThemeTarget::Title, "#71294f"));
            }
            let theme = compile_agentflow_theme(rules);
            let mut config = json!({"layout": "dagre", "htmlLabels": false});
            if configured {
                config["themeVariables"] = json!({"textColor": "#375971"});
            }
            let rendered = render_agentflow_theme_probe(
                source,
                Some(&theme),
                config,
                ThemePortabilityRequirement::RequirePortable,
            )
            .unwrap();
            let color = if configured {
                "#375971"
            } else if title_rule {
                "#71294f"
            } else {
                "#195a73"
            };
            assert!(rendered.svg().contains(&format!(
                ".agentflowTitleText{{text-anchor:middle;font-size:18px;fill:{color};}}"
            )));
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let title = document
                .descendants()
                .find(|node| node.attribute("class") == Some("agentflowTitleText"))
                .unwrap();
            assert_eq!(title.text(), Some("Workflow"));
            assert_eq!(title.attribute("style"), None);
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.required_count(), 1 + usize::from(title_rule));
            assert_eq!(evidence.accounted_count(), evidence.required_count());
            assert_eq!(evidence.theme_residual_count(), 0);
            if !configured {
                assert_eq!(evidence.applied_count(), evidence.required_count());
            }
        }
    }
}

#[test]
fn agentflow_base_typography_reaches_visible_labels_and_honors_explicit_config() {
    let stack = FontStack::new(["Agentflow Font", "monospace"]).unwrap();
    let font_css = stack.as_css();
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_family_style(
                    merman_render::DiagramFamilyId::AGENTFLOW,
                    ThemeTextStyle::default()
                        .with_font_stack(stack)
                        .with_font_size_px(23.0)
                        .unwrap(),
                ),
            ),
        )
        .unwrap();
    for backend in ["dagre", "elk"] {
        for html in [false, true] {
            for configured in [false, true] {
                let mut config = json!({"layout": backend, "htmlLabels": html});
                if configured {
                    config["fontFamily"] = json!("SourceFont");
                    config["themeVariables"] = json!({"fontSize": "31px"});
                }
                let rendered = render_agentflow_theme_probe(
                    AGENTFLOW_THEME_SOURCE,
                    Some(&theme),
                    config,
                    ThemePortabilityRequirement::RequirePortable,
                )
                .unwrap();
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let css = document
                    .descendants()
                    .find(|node| node.has_tag_name("style"))
                    .unwrap()
                    .text()
                    .unwrap();
                let expected = if configured {
                    "font-family:SourceFont;font-size:31px;".to_owned()
                } else {
                    format!("font-family:{font_css};font-size:23px;")
                };
                assert!(css.contains(&expected), "{backend}, html={html}: {css}");
                assert!(
                    agentflow_element(&document, "A", "node")
                        .descendants()
                        .any(|node| { node.has_tag_name(if html { "span" } else { "text" }) })
                );
                assert_agentflow_theme_evidence(
                    rendered,
                    if configured { 0 } else { 2 },
                    if configured { 2 } else { 0 },
                    0,
                );
            }
        }
    }
}

#[test]
fn agentflow_source_classes_inline_styles_and_edge_styles_keep_ownership() {
    let theme = compile_agentflow_theme([
        agentflow_fill(ThemeTarget::Node, "#abcd12"),
        agentflow_fill(ThemeTarget::NodeLabel, "#bcde23"),
        agentflow_rule(
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#cdef34").unwrap()),
        ),
    ]);
    let source = "agentflow-beta LR\nclassDef hot fill:#18395a,color:#2a4b6c\nA[Alpha]:::hot --> B[Beta]\nstyle B fill:#3c5d7e,color:#4e6f80\nlinkStyle 0 stroke:#507192\n";
    for html in [false, true] {
        let rendered = render_agentflow_theme_probe(
            source,
            Some(&theme),
            json!({"layout": "dagre", "htmlLabels": html}),
            ThemePortabilityRequirement::RequirePortable,
        )
        .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let node = agentflow_element(&document, "B", "node");
        assert!(node.descendants().any(|node| {
            node.attribute("style")
                .is_some_and(|style| style.contains("#3c5d7e"))
        }));
        assert!(document.descendants().any(|node| {
            node.attribute("data-edge") == Some("true")
                && node
                    .attribute("style")
                    .is_some_and(|style| style.contains("#507192"))
        }));
        for color in ["#abcd12", "#bcde23", "#cdef34"] {
            assert!(!rendered.svg().contains(color), "source owns {color}");
        }
        assert_agentflow_theme_evidence(rendered, 0, 3, 0);
    }
}

#[test]
fn agentflow_cluster_source_and_family_config_shadow_only_their_owned_facets() {
    let theme = compile_agentflow_theme([
        agentflow_fill(ThemeTarget::Cluster, "#d4e5f6"),
        agentflow_rule(
            ThemeTarget::Cluster,
            ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#4f657b").unwrap()),
        ),
    ]);
    for look in ["classic", "handDrawn"] {
        let rendered = render_agentflow_theme_probe(
            "agentflow-beta\nclassDef hot fill:#13579b\nflow Worker[Worker]\nA\nend\nclass Worker hot\n",
            Some(&theme),
            json!({"layout": "dagre", "look": look, "handDrawnSeed": 7, "themeVariables": {"flowContainerStroke": "#2468ac"}}),
            ThemePortabilityRequirement::RequirePortable,
        )
        .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let cluster = agentflow_element(&document, "Worker", "cluster");
        assert!(cluster.descendants().any(|node| {
            node.attribute("style")
                .is_some_and(|style| style.contains("#13579b"))
        }));
        assert!(
            cluster
                .descendants()
                .any(|node| node.attribute("stroke") == Some("#2468ac"))
        );
        assert!(!rendered.svg().contains("#d4e5f6"));
        assert!(!rendered.svg().contains("#4f657b"));
        assert_agentflow_theme_evidence(rendered, 0, 2, 0);
    }
}

#[test]
fn agentflow_collapsed_container_stroke_keeps_its_config_owner_per_occurrence() {
    let theme = compile_agentflow_theme([agentflow_rule(
        ThemeTarget::Node,
        ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#4f657b").unwrap()),
    )]);
    for backend in ["dagre", "elk"] {
        for ordinary_node in [false, true] {
            let source = format!(
                "agentflow-beta\nflow Worker[Worker]@{{view: collapsed}}\nA\nend\n{}",
                if ordinary_node { "B[Beta]\n" } else { "" },
            );
            let rendered = render_agentflow_theme_probe(
                &source,
                Some(&theme),
                json!({"layout": backend, "htmlLabels": false, "themeVariables": {"flowContainerStroke": "#2468ac"}}),
                ThemePortabilityRequirement::RequirePortable,
            )
            .unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let worker = agentflow_element(&document, "Worker", "node");
            let body = worker
                .descendants()
                .find(|node| {
                    node.attribute("class").is_some_and(|class| {
                        class
                            .split_whitespace()
                            .any(|class| class == "flow-collapsed")
                    })
                })
                .unwrap();
            assert_eq!(body.attribute("stroke"), Some("#2468ac"), "{backend}");
            assert!(
                !body
                    .attribute("style")
                    .unwrap_or_default()
                    .contains("#4f657b")
            );
            if ordinary_node {
                assert!(
                    agentflow_element(&document, "B", "node")
                        .descendants()
                        .any(|node| {
                            node.attribute("style")
                                .is_some_and(|style| style.contains("stroke:#4f657b !important"))
                        })
                );
            } else {
                assert!(!rendered.svg().contains("#4f657b"));
            }
            assert_agentflow_theme_evidence(
                rendered,
                usize::from(ordinary_node),
                usize::from(!ordinary_node),
                0,
            );
        }
    }
}

#[test]
fn agentflow_collapsed_only_label_reuses_its_prepared_subgraph_owner_and_local_fonts() {
    let theme = compile_agentflow_theme([agentflow_rule(
        ThemeTarget::NodeLabel,
        ThemeStylePatch {
            typography: TextStylePatch {
                font_stack: Specified::Value(FontStack::single("NodeFont").unwrap()),
                font_size_px: Specified::Value(29.0),
                ..Default::default()
            },
            ..Default::default()
        },
    )]);
    for backend in ["dagre", "elk"] {
        for html in [false, true] {
            let host = std::sync::Arc::new(AgentflowFontHost::default());
            let rendered = render_agentflow_theme_probe_with_environment(
                "agentflow-beta\nflow Worker[Worker]@{view: collapsed}\nA\nend\n",
                Some(&theme),
                json!({"layout": backend, "htmlLabels": html}),
                ThemePortabilityRequirement::RequirePortable,
                agentflow_font_environment(host.clone()),
            )
            .unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let worker = agentflow_element(&document, "Worker", "node");
            assert!(
                worker.descendants().any(|node| {
                    node.attribute("style").is_some_and(|style| {
                        style.contains("font-family:NodeFont !important")
                            && style.contains("font-size:29px !important")
                    })
                }),
                "{backend}, html={html}"
            );
            assert_eq!(
                document
                    .descendants()
                    .filter(|node| node.attribute("data-et") == Some("node"))
                    .count(),
                1
            );
            assert!(
                host.requests.lock().unwrap().iter().any(|(text, style)| {
                    text.contains("Worker")
                        && style.font_family.as_deref() == Some("NodeFont")
                        && style.font_size == 29.0
                }),
                "{backend}, html={html}"
            );
            assert_agentflow_theme_evidence(rendered, 1, 0, 0);
        }
    }
}

#[test]
fn agentflow_cluster_typed_paint_reaches_hand_drawn_fill_and_border_paths() {
    for transparent in [false, true] {
        let theme = compile_agentflow_theme([agentflow_rule(
            ThemeTarget::Cluster,
            ThemeStylePatch::default()
                .with_fill(if transparent {
                    CanvasPaint::Transparent
                } else {
                    CanvasPaint::solid("#b2d4f6").unwrap()
                })
                .with_stroke(CanvasPaint::solid("#2b4d6f").unwrap())
                .with_stroke_width(3.0)
                .unwrap(),
        )]);
        let rendered = render_agentflow_theme_probe(
            "agentflow-beta\nflow Worker[Worker]\nA\nend\n",
            Some(&theme),
            json!({"layout": "dagre", "look": "handDrawn", "handDrawnSeed": 7}),
            ThemePortabilityRequirement::RequirePortable,
        )
        .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let cluster = agentflow_element(&document, "Worker", "cluster");
        let shape = cluster.children().find(|node| node.is_element()).unwrap();
        let paths = shape
            .children()
            .filter(|node| node.has_tag_name("path"))
            .collect::<Vec<_>>();
        assert_eq!(paths.len(), if transparent { 1 } else { 2 });
        let border = paths.last().unwrap();
        assert_eq!(border.attribute("stroke"), Some("#2b4d6f"));
        assert_eq!(border.attribute("stroke-width"), Some("3"));
        assert!(
            border
                .attribute("style")
                .unwrap()
                .contains("stroke:#2b4d6f !important")
        );
        if !transparent {
            assert_eq!(paths[0].attribute("stroke"), Some("#b2d4f6"));
            assert!(
                paths[0]
                    .attribute("style")
                    .unwrap()
                    .contains("stroke:#b2d4f6 !important")
            );
        }
        assert_agentflow_theme_evidence(rendered, 1, 0, 0);
    }
}

#[test]
fn agentflow_unverified_source_paint_keeps_ownership_and_its_diagnostic() {
    let theme = compile_agentflow_theme([agentflow_fill(ThemeTarget::Node, "#d3a571")]);
    let source = "agentflow-beta\nA[Alpha]\nstyle A fill:var(--agentflow-fill)\n";
    let config = json!({"layout": "dagre", "htmlLabels": false});
    let rendered = render_agentflow_theme_probe(
        source,
        Some(&theme),
        config.clone(),
        ThemePortabilityRequirement::BestEffort,
    )
    .unwrap();
    assert!(rendered.svg().contains("fill:var(--agentflow-fill)"));
    assert!(!rendered.svg().contains("#d3a571"));
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert!(evidence.source_residual_count() > 0);
    assert!(
        render_agentflow_theme_probe(
            source,
            Some(&theme),
            config,
            ThemePortabilityRequirement::RequirePortable,
        )
        .is_err()
    );
}

#[test]
fn agentflow_label_local_fonts_share_the_actual_layout_and_svg_or_html_typography() {
    let theme = agentflow_label_font_theme();
    for backend in ["dagre", "elk"] {
        for html in [false, true] {
            let host = std::sync::Arc::new(AgentflowFontHost::default());
            let environment = agentflow_font_environment(host.clone());
            let rendered = render_agentflow_theme_probe_with_environment(
                "agentflow-beta LR\nA[Alpha] -- result --> B[Beta]\n",
                Some(&theme),
                json!({"layout": backend, "htmlLabels": html}),
                ThemePortabilityRequirement::RequirePortable,
                environment,
            )
            .unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let alpha = agentflow_element(&document, "A", "node");
            assert!(
                alpha
                    .descendants()
                    .any(|node| node.attribute("style").is_some_and(|style| {
                        style.contains("font-family:NodeFont !important")
                            && style.contains("font-size:29px !important")
                    }))
            );
            assert!(
                document
                    .descendants()
                    .any(|node| node.attribute("style").is_some_and(|style| {
                        style.contains("font-family:EdgeFont !important")
                            && style.contains("font-size:21px !important")
                    }))
            );
            let requests = host.requests.lock().unwrap();
            for (text, font, size) in [("Alpha", "NodeFont", 29.0), ("result", "EdgeFont", 21.0)] {
                assert!(
                    requests.iter().any(|(measured, style)| {
                        measured.contains(text)
                            && style.font_family.as_deref() == Some(font)
                            && style.font_size == size
                    }),
                    "{backend}, html={html}: missing {text} font measurement: {requests:?}"
                );
            }
            assert_agentflow_theme_evidence(rendered, 2, 0, 0);
        }
    }
}

#[test]
fn agentflow_local_font_source_precedence_is_per_property_and_keeps_relative_size_context() {
    let theme = DiagramThemeCompiler::new()
        .compile(agentflow_label_font_spec().with_typography(
            TypographySpec::default().with_family_style(
                merman_render::DiagramFamilyId::AGENTFLOW,
                ThemeTextStyle::default().with_font_size_px(17.0).unwrap(),
            ),
        ))
        .unwrap();
    for html in [false, true] {
        let host = std::sync::Arc::new(AgentflowFontHost::default());
        let source = "agentflow-beta LR\nA[Alpha] -- result --> B[Beta]\nstyle A font-size:2em\nstyle B font-family:SourceFont\nlinkStyle 0 font-size:2em\n";
        let rendered = render_agentflow_theme_probe_with_environment(
            source,
            Some(&theme),
            json!({"layout": "dagre", "htmlLabels": html}),
            if html {
                ThemePortabilityRequirement::BestEffort
            } else {
                ThemePortabilityRequirement::RequirePortable
            },
            agentflow_font_environment(host.clone()),
        )
        .unwrap();
        let requests = host.requests.lock().unwrap();
        for (text, font, size) in [
            ("Alpha", "NodeFont", 34.0),
            ("Beta", "SourceFont", 29.0),
            ("result", "EdgeFont", 34.0),
        ] {
            assert!(
                requests
                    .iter()
                    .any(|(measured, style)| measured.contains(text)
                        && style.font_family.as_deref() == Some(font)
                        && style.font_size == size),
                "html={html}, {text}: {requests:?}"
            );
        }
        assert!(rendered.svg().contains("font-size:2em"));
        assert!(rendered.svg().contains("font-family:NodeFont !important"));
        assert!(rendered.svg().contains("font-family:EdgeFont !important"));
        if html {
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            for (text, expected_size, expected_relative_ancestors) in
                [("Alpha", 136.0, 3), ("result", 68.0, 2)]
            {
                let glyphs = document
                    .descendants()
                    .find(|node| node.is_text() && node.text() == Some(text))
                    .unwrap();
                let (size, relative_ancestors) = agentflow_test_html_font_size(glyphs, 17.0);
                assert_eq!(relative_ancestors, expected_relative_ancestors, "{text}");
                assert_eq!(
                    size, expected_size,
                    "{text}: repeated relative source sizes remain unverified"
                );
                assert_ne!(
                    size, 34.0,
                    "the terminal cannot certify the measured source size"
                );
            }
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.required_count(), 3);
            assert_eq!(evidence.accounted_count(), 3);
            assert_eq!(evidence.applied_count(), 2);
            assert_eq!(evidence.not_applicable_count(), 1);
            assert_eq!(evidence.theme_residual_count(), 0);
            assert_eq!(evidence.source_residual_count(), 2);
            let error = render_agentflow_theme_probe(
                source,
                Some(&theme),
                json!({"layout": "dagre", "htmlLabels": true}),
                ThemePortabilityRequirement::RequirePortable,
            )
            .err()
            .expect("relative HTML source transport cannot claim portability");
            assert_eq!(
                error.unverified_family_style(),
                Some((merman_render::DiagramFamilyId::AGENTFLOW, 2))
            );
        } else {
            assert_agentflow_theme_evidence(rendered, 2, 1, 0);
        }
    }
}

fn agentflow_test_html_font_size(node: roxmltree::Node<'_, '_>, base_px: f64) -> (f64, usize) {
    let mut ancestors = node
        .ancestors()
        .filter(|node| node.is_element())
        .collect::<Vec<_>>();
    ancestors.reverse();
    let mut size = base_px;
    let mut relative_ancestors = 0;
    for ancestor in ancestors {
        let Some(value) = ancestor.attribute("style").and_then(|style| {
            style
                .split(';')
                .filter_map(|declaration| declaration.split_once(':'))
                .filter(|(property, _)| property.trim() == "font-size")
                .map(|(_, value)| value.trim().trim_end_matches("!important").trim())
                .next_back()
        }) else {
            continue;
        };
        if let Some(em) = value.strip_suffix("em") {
            size *= em.parse::<f64>().unwrap();
            relative_ancestors += 1;
        } else if let Some(px) = value.strip_suffix("px") {
            size = px.parse().unwrap();
        } else {
            panic!("unsupported font-size in this HTML inheritance fixture: {value}");
        }
    }
    (size, relative_ancestors)
}

#[test]
fn agentflow_xhtml_code_owns_only_the_local_font_family_and_preserves_size_consumption() {
    for backend in ["dagre", "elk"] {
        for target in [ThemeTarget::NodeLabel, ThemeTarget::EdgeLabel] {
            let font = if target == ThemeTarget::NodeLabel {
                "NodeFont"
            } else {
                "EdgeFont"
            };
            let theme = compile_agentflow_theme([
                agentflow_rule(
                    target,
                    ThemeStylePatch {
                        typography: TextStylePatch {
                            font_stack: Specified::Value(FontStack::single(font).unwrap()),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                ),
                agentflow_rule(
                    target,
                    ThemeStylePatch {
                        typography: TextStylePatch {
                            font_size_px: Specified::Value(29.0),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                ),
            ]);
            let source = if target == ThemeTarget::NodeLabel {
                "agentflow-beta\nA[\"<code>Alpha</code>\"]\n"
            } else {
                "agentflow-beta\nA -- \"<code>result</code>\" --> B\n"
            };
            let host = std::sync::Arc::new(AgentflowFontHost::default());
            let rendered = render_agentflow_theme_probe_with_environment(
                source,
                Some(&theme),
                json!({"layout": backend, "htmlLabels": true}),
                ThemePortabilityRequirement::RequirePortable,
                agentflow_font_environment(host.clone()),
            )
            .unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let code = document
                .descendants()
                .find(|node| node.has_tag_name("code"))
                .expect("retained code owns the visible font family");
            assert_eq!(agentflow_test_html_font_size(code, 16.0).0, 29.0);
            let text = if target == ThemeTarget::NodeLabel {
                "Alpha"
            } else {
                "result"
            };
            assert!(
                host.requests
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|(measured, style)| {
                        measured.contains(text)
                            && style.font_family.as_deref() == Some("monospace")
                            && style.font_size == 29.0
                    }),
                "{backend}, {target:?}: code measurement must use monospace"
            );
            assert_agentflow_theme_evidence(rendered, 1, 1, 0);

            let stack_only = compile_agentflow_theme([agentflow_rule(
                target,
                ThemeStylePatch {
                    typography: TextStylePatch {
                        font_stack: Specified::Value(FontStack::single(font).unwrap()),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )]);
            let rendered = render_agentflow_theme_probe(
                source,
                Some(&stack_only),
                json!({"layout": backend, "htmlLabels": true}),
                ThemePortabilityRequirement::RequirePortable,
            )
            .unwrap();
            assert_agentflow_theme_evidence(rendered, 0, 1, 0);
        }
    }
}

#[test]
fn agentflow_xhtml_descendant_size_owns_its_facet_and_keeps_its_source_residual() {
    for backend in ["dagre", "elk"] {
        for target in [ThemeTarget::NodeLabel, ThemeTarget::EdgeLabel] {
            let theme = compile_agentflow_theme([
                agentflow_rule(
                    target,
                    ThemeStylePatch {
                        typography: TextStylePatch {
                            font_stack: Specified::Value(FontStack::single("LabelFont").unwrap()),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                ),
                agentflow_rule(
                    target,
                    ThemeStylePatch {
                        typography: TextStylePatch {
                            font_size_px: Specified::Value(29.0),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                ),
            ]);
            let source = if target == ThemeTarget::NodeLabel {
                "agentflow-beta\nA[\"<span style='font-size:40px'>Alpha</span>\"]\n"
            } else {
                "agentflow-beta\nA -- \"<span style='font-size:40px'>result</span>\" --> B\n"
            };
            let config = json!({"layout": backend, "htmlLabels": true});
            let rendered = render_agentflow_theme_probe(
                source,
                Some(&theme),
                config.clone(),
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let descendant = document
                .descendants()
                .find(|node| {
                    node.attribute("style")
                        .is_some_and(|style| style.contains("font-size:40px"))
                })
                .unwrap();
            assert_eq!(agentflow_test_html_font_size(descendant, 16.0).0, 40.0);
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.required_count(), 2);
            assert_eq!(evidence.accounted_count(), 2);
            assert_eq!(
                evidence.applied_count(),
                1,
                "the font family still reaches visible text"
            );
            assert_eq!(
                evidence.not_applicable_count(),
                1,
                "the descendant owns font-size"
            );
            assert_eq!(evidence.theme_residual_count(), 0);
            assert_eq!(evidence.source_residual_count(), 1);
            let error = render_agentflow_theme_probe(
                source,
                Some(&theme),
                config,
                ThemePortabilityRequirement::RequirePortable,
            )
            .err()
            .expect("unmeasured descendant font-size remains nonportable");
            assert_eq!(
                error.unverified_family_style(),
                Some((merman_render::DiagramFamilyId::AGENTFLOW, 1))
            );
        }
    }
}

#[test]
fn agentflow_local_font_rules_reach_svg_and_html_markdown_without_changing_markup() {
    let theme = agentflow_label_font_theme();
    for html in [false, true] {
        let rendered = render_agentflow_theme_probe(
            "agentflow-beta LR\nA[\"`**Alpha**`\"] -- \"`**result**`\" --> B[Beta]\n",
            Some(&theme),
            json!({"layout": "dagre", "htmlLabels": html}),
            ThemePortabilityRequirement::RequirePortable,
        )
        .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        for (font, size) in [("NodeFont", 29), ("EdgeFont", 21)] {
            assert!(
                document.descendants().any(|node| node
                    .attribute("style")
                    .is_some_and(|style| style
                        .contains(&format!("font-family:{font} !important"))
                        && style.contains(&format!("font-size:{size}px !important"))))
            );
        }
        assert!(document.descendants().any(|node| if html {
            node.has_tag_name("strong")
        } else {
            node.has_tag_name("tspan") && node.attribute("font-weight") == Some("bold")
        }));
        assert_agentflow_theme_evidence(rendered, 2, 0, 0);
    }
}

fn agentflow_label_font_theme() -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(agentflow_label_font_spec())
        .unwrap()
}

fn agentflow_label_font_spec() -> DiagramThemeSpec {
    let rules = [
        (ThemeTarget::NodeLabel, "NodeFont", 29.0),
        (ThemeTarget::EdgeLabel, "EdgeFont", 21.0),
    ]
    .map(|(target, font, size)| {
        agentflow_rule(
            target,
            ThemeStylePatch {
                typography: TextStylePatch {
                    font_stack: Specified::Value(FontStack::single(font).unwrap()),
                    font_size_px: Specified::Value(size),
                    ..Default::default()
                },
                ..Default::default()
            },
        )
    });
    DiagramThemeSpec::new().with_styles(
        rules
            .into_iter()
            .fold(ThemeRuleSet::default(), |set, rule| set.with_rule(rule)),
    )
}

#[derive(Default)]
struct AgentflowFontHost {
    requests: std::sync::Mutex<Vec<(String, merman_render::text::TextStyle)>>,
}

impl merman_render::environment::HostTextMeasurer for AgentflowFontHost {
    fn measure(
        &self,
        request: merman_render::environment::HostTextMeasurementRequest<'_>,
    ) -> merman_render::environment::HostMeasurementResult {
        self.requests
            .lock()
            .unwrap()
            .push((request.text.to_owned(), request.style.clone()));
        Ok(None)
    }
}

fn agentflow_font_environment(host: std::sync::Arc<AgentflowFontHost>) -> RenderEnvironment {
    use merman_render::environment::{
        MeasurementProfileId, TextMeasurementPhase, TextMeasurementPolicy,
        TextMeasurementProfileIdentity,
    };
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.agentflow-fonts").unwrap(),
        "1",
    )
    .unwrap();
    RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::host_display(identity, host, TextMeasurementPhase::ALL),
    )
}

#[test]
fn agentflow_label_local_fonts_preserve_class_inline_edge_and_config_winners() {
    let theme = agentflow_label_font_theme();
    let source = "agentflow-beta LR\nclassDef hot font-family:ClassFont,font-size:33px\nA[Alpha]:::hot -- result --> B[Beta]\nstyle B font-family:InlineFont,font-size:35px\nlinkStyle 0 font-family:LinkFont,font-size:37px\n";
    for backend in ["dagre", "elk"] {
        for html in [false, true] {
            let host = std::sync::Arc::new(AgentflowFontHost::default());
            let rendered = render_agentflow_theme_probe_with_environment(
                source,
                Some(&theme),
                json!({"layout": backend, "htmlLabels": html}),
                ThemePortabilityRequirement::RequirePortable,
                agentflow_font_environment(host.clone()),
            )
            .unwrap();
            assert!(!rendered.svg().contains("NodeFont"));
            assert!(!rendered.svg().contains("EdgeFont"));
            let requests = host.requests.lock().unwrap();
            for (text, font, size) in [
                ("Alpha", "ClassFont", 33.0),
                ("Beta", "InlineFont", 35.0),
                ("result", "LinkFont", 37.0),
            ] {
                assert!(
                    requests.iter().any(|(measured, style)| {
                        measured.contains(text)
                            && style.font_family.as_deref() == Some(font)
                            && style.font_size == size
                    }),
                    "{backend}, html={html}: missing {font}: {requests:?}"
                );
                assert!(rendered.svg().contains(font));
            }
            assert_agentflow_theme_evidence(rendered, 0, 2, 0);

            let rendered = render_agentflow_theme_probe(
                "agentflow-beta LR\nA[Alpha] -- result --> B[Beta]\n", Some(&theme),
                json!({"layout": backend, "htmlLabels": html, "fontFamily": "ConfigFont", "themeVariables": {"fontSize": "31px"}}),
                ThemePortabilityRequirement::RequirePortable,
            ).unwrap();
            assert!(!rendered.svg().contains("NodeFont"));
            assert!(!rendered.svg().contains("EdgeFont"));
            assert!(rendered.svg().contains("font-size:31px"));
            assert_agentflow_theme_evidence(rendered, 0, 2, 0);
        }
    }
}

#[test]
fn agentflow_ordinal_palette_uses_visible_nodes_and_keeps_source_fill_priority() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_ordinal_palette(
                    ThemeTarget::Node,
                    OrdinalPalette::new([
                        ThemeColorValue::parse("#a1b2c3").unwrap(),
                        ThemeColorValue::parse("#b1c2d3").unwrap(),
                        ThemeColorValue::parse("#c1d2e3").unwrap(),
                    ])
                    .unwrap(),
                ),
            ),
        )
        .unwrap();
    for backend in ["dagre", "elk"] {
        for look in ["classic", "neo"] {
            for authored in [false, true] {
                let mut source =
                    "agentflow-beta\nA[Task]\nB[Search]@{shape: tool}\nconnector C[API]\n"
                        .to_owned();
                if authored {
                    source.push_str("style B fill:#315273\n");
                }
                let rendered = render_agentflow_theme_probe(
                    &source,
                    Some(&theme),
                    json!({"layout": backend, "look": look, "htmlLabels": false}),
                    ThemePortabilityRequirement::RequirePortable,
                )
                .unwrap();
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                for (id, color) in [
                    ("A", "#a1b2c3"),
                    ("B", if authored { "#315273" } else { "#b1c2d3" }),
                    ("C", "#c1d2e3"),
                ] {
                    assert!(
                        agentflow_element(&document, id, "node")
                            .descendants()
                            .any(|node| {
                                node.attribute("style")
                                    .is_some_and(|style| style.contains(&format!("fill:{color}")))
                            }),
                        "{backend}, {look}, {id}"
                    );
                }
                assert_agentflow_theme_evidence(rendered, 1, 0, 0);
            }

            let rendered = render_agentflow_theme_probe(
                "agentflow-beta\nflow Worker[Worker]@{view: collapsed}\nA --> B\nend\n",
                Some(&theme),
                json!({"layout": backend, "look": look, "htmlLabels": false}),
                ThemePortabilityRequirement::RequirePortable,
            )
            .unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let worker = agentflow_element(&document, "Worker", "node");
            assert!(worker.descendants().any(|node| {
                node.has_tag_name("rect")
                    && node
                        .attribute("style")
                        .is_some_and(|style| style.contains("fill:#a1b2c3 !important"))
            }));
            assert_agentflow_theme_evidence(rendered, 1, 0, 0);
        }
    }
}

fn agentflow_effect_theme() -> DiagramTheme {
    let graph = EffectGraph::new(
        "agentflow-glow",
        [EffectPrimitive::DropShadow {
            input: EffectInput::SourceGraphic,
            offset_x: 2.0,
            offset_y: 3.0,
            blur_radius: 2.0,
            spread: 0.0,
            color: ThemeColorValue::parse("#b2468c").unwrap(),
        }],
    )
    .unwrap();
    let mut effects = DiagramEffectSet::default().with_graph(graph).unwrap();
    for target in [
        ThemeTarget::Node,
        ThemeTarget::NodeLabel,
        ThemeTarget::Edge,
        ThemeTarget::EdgeLabel,
    ] {
        effects = effects
            .with_binding(EffectBinding::new(target, "agentflow-glow").unwrap())
            .unwrap();
    }
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_effects(effects))
        .unwrap()
}

#[test]
fn agentflow_basic_effects_use_the_shared_classic_and_neo_body_edge_and_label_terminals() {
    let theme = agentflow_effect_theme();
    for backend in ["dagre", "elk"] {
        for look in ["classic", "neo"] {
            for html in [false, true] {
                let rendered = render_agentflow_theme_probe(
                    "agentflow-beta LR\nA[Alpha] -- result --> B[Beta]\n",
                    Some(&theme),
                    json!({"layout": backend, "look": look, "htmlLabels": html}),
                    ThemePortabilityRequirement::RequirePortable,
                )
                .unwrap();
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let applications = document
                    .descendants()
                    .filter_map(|node| {
                        agentflow_theme_filter_reference(node).map(|reference| (node, reference))
                    })
                    .collect::<Vec<_>>();
                assert_eq!(applications.len(), 6, "{backend}, {look}, html={html}");
                let first_edge = document
                    .descendants()
                    .find(|n| n.attribute("data-edge") == Some("true"))
                    .unwrap()
                    .range()
                    .start;
                let body_depths: Vec<_> = document
                    .descendants()
                    .filter(|n| n.attribute("data-merman-theme-depth") == Some("node"))
                    .collect();
                assert_eq!(body_depths.len(), 2);
                for depth in body_depths {
                    assert!(depth.range().end < first_edge);
                    let body = depth.children().find(|n| n.has_tag_name("rect")).unwrap();
                    assert!(agentflow_theme_filter_reference(body).is_some());
                    assert!(!depth.descendants().any(|n| n.attribute("id").is_some()
                        || n.has_tag_name("text")
                        || n.has_tag_name("foreignObject")));
                }
                for node_id in ["A", "B"] {
                    let node = agentflow_element(&document, node_id, "node");
                    assert_eq!(
                        node.children()
                            .filter(|shape| {
                                shape.has_tag_name("rect")
                                    && agentflow_theme_filter_reference(*shape).is_some()
                            })
                            .count(),
                        0,
                        "{backend}, {look}: {node_id} face must not repaint the compiled depth"
                    );
                    let label = node
                        .descendants()
                        .find(|node| node.attribute("class") == Some("label"))
                        .unwrap();
                    assert_eq!(
                        label
                            .descendants()
                            .filter_map(agentflow_theme_filter_reference)
                            .count(),
                        1,
                        "{backend}, {look}: missing {node_id} label filter"
                    );
                }
                assert_eq!(
                    applications
                        .iter()
                        .filter(|(node, _)| { node.attribute("data-edge") == Some("true") })
                        .count(),
                    1,
                    "{backend}, {look}: missing edge path filter"
                );
                let edge_label = document
                    .descendants()
                    .find(|node| node.attribute("class") == Some("edgeLabels"))
                    .unwrap();
                let label_applications = edge_label
                    .descendants()
                    .filter(|node| agentflow_theme_filter_reference(*node).is_some())
                    .collect::<Vec<_>>();
                assert_eq!(label_applications.len(), 1);
                if html {
                    let paragraph = label_applications[0];
                    assert!(paragraph.has_tag_name("p"));
                    assert_eq!(paragraph.text(), Some("result"));
                    assert_eq!(paragraph.attribute("filter"), None);
                    let host = paragraph
                        .ancestors()
                        .find(|node| node.has_tag_name("foreignObject"))
                        .unwrap();
                    assert_eq!(
                        host.attribute("data-merman-fallback-text-filter"),
                        agentflow_theme_filter_reference(paragraph)
                    );
                } else {
                    assert!(label_applications[0].has_tag_name("g"));
                    assert!(label_applications[0].attribute("filter").is_some());
                }
                drop(label_applications);
                let mut filter_ids = std::collections::BTreeSet::new();
                for (_, reference) in applications {
                    let id = reference
                        .strip_prefix("url(#")
                        .unwrap()
                        .strip_suffix(')')
                        .unwrap();
                    assert!(
                        filter_ids.insert(id),
                        "each terminal owns a distinct filter"
                    );
                    assert!(document.descendants().any(|node| {
                        node.has_tag_name("filter") && node.attribute("id") == Some(id)
                    }));
                }
                assert_eq!(filter_ids.len(), 6);
                assert_eq!(
                    document
                        .descendants()
                        .filter(|node| {
                            node.has_tag_name("filter")
                                && node
                                    .attribute("id")
                                    .is_some_and(|id| id.contains("-theme-effect-"))
                        })
                        .count(),
                    filter_ids.len()
                );
                drop(filter_ids);
                drop(document);
                let completion = rendered.into_completion();
                let evidence = merman_render::__private::family_evidence(completion.report());
                assert_eq!(evidence.required_count(), 4);
                assert_eq!(evidence.accounted_count(), 4);
                assert_eq!(evidence.applied_count(), 4);
                assert_eq!(evidence.theme_residual_count(), 0);
                assert_eq!(evidence.source_residual_count(), 0);
                let receipt =
                    merman_render::__private::family_native_filter_receipt(completion.report())
                        .expect("Agentflow retains its emitted filter receipt");
                assert_eq!(receipt.filter_count(), 6);
                assert_eq!(receipt.reference_count(), 6);
            }
        }
    }
}

fn agentflow_theme_filter_reference<'a, 'input>(
    node: roxmltree::Node<'a, 'input>,
) -> Option<&'a str> {
    node.attribute("filter")
        .or_else(|| {
            node.attribute("style")?.split(';').find_map(|declaration| {
                let (property, value) = declaration.split_once(':')?;
                (property.trim() == "filter")
                    .then(|| value.trim().trim_end_matches("!important").trim())
            })
        })
        .filter(|value| value.starts_with("url(#") && value.contains("-theme-effect-"))
}

#[test]
fn agentflow_hand_drawn_effect_requests_remain_residual_without_changing_look() {
    let theme = agentflow_effect_theme();
    let source = "agentflow-beta LR\nA[Alpha] -- result --> B[Beta]\n";
    let config =
        json!({"layout": "dagre", "look": "handDrawn", "handDrawnSeed": 7, "htmlLabels": false});
    let rendered = render_agentflow_theme_probe(
        source,
        Some(&theme),
        config.clone(),
        ThemePortabilityRequirement::BestEffort,
    )
    .unwrap();
    assert!(rendered.svg().contains("data-look=\"handDrawn\""));
    assert!(!rendered.svg().contains("-theme-effect-"));
    assert_agentflow_theme_evidence(rendered, 0, 0, 4);
    assert!(
        render_agentflow_theme_probe(
            source,
            Some(&theme),
            config,
            ThemePortabilityRequirement::RequirePortable,
        )
        .is_err()
    );
}

#[test]
fn agentflow_empty_compiled_theme_preserves_unthemed_svg_bytes() {
    let neutral = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .unwrap();
    for backend in ["dagre", "elk"] {
        for look in ["classic", "neo", "handDrawn"] {
            for source in [
                AGENTFLOW_THEME_SOURCE,
                include_str!("../../../fixtures/agentflow/container-and-shapes.mmd"),
                "agentflow-beta\nclassDef hot fill:#123456\nflow Worker\nA --> A\nend\nclass Worker hot\n",
            ] {
                let config = json!({"layout": backend, "look": look, "handDrawnSeed": 7, "htmlLabels": false});
                let unthemed = render_agentflow_theme_probe(
                    source,
                    None,
                    config.clone(),
                    ThemePortabilityRequirement::BestEffort,
                )
                .unwrap();
                let themed = render_agentflow_theme_probe(
                    source,
                    Some(&neutral),
                    config,
                    ThemePortabilityRequirement::BestEffort,
                )
                .unwrap();
                assert_eq!(unthemed.svg(), themed.svg(), "{backend}, {look}, {source}");
                let before = unthemed.into_completion();
                let after = themed.into_completion();
                let before = merman_render::__private::family_evidence(before.report());
                let after = merman_render::__private::family_evidence(after.report());
                assert_eq!(after.required_count(), 0);
                assert_eq!(after.theme_residual_count(), 0);
                assert_eq!(
                    before.source_residual_count(),
                    after.source_residual_count()
                );
            }
        }
    }
}

#[allow(dead_code)]
mod common;
