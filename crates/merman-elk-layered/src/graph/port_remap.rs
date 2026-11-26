//! Port slot mappings and reference sites used while reordering a graph.

use super::{LGraph, PortRef, ReorderNodePortsWorkContext, remap_self_loop_holder_ports};
use crate::work::{WorkControl, WorkError, checked_mul, checked_sum};

/// Composes original-to-current port slots during a pass that reads only port records.
/// Edge endpoints and all other external port references are restored together before the
/// pass returns, including after an interruption. Stable node slots key the maps. The sorter
/// admits deferred permutations only after checking all reference indices against fixed sizes.
#[derive(Default)]
pub(crate) enum PendingPortRemap {
    #[default]
    Empty,
    Single {
        owner: usize,
        slots: Vec<usize>,
    },
    Multiple(Vec<Option<Vec<usize>>>),
}

impl PendingPortRemap {
    pub(crate) fn reorder(&mut self, graph: &mut LGraph, owner: usize, order: Vec<usize>) {
        let Some((old_to_new, true)) = graph.permute_node_ports(owner, order) else {
            return;
        };
        match self {
            Self::Empty => {
                *self = Self::Single {
                    owner,
                    slots: old_to_new,
                };
            }
            Self::Single {
                owner: first,
                slots,
            } if *first == owner => compose_port_slots(slots, &old_to_new),
            Self::Single {
                owner: first,
                slots,
            } => {
                let mut owners = Vec::new();
                owners.resize_with(graph.layerless_nodes.len(), || None);
                owners[*first] = Some(std::mem::take(slots));
                owners[owner] = Some(old_to_new);
                *self = Self::Multiple(owners);
            }
            Self::Multiple(owners) => {
                let pending = &mut owners[owner];
                if let Some(slots) = pending {
                    compose_port_slots(slots, &old_to_new);
                } else {
                    *pending = Some(old_to_new);
                }
            }
        }
    }

    pub(crate) fn apply(self, graph: &mut LGraph) {
        match self {
            Self::Empty => {}
            Self::Single { owner, slots } => {
                graph.remap_deferred_port_references(|node| {
                    (node == owner).then_some(slots.as_slice())
                });
            }
            Self::Multiple(owners) => {
                graph.remap_deferred_port_references(|owner| {
                    owners.get(owner).and_then(Option::as_deref)
                });
            }
        }
    }
}

fn compose_port_slots(slots: &mut [usize], old_to_new: &[usize]) {
    for current in slots {
        *current = old_to_new[*current];
    }
}

#[derive(Debug, Clone, Copy)]
enum Reference {
    Source(usize),
    Target(usize),
    Opposite(usize),
    SelfLoop(usize),
    Origin(usize),
    LongSource(usize),
    LongTarget(usize),
}

/// A port permutation changes slot indices while each reference still belongs to the same node.
/// The sweep leaves the graph's edge, node and holder arenas intact. Record reference sites in
/// the order the full remap visits them, keyed by the owner of the port to be permuted.
#[derive(Debug, Clone)]
pub(crate) struct FlatPortRemap(Vec<Vec<Reference>>);

impl FlatPortRemap {
    pub(crate) fn new(graph: &LGraph, work: &mut dyn WorkControl) -> Result<Self, WorkError> {
        let nodes = graph.layerless_nodes.len();
        let edges = graph.edges.len();
        let holders = graph.self_loop_holders.len();
        const POLL_INTERVAL: usize = 64;
        // Reserve one visit per slot or field and one write per potential site. Charge
        // same-length graph ID comparisons separately when an origin actually exists.
        let initial = checked_sum([
            checked_mul(nodes, 7)?,
            checked_mul(edges, 6)?,
            checked_mul(holders, 2)?,
            nodes.div_ceil(POLL_INTERVAL),
            edges.div_ceil(POLL_INTERVAL),
            holders.div_ceil(POLL_INTERVAL),
        ])?;
        work.check(initial)?;
        work.charge(initial)?;

        let mut references = vec![Vec::new(); nodes];
        let mut add = |owner, reference| {
            if let Some(sites) = references.get_mut(owner) {
                Vec::push(sites, reference);
            }
        };
        for (index, edge) in graph.edges.iter().enumerate() {
            if index % POLL_INTERVAL == 0 {
                work.check(0)?;
            }
            add(edge.source.node, Reference::Source(index));
            add(edge.target.node, Reference::Target(index));
            if let Some(port) = edge.original_opposite_port {
                add(port.node, Reference::Opposite(index));
            }
        }
        for (index, holder) in graph.self_loop_holders.iter().enumerate() {
            if index % POLL_INTERVAL == 0 {
                work.check(0)?;
            }
            add(holder.node, Reference::SelfLoop(index));
        }
        for (index, node) in graph.layerless_nodes.iter().enumerate() {
            if index % POLL_INTERVAL == 0 {
                work.check(0)?;
            }
            debug_assert!(node.nested_graph.is_none());
            if let Some(origin) = &node.origin_port
                && origin.graph_id.len() == graph.id.len()
            {
                let bytes = graph.id.len();
                work.check(bytes)?;
                work.charge(bytes)?;
                if origin.graph_id == graph.id {
                    add(origin.port.node, Reference::Origin(index));
                }
            }
            if let Some(port) = node.long_edge_source {
                add(port.node, Reference::LongSource(index));
            }
            if let Some(port) = node.long_edge_target {
                add(port.node, Reference::LongTarget(index));
            }
        }
        Ok(Self(references))
    }

    pub(crate) fn node_work(
        &self,
        graph: &LGraph,
        owner: usize,
        context: &ReorderNodePortsWorkContext,
    ) -> Result<usize, WorkError> {
        checked_sum([
            checked_mul(graph.layerless_nodes[owner].ports.len(), 4)?,
            self.0[owner].len(),
            context.self_loop_payload_by_node[owner],
        ])
    }

    pub(super) fn apply(&self, graph: &mut LGraph, owner: usize, old_to_new: &[usize]) {
        for &reference in &self.0[owner] {
            let port: &mut PortRef = match reference {
                Reference::Source(edge) => &mut graph.edges[edge].source,
                Reference::Target(edge) => &mut graph.edges[edge].target,
                Reference::Opposite(edge) => {
                    graph.edges[edge].original_opposite_port.as_mut().unwrap()
                }
                Reference::SelfLoop(holder) => {
                    remap_self_loop_holder_ports(&mut graph.self_loop_holders[holder], old_to_new);
                    continue;
                }
                Reference::Origin(node) => {
                    &mut graph.layerless_nodes[node]
                        .origin_port
                        .as_mut()
                        .unwrap()
                        .port
                }
                Reference::LongSource(node) => graph.layerless_nodes[node]
                    .long_edge_source
                    .as_mut()
                    .unwrap(),
                Reference::LongTarget(node) => graph.layerless_nodes[node]
                    .long_edge_target
                    .as_mut()
                    .unwrap(),
            };
            debug_assert_eq!(port.node, owner);
            port.port = old_to_new[port.port];
        }
    }
}
