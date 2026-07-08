use crate::diagram::legacy_warning_messages;
use crate::sanitize::sanitize_text;
use crate::{
    DiagramWarningFact, EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts,
    EditorSemanticKind, EditorSemanticRole, EditorSemanticSymbol, Error,
    FLOWCHART_EXPLICIT_DIRECTION_WARNING_RULE_ID, MermaidConfig, ParseMetadata, Result, SourceSpan,
    editor::{format_lalrpop_parse_error, lalrpop_parse_diagnostic, lalrpop_recovery_span},
};
use indexmap::IndexMap;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

lalrpop_util::lalrpop_mod!(
    #[allow(
        clippy::empty_line_after_outer_attr,
        clippy::type_complexity,
        clippy::result_large_err
    )]
    flowchart_grammar,
    "/diagrams/flowchart_grammar.rs"
);

mod accessibility;
mod ast;
mod build;
mod lex;
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
    parse_edge_label_text, parse_label_text, strip_wrapping_backticks, title_kind_str, unquote,
};

pub use model::{FlowEdge, FlowEdgeDefaults, FlowNode, FlowSubgraph, FlowchartV2Model};

pub(crate) use model::{
    Edge, EdgeDefaults, LabeledText, LinkToken, Node, SubgraphHeader, TitleKind,
};

pub(crate) use ast::{
    ClassAssignStmt, ClassDefStmt, ClickAction, ClickStmt, FlowchartAst, LinkStylePos,
    LinkStyleStmt, Stmt, StyleStmt, SubgraphBlock,
};

pub(crate) use tokens::{LexError, NodeLabelToken, Tok};

use accessibility::extract_flowchart_accessibility_statements;
use build::FlowchartBuildState;
use lexer::Lexer;
use link::{destruct_end_link, destruct_start_link};
use semantic::{FlowchartSemanticContext, apply_semantic_statements};
use shape_data::{
    apply_shape_data_to_node, parse_shape_data, public_shape_names_11_12_2, value_to_bool,
    value_to_string,
};
use subgraph::SubgraphBuilder;

#[derive(Debug, Clone)]
pub(crate) struct FlowSubGraph {
    pub id: String,
    pub nodes: Vec<String>,
    pub title: String,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
    pub dir: Option<String>,
    pub label_type: String,
}

struct FlowchartSemanticSource {
    keyword: String,
    direction: Option<String>,
    effective_direction: Option<String>,
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

pub fn parse_flowchart(code: &str, meta: &ParseMetadata) -> Result<Value> {
    Ok(parse_flowchart_semantic_source(code, meta)?
        .into_compat_json(&meta.diagram_type, &meta.effective_config))
}

pub(crate) fn parse_flowchart_json_and_editor_facts(
    code: &str,
    meta: &ParseMetadata,
) -> Result<(Value, EditorSemanticFacts)> {
    let original_code = code;
    let (code, acc_title, acc_descr) = flowchart_parser_input_and_accessibility(code);
    let ast = parse_flowchart_ast(&code, meta)?;
    let mut facts = editor_facts_from_flowchart_ast(&ast);
    collect_accessibility_directive_prefixes(original_code, &mut facts);
    collect_expected_syntax_from_tokens(&code, &mut facts);
    let model = parse_flowchart_semantic_source_from_ast(ast, acc_title, acc_descr, meta)?
        .into_compat_json(&meta.diagram_type, &meta.effective_config);
    Ok((model, facts))
}

pub fn parse_flowchart_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<FlowchartV2Model> {
    parse_flowchart_semantic_source(code, meta)?.into_render_model(meta)
}

pub fn flowchart_public_shape_names() -> impl Iterator<Item = &'static str> {
    public_shape_names_11_12_2()
}

pub fn parse_flowchart_editor_facts(
    code: &str,
    _meta: &ParseMetadata,
) -> Result<EditorSemanticFacts> {
    let original_code = code;
    let code = mask_flowchart_editor_parse_input(code);
    match flowchart_grammar::FlowchartAstParser::new().parse(Lexer::new(&code)) {
        Ok(ast) => {
            let mut facts = editor_facts_from_flowchart_ast(&ast);
            collect_accessibility_directive_prefixes(original_code, &mut facts);
            collect_expected_syntax_from_tokens(&code, &mut facts);
            Ok(facts)
        }
        Err(error) => {
            let span = lalrpop_recovery_span(&error, code.len());
            let mut facts = recover_flowchart_editor_facts_from_tokens(&code);
            collect_accessibility_directive_prefixes(original_code, &mut facts);
            facts.mark_recovered_from_parse_error(
                format!(
                    "flowchart parser recovered after parse error: {}",
                    format_lalrpop_parse_error(&error)
                ),
                Some(span),
            );
            Ok(facts)
        }
    }
}

fn parse_flowchart_semantic_source(
    code: &str,
    meta: &ParseMetadata,
) -> Result<FlowchartSemanticSource> {
    let (code, acc_title, acc_descr) = flowchart_parser_input_and_accessibility(code);
    let ast = parse_flowchart_ast(&code, meta)?;
    parse_flowchart_semantic_source_from_ast(ast, acc_title, acc_descr, meta)
}

fn parse_flowchart_semantic_source_from_ast(
    ast: FlowchartAst,
    acc_title: Option<String>,
    acc_descr: Option<String>,
    meta: &ParseMetadata,
) -> Result<FlowchartSemanticSource> {
    let inherit_dir = meta
        .effective_config
        .as_value()
        .get("flowchart")
        .and_then(|v| v.get("inheritDir"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let mut builder = SubgraphBuilder::new(inherit_dir, ast.direction.clone());
    builder.visit_statements(&ast.statements);

    let subgraph_ids: HashSet<String> = builder
        .subgraphs
        .iter()
        .map(|subgraph| subgraph.id.clone())
        .collect();

    let mut build = FlowchartBuildState::new(subgraph_ids);
    build
        .add_statements(&ast.statements)
        .map_err(|e| Error::diagram_parse_fallback(meta.diagram_type.clone(), e))?;
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
        node_index.insert(n.id.clone(), idx);
    }
    let mut subgraph_index: HashMap<String, usize> = HashMap::new();
    for (idx, sg) in builder.subgraphs.iter().enumerate() {
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
        };
        apply_semantic_statements(&ast.statements, &mut semantic_ctx)?;
    }

    let direction = ast.direction;
    let mut warning_facts = build_warning_facts;
    warning_facts.extend(flowchart_warning_facts(&direction, ast.header_span));
    let effective_direction = direction.clone().or_else(|| Some("TB".to_string()));

    Ok(FlowchartSemanticSource {
        keyword: ast.keyword,
        direction,
        effective_direction,
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
    })
}

fn flowchart_parser_input_and_accessibility(
    code: &str,
) -> (String, Option<String>, Option<String>) {
    let (_, acc_title, acc_descr) = extract_flowchart_accessibility_statements(code);
    let mut bytes = code.as_bytes().to_vec();
    mask_accessibility_statements(code, &mut bytes);
    let code = String::from_utf8(bytes)
        .unwrap_or_else(|err| String::from_utf8_lossy(err.as_bytes()).into());
    (code, acc_title, acc_descr)
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

fn mask_flowchart_editor_parse_input(code: &str) -> String {
    let mut bytes = code.as_bytes().to_vec();

    if let Some((start, end)) = frontmatter_range(code) {
        mask_range_preserving_newlines(&mut bytes, start, end);
    }
    mask_directives(code, &mut bytes);
    mask_mermaid_comment_lines(code, &mut bytes);
    mask_accessibility_statements(code, &mut bytes);

    String::from_utf8(bytes).unwrap_or_else(|err| String::from_utf8_lossy(err.as_bytes()).into())
}

fn frontmatter_range(code: &str) -> Option<(usize, usize)> {
    let after_marker = code.strip_prefix("---")?;
    let open_line_end = after_marker.find('\n')?;
    if !after_marker[..open_line_end].trim().is_empty() {
        return None;
    }

    let body_start = 3 + open_line_end + 1;
    let body = &code[body_start..];
    let mut offset = 0usize;
    for line in body.split_inclusive('\n') {
        if line.trim_end_matches(['\r', '\n']).trim() == "---" {
            return Some((0, body_start + offset + line.len()));
        }
        offset += line.len();
    }

    None
}

fn mask_directives(code: &str, bytes: &mut [u8]) {
    let mut pos = 0usize;
    while let Some(rel) = code[pos..].find("%%{") {
        let start = pos + rel;
        let after_start = start + 3;
        let end = code[after_start..]
            .find("}%%")
            .map_or(code.len(), |rel_end| after_start + rel_end + 3);
        mask_range_preserving_newlines(bytes, start, end);
        pos = end;
    }
}

fn mask_mermaid_comment_lines(code: &str, bytes: &mut [u8]) {
    let mut start = 0usize;
    for line in code.split_inclusive('\n') {
        let end = start + line.len();
        let trimmed = line.trim_start();
        if let Some(after_marker) = trimmed.strip_prefix("%%") {
            let has_comment_body = after_marker.chars().next().is_some_and(|ch| ch != '\n');
            if !after_marker.starts_with('{') && has_comment_body {
                mask_range_preserving_newlines(bytes, start, end);
            }
        }
        start = end;
    }
}

fn mask_accessibility_statements(code: &str, bytes: &mut [u8]) {
    let mut start = 0usize;
    while start < code.len() {
        let end = next_line_end(code, start);
        let line = &code[start..end];
        let trimmed = line.trim_start();

        if is_accessibility_title_line(trimmed) {
            mask_range_preserving_newlines(bytes, start, end);
            start = end;
            continue;
        }

        if let Some(is_block) = accessibility_description_line_kind(trimmed) {
            let mut block_end = end;
            if is_block && !trimmed.contains('}') {
                while block_end < code.len() {
                    let next_end = next_line_end(code, block_end);
                    let next_line = &code[block_end..next_end];
                    block_end = next_end;
                    if next_line.contains('}') {
                        break;
                    }
                }
            }
            mask_range_preserving_newlines(bytes, start, block_end);
            start = block_end;
            continue;
        }

        start = end;
    }
}

fn is_accessibility_title_line(trimmed: &str) -> bool {
    trimmed
        .strip_prefix("accTitle")
        .is_some_and(|rest| rest.trim_start().starts_with(':'))
}

fn accessibility_description_line_kind(trimmed: &str) -> Option<bool> {
    for prefix in ["accDescription", "accDescr"] {
        let Some(rest) = trimmed.strip_prefix(prefix) else {
            continue;
        };
        let rest = rest.trim_start();
        if rest.starts_with(':') {
            return Some(false);
        }
        if rest.starts_with('{') {
            return Some(true);
        }
    }
    None
}

fn next_line_end(code: &str, start: usize) -> usize {
    code[start..]
        .find('\n')
        .map_or(code.len(), |relative| start + relative + 1)
}

fn mask_range_preserving_newlines(bytes: &mut [u8], start: usize, end: usize) {
    for byte in &mut bytes[start..end] {
        if *byte != b'\n' && *byte != b'\r' {
            *byte = b' ';
        }
    }
}

fn editor_facts_from_flowchart_ast(ast: &FlowchartAst) -> EditorSemanticFacts {
    let mut facts = EditorSemanticFacts::new();
    collect_editor_facts_from_statements(&ast.statements, &mut facts);
    facts
}

fn recover_flowchart_editor_facts_from_tokens(code: &str) -> EditorSemanticFacts {
    let mut facts = EditorSemanticFacts::new();
    facts.mark_recovered();
    let mut collector = FlowchartRecoveryFactCollector::default();
    let mut lexer = Lexer::recovering(code);
    let mut last_position = lexer.position();

    while let Some(result) = lexer.next() {
        let current_position = lexer.position();
        if let Ok((start, token, end)) = result {
            collector.accept(code, token, start, end, &mut facts);
        } else if current_position == last_position {
            break;
        }
        last_position = current_position;
    }
    collector.finish(code.len(), &mut facts);

    facts
}

fn collect_expected_syntax_from_tokens(code: &str, facts: &mut EditorSemanticFacts) {
    let mut lexer = Lexer::new(code);
    let mut last_position = lexer.position();
    while let Some(result) = lexer.next() {
        let current_position = lexer.position();
        let Ok((start, token, end)) = result else {
            if current_position == last_position {
                break;
            }
            last_position = current_position;
            continue;
        };

        match token {
            Tok::NodeLabel(label) => {
                if let Some(trigger_span) = label.trigger_span {
                    push_flowchart_shape_trigger_expected_syntax(trigger_span, facts);
                }
            }
            Tok::ShapeData(_) => {
                push_flowchart_shape_value_expected_syntax(code, start, end, facts)
            }
            Tok::DirectionStmt(dir) => {
                push_flowchart_direction_value_expected_syntax(code, start, end, &dir, facts)
            }
            _ => {}
        }
        last_position = current_position;
    }
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
        token: Tok,
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

        let token_kind = match &token {
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
    token: Tok,
    start: usize,
    end: usize,
    facts: &mut EditorSemanticFacts,
) {
    match token {
        Tok::Id(id) => push_flowchart_token_symbol(facts, id, start, end),
        Tok::SubgraphHeader(header) => {
            push_flowchart_header_symbol(facts, &header);
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
            &label,
            Some(SourceSpan::new(start, end)),
            "flowchart edge label",
        ),
        Tok::StyleStmt(stmt) => push_flowchart_style_stmt_facts(facts, &stmt),
        Tok::ClassDefStmt(stmt) => push_flowchart_classdef_stmt_facts(facts, &stmt),
        Tok::ClassAssignStmt(stmt) => push_flowchart_class_assign_stmt_facts(facts, &stmt),
        Tok::ClickStmt(_) => facts.push_directive_prefix("click"),
        Tok::LinkStyleStmt(_) => facts.push_directive_prefix("linkStyle"),
        Tok::KwGraph
        | Tok::KwFlowchart
        | Tok::KwFlowchartElk
        | Tok::KwSubgraph
        | Tok::KwEnd
        | Tok::Sep
        | Tok::Amp
        | Tok::StyleSep
        | Tok::Direction(_) => {}
        Tok::DirectionStmt(dir) => {
            push_flowchart_direction_value_expected_syntax(code, start, end, &dir, facts)
        }
        Tok::Arrow(_) | Tok::EdgeId(_) => {}
        Tok::ShapeData(_) => push_flowchart_shape_value_expected_syntax(code, start, end, facts),
    }
}

fn collect_editor_facts_from_statements(statements: &[Stmt], facts: &mut EditorSemanticFacts) {
    let mut emitted_edge_label_spans = HashSet::new();
    let mut seen_edge_ids = HashSet::new();
    collect_editor_facts_from_statements_with_seen_edges(
        statements,
        facts,
        &mut emitted_edge_label_spans,
        &mut seen_edge_ids,
    );
}

fn collect_editor_facts_from_statements_with_seen_edges(
    statements: &[Stmt],
    facts: &mut EditorSemanticFacts,
    emitted_edge_label_spans: &mut HashSet<(usize, usize)>,
    seen_edge_ids: &mut HashSet<String>,
) {
    for stmt in statements {
        match stmt {
            Stmt::Chain { nodes, edges } => {
                for node in nodes {
                    push_flowchart_node_symbol(facts, node);
                }
                for edge in edges {
                    push_flowchart_edge_label_symbol(facts, edge, emitted_edge_label_spans);
                    if let Some(id) = edge.id.as_deref() {
                        seen_edge_ids.insert(id.to_string());
                    }
                }
            }
            Stmt::Node(node) => push_flowchart_node_symbol(facts, node),
            Stmt::Subgraph(subgraph) => {
                push_flowchart_subgraph_symbol(facts, subgraph);
                collect_editor_facts_from_statements_with_seen_edges(
                    &subgraph.statements,
                    facts,
                    emitted_edge_label_spans,
                    seen_edge_ids,
                );
            }
            Stmt::Style(stmt) => push_flowchart_style_stmt_facts(facts, stmt),
            Stmt::ClassDef(stmt) => push_flowchart_classdef_stmt_facts(facts, stmt),
            Stmt::ClassAssign(stmt) => push_flowchart_class_assign_stmt_facts(facts, stmt),
            Stmt::Click(_) => facts.push_directive_prefix("click"),
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
}

fn push_flowchart_node_symbol(facts: &mut EditorSemanticFacts, node: &Node) {
    if let Some(span) = node.id_span {
        facts.push_symbol(EditorSemanticSymbol::new(
            node.id.clone(),
            Some("flowchart node".to_string()),
            EditorSemanticKind::Module,
            span,
            span,
        ));
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
    for (id, span) in stmt.ids.iter().zip(stmt.id_spans.iter().copied()) {
        push_flowchart_span_symbol(
            facts,
            id,
            "flowchart class definition",
            EditorSemanticKind::Property,
            Some(span),
            EditorSemanticRole::Outline,
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
    facts.push_symbol(EditorSemanticSymbol::with_role(
        name.to_string(),
        Some(detail.to_string()),
        kind,
        role,
        span,
        span,
    ));
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
    code: &str,
    start: usize,
    end: usize,
    dir: &str,
    facts: &mut EditorSemanticFacts,
) {
    let Some(span) = direction_value_expected_span(code, start, end, dir) else {
        return;
    };

    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::DirectionValue,
        span,
    ));
}

fn push_flowchart_shape_trigger_expected_syntax(span: SourceSpan, facts: &mut EditorSemanticFacts) {
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::ShapeTrigger,
        span,
    ));
}

fn shape_value_expected_span(code: &str, start: usize, end: usize) -> Option<SourceSpan> {
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

fn collect_accessibility_directive_prefixes(code: &str, facts: &mut EditorSemanticFacts) {
    for line in code.lines() {
        let trimmed = line.trim_start();
        if is_accessibility_title_line(trimmed) {
            facts.push_directive_prefix("accTitle");
            continue;
        }
        if let Some(prefix) = accessibility_description_prefix(trimmed) {
            facts.push_directive_prefix(prefix);
        }
    }
}

fn accessibility_description_prefix(trimmed: &str) -> Option<&'static str> {
    for prefix in ["accDescription", "accDescr"] {
        let Some(rest) = trimmed.strip_prefix(prefix) else {
            continue;
        };
        let rest = rest.trim_start();
        if rest.starts_with(':') || rest.starts_with('{') {
            return Some(prefix);
        }
    }
    None
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

fn direction_value_expected_span(
    code: &str,
    start: usize,
    end: usize,
    dir: &str,
) -> Option<SourceSpan> {
    let raw = code.get(start..end)?;
    let trimmed = raw.trim_start();
    let leading = raw.len().saturating_sub(trimmed.len());
    let after_keyword = trimmed.strip_prefix("direction")?;
    let keyword_ws = after_keyword
        .chars()
        .take_while(|ch| ch.is_whitespace())
        .map(|ch| ch.len_utf8())
        .sum::<usize>();
    let value_start = start + leading + "direction".len() + keyword_ws;
    let value_end = value_start + dir.len();

    Some(SourceSpan::new(value_start, value_end))
}

fn push_flowchart_subgraph_symbol(facts: &mut EditorSemanticFacts, subgraph: &SubgraphBlock) {
    push_flowchart_header_symbol(facts, &subgraph.header);
}

fn push_flowchart_header_symbol(facts: &mut EditorSemanticFacts, header: &SubgraphHeader) {
    if let Some(span) = header.header_span.or(header.raw_id_span) {
        let name = header.raw_id.trim();
        if name.is_empty() {
            return;
        }
        let selection = header.raw_id_span.unwrap_or(span);
        facts.push_symbol(EditorSemanticSymbol::new(
            name.to_string(),
            Some("subgraph".to_string()),
            EditorSemanticKind::Namespace,
            span,
            selection,
        ));
    }
}

fn push_flowchart_token_symbol(
    facts: &mut EditorSemanticFacts,
    id: String,
    start: usize,
    end: usize,
) {
    if id.is_empty() {
        return;
    }
    let span = crate::SourceSpan::new(start, end);
    facts.push_symbol(EditorSemanticSymbol::new(
        id,
        Some("flowchart node".to_string()),
        EditorSemanticKind::Module,
        span,
        span,
    ));
}

impl FlowchartSemanticSource {
    fn into_compat_json(self, diagram_type: &str, config: &MermaidConfig) -> Value {
        let FlowchartSemanticSource {
            keyword,
            direction,
            acc_title,
            acc_descr,
            class_defs,
            tooltips,
            edge_defaults,
            vertex_calls,
            mut nodes,
            edges,
            subgraphs,
            warning_facts,
            ..
        } = self;

        if diagram_type == "flowchart-elk" {
            append_missing_subgraph_nodes(&mut nodes, &subgraphs);
        }

        let mut model = json!({
            "type": diagram_type,
            "keyword": keyword,
            "direction": direction,
            "accTitle": acc_title,
            "accDescr": acc_descr,
            "classDefs": class_defs,
            "tooltips": tooltips,
            "edgeDefaults": {
                "style": edge_defaults.style,
                "interpolate": edge_defaults.interpolate,
            },
            "vertexCalls": vertex_calls,
            "nodes": nodes
                .into_iter()
                .map(|node| flow_node_to_json(node, config))
                .collect::<Vec<_>>(),
            "edges": edges
                .into_iter()
                .map(|edge| flow_edge_to_json(edge, config))
                .collect::<Vec<_>>(),
            "subgraphs": subgraphs
                .into_iter()
                .map(flow_subgraph_to_json)
                .collect::<Vec<_>>(),
        });

        if !warning_facts.is_empty() {
            let warnings = legacy_warning_messages(&warning_facts);
            model["warningFacts"] = json!(warning_facts);
            model["warnings"] = json!(warnings);
        }

        model
    }

    fn into_render_model(self, meta: &ParseMetadata) -> Result<FlowchartV2Model> {
        let FlowchartSemanticSource {
            acc_descr,
            acc_title,
            class_defs,
            direction: _,
            edge_defaults,
            vertex_calls,
            mut nodes,
            edges,
            subgraphs,
            warning_facts,
            effective_direction,
            tooltips,
            keyword: _,
        } = self;

        if meta.diagram_type == "flowchart-elk" {
            append_missing_subgraph_nodes(&mut nodes, &subgraphs);
        }

        Ok(FlowchartV2Model {
            acc_descr,
            acc_title,
            class_defs,
            direction: effective_direction,
            edge_defaults: Some(FlowEdgeDefaults {
                style: edge_defaults.style,
                interpolate: edge_defaults.interpolate,
            }),
            vertex_calls,
            nodes: nodes
                .into_iter()
                .map(|node| flow_node_to_model(node, &meta.effective_config))
                .collect::<Vec<_>>(),
            edges: edges
                .into_iter()
                .map(|edge| flow_edge_to_model(edge, meta))
                .collect::<Result<Vec<_>>>()?,
            subgraphs: subgraphs
                .into_iter()
                .map(flow_subgraph_to_model)
                .collect::<Vec<_>>(),
            tooltips: tooltips.into_iter().collect(),
            warning_facts,
        })
    }
}

fn append_missing_subgraph_nodes(nodes: &mut Vec<Node>, subgraphs: &[FlowSubGraph]) {
    let mut existing_ids: HashSet<String> = nodes.iter().map(|node| node.id.clone()).collect();
    for subgraph in subgraphs {
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
}

fn flow_node_to_json(n: Node, config: &MermaidConfig) -> Value {
    let layout_shape = layout_shape_for_node(&n);
    let label = sanitized_node_label(&n, config);

    json!({
        "id": n.id,
        "label": label,
        "labelType": title_kind_str(&n.label_type),
        "shape": n.shape,
        "layoutShape": layout_shape,
        "icon": n.icon,
        "form": n.form,
        "pos": n.pos,
        "img": n.img,
        "constraint": n.constraint,
        "assetWidth": n.asset_width,
        "assetHeight": n.asset_height,
        "styles": n.styles,
        "classes": n.classes,
        "link": n.link,
        "linkTarget": n.link_target,
        "haveCallback": n.have_callback,
    })
}

fn flow_node_to_model(n: Node, config: &MermaidConfig) -> FlowNode {
    let layout_shape = layout_shape_for_node(&n);
    let label = sanitized_node_label(&n, config);

    FlowNode {
        id: n.id,
        label: Some(label),
        label_type: Some(title_kind_str(&n.label_type).to_string()),
        layout_shape: Some(layout_shape),
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
    }
}

fn flow_edge_to_json(e: Edge, config: &MermaidConfig) -> Value {
    let label = sanitized_optional_label(e.label.as_deref(), config);

    json!({
        "from": e.from,
        "to": e.to,
        "id": e.id,
        "isUserDefinedId": e.is_user_defined_id,
        "arrow": e.link.end,
        "type": e.link.edge_type,
        "stroke": e.link.stroke,
        "length": e.link.length,
        "label": label,
        "labelType": title_kind_str(&e.label_type),
        "style": e.style,
        "classes": e.classes,
        "interpolate": e.interpolate,
        "animate": e.animate,
        "animation": e.animation,
    })
}

fn flow_edge_to_model(e: Edge, meta: &ParseMetadata) -> Result<FlowEdge> {
    let label = sanitized_optional_label(e.label.as_deref(), &meta.effective_config);
    let id = e.id.ok_or_else(|| {
        Error::diagram_parse_fallback(
            meta.diagram_type.clone(),
            "flowchart edge id missing".to_string(),
        )
    })?;

    Ok(FlowEdge {
        id,
        from: e.from,
        to: e.to,
        label,
        label_type: Some(title_kind_str(&e.label_type).to_string()),
        edge_type: Some(e.link.edge_type),
        stroke: Some(e.link.stroke),
        length: e.link.length,
        style: e.style,
        classes: e.classes,
        interpolate: e.interpolate,
        animate: e.animate,
        animation: e.animation,
    })
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

fn sanitized_optional_label(label: Option<&str>, config: &MermaidConfig) -> Option<String> {
    label.map(|s| sanitized_label(s, config))
}

fn sanitized_label(raw: &str, config: &MermaidConfig) -> String {
    let decoded = decode_mermaid_hash_entities(raw);
    sanitize_text(&decoded, config)
}

fn decode_mermaid_hash_entities(input: &str) -> std::borrow::Cow<'_, str> {
    // Mermaid runs `encodeEntities(...)` before parsing and later decodes with browser
    // `entityDecode(...)`. In our headless pipeline we decode into Unicode during parsing so
    // layout + SVG output match upstream.
    crate::entities::decode_mermaid_entities_to_unicode(input)
}

fn flow_subgraph_to_json(sg: FlowSubGraph) -> Value {
    let title = crate::entities::decode_mermaid_entities_to_unicode(&sg.title).into_owned();
    json!({
        "id": sg.id,
        "nodes": sg.nodes,
        "title": title,
        "classes": sg.classes,
        "styles": sg.styles,
        "dir": sg.dir,
        "labelType": sg.label_type,
    })
}

fn flow_subgraph_to_model(sg: FlowSubGraph) -> FlowSubgraph {
    FlowSubgraph {
        id: sg.id,
        nodes: sg.nodes,
        title: crate::entities::decode_mermaid_entities_to_unicode(&sg.title).into_owned(),
        classes: sg.classes,
        styles: sg.styles,
        dir: sg.dir,
        label_type: Some(sg.label_type),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
                label_type: "text".to_string(),
            },
            FlowSubGraph {
                id: "sg1".to_string(),
                nodes: vec!["f".to_string(), "g".to_string(), "h".to_string()],
                title: "".to_string(),
                classes: Vec::new(),
                styles: Vec::new(),
                dir: None,
                label_type: "text".to_string(),
            },
            FlowSubGraph {
                id: "sg2".to_string(),
                nodes: vec!["i".to_string(), "j".to_string()],
                title: "".to_string(),
                classes: Vec::new(),
                styles: Vec::new(),
                dir: None,
                label_type: "text".to_string(),
            },
            FlowSubGraph {
                id: "sg3".to_string(),
                nodes: vec!["k".to_string()],
                title: "".to_string(),
                classes: Vec::new(),
                styles: Vec::new(),
                dir: None,
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
