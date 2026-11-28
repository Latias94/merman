use std::sync::Arc;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use merman_core::{MermaidEngine, ParseOptions, presentation::SourcePresentationConfig};
use merman_model::{
    OperationPhase,
    family::DiagramFamilyId,
    presentation::{PresentationDiagram, RenderSemanticModel},
};
use merman_render_core::{
    resources::{RenderResourcePolicy, ResourceLimitId},
    root_svg::RootViewportContext,
    svg_css::SvgId,
    svg_output::timing::RenderTiming,
    text::{
        DeterministicTextMeasurer, TextMetrics, create_text_html::FlowchartLabelMetricsRequest,
        html_label_layout::measure_flowchart_label_for_layout,
    },
};
use roughr::core::{RoughJsSeed, RoughMathRandom, RoughRandomness};
use serde_json::json;

use super::*;
use crate::class::{
    ClassNodeVisualPlan, ClassRelationThemePlan, ClassSvgContext, ClassTextThemePlan,
    class_layout_work_units, render_class_diagram_svg_model_with_config,
};

const COMPOUND: &str = "\
classDiagram
direction LR
namespace Shop.Core {
  class Order
  class LineItem
}
namespace Audit {
  class Ledger
}
class Customer
Customer \"1\" --> \"*\" Order : **places**
Customer ..> Order : retries
Order *-- LineItem
Order --> Order : loop
Ledger <|-- Customer
Wallet ()-- Customer
note for Order \"registered users\"
style Order fill:#ffe2aa,stroke:#124578
";

const CROSSING: &str = r#"classDiagram
class A["Wide Class A"]
class B["Wide Class B"]
class C["Wide Class C"]
class D["Wide Class D"]
class E["Wide Class E"]
A -- C
A -- D
A -- E
B -- C
B -- D
C -- D
C -- E
"#;

fn parse(source: &str, site: Value) -> PresentationDiagram {
    MermaidEngine::new()
        .with_site_config(SourcePresentationConfig::from_value(site))
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation()
}

fn model(parsed: &PresentationDiagram) -> &ClassDiagramModel {
    let RenderSemanticModel::Class(model) = parsed.model() else {
        panic!("expected a Class diagram")
    };
    model
}

pub(super) fn measure_interface(request: FlowchartLabelMetricsRequest<'_>) -> TextMetrics {
    measure_flowchart_label_for_layout(request, Default::default(), false)
}

fn prepare(
    parsed: &PresentationDiagram,
    work: &Arc<OperationWorkMeter>,
) -> Result<Option<(ClassDiagramLayout, ClassPreparedLabels)>> {
    let model = model(parsed);
    let config = &parsed.metadata().effective_config;
    let typography = ClassTextThemePlan::resolve(None, config);
    let settings = ClassRenderConfig::resolve(config, &typography);
    let mut control = OperationLayoutWorkControl::new(work.clone());
    let preparation = class_layout_work_units(model, &control)?;
    control.charge_adapter(preparation)?;
    layout(
        model,
        config,
        &DeterministicTextMeasurer::default(),
        None,
        &settings,
        measure_interface,
        &control,
        work,
    )
}

fn render(
    parsed: &PresentationDiagram,
    layout: &ClassDiagramLayout,
    labels: &ClassPreparedLabels,
    work: &OperationWorkMeter,
) -> Result<String> {
    let model = model(parsed);
    let config = &parsed.metadata().effective_config;
    let typography = ClassTextThemePlan::resolve(None, config);
    let settings = ClassRenderConfig::resolve(config, &typography);
    let relations = ClassRelationThemePlan::resolve(None, config, None);
    let visual = ClassNodeVisualPlan::resolve(model, work)?;
    let diagram_id = SvgId::new("class-layered").unwrap();
    render_class_diagram_svg_model_with_config(
        layout,
        labels,
        model,
        &relations,
        &typography,
        &visual,
        &settings,
        config,
        parsed.metadata().title.as_deref(),
        &DeterministicTextMeasurer::default(),
        ClassSvgContext {
            diagram_id,
            viewport: RootViewportContext::new(DiagramFamilyId::CLASS, diagram_id),
            work_meter: work,
            math_renderer: None,
            timing: RenderTiming::disabled(),
            hand_drawn_seed: RoughRandomness::new(RoughJsSeed::new(1.0), RoughMathRandom::new(1)),
            elk_parity: false,
            #[cfg(feature = "layout-grid")]
            grid: None,
            #[cfg(feature = "layout-shoal")]
            canonical_routes: None,
        },
    )?
    .into_string_for(DiagramFamilyId::CLASS)
}

fn policy() -> RenderResourcePolicy {
    RenderResourcePolicy::unbounded_for_trusted_input()
}

fn meter() -> Arc<OperationWorkMeter> {
    Arc::new(OperationWorkMeter::new(policy()))
}

#[test]
fn browser_measured_fan_matches_layered_positions_and_geometry() {
    // Headless Chrome, Mermaid 12.4.0, classic SVG labels. The browser supplied these box sizes.
    let parsed = parse("classDiagram\nA --> B\nA --> C", json!({}));
    let work = meter();
    let mut measured = ClassMeasurement {
        groups: vec![],
        nodes: vec![],
        edges: vec![],
        work: &work,
    };
    for (id, width) in [("A", 35.125), ("B", 34.125), ("C", 34.125)] {
        measured.node(id, None, (width, 76.0)).unwrap();
    }
    measured
        .edge("A", "B", Cow::Borrowed("id_A_B_1"), (0.0, 0.0), None)
        .unwrap();
    measured
        .edge("A", "C", Cow::Borrowed("id_A_C_2"), (0.0, 0.0), None)
        .unwrap();
    let graph = measured
        .graph(
            model(&parsed),
            &parsed.metadata().effective_config,
            EdgeRouting::Rounded,
        )
        .unwrap()
        .unwrap();
    let solved = mermaid_layered::layout(&graph, Options::default(), &work)
        .unwrap()
        .graph;
    let centers = solved
        .nodes
        .into_iter()
        .map(|node| {
            let p = node.center();
            (p.x, p.y)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        centers,
        [(59.125, 38.0), (17.0625, 164.0), (101.1875, 164.0)]
    );
    for (edge, points) in solved.edges.iter().zip([
        [
            (53.5625, 76.0),
            (53.5625, 101.0),
            (17.0625, 101.0),
            (17.0625, 126.0),
        ],
        [
            (64.6875, 76.0),
            (64.6875, 101.0),
            (101.1875, 101.0),
            (101.1875, 126.0),
        ],
    ]) {
        assert_eq!(
            edge.points
                .iter()
                .map(|p| (p.x, p.y, p.axis))
                .collect::<Vec<_>>(),
            points.map(|(x, y)| (x, y, Axis::Vertical))
        );
    }
    assert_eq!(
        merman_render_core::svg_edge_parity::layered_route::path(
            &solved.edges[0].points,
            merman_render_core::svg_edge_parity::layered_route::Curve::Rounded,
            0.0,
            6.0,
            &work,
        )
        .unwrap(),
        "M53.56,76L53.56,96A5,5 0 0,1 48.56,101L22.06,101A5,5 0 0,0 17.06,106L17.06,120"
    );
}

#[test]
fn compound_dom_keeps_source_slots_styles_and_cardinalities() {
    for (routing, html) in [
        ("rounded", false),
        ("curved", false),
        ("stepped", false),
        ("straight", false),
        ("rounded", true),
    ] {
        let parsed = parse(
            COMPOUND,
            json!({
                "look": "classic", "htmlLabels": html, "state": {"nodeSpacing": 37, "rankSpacing": 61},
                "flowchart": {"subGraphTitleMargin": {"top": 7, "bottom": 3}},
                "layered": {"edgeRouting": routing, "lineHops": false},
            }),
        );
        let work = meter();
        let (layout, labels) = prepare(&parsed, &work).unwrap().unwrap();
        assert_eq!(
            layout
                .clusters
                .iter()
                .map(|cluster| cluster.id.as_str())
                .collect::<Vec<_>>(),
            ["Shop", "Shop.Core", "Audit"]
        );
        assert!(
            layout.edges[1].start_label_right.is_some() && layout.edges[1].end_label_left.is_some()
        );
        assert!(layout.edges[4].points.len() >= 4);
        let svg = render(&parsed, &layout, &labels, &work).unwrap();
        let doc = roxmltree::Document::parse(&svg).unwrap();
        assert_eq!(doc.root_element().attribute("data-layout-fallback"), None);
        let group = |class| {
            doc.descendants()
                .find(|node| node.attribute("class") == Some(class))
                .unwrap()
        };
        assert_eq!(
            group("root")
                .children()
                .filter_map(|node| node.attribute("class"))
                .collect::<Vec<_>>(),
            ["clusters", "edges edgePaths", "edgeLabels", "nodes"]
        );
        assert_eq!(
            group("clusters")
                .children()
                .filter_map(|node| node.attribute("id"))
                .collect::<Vec<_>>(),
            [
                "class-layered-Shop",
                "class-layered-Audit",
                "class-layered-Shop.Core"
            ]
        );
        let edges = group("edges edgePaths")
            .children()
            .filter(|node| node.attribute("data-edge") == Some("true"))
            .collect::<Vec<_>>();
        assert_eq!(
            edges
                .iter()
                .map(|edge| edge.attribute("data-id").unwrap())
                .collect::<Vec<_>>(),
            [
                "edgeNote0",
                "id_Customer_Order_1",
                "id_Customer_Order_2",
                "id_Order_LineItem_3",
                "id_Order_Order_4",
                "id_Ledger_Customer_5",
                "id_interface0_Customer_6"
            ]
        );
        for (edge, source) in edges.iter().zip(&layout.edges) {
            let points = axis_points(edge.attribute("data-points").unwrap());
            assert_eq!(points.len(), source.points.len());
            for (p, source) in points.iter().zip(&source.points) {
                assert!(matches!(p["axis"].as_str(), Some("h" | "v")));
                assert_eq!(
                    (p["x"].as_f64(), p["y"].as_f64()),
                    (Some(source.x), Some(source.y))
                );
            }
        }
        assert!(
            edges[6]
                .attribute("marker-start")
                .is_some_and(|marker| marker.contains("lollipop"))
        );
        let edge_labels = group("edgeLabels")
            .children()
            .filter(|node| node.attribute("class").is_some())
            .collect::<Vec<_>>();
        let kinds = edge_labels
            .iter()
            .map(|node| node.attribute("class").unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            kinds,
            [
                "edgeLabel",
                "edgeTerminals",
                "edgeTerminals",
                "edgeLabel",
                "edgeLabel"
            ]
        );
        assert!(
            edge_labels[0]
                .descendants()
                .any(|node| node.text() == Some("places"))
        );
        let center = layout.edges[1].label.as_ref().unwrap();
        let transform = edge_labels[0].attribute("transform").unwrap();
        let (x, y) = transform
            .strip_prefix("translate(")
            .unwrap()
            .strip_suffix(')')
            .unwrap()
            .split_once(", ")
            .unwrap();
        assert!((x.parse::<f64>().unwrap() - center.x).abs() < 1e-6);
        assert!((y.parse::<f64>().unwrap() - center.y - 5.0).abs() < 1e-6);
        assert_eq!(
            edge_labels[0]
                .descendants()
                .any(|node| node.has_tag_name("foreignObject")),
            html
        );
        assert!(svg.contains("#ffe2aa") || svg.contains("#FFE2AA"));
        let shaft = edges[4].attribute("d").unwrap();
        assert!(
            match routing {
                "rounded" => shaft.contains('A'),
                "stepped" => shaft.contains('Q'),
                "straight" => !shaft.contains(['A', 'C', 'Q']),
                _ => shaft.contains('C'),
            },
            "{routing}: {shaft}"
        );
    }
}

fn axis_points(base64: &str) -> Vec<Value> {
    serde_json::from_slice(&STANDARD.decode(base64).unwrap()).unwrap()
}

#[test]
fn line_hops_change_strokes_without_changing_route_data() {
    let paths = |line_hops: Value| {
        let parsed = parse(
            CROSSING,
            json!({ "look": "classic", "htmlLabels": false,
            "state": {"nodeSpacing":140,"rankSpacing":80}, "layered": {"lineHops":line_hops} }),
        );
        let work = meter();
        let (layout, labels) = prepare(&parsed, &work).unwrap().unwrap();
        let svg = render(&parsed, &layout, &labels, &work).unwrap();
        roxmltree::Document::parse(&svg)
            .unwrap()
            .descendants()
            .filter(|node| node.attribute("data-edge") == Some("true"))
            .map(|node| {
                ["data-id", "data-points", "d"].map(|key| node.attribute(key).unwrap().to_owned())
            })
            .collect::<Vec<_>>()
    };
    let disabled = paths(json!(false));
    let gap = paths(json!("gap"));
    assert_eq!(disabled.len(), gap.len());
    assert!(disabled.iter().zip(&gap).all(|(a, b)| a[..2] == b[..2]));
    assert!(
        disabled
            .iter()
            .all(|edge| edge[2].matches('M').count() == 1)
    );
    assert!(gap.iter().any(|edge| edge[2].matches('M').count() > 1));
}

#[test]
fn exact_limits_and_sticky_cancellation_never_request_dagre() {
    let parsed = parse(COMPOUND, json!({"look":"classic", "htmlLabels":false}));
    let work = meter();
    let (layout, labels) = prepare(&parsed, &work).unwrap().unwrap();
    let layout_work = work.used();
    let svg = render(&parsed, &layout, &labels, &work).unwrap();
    let cumulative_work = work.used();
    let cap = |id, max| {
        Arc::new(OperationWorkMeter::new(
            policy().with_limit(id, max).unwrap(),
        ))
    };
    let run = |work: Arc<_>| {
        let (layout, labels) = prepare(&parsed, &work)?.expect("Layered layout");
        render(&parsed, &layout, &labels, &work)
    };
    let units = ResourceLimitId::MaxLayoutWorkUnits;
    assert!(
        prepare(&parsed, &cap(units, layout_work))
            .unwrap()
            .is_some()
    );
    assert!(matches!(
        prepare(&parsed, &cap(units, layout_work - 1)),
        Err(Error::ResourceLimitExceeded(_))
    ));
    assert_eq!(run(cap(units, cumulative_work)).unwrap(), svg);
    assert!(matches!(
        run(cap(units, cumulative_work - 1)),
        Err(Error::ResourceLimitExceeded(_))
    ));
    assert_eq!(
        run(cap(ResourceLimitId::MaxSvgBytes, svg.len())).unwrap(),
        svg
    );
    assert!(matches!(
        run(cap(ResourceLimitId::MaxSvgBytes, svg.len() - 1)),
        Err(Error::ResourceLimitExceeded(_))
    ));
    let cancelled = meter();
    cancelled.control().cancel_after_checkpoints(40);
    for _ in 0..2 {
        assert!(
            matches!(prepare(&parsed, &cancelled), Err(Error::Cancelled(error)) if error.phase == OperationPhase::Layout)
        );
    }
    println!(
        "Class Layered exact caps: layout {layout_work}, cumulative {cumulative_work}, SVG {} bytes",
        svg.len()
    );
}

#[test]
fn oversized_title_band_requests_a_retry_and_drops_its_labels() {
    let parsed = parse(
        COMPOUND,
        json!({"look":"classic", "htmlLabels":false,
        "flowchart":{"subGraphTitleMargin":{"top":1.7e308,"bottom":1.7e308}} }),
    );
    let work = meter();
    assert!(prepare(&parsed, &work).unwrap().is_none());
    assert!(work.used() > 0);
    assert_eq!(work.prepared_text_retained_bytes(), 0);
}
