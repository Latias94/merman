#[cfg(feature = "editor-facts")]
use super::FlowchartLexicalExpectation;
#[cfg(any(test, feature = "editor-facts"))]
use super::LexError;
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
use super::{LabeledText, LinkToken, SubgraphHeader};
use merman_model::SourceSpan;

#[derive(Debug, Clone, Copy, Default)]
#[cfg_attr(
    not(any(feature = "diagram-flowchart", feature = "diagram-swimlane")),
    allow(
        dead_code,
        reason = "Agentflow shares presentation lexing but does not consume Flowchart grammar and recovery payloads."
    )
)]
#[cfg(feature = "editor-facts")]
pub struct FlowchartDirectiveEditorEvidence {
    expected_syntax: [Option<FlowchartLexicalExpectation>; 3],
}

#[cfg(feature = "editor-facts")]
impl FlowchartDirectiveEditorEvidence {
    pub fn new(
        directive: FlowchartLexicalExpectation,
        first: Option<FlowchartLexicalExpectation>,
        following: Option<FlowchartLexicalExpectation>,
    ) -> Self {
        Self {
            expected_syntax: [Some(directive), first, following],
        }
    }

    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    pub fn iter(&self) -> impl Iterator<Item = FlowchartLexicalExpectation> + '_ {
        self.expected_syntax.iter().flatten().copied()
    }
}

#[derive(Debug, Clone, Copy, Default)]
#[cfg(feature = "editor-facts")]
pub struct FlowchartClickEditorEvidence {
    action: Option<FlowchartLexicalExpectation>,
    payload: Option<FlowchartLexicalExpectation>,
}

#[cfg(feature = "editor-facts")]
impl FlowchartClickEditorEvidence {
    pub fn new(action: Option<SourceSpan>, payload: Option<SourceSpan>) -> Self {
        Self {
            action: action.map(|span| {
                FlowchartLexicalExpectation::new(
                    super::FlowchartExpectationKind::InteractionAction,
                    span,
                )
            }),
            payload: payload.map(|span| {
                FlowchartLexicalExpectation::new(super::FlowchartExpectationKind::Payload, span)
            }),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = FlowchartLexicalExpectation> + '_ {
        [self.action, self.payload].into_iter().flatten()
    }
}

#[derive(Debug, Clone)]
pub struct StyleStmt {
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
pub struct ClassDefStmt {
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
pub struct ClassAssignStmt {
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
pub enum ClickAction {
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
pub struct ClickStmt {
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
pub enum LinkStylePos {
    Default,
    Index(usize),
}

#[derive(Debug, Clone)]
pub struct LinkStyleStmt {
    pub positions: Vec<LinkStylePos>,
    pub interpolate: Option<String>,
    pub styles: Vec<String>,
}

#[derive(Debug, Clone)]
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub struct FlowchartAst {
    pub keyword: String,
    pub direction: Option<String>,
    pub header_span: SourceSpan,
    pub(super) statements: StatementArena,
    pub(super) root: StatementList,
}

#[derive(Debug, Clone)]
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub struct SubgraphBlock {
    pub header: SubgraphHeader,
    pub(super) statements: StatementList,
}

/// An authored node occurrence. Database defaults and mutations are applied during replay.
#[derive(Debug, Clone)]
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub struct NodeAction {
    pub id: String,
    pub id_span: Option<SourceSpan>,
    pub label: Option<LabeledText>,
    pub shape: Option<String>,
    pub shape_data: Option<String>,
    pub classes: Vec<String>,
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
impl NodeAction {
    pub fn is_explicit(&self) -> bool {
        self.label.is_some() || self.shape_data.is_some() || !self.classes.is_empty()
    }
}

/// One edge from an admitted Cartesian product, in source expansion order.
#[derive(Debug, Clone)]
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub struct EdgeAction {
    pub from: String,
    pub to: String,
    pub id: Option<String>,
    pub link: LinkToken,
    pub label: Option<LabeledText>,
}

#[derive(Debug, Clone)]
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub enum Stmt {
    Chain {
        node_groups: Vec<Vec<NodeAction>>,
        edge_groups: Vec<Vec<EdgeAction>>,
    },
    Node(Box<NodeAction>),
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
pub(super) struct StatementId(usize);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(super) struct StatementList {
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
pub(super) struct StatementArena {
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
        control: &merman_model::OperationControl,
    ) -> merman_model::OperationControlResult<()> {
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
        control: &merman_model::OperationControl,
    ) -> merman_model::OperationControlResult<()> {
        if self.construction_steps.is_multiple_of(128) {
            control.checkpoint()?;
        }
        self.construction_steps = self.construction_steps.saturating_add(1);
        Ok(())
    }
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
impl FlowchartAst {
    pub fn walk(&self) -> impl Iterator<Item = StatementEvent<'_>> + '_ {
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
pub enum StatementEvent<'a> {
    Enter(&'a Stmt),
    Exit(&'a SubgraphBlock),
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
struct StatementWalkFrame {
    next: Option<StatementId>,
    finish: Option<StatementId>,
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(super) struct StatementWalk<'a> {
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
    use merman_model::{CancelReason, OperationControl};

    fn syntax(source: &str) -> FlowchartAst {
        use super::super::{Lexer, parse_flowchart};

        let control = OperationControl::new();
        let lexer = Lexer::new(source).with_control(&control);
        parse_flowchart(lexer, &control, 500, source.len())
            .unwrap()
            .unwrap()
    }

    #[test]
    fn flowchart_statement_arena_keeps_wide_carrier_work_linear() {
        for width in [1, 32, 1024] {
            let mut source = String::from("flowchart TB\n");
            for index in 0..width {
                source.push_str(&format!("n{index}\n"));
            }
            let ast = syntax(&source);
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
        let ast = syntax(source);
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
        let result = super::super::parse_flowchart(
            tokens.into_iter().enumerate().map(|(index, token)| {
                seen.set(index + 1);
                Ok((index, token, index + 1))
            }),
            &control,
            500,
            first_close + 512,
        );
        assert_eq!(result.unwrap_err().reason, CancelReason::Requested);
        assert_eq!(seen.get(), first_close + 65);
        syntax("flowchart TB\nA-->B\n");
    }
}
