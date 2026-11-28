//! Port slot mappings and reference sites used while reordering a graph.

use super::{LGraph, PortRef, ReorderNodePortsWorkContext, remap_self_loop_holder_ports};
use crate::work::{WorkControl, WorkError, checked_mul, checked_sum};

/// Composes original-to-current port slots during a port-order pass.
/// Edge endpoints and all other external port references are restored together before the
/// pass returns, including after an interruption. Until then local references identify ports
/// by their original slots. Compare their current order through `current_rank`. Stable node
/// slots key the maps. Deferral requires checking reference indices against fixed sizes.
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
    pub(crate) fn current_rank(&self, reference: PortRef) -> usize {
        match self {
            Self::Single { owner, slots } if *owner == reference.node => slots[reference.port],
            Self::Multiple(owners) => owners
                .get(reference.node)
                .and_then(Option::as_ref)
                .map_or(reference.port, |slots| slots[reference.port]),
            _ => reference.port,
        }
    }

    pub(crate) fn reorder_work(
        &self,
        owner: usize,
        context: &ReorderNodePortsWorkContext,
    ) -> Result<usize, WorkError> {
        let payload = context.self_loop_payload_by_node[owner];
        match self {
            Self::Empty => checked_sum([1, context.deferred_reference_scan_work()?, payload]),
            Self::Single {
                owner: first,
                slots,
            } if *first == owner => checked_sum([1, slots.len()]),
            Self::Single { .. } => {
                let owner_table_work = checked_mul(context.self_loop_payload_by_node.len(), 2)?;
                checked_sum([2, owner_table_work, payload])
            }
            Self::Multiple(owners) => {
                checked_sum([1, owners[owner].as_ref().map_or(payload, Vec::len)])
            }
        }
    }

    pub(crate) fn reorder(&mut self, graph: &mut LGraph, owner: usize, order: Vec<usize>) {
        let Some((old_to_new, true)) = graph.permute_node_ports(owner, order) else {
            return;
        };
        self.record_order(graph.layerless_nodes.len(), owner, old_to_new);
    }

    fn record_order(&mut self, node_count: usize, owner: usize, old_to_new: Vec<usize>) {
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
                owners.resize_with(node_count, || None);
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

/// PortListSorter keeps the active owner's records in their original slots while it
/// reads them by rank. The inverse maps for eventual reference repair are separate.
#[derive(Default)]
pub(crate) struct PortListStorage {
    active: Option<StablePortOrder>,
}

struct StablePortOrder {
    owner: usize,
    ids_by_rank: Box<[usize]>,
}

impl StablePortOrder {
    fn install(mut self, graph: &mut LGraph) {
        let ports = &mut graph.layerless_nodes[self.owner].ports;
        // Consume the already validated permutation by cycles. The scratch space for the
        // visited marks is the order itself, so an interrupted pass can install it in Drop.
        const INSTALLED: usize = usize::MAX;
        for start in 0..self.ids_by_rank.len() {
            if self.ids_by_rank[start] == INSTALLED {
                continue;
            }
            let mut rank = start;
            loop {
                let id = std::mem::replace(&mut self.ids_by_rank[rank], INSTALLED);
                if id != start {
                    ports.swap(rank, id);
                }
                ports[rank].node = self.owner;
                if id == start {
                    break;
                }
                rank = id;
            }
        }
    }
}

impl PortListStorage {
    pub(crate) fn port_id_at_rank(&self, owner: usize, rank: usize) -> usize {
        self.active
            .as_ref()
            .filter(|order| order.owner == owner)
            .map_or(rank, |order| order.ids_by_rank[rank])
    }

    pub(crate) fn stage_order(
        &mut self,
        graph: &mut LGraph,
        pending: &mut PendingPortRemap,
        owner: usize,
        mut order: Vec<usize>,
    ) {
        let Some(node) = graph.layerless_nodes.get_mut(owner) else {
            return;
        };
        if order.len() != node.ports.len() {
            return;
        }
        // The inverse table doubles as the duplicate check. Validate before replacing any
        // staged order, and keep fixed-size storage for the one active owner.
        let mut old_to_new = vec![usize::MAX; order.len()];
        let mut identity = true;
        for (rank, &old_rank) in order.iter().enumerate() {
            let Some(slot) = old_to_new.get_mut(old_rank) else {
                return;
            };
            if *slot != usize::MAX {
                return;
            }
            *slot = rank;
            identity &= rank == old_rank;
        }
        if identity {
            for port in &mut node.ports {
                port.node = owner;
            }
            return;
        }

        if let Some(active) = self.active.as_ref().filter(|active| active.owner == owner) {
            for slot in &mut order {
                *slot = active.ids_by_rank[*slot];
            }
        } else if let Some(previous) = self.active.take() {
            previous.install(graph);
        }
        self.active = Some(StablePortOrder {
            owner,
            ids_by_rank: order.into_boxed_slice(),
        });
        pending.record_order(graph.layerless_nodes.len(), owner, old_to_new);
    }

    pub(crate) fn install(self, graph: &mut LGraph) {
        if let Some(active) = self.active {
            active.install(graph);
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
            if let Some(origin) = &node.origin_port {
                if origin.graph_id.len() == graph.id.len() {
                    let bytes = graph.id.len();
                    work.check(bytes)?;
                    work.charge(bytes)?;
                }
                if origin.belongs_to(graph) {
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
