/// Byte span in the parser input that produced an editor-visible semantic fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// A parser-produced symbol occurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorSemanticSymbol {
    pub name: String,
    pub detail: Option<String>,
    pub kind: EditorSemanticKind,
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
        Self {
            name: name.into(),
            detail,
            kind,
            span,
            selection,
        }
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

    pub fn push_directive_prefix(&mut self, prefix: impl Into<String>) {
        let prefix = prefix.into();
        if !self.directive_prefixes.contains(&prefix) {
            self.directive_prefixes.push(prefix);
        }
    }
}
