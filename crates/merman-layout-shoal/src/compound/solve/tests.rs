use super::*;
use crate::compound::{hierarchy, sides};
use crate::{
    Direction, EdgeId, GroupId, GroupSpec, Insets, MarkerMetrics, NoopWorkControl, Options,
    PackingPolicy, TitlePosition, WidthPolicy, layout,
};

fn size(width: f64, height: f64) -> Size {
    Size { width, height }
}

fn node(seq: u32, width: f64, height: f64, parent: Option<u32>) -> NodeSpec {
    let mut node = NodeSpec::new(size(width, height), seq);
    node.parent = parent.map(GroupId);
    node
}

fn edge(from: impl Into<Endpoint>, to: impl Into<Endpoint>) -> EdgeSpec {
    EdgeSpec::new(from.into(), to.into())
}

fn group(seq: u32, parent: Option<u32>, direction: Direction) -> GroupSpec {
    GroupSpec {
        parent: parent.map(GroupId),
        direction: Some(direction),
        padding: Insets {
            top: 32.5,
            right: 17.25,
            bottom: 29.75,
            left: 15.375,
        },
        seq,
        ..GroupSpec::default()
    }
}

fn independent() -> Options {
    let mut options = Options::default();
    options.group_policy.inherit_crossed_direction = false;
    options
}

fn tip(edge: &EdgeGeometry, end: usize) -> Point {
    point(if end == 0 {
        edge.points[0]
    } else {
        *edge.points.last().unwrap()
    })
}

fn overlap(first: Rect, second: Rect) -> bool {
    first.x < second.x + second.width
        && second.x < first.x + first.width
        && first.y < second.y + second.height
        && second.y < first.y + first.height
}

fn inside(p: Point, r: Rect) -> bool {
    p.x + 1.0e-9 >= r.x
        && p.y + 1.0e-9 >= r.y
        && p.x <= r.x + r.width + 1.0e-9
        && p.y <= r.y + r.height + 1.0e-9
}

fn enters(rect: Rect, a: Point, b: Point) -> bool {
    let mut near = 0_f64;
    let mut far = 1_f64;
    for (from, to, lo, hi) in [
        (a.x, b.x, rect.x, rect.x + rect.width),
        (a.y, b.y, rect.y, rect.y + rect.height),
    ] {
        if from == to {
            if from <= lo + 1.0e-9 || from >= hi - 1.0e-9 {
                return false;
            }
        } else {
            let u = (lo + 1.0e-9 - from) / (to - from);
            let v = (hi - 1.0e-9 - from) / (to - from);
            near = near.max(u.min(v));
            far = far.min(u.max(v));
        }
    }
    near < far
}

#[test]
fn measured_parallel_ports_enter_flat_as_boundary_ranks_and_perpendicular_ports_pin_only_the_member()
 {
    let mut graph = Graph::default();
    graph.groups.push(group(2, None, Direction::Right));
    graph.nodes = vec![node(0, 120., 48., Some(0)), node(1, 60., 30., None)];
    graph.nodes[0].port_span = Some(90.);
    graph.nodes[0].perpendicular_port_span = Some(28.);
    graph.nodes[0].order_hint = Some(OrderHint {
        rank: 6,
        cross_key: 7.,
    });
    graph.edges = vec![
        edge(NodeId(1), NodeId(0)),
        edge(NodeId(0), NodeId(1)),
        edge(NodeId(1), NodeId(0)),
    ];
    let options = independent();
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let hierarchy = hierarchy::build(&graph, &options, &mut work).unwrap();
    let sides: Vec<_> = hierarchy
        .borders
        .iter()
        .map(|border| [Side::Left, Side::Right, Side::Top][border.edge])
        .collect();
    let hints = vec![Some(17.); sides.len()];
    let pass = Pass {
        graph: &graph,
        hierarchy: &hierarchy,
        options: &options,
        sides: &sides,
        items: &[Some(9.), None, None],
        ports: &hints,
    };
    let (local, wires) = measure(0, &pass, &[], &mut work).unwrap();
    assert_eq!(local.direction, Direction::Right);
    assert_eq!(
        (local.nodes.len(), local.edges.len(), wires.len()),
        (3, 2, 3)
    );
    assert_eq!(local.nodes[0].pin, -1);
    assert_eq!(local.nodes[0].boundary, None);
    assert_eq!(
        local.nodes[0].effective_order_hint(),
        Some(OrderHint {
            rank: 6,
            cross_key: 9.
        })
    );
    assert_eq!(local.nodes[0].order_hint, graph.nodes[0].order_hint);
    assert_eq!(
        (
            local.nodes[0].port_span,
            local.nodes[0].perpendicular_port_span
        ),
        (Some(28.), Some(90.))
    );
    assert_eq!(local.nodes[1].boundary, Some(BoundaryRank::Start));
    assert_eq!(local.nodes[2].boundary, Some(BoundaryRank::End));
    assert!(
        local.nodes[1..]
            .iter()
            .all(|node| node.size == Size::default()
                && node.effective_order_hint().unwrap().cross_key == 17.)
    );
}

#[test]
fn parent_and_child_legs_use_the_same_resolved_contacts_in_all_directions() {
    let options = independent();
    for direction in [
        Direction::Down,
        Direction::Right,
        Direction::Up,
        Direction::Left,
    ] {
        let mut graph = Graph {
            direction,
            ..Graph::default()
        };
        graph.groups = vec![
            group(0, None, direction),
            group(1, Some(0), Direction::Left),
        ];
        graph.nodes = vec![node(2, 81.35, 32.25, Some(1)), node(3, 67.75, 22.125, None)];
        graph.edges = vec![edge(NodeId(0), NodeId(1)), edge(NodeId(0), NodeId(1))];
        graph.edges[0].label = Some(size(35.375, 14.75));
        graph.edges[0].start_label = Some(size(11., 6.5));
        graph.edges[0].end_label = Some(size(14., 7.));
        let mut control = NoopWorkControl;
        let mut work = Work::new(&mut control);
        let hierarchy = hierarchy::build(&graph, &options, &mut work).unwrap();
        let sides = sides::assign(&graph, &hierarchy, &options, &mut work).unwrap();
        let ports = vec![None; sides.len()];
        let pass = Pass {
            graph: &graph,
            hierarchy: &hierarchy,
            options: &options,
            sides: &sides,
            items: &[None; 4],
            ports: &ports,
        };
        let mut frames: Vec<Option<Frame>> = (0..3).map(|_| None).collect();
        let mut expanded = Expanded::default();
        for &id in hierarchy.order.iter().rev() {
            frames[id] = Some(level(id, &pass, &frames, &mut expanded, &mut work).unwrap());
        }
        for (id, local) in hierarchy.levels.iter().enumerate() {
            let frame = frames[id].as_ref().unwrap();
            for (leg, route) in local.legs.iter().zip(&frame.legs) {
                for (end, terminal) in [leg.source, leg.target].into_iter().enumerate() {
                    let expected = match terminal {
                        Terminal::Boundary(border) => frame.ports[hierarchy.borders[border].slot],
                        Terminal::Item(item, Some(border)) => {
                            let record = hierarchy.borders[border];
                            let port = frames[record.level].as_ref().unwrap().ports[record.slot];
                            let rect = frame.items[hierarchy.item_slot(item)];
                            translated(
                                port,
                                Point {
                                    x: rect.x,
                                    y: rect.y,
                                },
                            )
                        }
                        _ => continue,
                    };
                    assert_eq!(
                        tip(route, end),
                        expected,
                        "{direction:?}, level {id}, {terminal:?}"
                    );
                }
            }
        }
        let path = &hierarchy.paths[0];
        for (step, &(id, leg)) in path.iter().enumerate() {
            let edge = &frames[id].as_ref().unwrap().legs[leg];
            assert_eq!(edge.start_label_center.is_some(), step == 0);
            assert_eq!(edge.end_label_center.is_some(), step + 1 == path.len());
            assert_eq!(
                edge.label_center.is_some(),
                hierarchy.levels[id].legs[leg].main
            );
        }
    }
}

#[test]
fn perpendicular_ports_on_one_rank_route_inside_the_frame_around_occluding_siblings() {
    let mut graph = Graph::default();
    graph.groups.push(group(0, None, Direction::Right));
    graph.nodes = (0..6)
        .map(|seq| node(seq, 67., 32., (seq < 5).then_some(0)))
        .collect();
    for middle in 1..=3 {
        graph.edges.extend([
            edge(NodeId(0), NodeId(middle)),
            edge(NodeId(middle), NodeId(4)),
        ]);
    }
    for middle in 1..=3 {
        graph.edges.push(edge(NodeId(5), NodeId(middle)));
    }
    let drawing = layout(&graph, &independent()).unwrap();
    let owner = drawing.groups[0].rect;
    let mut detours = 0;
    for (index, route) in drawing.edges.iter().enumerate().skip(6) {
        let seam = route
            .points
            .iter()
            .position(|point| point.y == owner.y && inside(super::point(*point), owner))
            .unwrap();
        let interior = &route.points[seam..];
        for &knot in interior {
            assert!(inside(point(knot), owner));
        }
        for pair in interior.windows(2) {
            let (a, b) = (point(pair[0]), point(pair[1]));
            assert!(a.x == b.x || a.y == b.y);
            for (node, &rect) in drawing.nodes[..5].iter().enumerate() {
                assert!(
                    !enters(rect, a, b),
                    "edge {index} enters member {node}: {interior:?}"
                );
            }
        }
        detours += usize::from(interior.len() > 3);
    }
    assert!(detours > 0);
}

#[test]
fn own_member_captions_reserve_a_band_and_keep_the_member_face_free_for_other_wires() {
    let mut graph = Graph::default();
    graph.groups.push(group(0, None, Direction::Down));
    graph.groups[0].title = Some(size(60., 16.));
    graph.nodes = vec![
        node(0, 175., 35., Some(0)),
        node(1, 55., 30., Some(0)),
        node(2, 35., 25., None),
    ];
    graph.edges = vec![
        edge(NodeId(1), NodeId(0)),
        edge(NodeId(2), NodeId(0)),
        edge(GroupId(0), NodeId(0)),
        edge(GroupId(0), NodeId(0)),
    ];
    for (which, spec) in graph.edges[2..].iter_mut().enumerate() {
        spec.label = Some(size(60. + which as f64 * 15., 14.));
        spec.start_marker = MarkerMetrics {
            length: 11.,
            width: 6.,
        };
        spec.end_marker = MarkerMetrics {
            length: 8.,
            width: 5.,
        };
    }
    let drawing = layout(&graph, &independent()).unwrap();
    let owner = drawing.groups[0].rect;
    let title = centered(
        drawing.groups[0].title_center.unwrap(),
        graph.groups[0].title.unwrap(),
    );
    let mut captions = Vec::new();
    for (id, route) in drawing.edges[2..].iter().enumerate() {
        assert_eq!(route.ports, (Side::Top, Side::Top));
        let start = tip(route, 0);
        let next = point(route.points[1]);
        assert_eq!((start.y, start.x), (owner.y, next.x));
        assert!(next.y >= start.y + 11.);
        let target = tip(route, 1);
        for other in &drawing.edges[..2 + id] {
            assert!((target.x - tip(other, 1).x).abs() >= 8.);
        }
        let caption = centered(
            route.label_center.unwrap(),
            graph.edges[2 + id].label.unwrap(),
        );
        assert!(!overlap(caption, title));
        assert!(
            caption.y + caption.height
                <= drawing
                    .nodes
                    .iter()
                    .take(2)
                    .map(|node| node.y)
                    .fold(f64::INFINITY, f64::min)
        );
        assert!(captions.iter().all(|&other| !overlap(caption, other)));
        for other in &drawing.edges {
            for segment in other.points.windows(2) {
                assert!(!enters(caption, point(segment[0]), point(segment[1])));
            }
        }
        captions.push(caption);
    }
}

#[test]
fn a_caption_on_an_ancestor_boundary_does_not_replace_the_nested_member_route() {
    let mut incoming = edge(GroupId(0), NodeId(0));
    let mut outgoing = edge(NodeId(0), GroupId(0));
    let caption = Some(size(75., 18.));
    let end_label = Some(size(10., 10.));
    incoming.label = caption;
    incoming.end_label = end_label;
    outgoing.label = caption;
    outgoing.start_label = end_label;
    let graph = Graph {
        groups: vec![
            group(0, None, Direction::Down),
            group(1, Some(0), Direction::Down),
        ],
        nodes: vec![node(2, 80., 40., Some(1))],
        edges: vec![incoming, outgoing],
        ..Graph::default()
    };

    let options = independent();
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let hierarchy = hierarchy::build(&graph, &options, &mut work).unwrap();
    let sides = sides::assign(&graph, &hierarchy, &options, &mut work).unwrap();
    let pass = Pass {
        graph: &graph,
        hierarchy: &hierarchy,
        options: &options,
        sides: &sides,
        items: &[None; 3],
        ports: &[None; 4],
    };
    let (inner, wires) = measure(1, &pass, &[], &mut work).unwrap();
    assert_eq!(inner.edges.len(), 2);
    assert!(wires.iter().all(Wire::flat));
    assert!(inner.edges.iter().all(|edge| edge.label.is_none()));
    assert_eq!(inner.edges[0].end_label, end_label);
    assert_eq!(inner.edges[1].start_label, end_label);

    let drawing = layout(&graph, &options).unwrap();
    for (index, edge) in drawing.edges.iter().enumerate() {
        let caption = edge.label_center.unwrap();
        assert!(inside(caption, drawing.groups[0].rect));
        assert!(!inside(caption, drawing.groups[1].rect));
        assert!(
            drawing.border_stitches.iter().any(|stitch| {
                stitch.edge == EdgeId(index as u32) && stitch.group == GroupId(1)
            })
        );
    }
}

#[test]
fn explicit_titles_use_measured_padding_on_all_six_slots() {
    use TitlePosition::*;
    let mut graph = Graph::default();
    graph.groups.push(group(0, None, Direction::Down));
    graph.groups[0].title = Some(size(80., 18.));
    graph.groups[0].min_size = size(320., 180.);
    graph.nodes.push(node(0, 40., 24., Some(0)));
    for position in [
        TopLeft,
        TopCenter,
        TopRight,
        BottomLeft,
        BottomCenter,
        BottomRight,
    ] {
        graph.groups[0].title_position = position;
        let drawing = layout(&graph, &independent()).unwrap();
        let frame = &drawing.groups[0];
        assert_eq!((frame.rect.width, frame.rect.height), (320., 180.));
        let title = frame.title_center.unwrap();
        let expected_x = match position {
            TopLeft | BottomLeft => 15.375 + 40.,
            TopRight | BottomRight => 320. - 17.25 - 40.,
            _ => 160.,
        };
        let expected_y = match position {
            TopLeft | TopCenter | TopRight => 32.5 / 2.,
            _ => 180. - 29.75 / 2.,
        };
        assert_eq!(
            title,
            Point {
                x: expected_x,
                y: expected_y
            }
        );
    }
}

#[test]
fn right_aligned_titles_keep_an_isolated_port_on_its_trunk() {
    let mut spec = group(0, None, Direction::Down);
    spec.padding.right = 15.1;
    let options = Options::default();
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);

    for (position, side, title_width, content_width) in [
        (TitlePosition::TopRight, Side::Top, 2030.0, 84.2),
        (TitlePosition::BottomRight, Side::Bottom, 1859.1, 297.0),
    ] {
        spec.title_position = position;
        spec.title = Some(size(title_width, 18.0));
        let content = size(content_width, 40.0);
        let inner = Contact {
            at: face(
                Rect {
                    width: content.width,
                    height: content.height,
                    ..Rect::default()
                },
                side,
                content.width / 2.0,
            ),
            side,
        };
        let wires = [Wire {
            ends: [
                End::Border {
                    slot: 0,
                    side,
                    flat: None,
                    hint: PortSideHint::Auto,
                },
                End::Item {
                    slot: 0,
                    hint: PortSideHint::Auto,
                    child: None,
                },
            ],
            spec: edge(GroupId(0), NodeId(0)),
            route: None,
            contacts: [None, Some(inner)],
        }];
        let fit =
            fit::Fit::new(Some(&spec), content, &[], &wires, 0.0, &options, &mut work).unwrap();
        let mut ports = ports::border(&wires, &[], &fit, 1, &options, &mut work).unwrap();
        let title = fit
            .title(Some(&spec), &wires, &ports, &mut work)
            .unwrap()
            .unwrap()
            .rect;
        let trunk = inner.at.x + fit.offset.x;
        assert!(trunk < title.x - fit.title_margin(), "{position:?}");
        ports::beside_title(&wires, &mut ports, title, side, &fit, &options, &mut work).unwrap();
        assert_eq!(ports[0].x, trunk);
    }
}

#[test]
fn automatic_top_titles_prefer_left_right_then_center() {
    let mut spec = group(0, None, Direction::Down);
    spec.title = Some(size(80., 18.));
    spec.min_size = size(320., 180.);
    spec.title_position = TitlePosition::TopAuto;
    let mut graph = Graph {
        nodes: vec![node(0, 40., 24., Some(0))],
        groups: vec![spec],
        ..Graph::default()
    };
    let left = 15.375 + 40.;
    let right = 320. - 17.25 - 40.;
    let center = 160.;
    for (crossings, expected) in [
        (&[][..], left),
        (&[left][..], right),
        (&[left, right][..], center),
        (&[left, right, center + 25.][..], center),
    ] {
        graph.edges = crossings
            .iter()
            .map(|&x| {
                let mut edge = edge(GroupId(0), NodeId(0));
                edge.source_port = PortSideHint::FixedOffset {
                    side: Side::Top,
                    offset: x - center,
                };
                edge
            })
            .collect();
        let drawing = layout(&graph, &independent()).unwrap();
        let frame = &drawing.groups[0];
        assert_eq!(frame.rect.width, 320.);
        assert_eq!(frame.title_center.unwrap().x - frame.rect.x, expected);
        if crossings.len() == 3 {
            assert!(frame.title_struck);
        }
        for (edge, &x) in drawing.edges.iter().zip(crossings) {
            assert_eq!(tip(edge, 0).x - frame.rect.x, x);
        }
    }
}

#[test]
fn cross_group_routes_clear_automatic_titles_in_vertical_and_fit_width_layouts() {
    for (direction, width_policy) in [
        (Direction::Down, WidthPolicy::Natural),
        (Direction::Up, WidthPolicy::Natural),
        (
            Direction::Right,
            WidthPolicy::FitWidth {
                max_width: 320.0,
                allow_turn: true,
                turn_breakpoint: 640.0,
                fold_wide_ranks: true,
            },
        ),
    ] {
        let graph = Graph {
            direction,
            groups: (0..3)
                .map(|seq| GroupSpec {
                    title: Some(size(125.0, 21.0)),
                    title_position: TitlePosition::TopAuto,
                    padding: Insets {
                        top: 37.0,
                        left: 8.0,
                        right: 8.0,
                        bottom: 8.0,
                    },
                    ..GroupSpec::new(seq)
                })
                .collect(),
            nodes: (0..6)
                .map(|seq| node(seq, 56.0, 36.0, Some(seq / 2)))
                .collect(),
            edges: (0..5)
                .map(|source| {
                    let mut edge = edge(NodeId(source), NodeId(source + 1));
                    edge.end_marker = MarkerMetrics {
                        width: 7.0,
                        length: 4.0,
                    };
                    edge
                })
                .collect(),
            ..Graph::default()
        };
        let options = Options {
            width_policy,
            ..Options::default()
        };
        let drawing = layout(&graph, &options).unwrap();
        assert!(matches!(drawing.direction, Direction::Down | Direction::Up));
        for (group, spec) in drawing.groups.iter().zip(&graph.groups) {
            let title = centered(group.title_center.unwrap(), spec.title.unwrap());
            assert!(!group.title_struck, "{direction:?} {title:?}");
            for route in &drawing.edges {
                assert!(
                    route.points.windows(2).all(|pair| !enters(
                        title,
                        point(pair[0]),
                        point(pair[1]),
                    )),
                    "{direction:?} {title:?} {:?}",
                    route.points
                );
            }
        }
    }
}

#[test]
fn automatic_top_title_keeps_a_clear_trunk_and_reroutes_an_obstructed_one() {
    let mut graph = Graph::default();
    let mut spec = group(0, None, Direction::Down);
    spec.title = Some(size(80., 18.));
    spec.min_size = size(320., 180.);
    spec.title_position = TitlePosition::TopAuto;
    graph.groups.push(spec);
    graph.nodes.push(node(0, 40., 24., Some(0)));
    graph.nodes.push(node(1, 37., 17., None));
    graph.edges.push(edge(NodeId(1), NodeId(0)));
    let adapted = layout(&graph, &independent()).unwrap();
    let frame = &adapted.groups[0];
    assert_eq!(frame.title_center.unwrap().x - frame.rect.x, 15.375 + 40.);
    assert!(!frame.title_struck);
    let seam = &adapted.border_stitches[0];
    assert_eq!(seam.side, Side::Top);
    let crossing_x = adapted.edges[0].points[seam.point].x;
    assert!((crossing_x - adapted.nodes[0].center().x).abs() < 1e-8);

    let mut options = independent();
    options.group_policy.adaptive_title_clearance = false;
    let fixed = layout(&graph, &options).unwrap();
    assert_eq!(
        fixed.groups[0].title_center.unwrap().x,
        fixed.groups[0].rect.center().x
    );
    graph.groups[0].title_position = TitlePosition::TopCenter;
    assert_eq!(layout(&graph, &independent()).unwrap(), fixed);

    graph.groups[0].title_position = TitlePosition::TopAuto;
    graph.groups[0].title = Some(size(300., 18.));
    let drawing = layout(&graph, &independent()).unwrap();
    let frame = &drawing.groups[0];
    assert!(frame.rect.width > 300. + spec.padding.left + spec.padding.right);
    assert!(!frame.title_struck);
    let seam = drawing.border_stitches[0];
    let points = &drawing.edges[0].points[seam.point..];
    let title = centered(frame.title_center.unwrap(), graph.groups[0].title.unwrap());
    assert!(
        (points[0].x - drawing.nodes[0].center().x).abs() < 1e-8,
        "an automatic title reservation should leave the only entrance on its trunk: {points:?}"
    );
    assert!(
        points[0].x <= title.x - TITLE_CLEAR || points[0].x >= title.x + title.width + TITLE_CLEAR
    );
    assert!(
        points
            .windows(2)
            .all(|pair| !enters(title, point(pair[0]), point(pair[1])))
    );
}

#[test]
fn automatic_title_placement_leaves_the_lateral_entry_clear() {
    let spec = GroupSpec {
        title: Some(size(80., 18.)),
        title_position: TitlePosition::TopAuto,
        min_size: size(320., 180.),
        padding: Insets {
            top: 32.5,
            left: 15.,
            right: 17.,
            ..Insets::default()
        },
        ..GroupSpec::new(0)
    };
    let options = Options::default();
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let frame = fit::Fit::new(
        Some(&spec),
        size(120., 50.),
        &[],
        &[],
        0.,
        &options,
        &mut work,
    )
    .unwrap();
    let lateral = Wire {
        ends: [
            End::Border {
                slot: 0,
                side: Side::Left,
                flat: None,
                hint: PortSideHint::Auto,
            },
            End::Item {
                slot: 0,
                hint: PortSideHint::Auto,
                child: None,
            },
        ],
        spec: edge(GroupId(0), NodeId(0)),
        route: None,
        contacts: [
            None,
            Some(Contact {
                at: Point { x: 22., y: 24. },
                side: Side::Left,
            }),
        ],
    };
    let title = frame
        .title(
            Some(&spec),
            &[lateral],
            &[Point { x: 0., y: 17. }],
            &mut work,
        )
        .unwrap()
        .unwrap();
    assert_eq!(title.rect.center().x, 263.);
}

#[test]
fn an_automatic_title_can_fit_between_top_ports_without_widening_the_group() {
    let spec = GroupSpec {
        title: Some(size(60., 20.)),
        title_position: TitlePosition::TopAuto,
        min_size: size(320., 150.),
        padding: Insets {
            left: 20.,
            right: 20.,
            top: 30.,
            ..Insets::default()
        },
        ..GroupSpec::new(0)
    };
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let fit = fit::Fit::new(
        Some(&spec),
        size(120., 50.),
        &[],
        &[],
        0.,
        &Options::default(),
        &mut work,
    )
    .unwrap();
    let wires = std::array::from_fn::<_, 3, _>(|slot| Wire {
        ends: [
            End::Border {
                slot,
                side: Side::Top,
                flat: None,
                hint: PortSideHint::Auto,
            },
            End::Item {
                slot: 0,
                hint: PortSideHint::Auto,
                child: None,
            },
        ],
        spec: edge(GroupId(0), NodeId(0)),
        route: None,
        contacts: [None; 2],
    });
    let points = [50., 160., 270.].map(|x| Point { x, y: 0. });
    let placement = fit
        .title(Some(&spec), &wires, &points, &mut work)
        .unwrap()
        .unwrap();
    assert_eq!(placement.reserve_at, None);
    let title = placement.rect;
    assert!(
        points
            .iter()
            .all(|p| p.x <= title.x - TITLE_CLEAR || p.x >= title.x + title.width + TITLE_CLEAR)
    );
}

#[test]
fn fixed_contacts_under_a_title_do_not_consume_either_clear_port_run() {
    let options = Options::default();
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let frame = fit::Fit::new(None, size(100., 100.), &[], &[], 0., &options, &mut work).unwrap();
    let spec = edge(GroupId(0), NodeId(0));
    let mut points = [
        Point { x: 40., y: 0. },
        Point { x: 50., y: 0. },
        Point { x: 64., y: 0. },
    ];
    let wires = std::array::from_fn::<_, 3, _>(|slot| Wire {
        ends: [
            End::Border {
                slot,
                side: Side::Top,
                flat: None,
                hint: if slot == 0 {
                    PortSideHint::FixedOffset {
                        side: Side::Top,
                        offset: -10.,
                    }
                } else {
                    PortSideHint::Auto
                },
            },
            End::Item {
                slot: 0,
                hint: PortSideHint::Auto,
                child: None,
            },
        ],
        spec,
        route: None,
        contacts: [None; 2],
    });
    let title = centered(
        Point { x: 50., y: 15. },
        size(2. * (36. - TITLE_CLEAR), 18.),
    );
    ports::beside_title(
        &wires,
        &mut points,
        title,
        Side::Top,
        &frame,
        &options,
        &mut work,
    )
    .unwrap();
    assert_eq!(
        points.map(|point| point.x),
        [
            40.,
            title.x - TITLE_CLEAR,
            title.x + title.width + TITLE_CLEAR
        ]
    );
}

#[test]
fn bottom_title_reports_a_strike_from_an_explicit_bottom_contact() {
    let mut graph = Graph {
        direction: Direction::Up,
        groups: vec![GroupSpec {
            title: Some(size(100.0, 20.0)),
            title_position: TitlePosition::BottomCenter,
            padding: Insets {
                bottom: 24.0,
                ..Insets::default()
            },
            ..GroupSpec::new(0)
        }],
        nodes: vec![node(0, 80.0, 30.0, Some(0))],
        ..Graph::default()
    };
    let mut wire = edge(GroupId(0), NodeId(0));
    wire.source_port = PortSideHint::FixedOffset {
        side: Side::Bottom,
        offset: 0.0,
    };
    graph.edges.push(wire);
    let drawing = layout(&graph, &Options::default()).unwrap();
    let frame = &drawing.groups[0];
    assert_eq!(tip(&drawing.edges[0], 0).x, frame.rect.center().x);
    assert!(frame.title_struck);
}

#[test]
fn mixed_member_and_own_border_markers_have_a_full_shaft_on_both_sides_of_a_wide_title() {
    for own in [false, true] {
        let mut graph = Graph::default();
        graph.groups.push(group(0, None, Direction::Right));
        graph.groups[0].title = Some(size(77., 17.));
        graph.groups[0].padding = Insets {
            top: 30.,
            bottom: 15.,
            left: 15.,
            right: 15.,
        };
        graph.nodes = vec![node(0, 79., 32., Some(0)), node(1, 40., 24., None)];
        let mut spec = if own {
            edge(GroupId(0), NodeId(0))
        } else {
            edge(NodeId(1), NodeId(0))
        };
        spec.end_marker = MarkerMetrics {
            length: 10.,
            width: 9.,
        };
        if own {
            spec.source_port = PortSideHint::Side(Side::Top);
            spec.start_marker = MarkerMetrics {
                length: 12.,
                width: 9.,
            };
        }
        graph.edges.push(spec);
        let result = layout(&graph, &independent()).unwrap();
        let rect = result.groups[0].rect;
        let title = centered(
            result.groups[0].title_center.unwrap(),
            graph.groups[0].title.unwrap(),
        );
        let route = &result.edges[0];
        let target = tip(route, 1);
        let before = point(route.points[route.points.len() - 2]);
        assert_eq!(before.x, target.x);
        assert!(before.y <= target.y - 10.);
        if own {
            let start = tip(route, 0);
            assert_eq!(start.y, rect.y);
            assert_eq!(start.x, route.points[1].x);
            assert!(route.points[1].y >= start.y + 12.);
        }
        for segment in route.points.windows(2) {
            assert!(!enters(title, point(segment[0]), point(segment[1])));
        }
    }
}

#[test]
fn perpendicular_group_entry_clears_a_self_loop_label_with_the_full_marker() {
    let mut graph = Graph {
        direction: Direction::Up,
        ..Default::default()
    };
    let mut frame = group(0, None, Direction::Right);
    frame.padding = Insets {
        top: 37.,
        right: 8.,
        bottom: 8.,
        left: 8.,
    };
    frame.title = Some(size(32.48, 21.));
    graph.groups.push(frame);
    graph.nodes = vec![node(1, 100., 81., Some(0)), node(2, 100., 81., None)];
    graph.nodes[0].port_span = Some(84.);
    graph.nodes[0].perpendicular_port_span = Some(65.);
    let mut loop_edge = edge(NodeId(0), NodeId(0));
    loop_edge.label = Some(size(21.98, 21.));
    let mut incoming = edge(NodeId(1), NodeId(0));
    incoming.label = Some(size(48.16, 21.));
    let mut options = independent();
    options.spacing.node = 140.;
    options.spacing.rank = 80.;
    options.spacing.component = 140.;

    for (loop_start, loop_end, entry, width) in [(42.5, 27.5, 54.5, 19.), (45., 30., 57., 36.)] {
        loop_edge.start_marker = MarkerMetrics {
            length: loop_start,
            width,
        };
        loop_edge.end_marker = MarkerMetrics {
            length: loop_end,
            width: 18.,
        };
        incoming.start_marker = MarkerMetrics {
            length: 18.,
            width: 18.,
        };
        incoming.end_marker = MarkerMetrics {
            length: entry,
            width,
        };
        graph.edges = vec![loop_edge, incoming];
        let drawing = layout(&graph, &options).unwrap();
        let member = drawing.nodes[0];
        let group = drawing.groups[0].rect;
        let route = &drawing.edges[1];
        assert_eq!(route.ports.1, Side::Bottom);
        let end = tip(route, 1);
        assert_eq!(end.y, member.y + member.height);
        assert!(end.x > member.x && end.x < member.x + member.width);
        let shaft = route
            .points
            .iter()
            .rev()
            .take_while(|point| point.x == end.x)
            .last()
            .unwrap();
        assert!(shaft.y - end.y >= entry);
        let label = centered(
            drawing.edges[0].label_center.unwrap(),
            loop_edge.label.unwrap(),
        );
        let footprint = Rect {
            x: end.x - width / 2.,
            y: end.y,
            width,
            height: entry,
        };
        assert!(!overlap(footprint, label));
        for x in [footprint.x, footprint.x + footprint.width] {
            assert!(inside(
                Point {
                    x,
                    y: footprint.y + footprint.height,
                },
                group,
            ));
        }
        for segment in route.points.windows(2) {
            assert!(!enters(label, point(segment[0]), point(segment[1])));
        }
        let [stitch] = drawing.border_stitches.as_slice() else {
            panic!("expected one border stitch");
        };
        assert_eq!(
            (stitch.edge, stitch.group, stitch.side),
            (EdgeId(1), GroupId(0), Side::Bottom)
        );
        assert_eq!(route.points[stitch.point].y, group.y + group.height);
    }
}

#[test]
fn nested_title_fan_stitches_beside_the_measured_title_and_never_doubles_back() {
    let mut graph = Graph::default();
    let parent = |index| {
        if index < 4 {
            Some(0)
        } else if index < 6 {
            Some(1)
        } else {
            None
        }
    };
    for (index, (width, height)) in [
        (171.25, 45.),
        (171., 171.),
        (152., 45.),
        (186., 36.),
        (144., 85.14869888475837),
        (171.25, 45.),
        (171.25, 45.),
    ]
    .into_iter()
    .enumerate()
    {
        graph
            .nodes
            .push(node(index as u32, width, height, parent(index)));
    }
    graph.nodes[4].port_span = Some(144. * 0.6);
    for (index, width) in [87.78, 87.78, 85.54].into_iter().enumerate() {
        let mut spec = group(
            index as u32 + 7,
            (index < 2).then_some(2),
            if index < 2 {
                Direction::Down
            } else {
                Direction::Right
            },
        );
        spec.padding = Insets {
            left: 8.,
            right: 8.,
            top: 37.,
            bottom: 8.,
        };
        spec.title = Some(size(width, 21.));
        graph.groups.push(spec);
    }
    for (source, target) in [(0, 1), (1, 2), (1, 3), (2, 4), (3, 4), (4, 5), (5, 6)] {
        let mut spec = edge(NodeId(source), NodeId(target));
        spec.start_marker.length = 6.;
        spec.end_marker = MarkerMetrics {
            length: 12.,
            width: 8.,
        };
        graph.edges.push(spec);
    }
    graph.edges[1].label = Some(size(19.74, 21.));
    graph.edges[2].label = Some(size(12.04, 21.));
    for direction in [Direction::Down, Direction::Up] {
        graph.direction = direction;
        let top = direction == Direction::Down;
        graph.groups[1].title_position = if top {
            TitlePosition::TopCenter
        } else {
            TitlePosition::BottomCenter
        };
        let output = layout(&graph, &Options::default()).unwrap();
        let group = &output.groups[1];
        let title = centered(group.title_center.unwrap(), graph.groups[1].title.unwrap());
        let border = if top {
            group.rect.y
        } else {
            group.rect.y + group.rect.height
        };
        for (index, route) in output.edges[3..=4].iter().enumerate() {
            let seam = route
                .points
                .iter()
                .position(|p| (p.y - border).abs() < 1e-8 && inside(point(*p), group.rect))
                .unwrap();
            let run = &route.points[seam..];
            let entry = run[0];
            let clearance = if index == 0 {
                title.x - entry.x
            } else {
                entry.x - title.x - title.width
            };
            assert!(clearance >= TITLE_CLEAR - 1e-8, "{run:?}");
            assert_eq!(entry.x, run[1].x);
            let past_title = if top {
                run[1].y - title.y - title.height
            } else {
                title.y - run[1].y
            };
            assert!(past_title >= TITLE_CLEAR - 1e-8);
            let shift = (run[2].x - run[1].x).abs();
            let room = (run[2].y - run[1].y).abs();
            assert!(room + 1e-8 >= (shift * 0.75).max(16.0), "{run:?}");
            let across: f64 = run
                .windows(2)
                .map(|pair| (pair[0].x - pair[1].x).abs())
                .sum();
            assert!(
                (across - (entry.x - run.last().unwrap().x).abs()).abs() < 1e-8,
                "{run:?}"
            );
            assert!(run.iter().all(|&p| inside(point(p), group.rect)));
        }
    }
}

#[test]
fn explicit_own_port_on_the_title_keeps_its_offset_and_has_an_inward_shaft() {
    let mut graph = Graph::default();
    let mut spec = group(0, None, Direction::Down);
    spec.title = Some(size(100., 20.));
    graph.groups.push(spec);
    graph.nodes.push(node(0, 45., 25., Some(0)));
    let mut own = edge(GroupId(0), NodeId(0));
    own.source_port = PortSideHint::FixedOffset {
        side: Side::Top,
        offset: 5.,
    };
    own.start_marker = MarkerMetrics {
        length: 13.,
        width: 6.,
    };
    graph.edges.push(own);
    let output = layout(&graph, &independent()).unwrap();
    let group = &output.groups[0];
    let title = centered(group.title_center.unwrap(), graph.groups[0].title.unwrap());
    let route = &output.edges[0];
    let at = tip(route, 0);
    assert_eq!(
        at,
        Point {
            x: group.rect.center().x + 5.,
            y: group.rect.y
        }
    );
    assert_eq!(route.points[1].x, at.x);
    assert!(route.points[1].y >= at.y + 13.);
    assert!(
        route
            .points
            .windows(2)
            .all(|pair| !enters(title, point(pair[0]), point(pair[1])))
    );
}

#[test]
fn a_mixed_leg_reserves_the_marker_on_its_requested_member_face() {
    let mut graph = Graph::default();
    let mut owner = GroupSpec::new(0);
    owner.direction = Some(Direction::Left);
    graph.groups.push(owner);
    graph.nodes = vec![node(0, 48., 31., Some(0)), node(1, 55., 33., None)];
    let mut wire = edge(NodeId(1), NodeId(0));
    wire.end_marker = MarkerMetrics {
        length: 70.,
        width: 8.,
    };
    graph.edges.push(wire);
    for side in SIDES {
        graph.edges[0].target_port = PortSideHint::FixedOffset { side, offset: 3. };
        let output = layout(&graph, &independent()).unwrap();
        let route = &output.edges[0];
        let target = tip(route, 1);
        let expected = face(
            output.nodes[0],
            side,
            cross(side, output.nodes[0].center()) + 3.,
        );
        assert_eq!(target, expected);
        let shaft = towards(target, side, 70.);
        assert!(inside(shaft, output.groups[0].rect));
        let before = point(route.points[route.points.len() - 2]);
        assert_eq!(cross(side, before), cross(side, target));
        assert!((before.x - target.x).abs() + (before.y - target.y).abs() >= 70.);
    }
}

#[test]
fn fixed_offsets_on_non_flat_group_items_must_fit_the_physical_face() {
    let mut graph = Graph {
        groups: vec![
            group(0, None, Direction::Right),
            group(1, Some(0), Direction::Right),
        ],
        nodes: vec![node(0, 56.0, 36.0, Some(1)), node(1, 40.0, 30.0, None)],
        edges: vec![edge(NodeId(1), GroupId(1))],
        ..Graph::default()
    };
    for offset in [-1_000.0, 1_000.0, f64::MAX] {
        graph.edges[0].target_port = PortSideHint::FixedOffset {
            side: Side::Top,
            offset,
        };
        assert_eq!(
            layout(&graph, &independent()),
            Err(LayoutError::InvalidInput("port offset outside face"))
        );
    }

    graph.edges[0].target_port = PortSideHint::FixedOffset {
        side: Side::Top,
        offset: -12.0,
    };
    let drawing = layout(&graph, &independent()).unwrap();
    let rect = drawing.groups[1].rect;
    assert_eq!(
        tip(&drawing.edges[0], 1),
        face(rect, Side::Top, rect.center().x - 12.0)
    );
}

#[test]
fn compound_auto_ports_try_other_faces_when_a_sibling_blocks_the_shaft() {
    let rects = [
        Rect {
            x: 40.0,
            y: 66.0,
            width: 32.0,
            height: 40.0,
        },
        Rect {
            x: 28.0,
            y: 24.0,
            width: 58.0,
            height: 32.0,
        },
        Rect {
            x: 56.0,
            ..Rect::default()
        },
    ];
    let graph = Graph {
        direction: Direction::Right,
        nodes: vec![node(0, 32.0, 40.0, None), node(1, 58.0, 32.0, None)],
        ..Graph::default()
    };
    let mut spec = edge(GroupId(0), NodeId(0));
    spec.end_marker = MarkerMetrics {
        length: 45.0,
        width: 8.0,
    };
    let options = independent();
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    for (flat, hint) in [
        (None, PortSideHint::Side(Side::Top)),
        (None, PortSideHint::Auto),
        (Some(NodeId(2)), PortSideHint::Auto),
    ] {
        spec.label = flat.map(|_| size(10.0, 8.0));
        let mut wires = [Wire {
            ends: [
                End::Border {
                    slot: 0,
                    side: Side::Top,
                    flat,
                    hint: PortSideHint::Auto,
                },
                End::Item {
                    slot: 0,
                    hint,
                    child: None,
                },
            ],
            spec,
            route: None,
            contacts: [None; 2],
        }];
        ports::items(&mut wires, &graph, &rects[..2], &options, &mut work).unwrap();
        let fit = fit::Fit::new(
            None,
            size(90.0, 106.0),
            &rects,
            &wires,
            0.0,
            &options,
            &mut work,
        )
        .unwrap();
        let ports = ports::border(&wires, &rects, &fit, 1, &options, &mut work).unwrap();
        let nodes = rects.map(|rect| Rect {
            x: rect.x + fit.offset.x,
            y: rect.y + fit.offset.y,
            ..rect
        });
        let routes = routes::finish(
            &fit,
            None,
            &nodes[..2],
            &mut wires,
            &ports,
            &options,
            &mut work,
        );
        if hint == PortSideHint::Auto {
            let route = &routes.unwrap()[0];
            let side = route.ports.1;
            assert_ne!(side, Side::Top);
            let end = tip(route, 1);
            assert_eq!(end, face(nodes[0], side, cross(side, end)));
            let previous = point(route.points[route.points.len() - 2]);
            assert_eq!(cross(side, previous), cross(side, end));
            assert!((previous.x - end.x).hypot(previous.y - end.y) >= spec.end_marker.length);
        } else {
            assert_eq!(
                routes,
                Err(LayoutError::InvalidInput(
                    "compound port has no room for its marker shaft"
                ))
            );
        }
    }
}

fn oversized_boundary_marker() -> (Graph, Options) {
    let mut graph = Graph::default();
    let mut owner = GroupSpec::new(0);
    owner.direction = Some(Direction::Right);
    owner.min_size = size(60.0, 50.0);
    graph.groups.push(owner);
    graph.nodes = vec![node(0, 40.0, 30.0, Some(0)), node(1, 40.0, 100.0, None)];
    let mut own = edge(GroupId(0), NodeId(0));
    own.start_marker = MarkerMetrics {
        length: 12.0,
        width: 180.0,
    };
    graph.edges.push(own);
    let mut options = independent();
    options.width_policy = WidthPolicy::FitWidth {
        max_width: 160.0,
        allow_turn: false,
        turn_breakpoint: 640.0,
        fold_wide_ranks: false,
    };
    (graph, options)
}

#[test]
fn fit_width_packs_an_inward_oversized_marker_clear_of_the_next_component() {
    let (mut graph, options) = oversized_boundary_marker();
    let fixed = |offset| PortSideHint::FixedOffset {
        side: Side::Top,
        offset,
    };
    graph.edges[0].source_port = fixed(0.0);
    let mut legacy = options;
    legacy.width_policy = WidthPolicy::Natural;
    legacy.packing = PackingPolicy::FitWidth { max_width: 160.0 };
    let natural = layout(&graph, &legacy).unwrap();
    assert_eq!(natural.bounds.width, 150.0);
    assert_eq!(natural.groups[0].rect.width, 60.0);

    for (hint, expected_width) in [
        (fixed(0.0), 180.0),
        (fixed(20.0), 220.0),
        (fixed(-20.0), 220.0),
        (PortSideHint::Side(Side::Top), 180.0),
    ] {
        graph.edges[0].source_port = hint;
        let drawing = layout(&graph, &options).unwrap();
        let group = drawing.groups[0].rect;
        let route = &drawing.edges[0];
        let contact = tip(route, 0);
        let next = point(route.points[1]);
        let footprint = Rect {
            x: contact.x - 90.0,
            y: contact.y,
            width: 180.0,
            height: 12.0,
        };
        assert_eq!(drawing.bounds.width, expected_width, "{hint:?}");
        assert_eq!(group.width, expected_width, "{hint:?}");
        assert_eq!(drawing.groups[0].effective_direction, Direction::Right);
        assert_eq!(route.ports.0, Side::Top);
        assert_eq!(contact.y, group.y);
        assert_eq!(next.x, contact.x);
        assert!(next.y - contact.y >= footprint.height);
        assert!(footprint.x >= group.x);
        assert!(footprint.x + footprint.width <= group.x + group.width);
        assert!(!overlap(footprint, drawing.nodes[1]));
        if let PortSideHint::FixedOffset { offset, .. } = hint {
            assert_eq!(contact.x - group.center().x, offset);
        }
    }
    graph.edges[0].source_port = fixed(31.0);
    assert_eq!(
        layout(&graph, &options),
        Err(LayoutError::InvalidInput("port offset outside group face"))
    );
}

#[test]
fn fit_width_preserves_marker_room_when_moving_a_port_beside_the_title() {
    let (mut graph, options) = oversized_boundary_marker();
    graph.groups[0].title = Some(size(80.0, 12.0));
    graph.groups[0].title_position = TitlePosition::TopLeft;
    graph.edges[0].source_port = PortSideHint::Side(Side::Top);
    let drawing = layout(&graph, &options).unwrap();
    let group = &drawing.groups[0];
    let title = centered(group.title_center.unwrap(), graph.groups[0].title.unwrap());
    let route = &drawing.edges[0];
    let contact = tip(route, 0);
    let footprint = Rect {
        x: contact.x - 90.0,
        y: contact.y,
        width: 180.0,
        height: 12.0,
    };
    assert_eq!(route.ports.0, Side::Top);
    assert_eq!(contact.y, group.rect.y);
    assert!(footprint.x >= title.x + title.width + TITLE_CLEAR);
    assert!(footprint.x + footprint.width <= group.rect.x + group.rect.width);
    assert!(!overlap(footprint, drawing.nodes[1]));
}
