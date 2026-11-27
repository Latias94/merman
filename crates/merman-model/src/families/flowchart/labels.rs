use super::{FlowEdge, FlowNode, FlowSubgraph, FlowchartModel};
use crate::{OperationControl, OperationControlResult};
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Serialize, Serializer};
use std::sync::Arc;

/// Label spellings indexed by canonical identity and encounter ordinal.
///
/// Core retains distinct source spellings for Mermaid markup and native SVG text before the
/// compatibility projection loses entity provenance. Clones share storage with copy-on-write
/// insertion. Independently supplied strings certify no sanitization history.
#[derive(Debug, Clone, Default)]
pub struct FlowchartRenderLabelSources {
    sources: Option<Arc<LabelSources>>,
    svg_text: Option<Arc<LabelSources>>,
}

#[derive(Debug, Clone, Default, Serialize)]
struct LabelSources {
    nodes: FxHashMap<String, String>,
    edges: Vec<(usize, String)>,
    subgraphs: Vec<Option<SubgraphLabelSource>>,
}

/// Prepares literal SVG text while preserving the source's authored line breaks.
#[doc(hidden)]
#[cfg(feature = "html-entities")]
pub fn flowchart_svg_text_source(raw: &str, fallback: &str) -> Option<String> {
    if !raw.contains(['&', '#', 'ﬂ', '¶', '<', '>']) {
        return None;
    }
    let append_text = |text: &str, out: &mut String| {
        for ch in crate::text::decode_mermaid_entities_to_unicode(text).chars() {
            match ch {
                '&' => out.push_str("&amp;"),
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                _ => out.push(ch),
            }
        }
    };
    // Recognize only authored break tokens before expanding entities. Encode the other angle
    // brackets and ampersands for the SVG row tokenizer, which decodes them after tokenization.
    let mut source = String::with_capacity(raw.len());
    let mut copied = 0;
    for (start, _) in raw.match_indices('<') {
        let tail = &raw[start + 1..];
        let name = tail.strip_prefix('/').unwrap_or(tail);
        if !name
            .get(..2)
            .is_some_and(|name| name.eq_ignore_ascii_case("br"))
        {
            continue;
        }
        let tail = name[2..].trim_start_matches(crate::text::is_ecmascript_whitespace);
        let tail = tail.strip_prefix('/').unwrap_or(tail);
        if let Some(after) = tail.strip_prefix('>') {
            append_text(&raw[copied..start], &mut source);
            source.push_str("<br/>");
            copied = raw.len() - after.len();
        }
    }
    append_text(&raw[copied..], &mut source);
    (source != fallback).then_some(source)
}

/// Identifies the owner of an alternate Flowchart label spelling.
#[doc(hidden)]
#[derive(Clone, Copy)]
pub enum FlowchartLabelSourceOwner<'a> {
    Node(&'a str),
    Edge(usize),
    Subgraph(usize, &'a str),
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
    /// Returns whether the serialized markup table is empty. An empty string is still a fact.
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
            .as_deref()?
            .get(FlowchartLabelSourceOwner::Node(node_id))
    }

    /// Returns only the source fact at the edge's semantic ordinal, even if its ID repeats.
    pub fn edge_source(&self, semantic_index: usize) -> Option<&str> {
        self.sources
            .as_deref()?
            .get(FlowchartLabelSourceOwner::Edge(semantic_index))
    }

    /// Returns only a source fact owned by this exact declaration ordinal and subgraph ID.
    pub fn subgraph_source(&self, declaration_ordinal: usize, subgraph_id: &str) -> Option<&str> {
        self.sources
            .as_deref()?
            .get(FlowchartLabelSourceOwner::Subgraph(
                declaration_ordinal,
                subgraph_id,
            ))
    }

    /// Source for native SVG text tokenization. The writer still escapes the visible text.
    #[doc(hidden)]
    pub fn svg_text_source(&self, owner: FlowchartLabelSourceOwner<'_>) -> Option<&str> {
        self.svg_text.as_deref()?.get(owner)
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
        Self::table_mut(&mut self.sources).nodes.insert(id, source);
    }

    pub fn insert_edge(&mut self, semantic_index: usize, source: String) {
        Self::table_mut(&mut self.sources).insert_edge(semantic_index, source);
    }

    pub fn insert_subgraph(
        &mut self,
        id: String,
        declaration_ordinal: usize,
        source: Option<String>,
    ) {
        if let Some(source) = source {
            Self::table_mut(&mut self.sources).insert_subgraph(id, declaration_ordinal, source);
        }
    }

    #[doc(hidden)]
    pub fn insert_svg_text(&mut self, owner: FlowchartLabelSourceOwner<'_>, source: String) {
        let table = Self::table_mut(&mut self.svg_text);
        match owner {
            FlowchartLabelSourceOwner::Node(id) => {
                table.nodes.insert(id.to_owned(), source);
            }
            FlowchartLabelSourceOwner::Edge(index) => table.insert_edge(index, source),
            FlowchartLabelSourceOwner::Subgraph(index, id) => {
                table.insert_subgraph(id.to_owned(), index, source);
            }
        }
    }

    fn table_mut(table: &mut Option<Arc<LabelSources>>) -> &mut LabelSources {
        Arc::make_mut(table.get_or_insert_with(|| Arc::new(LabelSources::default())))
    }

    #[cfg(any(feature = "presentation-data", test))]
    pub(super) fn shares_storage_with(&self, other: &Self) -> bool {
        Self::tables_share_storage(&self.sources, &other.sources)
            && Self::tables_share_storage(&self.svg_text, &other.svg_text)
    }

    #[cfg(any(feature = "presentation-data", test))]
    fn tables_share_storage(
        left: &Option<Arc<LabelSources>>,
        right: &Option<Arc<LabelSources>>,
    ) -> bool {
        match (left, right) {
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
        if let Err(message) = LabelSources::validate_compatible_with_controlled(
            self.svg_text.as_deref(),
            other.svg_text.as_deref(),
            control,
        )? {
            return Ok(Err(message));
        }
        LabelSources::validate_compatible_with_controlled(
            self.sources.as_deref(),
            other.sources.as_deref(),
            control,
        )
    }

    /// Checks node identities, edge semantic ordinals, and subgraph declaration owners.
    pub fn validate_for_model_controlled(
        &self,
        model: &FlowchartModel,
        control: &OperationControl,
    ) -> OperationControlResult<Result<(), &'static str>> {
        control.checkpoint()?;
        if self.sources.is_none() && self.svg_text.is_none() {
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
        for sources in [&self.sources, &self.svg_text].into_iter().flatten() {
            if let Err(message) = sources.validate_with_node_ids(model, node_ids, control)? {
                return Ok(Err(message));
            }
        }
        Ok(Ok(()))
    }

    /// Counts retained string bytes, excluding shared allocation overhead.
    pub fn retained_bytes(&self) -> usize {
        [&self.sources, &self.svg_text]
            .into_iter()
            .flatten()
            .fold(0usize, |total, sources| {
                total.saturating_add(sources.retained_bytes())
            })
    }

    #[cfg(feature = "presentation-data")]
    pub(super) fn unshared_retained_bytes(&self, other: &Self) -> usize {
        [
            (&self.sources, &other.sources),
            (&self.svg_text, &other.svg_text),
        ]
        .into_iter()
        .filter(|(left, right)| !Self::tables_share_storage(left, right))
        .filter_map(|(left, _)| left.as_deref())
        .fold(0usize, |total, sources| {
            total.saturating_add(sources.retained_bytes())
        })
    }
}

impl LabelSources {
    fn get(&self, owner: FlowchartLabelSourceOwner<'_>) -> Option<&str> {
        match owner {
            FlowchartLabelSourceOwner::Node(id) => self.nodes.get(id).map(String::as_str),
            FlowchartLabelSourceOwner::Edge(index) => self
                .edges
                .binary_search_by_key(&index, |(index, _)| *index)
                .ok()
                .map(|position| self.edges[position].1.as_str()),
            FlowchartLabelSourceOwner::Subgraph(index, id) => self
                .subgraphs
                .get(index)
                .and_then(Option::as_ref)
                .filter(|source| source.id == id)
                .map(|source| source.source.as_str()),
        }
    }

    fn insert_edge(&mut self, semantic_index: usize, source: String) {
        let edges = &mut self.edges;
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

    fn insert_subgraph(&mut self, id: String, ordinal: usize, source: String) {
        if self.subgraphs.len() <= ordinal {
            self.subgraphs.resize_with(ordinal + 1, || None);
        }
        self.subgraphs[ordinal] = Some(SubgraphLabelSource { id, source });
    }

    #[cfg(feature = "presentation-data")]
    fn validate_compatible_with_controlled(
        left: Option<&Self>,
        right: Option<&Self>,
        control: &OperationControl,
    ) -> OperationControlResult<Result<(), &'static str>> {
        // Historical caller-authored presentations may keep their only source facts in context.
        let Some(right) = right else {
            return Ok(Ok(()));
        };
        let Some(left) = left else {
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

    fn validate_with_node_ids(
        &self,
        model: &FlowchartModel,
        node_ids: &FxHashSet<&str>,
        control: &OperationControl,
    ) -> OperationControlResult<Result<(), &'static str>> {
        control.checkpoint()?;
        for id in self.nodes.keys() {
            control.checkpoint()?;
            if !node_ids.contains(id.as_str()) {
                return Ok(Err("flowchart node label refers to an unknown node"));
            }
        }
        let mut previous_edge_index = None;
        for (index, _) in &self.edges {
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
        if self.subgraphs.len() > model.subgraphs.len() {
            return Ok(Err(
                "flowchart subgraph label declaration ordinal is out of range",
            ));
        }
        for (ordinal, source) in self.subgraphs.iter().enumerate() {
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

    fn retained_bytes(&self) -> usize {
        self.nodes
            .iter()
            .fold(0usize, |total, (id, label)| {
                total.saturating_add(id.len()).saturating_add(label.len())
            })
            .saturating_add(self.edges.iter().fold(0usize, |total, (_, source)| {
                total.saturating_add(source.len())
            }))
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

#[cfg(test)]
#[path = "labels_tests.rs"]
mod tests;
