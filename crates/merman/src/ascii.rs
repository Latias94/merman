//! ASCII target-local types, capabilities, and terminal-safe diagnostic projection.
//!
//! Source-to-text operations use [`crate::Renderer`] so parsing, resource policy, cancellation,
//! and deadlines share one operation owner. Hosts that already own an operation-bound typed model
//! may use [`AsciiRenderer`] as the lower-level target backend.

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
    normalize_terminal_diagnostic, normalize_terminal_text,
};

/// Machine-readable context for a terminal-safe ASCII diagnostic.
///
/// Authored string fields are terminal-safe and bounded. Byte spans remain separate from the
/// human-readable message so bindings do not need to parse display text.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct AsciiDiagnosticDetails {
    pub code: String,
    pub span: Option<merman_core::SourceSpan>,
    pub span_kind: Option<merman_core::ParseDiagnosticSpanKind>,
    pub field: Option<String>,
    pub diagram_type: Option<String>,
}

/// Terminal-safe projection for parse, runtime-policy, and ASCII target errors.
///
/// The canonical [`crate::RenderError`] remains the operation error contract. This type exists
/// only for hosts that must display a bounded diagnostic on an untrusted terminal surface.
#[non_exhaustive]
pub enum AsciiDiagnostic {
    Parse(merman_core::Error),
    Target(merman_ascii::AsciiError),
    RuntimePolicy(merman_core::runtime::RuntimePolicyError),
}

impl AsciiDiagnostic {
    pub fn terminal_safe_message(&self) -> String {
        match self {
            Self::Parse(error) => safe_parse_error(error),
            Self::Target(error) => safe_ascii_error(error),
            Self::RuntimePolicy(error) => safe_runtime_policy_error(error),
        }
    }

    pub fn terminal_diagnostic_details(&self) -> Option<AsciiDiagnosticDetails> {
        match self {
            Self::Parse(error) => Some(safe_parse_details(error)),
            Self::Target(error) => safe_ascii_details(error),
            Self::RuntimePolicy(_) => None,
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
        Self::Parse(error)
    }
}

impl From<merman_ascii::AsciiError> for AsciiDiagnostic {
    fn from(error: merman_ascii::AsciiError) -> Self {
        Self::Target(error)
    }
}

impl From<merman_core::runtime::RuntimePolicyError> for AsciiDiagnostic {
    fn from(error: merman_core::runtime::RuntimePolicyError) -> Self {
        Self::RuntimePolicy(error)
    }
}

fn safe_parse_error(error: &merman_core::Error) -> String {
    match error {
        merman_core::Error::OperationCancelled(_) => "parse operation cancelled".to_string(),
        merman_core::Error::ThemeColor(error) => match error {
            merman_core::theme_color::ColorError::UnsupportedFormat { input } => {
                bounded_message("Unsupported color format: \"", input, "\"")
            }
            merman_core::theme_color::ColorError::MixedColorSpaces => {
                "Cannot change both RGB and HSL channels at the same time".to_string()
            }
        },
        merman_core::Error::RuntimePolicy(error) => safe_runtime_policy_error(error),
        merman_core::Error::DetectType(_) => "No Mermaid diagram type detected".to_string(),
        merman_core::Error::UnsupportedDiagram { diagram_type } => {
            bounded_message("Unsupported diagram type: ", diagram_type, "")
        }
        merman_core::Error::DiagramParse {
            diagram_type,
            diagnostic,
        } => bounded_two_field_message(
            "Diagram parse error (",
            diagram_type,
            "): ",
            diagnostic.message(),
        ),
        merman_core::Error::MalformedFrontMatter => {
            "Malformed YAML front-matter. If you were trying to use a YAML front-matter, please ensure that you've correctly opened and closed the YAML front-matter with un-indented `---` blocks".to_string()
        }
        merman_core::Error::InvalidDirectiveJson { message } => {
            bounded_message("Invalid directive JSON: ", message, "")
        }
        merman_core::Error::InvalidFrontMatterYaml { message } => {
            bounded_message("Invalid YAML front-matter: ", message, "")
        }
    }
}

fn safe_parse_details(error: &merman_core::Error) -> AsciiDiagnosticDetails {
    let mut details = AsciiDiagnosticDetails {
        code: "merman.ascii.parse".to_string(),
        span: None,
        span_kind: None,
        field: None,
        diagram_type: None,
    };
    match error {
        merman_core::Error::OperationCancelled(_) => {
            details.code = "merman.ascii.parse.cancelled".to_string();
        }
        merman_core::Error::ThemeColor(_) => {
            details.code = "merman.ascii.theme_color".to_string();
            details.field = Some("theme_color".to_string());
        }
        merman_core::Error::RuntimePolicy(_) => {
            details.code = "merman.ascii.runtime_policy".to_string();
        }
        merman_core::Error::DetectType(_) => {
            details.code = "merman.ascii.no_diagram_detected".to_string();
        }
        merman_core::Error::UnsupportedDiagram { diagram_type } => {
            details.code = "merman.ascii.unsupported_diagram".to_string();
            details.diagram_type = Some(normalize_terminal_diagnostic(diagram_type));
        }
        merman_core::Error::DiagramParse {
            diagram_type,
            diagnostic,
        } => {
            details.code = diagnostic
                .code()
                .map(normalize_terminal_diagnostic)
                .filter(|code| !code.is_empty())
                .unwrap_or_else(|| "merman.ascii.diagram_parse".to_string());
            details.span = diagnostic.span();
            details.span_kind = diagnostic.span().map(|_| diagnostic.span_kind());
            details.diagram_type = Some(normalize_terminal_diagnostic(diagram_type));
        }
        merman_core::Error::MalformedFrontMatter => {
            details.code = "merman.ascii.front_matter.malformed".to_string();
            details.field = Some("front_matter".to_string());
        }
        merman_core::Error::InvalidDirectiveJson { .. } => {
            details.code = "merman.ascii.directive.invalid_json".to_string();
            details.field = Some("directive".to_string());
        }
        merman_core::Error::InvalidFrontMatterYaml { .. } => {
            details.code = "merman.ascii.front_matter.invalid_yaml".to_string();
            details.field = Some("front_matter".to_string());
        }
    }
    details
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

fn safe_ascii_details(error: &merman_ascii::AsciiError) -> Option<AsciiDiagnosticDetails> {
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
    Some(AsciiDiagnosticDetails {
        code: code.to_string(),
        span: None,
        span_kind: None,
        field,
        diagram_type,
    })
}

fn safe_runtime_policy_error(error: &merman_core::runtime::RuntimePolicyError) -> String {
    match error {
        merman_core::runtime::RuntimePolicyError::SystemTimeZone(message) => {
            bounded_message("system time-zone adapter failed: ", message, "")
        }
        merman_core::runtime::RuntimePolicyError::SystemRandom(message) => {
            bounded_message("system random adapter failed: ", message, "")
        }
        _ => normalize_terminal_diagnostic(&error.to_string()),
    }
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
    fn diagnostic_debug_and_source_do_not_bypass_terminal_safety() {
        let error = AsciiDiagnostic::from(merman_core::Error::diagram_parse_fallback(
            "flow\u{1b}",
            format!("bad\u{7}{}", "\u{301}".repeat(20_000)),
        ));

        let display = error.to_string();
        let debug = format!("{error:?}");

        assert!(display.len() <= 4 * 1024);
        assert!(!display.contains('\u{1b}'));
        assert!(!display.contains('\u{7}'));
        assert!(!debug.contains('\u{1b}'));
        assert!(!debug.contains('\u{7}'));
        assert!(std::error::Error::source(&error).is_none());
    }

    #[test]
    fn diagnostic_preserves_bounded_structured_parse_details() {
        let span = merman_core::SourceSpan::new(4, 9);
        let diagnostic = merman_core::ParseDiagnostic::new("bad input")
            .with_span(span, merman_core::ParseDiagnosticSpanKind::Exact)
            .with_code("merman.test\u{1b}");
        let error = AsciiDiagnostic::from(merman_core::Error::diagram_parse_diagnostic(
            "flow\u{7}",
            diagnostic,
        ));

        let details = error
            .terminal_diagnostic_details()
            .expect("parse details should be available");

        assert_eq!(details.code, "merman.test\\u{1B}");
        assert_eq!(details.span, Some(span));
        assert_eq!(
            details.span_kind,
            Some(merman_core::ParseDiagnosticSpanKind::Exact)
        );
        assert_eq!(details.diagram_type.as_deref(), Some("flow\\u{7}"));
        assert_eq!(details.field, None);
    }

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
}
