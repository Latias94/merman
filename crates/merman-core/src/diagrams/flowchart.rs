use crate::diagram::legacy_warning_messages;
use crate::sanitize::sanitize_text;
use crate::{
    DiagramWarningFact, EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorLexemeKind,
    EditorLexemeModifiers, EditorRenamePolicy, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticRole, EditorSemanticSymbol, Error, FLOWCHART_EXPLICIT_DIRECTION_WARNING_RULE_ID,
    MermaidConfig, ParseControl, ParseControlResult, ParseMetadata, Result, SourceSpan,
    editor::{
        EditorLexemeJournal, format_lalrpop_parse_error, lalrpop_parse_diagnostic,
        lalrpop_recovery_span,
    },
};
use indexmap::IndexMap;
use serde_json::{Value, json};
#[cfg(test)]
use std::cell::Cell;
use std::collections::{HashMap, HashSet};

#[cfg(test)]
thread_local! {
    static FLOWCHART_TOKEN_TRACE_CONSTRUCTION_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_flowchart_token_trace_construction_count() {
    FLOWCHART_TOKEN_TRACE_CONSTRUCTION_COUNT.set(0);
}

#[cfg(test)]
pub(crate) fn flowchart_token_trace_construction_count() -> usize {
    FLOWCHART_TOKEN_TRACE_CONSTRUCTION_COUNT.get()
}

#[cfg(test)]
pub(crate) fn reset_flowchart_accessibility_scan_count() {
    accessibility::reset_flowchart_accessibility_scan_count();
}

#[cfg(test)]
pub(crate) fn flowchart_accessibility_scan_count() -> usize {
    accessibility::flowchart_accessibility_scan_count()
}

include_checked_in_lalrpop_parser!(
    #[allow(
        clippy::empty_line_after_outer_attr,
        clippy::large_enum_variant,
        clippy::type_complexity,
        clippy::result_large_err
    )]
    flowchart_grammar,
    "flowchart_grammar.rs"
);

mod accessibility;
mod ast;
mod build;
mod lex;
mod lexeme;
mod lexer;
mod lexer_iter;
mod link;
mod model;
mod semantic;
mod shape_data;
mod subgraph;
mod text;
mod tokens;

use text::{
    is_ecmascript_trim_char, parse_edge_label_text, parse_label_text, strip_wrapping_backticks,
    title_kind_str, trim_flowdb_label_text, unquote,
};

#[doc(hidden)]
pub use model::FlowchartRenderLabelSources;
pub use model::{FlowEdge, FlowEdgeDefaults, FlowNode, FlowSubgraph, FlowchartModel};

pub(crate) use model::{
    Edge, EdgeDefaults, LabeledText, LinkToken, Node, SubgraphHeader, TitleKind,
};

pub(crate) use ast::{
    ClassAssignStmt, ClassDefStmt, ClickAction, ClickStmt, FlowchartAst, LinkStylePos,
    LinkStyleStmt, Stmt, StyleStmt, SubgraphBlock,
};

pub(crate) use lexeme::FlowchartLexemeComponent;
pub(crate) use tokens::{ArrowToken, DirectionStatementToken, LexError, NodeLabelToken, Tok};

use accessibility::{
    FlowchartAccessibilityScan, FlowchartAccessibilityStatement, scan_flowchart_accessibility,
    scan_flowchart_accessibility_controlled,
};
use build::FlowchartBuildState;
use lexer::Lexer;
use link::{destruct_end_link, destruct_start_link};
use semantic::{FlowchartSemanticContext, apply_semantic_statements};
use shape_data::{
    apply_shape_data_value_to_node, public_pinned_shape_names, value_to_bool, value_to_string,
};
use subgraph::SubgraphBuilder;

pub(crate) fn is_valid_editor_node_id(candidate: &str) -> bool {
    let mut lexer = Lexer::new(candidate);
    matches!(
        (lexer.next(), lexer.next()),
        (Some(Ok((0, Tok::Id(_), end))), None) if end == candidate.len()
    )
}

#[derive(Debug, Clone)]
pub(crate) struct FlowSubGraph {
    pub id: String,
    pub nodes: Vec<String>,
    pub title: String,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
    pub dir: Option<String>,
    pub has_explicit_dir: bool,
    pub label_type: String,
}

struct FlowchartSemanticSource {
    keyword: String,
    direction: Option<String>,
    acc_title: Option<String>,
    acc_descr: Option<String>,
    class_defs: IndexMap<String, Vec<String>>,
    tooltips: HashMap<String, String>,
    edge_defaults: EdgeDefaults,
    vertex_calls: Vec<String>,
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    subgraphs: Vec<FlowSubGraph>,
    warning_facts: Vec<DiagramWarningFact>,
}

pub(crate) fn parse_flowchart(code: &str, meta: &ParseMetadata) -> Result<Value> {
    parse_flowchart_with_warning_facts(code, meta)
        .map(crate::family::WarningSemanticParse::into_model)
}

pub(crate) fn parse_flowchart_with_warning_facts(
    code: &str,
    meta: &ParseMetadata,
) -> Result<crate::family::WarningSemanticParse> {
    let model = parse_flowchart_semantic_source(code, meta)?.into_render_model(meta)?;
    let compatibility = render_model_to_compat_json(&model, meta)?;
    Ok(crate::family::WarningSemanticParse::new(
        compatibility,
        model.warning_facts,
    ))
}

pub(crate) fn parse_flowchart_json_and_editor_facts(
    code: &str,
    meta: &ParseMetadata,
    control: &ParseControl,
) -> ParseControlResult<crate::family::CombinedSemanticParse> {
    control.checkpoint()?;
    let FlowchartAccessibilityScan {
        parser_input: code,
        title: acc_title,
        description: acc_descr,
        statements: accessibility_statements,
    } = scan_flowchart_accessibility_controlled(code, control)?;
    control.checkpoint()?;
    let trace = construct_flowchart_token_trace(&code, &accessibility_statements, control)?;
    let construction = match parse_flowchart_ast_from_trace(&trace, control)? {
        Ok(ast) => {
            let mut facts = editor_facts_from_flowchart_ast(&ast, control)?;
            control.checkpoint()?;
            collect_accessibility_directive_prefixes(
                &accessibility_statements,
                &mut facts,
                control,
            )?;
            collect_expected_syntax_from_tokens(&code, trace.editor_tokens(), &mut facts, control)?;
            facts.replace_family_lexemes(trace.lexemes);
            let (model, warning_facts) = match parse_flowchart_semantic_source_from_ast_controlled(
                ast, acc_title, acc_descr, meta, control,
            )? {
                Ok(source) => match source.into_render_model_controlled(meta, control)? {
                    Ok(model) => {
                        let compatibility = render_model_to_compat_json(&model, meta);
                        (compatibility, model.warning_facts)
                    }
                    Err(error) => (Err(error), Vec::new()),
                },
                Err(error) => (Err(error), Vec::new()),
            };
            control.checkpoint()?;
            Ok((model, facts, warning_facts))
        }
        Err(error) => {
            let facts = flowchart_recovery_facts(
                &code,
                trace,
                &accessibility_statements,
                error.as_ref(),
                control,
            )?;
            let error = Error::diagram_parse_diagnostic(
                meta.diagram_type.clone(),
                flowchart_parse_diagnostic(error.as_ref(), &code, &facts),
            );
            Err(crate::family::CombinedSemanticFailure::new(error, facts))
        }
    };
    let parsed = crate::family::CombinedSemanticParse::from_construction_with_warning_facts(
        construction,
        |parts| parts,
        crate::family::CombinedSemanticFailure::into_parts,
    );
    control.checkpoint()?;
    Ok(parsed)
}

#[cfg(test)]
pub(crate) fn parse_flowchart_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<FlowchartModel> {
    parse_flowchart_semantic_source(code, meta)?.into_render_model(meta)
}

pub(crate) fn parse_flowchart_model_with_render_context(
    code: &str,
    meta: &ParseMetadata,
) -> Result<(FlowchartModel, FlowchartRenderLabelSources)> {
    parse_flowchart_semantic_source(code, meta)?.into_render_model_parts(meta)
}

pub(crate) fn render_model_to_compat_json(
    model: &FlowchartModel,
    meta: &ParseMetadata,
) -> Result<Value> {
    let mut value =
        serde_json::to_value(model).expect("Flowchart typed model must remain JSON-serializable");
    let root = value.as_object_mut().ok_or_else(|| {
        Error::diagram_parse_fallback(
            meta.diagram_type.clone(),
            "flowchart typed model did not serialize to an object".to_string(),
        )
    })?;
    root.insert("type".to_string(), Value::String(meta.diagram_type.clone()));
    if model
        .warning_facts
        .iter()
        .any(|fact| fact.rule_id == FLOWCHART_EXPLICIT_DIRECTION_WARNING_RULE_ID)
    {
        root.insert("direction".to_string(), Value::Null);
    }
    if !model.warning_facts.is_empty() {
        root.insert(
            "warnings".to_string(),
            json!(legacy_warning_messages(&model.warning_facts)),
        );
    }
    Ok(value)
}

pub fn flowchart_public_shape_names() -> impl Iterator<Item = &'static str> {
    public_pinned_shape_names()
}

fn parse_flowchart_semantic_source(
    code: &str,
    meta: &ParseMetadata,
) -> Result<FlowchartSemanticSource> {
    let FlowchartAccessibilityScan {
        parser_input: code,
        title: acc_title,
        description: acc_descr,
        ..
    } = scan_flowchart_accessibility(code);
    let ast = parse_flowchart_ast(&code, meta)?;
    let control = ParseControl::new();
    parse_flowchart_semantic_source_from_ast_controlled(ast, acc_title, acc_descr, meta, &control)
        .expect("a private parse control cannot be cancelled")
}

fn parse_flowchart_semantic_source_from_ast_controlled(
    ast: FlowchartAst,
    acc_title: Option<String>,
    acc_descr: Option<String>,
    meta: &ParseMetadata,
    control: &ParseControl,
) -> ParseControlResult<Result<FlowchartSemanticSource>> {
    let shape_data_documents = prepare_flowchart_shape_data(&ast.statements, control)?;
    control.checkpoint()?;
    let inherit_dir = meta
        .effective_config
        .as_value()
        .get("flowchart")
        .and_then(|v| v.get("inheritDir"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let mut builder = SubgraphBuilder::new(inherit_dir, ast.direction.clone());
    builder.visit_statements(&ast.statements, control)?;

    let subgraph_ids: HashSet<String> = builder
        .subgraphs
        .iter()
        .map(|subgraph| subgraph.id.clone())
        .collect();

    let mut build = FlowchartBuildState::new(subgraph_ids);
    if let Err(error) = build.add_statements(&ast.statements, &shape_data_documents, control)? {
        return Ok(Err(Error::diagram_parse_fallback(
            meta.diagram_type.clone(),
            error,
        )));
    }
    let FlowchartBuildState {
        nodes,
        edges,
        vertex_calls,
        warning_facts: build_warning_facts,
        ..
    } = build;
    let mut nodes = nodes;
    let mut edges = edges;

    let mut class_defs: IndexMap<String, Vec<String>> = IndexMap::new();
    let mut tooltips: HashMap<String, String> = HashMap::new();
    let mut edge_defaults = EdgeDefaults {
        style: Vec::new(),
        interpolate: None,
    };

    let mut node_index: HashMap<String, usize> = HashMap::new();
    for (idx, n) in nodes.iter().enumerate() {
        if idx % 128 == 0 {
            control.checkpoint()?;
        }
        node_index.insert(n.id.clone(), idx);
    }
    let mut subgraph_index: HashMap<String, usize> = HashMap::new();
    for (idx, sg) in builder.subgraphs.iter().enumerate() {
        if idx % 128 == 0 {
            control.checkpoint()?;
        }
        subgraph_index.insert(sg.id.clone(), idx);
    }

    let security_level_loose = meta.effective_config.get_str("securityLevel") == Some("loose");
    {
        let mut semantic_ctx = FlowchartSemanticContext {
            nodes: &mut nodes,
            node_index: &mut node_index,
            edges: &mut edges,
            subgraphs: &mut builder.subgraphs,
            subgraph_index: &mut subgraph_index,
            class_defs: &mut class_defs,
            tooltips: &mut tooltips,
            edge_defaults: &mut edge_defaults,
            security_level_loose,
            diagram_type: &meta.diagram_type,
            config: &meta.effective_config,
            shape_data_documents: &shape_data_documents,
            control,
        };
        if let Err(error) = apply_semantic_statements(&ast.statements, &mut semantic_ctx)? {
            return Ok(Err(error));
        }
    }

    let direction = ast.direction;
    let mut warning_facts = build_warning_facts;
    warning_facts.extend(flowchart_warning_facts(&direction, ast.header_span));
    control.checkpoint()?;
    Ok(Ok(FlowchartSemanticSource {
        keyword: ast.keyword,
        direction,
        acc_descr,
        acc_title,
        class_defs,
        tooltips,
        edge_defaults,
        vertex_calls,
        nodes,
        edges,
        subgraphs: builder.subgraphs,
        warning_facts,
    }))
}

fn prepare_flowchart_shape_data(
    statements: &[Stmt],
    control: &ParseControl,
) -> ParseControlResult<HashMap<String, std::result::Result<Value, String>>> {
    let mut documents = HashMap::new();
    let mut stack = vec![statements.iter()];
    let mut visited = 0usize;

    while let Some(iter) = stack.last_mut() {
        let Some(statement) = iter.next() else {
            stack.pop();
            continue;
        };
        if visited.is_multiple_of(128) {
            control.checkpoint()?;
        }
        visited = visited.saturating_add(1);

        match statement {
            Stmt::Chain { nodes, .. } => {
                for (index, node) in nodes.iter().enumerate() {
                    if index % 128 == 0 {
                        control.checkpoint()?;
                    }
                    if let Some(source) = node.shape_data.as_deref() {
                        prepare_flowchart_shape_data_document(source, control, &mut documents)?;
                    }
                }
            }
            Stmt::Node(node) => {
                if let Some(source) = node.shape_data.as_deref() {
                    prepare_flowchart_shape_data_document(source, control, &mut documents)?;
                }
            }
            Stmt::Subgraph(subgraph) => stack.push(subgraph.statements.iter()),
            Stmt::ShapeData { yaml, .. } => {
                prepare_flowchart_shape_data_document(yaml, control, &mut documents)?;
            }
            Stmt::Direction(_)
            | Stmt::Style(_)
            | Stmt::ClassDef(_)
            | Stmt::ClassAssign(_)
            | Stmt::Click(_)
            | Stmt::LinkStyle(_) => {}
        }
    }

    control.checkpoint()?;
    Ok(documents)
}

fn prepare_flowchart_shape_data_document(
    source: &str,
    control: &ParseControl,
    documents: &mut HashMap<String, std::result::Result<Value, String>>,
) -> ParseControlResult<()> {
    if documents.contains_key(source) {
        return control.checkpoint();
    }
    let document = crate::inline_config::parse_mermaid_inline_object_controlled(source, control)?;
    documents.insert(source.to_string(), document);
    Ok(())
}

fn parse_flowchart_ast(code: &str, meta: &ParseMetadata) -> Result<FlowchartAst> {
    flowchart_grammar::FlowchartAstParser::new()
        .parse(Lexer::new(code))
        .map_err(|e| {
            Error::diagram_parse_diagnostic(
                meta.diagram_type.clone(),
                lalrpop_parse_diagnostic(&e, code.len()),
            )
        })
}

type FlowchartAstParseError = lalrpop_util::ParseError<usize, Tok, LexError>;
type FlowchartToken = (usize, Tok, usize);
type FlowchartLexerItem = std::result::Result<FlowchartToken, LexError>;

enum FlowchartTracedItem {
    Token(FlowchartToken),
    RecoveredToken {
        token: FlowchartToken,
        strict_error: LexError,
    },
    LexerError(LexError),
}

struct FlowchartTokenTrace {
    items: Vec<FlowchartTracedItem>,
    lexemes: crate::editor::EditorLexemeBatchResult,
}

impl FlowchartTokenTrace {
    fn parser_items<'a>(
        &'a self,
        control: &'a ParseControl,
    ) -> impl Iterator<Item = FlowchartLexerItem> + 'a {
        let mut emitted = 0usize;
        self.items
            .iter()
            .take_while(move |_| {
                let active = !emitted.is_multiple_of(128) || !control.is_cancelled();
                emitted = emitted.saturating_add(1);
                active
            })
            .map(|item| match item {
                FlowchartTracedItem::Token(token) => Ok(token.clone()),
                FlowchartTracedItem::RecoveredToken { strict_error, .. }
                | FlowchartTracedItem::LexerError(strict_error) => Err(strict_error.clone()),
            })
    }

    fn editor_tokens(&self) -> impl Iterator<Item = &FlowchartToken> {
        self.items.iter().filter_map(|item| match item {
            FlowchartTracedItem::Token(token)
            | FlowchartTracedItem::RecoveredToken { token, .. } => Some(token),
            FlowchartTracedItem::LexerError(_) => None,
        })
    }
}

fn construct_flowchart_token_trace(
    code: &str,
    accessibility_statements: &[FlowchartAccessibilityStatement],
    control: &ParseControl,
) -> ParseControlResult<FlowchartTokenTrace> {
    #[cfg(test)]
    FLOWCHART_TOKEN_TRACE_CONSTRUCTION_COUNT.set(
        FLOWCHART_TOKEN_TRACE_CONSTRUCTION_COUNT
            .get()
            .saturating_add(1),
    );

    let mut journal = EditorLexemeJournal::family_lexer(code);
    for (index, component) in accessibility_statements
        .iter()
        .flat_map(|statement| statement.lexemes.iter())
        .enumerate()
    {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        journal.push(component.kind, EditorLexemeModifiers::NONE, component.span);
    }
    let mut items = Vec::new();
    for item in Lexer::recovering(code) {
        if items.len() % 128 == 0 {
            control.checkpoint()?;
        }
        match item {
            Ok(mut token @ (start, _, end)) => {
                record_flowchart_lexeme(&mut journal, &token.1, start, end);
                let strict_error = match &mut token.1 {
                    Tok::NodeLabel(label) => label.recovery_error.take(),
                    Tok::DirectionStmt(direction) => direction.recovery_error.take(),
                    Tok::Arrow(arrow) => arrow.recovery_error.take(),
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
        lexemes: journal.finish(),
    })
}

fn parse_flowchart_ast_from_trace(
    trace: &FlowchartTokenTrace,
    control: &ParseControl,
) -> ParseControlResult<std::result::Result<FlowchartAst, Box<FlowchartAstParseError>>> {
    control.checkpoint()?;
    let parsed = flowchart_grammar::FlowchartAstParser::new()
        .parse(trace.parser_items(control))
        .map_err(Box::new);
    control.checkpoint()?;
    Ok(parsed)
}

fn record_flowchart_lexeme(
    journal: &mut EditorLexemeJournal<'_>,
    token: &Tok,
    start: usize,
    end: usize,
) {
    let components = match token {
        Tok::DirectionStmt(token) => Some(token.lexeme_components.as_slice()),
        Tok::NodeLabel(token) => Some(token.lexeme_components.as_slice()),
        Tok::Arrow(arrow) if !arrow.lexeme_components.is_empty() => {
            Some(arrow.lexeme_components.as_slice())
        }
        Tok::EdgeLabel(label) => Some(label.lexeme_components.as_slice()),
        Tok::SubgraphHeader(header) => Some(header.lexeme_components.as_slice()),
        Tok::StyleStmt(stmt) => Some(stmt.lexeme_components.as_slice()),
        Tok::ClassDefStmt(stmt) => Some(stmt.lexeme_components.as_slice()),
        Tok::ClassAssignStmt(stmt) => Some(stmt.lexeme_components.as_slice()),
        Tok::ClickStmt(stmt) => Some(stmt.lexeme_components.as_slice()),
        Tok::LinkStyleStmt(stmt) => Some(stmt.lexeme_components.as_slice()),
        _ => None,
    };
    if let Some(components) = components {
        for component in components {
            journal.push(component.kind, EditorLexemeModifiers::NONE, component.span);
        }
        return;
    }

    let span = SourceSpan::new(start, end);
    let kind = match token {
        Tok::KwGraph
        | Tok::KwFlowchart
        | Tok::KwFlowchartElk
        | Tok::KwSwimlane
        | Tok::KwSubgraph
        | Tok::KwEnd => EditorLexemeKind::Keyword,
        Tok::Amp | Tok::StyleSep | Tok::Arrow(_) => EditorLexemeKind::Operator,
        Tok::Direction(_) => EditorLexemeKind::Literal,
        Tok::ShapeData(_) => EditorLexemeKind::Style,
        Tok::Id(_) => EditorLexemeKind::Identifier,
        Tok::EdgeId(_) => {
            if start + 1 < end {
                journal.push(
                    EditorLexemeKind::Identifier,
                    EditorLexemeModifiers::NONE,
                    SourceSpan::new(start, end - 1),
                );
            }
            journal.push(
                EditorLexemeKind::Operator,
                EditorLexemeModifiers::NONE,
                SourceSpan::new(end - 1, end),
            );
            return;
        }
        Tok::DirectionStmt(_)
        | Tok::NodeLabel(_)
        | Tok::EdgeLabel(_)
        | Tok::SubgraphHeader(_)
        | Tok::StyleStmt(_)
        | Tok::ClassDefStmt(_)
        | Tok::ClassAssignStmt(_)
        | Tok::ClickStmt(_)
        | Tok::LinkStyleStmt(_) => unreachable!("compound tokens return above"),
        Tok::Sep => return,
    };
    journal.push(kind, EditorLexemeModifiers::NONE, span);
}

fn flowchart_warning_facts(
    direction: &Option<String>,
    header_span: crate::SourceSpan,
) -> Vec<DiagramWarningFact> {
    if direction.is_some() {
        return Vec::new();
    }

    vec![
        DiagramWarningFact::new(
            FLOWCHART_EXPLICIT_DIRECTION_WARNING_RULE_ID,
            "flowchart headers should declare an explicit direction such as `TB`, `TD`, `BT`, `LR`, or `RL`",
        )
        .with_span(header_span)
        .with_fix_span(crate::SourceSpan::new(header_span.end, header_span.end)),
    ]
}

fn editor_facts_from_flowchart_ast(
    ast: &FlowchartAst,
    control: &ParseControl,
) -> ParseControlResult<EditorSemanticFacts> {
    let mut facts = EditorSemanticFacts::new();
    collect_editor_facts_from_statements(&ast.statements, &mut facts, control)?;
    Ok(facts)
}

fn recover_flowchart_editor_facts_from_tokens(
    code: &str,
    trace: FlowchartTokenTrace,
    control: &ParseControl,
) -> ParseControlResult<EditorSemanticFacts> {
    let mut facts = EditorSemanticFacts::new();
    facts.mark_recovered();
    let mut collector = FlowchartRecoveryFactCollector::default();
    for (index, (start, token, end)) in trace.editor_tokens().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        collector.accept(code, token, *start, *end, &mut facts);
    }
    collector.finish(code.len(), &mut facts);
    for item in &trace.items {
        let error = match item {
            FlowchartTracedItem::RecoveredToken { strict_error, .. }
            | FlowchartTracedItem::LexerError(strict_error) => Some(strict_error),
            FlowchartTracedItem::Token(_) => None,
        };
        if let Some(error) = error {
            push_flowchart_expected_syntax(&mut facts, error.expected_syntax.iter().copied());
        }
        if let Some(prefix) = error.and_then(|error| error.directive_prefix) {
            facts.push_directive_prefix(prefix);
        }
    }
    facts.replace_family_lexemes(trace.lexemes);

    control.checkpoint()?;
    Ok(facts)
}

fn flowchart_recovery_facts(
    parser_code: &str,
    trace: FlowchartTokenTrace,
    accessibility_statements: &[FlowchartAccessibilityStatement],
    error: &FlowchartAstParseError,
    control: &ParseControl,
) -> ParseControlResult<EditorSemanticFacts> {
    let mut facts = recover_flowchart_editor_facts_from_tokens(parser_code, trace, control)?;
    collect_accessibility_directive_prefixes(accessibility_statements, &mut facts, control)?;
    let span = flowchart_eof_recovery_insertion(error, parser_code, &facts)
        .map(|insertion| SourceSpan::new(insertion, insertion))
        .unwrap_or_else(|| lalrpop_recovery_span(error, parser_code.len()));
    facts.mark_recovered_from_parse_error(
        format!(
            "flowchart parser recovered after parse error: {}",
            format_lalrpop_parse_error(error)
        ),
        Some(span),
    );
    control.checkpoint()?;
    Ok(facts)
}

fn flowchart_parse_diagnostic(
    error: &FlowchartAstParseError,
    code: &str,
    facts: &EditorSemanticFacts,
) -> crate::ParseDiagnostic {
    let diagnostic = lalrpop_parse_diagnostic(error, code.len());
    match flowchart_eof_recovery_insertion(error, code, facts) {
        Some(insertion) => diagnostic.map_span(|_| SourceSpan::new(insertion, insertion)),
        None => diagnostic,
    }
}

fn flowchart_eof_recovery_insertion(
    error: &FlowchartAstParseError,
    code: &str,
    facts: &EditorSemanticFacts,
) -> Option<usize> {
    if !matches!(error, lalrpop_util::ParseError::UnrecognizedEof { .. }) {
        return None;
    }
    let insertion = code.trim_end_matches(['\r', '\n']).len();
    (insertion < code.len()
        && facts.expected_syntax.iter().any(|expected| {
            matches!(
                expected.kind,
                EditorExpectedSyntaxKind::NodeIdentifier
                    | EditorExpectedSyntaxKind::FlowchartOperator
            ) && expected.span.end == code.len()
        }))
    .then_some(insertion)
}

fn collect_expected_syntax_from_tokens<'a>(
    code: &str,
    tokens: impl Iterator<Item = &'a FlowchartToken>,
    facts: &mut EditorSemanticFacts,
    control: &ParseControl,
) -> ParseControlResult<()> {
    for (index, (start, token, end)) in tokens.enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        match token {
            Tok::NodeLabel(label) => {
                if let Some(trigger_span) = label.trigger_span {
                    push_flowchart_shape_trigger_expected_syntax(trigger_span, facts);
                }
            }
            Tok::ShapeData(_) => {
                push_flowchart_shape_value_expected_syntax(code, *start, *end, facts)
            }
            Tok::DirectionStmt(stmt) => {
                push_flowchart_direction_value_expected_syntax(stmt.selection, facts)
            }
            _ => {}
        }
    }
    control.checkpoint()
}

#[derive(Debug, Default)]
struct FlowchartRecoveryFactCollector {
    pending_node_identifier: Option<FlowchartRecoveryTargetState>,
}

#[derive(Debug, Clone, Copy)]
enum FlowchartRecoveryTargetState {
    Awaiting(SourceSpan),
    Sealed(SourceSpan),
}

impl FlowchartRecoveryFactCollector {
    fn accept(
        &mut self,
        code: &str,
        token: &Tok,
        start: usize,
        end: usize,
        facts: &mut EditorSemanticFacts,
    ) {
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
            Tok::Sep => TokenKind::Sep,
            _ => TokenKind::Other,
        };
        collect_editor_fact_from_token(code, token, start, end, facts);
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

    fn finish(self, code_len: usize, facts: &mut EditorSemanticFacts) {
        let Some(state) = self.pending_node_identifier else {
            return;
        };

        let span = match state {
            FlowchartRecoveryTargetState::Awaiting(mut span) => {
                span.end = code_len;
                span
            }
            FlowchartRecoveryTargetState::Sealed(span) => span,
        };

        if span.end >= span.start {
            facts.push_expected_syntax(EditorExpectedSyntax::new(
                EditorExpectedSyntaxKind::NodeIdentifier,
                span,
            ));
        }
    }
}

fn collect_editor_fact_from_token(
    code: &str,
    token: &Tok,
    start: usize,
    end: usize,
    facts: &mut EditorSemanticFacts,
) {
    match token {
        Tok::Id(id) => push_flowchart_token_symbol(facts, id, start, end),
        Tok::SubgraphHeader(header) => {
            push_flowchart_header_symbol(facts, header);
        }
        Tok::NodeLabel(label) => {
            if let Some(trigger_span) = label.trigger_span {
                push_flowchart_shape_trigger_expected_syntax(trigger_span, facts);
            }
            push_flowchart_labeled_payload_symbol(
                facts,
                &label.text,
                Some(SourceSpan::new(start, end)),
                "flowchart node label",
            )
        }
        Tok::EdgeLabel(label) => push_flowchart_labeled_payload_symbol(
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
        | Tok::Amp
        | Tok::StyleSep
        | Tok::Direction(_) => {}
        Tok::DirectionStmt(stmt) => {
            push_flowchart_direction_value_expected_syntax(stmt.selection, facts)
        }
        Tok::Arrow(_) | Tok::EdgeId(_) => {}
        Tok::ShapeData(_) => push_flowchart_shape_value_expected_syntax(code, start, end, facts),
    }
}

fn collect_editor_facts_from_statements(
    statements: &[Stmt],
    facts: &mut EditorSemanticFacts,
    control: &ParseControl,
) -> ParseControlResult<()> {
    let mut emitted_edge_label_spans = HashSet::new();
    let mut seen_edge_ids = HashSet::new();
    collect_editor_facts_from_statements_with_seen_edges(
        statements,
        facts,
        &mut emitted_edge_label_spans,
        &mut seen_edge_ids,
        control,
    )
}

fn collect_editor_facts_from_statements_with_seen_edges(
    statements: &[Stmt],
    facts: &mut EditorSemanticFacts,
    emitted_edge_label_spans: &mut HashSet<(usize, usize)>,
    seen_edge_ids: &mut HashSet<String>,
    control: &ParseControl,
) -> ParseControlResult<()> {
    let mut stack = vec![statements.iter()];
    let mut visited = 0usize;
    while let Some(iter) = stack.last_mut() {
        let Some(stmt) = iter.next() else {
            stack.pop();
            continue;
        };
        if visited.is_multiple_of(128) {
            control.checkpoint()?;
        }
        visited = visited.saturating_add(1);

        match stmt {
            Stmt::Chain { nodes, edges } => {
                for (index, node) in nodes.iter().enumerate() {
                    if index % 128 == 0 {
                        control.checkpoint()?;
                    }
                    push_flowchart_node_symbol(facts, node);
                }
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
            Stmt::Node(node) => push_flowchart_node_symbol(facts, node),
            Stmt::Subgraph(subgraph) => {
                push_flowchart_subgraph_symbol(facts, subgraph);
                stack.push(subgraph.statements.iter());
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
                    push_flowchart_span_symbol(
                        facts,
                        target,
                        "flowchart node",
                        EditorSemanticKind::Module,
                        *target_span,
                        EditorSemanticRole::Entity,
                    );
                }
            }
            Stmt::Direction(_) => {}
        }
    }
    control.checkpoint()
}

fn push_flowchart_node_symbol(facts: &mut EditorSemanticFacts, node: &Node) {
    if let Some(span) = node.id_span {
        facts.push_symbol(
            EditorSemanticSymbol::new(
                node.id.clone(),
                Some("flowchart node".to_string()),
                EditorSemanticKind::Module,
                span,
                span,
            )
            .with_rename_policy(EditorRenamePolicy::FlowchartNodeId),
        );
    }

    if let Some(label) = node.label.as_deref() {
        push_flowchart_payload_symbol(
            facts,
            label,
            "flowchart node label",
            node.label_span,
            node.label_selection,
        );
    }
}

fn push_flowchart_edge_label_symbol(
    facts: &mut EditorSemanticFacts,
    edge: &Edge,
    emitted_spans: &mut HashSet<(usize, usize)>,
) {
    let Some(label) = edge.label.as_deref() else {
        return;
    };
    let Some(span) = edge.label_span else {
        return;
    };
    if !emitted_spans.insert((span.start, span.end)) {
        return;
    }
    push_flowchart_payload_symbol(
        facts,
        label,
        "flowchart edge label",
        Some(span),
        edge.label_selection,
    );
}

fn push_flowchart_style_stmt_facts(facts: &mut EditorSemanticFacts, stmt: &StyleStmt) {
    facts.push_directive_prefix("style");
    push_flowchart_expected_syntax(facts, stmt.editor_evidence.iter());
    push_flowchart_span_symbol(
        facts,
        &stmt.target,
        "flowchart style target",
        EditorSemanticKind::Module,
        stmt.target_span,
        EditorSemanticRole::Entity,
    );
    if let (Some(text), Some(span)) = (stmt.styles_text.as_deref(), stmt.styles_span) {
        push_flowchart_payload_symbol(facts, text, "flowchart style", Some(span), Some(span));
    }
}

fn push_flowchart_classdef_stmt_facts(facts: &mut EditorSemanticFacts, stmt: &ClassDefStmt) {
    facts.push_directive_prefix("classDef");
    push_flowchart_expected_syntax(facts, stmt.editor_evidence.iter());
    for (id, span) in stmt.ids.iter().zip(stmt.id_spans.iter().copied()) {
        push_flowchart_span_symbol(
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
            facts,
            text,
            "flowchart class definition style",
            Some(span),
            Some(span),
        );
    }
}

fn push_flowchart_class_assign_stmt_facts(facts: &mut EditorSemanticFacts, stmt: &ClassAssignStmt) {
    facts.push_directive_prefix("class");
    push_flowchart_expected_syntax(facts, stmt.editor_evidence.iter());
    for (target, span) in stmt.targets.iter().zip(stmt.target_spans.iter().copied()) {
        push_flowchart_span_symbol(
            facts,
            target,
            "flowchart class target",
            EditorSemanticKind::Module,
            Some(span),
            EditorSemanticRole::Entity,
        );
    }
    push_flowchart_span_symbol(
        facts,
        &stmt.class_name,
        "flowchart class name",
        EditorSemanticKind::Property,
        stmt.class_name_span,
        EditorSemanticRole::Payload,
    );
}

fn push_flowchart_click_stmt_facts(facts: &mut EditorSemanticFacts, stmt: &ClickStmt) {
    facts.push_directive_prefix("click");
    push_flowchart_expected_syntax(facts, stmt.editor_evidence.iter());
    push_flowchart_expected_syntax(facts, stmt.interaction_evidence.iter());
}

fn push_flowchart_expected_syntax(
    facts: &mut EditorSemanticFacts,
    expected_syntax: impl IntoIterator<Item = EditorExpectedSyntax>,
) {
    for expected in expected_syntax {
        facts.push_expected_syntax(expected);
    }
}

fn push_flowchart_span_symbol(
    facts: &mut EditorSemanticFacts,
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
    facts.push_symbol(if role == EditorSemanticRole::Entity {
        symbol.with_rename_policy(EditorRenamePolicy::FlowchartNodeId)
    } else {
        symbol
    });
}

fn push_flowchart_labeled_payload_symbol(
    facts: &mut EditorSemanticFacts,
    label: &LabeledText,
    fallback_span: Option<SourceSpan>,
    detail: &'static str,
) {
    push_flowchart_payload_symbol(
        facts,
        &label.text,
        detail,
        label.span.or(fallback_span),
        label.selection,
    );
}

fn push_flowchart_payload_symbol(
    facts: &mut EditorSemanticFacts,
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

fn push_flowchart_shape_value_expected_syntax(
    code: &str,
    start: usize,
    end: usize,
    facts: &mut EditorSemanticFacts,
) {
    let Some(span) = shape_value_expected_span(code, start, end) else {
        return;
    };

    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::ShapeValue,
        span,
    ));
}

fn push_flowchart_direction_value_expected_syntax(
    span: SourceSpan,
    facts: &mut EditorSemanticFacts,
) {
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::FlowchartDirectionValue,
        span,
    ));
}

fn push_flowchart_shape_trigger_expected_syntax(span: SourceSpan, facts: &mut EditorSemanticFacts) {
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::ShapeTrigger,
        span,
    ));
}

pub(super) fn shape_value_expected_span(
    code: &str,
    start: usize,
    end: usize,
) -> Option<SourceSpan> {
    let raw = code.get(start..end)?;
    let body = raw.strip_prefix("@{")?;
    let body = body.strip_suffix('}').unwrap_or(body);
    let body_base = start + 2;
    let mut pos = 0usize;
    let mut in_string: Option<char> = None;
    let mut depth = 0usize;

    while pos < body.len() {
        let Some(ch) = body[pos..].chars().next() else {
            break;
        };
        if let Some(quote) = in_string {
            if ch == '\\' {
                pos += ch.len_utf8();
                if pos < body.len() {
                    let Some(escaped) = body[pos..].chars().next() else {
                        break;
                    };
                    pos += escaped.len_utf8();
                }
                continue;
            }
            if ch == quote {
                in_string = None;
            }
            pos += ch.len_utf8();
            continue;
        }

        match ch {
            '"' | '\'' => {
                in_string = Some(ch);
                pos += ch.len_utf8();
            }
            '{' | '[' | '(' => {
                depth += 1;
                pos += ch.len_utf8();
            }
            '}' => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
                pos += ch.len_utf8();
            }
            ']' | ')' => {
                depth = depth.saturating_sub(1);
                pos += ch.len_utf8();
            }
            _ => {
                if depth == 0 && body[pos..].starts_with("shape") && shape_key_boundary(body, pos) {
                    let mut key_end = pos + "shape".len();
                    while let Some(ch) = body[key_end..].chars().next() {
                        if ch.is_whitespace() {
                            key_end += ch.len_utf8();
                        } else {
                            break;
                        }
                    }
                    if body[key_end..].starts_with(':') {
                        let mut value_start = key_end + 1;
                        while let Some(ch) = body[value_start..].chars().next() {
                            if ch.is_whitespace() {
                                value_start += ch.len_utf8();
                            } else {
                                break;
                            }
                        }
                        let value_end = shape_value_end(body, value_start);
                        return Some(SourceSpan::new(
                            body_base + value_start,
                            body_base + value_end,
                        ));
                    }
                }
                pos += ch.len_utf8();
            }
        }
    }

    None
}

fn collect_accessibility_directive_prefixes(
    statements: &[FlowchartAccessibilityStatement],
    facts: &mut EditorSemanticFacts,
    control: &ParseControl,
) -> ParseControlResult<()> {
    for (index, statement) in statements.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        if statement.complete {
            facts.push_directive_prefix(statement.directive.prefix());
        }
    }
    control.checkpoint()
}

fn shape_key_boundary(body: &str, pos: usize) -> bool {
    let before = if pos == 0 {
        None
    } else {
        body[..pos].chars().next_back()
    };
    let after = body[pos + "shape".len()..].chars().next();

    before.is_none_or(|ch| !ch.is_ascii_alphanumeric() && ch != '_')
        && after.is_none_or(|ch| ch.is_whitespace() || ch == ':')
}

fn shape_value_end(body: &str, start: usize) -> usize {
    if start >= body.len() {
        return start;
    }

    let Some(first) = body[start..].chars().next() else {
        return start;
    };

    match first {
        '"' | '\'' => {
            let quote = first;
            let mut pos = start + 1;
            while pos < body.len() {
                let Some(ch) = body[pos..].chars().next() else {
                    break;
                };
                if ch == '\\' {
                    pos += ch.len_utf8();
                    if pos < body.len() {
                        let Some(escaped) = body[pos..].chars().next() else {
                            break;
                        };
                        pos += escaped.len_utf8();
                    }
                    continue;
                }
                if ch == quote {
                    pos += ch.len_utf8();
                    break;
                }
                pos += ch.len_utf8();
            }
            pos
        }
        _ => {
            let mut pos = start;
            while pos < body.len() {
                let Some(ch) = body[pos..].chars().next() else {
                    break;
                };
                match ch {
                    ',' | '}' | '\n' | '\r' | ' ' | '\t' => break,
                    _ => pos += ch.len_utf8(),
                }
            }
            pos
        }
    }
}

fn push_flowchart_subgraph_symbol(facts: &mut EditorSemanticFacts, subgraph: &SubgraphBlock) {
    push_flowchart_header_symbol(facts, &subgraph.header);
}

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

fn flowchart_subgraph_symbol_id(header: &SubgraphHeader) -> Option<(String, SourceSpan)> {
    if header.id_equals_title && header.raw_title.chars().any(is_ecmascript_trim_char) {
        // FlowDB replaces this authored title/id with `subGraphN`; exposing the raw title as a
        // renameable id would point editor operations at a symbol that does not exist in the model.
        return None;
    }

    let raw_span = header.raw_id_span?;
    let raw = header.raw_id.as_str();
    let mut start = 0usize;
    let mut end = raw.len();

    if raw.starts_with('"') && raw.ends_with('"') && raw.len() >= 2 {
        start += 1;
        end -= 1;
        let unquoted = &raw[start..end];
        if unquoted.starts_with('`') && unquoted.ends_with('`') && unquoted.len() >= 2 {
            start += 1;
            end -= 1;
        }
    }

    let candidate = &raw[start..end];
    let leading = candidate
        .len()
        .saturating_sub(candidate.trim_start_matches(is_ecmascript_trim_char).len());
    start += leading;
    let trimmed = &raw[start..end];
    end = start + trimmed.trim_end_matches(is_ecmascript_trim_char).len();

    let name = raw[start..end].to_string();
    if name.is_empty() {
        return None;
    }
    Some((
        name,
        SourceSpan::new(raw_span.start + start, raw_span.start + end),
    ))
}

fn push_flowchart_token_symbol(
    facts: &mut EditorSemanticFacts,
    id: &str,
    start: usize,
    end: usize,
) {
    if id.is_empty() {
        return;
    }
    let span = crate::SourceSpan::new(start, end);
    facts.push_symbol(
        EditorSemanticSymbol::new(
            id.to_string(),
            Some("flowchart node".to_string()),
            EditorSemanticKind::Module,
            span,
            span,
        )
        .with_rename_policy(EditorRenamePolicy::FlowchartNodeId),
    );
}

impl FlowchartSemanticSource {
    fn into_render_model(self, meta: &ParseMetadata) -> Result<FlowchartModel> {
        self.into_render_model_parts(meta).map(|(model, _)| model)
    }

    fn into_render_model_parts(
        self,
        meta: &ParseMetadata,
    ) -> Result<(FlowchartModel, FlowchartRenderLabelSources)> {
        let control = ParseControl::new();
        self.into_render_model_parts_controlled(meta, &control)
            .expect("a private parse control cannot be cancelled")
    }

    fn into_render_model_controlled(
        self,
        meta: &ParseMetadata,
        control: &ParseControl,
    ) -> ParseControlResult<Result<FlowchartModel>> {
        Ok(self
            .into_render_model_parts_controlled(meta, control)?
            .map(|(model, _)| model))
    }

    fn into_render_model_parts_controlled(
        self,
        meta: &ParseMetadata,
        control: &ParseControl,
    ) -> ParseControlResult<Result<(FlowchartModel, FlowchartRenderLabelSources)>> {
        control.checkpoint()?;
        let FlowchartSemanticSource {
            acc_descr,
            acc_title,
            class_defs,
            edge_defaults,
            vertex_calls,
            mut nodes,
            edges,
            subgraphs,
            warning_facts,
            tooltips,
            keyword,
            direction,
        } = self;

        if meta.diagram_type == "flowchart-elk" {
            append_missing_subgraph_nodes(&mut nodes, &subgraphs, control)?;
        }

        let mut render_nodes = Vec::with_capacity(nodes.len());
        let mut render_label_sources = FlowchartRenderLabelSources::default();
        for (index, node) in nodes.into_iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            let (node, render_label_source) = flow_node_to_model(node, &meta.effective_config);
            if let Some(source) = render_label_source {
                render_label_sources.insert_node(node.id.clone(), source);
            }
            render_nodes.push(node);
        }
        let mut render_edges = Vec::with_capacity(edges.len());
        for (index, edge) in edges.into_iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            match flow_edge_to_model(edge, meta) {
                Ok((edge, render_label_source)) => {
                    if let Some(source) = render_label_source {
                        render_label_sources.insert_edge(edge.id.clone(), source);
                    }
                    render_edges.push(edge);
                }
                Err(error) => return Ok(Err(error)),
            }
        }
        let mut render_subgraphs = Vec::with_capacity(subgraphs.len());
        for (index, subgraph) in subgraphs.into_iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            let (subgraph, render_title_source) =
                flow_subgraph_to_model(subgraph, &meta.effective_config);
            render_label_sources.set_subgraph(subgraph.id.clone(), render_title_source);
            render_subgraphs.push(subgraph);
        }
        let mut render_tooltips = rustc_hash::FxHashMap::default();
        render_tooltips.reserve(tooltips.len());
        for (index, (id, tooltip)) in tooltips.into_iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            render_tooltips.insert(id, tooltip);
        }

        control.checkpoint()?;
        let model = FlowchartModel {
            keyword,
            acc_descr,
            acc_title,
            class_defs,
            direction: direction.or_else(|| Some("TB".to_string())),
            edge_defaults: Some(FlowEdgeDefaults {
                style: edge_defaults.style,
                interpolate: edge_defaults.interpolate,
            }),
            vertex_calls,
            nodes: render_nodes,
            edges: render_edges,
            subgraphs: render_subgraphs,
            tooltips: render_tooltips,
            warning_facts,
        };
        Ok(Ok((model, render_label_sources)))
    }
}

fn append_missing_subgraph_nodes(
    nodes: &mut Vec<Node>,
    subgraphs: &[FlowSubGraph],
    control: &ParseControl,
) -> ParseControlResult<()> {
    let mut existing_ids = HashSet::with_capacity(nodes.len());
    for (index, node) in nodes.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        existing_ids.insert(node.id.clone());
    }
    for (index, subgraph) in subgraphs.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        if existing_ids.insert(subgraph.id.clone()) {
            nodes.push(Node {
                id: subgraph.id.clone(),
                id_span: None,
                label: None,
                label_type: TitleKind::Text,
                label_span: None,
                label_selection: None,
                shape: None,
                shape_data: None,
                icon: None,
                form: None,
                pos: None,
                img: None,
                constraint: None,
                asset_width: None,
                asset_height: None,
                styles: Vec::new(),
                classes: Vec::new(),
                link: None,
                link_target: None,
                have_callback: false,
            });
        }
    }
    control.checkpoint()
}

fn flow_node_to_model(n: Node, config: &MermaidConfig) -> (FlowNode, Option<String>) {
    let layout_shape = layout_shape_for_node(&n);
    let label = sanitized_node_label(&n, config);
    let label_raw = n.label.as_deref().unwrap_or(&n.id);
    let render_label_source = render_label_source_needs_provenance(label_raw)
        .then(|| sanitized_node_render_label_source(&n, config))
        .filter(|source| *source != label);

    let node = FlowNode {
        id: n.id,
        label: Some(label),
        label_type: Some(title_kind_str(&n.label_type).to_string()),
        layout_shape: Some(layout_shape),
        shape: n.shape,
        icon: n.icon,
        form: n.form,
        pos: n.pos,
        img: n.img,
        constraint: n.constraint,
        asset_width: n.asset_width,
        asset_height: n.asset_height,
        classes: n.classes,
        styles: n.styles,
        link: n.link,
        link_target: n.link_target,
        have_callback: n.have_callback,
    };
    (node, render_label_source)
}

fn flow_edge_to_model(e: Edge, meta: &ParseMetadata) -> Result<(FlowEdge, Option<String>)> {
    let label = sanitized_optional_label(e.label.as_deref(), &meta.effective_config);
    let render_label_source = e
        .label
        .as_deref()
        .filter(|raw| render_label_source_needs_provenance(raw))
        .map(|raw| sanitized_render_label_source(raw, &meta.effective_config));
    let render_label_source = match (&label, render_label_source) {
        (Some(label), Some(source)) if *label != source => Some(source),
        _ => None,
    };
    let id = e.id.ok_or_else(|| {
        Error::diagram_parse_fallback(
            meta.diagram_type.clone(),
            "flowchart edge id missing".to_string(),
        )
    })?;

    Ok((
        FlowEdge {
            id,
            from: e.from,
            to: e.to,
            label,
            label_type: Some(title_kind_str(&e.label_type).to_string()),
            edge_type: Some(e.link.edge_type),
            arrow: e.link.end,
            is_user_defined_id: e.is_user_defined_id,
            stroke: Some(e.link.stroke),
            length: e.link.length,
            style: e.style,
            classes: e.classes,
            interpolate: e.interpolate,
            animate: e.animate,
            animation: e.animation,
        },
        render_label_source,
    ))
}

fn layout_shape_for_node(n: &Node) -> String {
    // Mirrors Mermaid FlowDB `getTypeFromVertex` logic at 11.12.2.
    if n.img.is_some() {
        return "imageSquare".to_string();
    }
    if n.icon.is_some() {
        match n.form.as_deref() {
            Some("circle") => return "iconCircle".to_string(),
            Some("square") => return "iconSquare".to_string(),
            Some("rounded") => return "iconRounded".to_string(),
            _ => return "icon".to_string(),
        }
    }
    match n.shape.as_deref() {
        Some("square") | None => "squareRect".to_string(),
        Some("round") => "roundedRect".to_string(),
        Some("ellipse") => "ellipse".to_string(),
        Some(other) => other.to_string(),
    }
}

fn sanitized_node_label(n: &Node, config: &MermaidConfig) -> String {
    let label_raw = n.label.as_ref().unwrap_or(&n.id);
    let mut label = sanitized_label(label_raw, config);
    if label.len() >= 2 && label.starts_with('\"') && label.ends_with('\"') {
        label = label[1..label.len() - 1].to_string();
    }
    label
}

fn sanitized_node_render_label_source(n: &Node, config: &MermaidConfig) -> String {
    let label_raw = n.label.as_ref().unwrap_or(&n.id);
    let mut label = sanitized_render_label_source(label_raw, config);
    if label.len() >= 2 && label.starts_with('"') && label.ends_with('"') {
        label = label[1..label.len() - 1].to_string();
    }
    label
}

fn sanitized_optional_label(label: Option<&str>, config: &MermaidConfig) -> Option<String> {
    label.map(|s| sanitized_label(s, config))
}

fn sanitized_label(raw: &str, config: &MermaidConfig) -> String {
    let decoded = decode_mermaid_hash_entities(raw);
    sanitize_text(&decoded, config)
}

fn sanitized_render_label_source(raw: &str, config: &MermaidConfig) -> String {
    let flow_db_label = sanitize_text(raw, config);
    let decoded = crate::entities::restore_mermaid_entity_spelling(&flow_db_label);
    crate::sanitize::sanitize_text_as_html_fragment(decoded.as_ref(), config)
}

fn render_label_source_needs_provenance(raw: &str) -> bool {
    raw.contains(['&', '#', 'ﬂ', '¶', '<', '>'])
}

fn decode_mermaid_hash_entities(input: &str) -> std::borrow::Cow<'_, str> {
    // Mermaid runs `encodeEntities(...)` before parsing and later decodes with browser
    // `entityDecode(...)`. In our headless pipeline we decode into Unicode during parsing so
    // layout + SVG output match upstream.
    crate::entities::decode_mermaid_entities_to_unicode(input)
}

fn flow_subgraph_to_model(
    sg: FlowSubGraph,
    config: &MermaidConfig,
) -> (FlowSubgraph, Option<String>) {
    let sanitized_title = sanitize_text(&sg.title, config);
    let title = decode_mermaid_hash_entities(&sanitized_title).into_owned();
    let render_title_source = render_label_source_needs_provenance(&sg.title)
        .then(|| sanitized_render_label_source(&sg.title, config))
        .filter(|source| *source != title);
    let subgraph = FlowSubgraph {
        id: sg.id,
        nodes: sg.nodes,
        title,
        classes: sg.classes,
        styles: sg.styles,
        dir: sg.dir,
        has_explicit_dir: sg.has_explicit_dir,
        label_type: Some(sg.label_type),
    };
    (subgraph, render_title_source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
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
        let meta = ParseMetadata {
            diagram_type: "flowchart-v2".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: MermaidConfig::empty_object(),
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
            let edge = model
                .edges
                .iter()
                .find(|edge| edge.from == from)
                .unwrap_or_else(|| panic!("missing edge from {from}"));
            render_label_sources.edge_label_for_render(edge)
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
            subgraph_render_label_sources.subgraph_title_for_render(subgraph),
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
            entity_subgraph_render_label_sources.subgraph_title_for_render(entity_subgraph),
            "A &amp; &lt; &gt;"
        );

        let duplicate_subgraph_source =
            "flowchart LR\nsubgraph X[\"&nbsp;First\"]\n  A\nend\nsubgraph X[Second]\n  B\nend\n";
        let (duplicate_model, duplicate_sources) =
            parse_flowchart_model_with_render_context(duplicate_subgraph_source, &meta)
                .expect("duplicate subgraph model");
        assert_eq!(duplicate_model.subgraphs.len(), 2);
        assert_eq!(
            duplicate_sources.subgraph_title_for_render(&duplicate_model.subgraphs[1]),
            "Second"
        );

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

    #[test]
    fn flowchart_render_label_context_stays_out_of_the_public_model_contract() {
        let parsed = crate::Engine::new()
            .parse_diagram_for_render_model_sync(
                "flowchart LR\nA[\"&nbsp;A&nbsp;\"] -->|\"&nbsp;E&nbsp;\"| B\n",
                crate::ParseOptions::strict(),
            )
            .expect("parse flowchart")
            .expect("detect flowchart");
        assert!(parsed.retained_render_context_bytes() > 0);

        let (_, crate::RenderSemanticModel::Flowchart(model)) = parsed.into_parts() else {
            panic!("expected Flowchart model");
        };
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

    #[test]
    fn flowchart_render_label_context_applies_shape_sanitization_to_angle_text() {
        let parsed = crate::Engine::new()
            .parse_diagram_for_render_model_sync(
                include_str!(
                    "../../../../fixtures/flowchart/stress_flowchart_svglike_escaped_tags_025.mmd"
                ),
                crate::ParseOptions::strict(),
            )
            .expect("parse flowchart")
            .expect("detect flowchart");
        let render_label_sources = parsed
            .flowchart_render_label_sources()
            .expect("flowchart render label sources");
        let crate::RenderSemanticModel::Flowchart(model) = parsed.model() else {
            panic!("expected Flowchart model");
        };

        let comparison = model
            .nodes
            .iter()
            .find(|node| node.id == "C")
            .expect("comparison node");
        assert_eq!(comparison.label.as_deref(), Some("x &lt; y and y > z"));
        assert_eq!(
            render_label_sources.node_label_for_render(comparison),
            Some("x &lt; y and y &gt; z")
        );

        let formatted = model
            .nodes
            .iter()
            .find(|node| node.id == "D")
            .expect("formatted node");
        assert_eq!(
            render_label_sources.node_label_for_render(formatted),
            Some("<u>under</u> and <i>italic</i>")
        );
    }

    #[test]
    fn flowchart_parser_families_expose_typed_editor_semantics() {
        let semantics = crate::family::diagram_type_editor_semantics("flowchart-v2")
            .expect("flowchart is an admitted family");

        assert_eq!(semantics.outline_kind(), EditorSemanticKind::Module);
        let swimlane = crate::family::diagram_type_editor_semantics("swimlane")
            .expect("swimlane is an admitted family");
        assert_eq!(swimlane.outline_kind(), EditorSemanticKind::Variable);
    }

    #[test]
    fn combined_flowchart_parse_propagates_preexisting_cancellation() {
        let meta = ParseMetadata {
            diagram_type: "flowchart-v2".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: MermaidConfig::empty_object(),
            title: None,
        };
        let control = ParseControl::new();
        control.cancel();

        assert!(matches!(
            parse_flowchart_json_and_editor_facts("flowchart TD\nA-->B\n", &meta, &control),
            Err(crate::ParseCancelled)
        ));
    }

    #[test]
    fn token_trace_stops_after_cancellation_during_parser_work() {
        let mut source = String::from("flowchart TD\n");
        for index in 0..512 {
            source.push_str(&format!("n{index}-->n{}\n", index + 1));
        }
        let control = ParseControl::new();
        control.cancel_after_checkpoints(2);

        assert!(matches!(
            construct_flowchart_token_trace(&source, &[], &control),
            Err(crate::ParseCancelled)
        ));
        assert!(control.is_cancelled());
    }

    #[test]
    fn single_large_chain_observes_cancellation_in_post_parse_projections() {
        let nodes = (0..256)
            .map(|index| format!("n{index}"))
            .collect::<Vec<_>>()
            .join(" & ");
        let source = format!("flowchart TD\n{nodes}\n");
        let meta = ParseMetadata {
            diagram_type: "flowchart-v2".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: MermaidConfig::empty_object(),
            title: None,
        };
        let ast = parse_flowchart_ast(&source, &meta).expect("large node group should parse");
        assert!(matches!(
            ast.statements.as_slice(),
            [Stmt::Chain { nodes, .. }] if nodes.len() == 256
        ));

        let shape_data_control = ParseControl::new();
        shape_data_control.cancel_after_checkpoints(2);
        assert!(matches!(
            prepare_flowchart_shape_data(&ast.statements, &shape_data_control),
            Err(crate::ParseCancelled)
        ));

        let editor_facts_control = ParseControl::new();
        editor_facts_control.cancel_after_checkpoints(2);
        assert!(matches!(
            editor_facts_from_flowchart_ast(&ast, &editor_facts_control),
            Err(crate::ParseCancelled)
        ));

        let semantic_source =
            parse_flowchart_semantic_source(&source, &meta).expect("large node group should build");
        let render_control = ParseControl::new();
        render_control.cancel_after_checkpoints(2);
        assert!(matches!(
            semantic_source.into_render_model_controlled(&meta, &render_control),
            Err(crate::ParseCancelled)
        ));
    }

    #[test]
    fn single_large_subgraph_chain_observes_cancellation_in_membership_projection() {
        let nodes = (0..256)
            .map(|index| format!("n{index}"))
            .collect::<Vec<_>>()
            .join(" & ");
        let source = format!("flowchart TD\nsubgraph group\n{nodes}\nend\n");
        let meta = ParseMetadata {
            diagram_type: "flowchart-v2".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: MermaidConfig::empty_object(),
            title: None,
        };
        let ast =
            parse_flowchart_ast(&source, &meta).expect("large subgraph node group should parse");
        let mut builder = SubgraphBuilder::new(false, ast.direction.clone());
        let control = ParseControl::new();
        control.cancel_after_checkpoints(2);

        assert!(matches!(
            builder.visit_statements(&ast.statements, &control),
            Err(crate::ParseCancelled)
        ));
    }

    #[test]
    fn flowchart_subgraphs_exist_matches_mermaid_flowdb_spec() {
        let subgraphs = vec![
            FlowSubGraph {
                id: "sg0".to_string(),
                nodes: vec![
                    "a".to_string(),
                    "b".to_string(),
                    "c".to_string(),
                    "e".to_string(),
                ],
                title: "".to_string(),
                classes: Vec::new(),
                styles: Vec::new(),
                dir: None,
                has_explicit_dir: false,
                label_type: "text".to_string(),
            },
            FlowSubGraph {
                id: "sg1".to_string(),
                nodes: vec!["f".to_string(), "g".to_string(), "h".to_string()],
                title: "".to_string(),
                classes: Vec::new(),
                styles: Vec::new(),
                dir: None,
                has_explicit_dir: false,
                label_type: "text".to_string(),
            },
            FlowSubGraph {
                id: "sg2".to_string(),
                nodes: vec!["i".to_string(), "j".to_string()],
                title: "".to_string(),
                classes: Vec::new(),
                styles: Vec::new(),
                dir: None,
                has_explicit_dir: false,
                label_type: "text".to_string(),
            },
            FlowSubGraph {
                id: "sg3".to_string(),
                nodes: vec!["k".to_string()],
                title: "".to_string(),
                classes: Vec::new(),
                styles: Vec::new(),
                dir: None,
                has_explicit_dir: false,
                label_type: "text".to_string(),
            },
        ];

        assert!(super::subgraph::subgraphs_exist(&subgraphs, "a"));
        assert!(super::subgraph::subgraphs_exist(&subgraphs, "h"));
        assert!(super::subgraph::subgraphs_exist(&subgraphs, "j"));
        assert!(super::subgraph::subgraphs_exist(&subgraphs, "k"));

        assert!(!super::subgraph::subgraphs_exist(&subgraphs, "a2"));
        assert!(!super::subgraph::subgraphs_exist(&subgraphs, "l"));
    }
}
