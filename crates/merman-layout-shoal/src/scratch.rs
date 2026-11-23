use crate::{LayoutError, work::LayoutResult};

/// Caller-owned traversal buffers. The cap limits the sum of retained vector capacities
/// between calls. A solve can temporarily grow larger, including when the cap is zero.
#[derive(Debug)]
pub struct LayoutScratch {
    max_retained_bytes: usize,
    buffers: Buffers,
}

impl Default for LayoutScratch {
    fn default() -> Self {
        Self::with_max_retained_bytes(2 * 1024 * 1024)
    }
}

impl LayoutScratch {
    pub fn with_max_retained_bytes(max_retained_bytes: usize) -> Self {
        Self {
            max_retained_bytes,
            buffers: Buffers::default(),
        }
    }

    pub fn retained_bytes(&self) -> usize {
        (self.buffers.remaining.capacity() + self.buffers.worklist.capacity()) * size_of::<u32>()
    }

    pub(crate) fn frame(&mut self) -> Frame<'_> {
        Frame {
            buffers: &mut self.buffers,
            limit: self.max_retained_bytes,
        }
    }
}

/// Both lists contain dense counts or handles and can be reused by any flat solve.
#[derive(Debug, Default)]
pub(crate) struct Buffers {
    pub(crate) remaining: Vec<u32>,
    pub(crate) worklist: Vec<u32>,
}

impl Buffers {
    pub(crate) fn prepare(&mut self, nodes: usize) -> LayoutResult<()> {
        for buffer in [&mut self.remaining, &mut self.worklist] {
            buffer.clear();
            buffer
                .try_reserve_exact(nodes)
                .map_err(|_| LayoutError::AllocationLimit)?;
        }
        Ok(())
    }
}

pub(crate) struct Frame<'a> {
    pub(crate) buffers: &'a mut Buffers,
    limit: usize,
}

impl Drop for Frame<'_> {
    fn drop(&mut self) {
        let mut available = self.limit / size_of::<u32>();
        for buffer in [&mut self.buffers.remaining, &mut self.buffers.worklist] {
            buffer.clear();
            if buffer.capacity() <= available {
                available -= buffer.capacity();
            } else {
                *buffer = Vec::new();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Graph, NodeSpec, NoopWorkControl, Options, Size, layout_with_control};

    #[test]
    fn successive_solves_keep_the_actual_traversal_storage() {
        let mut graph = Graph {
            nodes: (0..32)
                .map(|seq| NodeSpec::new(Size::default(), seq))
                .collect(),
            ..Graph::default()
        };
        let mut scratch = LayoutScratch::default();
        layout_with_control(
            &graph,
            &Options::default(),
            &mut scratch,
            &mut NoopWorkControl,
        )
        .unwrap();
        let first = [
            scratch.buffers.remaining.as_ptr(),
            scratch.buffers.worklist.as_ptr(),
        ];
        graph.nodes.truncate(8);
        layout_with_control(
            &graph,
            &Options::default(),
            &mut scratch,
            &mut NoopWorkControl,
        )
        .unwrap();
        assert_eq!(
            first,
            [
                scratch.buffers.remaining.as_ptr(),
                scratch.buffers.worklist.as_ptr()
            ]
        );
        assert!(scratch.buffers.remaining.is_empty() && scratch.buffers.worklist.is_empty());
    }
}
