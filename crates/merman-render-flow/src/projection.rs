//! Borrowed semantic projections and layout edge identities for the flow families.

use merman_model::families::flowchart::{
    FlowEdge, FlowNode, FlowSubgraph, FlowchartModel, FlowchartRenderContext,
};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::ops::Deref;

/// Operation-local identity for one rendered Flowchart edge occurrence.
///
/// Mermaid permits distinct semantic edges to share the same public `edge.id`. Internal layout
/// and SVG caches must therefore use the semantic owner plus any generated self-loop segment.
/// Source styles and labels continue to use the stable semantic model index instead of raw DOM ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FlowchartEdgeKey {
    semantic_index: usize,
    self_loop_segment: Option<u8>,
}

impl FlowchartEdgeKey {
    pub const fn new(semantic_index: usize) -> Self {
        Self {
            semantic_index,
            self_loop_segment: None,
        }
    }

    pub const fn self_loop_segment(semantic_index: usize, order: u8) -> Self {
        Self {
            semantic_index,
            self_loop_segment: Some(order),
        }
    }

    pub const fn semantic_index(self) -> usize {
        self.semantic_index
    }

    pub const fn self_loop_segment_order(self) -> Option<u8> {
        self.self_loop_segment
    }

    fn adapter_id(self) -> String {
        match self.self_loop_segment {
            None => format!("__merman_flowchart_edge_{:020}", self.semantic_index),
            Some(order) => {
                format!(
                    "__merman_flowchart_edge_{:020}_segment_{order}",
                    self.semantic_index
                )
            }
        }
    }
}

/// Collision-safe adapter identities for one prepared set of semantic edge occurrences.
///
/// Unique Mermaid ids stay byte-identical so source-backed adapter ordering remains unchanged.
/// Only repeated ids enter the private occurrence namespace, and generated ids avoid every raw id
/// already present in the same model.
#[derive(Debug, Clone)]
pub struct FlowchartEdgeTransportPlan<'a> {
    ids: HashMap<FlowchartEdgeKey, Cow<'a, str>>,
}

impl<'a> FlowchartEdgeTransportPlan<'a> {
    #[cfg(test)]
    fn for_semantic_edges(edges: &'a [FlowEdge]) -> Self {
        Self::for_keyed_edges(
            edges
                .iter()
                .enumerate()
                .map(|(index, edge)| (FlowchartEdgeKey::new(index), edge)),
        )
    }

    pub fn for_keyed_edges(
        edges: impl Iterator<Item = (FlowchartEdgeKey, &'a FlowEdge)> + Clone,
    ) -> Self {
        let edge_count = edges.size_hint().0;
        let mut counts = HashMap::<&str, usize>::with_capacity(edge_count);
        for (_, edge) in edges.clone() {
            *counts.entry(edge.id.as_str()).or_default() += 1;
        }

        let mut ids = HashMap::with_capacity(edge_count);
        let mut generated = HashSet::new();
        for (key, edge) in edges {
            let id = if counts.get(edge.id.as_str()) == Some(&1) {
                Cow::Borrowed(edge.id.as_str())
            } else {
                let base = key.adapter_id();
                let mut candidate = base.clone();
                let mut suffix = 0usize;
                while counts.contains_key(candidate.as_str()) || generated.contains(&candidate) {
                    candidate = format!("{base}_{suffix:020}");
                    suffix = suffix.saturating_add(1);
                }
                generated.insert(candidate.clone());
                Cow::Owned(candidate)
            };
            ids.insert(key, id);
        }

        Self { ids }
    }

    pub fn id(&self, key: FlowchartEdgeKey) -> Option<&str> {
        self.ids.get(&key).map(Cow::as_ref)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FlowchartRenderModelRef<'a> {
    pub semantic: &'a FlowchartModel,
    pub render_context: &'a FlowchartRenderContext,
}

impl<'a> FlowchartRenderModelRef<'a> {
    pub const fn new(
        semantic: &'a FlowchartModel,
        render_context: &'a FlowchartRenderContext,
    ) -> Self {
        Self {
            semantic,
            render_context,
        }
    }

    pub fn node_label_for_render<'b>(&'b self, node: &'b FlowNode) -> Option<&'b str> {
        self.render_context.node_label_for_render(node)
    }

    pub fn edge_label_for_render<'b>(
        &'b self,
        semantic_index: usize,
        edge: &'b FlowEdge,
    ) -> Option<&'b str> {
        self.render_context
            .edge_label_for_render(semantic_index, edge)
    }

    pub fn edge_label_for_occurrence<'b>(
        &'b self,
        key: FlowchartEdgeKey,
        edge: &'b FlowEdge,
    ) -> Option<&'b str> {
        match key.self_loop_segment_order() {
            None | Some(1) => self.edge_label_for_render(key.semantic_index(), edge),
            Some(_) => None,
        }
    }

    pub fn subgraph_title_for_render<'b>(
        &'b self,
        declaration_ordinal: usize,
        subgraph: &'b FlowSubgraph,
    ) -> &'b str {
        self.render_context
            .subgraph_title_for_render(declaration_ordinal, subgraph)
    }

    pub fn effective_subgraph_css<'b>(
        &'b self,
        declaration_ordinal: usize,
        subgraph: &'b FlowSubgraph,
    ) -> (&'b [String], &'b [String]) {
        self.render_context
            .effective_subgraph_css(declaration_ordinal, subgraph)
    }

    pub fn is_subgraph_collapsed(&self, id: &str) -> bool {
        self.render_context.is_subgraph_collapsed(id)
    }

    pub fn collapsed_replacement(&self, id: &str) -> Option<&str> {
        self.render_context.collapsed_replacement(id)
    }

    pub fn subgraph_color_ordinal(&self, id: &str) -> Option<usize> {
        self.render_context.subgraph_color_ordinal(id)
    }

    /// Parent relations in the graph sent to layout, in FlowDB insertion order.
    pub fn visible_memberships(self) -> impl Iterator<Item = (&'a str, &'a str)> {
        self.semantic
            .subgraphs
            .iter()
            .rev()
            .filter(move |group| {
                self.collapsed_replacement(&group.id).is_none()
                    && !self.is_subgraph_collapsed(&group.id)
            })
            .flat_map(|group| {
                group
                    .nodes
                    .iter()
                    .map(move |child| (child.as_str(), group.id.as_str()))
            })
    }

    pub fn projected_edges(
        self,
    ) -> impl Iterator<Item = (FlowchartEdgeKey, ProjectedFlowchartEdge<'a>)> + Clone {
        self.semantic
            .edges
            .iter()
            .enumerate()
            .filter_map(move |(index, edge)| {
                project_flowchart_edge(edge, self.render_context)
                    .map(|edge| (FlowchartEdgeKey::new(index), edge))
            })
    }
}

pub struct ProjectedFlowchartEdge<'a> {
    pub semantic: &'a FlowEdge,
    pub from: &'a str,
    pub to: &'a str,
}

impl<'a> ProjectedFlowchartEdge<'a> {
    pub fn into_render_edge(self) -> Cow<'a, FlowEdge> {
        if self.from == self.semantic.from && self.to == self.semantic.to {
            return Cow::Borrowed(self.semantic);
        }

        let mut edge = self.semantic.clone();
        self.from.clone_into(&mut edge.from);
        self.to.clone_into(&mut edge.to);
        Cow::Owned(edge)
    }
}

/// Keeps the semantic edge intact while redirecting boundary edges to visible collapsed nodes.
/// Edges that collapse onto one endpoint are hidden, while unchanged self-loops survive.
pub fn project_flowchart_edge<'a>(
    edge: &'a FlowEdge,
    render_context: &'a FlowchartRenderContext,
) -> Option<ProjectedFlowchartEdge<'a>> {
    let from = render_context
        .collapsed_replacement(edge.from.as_str())
        .unwrap_or(edge.from.as_str());
    let to = render_context
        .collapsed_replacement(edge.to.as_str())
        .unwrap_or(edge.to.as_str());
    if from == to && (from != edge.from || to != edge.to) {
        return None;
    }
    Some(ProjectedFlowchartEdge {
        semantic: edge,
        from,
        to,
    })
}

impl Deref for FlowchartRenderModelRef<'_> {
    type Target = FlowchartModel;

    fn deref(&self) -> &Self::Target {
        self.semantic
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn semantic_edge(id: &str) -> FlowEdge {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "from": format!("{id}-from"),
            "to": format!("{id}-to"),
            "length": 1,
        }))
        .unwrap()
    }

    #[test]
    fn transport_plan_preserves_unique_ids_and_avoids_raw_id_collisions() {
        let edges = vec![
            semantic_edge("duplicate"),
            semantic_edge("duplicate"),
            semantic_edge("__merman_flowchart_edge_00000000000000000000"),
            semantic_edge("__merman_flowchart_edge_00000000000000000000_00000000000000000000"),
        ];
        let plan = FlowchartEdgeTransportPlan::for_semantic_edges(&edges);
        let ids = (0..edges.len())
            .map(|index| plan.id(FlowchartEdgeKey::new(index)).expect("transport id"))
            .collect::<Vec<_>>();

        assert_eq!(
            ids[0],
            "__merman_flowchart_edge_00000000000000000000_00000000000000000001"
        );
        assert_eq!(ids[1], "__merman_flowchart_edge_00000000000000000001");
        assert!(std::ptr::eq(ids[2], edges[2].id.as_str()));
        assert!(std::ptr::eq(ids[3], edges[3].id.as_str()));
        assert_eq!(ids.iter().copied().collect::<HashSet<_>>().len(), ids.len());
        assert!(plan.id(FlowchartEdgeKey::new(edges.len())).is_none());
    }
}
