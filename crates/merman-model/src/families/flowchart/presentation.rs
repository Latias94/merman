//! Source-faithful labels and frozen Flowchart render facts.

use super::{
    FlowEdge, FlowNode, FlowSubgraph, FlowchartLabelSourceOwner, FlowchartModel,
    FlowchartRenderLabelSources, FlowchartSubgraphStyles,
};
use crate::{OperationControl, OperationControlResult};
use rustc_hash::{FxHashMap, FxHashSet};

/// Frozen render facts and a shared view of canonical authored subgraph CSS.
#[doc(hidden)]
#[derive(Debug, Clone, Default)]
pub struct FlowchartRenderContext {
    labels: FlowchartRenderLabelSources,
    styles: FlowchartSubgraphStyles,
    collapsed_subgraphs: FxHashSet<String>,
    collapsed_replacements: FxHashMap<String, String>,
    subgraph_color_ordinals: FxHashMap<String, usize>,
    node_dom_indices: FxHashMap<String, usize>,
}

impl FlowchartRenderContext {
    /// Inherits canonical labels and CSS without adding collapse facts or source-assigned IDs.
    /// The canonical model already accounts for the shared allocations.
    pub fn from_model(model: &FlowchartModel) -> Self {
        Self {
            labels: model.label_sources.clone(),
            styles: model.subgraph_vertex_styles.clone(),
            ..Self::default()
        }
    }

    /// Checks the relationships represented by these completed output facts.
    ///
    /// The caller admits model complexity and retained context before allocating these linear
    /// indexes. Registration history and hidden-container ancestry cannot be recovered from the
    /// projected graph; this method does not authenticate that history or impose a DOM upper bound.
    pub fn validate_for_model_controlled(
        &self,
        model: &FlowchartModel,
        control: &OperationControl,
    ) -> OperationControlResult<Result<(), &'static str>> {
        control.checkpoint()?;
        let mut node_ids = FxHashSet::default();
        for node in &model.nodes {
            control.checkpoint()?;
            node_ids.insert(node.id.as_str());
        }
        let mut subgraph_ids = FxHashSet::default();
        for subgraph in &model.subgraphs {
            control.checkpoint()?;
            subgraph_ids.insert(subgraph.id.as_str());
        }

        if let Err(message) = model
            .label_sources
            .validate_with_node_ids(model, &node_ids, control)?
        {
            return Ok(Err(message));
        }
        if !self.labels.shares_storage_with(&model.label_sources) {
            if let Err(message) = self
                .labels
                .validate_with_node_ids(model, &node_ids, control)?
            {
                return Ok(Err(message));
            }
            if let Err(message) = self
                .labels
                .validate_compatible_with_controlled(&model.label_sources, control)?
            {
                return Ok(Err(message));
            }
        }
        if let Err(message) = model
            .subgraph_vertex_styles
            .validate_for_model_controlled(model, control)?
        {
            return Ok(Err(message));
        }
        if let Err(message) = self.styles.validate_for_model_controlled(model, control)? {
            return Ok(Err(message));
        }

        for id in &self.collapsed_subgraphs {
            control.checkpoint()?;
            if !subgraph_ids.contains(id.as_str()) {
                return Ok(Err(
                    "flowchart collapsed subgraph refers to an unknown subgraph",
                ));
            }
        }
        for (id, replacement) in &self.collapsed_replacements {
            control.checkpoint()?;
            // All canonical nodes remain in the identity domain even when collapse hides them.
            if !node_ids.contains(id.as_str()) && !subgraph_ids.contains(id.as_str()) {
                return Ok(Err(
                    "flowchart collapsed replacement refers to an unknown node or subgraph",
                ));
            }
            if !self.collapsed_subgraphs.contains(replacement) {
                return Ok(Err(
                    "flowchart collapsed replacement target is not a collapsed subgraph",
                ));
            }
            if id == replacement {
                return Ok(Err("flowchart collapsed replacement targets itself"));
            }
        }

        let mut palette_ordinals = FxHashSet::default();
        for (id, ordinal) in &self.subgraph_color_ordinals {
            control.checkpoint()?;
            if !subgraph_ids.contains(id.as_str()) {
                return Ok(Err(
                    "flowchart palette ordinal refers to an unknown subgraph",
                ));
            }
            if *ordinal >= subgraph_ids.len() {
                return Ok(Err("flowchart palette ordinal is out of range"));
            }
            if !palette_ordinals.insert(*ordinal) {
                return Ok(Err("flowchart palette ordinals must be distinct"));
            }
        }
        let mut dom_ordinals = FxHashSet::default();
        for (id, ordinal) in &self.node_dom_indices {
            control.checkpoint()?;
            if !node_ids.contains(id.as_str()) && !subgraph_ids.contains(id.as_str()) {
                return Ok(Err(
                    "flowchart DOM ordinal refers to an unknown node or subgraph",
                ));
            }
            if !dom_ordinals.insert(*ordinal) {
                return Ok(Err("flowchart DOM ordinals must be distinct"));
            }
        }
        control.checkpoint()?;
        Ok(Ok(()))
    }

    /// Freezes label, CSS, and collapse facts for the supplied canonical subgraph order.
    pub fn new(
        labels: FlowchartRenderLabelSources,
        styles: FlowchartSubgraphStyles,
        collapsed_subgraphs: FxHashSet<String>,
        subgraphs: &[FlowSubgraph],
    ) -> Self {
        let collapsed_replacements = build_collapsed_replacements(&collapsed_subgraphs, subgraphs);
        Self {
            labels,
            styles,
            collapsed_subgraphs,
            collapsed_replacements,
            subgraph_color_ordinals: build_subgraph_color_ordinals(subgraphs),
            node_dom_indices: FxHashMap::default(),
        }
    }

    /// Replaces the family-assigned DOM ordinals as one complete indexed data set.
    pub fn set_node_dom_indices(&mut self, indices: impl IntoIterator<Item = (String, usize)>) {
        self.node_dom_indices = indices.into_iter().collect();
    }

    /// Returns a parser-assigned DOM ordinal when the family owns node registration.
    #[doc(hidden)]
    pub fn node_dom_index(&self, id: &str) -> Option<usize> {
        self.node_dom_indices.get(id).copied()
    }

    /// Replaces the visible-container mapping with the family's complete frozen result.
    pub fn set_collapsed_replacements(
        &mut self,
        replacements: impl IntoIterator<Item = (String, String)>,
    ) {
        self.collapsed_replacements = replacements.into_iter().collect();
    }

    /// Replaces the palette ordinals with the family's complete frozen result.
    pub fn set_subgraph_color_ordinals(
        &mut self,
        ordinals: impl IntoIterator<Item = (String, usize)>,
    ) {
        self.subgraph_color_ordinals = ordinals.into_iter().collect();
    }

    /// Returns the container's palette ordinal, independent of model storage order.
    #[doc(hidden)]
    pub fn subgraph_color_ordinal(&self, id: &str) -> Option<usize> {
        self.subgraph_color_ordinals.get(id).copied()
    }

    #[doc(hidden)]
    pub fn node_label_for_render<'a>(&'a self, node: &'a FlowNode) -> Option<&'a str> {
        self.labels.node_label_for_render(node)
    }

    #[doc(hidden)]
    pub fn svg_text_source(&self, owner: FlowchartLabelSourceOwner<'_>) -> Option<&str> {
        self.labels.svg_text_source(owner)
    }

    #[doc(hidden)]
    /// Returns the source-faithful label for one edge at its semantic model ordinal.
    ///
    /// The ordinal is required because Mermaid permits duplicate edge ids.
    pub fn edge_label_for_render<'a>(
        &'a self,
        semantic_index: usize,
        edge: &'a FlowEdge,
    ) -> Option<&'a str> {
        self.labels.edge_label_for_render(semantic_index, edge)
    }

    #[doc(hidden)]
    pub fn subgraph_title_for_render<'a>(
        &'a self,
        declaration_ordinal: usize,
        subgraph: &'a FlowSubgraph,
    ) -> &'a str {
        self.labels
            .subgraph_title_for_render(declaration_ordinal, subgraph)
    }

    #[doc(hidden)]
    pub fn effective_subgraph_css<'a>(
        &'a self,
        declaration_ordinal: usize,
        subgraph: &'a FlowSubgraph,
    ) -> (&'a [String], &'a [String]) {
        self.styles
            .effective_subgraph_css(declaration_ordinal, subgraph)
    }

    #[doc(hidden)]
    pub fn contains_subgraph_vertex(&self, id: &str) -> bool {
        self.styles.contains_subgraph_vertex(id)
    }

    /// Returns whether the parser attached Mermaid's collapsed view to this subgraph.
    #[doc(hidden)]
    pub fn is_subgraph_collapsed(&self, id: &str) -> bool {
        self.collapsed_subgraphs.contains(id)
    }

    /// Returns the visible collapsed subgraph that replaces this node, when the node is hidden
    /// beneath a collapsed subgraph. The visible collapsed subgraph itself is not replaced.
    #[doc(hidden)]
    pub fn collapsed_replacement(&self, id: &str) -> Option<&str> {
        self.collapsed_replacements.get(id).map(String::as_str)
    }

    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes_with_labels(self.labels.retained_bytes())
    }

    fn retained_bytes_with_labels(&self, label_bytes: usize) -> usize {
        label_bytes
            .saturating_add(
                self.collapsed_subgraphs
                    .iter()
                    .map(String::len)
                    .sum::<usize>(),
            )
            .saturating_add(self.collapsed_replacements.iter().fold(
                0usize,
                |total, (id, replacement)| {
                    total
                        .saturating_add(id.len())
                        .saturating_add(replacement.len())
                },
            ))
            .saturating_add(
                self.subgraph_color_ordinals
                    .keys()
                    .chain(self.node_dom_indices.keys())
                    .fold(0usize, |total, id| {
                        total
                            .saturating_add(id.len())
                            .saturating_add(std::mem::size_of::<usize>())
                    }),
            )
    }

    /// Counts context text not already owned by this model's shared label allocation.
    pub fn retained_bytes_for_model(&self, model: &FlowchartModel) -> usize {
        self.retained_bytes_with_labels(self.labels.unshared_retained_bytes(&model.label_sources))
    }
}

fn build_subgraph_color_ordinals(subgraphs: &[FlowSubgraph]) -> FxHashMap<String, usize> {
    let ids: FxHashSet<&str> = subgraphs.iter().map(|graph| graph.id.as_str()).collect();
    let mut parents = FxHashMap::default();
    for graph in subgraphs {
        for child in &graph.nodes {
            if ids.contains(child.as_str()) {
                parents.insert(child.as_str(), graph.id.as_str());
            }
        }
    }
    let mut children: FxHashMap<&str, Vec<&str>> = FxHashMap::default();
    for graph in subgraphs {
        if let Some(&parent) = parents.get(graph.id.as_str()) {
            children.entry(parent).or_default().push(graph.id.as_str());
        }
    }
    // Jison stores containers when they close. A containment preorder restores declaration
    // order and keeps each slot stable when the same container is rendered collapsed.
    let mut pending: Vec<_> = subgraphs
        .iter()
        .rev()
        .filter(|graph| !parents.contains_key(graph.id.as_str()))
        .map(|graph| graph.id.as_str())
        .collect();
    let mut ordinals = FxHashMap::default();
    while let Some(id) = pending.pop() {
        if ordinals.contains_key(id) {
            continue;
        }
        ordinals.insert(id.to_owned(), ordinals.len());
        if let Some(children) = children.get(id) {
            pending.extend(children.iter().rev().copied());
        }
    }
    ordinals
}

fn build_collapsed_replacements(
    collapsed_subgraphs: &FxHashSet<String>,
    subgraphs: &[FlowSubgraph],
) -> FxHashMap<String, String> {
    let subgraph_ids: FxHashSet<&str> = subgraphs.iter().map(|sg| sg.id.as_str()).collect();
    let mut parent_by_id: FxHashMap<&str, &str> = FxHashMap::default();
    for subgraph in subgraphs {
        for child in &subgraph.nodes {
            if subgraph_ids.contains(child.as_str()) {
                parent_by_id.insert(child.as_str(), subgraph.id.as_str());
            }
        }
    }

    fn outermost_collapsed<'a>(
        id: &'a str,
        collapsed_subgraphs: &FxHashSet<String>,
        parent_by_id: &FxHashMap<&'a str, &'a str>,
    ) -> Option<&'a str> {
        let mut current = Some(id);
        let mut result = None;
        let mut seen: FxHashSet<&str> = FxHashSet::default();
        while let Some(candidate) = current {
            if !seen.insert(candidate) {
                break;
            }
            if collapsed_subgraphs.contains(candidate) {
                result = Some(candidate);
            }
            current = parent_by_id.get(candidate).copied();
        }
        result
    }

    let mut replacements = FxHashMap::default();
    for subgraph in subgraphs {
        let Some(ancestor) =
            outermost_collapsed(subgraph.id.as_str(), collapsed_subgraphs, &parent_by_id)
        else {
            continue;
        };
        if subgraph.id != ancestor {
            replacements.insert(subgraph.id.clone(), ancestor.to_string());
        }
        for child in &subgraph.nodes {
            if child != ancestor {
                replacements.insert(child.clone(), ancestor.to_string());
            }
        }
    }
    replacements
}

#[cfg(test)]
#[path = "presentation_tests.rs"]
mod tests;
