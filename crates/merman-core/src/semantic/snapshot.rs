use crate::preprocess::SourceConfigEvidence;
use crate::{CapturedPanic, Error, ParsedEditorFacts, SemanticDiagramModel, SemanticMetadata};

/// Canonical result of the single family construction requested by an editor capture.
#[derive(Debug)]
#[allow(
    clippy::large_enum_variant,
    reason = "The snapshot owns the canonical model inline without a separate success allocation."
)]
pub enum SemanticParseOutcome {
    Parsed(SemanticDiagramModel),
    Failed(Error),
    Panicked(String),
}

impl SemanticParseOutcome {
    pub fn parsed_model(&self) -> Option<&SemanticDiagramModel> {
        match self {
            Self::Parsed(model) => Some(model),
            Self::Failed(_) | Self::Panicked(_) => None,
        }
    }
}

/// Canonical meaning and requested recovery facts from one semantic parse operation.
#[derive(Debug)]
pub struct SemanticDiagramSnapshot {
    metadata: SemanticMetadata,
    outcome: SemanticParseOutcome,
    editor_facts: ParsedEditorFacts,
    source_config: SourceConfigEvidence,
    recovered_incomplete_directive: bool,
}

impl SemanticDiagramSnapshot {
    pub(super) fn new(
        metadata: SemanticMetadata,
        outcome: SemanticParseOutcome,
        editor_facts: ParsedEditorFacts,
        source_config: SourceConfigEvidence,
        recovered_incomplete_directive: bool,
    ) -> Self {
        Self {
            metadata,
            outcome,
            editor_facts,
            source_config,
            recovered_incomplete_directive,
        }
    }

    pub fn metadata(&self) -> &SemanticMetadata {
        &self.metadata
    }

    pub fn outcome(&self) -> &SemanticParseOutcome {
        &self.outcome
    }

    pub fn editor_facts(&self) -> &ParsedEditorFacts {
        &self.editor_facts
    }

    pub fn source_config(&self) -> &SourceConfigEvidence {
        &self.source_config
    }

    pub const fn recovered_incomplete_directive(&self) -> bool {
        self.recovered_incomplete_directive
    }

    pub fn into_parts(self) -> (SemanticMetadata, SemanticParseOutcome, ParsedEditorFacts) {
        (self.metadata, self.outcome, self.editor_facts)
    }

    pub fn into_parts_with_source_config(
        self,
    ) -> (
        SemanticMetadata,
        SemanticParseOutcome,
        ParsedEditorFacts,
        SourceConfigEvidence,
    ) {
        (
            self.metadata,
            self.outcome,
            self.editor_facts,
            self.source_config,
        )
    }
}

/// A non-cancellation capture, retaining source-config evidence even before metadata exists.
#[derive(Debug)]
#[allow(
    clippy::large_enum_variant,
    reason = "Captures retain their complete snapshot inline without an additional heap owner."
)]
pub enum SemanticSnapshotCapture {
    Snapshot(SemanticDiagramSnapshot),
    Failed {
        error: Error,
        source_config: SourceConfigEvidence,
    },
    Panicked {
        panic: CapturedPanic,
        source_config: SourceConfigEvidence,
    },
}
