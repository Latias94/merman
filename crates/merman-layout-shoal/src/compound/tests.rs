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
