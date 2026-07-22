use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorLexemeKind, EditorLexemeModifier,
    EditorLexemeModifiers, EditorSemanticFacts, EditorSemanticKind, EditorSemanticSymbol, Error,
    ParseMetadata, Result, SourceSpan,
    editor::{
        EditorLexemeBatchResult, EditorLexemeJournal, format_lalrpop_parse_error,
        lalrpop_parse_diagnostic, lalrpop_recovery_span,
    },
};
use serde_json::{Value, json};
#[cfg(test)]
use std::cell::Cell;
use std::collections::{BTreeMap, HashMap, VecDeque};

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

include_checked_in_lalrpop_parser!(
    #[allow(clippy::empty_line_after_outer_attr)]
    er_grammar,
    "er_grammar.rs"
);

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ErDiagramRenderModel {
    #[serde(default, rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(default, rename = "accDescr")]
    pub acc_descr: Option<String>,
    pub direction: String,
    #[serde(default)]
    pub classes: BTreeMap<String, ErClassDefRenderModel>,
    #[serde(default)]
    pub entities: BTreeMap<String, ErEntityRenderModel>,
    #[serde(default)]
    pub relationships: Vec<ErRelationshipRenderModel>,
}

impl ErDiagramRenderModel {
    pub(crate) fn sanitize_common_db_fields(&mut self, config: &crate::MermaidConfig) {
        crate::common_db::sanitize_optional_acc_title(&mut self.acc_title, config);
        crate::common_db::sanitize_optional_acc_descr(&mut self.acc_descr, config);
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ErAttributeRenderModel {
    #[serde(rename = "type")]
    pub ty: String,
    pub name: String,
    #[serde(default)]
    pub keys: Vec<String>,
    #[serde(default)]
    pub comment: String,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ErRelSpecRenderModel {
    #[serde(rename = "cardA")]
    pub card_a: String,
    #[serde(rename = "cardB")]
    pub card_b: String,
    #[serde(rename = "relType")]
    pub rel_type: String,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ErRelationshipRenderModel {
    #[serde(rename = "entityA")]
    pub entity_a: String,
    #[serde(default, rename = "roleA")]
    pub role_a: String,
    #[serde(rename = "entityB")]
    pub entity_b: String,
    #[serde(default, rename = "relSpec")]
    pub rel_spec: ErRelSpecRenderModel,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ErClassDefRenderModel {
    pub id: String,
    #[serde(default)]
    pub styles: Vec<String>,
    #[serde(default, rename = "textStyles")]
    pub text_styles: Vec<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ErEntityRenderModel {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub attributes: Vec<ErAttributeRenderModel>,
    #[serde(default)]
    pub alias: String,
    #[serde(default)]
    pub shape: String,
    #[serde(default, rename = "cssClasses")]
    pub css_classes: String,
    #[serde(default, rename = "cssStyles")]
    pub css_styles: Vec<String>,
}

pub(crate) type Attribute = ErAttributeRenderModel;
pub(crate) type RelSpec = ErRelSpecRenderModel;
type Relationship = ErRelationshipRenderModel;
type EntityClass = ErClassDefRenderModel;
type EntityNode = ErEntityRenderModel;

#[derive(Debug, Clone)]
enum Action {
    AddEntity {
        name: String,
        alias: Option<String>,
    },
    AddAttributes {
        entity: String,
        attributes: Vec<Attribute>,
    },
    AddRelationship {
        a: String,
        role: String,
        b: String,
        spec: RelSpec,
    },
    SetClass {
        entities: Vec<String>,
        classes: Vec<String>,
    },
    AddClassDef {
        classes: Vec<String>,
        raw: String,
    },
    AddCssStyles {
        entities: Vec<String>,
        raw: String,
    },
    SetDirection(String),
    SetAccTitle(String),
    SetAccDescr(String),
}

#[derive(Debug, Clone)]
struct SpannedId {
    name: String,
    span: SourceSpan,
}

#[derive(Debug, Clone)]
struct SpannedIdList {
    ids: Vec<SpannedId>,
}

impl SpannedIdList {
    fn into_names(self) -> Vec<String> {
        self.ids.into_iter().map(|id| id.name).collect()
    }

    fn span(&self) -> SourceSpan {
        let start = self.ids.first().map(|id| id.span.start).unwrap_or(0);
        let end = self.ids.last().map(|id| id.span.end).unwrap_or(start);
        SourceSpan::new(start, end)
    }
}

#[derive(Debug, Default)]
struct ErDb {
    entities: HashMap<String, EntityNode>,
    relationships: Vec<Relationship>,
    classes: HashMap<String, EntityClass>,
    direction: String,
    entity_counter: usize,
    acc_title: Option<String>,
    acc_descr: Option<String>,
}

impl ErDb {
    fn new() -> Self {
        Self {
            direction: "TB".to_string(),
            ..Default::default()
        }
    }

    fn add_entity(&mut self, name: &str, alias: Option<&str>) {
        let Some(existing) = self.entities.get_mut(name) else {
            let id = format!("entity-{name}-{}", self.entity_counter);
            self.entity_counter += 1;
            self.entities.insert(
                name.to_string(),
                EntityNode {
                    id,
                    label: name.to_string(),
                    attributes: Vec::new(),
                    alias: alias.unwrap_or("").to_string(),
                    shape: "erBox".to_string(),
                    css_classes: "default".to_string(),
                    css_styles: Vec::new(),
                },
            );
            return;
        };

        if existing.alias.is_empty()
            && let Some(a) = alias
            && !a.is_empty()
        {
            existing.alias = a.to_string();
        }
    }

    fn add_attributes(&mut self, entity: &str, attributes: Vec<Attribute>) {
        self.add_entity(entity, None);
        let Some(e) = self.entities.get_mut(entity) else {
            return;
        };
        for a in attributes {
            e.attributes.push(a);
        }
    }

    fn add_relationship(&mut self, a: &str, role: &str, b: &str, spec: RelSpec) {
        let (Some(entity_a), Some(entity_b)) = (self.entities.get(a), self.entities.get(b)) else {
            return;
        };
        self.relationships.push(Relationship {
            entity_a: entity_a.id.clone(),
            role_a: role.to_string(),
            entity_b: entity_b.id.clone(),
            rel_spec: spec,
        });
    }

    fn set_class(&mut self, entities: &[String], classes: &[String]) {
        for e in entities {
            let Some(node) = self.entities.get_mut(e) else {
                continue;
            };
            for cls in classes {
                node.css_classes.push(' ');
                node.css_classes.push_str(cls);
            }
        }
    }

    fn add_class_def(&mut self, classes: &[String], styles: &[String]) {
        for id in classes {
            let entry = self
                .classes
                .entry(id.to_string())
                .or_insert_with(|| EntityClass {
                    id: id.to_string(),
                    ..Default::default()
                });

            for s in styles {
                if s.contains("color") {
                    entry.text_styles.push(s.replace("fill", "bgFill"));
                }
                entry.styles.push(s.to_string());
            }
        }
    }

    fn add_css_styles(&mut self, entities: &[String], styles: &[String]) {
        for id in entities {
            let Some(entity) = self.entities.get_mut(id) else {
                continue;
            };
            for style in styles {
                entity.css_styles.push(style.to_string());
            }
        }
    }

    fn apply(&mut self, a: Action) {
        match a {
            Action::AddEntity { name, alias } => self.add_entity(&name, alias.as_deref()),
            Action::AddAttributes { entity, attributes } => {
                self.add_attributes(&entity, attributes)
            }
            Action::AddRelationship { a, role, b, spec } => {
                self.add_relationship(&a, &role, &b, spec)
            }
            Action::SetClass { entities, classes } => self.set_class(&entities, &classes),
            Action::AddClassDef { classes, raw } => {
                let styles = split_styles(&raw);
                self.add_class_def(&classes, &styles);
            }
            Action::AddCssStyles { entities, raw } => {
                let styles = split_styles(&raw);
                self.add_css_styles(&entities, &styles);
            }
            Action::SetDirection(dir) => self.direction = dir,
            Action::SetAccTitle(t) => {
                self.acc_title = Some(t.trim().trim_start().to_string());
            }
            Action::SetAccDescr(t) => {
                // Mermaid's commonDb.ts: `sanitizeText(txt).replace(/\n\s+/g, '\n')`
                let trimmed = t.trim();
                let mut out = String::with_capacity(trimmed.len());
                let mut chars = trimmed.chars().peekable();
                while let Some(ch) = chars.next() {
                    out.push(ch);
                    if ch == '\n' {
                        while chars.peek().is_some_and(|c| c.is_whitespace()) {
                            chars.next();
                        }
                    }
                }
                self.acc_descr = Some(out);
            }
        }
    }

    fn into_render_model(self) -> ErDiagramRenderModel {
        ErDiagramRenderModel {
            acc_title: self.acc_title,
            acc_descr: self.acc_descr,
            direction: self.direction,
            classes: self.classes.into_iter().collect(),
            entities: self.entities.into_iter().collect(),
            relationships: self.relationships,
        }
    }

    fn into_model(self, meta: &ParseMetadata) -> Result<Value> {
        let model = self.into_render_model();
        render_model_to_compat_json(&model, meta)
    }
}

pub(crate) fn render_model_to_compat_json(
    model: &ErDiagramRenderModel,
    meta: &ParseMetadata,
) -> Result<Value> {
    let mut value = serde_json::to_value(model)
        .map_err(|e| Error::diagram_parse_fallback(meta.diagram_type.clone(), e.to_string()))?;
    let Value::Object(obj) = &mut value else {
        return Ok(value);
    };

    obj.insert("type".to_string(), json!(meta.diagram_type));
    obj.insert(
        "constants".to_string(),
        json!({
            "cardinality": {
                "zeroOrOne": "ZERO_OR_ONE",
                "zeroOrMore": "ZERO_OR_MORE",
                "oneOrMore": "ONE_OR_MORE",
                "onlyOne": "ONLY_ONE",
                "mdParent": "MD_PARENT",
            },
            "identification": {
                "nonIdentifying": "NON_IDENTIFYING",
                "identifying": "IDENTIFYING",
            }
        }),
    );

    Ok(value)
}

fn split_styles(raw: &str) -> Vec<String> {
    let compact: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
    compact
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

type ErLexicalEvent = std::result::Result<(usize, Tok, usize), LexError>;
type ErGrammarError = lalrpop_util::ParseError<usize, Tok, LexError>;

struct ErSyntax {
    events: Vec<ErLexicalEvent>,
    lexemes: EditorLexemeBatchResult,
}

impl ErSyntax {
    fn lex(code: &str) -> Self {
        let mut journal = EditorLexemeJournal::family_lexer(code);
        let events = {
            let lexer = Lexer::new(code, &mut journal);
            lexer.collect()
        };
        Self {
            events,
            lexemes: journal.finish(),
        }
    }

    fn into_editor_facts_and_actions(
        self,
    ) -> (
        EditorSemanticFacts,
        std::result::Result<Vec<Action>, ErGrammarError>,
    ) {
        let mut facts = EditorSemanticFacts::new();
        let mut collector = ErEditorFactCollector::default();
        for event in &self.events {
            match event {
                Ok((start, token, end)) => collector.accept(token, *start, *end, &mut facts),
                Err(_) => facts.mark_recovered(),
            }
        }
        collector.finish(&mut facts);
        facts.replace_family_lexemes(self.lexemes);
        let actions = er_grammar::ActionsParser::new().parse(self.events);
        (facts, actions)
    }
}

struct ErSemanticSource {
    db: ErDb,
    editor_facts: EditorSemanticFacts,
}

struct ErSemanticFailure {
    error: ErGrammarError,
    editor_facts: EditorSemanticFacts,
}

impl ErSemanticFailure {
    fn recovery_span(&self, fallback_offset: usize) -> SourceSpan {
        match &self.error {
            lalrpop_util::ParseError::User { error } => error.span,
            _ => lalrpop_recovery_span(&self.error, fallback_offset),
        }
    }

    fn into_parse_error(self, meta: &ParseMetadata, fallback_offset: usize) -> Error {
        self.into_error_and_editor_facts(meta, fallback_offset).0
    }

    fn into_error_and_editor_facts(
        self,
        meta: &ParseMetadata,
        fallback_offset: usize,
    ) -> (Error, EditorSemanticFacts) {
        self.into_error_and_editor_facts_for_type(&meta.diagram_type, fallback_offset)
    }

    fn into_error_and_editor_facts_for_type(
        mut self,
        diagram_type: &str,
        fallback_offset: usize,
    ) -> (Error, EditorSemanticFacts) {
        let error = Error::diagram_parse_diagnostic(
            diagram_type.to_string(),
            lalrpop_parse_diagnostic(&self.error, fallback_offset),
        );
        let span = self.recovery_span(fallback_offset);
        self.editor_facts.mark_recovered_from_parse_error(
            format!(
                "er parser recovered after parse error: {}",
                format_lalrpop_parse_error(&self.error)
            ),
            Some(span),
        );
        (error, self.editor_facts)
    }
}

fn construct_er_semantic_source(
    code: &str,
) -> std::result::Result<ErSemanticSource, Box<ErSemanticFailure>> {
    let syntax = ErSyntax::lex(code);
    let (editor_facts, actions) = syntax.into_editor_facts_and_actions();
    let actions = match actions {
        Ok(actions) => actions,
        Err(error) => {
            return Err(Box::new(ErSemanticFailure {
                error,
                editor_facts,
            }));
        }
    };

    let mut db = ErDb::new();
    for a in actions {
        db.apply(a);
    }
    Ok(ErSemanticSource { db, editor_facts })
}

fn parse_er_semantic_source(code: &str, meta: &ParseMetadata) -> Result<ErSemanticSource> {
    construct_er_semantic_source(code)
        .map_err(|failure| (*failure).into_parse_error(meta, code.len()))
}

pub(crate) fn parse_er_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<ErDiagramRenderModel> {
    Ok(parse_er_semantic_source(code, meta)?.db.into_render_model())
}

pub(crate) fn parse_er(code: &str, meta: &ParseMetadata) -> Result<Value> {
    parse_er_semantic_source(code, meta)?.db.into_model(meta)
}

pub(crate) fn parse_er_json_and_editor_facts(
    code: &str,
    meta: &ParseMetadata,
) -> crate::family::CombinedSemanticParse {
    crate::family::CombinedSemanticParse::from_construction(
        construct_er_semantic_source(code),
        |ErSemanticSource { db, editor_facts }| (db.into_model(meta), editor_facts),
        |failure| (*failure).into_error_and_editor_facts(meta, code.len()),
    )
}

#[derive(Debug, Default)]
struct ErEditorFactCollector {
    pending_entity: Option<ErTokenSymbol>,
    expected_id_list: Option<ExpectedErIdList>,
    in_attribute_block: bool,
    in_alias: bool,
    in_relationship_role: bool,
    attr_word_index: usize,
}

impl ErEditorFactCollector {
    fn finish(&mut self, facts: &mut EditorSemanticFacts) {
        self.push_pending_entity(facts);
    }

    fn finish_line(&mut self, facts: &mut EditorSemanticFacts) {
        if !self.in_alias && !self.in_relationship_role {
            self.push_pending_entity(facts);
        }
    }
}

#[derive(Debug, Clone)]
struct ErTokenSymbol {
    name: String,
    span: SourceSpan,
}

#[derive(Debug, Clone, Copy)]
enum ExpectedErIdList {
    StyleEntities,
    ClassDef,
    ClassEntities,
    ClassNames,
    InlineClasses,
}

impl ErEditorFactCollector {
    fn accept(&mut self, token: &Tok, start: usize, end: usize, facts: &mut EditorSemanticFacts) {
        match token {
            Tok::ErDiagram => self.reset_line_state(),
            Tok::Newline => {
                self.finish_line(facts);
                self.reset_line_state();
            }
            Tok::StyleKw => {
                facts.push_directive_prefix("style");
                self.expected_id_list = Some(ExpectedErIdList::StyleEntities);
            }
            Tok::ClassDefKw => {
                facts.push_directive_prefix("classDef");
                self.expected_id_list = Some(ExpectedErIdList::ClassDef);
            }
            Tok::ClassKw => {
                facts.push_directive_prefix("class");
                self.expected_id_list = Some(ExpectedErIdList::ClassEntities);
            }
            Tok::StyleSeparator => {
                self.push_pending_entity(facts);
                self.expected_id_list = Some(ExpectedErIdList::InlineClasses);
            }
            Tok::IdList(ids) => self.push_id_list(ids.clone(), facts),
            Tok::Name(name) => {
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
                    span,
                };
                if let Some(entity) = self.pending_entity.replace(symbol) {
                    self.push_entity_symbol(facts, entity, "er entity reference");
                }
            }
            Tok::ZeroOrOne
            | Tok::ZeroOrMore
            | Tok::OneOrMore
            | Tok::OnlyOne
            | Tok::MdParent
            | Tok::Identifying
            | Tok::NonIdentifying => self.push_pending_entity(facts),
            Tok::Colon => {
                self.push_pending_entity(facts);
                self.in_relationship_role = true;
            }
            Tok::BlockStart => {
                self.push_pending_entity(facts);
                self.in_attribute_block = true;
                self.attr_word_index = 0;
            }
            Tok::BlockStop => {
                self.in_attribute_block = false;
                self.attr_word_index = 0;
            }
            Tok::AttrWord(word) => {
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
            Tok::Comma => {
                if self.in_attribute_block && self.attr_word_index > 2 {
                    self.attr_word_index = 2;
                }
            }
            Tok::Question => {}
            Tok::AttrKey(key) => {
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
            Tok::Comment(comment) => {
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
            Tok::AccTitle(_) => facts.push_directive_prefix("accTitle"),
            Tok::AccDescr(_) | Tok::AccDescrMultiline(_) => facts.push_directive_prefix("accDescr"),
            Tok::SquareStart => {
                self.push_pending_entity(facts);
                self.in_alias = true;
            }
            Tok::SquareStop => self.in_alias = false,
            Tok::Str(value) => {
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
            Tok::Direction(_) | Tok::RestOfLine(_) => {}
        }
    }

    fn reset_line_state(&mut self) {
        self.pending_entity = None;
        self.expected_id_list = None;
        self.in_alias = false;
        self.in_relationship_role = false;
        if !self.in_attribute_block {
            self.attr_word_index = 0;
        }
    }

    fn push_pending_entity(&mut self, facts: &mut EditorSemanticFacts) {
        if let Some(entity) = self.pending_entity.take() {
            self.push_entity_symbol(facts, entity, "er entity");
        }
    }

    fn push_id_list(&mut self, ids: SpannedIdList, facts: &mut EditorSemanticFacts) {
        let expected = self.expected_id_list.take();
        let span = ids.span();
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

        if expected.is_some() {
            facts.push_expected_syntax(EditorExpectedSyntax::new(
                EditorExpectedSyntaxKind::IdList,
                span,
            ));
        }

        for id in ids.ids {
            if id.name.is_empty() {
                continue;
            }
            facts.push_symbol(EditorSemanticSymbol::new(
                id.name,
                Some(detail.to_string()),
                kind,
                id.span,
                id.span,
            ));
        }

        if matches!(expected, Some(ExpectedErIdList::ClassEntities)) {
            self.expected_id_list = Some(ExpectedErIdList::ClassNames);
        }
    }

    fn push_entity_symbol(
        &self,
        facts: &mut EditorSemanticFacts,
        symbol: ErTokenSymbol,
        detail: &'static str,
    ) {
        if symbol.name.is_empty() {
            return;
        }
        facts.push_symbol(EditorSemanticSymbol::new(
            symbol.name,
            Some(detail.to_string()),
            EditorSemanticKind::Struct,
            symbol.span,
            symbol.span,
        ));
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

#[derive(Debug, Clone)]
enum Tok {
    ErDiagram,
    Newline,

    Name(String),
    Str(String),
    IdList(SpannedIdList),
    RestOfLine(String),

    AccTitle(String),
    AccDescr(String),
    AccDescrMultiline(String),

    BlockStart,
    BlockStop,
    SquareStart,
    SquareStop,
    StyleSeparator,
    Colon,
    Comma,
    Question,

    StyleKw,
    ClassDefKw,
    ClassKw,
    Direction(String),

    ZeroOrOne,
    ZeroOrMore,
    OneOrMore,
    OnlyOne,
    MdParent,
    Identifying,
    NonIdentifying,

    AttrWord(String),
    AttrKey(String),
    Comment(String),
}

#[derive(Debug, Clone)]
struct LexError {
    message: String,
    span: SourceSpan,
}

impl LexError {
    fn new(message: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for LexError {}

impl crate::error::ParseErrorSourceSpan for LexError {
    fn source_span(&self) -> Option<crate::SourceSpan> {
        Some(self.span)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Default,
    Block,
    NeedIdListOnly,
    NeedIdListThenLineRest,
    NeedClassFirstIdList,
    NeedClassSecondIdList,
    LineRest,
}

struct Lexer<'input, 'journal> {
    input: &'input str,
    lexemes: &'journal mut EditorLexemeJournal<'input>,
    pos: usize,
    pending: VecDeque<(usize, Tok, usize)>,
    mode: Mode,
}

impl<'input, 'journal> Lexer<'input, 'journal> {
    fn new(input: &'input str, lexemes: &'journal mut EditorLexemeJournal<'input>) -> Self {
        #[cfg(test)]
        ER_SYNTAX_CONSTRUCTION_COUNT.set(ER_SYNTAX_CONSTRUCTION_COUNT.get() + 1);

        Self {
            input,
            lexemes,
            pos: 0,
            pending: VecDeque::new(),
            mode: Mode::Default,
        }
    }

    fn push_lexeme(&mut self, kind: EditorLexemeKind, start: usize, end: usize) {
        self.lexemes.push(
            kind,
            EditorLexemeModifiers::NONE,
            SourceSpan::new(start, end),
        );
    }

    fn push_modified_lexeme(
        &mut self,
        kind: EditorLexemeKind,
        modifier: EditorLexemeModifier,
        start: usize,
        end: usize,
    ) {
        self.lexemes.push(
            kind,
            EditorLexemeModifiers::from_modifier(modifier),
            SourceSpan::new(start, end),
        );
    }

    fn emit_token(
        &mut self,
        token: (usize, Tok, usize),
    ) -> std::result::Result<(usize, Tok, usize), LexError> {
        self.record_token_lexemes(&token.1, token.0, token.2);
        Ok(token)
    }

    fn emit_result(
        &mut self,
        result: std::result::Result<(usize, Tok, usize), LexError>,
    ) -> std::result::Result<(usize, Tok, usize), LexError> {
        match result {
            Ok(token) => self.emit_token(token),
            Err(error) => Err(error),
        }
    }

    fn record_token_lexemes(&mut self, token: &Tok, start: usize, end: usize) {
        match token {
            Tok::Newline => {}
            Tok::ErDiagram | Tok::StyleKw | Tok::ClassDefKw | Tok::ClassKw => {
                self.push_lexeme(EditorLexemeKind::Keyword, start, end)
            }
            Tok::Name(_) | Tok::AttrWord(_) => self.record_string_or_identifier(start, end),
            Tok::Str(_) | Tok::Comment(_) => self.record_quoted_string(start, end),
            Tok::IdList(ids) => self.record_id_list(ids, start, end),
            Tok::RestOfLine(_) => self.record_trimmed(EditorLexemeKind::Style, start, end),
            Tok::AccTitle(_) => self.record_keyword_value(start, end, "accTitle"),
            Tok::AccDescr(_) | Tok::AccDescrMultiline(_) => {
                self.record_keyword_value(start, end, "accDescr")
            }
            Tok::Direction(direction) => self.record_direction(start, end, direction),
            Tok::BlockStart
            | Tok::BlockStop
            | Tok::SquareStart
            | Tok::SquareStop
            | Tok::StyleSeparator
            | Tok::Colon
            | Tok::Comma => self.push_lexeme(EditorLexemeKind::Delimiter, start, end),
            Tok::Question => self.push_lexeme(EditorLexemeKind::Operator, start, end),
            Tok::ZeroOrOne
            | Tok::ZeroOrMore
            | Tok::OneOrMore
            | Tok::OnlyOne
            | Tok::MdParent
            | Tok::Identifying
            | Tok::NonIdentifying => self.push_lexeme(EditorLexemeKind::Operator, start, end),
            Tok::AttrKey(_) => self.push_modified_lexeme(
                EditorLexemeKind::Keyword,
                EditorLexemeModifier::Readonly,
                start,
                end,
            ),
        }
    }

    fn trimmed_bounds(&self, start: usize, end: usize) -> Option<(usize, usize)> {
        let raw = self.input.get(start..end)?;
        let leading = raw.len().saturating_sub(raw.trim_start().len());
        let trailing = raw.trim_end().len();
        (leading < trailing).then_some((start + leading, start + trailing))
    }

    fn record_trimmed(&mut self, kind: EditorLexemeKind, start: usize, end: usize) {
        if let Some((start, end)) = self.trimmed_bounds(start, end) {
            self.push_lexeme(kind, start, end);
        }
    }

    fn record_string_or_identifier(&mut self, start: usize, end: usize) {
        let Some((start, end)) = self.trimmed_bounds(start, end) else {
            return;
        };
        let raw = &self.input[start..end];
        if (raw.starts_with('"') && raw.ends_with('"'))
            || (raw.starts_with('`') && raw.ends_with('`'))
        {
            self.record_quoted_string(start, end);
        } else {
            self.push_lexeme(EditorLexemeKind::Identifier, start, end);
        }
    }

    fn record_quoted_string(&mut self, start: usize, end: usize) {
        let Some((start, end)) = self.trimmed_bounds(start, end) else {
            return;
        };
        let raw = &self.input[start..end];
        let delimiter = raw.as_bytes().first().copied();
        if raw.len() >= 2
            && delimiter.is_some_and(|byte| matches!(byte, b'"' | b'`'))
            && raw.as_bytes().last().copied() == delimiter
        {
            self.push_lexeme(EditorLexemeKind::Delimiter, start, start + 1);
            if end > start + 2 {
                self.push_lexeme(EditorLexemeKind::String, start + 1, end - 1);
            }
            self.push_lexeme(EditorLexemeKind::Delimiter, end - 1, end);
        } else {
            self.push_lexeme(EditorLexemeKind::String, start, end);
        }
    }

    fn record_id_list(&mut self, ids: &SpannedIdList, start: usize, end: usize) {
        let mut cursor = start;
        for id in &ids.ids {
            if cursor < id.span.start
                && let Some(gap) = self.input.get(cursor..id.span.start)
            {
                for (offset, byte) in gap.bytes().enumerate() {
                    if byte == b',' {
                        self.push_lexeme(
                            EditorLexemeKind::Delimiter,
                            cursor + offset,
                            cursor + offset + 1,
                        );
                    }
                }
            }
            self.push_lexeme(EditorLexemeKind::Identifier, id.span.start, id.span.end);
            cursor = id.span.end;
        }
        if cursor < end
            && let Some(gap) = self.input.get(cursor..end)
        {
            for (offset, byte) in gap.bytes().enumerate() {
                if byte == b',' {
                    self.push_lexeme(
                        EditorLexemeKind::Delimiter,
                        cursor + offset,
                        cursor + offset + 1,
                    );
                }
            }
        }
    }

    fn record_keyword_value(&mut self, start: usize, end: usize, keyword: &str) {
        let Some(raw) = self.input.get(start..end) else {
            return;
        };
        if !raw
            .get(..keyword.len())
            .is_some_and(|value| value.eq_ignore_ascii_case(keyword))
        {
            return;
        }
        self.push_lexeme(EditorLexemeKind::Keyword, start, start + keyword.len());
        let mut cursor = keyword.len();
        while raw
            .as_bytes()
            .get(cursor)
            .is_some_and(u8::is_ascii_whitespace)
        {
            cursor += 1;
        }
        if raw
            .as_bytes()
            .get(cursor)
            .is_some_and(|byte| matches!(byte, b':' | b'{'))
        {
            self.push_lexeme(
                EditorLexemeKind::Delimiter,
                start + cursor,
                start + cursor + 1,
            );
            cursor += 1;
        }
        while raw
            .as_bytes()
            .get(cursor)
            .is_some_and(u8::is_ascii_whitespace)
        {
            cursor += 1;
        }
        let mut value_end = raw.trim_end().len();
        if raw.as_bytes().get(value_end.wrapping_sub(1)) == Some(&b'}') {
            self.push_lexeme(
                EditorLexemeKind::Delimiter,
                start + value_end - 1,
                start + value_end,
            );
            value_end -= 1;
        }
        if cursor < value_end {
            let kind = if keyword.eq_ignore_ascii_case("direction") {
                EditorLexemeKind::Literal
            } else {
                EditorLexemeKind::String
            };
            self.push_lexeme(kind, start + cursor, start + value_end);
        }
    }

    fn record_direction(&mut self, start: usize, end: usize, direction: &str) {
        let Some(raw) = self.input.get(start..end) else {
            return;
        };
        let keyword = "direction";
        if !raw
            .get(..keyword.len())
            .is_some_and(|value| value.eq_ignore_ascii_case(keyword))
        {
            return;
        }
        self.push_lexeme(EditorLexemeKind::Keyword, start, start + keyword.len());
        let mut cursor = keyword.len();
        while raw
            .as_bytes()
            .get(cursor)
            .is_some_and(u8::is_ascii_whitespace)
        {
            cursor += 1;
        }
        let value_end = cursor.saturating_add(direction.len());
        if raw
            .get(cursor..value_end)
            .is_some_and(|value| value.eq_ignore_ascii_case(direction))
        {
            self.push_lexeme(EditorLexemeKind::Literal, start + cursor, start + value_end);
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.as_bytes().get(self.pos).copied()
    }

    fn skip_ws_default(&mut self) {
        while let Some(b) = self.peek() {
            if b == b' ' || b == b'\t' || b == b'\r' {
                self.pos += 1;
                continue;
            }
            break;
        }
    }

    fn skip_ws_block(&mut self) {
        while let Some(b) = self.peek() {
            if matches!(b, b' ' | b'\t' | b'\r' | b'\n') {
                self.pos += 1;
                continue;
            }
            break;
        }
    }

    fn starts_with_ci(&self, s: &str) -> bool {
        self.input[self.pos..]
            .get(..s.len())
            .is_some_and(|h| h.eq_ignore_ascii_case(s))
    }

    fn starts_with_word_ci(&self, s: &str) -> bool {
        if !self.starts_with_ci(s) {
            return false;
        }
        let after = self.pos + s.len();
        if after >= self.input.len() {
            return true;
        }
        let b = self.input.as_bytes()[after];
        b.is_ascii_whitespace() || matches!(b, b':' | b'{' | b'}' | b'[' | b']' | b';')
    }

    fn read_to_newline(&mut self) -> String {
        let start = self.pos;
        while let Some(b) = self.peek() {
            if b == b'\n' {
                break;
            }
            self.pos += 1;
        }
        self.input[start..self.pos].to_string()
    }

    fn lex_comment(&mut self) -> bool {
        if self.input[self.pos..].starts_with("%%") {
            let _ = self.read_to_newline();
            return true;
        }
        false
    }

    fn lex_newline(&mut self) -> Option<(usize, Tok, usize)> {
        if self.mode == Mode::Block {
            return None;
        }
        if self.peek()? != b'\n' {
            return None;
        }
        let start = self.pos;
        while let Some(b'\n') = self.peek() {
            self.pos += 1;
        }
        if self.mode == Mode::LineRest {
            self.mode = Mode::Default;
        }
        Some((start, Tok::Newline, self.pos))
    }

    fn lex_acc_title(&mut self) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        if !self.starts_with_ci("accTitle") {
            return None;
        }
        let after = self.pos + "accTitle".len();
        let rest = &self.input[after..];
        let rest_trim = rest.trim_start();
        if !rest_trim.starts_with(':') {
            return None;
        }
        let consumed_ws = rest.len() - rest_trim.len();
        self.pos = after + consumed_ws + 1;
        let s = self.read_to_newline();
        Some(Ok((start, Tok::AccTitle(s.trim().to_string()), self.pos)))
    }

    fn lex_acc_descr(&mut self) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        if !self.starts_with_ci("accDescr") {
            return None;
        }
        let after = self.pos + "accDescr".len();
        let rest = &self.input[after..];
        let rest_trim = rest.trim_start();
        if rest_trim.starts_with('{') {
            let consumed_ws = rest.len() - rest_trim.len();
            self.pos = after + consumed_ws + 1;
            let Some(end_rel) = self.input[self.pos..].find('}') else {
                self.record_keyword_value(start, self.input.len(), "accDescr");
                self.pos = self.input.len();
                return Some(Err(LexError::new(
                    "Unterminated accDescr block; missing '}'",
                    SourceSpan::new(start, self.input.len()),
                )));
            };
            let body = self.input[self.pos..self.pos + end_rel].to_string();
            self.pos = self.pos + end_rel + 1;
            return Some(Ok((
                start,
                Tok::AccDescrMultiline(body.trim().to_string()),
                self.pos,
            )));
        }
        let colon_pos = rest.find(':')?;
        self.pos = after + colon_pos + 1;
        let s = self.read_to_newline();
        Some(Ok((start, Tok::AccDescr(s.trim().to_string()), self.pos)))
    }

    fn lex_direction(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        if !self.starts_with_word_ci("direction") {
            return None;
        }
        self.pos += "direction".len();
        self.skip_ws_default();
        let rest = &self.input[self.pos..].to_ascii_uppercase();
        let dir = if rest.starts_with("TB") {
            self.pos += 2;
            "TB"
        } else if rest.starts_with("BT") {
            self.pos += 2;
            "BT"
        } else if rest.starts_with("LR") {
            self.pos += 2;
            "LR"
        } else if rest.starts_with("RL") {
            self.pos += 2;
            "RL"
        } else {
            return None;
        };
        let _ = self.read_to_newline();
        Some((start, Tok::Direction(dir.to_string()), self.pos))
    }

    fn lex_keyword(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        if self.starts_with_word_ci("erDiagram") {
            self.pos += "erDiagram".len();
            return Some((start, Tok::ErDiagram, self.pos));
        }
        if self.starts_with_word_ci("style") {
            self.pos += "style".len();
            self.mode = Mode::NeedIdListThenLineRest;
            return Some((start, Tok::StyleKw, self.pos));
        }
        if self.starts_with_word_ci("classDef") {
            self.pos += "classDef".len();
            self.mode = Mode::NeedIdListThenLineRest;
            return Some((start, Tok::ClassDefKw, self.pos));
        }
        if self.starts_with_word_ci("class") {
            self.pos += "class".len();
            self.mode = Mode::NeedClassFirstIdList;
            return Some((start, Tok::ClassKw, self.pos));
        }
        None
    }

    fn lex_id_list(&mut self) -> Option<(usize, Tok, usize)> {
        if !matches!(
            self.mode,
            Mode::NeedIdListOnly
                | Mode::NeedIdListThenLineRest
                | Mode::NeedClassFirstIdList
                | Mode::NeedClassSecondIdList
        ) {
            return None;
        }
        let start = self.pos;
        self.skip_ws_default();
        let mut ids: Vec<SpannedId> = Vec::new();
        loop {
            let id_start = self.pos;
            let mut id_end = self.pos;
            for (rel, ch) in self.input[self.pos..].char_indices() {
                let ok =
                    !ch.is_ascii() || ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '*');
                if !ok {
                    break;
                }
                id_end = self.pos + rel + ch.len_utf8();
            }
            if id_end == id_start {
                break;
            }
            ids.push(SpannedId {
                name: self.input[id_start..id_end].to_string(),
                span: SourceSpan::new(id_start, id_end),
            });
            self.pos = id_end;

            self.skip_ws_default();
            if self.peek() != Some(b',') {
                break;
            }
            self.pos += 1;
            self.skip_ws_default();
        }
        if ids.is_empty() {
            return None;
        }
        self.mode = match self.mode {
            Mode::NeedIdListOnly => Mode::Default,
            Mode::NeedIdListThenLineRest => Mode::LineRest,
            Mode::NeedClassFirstIdList => Mode::NeedClassSecondIdList,
            Mode::NeedClassSecondIdList => Mode::Default,
            _ => Mode::Default,
        };
        Some((start, Tok::IdList(SpannedIdList { ids }), self.pos))
    }

    fn lex_rest_of_line(&mut self) -> Option<(usize, Tok, usize)> {
        if self.mode != Mode::LineRest {
            return None;
        }
        let start = self.pos;
        self.skip_ws_default();
        let s = self.read_to_newline();
        self.mode = Mode::Default;
        Some((
            start,
            Tok::RestOfLine(s.trim().trim_end_matches(';').to_string()),
            self.pos,
        ))
    }

    fn lex_rel_tokens(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        let s = &self.input[self.pos..];

        let lower = s.to_ascii_lowercase();
        for (pat, tok) in [
            ("optionally to", Tok::NonIdentifying),
            ("one or zero", Tok::ZeroOrOne),
            ("zero or one", Tok::ZeroOrOne),
            ("one or more", Tok::OneOrMore),
            ("one or many", Tok::OneOrMore),
            ("zero or more", Tok::ZeroOrMore),
            ("zero or many", Tok::ZeroOrMore),
            ("only one", Tok::OnlyOne),
        ] {
            if lower.starts_with(pat) {
                self.pos += pat.len();
                return Some((start, tok, self.pos));
            }
        }

        if lower.starts_with("many(0)") {
            self.pos += "many(0)".len();
            return Some((start, Tok::ZeroOrMore, self.pos));
        }
        if lower.starts_with("many(1)") {
            self.pos += "many(1)".len();
            return Some((start, Tok::OneOrMore, self.pos));
        }
        if lower.starts_with("0+") {
            self.pos += "0+".len();
            return Some((start, Tok::ZeroOrMore, self.pos));
        }
        if lower.starts_with("1+") {
            self.pos += "1+".len();
            return Some((start, Tok::OneOrMore, self.pos));
        }
        if lower.starts_with("many") {
            self.pos += "many".len();
            return Some((start, Tok::ZeroOrMore, self.pos));
        }
        if lower.starts_with("one") {
            self.pos += "one".len();
            return Some((start, Tok::OnlyOne, self.pos));
        }
        if self.one_starts_numeric_cardinality() {
            self.pos += 1;
            return Some((start, Tok::OnlyOne, self.pos));
        }
        if lower.starts_with("to") {
            self.pos += "to".len();
            return Some((start, Tok::Identifying, self.pos));
        }

        for (pat, tok) in [
            ("||", Tok::OnlyOne),
            ("|o", Tok::ZeroOrOne),
            ("o|", Tok::ZeroOrOne),
            ("|{", Tok::OneOrMore),
            ("o{", Tok::ZeroOrMore),
            ("}|", Tok::OneOrMore),
            ("}o", Tok::ZeroOrMore),
        ] {
            if s.starts_with(pat) {
                self.pos += pat.len();
                return Some((start, tok, self.pos));
            }
        }

        if s.starts_with("..") || s.starts_with(".-") || s.starts_with("-.") {
            self.pos += 2;
            return Some((start, Tok::NonIdentifying, self.pos));
        }
        if s.starts_with("--") {
            self.pos += 2;
            return Some((start, Tok::Identifying, self.pos));
        }

        if s.starts_with('u')
            && self
                .input
                .as_bytes()
                .get(self.pos.wrapping_sub(1))
                .copied()
                .is_some_and(|b| matches!(b, b' ' | b'\t' | b'\r'))
            && self
                .input
                .as_bytes()
                .get(self.pos + 1)
                .copied()
                .is_some_and(|b| matches!(b, b'-' | b'.'))
        {
            self.pos += 1;
            return Some((start, Tok::MdParent, self.pos));
        }

        None
    }

    fn one_starts_numeric_cardinality(&self) -> bool {
        let Some(after_one) = self.input[self.pos..].strip_prefix('1') else {
            return false;
        };

        if ["--", "..", ".-", "-."]
            .iter()
            .any(|operator| after_one.starts_with(operator))
        {
            return true;
        }

        let Some(first) = after_one.chars().next() else {
            return false;
        };
        if !first.is_whitespace() {
            return false;
        }

        after_one
            .trim_start_matches(char::is_whitespace)
            .chars()
            .next()
            .is_some_and(|next| {
                next.is_ascii_alphabetic()
                    || next.is_ascii_digit()
                    || matches!(next, '_' | '"' | '\'')
            })
    }

    fn lex_punct(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        match self.peek()? {
            b'{' => {
                self.pos += 1;
                self.mode = Mode::Block;
                Some((start, Tok::BlockStart, self.pos))
            }
            b'}' => {
                if self.mode != Mode::Block {
                    return None;
                }
                self.pos += 1;
                self.mode = Mode::Default;
                Some((start, Tok::BlockStop, self.pos))
            }
            b'[' => {
                self.pos += 1;
                Some((start, Tok::SquareStart, self.pos))
            }
            b']' => {
                self.pos += 1;
                Some((start, Tok::SquareStop, self.pos))
            }
            b':' => {
                if self.input[self.pos..].starts_with(":::") {
                    self.pos += 3;
                    self.mode = Mode::NeedIdListOnly;
                    return Some((start, Tok::StyleSeparator, self.pos));
                }
                self.pos += 1;
                Some((start, Tok::Colon, self.pos))
            }
            b',' => {
                self.pos += 1;
                Some((start, Tok::Comma, self.pos))
            }
            _ => None,
        }
    }

    fn lex_block_token(&mut self) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        if self.mode != Mode::Block {
            return None;
        }
        let start = self.pos;
        self.skip_ws_block();
        if self.pos >= self.input.len() {
            self.mode = Mode::Default;
            return Some(Err(LexError::new(
                "EOF inside attribute block",
                SourceSpan::new(self.pos, self.pos),
            )));
        }
        if self.peek() == Some(b'}') {
            return None;
        }
        if self.peek() == Some(b',') {
            self.pos += 1;
            return Some(Ok((start, Tok::Comma, self.pos)));
        }
        if self.peek() == Some(b'?') {
            self.pos += 1;
            return Some(Ok((start, Tok::Question, self.pos)));
        }
        if self.peek() == Some(b'"') {
            self.pos += 1;
            let Some(rel_end) = self.input[self.pos..].find('"') else {
                return Some(Err(LexError::new(
                    "Unterminated comment string; missing '\"'",
                    SourceSpan::new(start, self.input.len()),
                )));
            };
            let s = self.input[self.pos..self.pos + rel_end].to_string();
            self.pos = self.pos + rel_end + 1;
            return Some(Ok((start, Tok::Comment(s), self.pos)));
        }
        if self.peek() == Some(b'`') {
            let delimiter_start = self.pos;
            self.pos += 1;
            let body_start = self.pos;
            let Some(rel_end) = self.input[self.pos..].find('`') else {
                return Some(Err(LexError::new(
                    "Unterminated attribute word; missing '`'",
                    SourceSpan::new(start, self.input.len()),
                )));
            };
            let body_end = self.pos + rel_end;
            if body_end == body_start {
                self.pos = body_end + 1;
                return Some(Err(LexError::new(
                    "Empty backtick attribute word",
                    SourceSpan::new(delimiter_start, self.pos),
                )));
            }
            let s = self.input[body_start..body_end].to_string();
            self.pos = body_end + 1;
            self.push_lexeme(
                EditorLexemeKind::Delimiter,
                delimiter_start,
                delimiter_start + 1,
            );
            self.push_lexeme(EditorLexemeKind::Delimiter, body_end, body_end + 1);
            return Some(Ok((body_start, Tok::AttrWord(s), body_end)));
        }
        if let Some(two) = self.input[self.pos..].get(..2) {
            let two_upper = two.to_ascii_uppercase();
            if matches!(two_upper.as_str(), "PK" | "FK" | "UK") {
                let prev_ok = self.pos == 0
                    || matches!(
                        self.input.as_bytes()[self.pos - 1],
                        b' ' | b'\t' | b'\r' | b'\n' | b','
                    );
                let next_ok = self
                    .input
                    .as_bytes()
                    .get(self.pos + 2)
                    .copied()
                    .map(|b| b.is_ascii_whitespace() || matches!(b, b',' | b'"' | b'}'))
                    .unwrap_or(true);
                if prev_ok && next_ok {
                    self.pos += 2;
                    return Some(Ok((start, Tok::AttrKey(two_upper), self.pos)));
                }
            }
        }

        let start_word = self.pos;
        let mut end = self.pos;
        for (rel, ch) in self.input[self.pos..].char_indices() {
            if ch.is_whitespace() || matches!(ch, '"' | '}' | '?') {
                break;
            }
            end = self.pos + rel + ch.len_utf8();
        }
        if end == start_word {
            self.pos += self.peek().map(|_| 1).unwrap_or(0);
            return Some(Err(LexError::new(
                format!("Unexpected character inside attribute block at {start_word}"),
                SourceSpan::new(start_word, self.pos),
            )));
        }
        self.pos = end;
        let raw = &self.input[start_word..end];
        let tilde_count = raw.chars().filter(|&c| c == '~').count();
        if tilde_count >= 2 {
            return Some(Ok((start, Tok::AttrWord(raw.to_string()), self.pos)));
        }

        let mut chars = raw.chars();
        let first = chars.next()?;
        let first_ok = first == '*' || first == '_' || first.is_alphabetic() || !first.is_ascii();
        let rest_ok = chars.all(|c| {
            c == '*'
                || c == '-'
                || c == '_'
                || c == '.'
                || c == ','
                || c.is_ascii_digit()
                || c.is_alphabetic()
                || matches!(c, '[' | ']' | '(' | ')')
                || !c.is_ascii()
        });
        if !first_ok || !rest_ok {
            return Some(Err(LexError::new(
                "Invalid attribute word",
                SourceSpan::new(start_word, end),
            )));
        }
        Some(Ok((start, Tok::AttrWord(raw.to_string()), self.pos)))
    }

    fn lex_name_or_str(&mut self) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        if self.mode == Mode::Block {
            return None;
        }
        let start = self.pos;
        if self.peek()? == b'"' {
            self.pos += 1;
            let Some(rel_end) = self.input[self.pos..].find('"') else {
                return Some(Err(LexError::new(
                    "Unterminated string literal; missing '\"'",
                    SourceSpan::new(start, self.input.len()),
                )));
            };
            let s = self.input[self.pos..self.pos + rel_end].to_string();
            self.pos = self.pos + rel_end + 1;
            let is_entity_name = !s.is_empty()
                && !s.contains('%')
                && !s.contains('\\')
                && !s.contains('\r')
                && !s.contains('\n')
                && !s.contains('\u{0008}')
                && !s.contains('\u{000B}');
            if is_entity_name {
                return Some(Ok((start, Tok::Name(s), self.pos)));
            }
            return Some(Ok((start, Tok::Str(s), self.pos)));
        }

        let mut end = self.pos;
        for (rel, ch) in self.input[self.pos..].char_indices() {
            let ok = !ch.is_ascii() || ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '*');
            if !ok {
                break;
            }
            end = self.pos + rel + ch.len_utf8();
        }
        if end == self.pos {
            return None;
        }
        let s = self.input[self.pos..end].to_string();
        self.pos = end;
        Some(Ok((start, Tok::Name(s), self.pos)))
    }
}

impl Iterator for Lexer<'_, '_> {
    type Item = std::result::Result<(usize, Tok, usize), LexError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(tok) = self.pending.pop_front() {
            return Some(self.emit_token(tok));
        }

        loop {
            match self.mode {
                Mode::Block => self.skip_ws_block(),
                _ => self.skip_ws_default(),
            }

            if self.pos >= self.input.len() {
                if self.mode == Mode::Block {
                    self.mode = Mode::Default;
                    return Some(Err(LexError::new(
                        "EOF inside attribute block",
                        SourceSpan::new(self.pos, self.pos),
                    )));
                }
                return None;
            }

            if self.lex_comment() {
                continue;
            }

            if let Some(tok) = self.lex_block_token() {
                return Some(self.emit_result(tok));
            }

            if let Some(tok) = self.lex_rest_of_line() {
                return Some(self.emit_token(tok));
            }

            if let Some(tok) = self.lex_newline() {
                return Some(self.emit_token(tok));
            }

            if let Some(tok) = self.lex_acc_title() {
                return Some(self.emit_result(tok));
            }

            if let Some(tok) = self.lex_acc_descr() {
                return Some(self.emit_result(tok));
            }

            if let Some(tok) = self.lex_direction() {
                return Some(self.emit_token(tok));
            }

            if let Some(tok) = self.lex_keyword() {
                return Some(self.emit_token(tok));
            }

            if let Some(tok) = self.lex_id_list() {
                return Some(self.emit_token(tok));
            }

            if let Some(tok) = self.lex_punct() {
                return Some(self.emit_token(tok));
            }

            if let Some(tok) = self.lex_rel_tokens() {
                return Some(self.emit_token(tok));
            }

            if let Some(tok) = self.lex_name_or_str() {
                return Some(self.emit_result(tok));
            }

            let start = self.pos;
            self.pos += 1;
            return Some(Err(LexError::new(
                format!("Unexpected character at {start}"),
                SourceSpan::new(start, self.pos),
            )));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MermaidConfig;

    fn meta() -> ParseMetadata {
        ParseMetadata {
            diagram_type: "er".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: MermaidConfig::empty_object(),
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
}
