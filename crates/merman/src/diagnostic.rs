//! Target-neutral, terminal-safe projection for parser and runtime-policy diagnostics.
//!
//! Core parser errors retain authored context and runtime-policy errors may retain host-adapter
//! messages. This module owns their structured projection and consumes the shared bounded terminal
//! normalization boundary so hosts do not need an ASCII feature merely to report an error safely.

pub use merman_core::terminal_text::{normalize_terminal_diagnostic, normalize_terminal_text};

/// Machine-readable context for a terminal-safe parser diagnostic.
///
/// Authored string fields are terminal-safe and bounded. Byte spans remain separate from the
/// human-readable message so bindings do not need to recover structure from display text.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TerminalDiagnosticDetails {
    pub code: String,
    pub span: Option<merman_core::SourceSpan>,
    pub span_kind: Option<merman_core::ParseDiagnosticSpanKind>,
    pub field: Option<String>,
    pub diagram_type: Option<String>,
    pub requested_max_width: Option<usize>,
    pub actual_width: Option<usize>,
    pub width_profile: Option<String>,
    pub fallback_reason: Option<String>,
}

/// Bounded terminal-safe projection of a core parser error.
///
/// The wrapped error is deliberately not exposed through [`std::error::Error::source`], because
/// its display and debug representations may retain authored source text or host-adapter error
/// messages. Use [`Self::terminal_diagnostic_details`] for stable machine-readable context.
pub struct TerminalDiagnostic {
    error: merman_core::Error,
}

/// Bounded terminal-safe projection of a runtime-policy failure.
///
/// This wrapper preserves capability classification without exposing host-adapter messages through
/// [`std::fmt::Display`], [`std::fmt::Debug`], or [`std::error::Error::source`].
#[derive(Clone, PartialEq, Eq)]
pub struct TerminalRuntimePolicyError {
    error: merman_core::runtime::RuntimePolicyError,
}

impl TerminalDiagnostic {
    #[must_use]
    pub fn terminal_safe_message(&self) -> String {
        safe_parse_error(&self.error)
    }

    #[must_use]
    pub fn terminal_diagnostic_details(&self) -> TerminalDiagnosticDetails {
        safe_parse_details(&self.error)
    }

    fn kind(&self) -> &'static str {
        match &self.error {
            merman_core::Error::OperationCancelled(_) => "cancelled",
            merman_core::Error::ThemeColor(_) => "theme_color",
            merman_core::Error::RuntimePolicy(_) => "runtime_policy",
            merman_core::Error::DetectType(_) => "detect_type",
            merman_core::Error::UnsupportedDiagram { .. } => "unsupported_diagram",
            merman_core::Error::DiagramParse { .. } => "diagram_parse",
            merman_core::Error::MalformedFrontMatter => "front_matter",
            merman_core::Error::InvalidDirectiveJson { .. } => "directive",
            merman_core::Error::InvalidFrontMatterYaml { .. } => "front_matter",
        }
    }
}

impl std::fmt::Debug for TerminalDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TerminalDiagnostic")
            .field("kind", &self.kind())
            .field("message", &self.terminal_safe_message())
            .field("details", &self.terminal_diagnostic_details())
            .finish()
    }
}

impl std::fmt::Display for TerminalDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.terminal_safe_message())
    }
}

impl std::error::Error for TerminalDiagnostic {}

impl From<merman_core::Error> for TerminalDiagnostic {
    fn from(error: merman_core::Error) -> Self {
        Self { error }
    }
}

impl From<merman_core::runtime::RuntimePolicyError> for TerminalDiagnostic {
    fn from(error: merman_core::runtime::RuntimePolicyError) -> Self {
        Self::from(merman_core::Error::RuntimePolicy(error))
    }
}

impl TerminalRuntimePolicyError {
    #[must_use]
    pub fn terminal_safe_message(&self) -> String {
        safe_runtime_policy_error(&self.error)
    }

    #[must_use]
    pub fn terminal_diagnostic_details(&self) -> TerminalDiagnosticDetails {
        runtime_policy_details()
    }

    #[must_use]
    pub const fn missing_capability(&self) -> Option<merman_core::runtime::RuntimeCapability> {
        self.error.missing_capability()
    }
}

impl std::fmt::Debug for TerminalRuntimePolicyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TerminalRuntimePolicyError")
            .field("message", &self.terminal_safe_message())
            .field("details", &self.terminal_diagnostic_details())
            .finish()
    }
}

impl std::fmt::Display for TerminalRuntimePolicyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.terminal_safe_message())
    }
}

impl std::error::Error for TerminalRuntimePolicyError {}

impl From<merman_core::runtime::RuntimePolicyError> for TerminalRuntimePolicyError {
    fn from(error: merman_core::runtime::RuntimePolicyError) -> Self {
        Self { error }
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

fn safe_parse_details(error: &merman_core::Error) -> TerminalDiagnosticDetails {
    let mut details = TerminalDiagnosticDetails {
        code: "merman.parse".to_string(),
        span: None,
        span_kind: None,
        field: None,
        diagram_type: None,
        requested_max_width: None,
        actual_width: None,
        width_profile: None,
        fallback_reason: None,
    };
    match error {
        merman_core::Error::OperationCancelled(_) => {
            details.code = "merman.parse.cancelled".to_string();
        }
        merman_core::Error::ThemeColor(_) => {
            details.code = "merman.parse.theme_color".to_string();
            details.field = Some("theme_color".to_string());
        }
        merman_core::Error::RuntimePolicy(_) => return runtime_policy_details(),
        merman_core::Error::DetectType(_) => {
            details.code = "merman.parse.no_diagram_detected".to_string();
        }
        merman_core::Error::UnsupportedDiagram { diagram_type } => {
            details.code = "merman.parse.unsupported_diagram".to_string();
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
                .unwrap_or_else(|| "merman.parse.diagram_parse".to_string());
            details.span = diagnostic.span();
            details.span_kind = diagnostic.span().map(|_| diagnostic.span_kind());
            details.diagram_type = Some(normalize_terminal_diagnostic(diagram_type));
        }
        merman_core::Error::MalformedFrontMatter => {
            details.code = "merman.parse.front_matter.malformed".to_string();
            details.field = Some("front_matter".to_string());
        }
        merman_core::Error::InvalidDirectiveJson { .. } => {
            details.code = "merman.parse.directive.invalid_json".to_string();
            details.field = Some("directive".to_string());
        }
        merman_core::Error::InvalidFrontMatterYaml { .. } => {
            details.code = "merman.parse.front_matter.invalid_yaml".to_string();
            details.field = Some("front_matter".to_string());
        }
    }
    details
}

fn runtime_policy_details() -> TerminalDiagnosticDetails {
    TerminalDiagnosticDetails {
        code: "merman.runtime_policy".to_string(),
        span: None,
        span_kind: None,
        field: None,
        diagram_type: None,
        requested_max_width: None,
        actual_width: None,
        width_profile: None,
        fallback_reason: None,
    }
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
    fn parser_projection_is_safe_and_preserves_structured_context() {
        const MAX_INPUT_BYTES: usize = 16 * 1024;
        const MAX_OUTPUT_BYTES: usize = 4 * 1024;
        let span = merman_core::SourceSpan::new(4, 9);
        let diagnostic = merman_core::ParseDiagnostic::new(format!(
            "bad\u{7}{}",
            "\u{301}".repeat(MAX_INPUT_BYTES)
        ))
        .with_span(span, merman_core::ParseDiagnosticSpanKind::Exact)
        .with_code("merman.test\u{1b}");
        let error = TerminalDiagnostic::from(merman_core::Error::diagram_parse_diagnostic(
            "flow\u{1b}",
            diagnostic,
        ));

        let display = error.to_string();
        let debug = format!("{error:?}");
        let details = error.terminal_diagnostic_details();

        assert!(display.len() <= MAX_OUTPUT_BYTES);
        assert!(!display.contains('\u{1b}'));
        assert!(!display.contains('\u{7}'));
        assert!(!debug.contains('\u{1b}'));
        assert!(!debug.contains('\u{7}'));
        assert_eq!(details.code, "merman.test\\u{1B}");
        assert_eq!(details.span, Some(span));
        assert_eq!(
            details.span_kind,
            Some(merman_core::ParseDiagnosticSpanKind::Exact)
        );
        assert_eq!(details.diagram_type.as_deref(), Some("flow\\u{1B}"));
        assert!(std::error::Error::source(&error).is_none());

        let render_error = crate::RenderError::from(merman_core::Error::diagram_parse_fallback(
            "state\u{1b}",
            "bad\u{7}input",
        ));
        assert!(!render_error.to_string().contains('\u{1b}'));
        assert!(!render_error.to_string().contains('\u{7}'));
        assert!(!format!("{render_error:?}").contains('\u{1b}'));

        let detect_error = TerminalDiagnostic::from(merman_core::Error::DetectType(
            merman_core::detect::DetectTypeError {
                text: "not-a-diagram".to_string(),
            },
        ));
        assert_eq!(detect_error.to_string(), "No Mermaid diagram type detected");
    }

    #[test]
    fn runtime_policy_projection_is_safe_and_preserves_capability_classification() {
        let diagnostic = TerminalRuntimePolicyError::from(
            merman_core::runtime::RuntimePolicyError::SystemTimeZone(
                "adapter\u{1b}\u{7}".to_string(),
            ),
        );
        assert!(!diagnostic.to_string().contains('\u{1b}'));
        assert!(!diagnostic.to_string().contains('\u{7}'));
        assert!(!format!("{diagnostic:?}").contains('\u{1b}'));
        assert_eq!(
            diagnostic.terminal_diagnostic_details().code,
            "merman.runtime_policy"
        );
        assert_eq!(diagnostic.missing_capability(), None);

        let render_error =
            crate::RenderError::from(merman_core::runtime::RuntimePolicyError::SystemRandom(
                "adapter\u{1b}\u{7}".to_string(),
            ));
        assert!(!render_error.to_string().contains('\u{1b}'));
        assert!(!render_error.to_string().contains('\u{7}'));
        assert!(!format!("{render_error:?}").contains('\u{1b}'));

        let missing = TerminalRuntimePolicyError::from(
            merman_core::runtime::RuntimePolicyError::MissingCapability(
                merman_core::runtime::RuntimeCapability::SystemRandom,
            ),
        );
        assert_eq!(
            missing.missing_capability(),
            Some(merman_core::runtime::RuntimeCapability::SystemRandom)
        );
        assert!(std::error::Error::source(&missing).is_none());

        let nested = crate::RenderError::from(merman_core::Error::RuntimePolicy(
            merman_core::runtime::RuntimePolicyError::MissingCapability(
                merman_core::runtime::RuntimeCapability::SystemRandom,
            ),
        ));
        let crate::RenderError::RuntimePolicy(nested) = nested else {
            panic!("nested core runtime-policy errors must not become parse errors");
        };
        assert_eq!(
            nested.missing_capability(),
            Some(merman_core::runtime::RuntimeCapability::SystemRandom)
        );
    }
}
