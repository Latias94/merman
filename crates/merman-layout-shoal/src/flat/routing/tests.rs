use super::*;
use crate::flat::Link;
use crate::{EdgeSpec, NodeId, NodeSpec, NoopWorkControl, WorkControl};

const STYLES: [RoutingStyle; 3] = [
    RoutingStyle::Curved,
    RoutingStyle::Orthogonal,
    RoutingStyle::Polyline,
];

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
                    assert!(pair[1].y < top || pair[0].y > bottom, "{style:?} {pair:?}");
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
            .map(|&(from, to)| Piece {
                span: Span { from, to },
                lower: 0,
                track: None,
            })
            .collect(),
        ..Gap::default()
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
                assert!(p[0].y > top && p[1].y < bottom);
                assert_eq!(p[1].axis, Axis::Vertical);
            }
            if tall {
                assert_eq!(diagonals.len(), 1);
                assert_eq!(bottom - top, 150.);
                assert!(edge.points.iter().all(|p| p.axis == Axis::Vertical));
            } else {
                assert_eq!(diagonals.len(), 2);
                assert!(top < level(&edge.points) && level(&edge.points) < bottom);
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
                if dense {
                    count
                } else {
                    usize::from(style == RoutingStyle::Orthogonal)
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
