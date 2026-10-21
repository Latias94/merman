use super::model::{FlowEdge, FlowNode, FlowSubgraph};
use indexmap::IndexMap;
use rustc_hash::{FxHashMap, FxHashSet};

#[derive(Debug, Clone, Default)]
pub(crate) struct FlowchartRenderLabelSources {
    nodes: FxHashMap<String, String>,
    edges: Vec<(usize, String)>,
    subgraphs: Vec<Option<FlowSubgraphRenderLabelSource>>,
}

#[derive(Debug, Clone)]
struct FlowSubgraphRenderLabelSource {
    id: String,
    source: String,
}

impl FlowchartRenderLabelSources {
    pub(crate) fn node_label_for_render<'a>(&'a self, node: &'a FlowNode) -> Option<&'a str> {
        self.nodes
            .get(&node.id)
            .map(String::as_str)
            .or(node.label.as_deref())
    }

    pub(crate) fn edge_label_for_render<'a>(
        &'a self,
        semantic_index: usize,
        edge: &'a FlowEdge,
    ) -> Option<&'a str> {
        self.edges
            .binary_search_by_key(&semantic_index, |(index, _)| *index)
            .ok()
            .map(|position| self.edges[position].1.as_str())
            .or(edge.label.as_deref())
    }

    pub(crate) fn subgraph_title_for_render<'a>(
        &'a self,
        declaration_ordinal: usize,
        subgraph: &'a FlowSubgraph,
    ) -> &'a str {
        self.subgraphs
            .get(declaration_ordinal)
            .and_then(Option::as_ref)
            .filter(|source| source.id == subgraph.id)
            .map(|source| source.source.as_str())
            .unwrap_or(subgraph.title.as_str())
    }

    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    pub(crate) fn insert_node(&mut self, id: String, source: String) {
        self.nodes.insert(id, source);
    }

    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    pub(crate) fn insert_edge(&mut self, semantic_index: usize, source: String) {
        if self
            .edges
            .last()
            .is_none_or(|(last_index, _)| *last_index < semantic_index)
        {
            self.edges.push((semantic_index, source));
            return;
        }

        match self
            .edges
            .binary_search_by_key(&semantic_index, |(index, _)| *index)
        {
            Ok(position) => self.edges[position].1 = source,
            Err(position) => self.edges.insert(position, (semantic_index, source)),
        }
    }

    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    pub(crate) fn insert_subgraph(
        &mut self,
        id: String,
        declaration_ordinal: usize,
        source: Option<String>,
    ) {
        if let Some(source) = source {
            if self.subgraphs.len() <= declaration_ordinal {
                self.subgraphs.resize_with(declaration_ordinal + 1, || None);
            }
            self.subgraphs[declaration_ordinal] =
                Some(FlowSubgraphRenderLabelSource { id, source });
        }
    }

    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    pub(crate) fn retained_bytes(&self) -> usize {
        self.nodes
            .iter()
            .fold(0usize, |total, (id, label)| {
                total.saturating_add(id.len()).saturating_add(label.len())
            })
            .saturating_add(
                self.edges
                    .iter()
                    .map(|(_, source)| source.len())
                    .sum::<usize>(),
            )
            .saturating_add(
                self.subgraphs
                    .iter()
                    .flatten()
                    .fold(0usize, |total, source| {
                        total
                            .saturating_add(source.id.len())
                            .saturating_add(source.source.len())
                    }),
            )
    }
}

/// Parser-owned CSS provenance used only while rendering a Flowchart model.
#[derive(Debug, Clone, Default)]
pub(crate) struct FlowchartRenderStyleSources {
    subgraph_vertices: IndexMap<String, FlowSubgraphVertexStyleSource>,
}

impl FlowchartRenderStyleSources {
    /// Returns the CSS sources exposed by Mermaid for the rendered subgraph node.
    pub(crate) fn effective_subgraph_css<'a>(
        &'a self,
        declaration_ordinal: usize,
        subgraph: &'a FlowSubgraph,
    ) -> (&'a [String], &'a [String]) {
        self.subgraph_vertices
            .get(&subgraph.id)
            .filter(|source| source.declaration_ordinal == declaration_ordinal)
            .map_or_else(
                || (subgraph.classes.as_slice(), subgraph.styles.as_slice()),
                |source| {
                    (
                        source.style.classes.as_slice(),
                        source.style.styles.as_slice(),
                    )
                },
            )
    }

    /// Returns whether parsing observed a FlowDB vertex with this subgraph ID.
    pub(crate) fn contains_subgraph_vertex(&self, id: &str) -> bool {
        self.subgraph_vertices.contains_key(id)
    }

    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    pub(crate) fn insert(
        &mut self,
        id: String,
        declaration_ordinal: usize,
        style: FlowSubgraphVertexStyle,
    ) {
        self.subgraph_vertices.insert(
            id,
            FlowSubgraphVertexStyleSource {
                declaration_ordinal,
                style,
            },
        );
    }

    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    pub(crate) fn retained_bytes(&self) -> usize {
        self.subgraph_vertices
            .iter()
            .fold(0usize, |total, (id, source)| {
                source
                    .style
                    .classes
                    .iter()
                    .chain(&source.style.styles)
                    .fold(total.saturating_add(id.len()), |subtotal, value| {
                        subtotal.saturating_add(value.len())
                    })
            })
    }
}

/// Parser-owned render facts that are intentionally absent from [`FlowchartModel`].
#[doc(hidden)]
#[derive(Debug, Clone, Default)]
pub struct FlowchartRenderContext {
    labels: FlowchartRenderLabelSources,
    styles: FlowchartRenderStyleSources,
    collapsed_subgraphs: FxHashSet<String>,
    collapsed_replacements: FxHashMap<String, String>,
    subgraph_color_ordinals: FxHashMap<String, usize>,
    node_dom_indices: FxHashMap<String, usize>,
}

impl FlowchartRenderContext {
    pub(crate) fn new(
        labels: FlowchartRenderLabelSources,
        styles: FlowchartRenderStyleSources,
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

    #[cfg(feature = "diagram-agentflow")]
    pub(crate) fn set_node_dom_indices(
        &mut self,
        indices: impl IntoIterator<Item = (String, usize)>,
    ) {
        self.node_dom_indices = indices.into_iter().collect();
    }

    /// Returns a parser-assigned DOM ordinal when the family owns node registration.
    #[doc(hidden)]
    pub fn node_dom_index(&self, id: &str) -> Option<usize> {
        self.node_dom_indices.get(id).copied()
    }

    #[cfg(feature = "diagram-agentflow")]
    pub(crate) fn set_collapsed_replacements(
        &mut self,
        replacements: impl IntoIterator<Item = (String, String)>,
    ) {
        self.collapsed_replacements = replacements.into_iter().collect();
    }

    #[cfg(feature = "diagram-agentflow")]
    pub(crate) fn set_subgraph_color_ordinals(
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

    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    pub(crate) fn retained_bytes(&self) -> usize {
        self.labels
            .retained_bytes()
            .saturating_add(self.styles.retained_bytes())
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct FlowSubgraphVertexStyle {
    pub(crate) classes: Vec<String>,
    pub(crate) styles: Vec<String>,
}

#[derive(Debug, Clone)]
struct FlowSubgraphVertexStyleSource {
    declaration_ordinal: usize,
    style: FlowSubgraphVertexStyle,
}
