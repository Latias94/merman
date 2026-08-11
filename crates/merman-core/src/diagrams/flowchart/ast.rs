use super::{Edge, FlowchartLexemeComponent, Node, SubgraphHeader};
use crate::{EditorExpectedSyntax, SourceSpan};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct FlowchartDirectiveEditorEvidence {
    expected_syntax: [Option<EditorExpectedSyntax>; 3],
}

impl FlowchartDirectiveEditorEvidence {
    pub(crate) fn new(
        directive: EditorExpectedSyntax,
        first: Option<EditorExpectedSyntax>,
        following: Option<EditorExpectedSyntax>,
    ) -> Self {
        Self {
            expected_syntax: [Some(directive), first, following],
        }
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = EditorExpectedSyntax> + '_ {
        self.expected_syntax.iter().flatten().copied()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct FlowchartClickEditorEvidence {
    action: Option<EditorExpectedSyntax>,
    payload: Option<EditorExpectedSyntax>,
}

impl FlowchartClickEditorEvidence {
    pub(crate) fn new(action: Option<SourceSpan>, payload: Option<SourceSpan>) -> Self {
        Self {
            action: action.map(|span| {
                EditorExpectedSyntax::new(crate::EditorExpectedSyntaxKind::InteractionAction, span)
            }),
            payload: payload.map(|span| {
                EditorExpectedSyntax::new(crate::EditorExpectedSyntaxKind::Payload, span)
            }),
        }
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = EditorExpectedSyntax> + '_ {
        [self.action, self.payload].into_iter().flatten()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct StyleStmt {
    pub target: String,
    pub target_span: Option<SourceSpan>,
    pub styles: Vec<String>,
    pub styles_text: Option<String>,
    pub styles_span: Option<SourceSpan>,
    pub editor_evidence: FlowchartDirectiveEditorEvidence,
    pub lexeme_components: Vec<FlowchartLexemeComponent>,
}

#[derive(Debug, Clone)]
pub(crate) struct ClassDefStmt {
    pub ids: Vec<String>,
    pub id_spans: Vec<SourceSpan>,
    pub styles: Vec<String>,
    pub styles_text: Option<String>,
    pub styles_span: Option<SourceSpan>,
    pub editor_evidence: FlowchartDirectiveEditorEvidence,
    pub lexeme_components: Vec<FlowchartLexemeComponent>,
}

#[derive(Debug, Clone)]
pub(crate) struct ClassAssignStmt {
    pub targets: Vec<String>,
    pub target_spans: Vec<SourceSpan>,
    pub class_name: String,
    pub class_name_span: Option<SourceSpan>,
    pub editor_evidence: FlowchartDirectiveEditorEvidence,
    pub lexeme_components: Vec<FlowchartLexemeComponent>,
}

#[derive(Debug, Clone)]
pub(crate) enum ClickAction {
    Callback,
    Link {
        href: String,
        target: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct ClickStmt {
    pub ids: Vec<String>,
    pub tooltip: Option<String>,
    pub action: ClickAction,
    pub editor_evidence: FlowchartDirectiveEditorEvidence,
    pub interaction_evidence: FlowchartClickEditorEvidence,
    pub lexeme_components: Vec<FlowchartLexemeComponent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LinkStylePos {
    Default,
    Index(usize),
}

#[derive(Debug, Clone)]
pub(crate) struct LinkStyleStmt {
    pub positions: Vec<LinkStylePos>,
    pub interpolate: Option<String>,
    pub styles: Vec<String>,
    pub lexeme_components: Vec<FlowchartLexemeComponent>,
}

#[derive(Debug, Clone)]
pub(crate) struct FlowchartAst {
    pub keyword: String,
    pub direction: Option<String>,
    pub header_span: SourceSpan,
    pub statements: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub(crate) struct SubgraphBlock {
    pub header: SubgraphHeader,
    pub statements: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub(crate) enum Stmt {
    Chain {
        nodes: Vec<Node>,
        edges: Vec<Edge>,
    },
    Node(Box<Node>),
    Subgraph(SubgraphBlock),
    Direction(String),
    Style(StyleStmt),
    ClassDef(ClassDefStmt),
    ClassAssign(ClassAssignStmt),
    Click(ClickStmt),
    LinkStyle(LinkStyleStmt),
    ShapeData {
        target: String,
        target_span: Option<SourceSpan>,
        yaml: String,
    },
}
