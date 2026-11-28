//! Controlled C4 parsing and canonical model construction.

use merman_model::SourceSpan;

mod db;
mod parse;

pub use parse::parse_c4_model_controlled;
#[cfg(feature = "editor-facts")]
pub use parse::{C4SourceIssue, parse_c4_model_observed_controlled};
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub use parse::{c4_syntax_construction_count, reset_c4_syntax_construction_count};

type C4Result<T> = Result<T, C4Diagnostic>;

/// The location reported by the parser, independent of the statement's recovery span.
#[derive(Debug, Clone, Copy)]
pub enum C4DiagnosticLocation {
    Exact(SourceSpan),
    InsertionPoint(usize),
    Fallback,
}

/// A syntax, replay, or final model validation error in the parser's input coordinates.
#[derive(Debug)]
pub struct C4Diagnostic {
    pub message: String,
    pub location: C4DiagnosticLocation,
    use_detected_type: bool,
}

impl C4Diagnostic {
    fn new(
        message: impl Into<String>,
        location: C4DiagnosticLocation,
        use_detected_type: bool,
    ) -> Self {
        Self {
            message: message.into(),
            location,
            use_detected_type,
        }
    }

    fn detected_exact(message: impl Into<String>, span: SourceSpan) -> Self {
        Self::new(message, C4DiagnosticLocation::Exact(span), true)
    }

    fn insertion_point(message: impl Into<String>, offset: usize) -> Self {
        Self::new(message, C4DiagnosticLocation::InsertionPoint(offset), false)
    }

    fn detected_insertion_point(message: impl Into<String>, offset: usize) -> Self {
        Self::new(message, C4DiagnosticLocation::InsertionPoint(offset), true)
    }

    fn fallback(message: impl Into<String>) -> Self {
        Self::new(message, C4DiagnosticLocation::Fallback, false)
    }

    fn detected_fallback(message: impl Into<String>) -> Self {
        Self::new(message, C4DiagnosticLocation::Fallback, true)
    }

    /// Retains the original distinction between the detected type and a family-local error.
    pub fn diagram_type<'a>(&self, detected_type: &'a str) -> &'a str {
        if self.use_detected_type {
            detected_type
        } else {
            "c4"
        }
    }

    fn span(&self) -> Option<SourceSpan> {
        match self.location {
            C4DiagnosticLocation::Exact(span) => Some(span),
            C4DiagnosticLocation::InsertionPoint(offset) => Some(SourceSpan::new(offset, offset)),
            C4DiagnosticLocation::Fallback => None,
        }
    }
}

/// A borrowed observation from the same statement scan used for model construction.
#[cfg(feature = "editor-facts")]
pub enum C4Observation<'statement> {
    CaptureStarted,
    DirectivePrefix(&'static str),
    Entity {
        text: &'statement str,
        span: SourceSpan,
        detail: &'static str,
        occurrence: C4EntityOccurrence,
    },
    Payload {
        text: &'statement str,
        span: SourceSpan,
        detail: &'static str,
        argument_name: Option<&'statement str>,
    },
    /// A C4Dynamic declaration replaced the relationship at this ordered pair.
    RelationOverwritten {
        from: &'statement str,
        to: &'statement str,
        span: SourceSpan,
    },
    DirectionValue(SourceSpan),
}

#[cfg(feature = "editor-facts")]
pub enum C4EntityOccurrence {
    Definition,
    Reference,
}

/// Observed parses distinguish recoverable source issues from final model validation errors.
#[cfg(feature = "editor-facts")]
#[derive(Debug)]
pub enum C4ObservedFailure {
    Source {
        first: C4SourceIssue,
        remaining: std::vec::IntoIter<C4SourceIssue>,
    },
    Model(C4Diagnostic),
}
