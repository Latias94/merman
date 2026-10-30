//! Flat Block data, palette facts, and declared compatibility field presence.

#[cfg(feature = "compat-json")]
pub mod compat;

use crate::resources::ModelComplexity;
use crate::warnings::DiagramWarningFact;
use indexmap::IndexMap;

#[cfg(all(test, feature = "presentation-data"))]
mod tests;

/// Flat Block records in Mermaid database insertion order.
///
/// Child indices address `blocks_flat`; compatibility serialization expands the nested wire shape.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct BlockDiagramModel {
    pub blocks_flat: Vec<BlockNode>,
    pub edges: Vec<BlockEdge>,
    pub warning_facts: Vec<DiagramWarningFact>,
    pub class_defs: IndexMap<String, BlockClassDef>,
    pub root: Option<usize>,
}

#[cfg(feature = "presentation-data")]
#[derive(Clone, Copy, Default)]
struct BlockRecordMetrics {
    items: usize,
    text_bytes: usize,
    container_depth: usize,
}

impl BlockDiagramModel {
    /// Returns the canonical root composite, when present.
    pub fn root(&self) -> Option<&BlockNode> {
        self.root.and_then(|index| self.blocks_flat.get(index))
    }

    /// Resolves a direct child index without constructing a subtree.
    pub fn block(&self, index: usize) -> Option<&BlockNode> {
        self.blocks_flat.get(index)
    }

    #[cfg(feature = "presentation-data")]
    fn record_metrics(&self) -> std::result::Result<Vec<BlockRecordMetrics>, String> {
        let mut metrics = vec![BlockRecordMetrics::default(); self.blocks_flat.len()];
        let mut state = vec![0u8; self.blocks_flat.len()];
        for start in 0..self.blocks_flat.len() {
            let mut stack = vec![(start, false)];
            while let Some((index, complete)) = stack.pop() {
                let Some(block) = self.blocks_flat.get(index) else {
                    return Err(format!("invalid Block child index {index}"));
                };
                if complete {
                    let mut value = BlockRecordMetrics {
                        items: block
                            .classes
                            .len()
                            .saturating_add(block.styles.len())
                            .saturating_add(block.directions.len())
                            .saturating_add(block.children.len()),
                        text_bytes: block
                            .id
                            .len()
                            .saturating_add(block.label.len())
                            .saturating_add(block.block_type.len()),
                        container_depth: 2,
                    };
                    for text in block
                        .classes
                        .iter()
                        .chain(&block.styles)
                        .chain(&block.directions)
                    {
                        value.text_bytes = value.text_bytes.saturating_add(text.len());
                    }
                    for &child in &block.children {
                        value.items = value.items.saturating_add(metrics[child].items);
                        value.text_bytes =
                            value.text_bytes.saturating_add(metrics[child].text_bytes);
                        value.container_depth = value
                            .container_depth
                            .max(metrics[child].container_depth.saturating_add(2));
                    }
                    metrics[index] = value;
                    state[index] = 2;
                } else if state[index] != 2 {
                    if state[index] == 1 {
                        return Err("cyclic Block child indices".to_string());
                    }
                    state[index] = 1;
                    stack.push((index, true));
                    stack.extend(block.children.iter().rev().map(|&child| (child, false)));
                }
            }
        }
        Ok(metrics)
    }

    /// Checks flat child references before an upper owner expands nested output.
    #[cfg(feature = "presentation-data")]
    pub fn validate_child_relationships(&self) -> Result<(), String> {
        self.record_metrics().map(|_| ())
    }

    /// Counts the previous typed wire shape without allocating repeated descendant output.
    #[cfg(feature = "presentation-data")]
    fn wire_model_complexity(&self) -> ModelComplexity {
        let Ok(metrics) = self.record_metrics() else {
            return ModelComplexity::new(usize::MAX, usize::MAX, usize::MAX);
        };
        let mut result = ModelComplexity::new(self.blocks_flat.len(), 0, 1);
        for value in metrics {
            result.items = result.items.saturating_add(value.items);
            result.text_bytes = result.text_bytes.saturating_add(value.text_bytes);
            result.nesting_depth = result
                .nesting_depth
                .max(value.container_depth.saturating_add(1));
        }
        for (value, empty) in [
            (
                ModelComplexity::from_serializable(&self.edges),
                self.edges.is_empty(),
            ),
            (
                ModelComplexity::from_serializable(&self.class_defs),
                self.class_defs.is_empty(),
            ),
            (
                ModelComplexity::from_serializable(&self.warning_facts),
                self.warning_facts.is_empty(),
            ),
        ] {
            result.items = result
                .items
                .saturating_add(if empty { 0 } else { value.items });
            result.text_bytes = result.text_bytes.saturating_add(value.text_bytes);
            result.nesting_depth = result
                .nesting_depth
                .max(value.nesting_depth.saturating_add(1));
        }
        result.items = result.items.max(1);
        result
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub struct BlockClassDef {
    pub id: String,
    #[serde(default)]
    pub styles: Vec<String>,
    #[serde(default, rename = "textStyles")]
    pub text_styles: Vec<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct BlockNode {
    pub id: String,
    pub label: String,
    pub block_type: String,
    /// Indices of direct children in the owning model's `blocks_flat` records.
    pub children: Vec<usize>,
    pub columns: Option<i64>,
    pub width_in_columns: Option<i64>,
    pub width: Option<i64>,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
    pub directions: Vec<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct BlockEdge {
    pub id: String,
    pub start: String,
    pub end: String,
    #[serde(default, rename = "arrowTypeEnd")]
    pub arrow_type_end: Option<String>,
    #[serde(default, rename = "arrowTypeStart")]
    pub arrow_type_start: Option<String>,
    #[serde(default)]
    pub label: String,
}

impl BlockClassDef {
    /// Creates an empty class definition with its canonical identifier.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            ..Self::default()
        }
    }
}

/// Declares whether an optional compatibility array is emitted.
///
/// Manual models use their nonempty data unless explicit field presence is supplied.
#[cfg(feature = "presentation-data")]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BlockFieldPresence {
    #[default]
    Model,
    Omitted,
    Present,
}

#[cfg(feature = "presentation-data")]
impl BlockFieldPresence {
    pub fn from_option<T>(value: &Option<T>) -> Self {
        if value.is_some() {
            Self::Present
        } else {
            Self::Omitted
        }
    }

    pub fn is_present(self, model_nonempty: bool) -> bool {
        match self {
            Self::Model => model_nonempty,
            Self::Omitted => false,
            Self::Present => true,
        }
    }
}

#[cfg(feature = "presentation-data")]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BlockNodeFieldPresence {
    pub styles: BlockFieldPresence,
    pub directions: BlockFieldPresence,
}

/// Optional declared output facts, indexed in canonical node and edge order.
///
/// These facts preserve empty versus omitted arrays; they do not certify source admission.
#[cfg(feature = "presentation-data")]
#[derive(Debug, Clone, Default)]
pub enum BlockProjectionFacts {
    #[default]
    Model,
    Explicit {
        nodes: Vec<BlockNodeFieldPresence>,
        edge_directions: Vec<Option<Vec<String>>>,
    },
}

#[cfg(feature = "presentation-data")]
#[derive(Debug, Clone, Default)]
pub struct BlockVisuals {
    /// Palette slots in canonical insertion order; the root has no slot.
    pub color_indices: Vec<Option<usize>>,
}

#[cfg(feature = "presentation-data")]
#[derive(Debug, Clone, Default)]
pub struct BlockDiagramRenderModel {
    pub semantic: BlockDiagramModel,
    pub visuals: BlockVisuals,
    projection_facts: BlockProjectionFacts,
}

#[cfg(feature = "presentation-data")]
impl std::ops::Deref for BlockDiagramRenderModel {
    type Target = BlockDiagramModel;
    fn deref(&self) -> &Self::Target {
        &self.semantic
    }
}

#[cfg(feature = "presentation-data")]
impl std::ops::DerefMut for BlockDiagramRenderModel {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.semantic
    }
}

#[cfg(feature = "presentation-data")]
impl BlockDiagramRenderModel {
    pub fn root(&self) -> Option<&BlockNode> {
        self.semantic.root().or_else(|| {
            self.semantic
                .blocks_flat
                .iter()
                .find(|block| block.id == "root")
        })
    }

    pub fn from_semantic_model(semantic: BlockDiagramModel) -> Self {
        Self {
            semantic,
            ..Self::default()
        }
    }

    /// Assembles data and declared output facts without granting source provenance.
    pub fn from_parts(
        semantic: BlockDiagramModel,
        visuals: BlockVisuals,
        projection_facts: BlockProjectionFacts,
    ) -> Self {
        Self {
            semantic,
            visuals,
            projection_facts,
        }
    }

    pub fn projection_facts(&self) -> &BlockProjectionFacts {
        &self.projection_facts
    }

    pub fn node_field_presence(&self, index: usize) -> BlockNodeFieldPresence {
        match &self.projection_facts {
            BlockProjectionFacts::Model => BlockNodeFieldPresence::default(),
            BlockProjectionFacts::Explicit { nodes, .. } => {
                nodes.get(index).copied().unwrap_or_default()
            }
        }
    }

    pub fn edge_directions(&self, index: usize) -> Option<&[String]> {
        match &self.projection_facts {
            BlockProjectionFacts::Model => None,
            BlockProjectionFacts::Explicit {
                edge_directions, ..
            } => edge_directions.get(index).and_then(Option::as_deref),
        }
    }

    pub fn model_complexity(&self) -> ModelComplexity {
        self.semantic.wire_model_complexity()
    }
}

pub fn model_complexity_from_block(model: &BlockDiagramModel) -> ModelComplexity {
    let mut complexity = ModelComplexity::from_serializable(model);
    let mut pending = model
        .root
        .map(|root| vec![(root, 0usize)])
        .unwrap_or_default();
    let mut visited = vec![false; model.blocks_flat.len()];
    while let Some((index, depth)) = pending.pop() {
        let Some(node) = model.blocks_flat.get(index) else {
            continue;
        };
        if std::mem::replace(&mut visited[index], true) {
            continue;
        }
        complexity.nesting_depth = complexity.nesting_depth.max(depth);
        pending.extend(
            node.children
                .iter()
                .map(|child| (*child, depth.saturating_add(1))),
        );
    }
    complexity
}
