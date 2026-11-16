use super::{FlowEdge, FlowNode, FlowSubgraph, FlowchartModel};
use crate::{OperationControl, OperationControlResult};
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Serialize, Serializer};
use std::sync::Arc;

/// Sanitized label spellings indexed by canonical identity and encounter ordinal.
///
/// Core records these before the compatibility projection loses entity provenance. They are
/// markup sources, not decoded display text. Clones share storage; insertion uses copy on write.
/// Independently supplied strings remain caller-owned data and certify no sanitization history.
#[derive(Debug, Clone, Default)]
pub struct FlowchartRenderLabelSources {
    sources: Option<Arc<LabelSources>>,
}

#[derive(Debug, Clone, Default, Serialize)]
struct LabelSources {
    nodes: FxHashMap<String, String>,
    edges: Vec<(usize, String)>,
    subgraphs: Vec<Option<SubgraphLabelSource>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct SubgraphLabelSource {
    id: String,
    source: String,
}

/// Explicit source-table projection for consumers that retain labels beyond canonical JSON.
/// `FlowchartModel` deliberately skips this field in its own serialization.
impl Serialize for FlowchartRenderLabelSources {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.sources.as_deref() {
            Some(sources) => sources.serialize(serializer),
            None => LabelSources::default().serialize(serializer),
        }
    }
}

impl FlowchartRenderLabelSources {
    /// Returns whether no source facts are retained. An empty-string source is still a fact.
    pub fn is_empty(&self) -> bool {
        self.sources.as_deref().is_none_or(|sources| {
            sources.nodes.is_empty()
                && sources.edges.is_empty()
                && sources.subgraphs.iter().all(Option::is_none)
        })
    }

    /// Returns only a retained source fact, without interpreting a canonical label as markup.
    pub fn node_source(&self, node_id: &str) -> Option<&str> {
        self.sources
            .as_deref()
            .and_then(|sources| sources.nodes.get(node_id))
            .map(String::as_str)
    }

    /// Returns only the source fact at the edge's semantic ordinal, even if its ID repeats.
    pub fn edge_source(&self, semantic_index: usize) -> Option<&str> {
        self.sources.as_deref().and_then(|sources| {
            sources
                .edges
                .binary_search_by_key(&semantic_index, |(index, _)| *index)
                .ok()
                .map(|position| sources.edges[position].1.as_str())
        })
    }

    /// Returns only a source fact owned by this exact declaration ordinal and subgraph ID.
    pub fn subgraph_source(&self, declaration_ordinal: usize, subgraph_id: &str) -> Option<&str> {
        self.sources
            .as_deref()
            .and_then(|sources| sources.subgraphs.get(declaration_ordinal))
            .and_then(Option::as_ref)
            .filter(|source| source.id == subgraph_id)
            .map(|source| source.source.as_str())
    }

    pub fn node_label_for_render<'a>(&'a self, node: &'a FlowNode) -> Option<&'a str> {
        self.node_source(&node.id).or(node.label.as_deref())
    }

    pub fn edge_label_for_render<'a>(
        &'a self,
        semantic_index: usize,
        edge: &'a FlowEdge,
    ) -> Option<&'a str> {
        self.edge_source(semantic_index).or(edge.label.as_deref())
    }

    pub fn subgraph_title_for_render<'a>(
        &'a self,
        declaration_ordinal: usize,
        subgraph: &'a FlowSubgraph,
    ) -> &'a str {
        self.subgraph_source(declaration_ordinal, &subgraph.id)
            .unwrap_or(subgraph.title.as_str())
    }

    pub fn insert_node(&mut self, id: String, source: String) {
        self.sources_mut().nodes.insert(id, source);
    }

    pub fn insert_edge(&mut self, semantic_index: usize, source: String) {
        let edges = &mut self.sources_mut().edges;
        if edges
            .last()
            .is_none_or(|(last_index, _)| *last_index < semantic_index)
        {
            edges.push((semantic_index, source));
            return;
        }

        match edges.binary_search_by_key(&semantic_index, |(index, _)| *index) {
            Ok(position) => edges[position].1 = source,
            Err(position) => edges.insert(position, (semantic_index, source)),
        }
    }

    pub fn insert_subgraph(
        &mut self,
        id: String,
        declaration_ordinal: usize,
        source: Option<String>,
    ) {
        if let Some(source) = source {
            let subgraphs = &mut self.sources_mut().subgraphs;
            if subgraphs.len() <= declaration_ordinal {
                subgraphs.resize_with(declaration_ordinal + 1, || None);
            }
            subgraphs[declaration_ordinal] = Some(SubgraphLabelSource { id, source });
        }
    }

    fn sources_mut(&mut self) -> &mut LabelSources {
        Arc::make_mut(
            self.sources
                .get_or_insert_with(|| Arc::new(LabelSources::default())),
        )
    }

    #[cfg(any(feature = "presentation-data", test))]
    pub(super) fn shares_storage_with(&self, other: &Self) -> bool {
        match (&self.sources, &other.sources) {
            (Some(left), Some(right)) => Arc::ptr_eq(left, right),
            (None, None) => true,
            _ => false,
        }
    }

    #[cfg(feature = "presentation-data")]
    pub(super) fn validate_compatible_with_controlled(
        &self,
        other: &Self,
        control: &OperationControl,
    ) -> OperationControlResult<Result<(), &'static str>> {
        control.checkpoint()?;
        // Historical caller-authored presentations may keep their only source facts in context.
        if other.is_empty() {
            return Ok(Ok(()));
        }
        let (Some(left), Some(right)) = (&self.sources, &other.sources) else {
            return Ok(Err(
                "flowchart label sources differ between model and render context",
            ));
        };
        if left.nodes.len() != right.nodes.len() {
            return Ok(Err(
                "flowchart node label sources differ between model and render context",
            ));
        }
        for (id, source) in &left.nodes {
            control.checkpoint()?;
            if right.nodes.get(id) != Some(source) {
                return Ok(Err(
                    "flowchart node label sources differ between model and render context",
                ));
            }
        }
        if left.edges.len() != right.edges.len() {
            return Ok(Err(
                "flowchart edge label sources differ between model and render context",
            ));
        }
        for (left, right) in left.edges.iter().zip(&right.edges) {
            control.checkpoint()?;
            if left != right {
                return Ok(Err(
                    "flowchart edge label sources differ between model and render context",
                ));
            }
        }
        if left.subgraphs.len() != right.subgraphs.len() {
            return Ok(Err(
                "flowchart subgraph label sources differ between model and render context",
            ));
        }
        for (left, right) in left.subgraphs.iter().zip(&right.subgraphs) {
            control.checkpoint()?;
            if left != right {
                return Ok(Err(
                    "flowchart subgraph label sources differ between model and render context",
                ));
            }
        }
        control.checkpoint()?;
        Ok(Ok(()))
    }

    /// Checks node identities, edge semantic ordinals, and subgraph declaration owners.
    pub fn validate_for_model_controlled(
        &self,
        model: &FlowchartModel,
        control: &OperationControl,
    ) -> OperationControlResult<Result<(), &'static str>> {
        control.checkpoint()?;
        if self.sources.is_none() {
            return Ok(Ok(()));
        }
        let mut node_ids = FxHashSet::default();
        for node in &model.nodes {
            control.checkpoint()?;
            node_ids.insert(node.id.as_str());
        }
        self.validate_with_node_ids(model, &node_ids, control)
    }

    pub(super) fn validate_with_node_ids(
        &self,
        model: &FlowchartModel,
        node_ids: &FxHashSet<&str>,
        control: &OperationControl,
    ) -> OperationControlResult<Result<(), &'static str>> {
        control.checkpoint()?;
        let Some(sources) = &self.sources else {
            return Ok(Ok(()));
        };
        for id in sources.nodes.keys() {
            control.checkpoint()?;
            if !node_ids.contains(id.as_str()) {
                return Ok(Err("flowchart node label refers to an unknown node"));
            }
        }
        let mut previous_edge_index = None;
        for (index, _) in &sources.edges {
            control.checkpoint()?;
            if *index >= model.edges.len() {
                return Ok(Err("flowchart edge label semantic index is out of range"));
            }
            if previous_edge_index.is_some_and(|previous| previous >= *index) {
                return Ok(Err(
                    "flowchart edge label semantic indices are not strictly increasing",
                ));
            }
            previous_edge_index = Some(*index);
        }
        if sources.subgraphs.len() > model.subgraphs.len() {
            return Ok(Err(
                "flowchart subgraph label declaration ordinal is out of range",
            ));
        }
        for (ordinal, source) in sources.subgraphs.iter().enumerate() {
            control.checkpoint()?;
            if let Some(source) = source
                && model.subgraphs[ordinal].id != source.id
            {
                return Ok(Err(
                    "flowchart subgraph label ID does not match its declaration ordinal",
                ));
            }
        }
        control.checkpoint()?;
        Ok(Ok(()))
    }

    /// Counts retained string bytes, excluding shared allocation overhead.
    pub fn retained_bytes(&self) -> usize {
        let Some(sources) = &self.sources else {
            return 0;
        };
        sources
            .nodes
            .iter()
            .fold(0usize, |total, (id, label)| {
                total.saturating_add(id.len()).saturating_add(label.len())
            })
            .saturating_add(sources.edges.iter().fold(0usize, |total, (_, source)| {
                total.saturating_add(source.len())
            }))
            .saturating_add(
                sources
                    .subgraphs
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

#[cfg(test)]
#[path = "labels_tests.rs"]
mod tests;
