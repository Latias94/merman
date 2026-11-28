//! Expanded level slots, in the reference's edge and dummy occurrence order.

use std::ops::Range;

use super::super::Size;
use super::level::{Attachment, Membership};
use super::ranking::RankPlan;
use super::work::compare_number;
use super::{Adjacency, LevelPlan, NodeKind, Result, Work, add, inconsistent, prefix};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Node,
    Bend,
    Label,
    StartPort,
    EndPort,
}

impl Kind {
    pub fn is_dummy(self) -> bool {
        matches!(self, Self::Bend | Self::Label)
    }
}

impl From<NodeKind> for Kind {
    fn from(value: NodeKind) -> Self {
        match value {
            NodeKind::Node => Self::Node,
            NodeKind::StartPort => Self::StartPort,
            NodeKind::EndPort => Self::EndPort,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Vertex {
    pub rank: usize,
    pub size: Size,
    pub span: f64,
    pub kind: Kind,
    pub pin: i8,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Along {
    Up = 0,
    Down = 1,
}

impl Along {
    pub fn other(self) -> Self {
        match self {
            Self::Up => Self::Down,
            Self::Down => Self::Up,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum End {
    Held,
    Leaves,
    Arrives,
}

#[derive(Clone, Copy)]
pub(super) struct Ends {
    pub offset: [f64; 2],
    pub kind: [End; 2],
}

impl Ends {
    fn expanded(up: Attachment, down: Attachment, reversed: bool) -> Self {
        Self {
            offset: [up.dx, down.dx],
            kind: [
                if up.fixed {
                    End::Held
                } else if reversed {
                    End::Leaves
                } else {
                    End::Arrives
                },
                if down.fixed {
                    End::Held
                } else if reversed {
                    End::Arrives
                } else {
                    End::Leaves
                },
            ],
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Segment {
    pub from: usize,
    pub to: usize,
}

pub(super) struct EdgeExpansion {
    pub dummies: Range<usize>,
    pub label_node: Option<usize>,
    pub segments: Range<usize>,
    pub original: [Attachment; 2],
    pub label: Size,
}

pub(super) struct FlatOrder {
    pub real: usize,
    pub vertices: Vec<Vertex>,
    pub segments: Vec<Segment>,
    pub ends: Vec<Ends>,
    pub edges: Vec<EdgeExpansion>,
    pub label_heads: Vec<u8>,
    pub outgoing: Adjacency,
    pub incoming: Adjacency,
    pub layer_start: Vec<usize>,
    pub order: Vec<usize>,
    pub positions: Vec<usize>,
    pub offsets: bool,
    pub fixed: Vec<bool>,
    pub groups: Option<Groups>,
    pub declared_crossings: f64,
    pub crossings: f64,
}

impl FlatOrder {
    pub fn rows(&self) -> usize {
        self.layer_start.len() - 1
    }

    pub fn rank_range(&self, rank: usize) -> Range<usize> {
        self.layer_start[rank]..self.layer_start[rank + 1]
    }

    pub fn set_positions<E>(&mut self, work: &mut Work<'_, E>) -> Result<(), E> {
        work.price(self.rows())?;
        work.price(self.order.len())?;
        for rank in 0..self.rows() {
            work.poll(rank)?;
            for (i, &node) in self.order[self.rank_range(rank)].iter().enumerate() {
                work.poll(i)?;
                self.positions[node] = i;
            }
        }
        Ok(())
    }
}

pub(super) fn expand<E>(
    plan: LevelPlan<'_>,
    ranked: &RankPlan,
    work: &mut Work<'_, E>,
) -> Result<FlatOrder, E> {
    let real = plan.nodes.len();
    let node_count = add(real, ranked.dummy_count)?;
    let layer_slots = add(ranked.max_rank, 2)?;
    let mut vertices = work.reserve(node_count)?;
    let mut keys = work.reserve(node_count)?;
    let mut segments = work.reserve(ranked.segment_count)?;
    let mut ends = work.reserve(ranked.segment_count)?;
    let mut edges = work.reserve(ranked.edges.len())?;
    let mut label_heads = work.filled(layer_slots - 1, 0u8)?;
    let mut groups = plan
        .groups
        .map(|m| Groups::new(m, node_count, work))
        .transpose()?;
    work.price(real)?;
    let mut pinned = false;
    for (i, n) in plan.nodes.iter().enumerate() {
        work.poll(i)?;
        keys.push(n.key.unwrap_or(f64::NAN));
        vertices.push(Vertex {
            rank: ranked.ranks[i],
            size: n.size,
            span: n.span,
            kind: n.kind.into(),
            pin: n.pin,
        });
        pinned |= n.pin != 0;
    }

    let held = Attachment {
        dx: 0.0,
        fixed: true,
    };
    let mut offsets = false;
    work.price(plan.edges.len())?;
    for (i, (edge, input)) in ranked.edges.iter().zip(plan.edges).enumerate() {
        work.poll(i)?;
        let (a, b) = edge.rank_endpoints();
        let (ra, rb) = (ranked.ranks[a], ranked.ranks[b]);
        let (first, last) = if edge.reversed {
            (input.head_attachment, input.tail_attachment)
        } else {
            (input.tail_attachment, input.head_attachment)
        };
        let mut expansion = EdgeExpansion {
            dummies: vertices.len()..vertices.len(),
            label_node: None,
            segments: segments.len()..segments.len(),
            original: [last, first],
            label: input.label,
        };
        if !edge.direct {
            let label_rank = (input.label.width > 0.0)
                .then(|| midpoint(ra, edge.span, ranked.step, edge.reversed));
            let group = if edge.span > 1 {
                groups
                    .as_ref()
                    .map(|g| g.lca(g.of[input.tail], g.of[input.head], work))
                    .transpose()?
                    .flatten()
            } else {
                None
            };
            offsets |= first.dx != 0.0 || last.dx != 0.0;
            let mut prev = a;
            work.price(edge.span)?;
            for (i, heads) in label_heads[ra + 1..rb].iter_mut().enumerate() {
                work.poll(i)?;
                let rank = ra + 1 + i;
                let node = vertices.len();
                let (kind, size) = if Some(rank) == label_rank {
                    expansion.label_node = Some(node);
                    *heads |= if edge.reversed { 2 } else { 1 };
                    (Kind::Label, input.label)
                } else {
                    (Kind::Bend, Size::default())
                };
                vertices.push(Vertex {
                    rank,
                    size,
                    span: 0.0,
                    kind,
                    pin: 0,
                });
                keys.push(keys[a] + ((keys[b] - keys[a]) * (rank - ra) as f64) / (rb - ra) as f64);
                if let Some(groups) = &mut groups {
                    groups.of.push(group);
                }
                segments.push(Segment {
                    from: prev,
                    to: node,
                });
                ends.push(Ends::expanded(
                    held,
                    if prev == a { first } else { held },
                    edge.reversed,
                ));
                prev = node;
            }
            segments.push(Segment { from: prev, to: b });
            ends.push(Ends::expanded(
                last,
                if prev == a { first } else { held },
                edge.reversed,
            ));
            expansion.dummies.end = vertices.len();
            expansion.segments.end = segments.len();
        }
        edges.push(expansion);
    }

    let outgoing = Adjacency::build(node_count, segments.len(), |i| Some(segments[i].from), work)?;
    let incoming = Adjacency::build(node_count, segments.len(), |i| Some(segments[i].to), work)?;
    let mut layer_start = work.filled(layer_slots, 0usize)?;
    work.price(node_count)?;
    for (i, v) in vertices.iter().enumerate() {
        work.poll(i)?;
        layer_start[v.rank + 1] += 1;
    }
    prefix(&mut layer_start, work)?;
    let mut order = initial_order(&layer_start, &outgoing, &segments, &vertices, work)?;
    let mut spare = Vec::new();
    work.price(layer_start.len() - 1)?;
    for (i, rank) in layer_start.windows(2).enumerate() {
        work.poll(i)?;
        sort_rank(&mut order[rank[0]..rank[1]], &mut spare, |v| keys[v], work)?;
    }
    if pinned {
        work.price(layer_start.len() - 1)?;
        for (i, rank) in layer_start.windows(2).enumerate() {
            work.poll(i)?;
            work.sort_with(&mut order[rank[0]..rank[1]], &mut spare, |a, b, _| {
                Ok(vertices[a].pin.cmp(&vertices[b].pin))
            })?;
        }
    }
    let mut fixed = work.filled(layer_slots - 1, false)?;
    for rank in [0, ranked.max_rank] {
        fixed[rank] = sort_rank(
            &mut order[layer_start[rank]..layer_start[rank + 1]],
            &mut spare,
            |v| {
                plan.nodes
                    .get(v)
                    .filter(|n| n.kind != NodeKind::Node)
                    .and_then(|n| n.hint)
                    .unwrap_or(f64::NAN)
            },
            work,
        )?;
    }
    let mut graph = FlatOrder {
        real,
        vertices,
        segments,
        ends,
        edges,
        label_heads,
        outgoing,
        incoming,
        layer_start,
        order,
        positions: work.filled(node_count, 0)?,
        offsets,
        fixed,
        groups,
        crossings: 0.0,
        declared_crossings: 0.0,
    };
    graph.set_positions(work)?;
    Ok(graph)
}

fn midpoint(start: usize, span: usize, step: i32, reversed: bool) -> usize {
    let middle = start + (span >> 1);
    if step == 2 && (middle % 2 == 1) == (span >= 4) {
        if reversed { middle + 1 } else { middle - 1 }
    } else {
        middle
    }
}

fn sort_rank<E>(
    row: &mut [usize],
    spare: &mut Vec<usize>,
    key: impl Fn(usize) -> f64,
    work: &mut Work<'_, E>,
) -> Result<bool, E> {
    if row.len() < 2 {
        return Ok(false);
    }
    work.price(row.len())?;
    for (i, &v) in row.iter().enumerate() {
        work.poll(i)?;
        if key(v).is_nan() {
            return Ok(false);
        }
    }
    work.sort_with(row, spare, |a, b, _| Ok(compare_number(key(a), key(b))))?;
    Ok(true)
}

fn initial_order<E>(
    starts: &[usize],
    outgoing: &Adjacency,
    segments: &[Segment],
    vertices: &[Vertex],
    work: &mut Work<'_, E>,
) -> Result<Vec<usize>, E> {
    let nodes = vertices.len();
    let starts = &starts[..starts.len() - 1];
    let mut by_rank = work.filled(nodes, 0usize)?;
    let mut fill = work.copy(starts)?;
    work.price(nodes)?;
    for (i, v) in vertices.iter().enumerate() {
        work.poll(i)?;
        let position = &mut fill[v.rank];
        by_rank[*position] = i;
        *position += 1;
    }
    work.copy_into(&mut fill, starts)?;
    let mut order = work.filled(nodes, 0usize)?;
    let mut visited = work.filled(nodes, false)?;
    // Each frame resumes source-ordered successors without putting duplicate nodes on the stack.
    let mut stack = work.reserve(nodes.min(starts.len()))?;
    let mut visit = |node: usize, stack: &mut Vec<_>, visited: &mut [bool]| {
        visited[node] = true;
        let position = &mut fill[vertices[node].rank];
        order[*position] = node;
        *position += 1;
        stack.push((node, outgoing.starts[node]));
    };
    work.scaled(nodes, 2)?;
    work.price(segments.len())?;
    let mut walked = 0usize;
    for (i, root) in by_rank.into_iter().enumerate() {
        work.poll(i)?;
        if visited[root] {
            continue;
        }
        visit(root, &mut stack, &mut visited);
        while let Some((node, next)) = stack.last_mut() {
            work.poll(walked)?;
            walked += 1;
            if *next == outgoing.starts[*node + 1] {
                stack.pop();
                continue;
            }
            let target = segments[outgoing.slots[*next]].to;
            *next += 1;
            if !visited[target] {
                visit(target, &mut stack, &mut visited);
            }
        }
    }
    Ok(order)
}

pub(super) struct Groups {
    pub of: Vec<Option<usize>>,
    pub parent: Vec<Option<usize>>,
    pub depth: Vec<usize>,
}

pub(super) struct LiftedGroups {
    pub left: Option<usize>,
    pub right: Option<usize>,
    ancestor: Option<usize>,
}

impl Groups {
    fn new<E>(input: Membership<'_>, nodes: usize, work: &mut Work<'_, E>) -> Result<Self, E> {
        let len = input.parent.len();
        let mut of = work.reserve(nodes)?;
        work.price(input.of.len())?;
        for (i, &group) in input.of.iter().enumerate() {
            work.poll(i)?;
            if group.is_some_and(|g| g >= len) {
                return Err(inconsistent("invalid node group slot"));
            }
            of.push(group);
        }
        let parent = work.copy(input.parent)?;
        work.price(len)?;
        for (i, &slot) in parent.iter().enumerate() {
            work.poll(i)?;
            if slot.is_some_and(|p| p >= len) {
                return Err(inconsistent("invalid parent group slot"));
            }
        }
        let mut depth = work.filled(len, 0usize)?;
        let mut stack = Vec::new();
        work.price(len)?;
        for start in 0..len {
            work.poll(start)?;
            let mut next = Some(start);
            let mut known = 0;
            while let Some(g) = next {
                work.price(1)?;
                match depth[g] {
                    usize::MAX => return Err(inconsistent("cycle in group membership")),
                    0 => {
                        depth[g] = usize::MAX;
                        work.push(&mut stack, g)?;
                        next = parent[g];
                    }
                    value => {
                        known = value;
                        break;
                    }
                }
            }
            work.price(stack.len())?;
            for (i, g) in stack.drain(..).rev().enumerate() {
                work.poll(i)?;
                known += 1;
                depth[g] = known;
            }
        }
        Ok(Self { of, parent, depth })
    }

    pub fn lift<E>(
        &self,
        mut a: Option<usize>,
        mut b: Option<usize>,
        work: &mut Work<'_, E>,
    ) -> Result<LiftedGroups, E> {
        let mut left = None;
        let mut right = None;
        let mut da = a.map_or(0, |g| self.depth[g]);
        let mut db = b.map_or(0, |g| self.depth[g]);
        while da != db {
            work.price(1)?;
            if da > db {
                left = a;
                a = a.and_then(|g| self.parent[g]);
                da -= 1;
            } else {
                right = b;
                b = b.and_then(|g| self.parent[g]);
                db -= 1;
            }
        }
        while a != b {
            work.price(1)?;
            left = a;
            right = b;
            a = a.and_then(|g| self.parent[g]);
            b = b.and_then(|g| self.parent[g]);
        }
        Ok(LiftedGroups {
            left,
            right,
            ancestor: a,
        })
    }

    fn lca<E>(
        &self,
        a: Option<usize>,
        b: Option<usize>,
        work: &mut Work<'_, E>,
    ) -> Result<Option<usize>, E> {
        Ok(self.lift(a, b, work)?.ancestor)
    }
}
