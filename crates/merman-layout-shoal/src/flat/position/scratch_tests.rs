use super::*;
use crate::{EdgeSpec, LayoutScratch, NodeId, Size, WorkControl, layout_with_control};

fn layered(width: u32) -> Graph {
    Graph {
        nodes: (0..2 * width)
            .map(|seq| {
                NodeSpec::new(
                    Size {
                        width: 60.0,
                        height: 24.0,
                    },
                    seq,
                )
            })
            .collect(),
        edges: (0..width)
            .flat_map(|a| {
                (width..2 * width).map(move |b| EdgeSpec::new(NodeId(a).into(), NodeId(b).into()))
            })
            .collect(),
        ..Graph::default()
    }
}

#[derive(Default)]
struct Meter {
    checks: usize,
    reject_at: usize,
    units: usize,
}

impl WorkControl for Meter {
    fn check(&mut self) -> Result<(), WorkError> {
        self.checks += 1;
        if self.checks == self.reject_at {
            return Err(WorkError::Interrupted);
        }
        Ok(())
    }

    fn charge(&mut self, units: usize) -> Result<(), WorkError> {
        self.units += units;
        Ok(())
    }
}

fn coordinate_buffers(scratch: &mut LayoutScratch) -> [*const u32; 3] {
    let frame = scratch.frame();
    let position = &frame.expanded.position_scratch;
    assert!(!position.lists.pred.entries.is_empty());
    assert!(!position.compaction.outgoing.entries.is_empty());
    assert!(!position.blocks.root.is_empty());
    [
        position.lists.pred.entries.as_ptr(),
        position.compaction.outgoing.entries.as_ptr(),
        position.blocks.root.as_ptr(),
    ]
}

#[test]
fn scratch_keeps_search_and_coordinate_capacity_after_a_late_failure() {
    let graph = layered(4);
    let options = Options::default();
    let mut cold = Meter::default();
    let mut full = LayoutScratch::default();
    let expected = layout_with_control(&graph, &options, &mut full, &mut cold).unwrap();
    let retained = full.retained_bytes();
    assert!(expected.stats.topological_crossings > 0);
    assert!(full.frame().expanded.order_scratch.retained_bytes() > 0);

    for limit in [retained - 1, retained] {
        let mut scratch = LayoutScratch::with_max_retained_bytes(limit);
        let mut cancelled = Meter {
            reject_at: cold.checks,
            ..Meter::default()
        };
        assert_eq!(
            layout_with_control(&graph, &options, &mut scratch, &mut cancelled),
            Err(LayoutError::Work(WorkError::Interrupted))
        );
        assert_eq!(cancelled.units, cold.units);
        let should_retain = limit == retained;
        assert_eq!(
            scratch.retained_bytes(),
            if should_retain { retained } else { 0 }
        );
        let retained_buffers = should_retain.then(|| coordinate_buffers(&mut scratch));

        // Leave a different set of CSR ranges and port roles in the same scratch before retrying.
        let smaller = layered(2);
        let result = layout_with_control(&smaller, &options, &mut scratch, &mut Meter::default());
        assert_eq!(result, crate::layout(&smaller, &options));
        let mut retry = Meter::default();
        assert_eq!(
            layout_with_control(&graph, &options, &mut scratch, &mut retry).unwrap(),
            expected
        );
        assert_eq!((retry.units, retry.checks), (cold.units, cold.checks));
        assert!(scratch.retained_bytes() <= limit);
        if let Some(buffers) = retained_buffers {
            assert_eq!(coordinate_buffers(&mut scratch), buffers);
        }
    }
}
