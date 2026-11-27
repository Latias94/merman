#![cfg(all(feature = "diagram-swimlane", feature = "layout-shoal"))]

use kurbo::{Point, Rect};
use merman_core::{MermaidEngine, ParseOptions};
use merman_model::OperationControl;
use merman_model::presentation::{PresentationDiagram, RenderSemanticModel};
use merman_model::resources::InputResourcePolicy;
use merman_render::environment::RenderEnvironment;
use merman_render::model::SwimlaneLayout;
use merman_render::theme::{NativeTheme, ThemeCompiler};
use merman_render::{LayoutOptions, family};
use roxmltree::Document;

fn render_pool(direction: &str, title: &str, nested: bool) -> (String, SwimlaneLayout) {
    let (inner_start, inner_end, inner_port) = if nested {
        (
            "subgraph inner[Team]\n",
            "end\n",
            "X innerPort@--> inner\nX longCross@------> A",
        )
    } else {
        ("", "", "")
    };
    let source = format!(
        r#"---
config:
  layout: shoal
  htmlLabels: false
  flowchart:
    htmlLabels: false
    nodeSpacing: 8
---
swimlane-beta {direction}
subgraph pool["{title}"]
  {inner_start}
  subgraph sales[Sales]
    A
  end
  {inner_end}
  subgraph ops[Ops]
    B
  end
end
subgraph external[External]
  X
end
X poolPort@--> pool
{inner_port}
"#,
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
            "pool" | "inner" => Some(serde_json::json!({"laneRole": "pool"})),
            "external" => Some(serde_json::json!({"laneIndex": 0})),
            "sales" => Some(serde_json::json!({"laneIndex": 1})),
            "ops" => Some(serde_json::json!({"laneIndex": 2})),
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
    let theme = ThemeCompiler::new()
        .compile_json(br#"{"schema_version":2,"extends":"studio"}"#, None)
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_native_theme(NativeTheme::new(theme))
        .unwrap();
    let artifact = family::prepare(
        &presentation,
        &LayoutOptions::headless_svg_defaults(),
        session,
    )
    .unwrap();
    let layout = serde_json::from_value(
        artifact.layout_json().unwrap()["layout"]["SwimlaneDiagram"].clone(),
    )
    .unwrap();
    let svg = artifact
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
    (svg.svg().to_owned(), layout)
}

fn lane_rect(layout: &SwimlaneLayout, id: &str) -> Rect {
    let lane = layout.lanes.iter().find(|lane| lane.id == id).unwrap();
    Rect::from_center_size((lane.x, lane.y), (lane.width, lane.height))
}

#[test]
fn shoal_empty_pool_fits_a_long_header_across_its_child_lanes() {
    const TITLE: &str = "Customer Experience and Self-Service Refund Operations with Platform Infrastructure, Ledger Sync, and Compliance Escalations";
    for direction in ["RL", "LR", "TB", "BT"] {
        let (svg, long) = render_pool(direction, TITLE, false);
        let (_, short) = render_pool(direction, "Refunds", false);
        let horizontal = matches!(direction, "RL" | "LR");
        let across = |rect: Rect| {
            if horizontal {
                (rect.y0, rect.y1)
            } else {
                (rect.x0, rect.x1)
            }
        };
        let pool = lane_rect(&long, "pool");
        let children = lane_rect(&long, "sales").union(lane_rect(&long, "ops"));
        let (start, end) = across(pool);
        let (child_start, child_end) = across(children);
        assert!(
            (start - child_start).abs() < 1e-8,
            "{direction} empty pool column: {start} to {child_start}"
        );
        assert!(
            (end - child_end).abs() < 1e-8,
            "{direction} child-lane extent"
        );
        for id in ["sales", "ops"] {
            let (before, after) = across(lane_rect(&long, id));
            let (short_before, short_after) = across(lane_rect(&short, id));
            if horizontal {
                assert!(
                    after - before > short_after - short_before,
                    "{direction} {id} should share the deficit: {} vs {}",
                    after - before,
                    short_after - short_before,
                );
            }
            let lane = long.lanes.iter().find(|lane| lane.id == id).unwrap();
            assert_eq!(lane.parent_id.as_deref(), Some("pool"));
        }
        let header = long
            .lanes
            .iter()
            .find(|lane| lane.id == "pool")
            .unwrap()
            .title_rect
            .as_ref()
            .unwrap();
        let (title_start, title_end) = across(Rect::new(
            header.left,
            header.top,
            header.right,
            header.bottom,
        ));
        assert!(
            title_start > start && title_end < end,
            "{direction} pool header fits"
        );
        let edge = long
            .edges
            .iter()
            .find(|edge| edge.id == "poolPort")
            .unwrap();
        let contact = edge.points.last().unwrap();
        assert!(
            pool.inflate(1e-8, 1e-8)
                .contains(Point::new(contact.x, contact.y))
        );
        let cross = if horizontal { contact.y } else { contact.x };
        assert!(
            (cross - start).abs() < 1e-8,
            "{direction} pool port stays on the outer face"
        );

        let document = Document::parse(&svg).unwrap();
        let shell = document
            .descendants()
            .find(|element| {
                element.attribute("data-id") == Some("pool")
                    && element.attribute("data-et") == Some("cluster")
            })
            .unwrap();
        assert!(
            shell
                .attribute("class")
                .unwrap()
                .split_ascii_whitespace()
                .any(|class| class == "pool")
        );
        assert!(shell.attribute("id").is_some());
    }
}

#[test]
fn shoal_empty_nested_pool_ports_share_the_child_lanes_outer_face() {
    let (_, layout) = render_pool("RL", "Refunds", true);
    let pool = lane_rect(&layout, "pool");
    let inner = lane_rect(&layout, "inner");
    let sales = lane_rect(&layout, "sales");
    let external = lane_rect(&layout, "external");
    assert!((external.y1 - sales.y0).abs() < 1e-8);
    assert!(
        (pool.y0 - sales.y0).abs() < 1e-8,
        "outer pool {pool:?} spent a column before its child {sales:?}; inner: {inner:?}, ops: {:?}",
        lane_rect(&layout, "ops"),
    );
    assert!(
        (inner.y0 - sales.y0).abs() < 1e-8,
        "inner pool spent a column before its child"
    );
    for (edge_id, rect) in [("poolPort", pool), ("innerPort", inner)] {
        let edge = layout.edges.iter().find(|edge| edge.id == edge_id).unwrap();
        let contact = edge.points.last().unwrap();
        assert!(
            rect.inflate(1e-8, 1e-8)
                .contains(Point::new(contact.x, contact.y))
        );
        assert!(
            (rect.y0 - contact.y).abs() < 1e-8,
            "{edge_id} should terminate on the pool face"
        );
    }
}
