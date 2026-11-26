//! Core Dagre label types and geometry primitives.
//!
//! These are intentionally lightweight and `Clone`-friendly to support deterministic tests and
//! parity-oriented porting from upstream JS.

use crate::graphlib::EdgeKey;
use serde_json::Value;
use std::collections::BTreeMap;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RankDir {
    #[default]
    TB,
    BT,
    LR,
    RL,
}

#[derive(Debug, Clone)]
pub struct GraphLabel {
    pub rankdir: RankDir,
    pub nodesep: f64,
    pub ranksep: f64,
    pub edgesep: f64,
    pub marginx: f64,
    pub marginy: f64,
    pub width: f64,
    pub height: f64,
    pub align: Option<String>,
    pub ranker: Option<String>,
    pub acyclicer: Option<String>,
    pub dummy_chains: Vec<String>,
    pub nesting_root: Option<String>,
    pub node_rank_factor: Option<usize>,
}

impl Default for GraphLabel {
    fn default() -> Self {
        Self {
            rankdir: RankDir::TB,
            nodesep: 50.0,
            ranksep: 50.0,
            edgesep: 20.0,
            marginx: 0.0,
            marginy: 0.0,
            width: 0.0,
            height: 0.0,
            align: None,
            ranker: None,
            acyclicer: None,
            dummy_chains: Vec::new(),
            nesting_root: None,
            node_rank_factor: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NodeLabel {
    pub width: f64,
    pub height: f64,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub rank: Option<i32>,
    pub order: Option<usize>,
    pub dummy: Option<DummyKind>,
    pub labelpos: Option<LabelPos>,
    pub dummy_edge: DummyEdge,
    pub border_type: Option<BorderSide>,
    /// Rank and border state materialized by the compound layout stages.
    pub compound: Option<Box<CompoundNodeLabel>>,
    pub self_edges: Vec<OriginalEdge>,
}

impl NodeLabel {
    pub(crate) fn compound_mut(&mut self) -> &mut CompoundNodeLabel {
        self.compound.get_or_insert_with(Box::default)
    }
}

/// Rank bounds and border nodes for a compound graph node.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CompoundNodeLabel {
    pub min_rank: Option<i32>,
    pub max_rank: Option<i32>,
    pub border_left: Vec<Option<String>>,
    pub border_right: Vec<Option<String>>,
    pub border_top: Option<String>,
    pub border_bottom: Option<String>,
}

/// The role of a temporary Dagre node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DummyKind {
    Edge,
    EdgeLabel,
    EdgeProxy,
    Border,
    NestingRoot,
    SelfEdge,
    /// A generic dummy used by callers of the individual layout stages.
    Other,
}

/// The side of a rank's compound border segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderSide {
    Left,
    Right,
}

/// The original edge data needed by a temporary layout node.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum DummyEdge {
    /// Ordinary nodes and interior chain dummies own no original edge data.
    #[default]
    None,
    /// A rank proxy only needs to identify the edge receiving its rank.
    RankProxy(Box<EdgeKey>),
    /// A chain head or positioned self-loop dummy owns the edge it will restore.
    Original(Box<OriginalEdge>),
}

impl DummyEdge {
    pub(crate) fn original(&self) -> Option<&OriginalEdge> {
        match self {
            Self::Original(edge) => Some(edge),
            Self::None | Self::RankProxy(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LabelPos {
    #[default]
    C,
    L,
    R,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// Optional edge metadata with value semantics and copy-on-write storage.
///
/// Reading or cloning empty metadata does not allocate. Clones of populated metadata share its
/// storage until one is mutated.
#[derive(Debug, Clone, Default)]
pub struct EdgeExtras(Option<Arc<BTreeMap<String, Value>>>);

impl Deref for EdgeExtras {
    type Target = BTreeMap<String, Value>;

    fn deref(&self) -> &Self::Target {
        static EMPTY: BTreeMap<String, Value> = BTreeMap::new();
        self.0.as_deref().unwrap_or(&EMPTY)
    }
}

impl DerefMut for EdgeExtras {
    fn deref_mut(&mut self) -> &mut Self::Target {
        Arc::make_mut(self.0.get_or_insert_with(Arc::default))
    }
}

impl PartialEq for EdgeExtras {
    fn eq(&self, other: &Self) -> bool {
        self.deref() == other.deref()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EdgeLabel {
    pub width: f64,
    pub height: f64,
    pub labelpos: LabelPos,
    pub labeloffset: f64,
    pub label_rank: Option<i32>,
    pub minlen: usize,
    pub weight: f64,
    pub nesting_edge: bool,
    pub reversed: bool,
    pub forward_name: Option<String>,
    /// An adapter-owned edge ordinal. Layout algorithms leave its meaning to the caller.
    pub owner_index: Option<u32>,
    pub extras: EdgeExtras,

    pub x: Option<f64>,
    pub y: Option<f64>,
    pub points: Vec<Point>,
}

/// Key and label of an edge temporarily replaced by layout dummies.
#[derive(Debug, Clone, PartialEq)]
pub struct OriginalEdge {
    pub key: EdgeKey,
    pub label: EdgeLabel,
}

impl Default for EdgeLabel {
    fn default() -> Self {
        Self {
            width: 0.0,
            height: 0.0,
            labelpos: LabelPos::C,
            labeloffset: 0.0,
            label_rank: None,
            minlen: 1,
            weight: 0.0,
            nesting_edge: false,
            reversed: false,
            forward_name: None,
            owner_index: None,
            extras: EdgeExtras::default(),
            x: None,
            y: None,
            points: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_extras_keep_map_value_semantics_across_clones() {
        let mut original = EdgeLabel::default();
        original.extras.clear();
        assert_eq!(original, EdgeLabel::default());

        original
            .extras
            .insert("metadata".into(), serde_json::json!({ "nested": [1, 2] }));
        let mut copy = original.clone();
        copy.extras.get_mut("metadata").unwrap()["nested"][0] = 3.into();
        assert_eq!(original.extras["metadata"]["nested"][0], 1);
        assert_eq!(copy.extras["metadata"]["nested"][0], 3);
        copy.extras.clear();
        assert_eq!(copy, EdgeLabel::default());
    }
}
