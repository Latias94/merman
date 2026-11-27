#[cfg(feature = "presentation")]
use crate::SourcePresentationMetadata;
#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
use crate::parse_diagnostic::{unexpected_eof_message, unexpected_token_name_message};
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorRenamePolicy, EditorSemanticFacts,
    EditorSemanticKind, EditorSemanticRole, EditorSemanticSymbol,
};
use crate::{
    Error, MermaidConfig, OperationControl, OperationControlResult, ParseDiagnostic,
    ParseDiagnosticSpanKind, Result, SourceSpan,
};
#[cfg(test)]
use merman_core_flow::flowchart::TitleKind;
#[cfg(feature = "editor-facts")]
use merman_core_flow::flowchart::{
    ClassAssignStmt, ClassDefStmt, ClickStmt, EdgeAction, FlowchartAccessibilityStatement,
    FlowchartExpectationKind, FlowchartLexicalError as LexError, FlowchartLexicalEvent,
    FlowchartLexicalExpectation, LabeledText, NodeAction, StyleStmt, SubgraphBlock, SubgraphHeader,
    shape_value_expected_span, trim_flowdb_label_text,
};
use merman_core_flow::flowchart::{
    FlowchartAccessibility, FlowchartAst, FlowchartLexer as Lexer, FlowchartSyntaxDiagnostic,
    parse_flowchart,
};
#[cfg(any(test, feature = "editor-facts"))]
use merman_core_flow::flowchart::{FlowchartToken as Tok, StatementEvent, Stmt};
use merman_core_flow::grid::InlineGridCoordinates;
#[cfg(feature = "editor-facts")]
use merman_model::text::is_ecmascript_whitespace as is_ecmascript_trim_char;
use serde_json::Value;
#[cfg(test)]
use serde_json::json;
#[cfg(test)]
use std::cell::Cell;
#[cfg(feature = "editor-facts")]
use std::collections::HashSet;

#[cfg(test)]
pub(crate) use merman_model::families::flowchart::FlowEdgeStroke;
#[cfg(test)]
use merman_model::families::flowchart::FlowNodeProvenance;
#[cfg(all(test, feature = "presentation"))]
use merman_model::families::flowchart::FlowSubgraph;
pub(crate) use merman_model::families::flowchart::FlowchartModel;
#[cfg(feature = "presentation")]
pub(crate) use merman_model::families::flowchart::FlowchartRenderContext;
#[cfg(all(
    test,
    feature = "diagram-flowchart",
    feature = "compat-json",
    feature = "editor-facts"
))]
pub(crate) use merman_model::families::flowchart::{FlowEdge, FlowEdgeMarker, FlowEdgeVisibility};

#[cfg(all(test, any(feature = "diagram-flowchart", feature = "diagram-swimlane")))]
thread_local! {
    static FLOWCHART_TOKEN_TRACE_CONSTRUCTION_COUNT: Cell<usize> = const { Cell::new(0) };
    static FLOWCHART_ACCESSIBILITY_SCAN_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(all(test, any(feature = "diagram-flowchart", feature = "diagram-swimlane")))]
pub(crate) fn reset_flowchart_token_trace_construction_count() {
    FLOWCHART_TOKEN_TRACE_CONSTRUCTION_COUNT.set(0);
}

#[cfg(all(test, any(feature = "diagram-flowchart", feature = "diagram-swimlane")))]
pub(crate) fn flowchart_token_trace_construction_count() -> usize {
    FLOWCHART_TOKEN_TRACE_CONSTRUCTION_COUNT.get()
}

#[cfg(all(test, any(feature = "diagram-flowchart", feature = "diagram-swimlane")))]
pub(crate) fn reset_flowchart_accessibility_scan_count() {
    FLOWCHART_ACCESSIBILITY_SCAN_COUNT.set(0);
}

#[cfg(all(test, any(feature = "diagram-flowchart", feature = "diagram-swimlane")))]
pub(crate) fn flowchart_accessibility_scan_count() -> usize {
    FLOWCHART_ACCESSIBILITY_SCAN_COUNT.get()
}

#[cfg(feature = "editor-facts")]
mod editor_rename;
#[cfg(feature = "editor-facts")]
pub(crate) use editor_rename::rename_preserves_targets;
#[cfg(all(test, feature = "diagram-flowchart"))]
mod link;
#[cfg(test)]
mod mermaid_12_3_tests;

pub(crate) fn sanitize_common_db_fields(model: &mut FlowchartModel, config: &MermaidConfig) {
    crate::common_db::sanitize_optional_acc_title(&mut model.acc_title, config);
    crate::common_db::sanitize_optional_acc_descr(&mut model.acc_descr, config);
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(crate) trait FlowchartMetadata {
    fn diagram_type(&self) -> &str;
    fn diagram_config(&self) -> &MermaidConfig;
}

#[cfg(all(
    feature = "presentation",
    any(feature = "diagram-flowchart", feature = "diagram-swimlane")
))]
impl FlowchartMetadata for SourcePresentationMetadata {
    fn diagram_type(&self) -> &str {
        &self.diagram_type
    }

    fn diagram_config(&self) -> &MermaidConfig {
        self.effective_config.as_config()
    }
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
impl FlowchartMetadata for crate::SemanticMetadata {
    fn diagram_type(&self) -> &str {
        &self.diagram_type
    }

    fn diagram_config(&self) -> &MermaidConfig {
        &self.semantic_config
    }
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "compat-json")]
pub(crate) fn parse_flowchart_compatibility_model_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<crate::family::FamilySemanticModel>> {
    let model = parse_flowchart_model_controlled(code, meta, control)?;
    control.checkpoint()?;
    Ok(model.map(|model| crate::family::FamilySemanticModel::Flowchart(Box::new(model))))
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub(crate) fn parse_flowchart_json_and_editor_facts(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<crate::family::CombinedSemanticParse> {
    let construction = construct_flowchart_captured_semantic_model_controlled(
        code,
        meta,
        EditorFactsDemand::Capture,
        control,
    )?;
    let parsed = crate::family::CombinedSemanticParse::from_parts(
        construction
            .model
            .map(|model| crate::family::FamilySemanticModel::Flowchart(Box::new(model))),
        construction.editor_facts.unwrap_or_default(),
    );
    control.checkpoint()?;
    Ok(parsed)
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
fn construct_flowchart_captured_semantic_model_controlled<M: FlowchartMetadata>(
    code: &str,
    meta: &M,
    editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<crate::family::SemanticConstruction<FlowchartModel>> {
    control.checkpoint()?;
    let mut trace = construct_flowchart_token_trace(code, control)?;
    let accessibility = std::mem::take(&mut trace.accessibility);
    let max_edges = flowchart_max_edges(meta);
    control.checkpoint()?;
    let (model, facts) =
        match parse_flowchart(trace.parser_items(), control, max_edges, code.len())? {
            Ok(ast) => {
                let mut facts = EditorSemanticFacts::new();
                if editor_demand.captures_rich() {
                    facts = editor_facts_from_flowchart_ast(&ast, control)?;
                    control.checkpoint()?;
                    collect_accessibility_directive_prefixes(
                        accessibility.statements(),
                        &mut facts,
                        control,
                    )?;
                    collect_expected_syntax_from_tokens(code, &trace, &mut facts, control)?;
                }
                let (acc_title, acc_descr) = flowchart_accessibility_metadata(&accessibility, code);
                let model = finish_grid_validation(
                    ast.into_model_with_grid_coordinates(
                        meta.diagram_type(),
                        meta.diagram_config(),
                        acc_title,
                        acc_descr,
                        control,
                    )?,
                    meta,
                    control,
                )?;
                control.checkpoint()?;
                (model, facts)
            }
            Err(error) => {
                let (diagnostic, recovery_span) = flowchart_parse_diagnostic(error);
                let (facts, eof_insertion) = flowchart_recovery_facts(
                    code,
                    trace,
                    accessibility.statements(),
                    &diagnostic,
                    recovery_span,
                    editor_demand,
                    control,
                )?;
                let diagnostic = match eof_insertion {
                    Some(insertion) => {
                        diagnostic.map_span(|_| SourceSpan::new(insertion, insertion))
                    }
                    None => diagnostic,
                };
                let error =
                    Error::diagram_parse_diagnostic(meta.diagram_type().to_string(), diagnostic);
                (Err(error), facts)
            }
        };
    control.checkpoint()?;
    Ok(crate::family::SemanticConstruction {
        model,
        #[cfg(feature = "editor-facts")]
        editor_facts: Some(facts),
    })
}

#[cfg(all(test, any(feature = "diagram-flowchart", feature = "diagram-swimlane")))]
pub(crate) fn parse_flowchart_model_for_render<M: FlowchartMetadata>(
    code: &str,
    meta: &M,
) -> Result<FlowchartModel> {
    parse_flowchart_model_controlled(code, meta, &OperationControl::new())
        .expect("a private parse control cannot be cancelled")
}

#[cfg(all(test, any(feature = "diagram-flowchart", feature = "diagram-swimlane")))]
#[cfg(feature = "presentation")]
pub(crate) fn parse_flowchart_model_with_render_context(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<(FlowchartModel, FlowchartRenderContext)> {
    parse_flowchart_model_with_render_context_controlled(code, meta, &OperationControl::new())
        .expect("a private parse control cannot be cancelled")
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "presentation")]
pub(crate) fn parse_flowchart_model_with_render_context_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<(FlowchartModel, FlowchartRenderContext)>> {
    control.checkpoint()?;
    let (ast, accessibility) = match parse_flowchart_ast_controlled(code, meta, control)? {
        Ok(parsed) => parsed,
        Err(error) => return Ok(Err(error)),
    };
    let (acc_title, acc_descr) = flowchart_accessibility_metadata(&accessibility, code);
    let (model, mut context, coordinates) = match ast
        .into_model_with_render_context_and_grid_coordinates(
            meta.diagram_type(),
            meta.diagram_config(),
            acc_title,
            acc_descr,
            control,
        )? {
        Ok(parsed) => parsed,
        Err(message) => {
            return Ok(Err(Error::diagram_parse_fallback(
                meta.diagram_type(),
                message,
            )));
        }
    };
    Ok(super::grid::validate_inline_grid(
        meta.diagram_config(),
        meta.diagram_type(),
        &coordinates,
        true,
        control,
    )?
    .map(|placements| {
        context.set_grid_placements(placements);
        (model, context)
    }))
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(crate) fn parse_semantic_model_controlled(
    code: &str,
    meta: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: crate::family::EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<crate::family::SemanticConstruction<FlowchartModel>> {
    #[cfg(feature = "editor-facts")]
    if editor_demand.needs_recovery() {
        let mut construction = construct_flowchart_captured_semantic_model_controlled(
            code,
            meta,
            editor_demand,
            control,
        )?;
        if let Ok(model) = &mut construction.model {
            sanitize_common_db_fields(model, &meta.semantic_config);
        }
        control.checkpoint()?;
        return Ok(construction);
    }
    let model = parse_flowchart_model_controlled(code, meta, control)?.map(|mut model| {
        sanitize_common_db_fields(&mut model, &meta.semantic_config);
        model
    });
    control.checkpoint()?;
    Ok(crate::family::SemanticConstruction {
        model,
        #[cfg(feature = "editor-facts")]
        editor_facts: None,
    })
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(all(test, feature = "compat-json"))]
pub(crate) fn render_model_to_compat_json(
    model: &FlowchartModel,
    meta: &SourcePresentationMetadata,
) -> Result<Value> {
    merman_model::families::flowchart::compat::project(model, &meta.compat_json_metadata())
        .map_err(crate::Error::from)
}

fn parse_flowchart_model_controlled<M: FlowchartMetadata>(
    code: &str,
    meta: &M,
    control: &OperationControl,
) -> OperationControlResult<Result<FlowchartModel>> {
    control.checkpoint()?;
    let (ast, accessibility) = match parse_flowchart_ast_controlled(code, meta, control)? {
        Ok(parsed) => parsed,
        Err(error) => return Ok(Err(error)),
    };
    let (acc_title, acc_descr) = flowchart_accessibility_metadata(&accessibility, code);
    let parsed = ast.into_model_with_grid_coordinates(
        meta.diagram_type(),
        meta.diagram_config(),
        acc_title,
        acc_descr,
        control,
    )?;
    finish_grid_validation(parsed, meta, control)
}

fn finish_grid_validation<T>(
    parsed: std::result::Result<(T, InlineGridCoordinates), String>,
    meta: &impl FlowchartMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<T>> {
    let (model, coordinates) = match parsed {
        Ok(parsed) => parsed,
        Err(message) => {
            return Ok(Err(Error::diagram_parse_fallback(
                meta.diagram_type(),
                message,
            )));
        }
    };
    Ok(super::grid::validate_inline_grid(
        meta.diagram_config(),
        meta.diagram_type(),
        &coordinates,
        false,
        control,
    )?
    .map(|_| model))
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
fn parse_flowchart_ast_controlled<M: FlowchartMetadata>(
    code: &str,
    meta: &M,
    control: &OperationControl,
) -> OperationControlResult<Result<(FlowchartAst, FlowchartAccessibility)>> {
    control.checkpoint()?;
    let max_edges = flowchart_max_edges(meta);
    let mut lexer = controlled_flowchart_lexer(Lexer::new(code), control);
    let parsed = parse_flowchart(lexer.by_ref(), control, max_edges, code.len())?;
    Ok(parsed
        .map(|ast| (ast, lexer.into_accessibility()))
        .map_err(|error| {
            let (diagnostic, _) = flowchart_parse_diagnostic(error);
            Error::diagram_parse_diagnostic(meta.diagram_type().to_string(), diagnostic)
        }))
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
const DEFAULT_FLOWCHART_MAX_EDGES: usize = 500;

fn flowchart_max_edges<M: FlowchartMetadata>(meta: &M) -> usize {
    meta.diagram_config()
        .get("maxEdges")
        .and_then(Value::as_u64)
        .map_or(DEFAULT_FLOWCHART_MAX_EDGES, |value| {
            usize::try_from(value).unwrap_or(usize::MAX)
        })
}

fn controlled_flowchart_lexer<'a>(lexer: Lexer<'a>, control: &OperationControl) -> Lexer<'a> {
    #[cfg(test)]
    FLOWCHART_ACCESSIBILITY_SCAN_COUNT
        .set(FLOWCHART_ACCESSIBILITY_SCAN_COUNT.get().saturating_add(1));
    lexer.with_control(control)
}

fn flowchart_accessibility_metadata(
    accessibility: &FlowchartAccessibility,
    code: &str,
) -> (Option<String>, Option<String>) {
    let (title, description) = accessibility.value_spans();
    let value = |span: SourceSpan| code[span.start..span.end].to_owned();
    (title.map(value), description.map(value))
}

fn flowchart_parse_diagnostic(error: FlowchartSyntaxDiagnostic) -> (ParseDiagnostic, SourceSpan) {
    use ParseDiagnosticSpanKind::{Exact, Fallback, InsertionPoint};

    let (message, span, kind, recovery) = match error {
        FlowchartSyntaxDiagnostic::User {
            message,
            span,
            recovery_span,
        } => {
            let kind = if span.is_some() { Exact } else { Fallback };
            (message, span.unwrap_or(recovery_span), kind, recovery_span)
        }
        FlowchartSyntaxDiagnostic::UnexpectedToken {
            token,
            span,
            expected,
        } => (
            unexpected_token_name_message(token, expected.names()),
            span,
            Exact,
            span,
        ),
        FlowchartSyntaxDiagnostic::UnexpectedEof { offset, expected } => {
            let span = SourceSpan::new(offset, offset);
            (
                unexpected_eof_message(expected.names()),
                span,
                InsertionPoint,
                span,
            )
        }
    };
    (
        ParseDiagnostic::new(message).with_span(span, kind),
        recovery,
    )
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(test)]
fn parse_flowchart_ast<M: FlowchartMetadata>(code: &str, meta: &M) -> Result<FlowchartAst> {
    parse_flowchart_ast_controlled(code, meta, &OperationControl::new())?.map(|(ast, _)| ast)
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
type FlowchartToken = (usize, Tok, usize);
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
enum FlowchartTracedItem {
    Token(FlowchartToken),
    RecoveredToken {
        token: FlowchartToken,
        strict_error: LexError,
    },
    LexerError(LexError),
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
struct FlowchartTokenTrace {
    items: Vec<FlowchartTracedItem>,
    accessibility: FlowchartAccessibility,
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
impl FlowchartTokenTrace {
    fn parser_items(&self) -> impl Iterator<Item = FlowchartLexicalEvent> + '_ {
        self.items.iter().map(|item| match item {
            FlowchartTracedItem::Token(token) => Ok(token.clone()),
            FlowchartTracedItem::RecoveredToken { strict_error, .. }
            | FlowchartTracedItem::LexerError(strict_error) => Err(strict_error.clone()),
        })
    }

    fn editor_tokens(&self) -> impl Iterator<Item = (usize, &FlowchartToken)> {
        self.items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| match item {
                FlowchartTracedItem::Token(token)
                | FlowchartTracedItem::RecoveredToken { token, .. } => Some((index, token)),
                FlowchartTracedItem::LexerError(_) => None,
            })
    }

    fn direction_value_span(
        &self,
        index: usize,
        token: &FlowchartToken,
        code_len: usize,
    ) -> Option<SourceSpan> {
        match &token.1 {
            Tok::DirectionStmt(stmt) => stmt.completion_span,
            Tok::Id(id) if id == "direction" => {
                // A standalone vertex also offers a direction value after trailing whitespace.
                let Some(FlowchartTracedItem::Token((_, Tok::Sep, _))) = self.items[..index].last()
                else {
                    return None;
                };
                let insertion = match self.items.get(index + 1) {
                    Some(FlowchartTracedItem::Token((start, Tok::Sep, _))) => *start,
                    None => code_len,
                    _ => return None,
                };
                (insertion > token.2).then_some(SourceSpan::new(insertion, insertion))
            }
            _ => None,
        }
    }
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
fn construct_flowchart_token_trace(
    code: &str,
    control: &OperationControl,
) -> OperationControlResult<FlowchartTokenTrace> {
    #[cfg(all(test, any(feature = "diagram-flowchart", feature = "diagram-swimlane")))]
    FLOWCHART_TOKEN_TRACE_CONSTRUCTION_COUNT.set(
        FLOWCHART_TOKEN_TRACE_CONSTRUCTION_COUNT
            .get()
            .saturating_add(1),
    );

    let mut items = Vec::new();
    let mut lexer = controlled_flowchart_lexer(Lexer::recovering(code), control);
    for item in lexer.by_ref() {
        if items.len() % 128 == 0 {
            control.checkpoint()?;
        }
        match item {
            Ok(mut token) => {
                let strict_error = match &mut token.1 {
                    Tok::NodeLabel(label) => label.recovery_error.take(),
                    Tok::DirectionStmt(direction) => direction.recovery_error.take(),
                    Tok::Arrow(arrow) => arrow.recovery_error.take(),
                    Tok::ClickStmt(click) => click.recovery_error.take(),
                    _ => None,
                };
                items.push(match strict_error {
                    Some(strict_error) => FlowchartTracedItem::RecoveredToken {
                        token,
                        strict_error,
                    },
                    None => FlowchartTracedItem::Token(token),
                });
            }
            Err(error) => items.push(FlowchartTracedItem::LexerError(error)),
        }
    }

    control.checkpoint()?;
    Ok(FlowchartTokenTrace {
        items,
        accessibility: lexer.into_accessibility(),
    })
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
fn editor_facts_from_flowchart_ast(
    ast: &FlowchartAst,
    control: &OperationControl,
) -> OperationControlResult<EditorSemanticFacts> {
    let mut facts = EditorSemanticFacts::new();
    collect_editor_facts_from_statements(ast, &mut facts, control)?;
    Ok(facts)
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
fn recover_flowchart_editor_facts_from_tokens(
    code: &str,
    trace: FlowchartTokenTrace,
    editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<(EditorSemanticFacts, bool)> {
    let mut facts = EditorSemanticFacts::new();
    facts.mark_recovered();
    let mut collector = FlowchartRecoveryFactCollector::default();
    let mut tokens = trace.editor_tokens().enumerate().peekable();
    while let Some((emitted, (index, token))) = tokens.next() {
        if emitted.is_multiple_of(128) {
            control.checkpoint()?;
        }
        if editor_demand.captures_rich()
            && let Some(span) = trace.direction_value_span(index, token, code.len())
        {
            push_flowchart_direction_value_expected_syntax(span, &mut facts);
        }
        let shape_data_target = tokens
            .peek()
            .is_some_and(|(_, (_, next))| matches!(next.1, Tok::ShapeData(_)));
        let (start, token, end) = token;
        collector.accept(
            code,
            token,
            SourceSpan::new(*start, *end),
            shape_data_target,
            &mut facts,
            editor_demand,
        );
    }
    let pending = collector.finish(code.len());
    let mut has_eof_expected = pending.is_some_and(|span| span.end == code.len());
    if editor_demand.captures_rich()
        && let Some(span) = pending
    {
        facts.push_expected_syntax(EditorExpectedSyntax::new(
            EditorExpectedSyntaxKind::NodeIdentifier,
            span,
        ));
    }
    for item in &trace.items {
        let error = match item {
            FlowchartTracedItem::RecoveredToken { strict_error, .. }
            | FlowchartTracedItem::LexerError(strict_error) => Some(strict_error),
            FlowchartTracedItem::Token(_) => None,
        };
        if let Some(error) = error {
            has_eof_expected |= error.expected_syntax.iter().any(|expected| {
                matches!(
                    expected.kind,
                    FlowchartExpectationKind::NodeIdentifier | FlowchartExpectationKind::Operator
                ) && expected.span.end == code.len()
            });
            if editor_demand.captures_rich() {
                push_flowchart_expected_syntax(&mut facts, error.expected_syntax.iter().copied());
            }
        }
        if editor_demand.captures_rich()
            && let Some(prefix) = error.and_then(|error| error.directive_prefix)
        {
            facts.push_directive_prefix(prefix);
        }
    }
    control.checkpoint()?;
    Ok((facts, has_eof_expected))
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
fn flowchart_recovery_facts(
    parser_code: &str,
    trace: FlowchartTokenTrace,
    accessibility_statements: &[FlowchartAccessibilityStatement],
    diagnostic: &ParseDiagnostic,
    recovery_span: SourceSpan,
    editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<(EditorSemanticFacts, Option<usize>)> {
    let (mut facts, has_eof_expected) =
        recover_flowchart_editor_facts_from_tokens(parser_code, trace, editor_demand, control)?;
    if editor_demand.captures_rich() {
        collect_accessibility_directive_prefixes(accessibility_statements, &mut facts, control)?;
    }
    let eof_insertion = flowchart_eof_recovery_insertion(diagnostic, parser_code, has_eof_expected);
    let span = eof_insertion
        .map(|insertion| SourceSpan::new(insertion, insertion))
        .unwrap_or(recovery_span);
    facts.mark_recovered_from_parse_error(
        format!(
            "flowchart parser recovered after parse error: {}",
            diagnostic.message(),
        ),
        Some(span),
    );
    control.checkpoint()?;
    Ok((facts, eof_insertion))
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
fn flowchart_eof_recovery_insertion(
    diagnostic: &ParseDiagnostic,
    code: &str,
    has_eof_expected: bool,
) -> Option<usize> {
    if diagnostic.span_kind() != crate::ParseDiagnosticSpanKind::InsertionPoint {
        return None;
    }
    let insertion = code.trim_end_matches(['\r', '\n']).len();
    (insertion < code.len() && has_eof_expected).then_some(insertion)
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
fn collect_expected_syntax_from_tokens(
    code: &str,
    trace: &FlowchartTokenTrace,
    #[cfg(feature = "editor-facts")] facts: &mut EditorSemanticFacts,
    control: &OperationControl,
) -> OperationControlResult<()> {
    for (index, token) in trace.editor_tokens() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        if let Some(span) = trace.direction_value_span(index, token, code.len()) {
            push_flowchart_direction_value_expected_syntax(span, facts);
        }
        let (start, token, end) = token;
        match token {
            Tok::NodeLabel(label) => {
                if let Some(trigger_span) = label.trigger_span {
                    push_flowchart_shape_trigger_expected_syntax(trigger_span, facts);
                }
            }
            Tok::ShapeData(_) => {
                push_flowchart_shape_value_expected_syntax(code, *start, *end, facts)
            }
            _ => {}
        }
    }
    control.checkpoint()
}

#[derive(Debug, Default)]
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
struct FlowchartRecoveryFactCollector {
    pending_node_identifier: Option<FlowchartRecoveryTargetState>,
    seen_entities: HashSet<String>,
    seen_edge_ids: HashSet<String>,
}

#[derive(Debug, Clone, Copy)]
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
enum FlowchartRecoveryTargetState {
    Awaiting(SourceSpan),
    Sealed(SourceSpan),
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
impl FlowchartRecoveryFactCollector {
    fn accept(
        &mut self,
        code: &str,
        token: &Tok,
        span: SourceSpan,
        shape_data_target: bool,
        #[cfg(feature = "editor-facts")] facts: &mut EditorSemanticFacts,
        editor_demand: EditorFactsDemand,
    ) {
        let SourceSpan { start, end } = span;
        enum TokenKind {
            Arrow,
            EdgeLabel,
            Id,
            Sep,
            Other,
        }

        let token_kind = match token {
            Tok::Arrow(_) => TokenKind::Arrow,
            Tok::EdgeLabel(_) => TokenKind::EdgeLabel,
            Tok::Id(_) => TokenKind::Id,
            Tok::Sep | Tok::AccessibilityValue(_) => TokenKind::Sep,
            _ => TokenKind::Other,
        };
        if editor_demand.captures_rich() {
            if let Tok::EdgeId(id) = token {
                self.seen_edge_ids.insert(id.clone());
            }
            let edge_metadata = shape_data_target
                && matches!(token, Tok::Id(id) if self.seen_edge_ids.contains(id));
            if !edge_metadata {
                collect_editor_fact_from_token(
                    code,
                    token,
                    start,
                    end,
                    facts,
                    &mut self.seen_entities,
                );
            }
        }
        match token_kind {
            TokenKind::Arrow => {
                self.pending_node_identifier = Some(FlowchartRecoveryTargetState::Awaiting(
                    SourceSpan::new(end, end),
                ));
            }
            TokenKind::EdgeLabel => {
                if matches!(
                    self.pending_node_identifier,
                    Some(FlowchartRecoveryTargetState::Awaiting(_))
                ) {
                    self.pending_node_identifier = Some(FlowchartRecoveryTargetState::Awaiting(
                        SourceSpan::new(end, end),
                    ));
                }
            }
            TokenKind::Id => {
                self.pending_node_identifier = None;
            }
            TokenKind::Sep => {
                if let Some(FlowchartRecoveryTargetState::Awaiting(mut span)) =
                    self.pending_node_identifier.take()
                {
                    span.end = start;
                    self.pending_node_identifier = Some(FlowchartRecoveryTargetState::Sealed(span));
                }
            }
            TokenKind::Other => {
                self.pending_node_identifier = None;
            }
        }
    }

    fn finish(self, code_len: usize) -> Option<SourceSpan> {
        let state = self.pending_node_identifier?;

        let span = match state {
            FlowchartRecoveryTargetState::Awaiting(mut span) => {
                span.end = code_len;
                span
            }
            FlowchartRecoveryTargetState::Sealed(span) => span,
        };

        (span.end >= span.start).then_some(span)
    }
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
fn collect_editor_fact_from_token(
    code: &str,
    token: &Tok,
    start: usize,
    end: usize,
    #[cfg(feature = "editor-facts")] facts: &mut EditorSemanticFacts,
    seen_entities: &mut HashSet<String>,
) {
    match token {
        Tok::Id(id) => push_flowchart_token_symbol(facts, id, start, end, seen_entities),
        Tok::SubgraphHeader(header) => {
            push_flowchart_header_symbol(facts, header);
        }
        Tok::NodeLabel(label) => {
            if let Some(trigger_span) = label.trigger_span {
                push_flowchart_shape_trigger_expected_syntax(trigger_span, facts);
            }
            push_flowchart_labeled_payload_symbol(
                #[cfg(feature = "editor-facts")]
                facts,
                &label.text,
                Some(SourceSpan::new(start, end)),
                "flowchart node label",
            )
        }
        Tok::EdgeLabel(label) => push_flowchart_labeled_payload_symbol(
            #[cfg(feature = "editor-facts")]
            facts,
            label,
            Some(SourceSpan::new(start, end)),
            "flowchart edge label",
        ),
        Tok::StyleStmt(stmt) => push_flowchart_style_stmt_facts(facts, stmt),
        Tok::ClassDefStmt(stmt) => push_flowchart_classdef_stmt_facts(facts, stmt),
        Tok::ClassAssignStmt(stmt) => push_flowchart_class_assign_stmt_facts(facts, stmt),
        Tok::ClickStmt(stmt) => push_flowchart_click_stmt_facts(facts, stmt),
        Tok::LinkStyleStmt(_) => facts.push_directive_prefix("linkStyle"),
        Tok::KwGraph
        | Tok::KwFlowchart
        | Tok::KwFlowchartElk
        | Tok::KwSwimlane
        | Tok::KwSubgraph
        | Tok::KwEnd
        | Tok::Sep
        | Tok::AccessibilityValue(_)
        | Tok::Amp
        | Tok::StyleSep
        | Tok::Direction(_)
        | Tok::DirectionStmt(_) => {}
        Tok::Arrow(_) | Tok::EdgeId(_) => {}
        Tok::ShapeData(_) => push_flowchart_shape_value_expected_syntax(code, start, end, facts),
    }
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
fn collect_editor_facts_from_statements(
    ast: &FlowchartAst,
    #[cfg(feature = "editor-facts")] facts: &mut EditorSemanticFacts,
    control: &OperationControl,
) -> OperationControlResult<()> {
    let mut emitted_edge_label_spans = HashSet::new();
    let mut seen_edge_ids = HashSet::new();
    let mut seen_entities = HashSet::new();
    collect_editor_facts_from_statements_with_seen_edges(
        ast,
        #[cfg(feature = "editor-facts")]
        facts,
        &mut emitted_edge_label_spans,
        &mut seen_edge_ids,
        &mut seen_entities,
        control,
    )
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
fn collect_editor_facts_from_statements_with_seen_edges(
    ast: &FlowchartAst,
    #[cfg(feature = "editor-facts")] facts: &mut EditorSemanticFacts,
    emitted_edge_label_spans: &mut HashSet<(usize, usize)>,
    seen_edge_ids: &mut HashSet<String>,
    seen_entities: &mut HashSet<String>,
    control: &OperationControl,
) -> OperationControlResult<()> {
    for (visited, event) in ast.walk().enumerate() {
        if visited.is_multiple_of(128) {
            control.checkpoint()?;
        }
        let StatementEvent::Enter(stmt) = event else {
            continue;
        };

        match stmt {
            Stmt::Chain {
                node_groups,
                edge_groups,
            } => {
                for (group_index, nodes) in node_groups.iter().enumerate() {
                    for (index, node) in nodes.iter().enumerate() {
                        if index % 128 == 0 {
                            control.checkpoint()?;
                        }
                        if seen_edge_ids.contains(&node.id) {
                            if !node.classes.is_empty() && seen_entities.contains(&node.id) {
                                push_flowchart_span_symbol(
                                    facts,
                                    &node.id,
                                    "flowchart node",
                                    EditorSemanticKind::Module,
                                    node.id_span,
                                    EditorSemanticRole::Reference,
                                );
                            }
                            continue;
                        }
                        let occurrence = if edge_groups.is_empty() || node.is_explicit() {
                            FlowchartNodeOccurrence::Definition
                        } else {
                            FlowchartNodeOccurrence::RelationEndpoint
                        };
                        push_flowchart_node_symbol(facts, node, seen_entities, occurrence);
                    }
                    if let Some(edges) = group_index
                        .checked_sub(1)
                        .and_then(|index| edge_groups.get(index))
                    {
                        for (index, edge) in edges.iter().enumerate() {
                            if index % 128 == 0 {
                                control.checkpoint()?;
                            }
                            push_flowchart_edge_label_symbol(facts, edge, emitted_edge_label_spans);
                            if let Some(id) = edge.id.as_deref() {
                                seen_edge_ids.insert(id.to_string());
                            }
                        }
                    }
                }
            }
            Stmt::Node(node) => {
                if !seen_edge_ids.contains(&node.id) {
                    push_flowchart_node_symbol(
                        #[cfg(feature = "editor-facts")]
                        facts,
                        node,
                        seen_entities,
                        FlowchartNodeOccurrence::Definition,
                    );
                } else if !node.classes.is_empty() && seen_entities.contains(&node.id) {
                    push_flowchart_span_symbol(
                        facts,
                        &node.id,
                        "flowchart node",
                        EditorSemanticKind::Module,
                        node.id_span,
                        EditorSemanticRole::Reference,
                    );
                }
            }
            Stmt::Subgraph(subgraph) => {
                push_flowchart_subgraph_symbol(facts, subgraph);
            }
            Stmt::Style(stmt) => push_flowchart_style_stmt_facts(facts, stmt),
            Stmt::ClassDef(stmt) => push_flowchart_classdef_stmt_facts(facts, stmt),
            Stmt::ClassAssign(stmt) => push_flowchart_class_assign_stmt_facts(facts, stmt),
            Stmt::Click(stmt) => push_flowchart_click_stmt_facts(facts, stmt),
            Stmt::LinkStyle(_) => facts.push_directive_prefix("linkStyle"),
            Stmt::ShapeData {
                target,
                target_span,
                ..
            } => {
                if !seen_edge_ids.contains(target) {
                    seen_entities.insert(target.clone());
                    push_flowchart_span_symbol(
                        #[cfg(feature = "editor-facts")]
                        facts,
                        target,
                        "flowchart node",
                        EditorSemanticKind::Module,
                        *target_span,
                        EditorSemanticRole::Entity,
                    );
                }
            }
            Stmt::Direction(_) | Stmt::AccessibilityValue(_) => {}
        }
    }
    control.checkpoint()
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
fn push_flowchart_node_symbol(
    #[cfg(feature = "editor-facts")] facts: &mut EditorSemanticFacts,
    node: &NodeAction,
    seen_entities: &mut HashSet<String>,
    occurrence: FlowchartNodeOccurrence,
) {
    if let Some(span) = node.id_span {
        let role = match occurrence {
            FlowchartNodeOccurrence::Definition => {
                seen_entities.insert(node.id.clone());
                EditorSemanticRole::Entity
            }
            FlowchartNodeOccurrence::RelationEndpoint => {
                if seen_entities.insert(node.id.clone()) {
                    EditorSemanticRole::Entity
                } else {
                    EditorSemanticRole::Reference
                }
            }
        };
        push_flowchart_span_symbol(
            #[cfg(feature = "editor-facts")]
            facts,
            &node.id,
            "flowchart node",
            EditorSemanticKind::Module,
            Some(span),
            role,
        );
    }

    if let Some(label) = &node.label {
        push_flowchart_labeled_payload_symbol(facts, label, None, "flowchart node label");
    }
}

#[derive(Debug, Clone, Copy)]
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
enum FlowchartNodeOccurrence {
    Definition,
    RelationEndpoint,
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
fn push_flowchart_edge_label_symbol(
    #[cfg(feature = "editor-facts")] facts: &mut EditorSemanticFacts,
    edge: &EdgeAction,
    emitted_spans: &mut HashSet<(usize, usize)>,
) {
    let Some(label) = &edge.label else {
        return;
    };
    let Some(span) = label.span else {
        return;
    };
    if !emitted_spans.insert((span.start, span.end)) {
        return;
    }
    push_flowchart_payload_symbol(
        #[cfg(feature = "editor-facts")]
        facts,
        &label.text,
        "flowchart edge label",
        Some(span),
        label.selection,
    );
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
fn push_flowchart_style_stmt_facts(facts: &mut EditorSemanticFacts, stmt: &StyleStmt) {
    facts.push_directive_prefix("style");
    push_flowchart_expected_syntax(facts, stmt.editor_evidence.iter());
    push_flowchart_span_symbol(
        #[cfg(feature = "editor-facts")]
        facts,
        &stmt.target,
        "flowchart style target",
        EditorSemanticKind::Module,
        stmt.target_span,
        EditorSemanticRole::Reference,
    );
    if let (Some(text), Some(span)) = (stmt.styles_text.as_deref(), stmt.styles_span) {
        push_flowchart_payload_symbol(facts, text, "flowchart style", Some(span), Some(span));
    }
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
fn push_flowchart_classdef_stmt_facts(facts: &mut EditorSemanticFacts, stmt: &ClassDefStmt) {
    facts.push_directive_prefix("classDef");
    push_flowchart_expected_syntax(facts, stmt.editor_evidence.iter());
    for (id, span) in stmt.ids.iter().zip(stmt.id_spans.iter().copied()) {
        push_flowchart_span_symbol(
            #[cfg(feature = "editor-facts")]
            facts,
            id,
            "flowchart class definition",
            EditorSemanticKind::Property,
            Some(span),
            EditorSemanticRole::ClassDefinition,
        );
    }
    if let (Some(text), Some(span)) = (stmt.styles_text.as_deref(), stmt.styles_span) {
        push_flowchart_payload_symbol(
            #[cfg(feature = "editor-facts")]
            facts,
            text,
            "flowchart class definition style",
            Some(span),
            Some(span),
        );
    }
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
fn push_flowchart_class_assign_stmt_facts(facts: &mut EditorSemanticFacts, stmt: &ClassAssignStmt) {
    facts.push_directive_prefix("class");
    push_flowchart_expected_syntax(facts, stmt.editor_evidence.iter());
    for (target, span) in stmt.targets.iter().zip(stmt.target_spans.iter().copied()) {
        push_flowchart_span_symbol(
            #[cfg(feature = "editor-facts")]
            facts,
            target,
            "flowchart class target",
            EditorSemanticKind::Module,
            Some(span),
            EditorSemanticRole::Reference,
        );
    }
    push_flowchart_span_symbol(
        #[cfg(feature = "editor-facts")]
        facts,
        &stmt.class_name,
        "flowchart class name",
        EditorSemanticKind::Property,
        stmt.class_name_span,
        EditorSemanticRole::Payload,
    );
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
fn push_flowchart_click_stmt_facts(facts: &mut EditorSemanticFacts, stmt: &ClickStmt) {
    facts.push_directive_prefix("click");
    push_flowchart_expected_syntax(facts, stmt.editor_evidence.iter());
    push_flowchart_expected_syntax(facts, stmt.interaction_evidence.iter());
    for (id, span) in stmt.ids.iter().zip(stmt.id_spans.iter().copied()) {
        push_flowchart_span_symbol(
            #[cfg(feature = "editor-facts")]
            facts,
            id,
            "flowchart interaction target",
            EditorSemanticKind::Module,
            Some(span),
            EditorSemanticRole::Reference,
        );
    }
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
fn push_flowchart_expected_syntax(
    #[cfg(feature = "editor-facts")] facts: &mut EditorSemanticFacts,
    expected_syntax: impl IntoIterator<Item = FlowchartLexicalExpectation>,
) {
    for expected in expected_syntax {
        let kind = match expected.kind {
            FlowchartExpectationKind::Directive => EditorExpectedSyntaxKind::Directive,
            FlowchartExpectationKind::NodeIdentifier => EditorExpectedSyntaxKind::NodeIdentifier,
            FlowchartExpectationKind::StyleValue => EditorExpectedSyntaxKind::StyleValue,
            FlowchartExpectationKind::ClassName => EditorExpectedSyntaxKind::ClassName,
            FlowchartExpectationKind::IdList => EditorExpectedSyntaxKind::IdList,
            FlowchartExpectationKind::InteractionAction => {
                EditorExpectedSyntaxKind::InteractionAction
            }
            FlowchartExpectationKind::Payload => EditorExpectedSyntaxKind::Payload,
            FlowchartExpectationKind::ShapeValue => EditorExpectedSyntaxKind::ShapeValue,
            FlowchartExpectationKind::DirectionValue => {
                EditorExpectedSyntaxKind::FlowchartDirectionValue
            }
            FlowchartExpectationKind::Operator => EditorExpectedSyntaxKind::FlowchartOperator,
        };
        facts.push_expected_syntax(EditorExpectedSyntax::new(kind, expected.span));
    }
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
fn push_flowchart_span_symbol(
    #[cfg(feature = "editor-facts")] facts: &mut EditorSemanticFacts,
    name: &str,
    detail: &'static str,
    kind: EditorSemanticKind,
    span: Option<SourceSpan>,
    role: EditorSemanticRole,
) {
    let Some(span) = span else {
        return;
    };
    if name.is_empty() {
        return;
    }
    let symbol = EditorSemanticSymbol::with_role(
        name.to_string(),
        Some(detail.to_string()),
        kind,
        role,
        span,
        span,
    );
    facts.push_symbol(
        if matches!(
            role,
            EditorSemanticRole::Entity | EditorSemanticRole::Reference
        ) {
            symbol.with_rename_policy(EditorRenamePolicy::FlowchartNodeId)
        } else {
            symbol
        },
    );
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
fn push_flowchart_labeled_payload_symbol(
    #[cfg(feature = "editor-facts")] facts: &mut EditorSemanticFacts,
    label: &LabeledText,
    fallback_span: Option<SourceSpan>,
    detail: &'static str,
) {
    push_flowchart_payload_symbol(
        #[cfg(feature = "editor-facts")]
        facts,
        &label.text,
        detail,
        label.span.or(fallback_span),
        label.selection,
    );
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
fn push_flowchart_payload_symbol(
    #[cfg(feature = "editor-facts")] facts: &mut EditorSemanticFacts,
    name: &str,
    detail: &'static str,
    span: Option<SourceSpan>,
    selection: Option<SourceSpan>,
) {
    if name.is_empty() {
        return;
    }
    let Some(span) = span else {
        return;
    };
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::Payload,
        selection.unwrap_or(span),
    ));
    facts.push_symbol(EditorSemanticSymbol::payload(
        name.to_string(),
        Some(detail.to_string()),
        EditorSemanticKind::String,
        span,
        selection.unwrap_or(span),
    ));
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
fn push_flowchart_shape_value_expected_syntax(
    code: &str,
    start: usize,
    end: usize,
    #[cfg(feature = "editor-facts")] facts: &mut EditorSemanticFacts,
) {
    let Some(span) = shape_value_expected_span(code, start, end) else {
        return;
    };

    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::ShapeValue,
        span,
    ));
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
fn push_flowchart_direction_value_expected_syntax(
    span: SourceSpan,
    #[cfg(feature = "editor-facts")] facts: &mut EditorSemanticFacts,
) {
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::FlowchartDirectionValue,
        span,
    ));
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
fn push_flowchart_shape_trigger_expected_syntax(span: SourceSpan, facts: &mut EditorSemanticFacts) {
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::ShapeTrigger,
        span,
    ));
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
fn collect_accessibility_directive_prefixes(
    statements: &[FlowchartAccessibilityStatement],
    #[cfg(feature = "editor-facts")] facts: &mut EditorSemanticFacts,
    control: &OperationControl,
) -> OperationControlResult<()> {
    for (index, statement) in statements.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        if statement.complete {
            facts.push_directive_prefix(statement.directive.prefix());
            facts.push_expected_syntax(EditorExpectedSyntax::new(
                EditorExpectedSyntaxKind::Directive,
                statement.span,
            ));
        }
    }
    control.checkpoint()
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
fn push_flowchart_subgraph_symbol(facts: &mut EditorSemanticFacts, subgraph: &SubgraphBlock) {
    push_flowchart_header_symbol(facts, &subgraph.header);
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
fn push_flowchart_header_symbol(facts: &mut EditorSemanticFacts, header: &SubgraphHeader) {
    let Some(span) = header.header_span.or(header.raw_id_span) else {
        return;
    };
    let Some((name, selection)) = flowchart_subgraph_symbol_id(header) else {
        return;
    };
    facts.push_symbol(
        EditorSemanticSymbol::new(
            name,
            Some("subgraph".to_string()),
            EditorSemanticKind::Namespace,
            span,
            selection,
        )
        .with_rename_policy(EditorRenamePolicy::FlowchartNodeId),
    );
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
fn flowchart_subgraph_symbol_id(header: &SubgraphHeader) -> Option<(String, SourceSpan)> {
    if header.id_equals_title && header.raw_title.chars().any(is_ecmascript_trim_char) {
        // FlowDB replaces this authored title/id with `subGraphN`; exposing the raw title as a
        // renameable id would point editor operations at a symbol that does not exist in the model.
        return None;
    }

    let raw_span = header.raw_id_span?;
    let raw = header.raw_id.as_str();
    let trimmed = trim_flowdb_label_text(raw);
    let leading = raw.len() - raw.trim_start_matches(is_ecmascript_trim_char).len();
    let name = trimmed.to_string();
    if name.is_empty() {
        return None;
    }
    Some((
        name,
        SourceSpan::new(
            raw_span.start + leading,
            raw_span.start + leading + trimmed.len(),
        ),
    ))
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[cfg(feature = "editor-facts")]
fn push_flowchart_token_symbol(
    #[cfg(feature = "editor-facts")] facts: &mut EditorSemanticFacts,
    id: &str,
    start: usize,
    end: usize,
    seen_entities: &mut HashSet<String>,
) {
    if id.is_empty() {
        return;
    }
    let span = crate::SourceSpan::new(start, end);
    let role = if seen_entities.insert(id.to_string()) {
        EditorSemanticRole::Entity
    } else {
        EditorSemanticRole::Reference
    };
    push_flowchart_span_symbol(
        #[cfg(feature = "editor-facts")]
        facts,
        id,
        "flowchart node",
        EditorSemanticKind::Module,
        Some(span),
        role,
    );
}

#[cfg(all(test, any(feature = "diagram-flowchart", feature = "diagram-swimlane")))]
mod tests {
    use super::*;

    #[cfg(feature = "presentation")]
    fn flowchart_test_meta(diagram_type: &str) -> SourcePresentationMetadata {
        SourcePresentationMetadata {
            diagram_type: diagram_type.to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        }
    }

    fn semantic_test_meta(diagram_type: &str) -> crate::SemanticMetadata {
        crate::SemanticMetadata {
            diagram_type: diagram_type.to_string(),
            config: MermaidConfig::empty_object(),
            semantic_config: MermaidConfig::empty_object(),
            title: None,
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn captured_flowchart_observes_cancellation_in_a_long_accessibility_statement() {
        let source = format!("flowchart TD\naccDescr: {}\nA --> B\n", "x".repeat(65_536));
        let meta = semantic_test_meta("flowchart-v2");
        let control = OperationControl::new();
        control.cancel_after_checkpoints(8);
        reset_flowchart_token_trace_construction_count();

        let result = construct_flowchart_captured_semantic_model_controlled(
            &source,
            &meta,
            EditorFactsDemand::Capture,
            &control,
        );

        assert!(matches!(result, Err(crate::OperationCancelled { .. })));
        assert_eq!(flowchart_token_trace_construction_count(), 1);
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn canonical_flowchart_capture_uses_one_trace_and_keeps_failure_facts() {
        for diagram_type in ["flowchart-v2", "swimlane"] {
            let meta = semantic_test_meta(diagram_type);
            let source = "flowchart TD\naccTitle: Accessible\nA-->B\n";
            reset_flowchart_token_trace_construction_count();
            let omitted = parse_semantic_model_controlled(
                source,
                &meta,
                crate::family::EditorFactsDemand::Omit,
                &OperationControl::new(),
            )
            .unwrap();
            assert!(omitted.editor_facts.is_none());
            assert_eq!(flowchart_token_trace_construction_count(), 0);
            reset_flowchart_accessibility_scan_count();
            let captured = parse_semantic_model_controlled(
                source,
                &meta,
                crate::family::EditorFactsDemand::Capture,
                &OperationControl::new(),
            )
            .unwrap();
            assert_eq!(flowchart_token_trace_construction_count(), 1);
            assert_eq!(flowchart_accessibility_scan_count(), 1);
            let omitted = omitted.model.unwrap();
            let model = captured.model.unwrap();
            assert_eq!(model.nodes.len(), omitted.nodes.len());
            assert_eq!(model.edges.len(), omitted.edges.len());
            let facts = captured.editor_facts.unwrap();
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "A"));
            assert!(
                facts
                    .directive_prefixes
                    .iter()
                    .any(|prefix| prefix == "accTitle")
            );

            reset_flowchart_token_trace_construction_count();
            reset_flowchart_accessibility_scan_count();
            let failed = parse_semantic_model_controlled(
                "flowchart TD\nA-->B\nC -->",
                &meta,
                crate::family::EditorFactsDemand::Capture,
                &OperationControl::new(),
            )
            .unwrap();
            assert_eq!(flowchart_token_trace_construction_count(), 1);
            assert_eq!(flowchart_accessibility_scan_count(), 1);
            let Error::DiagramParse {
                diagram_type: actual,
                ..
            } = failed.model.unwrap_err()
            else {
                panic!("expected captured flowchart diagnostic");
            };
            assert_eq!(actual, diagram_type);
            let facts = failed.editor_facts.unwrap();
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "A"));
            assert!(!facts.diagnostics.is_empty());

            let bad_shape = "flowchart TD\nA@{ shape: this-shape-does-not-exist }\nLater\n";
            reset_flowchart_token_trace_construction_count();
            reset_flowchart_accessibility_scan_count();
            let failed = parse_semantic_model_controlled(
                bad_shape,
                &meta,
                EditorFactsDemand::Capture,
                &OperationControl::new(),
            )
            .unwrap();
            assert_eq!(flowchart_token_trace_construction_count(), 1);
            assert_eq!(flowchart_accessibility_scan_count(), 1);
            let Error::DiagramParse {
                diagram_type: actual,
                diagnostic,
            } = failed.model.unwrap_err()
            else {
                panic!("expected a Flowchart model diagnostic");
            };
            assert_eq!(actual, diagram_type);
            assert_eq!(
                diagnostic.message(),
                "No such shape: this-shape-does-not-exist."
            );
            assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Fallback);
            assert_eq!(diagnostic.span(), None);
            let facts = failed.editor_facts.unwrap();
            let later = bad_shape.find("Later").unwrap();
            assert!(facts.symbols.iter().any(|symbol| {
                symbol.name == "Later" && symbol.span == SourceSpan::new(later, later + 5)
            }));

            let incomplete = format!("{bad_shape}C -->");
            let failed = parse_semantic_model_controlled(
                &incomplete,
                &meta,
                EditorFactsDemand::Capture,
                &OperationControl::new(),
            )
            .unwrap();
            let Error::DiagramParse { diagnostic, .. } = failed.model.unwrap_err() else {
                panic!("expected a Flowchart syntax diagnostic");
            };
            assert_eq!(
                diagnostic.span_kind(),
                ParseDiagnosticSpanKind::InsertionPoint
            );
        }
    }

    #[test]
    fn flowchart_subgraph_endpoint_provenance_preserves_authored_nodes() {
        let meta = semantic_test_meta("flowchart-v2");
        let endpoint_source = "flowchart TD\nsubgraph G\n  A\nend\nG --> A\n";
        let endpoint_only = parse_flowchart_model_for_render(endpoint_source, &meta)
            .expect("subgraph endpoint model");
        let endpoint = endpoint_only
            .nodes
            .iter()
            .find(|node| node.id == "G")
            .expect("subgraph endpoint node");
        assert_eq!(endpoint.provenance, FlowNodeProvenance::SubgraphAnchor);
        #[cfg(feature = "presentation")]
        {
            let (_, endpoint_context) = parse_flowchart_model_with_render_context(
                endpoint_source,
                &flowchart_test_meta("flowchart-v2"),
            )
            .expect("subgraph endpoint context");
            assert!(endpoint_context.contains_subgraph_vertex("G"));
        }

        for source in [
            "flowchart TD\nG\nsubgraph G\n  A\nend\nG --> A\n",
            "flowchart TD\nsubgraph G\n  A\nend\nG --> A\nG\n",
            "flowchart TD\nsubgraph G\n  A\nend\nG:::authored --> A\n",
            concat!(
                "flowchart TD\n",
                "G@{ shape: rect, label: \"Authored G\" }\n",
                "subgraph G\n  A\nend\n",
                "G --> A\n",
            ),
        ] {
            let model = parse_flowchart_model_for_render(source, &meta)
                .expect("authored node and subgraph model");
            let node = model
                .nodes
                .iter()
                .find(|node| node.id == "G")
                .expect("authored G node");
            assert_eq!(node.provenance, FlowNodeProvenance::Authored, "{source}");
            #[cfg(feature = "presentation")]
            {
                let (_, context) = parse_flowchart_model_with_render_context(
                    source,
                    &flowchart_test_meta("flowchart-v2"),
                )
                .expect("authored node and subgraph context");
                assert!(context.contains_subgraph_vertex("G"), "{source}");
            }
        }
    }

    #[test]
    #[cfg(feature = "presentation")]
    fn flowchart_subgraph_metadata_survives_render_model_projection() {
        // Mermaid 12 FlowDB forwards the shallow-merged metadata unchanged to group nodes.
        let meta = flowchart_test_meta("flowchart-v2");
        let (model, context) = parse_flowchart_model_with_render_context(
            concat!(
                "flowchart TD\n",
                "subgraph G[Group Title]\n A\nend\n",
                "subgraph H\n B\nend\n",
                "G@{\nalgorithm: elk.box\ncustom:\n  first: 1\nview: collapsed\n}\n",
                "G@{\nalgorithm: elk.rectpacking\ncustom:\n  second: 2\nview: expanded\nlabel: Ignored\n}\n",
            ),
            &meta,
        )
        .expect("subgraph metadata should parse");
        let group = model.subgraphs.iter().find(|sg| sg.id == "G").unwrap();
        assert_eq!(group.title, "Group Title");
        assert_eq!(
            group.metadata,
            Some(json!({
                "algorithm": "elk.rectpacking",
                "custom": { "second": 2 },
                "view": "expanded",
                "label": "Ignored",
            }))
        );
        assert!(!context.is_subgraph_collapsed("G"));
        assert!(
            model
                .subgraphs
                .iter()
                .find(|sg| sg.id == "H")
                .unwrap()
                .metadata
                .is_none()
        );

        let serialized = serde_json::to_value(group).unwrap();
        let restored: FlowSubgraph = serde_json::from_value(serialized.clone()).unwrap();
        assert_eq!(restored.metadata, group.metadata);
        assert_eq!(
            serialized["metadata"],
            group.metadata.as_ref().unwrap().clone()
        );
        let mut without_metadata = serialized;
        without_metadata.as_object_mut().unwrap().remove("metadata");
        let restored: FlowSubgraph = serde_json::from_value(without_metadata).unwrap();
        assert!(restored.metadata.is_none());
        assert!(
            serde_json::to_value(restored)
                .unwrap()
                .get("metadata")
                .is_none()
        );
    }

    #[test]
    fn flowchart_subgraph_metadata_updates_the_canonical_group() {
        let meta = semantic_test_meta("flowchart-v2");
        let model = parse_flowchart_model_for_render(
            concat!(
                "flowchart TD\n",
                "subgraph G[First]\n A\nend\n",
                "G@{ algorithm: elk.box }\n",
                "subgraph G[Second]\n B\nend\n",
                "G@{ algorithm: elk.radial }\n",
            ),
            &meta,
        )
        .expect("duplicate subgraph declarations should parse");
        assert_eq!(model.subgraphs.len(), 1);
        let group = &model.subgraphs[0];
        assert_eq!(group.title, "First");
        assert_eq!(group.nodes, ["A", "B"]);
        assert_eq!(group.metadata.as_ref().unwrap()["algorithm"], "elk.radial");
    }

    #[test]
    #[cfg(feature = "compat-json")]
    fn duplicate_subgraph_metadata_and_classes_follow_the_canonical_owner() {
        let meta = flowchart_test_meta("flowchart-v2");
        for metadata_between in [false, true] {
            let update = "class G hot\nG@{ view: collapsed, algorithm: elk.box }\n";
            let source = format!(
                "flowchart TB\nsubgraph G[First]\ndirection LR\nA\nend\n{}subgraph G[Second]\ndirection RL\nB\nA\nend\n{}",
                if metadata_between { update } else { "" },
                if metadata_between { "" } else { update },
            );
            let (model, context) = parse_flowchart_model_with_render_context(&source, &meta)
                .expect("repeated groups share their metadata target");
            assert_eq!(model.subgraphs.len(), 1, "{source}");
            let group = &model.subgraphs[0];
            assert_eq!(group.title, "First");
            assert_eq!(group.dir.as_deref(), Some("LR"));
            assert_eq!(group.nodes, ["A", "B"]);
            assert_eq!(group.classes, ["hot"]);
            assert_eq!(group.metadata.as_ref().unwrap()["algorithm"], "elk.box");
            assert!(context.is_subgraph_collapsed("G"));
            for member in ["A", "B"] {
                assert_eq!(context.collapsed_replacement(member), Some("G"));
            }
            let json = render_model_to_compat_json(&model, &meta).unwrap();
            assert_eq!(json["subgraphs"].as_array().unwrap().len(), 1);
            assert_eq!(json["subgraphs"][0]["nodes"], json!(["A", "B"]));
        }
    }

    #[test]
    fn duplicate_subgraph_membership_excludes_self_and_preserves_nested_groups() {
        let meta = semantic_test_meta("flowchart-v2");
        let cases = [
            (
                "flowchart TB\nsubgraph S[Outer]\nx\nsubgraph S[Inner]\ny\nend\nend\n",
                vec![("S", "Inner", vec!["y", "x"])],
            ),
            (
                "flowchart TB\nsubgraph S[First]\nend\nsubgraph S[Second]\nsubgraph T\ny\nend\nend\n",
                vec![("S", "First", vec!["T"]), ("T", "T", vec!["y"])],
            ),
            (
                "flowchart TB\nsubgraph S\nx\nend\nsubgraph S\ny\nend\nsubgraph Anonymous group\na\nend\nsubgraph Anonymous group\nb\nend\n",
                vec![
                    ("S", "S", vec!["x", "y"]),
                    ("subGraph2", "Anonymous group", vec!["a"]),
                    ("subGraph3", "Anonymous group", vec!["b"]),
                ],
            ),
        ];
        for (source, expected) in cases {
            let model = parse_flowchart_model_for_render(source, &meta).unwrap();
            assert_eq!(model.subgraphs.len(), expected.len(), "{source}");
            for (group, (id, title, nodes)) in model.subgraphs.iter().zip(expected) {
                assert_eq!(group.id, id, "{source}");
                assert_eq!(group.title, title, "{source}");
                assert_eq!(group.nodes, nodes, "{source}");
                assert!(!group.nodes.contains(&group.id), "{source}");
            }
        }
    }

    #[test]
    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    fn duplicate_subgraph_editor_facts_keep_each_declaration_span() {
        let source = "flowchart TB\nsubgraph S[First]\nx\nend\nsubgraph S[Second]\ny\nend\nclass S hot\nS --> z\n";
        let control = OperationControl::new();
        let meta = flowchart_test_meta("flowchart-v2");
        let (model, facts) = crate::family::test_support::into_projected_result(
            parse_flowchart_json_and_editor_facts(source, &meta, &control),
            &meta,
        )
        .unwrap();
        let symbols: Vec<_> = facts
            .symbols
            .iter()
            .filter(|symbol| symbol.name == "S")
            .collect();
        let declarations: Vec<_> = symbols
            .iter()
            .filter(|symbol| symbol.kind == EditorSemanticKind::Namespace)
            .collect();
        assert_eq!(declarations.len(), 2);
        assert_ne!(declarations[0].selection, declarations[1].selection);
        assert!(
            symbols.len() >= 4,
            "declarations, class target and edge endpoint stay linked"
        );
        for symbol in symbols {
            assert_eq!(symbol.rename_policy, EditorRenamePolicy::FlowchartNodeId);
            assert_eq!(&source[symbol.selection.start..symbol.selection.end], "S");
        }
        assert_eq!(model["subgraphs"].as_array().unwrap().len(), 1);
        assert_eq!(model["subgraphs"][0]["nodes"], json!(["x", "y"]));
    }

    #[test]
    fn flowchart_subgraph_metadata_preserves_algorithm_values_for_layout_resolution() {
        let meta = semantic_test_meta("flowchart-v2");
        for (authored, expected) in [
            ("null", json!(null)),
            ("42", json!(42)),
            ("unknown-layout", json!("unknown-layout")),
            ("[elk.box]", json!(["elk.box"])),
        ] {
            let input =
                format!("flowchart TD\nsubgraph G\n A\nend\nG@{{ algorithm: {authored} }}\n");
            let model = parse_flowchart_model_for_render(&input, &meta)
                .expect("opaque metadata should parse");
            assert_eq!(
                model.subgraphs[0].metadata.as_ref().unwrap()["algorithm"],
                expected
            );
        }
    }

    #[test]
    fn subgraph_shape_data_preserves_only_the_bare_vertex_call() {
        let source = "flowchart TD\nsubgraph G\n  A\nend\nG@{ view: collapsed }\nB\n";
        let model = parse_flowchart_model_for_render(source, &semantic_test_meta("flowchart-v2"))
            .expect("subgraph shapeData should parse");

        assert_eq!(model.vertex_calls, ["A", "G", "B"]);
        #[cfg(feature = "presentation")]
        {
            let (_, context) = parse_flowchart_model_with_render_context(
                source,
                &flowchart_test_meta("flowchart-v2"),
            )
            .expect("subgraph shapeData context");
            assert!(context.is_subgraph_collapsed("G"));
        }
    }

    #[test]
    fn standalone_shape_data_preserves_upstream_vertex_call_sequence() {
        let meta = semantic_test_meta("flowchart-v2");
        let model = parse_flowchart_model_for_render(
            "flowchart TD\nA@{ shape: rect }\nB@{ shape: stadium }\n",
            &meta,
        )
        .expect("standalone shapeData should parse");

        assert_eq!(model.vertex_calls, ["A", "A", "B", "B"]);
    }

    #[test]
    #[cfg(feature = "presentation")]
    fn collapsed_subgraph_shape_data_is_retained_in_render_context() {
        let meta = flowchart_test_meta("flowchart-v2");
        let (model, context) = parse_flowchart_model_with_render_context(
            "flowchart TD\nsubgraph one[My Group]\n  A --> B\nend\nC --> A\none@{ view: collapsed }\n",
            &meta,
        )
        .expect("collapsed subgraph should parse");
        assert!(context.is_subgraph_collapsed("one"));
        assert_eq!(context.collapsed_replacement("A"), Some("one"));
        assert_eq!(context.collapsed_replacement("B"), Some("one"));
        assert_eq!(context.collapsed_replacement("one"), None);
        assert!(model.subgraphs.iter().any(|sg| sg.id == "one"));
    }

    #[test]
    #[cfg(feature = "presentation")]
    fn collapsed_subgraph_shape_data_in_chains_uses_subgraph_dispatch() {
        let meta = flowchart_test_meta("flowchart-v2");
        for source in [
            "flowchart TD\nsubgraph G\n  A\nend\nG@{ view: collapsed } --> B\n",
            "flowchart TD\nsubgraph G\n  A\nend\nB --> G@{ view: collapsed }\n",
        ] {
            let (_, context) = parse_flowchart_model_with_render_context(source, &meta)
                .expect("chained subgraph metadata should parse");
            assert!(context.is_subgraph_collapsed("G"), "{source}");
        }
    }

    #[test]
    fn subgraph_shape_data_precedes_same_id_edge_metadata() {
        let meta = semantic_test_meta("flowchart-v2");
        let model = parse_flowchart_model_for_render(
            concat!(
                "flowchart TD\n",
                "subgraph e1\n  A\nend\n",
                "A e1@--> B\n",
                "e1@{ curve: basis }\n",
            ),
            &meta,
        )
        .expect("same-id subgraph and edge metadata should parse");

        let edge = model
            .edges
            .iter()
            .find(|edge| edge.id == "e1")
            .expect("user-defined edge");
        assert_eq!(edge.interpolate, None);
    }

    #[test]
    fn same_id_subgraph_vertex_style_preserves_statement_order() {
        let meta = semantic_test_meta("flowchart-v2");
        let cases = [
            (
                concat!(
                    "flowchart TD\n",
                    "classDef base stroke:#00f\n",
                    "subgraph G\n  A\nend\n",
                    "class G base\n",
                    "style G fill:#f00\n",
                ),
                Vec::<String>::new(),
            ),
            (
                concat!(
                    "flowchart TD\n",
                    "classDef base stroke:#00f\n",
                    "subgraph G\n  A\nend\n",
                    "style G fill:#f00\n",
                    "class G base\n",
                ),
                vec!["base".to_string()],
            ),
        ];

        for (source, expected_vertex_classes) in cases {
            let model =
                parse_flowchart_model_for_render(source, &meta).expect("same-ID group model");
            let (subgraph_index, subgraph) = model
                .subgraphs
                .iter()
                .enumerate()
                .find(|(_, subgraph)| subgraph.id == "G")
                .expect("subgraph G");
            let (vertex_classes, vertex_styles) =
                model.effective_subgraph_css(subgraph_index, subgraph);

            assert_eq!(subgraph.classes, ["base"]);
            assert_eq!(vertex_classes, expected_vertex_classes, "{source}");
            assert_eq!(vertex_styles, ["fill:#f00"], "{source}");
            assert!(model.contains_subgraph_vertex("G"));

            #[cfg(feature = "presentation")]
            {
                let (full_model, context) = parse_flowchart_model_with_render_context(
                    source,
                    &flowchart_test_meta("flowchart-v2"),
                )
                .expect("same-ID group render context");
                let full_subgraph = &full_model.subgraphs[subgraph_index];
                let semantic_css = full_model.effective_subgraph_css(subgraph_index, full_subgraph);
                let render_css = context.effective_subgraph_css(subgraph_index, full_subgraph);
                assert_eq!(render_css, (vertex_classes, vertex_styles));
                assert!(std::ptr::eq(semantic_css.0, render_css.0));
                assert!(std::ptr::eq(semantic_css.1, render_css.1));
            }
        }
    }

    #[test]
    fn effective_subgraph_css_preserves_group_fallback_and_owner_ordinal() {
        let meta = semantic_test_meta("flowchart-v2");
        let group_only = parse_flowchart_model_for_render(
            "flowchart TD\nsubgraph G\n A\nend\nclass G base\n",
            &meta,
        )
        .expect("group-only CSS model");
        let group = &group_only.subgraphs[0];
        assert!(!group_only.contains_subgraph_vertex("G"));
        assert_eq!(
            group_only.effective_subgraph_css(0, group),
            (group.classes.as_slice(), group.styles.as_slice()),
        );

        let vertex = parse_flowchart_model_for_render(
            "flowchart TD\nsubgraph G\n A\nend\nclass G base\nstyle G fill:#f00\nclass G hot\n",
            &meta,
        )
        .expect("same-ID vertex CSS model");
        let group = &vertex.subgraphs[0];
        let (classes, styles) = vertex.effective_subgraph_css(0, group);
        assert_eq!(classes, ["hot"]);
        assert_eq!(styles, ["fill:#f00"]);
        assert_eq!(
            vertex.effective_subgraph_css(1, group),
            (group.classes.as_slice(), group.styles.as_slice()),
            "a different declaration owner must retain the group's own CSS",
        );
        #[cfg(feature = "presentation")]
        {
            let (full_model, context) = parse_flowchart_model_with_render_context(
                "flowchart TD\nsubgraph G\n A\nend\nclass G base\nstyle G fill:#f00\nclass G hot\n",
                &flowchart_test_meta("flowchart-v2"),
            )
            .expect("same-ID vertex CSS render context");
            let full_group = &full_model.subgraphs[0];
            assert_eq!(
                context.effective_subgraph_css(1, full_group),
                (full_group.classes.as_slice(), full_group.styles.as_slice()),
            );
        }
    }

    #[test]
    #[cfg(feature = "compat-json")]
    fn same_id_subgraph_vertex_style_stays_out_of_serialized_models() {
        let meta = flowchart_test_meta("flowchart-v2");
        let (model, context) = parse_flowchart_model_with_render_context(
            concat!(
                "flowchart TD\n",
                "subgraph G\n  A\nend\n",
                "style G fill:#f00\n",
                "class G hot\n",
            ),
            &meta,
        )
        .expect("same-ID group model");

        let json = serde_json::to_value(&model).expect("serialize Flowchart model");
        assert!(json.get("subgraph_vertex_styles").is_none());
        assert!(json["subgraphs"][0].get("sameIdVertexStyle").is_none());
        let subgraph = &model.subgraphs[0];
        assert!(subgraph.styles.is_empty());
        let (classes, styles) = context.effective_subgraph_css(0, subgraph);
        assert_eq!(classes, ["hot"]);
        assert_eq!(styles, ["fill:#f00"]);

        let compatibility =
            render_model_to_compat_json(&model, &meta).expect("legacy flowchart JSON");
        assert!(
            compatibility["subgraphs"][0]
                .get("sameIdVertexStyle")
                .is_none()
        );
    }

    #[test]
    fn duplicate_subgraph_vertex_css_is_owned_by_the_canonical_group() {
        let meta = semantic_test_meta("flowchart-v2");
        let cases = [
            concat!(
                "flowchart TD\n",
                "style X fill:#f00\n",
                "subgraph X\n  A\nend\n",
                "subgraph X\n  B\nend\n",
            ),
            concat!(
                "flowchart TD\n",
                "subgraph X\n  A\nend\n",
                "style X fill:#f00\n",
                "subgraph X\n  B\nend\n",
            ),
            concat!(
                "flowchart TD\n",
                "subgraph X\n  A\nend\n",
                "subgraph X\n  B\nend\n",
                "style X fill:#f00\n",
            ),
        ];

        for source in cases {
            let model = parse_flowchart_model_for_render(source, &meta)
                .expect("duplicate subgraph declarations should parse");

            assert_eq!(model.subgraphs.len(), 1, "{source}");
            assert!(
                model
                    .subgraphs
                    .iter()
                    .all(|subgraph| subgraph.styles.is_empty()),
                "style statements belong to FlowDB's vertex record, not FlowSubgraph: {source}"
            );
            let (_, styles) = model.effective_subgraph_css(0, &model.subgraphs[0]);
            assert_eq!(styles, ["fill:#f00"], "{source}");
        }
    }

    #[test]
    fn flowchart_elk_missing_subgraph_node_is_an_explicit_anchor_fact() {
        let meta = semantic_test_meta("flowchart-elk");
        let model =
            parse_flowchart_model_for_render("flowchart-elk TD\nsubgraph Empty\nend\n", &meta)
                .expect("empty ELK subgraph model");
        let node = model
            .nodes
            .iter()
            .find(|node| node.id == "Empty")
            .expect("ELK subgraph anchor");

        assert_eq!(node.provenance, FlowNodeProvenance::SubgraphAnchor);
    }

    #[test]
    #[cfg(feature = "compat-json")]
    fn legacy_flowchart_json_omits_typed_node_provenance() {
        let meta = flowchart_test_meta("flowchart-v2");
        let model = parse_flowchart_model_for_render(
            "flowchart TD\nsubgraph G\n  A\nend\nG --> A\n",
            &meta,
        )
        .expect("subgraph endpoint model");
        assert!(
            model
                .nodes
                .iter()
                .any(|node| node.provenance == FlowNodeProvenance::SubgraphAnchor)
        );

        let compatibility = render_model_to_compat_json(&model, &meta)
            .expect("legacy flowchart compatibility JSON");
        let nodes = compatibility
            .get("nodes")
            .and_then(Value::as_array)
            .expect("legacy nodes array");
        assert!(nodes.iter().all(|node| node.get("provenance").is_none()));
    }

    #[test]
    #[cfg(feature = "presentation")]
    fn flowchart_model_trims_parser_labels_before_entity_decode() {
        let nbsp = '\u{00a0}';
        let source = format!(
            r#"flowchart LR
Direct["{nbsp}Direct{nbsp}"]
Entity["&nbsp;Entity&nbsp;"]
Mixed["{nbsp}&nbsp;Mixed&nbsp;{nbsp}"]
Internal["A{nbsp}B"]
DirectOnly["{nbsp}"]
EntityOnly["&nbsp;"]
Shape@{{ label: "{nbsp}&nbsp;Shape&nbsp;{nbsp}", labelType: "string", shape: "rect" }}
A -->|{nbsp}Text{nbsp}| B
B -- "{nbsp}String{nbsp}" --> C
C -- "`{nbsp}Markdown{nbsp}`" --> D
D -- "{nbsp}&nbsp;MixedEdge&nbsp;{nbsp}" --> E
E -- "{nbsp}" --> F
F -- "&nbsp;" --> G
"#
        );
        let meta = SourcePresentationMetadata {
            diagram_type: "flowchart-v2".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        };
        let (model, render_label_sources) =
            parse_flowchart_model_with_render_context(&source, &meta).expect("flowchart model");
        let node_label = |id: &str| {
            model
                .nodes
                .iter()
                .find(|node| node.id == id)
                .and_then(|node| node.label.as_deref())
                .unwrap_or_else(|| panic!("missing node label for {id}"))
        };
        let node_render_label = |id: &str| {
            let node = model
                .nodes
                .iter()
                .find(|node| node.id == id)
                .unwrap_or_else(|| panic!("missing node for {id}"));
            render_label_sources
                .node_label_for_render(node)
                .unwrap_or_else(|| panic!("missing render label for {id}"))
        };
        let edge_label = |from: &str| {
            model
                .edges
                .iter()
                .find(|edge| edge.from == from)
                .unwrap_or_else(|| panic!("missing edge from {from}"))
                .label
                .as_deref()
        };
        let edge_render_label = |from: &str| {
            let (semantic_index, edge) = model
                .edges
                .iter()
                .enumerate()
                .find(|(_, edge)| edge.from == from)
                .unwrap_or_else(|| panic!("missing edge from {from}"));
            render_label_sources.edge_label_for_render(semantic_index, edge)
        };

        assert_eq!(node_label("Direct"), "Direct");
        assert_eq!(node_label("Entity"), format!("{nbsp}Entity{nbsp}"));
        assert_eq!(node_label("Mixed"), format!("{nbsp}Mixed{nbsp}"));
        assert_eq!(node_label("Internal"), format!("A{nbsp}B"));
        assert_eq!(node_label("DirectOnly"), "");
        assert_eq!(node_label("EntityOnly"), nbsp.to_string());
        assert_eq!(
            node_label("Shape"),
            format!("{nbsp}{nbsp}Shape{nbsp}{nbsp}")
        );

        assert_eq!(edge_label("A"), Some("Text"));
        assert_eq!(edge_label("B"), Some("String"));
        assert_eq!(edge_label("C"), Some("Markdown"));
        let mixed_edge_label = format!("{nbsp}MixedEdge{nbsp}");
        assert_eq!(edge_label("D"), Some(mixed_edge_label.as_str()));
        assert_eq!(edge_label("E"), Some(""));
        let nbsp_label = nbsp.to_string();
        assert_eq!(edge_label("F"), Some(nbsp_label.as_str()));

        assert_eq!(node_render_label("Direct"), "Direct");
        assert_eq!(node_render_label("Entity"), "&nbsp;Entity&nbsp;");
        assert_eq!(node_render_label("Mixed"), "&nbsp;Mixed&nbsp;");
        assert_eq!(node_render_label("DirectOnly"), "");
        assert_eq!(node_render_label("EntityOnly"), "&nbsp;");
        assert_eq!(
            node_render_label("Shape"),
            format!("{nbsp}&nbsp;Shape&nbsp;{nbsp}")
        );
        assert_eq!(edge_render_label("A"), Some("Text"));
        assert_eq!(edge_render_label("D"), Some("&nbsp;MixedEdge&nbsp;"));
        assert_eq!(edge_render_label("E"), Some(""));
        assert_eq!(edge_render_label("F"), Some("&nbsp;"));

        let duplicate_edge_source = r#"flowchart LR
X L_A_B_0@-->|first &amp; owner| Y
A -->|second &lt; owner| B
"#;
        let (duplicate_edge_model, duplicate_edge_sources) =
            parse_flowchart_model_with_render_context(duplicate_edge_source, &meta)
                .expect("duplicate-id edge model");
        assert_eq!(duplicate_edge_model.edges.len(), 2);
        assert_eq!(duplicate_edge_model.edges[0].id, "L_A_B_0");
        assert_eq!(duplicate_edge_model.edges[1].id, "L_A_B_0");
        assert_eq!(
            duplicate_edge_sources.edge_label_for_render(0, &duplicate_edge_model.edges[0]),
            Some("first &amp; owner")
        );
        assert_eq!(
            duplicate_edge_sources.edge_label_for_render(1, &duplicate_edge_model.edges[1]),
            Some("second &lt; owner")
        );

        let subgraph_source =
            format!("flowchart LR\nsubgraph SG[\"{nbsp}&nbsp;Group&nbsp;{nbsp}\"]\n  H\nend\n");
        let (subgraph_model, subgraph_render_label_sources) =
            parse_flowchart_model_with_render_context(&subgraph_source, &meta)
                .expect("subgraph model");
        let subgraph = subgraph_model
            .subgraphs
            .iter()
            .find(|subgraph| subgraph.id == "SG")
            .expect("subgraph SG");
        assert_eq!(subgraph.title, format!("{nbsp}Group{nbsp}"));
        assert_eq!(
            subgraph_render_label_sources.subgraph_title_for_render(0, subgraph),
            "&nbsp;Group&nbsp;"
        );

        let entity_subgraph_source =
            "flowchart LR\nsubgraph Entity[\"A &amp; &lt; >\"]\n  Inside\nend\n";
        let (entity_subgraph_model, entity_subgraph_render_label_sources) =
            parse_flowchart_model_with_render_context(entity_subgraph_source, &meta)
                .expect("entity subgraph model");
        let entity_subgraph = entity_subgraph_model
            .subgraphs
            .iter()
            .find(|subgraph| subgraph.id == "Entity")
            .expect("entity subgraph");
        assert_eq!(entity_subgraph.title, "A & < >");
        assert_eq!(
            entity_subgraph_render_label_sources.subgraph_title_for_render(0, entity_subgraph),
            "A &amp; &lt; &gt;"
        );

        let duplicate_subgraph_source =
            "flowchart LR\nsubgraph X[\"&nbsp;First\"]\n  A\nend\nsubgraph X[Second]\n  B\nend\n";
        let (duplicate_model, duplicate_sources) =
            parse_flowchart_model_with_render_context(duplicate_subgraph_source, &meta)
                .expect("duplicate subgraph model");
        assert_eq!(duplicate_model.subgraphs.len(), 1);
        assert_eq!(
            duplicate_sources.subgraph_title_for_render(0, &duplicate_model.subgraphs[0]),
            "&nbsp;First"
        );
        assert_eq!(duplicate_model.subgraphs[0].nodes, ["A", "B"]);

        let later_entity_source =
            "flowchart LR\nsubgraph X[First]\n  A\nend\nsubgraph X[\"&nbsp;Second\"]\n  B\nend\n";
        let (later_entity_model, later_entity_sources) =
            parse_flowchart_model_with_render_context(later_entity_source, &meta)
                .expect("duplicate subgraph model with later entity label");
        assert_eq!(
            later_entity_sources.subgraph_title_for_render(0, &later_entity_model.subgraphs[0]),
            "First"
        );
        assert_eq!(later_entity_model.subgraphs.len(), 1);
        assert_eq!(later_entity_model.subgraphs[0].nodes, ["A", "B"]);

        let punctuation_source = "flowchart LR\nsubgraph \"A;B\"\n  Bare\nend\nsubgraph \"`M;D`\"\n  Markdown\nend\nsubgraph SG[\"A]B\"]\n  Bracket\nend\n";
        let punctuation_model = parse_flowchart_model_for_render(punctuation_source, &meta)
            .expect("punctuation subgraph model");
        assert!(
            punctuation_model
                .subgraphs
                .iter()
                .any(|subgraph| subgraph.id == "A;B" && subgraph.title == "A;B")
        );
        assert!(punctuation_model.subgraphs.iter().any(|subgraph| {
            subgraph.id == "M;D"
                && subgraph.title == "M;D"
                && subgraph.label_type.as_deref() == Some("markdown")
        }));
        assert!(
            punctuation_model
                .subgraphs
                .iter()
                .any(|subgraph| subgraph.id == "SG" && subgraph.title == "A]B")
        );
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    #[cfg(feature = "presentation")]
    fn flowchart_render_label_context_stays_out_of_the_public_model_contract() {
        let parsed = crate::MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "flowchart LR\nA[\"&nbsp;A&nbsp;\"] -->|\"&nbsp;E&nbsp;\"| B\n",
                crate::ParseOptions::strict(),
            )
            .expect("parse flowchart")
            .expect("detect flowchart");
        assert_eq!(parsed.retained_render_context_bytes(), 0);

        let (_, crate::RenderSemanticModel::Flowchart(model)) = parsed.into_parts() else {
            panic!("expected Flowchart model");
        };
        assert!(model.label_sources.retained_bytes() > 0);
        let json = serde_json::to_value(&model).expect("serialize Flowchart model");
        assert!(json.get("renderLabelSources").is_none());
        assert!(json.get("render_label_sources").is_none());

        let roundtrip: FlowchartModel =
            serde_json::from_value(json).expect("deserialize Flowchart model");
        assert_eq!(roundtrip.nodes.len(), model.nodes.len());
        assert_eq!(roundtrip.edges.len(), model.edges.len());
        assert_eq!(roundtrip.nodes[0].label, model.nodes[0].label);
        assert_eq!(roundtrip.edges[0].label, model.edges[0].label);
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    #[cfg(feature = "presentation")]
    fn flowchart_render_label_context_applies_shape_sanitization_to_angle_text() {
        let parsed = crate::MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                include_str!(
                    "../../../../fixtures/flowchart/stress_flowchart_svglike_escaped_tags_025.mmd"
                ),
                crate::ParseOptions::strict(),
            )
            .expect("parse flowchart")
            .expect("detect flowchart");
        let render_context = parsed
            .flowchart_render_context()
            .expect("flowchart render context");
        let crate::RenderSemanticModel::Flowchart(model) = parsed.model() else {
            panic!("expected Flowchart model");
        };

        let comparison = model
            .nodes
            .iter()
            .find(|node| node.id == "C")
            .expect("comparison node");
        assert_eq!(comparison.label.as_deref(), Some("x &lt; y and y &gt; z"));
        assert_eq!(
            render_context.node_label_for_render(comparison),
            Some("x &lt; y and y &gt; z")
        );

        let formatted = model
            .nodes
            .iter()
            .find(|node| node.id == "D")
            .expect("formatted node");
        assert_eq!(
            render_context.node_label_for_render(formatted),
            Some("<u>under</u> and <i>italic</i>")
        );
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn flowchart_parser_families_expose_typed_editor_semantics() {
        let semantics = crate::family::diagram_type_editor_semantics("flowchart-v2")
            .expect("flowchart is an admitted family");

        assert_eq!(semantics.outline_kind(), EditorSemanticKind::Module);
        let swimlane = crate::family::diagram_type_editor_semantics("swimlane")
            .expect("swimlane is an admitted family");
        assert_eq!(swimlane.outline_kind(), EditorSemanticKind::Variable);
    }

    #[test]
    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    fn combined_flowchart_parse_propagates_preexisting_cancellation() {
        let meta = SourcePresentationMetadata {
            diagram_type: "flowchart-v2".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        };
        let control = OperationControl::new();
        control.cancel();

        assert!(matches!(
            parse_flowchart_json_and_editor_facts("flowchart TD\nA-->B\n", &meta, &control),
            Err(crate::OperationCancelled { .. })
        ));
    }

    #[test]
    fn flowchart_parse_error_cannot_mask_post_parser_cancellation() {
        let meta = semantic_test_meta("flowchart-v2");
        let control = OperationControl::new().for_phase(crate::OperationPhase::Parse);
        control.cancel_after_checkpoints(1);

        assert!(matches!(
            parse_flowchart_ast_controlled("flowchart TD\nA-->\n", &meta, &control),
            Err(crate::OperationCancelled {
                phase: crate::OperationPhase::Parse,
                reason: crate::CancelReason::Requested,
            })
        ));
        assert!(control.is_cancelled());
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn token_trace_stops_after_cancellation_during_parser_work() {
        let mut source = String::from("flowchart TD\n");
        for index in 0..512 {
            source.push_str(&format!("n{index}-->n{}\n", index + 1));
        }
        let control = OperationControl::new();
        control.cancel_after_checkpoints(2);

        assert!(matches!(
            construct_flowchart_token_trace(&source, &control),
            Err(crate::OperationCancelled { .. })
        ));
        assert!(control.is_cancelled());
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn single_large_chain_observes_cancellation_in_editor_projection() {
        let nodes = (0..256)
            .map(|index| format!("n{index}"))
            .collect::<Vec<_>>()
            .join(" & ");
        let source = format!("flowchart TD\n{nodes}\n");
        let meta = semantic_test_meta("flowchart-v2");
        let ast = parse_flowchart_ast(&source, &meta).expect("large node group should parse");
        assert!(matches!(
            ast.walk().next(),
            Some(StatementEvent::Enter(Stmt::Chain { node_groups, .. }))
                if node_groups.iter().map(Vec::len).sum::<usize>() == 256
        ));

        let control = OperationControl::new();
        control.cancel_after_checkpoints(2);
        assert!(matches!(
            editor_facts_from_flowchart_ast(&ast, &control),
            Err(crate::OperationCancelled { .. })
        ));
    }

    #[test]
    fn grouped_flowchart_edges_respect_configured_max_edges_before_expansion() {
        let mut meta = semantic_test_meta("flowchart-v2");
        meta.semantic_config = MermaidConfig::from_value(json!({"maxEdges": 6}));

        let at_limit = parse_flowchart_ast("flowchart TD\na & b --> c & d & e\n", &meta)
            .expect("an edge group at maxEdges should parse");
        let Some(StatementEvent::Enter(Stmt::Chain { edge_groups, .. })) = at_limit.walk().next()
        else {
            panic!("expected grouped edge chain");
        };
        assert_eq!(edge_groups[0].len(), 6);

        let over_limit = parse_flowchart_ast("flowchart TD\na & b --> c & d & e & f\n", &meta)
            .expect_err("an edge group over maxEdges should fail during parsing");
        assert!(
            over_limit
                .to_string()
                .contains("Flowchart edge limit exceeded")
        );
        assert!(over_limit.to_string().contains("maxEdges"));
    }

    #[test]
    fn grouped_flowchart_edge_budget_is_shared_across_chains() {
        let mut meta = semantic_test_meta("flowchart-v2");
        meta.semantic_config = MermaidConfig::from_value(json!({"maxEdges": 7}));

        let error = parse_flowchart_ast("flowchart TD\na & b --> c & d\ne & f --> g & h\n", &meta)
            .expect_err("the shared maxEdges budget should cover every chain");
        assert!(
            error
                .to_string()
                .contains("only 3 of the maxEdges budget remain")
        );
    }

    #[test]
    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    fn combined_flowchart_parser_enforces_max_edges_in_the_traced_path() {
        let mut meta = flowchart_test_meta("flowchart-v2");
        meta.effective_config =
            crate::presentation::SourcePresentationConfig::from_value(json!({"maxEdges": 3}));
        let control = OperationControl::new();

        let parsed = parse_flowchart_json_and_editor_facts(
            "flowchart TD\na & b --> c & d\n",
            &meta,
            &control,
        )
        .expect("the controlled parser should return a combined result");
        let (model, facts) = parsed.into_parts();
        let error = model.expect_err("the traced parser should reject the expanded edge group");
        assert!(error.to_string().contains("Flowchart edge limit exceeded"));
        assert!(
            facts
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("flowchart parser recovered"))
        );
    }
}
