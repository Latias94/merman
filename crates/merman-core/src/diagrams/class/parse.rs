use crate::models::class_diagram as class_typed;
use crate::{
    EditorSemanticFacts, EditorSemanticKind, EditorSemanticSymbol, Error, ParseMetadata, Result,
    SourceSpan,
};
use serde_json::Value;

use super::class_grammar;
use super::db::ClassDb;
use super::fast::parse_class_fast_db;
use super::lexer::Lexer;
use super::{MERMAID_DOM_ID_PREFIX, Tok};

fn prefer_fast_class_parser() -> bool {
    match std::env::var("MERMAN_CLASS_PARSER").as_deref() {
        Ok("slow") | Ok("0") | Ok("false") => false,
        Ok("fast") | Ok("1") | Ok("true") => true,
        // Default to "auto": attempt the fast parser and fall back to LALRPOP when it declines.
        _ => true,
    }
}

pub(super) fn parse_class_via_lalrpop_db<'a>(
    code: &str,
    meta: &'a ParseMetadata,
) -> Result<ClassDb<'a>> {
    let actions = class_grammar::ActionsParser::new()
        .parse(Lexer::new(code))
        .map_err(|e| Error::DiagramParse {
            diagram_type: meta.diagram_type.clone(),
            message: format!("{e:?}"),
        })?;

    let mut db = ClassDb::new(&meta.effective_config);
    for a in actions {
        db.apply(a).map_err(|e| Error::DiagramParse {
            diagram_type: meta.diagram_type.clone(),
            message: e,
        })?;
    }
    Ok(db)
}

pub(super) fn parse_class_via_lalrpop(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let db = parse_class_via_lalrpop_db(code, meta)?;
    Ok(db.into_model(meta))
}

pub fn parse_class(code: &str, meta: &ParseMetadata) -> Result<Value> {
    if prefer_fast_class_parser()
        && let Some(db) = parse_class_fast_db(code, meta)?
    {
        return Ok(db.into_model(meta));
    }

    parse_class_via_lalrpop(code, meta)
}

pub fn parse_class_typed(code: &str, meta: &ParseMetadata) -> Result<class_typed::ClassDiagram> {
    if prefer_fast_class_parser()
        && let Some(db) = parse_class_fast_db(code, meta)?
    {
        return Ok(db.into_typed_model(meta));
    }

    let db = parse_class_via_lalrpop_db(code, meta)?;
    Ok(db.into_typed_model(meta))
}

pub fn parse_class_editor_facts(code: &str, _meta: &ParseMetadata) -> EditorSemanticFacts {
    let complete = class_grammar::ActionsParser::new()
        .parse(Lexer::new(code))
        .is_ok();
    let mut facts = collect_class_editor_facts_from_tokens(code);
    if !complete {
        facts.mark_recovered();
    }

    facts
}

fn collect_class_editor_facts_from_tokens(code: &str) -> EditorSemanticFacts {
    let mut facts = EditorSemanticFacts::new();
    let mut collector = ClassEditorFactCollector::default();

    let mut lexer = Lexer::new(code);
    while let Some(result) = lexer.next() {
        match result {
            Ok((start, token, end)) => collector.accept(token, start, end, &mut facts),
            Err(_) => facts.mark_recovered(),
        }
    }

    facts
}

#[derive(Debug, Default)]
struct ClassEditorFactCollector {
    expected_name: Option<ExpectedClassName>,
    pending_relation_source: Option<ClassTokenSymbol>,
    after_annotation_start: bool,
}

#[derive(Debug, Clone)]
struct ClassTokenSymbol {
    name: String,
    span: SourceSpan,
}

#[derive(Debug, Clone, Copy)]
enum ExpectedClassName {
    Class,
    Namespace,
    MemberOwner,
    AnnotationName,
    NoteTarget,
    CssClassTarget,
    StyleTarget,
    ClassDef,
    ClickTarget,
    RelationTarget,
}

impl ClassEditorFactCollector {
    fn accept(&mut self, token: Tok, start: usize, end: usize, facts: &mut EditorSemanticFacts) {
        match token {
            Tok::Newline | Tok::ClassDiagram | Tok::StructStop => self.reset_line_state(),
            Tok::ClassKw => {
                self.pending_relation_source = None;
                self.expect_name(ExpectedClassName::Class);
            }
            Tok::NamespaceKw => {
                self.pending_relation_source = None;
                self.expect_name(ExpectedClassName::Namespace);
            }
            Tok::NoteFor => self.expect_name(ExpectedClassName::NoteTarget),
            Tok::CssClass => {
                facts.push_directive_prefix("cssClass");
                self.expect_name(ExpectedClassName::CssClassTarget);
            }
            Tok::StyleKw => {
                facts.push_directive_prefix("style");
                self.expect_name(ExpectedClassName::StyleTarget);
            }
            Tok::ClassDefKw => {
                facts.push_directive_prefix("classDef");
                self.expect_name(ExpectedClassName::ClassDef);
            }
            Tok::ClickKw | Tok::LinkKw | Tok::CallbackKw => {
                facts.push_directive_prefix(match token {
                    Tok::LinkKw => "link",
                    Tok::CallbackKw => "callback",
                    _ => "click",
                });
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
            Tok::Label(_) => {
                if let Some(symbol) = self.pending_relation_source.take() {
                    self.push_symbol(facts, symbol, ExpectedClassName::MemberOwner);
                }
            }
            Tok::Name(name) => {
                let symbol = ClassTokenSymbol {
                    name,
                    span: SourceSpan::new(start, end),
                };
                if self.after_annotation_start {
                    self.after_annotation_start = false;
                    return;
                }

                if let Some(expected) = self.expected_name.take() {
                    self.push_symbol(facts, symbol, expected);
                } else {
                    self.pending_relation_source = Some(symbol);
                }
            }
            Tok::AccTitle(_) => facts.push_directive_prefix("accTitle"),
            Tok::AccDescr(_) | Tok::AccDescrMultiline(_) => facts.push_directive_prefix("accDescr"),
            Tok::Direction(_)
            | Tok::Note
            | Tok::HrefKw
            | Tok::StructStart
            | Tok::SquareStart
            | Tok::SquareStop
            | Tok::StyleSeparator
            | Tok::Str(_)
            | Tok::Member(_)
            | Tok::RestOfLine(_)
            | Tok::LinkTarget(_)
            | Tok::CallbackName(_)
            | Tok::CallbackArgs(_) => {}
        }
    }

    fn reset_line_state(&mut self) {
        self.expected_name = None;
        self.pending_relation_source = None;
        self.after_annotation_start = false;
    }

    fn expect_name(&mut self, expected: ExpectedClassName) {
        self.expected_name = Some(expected);
    }

    fn push_pending_relation_source(&mut self, facts: &mut EditorSemanticFacts) {
        if let Some(symbol) = self.pending_relation_source.take() {
            self.push_symbol(facts, symbol, ExpectedClassName::RelationTarget);
        }
    }

    fn push_symbol(
        &self,
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
            ExpectedClassName::CssClassTarget => "class css target",
            ExpectedClassName::StyleTarget => "class style target",
            ExpectedClassName::ClassDef => "class definition",
            ExpectedClassName::ClickTarget => "class interaction target",
            ExpectedClassName::RelationTarget => "class relation target",
        };
        let kind = match expected {
            ExpectedClassName::Namespace => EditorSemanticKind::Namespace,
            ExpectedClassName::ClassDef
            | ExpectedClassName::StyleTarget
            | ExpectedClassName::CssClassTarget => EditorSemanticKind::Property,
            ExpectedClassName::ClickTarget => EditorSemanticKind::Function,
            _ => EditorSemanticKind::Class,
        };
        let selection = selection_span_for_class_name(&symbol.name, symbol.span);
        facts.push_symbol(EditorSemanticSymbol::new(
            symbol.name,
            Some(detail.to_string()),
            kind,
            symbol.span,
            selection,
        ));
    }
}

fn selection_span_for_class_name(name: &str, span: SourceSpan) -> SourceSpan {
    if let Some(raw) = name.strip_prefix(MERMAID_DOM_ID_PREFIX) {
        return SourceSpan::new(span.start, span.start + raw.len());
    }

    if span.end > span.start + 1 {
        return SourceSpan::new(span.start, span.end);
    }

    span
}
