//! Component-local topology and geometry in the canonical top-to-bottom coordinate space.

mod acyclic;
mod expand;
mod order;
mod rank;
mod validate;

use crate::work::{LayoutResult, Work, filled};
use crate::{Direction, Graph, LayoutError, NodeSpec, Options, Size, WorkError};

/// Orientation is independent of the edge's semantic direction and input occurrence.
#[derive(Clone, Copy, Debug)]
struct Link {
    from: u32,
    to: u32,
    edge: u32,
    span: u32,
    weight: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    Node,
    Dummy,
    Label,
}

#[derive(Debug, Default)]
struct Csr {
    start: Vec<u32>,
    entries: Vec<u32>,
}

impl Csr {
    fn new(
        n: usize,
        endpoints: impl Iterator<Item = u32> + Clone,
        work: &mut Work<'_>,
    ) -> LayoutResult<Self> {
        let mut result = Self::default();
        result.rebuild(n, endpoints, work)?;
        Ok(result)
    }

    fn rebuild(
        &mut self,
        n: usize,
        endpoints: impl Iterator<Item = u32> + Clone,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        let count = n.checked_add(1).ok_or(WorkError::ArithmeticOverflow)?;
        work.charge(count)?;
        self.start.clear();
        self.start
            .try_reserve_exact(count)
            .map_err(|_| LayoutError::AllocationLimit)?;
        self.start.resize(count, 0);
        for endpoint in endpoints.clone() {
            work.visit(1)?;
            let endpoint = endpoint as usize;
            if endpoint >= n {
                return Err(LayoutError::InvalidInput("invalid adjacency endpoint"));
            }
            let bin = &mut self.start[endpoint + 1];
            *bin = bin.checked_add(1).ok_or(WorkError::ArithmeticOverflow)?;
        }
        for i in 1..count {
            self.start[i] = self.start[i]
                .checked_add(self.start[i - 1])
                .ok_or(WorkError::ArithmeticOverflow)?;
        }
        let len = self.start[n] as usize;
        work.charge(len)?;
        self.entries.clear();
        self.entries
            .try_reserve_exact(len)
            .map_err(|_| LayoutError::AllocationLimit)?;
        self.entries.resize(len, 0);
        // The starts double as fill cursors; restore the prefix array in place afterwards.
        for (index, endpoint) in endpoints.enumerate() {
            let cursor = &mut self.start[endpoint as usize];
            self.entries[*cursor as usize] = index as u32;
            *cursor += 1;
        }
        self.start.copy_within(..n, 1);
        self.start[0] = 0;
        Ok(())
    }

    fn at(&self, node: u32) -> &[u32] {
        let i = node as usize;
        &self.entries[self.start[i] as usize..self.start[i + 1] as usize]
    }
}

#[derive(Debug, Default)]
pub(crate) struct Expanded {
    rank: Vec<u32>,
    logical_rank: Vec<u64>,
    layers: Vec<Vec<u32>>,
    width: Vec<f64>,
    height: Vec<f64>,
    kind: Vec<Kind>,
    from: Vec<u32>,
    to: Vec<u32>,
    weight: Vec<f32>,
    source_offset: Vec<f64>,
    target_offset: Vec<f64>,
    chains: Vec<Vec<u32>>,
    label_vertex: Vec<Option<u32>>,
    direct: Vec<bool>,
    pred: Csr,
    succ: Csr,
}

struct Loop {
    edge: u32,
    node: u32,
    right: bool,
    /// Outer edge of this loop's reserved band, measured from the true node face.
    reach: f64,
}

#[derive(Default)]
struct Component {
    ids: Vec<u32>,
    nodes: Vec<NodeSpec>,
    links: Vec<Link>,
    loops: Vec<Loop>,
    margin_left: Vec<f64>,
    margin_right: Vec<f64>,
}

fn canonical_size(dir: Direction, size: Size) -> Size {
    match dir {
        Direction::Down | Direction::Up => size,
        Direction::Right | Direction::Left => Size {
            width: size.height,
            height: size.width,
        },
    }
}

#[cfg(test)]
mod tests;

/// Footprint of owned allocations, including the individual paths and ranks.
impl Expanded {
    pub(super) fn retained_bytes(&self) -> usize {
        fn bytes<T>(v: &Vec<T>) -> usize {
            v.capacity() * size_of::<T>()
        }
        let nested = |v: &Vec<Vec<u32>>| bytes(v) + v.iter().map(bytes).sum::<usize>();
        let csr = |c: &Csr| bytes(&c.start) + bytes(&c.entries);
        bytes(&self.rank)
            + bytes(&self.logical_rank)
            + nested(&self.layers)
            + bytes(&self.width)
            + bytes(&self.height)
            + bytes(&self.kind)
            + bytes(&self.from)
            + bytes(&self.to)
            + bytes(&self.weight)
            + bytes(&self.source_offset)
            + bytes(&self.target_offset)
            + nested(&self.chains)
            + bytes(&self.label_vertex)
            + bytes(&self.direct)
            + csr(&self.pred)
            + csr(&self.succ)
    }
}

fn prepare_component(
    c: &Component,
    graph: &Graph,
    options: &Options,
    work: &mut Work<'_>,
    expanded: &mut Expanded,
) -> LayoutResult<(acyclic::Orientation, u64)> {
    let orientation = acyclic::orient(&c.nodes, &c.links, options.cycle_policy, work)?;
    expand::build(
        &c.nodes,
        &c.links,
        &orientation,
        graph,
        options,
        work,
        expanded,
    )?;
    let crossings = order::reduce(expanded, &c.nodes, options, work)?;
    Ok((orientation, crossings))
}
