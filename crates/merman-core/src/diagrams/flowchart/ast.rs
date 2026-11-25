#[cfg(any(test, feature = "editor-facts"))]
use super::LexError;
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
use super::{Edge, Node, SubgraphHeader};
#[cfg(feature = "editor-facts")]
use crate::EditorExpectedSyntax;
use crate::SourceSpan;

#[derive(Debug, Clone, Copy, Default)]
#[cfg_attr(
    not(any(feature = "diagram-flowchart", feature = "diagram-swimlane")),
    allow(
        dead_code,
        reason = "Agentflow shares presentation lexing but does not consume Flowchart grammar and recovery payloads."
    )
)]
#[cfg(feature = "editor-facts")]
pub(crate) struct FlowchartDirectiveEditorEvidence {
    expected_syntax: [Option<EditorExpectedSyntax>; 3],
}

#[cfg(feature = "editor-facts")]
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
#[cfg(feature = "editor-facts")]
pub(crate) struct FlowchartClickEditorEvidence {
    action: Option<EditorExpectedSyntax>,
    payload: Option<EditorExpectedSyntax>,
}

#[cfg(feature = "editor-facts")]
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
    #[cfg(feature = "editor-facts")]
    pub styles_text: Option<String>,
    #[cfg(feature = "editor-facts")]
    pub styles_span: Option<SourceSpan>,
    #[cfg(feature = "editor-facts")]
    pub editor_evidence: FlowchartDirectiveEditorEvidence,
}

#[derive(Debug, Clone)]
pub(crate) struct ClassDefStmt {
    pub ids: Vec<String>,
    #[cfg(feature = "editor-facts")]
    pub id_spans: Vec<SourceSpan>,
    pub styles: Vec<String>,
    #[cfg(feature = "editor-facts")]
    pub styles_text: Option<String>,
    #[cfg(feature = "editor-facts")]
    pub styles_span: Option<SourceSpan>,
    #[cfg(feature = "editor-facts")]
    pub editor_evidence: FlowchartDirectiveEditorEvidence,
}

#[derive(Debug, Clone)]
pub(crate) struct ClassAssignStmt {
    pub targets: Vec<String>,
    #[cfg(feature = "editor-facts")]
    pub target_spans: Vec<SourceSpan>,
    pub class_name: String,
    #[cfg(feature = "editor-facts")]
    pub class_name_span: Option<SourceSpan>,
    #[cfg(feature = "editor-facts")]
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
    #[cfg(feature = "editor-facts")]
    pub id_spans: Vec<SourceSpan>,
    pub tooltip: Option<String>,
    pub action: ClickAction,
    #[cfg(feature = "editor-facts")]
    pub editor_evidence: FlowchartDirectiveEditorEvidence,
    #[cfg(feature = "editor-facts")]
    pub interaction_evidence: FlowchartClickEditorEvidence,
    #[cfg(any(test, feature = "editor-facts"))]
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
    AccessibilityValue(String),
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
#[derive(Default)]
pub(super) struct StatementListBuilder {
    list: StatementList,
    tail: Option<StatementId>,
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
impl StatementListBuilder {
    pub(super) fn finish(self) -> StatementList {
        self.list
    }

    pub(super) fn push(
        &mut self,
        statement: Stmt,
        arena: &mut StatementArena,
        control: &crate::OperationControl,
    ) -> crate::OperationControlResult<()> {
        arena.checkpoint(control)?;
        let id = StatementId(arena.records.len());
        arena.records.push(StatementRecord {
            statement,
            next: None,
        });
        arena.checkpoint(control)?;
        if let Some(tail) = self.tail.replace(id) {
            arena.records[tail.0].next = Some(id);
        } else {
            self.list.head = Some(id);
        }
        Ok(())
    }
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
impl StatementArena {
    pub(super) fn checkpoint(
        &mut self,
        control: &crate::OperationControl,
    ) -> crate::OperationControlResult<()> {
        if self.construction_steps.is_multiple_of(128) {
            control.checkpoint()?;
        }
        self.construction_steps = self.construction_steps.saturating_add(1);
        Ok(())
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
    use crate::{MermaidConfig, OperationControl, SemanticMetadata};

    fn metadata() -> SemanticMetadata {
        SemanticMetadata {
            diagram_type: "flowchart-v2".to_string(),
            config: MermaidConfig::empty_object(),
            semantic_config: MermaidConfig::empty_object(),
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
        use super::super::Tok;
        use std::cell::Cell;

        let mut tokens = vec![Tok::Sep; 126];
        tokens.extend([Tok::KwFlowchart, Tok::Sep]);
        for _ in 0..512 {
            tokens.extend([Tok::KwSubgraph, Tok::Sep]);
        }
        let first_close = tokens.len();
        tokens.extend(std::iter::repeat_n(Tok::KwEnd, 512));
        let seen = Cell::new(0);
        let control = OperationControl::new();
        // Arena construction checks again before the next token delivery checkpoint.
        control.cancel_after_checkpoints(first_close / 128 + 2);
        let edge_budget = super::super::flowchart_edge_budget(&metadata());
        let result = super::super::parser::parse(
            tokens.into_iter().enumerate().map(|(index, token)| {
                seen.set(index + 1);
                Ok((index, token, index + 1))
            }),
            &control,
            &edge_budget,
            first_close + 512,
        );
        assert_eq!(result.unwrap_err().reason, crate::CancelReason::Requested);
        assert_eq!(seen.get(), first_close + 65);
        super::super::parse_flowchart_ast("flowchart TB\nA-->B\n", &metadata()).unwrap();
    }
}
