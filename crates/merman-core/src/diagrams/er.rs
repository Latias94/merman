#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
use crate::family::SemanticConstruction;
use crate::parse_diagnostic::{unexpected_eof_message, unexpected_token_message};
#[cfg(feature = "editor-facts")]
use crate::source_span::{
    has_ascii_separator, line_content_end, source_value_span, trailing_ascii_whitespace_slot,
};
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol,
};
use crate::{
    Error, OperationControl, OperationControlResult, ParseDiagnostic, ParseDiagnosticSpanKind,
    SourceSpan,
};
#[cfg(feature = "presentation")]
use crate::{Result, SourcePresentationMetadata};
#[cfg(feature = "editor-facts")]
use merman_core_graph::er::{ErIdList, ErToken};
use merman_core_graph::er::{ErSyntax, ErSyntaxDiagnostic};
use merman_model::families::er::ErDiagramModel;
#[cfg(all(test, feature = "compat-json"))]
use merman_model::families::er::ErEntity;
#[cfg(feature = "editor-facts")]
use merman_model::text::is_ecmascript_whitespace;
#[cfg(feature = "compat-json")]
use serde_json::Value;
#[cfg(all(test, feature = "compat-json"))]
use serde_json::json;
#[cfg(test)]
use std::cell::Cell;
#[cfg(feature = "editor-facts")]
use std::collections::HashSet;

#[cfg(test)]
thread_local! {
    static ER_SYNTAX_CONSTRUCTION_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_er_syntax_construction_count() {
    ER_SYNTAX_CONSTRUCTION_COUNT.set(0);
}

#[cfg(test)]
pub(crate) fn er_syntax_construction_count() -> usize {
    ER_SYNTAX_CONSTRUCTION_COUNT.get()
}

pub(crate) fn sanitize_common_db_fields(model: &mut ErDiagramModel, config: &crate::MermaidConfig) {
    crate::common_db::sanitize_optional_acc_title(&mut model.acc_title, config);
    crate::common_db::sanitize_optional_acc_descr(&mut model.acc_descr, config);
    for subgraph in &mut model.subgraphs {
        subgraph.title = crate::sanitize::sanitize_text(&subgraph.title, config);
    }
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &ErDiagramModel,
    meta: &SourcePresentationMetadata,
) -> Result<Value> {
    merman_model::families::er::compat::project(model, &meta.compat_json_metadata())
        .map_err(crate::Error::from)
}

#[cfg(feature = "editor-facts")]
fn collect_editor_facts(
    syntax: &ErSyntax,
    code: &str,
    editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<Option<EditorSemanticFacts>> {
    if editor_demand == EditorFactsDemand::Capture {
        let mut facts = EditorSemanticFacts::new();
        #[cfg(test)]
        crate::editor::test_work::record_collection();
        let mut collector = ErEditorFactCollector::default();
        for (index, event) in syntax.events().iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            match event {
                Ok((start, token, end)) => collector.accept(code, token, *start, *end, &mut facts),
                Err(_) => facts.mark_recovered(),
            }
        }
        collector.finish(code, &mut facts);
        Ok(Some(facts))
    } else {
        Ok(editor_demand
            .needs_recovery()
            .then(EditorSemanticFacts::new))
    }
}

fn er_parse_diagnostic(diagnostic: ErSyntaxDiagnostic) -> ParseDiagnostic {
    use ParseDiagnosticSpanKind::{Exact, InsertionPoint};
    match diagnostic {
        ErSyntaxDiagnostic::Lexical(error) => {
            ParseDiagnostic::new(error.message).with_span(error.span, Exact)
        }
        ErSyntaxDiagnostic::UnexpectedToken {
            token,
            span,
            expected,
        } => ParseDiagnostic::new(unexpected_token_message(&token, expected.names()))
            .with_span(span, Exact),
        ErSyntaxDiagnostic::UnexpectedEof { offset, expected } => {
            ParseDiagnostic::new(unexpected_eof_message(expected.names()))
                .with_span(SourceSpan::new(offset, offset), InsertionPoint)
        }
    }
}

struct ErSemanticSource {
    model: ErDiagramModel,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<EditorSemanticFacts>,
}

struct ErSemanticFailure {
    diagnostic: ParseDiagnostic,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<EditorSemanticFacts>,
}

impl ErSemanticFailure {
    fn into_parse_error(self, diagram_type: &str) -> Error {
        Error::diagram_parse_diagnostic(diagram_type, self.diagnostic)
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    fn into_error_and_editor_facts(
        self,
        meta: &SourcePresentationMetadata,
    ) -> (Error, EditorSemanticFacts) {
        self.into_error_and_editor_facts_for_type(&meta.diagram_type)
    }

    #[cfg(feature = "editor-facts")]
    fn into_error_and_editor_facts_for_type(
        self,
        diagram_type: &str,
    ) -> (Error, EditorSemanticFacts) {
        let mut editor_facts = self
            .editor_facts
            .expect("ER capture requests its failure journal");
        editor_facts.mark_recovered_from_parse_error(
            format!(
                "er parser recovered after parse error: {}",
                self.diagnostic.message()
            ),
            self.diagnostic.span(),
        );
        let error = Error::diagram_parse_diagnostic(diagram_type, self.diagnostic);
        (error, editor_facts)
    }
}

fn construct_er_semantic_source(
    code: &str,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<ErSemanticSource, Box<ErSemanticFailure>>> {
    #[cfg(test)]
    ER_SYNTAX_CONSTRUCTION_COUNT.set(ER_SYNTAX_CONSTRUCTION_COUNT.get() + 1);

    let syntax = ErSyntax::lex(code, control)?;
    #[cfg(feature = "editor-facts")]
    let editor_facts = collect_editor_facts(&syntax, code, editor_demand, control)?;
    match syntax.into_model(control)? {
        Ok(model) => Ok(Ok(ErSemanticSource {
            model,
            #[cfg(feature = "editor-facts")]
            editor_facts,
        })),
        Err(diagnostic) => Ok(Err(Box::new(ErSemanticFailure {
            diagnostic: er_parse_diagnostic(diagnostic),
            #[cfg(feature = "editor-facts")]
            editor_facts,
        }))),
    }
}

pub(crate) fn parse_er_semantic_model_controlled(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<SemanticConstruction<ErDiagramModel>> {
    let construction = construct_er_semantic_source(
        code,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?;
    Ok(match construction {
        Ok(ErSemanticSource {
            mut model,
            #[cfg(feature = "editor-facts")]
            editor_facts,
        }) => {
            sanitize_common_db_fields(&mut model, &metadata.semantic_config);
            SemanticConstruction {
                model: Ok(model),
                #[cfg(feature = "editor-facts")]
                editor_facts,
            }
        }
        #[cfg(feature = "editor-facts")]
        Err(failure) if editor_demand.needs_recovery() => {
            let (error, editor_facts) =
                (*failure).into_error_and_editor_facts_for_type(&metadata.diagram_type);
            SemanticConstruction {
                model: Err(error),
                #[cfg(feature = "editor-facts")]
                editor_facts: Some(editor_facts),
            }
        }
        Err(failure) => SemanticConstruction {
            model: Err((*failure).into_parse_error(&metadata.diagram_type)),
            #[cfg(feature = "editor-facts")]
            editor_facts: None,
        },
    })
}

#[cfg(any(feature = "compat-json", all(test, feature = "presentation")))]
fn parse_er_semantic_source(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<ErSemanticSource> {
    construct_er_semantic_source(
        code,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        &OperationControl::new(),
    )
    .expect("a private parse control cannot be cancelled")
    .map_err(|failure| (*failure).into_parse_error(&meta.diagram_type))
}

#[cfg(all(test, feature = "presentation"))]
pub(crate) fn parse_er_model_for_render(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<ErDiagramModel> {
    Ok(parse_er_semantic_source(code, meta)?.model)
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_er_model_for_render_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<ErDiagramModel>> {
    let construction = construct_er_semantic_source(
        code,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )?;
    let source = match construction {
        Ok(source) => source,
        Err(failure) => {
            return Ok(Err((*failure).into_parse_error(&meta.diagram_type)));
        }
    };
    control.checkpoint()?;
    Ok(Ok(source.model))
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_er(code: &str, meta: &SourcePresentationMetadata) -> Result<Value> {
    let source = parse_er_semantic_source(code, meta)?;
    render_model_to_compat_json(&source.model, meta)
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub(crate) fn parse_er_json_and_editor_facts(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<crate::family::CombinedSemanticParse> {
    let construction = construct_er_semantic_source(code, EditorFactsDemand::Capture, control)?;
    let parsed = crate::family::CombinedSemanticParse::from_construction(
        construction,
        |ErSemanticSource {
             model,
             #[cfg(feature = "editor-facts")]
             editor_facts,
         }| {
            (
                render_model_to_compat_json(&model, meta),
                editor_facts.expect("ER combined parse requests editor facts"),
            )
        },
        |failure| (*failure).into_error_and_editor_facts(meta),
    );
    control.checkpoint()?;
    Ok(parsed)
}

#[derive(Debug, Default)]
#[cfg(feature = "editor-facts")]
struct ErEditorFactCollector {
    pending_entity: Option<ErTokenSymbol>,
    deferred_styled_entity: Option<ErTokenSymbol>,
    deferred_styled_ids: Option<ErIdList>,
    relationship_started: bool,
    known_entities: HashSet<String>,
    expected_id_list: Option<PendingErExpectation>,
    directive_start: Option<usize>,
    style_payload_pending: bool,
    style_payload_anchor_end: Option<usize>,
    subgraph_id_pending: bool,
    in_attribute_block: bool,
    in_alias: bool,
    in_relationship_role: bool,
    attr_word_index: usize,
}

#[cfg(feature = "editor-facts")]
impl ErEditorFactCollector {
    fn finish(&mut self, code: &str, facts: &mut EditorSemanticFacts) {
        self.finish_line(code, line_content_end(code, code.len()), facts);
        self.push_deferred_styled_entity(facts, ErEntityOccurrence::Definition);
        self.push_pending_entity(facts, ErEntityOccurrence::Definition);
    }

    fn finish_line(&mut self, code: &str, line_end: usize, facts: &mut EditorSemanticFacts) {
        // The exclusive style whitespace rule can consume physical line endings. Keep an
        // unfinished target/value slot on its authored line without changing strict lexing.
        let line_end = if self.style_payload_pending {
            self.style_payload_anchor_end
                .or(self.directive_start)
                .and_then(|anchor_end| {
                    code.get(anchor_end..line_end)?
                        .find(['\r', '\n'])
                        .map(|offset| anchor_end + offset)
                })
                .unwrap_or(line_end)
        } else {
            line_end
        };
        if let Some(start) = self.directive_start {
            facts.push_expected_syntax(EditorExpectedSyntax::new(
                EditorExpectedSyntaxKind::Directive,
                SourceSpan::new(start, line_end),
            ));
        }
        if let Some(expected) = self.expected_id_list {
            let kind = er_expected_id_list_kind(expected.kind);
            let separator_ready = expected
                .anchor_end
                .is_none_or(|anchor_end| has_ascii_separator(code, anchor_end, line_end));
            if !matches!(expected.kind, ExpectedErIdList::ClassDef)
                && (!matches!(kind, EditorExpectedSyntaxKind::ClassName) || separator_ready)
            {
                facts.push_expected_syntax(EditorExpectedSyntax::new(
                    kind,
                    SourceSpan::new(line_end, line_end),
                ));
            }
        } else if self.style_payload_pending
            && self
                .style_payload_anchor_end
                .is_some_and(|anchor_end| has_ascii_separator(code, anchor_end, line_end))
        {
            facts.push_expected_syntax(EditorExpectedSyntax::new(
                EditorExpectedSyntaxKind::StyleValue,
                SourceSpan::new(line_end, line_end),
            ));
        }
        if !self.in_alias && !self.in_relationship_role {
            let occurrence = if self.relationship_started {
                ErEntityOccurrence::Relationship
            } else {
                ErEntityOccurrence::Definition
            };
            self.push_deferred_styled_entity(facts, occurrence);
            self.push_pending_entity(facts, occurrence);
        }
    }
}

#[derive(Debug, Clone)]
#[cfg(feature = "editor-facts")]
struct ErTokenSymbol {
    name: String,
    span: SourceSpan,
    selection: SourceSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(feature = "editor-facts")]
enum ErEntityOccurrence {
    Definition,
    Relationship,
}

#[derive(Debug, Clone, Copy)]
#[cfg(feature = "editor-facts")]
enum ExpectedErIdList {
    StyleEntities,
    ClassDef,
    ClassEntities,
    ClassNames,
    InlineClasses,
}

#[derive(Debug, Clone, Copy)]
#[cfg(feature = "editor-facts")]
struct PendingErExpectation {
    kind: ExpectedErIdList,
    anchor_end: Option<usize>,
}

#[cfg(feature = "editor-facts")]
impl ErEditorFactCollector {
    fn accept(
        &mut self,
        code: &str,
        token: &ErToken,
        start: usize,
        end: usize,
        facts: &mut EditorSemanticFacts,
    ) {
        if self.deferred_styled_entity.is_some() && !matches!(token, ErToken::IdList(_)) {
            let occurrence = if matches!(
                token,
                ErToken::ZeroOrOne
                    | ErToken::ZeroOrMore
                    | ErToken::OneOrMore
                    | ErToken::OnlyOne
                    | ErToken::MdParent
                    | ErToken::Identifying
                    | ErToken::NonIdentifying
            ) {
                ErEntityOccurrence::Relationship
            } else {
                ErEntityOccurrence::Definition
            };
            self.push_deferred_styled_entity(facts, occurrence);
        }

        if self.subgraph_id_pending
            && let ErToken::Name(name) | ErToken::Str(name) = token
        {
            self.subgraph_id_pending = false;
            self.push_pending_entity(facts, ErEntityOccurrence::Definition);
            let id = name.trim_matches(is_ecmascript_whitespace);
            let span = SourceSpan::new(start, end);
            if !id.is_empty()
                && let Some(selection) = source_value_span(code, span, id)
            {
                self.known_entities.insert(id.to_string());
                facts.push_symbol(EditorSemanticSymbol::new(
                    id.to_string(),
                    Some("er subgraph".to_string()),
                    EditorSemanticKind::Struct,
                    span,
                    selection,
                ));
            }
            return;
        }

        match token {
            ErToken::ErDiagram => self.reset_line_state(),
            ErToken::Newline => {
                self.finish_line(code, line_content_end(code, start), facts);
                self.reset_line_state();
            }
            ErToken::StyleKw => {
                facts.push_directive_prefix("style");
                self.directive_start = Some(start);
                self.style_payload_pending = true;
                self.expect_id_list(ExpectedErIdList::StyleEntities);
            }
            ErToken::ClassDefKw => {
                facts.push_directive_prefix("classDef");
                self.directive_start = Some(start);
                self.style_payload_pending = true;
                self.expect_id_list_after(ExpectedErIdList::ClassDef, end);
            }
            ErToken::ClassKw => {
                facts.push_directive_prefix("class");
                self.directive_start = Some(start);
                self.expect_id_list(ExpectedErIdList::ClassEntities);
            }
            ErToken::SubgraphKw => {
                facts.push_directive_prefix("subgraph");
                self.directive_start = Some(start);
                self.subgraph_id_pending = true;
            }
            ErToken::EndKw => {
                self.directive_start = None;
            }
            ErToken::StyleSeparator => {
                if self.relationship_started {
                    self.push_pending_entity(facts, ErEntityOccurrence::Relationship);
                } else {
                    self.deferred_styled_entity = self.pending_entity.take();
                }
                self.directive_start = Some(start);
                self.expect_id_list(ExpectedErIdList::InlineClasses);
            }
            ErToken::IdList(ids) => {
                if self.deferred_styled_entity.is_some() {
                    self.deferred_styled_ids = Some(ids.clone());
                } else {
                    self.push_id_list(ids.clone(), facts);
                }
            }
            ErToken::Name(name) => {
                if self.in_attribute_block {
                    return;
                }
                if self.in_alias {
                    self.push_context_payload(facts, name.clone(), "er entity alias", start, end);
                    return;
                }
                if self.in_relationship_role {
                    self.push_context_payload(
                        facts,
                        name.clone(),
                        "er relationship role",
                        start,
                        end,
                    );
                    self.in_relationship_role = false;
                    return;
                }
                let span = if end.saturating_sub(start) == name.len().saturating_add(2) {
                    SourceSpan::new(start + 1, end - 1)
                } else {
                    SourceSpan::new(start, end)
                };
                let symbol = ErTokenSymbol {
                    name: name.clone(),
                    span: SourceSpan::new(start, end),
                    selection: span,
                };
                if let Some(entity) = self.pending_entity.replace(symbol) {
                    let occurrence = if self.relationship_started {
                        ErEntityOccurrence::Relationship
                    } else {
                        ErEntityOccurrence::Definition
                    };
                    self.push_entity_symbol(facts, entity, "er entity reference", occurrence);
                }
            }
            ErToken::ZeroOrOne
            | ErToken::ZeroOrMore
            | ErToken::OneOrMore
            | ErToken::OnlyOne
            | ErToken::MdParent
            | ErToken::Identifying
            | ErToken::NonIdentifying => {
                self.push_pending_entity(facts, ErEntityOccurrence::Relationship);
                self.relationship_started = true;
            }
            ErToken::Colon => {
                let occurrence = if self.relationship_started {
                    ErEntityOccurrence::Relationship
                } else {
                    ErEntityOccurrence::Definition
                };
                self.push_deferred_styled_entity(facts, occurrence);
                self.push_pending_entity(facts, occurrence);
                self.in_relationship_role = true;
            }
            ErToken::BlockStart => {
                self.push_deferred_styled_entity(facts, ErEntityOccurrence::Definition);
                self.push_pending_entity(facts, ErEntityOccurrence::Definition);
                self.in_attribute_block = true;
                self.attr_word_index = 0;
            }
            ErToken::BlockStop => {
                self.in_attribute_block = false;
                self.attr_word_index = 0;
            }
            ErToken::AttrWord(word) => {
                if !self.in_attribute_block {
                    return;
                }
                let span = SourceSpan::new(start, end);
                if self.attr_word_index.is_multiple_of(2) {
                    self.push_payload_symbol(
                        facts,
                        word.clone(),
                        "er attribute type",
                        EditorSemanticKind::String,
                        span,
                        span,
                    );
                } else {
                    self.push_attribute_symbol(facts, word.clone(), span);
                }
                self.attr_word_index += 1;
            }
            ErToken::Comma => {
                if self.in_attribute_block && self.attr_word_index > 2 {
                    self.attr_word_index = 2;
                }
            }
            ErToken::Question => {}
            ErToken::AttrKey(key) => {
                if self.in_attribute_block {
                    self.push_payload_symbol(
                        facts,
                        key.clone(),
                        "er attribute key",
                        EditorSemanticKind::Property,
                        SourceSpan::new(start, end),
                        SourceSpan::new(start, end),
                    );
                }
            }
            ErToken::Comment(comment) => {
                if self.in_attribute_block {
                    let span = SourceSpan::new(start, end);
                    let selection = if end.saturating_sub(start) >= 2 {
                        SourceSpan::new(start + 1, end - 1)
                    } else {
                        span
                    };
                    self.push_payload_symbol(
                        facts,
                        comment.clone(),
                        "er attribute comment",
                        EditorSemanticKind::String,
                        span,
                        selection,
                    );
                }
            }
            ErToken::AccTitle(_) => facts.push_directive_prefix("accTitle"),
            ErToken::AccDescr(_) | ErToken::AccDescrMultiline(_) => {
                facts.push_directive_prefix("accDescr")
            }
            ErToken::SquareStart => {
                self.push_deferred_styled_entity(facts, ErEntityOccurrence::Definition);
                self.push_pending_entity(facts, ErEntityOccurrence::Definition);
                self.in_alias = true;
            }
            ErToken::SquareStop => self.in_alias = false,
            ErToken::Str(value) => {
                let detail = if self.in_alias {
                    Some("er entity alias")
                } else if self.in_relationship_role {
                    self.in_relationship_role = false;
                    Some("er relationship role")
                } else {
                    None
                };
                if let Some(detail) = detail {
                    self.push_context_payload(facts, value.clone(), detail, start, end);
                }
            }
            ErToken::Direction(direction) => {
                facts.push_expected_syntax(EditorExpectedSyntax::new(
                    EditorExpectedSyntaxKind::CardinalDirectionValue,
                    direction.selection(),
                ));
            }
            ErToken::RestOfLine(raw) => {
                if self.style_payload_pending {
                    let line_end = end + code[end..].find(['\r', '\n']).unwrap_or(code.len() - end);
                    let trailing_slot = trailing_ascii_whitespace_slot(code, start, line_end)
                        .or_else(|| {
                            (raw.is_empty()
                                && self.style_payload_anchor_end.is_some_and(|anchor_end| {
                                    has_ascii_separator(code, anchor_end, line_end)
                                }))
                            .then_some(SourceSpan::new(line_end, line_end))
                        });
                    if let Some(span) = trailing_slot {
                        facts.push_expected_syntax(EditorExpectedSyntax::new(
                            EditorExpectedSyntaxKind::StyleValue,
                            span,
                        ));
                    }
                    if start < end {
                        facts.push_expected_syntax(EditorExpectedSyntax::new(
                            EditorExpectedSyntaxKind::Payload,
                            SourceSpan::new(start, end),
                        ));
                    }
                    self.style_payload_pending = false;
                }
            }
        }
    }

    fn reset_line_state(&mut self) {
        self.pending_entity = None;
        self.deferred_styled_entity = None;
        self.deferred_styled_ids = None;
        self.expected_id_list = None;
        self.directive_start = None;
        self.style_payload_pending = false;
        self.style_payload_anchor_end = None;
        self.subgraph_id_pending = false;
        self.in_alias = false;
        self.in_relationship_role = false;
        self.relationship_started = false;
        if !self.in_attribute_block {
            self.attr_word_index = 0;
        }
    }

    fn push_pending_entity(
        &mut self,
        facts: &mut EditorSemanticFacts,
        occurrence: ErEntityOccurrence,
    ) {
        if let Some(entity) = self.pending_entity.take() {
            let detail = match occurrence {
                ErEntityOccurrence::Definition => "er entity",
                ErEntityOccurrence::Relationship => "er relationship entity",
            };
            self.push_entity_symbol(facts, entity, detail, occurrence);
        }
    }

    fn push_deferred_styled_entity(
        &mut self,
        facts: &mut EditorSemanticFacts,
        occurrence: ErEntityOccurrence,
    ) {
        if let Some(entity) = self.deferred_styled_entity.take() {
            let detail = match occurrence {
                ErEntityOccurrence::Definition => "er entity",
                ErEntityOccurrence::Relationship => "er relationship entity",
            };
            self.push_entity_symbol(facts, entity, detail, occurrence);
        }
        if let Some(ids) = self.deferred_styled_ids.take() {
            self.push_id_list(ids, facts);
        }
    }

    fn push_id_list(&mut self, ids: ErIdList, facts: &mut EditorSemanticFacts) {
        let expected = self.expected_id_list.take().map(|pending| pending.kind);
        let span = ids.span();
        if matches!(
            expected,
            Some(ExpectedErIdList::StyleEntities | ExpectedErIdList::ClassDef)
        ) {
            self.style_payload_anchor_end = Some(span.end);
        }
        let detail = match expected {
            Some(ExpectedErIdList::StyleEntities) => "er style target",
            Some(ExpectedErIdList::ClassDef) => "er class definition",
            Some(ExpectedErIdList::ClassEntities) => "er class target",
            Some(ExpectedErIdList::ClassNames) => "er class name",
            Some(ExpectedErIdList::InlineClasses) => "er inline class",
            None => "er id",
        };
        let kind = match expected {
            Some(ExpectedErIdList::ClassDef)
            | Some(ExpectedErIdList::ClassNames)
            | Some(ExpectedErIdList::InlineClasses) => EditorSemanticKind::Property,
            _ => EditorSemanticKind::Struct,
        };

        if let Some(expected) = expected {
            facts.push_expected_syntax(EditorExpectedSyntax::new(
                er_expected_id_list_kind(expected),
                span,
            ));
        }

        for (name, span) in ids.into_spanned_ids() {
            if name.is_empty() {
                continue;
            }
            let symbol = match expected {
                Some(ExpectedErIdList::ClassDef) => EditorSemanticSymbol::class_definition(
                    name,
                    Some(detail.to_string()),
                    kind,
                    span,
                    span,
                ),
                Some(ExpectedErIdList::StyleEntities | ExpectedErIdList::ClassEntities) => {
                    EditorSemanticSymbol::reference(
                        name,
                        Some(detail.to_string()),
                        kind,
                        span,
                        span,
                    )
                }
                Some(ExpectedErIdList::ClassNames | ExpectedErIdList::InlineClasses) => {
                    EditorSemanticSymbol::payload(name, Some(detail.to_string()), kind, span, span)
                }
                None => {
                    EditorSemanticSymbol::payload(name, Some(detail.to_string()), kind, span, span)
                }
            };
            facts.push_symbol(symbol);
        }

        if matches!(expected, Some(ExpectedErIdList::ClassEntities)) {
            self.expect_id_list_after(ExpectedErIdList::ClassNames, span.end);
        }
    }

    fn expect_id_list(&mut self, expected: ExpectedErIdList) {
        self.expected_id_list = Some(PendingErExpectation {
            kind: expected,
            anchor_end: None,
        });
    }

    fn expect_id_list_after(&mut self, expected: ExpectedErIdList, anchor_end: usize) {
        self.expected_id_list = Some(PendingErExpectation {
            kind: expected,
            anchor_end: Some(anchor_end),
        });
    }

    fn push_entity_symbol(
        &mut self,
        facts: &mut EditorSemanticFacts,
        symbol: ErTokenSymbol,
        detail: &'static str,
        occurrence: ErEntityOccurrence,
    ) {
        if symbol.name.is_empty() {
            return;
        }
        let is_implicit_definition = matches!(occurrence, ErEntityOccurrence::Relationship)
            && !self.known_entities.contains(&symbol.name);
        if matches!(occurrence, ErEntityOccurrence::Definition) || is_implicit_definition {
            self.known_entities.insert(symbol.name.clone());
        }
        let detail = Some(if is_implicit_definition {
            "er implicit entity".to_string()
        } else {
            detail.to_string()
        });
        let entity =
            if matches!(occurrence, ErEntityOccurrence::Relationship) && !is_implicit_definition {
                EditorSemanticSymbol::reference(
                    symbol.name,
                    detail,
                    EditorSemanticKind::Struct,
                    symbol.span,
                    symbol.selection,
                )
            } else {
                EditorSemanticSymbol::new(
                    symbol.name,
                    detail,
                    EditorSemanticKind::Struct,
                    symbol.span,
                    symbol.selection,
                )
            };
        facts.push_symbol(entity);
    }

    fn push_attribute_symbol(
        &self,
        facts: &mut EditorSemanticFacts,
        name: String,
        span: SourceSpan,
    ) {
        if name.is_empty() {
            return;
        }
        facts.push_symbol(EditorSemanticSymbol::outline(
            name,
            Some("er attribute".to_string()),
            EditorSemanticKind::Property,
            span,
            span,
        ));
    }

    fn push_payload_symbol(
        &self,
        facts: &mut EditorSemanticFacts,
        name: String,
        detail: &'static str,
        kind: EditorSemanticKind,
        span: SourceSpan,
        selection: SourceSpan,
    ) {
        if name.is_empty() {
            return;
        }
        facts.push_symbol(EditorSemanticSymbol::payload(
            name,
            Some(detail.to_string()),
            kind,
            span,
            selection,
        ));
    }

    fn push_context_payload(
        &self,
        facts: &mut EditorSemanticFacts,
        name: String,
        detail: &'static str,
        start: usize,
        end: usize,
    ) {
        let span = SourceSpan::new(start, end);
        let selection = if end.saturating_sub(start) == name.len().saturating_add(2) {
            SourceSpan::new(start + 1, end - 1)
        } else {
            span
        };
        self.push_payload_symbol(
            facts,
            name,
            detail,
            EditorSemanticKind::String,
            span,
            selection,
        );
    }
}

#[cfg(feature = "editor-facts")]
fn er_expected_id_list_kind(expected: ExpectedErIdList) -> EditorExpectedSyntaxKind {
    match expected {
        ExpectedErIdList::ClassDef
        | ExpectedErIdList::ClassNames
        | ExpectedErIdList::InlineClasses => EditorExpectedSyntaxKind::ClassName,
        ExpectedErIdList::StyleEntities | ExpectedErIdList::ClassEntities => {
            EditorExpectedSyntaxKind::IdList
        }
    }
}

#[cfg(all(test, feature = "compat-json"))]
mod tests {
    use super::*;
    #[cfg(feature = "editor-facts")]
    use crate::EditorSemanticRole;
    use crate::MermaidConfig;

    fn meta() -> SourcePresentationMetadata {
        SourcePresentationMetadata {
            diagram_type: "er".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        }
    }

    #[test]
    fn er_typed_projection_matches_complete_compat_json() {
        let text = concat!(
            "erDiagram\n",
            "accTitle: Orders\n",
            "CUSTOMER ||--o{ ORDER : places\n",
        );
        let meta = meta();
        let compat = parse_er(text, &meta).unwrap();
        let typed = parse_er_model_for_render(text, &meta).unwrap();
        let projection = render_model_to_compat_json(&typed, &meta).unwrap();

        assert_eq!(projection, compat);
        assert_eq!(projection["type"], json!("er"));
        assert_eq!(projection["accDescr"], Value::Null);
        assert_eq!(
            projection["constants"]["cardinality"]["onlyOne"],
            json!("ONLY_ONE")
        );
    }

    #[test]
    fn er_accepts_decimal_and_dotted_entity_names() {
        let model = parse_er("erDiagram\n1.5 ||--|| Sales.Order : owns\n", &meta())
            .expect("ER entities with dots and decimal names should parse");

        assert!(model["entities"].get("1.5").is_some());
        assert!(model["entities"].get("Sales.Order").is_some());
        assert_eq!(model["relationships"].as_array().map(Vec::len), Some(1));
    }

    #[test]
    fn er_entity_render_label_uses_javascript_string_truthiness() {
        let mut entity = ErEntity {
            label: "CUSTOMER".to_string(),
            alias: "Buyer".to_string(),
            ..Default::default()
        };

        assert_eq!(entity.label_source_for_render(), "Buyer");

        entity.alias = " \t ".to_string();
        assert_eq!(entity.label_source_for_render(), " \t ");

        entity.alias.clear();
        assert_eq!(entity.label_source_for_render(), "CUSTOMER");
    }

    #[test]
    fn er_typed_serde_preserves_declaration_order_and_compat_json_members() {
        let text = concat!(
            "erDiagram\n",
            "ZETA\n",
            "ALPHA\n",
            "MIDDLE\n",
            "classDef zeta fill:#fff\n",
            "classDef alpha fill:#000\n",
            "classDef middle fill:#ccc\n",
        );
        let meta = meta();
        let typed = parse_er_model_for_render(text, &meta).expect("ER model should parse");
        let entity_order = ["ZETA", "ALPHA", "MIDDLE"];
        let class_order = ["zeta", "alpha", "middle"];
        assert!(
            typed
                .entities
                .values()
                .map(|entity| entity.label.as_str())
                .eq(entity_order)
        );
        assert!(typed.classes.keys().map(String::as_str).eq(class_order));

        let compat = render_model_to_compat_json(&typed, &meta)
            .expect("ER compatibility projection should serialize");
        let entities = compat["entities"].as_object().unwrap();
        assert_eq!(entities.len(), entity_order.len());
        for name in entity_order {
            assert_eq!(entities[name]["label"], name);
        }
        let classes = compat["classes"].as_object().unwrap();
        assert_eq!(classes.len(), class_order.len());
        for name in class_order {
            assert_eq!(classes[name]["id"], name);
        }

        let wire = serde_json::to_vec(&typed).expect("ER typed model should serialize");
        let round_trip: ErDiagramModel =
            serde_json::from_slice(&wire).expect("ER typed model should deserialize");
        assert!(
            round_trip
                .entities
                .values()
                .map(|entity| entity.label.as_str())
                .eq(entity_order)
        );
        assert!(
            round_trip
                .classes
                .keys()
                .map(String::as_str)
                .eq(class_order)
        );
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn er_entity_occurrence_role_does_not_depend_on_display_detail() {
        let span = SourceSpan::new(0, "CUSTOMER".len());
        let symbol = || ErTokenSymbol {
            name: "CUSTOMER".to_string(),
            span,
            selection: span,
        };
        let mut collector = ErEditorFactCollector::default();
        let mut facts = EditorSemanticFacts::new();
        collector.push_entity_symbol(
            &mut facts,
            symbol(),
            "reference-looking definition",
            ErEntityOccurrence::Definition,
        );
        collector.push_entity_symbol(
            &mut facts,
            symbol(),
            "definition-looking relationship occurrence",
            ErEntityOccurrence::Relationship,
        );

        assert_eq!(facts.symbols[0].role, EditorSemanticRole::Entity);
        assert_eq!(facts.symbols[1].role, EditorSemanticRole::Reference);
    }

    #[test]
    fn er_subgraphs_preserve_membership_and_local_direction() {
        let text = concat!(
            "erDiagram\n",
            "direction LR\n",
            "subgraph Orders [Order Domain]\n",
            "  direction TB\n",
            "  CUSTOMER ||--o{ ORDER : places\n",
            "end\n",
            "WAREHOUSE ||--o{ Orders : ships\n",
        );
        let model = parse_er_model_for_render(text, &meta()).expect("ER subgraphs should parse");

        assert_eq!(model.direction, "LR");
        assert_eq!(model.subgraphs.len(), 1);
        let subgraph = &model.subgraphs[0];
        assert_eq!(subgraph.id, "Orders");
        assert_eq!(subgraph.title, "Order Domain");
        assert_eq!(subgraph.dir.as_deref(), Some("TB"));
        assert_eq!(subgraph.nodes, ["CUSTOMER", "ORDER"]);
        assert_eq!(model.entities.len(), 4);
        assert_eq!(model.entities["Orders"].id, "entity-Orders-3");
        assert_eq!(model.relationships.len(), 2);
        assert_eq!(model.relationships[1].entity_b, "Orders");
    }

    #[test]
    fn er_subgraphs_support_nested_and_quoted_headers() {
        let text = concat!(
            "erDiagram\n",
            "subgraph \"Customer Domain\" [\"Customer %% Domain\"]\n",
            "  subgraph Billing [Billing Area]\n",
            "    INVOICE\n",
            "  end\n",
            "  CUSTOMER\n",
            "end\n",
        );
        let model = parse_er_model_for_render(text, &meta()).expect("nested ER subgraphs parse");

        assert_eq!(model.subgraphs.len(), 2);
        assert_eq!(model.subgraphs[0].id, "Billing");
        assert_eq!(model.subgraphs[0].title, "Billing Area");
        assert_eq!(model.subgraphs[1].id, "Customer Domain");
        assert_eq!(model.subgraphs[1].title, "Customer %% Domain");
        assert_eq!(model.subgraphs[1].nodes, ["Billing", "CUSTOMER"]);
        assert_eq!(model.subgraphs[0].nodes, ["INVOICE"]);
    }

    #[test]
    fn er_subgraph_styles_apply_after_the_subgraph_is_registered() {
        let text = concat!(
            "erDiagram\n",
            "subgraph Orders\n",
            "  ORDER\n",
            "end\n",
            "class Orders important\n",
            "style Orders fill:#eee,stroke:#333\n",
        );
        let model = parse_er_model_for_render(text, &meta()).expect("styled ER subgraph parses");
        let subgraph = &model.subgraphs[0];
        assert_eq!(subgraph.classes, ["important"]);
        assert_eq!(subgraph.css_styles, ["fill:#eee", "stroke:#333"]);
    }

    #[test]
    fn er_subgraph_header_may_follow_er_diagram_on_same_line() {
        let model = parse_er_model_for_render(
            "erDiagram subgraph WithRL\ndirection RL\nA\nB\nend\n",
            &meta(),
        )
        .expect("same-line ER subgraph parses");
        assert_eq!(model.subgraphs.len(), 1);
        assert_eq!(model.subgraphs[0].id, "WithRL");
        assert_eq!(model.subgraphs[0].dir.as_deref(), Some("RL"));
        assert_eq!(model.subgraphs[0].nodes, ["A", "B"]);
    }
}

#[cfg(test)]
mod semantic_tests {
    use super::*;

    fn metadata() -> crate::SemanticMetadata {
        crate::SemanticMetadata {
            diagram_type: "er".to_string(),
            config: crate::MermaidConfig::empty_object(),
            semantic_config: crate::MermaidConfig::empty_object(),
            title: None,
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn er_canonical_capture_retains_success_and_grammar_failure_facts() {
        for (source, valid) in [
            ("erDiagram\nA\n", true),
            ("erDiagram\nA {\n string\n}\nAFTER\n", false),
        ] {
            reset_er_syntax_construction_count();
            #[cfg(feature = "editor-facts")]
            crate::editor::test_work::reset();
            let parsed = parse_er_semantic_model_controlled(
                source,
                &metadata(),
                EditorFactsDemand::Capture,
                &OperationControl::new(),
            )
            .unwrap();
            assert_eq!(parsed.model.is_ok(), valid);
            let facts = parsed.editor_facts.expect("same-pass ER facts");
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "A"));
            if let Some(start) = source.find("AFTER") {
                assert!(facts.symbols.iter().any(|symbol| {
                    symbol.name == "AFTER" && symbol.selection == SourceSpan::new(start, start + 5)
                }));
            }
            assert_eq!(er_syntax_construction_count(), 1);
            #[cfg(feature = "editor-facts")]
            assert_eq!(crate::editor::test_work::read().collections, 1);
            assert_eq!(
                facts.completeness,
                if valid {
                    crate::EditorSemanticCompleteness::Complete
                } else {
                    crate::EditorSemanticCompleteness::Recovered
                }
            );
            assert_eq!(facts.diagnostics.is_empty(), valid);
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn er_recovery_demand_records_only_the_first_diagnostic() {
        use crate::{EditorSemanticCompleteness, EditorSemanticDiagnosticKind};

        let source = "erDiagram\nBEFORE\n@\nAFTER\n";
        let start = source.find('@').unwrap();
        let span = SourceSpan::new(start, start + 1);
        crate::editor::test_work::reset();

        let parsed = parse_er_semantic_model_controlled(
            source,
            &metadata(),
            EditorFactsDemand::Recovery,
            &OperationControl::new(),
        )
        .unwrap();
        let Error::DiagramParse { diagnostic, .. } = parsed.model.unwrap_err() else {
            panic!("expected an ER parse error");
        };
        assert_eq!(diagnostic.span(), Some(span));
        let facts = parsed.editor_facts.expect("ER recovery diagnostic");
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(facts.symbols.is_empty());
        assert_eq!(facts.diagnostics.len(), 1);
        assert_eq!(facts.diagnostics[0].span, Some(span));
        assert_eq!(
            facts.diagnostics[0].kind,
            EditorSemanticDiagnosticKind::ParserRecovery
        );
        assert_eq!(crate::editor::test_work::read().collections, 0);
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn er_incomplete_style_slots_do_not_relax_required_targets_or_payloads() {
        for newline in ["\n", "\r\n", "\r"] {
            for (statement, kind) in [
                ("style ", EditorExpectedSyntaxKind::IdList),
                ("style A ", EditorExpectedSyntaxKind::StyleValue),
                ("classDef accent ", EditorExpectedSyntaxKind::StyleValue),
            ] {
                let source =
                    format!("erDiagram{newline}A{newline}{statement}{newline}\t {newline}");
                let parsed = parse_er_semantic_model_controlled(
                    &source,
                    &metadata(),
                    EditorFactsDemand::Capture,
                    &OperationControl::new(),
                )
                .unwrap();
                assert!(parsed.model.is_err(), "{source:?}");
                let facts = parsed.editor_facts.expect("same-pass ER recovery facts");
                assert_eq!(
                    facts.completeness,
                    crate::EditorSemanticCompleteness::Recovered
                );
                let slot = source.find(statement).unwrap() + statement.len();
                assert!(facts.expected_syntax.contains(&EditorExpectedSyntax::new(
                    kind,
                    SourceSpan::new(slot, slot),
                )));
            }
        }
    }

    #[test]
    fn er_canonical_model_constructs_once_without_editor_work() {
        let source = "erDiagram\ndirection LR\nsubgraph Orders [Order Domain]\nCUSTOMER ||--o{ ORDER : places\nend\n";
        reset_er_syntax_construction_count();
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let model = parse_er_semantic_model_controlled(
            source,
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();
        assert_eq!(model.direction, "LR");
        assert_eq!(
            model
                .entities
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["CUSTOMER", "ORDER"]
        );
        assert_eq!(model.relationships[0].role_a, "places");
        assert_eq!(model.subgraphs[0].nodes, ["CUSTOMER", "ORDER"]);
        assert_eq!(er_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
    }

    #[test]
    fn er_parser_preserves_expected_tokens_and_spans() {
        use ParseDiagnosticSpanKind::{Exact, InsertionPoint};

        let endpoint = "erDiagram\nA ||--|| B {";
        let role = "erDiagram\nA ||--|| B : \t";
        let role_end = role.find(':').unwrap() + 1;
        let subgraph = "erDiagram\nsubgraph G\n \n";
        let attribute_block = "erDiagram\nA { int id   ";
        for (source, message, span, span_kind) in [
            (
                "\nA",
                "unexpected name; expected \"er diagram\"",
                SourceSpan::new(1, 2),
                Exact,
            ),
            (
                endpoint,
                "unexpected block start; expected style separator, colon",
                SourceSpan::new(endpoint.len() - 1, endpoint.len()),
                Exact,
            ),
            (
                role,
                "unexpected end of input; expected name, str",
                SourceSpan::new(role_end, role_end),
                InsertionPoint,
            ),
            (
                subgraph,
                concat!(
                    "unexpected end of input; expected name, acc title, acc descr, ",
                    "acc descr multiline, diagram direction, \"style\", \"class def\", ",
                    "\"class\", \"subgraph\", \"end\""
                ),
                SourceSpan::new(subgraph.len(), subgraph.len()),
                InsertionPoint,
            ),
            (
                attribute_block,
                "EOF inside attribute block",
                SourceSpan::new(attribute_block.len(), attribute_block.len()),
                Exact,
            ),
        ] {
            let parse = parse_er_semantic_model_controlled(
                source,
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &OperationControl::new(),
            )
            .unwrap();
            let Error::DiagramParse { diagnostic, .. } = parse.model.unwrap_err() else {
                panic!("expected parse diagnostic for {source:?}");
            };
            assert_eq!(diagnostic.message(), message, "{source:?}");
            assert_eq!(diagnostic.span(), Some(span), "{source:?}");
            assert_eq!(diagnostic.span_kind(), span_kind, "{source:?}");
        }
    }

    #[test]
    fn er_canonical_errors_keep_spans_without_editor_work() {
        let source = "erDiagram\nA {\n string\n}\n";
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let error = parse_er_semantic_model_controlled(
            source,
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected parse diagnostic")
        };
        assert!(diagnostic.span().is_some());
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        #[cfg(feature = "compat-json")]
        {
            let Error::DiagramParse {
                diagnostic: full, ..
            } = parse_er(
                source,
                &SourcePresentationMetadata {
                    diagram_type: "er".to_string(),
                    config: crate::MermaidConfig::empty_object(),
                    effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
                    title: None,
                },
            )
            .unwrap_err()
            else {
                panic!("expected parse diagnostic")
            };
            assert_eq!(diagnostic, full);
        }
    }

    #[test]
    fn er_canonical_construction_observes_cancellation() {
        let control = OperationControl::new();
        control.cancel();
        assert!(
            parse_er_semantic_model_controlled(
                "erDiagram\nA\n",
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &control,
            )
            .is_err()
        );
    }
    #[cfg(feature = "presentation")]
    #[test]
    fn er_typed_projection_omits_editor_observers() {
        let metadata = SourcePresentationMetadata {
            diagram_type: "er".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        };
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let model = parse_er_model_for_render_controlled(
            "erDiagram\nA\n",
            &metadata,
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        assert!(!model.entities.is_empty());
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
    }
}
