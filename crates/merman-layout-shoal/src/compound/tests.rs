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
    frames.draw(&first, None, &mut scratch, &mut work).unwrap();
    let measure = |frames: &mut Frames, previous: Option<&Pass<'_>>| {
        let mut meter = Meter::default();
        let mut work = Work::new(&mut meter);
        let result = frames
            .draw(&edited, previous, &mut Expanded::default(), &mut work)
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
