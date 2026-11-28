#![cfg(feature = "diagram-er")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_model::presentation::PresentationDiagram;
use merman_render::environment::RenderEnvironment;
use merman_render::{DiagramFamilyId, Error, RenderCapability, family};

// Mermaid 12.4.0 e2e/diagrams/grid/pr-check/05-er-authored-placement-ids.mmd.
const AUTHORED_PLACEMENTS: &str = "\
---
description:
  An ER diagram whose entities are placed with a placement map by their
  authored ids, with cardinality markers on each edge.
config:
  layout: grid
  grid:
    columnGap: 150
    rowGap: 100
    placements:
      CUSTOMER: { row: 1, column: 1 }
      ORDER: { row: 1, column: 2 }
      PAYMENT: { row: 2, column: 2 }
      LINE_ITEM: { row: 1, column: 3 }
---
erDiagram
  CUSTOMER ||--o{ ORDER : places
  ORDER ||--|{ LINE_ITEM : contains
  ORDER ||--o| PAYMENT : has
";

#[cfg(feature = "layout-grid")]
const TABLES: &str = r#"---
config:
  layout: grid
  look: classic
  htmlLabels: false
  flowchart:
    htmlLabels: true
  er:
    minEntityWidth: 50
    diagramPadding: 16
    entityPadding: 8
  grid:
    curve: linear
    lineHops: false
    placements:
      ACCOUNT: { row: 3, column: 2 }
      ENTRY: { row: 3, column: -1 }
      entity-ACCOUNT-0: { row: 8, column: -8 }
      "Customer ledger": { row: 8, column: -9 }
---
erDiagram
    ACCOUNT["Customer ledger"] {
        string account_id PK "Account number"
        string outstanding_balance
    }
    ENTRY {
        int id PK
    }
    ACCOUNT ||..o{ ENTRY : "settles entries"
"#;

fn parse(source: &str) -> PresentationDiagram {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse ER fixture")
        .expect("detect ER")
        .into_presentation();
    assert_eq!(parsed.family_id(), Some(DiagramFamilyId::ER));
    assert_eq!(parsed.metadata().diagram_type, "er");
    parsed
}

#[test]
fn default_and_explicit_er_backend_selection_keep_their_existing_mapping() {
    let environment = RenderEnvironment::deterministic();
    let session = environment.begin_session().unwrap();
    // The reference's default is ELK. A Dagre-only Merman build uses its existing fallback.
    let elk = if cfg!(feature = "layout-elk") || !cfg!(feature = "layout-dagre") {
        RenderCapability::LayoutElk
    } else {
        RenderCapability::LayoutDagre
    };
    for (source, expected) in [
        (AUTHORED_PLACEMENTS.replace("  layout: grid\n", ""), elk),
        (
            AUTHORED_PLACEMENTS.replace("layout: grid", "layout: dagre"),
            RenderCapability::LayoutDagre,
        ),
        (
            AUTHORED_PLACEMENTS.replace("layout: grid", "layout: elk"),
            elk,
        ),
    ] {
        let plan = family::plan_render(&parse(&source), &session).unwrap();
        assert_eq!(plan.family_id(), DiagramFamilyId::ER);
        assert_eq!(plan.required_capabilities(), [expected]);
    }
}

#[cfg(not(feature = "layout-grid"))]
#[test]
fn explicit_er_grid_requires_a_compiled_provider() {
    let parsed = parse(AUTHORED_PLACEMENTS);
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    assert!(matches!(
        family::plan_render(&parsed, &session),
        Err(Error::UnsupportedLayoutBackend { backend: "grid", diagram_type }) if diagram_type == "er"
    ));
}

#[cfg(feature = "layout-grid")]
mod grid {
    use super::*;
    use merman_render::family::FamilyRenderArtifact;
    use merman_render::model::{ErDiagramLayout, LayoutNode};
    use merman_render::svg::SvgRenderOptions;
    use merman_render::{LayoutOptions, RenderCapabilityPolicy};

    fn prepare(parsed: &PresentationDiagram) -> FamilyRenderArtifact<'_> {
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        family::prepare(parsed, &LayoutOptions::default(), session).expect("prepare ER")
    }

    fn layout(artifact: &FamilyRenderArtifact<'_>) -> ErDiagramLayout {
        let geometry = artifact.layout_geometry_json().unwrap();
        assert_eq!(geometry["family_id"], "er");
        assert_eq!(geometry["diagram_type"], "er");
        let snapshot = artifact.layout_json().unwrap();
        assert_eq!(geometry["layout"], snapshot["layout"]);
        serde_json::from_value(geometry["layout"]["ErDiagram"].clone()).unwrap()
    }

    fn node<'a>(layout: &'a ErDiagramLayout, id: &str) -> &'a LayoutNode {
        layout.nodes.iter().find(|node| node.id == id).unwrap()
    }

    fn svg(artifact: FamilyRenderArtifact<'_>) -> String {
        let rendered = artifact
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some("grid-er".into()),
                    ..Default::default()
                },
                &Default::default(),
            )
            .unwrap();
        assert_eq!(rendered.family_id(), DiagramFamilyId::ER);
        rendered.svg().to_owned()
    }

    fn has_class(node: roxmltree::Node<'_, '_>, class: &str) -> bool {
        node.attribute("class")
            .is_some_and(|classes| classes.split_ascii_whitespace().any(|word| word == class))
    }

    #[test]
    fn pinned_er_grid_fixture_uses_authored_placements_with_ordinary_er_edge_ids() {
        let parsed = parse(AUTHORED_PLACEMENTS);
        let environment = RenderEnvironment::deterministic();
        let session = environment.begin_session().unwrap();
        let plan = family::plan_render(&parsed, &session).unwrap();
        assert_eq!(plan.family_id(), DiagramFamilyId::ER);
        assert_eq!(plan.required_capabilities(), [RenderCapability::LayoutGrid]);
        assert!(plan.is_ready());

        let artifact = prepare(&parsed);
        let layout = layout(&artifact);
        assert_eq!(layout.nodes.len(), 4);
        let customer = node(&layout, "entity-CUSTOMER-0");
        let order = node(&layout, "entity-ORDER-1");
        let line_item = node(&layout, "entity-LINE_ITEM-2");
        let payment = node(&layout, "entity-PAYMENT-3");
        assert!(customer.x < order.x && order.x < line_item.x);
        assert!((customer.y - order.y).abs() < 1e-6);
        assert!((order.y - line_item.y).abs() < 1e-6);
        assert!((order.x - payment.x).abs() < 1e-6);
        assert!(order.y < payment.y);

        let endpoints = [
            (&customer.id, &order.id, "ZERO_OR_MORE_END"),
            (&order.id, &line_item.id, "ONE_OR_MORE_END"),
            (&order.id, &payment.id, "ZERO_OR_ONE_END"),
        ];
        assert_eq!(layout.edges.len(), endpoints.len());
        for (index, (edge, (source, target, end_marker))) in
            layout.edges.iter().zip(endpoints).enumerate()
        {
            assert_eq!(edge.id, format!("er-rel-{index}"));
            assert_eq!((&edge.from, &edge.to), (source, target));
            assert_eq!(edge.start_marker.as_deref(), Some("ONLY_ONE_START"));
            assert_eq!(edge.end_marker.as_deref(), Some(end_marker));
            assert!(edge.label.is_some());
        }

        let rendered = svg(artifact);
        let doc = roxmltree::Document::parse(&rendered).unwrap();
        assert_eq!(
            doc.root_element().attribute("aria-roledescription"),
            Some("er")
        );
        assert!(
            doc.root_element()
                .attribute("data-layout-fallback")
                .is_none()
        );
        let dom_edges = doc
            .descendants()
            .filter(|node| node.attribute("data-et") == Some("edge"))
            .collect::<Vec<_>>();
        assert_eq!(dom_edges.len(), layout.edges.len());
        for (index, (path, edge)) in dom_edges.iter().zip(&layout.edges).enumerate() {
            let id = format!("id_{}_{}_{index}", edge.from, edge.to);
            assert_eq!(path.attribute("data-id"), Some(id.as_str()));
            assert_eq!(
                path.attribute("marker-start"),
                Some("url(#grid-er_er-onlyOneStart)")
            );
            assert!(path.attribute("marker-end").is_some());
        }

        let denied = environment
            .with_capability_policy(
                RenderCapabilityPolicy::unrestricted().with_denied(RenderCapability::LayoutGrid),
            )
            .begin_session()
            .unwrap();
        let plan = family::plan_render(&parsed, &denied).unwrap();
        assert_eq!(plan.missing_capabilities(), [RenderCapability::LayoutGrid]);
        assert!(matches!(
            family::prepare(&parsed, &LayoutOptions::default(), denied),
            Err(Error::MissingCapability { capability: RenderCapability::LayoutGrid, diagram_type })
                if diagram_type == "er"
        ));
    }

    #[test]
    fn grid_reuses_measured_er_tables_and_paints_plain_relationship_label_rects() {
        let parsed = parse(TABLES);
        let artifact = prepare(&parsed);
        let measured = layout(&artifact);
        let account = node(&measured, "entity-ACCOUNT-0");
        let entry = node(&measured, "entity-ENTRY-1");
        assert!((account.y - entry.y).abs() < 1e-6);
        assert!(account.x > entry.x);
        assert!(account.width > entry.width && account.height > entry.height);
        let [edge] = measured.edges.as_slice() else {
            panic!("one table relationship");
        };
        let first = edge.points.first().unwrap();
        let last = edge.points.last().unwrap();
        assert!((first.x - (account.x - account.width / 2.0)).abs() < 1e-6);
        assert!((last.x - (entry.x + entry.width / 2.0)).abs() < 1e-6);
        assert_eq!(edge.stroke_dasharray.as_deref(), Some("8,8"));

        let wrong_keys = TABLES
            .replace("      ACCOUNT: { row: 3, column: 2 }\n", "")
            .replace("      ENTRY: { row: 3, column: -1 }\n", "");
        let unplaced = layout(&prepare(&parse(&wrong_keys)));
        assert!(node(&unplaced, &account.id).x < node(&unplaced, &entry.id).x);

        for (backend, available) in [
            ("dagre", cfg!(feature = "layout-dagre")),
            ("elk", cfg!(feature = "layout-elk")),
        ] {
            if available {
                let layered = layout(&prepare(&parse(
                    &TABLES.replace("layout: grid", &format!("layout: {backend}")),
                )));
                for expected in [account, entry] {
                    let actual = node(&layered, &expected.id);
                    assert_eq!(
                        (actual.width, actual.height),
                        (expected.width, expected.height)
                    );
                }
                assert!(node(&layered, &account.id).y < node(&layered, &entry.id).y);
            }
        }

        let html_tables = TABLES.replace(
            "htmlLabels: false\n  flowchart:\n    htmlLabels: true",
            "htmlLabels: true\n  flowchart:\n    htmlLabels: false",
        );
        for (rendered, html) in [
            (svg(artifact), false),
            (svg(prepare(&parse(&html_tables))), true),
        ] {
            let doc = roxmltree::Document::parse(&rendered).unwrap();
            let edge_labels = doc
                .descendants()
                .find(|node| node.attribute("class") == Some("edgeLabels"))
                .unwrap();
            assert!(!edge_labels.children().any(|child| child.is_element()));
            let helper = doc
                .descendants()
                .find(|node| node.attribute("class") == Some("label edgeLabel"))
                .unwrap();
            assert_eq!(
                helper.parent_element().unwrap().attribute("class"),
                Some("nodes")
            );
            assert_eq!(
                helper.attribute("id"),
                Some(
                    "edge-label-entity-ACCOUNT-0-entity-ENTRY-1-id_entity-ACCOUNT-0_entity-ENTRY-1_0"
                )
            );
            assert_eq!(
                helper
                    .descendants()
                    .any(|node| node.has_tag_name("foreignObject")),
                html
            );
            let text = helper
                .descendants()
                .filter(roxmltree::Node::is_text)
                .filter_map(|node| node.text())
                .collect::<String>();
            assert_eq!(text, "settles entries");
            let attributes = doc
                .descendants()
                .filter(|node| has_class(*node, "attribute-name"))
                .count();
            assert_eq!(attributes, 3);
            let path = doc
                .descendants()
                .find(|node| node.attribute("data-et") == Some("edge"))
                .unwrap();
            let shaft = path.attribute("d").unwrap();
            assert!(shaft.starts_with('M') && !shaft.contains(['C', 'Q']));
        }
    }
}
