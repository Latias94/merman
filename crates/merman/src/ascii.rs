//! ASCII target-local types, capabilities, and terminal-safe diagnostic projection.
//!
//! Source-to-text operations use [`crate::Renderer`] so parsing, resource policy, cancellation,
//! and deadlines share one operation owner. Hosts that already own an operation-bound typed model
//! may use [`AsciiRenderer`] as the lower-level target backend.

pub use crate::{normalize_terminal_diagnostic, normalize_terminal_text};

pub use merman_ascii::{
    ASCII_RESOURCE_LIMIT_COUNT, ASCII_RESOURCE_LIMIT_DESCRIPTORS, AsciiCapability,
    AsciiCapabilityEvidence, AsciiCharset, AsciiColorMode, AsciiColorRole, AsciiColorTheme,
    AsciiDirection, AsciiError, AsciiEvidenceKind, AsciiPrimaryProjection, AsciiRenderOptions,
    AsciiRenderer, AsciiResourceLimitCause, AsciiResourceLimitDescriptor,
    AsciiResourceLimitExceeded, AsciiResourceLimitId, AsciiResourceLimitOverrideError,
    AsciiResourceLimitPhase, AsciiResourcePolicy, AsciiRgb, AsciiSemanticCoverage,
    AsciiSupportLevel, AsciiTerminalPalette, MAX_ASCII_DOCUMENT_CELLS_RESOURCE_LIMIT_ID,
    MAX_ASCII_GRAPHEME_BYTES_RESOURCE_LIMIT_ID, MAX_ASCII_GRID_CELLS_RESOURCE_LIMIT_ID,
    MAX_ASCII_LAYOUT_WORK_UNITS_RESOURCE_LIMIT_ID, MAX_ASCII_NESTING_DEPTH_RESOURCE_LIMIT_ID,
    MAX_ASCII_OUTPUT_BYTES_RESOURCE_LIMIT_ID, TerminalWidthProfile, ascii_capabilities,
    ascii_diagrammatic_diagram_types, ascii_resource_profile_value, ascii_supported_diagram_types,
};

/// Terminal-safe projection for ASCII target errors and shared facade diagnostics.
///
/// The canonical [`crate::RenderError`] remains the operation error contract. This type exists
/// for hosts that also need to project ASCII target-local failures. Parse failures use
/// [`crate::TerminalDiagnostic`]; runtime-policy failures use
/// [`crate::TerminalRuntimePolicyError`] so capability classification remains explicit.
#[non_exhaustive]
pub enum AsciiDiagnostic {
    Parse(crate::TerminalDiagnostic),
    Target(merman_ascii::AsciiError),
    RuntimePolicy(crate::TerminalRuntimePolicyError),
}

impl AsciiDiagnostic {
    pub fn terminal_safe_message(&self) -> String {
        match self {
            Self::Parse(error) => error.terminal_safe_message(),
            Self::Target(error) => safe_ascii_error(error),
            Self::RuntimePolicy(error) => error.terminal_safe_message(),
        }
    }

    pub fn terminal_diagnostic_details(&self) -> Option<crate::TerminalDiagnosticDetails> {
        match self {
            Self::Parse(error) => Some(error.terminal_diagnostic_details()),
            Self::Target(error) => safe_ascii_details(error),
            Self::RuntimePolicy(error) => Some(error.terminal_diagnostic_details()),
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            Self::Parse(_) => "parse",
            Self::Target(_) => "ascii",
            Self::RuntimePolicy(_) => "runtime_policy",
        }
    }
}

impl std::fmt::Debug for AsciiDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AsciiDiagnostic")
            .field("kind", &self.kind())
            .field("message", &self.terminal_safe_message())
            .finish()
    }
}

impl std::fmt::Display for AsciiDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.terminal_safe_message())
    }
}

// The wrapped errors may retain authored source or host-adapter messages. This public display
// boundary therefore intentionally does not expose them through `Error::source()`.
impl std::error::Error for AsciiDiagnostic {}

impl From<merman_core::Error> for AsciiDiagnostic {
    fn from(error: merman_core::Error) -> Self {
        match error {
            merman_core::Error::RuntimePolicy(error) => {
                Self::RuntimePolicy(crate::TerminalRuntimePolicyError::from(error))
            }
            error => Self::Parse(crate::TerminalDiagnostic::from(error)),
        }
    }
}

impl From<merman_ascii::AsciiError> for AsciiDiagnostic {
    fn from(error: merman_ascii::AsciiError) -> Self {
        Self::Target(error)
    }
}

impl From<merman_core::runtime::RuntimePolicyError> for AsciiDiagnostic {
    fn from(error: merman_core::runtime::RuntimePolicyError) -> Self {
        Self::RuntimePolicy(crate::TerminalRuntimePolicyError::from(error))
    }
}

fn safe_ascii_error(error: &merman_ascii::AsciiError) -> String {
    match error {
        merman_ascii::AsciiError::InvalidOption { field, message } => {
            bounded_two_field_message("invalid ASCII render option `", field, "`: ", message)
        }
        merman_ascii::AsciiError::UnsupportedDiagram { diagram_type } => bounded_message(
            "ASCII rendering does not support diagram type `",
            diagram_type,
            "`",
        ),
        merman_ascii::AsciiError::UnsupportedFeature {
            diagram_type,
            feature,
        } => {
            let feature = normalize_terminal_diagnostic(feature);
            let diagram_type = normalize_terminal_diagnostic(diagram_type);
            normalize_terminal_diagnostic(&format!(
                "ASCII rendering does not support `{feature}` for `{diagram_type}` yet"
            ))
        }
        merman_ascii::AsciiError::ResourceLimitExceeded(details) => details.to_string(),
        _ => "ASCII rendering failed".to_string(),
    }
}

fn safe_ascii_details(
    error: &merman_ascii::AsciiError,
) -> Option<crate::TerminalDiagnosticDetails> {
    let (code, field, diagram_type) = match error {
        merman_ascii::AsciiError::InvalidOption { field, .. } => (
            "merman.ascii.invalid_option",
            Some(normalize_terminal_diagnostic(field)),
            None,
        ),
        merman_ascii::AsciiError::UnsupportedDiagram { diagram_type } => (
            "merman.ascii.unsupported_diagram",
            None,
            Some(normalize_terminal_diagnostic(diagram_type)),
        ),
        merman_ascii::AsciiError::UnsupportedFeature {
            diagram_type,
            feature,
        } => (
            "merman.ascii.unsupported_feature",
            Some(normalize_terminal_diagnostic(feature)),
            Some(normalize_terminal_diagnostic(diagram_type)),
        ),
        merman_ascii::AsciiError::ResourceLimitExceeded(_) => return None,
        _ => ("merman.ascii.render", None, None),
    };
    Some(crate::TerminalDiagnosticDetails {
        code: code.to_string(),
        span: None,
        span_kind: None,
        field,
        diagram_type,
    })
}

fn bounded_message(prefix: &str, value: &str, suffix: &str) -> String {
    let value = normalize_terminal_diagnostic(value);
    normalize_terminal_diagnostic(&format!("{prefix}{value}{suffix}"))
}

fn bounded_two_field_message(prefix: &str, first: &str, separator: &str, second: &str) -> String {
    let first = normalize_terminal_diagnostic(first);
    let second = normalize_terminal_diagnostic(second);
    normalize_terminal_diagnostic(&format!("{prefix}{first}{separator}{second}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_classifies_target_failures_without_unsafe_text() {
        let cases = [
            (
                AsciiDiagnostic::from(merman_ascii::AsciiError::InvalidOption {
                    field: "box_border_padding",
                    message: "must be positive",
                }),
                "merman.ascii.invalid_option",
                Some("box_border_padding"),
                None,
            ),
            (
                AsciiDiagnostic::from(merman_ascii::AsciiError::UnsupportedDiagram {
                    diagram_type: "radar\u{1b}".to_string(),
                }),
                "merman.ascii.unsupported_diagram",
                None,
                Some("radar\\u{1B}"),
            ),
            (
                AsciiDiagnostic::from(merman_ascii::AsciiError::UnsupportedFeature {
                    diagram_type: "flowchart",
                    feature: "missing endpoint nodes",
                }),
                "merman.ascii.unsupported_feature",
                Some("missing endpoint nodes"),
                Some("flowchart"),
            ),
        ];

        for (error, code, field, diagram_type) in cases {
            let message = error.terminal_safe_message();
            let details = error
                .terminal_diagnostic_details()
                .expect("renderer failure should have diagnostic details");

            assert!(!message.contains('\u{1b}'));
            assert_eq!(details.code, code);
            assert_eq!(details.field.as_deref(), field);
            assert_eq!(details.diagram_type.as_deref(), diagram_type);
        }
    }

    #[test]
    fn core_runtime_policy_errors_keep_their_capability_classification() {
        let error = AsciiDiagnostic::from(merman_core::Error::RuntimePolicy(
            merman_core::runtime::RuntimePolicyError::MissingCapability(
                merman_core::runtime::RuntimeCapability::SystemRandom,
            ),
        ));

        assert!(matches!(error, AsciiDiagnostic::RuntimePolicy(_)));
        assert_eq!(
            error
                .terminal_diagnostic_details()
                .expect("runtime-policy failures expose diagnostic details")
                .code,
            "merman.runtime_policy"
        );
    }
}
