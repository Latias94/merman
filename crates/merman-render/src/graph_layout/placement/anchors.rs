use crate::graph_layout::ir::{Endpoint, MeasuredGraph, Point, Rect};
use crate::layout_frame::{FrameSignature, ShoalFramePosition, ShoalLayoutFrame};
use crate::layout_work::OperationLayoutWorkControl;
use crate::{Error, Result};
use std::collections::HashMap;

/// The origin is the upper-left of the component's node boxes in physical drawing axes.
/// Route and label extents can change without translating these anchors.
pub(super) struct Components {
    owners: Vec<usize>,
    ids: Vec<u64>,
    incident: Vec<u64>,
}

impl Components {
    pub(super) fn new<'id>(
        graph: &MeasuredGraph,
        ids: impl ExactSizeIterator<Item = &'id str>,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<Self> {
        let count = ids.len();
        work.charge_adapter(work.checked_mul(count, 5)?)?;
        let mut identities = Vec::with_capacity(count);
        for id in ids {
            work.charge_adapter(id.len())?;
            // Fixed FNV-1a over UTF-8 bytes, independent of process and Rust hash seeds.
            identities.push(id.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3)
            }));
        }
        let mut owners: Vec<_> = (0..count).collect();
        let mut sizes = vec![1usize; count];
        let mut incident = vec![0u64; count];
        let mut components = identities.clone();
        let endpoint = |endpoint| match endpoint {
            Endpoint::Node(node) if (node as usize) < count => Ok(node as usize),
            _ => Err(Error::InvalidModel {
                message: "Invalid flat Shoal frame endpoint".into(),
            }),
        };
        for edge in &graph.edges {
            work.charge_adapter(1)?;
            let from = endpoint(edge.source)?;
            let to = endpoint(edge.target)?;
            // Addition retains multiplicity and is invariant to declaration order.
            incident[from] =
                incident[from].wrapping_add(mix(identities[to] ^ 0x868d_c31d_c2e9_79ce));
            incident[to] = incident[to].wrapping_add(mix(identities[from] ^ 0x4948_2e0f_3e33_4ea1));
            let mut a = root(&mut owners, from);
            let mut b = root(&mut owners, to);
            if a != b {
                if sizes[a] < sizes[b] {
                    std::mem::swap(&mut a, &mut b);
                }
                owners[b] = a;
                sizes[a] += sizes[b];
                components[a] = components[a].min(components[b]);
            }
        }
        for node in 0..owners.len() {
            work.charge_adapter(1)?;
            owners[node] = root(&mut owners, node);
        }
        Ok(Self {
            owners,
            ids: components,
            incident,
        })
    }

    pub(super) fn signature(&self, node: usize) -> u64 {
        self.incident[node]
    }

    pub(super) fn signatures(&self) -> &[u64] {
        &self.incident
    }

    pub(super) fn matches<'id>(
        &self,
        ids: impl ExactSizeIterator<Item = &'id str>,
        previous: &ShoalLayoutFrame,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<Vec<bool>> {
        let mut by_old = HashMap::with_capacity(ids.len().min(previous.nodes.len()));
        let mut by_new = vec![None; ids.len()];
        let mut merged = vec![false; ids.len()];
        let mut pairs = Vec::with_capacity(ids.len());
        let lookup = previous.nodes.len().ilog2() as usize + 1;
        for (owner, id) in self.owners.iter().copied().zip(ids) {
            work.charge_adapter(lookup)?;
            let token = previous
                .nodes
                .get(id)
                .and_then(|node| node.position)
                .map(|p| p.component);
            if let Some(token) = token {
                let old = by_old.entry(token).or_insert(Some(owner));
                if *old != Some(owner) {
                    *old = None;
                }
                merged[owner] |= *by_new[owner].get_or_insert(token) != token;
            }
            pairs.push(token);
        }
        work.charge_adapter(pairs.len())?;
        Ok(self
            .owners
            .iter()
            .copied()
            .zip(pairs)
            .map(|(owner, token)| {
                !merged[owner] && token.is_some_and(|token| by_old[&token] == Some(owner))
            })
            .collect())
    }

    pub(super) fn positions(
        self,
        rects: &[Rect],
        work: &mut OperationLayoutWorkControl,
    ) -> Result<Vec<ShoalFramePosition>> {
        work.charge_adapter(work.checked_mul(rects.len(), 2)?)?;
        let infinity = Point {
            x: f64::INFINITY,
            y: f64::INFINITY,
        };
        let mut origins = vec![infinity; rects.len()];
        for (rect, &owner) in rects.iter().zip(&self.owners) {
            let origin = &mut origins[owner];
            origin.x = origin.x.min(rect.x);
            origin.y = origin.y.min(rect.y);
        }
        Ok(rects
            .iter()
            .enumerate()
            .map(|(node, rect)| {
                let owner = self.owners[node];
                let center = rect.center();
                ShoalFramePosition {
                    x: center.x - origins[owner].x,
                    y: center.y - origins[owner].y,
                    width: rect.width,
                    height: rect.height,
                    component: FrameSignature::from_bits(self.ids[owner]),
                    incident_signature: FrameSignature::from_bits(self.incident[node]),
                }
            })
            .collect())
    }
}

fn root(owners: &mut [usize], mut node: usize) -> usize {
    while owners[node] != node {
        owners[node] = owners[owners[node]];
        node = owners[node];
    }
    node
}

fn mix(mut bits: u64) -> u64 {
    bits = (bits ^ (bits >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    bits = (bits ^ (bits >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    bits ^ (bits >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph_layout::ir::{Direction, MeasuredEdge, MeasuredNode, RoutingStyle, Size};
    use crate::layout_frame::{FrameDirection, ShoalFrameNode};

    const IDS: [&str; 4] = ["A", "B", "C", "D"];

    fn graph(pairs: &[(u32, u32)]) -> MeasuredGraph {
        MeasuredGraph {
            direction: Direction::Down,
            routing: RoutingStyle::Orthogonal,
            node_spacing: 50.0,
            rank_spacing: 50.0,
            component_spacing: 50.0,
            inherit_crossed_group_direction: false,
            nodes: (0..4)
                .map(|seq| MeasuredNode {
                    size: Size {
                        width: 40.0,
                        height: 20.0,
                    },
                    seq,
                    parent: None,
                    port_span: None,
                    perpendicular_port_span: None,
                    anchor_y: None,
                })
                .collect(),
            groups: Vec::new(),
            edges: pairs
                .iter()
                .map(|&(from, to)| MeasuredEdge {
                    source: Endpoint::Node(from),
                    target: Endpoint::Node(to),
                    min_rank_span: 1,
                    weight: 1.0,
                    label: None,
                    start_label: None,
                    end_label: None,
                    start_marker: Default::default(),
                    end_marker: Default::default(),
                })
                .collect(),
        }
    }

    #[test]
    fn signatures_ignore_statement_order_and_anchors_do_not_cross_a_split_or_merge() {
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let mut work = OperationLayoutWorkControl::new(std::sync::Arc::clone(session.work_meter()));
        let source = graph(&[(0, 1), (2, 3), (0, 1)]);
        let prior = Components::new(&source, IDS.into_iter(), &mut work).unwrap();
        let reordered = Components::new(
            &graph(&[(2, 3), (0, 1), (0, 1)]),
            IDS.into_iter(),
            &mut work,
        )
        .unwrap();
        assert_eq!(prior.incident, reordered.incident);
        let positions = prior
            .positions(
                &[
                    Rect {
                        x: 35.,
                        y: 15.,
                        width: 40.,
                        height: 20.,
                    },
                    Rect {
                        x: 45.,
                        y: 85.,
                        width: 40.,
                        height: 20.,
                    },
                    Rect {
                        x: 200.,
                        y: 50.,
                        width: 40.,
                        height: 20.,
                    },
                    Rect {
                        x: 212.,
                        y: 90.,
                        width: 40.,
                        height: 20.,
                    },
                ],
                &mut work,
            )
            .unwrap();
        assert_eq!((positions[0].x, positions[0].y), (20.0, 10.0));
        assert_eq!((positions[2].x, positions[2].y), (20.0, 10.0));
        assert_eq!((positions[1].x, positions[1].y), (30.0, 80.0));
        assert_ne!(positions[0].component, positions[2].component);
        let previous = ShoalLayoutFrame {
            family: "flowchart".into(),
            groups: Vec::new(),
            nodes: IDS
                .into_iter()
                .zip(positions)
                .map(|(id, position)| {
                    (
                        id.to_owned(),
                        ShoalFrameNode {
                            parent: None,
                            rank: 0,
                            cross_key: position.x,
                            direction: FrameDirection::Down,
                            position: Some(position),
                        },
                    )
                })
                .collect(),
            scratch: None,
        };
        for (edges, expected) in [
            (&[(0, 1), (2, 3)][..], [true, true, true, true]),
            (&[(0, 1)][..], [true, true, false, false]),
            (&[(0, 1), (2, 3), (1, 3)][..], [false; 4]),
        ] {
            let current = Components::new(&graph(edges), IDS.into_iter(), &mut work).unwrap();
            assert_eq!(
                current
                    .matches(IDS.into_iter(), &previous, &mut work)
                    .unwrap(),
                expected
            );
        }
        let changed =
            Components::new(&graph(&[(0, 1), (2, 3)]), IDS.into_iter(), &mut work).unwrap();
        for (index, id) in IDS.into_iter().enumerate() {
            assert_eq!(
                changed.signature(index)
                    == previous.nodes[id]
                        .position
                        .unwrap()
                        .incident_signature
                        .bits(),
                index >= 2
            );
        }
    }
}
