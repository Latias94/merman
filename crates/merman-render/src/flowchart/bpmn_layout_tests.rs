use super::bpmn::BpmnShape;
use crate::model::LayoutPoint;

#[test]
fn bpmn_elk_caption_projection_matches_the_pinned_twenty_step_oracle() {
    use crate::elk_edge_geometry::{Outline, Shape, sanitize};
    use crate::model::LayoutNode;

    let center = LayoutPoint {
        x: 128.1328125,
        y: 75.4,
    };
    let bounds = LayoutNode {
        id: "event".to_owned(),
        x: center.x,
        y: center.y,
        width: 232.265625,
        height: 112.8,
        is_cluster: false,
        label_width: None,
        label_height: None,
    };
    let port = LayoutPoint {
        x: bounds.x + bounds.width / 2.0,
        y: bounds.y,
    };
    let next = LayoutPoint {
        x: port.x + 40.0,
        y: port.y,
    };
    let target = LayoutNode {
        id: "target".to_owned(),
        x: next.x + 20.0,
        width: 40.0,
        height: 36.0,
        ..bounds.clone()
    };
    let intersection = |point: &LayoutPoint| {
        BpmnShape::Start.intersect(&center, bounds.width, bounds.height, point, false)
    };
    let direct = intersection(&port);
    assert_eq!((direct.x, direct.y), (146.1328125, 75.4));
    let projected = sanitize(
        &[
            center.clone(),
            port,
            next,
            LayoutPoint {
                x: target.x,
                y: target.y,
            },
        ],
        Shape {
            node: &bounds,
            outline: Outline::Rect,
            intersection: Some(&intersection),
        },
        Shape {
            node: &target,
            outline: Outline::Rect,
            intersection: None,
        },
    );
    assert_eq!((projected[0].x, projected[0].y), (146.13270290941, 75.4));
}
