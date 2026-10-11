use super::LexError;
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
use super::{Edge, Node, SubgraphHeader};
use crate::{EditorExpectedSyntax, SourceSpan};

#[derive(Debug, Clone, Copy, Default)]
#[cfg_attr(
    not(any(feature = "diagram-flowchart", feature = "diagram-swimlane")),
    allow(
        dead_code,
        reason = "Agentflow shares presentation lexing but does not consume Flowchart grammar and recovery payloads."
    )
)]
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

    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
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
}

#[derive(Debug, Clone)]
pub(crate) struct ClassDefStmt {
    pub ids: Vec<String>,
    pub id_spans: Vec<SourceSpan>,
    pub styles: Vec<String>,
    pub styles_text: Option<String>,
    pub styles_span: Option<SourceSpan>,
    pub editor_evidence: FlowchartDirectiveEditorEvidence,
}

#[derive(Debug, Clone)]
pub(crate) struct ClassAssignStmt {
    pub targets: Vec<String>,
    pub target_spans: Vec<SourceSpan>,
    pub class_name: String,
    pub class_name_span: Option<SourceSpan>,
    pub editor_evidence: FlowchartDirectiveEditorEvidence,
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
#[cfg_attr(
    not(any(feature = "diagram-flowchart", feature = "diagram-swimlane")),
    allow(
        dead_code,
        reason = "Agentflow shares presentation lexing but does not consume Flowchart grammar and recovery payloads."
    )
)]
pub(crate) struct ClickStmt {
    pub ids: Vec<String>,
    pub id_spans: Vec<SourceSpan>,
    pub tooltip: Option<String>,
    pub action: ClickAction,
    pub editor_evidence: FlowchartDirectiveEditorEvidence,
    pub interaction_evidence: FlowchartClickEditorEvidence,
    pub recovery_error: Option<LexError>,
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
}

#[derive(Debug, Clone)]
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(crate) struct FlowchartAst {
    pub keyword: String,
    pub direction: Option<String>,
    pub header_span: SourceSpan,
    pub statements: StatementArena,
    pub root: StatementList,
}

#[derive(Debug, Clone)]
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(crate) struct SubgraphBlock {
    pub header: SubgraphHeader,
    pub statements: StatementList,
}

#[derive(Debug, Clone)]
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(crate) enum Stmt {
    Chain {
        node_groups: Vec<Vec<Node>>,
        edge_groups: Vec<Vec<Edge>>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(crate) struct StatementId(usize);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(crate) struct StatementList {
    head: Option<StatementId>,
}

#[derive(Debug, Clone)]
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
struct StatementRecord {
    statement: Stmt,
    next: Option<StatementId>,
}

/// Each parser fragment owns only IDs; the arena owns every statement exactly once.
#[derive(Debug, Clone, Default)]
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(crate) struct StatementArena {
    records: Vec<StatementRecord>,
    construction_steps: usize,
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
// Generated grammar actions retain the original lexer token and diagnostic span in errors.
#[allow(clippy::result_large_err)]
impl StatementArena {
    pub(crate) fn construction_checkpoint(
        &mut self,
        control: &crate::OperationControl,
    ) -> std::result::Result<(), lalrpop_util::ParseError<usize, super::Tok, LexError>> {
        if self.construction_steps.is_multiple_of(128) {
            control
                .checkpoint()
                .map_err(|error| lalrpop_util::ParseError::User {
                    error: LexError::new(error.to_string()),
                })?;
        }
        self.construction_steps = self.construction_steps.saturating_add(1);
        Ok(())
    }

    pub(crate) fn push(
        &mut self,
        statement: Stmt,
        control: &crate::OperationControl,
    ) -> std::result::Result<StatementId, lalrpop_util::ParseError<usize, super::Tok, LexError>>
    {
        self.construction_checkpoint(control)?;
        let id = StatementId(self.records.len());
        self.records.push(StatementRecord {
            statement,
            next: None,
        });
        Ok(id)
    }

    /// Right-recursive reductions attach a statement without moving their suffix.
    pub(crate) fn prepend(
        &mut self,
        id: StatementId,
        rest: StatementList,
        control: &crate::OperationControl,
    ) -> std::result::Result<StatementList, lalrpop_util::ParseError<usize, super::Tok, LexError>>
    {
        self.construction_checkpoint(control)?;
        self.records[id.0].next = rest.head;
        Ok(StatementList { head: Some(id) })
    }
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
impl FlowchartAst {
    pub(crate) fn walk(&self) -> StatementWalk<'_> {
        StatementWalk {
            arena: &self.statements,
            frames: vec![StatementWalkFrame {
                next: self.root.head,
                finish: None,
            }],
        }
    }
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(crate) enum StatementEvent<'a> {
    Enter(&'a Stmt),
    Exit(&'a SubgraphBlock),
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
struct StatementWalkFrame {
    next: Option<StatementId>,
    finish: Option<StatementId>,
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(crate) struct StatementWalk<'a> {
    arena: &'a StatementArena,
    frames: Vec<StatementWalkFrame>,
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
impl<'a> Iterator for StatementWalk<'a> {
    type Item = StatementEvent<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let frame = self.frames.last_mut()?;
            if let Some(id) = frame.next {
                let record = &self.arena.records[id.0];
                frame.next = record.next;
                if let Stmt::Subgraph(subgraph) = &record.statement {
                    self.frames.push(StatementWalkFrame {
                        next: subgraph.statements.head,
                        finish: Some(id),
                    });
                }
                return Some(StatementEvent::Enter(&record.statement));
            }
            let frame = self.frames.pop()?;
            if let Some(id) = frame.finish
                && let Stmt::Subgraph(subgraph) = &self.arena.records[id.0].statement
            {
                return Some(StatementEvent::Exit(subgraph));
            }
        }
    }
}

#[cfg(all(test, any(feature = "diagram-flowchart", feature = "diagram-swimlane")))]
mod tests {
    use super::*;
    use crate::{MermaidConfig, OperationControl, ParseMetadata};

    fn metadata() -> ParseMetadata {
        ParseMetadata {
            diagram_type: "flowchart-v2".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: MermaidConfig::empty_object(),
            title: None,
        }
    }

    #[test]
    fn flowchart_statement_arena_keeps_wide_carrier_work_linear() {
        for width in [1, 32, 1024] {
            let mut source = String::from("flowchart TB\n");
            for index in 0..width {
                source.push_str(&format!("n{index}\n"));
            }
            let ast = super::super::parse_flowchart_ast(&source, &metadata()).unwrap();
            assert_eq!(ast.statements.records.len(), width);
            assert_eq!(ast.statements.construction_steps, width * 2);
            let ids = ast
                .walk()
                .map(|event| match event {
                    StatementEvent::Enter(Stmt::Node(node)) => node.id.clone(),
                    _ => panic!("expected a flat node statement"),
                })
                .collect::<Vec<_>>();
            assert_eq!(
                ids,
                (0..width)
                    .map(|index| format!("n{index}"))
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn flowchart_statement_walker_preserves_enter_and_completion_order() {
        let source = "flowchart TB\nsubgraph outer\nA\nsubgraph inner\nB\nend\nC\nend\nD\n";
        let ast = super::super::parse_flowchart_ast(source, &metadata()).unwrap();
        let events = ast
            .walk()
            .map(|event| match event {
                StatementEvent::Enter(Stmt::Subgraph(subgraph)) => {
                    format!("enter:{}", subgraph.header.raw_id)
                }
                StatementEvent::Exit(subgraph) => format!("exit:{}", subgraph.header.raw_id),
                StatementEvent::Enter(Stmt::Node(node)) => node.id.clone(),
                _ => panic!("unexpected statement"),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            events,
            [
                "enter:outer",
                "A",
                "enter:inner",
                "B",
                "exit:inner",
                "C",
                "exit:outer",
                "D"
            ]
        );
        assert_eq!(ast.statements.records.len(), 6);
        assert_eq!(ast.statements.construction_steps, 12);
    }

    #[test]
    fn flowchart_nested_parser_completion_observes_cancellation_with_flat_fragments() {
        let mut source = String::from("flowchart TB\n");
        for index in 0..512 {
            source.push_str(&format!("subgraph s{index}\n"));
        }
        source.push_str("leaf\n");
        source.push_str(&"end\n".repeat(512));
        let control = OperationControl::new();
        control.cancel_after_checkpoints(2);
        let mut statements = StatementArena::default();
        let result = super::super::flowchart_grammar::FlowchartAstParser::new().parse(
            &mut statements,
            &control,
            super::super::Lexer::new(&source),
        );
        assert!(matches!(result, Err(lalrpop_util::ParseError::User { .. })));
        assert!(control.is_cancelled());
        assert_eq!(statements.construction_steps, 256);
        assert_eq!(statements.records.len(), 128);
        assert_eq!(
            statements
                .records
                .iter()
                .filter(|record| matches!(record.statement, Stmt::Subgraph(_)))
                .count(),
            127
        );
        // Both the retained arena and discarded parser symbols have flat ownership.
        drop(statements.clone());
        drop(statements);
        super::super::parse_flowchart_ast("flowchart TB\nA-->B\n", &metadata()).unwrap();
    }
}
