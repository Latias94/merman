#![cfg(feature = "diagram-class")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_model::presentation::PresentationDiagram;
use merman_render::environment::RenderEnvironment;
use merman_render::{DiagramFamilyId, RenderCapability, family};

// Mermaid 12.4.0 e2e/diagrams/grid/pr-check/03-class-placement-map.mmd.
const AUTHORED_PLACEMENTS: &str = "\
---
description:
  A class diagram placed with a grid placement map, with labelled relations
  across rows and columns.
config:
  layout: grid
  grid:
    columnGap: 100
    rowGap: 100
    placements:
      Customer: { row: 1, column: 1 }
      Order: { row: 1, column: 2 }
      Payment: { row: 2, column: 2 }
      Shipment: { row: 1, column: 3 }
---
classDiagram
  Customer --> Order : places
  Customer --> Payment : makes
  Order --> Payment : pays with
  Order --> Shipment : creates
";

fn parse(source: &str) -> PresentationDiagram {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse Class fixture")
        .expect("detect Class")
        .into_presentation();
    assert_eq!(parsed.family_id(), Some(DiagramFamilyId::CLASS));
    assert_eq!(parsed.metadata().diagram_type, "classDiagram");
    parsed
}

#[test]
fn explicit_grid_preserves_class_default_dagre_and_elk_selection() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let elk = if merman_render::layout_elk_available() || !merman_render::layout_dagre_available() {
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
        assert_eq!(plan.family_id(), DiagramFamilyId::CLASS);
        assert_eq!(plan.required_capabilities(), [expected]);
    }
}

#[cfg(not(feature = "layout-grid"))]
#[test]
fn explicit_class_grid_requires_a_compiled_provider() {
    let parsed = parse(AUTHORED_PLACEMENTS);
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    assert!(matches!(
        family::plan_render(&parsed, &session),
        Err(merman_render::Error::UnsupportedLayoutBackend { backend: "grid", diagram_type })
            if diagram_type == "classDiagram"
    ));
}

#[cfg(feature = "layout-grid")]
mod grid {
    use super::*;
    use merman_render::family::FamilyRenderArtifact;
    use merman_render::model::{ClassDiagramLayout, LayoutNode};
    use merman_render::svg::SvgRenderOptions;
    use merman_render::{Error, LayoutOptions, RenderCapabilityPolicy};

    const BOXES: &str = r#"---
config:
  layout: grid
  look: classic
  htmlLabels: false
  flowchart:
    htmlLabels: true
  grid:
    curve: linear
    lineHops: false
    placements:
      Order: { row: 3, column: 2 }
      Item: { row: 3, column: -1 }
      classId-Order-0: { row: 8, column: -8 }
      "Customer orders": { row: 8, column: -9 }
---
classDiagram
  class Order["Customer orders"] {
    +String customerAccountNumber
    +submitOrder() bool
  }
  class Item {
    +int qty
  }
  Order "1" --> "*" Item : **places items**
  style Order fill:#ffe2aa,stroke:#124578
"#;

    const COMPOUND: &str = "\
---
config:
  layout: grid
  look: classic
  htmlLabels: false
  class:
    hierarchicalNamespaces: true
  grid:
    lineHops: false
    placements:
      Shop: { row: 1, column: 2 }
      Shop.Core: { row: 1, column: 1 }
      Order: { row: 1, column: 1 }
      LineItem: { row: 1, column: 2 }
      Customer: { row: 1, column: 1 }
      Payment: { row: 2, column: 2 }
---
classDiagram
  namespace Shop.Core {
    class Order {
      +int id
      +place()
    }
    class LineItem {
      +int qty
    }
  }
  class Customer
  class Payment
  Customer \"1\" --> \"*\" Order : places
  Order *-- \"1..*\" LineItem
  Wallet ()-- Payment
  note for Customer \"Registered users only\"
";

    const NAMESPACES: &str = r#"---
config:
  layout: grid
  look: classic
  htmlLabels: false
  grid:
    placements:
      Shop.Core: { row: 1, column: 4 }
      Shop.Audit: { row: 1, column: 1 }
      Core: { row: 1, column: -4 }
---
classDiagram
  namespace Shop.Core {
    class Order
  }
  namespace Shop.Audit {
    class Review
  }
"#;

    const CROSSINGS: &str = r#"---
config:
  layout: grid
  look: classic
  htmlLabels: false
  grid:
    curve: linear
    lineHops: false
    placements:
      TL: { row: 1, column: 1 }
      TR: { row: 1, column: 2 }
      BL: { row: 2, column: 1 }
      BR: { row: 2, column: 2 }
---
classDiagram
  TL --> BR
  TL --> BL
  BL --> TR
"#;

    fn prepare(parsed: &PresentationDiagram) -> FamilyRenderArtifact<'_> {
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        family::prepare(parsed, &LayoutOptions::default(), session).unwrap()
    }

    fn layout(artifact: &FamilyRenderArtifact<'_>) -> ClassDiagramLayout {
        let geometry = artifact.layout_geometry_json().unwrap();
        assert_eq!(geometry["family_id"], "class");
        assert_eq!(geometry["diagram_type"], "classDiagram");
        assert_eq!(
            geometry["layout"],
            artifact.layout_json().unwrap()["layout"]
        );
        serde_json::from_value(geometry["layout"]["ClassDiagramV2"].clone()).unwrap()
    }

    fn node<'a>(layout: &'a ClassDiagramLayout, id: &str) -> &'a LayoutNode {
        layout.nodes.iter().find(|node| node.id == id).unwrap()
    }

    fn svg(artifact: FamilyRenderArtifact<'_>) -> String {
        let rendered = artifact
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some("grid-class".into()),
                    ..Default::default()
                },
                &Default::default(),
            )
            .unwrap();
        assert_eq!(rendered.family_id(), DiagramFamilyId::CLASS);
        rendered.svg().to_owned()
    }

    fn elements<'a, 'input>(
        doc: &'a roxmltree::Document<'input>,
        class: &'a str,
    ) -> impl Iterator<Item = roxmltree::Node<'a, 'input>> {
        doc.descendants()
            .filter(move |node| node.attribute("class") == Some(class))
    }

    fn text(node: roxmltree::Node<'_, '_>) -> String {
        node.descendants()
            .filter(roxmltree::Node::is_text)
            .filter_map(|node| node.text())
            .collect()
    }

    fn edge_ids<'a>(doc: &'a roxmltree::Document<'_>) -> Vec<&'a str> {
        doc.descendants()
            .filter(|node| node.attribute("data-edge") == Some("true"))
            .map(|node| node.attribute("data-id").unwrap())
            .collect()
    }

    fn close(left: f64, right: f64) {
        assert!((left - right).abs() < 1e-6, "{left} vs {right}");
    }

    #[test]
    fn pinned_class_fixture_preserves_placements_relations_and_capability_admission() {
        let parsed = parse(AUTHORED_PLACEMENTS);
        let environment = RenderEnvironment::deterministic();
        let session = environment.begin_session().unwrap();
        let plan = family::plan_render(&parsed, &session).unwrap();
        assert_eq!(plan.required_capabilities(), [RenderCapability::LayoutGrid]);
        assert!(plan.is_ready());

        let artifact = prepare(&parsed);
        let layout = layout(&artifact);
        let [customer, order, payment, shipment] =
            ["Customer", "Order", "Payment", "Shipment"].map(|id| node(&layout, id));
        assert!(customer.x < order.x && order.x < shipment.x);
        close(customer.y, order.y);
        close(order.y, shipment.y);
        close(order.x, payment.x);
        assert!(order.y < payment.y);
        assert!(
            layout
                .edges
                .iter()
                .all(|edge| edge.label.is_some() && edge.points.len() >= 2)
        );

        let rendered = svg(artifact);
        let doc = roxmltree::Document::parse(&rendered).unwrap();
        let root = doc.root_element();
        assert_eq!(root.attribute("aria-roledescription"), Some("classDiagram"));
        assert_eq!(root.attribute("data-layout-fallback"), None);
        assert_eq!(
            edge_ids(&doc),
            [
                "id_Customer_Order_1",
                "id_Customer_Payment_2",
                "id_Order_Payment_3",
                "id_Order_Shipment_4"
            ],
        );
        assert!(
            elements(&doc, "edgeLabels")
                .all(|group| !group.children().any(|node| node.is_element()))
        );
        assert_eq!(
            elements(&doc, "label edgeLabel")
                .map(text)
                .collect::<Vec<_>>(),
            ["places", "makes", "pays with", "creates"]
        );

        let denied = environment
            .with_capability_policy(
                RenderCapabilityPolicy::unrestricted().with_denied(RenderCapability::LayoutGrid),
            )
            .begin_session()
            .unwrap();
        assert!(matches!(
            family::prepare(&parsed, &LayoutOptions::default(), denied),
            Err(Error::MissingCapability { capability: RenderCapability::LayoutGrid, diagram_type })
                if diagram_type == "classDiagram"
        ));
    }

    #[test]
    fn grid_reuses_class_box_measurement_and_separates_center_and_terminal_labels() {
        let parsed = parse(BOXES);
        let artifact = prepare(&parsed);
        let measured = layout(&artifact);
        let order = node(&measured, "Order");
        let item = node(&measured, "Item");
        assert!(order.x > item.x && order.width > item.width && order.height > item.height);
        close(order.y, item.y);
        let [relation] = measured.edges.as_slice() else {
            panic!("one relation")
        };
        let start = relation.points.first().unwrap();
        let end = relation.points.last().unwrap();
        close(start.x, order.x - order.width / 2.0);
        close(end.x, item.x + item.width / 2.0);
        let source_cardinality = relation.start_label_right.as_ref().unwrap();
        let target_cardinality = relation.end_label_left.as_ref().unwrap();
        assert!((source_cardinality.x - start.x + 17.5).abs() < 1e-5);
        close(source_cardinality.y, start.y - 15.0);
        assert!((target_cardinality.x - end.x - 17.5).abs() < 1e-5);
        close(target_cardinality.y, end.y + 15.0);

        let unmatched = BOXES
            .replace("      Order: { row: 3, column: 2 }\n", "")
            .replace("      Item: { row: 3, column: -1 }\n", "");
        let automatic = layout(&prepare(&parse(&unmatched)));
        assert!(node(&automatic, "Order").x < node(&automatic, "Item").x);
        for (backend, available) in [
            ("dagre", merman_render::layout_dagre_available()),
            ("elk", merman_render::layout_elk_available()),
        ] {
            if available {
                let source = BOXES.replace("layout: grid", &format!("layout: {backend}"));
                let layered = layout(&prepare(&parse(&source)));
                for expected in [order, item] {
                    let measured = node(&layered, &expected.id);
                    assert_eq!(
                        (expected.width, expected.height),
                        (measured.width, measured.height)
                    );
                }
            }
        }

        let html = BOXES.replace(
            "htmlLabels: false\n  flowchart:\n    htmlLabels: true",
            "htmlLabels: true\n  flowchart:\n    htmlLabels: false",
        );
        for (rendered, uses_html) in [(svg(artifact), false), (svg(prepare(&parse(&html))), true)] {
            let doc = roxmltree::Document::parse(&rendered).unwrap();
            let helper = elements(&doc, "label edgeLabel").next().unwrap();
            assert_eq!(
                helper.attribute("id"),
                Some("edge-label-Order-Item-id_Order_Item_1")
            );
            assert_eq!(
                helper.parent_element().unwrap().attribute("class"),
                Some("nodes")
            );
            assert_eq!(text(helper), "**places items**");
            assert_eq!(
                helper
                    .descendants()
                    .any(|node| node.has_tag_name("foreignObject")),
                uses_html
            );
            let terminals = elements(&doc, "edgeTerminals").collect::<Vec<_>>();
            assert_eq!(
                terminals.iter().copied().map(text).collect::<Vec<_>>(),
                ["1", "*"]
            );
            assert!(terminals.iter().all(
                |node| node.parent_element().unwrap().attribute("class") == Some("edgeLabels")
            ));
            let empty = elements(&doc, "edgeLabel").next().unwrap();
            assert_eq!(text(empty), "");
            assert_eq!(
                empty.parent_element().unwrap().attribute("class"),
                Some("edgeLabels")
            );
            let path = doc
                .descendants()
                .find(|node| node.attribute("data-edge") == Some("true"))
                .unwrap();
            assert_eq!(
                path.attribute("marker-end"),
                Some("url(#grid-class_classDiagram-dependencyEnd)")
            );
            assert!(!path.attribute("d").unwrap().contains(['C', 'Q']));
            assert!(rendered.contains("#ffe2aa") && rendered.contains("#124578"));
        }
    }

    #[test]
    fn grid_uses_class_namespace_and_attachment_order() {
        let parsed = parse(COMPOUND);
        let artifact = prepare(&parsed);
        let measured = layout(&artifact);
        assert_eq!(
            measured
                .clusters
                .iter()
                .map(|cluster| cluster.id.as_str())
                .collect::<Vec<_>>(),
            ["Shop", "Shop.Core"]
        );
        let parent = node(&measured, "Shop");
        let child = node(&measured, "Shop.Core");
        let order = node(&measured, "Order");
        let item = node(&measured, "LineItem");
        assert!((child.x - parent.x).abs() + child.width / 2.0 < parent.width / 2.0);
        assert!((order.x - child.x).abs() + order.width / 2.0 < child.width / 2.0);
        assert!(order.x < item.x);
        close(order.y, item.y);
        assert!((node(&measured, "interface0").x - parent.x).abs() > parent.width / 2.0);

        let rendered = svg(artifact);
        let doc = roxmltree::Document::parse(&rendered).unwrap();
        assert_eq!(
            edge_ids(&doc),
            [
                "edgeNote0",
                "id_Customer_Order_1",
                "id_Order_LineItem_2",
                "id_interface0_Payment_3"
            ]
        );
        assert!(doc.descendants().any(|node| node.attribute("data-id")
            == Some("id_interface0_Payment_3")
            && node.attribute("marker-start")
                == Some("url(#grid-class_classDiagram-lollipopStart)")));
        assert!(
            doc.descendants()
                .any(|node| node.attribute("d").is_some_and(|d| d.contains('Q'))
                    && node.attribute("data-edge") == Some("true"))
        );

        let flat = COMPOUND.replace(
            "hierarchicalNamespaces: true",
            "hierarchicalNamespaces: false",
        );
        let flat = layout(&prepare(&parse(&flat)));
        assert_eq!(flat.clusters.len(), 1);
        assert_eq!(flat.clusters[0].id, "Shop.Core");
        assert_eq!(flat.clusters[0].title, "Shop.Core");
    }

    #[test]
    fn grid_placement_keys_use_the_full_namespace_id() {
        let placed = layout(&prepare(&parse(NAMESPACES)));
        assert!(node(&placed, "Shop.Core").x > node(&placed, "Shop.Audit").x);

        let unplaced = NAMESPACES
            .replace("      Shop.Core: { row: 1, column: 4 }\n", "")
            .replace("      Shop.Audit: { row: 1, column: 1 }\n", "");
        let automatic = layout(&prepare(&parse(&unplaced)));
        assert!(node(&automatic, "Shop.Core").x < node(&automatic, "Shop.Audit").x);
    }

    #[test]
    fn grid_gap_line_hops_preserve_layout_and_markers() {
        let before = parse(CROSSINGS);
        let after = parse(&CROSSINGS.replace("lineHops: false", "lineHops: gap"));
        let before = prepare(&before);
        let after = prepare(&after);
        assert_eq!(
            before.layout_geometry_json().unwrap(),
            after.layout_geometry_json().unwrap()
        );
        let paths = |svg: &str| {
            roxmltree::Document::parse(svg)
                .unwrap()
                .descendants()
                .filter(|node| node.attribute("data-edge") == Some("true"))
                .map(|node| {
                    ["data-points", "marker-end", "d"]
                        .map(|key| node.attribute(key).unwrap().to_owned())
                })
                .collect::<Vec<_>>()
        };
        let before = paths(&svg(before));
        let after = paths(&svg(after));
        assert_eq!(before.len(), after.len());
        assert!(
            before
                .iter()
                .zip(&after)
                .all(|(before, after)| before[..2] == after[..2])
        );
        assert!(after.iter().any(|edge| edge[2].matches('M').count() > 1));
    }
}
