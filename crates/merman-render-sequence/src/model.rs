use crate::frame::SequenceFrameLayout;
use merman_render_core::{Bounds, LayoutCluster, LayoutEdge, LayoutNode};
use serde::{Deserialize, Serialize};

/// One completed activation bar, ordered by its activation-start occurrence.
///
/// Unclosed activations have no drawing rectangle. The top-left coordinates include Mermaid's
/// cursor adjustments and the minimum-height rule, including adjustments from intervening notes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SequenceActivationLayout {
    pub actor: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Number of this participant's activations already open when this bar starts.
    pub depth: usize,
}

/// A Sequence title's text and drawing anchor in diagram coordinates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SequenceTextLayout {
    pub text: String,
    pub x: f64,
    pub y: f64,
}

/// Participant-group drawing rectangle, ordered by the source `box` occurrences.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SequenceParticipantBoxLayout {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub fill: String,
    /// Center anchor of the title. SVG presentation distributes multiline text around this anchor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<SequenceTextLayout>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceDiagramLayout {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    pub clusters: Vec<LayoutCluster>,
    pub bounds: Option<Bounds>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frames: Vec<SequenceFrameLayout>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub activations: Vec<SequenceActivationLayout>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub boxes: Vec<SequenceParticipantBoxLayout>,
    /// Diagram title uses SVG's start anchor and alphabetic baseline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<SequenceTextLayout>,
}
