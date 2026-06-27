use serde::{Deserialize, Serialize};

/// Byte span in the parser input that produced an editor-visible semantic fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpan {
    pub start: usize,
    pub end: usize,
}

impl SourceSpan {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

/// Protocol-independent symbol classification for editor-facing consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorSemanticKind {
    Class,
    Event,
    Function,
    Module,
    Namespace,
    Object,
    Package,
    Property,
    String,
    Struct,
    Variable,
}

/// How downstream editor indexes should project a parser-produced symbol.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum EditorSemanticRole {
    /// Addressable diagram entity: appears in completion, navigation, and outline surfaces.
    #[default]
    Entity,
    /// Structural symbol that belongs in outline/hover, but is not a graph-node completion item.
    Outline,
    /// Span-rich parser payload for lint or future semantic consumers; not projected into LSP
    /// outline/completion/navigation by the migration index.
    Payload,
}

impl EditorSemanticRole {
    pub fn contributes_completion(self) -> bool {
        matches!(self, Self::Entity)
    }

    pub fn contributes_references(self) -> bool {
        matches!(self, Self::Entity)
    }

    pub fn contributes_outline(self) -> bool {
        matches!(self, Self::Entity | Self::Outline)
    }
}

/// A parser-produced symbol occurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorSemanticSymbol {
    pub name: String,
    pub detail: Option<String>,
    pub kind: EditorSemanticKind,
    pub role: EditorSemanticRole,
    pub span: SourceSpan,
    pub selection: SourceSpan,
}

impl EditorSemanticSymbol {
    pub fn new(
        name: impl Into<String>,
        detail: Option<String>,
        kind: EditorSemanticKind,
        span: SourceSpan,
        selection: SourceSpan,
    ) -> Self {
        Self::with_role(
            name,
            detail,
            kind,
            EditorSemanticRole::Entity,
            span,
            selection,
        )
    }

    pub fn outline(
        name: impl Into<String>,
        detail: Option<String>,
        kind: EditorSemanticKind,
        span: SourceSpan,
        selection: SourceSpan,
    ) -> Self {
        Self::with_role(
            name,
            detail,
            kind,
            EditorSemanticRole::Outline,
            span,
            selection,
        )
    }

    pub fn payload(
        name: impl Into<String>,
        detail: Option<String>,
        kind: EditorSemanticKind,
        span: SourceSpan,
        selection: SourceSpan,
    ) -> Self {
        Self::with_role(
            name,
            detail,
            kind,
            EditorSemanticRole::Payload,
            span,
            selection,
        )
    }

    pub fn with_role(
        name: impl Into<String>,
        detail: Option<String>,
        kind: EditorSemanticKind,
        role: EditorSemanticRole,
        span: SourceSpan,
        selection: SourceSpan,
    ) -> Self {
        Self {
            name: name.into(),
            detail,
            kind,
            role,
            span,
            selection,
        }
    }
}

/// Parser-backed diagnostic emitted while producing editor-visible semantic facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorSemanticDiagnostic {
    pub message: String,
    pub span: Option<SourceSpan>,
}

impl EditorSemanticDiagnostic {
    pub fn new(message: impl Into<String>, span: Option<SourceSpan>) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

/// Parser-known syntax category that is expected at a source span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorExpectedSyntaxKind {
    NodeIdentifier,
    Payload,
}

/// Parser-produced cursor context hint for completion and other editor features.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorExpectedSyntax {
    pub kind: EditorExpectedSyntaxKind,
    pub span: SourceSpan,
}

impl EditorExpectedSyntax {
    pub fn new(kind: EditorExpectedSyntaxKind, span: SourceSpan) -> Self {
        Self { kind, span }
    }
}

/// Whether editor-facing facts came from a complete family parse or a recoverable partial parse.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum EditorSemanticCompleteness {
    #[default]
    Complete,
    Recovered,
}

/// Parser-produced facts used by lint, completion, and LSP without exposing a public AST.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EditorSemanticFacts {
    pub completeness: EditorSemanticCompleteness,
    pub symbols: Vec<EditorSemanticSymbol>,
    pub directive_prefixes: Vec<String>,
    pub diagnostics: Vec<EditorSemanticDiagnostic>,
    pub expected_syntax: Vec<EditorExpectedSyntax>,
}

impl EditorSemanticFacts {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_symbol(&mut self, symbol: EditorSemanticSymbol) {
        self.symbols.push(symbol);
    }

    pub fn mark_recovered(&mut self) {
        self.completeness = EditorSemanticCompleteness::Recovered;
    }

    pub fn mark_recovered_with_diagnostic(
        &mut self,
        message: impl Into<String>,
        span: Option<SourceSpan>,
    ) {
        self.mark_recovered();
        self.push_diagnostic(message, span);
    }

    pub fn push_diagnostic(&mut self, message: impl Into<String>, span: Option<SourceSpan>) {
        self.diagnostics
            .push(EditorSemanticDiagnostic::new(message, span));
    }

    pub fn push_directive_prefix(&mut self, prefix: impl Into<String>) {
        let prefix = prefix.into();
        if !self.directive_prefixes.contains(&prefix) {
            self.directive_prefixes.push(prefix);
        }
    }

    pub fn push_expected_syntax(&mut self, expected: EditorExpectedSyntax) {
        self.expected_syntax.push(expected);
    }
}

pub(crate) fn lalrpop_recovery_span<T, E>(
    error: &lalrpop_util::ParseError<usize, T, E>,
    fallback_offset: usize,
) -> SourceSpan {
    match error {
        lalrpop_util::ParseError::InvalidToken { location } => {
            SourceSpan::new(*location, *location)
        }
        lalrpop_util::ParseError::UnrecognizedEof { location, .. } => {
            SourceSpan::new(*location, *location)
        }
        lalrpop_util::ParseError::UnrecognizedToken { token, .. }
        | lalrpop_util::ParseError::ExtraToken { token } => SourceSpan::new(token.0, token.2),
        lalrpop_util::ParseError::User { .. } => SourceSpan::new(fallback_offset, fallback_offset),
    }
}
