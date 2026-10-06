use merman_core::{Engine, ParseOptions};
use merman_render::LayoutOptions;
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
            &[("A", 0), ("B", 2), ("C", 3), ("D", 4)][..],
        ),
        (
            "agentflow-beta\nA[Old]@{shape: task}\nstyle A fill:red\nconnector A[API]\nconnector A[Again]\nA@{instruction: call}\nB\n",
            &[("A", 3), ("B", 5)][..],
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

#[test]
fn agentflow_measurement_config_wins_over_flowchart_defaults_and_overrides() {
    for backend in ["elk", "dagre"] {
        for html_labels in [false, true] {
            for flowchart in [json!({}), json!({"minNodeWidth": 10})] {
                let engine =
                    Engine::new().with_site_config(merman_core::MermaidConfig::from_value(json!({
                        "layout": backend,
                        "htmlLabels": html_labels,
                        "agentflow": { "minNodeWidth": 300 },
                        "flowchart": flowchart
                    })));
                let parsed = engine
                    .parse_diagram_for_render_model_sync(
                        "agentflow-beta\nA[Hi]\n",
                        ParseOptions::strict(),
                    )
                    .unwrap()
                    .unwrap();
                let session = RenderEnvironment::deterministic().begin_session().unwrap();
                let artifact = family::prepare(parsed, &LayoutOptions::default(), session).unwrap();
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
            let engine =
                Engine::new().with_site_config(merman_core::MermaidConfig::from_value(json!({
                    "layout": backend,
                    "htmlLabels": false,
                    "agentflow": { "minNodeWidth": 0, "wrappingWidth": wrapping_width },
                    "flowchart": { "minNodeWidth": 0, "wrappingWidth": 200 }
                })));
            let parsed = engine
                .parse_diagram_for_render_model_sync(
                    "agentflow-beta\nA[one two three four five six seven eight nine ten]\n",
                    ParseOptions::strict(),
                )
                .unwrap()
                .unwrap();
            let session = RenderEnvironment::deterministic().begin_session().unwrap();
            let artifact = family::prepare(parsed, &LayoutOptions::default(), session).unwrap();
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
            Engine::new(),
            format!("---\nconfig: {config}\n---\n{source}"),
        )
    } else {
        (
            Engine::new().with_site_config(merman_core::MermaidConfig::from_value(config)),
            source.to_owned(),
        )
    };
    let parsed = engine
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).unwrap();
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
