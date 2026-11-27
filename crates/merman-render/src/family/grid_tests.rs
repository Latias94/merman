use super::*;
use crate::environment::RenderEnvironment;
use crate::resources::{RenderResourcePolicy, ResourceLimitId};
use crate::{RenderCapability, RenderCapabilityPolicy};
use merman_core::{MermaidEngine, ParseOptions};

#[test]
fn grid_layout_and_svg_share_signed_placements_and_edge_labels() {
    let source = "\
---
config:
  layout: grid
  htmlLabels: false
  grid:
    curve: linear
    lineHops: false
---
flowchart LR
A[Request]
B[Answer]
A@{ row: 0, column: 2 }
B@{ row: 0, column: -1 }
A approval@-->|approved| B
";
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let policy = RenderResourcePolicy::unbounded_for_trusted_input();
    let environment = RenderEnvironment::deterministic().with_resource_policy(policy);
    let options = LayoutOptions::default();
    let session = environment.begin_session().unwrap();
    let plan = plan_render(&parsed, &session).unwrap();
    assert_eq!(plan.required_capabilities(), [RenderCapability::LayoutGrid]);
    assert!(plan.is_ready());
    let meter = Arc::clone(session.work_meter());
    let artifact = prepare(&parsed, &options, session).unwrap();
    let BuiltinFamilyArtifact::Flowchart(flow) = &artifact.family else {
        panic!("expected a Flowchart artifact");
    };
    let layout = flow.pair().layout();
    let [a, b] = layout.nodes.as_slice() else {
        panic!("expected two Grid items");
    };
    assert_eq!((a.id.as_str(), b.id.as_str()), ("A", "B"));
    assert!(a.x > b.x);
    assert_eq!(a.y, b.y);
    let [edge] = layout.edges.as_slice() else {
        panic!("expected one Grid route");
    };
    assert_eq!(edge.id, "approval");
    let label = edge.label.as_ref().expect("Grid edge label");
    assert!(b.x < label.x && label.x < a.x);

    let limit = meter.used();
    let limited = |max| {
        RenderEnvironment::deterministic()
            .with_resource_policy(
                policy
                    .with_limit(ResourceLimitId::MaxLayoutWorkUnits, max)
                    .unwrap(),
            )
            .begin_session()
            .unwrap()
    };
    assert!(prepare(&parsed, &options, limited(limit)).is_ok());
    assert!(matches!(
        prepare(&parsed, &options, limited(limit - 1)),
        Err(Error::ResourceLimitExceeded(_))
    ));

    let svg = artifact
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
    let document = roxmltree::Document::parse(svg.svg()).unwrap();
    let shaft = document
        .descendants()
        .find(|node| node.has_tag_name("path") && node.attribute("data-id") == Some("approval"))
        .expect("Grid shaft");
    assert!(shaft.attribute("data-points").is_some());
    let edge_labels = document
        .descendants()
        .find(|node| node.attribute("class") == Some("edgeLabels"))
        .expect("edge label layer");
    assert!(!edge_labels.children().any(|node| node.is_element()));
    let helper = document
        .descendants()
        .find(|node| {
            node.attribute("data-et") == Some("edge-label")
                && node.attribute("data-id") == Some("approval")
        })
        .expect("Grid labelRect in the node layer");
    assert_eq!(helper.parent().unwrap().attribute("class"), Some("nodes"));
    assert!(
        helper
            .descendants()
            .any(|node| node.text() == Some("approved"))
    );

    let denied = environment
        .with_capability_policy(
            RenderCapabilityPolicy::unrestricted().with_denied(RenderCapability::LayoutGrid),
        )
        .begin_session()
        .unwrap();
    assert!(matches!(
        prepare(&parsed, &options, denied),
        Err(Error::MissingCapability {
            capability: RenderCapability::LayoutGrid,
            ..
        })
    ));
}

#[test]
fn grid_label_rect_measures_only_the_style_it_emits() {
    let measure_and_render = |styles| {
        let source = format!(
            "---\nconfig:\n  layout: grid\n  htmlLabels: false\n---\nflowchart LR\nA review@-->|Review| B\n{styles}"
        );
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let artifact = prepare(&parsed, &LayoutOptions::default(), session).unwrap();
        let BuiltinFamilyArtifact::Flowchart(flow) = &artifact.family else {
            panic!("expected a Flowchart artifact");
        };
        let label = flow.pair().layout().edges[0].label.as_ref().unwrap();
        let size = (label.width, label.height);
        let svg = artifact
            .render_svg(&Default::default(), &Default::default())
            .unwrap();
        let document = roxmltree::Document::parse(svg.svg()).unwrap();
        let helper_text = document
            .descendants()
            .find(|node| node.attribute("data-et") == Some("edge-label"))
            .unwrap()
            .descendants()
            .find(|node| node.has_tag_name("text"))
            .unwrap();
        (size, helper_text.attribute("style").unwrap().to_owned())
    };

    let (small, small_style) = measure_and_render("linkStyle 0 font-size:16px\n");
    assert!(small_style.contains("font-size:16px"));
    assert_eq!(
        measure_and_render("linkStyle 0 font-size:16px,font-size:96px\n"),
        (small, small_style)
    );
    let (large, large_style) =
        measure_and_render("linkStyle default font-size:32px\nlinkStyle 0 font-size:96px\n");
    assert!(large.0 > small.0 && large.1 > small.1);
    assert!(large_style.contains("font-size:32px"));
    assert!(!large_style.contains("96px"));
}

#[cfg(feature = "diagram-agentflow")]
#[test]
fn grid_retains_agentflow_geometry_and_family_identity() {
    // Mermaid 12.4.0 e2e/diagrams/grid/nightly-check/01-agentflow-inline-placement.mmd.
    let source = "\
---
config:
  layout: grid
---
agentflow-beta
  request[Receive request]@{ shape: input, row: 1, column: 1 }
  plan[Plan work]@{ shape: task, row: 1, column: 2 }
  run[Run tool]@{ shape: tool, row: 1, column: 3 }
  publish[Publish result]@{ shape: action, row: 1, column: 4 }
  request --> plan --> run --> publish
";
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = prepare(&parsed, &LayoutOptions::default(), session).unwrap();
    assert_eq!(artifact.family_id(), DiagramFamilyId::AGENTFLOW);
    let BuiltinFamilyArtifact::Agentflow { flow, .. } = &artifact.family else {
        panic!("expected an Agentflow artifact");
    };
    let layout = flow.pair().layout();
    assert_eq!(layout.nodes.len(), 4);
    assert_eq!(layout.edges.len(), 3);
    let nodes = ["request", "plan", "run", "publish"].map(|id| {
        layout
            .nodes
            .iter()
            .find(|node| node.id == id)
            .expect("authored node")
    });
    for [left, right] in nodes.array_windows::<2>() {
        assert!(left.x < right.x);
        assert_eq!(left.y, right.y);
    }
    let svg = artifact
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
    let document = roxmltree::Document::parse(svg.svg()).unwrap();
    assert_eq!(
        document.root_element().attribute("aria-roledescription"),
        Some("agentflow")
    );
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.has_tag_name("path") && node.attribute("data-points").is_some())
            .count(),
        3
    );
}
