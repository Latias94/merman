use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::{SwimlaneDirection, SwimlaneLayout};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::{Error, LayoutOptions, RenderResourcePolicy, ResourceLimitId};

const NESTED_MIXED: &str =
    include_str!("../../../fixtures/swimlane/local_nested_mixed_lane_security.mmd");
const NESTED_MIXED_UPSTREAM_SVG: &str =
    include_str!("../../../fixtures/upstream-svgs/swimlane/local_nested_mixed_lane_security.svg");

fn render_swimlane_with_layout(source: &str, diagram_id: &str) -> (String, SwimlaneLayout) {
    let (svg, layout, _) =
        try_render_swimlane_with_layout(source, diagram_id, RenderResourcePolicy::interactive())
            .unwrap();
    (svg, layout)
}

fn try_render_swimlane_with_layout(
    source: &str,
    diagram_id: &str,
    policy: RenderResourcePolicy,
) -> merman_render::Result<(String, SwimlaneLayout, usize)> {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse swimlane")
        .expect("detect swimlane")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .begin_session()
        .unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)?;
    let layout =
        serde_json::from_value(artifact.layout_json()?["layout"]["SwimlaneDiagram"].clone())
            .unwrap();
    let svg = artifact.render_svg(
        &SvgRenderOptions {
            diagram_id: Some(diagram_id.to_string()),
            ..Default::default()
        },
        &SvgDebugOptions::default(),
    )?;
    let (svg, report) = svg.into_completion().into_output_and_report();
    Ok((svg, layout, report.session_report().layout_work_units()))
}

fn render_swimlane(source: &str, diagram_id: &str) -> String {
    render_swimlane_with_layout(source, diagram_id).0
}

#[test]
fn explicit_shoal_swimlane_retains_the_family_and_checks_capability() {
    use merman_render::{LayoutAdaptation, RenderCapability, RenderCapabilityPolicy};

    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "---\nconfig:\n  layout: shoal\n---\nswimlane-beta TB\nA --> B",
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap()
        .into_presentation();
    for (policy, available) in [
        (
            RenderCapabilityPolicy::unrestricted(),
            cfg!(feature = "layout-shoal"),
        ),
        (RenderCapabilityPolicy::deny_all(), false),
    ] {
        let session = RenderEnvironment::deterministic()
            .with_capability_policy(policy)
            .begin_session()
            .unwrap();
        let plan = family::plan_render(&parsed, &session).unwrap();
        assert_eq!(plan.family_id(), merman_render::DiagramFamilyId::SWIMLANE);
        assert_eq!(
            plan.required_capabilities(),
            &[RenderCapability::LayoutShoal]
        );
        assert_eq!(
            plan.supported_layout_adaptations(),
            &[LayoutAdaptation::None]
        );
        let options = LayoutOptions::default().with_shoal_layout_frame(None);
        let result = family::prepare(&parsed, &options, session);
        if available {
            let svg = result
                .unwrap()
                .render_svg(&Default::default(), &Default::default())
                .unwrap();
            let frame = svg.shoal_layout_frame().unwrap();
            let options = options.with_shoal_layout_frame(Some(frame));
            let session = RenderEnvironment::deterministic().begin_session().unwrap();
            let replay = family::prepare(&parsed, &options, session)
                .unwrap()
                .render_svg(&Default::default(), &Default::default())
                .unwrap();
            assert_eq!(svg.svg(), replay.svg());
        } else {
            assert!(matches!(
                result,
                Err(Error::MissingCapability {
                    capability: RenderCapability::LayoutShoal,
                    ..
                })
            ));
        }
    }
}

#[cfg(feature = "layout-shoal")]
#[test]
fn shoal_swimlane_keeps_a_node_named_like_the_synthetic_lane() {
    let id = "__swimlane_default__";
    let source =
        format!("---\nconfig:\n  layout: shoal\n---\nswimlane-beta TB\n{id}[Review] --> B");
    let (svg, layout) = render_swimlane_with_layout(&source, "shoal-lane-id");
    assert_eq!(layout.lanes.len(), 1);
    assert_ne!(layout.lanes[0].id, id);
    assert!(
        layout
            .nodes
            .iter()
            .any(|node| node.id == id && node.parent_id.as_ref() == Some(&layout.lanes[0].id))
    );
    let doc = roxmltree::Document::parse(&svg).unwrap();
    assert!(
        doc.descendants()
            .any(|part| part.attribute("data-id") == Some(id)
                && part.attribute("data-et") == Some("node"))
    );
}

#[cfg(feature = "layout-shoal")]
#[test]
fn shoal_swimlane_aligns_ranks_and_paints_headers_and_markers_in_four_directions() {
    for direction in ["TB", "BT", "LR", "RL"] {
        let source = format!(
            "---\nconfig:\n  layout: shoal\n  htmlLabels: false\n  flowchart:\n    htmlLabels: false\n---\nswimlane-beta {direction}\n\
             subgraph customer[Customer]\nA[Open] --> B[Close]\nend\n\
             subgraph operator[Operator]\nC[Accept] --> D[Complete]\nend\n\
             subgraph observer[Observer]\nend\nA -->|Handoff| D"
        );
        let (svg, layout) = render_swimlane_with_layout(&source, "shoal-swimlane");
        let find = |id: &str| layout.nodes.iter().find(|node| node.id == id).unwrap();
        let horizontal = matches!(direction, "LR" | "RL");
        let along = |id| if horizontal { find(id).x } else { find(id).y };
        for (a, b) in [("A", "C"), ("B", "D")] {
            assert_eq!(find(a).layer, find(b).layer, "{direction}");
            assert!((along(a) - along(b)).abs() < 0.01, "{direction}");
        }
        assert_eq!(layout.edges.len(), 3, "{direction}");
        let doc = roxmltree::Document::parse(&svg).unwrap();
        let paths: Vec<_> = doc
            .descendants()
            .filter(|part| part.has_tag_name("path") && part.attribute("data-et") == Some("edge"))
            .collect();
        assert_eq!(paths.len(), layout.edges.len(), "{direction}");
        assert!(
            paths
                .iter()
                .all(|part| part.attribute("marker-end").is_some()),
            "{direction}"
        );
        assert!(
            doc.descendants().any(|part| part.text() == Some("Handoff")),
            "{direction}"
        );
        let heading = doc
            .descendants()
            .find(|part| {
                part.attribute("data-id") == Some("customer")
                    && part.attribute("data-et") == Some("cluster")
            })
            .unwrap();
        let title = heading
            .children()
            .find(|part| has_class(*part, "swimlane-title"))
            .unwrap();
        let body = heading
            .children()
            .find(|part| has_class(*part, "swimlane-body"))
            .unwrap();
        let from = if horizontal { "x" } else { "y" };
        let span = if horizontal { "width" } else { "height" };
        let number = |part: roxmltree::Node<'_, '_>, name| {
            part.attribute(name).unwrap().parse::<f64>().unwrap()
        };
        let (first, second) = if matches!(direction, "BT" | "RL") {
            (body, title)
        } else {
            (title, body)
        };
        assert!(
            (number(first, from) + number(first, span) - number(second, from)).abs() < 0.01,
            "{direction}"
        );
    }
}

#[test]
fn long_edge_placeholders_preserve_authored_nodes_and_routes() {
    let diagram_id = "long-edge-placeholder";
    let source = |middle| format!("swimlane-beta TB\n  A --> {middle}[Review] --> C\n  A --> C\n");
    let control_svg = render_swimlane(&source("checkpoint"), diagram_id);
    let control = roxmltree::Document::parse(&control_svg).unwrap();

    for middle in ["placeholder-0", "__merman_swimlane_placeholder_0"] {
        let (svg, layout) = render_swimlane_with_layout(&source(middle), diagram_id);
        let document = roxmltree::Document::parse(&svg).unwrap();
        assert_eq!(
            layout
                .nodes
                .iter()
                .map(|node| node.id.as_str())
                .collect::<Vec<_>>(),
            ["A", middle, "C"]
        );
        assert_eq!(
            text_content(node_element(&document, diagram_id, middle)),
            "Review"
        );

        let routes = [
            (("A", middle), "L_A_checkpoint_0"),
            ((middle, "C"), "L_checkpoint_C_0"),
            (("A", "C"), "L_A_C_0"),
        ];
        assert_eq!(layout.edges.len(), routes.len());
        for (edge, ((from, to), control_id)) in layout.edges.iter().zip(routes) {
            assert_eq!((edge.from.as_str(), edge.to.as_str()), (from, to));
            assert_eq!(edge.id, format!("L_{from}_{to}_0"));
            assert!(edge.points.len() >= 2, "missing route for {}", edge.id);
            let path = semantic_element(&document, &edge.id, "edge");
            let control_path = semantic_element(&control, control_id, "edge");
            assert_eq!(
                path.attribute("d").unwrap(),
                control_path.attribute("d").unwrap()
            );
        }
    }
}

#[test]
fn collapsed_groups_keep_parallel_boundary_routes_and_their_authored_labels() {
    let source = "swimlane-beta TB
subgraph G[Team]
A --> B
end
G@{ view: collapsed }
B next@-->|Ship| X
A retry@-->|Retry| X";
    let (svg, layout) = render_swimlane_with_layout(source, "collapsed");
    let document = roxmltree::Document::parse(&svg).unwrap();
    assert_eq!(
        layout
            .nodes
            .iter()
            .filter(|node| !node.is_edge_label)
            .map(|node| (node.id.as_str(), node.shape.as_str()))
            .collect::<Vec<_>>(),
        [("G", "collapsedGroup"), ("X", "squareRect")]
    );
    assert_eq!(layout.edges.len(), 2);
    assert!(layout.lanes.iter().all(|lane| lane.id != "G"));
    let collapsed = semantic_element(&document, "G", "node");
    assert!(has_class(collapsed, "node"));
    assert_eq!(text_content(collapsed), "Team");
    assert!(
        collapsed
            .children()
            .any(|node| has_class(node, "collapsed-group"))
    );
    for (edge, (id, label)) in layout
        .edges
        .iter()
        .zip([("next", "Ship"), ("retry", "Retry")])
    {
        assert_eq!(
            (edge.id.as_str(), edge.from.as_str(), edge.to.as_str()),
            (id, "G", "X")
        );
        assert!(edge.points.len() >= 2);
        let label_node = layout
            .nodes
            .iter()
            .find(|node| {
                node.is_edge_label && Some(node.id.as_str()) == edge.label_node_id.as_deref()
            })
            .expect("edge owns its measured label node");
        assert_eq!(label_node.label, label);
        assert!(
            !semantic_element(&document, id, "edge")
                .attribute("d")
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            text_content(semantic_element(&document, id, "edge-label")),
            label
        );
    }
}

fn swimlane_work_limit(max: usize) -> RenderResourcePolicy {
    RenderResourcePolicy::interactive()
        .with_limit(ResourceLimitId::MaxLayoutWorkUnits, max)
        .unwrap()
}

#[test]
fn collapsed_swimlane_charges_hidden_edges_linearly() {
    const HIDDEN_EDGES: usize = 32;
    let source = |hidden_edges| {
        let internal = "A --> B\n".repeat(hidden_edges);
        format!(
            "swimlane-beta TB\nsubgraph G[Team]\nA\nB\n{internal}end\nG@{{ view: collapsed }}\nB ship@-->|Ship| X\n"
        )
    };
    let policy = swimlane_work_limit(1_000);
    let (_, control, control_work) =
        try_render_swimlane_with_layout(&source(0), "review", policy).unwrap();
    let (svg, layout, work) =
        try_render_swimlane_with_layout(&source(HIDDEN_EDGES), "review", policy)
            .expect("one visible route fits the work budget");

    assert_eq!(work, control_work + HIDDEN_EDGES * 5);
    assert_eq!(
        serde_json::to_value(&layout).unwrap(),
        serde_json::to_value(&control).unwrap()
    );
    assert_eq!(layout.edges.len(), 1);
    let edge = &layout.edges[0];
    assert_eq!(
        (edge.id.as_str(), edge.from.as_str(), edge.to.as_str()),
        ("ship", "G", "X")
    );
    assert!(edge.points.len() >= 2);
    let document = roxmltree::Document::parse(&svg).unwrap();
    assert_eq!(
        text_content(semantic_element(&document, "ship", "edge-label")),
        "Ship"
    );
}

#[test]
fn uncollapsed_swimlane_keeps_its_layout_work_boundary() {
    let source = "swimlane-beta TB
subgraph G[Team]
A --> B
end
B next@-->|Ship| X
A retry@-->|Retry| X
";
    let (_, layout, required) =
        try_render_swimlane_with_layout(source, "review", RenderResourcePolicy::interactive())
            .unwrap();
    assert_eq!(layout.edges.len(), 3);
    let (_, _, work) =
        try_render_swimlane_with_layout(source, "review", swimlane_work_limit(required)).unwrap();
    assert_eq!(work, required);

    let error =
        try_render_swimlane_with_layout(source, "review", swimlane_work_limit(required - 1))
            .expect_err("the budget below the required work must fail");
    let Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected a layout work ceiling, got {error}");
    };
    assert_eq!(limit.limit, ResourceLimitId::MaxLayoutWorkUnits.as_str());
    assert_eq!((limit.actual, limit.max), (required, required - 1));
}

#[test]
fn dense_visible_swimlane_exceeds_the_route_pair_budget() {
    use std::fmt::Write as _;

    let mut source = String::from("swimlane-beta TB\n");
    for from in 0..6 {
        for to in 0..6 {
            writeln!(source, "A{from} --> B{to}").unwrap();
        }
    }
    let error = try_render_swimlane_with_layout(&source, "review", swimlane_work_limit(1_000))
        .expect_err("36 visible routes exceed the work budget");
    let Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected a layout work ceiling, got {error}");
    };
    assert_eq!(limit.limit, ResourceLimitId::MaxLayoutWorkUnits.as_str());
    assert_eq!(limit.max, 1_000);
    // The 12 nodes and 36 edges contribute 228 linear units and twice 630 pair units.
    assert!(limit.actual >= 1_488);
}

#[test]
fn collapsed_groups_skip_hidden_shapes_and_math_and_keep_authored_styles() {
    let source = r#"swimlane-beta TB
subgraph G[Team]
subgraph H["$$hiddenTitle$$"]
A(-Legacy-) -->|$$hiddenEdge$$| B["$$hiddenNode$$"]
end
end
G@{ view: collapsed }
A next@--> X
"#;
    let styled_source = format!("{source}classDef default fill:#ff0000,font-size:48px\n");
    let (_, baseline) = render_swimlane_with_layout(source, "collapsed");
    let (svg, layout) = render_swimlane_with_layout(&styled_source, "collapsed");
    let document = roxmltree::Document::parse(&svg).unwrap();
    assert_eq!(layout.nodes.len(), 2);
    assert_eq!(layout.edges.len(), 1);
    assert_eq!(
        (layout.edges[0].from.as_str(), layout.edges[0].to.as_str()),
        ("G", "X")
    );
    let collapsed = semantic_element(&document, "G", "node");
    assert_eq!(text_content(collapsed), "Team");
    assert!(!has_class(collapsed, "default"));
    let group_body = collapsed
        .children()
        .find(|node| has_class(*node, "collapsed-group"))
        .expect("compact group body");
    assert!(
        !group_body
            .attribute("style")
            .unwrap_or_default()
            .contains("#ff0000")
    );
    let label_metrics = |layout: &SwimlaneLayout| {
        let node = layout.nodes.iter().find(|node| node.id == "G").unwrap();
        (node.label_width.to_bits(), node.label_height.to_bits())
    };
    assert_eq!(label_metrics(&layout), label_metrics(&baseline));
}

#[test]
fn mermaid_12_3_shared_members_keep_empty_lane_shells_titles_and_edge_turns() {
    let source =
        include_str!("../../../fixtures/swimlane/local_mermaid_12_3_shared_members_tb.mmd");
    let upstream = roxmltree::Document::parse(include_str!(
        "../../../fixtures/upstream-svgs/swimlane/local_mermaid_12_3_shared_members_tb.svg"
    ))
    .unwrap();
    let (svg, layout) = render_swimlane_with_layout(source, "shared-members");
    let actual = roxmltree::Document::parse(&svg).unwrap();
    assert_eq!(layout.lanes.len(), 5);
    assert!(layout.lanes.iter().all(|lane| lane.id != "dropped"));
    let clusters: Vec<_> = actual
        .descendants()
        .filter(|node| has_class(*node, "cluster"))
        .collect();
    assert_eq!(clusters.len(), 5);
    for (index, identity) in ["placeholder", "partial", "used", "retained", "inner"]
        .into_iter()
        .enumerate()
    {
        assert_eq!(clusters[index].attribute("data-id"), Some(identity));
    }
    for identity in ["placeholder", "used"] {
        let expected = upstream
            .descendants()
            .find(|node| node.attribute("data-id") == Some(identity))
            .unwrap();
        let lane = semantic_element(&actual, identity, "cluster");
        assert_eq!(
            lane.attribute("data-color-id"),
            expected.attribute("data-color-id")
        );
        let rows = |node: roxmltree::Node<'_, '_>| {
            node.descendants()
                .filter(|row| has_class(*row, "row"))
                .map(text_content)
                .collect::<Vec<_>>()
        };
        assert_eq!(rows(lane), rows(expected), "{identity}");
    }
    for identity in ["L_A_B_0", "L_B_used_0", "L_B_C_0"] {
        let expected = upstream
            .descendants()
            .find(|node| node.has_tag_name("path") && node.attribute("data-id") == Some(identity))
            .unwrap();
        let edge = semantic_element(&actual, identity, "edge");
        assert_eq!(path_commands(edge), path_commands(expected), "{identity}");
    }
}

#[test]
fn mermaid_12_4_aligned_cross_lane_edge_keeps_contour_contacts_and_label() {
    let source = include_str!("../../../fixtures/swimlane/local_mermaid_12_3_drawn_ports_tb.mmd");
    let (svg, layout) = render_swimlane_with_layout(source, "drawn-ports");
    let actual = roxmltree::Document::parse(&svg).unwrap();
    let route = layout
        .edges
        .iter()
        .find(|edge| edge.id == "L_B_C_0")
        .unwrap();
    assert_eq!((route.from.as_str(), route.to.as_str()), ("B", "C"));
    assert_eq!(route.curve, "rounded");
    let source_node = layout
        .nodes
        .iter()
        .find(|node| node.id == route.from)
        .unwrap();
    let target_node = layout
        .nodes
        .iter()
        .find(|node| node.id == route.to)
        .unwrap();
    let label = layout
        .nodes
        .iter()
        .find(|node| Some(&node.id) == route.label_node_id.as_ref())
        .unwrap();
    let [from, to] = route.points.as_slice() else {
        panic!("expected a two-point shaft, got {:?}", route.points);
    };
    assert_eq!(
        (from.x, from.y),
        (source_node.x + source_node.width / 2.0, source_node.y)
    );
    assert_eq!(
        (to.x, to.y),
        (target_node.x - target_node.width / 2.0, source_node.y)
    );
    assert!(label.is_edge_label && label.x > from.x && label.x < to.x);
    assert_eq!(label.y, from.y);

    let edge = semantic_element(&actual, &route.id, "edge");
    assert_eq!(edge.attribute("marker-start"), None);
    assert!(
        edge.attribute("marker-end")
            .unwrap()
            .ends_with("_swimlane-pointEnd)")
    );
    assert_eq!(
        text_content(semantic_element(&actual, &route.id, "edge-label")),
        "accept"
    );
    use svgtypes::PathSegment::{LineTo, MoveTo};
    let path = svgtypes::PathParser::from(edge.attribute("d").unwrap())
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let [
        MoveTo {
            abs: true,
            x: start_x,
            y: start_y,
        },
        LineTo {
            abs: true,
            x: end_x,
            y: end_y,
        },
    ] = path.as_slice()
    else {
        panic!("expected an SVG move and line, got {path:?}");
    };
    assert!((start_x - from.x).abs() < 1e-3);
    assert!((start_y - from.y).abs() < 1e-3);
    assert!((end_x - (to.x - 4.0)).abs() < 1e-3);
    assert!((end_y - to.y).abs() < 1e-3);
}

fn path_commands(node: roxmltree::Node<'_, '_>) -> String {
    node.attribute("d")
        .unwrap()
        .chars()
        .filter(|character| matches!(character, 'M' | 'L' | 'Q' | 'C' | 'A' | 'Z'))
        .collect()
}

#[test]
fn producer_pool_metadata_selects_pool_shell_classes_without_author_syntax() {
    use merman_model::OperationControl;
    use merman_model::presentation::{PresentationDiagram, RenderSemanticModel};
    use merman_model::resources::InputResourcePolicy;

    for look in ["classic", "handDrawn"] {
        let source = format!(
            "%%{{init: {{\"look\": \"{look}\", \"handDrawnSeed\": 7}}}}%%\nswimlane-beta TB\nsubgraph pool\nsubgraph sales\nA\nend\nsubgraph warehouse\nB\nend\nend\nA --> B\n"
        );
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let (metadata, mut model, context) = parsed.into_parts();
        let RenderSemanticModel::Flowchart(flowchart) = &mut model else {
            panic!("expected Flowchart model");
        };
        flowchart
            .subgraphs
            .iter_mut()
            .find(|group| group.id == "pool")
            .unwrap()
            .metadata = Some(serde_json::json!({"laneRole": "pool"}));
        let presentation = PresentationDiagram::try_from_frozen(
            metadata,
            model,
            context,
            &InputResourcePolicy::default(),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let artifact = family::prepare(&presentation, &LayoutOptions::default(), session).unwrap();
        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let pool = document
            .descendants()
            .find(|element| {
                element.attribute("data-id") == Some("pool")
                    && element.attribute("class").is_some_and(|classes| {
                        classes.split_whitespace().any(|class| class == "pool")
                    })
            })
            .expect("pool shell");
        for class in ["pool-body", "pool-title", "pool-label"] {
            assert!(
                pool.descendants()
                    .any(
                        |element| element.attribute("class").is_some_and(|classes| classes
                            .split_whitespace()
                            .any(|candidate| candidate == class))
                    ),
                "missing {class} for {look}"
            );
        }
    }
}

#[cfg(feature = "layout-shoal")]
#[test]
fn shoal_pool_children_share_ranks_and_preserve_the_pool_endpoint() {
    use merman_model::OperationControl;
    use merman_model::presentation::{PresentationDiagram, RenderSemanticModel};
    use merman_model::resources::InputResourcePolicy;

    for direction in ["TB", "BT", "LR", "RL"] {
        let source = format!(
            "---\nconfig:\n  layout: shoal\n---\nswimlane-beta {direction}\n\
             subgraph pool[Order Handling]\n\
               subgraph sales[Sales]\nA --> C\nend\n\
               subgraph warehouse[Warehouse]\nB --> D\nend\n\
               P\nend\n\
             subgraph external[External]\nX\nend\n\
             A --> D\nP --> X\nX poolPort@--> pool\n"
        );
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let (metadata, mut model, context) = parsed.into_parts();
        let RenderSemanticModel::Flowchart(flowchart) = &mut model else {
            panic!("expected Flowchart model");
        };
        for group in &mut flowchart.subgraphs {
            group.metadata = match group.id.as_str() {
                "pool" => Some(serde_json::json!({"laneRole": "pool"})),
                "warehouse" => Some(serde_json::json!({"laneIndex": 0})),
                "external" => Some(serde_json::json!({"laneIndex": 1})),
                "sales" => Some(serde_json::json!({"laneIndex": 2})),
                _ => None,
            };
        }
        let presentation = PresentationDiagram::try_from_frozen(
            metadata,
            model,
            context,
            &InputResourcePolicy::default(),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let artifact = family::prepare(&presentation, &LayoutOptions::default(), session).unwrap();
        let layout: SwimlaneLayout = serde_json::from_value(
            artifact.layout_json().unwrap()["layout"]["SwimlaneDiagram"].clone(),
        )
        .unwrap();
        let lane = |id| layout.lanes.iter().find(|lane| lane.id == id).unwrap();
        let node = |id| layout.nodes.iter().find(|node| node.id == id).unwrap();
        let rect = |id| {
            let lane = lane(id);
            kurbo::Rect::from_center_size((lane.x, lane.y), (lane.width, lane.height))
        };
        let pool = rect("pool");
        let external = rect("external");
        let sales = rect("sales");
        let warehouse = rect("warehouse");
        for id in ["sales", "warehouse"] {
            let bounds = rect(id);
            assert_eq!(lane(id).parent_id.as_deref(), Some("pool"));
            assert!(pool.inflate(1e-8, 1e-8).contains_rect(bounds));
        }
        assert!(pool.intersect(external).area() < 1e-8);
        let horizontal = matches!(direction, "LR" | "RL");
        if horizontal {
            assert!(warehouse.y1 <= sales.y0 + 1e-8);
        } else {
            assert!(warehouse.x1 <= sales.x0 + 1e-8);
        }
        let (a, b) = (node("A"), node("B"));
        assert_eq!(a.top_lane_id.as_deref(), Some("sales"));
        assert_eq!(b.top_lane_id.as_deref(), Some("warehouse"));
        assert_eq!(a.layer, b.layer);
        assert!((if horizontal { a.x - b.x } else { a.y - b.y }).abs() < 1e-8);
        assert_eq!(node("P").parent_id.as_deref(), Some("pool"));
        assert_eq!(node("P").top_lane_id, None);
        let port = layout
            .edges
            .iter()
            .find(|edge| edge.id == "poolPort")
            .unwrap()
            .points
            .last()
            .unwrap();
        assert!(
            pool.inflate(1e-8, 1e-8)
                .contains(kurbo::Point::new(port.x, port.y))
        );
        assert!(
            (if horizontal {
                pool.y0 - port.y
            } else {
                pool.x0 - port.x
            })
            .abs()
                < 1e-8
        );

        let rendered = artifact
            .render_svg(&Default::default(), &Default::default())
            .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert!(has_class(
            semantic_element(&document, "pool", "cluster"),
            "pool"
        ));
        for id in ["warehouse", "sales"] {
            let lane = semantic_element(&document, id, "cluster");
            assert!(has_class(lane, "swimlane"), "{direction} {id}");
            assert!(!has_class(lane, "pool"), "{direction} {id}");
        }
    }
}

#[cfg(feature = "layout-shoal")]
#[test]
fn shoal_horizontal_swimlane_titles_keep_markdown_rows_in_the_measured_band() {
    for direction in ["LR", "RL"] {
        let source = format!(
            "---\nconfig:\n  layout: shoal\n  htmlLabels: false\n  flowchart:\n    htmlLabels: false\n---\n\
             swimlane-beta {direction}\n\
             subgraph Lane[\"`Customer Experience and **Self-Service Portal** for Supporting Evidence`\"]\nA\nend"
        );
        let (svg, layout) = render_swimlane_with_layout(&source, "svg-lane-title");
        let document = roxmltree::Document::parse(&svg).unwrap();
        let lane = &layout.lanes[0];
        let title = lane.title_rect.as_ref().unwrap();
        assert!(title.top >= lane.y - lane.height / 2.0);
        assert!(title.bottom <= lane.y + lane.height / 2.0);
        let label = semantic_element(&document, "Lane", "cluster")
            .descendants()
            .find(|node| has_class(*node, "swimlane-label"))
            .unwrap();
        assert!(
            label
                .attribute("transform")
                .unwrap()
                .contains("rotate(-90)")
        );
        assert!(
            label
                .descendants()
                .filter(|node| has_class(*node, "row"))
                .count()
                > 1
        );
        assert!(
            label
                .descendants()
                .any(|node| node.attribute("font-weight") == Some("bold"))
        );
        assert!(text_content(label).contains("Supporting Evidence"));
    }
}

#[cfg(feature = "layout-shoal")]
#[test]
fn shoal_swimlane_line_hops_modify_only_the_painted_straights() {
    use svgtypes::{PathParser, PathSegment};

    let render = |mode: &str| {
        render_swimlane(
            &format!(
                "---\nconfig:\n  layout: shoal\n  htmlLabels: false\n  swimlane:\n    lineHops: {mode}\n---\n\
                 swimlane-beta TB\nsubgraph Lane1\nA\nB\nend\nsubgraph Lane2\nC\nD\nend\n\
                 C inbound@--> B\nA cross@--> D\nA local@--> B\nD return@--> A"
            ),
            "swimlane-line-hops",
        )
    };
    let disabled = render("false");
    let arcs = render("true");
    let gaps = render("gap");
    let disabled = roxmltree::Document::parse(&disabled).unwrap();
    let arcs = roxmltree::Document::parse(&arcs).unwrap();
    let gaps = roxmltree::Document::parse(&gaps).unwrap();
    let read = |edge: roxmltree::Node<'_, '_>| {
        PathParser::from(edge.attribute("d").unwrap())
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    };
    let bends = |path: &[PathSegment]| {
        path.iter()
            .filter(|segment| {
                matches!(
                    segment,
                    PathSegment::EllipticalArc { .. } | PathSegment::CurveTo { .. }
                )
            })
            .copied()
            .collect::<Vec<_>>()
    };
    let moves = |path: &[PathSegment]| {
        path.iter()
            .filter(|command| matches!(command, PathSegment::MoveTo { .. }))
            .count()
    };

    let mut hopped = 0;
    for id in ["inbound", "cross", "local", "return"] {
        let base = semantic_element(&disabled, id, "edge");
        let arc = semantic_element(&arcs, id, "edge");
        let gap = semantic_element(&gaps, id, "edge");
        for candidate in [arc, gap] {
            assert_eq!(
                candidate.attribute("marker-end"),
                base.attribute("marker-end")
            );
            assert_eq!(
                candidate.attribute("data-points"),
                base.attribute("data-points")
            );
        }
        let (base, arc, gap) = (read(base), read(arc), read(gap));
        for path in [&arc, &gap] {
            assert_eq!(path.first(), base.first());
            assert_eq!(path.last(), base.last());
        }
        let expected_bends = bends(&base);
        assert_eq!(bends(&gap), expected_bends);
        let arc_bends = bends(&arc);
        // The fixture has ordinary rounded elbows. Their parameters and order must survive.
        let mut remaining = arc_bends.iter();
        assert!(
            expected_bends
                .iter()
                .all(|bend| remaining.any(|other| other == bend))
        );
        assert_eq!(moves(&arc), 1);
        assert_eq!(moves(&gap) - 1, arc_bends.len() - expected_bends.len());
        hopped += moves(&gap) - 1;
    }
    assert!(hopped > 0, "fixture must contain a paintable crossing");
}

#[test]
fn producer_pool_default_lane_reuse_preserves_membership_ranks_and_shells() {
    use merman_model::OperationControl;
    use merman_model::presentation::{PresentationDiagram, RenderSemanticModel};
    use merman_model::resources::InputResourcePolicy;

    let cases = ["classic", "handDrawn"].into_iter().flat_map(|look| {
        ["TB", "LR"].into_iter().flat_map(move |direction| {
            [false, true]
                .into_iter()
                .map(move |loose| (look, direction, loose))
        })
    });
    for canonical in [false, true] {
        if canonical && !cfg!(feature = "layout-shoal") {
            continue;
        }
        let layout_config = if canonical {
            r#", "layout": "shoal""#
        } else {
            ""
        };
        for (look, direction, with_loose_node) in cases.clone() {
            let loose_source = if with_loose_node { "B\nA --> B\n" } else { "" };
            let source = format!(
                "%%{{init: {{\"look\": \"{look}\", \"handDrawnSeed\": 7{layout_config}}}}}%%\nswimlane-beta {direction}\nsubgraph __swimlane_default__[Declared pool]\nsubgraph L[Child lane]\nA\nend\nend\n{loose_source}"
            );
            let parsed = MermaidEngine::new()
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap()
                .into_presentation();
            let (metadata, mut model, context) = parsed.into_parts();
            let RenderSemanticModel::Flowchart(flowchart) = &mut model else {
                panic!("expected Flowchart model");
            };
            flowchart
                .subgraphs
                .iter_mut()
                .find(|group| group.id == "__swimlane_default__")
                .unwrap()
                .metadata = Some(serde_json::json!({"laneRole": "pool"}));
            let presentation = PresentationDiagram::try_from_frozen(
                metadata,
                model,
                context,
                &InputResourcePolicy::default(),
                &OperationControl::new(),
            )
            .unwrap()
            .unwrap();
            let session = RenderEnvironment::deterministic().begin_session().unwrap();
            let artifact =
                family::prepare(&presentation, &LayoutOptions::default(), session).unwrap();
            let layout: SwimlaneLayout = serde_json::from_value(
                artifact.layout_json().unwrap()["layout"]["SwimlaneDiagram"].clone(),
            )
            .unwrap();
            let task = layout.nodes.iter().find(|node| node.id == "A").unwrap();
            assert_eq!(task.parent_id.as_deref(), Some("L"));
            assert_eq!(task.top_lane_id.as_deref(), Some("L"));
            assert_eq!(task.layer, 0);
            if with_loose_node {
                let loose = layout.nodes.iter().find(|node| node.id == "B").unwrap();
                assert_eq!(loose.parent_id.as_deref(), Some("__swimlane_default__"));
                assert_eq!(loose.top_lane_id, None);
                assert_eq!(loose.layer, usize::from(canonical));
                assert_eq!(layout.edges.len(), 1);
                assert_eq!(layout.edges[0].from, "A");
                assert_eq!(layout.edges[0].to, "B");
                assert!(layout.edges[0].points.len() >= 2);
            }
            let rendered = artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let default_group = semantic_element(&document, "__swimlane_default__", "cluster");
            assert!(has_class(default_group, "swimlane"));
            assert_eq!(has_class(default_group, "pool"), !with_loose_node);
            for (lane_class, pool_class) in [
                ("swimlane-body", "pool-body"),
                ("swimlane-title", "pool-title"),
                ("swimlane-label", "pool-label"),
            ] {
                assert!(
                    default_group
                        .children()
                        .any(|node| has_class(node, lane_class))
                );
                assert_eq!(
                    default_group
                        .children()
                        .any(|node| has_class(node, pool_class)),
                    !with_loose_node,
                );
            }
            let child_lane = semantic_element(&document, "L", "cluster");
            assert!(has_class(child_lane, "swimlane"));
            assert!(!has_class(child_lane, "pool"));
        }
    }
}

#[test]
fn annotation_layout_and_svg_use_shape_wrapping_width_for_html_and_svg_labels() {
    for html_labels in [false, true] {
        let mut expected = None;
        for wrapping_width in [80, 300] {
            let source = format!(
                "%%{{init: {{\"htmlLabels\": {html_labels}, \"flowchart\": {{\"wrappingWidth\": {wrapping_width}}}}}}}%%\nswimlane-beta TB\nsubgraph Lane\nA@{{ shape: bpmn-annotation, label: \"Review this request before approval\" }}\nend\n"
            );
            let (svg, layout) = render_swimlane_with_layout(&source, "annotation-width");
            let annotation = layout.nodes.iter().find(|node| node.id == "A").unwrap();
            let document = roxmltree::Document::parse(&svg).unwrap();
            let element = semantic_element(&document, "A", "node");
            let bounds = element
                .children()
                .find(|node| has_class(*node, "bpmn-bounds"))
                .unwrap();
            for (attribute, measured) in
                [("width", annotation.width), ("height", annotation.height)]
            {
                let drawn = bounds.attribute(attribute).unwrap().parse::<f64>().unwrap();
                assert!(
                    (drawn - measured).abs() < 0.01,
                    "HTML={html_labels}, wrappingWidth={wrapping_width}, {attribute}: layout={measured}, SVG={drawn}"
                );
            }
            let rows: Vec<_> = if html_labels {
                let foreign_object = element
                    .descendants()
                    .find(|node| node.has_tag_name("foreignObject"))
                    .unwrap();
                let div = foreign_object
                    .children()
                    .find(|node| node.has_tag_name("div"))
                    .unwrap();
                let style = div.attribute("style").unwrap();
                assert!(style.contains("max-width: 160px;"), "{style}");
                assert!(style.contains("width: 160px;"), "{style}");
                assert_eq!(foreign_object.attribute("width"), Some("160"));
                assert_eq!(annotation.label_width, 160.0);
                assert_eq!(annotation.width, 182.0);
                vec![text_content(div)]
            } else {
                assert!(
                    element
                        .descendants()
                        .all(|node| !node.has_tag_name("foreignObject"))
                );
                let rows: Vec<_> = element
                    .descendants()
                    .filter(|node| has_class(*node, "row"))
                    .map(text_content)
                    .collect();
                assert!(rows.len() >= 2);
                assert!(annotation.label_width <= 160.0);
                rows
            };
            let measured = (
                annotation.width,
                annotation.height,
                annotation.label_width,
                annotation.label_height,
                rows,
            );
            if let Some(expected) = &expected {
                assert_eq!(&measured, expected, "HTML={html_labels}");
            } else {
                expected = Some(measured);
            }
        }
    }
}

#[test]
fn long_html_edge_labels_wrap_with_the_measured_content_box() {
    for width in [96, 144] {
        let source = format!(
            r#"---
config:
  htmlLabels: true
  themeVariables:
    fontSize: 22px
  flowchart:
    wrappingWidth: {width}
---
swimlane-beta LR
A -->|First <b>review</b> the complete request before sending the final reply| B
"#
        );
        let (svg, layout) = render_swimlane_with_layout(&source, "wrapped-label");
        let document = roxmltree::Document::parse(&svg).unwrap();
        let label = semantic_element(&document, "L_A_B_0", "edge-label");
        let foreign_object = label
            .descendants()
            .find(|node| node.has_tag_name("foreignObject"))
            .unwrap();
        let div = foreign_object
            .children()
            .find(|node| node.has_tag_name("div"))
            .unwrap();
        let style = div.attribute("style").unwrap();
        assert!(
            style.contains("white-space: break-spaces"),
            "long HTML labels must wrap at {width}px: {style}"
        );
        assert!(style.contains(&format!("max-width: {width}px;")));
        assert!(style.contains(&format!("width: {width}px;")));
        assert!(
            foreign_object
                .attribute("style")
                .is_some_and(|style| style.contains("overflow: visible"))
        );
        let measured = layout.nodes.iter().find(|node| node.is_edge_label).unwrap();
        assert_eq!(
            foreign_object
                .attribute("width")
                .unwrap()
                .parse::<f64>()
                .unwrap(),
            measured.label_width
        );
        assert_eq!(
            foreign_object
                .attribute("height")
                .unwrap()
                .parse::<f64>()
                .unwrap(),
            measured.label_height
        );
        assert!(
            measured.label_height > 33.0,
            "22px labels must span multiple lines"
        );
        assert_eq!(
            text_content(foreign_object),
            "First review the complete request before sending the final reply"
        );
    }
}

#[test]
fn html_edge_label_wrapping_uses_only_the_first_mermaid_label_style() {
    for (styles, font_size, needs_wrap) in [
        (
            "linkStyle default font-size:24px,font-weight:bold\nlinkStyle 0 font-size:12px",
            24,
            true,
        ),
        ("linkStyle 0 font-size:12px,font-weight:bold", 12, false),
        (
            "linkStyle default fill:blue,font-size:24px\nlinkStyle 0 font-size:24px",
            8,
            false,
        ),
    ] {
        let source = format!(
            r#"---
config:
  htmlLabels: true
  themeVariables:
    fontSize: 8px
  flowchart:
    wrappingWidth: 144
---
swimlane-beta LR
A styled@-->|Review request| B
{styles}
"#
        );
        let (svg, layout) = render_swimlane_with_layout(&source, "styled-label");
        let document = roxmltree::Document::parse(&svg).unwrap();
        let label = semantic_element(&document, "styled", "edge-label");
        let foreign_object = label
            .descendants()
            .find(|node| node.has_tag_name("foreignObject"))
            .unwrap();
        let div = foreign_object
            .children()
            .find(|node| node.has_tag_name("div"))
            .unwrap();
        let style = div.attribute("style").unwrap();
        assert_eq!(
            style.contains("white-space: break-spaces"),
            needs_wrap,
            "{styles}: {style}"
        );
        assert_eq!(style.contains("white-space: nowrap"), !needs_wrap);
        assert!(
            !style.contains("font-weight"),
            "only the first style is copied"
        );
        if font_size == 8 {
            assert!(style.contains("color:blue"), "{style}");
            assert!(!style.contains("font-size"));
        } else {
            assert!(style.contains(&format!("font-size:{font_size}px")));
        }
        let measured = layout.nodes.iter().find(|node| node.is_edge_label).unwrap();
        assert_eq!(
            measured.label_height > f64::from(font_size) * 1.5,
            needs_wrap,
            "{styles}: {measured:?}"
        );
    }
}

#[test]
fn explicit_swimlane_cluster_ids_are_scoped_to_each_diagram() {
    let source = "swimlane-beta LR\nsubgraph Lane\nA --> B\nend\n";
    let mut cluster_ids = std::collections::HashSet::new();
    for diagram_id in ["first", "second"] {
        let svg = render_swimlane(source, diagram_id);
        let document = roxmltree::Document::parse(&svg).expect("valid Swimlane SVG");
        let cluster = canonical_lane_cluster(&document, diagram_id, "Lane");
        assert!(cluster_ids.insert(cluster.attribute("id").unwrap().to_owned()));
    }
    assert_eq!(
        cluster_ids.len(),
        2,
        "diagram scopes must own distinct lane IDs"
    );
}

#[test]
fn synthetic_swimlane_cluster_keeps_semantic_id_in_a_canonical_scope() {
    let svg = render_swimlane("swimlane-beta LR\nA --> B\n", "synthetic");
    let document = roxmltree::Document::parse(&svg).expect("valid synthetic-lane SVG");
    let cluster = canonical_lane_cluster(&document, "synthetic", "__swimlane_default__");
    assert_ne!(cluster.attribute("id"), cluster.attribute("data-id"));
}

#[test]
fn explicitly_declared_default_swimlane_cluster_is_scoped() {
    let source = "swimlane-beta LR\nsubgraph __swimlane_default__[Named lane]\nA --> B\nend\n";
    let svg = render_swimlane(source, "declared");
    let document = roxmltree::Document::parse(&svg).expect("valid declared-lane SVG");
    let cluster = canonical_lane_cluster(&document, "declared", "__swimlane_default__");
    let title = cluster
        .children()
        .find(|node| has_class(*node, "cluster-label"))
        .unwrap();
    assert_eq!(text_content(title), "Named lane");
}

fn canonical_lane_cluster<'a>(
    document: &'a roxmltree::Document<'a>,
    diagram_id: &str,
    semantic_id: &str,
) -> roxmltree::Node<'a, 'a> {
    let cluster = canonical_cluster(document, diagram_id, semantic_id);
    assert!(has_class(cluster, "cluster") && has_class(cluster, "swimlane"));
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
        "duplicate SVG DOM IDs"
    );
    cluster
}

fn semantic_element<'a>(
    document: &'a roxmltree::Document<'a>,
    semantic_id: &str,
    element_type: &str,
) -> roxmltree::Node<'a, 'a> {
    let mut matches = document.descendants().filter(|node| {
        node.attribute("data-id") == Some(semantic_id)
            && node.attribute("data-et") == Some(element_type)
    });
    let element = matches
        .next()
        .unwrap_or_else(|| panic!("missing {element_type} {semantic_id}"));
    assert!(
        matches.next().is_none(),
        "duplicate {element_type} {semantic_id}"
    );
    element
}

fn assert_canonical_id(node: roxmltree::Node<'_, '_>, diagram_id: &str, local_kind: &str) {
    let prefix = format!("{diagram_id}-merman-flowchart-document-{local_kind}-");
    let ordinal = node
        .attribute("id")
        .unwrap()
        .strip_prefix(&prefix)
        .expect("DOM identity belongs to its canonical diagram namespace and element owner");
    assert!(!ordinal.is_empty() && ordinal.bytes().all(|byte| byte.is_ascii_digit()));
}

fn canonical_cluster<'a>(
    document: &'a roxmltree::Document<'a>,
    diagram_id: &str,
    semantic_id: &str,
) -> roxmltree::Node<'a, 'a> {
    let cluster = semantic_element(document, semantic_id, "cluster");
    assert!(has_class(cluster, "cluster"));
    assert_canonical_id(
        cluster,
        diagram_id,
        if has_class(cluster, "swimlane") {
            "lane"
        } else {
            "cluster"
        },
    );
    cluster
}

fn has_class(node: roxmltree::Node<'_, '_>, class: &str) -> bool {
    node.attribute("class")
        .is_some_and(|classes| classes.split_ascii_whitespace().any(|value| value == class))
}

fn element_by_id<'a>(document: &'a roxmltree::Document<'a>, id: &str) -> roxmltree::Node<'a, 'a> {
    document
        .descendants()
        .find(|node| node.attribute("id") == Some(id))
        .unwrap_or_else(|| panic!("missing element {id}"))
}

fn node_element<'a>(
    document: &'a roxmltree::Document<'a>,
    diagram_id: &str,
    node_id: &str,
) -> roxmltree::Node<'a, 'a> {
    let node = semantic_element(document, node_id, "node");
    assert!(has_class(node, "node"));
    assert_canonical_id(node, diagram_id, "node");
    node
}

fn text_content(node: roxmltree::Node<'_, '_>) -> String {
    node.descendants()
        .filter_map(|child| child.is_text().then(|| child.text()).flatten())
        .collect()
}

#[test]
fn synthetic_edge_labels_keep_layout_anchors_inside_the_root() {
    for (source, edge_id) in [
        (
            include_str!("../../../fixtures/swimlane/upstream_7_car_sales_constr.mmd"),
            "L_I_K_0",
        ),
        (
            include_str!("../../../fixtures/swimlane/upstream_docs_basic.mmd"),
            "L_triage_investigate_0",
        ),
    ] {
        let (svg, layout) = render_swimlane_with_layout(source, "pinned-label");
        let document = roxmltree::Document::parse(&svg).unwrap();
        let edge = layout.edges.iter().find(|edge| edge.id == edge_id).unwrap();
        assert!(edge.points.len() >= 2);
        assert!(
            edge.points
                .iter()
                .all(|point| point.x.is_finite() && point.y.is_finite())
        );
        let label_id = edge.label_node_id.as_deref().unwrap();
        let label = layout
            .nodes
            .iter()
            .find(|node| node.id == label_id)
            .unwrap();
        assert!(label.is_edge_label);
        let element = semantic_element(&document, edge_id, "edge-label");
        assert_canonical_id(element, "pinned-label", "synthetic-label");
        let coordinates = element
            .attribute("transform")
            .unwrap()
            .trim_start_matches("translate(")
            .trim_end_matches(')')
            .split(',')
            .map(|coordinate| coordinate.trim().parse::<f64>().unwrap())
            .collect::<Vec<_>>();
        assert!((coordinates[0] - label.x).abs() < 1e-6, "{edge_id} x");
        assert!((coordinates[1] - label.y).abs() < 1e-6, "{edge_id} y");
        let viewport = document
            .root_element()
            .attribute("viewBox")
            .unwrap()
            .split_ascii_whitespace()
            .map(|value| value.parse::<f64>().unwrap())
            .collect::<Vec<_>>();
        assert!(coordinates[0] - label.label_width / 2.0 >= viewport[0]);
        assert!(coordinates[1] - label.label_height / 2.0 >= viewport[1]);
        assert!(coordinates[0] + label.label_width / 2.0 <= viewport[0] + viewport[2]);
        assert!(coordinates[1] + label.label_height / 2.0 <= viewport[1] + viewport[3]);
    }
}

#[test]
fn synthetic_lane_palette_follows_visible_container_declaration_slots() {
    let reference = roxmltree::Document::parse(NESTED_MIXED_UPSTREAM_SVG).unwrap();
    let reference_diagram_id = "local_nested_mixed_lane_security";
    // The Mermaid 12.2 SVG records preorder slots for Outer and ordinary Inner;
    // prepareLayoutForSwimlanes then appends the synthetic lane after both slots.
    for (id, expected) in [
        (format!("{reference_diagram_id}-Outer"), "color-0"),
        (format!("{reference_diagram_id}-Inner"), "color-1"),
        ("__swimlane_default__".to_string(), "color-2"),
    ] {
        assert_eq!(
            element_by_id(&reference, &id).attribute("data-color-id"),
            Some(expected)
        );
    }
    for theme in ["redux-color", "redux-dark-color"] {
        for html_labels in [false, true] {
            let source = NESTED_MIXED
                .replace(
                    "securityLevel: strict",
                    &format!("theme: {theme}\n  look: neo\n  securityLevel: strict"),
                )
                .replace("htmlLabels: false", &format!("htmlLabels: {html_labels}"));
            let diagram_id = "nested-colors";
            let svg = render_swimlane(&source, diagram_id);
            let document = roxmltree::Document::parse(&svg).unwrap();
            for id in ["Outer", "Inner", "__swimlane_default__"] {
                let scoped_id = |diagram_id: &str| {
                    if id == "__swimlane_default__" {
                        id.to_string()
                    } else {
                        format!("{diagram_id}-{id}")
                    }
                };
                assert_eq!(
                    canonical_cluster(&document, diagram_id, id).attribute("data-color-id"),
                    element_by_id(&reference, &scoped_id(reference_diagram_id))
                        .attribute("data-color-id"),
                    "{theme}, htmlLabels={html_labels}, {id}"
                );
            }
        }
    }
}

#[test]
fn distinct_nested_and_synthetic_lanes_preserve_ownership_ids_and_cross_lane_routes() {
    // Upstream prepares only top-level groups as Swimlanes, then synthesizes the
    // loose-node lane after render.ts assigns DOM IDs to the explicit groups.
    for (direction, expected_direction) in
        [("LR", SwimlaneDirection::Lr), ("RL", SwimlaneDirection::Rl)]
    {
        let source =
            NESTED_MIXED.replace("swimlane-beta LR", &format!("swimlane-beta {direction}"));
        for diagram_id in ["nested-first", "nested-second"] {
            let (svg, layout) = render_swimlane_with_layout(&source, diagram_id);
            assert_eq!(layout.direction, expected_direction);
            let document = roxmltree::Document::parse(&svg).expect("valid mixed-lane SVG");
            let outer = canonical_lane_cluster(&document, diagram_id, "Outer");
            let inner = canonical_cluster(&document, diagram_id, "Inner");
            let synthetic = canonical_lane_cluster(&document, diagram_id, "__swimlane_default__");
            assert!(has_class(outer, "cluster") && has_class(outer, "swimlane"));
            assert!(has_class(inner, "cluster") && !has_class(inner, "swimlane"));
            assert!(has_class(synthetic, "cluster") && has_class(synthetic, "swimlane"));
            assert_eq!(outer.attribute("data-id"), Some("Outer"));
            assert_eq!(synthetic.attribute("data-id"), Some("__swimlane_default__"));
            let ids = document
                .descendants()
                .filter_map(|node| node.attribute("id"))
                .collect::<Vec<_>>();
            let unique = ids
                .iter()
                .copied()
                .collect::<std::collections::HashSet<_>>();
            assert_eq!(ids.len(), unique.len(), "duplicate DOM IDs");

            let lane = |id: &str| layout.lanes.iter().find(|lane| lane.id == id).unwrap();
            assert!(lane("Outer").parent_id.is_none());
            assert_eq!(lane("Inner").parent_id.as_deref(), Some("Outer"));
            assert!(lane("__swimlane_default__").parent_id.is_none());
            let node = |id: &str| layout.nodes.iter().find(|node| node.id == id).unwrap();
            for id in ["A", "B"] {
                assert_eq!(node(id).parent_id.as_deref(), Some("Inner"));
                assert_eq!(node(id).top_lane_id.as_deref(), Some("Outer"));
                node_element(&document, diagram_id, id);
            }
            assert_eq!(node("C").parent_id.as_deref(), Some("__swimlane_default__"));
            assert_eq!(
                node("C").top_lane_id.as_deref(),
                Some("__swimlane_default__")
            );
            node_element(&document, diagram_id, "C");
            if direction == "LR" {
                assert!(node("A").x < node("B").x);
            } else {
                assert!(node("A").x > node("B").x);
            }
            let edge = layout
                .edges
                .iter()
                .find(|edge| edge.from == "B" && edge.to == "C")
                .unwrap();
            assert!(edge.points.len() >= 2);
            assert!(
                edge.points
                    .iter()
                    .all(|point| point.x.is_finite() && point.y.is_finite())
            );
            assert!(edge.points.windows(2).all(|pair| {
                (pair[0].x - pair[1].x).abs() < 1e-6 || (pair[0].y - pair[1].y).abs() < 1e-6
            }));
            for (id, endpoint) in [
                ("B", edge.points.first().unwrap()),
                ("C", edge.points.last().unwrap()),
            ] {
                let bounds = node(id);
                let dx = (endpoint.x - bounds.x).abs();
                let dy = (endpoint.y - bounds.y).abs();
                assert!(dx <= bounds.width / 2.0 + 1e-6 && dy <= bounds.height / 2.0 + 1e-6);
                assert!(
                    (dx - bounds.width / 2.0).abs() < 1e-6
                        || (dy - bounds.height / 2.0).abs() < 1e-6,
                    "{id}: {endpoint:?} is not on the node boundary"
                );
            }
            assert!(document.descendants().any(|node| {
                node.has_tag_name("path")
                    && node.attribute("data-id") == Some(edge.id.as_str())
                    && node.attribute("data-points").is_some()
                    && node
                        .attribute("d")
                        .is_some_and(|path| path.starts_with('M'))
            }));
        }
    }
}

#[test]
fn nested_mixed_lane_titles_and_nodes_escape_entities_and_remove_strict_unsafe_links() {
    for html_labels in [false, true] {
        let source =
            NESTED_MIXED.replace("htmlLabels: false", &format!("htmlLabels: {html_labels}"));
        let diagram_id = "nested-strict";
        let svg = render_swimlane(&source, diagram_id);
        let document = roxmltree::Document::parse(&svg).expect("valid escaped mixed-lane SVG");
        for (id, expected) in [("Outer", "Outer & < lane"), ("Inner", "Inner & < group")] {
            let cluster = canonical_cluster(&document, diagram_id, id);
            let title = cluster
                .children()
                .find(|node| has_class(*node, "cluster-label"))
                .unwrap();
            assert_eq!(text_content(title), expected);
        }
        for (id, expected) in [
            ("A", "A & < node"),
            ("B", "B & < node"),
            ("C", "Loose & < node"),
        ] {
            assert_eq!(
                text_content(node_element(&document, diagram_id, id)),
                expected
            );
        }
        assert!(document.descendants().all(|node| {
            !node.has_tag_name("script")
                && node.attributes().all(|attribute| {
                    !attribute.name().starts_with("on")
                        && !attribute.value().contains("javascript:")
                })
        }));
        let linked = node_element(&document, diagram_id, "A");
        let anchor = linked
            .ancestors()
            .find(|node| node.has_tag_name("a"))
            .expect("declared link keeps the upstream anchor wrapper");
        assert!(
            anchor
                .attributes()
                .all(|attribute| attribute.name() != "href")
        );
        assert_eq!(
            linked
                .descendants()
                .any(|node| node.has_tag_name("foreignObject")),
            html_labels
        );
    }
}
