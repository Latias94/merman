use merman_layout_shoal::{
    Direction, EdgeSpec, Endpoint, Graph, GroupId, GroupSpec, Insets, NodeId, NodeSpec, Options,
    Point, Rect, RoutePoint, Size, TitlePosition, layout,
};

const EPS: f64 = 1e-7;

fn size(width: f64, height: f64) -> Size {
    Size { width, height }
}

fn group(seq: u32, parent: Option<u32>, title: Size) -> GroupSpec {
    let mut spec = GroupSpec::new(seq);
    spec.parent = parent.map(GroupId);
    spec.padding = Insets {
        top: 11.,
        right: 13.,
        bottom: 17.,
        left: 19.,
    };
    spec.title = Some(title);
    spec
}

fn node(seq: u32, parent: Option<u32>) -> NodeSpec {
    let mut spec = NodeSpec::new(size(52., 26.), seq);
    spec.parent = parent.map(GroupId);
    spec
}

fn edge(source: impl Into<Endpoint>, target: impl Into<Endpoint>) -> EdgeSpec {
    EdgeSpec::new(source.into(), target.into())
}

fn label(center: Point, size: Size) -> Rect {
    Rect {
        x: center.x - size.width / 2.,
        y: center.y - size.height / 2.,
        width: size.width,
        height: size.height,
    }
}

fn contains(outer: Rect, inner: Rect) -> bool {
    outer.x <= inner.x + EPS
        && outer.y <= inner.y + EPS
        && inner.x + inner.width <= outer.x + outer.width + EPS
        && inner.y + inner.height <= outer.y + outer.height + EPS
}

fn overlaps(first: Rect, second: Rect) -> bool {
    first.x.max(second.x) + EPS < (first.x + first.width).min(second.x + second.width)
        && first.y.max(second.y) + EPS < (first.y + first.height).min(second.y + second.height)
}

fn on_border(point: &RoutePoint, rect: Rect) -> bool {
    let dx = point.x - rect.x;
    let dy = point.y - rect.y;
    (-EPS..=rect.width + EPS).contains(&dx)
        && (-EPS..=rect.height + EPS).contains(&dy)
        && dx
            .abs()
            .min((dx - rect.width).abs())
            .min(dy.abs())
            .min((dy - rect.height).abs())
            <= EPS
}

fn enters(from: &RoutePoint, to: &RoutePoint, rect: Rect) -> bool {
    let (mut enter, mut leave) = (0_f64, 1_f64);
    for (origin, end, low, extent) in [
        (from.x, to.x, rect.x, rect.width),
        (from.y, to.y, rect.y, rect.height),
    ] {
        let (low, high) = (low + EPS, low + extent - EPS);
        if end == origin {
            if origin <= low || origin >= high {
                return false;
            }
        } else {
            let first = (low - origin) / (end - origin);
            let last = (high - origin) / (end - origin);
            enter = enter.max(first.min(last));
            leave = leave.min(first.max(last));
        }
    }
    enter < leave
}

#[test]
fn nested_and_empty_group_titles_are_contained_and_clear_of_siblings() {
    use TitlePosition::{BottomCenter, BottomLeft, BottomRight, TopCenter, TopLeft, TopRight};

    let mut graph = Graph::default();
    graph.groups = vec![
        group(0, None, size(136., 20.)),
        group(1, Some(0), size(97., 19.)),
        group(2, Some(0), size(72., 17.)),
    ];
    graph.groups[0].min_size.width = 320.;
    graph.groups[2].min_size = size(110., 55.);
    graph.nodes.push(node(3, Some(1)));
    for position in [
        TopLeft,
        TopCenter,
        TopRight,
        BottomLeft,
        BottomCenter,
        BottomRight,
    ] {
        graph.groups[0].title_position = position;
        let drawing = layout(&graph, &Options::default()).unwrap();
        assert_eq!(drawing.groups.len(), 3);
        assert_eq!(drawing.nodes.len(), 1);
        assert!(drawing.edges.is_empty());
        let [outer, nested, empty] = drawing.groups.as_slice() else {
            unreachable!()
        };
        assert!(contains(outer.rect, nested.rect) && contains(outer.rect, empty.rect));
        assert!(contains(nested.rect, drawing.nodes[0]));
        assert!(!overlaps(nested.rect, empty.rect));
        assert!(empty.rect.width >= 110. && empty.rect.height >= 55.);
        for (index, geometry) in drawing.groups.iter().enumerate() {
            let title = label(
                geometry.title_center.unwrap(),
                graph.groups[index].title.unwrap(),
            );
            assert!(
                contains(geometry.rect, title),
                "{position:?} title {index}: {title:?}"
            );
            let contents: &[Rect] = match index {
                0 => &[nested.rect, empty.rect],
                1 => &drawing.nodes,
                _ => &[],
            };
            assert!(contents.iter().all(|&body| !overlaps(title, body)));
            if index == 0 {
                let padding = graph.groups[0].padding;
                let left = outer.rect.x + padding.left + title.width / 2.;
                let right = outer.rect.x + outer.rect.width - padding.right - title.width / 2.;
                assert!(right - left > EPS);
                assert_eq!(
                    title.center().y < outer.rect.center().y,
                    matches!(position, TopLeft | TopCenter | TopRight),
                    "{position:?}"
                );
                let expected = match position {
                    TopLeft | BottomLeft => left,
                    TopRight | BottomRight => right,
                    TopCenter | BottomCenter => outer.rect.center().x.clamp(left, right),
                };
                assert!(
                    (title.center().x - expected).abs() <= EPS,
                    "{position:?}: {title:?}, [{left}, {right}]"
                );
            }
        }
    }
    graph.groups.truncate(1);
    graph.groups[0].min_size = Size::default();
    graph.nodes.clear();
    for position in [TopLeft, TopCenter, TopRight] {
        graph.groups[0].title_position = position;
        let drawing = layout(&graph, &Options::default()).unwrap();
        let outer = &drawing.groups[0];
        let padding = graph.groups[0].padding;
        let width = graph.groups[0].title.unwrap().width;
        let left = outer.rect.x + padding.left + width / 2.;
        let right = outer.rect.x + outer.rect.width - padding.right - width / 2.;
        assert!((right - left).abs() <= EPS);
        assert!((outer.title_center.unwrap().x - left).abs() <= EPS);
    }
}

#[test]
fn mixed_direction_crossings_share_ordered_border_knots_and_clear_siblings() {
    let mut graph = Graph::default();
    graph.groups = vec![
        group(0, None, size(77., 16.)),
        group(1, Some(0), size(85., 18.)),
        group(2, None, size(93., 16.)),
    ];
    graph.nodes = vec![node(3, Some(1)), node(4, Some(1)), node(5, Some(2))];
    graph.edges = vec![
        edge(NodeId(0), NodeId(1)),
        edge(NodeId(0), NodeId(2)),
        edge(NodeId(0), NodeId(2)),
    ];
    let mut options = Options::default();
    options.group_policy.inherit_crossed_direction = false;
    for (root, child) in [
        (Direction::Down, Direction::Right),
        (Direction::Up, Direction::Left),
        (Direction::Right, Direction::Up),
        (Direction::Left, Direction::Down),
    ] {
        graph.direction = root;
        graph.groups[1].direction = Some(child);
        let drawing = layout(&graph, &options).unwrap();
        assert_eq!(drawing.groups[1].effective_direction, child);
        assert_eq!(drawing.edges.len(), 3);
        for (occurrence, route) in drawing.edges.iter().enumerate().skip(1) {
            let knots = &route.points;
            assert!(on_border(knots.first().unwrap(), drawing.nodes[0]));
            assert!(on_border(knots.last().unwrap(), drawing.nodes[2]));
            let mut interior = knots[1..knots.len() - 1].iter();
            for id in [1, 0, 2] {
                assert!(
                    interior.any(|point| on_border(point, drawing.groups[id].rect)),
                    "{root:?} edge #{occurrence}: missing ordered group {id} stitch in {knots:?}"
                );
            }
            let declared: Vec<_> = drawing
                .border_stitches
                .iter()
                .filter(|stitch| stitch.edge.0 as usize == occurrence)
                .collect();
            assert_eq!(
                declared
                    .iter()
                    .map(|stitch| stitch.group.0)
                    .collect::<Vec<_>>(),
                [1, 0, 2]
            );
            assert!(
                declared
                    .windows(2)
                    .all(|pair| pair[0].point < pair[1].point)
            );
            for stitch in declared {
                let point = knots[stitch.point];
                let group = drawing.groups[stitch.group.0 as usize].rect;
                assert!(on_border(&point, group));
                let gap = match stitch.side {
                    merman_layout_shoal::Side::Top => point.y - group.y,
                    merman_layout_shoal::Side::Bottom => point.y - (group.y + group.height),
                    merman_layout_shoal::Side::Left => point.x - group.x,
                    merman_layout_shoal::Side::Right => point.x - (group.x + group.width),
                };
                assert!(gap.abs() <= EPS);
            }
            let sibling = drawing.nodes[1];
            assert!(
                knots
                    .windows(2)
                    .all(|pair| !enters(&pair[0], &pair[1], sibling)),
                "{root:?} edge #{occurrence} crosses sibling {sibling:?}"
            );
        }
    }
}

#[test]
fn own_member_edges_keep_the_group_contact_and_label_inside_the_owner() {
    let mut graph = Graph::default();
    graph.groups.push(group(0, None, size(101., 19.)));
    graph.nodes = vec![node(1, Some(0)), node(2, Some(0))];
    graph.edges = vec![edge(GroupId(0), NodeId(0)), edge(NodeId(0), GroupId(0))];
    graph.edges[0].label = Some(size(83., 17.));
    let drawing = layout(&graph, &Options::default()).unwrap();
    let owner = &drawing.groups[0];
    let title = label(owner.title_center.unwrap(), graph.groups[0].title.unwrap());
    let caption = label(
        drawing.edges[0].label_center.unwrap(),
        graph.edges[0].label.unwrap(),
    );
    assert!(contains(owner.rect, caption));
    assert!(!overlaps(title, caption));
    assert!(drawing.nodes.iter().all(|&node| !overlaps(caption, node)));
    for (index, route) in drawing.edges.iter().enumerate() {
        let mut ends = [route.points.first().unwrap(), route.points.last().unwrap()];
        if index == 1 {
            ends.swap(0, 1);
        }
        assert!(on_border(ends[0], owner.rect) && on_border(ends[1], drawing.nodes[0]));
        for point in &route.points {
            assert!(contains(
                owner.rect,
                label(
                    Point {
                        x: point.x,
                        y: point.y
                    },
                    Size::default()
                )
            ));
        }
        for pair in route.points.windows(2) {
            assert!(
                !enters(&pair[0], &pair[1], title),
                "own-member edge #{index} crosses title"
            );
            assert!(!enters(&pair[0], &pair[1], drawing.nodes[1]));
        }
    }
}

#[test]
fn repaired_parentage_and_sparse_truncation_identify_physical_contacts() {
    let mut graph = Graph::default();
    graph.groups = vec![
        group(0, Some(1), size(76., 13.)),
        group(1, Some(0), size(84., 14.)),
        group(2, None, size(70., 14.)),
    ];
    graph.nodes = vec![node(3, Some(1)), node(4, Some(1)), node(5, Some(2))];
    graph.edges = vec![edge(NodeId(0), NodeId(1)), edge(NodeId(1), NodeId(2))];
    let mut options = Options::default();
    let complete = layout(&graph, &options).unwrap();
    assert_eq!(complete.groups[0].parent, None);
    assert_eq!(complete.groups[1].parent, Some(GroupId(0)));
    assert!(contains(complete.groups[0].rect, complete.groups[1].rect));
    assert!(complete.truncated_routes.is_empty());
    assert_eq!(complete.stats.truncated_border_ports, 0);
    for cap in [0, 1] {
        options.caps.max_border_ports = cap;
        let drawing = layout(&graph, &options).unwrap();
        assert!(drawing.stats.truncated_border_ports > 0);
        let [cut] = drawing.truncated_routes.as_slice() else {
            panic!(
                "only the cross-group occurrence can be truncated: {:?}",
                drawing.truncated_routes
            )
        };
        assert_eq!(cut.edge.0, 1);
        assert_ne!(
            (cut.source, cut.target),
            (graph.edges[1].source, graph.edges[1].target)
        );
        let physical = |endpoint| match endpoint {
            Endpoint::Group(id) => drawing.groups[id.0 as usize].rect,
            Endpoint::Node(id) => drawing.nodes[id.0 as usize],
        };
        let route = &drawing.edges[1].points;
        assert!(on_border(route.first().unwrap(), physical(cut.source)));
        assert!(on_border(route.last().unwrap(), physical(cut.target)));
        let intact = &drawing.edges[0].points;
        assert!(on_border(intact.first().unwrap(), drawing.nodes[0]));
        assert!(on_border(intact.last().unwrap(), drawing.nodes[1]));
    }
}
