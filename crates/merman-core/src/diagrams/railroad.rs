#[cfg(feature = "presentation")]
use crate::Result;
#[cfg(feature = "presentation")]
use crate::SourcePresentationMetadata;
#[cfg(any(test, feature = "editor-facts"))]
use crate::SourceSpan;
#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
use crate::sanitize::sanitize_text;
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorRenamePolicy, EditorSemanticFacts,
    EditorSemanticKind, EditorSemanticSymbol,
};
use crate::{Error, OperationControl, OperationControlResult, SemanticMetadata};
#[cfg(feature = "editor-facts")]
use merman_core_structured::railroad::RailroadParserTrace;
use merman_core_structured::railroad::{
    self as railroad_parser, RailroadDialect, RailroadParseDiagnostic,
};
pub(crate) use merman_model::families::railroad::{RailroadAstNode, RailroadDiagramModel};
#[cfg(all(test, feature = "compat-json", feature = "editor-facts"))]
pub(crate) use merman_model::families::railroad::{RailroadRepeatBound, RailroadRepeatBoundError};
#[cfg(feature = "compat-json")]
use serde_json::Value;
#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    static RAILROAD_SYNTAX_CONSTRUCTION_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_railroad_syntax_construction_count() {
    RAILROAD_SYNTAX_CONSTRUCTION_COUNT.set(0);
}

#[cfg(test)]
pub(crate) fn railroad_syntax_construction_count() -> usize {
    RAILROAD_SYNTAX_CONSTRUCTION_COUNT.get()
}

fn railroad_diagram_type(dialect: RailroadDialect) -> &'static str {
    match dialect {
        RailroadDialect::Ir => "railroad",
        RailroadDialect::Ebnf => "railroadEbnf",
        RailroadDialect::Abnf => "railroadAbnf",
        RailroadDialect::Peg => "railroadPeg",
    }
}

#[cfg(feature = "editor-facts")]
fn common_detail_prefix(dialect: RailroadDialect) -> &'static str {
    match dialect {
        RailroadDialect::Ir => "railroad",
        RailroadDialect::Ebnf => "railroad ebnf",
        RailroadDialect::Abnf => "railroad abnf",
        RailroadDialect::Peg => "railroad peg",
    }
}

#[cfg(feature = "editor-facts")]
fn editor_rename_policy(dialect: RailroadDialect) -> EditorRenamePolicy {
    match dialect {
        RailroadDialect::Ir => EditorRenamePolicy::RailroadIrRule,
        RailroadDialect::Ebnf => EditorRenamePolicy::RailroadEbnfRule,
        RailroadDialect::Abnf => EditorRenamePolicy::RailroadAbnfRule,
        RailroadDialect::Peg => EditorRenamePolicy::RailroadPegRule,
    }
}

#[cfg(feature = "editor-facts")]
pub(crate) fn is_valid_editor_ir_rule_identifier(candidate: &str) -> bool {
    railroad_parser::is_valid_rule_identifier(candidate, RailroadDialect::Ir)
}

#[cfg(feature = "editor-facts")]
pub(crate) fn is_valid_editor_ebnf_rule_identifier(candidate: &str) -> bool {
    railroad_parser::is_valid_rule_identifier(candidate, RailroadDialect::Ebnf)
}

#[cfg(feature = "editor-facts")]
pub(crate) fn is_valid_editor_peg_rule_identifier(candidate: &str) -> bool {
    railroad_parser::is_valid_rule_identifier(candidate, RailroadDialect::Peg)
}

#[cfg(feature = "editor-facts")]
pub(crate) fn is_valid_editor_abnf_rule_identifier(candidate: &str) -> bool {
    railroad_parser::is_valid_rule_identifier(candidate, RailroadDialect::Abnf)
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_railroad(code: &str, meta: &SourcePresentationMetadata) -> Result<Value> {
    parse_railroad_for_dialect(code, meta, RailroadDialect::Ir)
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_railroad_ebnf(code: &str, meta: &SourcePresentationMetadata) -> Result<Value> {
    parse_railroad_for_dialect(code, meta, RailroadDialect::Ebnf)
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_railroad_abnf(code: &str, meta: &SourcePresentationMetadata) -> Result<Value> {
    parse_railroad_for_dialect(code, meta, RailroadDialect::Abnf)
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_railroad_peg(code: &str, meta: &SourcePresentationMetadata) -> Result<Value> {
    parse_railroad_for_dialect(code, meta, RailroadDialect::Peg)
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_railroad_model_for_render_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<RailroadDiagramModel>> {
    parse_railroad_model_for_render_dialect_controlled(code, meta, RailroadDialect::Ir, control)
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_railroad_ebnf_model_for_render_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<RailroadDiagramModel>> {
    parse_railroad_model_for_render_dialect_controlled(code, meta, RailroadDialect::Ebnf, control)
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_railroad_abnf_model_for_render_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<RailroadDiagramModel>> {
    parse_railroad_model_for_render_dialect_controlled(code, meta, RailroadDialect::Abnf, control)
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_railroad_peg_model_for_render_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<RailroadDiagramModel>> {
    parse_railroad_model_for_render_dialect_controlled(code, meta, RailroadDialect::Peg, control)
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub(crate) fn parse_railroad_json_and_editor_facts(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<crate::family::CombinedSemanticParse> {
    parse_railroad_json_and_editor_facts_for_dialect(code, meta, RailroadDialect::Ir, control)
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub(crate) fn parse_railroad_ebnf_json_and_editor_facts(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<crate::family::CombinedSemanticParse> {
    parse_railroad_json_and_editor_facts_for_dialect(code, meta, RailroadDialect::Ebnf, control)
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub(crate) fn parse_railroad_abnf_json_and_editor_facts(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<crate::family::CombinedSemanticParse> {
    parse_railroad_json_and_editor_facts_for_dialect(code, meta, RailroadDialect::Abnf, control)
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub(crate) fn parse_railroad_peg_json_and_editor_facts(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<crate::family::CombinedSemanticParse> {
    parse_railroad_json_and_editor_facts_for_dialect(code, meta, RailroadDialect::Peg, control)
}

pub(crate) fn parse_railroad_semantic_model_controlled(
    code: &str,
    metadata: &SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<crate::family::SemanticConstruction<RailroadDiagramModel>> {
    parse_railroad_semantic_model_for_dialect_controlled(
        code,
        metadata,
        RailroadDialect::Ir,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )
}

pub(crate) fn parse_railroad_ebnf_semantic_model_controlled(
    code: &str,
    metadata: &SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<crate::family::SemanticConstruction<RailroadDiagramModel>> {
    parse_railroad_semantic_model_for_dialect_controlled(
        code,
        metadata,
        RailroadDialect::Ebnf,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )
}

pub(crate) fn parse_railroad_abnf_semantic_model_controlled(
    code: &str,
    metadata: &SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<crate::family::SemanticConstruction<RailroadDiagramModel>> {
    parse_railroad_semantic_model_for_dialect_controlled(
        code,
        metadata,
        RailroadDialect::Abnf,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )
}

pub(crate) fn parse_railroad_peg_semantic_model_controlled(
    code: &str,
    metadata: &SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<crate::family::SemanticConstruction<RailroadDiagramModel>> {
    parse_railroad_semantic_model_for_dialect_controlled(
        code,
        metadata,
        RailroadDialect::Peg,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )
}

fn parse_railroad_semantic_model_for_dialect_controlled(
    code: &str,
    metadata: &SemanticMetadata,
    dialect: RailroadDialect,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<crate::family::SemanticConstruction<RailroadDiagramModel>> {
    let construction = construct_railroad_semantic_source(
        code,
        &metadata.diagram_type,
        dialect,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?;
    Ok(match construction {
        Ok(source) => {
            #[cfg(feature = "editor-facts")]
            let mut source = source;
            #[cfg(feature = "editor-facts")]
            let editor_facts = source.editor_facts.take();
            crate::family::SemanticConstruction {
                model: Ok(source.into_model(&metadata.semantic_config)),
                #[cfg(feature = "editor-facts")]
                editor_facts,
            }
        }
        Err(failure) => crate::family::SemanticConstruction {
            model: Err(*failure.error),
            #[cfg(feature = "editor-facts")]
            editor_facts: failure.editor_facts.map(|facts| *facts),
        },
    })
}

struct RailroadSemanticSource {
    model: RailroadDiagramModel,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<EditorSemanticFacts>,
}

impl RailroadSemanticSource {
    fn into_model(mut self, config: &crate::MermaidConfig) -> RailroadDiagramModel {
        crate::common_db::sanitize_optional_title(&mut self.model.title, config);
        crate::common_db::sanitize_optional_acc_title(&mut self.model.acc_title, config);
        crate::common_db::sanitize_optional_acc_descr(&mut self.model.acc_descr, config);
        for rule in &mut self.model.rules {
            rule.name = sanitize_text(&rule.name, config);
            sanitize_ast_node(&mut rule.definition, config);
        }
        self.model
    }

    #[cfg(feature = "compat-json")]
    fn into_compat_json(self, meta: &SourcePresentationMetadata) -> Result<Value> {
        let model = self.into_model(meta.effective_config.as_config());
        render_model_to_compat_json(&model, meta)
    }

    #[cfg(feature = "presentation")]
    fn into_render_model(self, meta: &SourcePresentationMetadata) -> RailroadDiagramModel {
        self.into_model(meta.effective_config.as_config())
    }
}

struct RailroadParseFailure {
    error: Box<Error>,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<Box<EditorSemanticFacts>>,
}

#[cfg(feature = "compat-json")]
fn parse_railroad_for_dialect(
    code: &str,
    meta: &SourcePresentationMetadata,
    dialect: RailroadDialect,
) -> Result<Value> {
    parse_railroad_semantic_source(code, meta, dialect)?.into_compat_json(meta)
}

#[cfg(feature = "presentation")]
fn parse_railroad_model_for_render_dialect_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    dialect: RailroadDialect,
    control: &OperationControl,
) -> OperationControlResult<Result<RailroadDiagramModel>> {
    let construction = construct_railroad_semantic_source(
        code,
        &meta.diagram_type,
        dialect,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )?;
    match construction {
        Ok(source) => Ok(Ok(source.into_render_model(meta))),
        Err(failure) => Ok(Err(*failure.error)),
    }
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
fn parse_railroad_json_and_editor_facts_for_dialect(
    code: &str,
    meta: &SourcePresentationMetadata,
    dialect: RailroadDialect,
    control: &OperationControl,
) -> OperationControlResult<crate::family::CombinedSemanticParse> {
    let construction = construct_railroad_semantic_source(
        code,
        &meta.diagram_type,
        dialect,
        EditorFactsDemand::Capture,
        control,
    )?;
    let parsed = crate::family::CombinedSemanticParse::from_construction(
        construction,
        |mut source| {
            let editor_facts = source.editor_facts.take().unwrap_or_default();
            (source.into_compat_json(meta), editor_facts)
        },
        |failure| {
            (
                *failure.error,
                failure.editor_facts.map(|facts| *facts).unwrap_or_default(),
            )
        },
    );
    control.checkpoint()?;
    Ok(parsed)
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &RailroadDiagramModel,
    meta: &SourcePresentationMetadata,
) -> Result<Value> {
    merman_model::families::railroad::compat::project(model, &meta.compat_json_metadata())
        .map_err(crate::Error::from)
}

#[cfg(feature = "compat-json")]
fn parse_railroad_semantic_source(
    code: &str,
    meta: &SourcePresentationMetadata,
    dialect: RailroadDialect,
) -> Result<RailroadSemanticSource> {
    construct_railroad_semantic_source(
        code,
        &meta.diagram_type,
        dialect,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        &OperationControl::new(),
    )
    .expect("a private parse control cannot be cancelled")
    .map_err(|failure| *failure.error)
}

fn construct_railroad_semantic_source(
    code: &str,
    diagram_type: &str,
    dialect: RailroadDialect,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<RailroadSemanticSource, RailroadParseFailure>> {
    #[cfg(test)]
    RAILROAD_SYNTAX_CONSTRUCTION_COUNT.set(RAILROAD_SYNTAX_CONSTRUCTION_COUNT.get() + 1);

    #[cfg(feature = "editor-facts")]
    let capture_trace = editor_demand.captures_rich();
    #[cfg(not(feature = "editor-facts"))]
    let capture_trace = false;
    let parsed = railroad_parser::parse_recovering(code, dialect, capture_trace, control)?;
    #[cfg(feature = "editor-facts")]
    let mut editor_facts = match editor_demand {
        EditorFactsDemand::Omit => None,
        EditorFactsDemand::Recovery => Some(EditorSemanticFacts::new()),
        EditorFactsDemand::Capture => parsed
            .trace
            .as_ref()
            .map(|trace| editor_facts_from_model(&parsed.model, dialect, trace)),
    };
    control.checkpoint()?;

    if let Some(diagnostic) = parsed.diagnostic {
        let error = match diagnostic {
            RailroadParseDiagnostic::Exact { span, message } => {
                Error::diagram_parse_exact(diagram_type, message, span)
            }
            RailroadParseDiagnostic::InsertionPoint { offset, message } => {
                Error::diagram_parse_insertion_point(diagram_type, message, offset)
            }
            RailroadParseDiagnostic::EbnfFallback { message } => {
                Error::diagram_parse_fallback(railroad_diagram_type(RailroadDialect::Ebnf), message)
            }
        };
        #[cfg(feature = "editor-facts")]
        let span = railroad_error_span(&error, SourceSpan::new(0, code.len()));
        #[cfg(feature = "editor-facts")]
        if let Some(facts) = &mut editor_facts {
            facts.mark_recovered_from_parse_error(
                format!(
                    "{} parser recovered after parse error: {error}",
                    railroad_diagram_type(dialect)
                ),
                Some(span),
            );
        }
        return Ok(Err(RailroadParseFailure {
            error: Box::new(error),
            #[cfg(feature = "editor-facts")]
            editor_facts: editor_facts.map(Box::new),
        }));
    }

    control.checkpoint()?;
    Ok(Ok(RailroadSemanticSource {
        model: parsed.model,
        #[cfg(feature = "editor-facts")]
        editor_facts,
    }))
}

#[cfg(any(test, feature = "editor-facts"))]
fn railroad_error_span(error: &Error, fallback: SourceSpan) -> SourceSpan {
    match error {
        Error::DiagramParse { diagnostic, .. } => diagnostic.span().unwrap_or(fallback),
        _ => fallback,
    }
}

fn sanitize_ast_node(node: &mut RailroadAstNode, config: &crate::MermaidConfig) {
    match node {
        RailroadAstNode::Terminal { value, .. } => *value = sanitize_text(value, config),
        RailroadAstNode::NonTerminal { name, .. } => *name = sanitize_text(name, config),
        RailroadAstNode::Sequence { elements, .. } => {
            for element in elements {
                sanitize_ast_node(element, config);
            }
        }
        RailroadAstNode::Choice { alternatives, .. } => {
            for alternative in alternatives {
                sanitize_ast_node(alternative, config);
            }
        }
        RailroadAstNode::Optional { element, .. } => sanitize_ast_node(element, config),
        RailroadAstNode::Repetition {
            element, separator, ..
        } => {
            sanitize_ast_node(element, config);
            if let Some(separator) = separator {
                sanitize_ast_node(separator, config);
            }
        }
        RailroadAstNode::Special { text, .. } => *text = sanitize_text(text, config),
    }
}

#[cfg(feature = "editor-facts")]
fn editor_facts_from_model(
    model: &RailroadDiagramModel,
    dialect: RailroadDialect,
    trace: &RailroadParserTrace,
) -> EditorSemanticFacts {
    #[cfg(test)]
    crate::editor::test_work::record_collection();
    let mut facts = EditorSemanticFacts::new();
    let detail_prefix = common_detail_prefix(dialect);
    let rename_policy = editor_rename_policy(dialect);

    if let (Some(title), Some(span)) = (&model.title, trace.title_span) {
        facts.push_directive_prefix("title");
        push_payload_fact(
            &mut facts,
            title.clone(),
            span,
            format!("{detail_prefix} title"),
        );
    }
    if let (Some(acc_title), Some(span)) = (&model.acc_title, trace.acc_title_span) {
        facts.push_directive_prefix("accTitle");
        push_payload_fact(
            &mut facts,
            acc_title.clone(),
            span,
            format!("{detail_prefix} accessibility title"),
        );
    }
    if let (Some(acc_descr), Some(span)) = (&model.acc_descr, trace.acc_descr_span) {
        facts.push_directive_prefix("accDescr");
        push_payload_fact(
            &mut facts,
            acc_descr.clone(),
            span,
            format!("{detail_prefix} accessibility description"),
        );
    }

    for (rule, &name_span) in model.rules.iter().zip(&trace.rule_name_spans) {
        facts.push_expected_syntax(EditorExpectedSyntax::new(
            EditorExpectedSyntaxKind::NodeIdentifier,
            name_span,
        ));
        facts.push_symbol(
            EditorSemanticSymbol::new(
                rule.name.clone(),
                Some(format!("{detail_prefix} rule")),
                EditorSemanticKind::Function,
                name_span,
                name_span,
            )
            .with_rename_policy(rename_policy),
        );
        push_ast_facts(
            &mut facts,
            &rule.definition,
            detail_prefix,
            trace,
            rename_policy,
        );
    }

    facts
}

#[cfg(feature = "editor-facts")]
fn push_ast_facts(
    facts: &mut EditorSemanticFacts,
    node: &RailroadAstNode,
    detail_prefix: &str,
    trace: &RailroadParserTrace,
    rename_policy: EditorRenamePolicy,
) {
    match node {
        RailroadAstNode::Terminal {
            value, selection, ..
        } => {
            push_payload_fact(
                facts,
                value.clone(),
                *selection,
                format!("{detail_prefix} terminal"),
            );
        }
        RailroadAstNode::NonTerminal {
            name,
            span,
            selection,
        } => {
            facts.push_expected_syntax(EditorExpectedSyntax::new(
                EditorExpectedSyntaxKind::NodeIdentifier,
                *selection,
            ));
            facts.push_symbol(
                EditorSemanticSymbol::reference(
                    name.clone(),
                    Some(format!("{detail_prefix} nonterminal reference")),
                    EditorSemanticKind::Function,
                    *span,
                    *selection,
                )
                .with_rename_policy(rename_policy),
            );
        }
        RailroadAstNode::Special {
            text,
            span,
            selection,
        } => {
            if let Some(predicate) = trace
                .peg_predicates
                .iter()
                .find(|predicate| predicate.span == *span)
            {
                push_ast_facts(facts, &predicate.inner, detail_prefix, trace, rename_policy);
            } else {
                push_payload_fact(
                    facts,
                    text.clone(),
                    *selection,
                    format!("{detail_prefix} special"),
                );
            }
        }
        RailroadAstNode::Sequence { elements, .. } => {
            for element in elements {
                push_ast_facts(facts, element, detail_prefix, trace, rename_policy);
            }
        }
        RailroadAstNode::Choice { alternatives, .. } => {
            for alternative in alternatives {
                push_ast_facts(facts, alternative, detail_prefix, trace, rename_policy);
            }
        }
        RailroadAstNode::Optional { element, .. } => {
            push_ast_facts(facts, element, detail_prefix, trace, rename_policy)
        }
        RailroadAstNode::Repetition {
            element, separator, ..
        } => {
            push_ast_facts(facts, element, detail_prefix, trace, rename_policy);
            if let Some(separator) = separator {
                push_ast_facts(facts, separator, detail_prefix, trace, rename_policy);
            }
        }
    }
}

#[cfg(feature = "editor-facts")]
fn push_payload_fact(
    facts: &mut EditorSemanticFacts,
    value: String,
    selection: SourceSpan,
    detail: String,
) {
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::Payload,
        selection,
    ));
    facts.push_symbol(EditorSemanticSymbol::payload(
        value,
        Some(detail),
        EditorSemanticKind::String,
        selection,
        selection,
    ));
}

#[cfg(test)]
mod semantic_tests {
    use super::*;

    fn metadata(dialect: RailroadDialect) -> SemanticMetadata {
        SemanticMetadata {
            diagram_type: railroad_diagram_type(dialect).into(),
            config: crate::MermaidConfig::empty_object(),
            semantic_config: crate::MermaidConfig::empty_object(),
            title: None,
        }
    }

    #[test]
    fn canonical_entries_preserve_each_dialect_without_editor_work() {
        type Parser = fn(
            code: &str,
            metadata: &SemanticMetadata,
            #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
            control: &OperationControl,
        ) -> OperationControlResult<
            crate::family::SemanticConstruction<RailroadDiagramModel>,
        >;
        let cases: [(RailroadDialect, &str, Parser); 4] = [
            (
                RailroadDialect::Ir,
                "railroad-beta\ntitle Header\nentry = terminal(\"value\") ;\n",
                parse_railroad_semantic_model_controlled,
            ),
            (
                RailroadDialect::Ebnf,
                "railroad-ebnf-beta\ntitle Header\nentry = \"value\" ;\n",
                parse_railroad_ebnf_semantic_model_controlled,
            ),
            (
                RailroadDialect::Abnf,
                "railroad-abnf-beta\ntitle Header\nentry = \"value\" ;\n",
                parse_railroad_abnf_semantic_model_controlled,
            ),
            (
                RailroadDialect::Peg,
                "railroad-peg-beta\ntitle Header\nentry <- \"value\" ;\n",
                parse_railroad_peg_semantic_model_controlled,
            ),
        ];

        for (dialect, source, parse) in cases {
            reset_railroad_syntax_construction_count();
            #[cfg(feature = "editor-facts")]
            crate::editor::test_work::reset();
            let model = parse(
                source,
                &metadata(dialect),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &OperationControl::new(),
            )
            .unwrap()
            .model
            .unwrap();
            assert_eq!(model.title.as_deref(), Some("Header"));
            assert_eq!(model.rules.len(), 1);
            assert_eq!(model.rules[0].name, "entry");
            assert!(matches!(
                &model.rules[0].definition,
                RailroadAstNode::Terminal { value, .. } if value == "value"
            ));
            assert_eq!(railroad_syntax_construction_count(), 1);
            #[cfg(feature = "editor-facts")]
            assert_eq!(crate::editor::test_work::read(), Default::default());
        }
    }

    #[test]
    fn peg_trace_is_collected_only_for_requested_editor_observations() {
        let source = "railroad-peg-beta\nentry <- &target \"value\" ;\ntarget <- \"ok\" ;\n";
        let control = OperationControl::new();
        let mut models = Vec::new();
        let mut parse = |#[cfg(feature = "editor-facts")] demand: EditorFactsDemand| {
            #[cfg(feature = "editor-facts")]
            let capture_trace = demand.captures_rich();
            #[cfg(not(feature = "editor-facts"))]
            let capture_trace = false;
            let parsed = railroad_parser::parse_recovering(
                source,
                RailroadDialect::Peg,
                capture_trace,
                &control,
            )
            .unwrap();
            assert!(parsed.diagnostic.is_none());
            #[cfg(feature = "editor-facts")]
            assert_eq!(
                parsed
                    .trace
                    .as_ref()
                    .map(|trace| trace.peg_predicates.len()),
                (demand == EditorFactsDemand::Capture).then_some(1),
            );
            models.push(parsed.model);
        };
        #[cfg(feature = "editor-facts")]
        for demand in [
            EditorFactsDemand::Omit,
            EditorFactsDemand::Recovery,
            EditorFactsDemand::Capture,
        ] {
            parse(demand);
        }
        #[cfg(not(feature = "editor-facts"))]
        parse();
        #[cfg(feature = "editor-facts")]
        assert!(models.iter().all(|model| model == &models[0]));
    }

    #[test]
    fn semantic_and_editor_construction_share_exact_errors_and_cancellation() {
        let source = "railroad-ebnf-beta\nbroken = ( \"value\" ;\nafter = \"later\" ;\n";
        let meta = metadata(RailroadDialect::Ebnf);
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let semantic = parse_railroad_ebnf_semantic_model_controlled(
            source,
            &meta,
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap_err();
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        let failure = match construct_railroad_semantic_source(
            source,
            &meta.diagram_type,
            RailroadDialect::Ebnf,
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Capture,
            &OperationControl::new(),
        )
        .unwrap()
        {
            Ok(_) => panic!("expected recovered error"),
            Err(failure) => failure,
        };
        assert_eq!(semantic.to_string(), failure.error.to_string());
        assert_eq!(
            railroad_error_span(&semantic, SourceSpan::new(0, 0)),
            railroad_error_span(&failure.error, SourceSpan::new(0, 0))
        );
        #[cfg(feature = "editor-facts")]
        assert!(
            failure
                .editor_facts
                .unwrap()
                .symbols
                .iter()
                .any(|symbol| symbol.name == "after")
        );

        let control = OperationControl::new();
        control.cancel_after_checkpoints(1);
        assert!(
            parse_railroad_ebnf_semantic_model_controlled(
                source,
                &meta,
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &control,
            )
            .is_err()
        );
    }

    #[test]
    fn semantic_entry_sanitizes_with_semantic_config() {
        let source = concat!(
            "railroad-beta\n",
            "title <script>secret</script><b>Header &amp; &#x3C;</b>\n",
            "accTitle: <script>secret</script>   Accessible\n",
            "accDescr {First\n    <b>Second &amp; &#x3C;</b>}\n",
            "entry = terminal(\"<a href=javascript&colon;alert(1) onclick=alert(1)>value &amp; &#x3C;</a>\") ;\n",
        );
        for security_level in ["strict", "loose"] {
            let mut meta = metadata(RailroadDialect::Ir);
            meta.semantic_config = crate::MermaidConfig::from_value(serde_json::json!({
                "securityLevel": security_level,
                "htmlLabels": true
            }));
            let model = parse_railroad_semantic_model_controlled(
                source,
                &meta,
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &OperationControl::new(),
            )
            .unwrap()
            .model
            .unwrap();
            assert_eq!(model.title.as_deref(), Some("<b>Header &amp; &lt;</b>"));
            assert_eq!(model.acc_title.as_deref(), Some("Accessible"));
            assert_eq!(
                model.acc_descr.as_deref(),
                Some("First\n<b>Second &amp; &lt;</b>")
            );
            assert_eq!(model.rules[0].name, "entry");
            let RailroadAstNode::Terminal {
                value, selection, ..
            } = &model.rules[0].definition
            else {
                panic!("expected terminal");
            };
            assert_eq!(value, "<a>value &amp; &lt;</a>");
            assert_eq!(
                &source[selection.start..selection.end],
                "<a href=javascript&colon;alert(1) onclick=alert(1)>value &amp; &#x3C;</a>"
            );
        }
    }
}

#[cfg(all(test, feature = "compat-json"))]
mod tests {
    use super::*;
    #[cfg(feature = "editor-facts")]
    use crate::{EditorSemanticCompleteness, EditorSemanticRole};
    use crate::{MermaidConfig, SourcePresentationMetadata};
    use merman_model::families::railroad::RailroadRepeatBound;

    fn meta(dialect: RailroadDialect) -> SourcePresentationMetadata {
        SourcePresentationMetadata {
            diagram_type: railroad_diagram_type(dialect).to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        }
    }

    #[cfg(feature = "editor-facts")]
    fn combined_editor_facts(source: &str, dialect: RailroadDialect) -> EditorSemanticFacts {
        let parser: crate::family::CombinedSemanticParser = match dialect {
            RailroadDialect::Ir => parse_railroad_json_and_editor_facts,
            RailroadDialect::Ebnf => parse_railroad_ebnf_json_and_editor_facts,
            RailroadDialect::Abnf => parse_railroad_abnf_json_and_editor_facts,
            RailroadDialect::Peg => parse_railroad_peg_json_and_editor_facts,
        };
        crate::family::test_support::editor_facts(parser, source, &meta(dialect))
    }

    fn header(dialect: RailroadDialect) -> &'static str {
        let (_, variant) = merman_model::family::find_variant(railroad_diagram_type(dialect))
            .expect("railroad dialect is in the model catalog");
        variant.headers[0].label
    }

    fn nested_rule_source(dialect: RailroadDialect, nesting_depth: usize) -> String {
        let (assignment, terminal, groups): (&str, &str, &[(&str, &str)]) = match dialect {
            RailroadDialect::Ir => ("=", "terminal(\"value\")", &[("optional(", ")")]),
            RailroadDialect::Ebnf => ("=", "\"value\"", &[("(", ")"), ("[", "]"), ("{", "}")]),
            RailroadDialect::Abnf => ("=", "\"value\"", &[("(", ")"), ("[", "]")]),
            RailroadDialect::Peg => ("<-", "\"value\"", &[("(", ")")]),
        };
        let group_width = groups
            .iter()
            .map(|(opener, closer)| opener.len() + closer.len())
            .max()
            .unwrap_or_default();
        let mut source = String::with_capacity(
            header(dialect).len() + terminal.len() + group_width * nesting_depth,
        );
        source.push_str(header(dialect));
        source.push_str("\nentry ");
        source.push_str(assignment);
        source.push(' ');
        let mut closers = Vec::with_capacity(nesting_depth);
        for index in 0..nesting_depth {
            let (opener, closer) = groups[index % groups.len()];
            source.push_str(opener);
            closers.push(closer);
        }
        source.push_str(terminal);
        for closer in closers.into_iter().rev() {
            source.push_str(closer);
        }
        source.push_str(" ;\nafter ");
        source.push_str(assignment);
        source.push_str(if dialect == RailroadDialect::Ir {
            " terminal(\"later\") ;\n"
        } else {
            " \"later\" ;\n"
        });
        source
    }

    #[test]
    fn decodes_escaped_strings_for_non_abnf_dialects() {
        let source = concat!("railroad-beta\n", r#"title "a\n\t\"b\"""#, "\n");
        let parsed =
            parse_railroad_semantic_source(source, &meta(RailroadDialect::Ir), RailroadDialect::Ir)
                .unwrap_or_else(|error| panic!("railroad title failed: {error}"));
        assert_eq!(parsed.model.title.as_deref(), Some("a\n\t\"b\""));
    }

    #[test]
    fn yaml_fences_require_a_valid_opening_line() {
        let valid = concat!(
            "railroad-beta\n",
            "---\n",
            "title: valid metadata\n",
            "---\n",
            "entry = terminal(\"value\") ;\n",
        );
        assert!(
            parse_railroad_semantic_source(valid, &meta(RailroadDialect::Ir), RailroadDialect::Ir)
                .is_ok()
        );

        let invalid = concat!(
            "railroad-beta\n",
            "---not-yaml\n",
            "title: should remain source text\n",
            "---\n",
            "entry = terminal(\"value\") ;\n",
        );
        assert!(
            parse_railroad_semantic_source(
                invalid,
                &meta(RailroadDialect::Ir),
                RailroadDialect::Ir,
            )
            .is_err(),
            "an invalid opening YAML delimiter must not be hidden"
        );
    }

    #[test]
    fn non_abnf_strings_reject_physical_line_breaks() {
        let cases = [
            (
                RailroadDialect::Ir,
                "railroad-beta\nentry = terminal(\"left\\\nright\") ;\n",
            ),
            (
                RailroadDialect::Ebnf,
                "railroad-ebnf-beta\nentry = \"left\\\nright\" ;\n",
            ),
            (
                RailroadDialect::Peg,
                "railroad-peg-beta\nentry <- \"left\\\nright\" ;\n",
            ),
        ];
        for (dialect, source) in cases {
            let error = match parse_railroad_semantic_source(source, &meta(dialect), dialect) {
                Ok(_) => {
                    panic!("non-ABNF railroad strings must reject a physical line break");
                }
                Err(error) => error,
            };
            assert!(
                error
                    .to_string()
                    .contains("physical line breaks are not allowed"),
                "{}: {error}",
                railroad_diagram_type(dialect)
            );
        }

        let abnf = "railroad-abnf-beta\nentry = \"left\\\nright\" ;\n";
        assert!(
            parse_railroad_semantic_source(
                abnf,
                &meta(RailroadDialect::Abnf),
                RailroadDialect::Abnf,
            )
            .is_ok(),
            "ABNF string terminals intentionally retain physical line breaks"
        );
    }

    #[test]
    fn abnf_strings_slice_without_escape_decoding() {
        let source = concat!("railroad-abnf-beta\n", r#"entry = "a\n" ;"#, "\n");
        let parsed = parse_railroad_semantic_source(
            source,
            &meta(RailroadDialect::Abnf),
            RailroadDialect::Abnf,
        )
        .unwrap_or_else(|error| panic!("ABNF terminal failed: {error}"));
        let RailroadAstNode::Terminal { value, .. } = &parsed.model.rules[0].definition else {
            panic!("expected ABNF terminal");
        };
        assert_eq!(value, r#"a\n"#);
    }

    #[test]
    fn parses_repeat_bounds() {
        let repeat_bounds = |repeat: &str| {
            let source = format!("railroad-abnf-beta\nentry = {repeat}\"value\" ;\n");
            let parsed = parse_railroad_semantic_source(
                &source,
                &meta(RailroadDialect::Abnf),
                RailroadDialect::Abnf,
            )
            .unwrap_or_else(|error| panic!("ABNF repetition {repeat:?} failed: {error}"));
            let RailroadAstNode::Repetition { min, max, .. } = &parsed.model.rules[0].definition
            else {
                panic!("expected ABNF repetition for {repeat:?}");
            };
            (*min, *max)
        };
        assert_eq!(
            repeat_bounds("*"),
            (RailroadRepeatBound::ZERO, RailroadRepeatBound::INFINITY)
        );
        assert_eq!(
            repeat_bounds("1*"),
            (RailroadRepeatBound::ONE, RailroadRepeatBound::INFINITY)
        );
        assert_eq!(repeat_bounds("*2"), (RailroadRepeatBound::ZERO, 2.into()));
        assert_eq!(repeat_bounds("1*2"), (RailroadRepeatBound::ONE, 2.into()));
        assert_eq!(repeat_bounds("3"), (3.into(), 3.into()));
        assert_eq!(
            repeat_bounds("18446744073709551615"),
            (u64::MAX.into(), u64::MAX.into())
        );
        assert_eq!(
            repeat_bounds("18446744073709551616"),
            (u64::MAX.into(), u64::MAX.into())
        );
        assert_eq!(
            repeat_bounds("00000000000000000002*00000000000000000003"),
            (2.into(), 3.into())
        );
        let huge = "9".repeat(400);
        let (min, max) = repeat_bounds(&huge);
        assert!(min.is_infinite());
        assert!(max.is_infinite());
    }

    #[test]
    fn normalizes_multiline_accessibility_descriptions_like_mermaid() {
        let source = concat!(
            "railroad-beta\r\n",
            "accDescr {\r\n",
            "  first\r\n",
            "\r\n",
            "\r\n",
            "  second\r\n",
            "}\r\n",
            "entry = terminal(\"value\") ;\r\n",
        );

        let parsed =
            parse_railroad_semantic_source(source, &meta(RailroadDialect::Ir), RailroadDialect::Ir)
                .unwrap_or_else(|error| panic!("railroad fixture failed: {error}"));
        assert_eq!(parsed.model.acc_descr.as_deref(), Some("first\nsecond"));
    }

    #[test]
    fn ir_constructor_keywords_cannot_be_rule_names_and_recover() {
        for keyword in [
            "sequence",
            "choice",
            "optional",
            "oneOrMore",
            "zeroOrMore",
            "terminal",
            "nonterminal",
            "special",
        ] {
            let source = format!(
                "railroad-beta\n{keyword} = terminal(\"invalid\") ;\nafter = terminal(\"later\") ;\n"
            );
            let error = match parse_railroad_semantic_source(
                &source,
                &meta(RailroadDialect::Ir),
                RailroadDialect::Ir,
            ) {
                Ok(_) => panic!("{keyword} unexpectedly parsed as a Railroad IR rule name"),
                Err(error) => error,
            };
            assert!(
                error
                    .to_string()
                    .contains("is reserved for railroad expressions"),
                "{error}"
            );

            #[cfg(feature = "editor-facts")]
            {
                let facts = combined_editor_facts(&source, RailroadDialect::Ir);
                assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
                assert!(facts.symbols.iter().any(|symbol| symbol.name == "after"));
            }
        }
    }

    #[test]
    fn rule_names_follow_dialect_keyword_token_precedence() {
        for dialect in [
            RailroadDialect::Ir,
            RailroadDialect::Ebnf,
            RailroadDialect::Abnf,
            RailroadDialect::Peg,
        ] {
            let (assignment, expression) = match dialect {
                RailroadDialect::Ir => ("=", "terminal(\"value\")"),
                RailroadDialect::Ebnf | RailroadDialect::Abnf => ("=", "\"value\""),
                RailroadDialect::Peg => ("<-", "\"value\""),
            };

            let split_title = format!(
                "{}\ntitlecase {assignment} {expression} ;\n",
                header(dialect)
            );
            let parsed = parse_railroad_semantic_source(&split_title, &meta(dialect), dialect)
                .unwrap_or_else(|error| {
                    panic!(
                        "{} title prefix fixture failed: {error}",
                        railroad_diagram_type(dialect)
                    )
                });
            assert_eq!(parsed.model.title.as_deref(), Some(""));
            assert_eq!(parsed.model.rules[0].name, "case");

            let title_metadata =
                format!("{}\ntitle {assignment} {expression} ;\n", header(dialect));
            let parsed = parse_railroad_semantic_source(&title_metadata, &meta(dialect), dialect)
                .unwrap_or_else(|error| {
                    panic!(
                        "{} title metadata fixture failed: {error}",
                        railroad_diagram_type(dialect)
                    )
                });
            assert!(parsed.model.rules.is_empty());

            let header_rule = format!(
                "{}\n{} {assignment} {expression} ;\n",
                header(dialect),
                header(dialect)
            );
            assert!(
                parse_railroad_semantic_source(&header_rule, &meta(dialect), dialect).is_err(),
                "{} accepted its header as a rule name",
                railroad_diagram_type(dialect)
            );

            for valid in ["Title".to_string(), format!("{}x", header(dialect))] {
                let source = format!("{}\n{valid} {assignment} {expression} ;\n", header(dialect));
                let parsed = parse_railroad_semantic_source(&source, &meta(dialect), dialect)
                    .unwrap_or_else(|error| {
                        panic!(
                            "{} rejected {valid:?}: {error}",
                            railroad_diagram_type(dialect)
                        )
                    });
                assert_eq!(parsed.model.rules[0].name, valid);
            }

            #[cfg(feature = "editor-facts")]
            for invalid in ["title", "titlecase", header(dialect)] {
                assert!(
                    !railroad_parser::is_valid_rule_identifier(invalid, dialect),
                    "{} rename validation accepted {invalid:?}",
                    railroad_diagram_type(dialect)
                );
            }
        }
    }

    #[test]
    fn railroad_parsers_bound_nested_expressions_and_recover_later_rules() {
        for dialect in [
            RailroadDialect::Ir,
            RailroadDialect::Ebnf,
            RailroadDialect::Abnf,
            RailroadDialect::Peg,
        ] {
            let max_depth = nested_rule_source(dialect, crate::MAX_DIAGRAM_NESTING_DEPTH);
            let parsed = parse_railroad_semantic_source(&max_depth, &meta(dialect), dialect)
                .unwrap_or_else(|error| {
                    panic!(
                        "{} max-depth fixture failed: {error}",
                        railroad_diagram_type(dialect)
                    )
                });
            assert_eq!(
                parsed.model.rules.len(),
                2,
                "{}",
                railroad_diagram_type(dialect)
            );

            let compatibility_json =
                parse_railroad_for_dialect(&max_depth, &meta(dialect), dialect).unwrap_or_else(
                    |error| {
                        panic!(
                            "{} max-depth compatibility conversion failed: {error}",
                            railroad_diagram_type(dialect)
                        )
                    },
                );
            assert_eq!(
                compatibility_json["rules"].as_array().map(Vec::len),
                Some(2),
                "{}",
                railroad_diagram_type(dialect)
            );

            let excessive_depth = nested_rule_source(dialect, crate::MAX_DIAGRAM_NESTING_DEPTH + 1);
            let error =
                match parse_railroad_semantic_source(&excessive_depth, &meta(dialect), dialect) {
                    Ok(_) => panic!(
                        "{} accepted excessive nesting",
                        railroad_diagram_type(dialect)
                    ),
                    Err(error) => error,
                };
            assert!(
                error.to_string().contains("railroad nesting depth exceeds"),
                "{}: {error}",
                railroad_diagram_type(dialect)
            );

            #[cfg(feature = "editor-facts")]
            {
                let facts = combined_editor_facts(&excessive_depth, dialect);
                assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
                assert!(
                    facts.symbols.iter().any(|symbol| symbol.name == "after"),
                    "{} did not retain the rule after the bounded parse failure",
                    railroad_diagram_type(dialect)
                );
            }
        }
    }

    #[test]
    fn railroad_cancellation_disposes_a_completed_boundary_rule_for_every_dialect() {
        for dialect in [
            RailroadDialect::Ir,
            RailroadDialect::Ebnf,
            RailroadDialect::Abnf,
            RailroadDialect::Peg,
        ] {
            let depth = crate::MAX_DIAGRAM_NESTING_DEPTH;
            let source = nested_rule_source(dialect, depth);
            let (terminal_tokens, group_tokens) = match dialect {
                RailroadDialect::Ir => (4, 3),
                _ => (1, 2),
            };
            // The header, two rules (name, assignment, terminal, semicolon), and entry groups.
            let token_count = 1 + 2 * (3 + terminal_tokens) + depth * group_tokens;
            // The lexer visits every token once, then checks EOF and its final outcome.
            // The next three checkpoints enter construction, enter parsing, and admit entry.
            // Cancellation at the next rule checkpoint owns the completed maximum-depth rule.
            let control = OperationControl::new();
            control.cancel_after_checkpoints(token_count + 5);
            assert!(
                construct_railroad_semantic_source(
                    &source,
                    &meta(dialect).diagram_type,
                    dialect,
                    #[cfg(feature = "editor-facts")]
                    EditorFactsDemand::Omit,
                    &control,
                )
                .is_err()
            );
            let after = parse_railroad_semantic_source(
                &format!(
                    "{}\nafter {} {} ;\n",
                    header(dialect),
                    if dialect == RailroadDialect::Peg {
                        "<-"
                    } else {
                        "="
                    },
                    if dialect == RailroadDialect::Ir {
                        "terminal(\"ok\")"
                    } else {
                        "\"ok\""
                    }
                ),
                &meta(dialect),
                dialect,
            )
            .unwrap();
            assert_eq!(after.model.rules.len(), 1);
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn railroad_editor_facts_use_dialect_specific_rename_policies() {
        let cases = [
            (
                RailroadDialect::Ir,
                "railroad-beta\nentry = terminal(\"value\") ;\n",
                EditorRenamePolicy::RailroadIrRule,
            ),
            (
                RailroadDialect::Ebnf,
                "railroad-ebnf-beta\nentry = \"value\" ;\n",
                EditorRenamePolicy::RailroadEbnfRule,
            ),
            (
                RailroadDialect::Abnf,
                "railroad-abnf-beta\nentry = \"value\" ;\n",
                EditorRenamePolicy::RailroadAbnfRule,
            ),
            (
                RailroadDialect::Peg,
                "railroad-peg-beta\nentry <- \"value\" ;\n",
                EditorRenamePolicy::RailroadPegRule,
            ),
        ];

        for (dialect, source, expected_policy) in cases {
            let facts = combined_editor_facts(source, dialect);
            let entry = facts
                .symbols
                .iter()
                .find(|symbol| symbol.name == "entry")
                .unwrap_or_else(|| {
                    panic!(
                        "{} is missing its entry rule symbol",
                        railroad_diagram_type(dialect)
                    )
                });
            assert_eq!(entry.rename_policy, expected_policy);
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn railroad_rule_definitions_and_forward_references_have_distinct_roles() {
        let cases = [
            (
                RailroadDialect::Ir,
                concat!(
                    "railroad-beta\n",
                    "entry = nonterminal(\"later\") ;\n",
                    "later = terminal(\"value\") ;\n",
                ),
            ),
            (
                RailroadDialect::Ebnf,
                "railroad-ebnf-beta\nentry = later ;\nlater = \"value\" ;\n",
            ),
            (
                RailroadDialect::Abnf,
                "railroad-abnf-beta\nentry = later ;\nlater = \"value\" ;\n",
            ),
            (
                RailroadDialect::Peg,
                "railroad-peg-beta\nentry <- later ;\nlater <- \"value\" ;\n",
            ),
        ];

        for (dialect, source) in cases {
            let facts = combined_editor_facts(source, dialect);
            assert!(facts.symbols.iter().any(|symbol| {
                symbol.name == "entry" && symbol.role == EditorSemanticRole::Entity
            }));
            assert!(facts.symbols.iter().any(|symbol| {
                symbol.name == "value" && symbol.role == EditorSemanticRole::Payload
            }));
            let later: Vec<_> = facts
                .symbols
                .iter()
                .filter(|symbol| symbol.name == "later")
                .collect();

            assert_eq!(later.len(), 2, "{}", railroad_diagram_type(dialect));
            assert_eq!(
                later[0].role,
                EditorSemanticRole::Reference,
                "{}",
                railroad_diagram_type(dialect)
            );
            assert_eq!(
                later[1].role,
                EditorSemanticRole::Entity,
                "{}",
                railroad_diagram_type(dialect)
            );
            assert_eq!(
                later[0].rename_policy,
                later[1].rename_policy,
                "{}",
                railroad_diagram_type(dialect)
            );
        }
    }

    #[test]
    fn typed_render_models_project_exact_compatibility_json_for_every_alias() {
        let cases = [
            ("railroad", "railroad-beta\nentry = terminal(\"value\") ;\n"),
            ("railroadEbnf", "railroad-ebnf-beta\nentry = \"value\" ;\n"),
            ("railroadAbnf", "railroad-abnf-beta\nentry = \"value\" ;\n"),
            ("railroadPeg", "railroad-peg-beta\nentry <- \"value\" ;\n"),
        ];
        let engine = crate::MermaidEngine::new();

        for (diagram_type, source) in cases {
            let compat = engine
                .parse_diagram_sync(source, crate::ParseOptions::strict())
                .unwrap()
                .unwrap();
            let typed = engine
                .parse_diagram_for_render_model_sync(source, crate::ParseOptions::strict())
                .unwrap()
                .unwrap();
            let crate::RenderSemanticModel::Railroad(model) = typed.model() else {
                panic!("expected Railroad render model for {diagram_type}");
            };

            assert_eq!(typed.metadata().diagram_type, diagram_type);
            assert_eq!(
                merman_model::families::railroad::compat::project(
                    model,
                    &merman_model::presentation::compat::CompatJsonMetadata::from(typed.metadata())
                )
                .unwrap(),
                compat.model
            );
        }
    }

    #[test]
    fn recovery_keeps_later_rules_for_parser_and_lexer_errors() {
        let cases = [
            (
                RailroadDialect::Ebnf,
                concat!(
                    "railroad-ebnf-beta\r\n",
                    "before = \"ok\" ;\r\n",
                    "broken = ( \"x\" ;\r\n",
                    "after = \"later\" ;\r\n",
                ),
            ),
            (
                RailroadDialect::Peg,
                concat!(
                    "railroad-peg-beta\r\n",
                    "before <- \"ok\" ;\r\n",
                    "@\r\n",
                    "after <- \"later\" ;\r\n",
                ),
            ),
        ];

        for (dialect, source) in cases {
            assert!(parse_railroad_semantic_source(source, &meta(dialect), dialect).is_err());

            #[cfg(feature = "editor-facts")]
            {
                reset_railroad_syntax_construction_count();
                let facts = combined_editor_facts(source, dialect);
                assert_eq!(railroad_syntax_construction_count(), 1);
                assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
                assert!(facts.symbols.iter().any(|symbol| symbol.name == "after"));
                assert!(facts.symbols.iter().any(|symbol| symbol.name == "later"));
                assert_eq!(facts.diagnostics.len(), 1);
            }
        }
    }
}
