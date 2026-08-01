use crate::{EditorLexemeKind, SourceSpan};

/// One grammar-recognized component of a compound Flowchart lexer token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FlowchartLexemeComponent {
    pub(crate) kind: EditorLexemeKind,
    pub(crate) span: SourceSpan,
}

impl FlowchartLexemeComponent {
    pub(crate) const fn new(kind: EditorLexemeKind, span: SourceSpan) -> Self {
        Self { kind, span }
    }
}
