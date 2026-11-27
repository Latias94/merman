//! Put lanes on one rank grid while preserving their authored container hierarchy.

use super::*;
use kurbo::Rect;
use merman_layout_shoal::PortSideHint;

struct Header {
    size: Size,
    padding: f64,
}

#[derive(Clone, Copy, Default)]
struct ColumnMinimum {
    span: f64,
    columns: usize,
    extra_per_column: f64,
}

pub(super) struct Columns {
    pub(super) order: Vec<u32>,
    pub(super) boundaries: Vec<u32>,
    parents: Vec<Option<u32>>,
    on_grid: Vec<bool>,
    expanded: Vec<bool>,
    headers: Vec<Option<Header>>,
    flow: Direction,
}

impl Columns {
    pub(super) fn new(
        slots: &[usize],
        working: &WorkingLayout,
        parent: impl Fn(Option<&str>) -> Result<Option<u32>>,
        authored_order: Vec<usize>,
        flow: Direction,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<Self> {
        work.charge_adapter(slots.len())?;
        let parents: Vec<_> = slots
            .iter()
            .map(|&slot| parent(working.nodes[slot].parent_id.as_deref()))
            .collect::<Result<_>>()?;
        let mut on_grid = vec![false; slots.len()];
        for (id, &slot) in slots.iter().enumerate() {
            work.charge_adapter(1)?;
            let node = &working.nodes[slot];
            if shell_kind(node).is_some() {
                let mut group = Some(id);
                while let Some(id) = group.filter(|&id| !on_grid[id]) {
                    work.charge_adapter(1)?;
                    on_grid[id] = true;
                    group = parents[id].map(|id| id as usize);
                }
            }
        }

        let root = slots.len();
        let mut children = vec![Vec::new(); root + 1];
        let mut count = 0;
        for (id, (&column, &parent)) in on_grid.iter().zip(&parents).enumerate() {
            work.charge_adapter(1)?;
            if column {
                children[parent.map_or(root, |id| id as usize)].push(id);
                count += 1;
            }
        }

        let mut priority: Vec<_> = authored_order.into_iter().zip(0..root).collect();
        let mut breadth_first = children[root].clone();
        let mut cursor = 0;
        while let Some(&id) = breadth_first.get(cursor) {
            work.charge_adapter(children[id].len() + 1)?;
            breadth_first.extend_from_slice(&children[id]);
            cursor += 1;
        }
        if breadth_first.len() != count {
            return Err(Error::InvalidModel {
                message: "Cyclic Swimlane pool membership".to_owned(),
            });
        }
        for &id in breadth_first.iter().rev() {
            work.charge_adapter(1)?;
            if let Some(parent) = parents[id] {
                let parent = parent as usize;
                priority[parent] = priority[parent].min(priority[id]);
            }
        }
        for siblings in &mut children {
            let levels = siblings.len().checked_ilog2().unwrap_or(0) as usize + 1;
            work.charge_adapter(work.checked_mul(siblings.len(), levels)?)?;
            siblings.sort_unstable_by_key(|&id| (priority[id], id));
        }

        // Keep each shell's columns contiguous, with its own slot on the outside face.
        // An empty parent can keep that slot at zero width for authored edges to its face.
        let mut stack: Vec<_> = children[root].iter().rev().copied().collect();
        let mut order = Vec::with_capacity(count);
        while let Some(id) = stack.pop() {
            work.charge_adapter(children[id].len() + 1)?;
            order.push(index(id, work)?);
            stack.extend(children[id].iter().rev().copied());
        }
        let expanded = children[..root]
            .iter()
            .map(|children| !children.is_empty())
            .collect();
        Ok(Self {
            order,
            boundaries: Vec::new(),
            parents,
            on_grid,
            expanded,
            headers: (0..root).map(|_| None).collect(),
            flow,
        })
    }

    pub(super) fn is_column(&self, id: usize) -> bool {
        self.on_grid[id]
    }

    pub(super) fn solver_parent(&self, id: usize) -> Option<u32> {
        self.parents[id].filter(|_| !self.is_column(id))
    }

    fn horizontal(&self) -> bool {
        matches!(self.flow, Direction::Left | Direction::Right)
    }

    pub(super) fn reserve_shell_titles(
        &mut self,
        graph: &mut MeasuredGraph,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<()> {
        if !self.expanded.contains(&true) {
            return Ok(());
        }
        work.charge_adapter(graph.nodes.len())?;
        work.charge_adapter(graph.groups.len())?;
        let mut has_content = vec![false; graph.groups.len()];
        for parent in graph
            .nodes
            .iter()
            .filter_map(|node| node.parent)
            .chain(graph.groups.iter().filter_map(|group| group.parent))
        {
            has_content[parent as usize] = true;
        }
        let horizontal = self.horizontal();
        let cross = |size: Size| if horizontal { size.height } else { size.width };
        let cross_size = |span| {
            if horizontal {
                Size {
                    width: 0.0,
                    height: span,
                }
            } else {
                Size {
                    width: span,
                    height: 0.0,
                }
            }
        };
        let mut minima = vec![ColumnMinimum::default(); graph.groups.len()];
        for &id in self.order.iter().rev() {
            work.charge_adapter(1)?;
            let id = id as usize;
            let group = &mut graph.groups[id];
            let minimum = &mut minima[id];
            if self.expanded[id] {
                let padding = group.padding.x0;
                if let Some(size) = group.title.take() {
                    self.headers[id] = Some(Header { size, padding });
                }
                let title_span = self.headers[id]
                    .as_ref()
                    .map_or(0.0, |header| cross(header.size));
                let deficit = (title_span + 2.0 * padding - minimum.span).max(0.0);
                if has_content[id] {
                    group.min_size = cross_size(deficit.max(1.0));
                } else {
                    self.boundaries.push(id as u32);
                    group.padding = kurbo::Insets::ZERO;
                    group.min_size = Size::default();
                    minimum.span += deficit;
                    minimum.extra_per_column = deficit / minimum.columns as f64;
                }
            }
            if !self.expanded[id] || has_content[id] {
                minimum.span += cross(group.min_size).max(1.0);
                minimum.columns += 1;
            }
            if !minimum.span.is_finite() {
                return Err(work.record_arithmetic_overflow().into());
            }
            if let Some(parent) = self.parents[id] {
                let child = *minimum;
                minima[parent as usize].span += child.span;
                minima[parent as usize].columns += child.columns;
            }
        }

        // A nested shell passes down its own deficit together with its ancestors' shares.
        for &id in &self.order {
            work.charge_adapter(1)?;
            let id = id as usize;
            let extra =
                self.parents[id].map_or(0.0, |parent| minima[parent as usize].extra_per_column);
            minima[id].extra_per_column += extra;
            if !self.expanded[id] || has_content[id] {
                let size = &mut graph.groups[id].min_size;
                let width = cross(*size).max(1.0) + extra;
                if !width.is_finite() {
                    return Err(work.record_arithmetic_overflow().into());
                }
                if horizontal {
                    size.height = width;
                } else {
                    size.width = width;
                }
            }
        }
        Ok(())
    }

    pub(super) fn edge_ports(
        &self,
        graph: &MeasuredGraph,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<Vec<(PortSideHint, PortSideHint)>> {
        if !self.expanded.contains(&true) {
            return Ok(Vec::new());
        }
        let outer_face = if self.horizontal() {
            Side::Top
        } else {
            Side::Left
        };
        let port = |end| match end {
            Endpoint::Group(id) if self.expanded[id as usize] => PortSideHint::Side(outer_face),
            _ => PortSideHint::Auto,
        };
        work.charge_adapter(graph.edges.len())?;
        Ok(graph
            .edges
            .iter()
            .map(|edge| (port(edge.source), port(edge.target)))
            .collect())
    }

    pub(super) fn restore_shells(
        &self,
        solved: &mut SolvedGraph,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<()> {
        if !self.expanded.contains(&true) {
            return Ok(());
        }
        work.charge_adapter(solved.groups.len())?;
        let mut children: Vec<Option<Rect>> = vec![None; solved.groups.len()];
        for &id in self.order.iter().rev() {
            work.charge_adapter(1)?;
            let id = id as usize;
            let group = &mut solved.groups[id];
            group.parent = self.parents[id];
            let rect = group.rect;
            let mut shell = Rect::from_origin_size((rect.x, rect.y), (rect.width, rect.height));
            if let Some(contents) = children[id] {
                shell = shell.union(contents);
                if let Some(header) = &self.headers[id] {
                    let size = header.size;
                    let along = if self.horizontal() {
                        size.width
                    } else {
                        size.height
                    };
                    let band = along + 2.0 * header.padding;
                    // Put the outer heading beyond the shared rank grid's routed extent.
                    // A route or marker using an exterior rail cannot intersect the new band.
                    let center = match self.flow {
                        Direction::Down => {
                            shell.y0 = shell.y0.min(0.0) - band;
                            (shell.center().x, shell.y0 + band / 2.0)
                        }
                        Direction::Up => {
                            shell.y1 = shell.y1.max(solved.bounds.height) + band;
                            (shell.center().x, shell.y1 - band / 2.0)
                        }
                        Direction::Right => {
                            shell.x0 = shell.x0.min(0.0) - band;
                            (shell.x0 + band / 2.0, shell.center().y)
                        }
                        Direction::Left => {
                            shell.x1 = shell.x1.max(solved.bounds.width) + band;
                            (shell.x1 - band / 2.0, shell.center().y)
                        }
                    };
                    group.title = Some(crate::model::LayoutLabel {
                        x: center.0,
                        y: center.1,
                        width: size.width,
                        height: size.height,
                    });
                }
                if !shell.is_finite() || !shell.width().is_finite() || !shell.height().is_finite() {
                    return Err(work.record_arithmetic_overflow().into());
                }
                group.rect.x = shell.x0;
                group.rect.y = shell.y0;
                group.rect.width = shell.width();
                group.rect.height = shell.height();
            }
            if let Some(parent) = self.parents[id] {
                let current = &mut children[parent as usize];
                *current = Some(current.map_or(shell, |bounds| bounds.union(shell)));
            }
        }
        Ok(())
    }
}
