//! Shared StateDB node order and note ownership for flat Mermaid providers.

use merman_model::{OperationPhase, families::state::StateDiagramRenderModel};
use merman_render_core::{
    Bounds, Error, LayoutCluster, LayoutEdge, LayoutNode, Result,
    graph_layout::{Direction, direction_from_rank},
    resources::OperationWorkMeter,
};
use rustc_hash::FxHashMap;

use super::{HiddenPrefixMatcher, StateNode, note_group_owner_id};

pub(super) struct FlatStateNode<'a> {
    pub(super) node: &'a StateNode,
    pub(super) parent: Option<&'a str>,
    #[cfg(feature = "layout-grid")]
    pub(super) placement_id: Option<&'a str>,
    #[cfg(feature = "layout-grid")]
    pub(super) source_ordinal: usize,
}

pub(super) struct FlatStateInput<'a> {
    pub(super) nodes: Vec<FlatStateNode<'a>>,
    pub(super) slots: FxHashMap<&'a str, usize>,
    aliases: FxHashMap<&'a str, &'a str>,
}

impl FlatStateInput<'_> {
    pub(super) fn node_slot(&self, id: &str) -> Option<usize> {
        self.slots
            .get(self.aliases.get(id).copied().unwrap_or(id))
            .copied()
    }
}

pub(super) fn state_input<'a>(
    model: &'a StateDiagramRenderModel,
    hidden: &HiddenPrefixMatcher,
    work: &OperationWorkMeter,
    backend: &'static str,
) -> Result<FlatStateInput<'a>> {
    // StateDB updates a note group in its original slot and puts every note under it. The
    // parser retains a distinct generated identity for each declaration. Keep the last group
    // as its paint owner and the first group's placement key and source ordinal.
    let mut note_groups = FxHashMap::default();
    let mut aliases = FxHashMap::default();
    for (ordinal, node) in model.nodes.iter().enumerate() {
        work.checkpoint(OperationPhase::Layout)?;
        if let Some(owner) = note_group_owner(node).filter(|_| !hidden.is_hidden(&node.id)) {
            note_groups
                .try_reserve(1)
                .map_err(|_| allocation_error(backend))?;
            let slots = note_groups.entry(owner).or_insert((ordinal, ordinal));
            slots.1 = ordinal;
        }
    }
    for node in &model.nodes {
        work.checkpoint(OperationPhase::Layout)?;
        if let Some(&(_, last)) = note_group_owner(node).and_then(|id| note_groups.get(id)) {
            aliases
                .try_reserve(1)
                .map_err(|_| allocation_error(backend))?;
            aliases.insert(node.id.as_str(), model.nodes[last].id.as_str());
        }
    }
    let canonical = |id| aliases.get(id).copied().unwrap_or(id);
    let mut slots = FxHashMap::default();
    slots
        .try_reserve(model.nodes.len())
        .map_err(|_| allocation_error(backend))?;
    let mut nodes = buffer(model.nodes.len(), backend)?;
    for (ordinal, node) in model.nodes.iter().enumerate() {
        work.charge(node.id.len().max(1))?;
        if hidden.is_hidden(&node.id) {
            continue;
        }
        let (node, _placement_id) = if let Some(&(first, last)) =
            note_group_owner(node).and_then(|id| note_groups.get(id))
        {
            if ordinal != first {
                continue;
            }
            (
                &model.nodes[last],
                (first != last).then_some(node.id.as_str()),
            )
        } else {
            (node, None)
        };
        slots.insert(node.id.as_str(), nodes.len());
        nodes.push(FlatStateNode {
            node,
            parent: node
                .parent_id
                .as_deref()
                .filter(|id| !hidden.is_hidden(id))
                .map(canonical),
            #[cfg(feature = "layout-grid")]
            placement_id: _placement_id,
            #[cfg(feature = "layout-grid")]
            source_ordinal: ordinal,
        });
    }
    Ok(FlatStateInput {
        nodes,
        slots,
        aliases,
    })
}

fn note_group_owner(node: &StateNode) -> Option<&str> {
    if node.shape == "noteGroup" {
        note_group_owner_id(&node.id)
    } else {
        None
    }
}

pub(super) fn resolve_directions(
    nodes: &[FlatStateNode<'_>],
    slots: &FxHashMap<&str, usize>,
    root: Direction,
    work: &OperationWorkMeter,
    backend: &'static str,
) -> Result<Vec<Option<Direction>>> {
    work.charge(nodes.len())?;
    let mut directions = buffer(nodes.len(), backend)?;
    directions.extend(
        nodes
            .iter()
            .map(|input| input.node.dir.as_deref().map(direction_from_rank)),
    );
    let mut trail = Vec::new();
    for (slot, input) in nodes.iter().enumerate() {
        work.checkpoint(OperationPhase::Layout)?;
        if !matches!(input.node.shape.as_str(), "fork" | "join") || directions[slot].is_some() {
            continue;
        }
        let mut current = slot;
        let inherited = loop {
            if let Some(direction) = directions[current] {
                break direction;
            }
            work.charge(1)?;
            let Some(&parent) = nodes[current].parent.and_then(|id| slots.get(id)) else {
                break root;
            };
            if trail.len() == nodes.len() {
                let name = if backend == "grid" { "Grid" } else { "Layered" };
                return Err(Error::InvalidModel {
                    message: format!(
                        "Invalid State {name} projection: cycle while resolving shape directions"
                    ),
                });
            }
            trail
                .try_reserve(1)
                .map_err(|_| allocation_error(backend))?;
            trail.push(current);
            current = parent;
        };
        directions[current] = Some(inherited);
        for slot in trail.drain(..) {
            directions[slot] = Some(inherited);
        }
    }
    Ok(directions)
}

pub(super) fn charge_node_label(node: &StateNode, work: &OperationWorkMeter) -> Result<()> {
    match &node.label {
        Some(serde_json::Value::String(label)) => work.charge(label.len().max(1))?,
        Some(serde_json::Value::Array(lines)) => {
            for line in lines {
                work.charge(line.as_str().map_or(1, |line| line.len().max(1)))?;
            }
        }
        _ => {}
    }
    for line in node.description.iter().flatten() {
        work.charge(line.len().max(1))?;
    }
    Ok(())
}

pub(super) fn layout_bounds(
    nodes: &[LayoutNode],
    clusters: &[LayoutCluster],
    edges: &[LayoutEdge],
    work: &OperationWorkMeter,
) -> Result<Option<Bounds>> {
    fn extents(x: f64, y: f64, w: f64, h: f64) -> [(f64, f64); 2] {
        [(x - w / 2.0, y - h / 2.0), (x + w / 2.0, y + h / 2.0)]
    }
    work.charge(nodes.len())?;
    work.charge(clusters.len())?;
    for edge in edges {
        work.charge(edge.points.len().max(1))?;
    }
    let nodes = nodes
        .iter()
        .flat_map(|n| extents(n.x, n.y, n.width, n.height));
    let clusters = clusters
        .iter()
        .flat_map(|c| extents(c.x, c.y, c.width, c.height));
    let points = edges
        .iter()
        .flat_map(|edge| edge.points.iter().map(|p| (p.x, p.y)));
    let labels = edges
        .iter()
        .filter_map(|edge| edge.label.as_ref())
        .flat_map(|label| extents(label.x, label.y, label.width, label.height));
    Ok(Bounds::from_points(
        nodes.chain(clusters).chain(points).chain(labels),
    ))
}

pub(super) fn buffer<T>(capacity: usize, backend: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| allocation_error(backend))?;
    Ok(output)
}

fn allocation_error(backend: &'static str) -> Error {
    Error::LayoutAllocationFailed { backend }
}
