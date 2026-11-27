use std::num::NonZeroUsize;
use std::ops::Range;

#[derive(Debug, Clone, Copy)]
enum Item {
    Node(usize),
    Subgraph(NonZeroUsize),
}

#[derive(Debug)]
pub(super) struct SubgraphOrder {
    items: Range<usize>,
    len: usize,
    pub(super) barycenter: Option<f64>,
    pub(super) weight: Option<f64>,
}

#[derive(Debug, Default)]
pub(super) struct SortAssembly {
    subgraph_by_local: Vec<Option<NonZeroUsize>>,
    subgraphs: Vec<SubgraphOrder>,
    items: Vec<Item>,
    pending: Vec<Range<usize>>,
}

impl SortAssembly {
    pub(super) fn begin_rank(&mut self, local_count: usize) {
        self.subgraph_by_local.clear();
        self.subgraph_by_local.resize(local_count, None);
        self.subgraphs.clear();
        self.items.clear();
    }

    pub(super) fn subgraph(&self, local_ix: usize) -> Option<&SubgraphOrder> {
        let index = self.subgraph_by_local[local_ix]?;
        Some(&self.subgraphs[index.get() - 1])
    }

    pub(super) fn item_count(&self) -> usize {
        self.items.len()
    }

    pub(super) fn append_node(&mut self, local_ix: usize) {
        self.items.push(Item::Node(local_ix));
    }

    pub(super) fn append_members(&mut self, members: &[usize]) -> usize {
        let mut len = 0;
        for &local_ix in members {
            if let Some(index) = self.subgraph_by_local[local_ix] {
                self.items.push(Item::Subgraph(index));
                len += self.subgraphs[index.get() - 1].len;
            } else {
                self.append_node(local_ix);
                len += 1;
            }
        }
        len
    }

    pub(super) fn finish_subgraph(
        &mut self,
        local_ix: usize,
        start: usize,
        len: usize,
        barycenter: Option<f64>,
        weight: Option<f64>,
    ) {
        self.subgraphs.push(SubgraphOrder {
            items: start..self.items.len(),
            len,
            barycenter,
            weight,
        });
        self.subgraph_by_local[local_ix] = NonZeroUsize::new(self.subgraphs.len());
    }

    pub(super) fn flatten(&mut self, root: usize) -> Vec<usize> {
        let Some(index) = self.subgraph_by_local[root] else {
            return Vec::new();
        };
        let root = &self.subgraphs[index.get() - 1];
        let mut out = Vec::with_capacity(root.len);
        self.pending.clear();
        self.pending.push(root.items.clone());
        while let Some(range) = self.pending.last_mut() {
            let next = range.next();
            if range.start == range.end {
                self.pending.pop();
            }
            if let Some(index) = next {
                match self.items[index] {
                    Item::Node(local_ix) => out.push(local_ix),
                    Item::Subgraph(index) => {
                        self.pending
                            .push(self.subgraphs[index.get() - 1].items.clone());
                    }
                }
            }
        }
        out
    }
}
