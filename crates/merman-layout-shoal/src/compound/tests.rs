use super::*;
use crate::{EdgeSpec, GroupId, GroupSpec, NodeId, NodeSpec, Size, WorkControl};

#[derive(Default)]
struct Meter(usize);

impl WorkControl for Meter {
    fn check(&mut self) -> Result<(), WorkError> {
        Ok(())
    }

    fn charge(&mut self, units: usize) -> Result<(), WorkError> {
        self.0 += units;
        Ok(())
    }
}

#[test]
fn recomputing_an_edited_level_matches_a_fresh_solve_and_reuses_unchanged_levels() {
    let mut graph = Graph::default();
    graph.groups.extend([GroupSpec::new(0), GroupSpec::new(1)]);
    for index in 0..6 {
        let mut node = NodeSpec::new(
            Size {
                width: 48.,
                height: 26.,
            },
            index,
        );
        node.parent = Some(GroupId(index / 3));
        graph.nodes.push(node);
    }
    for (from, to) in [(0, 1), (0, 2), (3, 4), (3, 5)] {
        graph
            .edges
            .push(EdgeSpec::new(NodeId(from).into(), NodeId(to).into()));
    }
    let mut meter = Meter::default();
    let mut work = Work::new(&mut meter);
    let options = Options::default();
    let hierarchy = hierarchy::build(&graph, &options, &mut work).unwrap();
    let keys = vec![None; graph.nodes.len() + graph.groups.len()];
    let first = Pass {
        graph: &graph,
        hierarchy: &hierarchy,
        options: &options,
        sides: &[],
        items: &keys,
        ports: &[],
    };
    let mut updated = keys.clone();
    updated[1] = Some(80.);
    updated[2] = Some(-80.);
    let edited = Pass {
        items: &updated,
        ..first
    };
    let mut frames = Frames::new(hierarchy.levels.len()).unwrap();
    let mut scratch = Expanded::default();
    frames
        .draw(&first, None, &mut scratch, Some(u64::MAX), &mut work)
        .unwrap();
    let measure = |frames: &mut Frames, previous: Option<&Pass<'_>>| {
        let mut meter = Meter::default();
        let mut work = Work::new(&mut meter);
        let result = frames
            .draw(
                &edited,
                previous,
                &mut Expanded::default(),
                Some(u64::MAX),
                &mut work,
            )
            .unwrap();
        work.finish().unwrap();
        (result, meter.0)
    };
    let (candidate, changed_work) = measure(&mut frames, Some(&first));
    let (repeated, unchanged_work) = measure(&mut frames, Some(&edited));
    let (reference, cold_work) = measure(&mut Frames::new(hierarchy.levels.len()).unwrap(), None);
    assert_eq!(candidate, reference);
    assert_eq!(repeated, reference);
    assert!(unchanged_work < changed_work && changed_work < cold_work);
}

#[test]
fn geometry_omits_crossing_work_when_compound_ports_cannot_be_reordered() {
    let mut graph = Graph::default();
    graph.groups.push(GroupSpec::new(0));
    for index in 0..16 {
        let mut node = NodeSpec::new(
            Size {
                width: 48.,
                height: 26.,
            },
            index,
        );
        node.parent = (index < 8).then_some(GroupId(0));
        graph.nodes.push(node);
    }
    for index in 0..8 {
        for target in [8 + index, 15 - index] {
            graph
                .edges
                .push(EdgeSpec::new(NodeId(index).into(), NodeId(target).into()));
        }
    }
    let mut options = Options::default();
    options.caps.max_ordered_compound_ports = 0;
    let mut full_work = Meter::default();
    let full =
        crate::layout_with_control(&graph, &options, &mut Default::default(), &mut full_work)
            .unwrap();
    let mut geometry_work = Meter::default();
    let geometry = crate::layout_geometry_with_control(
        &graph,
        &options,
        &mut Default::default(),
        &mut geometry_work,
    )
    .unwrap();
    assert!(!full.border_stitches.is_empty());
    assert_eq!(geometry, crate::LayoutGeometry::from(full));
    assert!(geometry_work.0 < full_work.0);
}

#[test]
fn a_nested_crossing_exposed_by_the_first_port_retry_is_rechecked() {
    let mut graph = Graph {
        groups: (0..3).map(GroupSpec::new).collect(),
        ..Graph::default()
    };
    graph.groups[0].parent = Some(GroupId(1));
    for (id, (parent, width, height)) in [
        (Some(0), 127., 45.),
        (Some(0), 109., 45.),
        (Some(0), 107., 56.),
        (Some(1), 75., 37.),
        (Some(1), 45., 60.),
        (Some(2), 139., 61.),
        (Some(2), 46., 66.),
        (None, 93., 61.),
        (None, 102., 27.),
    ]
    .into_iter()
    .enumerate()
    {
        let mut node = NodeSpec::new(Size { width, height }, id as u32);
        node.parent = parent.map(GroupId);
        graph.nodes.push(node);
    }
    graph.edges = [
        (5, 8),
        (5, 6),
        (7, 1),
        (8, 4),
        (7, 0),
        (1, 4),
        (1, 7),
        (3, 7),
        (3, 4),
        (0, 7),
        (6, 3),
        (8, 7),
        (0, 3),
        (6, 8),
        (4, 6),
    ]
    .into_iter()
    .map(|(from, to)| EdgeSpec::new(NodeId(from).into(), NodeId(to).into()))
    .collect();
    let options = Options::default();
    let drawing = crate::layout(&graph, &options).unwrap();
    assert_eq!(drawing.stats.routed_crossings, 1);
    assert_eq!(drawing, crate::layout(&graph, &options).unwrap());
}

#[test]
fn opposing_routes_follow_the_inner_order_at_a_shared_seam() {
    use crate::{Direction, Insets, MarkerMetrics, TitlePosition};

    let mut graph = Graph::default();
    for (id, (parent, direction, width)) in [
        (Some(1), None, 219.4),
        (Some(2), Some(Direction::Left), 204.7),
        (Some(3), Some(Direction::Up), 220.1),
        (None, Some(Direction::Right), 258.0),
    ]
    .into_iter()
    .enumerate()
    {
        graph.groups.push(GroupSpec {
            parent: parent.map(GroupId),
            direction,
            padding: Insets {
                top: 33.,
                right: 8.,
                bottom: 8.,
                left: 8.,
            },
            title: Some(Size { width, height: 17. }),
            title_position: TitlePosition::TopAuto,
            ..GroupSpec::new(id as u32)
        });
    }
    for (id, parent) in [
        None,
        Some(3),
        Some(2),
        Some(1),
        Some(0),
        Some(0),
        Some(1),
        Some(2),
        Some(3),
    ]
    .into_iter()
    .enumerate()
    {
        let height = if matches!(id, 0 | 7) { 56.4 } else { 41. };
        let mut node = NodeSpec::new(
            Size {
                width: 152.,
                height,
            },
            id as u32,
        );
        node.parent = parent.map(GroupId);
        node.port_span = Some(136.);
        node.perpendicular_port_span = Some(height - 16.);
        graph.nodes.push(node);
    }
    let marked = |mut edge: EdgeSpec, label_width: Option<f64>| {
        edge.label = label_width.map(|width| Size { width, height: 21. });
        edge.end_marker = MarkerMetrics {
            length: 16.62,
            width: 14.,
        };
        edge.set_marker_runways([0., 16.62]);
        edge
    };
    for (from, to, label_width) in [
        (0, 1, None),
        (1, 2, None),
        (2, 3, None),
        (3, 4, Some(38.2)),
        (4, 5, None),
        (5, 6, Some(36.6)),
        (6, 7, None),
        (7, 8, None),
        (8, 0, None),
    ] {
        graph.edges.push(marked(
            EdgeSpec::new(NodeId(from).into(), NodeId(to).into()),
            label_width,
        ));
    }
    graph.edges.extend([
        marked(EdgeSpec::new(GroupId(0).into(), NodeId(7).into()), None),
        marked(
            EdgeSpec::new(GroupId(2).into(), NodeId(0).into()),
            Some(102.8),
        ),
        marked(
            EdgeSpec::new(NodeId(5).into(), NodeId(1).into()),
            Some(123.7),
        ),
    ]);

    let options = Options::default();
    let drawing = crate::layout(&graph, &options).unwrap();
    assert_eq!(drawing.stats.routed_crossings, 0);
    assert!(drawing.groups.iter().all(|group| !group.title_struck));
    let seam_x = |edge| {
        let seam = drawing
            .border_stitches
            .iter()
            .find(|stitch| {
                stitch.group == GroupId(3) && stitch.side == Side::Top && stitch.edge.0 == edge
            })
            .unwrap();
        drawing.edges[edge as usize].points[seam.point].x
    };
    assert!(seam_x(0) - seam_x(10) >= options.spacing.min_port_pitch);
    assert!(seam_x(8) - seam_x(0) >= options.spacing.min_port_pitch);
}
