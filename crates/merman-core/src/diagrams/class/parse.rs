#[cfg(any(feature = "editor-facts", test))]
use crate::SourceSpan;
#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
use crate::family::SemanticConstruction;
use crate::models::class_diagram as class_typed;
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol,
};
use crate::{
    Error, MermaidConfig, OperationControl, OperationControlResult,
    parse_diagnostic::lalrpop_parse_diagnostic,
};
#[cfg(feature = "presentation")]
use crate::{ParseMetadata, Result};
#[cfg(feature = "editor-facts")]
use crate::{
    parse_diagnostic::{format_lalrpop_parse_error, lalrpop_recovery_span},
    source_span::{
        has_ascii_separator, keyword_value_span, line_content_end, source_value_span,
        trailing_ascii_whitespace_slot,
    },
};
#[cfg(feature = "compat-json")]
use serde_json::Value;
#[cfg(test)]
use std::cell::Cell;
#[cfg(feature = "editor-facts")]
use std::collections::HashSet;

#[cfg(feature = "editor-facts")]
use super::MERMAID_DOM_ID_PREFIX;
use super::Tok;
use super::ast::{ActionArena, Actions};
use super::class_grammar;
use super::db::ClassDb;
use super::lexer::Lexer;

#[cfg(test)]
thread_local! {
    static CLASS_SYNTAX_CONSTRUCTION_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_class_syntax_construction_count() {
    CLASS_SYNTAX_CONSTRUCTION_COUNT.set(0);
}

#[cfg(test)]
pub(crate) fn class_syntax_construction_count() -> usize {
    CLASS_SYNTAX_CONSTRUCTION_COUNT.get()
}

type ClassLexicalEvent = std::result::Result<(usize, Tok, usize), super::LexError>;
type ClassGrammarError = lalrpop_util::ParseError<usize, Tok, super::LexError>;

struct ClassSyntaxOutcome {
    actions: std::result::Result<Actions, ClassGrammarError>,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<EditorSemanticFacts>,
}

struct ClassSyntax {
    events: Vec<ClassLexicalEvent>,
}

impl ClassSyntax {
    fn lex(code: &str, control: &OperationControl) -> OperationControlResult<Self> {
        #[cfg(test)]
        CLASS_SYNTAX_CONSTRUCTION_COUNT.set(CLASS_SYNTAX_CONSTRUCTION_COUNT.get() + 1);

        let events = crate::diagrams::scan::collect_lexical_events(&mut Lexer::new(code), control)?;
        Ok(Self { events })
    }

    fn into_actions(
        self,
        #[cfg(feature = "editor-facts")] code: &str,
        #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
        control: &OperationControl,
    ) -> OperationControlResult<ClassSyntaxOutcome> {
        let Self { events } = self;
        #[cfg(feature = "editor-facts")]
        let editor_facts = match editor_demand {
            EditorFactsDemand::Omit => None,
            EditorFactsDemand::Capture => Some(collect_class_editor_facts_from_events(
                &events, code, control,
            )?),
        };
        control.checkpoint()?;
        let mut emitted = 0usize;
        let controlled_events = events.into_iter().take_while(|_| {
            let active = !emitted.is_multiple_of(128) || control.checkpoint().is_ok();
            emitted = emitted.saturating_add(1);
            active
        });
        let mut arena = ActionArena::default();
        let actions = class_grammar::ActionsParser::new()
            .parse(&mut arena, control, controlled_events)
            .map(|list| arena.into_actions(list));
        control.checkpoint()?;
        Ok(ClassSyntaxOutcome {
            actions,
            #[cfg(feature = "editor-facts")]
            editor_facts,
        })
    }
}

struct ClassSemanticSource<'a> {
    db: ClassDb<'a>,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<EditorSemanticFacts>,
}

enum ClassSemanticFailure {
    Grammar {
        error: ClassGrammarError,
        #[cfg(feature = "editor-facts")]
        editor_facts: Option<EditorSemanticFacts>,
    },
    Db {
        message: String,
        #[cfg(feature = "editor-facts")]
        editor_facts: Option<EditorSemanticFacts>,
    },
}

impl ClassSemanticFailure {
    fn into_parse_error(self, diagram_type: &str, fallback_offset: usize) -> Error {
        self.parse_error(diagram_type, fallback_offset)
    }

    fn parse_error(&self, diagram_type: &str, fallback_offset: usize) -> Error {
        match self {
            Self::Grammar { error, .. } => Error::diagram_parse_diagnostic(
                diagram_type.to_string(),
                lalrpop_parse_diagnostic(error, fallback_offset),
            ),
            Self::Db { message, .. } => {
                Error::diagram_parse_fallback(diagram_type.to_string(), message.clone())
            }
        }
    }

    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    fn into_error_and_editor_facts(
        self,
        meta: &ParseMetadata,
        fallback_offset: usize,
    ) -> (Error, EditorSemanticFacts) {
        self.into_error_and_editor_facts_for_type(&meta.diagram_type, fallback_offset)
    }

    #[cfg(feature = "editor-facts")]
    fn into_error_and_editor_facts_for_type(
        self,
        diagram_type: &str,
        fallback_offset: usize,
    ) -> (Error, EditorSemanticFacts) {
        let parse_error = self.parse_error(diagram_type, fallback_offset);
        match self {
            Self::Grammar {
                error,
                editor_facts,
            } => {
                let span = match &error {
                    lalrpop_util::ParseError::User { error } => error.span,
                    _ => lalrpop_recovery_span(&error, fallback_offset),
                };
                let mut editor_facts =
                    editor_facts.expect("Class capture requests its failure journal");
                editor_facts.mark_recovered_from_parse_error(
                    format!(
                        "class parser recovered after parse error: {}",
                        format_lalrpop_parse_error(&error)
                    ),
                    Some(span),
                );
                (parse_error, editor_facts)
            }
            Self::Db {
                message,
                editor_facts,
            } => {
                let mut editor_facts =
                    editor_facts.expect("Class capture requests its failure journal");
                editor_facts.mark_recovered_from_parse_error(
                    format!("class semantic construction failed: {message}"),
                    None,
                );
                (parse_error, editor_facts)
            }
        }
    }
}

fn construct_class_semantic_source<'a>(
    code: &str,
    config: &'a MermaidConfig,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<ClassSemanticSource<'a>, Box<ClassSemanticFailure>>>
{
    let syntax = ClassSyntax::lex(code, control)?;
    let ClassSyntaxOutcome {
        actions,
        #[cfg(feature = "editor-facts")]
        editor_facts,
    } = syntax.into_actions(
        #[cfg(feature = "editor-facts")]
        code,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?;
    let actions = match actions {
        Ok(actions) => actions,
        Err(error) => {
            return Ok(Err(Box::new(ClassSemanticFailure::Grammar {
                error,
                #[cfg(feature = "editor-facts")]
                editor_facts,
            })));
        }
    };

    let mut db = ClassDb::new(config);
    for (index, action) in actions.into_iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        if let Err(message) = db.apply(action) {
            return Ok(Err(Box::new(ClassSemanticFailure::Db {
                message,
                #[cfg(feature = "editor-facts")]
                editor_facts,
            })));
        }
    }

    control.checkpoint()?;
    Ok(Ok(ClassSemanticSource {
        db,
        #[cfg(feature = "editor-facts")]
        editor_facts,
    }))
}

pub(crate) fn parse_class_semantic_model_controlled(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<SemanticConstruction<class_typed::ClassDiagramModel>> {
    let construction = construct_class_semantic_source(
        code,
        &metadata.semantic_config,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?;
    Ok(match construction {
        Ok(ClassSemanticSource {
            db,
            #[cfg(feature = "editor-facts")]
            editor_facts,
        }) => {
            let mut model = db.into_semantic_model(&metadata.diagram_type);
            model.sanitize_common_db_fields(&metadata.semantic_config);
            SemanticConstruction {
                model: Ok(model),
                #[cfg(feature = "editor-facts")]
                editor_facts,
            }
        }
        #[cfg(feature = "editor-facts")]
        Err(failure) if editor_demand == EditorFactsDemand::Capture => {
            let (error, editor_facts) =
                (*failure).into_error_and_editor_facts_for_type(&metadata.diagram_type, code.len());
            SemanticConstruction {
                model: Err(error),
                #[cfg(feature = "editor-facts")]
                editor_facts: Some(editor_facts),
            }
        }
        Err(failure) => SemanticConstruction {
            model: Err((*failure).into_parse_error(&metadata.diagram_type, code.len())),
            #[cfg(feature = "editor-facts")]
            editor_facts: None,
        },
    })
}

#[cfg(any(feature = "compat-json", all(test, feature = "presentation")))]
fn parse_class_semantic_source<'a>(
    code: &str,
    meta: &'a ParseMetadata,
) -> Result<ClassSemanticSource<'a>> {
    construct_class_semantic_source(
        code,
        meta.effective_config.as_config(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        &OperationControl::new(),
    )
    .expect("a private parse control cannot be cancelled")
    .map_err(|failure| (*failure).into_parse_error(&meta.diagram_type, code.len()))
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_class(code: &str, meta: &ParseMetadata) -> Result<Value> {
    Ok(parse_class_semantic_source(code, meta)?.db.into_model(meta))
}

#[cfg(all(test, feature = "presentation"))]
pub(crate) fn parse_class_typed_with_render_context(
    code: &str,
    meta: &ParseMetadata,
) -> Result<(
    class_typed::ClassDiagram,
    class_typed::ClassStylePrecedenceFacts,
)> {
    let model = parse_class_semantic_source(code, meta)?
        .db
        .into_semantic_model(&meta.diagram_type);
    let style_facts = model.style_precedence_facts.clone();
    Ok((
        class_typed::ClassDiagram::new(model, meta.effective_config.as_config()),
        style_facts,
    ))
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_class_typed_with_render_context_controlled(
    code: &str,
    meta: &ParseMetadata,
    control: &OperationControl,
) -> OperationControlResult<
    Result<(
        class_typed::ClassDiagram,
        class_typed::ClassStylePrecedenceFacts,
    )>,
> {
    let construction = construct_class_semantic_source(
        code,
        meta.effective_config.as_config(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )?;
    match construction {
        Ok(source) => {
            let model = source.db.into_semantic_model(&meta.diagram_type);
            let style_facts = model.style_precedence_facts.clone();
            Ok(Ok((
                class_typed::ClassDiagram::new(model, meta.effective_config.as_config()),
                style_facts,
            )))
        }
        Err(failure) => Ok(Err(
            (*failure).into_parse_error(&meta.diagram_type, code.len())
        )),
    }
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub(crate) fn parse_class_json_and_editor_facts(
    code: &str,
    meta: &ParseMetadata,
    control: &OperationControl,
) -> OperationControlResult<crate::family::CombinedSemanticParse> {
    let construction = construct_class_semantic_source(
        code,
        meta.effective_config.as_config(),
        EditorFactsDemand::Capture,
        control,
    )?;
    let parsed = crate::family::CombinedSemanticParse::from_construction(
        construction,
        |ClassSemanticSource {
             db,
             #[cfg(feature = "editor-facts")]
             editor_facts,
         }| {
            (
                Ok(db.into_model(meta)),
                editor_facts.expect("Class combined parse requests editor facts"),
            )
        },
        |failure| (*failure).into_error_and_editor_facts(meta, code.len()),
    );
    control.checkpoint()?;
    Ok(parsed)
}

#[cfg(feature = "editor-facts")]
fn collect_class_editor_facts_from_events(
    events: &[ClassLexicalEvent],
    code: &str,
    control: &OperationControl,
) -> OperationControlResult<EditorSemanticFacts> {
    #[cfg(test)]
    crate::editor::test_work::record_collection();
    let mut facts = EditorSemanticFacts::new();
    let mut collector = ClassEditorFactCollector::new(code);

    for (index, event) in events.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        match event {
            Ok((start, token, end)) => collector.accept(token, *start, *end, &mut facts),
            Err(error) => {
                facts.mark_recovered();
                if let Some(expected) = error.expected_syntax.as_ref() {
                    facts.push_expected_syntax(*expected);
                }
            }
        }
    }
    collector.finish(code.len(), &mut facts);

    control.checkpoint()?;
    Ok(facts)
}

#[derive(Debug)]
#[cfg(feature = "editor-facts")]
struct ClassEditorFactCollector<'a> {
    code: &'a str,
    expected_name: Option<PendingClassExpectation>,
    pending_relation_source: Option<ClassTokenSymbol>,
    after_annotation_start: bool,
    css_class_targets_pending: bool,
    interaction: Option<ClassInteractionKind>,
    interaction_action_seen: bool,
    interaction_action_end: Option<usize>,
    callback_statement_function_seen: bool,
    line_payload: Option<ClassLinePayloadKind>,
    line_payload_anchor_end: Option<usize>,
    directive_start: Option<usize>,
    interaction_target_end: Option<usize>,
    note_text_pending: bool,
    class_label_pending: bool,
    declared_entities: HashSet<String>,
}

#[derive(Debug, Clone)]
#[cfg(feature = "editor-facts")]
struct ClassTokenSymbol {
    name: String,
    span: SourceSpan,
}

#[derive(Debug, Clone, Copy)]
#[cfg(feature = "editor-facts")]
enum ExpectedClassName {
    Class,
    Namespace,
    MemberOwner,
    AnnotationName,
    NoteTarget,
    CssClassReference,
    InlineClassReference,
    StyleTarget,
    ClassDef,
    ClickTarget,
    RelationTarget,
}

#[derive(Debug, Clone, Copy)]
#[cfg(feature = "editor-facts")]
struct PendingClassExpectation {
    kind: ExpectedClassName,
    anchor_end: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(feature = "editor-facts")]
enum ClassInteractionKind {
    Click,
    Link,
    CallbackStatement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(feature = "editor-facts")]
enum ClassLinePayloadKind {
    Style,
    ClassDef,
}

#[cfg(feature = "editor-facts")]
impl<'a> ClassEditorFactCollector<'a> {
    fn new(code: &'a str) -> Self {
        Self {
            code,
            expected_name: None,
            pending_relation_source: None,
            after_annotation_start: false,
            css_class_targets_pending: false,
            interaction: None,
            interaction_action_seen: false,
            interaction_action_end: None,
            callback_statement_function_seen: false,
            line_payload: None,
            line_payload_anchor_end: None,
            directive_start: None,
            interaction_target_end: None,
            note_text_pending: false,
            class_label_pending: false,
            declared_entities: HashSet::new(),
        }
    }

    fn accept(&mut self, token: &Tok, start: usize, end: usize, facts: &mut EditorSemanticFacts) {
        match token {
            Tok::Newline => {
                self.finish_line(line_content_end(self.code, start), facts);
                self.reset_line_state();
            }
            Tok::ClassDiagram | Tok::StructStop => self.reset_line_state(),
            Tok::ClassKw => {
                facts.push_directive_prefix("class");
                self.directive_start = Some(start);
                self.pending_relation_source = None;
                self.expect_name(ExpectedClassName::Class);
            }
            Tok::NamespaceKw => {
                self.pending_relation_source = None;
                self.expect_name(ExpectedClassName::Namespace);
            }
            Tok::Note => {
                self.note_text_pending = true;
            }
            Tok::NoteFor => {
                self.note_text_pending = true;
                self.expect_name(ExpectedClassName::NoteTarget);
            }
            Tok::CssClass => {
                facts.push_directive_prefix("cssClass");
                self.directive_start = Some(start);
                self.css_class_targets_pending = true;
            }
            Tok::StyleKw => {
                facts.push_directive_prefix("style");
                self.directive_start = Some(start);
                self.line_payload = Some(ClassLinePayloadKind::Style);
                self.expect_name(ExpectedClassName::StyleTarget);
            }
            Tok::ClassDefKw => {
                facts.push_directive_prefix("classDef");
                self.directive_start = Some(start);
                self.line_payload = Some(ClassLinePayloadKind::ClassDef);
                self.expect_name_after(ExpectedClassName::ClassDef, end);
            }
            Tok::ClickKw => {
                facts.push_directive_prefix("click");
                self.directive_start = Some(start);
                self.interaction = Some(ClassInteractionKind::Click);
                self.interaction_action_seen = false;
                self.callback_statement_function_seen = false;
                self.expect_name(ExpectedClassName::ClickTarget);
            }
            Tok::LinkKw => {
                facts.push_directive_prefix("link");
                self.directive_start = Some(start);
                self.interaction = Some(ClassInteractionKind::Link);
                self.interaction_action_seen = false;
                self.callback_statement_function_seen = false;
                self.expect_name(ExpectedClassName::ClickTarget);
            }
            Tok::CallbackKw => {
                facts.push_directive_prefix("callback");
                self.directive_start = Some(start);
                self.interaction = Some(ClassInteractionKind::CallbackStatement);
                self.interaction_action_seen = false;
                self.callback_statement_function_seen = false;
                self.expect_name(ExpectedClassName::ClickTarget);
            }
            Tok::AnnotationStart => {
                self.after_annotation_start = true;
                self.pending_relation_source = None;
            }
            Tok::AnnotationStop => self.expect_name(ExpectedClassName::AnnotationName),
            Tok::Line
            | Tok::DottedLine
            | Tok::Ext
            | Tok::Dep
            | Tok::Comp
            | Tok::Agg
            | Tok::Lollipop => {
                self.push_pending_relation_source(facts);
                self.expect_name(ExpectedClassName::RelationTarget);
            }
            Tok::Label(label) => {
                if let Some(symbol) = self.pending_relation_source.take() {
                    self.push_symbol(facts, symbol, ExpectedClassName::MemberOwner);
                    self.push_label_member_symbol(facts, label, SourceSpan::new(start, end));
                } else {
                    self.push_label_payload_symbol(
                        facts,
                        label,
                        SourceSpan::new(start, end),
                        "class relation label",
                    );
                }
            }
            Tok::Name(name) => {
                if self.interaction == Some(ClassInteractionKind::Click)
                    && self.expected_name.is_none()
                    && !self.interaction_action_seen
                    && self
                        .interaction_target_end
                        .is_some_and(|target_end| has_ascii_separator(self.code, target_end, start))
                {
                    if is_class_interaction_action_prefix(name) {
                        self.push_interaction_expected_syntax(facts, start, end);
                    } else {
                        self.push_interaction_payload_expected_syntax(facts, start, end);
                        self.push_payload_symbol(
                            facts,
                            ClassTokenSymbol {
                                name: name.clone(),
                                span: SourceSpan::new(start, end),
                            },
                            "class interaction payload",
                            EditorSemanticKind::Function,
                        );
                    }
                    return;
                }
                let symbol = ClassTokenSymbol {
                    name: name.clone(),
                    span: SourceSpan::new(start, end),
                };
                if self.after_annotation_start {
                    self.after_annotation_start = false;
                    self.push_payload_symbol(
                        facts,
                        symbol,
                        "class annotation",
                        EditorSemanticKind::String,
                    );
                    return;
                }

                if let Some(expected) = self.expected_name.take() {
                    let expected = expected.kind;
                    facts.push_expected_syntax(EditorExpectedSyntax::new(
                        class_expected_syntax_kind(expected),
                        SourceSpan::new(start, end),
                    ));
                    if matches!(
                        expected,
                        ExpectedClassName::StyleTarget | ExpectedClassName::ClassDef
                    ) {
                        self.line_payload_anchor_end = Some(end);
                    }
                    if matches!(expected, ExpectedClassName::ClickTarget) {
                        self.interaction_target_end = Some(end);
                    }
                    self.push_symbol(facts, symbol, expected);
                } else {
                    self.pending_relation_source = Some(symbol);
                }
            }
            Tok::Member(member) => {
                self.push_member_symbol(facts, member, SourceSpan::new(start, end));
            }
            Tok::Str(text) => {
                if self.css_class_targets_pending {
                    self.css_class_targets_pending = false;
                    self.push_class_target_list_symbols(
                        facts,
                        text,
                        SourceSpan::new(start, end),
                        "class css target",
                    );
                    self.expect_name_after(ExpectedClassName::CssClassReference, end);
                    return;
                }

                if self.class_label_pending {
                    self.class_label_pending = false;
                    self.push_string_payload_symbol(
                        facts,
                        text,
                        SourceSpan::new(start, end),
                        "class display label",
                        EditorSemanticKind::String,
                    );
                    return;
                }

                if self.note_text_pending {
                    self.note_text_pending = false;
                    self.push_string_payload_symbol(
                        facts,
                        text,
                        SourceSpan::new(start, end),
                        "class note",
                        EditorSemanticKind::String,
                    );
                    return;
                }

                if self.pending_relation_source.is_some()
                    || matches!(
                        self.expected_name,
                        Some(PendingClassExpectation {
                            kind: ExpectedClassName::RelationTarget,
                            ..
                        })
                    )
                {
                    self.push_string_payload_symbol(
                        facts,
                        text,
                        SourceSpan::new(start, end),
                        "class relation multiplicity",
                        EditorSemanticKind::String,
                    );
                    return;
                }

                if self.interaction == Some(ClassInteractionKind::CallbackStatement)
                    && !self.callback_statement_function_seen
                {
                    self.push_interaction_payload_expected_syntax(facts, start, end);
                    self.callback_statement_function_seen = true;
                    self.push_string_payload_symbol(
                        facts,
                        text,
                        SourceSpan::new(start, end),
                        "class callback",
                        EditorSemanticKind::Function,
                    );
                    return;
                }

                if self.interaction.is_some() {
                    self.push_interaction_payload_expected_syntax(facts, start, end);
                    self.push_string_payload_symbol(
                        facts,
                        text,
                        SourceSpan::new(start, end),
                        "class interaction string",
                        EditorSemanticKind::String,
                    );
                }
            }
            Tok::LinkTarget(target) => {
                if self.interaction.is_some() {
                    self.push_interaction_payload_expected_syntax(facts, start, end);
                    self.push_payload_symbol(
                        facts,
                        ClassTokenSymbol {
                            name: target.clone(),
                            span: SourceSpan::new(start, end),
                        },
                        "class link target",
                        EditorSemanticKind::String,
                    );
                }
            }
            Tok::CallbackName(function) => {
                if self.interaction.is_some() {
                    self.push_interaction_payload_expected_syntax(facts, start, end);
                    self.push_payload_symbol(
                        facts,
                        ClassTokenSymbol {
                            name: function.clone(),
                            span: SourceSpan::new(start, end),
                        },
                        "class callback",
                        EditorSemanticKind::Function,
                    );
                }
            }
            Tok::CallbackArgs(args) => {
                if self.interaction.is_some() {
                    self.push_interaction_payload_expected_syntax(facts, start, end);
                    self.push_payload_symbol_from_token(
                        facts,
                        args,
                        SourceSpan::new(start, end),
                        "class callback args",
                        EditorSemanticKind::String,
                    );
                }
            }
            Tok::RestOfLine(raw) => {
                if let Some(kind) = self.line_payload.take() {
                    let line_end = line_content_end(self.code, end);
                    let trailing_slot = trailing_ascii_whitespace_slot(self.code, start, end)
                        .or_else(|| {
                            (raw.is_empty()
                                && self.line_payload_anchor_end.is_some_and(|anchor_end| {
                                    has_ascii_separator(self.code, anchor_end, line_end)
                                }))
                            .then_some(SourceSpan::new(line_end, line_end))
                        });
                    if let Some(span) = trailing_slot {
                        facts.push_expected_syntax(EditorExpectedSyntax::new(
                            EditorExpectedSyntaxKind::StyleValue,
                            span,
                        ));
                    }
                    let detail = match kind {
                        ClassLinePayloadKind::Style => "class style",
                        ClassLinePayloadKind::ClassDef => "class definition style",
                    };
                    let payload = raw.trim();
                    if !payload.is_empty()
                        && let Some(selection) =
                            source_value_span(self.code, SourceSpan::new(start, end), payload)
                    {
                        facts.push_expected_syntax(EditorExpectedSyntax::new(
                            EditorExpectedSyntaxKind::Payload,
                            selection,
                        ));
                    }
                    self.push_payload_symbol_from_token(
                        facts,
                        raw,
                        SourceSpan::new(start, end),
                        detail,
                        EditorSemanticKind::String,
                    );
                }
            }
            Tok::AccTitle(value) => {
                facts.push_directive_prefix("accTitle");
                self.push_payload_symbol_from_token(
                    facts,
                    value,
                    SourceSpan::new(start, end),
                    "class accessibility title",
                    EditorSemanticKind::String,
                );
            }
            Tok::AccDescr(value) | Tok::AccDescrMultiline(value) => {
                facts.push_directive_prefix("accDescr");
                self.push_payload_symbol_from_token(
                    facts,
                    value,
                    SourceSpan::new(start, end),
                    "class accessibility description",
                    EditorSemanticKind::String,
                );
            }
            Tok::Direction(_) => {
                if let Some(span) = keyword_value_span(self.code, start, end, "direction") {
                    facts.push_expected_syntax(EditorExpectedSyntax::new(
                        EditorExpectedSyntaxKind::CardinalDirectionValue,
                        span,
                    ));
                }
            }
            Tok::HrefKw => {
                if self.interaction == Some(ClassInteractionKind::Click) {
                    self.push_interaction_expected_syntax(facts, start, end);
                }
            }
            Tok::CallKw => {
                if self.interaction == Some(ClassInteractionKind::Click) {
                    self.push_interaction_expected_syntax(facts, start, end);
                }
            }
            Tok::StructStart => {}
            Tok::SquareStart => {
                self.class_label_pending = true;
            }
            Tok::SquareStop => {
                self.class_label_pending = false;
            }
            Tok::StyleSeparator => {
                self.directive_start = Some(start);
                self.expect_name(ExpectedClassName::InlineClassReference);
            }
        }
    }

    fn finish(&mut self, source_len: usize, facts: &mut EditorSemanticFacts) {
        self.finish_line(line_content_end(self.code, source_len), facts);
    }

    fn finish_line(&mut self, line_end: usize, facts: &mut EditorSemanticFacts) {
        if let Some(start) = self.directive_start {
            facts.push_expected_syntax(EditorExpectedSyntax::new(
                EditorExpectedSyntaxKind::Directive,
                SourceSpan::new(start, line_end),
            ));
        }

        if let Some(expected) = self.expected_name {
            let kind = class_expected_syntax_kind(expected.kind);
            let separator_ready = expected
                .anchor_end
                .is_none_or(|anchor_end| has_ascii_separator(self.code, anchor_end, line_end));
            if !matches!(expected.kind, ExpectedClassName::ClassDef)
                && (!matches!(kind, EditorExpectedSyntaxKind::ClassName) || separator_ready)
            {
                facts.push_expected_syntax(EditorExpectedSyntax::new(
                    kind,
                    SourceSpan::new(line_end, line_end),
                ));
            }
        } else if self.line_payload.is_some()
            && self
                .line_payload_anchor_end
                .is_some_and(|anchor_end| has_ascii_separator(self.code, anchor_end, line_end))
        {
            facts.push_expected_syntax(EditorExpectedSyntax::new(
                EditorExpectedSyntaxKind::StyleValue,
                SourceSpan::new(line_end, line_end),
            ));
        }

        let expects_interaction_action = self.interaction == Some(ClassInteractionKind::Click)
            && self.expected_name.is_none()
            && !self.interaction_action_seen
            && self
                .interaction_target_end
                .is_some_and(|target_end| has_ascii_separator(self.code, target_end, line_end));
        if expects_interaction_action {
            facts.push_expected_syntax(EditorExpectedSyntax::new(
                EditorExpectedSyntaxKind::InteractionAction,
                SourceSpan::new(line_end, line_end),
            ));
            return;
        }

        let click_payload_ready = self.interaction == Some(ClassInteractionKind::Click)
            && self.expected_name.is_none()
            && self
                .interaction_action_end
                .is_some_and(|action_end| has_ascii_separator(self.code, action_end, line_end));
        let statement_payload_ready = matches!(
            self.interaction,
            Some(ClassInteractionKind::Link | ClassInteractionKind::CallbackStatement)
        ) && self.expected_name.is_none()
            && self
                .interaction_target_end
                .is_some_and(|target_end| has_ascii_separator(self.code, target_end, line_end));
        if click_payload_ready || statement_payload_ready {
            facts.push_expected_syntax(EditorExpectedSyntax::new(
                EditorExpectedSyntaxKind::Payload,
                SourceSpan::new(line_end, line_end),
            ));
        }
    }

    fn push_interaction_expected_syntax(
        &mut self,
        facts: &mut EditorSemanticFacts,
        start: usize,
        end: usize,
    ) {
        self.interaction_action_seen = true;
        self.interaction_action_end = Some(end);
        facts.push_expected_syntax(EditorExpectedSyntax::new(
            EditorExpectedSyntaxKind::InteractionAction,
            SourceSpan::new(start, end),
        ));
    }

    fn push_interaction_payload_expected_syntax(
        &mut self,
        facts: &mut EditorSemanticFacts,
        start: usize,
        end: usize,
    ) {
        if self.interaction == Some(ClassInteractionKind::Click) {
            self.interaction_action_seen = true;
        }
        facts.push_expected_syntax(EditorExpectedSyntax::new(
            EditorExpectedSyntaxKind::Payload,
            SourceSpan::new(start, end),
        ));
    }

    fn reset_line_state(&mut self) {
        self.expected_name = None;
        self.pending_relation_source = None;
        self.after_annotation_start = false;
        self.css_class_targets_pending = false;
        self.interaction = None;
        self.interaction_action_seen = false;
        self.interaction_action_end = None;
        self.callback_statement_function_seen = false;
        self.line_payload = None;
        self.line_payload_anchor_end = None;
        self.directive_start = None;
        self.interaction_target_end = None;
        self.note_text_pending = false;
        self.class_label_pending = false;
    }

    fn expect_name(&mut self, expected: ExpectedClassName) {
        self.expected_name = Some(PendingClassExpectation {
            kind: expected,
            anchor_end: None,
        });
    }

    fn expect_name_after(&mut self, expected: ExpectedClassName, anchor_end: usize) {
        self.expected_name = Some(PendingClassExpectation {
            kind: expected,
            anchor_end: Some(anchor_end),
        });
    }

    fn push_pending_relation_source(&mut self, facts: &mut EditorSemanticFacts) {
        if let Some(symbol) = self.pending_relation_source.take() {
            self.push_symbol(facts, symbol, ExpectedClassName::RelationTarget);
        }
    }

    fn push_symbol(
        &mut self,
        facts: &mut EditorSemanticFacts,
        symbol: ClassTokenSymbol,
        expected: ExpectedClassName,
    ) {
        if symbol.name.is_empty() {
            return;
        }

        let detail = match expected {
            ExpectedClassName::Class => "class",
            ExpectedClassName::Namespace => "namespace",
            ExpectedClassName::MemberOwner => "class member owner",
            ExpectedClassName::AnnotationName => "class annotation target",
            ExpectedClassName::NoteTarget => "class note target",
            ExpectedClassName::CssClassReference => "class css reference",
            ExpectedClassName::InlineClassReference => "class inline class",
            ExpectedClassName::StyleTarget => "class style target",
            ExpectedClassName::ClassDef => "class definition",
            ExpectedClassName::ClickTarget => "class interaction target",
            ExpectedClassName::RelationTarget => "class relation target",
        };
        let kind = match expected {
            ExpectedClassName::Namespace => EditorSemanticKind::Namespace,
            ExpectedClassName::ClassDef
            | ExpectedClassName::CssClassReference
            | ExpectedClassName::InlineClassReference => EditorSemanticKind::Property,
            _ => EditorSemanticKind::Class,
        };
        let selection = selection_span_for_class_name(self.code, &symbol.name, symbol.span);
        match expected {
            ExpectedClassName::Class => {
                self.declared_entities.insert(symbol.name.clone());
                facts.push_symbol(EditorSemanticSymbol::new(
                    symbol.name,
                    Some(detail.to_string()),
                    kind,
                    symbol.span,
                    selection,
                ));
            }
            ExpectedClassName::Namespace => {
                facts.push_symbol(EditorSemanticSymbol::new(
                    symbol.name,
                    Some(detail.to_string()),
                    kind,
                    symbol.span,
                    selection,
                ));
            }
            ExpectedClassName::ClassDef => {
                facts.push_symbol(EditorSemanticSymbol::class_definition(
                    symbol.name,
                    Some(detail.to_string()),
                    kind,
                    symbol.span,
                    selection,
                ))
            }
            ExpectedClassName::CssClassReference | ExpectedClassName::InlineClassReference => {
                facts.push_symbol(EditorSemanticSymbol::payload(
                    symbol.name,
                    Some(detail.to_string()),
                    kind,
                    symbol.span,
                    selection,
                ));
            }
            ExpectedClassName::MemberOwner
            | ExpectedClassName::AnnotationName
            | ExpectedClassName::RelationTarget => {
                let role = if self.declared_entities.insert(symbol.name.clone()) {
                    crate::EditorSemanticRole::Entity
                } else {
                    crate::EditorSemanticRole::Reference
                };
                facts.push_symbol(EditorSemanticSymbol::with_role(
                    symbol.name,
                    Some(detail.to_string()),
                    kind,
                    role,
                    symbol.span,
                    selection,
                ));
            }
            ExpectedClassName::NoteTarget
            | ExpectedClassName::StyleTarget
            | ExpectedClassName::ClickTarget => {
                facts.push_symbol(EditorSemanticSymbol::reference(
                    symbol.name,
                    Some(detail.to_string()),
                    kind,
                    symbol.span,
                    selection,
                ));
            }
        }
    }

    fn push_payload_symbol(
        &self,
        facts: &mut EditorSemanticFacts,
        symbol: ClassTokenSymbol,
        detail: &'static str,
        kind: EditorSemanticKind,
    ) {
        if symbol.name.is_empty() {
            return;
        }
        facts.push_symbol(EditorSemanticSymbol::payload(
            symbol.name,
            Some(detail.to_string()),
            kind,
            symbol.span,
            symbol.span,
        ));
    }

    fn push_payload_symbol_from_token(
        &self,
        facts: &mut EditorSemanticFacts,
        value: &str,
        span: SourceSpan,
        detail: &'static str,
        kind: EditorSemanticKind,
    ) {
        let value = value.trim();
        if value.is_empty() {
            return;
        }

        let selection = source_value_span(self.code, span, value).unwrap_or(span);
        facts.push_symbol(EditorSemanticSymbol::payload(
            value,
            Some(detail.to_string()),
            kind,
            span,
            selection,
        ));
    }

    fn push_class_target_list_symbols(
        &self,
        facts: &mut EditorSemanticFacts,
        value: &str,
        span: SourceSpan,
        detail: &'static str,
    ) {
        let body_start = if span.end >= span.start + value.len() + 2 {
            span.start + 1
        } else {
            span.start
        };
        let mut cursor = 0usize;
        while cursor <= value.len() {
            let next_comma = value[cursor..]
                .find(',')
                .map(|offset| cursor + offset)
                .unwrap_or(value.len());
            let part = &value[cursor..next_comma];
            let leading = part.len().saturating_sub(part.trim_start().len());
            let trailing = part.trim_end().len();
            if leading < trailing {
                let id_start = cursor + leading;
                let id_end = cursor + trailing;
                let id = &value[id_start..id_end];
                let selection = SourceSpan::new(body_start + id_start, body_start + id_end);
                facts.push_symbol(EditorSemanticSymbol::reference(
                    id,
                    Some(detail.to_string()),
                    EditorSemanticKind::Class,
                    selection,
                    selection,
                ));
            }

            if next_comma == value.len() {
                break;
            }
            cursor = next_comma + 1;
        }
    }

    fn push_member_symbol(&self, facts: &mut EditorSemanticFacts, member: &str, span: SourceSpan) {
        let Some((name, selection)) = class_member_selection(member, span) else {
            return;
        };
        facts.push_symbol(EditorSemanticSymbol::outline(
            name,
            Some("class member".to_string()),
            EditorSemanticKind::Property,
            span,
            selection,
        ));
    }

    fn push_string_payload_symbol(
        &self,
        facts: &mut EditorSemanticFacts,
        text: &str,
        span: SourceSpan,
        detail: &'static str,
        kind: EditorSemanticKind,
    ) {
        if text.is_empty() {
            return;
        }
        let selection = if span.end >= span.start + text.len() + 2 {
            SourceSpan::new(span.start + 1, span.start + 1 + text.len())
        } else {
            span
        };
        facts.push_symbol(EditorSemanticSymbol::payload(
            text,
            Some(detail.to_string()),
            kind,
            span,
            selection,
        ));
    }

    fn push_label_payload_symbol(
        &self,
        facts: &mut EditorSemanticFacts,
        label: &str,
        span: SourceSpan,
        detail: &'static str,
    ) {
        let Some((name, selection)) = class_label_member_selection(label, span) else {
            return;
        };
        facts.push_symbol(EditorSemanticSymbol::payload(
            name,
            Some(detail.to_string()),
            EditorSemanticKind::String,
            span,
            selection,
        ));
    }

    fn push_label_member_symbol(
        &self,
        facts: &mut EditorSemanticFacts,
        label: &str,
        span: SourceSpan,
    ) {
        let Some((name, selection)) = class_label_member_selection(label, span) else {
            return;
        };
        facts.push_symbol(EditorSemanticSymbol::outline(
            name,
            Some("class member".to_string()),
            EditorSemanticKind::Property,
            span,
            selection,
        ));
    }
}

#[cfg(feature = "editor-facts")]
fn class_expected_syntax_kind(expected: ExpectedClassName) -> EditorExpectedSyntaxKind {
    match expected {
        ExpectedClassName::ClassDef
        | ExpectedClassName::CssClassReference
        | ExpectedClassName::InlineClassReference => EditorExpectedSyntaxKind::ClassName,
        ExpectedClassName::Class
        | ExpectedClassName::Namespace
        | ExpectedClassName::MemberOwner
        | ExpectedClassName::AnnotationName
        | ExpectedClassName::NoteTarget
        | ExpectedClassName::StyleTarget
        | ExpectedClassName::ClickTarget
        | ExpectedClassName::RelationTarget => EditorExpectedSyntaxKind::NodeIdentifier,
    }
}

#[cfg(feature = "editor-facts")]
fn is_class_interaction_action_prefix(value: &str) -> bool {
    !value.is_empty()
        && ["href", "call"]
            .iter()
            .any(|action| action.starts_with(value))
}

#[cfg(feature = "editor-facts")]
fn selection_span_for_class_name(code: &str, name: &str, span: SourceSpan) -> SourceSpan {
    if let Some(raw) = name.strip_prefix(MERMAID_DOM_ID_PREFIX) {
        if let Some(source) = code.get(span.start..span.end)
            && source.starts_with('`')
            && source.ends_with('`')
            && span.end >= span.start + raw.len() + 2
        {
            return SourceSpan::new(span.start + 1, span.start + 1 + raw.len());
        }
        return SourceSpan::new(span.start, span.start + raw.len());
    }

    if span.end > span.start + 1 {
        return SourceSpan::new(span.start, span.end);
    }

    span
}

#[cfg(feature = "editor-facts")]
fn class_member_selection(member: &str, span: SourceSpan) -> Option<(String, SourceSpan)> {
    let trimmed_start = member.len().saturating_sub(member.trim_start().len());
    let text = &member[trimmed_start..];
    let trimmed_len = text.trim_end().len();
    if trimmed_len == 0 {
        return None;
    }

    let text = &text[..trimmed_len];
    Some((
        text.to_string(),
        SourceSpan::new(
            span.start + trimmed_start,
            span.start + trimmed_start + text.len(),
        ),
    ))
}

#[cfg(feature = "editor-facts")]
fn class_label_member_selection(label: &str, span: SourceSpan) -> Option<(String, SourceSpan)> {
    let after_colon = label.strip_prefix(':').unwrap_or(label);
    let colon_offset = usize::from(label.starts_with(':'));
    let leading = after_colon
        .len()
        .saturating_sub(after_colon.trim_start().len());
    let text = &after_colon[leading..];
    let trimmed_len = text.trim_end().len();
    if trimmed_len == 0 {
        return None;
    }

    let text = &text[..trimmed_len];
    let start = span.start + colon_offset + leading;
    Some((text.to_string(), SourceSpan::new(start, start + text.len())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write as _;

    fn assert_carrier_work(source: &str, expected: (usize, usize, usize, usize)) {
        let control = OperationControl::new();
        let mut arena = ActionArena::default();
        let root = class_grammar::ActionsParser::new()
            .parse(&mut arena, &control, Lexer::new(source))
            .expect("Class grammar accepts carrier workload");
        assert_eq!(arena.carrier_counts(), expected);
        let materialized = arena.into_actions(root).count();
        assert_eq!(materialized, expected.0);
        println!(
            "source_bytes={} action_records={} action_links={} class_id_records={} \
             class_id_links={} replayed_events={materialized}",
            source.len(),
            expected.0,
            expected.1,
            expected.2,
            expected.3,
        );
    }

    #[test]
    fn grammar_carriers_attach_each_event_once_at_increasing_width_and_depth() {
        for size in [8, 128, 1_024] {
            let mut flat = String::from("classDiagram\n");
            for index in 0..size {
                writeln!(flat, "class C{index}").unwrap();
            }
            assert_carrier_work(&flat, (size, size - 1, 0, 0));

            let mut namespace = String::from("classDiagram\nnamespace N {\n");
            for index in 0..size {
                writeln!(namespace, "class C{index}").unwrap();
            }
            namespace.push_str("}\n");
            assert_carrier_work(&namespace, (size + 3, size + 2, size, size - 1));

            let mut nested = String::from("classDiagram\n");
            for index in 0..size {
                writeln!(nested, "namespace N{index} {{").unwrap();
            }
            nested.push_str("class Leaf\n");
            nested.push_str(&"}\n".repeat(size));
            assert_carrier_work(&nested, (size * 3 + 1, size * 3, 1, 0));
        }
    }

    #[test]
    fn grammar_carrier_reductions_observe_cancellation() {
        let source = format!("classDiagram\n{}", "class C\n".repeat(1_024));
        let control = OperationControl::new();
        control.cancel_after_checkpoints(1);
        let mut arena = ActionArena::default();
        let error = class_grammar::ActionsParser::new()
            .parse(&mut arena, &control, Lexer::new(&source))
            .err()
            .expect("construction checkpoint interrupts reductions");
        assert!(matches!(error, lalrpop_util::ParseError::User { .. }));
        assert!(arena.carrier_counts().0 < 1_024);
        assert_eq!(
            control.checkpoint().unwrap_err().reason,
            crate::CancelReason::Requested
        );
    }

    #[test]
    fn cancelled_token_delivery_returns_terminal_before_grammar_eof() {
        let source = format!("classDiagram\n{}", "class C\n".repeat(256));
        let control = OperationControl::new();
        let syntax = ClassSyntax::lex(&source, &control).unwrap();
        #[cfg(feature = "editor-facts")]
        let fact_checkpoints = syntax.events.len().div_ceil(128);
        #[cfg(not(feature = "editor-facts"))]
        let fact_checkpoints = 0;
        control.cancel_after_checkpoints(fact_checkpoints + 2);
        let error = syntax
            .into_actions(
                #[cfg(feature = "editor-facts")]
                &source,
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Capture,
                &control,
            )
            .err()
            .expect("cancelled delivery returns the outer terminal");
        assert_eq!(error.reason, crate::CancelReason::Requested);
    }
}

#[cfg(test)]
mod semantic_tests {
    use super::*;

    fn metadata() -> crate::SemanticMetadata {
        crate::SemanticMetadata {
            diagram_type: "classDiagram".to_string(),
            config: MermaidConfig::empty_object(),
            semantic_config: MermaidConfig::empty_object(),
            title: None,
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn canonical_class_captures_success_and_failure_from_one_construction() {
        for (source, valid) in [
            ("classDiagram\nclass Service\n", true),
            ("classDiagram\nclass Service {\n+field\n", false),
        ] {
            reset_class_syntax_construction_count();
            #[cfg(feature = "editor-facts")]
            crate::editor::test_work::reset();
            let parsed = parse_class_semantic_model_controlled(
                source,
                &metadata(),
                EditorFactsDemand::Capture,
                &OperationControl::new(),
            )
            .unwrap();
            assert_eq!(parsed.model.is_ok(), valid);
            let facts = parsed.editor_facts.expect("same-pass Class facts");
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "Service"));
            assert_eq!(class_syntax_construction_count(), 1);
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
    fn canonical_class_keeps_namespace_meaning_and_style_precedence_without_editor_work() {
        let source = r#"classDiagram
namespace A.B {
  class Service
  note "namespace note"
}
class Client
cssClass "Client" accent
classDef accent fill:red
Client --> Service
"#;
        let mut metadata = metadata();
        metadata
            .semantic_config
            .set_value("class.hierarchicalNamespaces", false.into());
        reset_class_syntax_construction_count();
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let model = parse_class_semantic_model_controlled(
            source,
            &metadata,
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();

        assert_eq!(
            model
                .namespaces
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["A", "A.B"]
        );
        assert!(!model.namespaces["A"].explicit);
        assert_eq!(model.namespaces["A.B"].label, "B");
        assert_eq!(model.namespaces["A.B"].parent.as_deref(), Some("A"));
        assert_eq!(model.classes["Service"].parent.as_deref(), Some("A.B"));
        assert_eq!(model.notes[0].parent.as_deref(), Some("A.B"));
        assert_eq!(model.namespaces["A.B"].class_ids, ["Service"]);
        assert_eq!(model.namespaces["A.B"].note_ids, ["note0"]);
        assert!(
            model
                .style_precedence_facts
                .assignment_before_definition_copy()
        );
        assert_eq!(model.classes["Client"].styles, ["fill:red"]);
        assert_eq!(class_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());

        let json = serde_json::to_value(&model).unwrap();
        assert!(json["classes"]["Service"].get("domId").is_none());
        assert!(json["namespaces"]["A.B"].get("domId").is_none());
        let round_trip: class_typed::ClassDiagramModel = serde_json::from_value(json).unwrap();
        assert_eq!(round_trip, model);
    }

    #[test]
    fn canonical_class_uses_admitted_security_config() {
        let source = "classDiagram\nclass Service\ncallback Service \"run\"\n";
        let mut metadata = metadata();
        metadata
            .semantic_config
            .set_value("securityLevel", "strict".into());
        let strict = parse_class_semantic_model_controlled(
            source,
            &metadata,
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();
        assert!(strict.classes["Service"].have_callback);
        assert!(!strict.classes["Service"].callback_effective);
        metadata
            .semantic_config
            .set_value("securityLevel", "loose".into());
        let loose = parse_class_semantic_model_controlled(
            source,
            &metadata,
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();
        assert!(loose.classes["Service"].callback_effective);
        assert_eq!(
            loose.classes["Service"].callback.as_ref().unwrap()["function"],
            "run"
        );
    }

    #[test]
    fn canonical_class_errors_preserve_exact_spans_without_editor_collection() {
        let source = "classDiagram\nclass Service {\n+field\n";
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let error = parse_class_semantic_model_controlled(
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
            panic!("expected Class parse diagnostic")
        };
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(source.len(), source.len()))
        );
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        #[cfg(feature = "compat-json")]
        {
            let Error::DiagramParse {
                diagnostic: full, ..
            } = parse_class(source, &full_metadata()).unwrap_err()
            else {
                panic!("expected full Class parse diagnostic")
            };
            assert_eq!(full, diagnostic);
        }
    }

    #[test]
    fn canonical_class_construction_observes_cancellation() {
        let control = OperationControl::new();
        control.cancel();
        assert!(
            parse_class_semantic_model_controlled(
                "classDiagram\nclass Service\n",
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &control,
            )
            .is_err()
        );
    }

    #[cfg(feature = "presentation")]
    fn full_metadata() -> ParseMetadata {
        ParseMetadata {
            diagram_type: "classDiagram".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: crate::presentation::PresentationConfig::empty_object(),
            title: None,
        }
    }

    #[cfg(feature = "presentation")]
    #[test]
    fn requested_class_presentation_keeps_canonical_namespaces_and_synthetic_ids() {
        let source = r#"classDiagram
namespace A.B {
  class Service
  note "namespace note"
}
class note0
class interface0
IService ()-- Service
"#;
        let model = parse_class_semantic_model_controlled(
            source,
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();
        let mut config = MermaidConfig::empty_object();
        config.set_value("class.hierarchicalNamespaces", false.into());
        let projection = class_typed::ClassPresentation::new(&model, &config);
        assert!(!projection.namespaces.contains_key("A"));
        assert_eq!(projection.namespaces["A.B"].label, "A.B");
        assert_eq!(projection.namespaces["A.B"].parent, None);
        assert_eq!(projection.namespace_dom_id("A.B"), Some("classId-A.B-1"));
        assert_eq!(
            projection.class_dom_id("Service"),
            Some("classId-Service-0")
        );
        assert_eq!(projection.note_ids, ["note0$1"]);
        assert_eq!(projection.namespaces["A.B"].note_ids, ["note0$1"]);
        assert_eq!(projection.interface_ids, ["interface0$1"]);
        assert_eq!(
            projection.relation_endpoints(&model.relations[0]),
            ("interface0$1", "Service")
        );
        assert_eq!(model.notes[0].id, "note0");
        assert_eq!(model.interfaces[0].id, "interface0");
        assert_eq!(model.relations[0].id1, "interface0");
        assert!(model.namespaces.contains_key("A"));
        assert_eq!(model.namespaces["A.B"].parent.as_deref(), Some("A"));
    }

    #[cfg(feature = "presentation")]
    #[test]
    fn flat_class_presentation_finishes_for_implicit_namespace_cycles() {
        let mut model = parse_class_semantic_model_controlled(
            "classDiagram\nnamespace A.B {\nclass Service\n}\n",
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();
        for namespace in model.namespaces.values_mut() {
            namespace.explicit = false;
        }
        model.namespaces.get_mut("A").unwrap().parent = Some("A.B".into());
        let mut config = MermaidConfig::empty_object();
        config.set_value("class.hierarchicalNamespaces", false.into());
        let (sender, receiver) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            sender
                .send(class_typed::ClassDiagram::new(model, &config))
                .unwrap();
        });
        let projected = receiver
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("flattening an implicit namespace cycle must finish");
        worker.join().unwrap();
        assert!(projected.namespaces().next().is_none());
        assert_eq!(projected.class("Service").unwrap().parent, None);
    }

    #[cfg(feature = "presentation")]
    #[test]
    fn full_class_typed_projection_constructs_once_without_editor_work() {
        reset_class_syntax_construction_count();
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let (model, _) = parse_class_typed_with_render_context_controlled(
            "classDiagram\nclass Service\n",
            &full_metadata(),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        assert!(model.class("Service").is_some());
        assert_eq!(class_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
    }

    #[cfg(feature = "presentation")]
    #[test]
    fn full_class_serde_preserves_projected_fields_without_claiming_authored_recovery() {
        let source = r#"classDiagram
namespace A.B {
  class Service
  note "namespace note"
}
class note0
class interface0
cssClass "Service" accent
classDef accent fill:red
IService ()-- Service
"#;
        let mut metadata = full_metadata();
        metadata
            .effective_config
            .set_value("class.hierarchicalNamespaces", false.into());
        let (model, _) = parse_class_typed_with_render_context_controlled(
            source,
            &metadata,
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        let wire = serde_json::to_value(&model).unwrap();
        assert_eq!(wire["classes"]["Service"]["domId"], "classId-Service-0");
        assert!(wire["namespaces"].get("A").is_none());
        assert_eq!(wire["namespaces"]["A.B"]["domId"], "classId-A.B-1");
        assert_eq!(wire["namespaces"]["A.B"]["label"], "A.B");
        assert_eq!(wire["notes"][0]["id"], "note0$1");
        assert_eq!(wire["namespaces"]["A.B"]["noteIds"][0], "note0$1");
        assert_eq!(wire["interfaces"][0]["id"], "interface0$1");
        assert_eq!(wire["relations"][0]["id1"], "interface0$1");
        assert!(wire["relations"][0]["relationTitle1"].is_null());
        assert!(wire.get("stylePrecedenceFacts").is_none());
        let decoded: class_typed::ClassDiagram = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(&decoded).unwrap(), wire);
        assert_eq!(decoded, model);
        assert!(model.model().namespaces.contains_key("A"));
        assert!(!decoded.model().namespaces.contains_key("A"));
        assert!(
            model
                .model()
                .style_precedence_facts
                .assignment_before_definition_copy()
        );
        assert_eq!(decoded.model().style_precedence_facts, Default::default());
    }

    #[cfg(feature = "compat-json")]
    #[test]
    fn class_compatibility_projection_keeps_full_shape_and_omits_editor_work() {
        let source = r#"classDiagram
namespace A.B {
  class Service
}
class Client
Client --> Service
"#;
        let mut metadata = full_metadata();
        metadata
            .effective_config
            .set_value("class.hierarchicalNamespaces", false.into());
        reset_class_syntax_construction_count();
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let json = parse_class(source, &metadata).unwrap();
        assert_eq!(json["classes"]["Service"]["domId"], "classId-Service-0");
        assert!(json["namespaces"].get("A").is_none());
        assert_eq!(json["namespaces"]["A.B"]["label"], "A.B");
        assert_eq!(json["namespaces"]["A.B"]["domId"], "classId-A.B-1");
        assert!(json["namespaces"]["A.B"]["parent"].is_null());
        assert_eq!(json["relations"][0]["relationTitle1"], "none");
        assert!(json.get("stylePrecedenceFacts").is_none());
        assert!(json.get("namespaceFacadeAliases").is_none());
        assert_eq!(class_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
    }

    #[cfg(feature = "compat-json")]
    #[test]
    #[cfg(feature = "editor-facts")]
    fn combined_class_parse_keeps_editor_capture_and_one_construction() {
        reset_class_syntax_construction_count();
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let _ = parse_class_json_and_editor_facts(
            "classDiagram\nclass Service\n",
            &full_metadata(),
            &OperationControl::new(),
        )
        .unwrap();
        assert_eq!(class_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_ne!(crate::editor::test_work::read(), Default::default());
    }
}
