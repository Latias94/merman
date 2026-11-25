use merman_layout_shoal::LayoutScratch;
use std::sync::Mutex;

const MAX_IDLE_ROW_BYTES: usize = 64 * 1024;

/// One idle workspace for independent, flat Shoal solves.
#[derive(Default)]
pub(crate) struct ShoalScratchPool {
    idle: Mutex<Option<LayoutScratch>>,
}

impl ShoalScratchPool {
    pub(crate) fn solve<T, E>(
        &self,
        solve: impl FnOnce(&mut LayoutScratch) -> Result<T, E>,
    ) -> Result<T, E> {
        // The lock only transfers ownership. A parallel or reentrant solve can start cold.
        let mut scratch = self
            .idle
            .try_lock()
            .ok()
            .and_then(|mut idle| idle.take())
            .unwrap_or_else(|| LayoutScratch::with_max_retained_row_bytes(MAX_IDLE_ROW_BYTES));
        let output = solve(&mut scratch)?;
        if scratch.retained_bytes() != 0
            && let Ok(mut idle) = self.idle.try_lock()
            && idle.is_none()
        {
            *idle = Some(scratch);
        }
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_layout_shoal::{
        EdgeSpec, Graph, LayoutError, NodeId, NodeSpec, NoopWorkControl, Options, Size,
        WorkControl, WorkError, layout_geometry_with_control,
    };

    #[test]
    fn independent_solves_can_overlap_and_cancelled_storage_is_dropped() {
        let graph = |fan| {
            let mut graph = Graph::default();
            graph.nodes.extend((0..9).map(|index| {
                NodeSpec::new(
                    Size {
                        width: 24.0 + f64::from(index),
                        height: 16.0,
                    },
                    index,
                )
            }));
            graph.edges.extend((1..9).map(|index| {
                EdgeSpec::new(
                    NodeId(if fan { 0 } else { index - 1 }).into(),
                    NodeId(index).into(),
                )
            }));
            graph
        };
        let options = Options::default();
        let first = graph(false);
        let second = graph(true);
        let solve = |graph: &Graph, scratch: &mut LayoutScratch| {
            layout_geometry_with_control(graph, &options, scratch, &mut NoopWorkControl)
        };
        let mut fresh = LayoutScratch::with_max_retained_bytes(0);
        let expected = solve(&first, &mut fresh).unwrap();
        let expected_second = solve(&second, &mut fresh).unwrap();
        let pool = ShoalScratchPool::default();
        assert_eq!(
            pool.solve(|scratch| solve(&first, scratch)).unwrap(),
            expected
        );
        let bytes = pool
            .idle
            .try_lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .retained_bytes();
        assert!((1..=MAX_IDLE_ROW_BYTES).contains(&bytes));
        let overlapping = pool.solve(|scratch| {
            assert_eq!(scratch.retained_bytes(), bytes);
            assert!(pool.idle.try_lock().unwrap().is_none());
            assert_eq!(
                pool.solve(|other_scratch| {
                    assert_eq!(other_scratch.retained_bytes(), 0);
                    solve(&second, other_scratch)
                })?,
                expected_second,
            );
            solve(&first, scratch)
        });
        assert_eq!(overlapping.unwrap(), expected);

        struct CancelAfterDebits(usize);

        impl WorkControl for CancelAfterDebits {
            fn check(&mut self) -> Result<(), WorkError> {
                if self.0 == 0 {
                    return Err(WorkError::Interrupted);
                }
                Ok(())
            }

            fn charge(&mut self, _units: usize) -> Result<(), WorkError> {
                self.0 -= 1;
                Ok(())
            }
        }

        let mut control = CancelAfterDebits(20);
        assert_eq!(
            pool.solve(|scratch| {
                assert!(scratch.retained_bytes() > 0);
                layout_geometry_with_control(&first, &options, scratch, &mut control)
            }),
            Err(LayoutError::Work(WorkError::Interrupted)),
        );
        assert!(pool.idle.try_lock().unwrap().is_none());
        assert_eq!(
            pool.solve(|scratch| solve(&second, scratch)).unwrap(),
            expected_second,
        );
    }
}
