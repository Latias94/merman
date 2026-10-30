//! Immutable family warning facts shared by model and analysis consumers.

use crate::SourceSpan;
use serde::{Deserialize, Serialize};

pub const AGENTFLOW_SHAPE_REMOVED_WARNING_RULE_ID: &str = "merman.semantic.agentflow.shape_removed";
pub const AGENTFLOW_SHAPE_UNSUPPORTED_WARNING_RULE_ID: &str =
    "merman.semantic.agentflow.shape_unsupported";
pub const AGENTFLOW_CONTAINMENT_VIOLATION_WARNING_RULE_ID: &str =
    "merman.semantic.agentflow.containment_violation";

pub const BLOCK_WIDTH_WARNING_RULE_ID: &str = "merman.block.width_exceeds_columns";
pub const FLOWCHART_EXPLICIT_DIRECTION_WARNING_RULE_ID: &str =
    "merman.authoring.flowchart.explicit_direction";
pub const FLOWCHART_UNKNOWN_STYLE_TARGET_WARNING_RULE_ID: &str =
    "merman.semantic.flowchart.unknown_style_target";
pub const GIT_GRAPH_DUPLICATE_COMMIT_WARNING_RULE_ID: &str = "merman.git_graph.duplicate_commit_id";

/// Shared warning fact emitted by diagram families for analysis and lint consumers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagramWarningFact {
    pub rule_id: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<SourceSpan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix_span: Option<SourceSpan>,
}

impl DiagramWarningFact {
    pub fn new(rule_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            rule_id: rule_id.into(),
            message: message.into(),
            span: None,
            fix_span: None,
        }
    }

    pub fn with_span(mut self, span: SourceSpan) -> Self {
        self.span = Some(span);
        self
    }

    pub fn with_fix_span(mut self, span: SourceSpan) -> Self {
        self.fix_span = Some(span);
        self
    }
}
/// Projects the legacy message-only warning array from canonical warning facts.
pub fn legacy_warning_messages(facts: &[DiagramWarningFact]) -> Vec<String> {
    facts.iter().map(|fact| fact.message.clone()).collect()
}
