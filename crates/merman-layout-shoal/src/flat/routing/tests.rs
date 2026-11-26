use super::*;
use crate::flat::Link;
use crate::{EdgeSpec, NodeId, NodeSpec, NoopWorkControl, WorkControl};

const STYLES: [RoutingStyle; 3] = [
    RoutingStyle::Curved,
    RoutingStyle::Orthogonal,
    RoutingStyle::Polyline,
];

fn circle_arrivals(direction: Direction) -> Graph {
    let mut graph = Graph {
        direction,
        nodes: (0..4)
            .map(|seq| {
                NodeSpec::new(
                    Size {
                        width: 40.0,
                        height: 24.0,
                    },
                    seq,
                )
            })
            .collect(),
        edges: [(0, 1), (0, 2), (1, 3), (2, 3)]
            .into_iter()
            .map(|(from, to)| {
                let mut edge = EdgeSpec::new(NodeId(from).into(), NodeId(to).into());
                edge.end_marker = MarkerMetrics {
                    length: 10.0,
                    width: 12.0,
                };
                edge.set_marker_runways([0.0, 16.0]);
                edge
            })
            .collect(),
        ..Graph::default()
    };
    let circle = &mut graph.nodes[3];
    circle.size = Size {
        width: 14.0,
        height: 14.0,
    };
    circle.contour = ShapeContourHint::Ellipse;
    circle.port_span = Some(14.0 * 0.55);
    circle.perpendicular_port_span = circle.port_span;
    graph
}

#[test]
fn small_terminal_circle_uses_upper_and_lower_arrivals_with_full_marker_leads() {
    for direction in [Direction::Right, Direction::Left] {
        let mut graph = circle_arrivals(direction);
        graph.nodes[3].split_incoming_ports = true;
        let drawing = crate::layout(&graph, &Options::default()).unwrap();
        let circle = drawing.nodes[3];
        let center = circle.center();
        for (edge, source) in drawing.edges[2..].iter().zip(&drawing.nodes[1..3]) {
            let (face, y, sign) = if source.center().y < center.y {
                (Side::Top, circle.y, -1.0)
            } else {
                (Side::Bottom, circle.y + circle.height, 1.0)
            };
            assert_eq!(edge.ports.1, face, "{direction:?}: {edge:?}");
            let [.., lead, target] = edge.points.as_slice() else {
                panic!("expected a normal marker lead");
            };
            assert_eq!((lead.x, target.x, target.y), (center.x, center.x, y));
            assert!(sign * (lead.y - y) >= 16.0);
        }
        assert_ne!(drawing.edges[2].ports.1, drawing.edges[3].ports.1);
    }
}

#[test]
fn terminal_arrival_hint_respects_fixed_ports_and_nonterminal_ellipse_geometry() {
    let options = Options::default();
    let mut graph = circle_arrivals(Direction::Right);
    let ordinary = crate::layout(&graph, &options).unwrap();
    assert!(
        ordinary.edges[2..]
            .iter()
            .any(|edge| edge.ports.1 == Side::Left)
    );

    graph.nodes[3].split_incoming_ports = true;
    graph.edges[2].target_port = PortSideHint::FixedOffset {
        side: Side::Left,
        offset: -2.0,
    };
    let drawing = crate::layout(&graph, &options).unwrap();
    let contact = drawing.edges[2].points.last().unwrap();
    assert_eq!(drawing.edges[2].ports.1, Side::Left);
    assert_eq!(contact.y, drawing.nodes[3].center().y - 2.0);
    assert_eq!(drawing.edges[3].ports.1, Side::Bottom);

    for (direction, diameter) in [(Direction::Down, 14.0), (Direction::Right, 40.0)] {
        let mut graph = circle_arrivals(direction);
        graph.nodes[3].size = Size {
            width: diameter,
            height: diameter,
        };
        let before = crate::layout(&graph, &options).unwrap();
        graph.nodes[3].split_incoming_ports = true;
        let after = crate::layout(&graph, &options).unwrap();
        assert_eq!(before.edges, after.edges);
    }
}

#[test]
fn parallel_and_reverse_occurrences_share_symmetric_slots_with_or_without_dummies() {
    let mut graph = Graph {
        nodes: (0..2)
            .map(|seq| {
                NodeSpec::new(
                    Size {
                        width: 40.0,
                        height: 40.0,
                    },
                    seq,
                )
            })
            .collect(),
        edges: [(0, 1), (1, 0), (0, 1), (0, 1), (0, 1)]
            .into_iter()
            .map(|(a, b)| EdgeSpec::new(NodeId(a).into(), NodeId(b).into()))
            .collect(),
        ..Graph::default()
    };
    for direction in [Direction::Down, Direction::Right] {
        graph.direction = direction;
        for cap in [Options::default().caps.max_dummies, 2, 0] {
            let mut options = Options::default();
            options.caps.max_dummies = cap;
            let drawing = crate::layout(&graph, &options).unwrap();
            assert_eq!(drawing.stats.reversed_edges, 1);
            let cross = |x, y| if direction == Direction::Down { x } else { y };
            let mut slots = Vec::new();
            for (spec, route) in graph.edges.iter().zip(&drawing.edges) {
                let [first, .., last] = route.points.as_slice() else {
                    panic!("missing contacts")
                };
                let (a, b) = if spec.source == NodeId(0).into() {
                    (first, last)
                } else {
                    (last, first)
                };
                let top = drawing.nodes[0].center();
                let bottom = drawing.nodes[1].center();
                let offset = cross(a.x, a.y) - cross(top.x, top.y);
                assert_eq!(offset, cross(b.x, b.y) - cross(bottom.x, bottom.y));
                slots.push(offset);
            }
            slots.sort_by(f64::total_cmp);
            assert_eq!(
                slots,
                [-8.0, -4.0, 0.0, 4.0, 8.0],
                "{direction:?}, cap {cap}"
            );
        }
    }
}

#[test]
fn a_parallel_pair_reuses_its_contiguous_port_run_on_a_shared_face() {
    let graph = Graph {
        nodes: (0..3)
            .map(|seq| {
                NodeSpec::new(
                    Size {
                        width: 40.0,
                        height: 24.0,
                    },
                    seq,
                )
            })
            .collect(),
        edges: [(0, 1), (0, 1), (0, 1), (0, 1), (1, 0), (0, 2)]
            .into_iter()
            .map(|(a, b)| EdgeSpec::new(NodeId(a).into(), NodeId(b).into()))
            .collect(),
        ..Graph::default()
    };
    let drawing = crate::layout(&graph, &Options::default()).unwrap();
    let contact = |route: &EdgeGeometry, rev| {
        let (a, b) = (
            route.points.first().unwrap().x,
            route.points.last().unwrap().x,
        );
        if rev { [b, a] } else { [a, b] }
    };
    let mut slots: Vec<_> = drawing.edges[..5]
        .iter()
        .enumerate()
        .map(|(edge, route)| contact(route, edge == 4))
        .collect();
    slots.sort_by(|a, b| a[0].total_cmp(&b[0]));
    let other = drawing.edges[5].points[0].x;
    assert!(
        slots[4][0] < other,
        "the neighboring port keeps its own run"
    );
    for pair in slots.windows(2) {
        assert!(pair[1][0] > pair[0][0]);
        assert!((pair[1][0] - pair[0][0] - 2.0).abs() < 1e-8);
        assert!((pair[1][1] - pair[0][1] - 2.0).abs() < 1e-8);
    }
}

#[test]
fn a_shared_face_keeps_the_deepest_measured_marker_runway() {
    let mut spec = EdgeSpec::new(NodeId(0).into(), NodeId(1).into());
    spec.end_marker = MarkerMetrics {
        length: 14.0,
        width: 10.0,
    };
    spec.set_marker_runways([0.0, 14.0]);
    assert_eq!(spec.sweep_runways(), [0.0, 14.0]);

    let mut room = EndRoom::default();
    for (length, runway) in [(14.0, 14.0), (12.0, 24.0), (0.0, 6.0)] {
        let marker = MarkerMetrics {
            length,
            width: 10.0,
        };
        room.include(marker, runway, None, Direction::Down, 0.0)
            .unwrap();
    }
    assert_eq!(room.guard(false), Ok(14.0));
    assert_eq!(room.guard(true), Ok(24.0));
}

struct Scene {
    g: Expanded,
    c: Component,
    orientation: Orientation,
    graph: Graph,
    options: Options,
    x: Vec<f64>,
}

impl Scene {
    fn new(heights: &[f64], layers: &[&[u32]], chains: &[&[u32]], x: &[f64]) -> Self {
        let mut g = Expanded {
            layers: layers.iter().map(|row| row.to_vec()).collect(),
            logical_rank: (0..layers.len() as u64).collect(),
            rank: vec![0; x.len()],
            width: vec![0.0; x.len()],
            height: vec![0.0; x.len()],
            kind: vec![Kind::Dummy; x.len()],
            keep: vec![true; x.len()],
            label_vertex: vec![None; chains.len()],
            ..Expanded::default()
        };
        for (rank, layer) in layers.iter().enumerate() {
            for &v in *layer {
                g.rank[v as usize] = rank as u32;
            }
        }
        let mut graph = Graph::default();
        let mut c = Component::default();
        for (id, &height) in heights.iter().enumerate() {
            g.kind[id] = Kind::Node;
            g.width[id] = 16.0;
            g.height[id] = height;
            graph.nodes.push(NodeSpec::new(
                Size {
                    width: 16.0,
                    height,
                },
                id as u32,
            ));
            c.ids.push(id as u32);
        }
        c.nodes = graph.nodes.clone();
        c.margin_left = vec![0.0; heights.len()];
        c.margin_right = vec![0.0; heights.len()];
        for (id, &chain) in chains.iter().enumerate() {
            let (from, to) = (chain[0], *chain.last().unwrap());
            graph
                .edges
                .push(EdgeSpec::new(NodeId(from).into(), NodeId(to).into()));
            c.links.push(Link {
                from,
                to,
                edge: id as u32,
                span: 1,
                weight: 1.0,
            });
            let direct = chain.len() == 2 && g.rank[to as usize] - g.rank[from as usize] > 1;
            g.direct.push(direct);
            g.chains.push(chain.to_vec());
            if !direct {
                for pair in chain.windows(2) {
                    g.from.push(pair[0]);
                    g.to.push(pair[1]);
                    g.source_offset.push(0.0);
                    g.target_offset.push(0.0);
                }
            }
        }
        let orientation = Orientation {
            topo: c.ids.clone(),
            reversed: vec![false; chains.len()],
            cyclic_sccs: 0,
        };
        Self {
            g,
            c,
            graph,
            orientation,
            x: x.to_vec(),
            options: Options::default(),
        }
    }

    fn reverse(&mut self, edge: usize) {
        let link = &mut self.c.links[edge];
        std::mem::swap(&mut link.from, &mut link.to);
        let spec = &mut self.graph.edges[link.edge as usize];
        std::mem::swap(&mut spec.source, &mut spec.target);
        self.orientation.reversed[edge] = true;
    }

    fn label(&mut self, edge: usize, vertex: usize, width: f64, height: f64) {
        self.g.kind[vertex] = Kind::Label;
        self.g.label_vertex[edge] = Some(vertex as u32);
        self.g.height[vertex] = height;
        self.g.width[vertex] = width;
        self.graph.edges[edge].label = Some(Size { width, height });
    }

    fn draw_with(&self, control: &mut impl WorkControl) -> LayoutResult<Drawing> {
        let mut work = Work::new(control);
        let result = route(
            &self.g,
            &self.c,
            &self.orientation,
            &self.graph,
            &self.options,
            &self.x,
            &mut work,
        )?;
        work.finish()?;
        Ok(result)
    }

    fn draw(&self) -> Drawing {
        self.draw_with(&mut NoopWorkControl).unwrap()
    }
}

fn level(points: &[RoutePoint]) -> f64 {
    points
        .windows(2)
        .find(|p| p[0].x != p[1].x && p[0].y == p[1].y)
        .unwrap()[0]
        .y
}

#[test]
fn pure_sources_share_bottom_pure_sinks_share_top_and_mixed_rows_center() {
    let scene = Scene::new(
        &[20., 8., 12., 30., 25., 6.],
        &[&[0, 1], &[2, 3], &[4, 5]],
        &[&[0, 2], &[1, 3], &[2, 4], &[3, 5]],
        &[0., 70., 0., 70., 0., 70.],
    );
    let drawing = scene.draw();
    let nodes = &drawing.nodes;
    assert_eq!(nodes[0].y + nodes[0].height, nodes[1].y + nodes[1].height);
    assert_eq!(nodes[2].center().y, nodes[3].center().y);
    assert_eq!(nodes[4].y, nodes[5].y);
    for (id, geom) in drawing.edges {
        let link = &scene.c.links[id as usize];
        let a = nodes[link.from as usize];
        let b = nodes[link.to as usize];
        assert_eq!(geom.points.len(), 2);
        assert_eq!(
            (geom.points[0].x, geom.points[0].y),
            (a.center().x, a.y + a.height)
        );
        assert_eq!((geom.points[1].x, geom.points[1].y), (b.center().x, b.y));
    }
}

#[test]
fn optical_center_uses_visible_stroke_and_only_unidirectional_label_rows_move() {
    let mut scene = Scene::new(
        &[20., 20.],
        &[&[0], &[2, 3], &[1]],
        &[&[0, 2, 1], &[0, 3, 1]],
        &[0., 0., 30., -30.],
    );
    scene.options.spacing.rank = 100.;
    for (edge, vertex) in [(0, 2), (1, 3)] {
        scene.label(edge, vertex, 12., 16.);
        scene.graph.edges[edge].start_marker.length = 4.;
        scene.graph.edges[edge].end_marker.length = 24.;
    }
    for (reverse, expected_shift) in [(None, -10.), (Some(0), 0.), (Some(1), 10.)] {
        if let Some(edge) = reverse {
            scene.reverse(edge);
        }
        let drawing = scene.draw();
        let midpoint = (drawing.nodes[0].y + drawing.nodes[0].height).midpoint(drawing.nodes[1].y);
        for (id, route) in drawing.edges {
            let center = route.label_center.unwrap();
            assert_eq!(
                center.x,
                scene.x[scene.g.label_vertex[id as usize].unwrap() as usize]
            );
            assert_eq!(center.y, midpoint + expected_shift);
            assert!(
                route
                    .points
                    .iter()
                    .any(|p| p.x == center.x && p.y == center.y)
            );
        }
    }
}

#[test]
fn terminal_stacks_and_markers_grow_only_their_own_gap_in_canonical_axes() {
    let mut scene = Scene::new(
        &[10.; 4],
        &[&[0], &[1, 2], &[3]],
        &[&[0, 1], &[0, 2], &[1, 3]],
        &[0., 80., -80., 80.],
    );
    scene.graph.direction = Direction::Right;
    scene.reverse(1);
    scene.graph.edges[0].start_label = Some(Size {
        width: 23.,
        height: 3.,
    });
    scene.graph.edges[0].start_marker.length = 19.;
    scene.graph.edges[1].end_label = Some(Size {
        width: 37.,
        height: 2.,
    });
    scene.graph.edges[1].end_marker.length = 17.;
    scene.graph.edges[1].start_label = Some(Size {
        width: 26.,
        height: 4.,
    });
    scene.graph.edges[1].start_marker.length = 29.;
    let drawing = scene.draw();
    let upper = 19. + 23. + 37. + 2. * scene.options.spacing.end_label_along;
    let lower = 29. + 26. + scene.options.spacing.end_label_along;
    let boundary = drawing.nodes[0].y + drawing.nodes[0].height;
    assert!(drawing.nodes[1].y - boundary >= upper + lower);
    for (edge, geom) in &drawing.edges[..2] {
        let mut points = geom.points.clone();
        if *edge == 1 {
            points.reverse();
        }
        let change = points.windows(2).find(|p| p[0].x != p[1].x).unwrap();
        assert!(change[0].y - boundary >= upper);
        assert!(drawing.nodes[2].y - change[1].y >= lower);
        assert!(geom.start_label_center.is_none() && geom.end_label_center.is_none());
    }
    assert_eq!(
        drawing.nodes[3].y - drawing.nodes[1].y - drawing.nodes[1].height,
        scene.options.spacing.rank
    );
}

#[test]
fn every_style_keeps_lens_and_protects_occupied_bands_while_omitting_a_bare_dummy_rank() {
    let mut scene = Scene::new(
        &[20., 38., 20.],
        &[&[0], &[3, 7], &[1, 4], &[5, 8], &[6, 9], &[2]],
        &[&[0, 3, 4, 5, 6, 2], &[0, 7, 1], &[1, 8, 9, 2]],
        &[0., 0., 120., 998., 70., 146., 83., 999., 1., 0.],
    );
    scene.label(2, 9, 36., 26.);
    for v in [3, 7, 4, 6, 8] {
        scene.g.keep[v] = false;
    }
    for style in STYLES {
        scene.options.routing = style;
        let drawing = scene.draw();
        let route = &drawing.edges[0].1;
        assert!(route.points.iter().any(|p| p.x == 146.));
        assert!(!route.points.iter().any(|p| p.x == 998.));
        let node = drawing.nodes[1];
        let label = drawing.edges[2].1.label_center.unwrap();
        for (x, top, bottom) in [
            (70., node.y, node.y + node.height),
            (83., label.y - 13., label.y + 13.),
        ] {
            for y in [top, bottom] {
                assert!(route.points.iter().any(|p| p.x == x && p.y == y));
            }
            for pair in route.points.windows(2) {
                if pair[0].x != pair[1].x {
                    assert!(
                        pair[1].y <= top || pair[0].y >= bottom,
                        "{style:?} {pair:?}"
                    );
                } else if pair[0].y < bottom && pair[1].y > top {
                    assert_eq!(pair[0].x, x);
                }
            }
        }
        if style == RoutingStyle::Orthogonal {
            for pair in route.points.windows(2) {
                assert_eq!(
                    pair[1].axis,
                    if pair[0].x == pair[1].x {
                        Axis::Vertical
                    } else {
                        Axis::Horizontal
                    }
                );
                assert!(pair[0].x == pair[1].x || pair[0].y == pair[1].y);
            }
        }
    }
    scene.g.keep[3] = true;
    assert!(scene.draw().edges[0].1.points.iter().any(|p| p.x == 998.));
}

#[test]
fn direct_paths_use_no_segments_and_reverse_paths_preserve_semantic_ports_and_axis() {
    let mut scene = Scene::new(
        &[10.; 3],
        &[&[0], &[1, 3], &[2]],
        &[&[0, 2], &[0, 1], &[0, 2], &[0, 3, 2], &[1, 2]],
        &[5., 80., 120., 30.],
    );
    scene.options.routing = RoutingStyle::Orthogonal;
    scene.g.source_offset[1] = -4.;
    scene.g.target_offset[2] = 6.;
    scene.label(3, 3, 6., 8.);
    scene.graph.edges[0].label = Some(Size {
        width: 12.,
        height: 8.,
    });
    scene
        .graph
        .edges
        .insert(0, EdgeSpec::new(NodeId(0).into(), NodeId(0).into()));
    for link in &mut scene.c.links {
        link.edge += 1;
    }
    let forward = scene.draw();
    scene.reverse(0);
    scene.reverse(3);
    let drawing = scene.draw();
    assert_eq!(
        drawing.edges.iter().map(|e| e.0).collect::<Vec<_>>(),
        [1, 2, 3, 4, 5]
    );
    for id in [0, 2] {
        assert_eq!(drawing.edges[id].1.points.len(), 2);
        assert_eq!(drawing.edges[id].1.label_center, None);
    }
    let old = &forward.edges[3].1;
    let new = &drawing.edges[3].1;
    assert!(new.reversed_for_ranking);
    assert_eq!(new.ports, (Side::Top, Side::Bottom));
    assert_eq!(new.points[0].x, 126.);
    assert_eq!(new.points.last().unwrap().x, 1.);
    assert_eq!(new.label_center, old.label_center);
    for (before, after) in old.points.windows(2).rev().zip(new.points.windows(2)) {
        assert_eq!((before[1].x, before[1].y), (after[0].x, after[0].y));
        assert_eq!(before[1].axis, after[1].axis);
    }
    assert_eq!(new.points[0].axis, Axis::Vertical);
    assert_eq!(forward.edges[4], drawing.edges[4]);
}

fn gap(spans: &[(f64, f64)]) -> Gap {
    Gap {
        height: 50.,
        pieces: spans
            .iter()
            .map(|&(from, to)| {
                let span = Span { from, to };
                Piece {
                    span,
                    occupied: span,
                    leads: [0.0; 2],
                    horizon: None,
                    keep_landing_runway: false,
                    lower: 0,
                    track: None,
                }
            })
            .collect(),
        ..Gap::default()
    }
}

#[test]
fn a_label_inserted_between_adjacent_nodes_shares_one_rank_gap() {
    let mut scene = Scene::new(&[20., 20.], &[&[0], &[2], &[1]], &[&[0, 2, 1]], &[0.; 3]);
    scene.label(0, 2, 25., 21.);
    scene.g.inserted_ranks.push(1);
    let drawing = scene.draw();
    let gap = |drawing: &Drawing| drawing.nodes[1].y - drawing.nodes[0].y - 20.;
    assert_eq!(gap(&drawing), scene.options.spacing.rank + 21.);

    scene.g.inserted_ranks.clear();
    assert_eq!(gap(&scene.draw()), 2. * scene.options.spacing.rank + 21.);
}

#[test]
fn a_separated_curve_uses_its_own_runway_and_keeps_the_marker_lead() {
    let mut gap = gap(&[(0., 20.), (90., 90.)]);
    gap.upper_guard = 22.;
    gap.lower_guard = 25.;
    gap.marker_half_width = 7.;
    gap.allow_local_sweeps = true;
    gap.pieces[0].leads = [0., 12.];
    let upper = Band {
        bottom: 10.,
        ..Band::default()
    };
    let lower = Band {
        top: 80.,
        ..Band::default()
    };
    let options = Options::default();
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let mut points = Vec::new();
    for (occupied, has_end_labels, expected) in [
        (90., false, [10., 68.]),
        (25., false, [32., 55.]),
        (90., true, [32., 55.]),
    ] {
        gap.pieces[1].occupied.from = occupied;
        gap.has_end_labels = has_end_labels;
        points.clear();
        draw_piece(
            &mut points,
            gap.next_piece(),
            &upper,
            &lower,
            &gap,
            &options,
            &mut work,
        )
        .unwrap();
        assert_eq!([points[0].y, points[1].y], expected);
    }
}

#[test]
fn sorted_sweep_emits_exactly_the_brute_force_touching_and_overlapping_pairs() {
    let mut state = 105u32;
    let mut next = || {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        f64::from(state % 40)
    };
    for _ in 0..100 {
        let spans: Vec<_> = (0..40).map(|_| (next(), next())).collect();
        let pieces = gap(&spans).pieces;
        let mut ids: Vec<_> = (0..pieces.len()).collect();
        ids.sort_unstable_by(|&a, &b| {
            pieces[a]
                .span
                .lo()
                .total_cmp(&pieces[b].span.lo())
                .then(a.cmp(&b))
        });
        let mut seen = vec![vec![false; ids.len()]; ids.len()];
        meetings(
            &pieces,
            &ids,
            &mut Work::new(&mut NoopWorkControl),
            |a, b, _| {
                let lo = ids[a].min(ids[b]);
                let hi = ids[a].max(ids[b]);
                assert!(!seen[lo][hi]);
                seen[lo][hi] = true;
                Ok(())
            },
        )
        .unwrap();
        for (i, a) in pieces.iter().enumerate() {
            for (j, b) in pieces.iter().enumerate().skip(i + 1) {
                assert_eq!(
                    seen[i][j],
                    a.span.lo().max(b.span.lo()) <= a.span.hi().min(b.span.hi())
                );
            }
        }
    }
}

#[test]
fn weighted_tracks_obey_crossing_precedence_and_share_mate_or_disjoint_tracks() {
    let mut scene = Scene::new(
        &[10.; 7],
        &[&[0, 1, 2], &[3, 4, 5, 6]],
        &[&[0, 3], &[1, 4], &[2, 5], &[2, 6]],
        &[0., 20., 100., 40., 60., 130., 146.],
    );
    scene.options.routing = RoutingStyle::Orthogonal;
    let drawing = scene.draw();
    let shelves: Vec<_> = drawing.edges.iter().map(|e| level(&e.1.points)).collect();
    assert!(shelves[1] < shelves[0]);
    assert_eq!(shelves[1], shelves[2]);
    assert_eq!(shelves[2], shelves[3]);
}

#[test]
fn weighted_track_order_matches_an_all_pairs_oracle_including_ties_and_cycles() {
    let mut state = 971u32;
    let mut coordinate = || {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        f64::from((state >> 16) % 19)
    };
    for count in (0..40).map(|_| 14).chain([80]) {
        let spans: Vec<_> = (0..count)
            .map(|_| {
                let from = coordinate();
                let to = coordinate();
                (from, if to == from { to + 1. } else { to })
            })
            .collect();
        let mut pieces = gap(&spans).pieces;
        let mut ids: Vec<_> = (0..count).collect();
        ids.sort_unstable_by(|&first, &second| {
            pieces[first]
                .span
                .lo()
                .total_cmp(&pieces[second].span.lo())
                .then(first.cmp(&second))
        });
        weighed_tracks(&mut pieces, &ids, &mut Work::new(&mut NoopWorkControl)).unwrap();
        let spans: Vec<_> = ids.iter().map(|&id| pieces[id].span).collect();
        let precedes = |upper: usize, lower: usize| {
            let upper = spans[upper];
            let lower = spans[lower];
            !upper.mates(lower)
                && upper.lo().max(lower.lo()) <= upper.hi().min(lower.hi())
                && upper.crossings_above(lower) < lower.crossings_above(upper)
        };
        let mut placed = vec![None; count];
        for _ in 0..count {
            let remaining: Vec<_> = (0..count).filter(|&id| placed[id].is_none()).collect();
            let waiting = |id| {
                remaining
                    .iter()
                    .filter(|&&other| precedes(other, id))
                    .count()
            };
            let next = *remaining
                .iter()
                .min_by(|&&first, &&second| {
                    waiting(first)
                        .cmp(&waiting(second))
                        .then_with(|| spans[second].width().total_cmp(&spans[first].width()))
                        .then(first.cmp(&second))
                })
                .unwrap();
            let mut minimum = 0;
            let mut taken = vec![false; count];
            for (other, &track) in placed.iter().enumerate() {
                if let Some(track) = track {
                    if precedes(other, next) {
                        minimum = minimum.max(track + 1);
                    }
                    if spans[other].clashes(spans[next]) {
                        taken[track] = true;
                    }
                }
            }
            placed[next] = Some((minimum..count).find(|&track| !taken[track]).unwrap());
        }
        for (id, expected) in ids.into_iter().zip(placed) {
            assert_eq!(pieces[id].track, expected);
        }
    }
}

#[test]
fn overlapping_nearby_s_curves_get_shelves_and_crossed_wide_curves_get_one_tall_gap() {
    for (ends, tall) in [([60., 62.], false), ([300., -12.], true)] {
        let x = if tall {
            [0., 312., ends[0], ends[1]]
        } else {
            [0., 2., ends[0], ends[1]]
        };
        let scene = Scene::new(&[10.; 4], &[&[0, 1], &[2, 3]], &[&[0, 2], &[1, 3]], &x);
        let drawing = scene.draw();
        let top = drawing.nodes[0].y + drawing.nodes[0].height;
        let bottom = drawing.nodes[2].y;
        for (_, edge) in &drawing.edges {
            let diagonals: Vec<_> = edge
                .points
                .windows(2)
                .filter(|p| p[0].x != p[1].x && p[0].y != p[1].y)
                .collect();
            for p in &diagonals {
                assert!(p[0].y >= top && p[1].y <= bottom);
                assert_eq!(p[1].axis, Axis::Vertical);
            }
            if tall {
                assert_eq!(diagonals.len(), 1);
                assert_eq!(bottom - top, 3.0 * scene.options.spacing.rank);
                assert!(edge.points.iter().all(|p| p.axis == Axis::Vertical));
                assert!(edge.points.iter().all(|p| p.kind == KnotKind::Free));
            } else {
                assert!(diagonals.is_empty());
                assert!(top < level(&edge.points) && level(&edge.points) < bottom);
                assert_eq!(
                    edge.points.iter().map(|p| p.kind).collect::<Vec<_>>(),
                    [
                        KnotKind::GapBoundary,
                        KnotKind::Free,
                        KnotKind::Free,
                        KnotKind::GapBoundary,
                    ]
                );
            }
        }
        if !tall {
            assert_ne!(
                level(&drawing.edges[0].1.points),
                level(&drawing.edges[1].1.points)
            );
        }
    }
    let mut isolated = gap(&[(0., 10_000.), (12_000., 12_002.)]);
    isolated
        .grade(&Options::default(), &mut Work::new(&mut NoopWorkControl))
        .unwrap();
    assert_eq!(isolated.tracks, 0);
    assert_eq!(isolated.height, 50.);
}

#[test]
fn an_isolated_sweep_uses_the_unmarked_face_and_only_the_marked_shaft() {
    let mut scene = Scene::new(&[10.; 2], &[&[0], &[1]], &[&[0, 1]], &[0., 80.]);
    scene.graph.edges[0].end_marker = MarkerMetrics {
        length: 10.,
        width: 12.,
    };
    let drawing = scene.draw();
    let points = &drawing.edges[0].1.points;
    let [start, shaft, target] = points.as_slice() else {
        panic!("expected one sweep and one straight shaft, got {points:?}");
    };
    assert_eq!(start.y, drawing.nodes[0].y + drawing.nodes[0].height);
    assert_eq!(shaft.x, target.x);
    assert_eq!(target.y - shaft.y, 16.);
    assert!(shaft.y - start.y > target.y - shaft.y);
    assert!(points.iter().all(|point| point.kind == KnotKind::Free));

    scene.graph.edges[0].start_label = Some(Size {
        width: 30.,
        height: 8.,
    });
    let drawing = scene.draw();
    let points = &drawing.edges[0].1.points;
    assert!(points[1].y - points[0].y >= 8. + scene.options.spacing.end_label_along);
}

#[test]
fn adjacent_sweeps_in_a_three_rank_dag_keep_their_geometry_and_gain_clearance() {
    let scene = Scene::new(
        &[20.; 4],
        &[&[0], &[1, 2], &[3]],
        &[&[0, 1], &[0, 2], &[1, 3], &[2, 3]],
        &[0., -40., 40., 0.],
    );
    let mut options = Options {
        capture_node_ranks: true,
        ..Options::default()
    };
    let drawing = crate::layout(&scene.graph, &options).unwrap();
    assert_eq!(drawing.node_ranks, [0, 1, 1, 2]);
    assert!(drawing.edges.iter().any(|edge| {
        edge.points
            .windows(2)
            .any(|p| p[0].x != p[1].x && p[0].y != p[1].y)
    }));
    assert!(
        drawing
            .edges
            .iter()
            .all(|edge| edge.corridor_clearance > 2.)
    );

    options.reuse_unchanged_components = true;
    let cached = crate::layout(&scene.graph, &options).unwrap();
    assert_eq!(drawing.nodes, cached.nodes);
    for (certified, fallback) in drawing.edges.iter().zip(cached.edges) {
        assert_eq!(certified.points, fallback.points);
        assert_eq!(certified.ports, fallback.ports);
        assert_eq!(fallback.corridor_clearance, 0.);
    }
}

#[test]
fn adjacent_sweep_uses_the_band_face_beyond_its_shorter_source_box() {
    let mut scene = Scene::new(
        &[10., 2., 60., 10.],
        &[&[0], &[1, 2], &[3]],
        &[&[0, 1], &[0, 2], &[1, 3]],
        &[0., 0., 20., 310.],
    );
    scene.options.spacing.rank = 12.;
    scene.options.compact_ranks = false;
    let drawing = scene.draw();
    let route = &drawing.edges[2].1;
    let [contact, from, to] = route.points.as_slice() else {
        panic!("expected one straight lead and one sweep, got {route:?}");
    };
    let neighbor = drawing.nodes[2];
    let bottom = neighbor.y + neighbor.height;
    assert_eq!(from.y, bottom);
    assert!(bottom - contact.y > 20.);

    // This exact point on the canonical cubic intersects the neighboring box's 2-unit outset.
    let along = to.y - from.y;
    let across = to.x - from.x;
    let x = from.x + across * sweep_x(0.125);
    let y = from.y + along * sweep_y(0.125, sweep_handle_fraction(across, along));
    assert!((neighbor.x..neighbor.x + neighbor.width).contains(&x));
    assert!(bottom < y && y < bottom + 2.);
    assert!(route.corridor_clearance > 0. && route.corridor_clearance < 2.);
}

#[test]
fn a_foreign_band_limits_clearance_even_with_no_neighbor_on_either_route_face() {
    let mut scene = Scene::new(
        &[10.; 3],
        &[&[0], &[1], &[2]],
        &[&[0, 1], &[1, 2]],
        &[0.; 3],
    );
    scene.options.spacing.rank = 0.5;
    scene.options.compact_ranks = false;
    let drawing = scene.draw();
    for (_, route) in drawing.edges {
        assert_eq!(route.points.len(), 2);
        assert!(route.corridor_clearance > 0. && route.corridor_clearance <= 0.5);
    }
}

#[test]
fn compaction_invalidates_moved_shafts_and_retains_the_promised_foreign_clearance() {
    let scene = Scene::new(
        &[10., 10., 80., 10., 10.],
        &[&[0], &[1, 2], &[3, 4]],
        &[&[0, 1], &[0, 2], &[1, 3], &[2, 4]],
        &[0., 0., 100., 0., 160.],
    );
    let mut drawing = scene.draw();
    let before = drawing.nodes[3];
    for edge in [0, 2, 3] {
        assert!(drawing.edges[edge].1.corridor_clearance > 0.);
    }
    compact_ranks(
        &mut drawing,
        &scene.c,
        &scene.g,
        &scene.graph,
        &scene.options,
        &mut Work::new(&mut NoopWorkControl),
    )
    .unwrap();
    assert!(drawing.nodes[3].y < before.y);
    assert_eq!(drawing.edges[2].1.corridor_clearance, 0.);
    let retained = drawing.edges[3].1.corridor_clearance;
    assert!(retained > 0. && retained <= scene.options.spacing.edge);
}

#[test]
fn a_dummy_band_cannot_certify_routes_through_or_along_its_gap() {
    let mut scene = Scene::new(
        &[20.; 5],
        &[&[0], &[1], &[2], &[5, 3], &[4]],
        &[&[0, 1], &[1, 2], &[2, 3], &[3, 4], &[2, 5, 4]],
        &[0., 40., 40., 40., 80., 10.],
    );
    scene.g.keep[5] = false;
    let drawing = scene.draw();
    assert!(
        drawing.edges[..2]
            .iter()
            .all(|(_, route)| route.corridor_clearance > 2.)
    );
    assert!(
        drawing.edges[2..]
            .iter()
            .all(|(_, route)| route.corridor_clearance == 0.)
    );

    let mut omitted = Scene::new(
        &[20.; 3],
        &[&[0], &[3], &[1], &[2]],
        &[&[0, 3, 1], &[1, 2]],
        &[0., 45., 85., 10.],
    );
    omitted.g.keep[3] = false;
    let drawing = omitted.draw();
    assert_eq!(drawing.edges[0].1.corridor_clearance, 0.);
    assert!(drawing.edges[1].1.corridor_clearance > 2.);
}

#[test]
fn labels_in_the_previous_gap_leave_an_unlabelled_sweep_certified() {
    let mut scene = Scene::new(
        &[20.; 3],
        &[&[0], &[1], &[2]],
        &[&[0, 1], &[1, 2]],
        &[0., 0., 0.],
    );
    let size = Size {
        width: 16.,
        height: 8.,
    };
    scene.graph.edges[0].label = Some(size);
    let drawing = crate::layout(&scene.graph, &scene.options).unwrap();
    assert_eq!(drawing.stats.dummy_nodes, 0, "the label must stay inline");
    assert!(drawing.edges[0].label_center.is_some());
    assert_eq!(drawing.edges[0].corridor_clearance, 0.0);
    assert!(drawing.edges[1].corridor_clearance > 2.0);

    scene.graph.edges[0].label = None;
    scene.graph.edges[0].end_label = Some(size);
    let drawing = crate::layout(&scene.graph, &scene.options).unwrap();
    assert!(drawing.edges[0].end_label_center.is_some());
    assert_eq!(drawing.edges[0].corridor_clearance, 0.0);
    assert!(drawing.edges[1].corridor_clearance > 2.0);
}

#[test]
fn a_direct_fallback_box_can_reduce_the_clearance_of_an_earlier_gap() {
    let mut graph = Graph {
        nodes: (0..6)
            .map(|seq| {
                NodeSpec::new(
                    Size {
                        width: 40.0,
                        height: 22.0,
                    },
                    seq,
                )
            })
            .collect(),
        edges: [(0, 1), (1, 3), (0, 2), (2, 4), (3, 5), (4, 5), (0, 5)]
            .into_iter()
            .map(|(a, b)| EdgeSpec::new(NodeId(a).into(), NodeId(b).into()))
            .collect(),
        ..Graph::default()
    };
    let mut options = Options {
        compact_ranks: false,
        ..Options::default()
    };
    options.caps.max_dummies = 0;
    options.spacing.rank = 24.0;
    options.spacing.node = 30.0;
    let before = crate::layout(&graph, &options).unwrap();
    assert!(before.edges[0].corridor_clearance > 2.0);
    let size = Size {
        width: 24.0,
        height: 64.0,
    };
    graph.edges[6].label = Some(size);
    let after = crate::layout(&graph, &options).unwrap();
    assert_eq!(after.stats.direct_fallback_edges, 1);
    let route = &after.edges[0];
    assert_eq!(route.points, before.edges[0].points);
    let [start, .., end] = route.points.as_slice() else {
        panic!("missing contacts")
    };
    let label = after.edges[6].label_center.unwrap();
    let margin = label.y - size.height * 0.5 - end.y;
    assert_eq!(margin, 2.0);
    assert!(start.x.min(end.x) < label.x + size.width * 0.5);
    assert!(label.x - size.width * 0.5 < start.x.max(end.x));
    assert!(route.corridor_clearance > 0.0 && route.corridor_clearance < margin);
}

#[test]
fn a_terminal_label_can_limit_a_different_gap_from_its_terminal_rank() {
    let mut scene = Scene::new(
        &[20.0; 4],
        &[&[0], &[1], &[2], &[3]],
        &[&[0, 1], &[1, 2], &[2, 3], &[0, 3]],
        &[0.0; 4],
    );
    scene.graph.direction = Direction::Right;
    let physical_size = Size {
        width: 10.0,
        height: 24.0,
    };
    let fallback = &mut scene.graph.edges[3];
    fallback.start_label = Some(physical_size);
    fallback.end_label = Some(physical_size);
    for kind in 0..2 {
        let mut drawing = scene.draw();
        assert_eq!(drawing.edges[3].1.corridor_clearance, 0.0);
        let route = &drawing.edges[1].1;
        assert!(route.corridor_clearance > 2.0);
        let [first, .., last] = route.points.as_slice() else {
            panic!("missing contacts")
        };
        // The physical height is the canonical cross extent. This box's near edge
        // clears the shaft by half a pixel, inside the original node-only radius.
        let near = Point {
            x: first.x + 12.5,
            y: first.y.midpoint(last.y),
        };
        let far = Point { x: 300.0, ..near };
        let mut centers = [Some(far); 2];
        centers[kind] = Some(near);
        let edge = &mut drawing.edges[3].1;
        [edge.start_label_center, edge.end_label_center] = centers;
        certify_final_clearance(
            &mut drawing,
            &scene.graph,
            &mut Work::new(&mut NoopWorkControl),
        )
        .unwrap();
        assert!(drawing.edges[0].1.corridor_clearance > 2.0);
        let narrowed = drawing.edges[1].1.corridor_clearance;
        assert!(
            narrowed > 0.0 && narrowed < 0.5,
            "label kind {kind}: {narrowed}"
        );
    }
}

#[test]
fn independently_packed_tiles_and_compound_stitches_defer_clearance() {
    let mut graph = Scene::new(
        &[20.0; 3],
        &[&[0], &[1], &[2]],
        &[&[0, 1], &[1, 2]],
        &[0.0; 3],
    )
    .graph;
    let options = Options::default();
    let flat = crate::layout(&graph, &options).unwrap();
    assert!(flat.edges.iter().all(|edge| edge.corridor_clearance > 2.0));

    graph.nodes.push(NodeSpec::new(graph.nodes[0].size, 3));
    let packed = crate::layout(&graph, &options).unwrap();
    assert_eq!(packed.stats.components, 2);
    assert!(
        packed
            .edges
            .iter()
            .all(|edge| edge.corridor_clearance == 0.0)
    );

    graph.nodes.pop();
    let mut group = crate::GroupSpec::new(0);
    group.title = Some(Size {
        width: 40.0,
        height: 16.0,
    });
    graph.groups.push(group);
    graph.nodes[0].parent = Some(crate::GroupId(0));
    let compound = crate::layout(&graph, &options).unwrap();
    assert!(!compound.border_stitches.is_empty());
    assert!(
        compound
            .edges
            .iter()
            .all(|edge| edge.corridor_clearance == 0.0)
    );
}

#[test]
fn occupied_band_clearance_uses_the_local_sweep_and_falls_back_at_an_obstacle() {
    let mut scene = Scene::new(
        &[10., 10., 20., 20.],
        &[&[0], &[2, 4], &[5, 3], &[1]],
        &[&[0, 4, 5, 1]],
        &[0., 160., 0., 163., 55., 105.],
    );
    scene.options.spacing.node = 12.;
    scene.options.spacing.edge = 4.;
    scene.g.keep[4..].fill(false);
    let offsets = |drawing: &Drawing| {
        drawing.edges[0]
            .1
            .points
            .windows(2)
            .filter(|pair| pair[0].x != pair[1].x)
            .count()
    };
    let clear = scene.draw();
    assert_eq!(offsets(&clear), 1);
    assert_eq!(
        clear.edges[0]
            .1
            .points
            .iter()
            .filter(|p| p.kind == KnotKind::GapBoundary)
            .count(),
        2
    );

    scene.x[2] = 30.;
    let blocked = scene.draw();
    assert!(offsets(&blocked) > 1);
    assert!(
        blocked.edges[0]
            .1
            .points
            .iter()
            .any(|p| p.x == 55. && p.y == blocked.nodes[2].y)
    );
    assert_eq!(clear.edges[0].1.ports, blocked.edges[0].1.ports);
}

#[test]
fn a_merged_sweep_reserves_the_path_it_uses_in_each_gap() {
    let gaps = &mut [gap(&[(0., 40.), (-60., -60.)]), gap(&[(40., 100.)])];
    gaps[0].upper_guard = 12.;
    let bands = [
        Band {
            bottom: 10.,
            ..Band::default()
        },
        Band {
            top: 40.,
            bottom: 60.,
            ..Band::default()
        },
        Band {
            top: 100.,
            ..Band::default()
        },
    ];
    let prove = |gaps: &[Gap], room| {
        prove_sweep(
            gaps,
            &bands,
            &[Some(room)],
            4.,
            &mut Work::new(&mut NoopWorkControl),
        )
        .unwrap()
        .expect("the original sweep fits")
    };

    let open = prove(gaps, (1., 99.));
    assert_eq!(open.sweep.start, 22.);
    assert_eq!(open.steps, 2);
    assert_eq!(open.occupied[0].lo(), 0.);
    assert!(open.occupied[0].hi() < open.occupied[1].lo());
    assert_eq!(open.occupied[1].hi(), 100.);
}

#[test]
fn a_long_departure_guard_leaves_room_for_the_next_sweep_in_both_edge_orders() {
    let a: &[u32] = &[0, 4, 1];
    let b: &[u32] = &[0, 5, 6, 2];
    for (chains, marked_edge) in [([a, b], 1), ([b, a], 0)] {
        let mut scene = Scene::new(
            &[10.; 4],
            &[&[0], &[4, 5, 3], &[1, 6], &[2]],
            &chains,
            &[110., 0., 85., 160., 88., 120., 120.],
        );
        scene.g.width[0] = 40.;
        scene.g.keep[4..].fill(false);
        scene.options.spacing.edge = 6.;
        let first_offset = if marked_edge == 0 { 10. } else { -10. };
        scene.g.source_offset[0] = first_offset;
        scene.g.source_offset[chains[0].len() - 1] = -first_offset;
        scene.graph.edges[marked_edge].start_marker = MarkerMetrics {
            length: 14.,
            width: 4.,
        };
        let drawing = scene.draw();
        for edge in [marked_edge, 1 - marked_edge] {
            let route = &drawing.edges[edge].1;
            assert_eq!(
                route.points[1],
                RoutePoint::gap_boundary(route.points[0].x, 30., Axis::Vertical),
                "the two sweeps can share the gaps with the original guard: {route:?}"
            );
        }
    }
}

#[derive(Default)]
struct Meter {
    used: usize,
    limit: Option<usize>,
}

impl WorkControl for Meter {
    fn check(&mut self) -> Result<(), WorkError> {
        Ok(())
    }
    fn charge(&mut self, units: usize) -> Result<(), WorkError> {
        self.used = self
            .used
            .checked_add(units)
            .ok_or(WorkError::ArithmeticOverflow)?;
        if self.limit.is_some_and(|limit| self.used > limit) {
            return Err(WorkError::Interrupted);
        }
        Ok(())
    }
}

#[test]
fn sparse_sweep_and_dense_fallback_take_subquadratic_work_and_large_fans_share_tracks() {
    let count = 2048;
    for style in [RoutingStyle::Orthogonal, RoutingStyle::Curved] {
        let mut options = Options {
            routing: style,
            ..Options::default()
        };
        for dense in [false, true] {
            options.caps.max_weighed_gap_pieces = if dense { 12 } else { count };
            let mut gap = gap(&(0..count)
                .map(|i| {
                    let x = i as f64;
                    if dense {
                        (x, x + count as f64)
                    } else {
                        (2. * x, 2. * x + 1.)
                    }
                })
                .collect::<Vec<_>>());
            let mut meter = Meter {
                limit: Some(count * 180),
                ..Meter::default()
            };
            let mut work = Work::new(&mut meter);
            gap.grade(&options, &mut work).unwrap();
            work.finish().unwrap();
            assert_eq!(
                gap.tracks,
                if style == RoutingStyle::Curved {
                    0
                } else if dense {
                    count
                } else {
                    1
                }
            );
        }
        let mut fan = gap(&(1..=count).map(|i| (0., i as f64)).collect::<Vec<_>>());
        options.caps.max_weighed_gap_pieces = 0;
        fan.grade(&options, &mut Work::new(&mut NoopWorkControl))
            .unwrap();
        assert_eq!(fan.tracks, usize::from(style == RoutingStyle::Orthogonal));
    }
}

#[test]
fn sparse_logical_distances_survive_without_dense_storage_and_interrupted_routes_retry() {
    let mut scene = Scene::new(
        &[10.; 3],
        &[&[0], &[1], &[2]],
        &[&[0, 1], &[1, 2]],
        &[0., 30., 60.],
    );
    let mut ordinary = Meter::default();
    scene.draw_with(&mut ordinary).unwrap();
    let span = u64::from(u32::MAX);
    scene.g.logical_rank = vec![0, span, 2 * span];
    let mut sparse = Meter::default();
    let expected = scene.draw_with(&mut sparse).unwrap();
    assert_eq!(ordinary.used, sparse.used);
    let gap = scene.options.spacing.rank * span as f64;
    for pair in expected.nodes.windows(2) {
        assert!(pair[1].y - pair[0].y - pair[0].height >= gap);
    }
    for limit in [0, sparse.used / 2, sparse.used - 1] {
        let mut denied = Meter {
            limit: Some(limit),
            ..Meter::default()
        };
        assert!(matches!(
            scene.draw_with(&mut denied),
            Err(LayoutError::Work(WorkError::Interrupted))
        ));
        assert_eq!(scene.draw().edges, expected.edges);
    }
}

#[test]
fn fixed_offset_ports_match_rect_center_without_ulp_drift_in_extras() {
    // Choose x and width such that `(x - w/2) + w/2 != x` by 1 ULP, and an offset where
    // `x + offset != ((x - w/2) + w/2) + offset`. `routing` must anchor on `Rect::center().x`
    // so `extras::finish` keeps the curved route instead of triggering an orthogonal redraw.
    let x_b = 36.04_f64;
    let w_b = 369.5_f64;
    let off_b = -45.25_f64;
    assert_ne!(
        x_b + off_b,
        ((x_b - w_b * 0.5) + w_b * 0.5) + off_b,
        "test coordinates must exhibit the 1-ULP center round-trip discrepancy"
    );

    let mut scene = Scene::new(
        &[100., 100.],
        &[&[0], &[2], &[1]],
        &[&[0, 2, 1]],
        &[0., x_b, 0.],
    );
    scene.g.width[0] = 377.5;
    scene.g.width[1] = w_b;
    scene.label(0, 2, 65.54, 21.0);
    scene.g.inserted_ranks.push(1);
    scene.g.source_offset[0] = 19.25;
    scene.g.target_offset[1] = off_b;
    scene.graph.edges[0].source_port = crate::PortSideHint::FixedOffset {
        side: Side::Bottom,
        offset: 19.25,
    };
    scene.graph.edges[0].target_port = crate::PortSideHint::FixedOffset {
        side: Side::Top,
        offset: off_b,
    };
    scene.c.nodes[0].group_proxy = true;
    scene.c.nodes[1].group_proxy = true;

    let mut drawing = scene.draw();
    let before = drawing.edges[0].1.points.clone();
    super::super::extras::finish(
        &mut drawing,
        &scene.g,
        &scene.c,
        &scene.graph,
        &scene.options,
        &mut Work::new(&mut NoopWorkControl),
    )
    .unwrap();
    assert_eq!(
        drawing.edges[0].1.points, before,
        "1-ULP center round-trip must not trigger an orthogonal redraw in extras::finish"
    );
}

#[test]
fn compound_proxy_gap_does_not_charge_shared_runway_on_adjacent_label_band() {
    let mut scene = Scene::new(
        &[40., 40.],
        &[&[0], &[2, 3], &[1]],
        &[&[0, 2, 1], &[0, 3, 1]],
        &[100., 100., 60., 140.],
    );
    scene.label(0, 2, 30., 20.);
    scene.label(1, 3, 30., 20.);
    scene.g.inserted_ranks.push(1);
    scene.g.source_offset[0] = -40.;
    scene.g.target_offset[1] = -20.;
    scene.g.source_offset[2] = 40.;
    scene.g.target_offset[3] = 20.;
    scene.c.nodes[0].group_proxy = true;
    scene.c.nodes[1].group_proxy = true;

    let drawing = scene.draw();
    let label_bottom = drawing.edges[0].1.label_center.unwrap().y + 10.0;
    let proxy_top = drawing.nodes[1].y;
    let lead = LEAD.max(scene.options.spacing.edge * 0.5);
    // In the lower gap (label -> proxy), the sweep starts right at `label_bottom`
    // (0 guard on the pure label band) and keeps `lead` only at `proxy_top`.
    let pts = &drawing.edges[0].1.points;
    let start_y = pts
        .iter()
        .find(|p| p.x == 60.0 && p.y >= label_bottom - 1e-6)
        .unwrap()
        .y;
    let end_y = pts
        .iter()
        .find(|p| p.x == 80.0 && p.y < proxy_top)
        .unwrap()
        .y;
    assert!(
        (start_y - label_bottom).abs() < 1e-6,
        "start_y={start_y} label_bottom={label_bottom}"
    );
    assert!(
        (proxy_top - end_y - lead).abs() < 1e-6,
        "proxy_top={proxy_top} end_y={end_y} lead={lead}"
    );
}
