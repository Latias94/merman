use crate::diagrams::scan::{LineCursor, leading_whitespace_len};
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticRole, EditorSemanticSymbol, Error, ParseMetadata, Result, SourceSpan,
};
use serde_json::{Map, Value, json};
#[cfg(test)]
use std::cell::Cell;
use std::collections::{BTreeMap, HashMap};

#[cfg(test)]
thread_local! {
    static REQUIREMENT_SYNTAX_CONSTRUCTION_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_requirement_syntax_construction_count() {
    REQUIREMENT_SYNTAX_CONSTRUCTION_COUNT.set(0);
}

#[cfg(test)]
pub(crate) fn requirement_syntax_construction_count() -> usize {
    REQUIREMENT_SYNTAX_CONSTRUCTION_COUNT.get()
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequirementRenderNode {
    pub name: String,
    #[serde(rename = "type")]
    pub node_type: String,
    #[serde(default, rename = "requirementId")]
    pub requirement_id: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub risk: String,
    #[serde(default, rename = "verifyMethod")]
    pub verify_method: String,
    #[serde(default)]
    pub css_styles: Vec<String>,
    #[serde(default)]
    pub classes: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequirementRenderElement {
    pub name: String,
    #[serde(rename = "type")]
    pub element_type: String,
    #[serde(default, rename = "docRef")]
    pub doc_ref: String,
    #[serde(default)]
    pub css_styles: Vec<String>,
    #[serde(default)]
    pub classes: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RequirementRenderRelationship {
    #[serde(rename = "type")]
    pub rel_type: String,
    pub src: String,
    pub dst: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequirementRenderClass {
    pub id: String,
    #[serde(default)]
    pub styles: Vec<String>,
    #[serde(default, rename = "textStyles")]
    pub text_styles: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequirementDiagramRenderModel {
    #[serde(default, rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(default, rename = "accDescr")]
    pub acc_descr: Option<String>,
    #[serde(default)]
    pub direction: String,
    #[serde(default)]
    pub requirements: Vec<RequirementRenderNode>,
    #[serde(default)]
    pub elements: Vec<RequirementRenderElement>,
    #[serde(default)]
    pub relationships: Vec<RequirementRenderRelationship>,
    #[serde(default)]
    pub classes: BTreeMap<String, RequirementRenderClass>,
}

impl RequirementDiagramRenderModel {
    pub(crate) fn sanitize_common_db_fields(&mut self, config: &crate::MermaidConfig) {
        crate::common_db::sanitize_optional_acc_title(&mut self.acc_title, config);
        crate::common_db::sanitize_optional_acc_descr(&mut self.acc_descr, config);
    }
}

#[derive(Debug, Clone)]
struct RequirementBuilder {
    requirement_id: String,
    text: String,
    risk: String,
    verify_method: String,
}

impl RequirementBuilder {
    fn new() -> Self {
        Self {
            requirement_id: String::new(),
            text: String::new(),
            risk: String::new(),
            verify_method: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct ElementBuilder {
    element_type: String,
    doc_ref: String,
}

impl ElementBuilder {
    fn new() -> Self {
        Self {
            element_type: String::new(),
            doc_ref: String::new(),
        }
    }
}

#[derive(Debug, Default, Clone)]
struct RequirementDb {
    direction: String,
    relations: Vec<RequirementRenderRelationship>,

    requirements: HashMap<String, RequirementRenderNode>,
    requirement_order: Vec<String>,

    elements: HashMap<String, RequirementRenderElement>,
    element_order: Vec<String>,

    classes: HashMap<String, RequirementRenderClass>,
}

impl RequirementDb {
    fn new() -> Self {
        Self {
            direction: "TB".to_string(),
            relations: Vec::new(),
            requirements: HashMap::new(),
            requirement_order: Vec::new(),
            elements: HashMap::new(),
            element_order: Vec::new(),
            classes: HashMap::new(),
        }
    }

    fn set_direction(&mut self, dir: &str) {
        self.direction = dir.to_string();
    }

    fn add_requirement(&mut self, name: &str, requirement_type: &str, b: RequirementBuilder) {
        if self.requirements.contains_key(name) {
            return;
        }
        self.requirement_order.push(name.to_string());
        self.requirements.insert(
            name.to_string(),
            RequirementRenderNode {
                name: name.to_string(),
                node_type: requirement_type.to_string(),
                requirement_id: b.requirement_id,
                text: b.text,
                risk: b.risk,
                verify_method: b.verify_method,
                css_styles: Vec::new(),
                classes: vec!["default".to_string()],
            },
        );
    }

    fn add_element(&mut self, name: &str, b: ElementBuilder) {
        if self.elements.contains_key(name) {
            return;
        }
        self.element_order.push(name.to_string());
        self.elements.insert(
            name.to_string(),
            RequirementRenderElement {
                name: name.to_string(),
                element_type: b.element_type,
                doc_ref: b.doc_ref,
                css_styles: Vec::new(),
                classes: vec!["default".to_string()],
            },
        );
    }

    fn add_relationship(&mut self, relationship_type: &str, src: &str, dst: &str) {
        self.relations.push(RequirementRenderRelationship {
            rel_type: relationship_type.to_string(),
            src: src.to_string(),
            dst: dst.to_string(),
        });
    }

    fn set_css_style(&mut self, ids: &[String], styles: &[String]) {
        for id in ids {
            let node_req = self.requirements.get_mut(id);
            if let Some(node) = node_req {
                push_styles(&mut node.css_styles, styles);
                continue;
            }
            let node_el = self.elements.get_mut(id);
            if let Some(node) = node_el {
                push_styles(&mut node.css_styles, styles);
                continue;
            }
        }
    }

    fn set_class(&mut self, ids: &[String], class_names: &[String]) {
        for id in ids {
            if let Some(node) = self.requirements.get_mut(id) {
                for cls in class_names {
                    node.classes.push(cls.clone());
                    if let Some(def) = self.classes.get(cls) {
                        node.css_styles.extend(def.styles.iter().cloned());
                    }
                }
                continue;
            }
            if let Some(node) = self.elements.get_mut(id) {
                for cls in class_names {
                    node.classes.push(cls.clone());
                    if let Some(def) = self.classes.get(cls) {
                        node.css_styles.extend(def.styles.iter().cloned());
                    }
                }
            }
        }
    }

    fn define_class(&mut self, ids: &[String], styles: &[String]) {
        for id in ids {
            let style_class =
                self.classes
                    .entry(id.to_string())
                    .or_insert_with(|| RequirementRenderClass {
                        id: id.to_string(),
                        styles: Vec::new(),
                        text_styles: Vec::new(),
                    });

            for s in styles {
                if s.contains("color") {
                    let new_style = s.replacen("fill", "bgFill", 1);
                    style_class.text_styles.push(new_style);
                }
                style_class.styles.push(s.clone());
            }

            for req_name in &self.requirement_order {
                if let Some(req) = self.requirements.get_mut(req_name)
                    && req.classes.iter().any(|c| c == id)
                {
                    req.css_styles.extend(
                        styles
                            .iter()
                            .flat_map(|s| s.split(','))
                            .map(|s| s.to_string()),
                    );
                }
            }
            for el_name in &self.element_order {
                if let Some(el) = self.elements.get_mut(el_name)
                    && el.classes.iter().any(|c| c == id)
                {
                    el.css_styles.extend(
                        styles
                            .iter()
                            .flat_map(|s| s.split(','))
                            .map(|s| s.to_string()),
                    );
                }
            }
        }
    }

    fn to_render_model(
        &self,
        acc_title: Option<String>,
        acc_descr: Option<String>,
    ) -> RequirementDiagramRenderModel {
        let requirements = self
            .requirement_order
            .iter()
            .filter_map(|k| self.requirements.get(k))
            .cloned()
            .collect::<Vec<_>>();

        let elements = self
            .element_order
            .iter()
            .filter_map(|k| self.elements.get(k))
            .cloned()
            .collect::<Vec<_>>();

        let mut classes = BTreeMap::new();
        for (k, c) in &self.classes {
            classes.insert(k.clone(), c.clone());
        }

        RequirementDiagramRenderModel {
            acc_title,
            acc_descr,
            direction: self.direction.clone(),
            requirements,
            elements,
            relationships: self.relations.clone(),
            classes,
        }
    }
}

fn push_styles(out: &mut Vec<String>, styles: &[String]) {
    for s in styles {
        if s.contains(',') {
            out.extend(s.split(',').map(|p| p.to_string()));
        } else {
            out.push(s.to_string());
        }
    }
}

pub fn parse_requirement(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let model = parse_requirement_semantic_source(code, meta)?.model;
    render_model_to_compat_json(&model, meta)
}

pub fn parse_requirement_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<RequirementDiagramRenderModel> {
    Ok(parse_requirement_semantic_source(code, meta)?.model)
}

pub(crate) fn parse_requirement_json_and_editor_facts(
    code: &str,
    meta: &ParseMetadata,
) -> Result<(Value, EditorSemanticFacts)> {
    let RequirementSemanticSource {
        model,
        editor_facts,
    } = parse_requirement_semantic_source(code, meta)?;
    Ok((render_model_to_compat_json(&model, meta)?, editor_facts))
}

struct RequirementSemanticSource {
    model: RequirementDiagramRenderModel,
    editor_facts: EditorSemanticFacts,
}

struct RequirementSemanticFailure {
    error: Box<Error>,
    editor_facts: Box<EditorSemanticFacts>,
}

impl RequirementSemanticFailure {
    fn into_editor_facts(mut self) -> EditorSemanticFacts {
        let (message, span) = match self.error.as_ref() {
            Error::DiagramParse { diagnostic, .. } => {
                (diagnostic.message().to_string(), diagnostic.span())
            }
            error => (error.to_string(), None),
        };
        self.editor_facts.mark_recovered_from_parse_error(
            format!("requirement parser recovered after parse error: {message}"),
            span,
        );
        *self.editor_facts
    }
}

#[derive(Debug, Clone, Copy)]
struct SpannedValue<'a> {
    text: &'a str,
    start: usize,
}

fn trim_spanned_value(raw: &str, raw_start: usize) -> Option<SpannedValue<'_>> {
    let leading = raw.len() - raw.trim_start().len();
    let without_leading = &raw[leading..];
    let trailing = without_leading.len() - without_leading.trim_end().len();
    let end = raw.len().saturating_sub(trailing);
    if leading >= end {
        return None;
    }
    Some(SpannedValue {
        text: &raw[leading..end],
        start: raw_start + leading,
    })
}

fn parse_keyword_rest_ci<'a>(line: &'a str, key: &str) -> Option<(&'a str, usize)> {
    let leading = line.len() - line.trim_start().len();
    let t = &line[leading..];
    if !t
        .get(..key.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(key))
    {
        return None;
    }
    let rest_start = leading + key.len();
    let rest = &line[rest_start..];
    let rest_leading = rest.len() - rest.trim_start().len();
    let rest_start = rest_start + rest_leading;
    let rest = &line[rest_start..];
    if rest.starts_with(':') || rest.starts_with('{') {
        Some((rest, rest_start))
    } else {
        None
    }
}

pub fn parse_requirement_editor_facts(code: &str, _meta: &ParseMetadata) -> EditorSemanticFacts {
    match construct_requirement_semantic_source(code, _meta) {
        Ok(source) => source.editor_facts,
        Err(failure) => failure.into_editor_facts(),
    }
}

fn push_requirement_payload_fact(
    facts: &mut EditorSemanticFacts,
    text: &str,
    start: usize,
    detail: &'static str,
    kind: EditorSemanticKind,
) {
    let text = text.trim();
    if text.is_empty() {
        return;
    }
    let span = SourceSpan::new(start, start + text.len());
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::Payload,
        span,
    ));
    facts.push_symbol(EditorSemanticSymbol::payload(
        text.to_string(),
        Some(detail.to_string()),
        kind,
        span,
        span,
    ));
}

fn push_requirement_class_refs(
    facts: &mut EditorSemanticFacts,
    line: &str,
    line_start: usize,
    classes: &[String],
    detail: &'static str,
) {
    push_requirement_class_refs_from(facts, line, line_start, 0, classes, detail);
}

fn push_requirement_class_refs_from(
    facts: &mut EditorSemanticFacts,
    line: &str,
    line_start: usize,
    search_start: usize,
    classes: &[String],
    detail: &'static str,
) {
    let mut cursor = search_start.min(line.len());
    for class_name in classes {
        if class_name.is_empty() {
            continue;
        }
        let Some(rel_from_cursor) = line
            .get(cursor..)
            .and_then(|suffix| suffix.find(class_name))
        else {
            continue;
        };
        let rel = cursor + rel_from_cursor;
        cursor = rel + class_name.len();
        let span = SourceSpan::new(line_start + rel, line_start + rel + class_name.len());
        facts.push_symbol(EditorSemanticSymbol::payload(
            class_name.clone(),
            Some(detail.to_string()),
            EditorSemanticKind::Property,
            span,
            span,
        ));
    }
}

struct RequirementIdSymbols<'a> {
    line: &'a str,
    line_start: usize,
    search_start: usize,
    ids: &'a [String],
    detail: &'static str,
    kind: EditorSemanticKind,
    role: EditorSemanticRole,
}

fn push_requirement_id_symbols(facts: &mut EditorSemanticFacts, request: RequirementIdSymbols<'_>) {
    let RequirementIdSymbols {
        line,
        line_start,
        search_start,
        ids,
        detail,
        kind,
        role,
    } = request;
    let mut cursor = search_start.min(line.len());
    for id in ids {
        if id.is_empty() {
            continue;
        }
        let Some(rel_from_cursor) = line.get(cursor..).and_then(|suffix| suffix.find(id)) else {
            continue;
        };
        let rel = cursor + rel_from_cursor;
        cursor = rel + id.len();
        let span = SourceSpan::new(line_start + rel, line_start + rel + id.len());
        let whole_line_span = SourceSpan::new(line_start, line_start + line.len());
        let symbol = match role {
            EditorSemanticRole::Entity => EditorSemanticSymbol::new(
                id.clone(),
                Some(detail.to_string()),
                kind,
                whole_line_span,
                span,
            ),
            EditorSemanticRole::Outline => EditorSemanticSymbol::outline(
                id.clone(),
                Some(detail.to_string()),
                kind,
                whole_line_span,
                span,
            ),
            EditorSemanticRole::Payload => EditorSemanticSymbol::payload(
                id.clone(),
                Some(detail.to_string()),
                kind,
                whole_line_span,
                span,
            ),
        };
        facts.push_symbol(symbol);
    }
}

fn line_id_list_end(line: &str, search_start: usize, ids: &[String]) -> usize {
    let mut cursor = search_start.min(line.len());
    for id in ids {
        if id.is_empty() {
            continue;
        }
        let Some(rel_from_cursor) = line.get(cursor..).and_then(|suffix| suffix.find(id)) else {
            continue;
        };
        cursor += rel_from_cursor + id.len();
    }
    cursor
}

fn push_requirement_style_refs(
    facts: &mut EditorSemanticFacts,
    line: &str,
    line_start: usize,
    styles: &[String],
    detail: &'static str,
) {
    for style in styles {
        if let Some(rel) = line.find(style) {
            let span = SourceSpan::new(line_start + rel, line_start + rel + style.len());
            facts.push_symbol(EditorSemanticSymbol::payload(
                style.clone(),
                Some(detail.to_string()),
                EditorSemanticKind::Property,
                span,
                span,
            ));
        }
    }
}

pub(crate) fn render_model_to_compat_json(
    model: &RequirementDiagramRenderModel,
    meta: &ParseMetadata,
) -> Result<Value> {
    let mut out = Map::with_capacity(9);
    out.insert("type".to_string(), Value::String(meta.diagram_type.clone()));
    out.insert("accTitle".to_string(), json!(&model.acc_title));
    out.insert("accDescr".to_string(), json!(&model.acc_descr));
    out.insert("direction".to_string(), json!(&model.direction));
    out.insert("requirements".to_string(), json!(&model.requirements));
    out.insert("elements".to_string(), json!(&model.elements));
    out.insert("relationships".to_string(), json!(&model.relationships));
    out.insert("classes".to_string(), json!(&model.classes));
    out.insert(
        "config".to_string(),
        crate::config::clone_value_nonrecursive(meta.effective_config.as_value()),
    );
    Ok(Value::Object(out))
}

fn parse_requirement_semantic_source(
    code: &str,
    meta: &ParseMetadata,
) -> Result<RequirementSemanticSource> {
    construct_requirement_semantic_source(code, meta).map_err(|failure| *failure.error)
}

fn construct_requirement_semantic_source(
    code: &str,
    meta: &ParseMetadata,
) -> std::result::Result<RequirementSemanticSource, RequirementSemanticFailure> {
    #[cfg(test)]
    REQUIREMENT_SYNTAX_CONSTRUCTION_COUNT.set(REQUIREMENT_SYNTAX_CONSTRUCTION_COUNT.get() + 1);

    let mut db = RequirementDb::new();
    let mut acc_title: Option<String> = None;
    let mut acc_descr: Option<String> = None;
    let mut editor_facts = EditorSemanticFacts::new();
    let mut lines = LineCursor::new(code);
    let mut saw_header = false;
    let mut first_error = None;

    while let Some((raw, line_start)) = lines.next_line() {
        let stripped = strip_inline_comment(raw);
        let t = stripped.trim();
        if t.is_empty() {
            continue;
        }

        if let Some((rest, rest_start)) = parse_keyword_rest_ci(&stripped, "accTitle")
            && let Some(raw_value) = rest.strip_prefix(':')
        {
            let value = trim_spanned_value(raw_value, rest_start + 1);
            editor_facts.push_directive_prefix("accTitle");
            if let Some(value) = value {
                push_requirement_payload_fact(
                    &mut editor_facts,
                    value.text,
                    line_start + value.start,
                    "requirement accessibility title",
                    EditorSemanticKind::String,
                );
            }
            acc_title = Some(
                value
                    .map(|value| value.text)
                    .unwrap_or_default()
                    .to_string(),
            );
            continue;
        }
        if let Some(parsed) = parse_requirement_acc_descr(&stripped, raw, line_start, &mut lines) {
            editor_facts.push_directive_prefix("accDescr");
            parsed.emit_editor_fact(&mut editor_facts);
            if !parsed.complete {
                editor_facts.mark_recovered_from_parse_error(
                    "requirement parser recovered from unterminated accDescr block",
                    Some(parsed.statement_span),
                );
            }
            acc_descr = Some(parsed.value);
            continue;
        }

        if !saw_header {
            if t.eq_ignore_ascii_case("requirementDiagram") {
                saw_header = true;
                continue;
            }
            first_error.get_or_insert(Error::diagram_parse_exact(
                meta.diagram_type.clone(),
                "expected requirementDiagram",
                requirement_statement_span(raw, line_start),
            ));
            continue;
        }

        if let Some(dir) = parse_direction(t) {
            emit_requirement_direction(&mut editor_facts, raw, line_start, dir);
            db.set_direction(dir);
            continue;
        }

        let requirement_open = match parse_requirement_def_open(t) {
            Ok(open) => open,
            Err(error) => {
                first_error.get_or_insert(requirement_exact_error(
                    error,
                    meta,
                    requirement_statement_span(raw, line_start),
                ));
                continue;
            }
        };
        if let Some(RequirementDefOpen {
            name,
            name_start,
            requirement_type: ty,
            classes,
        }) = requirement_open
        {
            let name_selection_start =
                requirement_statement_span(raw, line_start).start + name_start;
            emit_requirement_definition(
                &mut editor_facts,
                RequirementDefinitionSymbols {
                    line: raw,
                    line_start,
                    name: &name,
                    selection: SourceSpan::new(
                        name_selection_start,
                        name_selection_start + name.len(),
                    ),
                    detail: &ty.to_lowercase(),
                    kind: EditorSemanticKind::Struct,
                    classes: classes.as_deref(),
                },
            );
            let body = parse_requirement_body(&mut lines, meta, &mut editor_facts);
            if let Some(error) = body.error {
                first_error.get_or_insert(error);
            }
            db.add_requirement(&name, &ty, body.value);
            if let Some(classes) = classes {
                db.set_class(&[name], &classes);
            }
            continue;
        }

        let element_open = match parse_element_def_open(t) {
            Ok(open) => open,
            Err(error) => {
                first_error.get_or_insert(requirement_exact_error(
                    error,
                    meta,
                    requirement_statement_span(raw, line_start),
                ));
                continue;
            }
        };
        if let Some(ElementDefOpen {
            name,
            name_start,
            classes,
        }) = element_open
        {
            let name_selection_start =
                requirement_statement_span(raw, line_start).start + name_start;
            emit_requirement_definition(
                &mut editor_facts,
                RequirementDefinitionSymbols {
                    line: raw,
                    line_start,
                    name: &name,
                    selection: SourceSpan::new(
                        name_selection_start,
                        name_selection_start + name.len(),
                    ),
                    detail: "requirement element",
                    kind: EditorSemanticKind::Object,
                    classes: classes.as_deref(),
                },
            );
            let body = parse_element_body(&mut lines, meta, &mut editor_facts);
            if let Some(error) = body.error {
                first_error.get_or_insert(error);
            }
            db.add_element(&name, body.value);
            if let Some(classes) = classes {
                db.set_class(&[name], &classes);
            }
            continue;
        }

        let shorthand = match parse_shorthand_class_stmt(t) {
            Ok(statement) => statement,
            Err(error) => {
                first_error.get_or_insert(requirement_exact_error(
                    error,
                    meta,
                    requirement_statement_span(raw, line_start),
                ));
                continue;
            }
        };
        if let Some((target, classes)) = shorthand {
            emit_requirement_shorthand_class(&mut editor_facts, raw, line_start, &target, &classes);
            db.set_class(&[target], &classes);
            continue;
        }

        let style = match parse_style_stmt(t) {
            Ok(statement) => statement,
            Err(error) => {
                first_error.get_or_insert(requirement_exact_error(
                    error,
                    meta,
                    requirement_statement_span(raw, line_start),
                ));
                continue;
            }
        };
        if let Some((ids, styles)) = style {
            emit_requirement_style(&mut editor_facts, raw, line_start, &ids, &styles);
            db.set_css_style(&ids, &styles);
            continue;
        }

        let class_def = match parse_classdef_stmt(t) {
            Ok(statement) => statement,
            Err(error) => {
                first_error.get_or_insert(requirement_exact_error(
                    error,
                    meta,
                    requirement_statement_span(raw, line_start),
                ));
                continue;
            }
        };
        if let Some((ids, styles)) = class_def {
            emit_requirement_class_def(&mut editor_facts, raw, line_start, &ids, &styles);
            db.define_class(&ids, &styles);
            continue;
        }

        let class = match parse_class_stmt(t) {
            Ok(statement) => statement,
            Err(error) => {
                first_error.get_or_insert(requirement_exact_error(
                    error,
                    meta,
                    requirement_statement_span(raw, line_start),
                ));
                continue;
            }
        };
        if let Some((ids, classes)) = class {
            emit_requirement_class(&mut editor_facts, raw, line_start, &ids, &classes);
            db.set_class(&ids, &classes);
            continue;
        }

        let relationship = match parse_relationship_stmt(t) {
            Ok(statement) => statement,
            Err(error) => {
                first_error.get_or_insert(requirement_exact_error(
                    error,
                    meta,
                    requirement_statement_span(raw, line_start),
                ));
                continue;
            }
        };
        if let Some((rel, src, dst)) = relationship {
            emit_requirement_relationship(&mut editor_facts, raw, line_start, &rel, &src, &dst);
            db.add_relationship(&rel, &src, &dst);
            continue;
        }

        first_error.get_or_insert(Error::diagram_parse_exact(
            meta.diagram_type.clone(),
            format!("unexpected requirement statement: {t}"),
            requirement_statement_span(raw, line_start),
        ));
    }

    if !saw_header {
        first_error.get_or_insert(Error::diagram_parse_insertion_point(
            meta.diagram_type.clone(),
            "expected requirementDiagram",
            code.len(),
        ));
    }

    if let Some(error) = first_error {
        return Err(requirement_failure(error, editor_facts));
    }

    Ok(RequirementSemanticSource {
        model: db.to_render_model(acc_title, acc_descr),
        editor_facts,
    })
}

fn requirement_failure(
    error: Error,
    editor_facts: EditorSemanticFacts,
) -> RequirementSemanticFailure {
    RequirementSemanticFailure {
        error: Box::new(error),
        editor_facts: Box::new(editor_facts),
    }
}

fn requirement_exact_error(error: Error, meta: &ParseMetadata, span: SourceSpan) -> Error {
    let message = match error {
        Error::DiagramParse { diagnostic, .. } => diagnostic.message().to_string(),
        error => error.to_string(),
    };
    Error::diagram_parse_exact(meta.diagram_type.clone(), message, span)
}

fn requirement_statement_span(line: &str, line_start: usize) -> SourceSpan {
    let leading = leading_whitespace_len(line);
    let end = line.trim_end().len();
    SourceSpan::new(line_start + leading, line_start + end.max(leading))
}

struct RequirementAccDescr {
    value: String,
    statement_span: SourceSpan,
    selection: Option<SourceSpan>,
    complete: bool,
}

impl RequirementAccDescr {
    fn emit_editor_fact(&self, facts: &mut EditorSemanticFacts) {
        let Some(selection) = self.selection else {
            return;
        };
        facts.push_expected_syntax(EditorExpectedSyntax::new(
            EditorExpectedSyntaxKind::Payload,
            selection,
        ));
        facts.push_symbol(EditorSemanticSymbol::payload(
            self.value.clone(),
            Some("requirement accessibility description".to_string()),
            EditorSemanticKind::String,
            self.statement_span,
            selection,
        ));
    }
}

fn parse_requirement_acc_descr(
    line: &str,
    raw_line: &str,
    line_start: usize,
    cursor: &mut LineCursor<'_>,
) -> Option<RequirementAccDescr> {
    let (rest, rest_start) = parse_keyword_rest_ci(line, "accDescr")?;
    let statement_start = requirement_statement_span(raw_line, line_start).start;

    if let Some(raw_value) = rest.strip_prefix(':') {
        let value = trim_spanned_value(raw_value, rest_start + 1);
        return Some(RequirementAccDescr {
            value: value
                .map(|value| value.text)
                .unwrap_or_default()
                .to_string(),
            statement_span: requirement_statement_span(raw_line, line_start),
            selection: value.map(|value| {
                SourceSpan::new(
                    line_start + value.start,
                    line_start + value.start + value.text.len(),
                )
            }),
            complete: true,
        });
    }

    let after_brace = rest.strip_prefix('{')?;
    let mut value_lines = Vec::new();
    let mut first_content_start = None;
    let mut last_content_end = None;
    let mut statement_end = line_start + raw_line.len();
    let mut complete = false;

    let first_start = line_start + rest_start + 1;
    if let Some(close) = after_brace.find('}') {
        append_requirement_acc_descr_line(
            &after_brace[..close],
            first_start,
            &mut value_lines,
            &mut first_content_start,
            &mut last_content_end,
        );
        statement_end = first_start + close + 1;
        complete = true;
    } else {
        append_requirement_acc_descr_line(
            after_brace,
            first_start,
            &mut value_lines,
            &mut first_content_start,
            &mut last_content_end,
        );
        while let Some((next, next_start)) = cursor.next_line() {
            statement_end = next_start + next.len();
            if let Some(close) = next.find('}') {
                append_requirement_acc_descr_line(
                    &next[..close],
                    next_start,
                    &mut value_lines,
                    &mut first_content_start,
                    &mut last_content_end,
                );
                statement_end = next_start + close + 1;
                complete = true;
                break;
            }
            append_requirement_acc_descr_line(
                next,
                next_start,
                &mut value_lines,
                &mut first_content_start,
                &mut last_content_end,
            );
        }
    }

    Some(RequirementAccDescr {
        value: value_lines.join("\n").trim().to_string(),
        statement_span: SourceSpan::new(statement_start, statement_end),
        selection: first_content_start
            .zip(last_content_end)
            .map(|(start, end)| SourceSpan::new(start, end)),
        complete,
    })
}

fn append_requirement_acc_descr_line(
    raw: &str,
    start: usize,
    lines: &mut Vec<String>,
    first_content_start: &mut Option<usize>,
    last_content_end: &mut Option<usize>,
) {
    let leading = leading_whitespace_len(raw);
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        if !lines.is_empty() {
            lines.push(String::new());
        }
        return;
    }
    *first_content_start = Some(first_content_start.unwrap_or(start + leading));
    *last_content_end = Some(start + raw.trim_end().len());
    lines.push(trimmed.to_string());
}

fn emit_requirement_direction(
    facts: &mut EditorSemanticFacts,
    line: &str,
    line_start: usize,
    direction: &str,
) {
    let rel = line.find(direction).unwrap_or(0);
    let selection = SourceSpan::new(line_start + rel, line_start + rel + direction.len());
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::DirectionValue,
        selection,
    ));
    facts.push_symbol(EditorSemanticSymbol::payload(
        direction,
        Some("requirement direction".to_string()),
        EditorSemanticKind::String,
        requirement_statement_span(line, line_start),
        selection,
    ));
}

struct RequirementDefinitionSymbols<'a> {
    line: &'a str,
    line_start: usize,
    name: &'a str,
    selection: SourceSpan,
    detail: &'a str,
    kind: EditorSemanticKind,
    classes: Option<&'a [String]>,
}

fn emit_requirement_definition(
    facts: &mut EditorSemanticFacts,
    definition: RequirementDefinitionSymbols<'_>,
) {
    let RequirementDefinitionSymbols {
        line,
        line_start,
        name,
        selection,
        detail,
        kind,
        classes,
    } = definition;
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::NodeIdentifier,
        selection,
    ));
    facts.push_symbol(EditorSemanticSymbol::new(
        name,
        Some(detail.to_string()),
        kind,
        requirement_statement_span(line, line_start),
        selection,
    ));
    if let Some(classes) = classes {
        push_requirement_class_refs(facts, line, line_start, classes, "requirement class");
    }
}

fn emit_requirement_shorthand_class(
    facts: &mut EditorSemanticFacts,
    line: &str,
    line_start: usize,
    target: &str,
    classes: &[String],
) {
    if let Some(rel) = line.find(target) {
        facts.push_symbol(EditorSemanticSymbol::outline(
            target,
            Some("requirement class target".to_string()),
            EditorSemanticKind::Namespace,
            requirement_statement_span(line, line_start),
            SourceSpan::new(line_start + rel, line_start + rel + target.len()),
        ));
    }
    push_requirement_class_refs(facts, line, line_start, classes, "requirement class");
}

fn emit_requirement_style(
    facts: &mut EditorSemanticFacts,
    line: &str,
    line_start: usize,
    ids: &[String],
    styles: &[String],
) {
    let directive_start = leading_whitespace_len(line);
    push_requirement_id_symbols(
        facts,
        RequirementIdSymbols {
            line,
            line_start,
            search_start: directive_start + "style".len(),
            ids,
            detail: "requirement style target",
            kind: EditorSemanticKind::Property,
            role: EditorSemanticRole::Payload,
        },
    );
    push_requirement_style_refs(facts, line, line_start, styles, "requirement style");
}

fn emit_requirement_class_def(
    facts: &mut EditorSemanticFacts,
    line: &str,
    line_start: usize,
    ids: &[String],
    styles: &[String],
) {
    let directive_start = leading_whitespace_len(line);
    push_requirement_id_symbols(
        facts,
        RequirementIdSymbols {
            line,
            line_start,
            search_start: directive_start + "classDef".len(),
            ids,
            detail: "requirement class definition",
            kind: EditorSemanticKind::Property,
            role: EditorSemanticRole::Outline,
        },
    );
    push_requirement_style_refs(facts, line, line_start, styles, "requirement class style");
}

fn emit_requirement_class(
    facts: &mut EditorSemanticFacts,
    line: &str,
    line_start: usize,
    ids: &[String],
    classes: &[String],
) {
    let directive_start = leading_whitespace_len(line);
    let targets_start = directive_start + "class".len();
    let class_refs_start = line_id_list_end(line, targets_start, ids);
    push_requirement_class_refs_from(
        facts,
        line,
        line_start,
        class_refs_start,
        classes,
        "requirement class",
    );
    push_requirement_id_symbols(
        facts,
        RequirementIdSymbols {
            line,
            line_start,
            search_start: targets_start,
            ids,
            detail: "requirement class target",
            kind: EditorSemanticKind::Namespace,
            role: EditorSemanticRole::Entity,
        },
    );
}

fn emit_requirement_relationship(
    facts: &mut EditorSemanticFacts,
    line: &str,
    line_start: usize,
    relationship: &str,
    source: &str,
    target: &str,
) {
    let statement = requirement_statement_span(line, line_start);
    if let Some(rel) = line.find(relationship) {
        facts.push_symbol(EditorSemanticSymbol::payload(
            relationship,
            Some("requirement relationship".to_string()),
            EditorSemanticKind::String,
            statement,
            SourceSpan::new(line_start + rel, line_start + rel + relationship.len()),
        ));
    }
    if let Some(rel) = line.find(source) {
        facts.push_symbol(EditorSemanticSymbol::new(
            source,
            Some("requirement relationship source".to_string()),
            EditorSemanticKind::Struct,
            statement,
            SourceSpan::new(line_start + rel, line_start + rel + source.len()),
        ));
    }
    if let Some(rel) = line.rfind(target) {
        facts.push_symbol(EditorSemanticSymbol::new(
            target,
            Some("requirement relationship target".to_string()),
            EditorSemanticKind::Struct,
            statement,
            SourceSpan::new(line_start + rel, line_start + rel + target.len()),
        ));
    }
}

fn strip_inline_comment(line: &str) -> String {
    let lowered = line.trim_start().to_ascii_lowercase();
    if lowered.starts_with("style")
        || lowered.starts_with("classdef")
        || lowered.starts_with("class ")
        || lowered == "class"
    {
        return line.to_string();
    }

    let mut in_quotes = false;
    let mut idx = 0usize;
    let bytes = line.as_bytes();
    while idx < bytes.len() {
        let b = bytes[idx];
        if b == b'"' {
            in_quotes = !in_quotes;
            idx += 1;
            continue;
        }
        if !in_quotes {
            if b == b'#' {
                return line[..idx].to_string();
            }
            if b == b'%' && idx + 1 < bytes.len() && bytes[idx + 1] == b'%' {
                return line[..idx].to_string();
            }
        }
        idx += 1;
    }
    line.to_string()
}

fn parse_direction(t: &str) -> Option<&'static str> {
    let (keyword, rest) = split_first_word(t)?;
    if !keyword.eq_ignore_ascii_case("direction") {
        return None;
    }
    let (dir, _) = split_first_word(rest)?;
    match dir.to_ascii_uppercase().as_str() {
        "TB" => Some("TB"),
        "BT" => Some("BT"),
        "LR" => Some("LR"),
        "RL" => Some("RL"),
        _ => None,
    }
}

fn parse_requirement_def_open(t: &str) -> Result<Option<RequirementDefOpen>> {
    let t = t.trim();
    if !t.ends_with('{') {
        return Ok(None);
    }

    let without_brace = t[..t.len() - 1].trim_end();
    let (ty_raw, rest) = split_first_word(without_brace).ok_or_else(|| {
        Error::diagram_parse_fallback(
            "requirement".to_string(),
            "invalid requirement definition".to_string(),
        )
    })?;

    let requirement_type = match ty_raw.to_ascii_lowercase().as_str() {
        "requirement" => "Requirement",
        "functionalrequirement" => "Functional Requirement",
        "interfacerequirement" => "Interface Requirement",
        "performancerequirement" => "Performance Requirement",
        "physicalrequirement" => "Physical Requirement",
        "designconstraint" => "Design Constraint",
        _ => return Ok(None),
    }
    .to_string();

    let rest_start = without_brace.len() - rest.len();
    let rest_leading = leading_whitespace_len(rest);
    let name_and_classes = split_name_and_classes(rest.trim())?;
    let name = name_and_classes.name;
    if name.is_empty() {
        return Err(Error::diagram_parse_fallback(
            "requirement".to_string(),
            "requirement name is empty".to_string(),
        ));
    }
    Ok(Some(RequirementDefOpen {
        name,
        name_start: rest_start + rest_leading + name_and_classes.name_start,
        requirement_type,
        classes: name_and_classes.classes,
    }))
}

struct RequirementDefOpen {
    name: String,
    name_start: usize,
    requirement_type: String,
    classes: Option<Vec<String>>,
}

fn parse_element_def_open(t: &str) -> Result<Option<ElementDefOpen>> {
    let t = t.trim();
    if !t.ends_with('{') {
        return Ok(None);
    }

    let without_brace = t[..t.len() - 1].trim_end();
    let (kw, rest) = split_first_word(without_brace).ok_or_else(|| {
        Error::diagram_parse_fallback(
            "requirement".to_string(),
            "invalid element definition".to_string(),
        )
    })?;
    if !kw.eq_ignore_ascii_case("element") {
        return Ok(None);
    }

    let rest_start = without_brace.len() - rest.len();
    let rest_leading = leading_whitespace_len(rest);
    let name_and_classes = split_name_and_classes(rest.trim())?;
    let name = name_and_classes.name;
    if name.is_empty() {
        return Err(Error::diagram_parse_fallback(
            "requirement".to_string(),
            "element name is empty".to_string(),
        ));
    }
    Ok(Some(ElementDefOpen {
        name,
        name_start: rest_start + rest_leading + name_and_classes.name_start,
        classes: name_and_classes.classes,
    }))
}

struct ElementDefOpen {
    name: String,
    name_start: usize,
    classes: Option<Vec<String>>,
}

fn split_first_word(input: &str) -> Option<(&str, &str)> {
    let input = input.trim_start();
    if input.is_empty() {
        return None;
    }
    let mut iter = input.splitn(2, char::is_whitespace);
    let first = iter.next()?;
    let rest = iter.next().unwrap_or("");
    Some((first, rest))
}

fn split_name_and_classes(input: &str) -> Result<DefinitionName> {
    let leading = leading_whitespace_len(input);
    let input = input.trim();
    if input.is_empty() {
        return Ok(DefinitionName {
            name: String::new(),
            name_start: leading,
            classes: None,
        });
    }

    if let Some(pos) = input.find(":::") {
        let name_raw = input[..pos].trim_end();
        let classes_raw = input[pos + 3..].trim();
        let (name, _) = parse_id_or_name(name_raw)?;
        let classes = parse_id_list_all(classes_raw)?;
        let quoted_offset = usize::from(name_raw.trim_start().starts_with('"'));
        return Ok(DefinitionName {
            name,
            name_start: leading + leading_whitespace_len(name_raw) + quoted_offset,
            classes: Some(classes),
        });
    }

    let (name, _) = parse_id_or_name(input)?;
    let quoted_offset = usize::from(input.starts_with('"'));
    Ok(DefinitionName {
        name,
        name_start: leading + quoted_offset,
        classes: None,
    })
}

struct DefinitionName {
    name: String,
    name_start: usize,
    classes: Option<Vec<String>>,
}

fn parse_id_or_name(input: &str) -> Result<(String, &str)> {
    let input = input.trim_start();
    if input.starts_with('"') {
        if let Some((val, rest)) = parse_quoted_prefix(input) {
            return Ok((val, rest));
        }
        return Err(Error::diagram_parse_fallback(
            "requirement".to_string(),
            "unterminated string".to_string(),
        ));
    }
    Ok((input.trim().to_string(), ""))
}

fn parse_quoted_prefix(input: &str) -> Option<(String, &str)> {
    let mut chars = input.chars();
    if chars.next()? != '"' {
        return None;
    }
    let mut out = String::new();
    let mut idx = 1usize;
    for c in chars {
        idx += c.len_utf8();
        if c == '"' {
            return Some((out, &input[idx..]));
        }
        out.push(c);
    }
    None
}

struct RequirementBodyParse<T> {
    value: T,
    error: Option<Error>,
}

fn parse_requirement_body(
    lines: &mut LineCursor<'_>,
    meta: &ParseMetadata,
    facts: &mut EditorSemanticFacts,
) -> RequirementBodyParse<RequirementBuilder> {
    let mut b = RequirementBuilder::new();
    let mut error = None;
    while let Some((raw, line_start)) = lines.next_line() {
        let line = strip_inline_comment(raw);
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if t == "}" {
            return RequirementBodyParse { value: b, error };
        }

        let Some((k, value)) = split_key_value_spanned(&line) else {
            error.get_or_insert(Error::diagram_parse_exact(
                meta.diagram_type.clone(),
                format!("invalid requirement body line: {t}"),
                requirement_statement_span(raw, line_start),
            ));
            continue;
        };
        let key = k.to_ascii_lowercase();
        let value_span = SourceSpan::new(
            line_start + value.start,
            line_start + value.start + value.text.len(),
        );
        let detail = match key.as_str() {
            "id" => "requirement id",
            "text" => "requirement text",
            "risk" => "requirement risk",
            "verifymethod" => "requirement verify method",
            _ => {
                error.get_or_insert(Error::diagram_parse_exact(
                    meta.diagram_type.clone(),
                    format!("unexpected requirement body key: {k}"),
                    requirement_statement_span(raw, line_start),
                ));
                continue;
            }
        };
        push_requirement_payload_fact(
            facts,
            value.text,
            value_span.start,
            detail,
            EditorSemanticKind::String,
        );
        let parsed = match parse_simple_value(value.text) {
            Ok(parsed) => parsed,
            Err(parse_error) => {
                error.get_or_insert(requirement_exact_error(parse_error, meta, value_span));
                continue;
            }
        };
        match key.as_str() {
            "id" => b.requirement_id = parsed,
            "text" => b.text = parsed,
            "risk" => match normalize_risk(&parsed) {
                Ok(risk) => b.risk = risk,
                Err(parse_error) => {
                    error.get_or_insert(requirement_exact_error(parse_error, meta, value_span));
                }
            },
            "verifymethod" => match normalize_verify_method(&parsed) {
                Ok(method) => b.verify_method = method,
                Err(parse_error) => {
                    error.get_or_insert(requirement_exact_error(parse_error, meta, value_span));
                }
            },
            _ => unreachable!("body key was validated above"),
        }
    }

    error.get_or_insert(Error::diagram_parse_insertion_point(
        meta.diagram_type.clone(),
        "unterminated requirement block",
        lines.offset(),
    ));
    RequirementBodyParse { value: b, error }
}

fn parse_element_body(
    lines: &mut LineCursor<'_>,
    meta: &ParseMetadata,
    facts: &mut EditorSemanticFacts,
) -> RequirementBodyParse<ElementBuilder> {
    let mut b = ElementBuilder::new();
    let mut error = None;
    while let Some((raw, line_start)) = lines.next_line() {
        let line = strip_inline_comment(raw);
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if t == "}" {
            return RequirementBodyParse { value: b, error };
        }

        let Some((k, value)) = split_key_value_spanned(&line) else {
            error.get_or_insert(Error::diagram_parse_exact(
                meta.diagram_type.clone(),
                format!("invalid element body line: {t}"),
                requirement_statement_span(raw, line_start),
            ));
            continue;
        };
        let key = k.to_ascii_lowercase();
        let value_span = SourceSpan::new(
            line_start + value.start,
            line_start + value.start + value.text.len(),
        );
        let detail = match key.as_str() {
            "type" => "requirement element type",
            "docref" => "requirement doc ref",
            _ => {
                error.get_or_insert(Error::diagram_parse_exact(
                    meta.diagram_type.clone(),
                    format!("unexpected element body key: {k}"),
                    requirement_statement_span(raw, line_start),
                ));
                continue;
            }
        };
        push_requirement_payload_fact(
            facts,
            value.text,
            value_span.start,
            detail,
            EditorSemanticKind::String,
        );
        let parsed = match parse_simple_value(value.text) {
            Ok(parsed) => parsed,
            Err(parse_error) => {
                error.get_or_insert(requirement_exact_error(parse_error, meta, value_span));
                continue;
            }
        };
        match key.as_str() {
            "type" => b.element_type = parsed,
            "docref" => b.doc_ref = parsed,
            _ => unreachable!("element key was validated above"),
        }
    }

    error.get_or_insert(Error::diagram_parse_insertion_point(
        meta.diagram_type.clone(),
        "unterminated element block",
        lines.offset(),
    ));
    RequirementBodyParse { value: b, error }
}

fn split_key_value_spanned(input: &str) -> Option<(&str, SpannedValue<'_>)> {
    let idx = input.find(':')?;
    let key = input[..idx].trim();
    let value = trim_spanned_value(&input[idx + 1..], idx + 1)?;
    if key.is_empty() {
        return None;
    }
    Some((key, value))
}

fn parse_simple_value(input: &str) -> Result<String> {
    let input = input.trim();
    if input.starts_with('"') {
        if let Some((val, rest)) = parse_quoted_prefix(input) {
            if !rest.trim().is_empty() {
                return Err(Error::diagram_parse_fallback(
                    "requirement".to_string(),
                    format!("unexpected trailing tokens after string: {}", rest.trim()),
                ));
            }
            return Ok(val.trim().to_string());
        }
        return Err(Error::diagram_parse_fallback(
            "requirement".to_string(),
            "unterminated string".to_string(),
        ));
    }
    Ok(input.trim().to_string())
}

fn normalize_risk(input: &str) -> Result<String> {
    match input.trim().to_ascii_lowercase().as_str() {
        "low" => Ok("Low".to_string()),
        "medium" => Ok("Medium".to_string()),
        "high" => Ok("High".to_string()),
        other => Err(Error::diagram_parse_fallback(
            "requirement".to_string(),
            format!("invalid risk level: {other}"),
        )),
    }
}

fn normalize_verify_method(input: &str) -> Result<String> {
    match input.trim().to_ascii_lowercase().as_str() {
        "analysis" => Ok("Analysis".to_string()),
        "demonstration" => Ok("Demonstration".to_string()),
        "inspection" => Ok("Inspection".to_string()),
        "test" => Ok("Test".to_string()),
        other => Err(Error::diagram_parse_fallback(
            "requirement".to_string(),
            format!("invalid verify method: {other}"),
        )),
    }
}

fn parse_shorthand_class_stmt(t: &str) -> Result<Option<(String, Vec<String>)>> {
    let t = t.trim();
    if t.is_empty() || t.ends_with('{') {
        return Ok(None);
    }
    let Some(pos) = t.find(":::") else {
        return Ok(None);
    };
    let left = t[..pos].trim_end();
    let right = t[pos + 3..].trim_start();
    if left.is_empty() || right.is_empty() {
        return Err(Error::diagram_parse_fallback(
            "requirement".to_string(),
            format!("invalid class shorthand statement: {t}"),
        ));
    }
    let (target, _) = parse_id_or_name(left)?;
    let classes = parse_id_list_all(right)?;
    Ok(Some((target, classes)))
}

fn parse_style_stmt(t: &str) -> Result<Option<(Vec<String>, Vec<String>)>> {
    let t = t.trim_start();
    let Some((keyword, rest)) = split_first_word(t) else {
        return Ok(None);
    };
    if !keyword.eq_ignore_ascii_case("style") {
        return Ok(None);
    }
    let rest = rest.trim_start();
    let (ids, styles_str) = split_list_and_rest(rest)?;
    let styles = split_csv(styles_str);
    if ids.is_empty() || styles.is_empty() {
        return Err(Error::diagram_parse_fallback(
            "requirement".to_string(),
            format!("invalid style statement: {t}"),
        ));
    }
    Ok(Some((ids, styles)))
}

fn parse_classdef_stmt(t: &str) -> Result<Option<(Vec<String>, Vec<String>)>> {
    let t = t.trim_start();
    let Some((keyword, rest)) = split_first_word(t) else {
        return Ok(None);
    };
    if !keyword.eq_ignore_ascii_case("classdef") {
        return Ok(None);
    }
    let rest = rest.trim_start();
    let (ids, styles_str) = split_list_and_rest(rest)?;
    let styles = split_csv(styles_str);
    if ids.is_empty() || styles.is_empty() {
        return Err(Error::diagram_parse_fallback(
            "requirement".to_string(),
            format!("invalid classDef statement: {t}"),
        ));
    }
    Ok(Some((ids, styles)))
}

fn parse_class_stmt(t: &str) -> Result<Option<(Vec<String>, Vec<String>)>> {
    let t = t.trim_start();
    let Some((keyword, rest)) = split_first_word(t) else {
        return Ok(None);
    };
    if !keyword.eq_ignore_ascii_case("class") {
        return Ok(None);
    }
    let rest = rest.trim_start();
    let (ids, classes_str) = split_list_and_rest(rest)?;
    let classes = parse_id_list_all(classes_str)?;
    if ids.is_empty() || classes.is_empty() {
        return Err(Error::diagram_parse_fallback(
            "requirement".to_string(),
            format!("invalid class statement: {t}"),
        ));
    }
    Ok(Some((ids, classes)))
}

fn parse_relationship_stmt(t: &str) -> Result<Option<(String, String, String)>> {
    let t = t.trim();
    if t.is_empty() {
        return Ok(None);
    }

    if let Some(pos) = t.find("<-") {
        let left = t[..pos].trim_end();
        let rest = t[pos + 2..].trim_start();
        let (rel, right) = split_once_dash(rest)?;
        let relationship = normalize_relationship(rel)?;
        if relationship.is_empty() {
            return Ok(None);
        }
        let src = parse_simple_value(right)?;
        let dst = parse_simple_value(left)?;
        return Ok(Some((relationship, src, dst)));
    }

    if let Some(pos) = t.find("->") {
        let right = t[pos + 2..].trim_start();
        let left_part = t[..pos].trim_end();
        let (src, rel) = split_once_dash(left_part)?;
        let relationship = normalize_relationship(rel)?;
        if relationship.is_empty() {
            return Ok(None);
        }
        let src = parse_simple_value(src)?;
        let dst = parse_simple_value(right)?;
        return Ok(Some((relationship, src, dst)));
    }

    Ok(None)
}

fn split_once_dash(input: &str) -> Result<(&str, &str)> {
    let Some(idx) = input.find('-') else {
        return Err(Error::diagram_parse_fallback(
            "requirement".to_string(),
            format!("invalid relationship statement: {input}"),
        ));
    };
    Ok((input[..idx].trim(), input[idx + 1..].trim()))
}

fn normalize_relationship(input: &str) -> Result<String> {
    let rel = input.trim().to_ascii_lowercase();
    match rel.as_str() {
        "contains" | "copies" | "derives" | "satisfies" | "verifies" | "refines" | "traces" => {
            Ok(rel)
        }
        _ => Ok(String::new()),
    }
}

fn split_list_and_rest(input: &str) -> Result<(Vec<String>, &str)> {
    let mut cur = input.trim_start();
    let mut items = Vec::new();

    loop {
        cur = cur.trim_start();
        if cur.is_empty() {
            break;
        }

        let (item, rest) = if cur.starts_with('"') {
            parse_quoted_prefix(cur).ok_or_else(|| {
                Error::diagram_parse_fallback(
                    "requirement".to_string(),
                    "unterminated string".to_string(),
                )
            })?
        } else {
            let mut end = 0usize;
            for (i, c) in cur.char_indices() {
                if c == ',' || c.is_whitespace() {
                    break;
                }
                end = i + c.len_utf8();
            }
            if end == 0 {
                return Err(Error::diagram_parse_fallback(
                    "requirement".to_string(),
                    "expected identifier".to_string(),
                ));
            }
            (cur[..end].to_string(), &cur[end..])
        };

        items.push(item);
        cur = rest.trim_start();
        if cur.starts_with(',') {
            cur = &cur[1..];
            continue;
        }
        break;
    }

    let rest = cur.trim_start();
    Ok((items, rest))
}

fn parse_id_list_all(input: &str) -> Result<Vec<String>> {
    let mut out = Vec::new();
    let mut cur = input.trim_start();
    while !cur.is_empty() {
        let (item, rest) = if cur.starts_with('"') {
            parse_quoted_prefix(cur).ok_or_else(|| {
                Error::diagram_parse_fallback(
                    "requirement".to_string(),
                    "unterminated string".to_string(),
                )
            })?
        } else {
            let mut end = cur.len();
            for (i, c) in cur.char_indices() {
                if c == ',' {
                    end = i;
                    break;
                }
            }
            (cur[..end].trim().to_string(), &cur[end..])
        };
        if !item.is_empty() {
            out.push(item);
        }
        cur = rest.trim_start();
        if cur.starts_with(',') {
            cur = &cur[1..];
            continue;
        }
        break;
    }
    Ok(out)
}

fn split_csv(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EditorSemanticCompleteness, Engine, MermaidConfig, ParseDiagnosticSpanKind, ParseOptions,
    };
    use futures::executor::block_on;
    use serde_json::json;

    fn parse(text: &str) -> Value {
        let engine = Engine::new();
        block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap()
            .model
    }

    fn parse_err(text: &str) -> String {
        let engine = Engine::new();
        block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap_err()
            .to_string()
    }

    fn test_meta() -> ParseMetadata {
        ParseMetadata {
            diagram_type: "requirement".to_string(),
            config: MermaidConfig::default(),
            effective_config: MermaidConfig::default(),
            title: None,
        }
    }

    fn payload_selection(facts: &EditorSemanticFacts, detail: &str, name: &str) -> SourceSpan {
        facts
            .symbols
            .iter()
            .find(|symbol| symbol.detail.as_deref() == Some(detail) && symbol.name == name)
            .unwrap_or_else(|| panic!("missing payload symbol {detail:?} {name:?}"))
            .selection
    }

    #[test]
    fn requirement_entrypoints_construct_one_semantic_source() {
        let engine = Engine::new();
        let text = concat!(
            "requirementDiagram\n",
            "requirement login {\n",
            "  id: REQ-1\n",
            "  risk: high\n",
            "  verifymethod: test\n",
            "}\n",
        );

        reset_requirement_syntax_construction_count();
        engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(requirement_syntax_construction_count(), 1);

        reset_requirement_syntax_construction_count();
        engine
            .parse_diagram_for_render_model_sync(text, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(requirement_syntax_construction_count(), 1);

        reset_requirement_syntax_construction_count();
        engine
            .parse_editor_semantic_facts_with_type_sync("requirement", text, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(requirement_syntax_construction_count(), 1);
    }

    #[test]
    fn requirement_combined_projection_constructs_once_and_matches_standalone_entrypoints() {
        let text = concat!(
            "requirementDiagram\n",
            "accTitle: Login requirements\n",
            "direction LR\n",
            "requirement login:::critical {\n",
            "  id: REQ-1\n",
            "  text: Login must work\n",
            "  risk: high\n",
            "  verifymethod: test\n",
            "}\n",
            "element api {\n",
            "  type: service\n",
            "}\n",
            "classDef critical fill:#f9f\n",
            "login - verifies -> api\n",
        );
        let mut meta = test_meta();
        meta.effective_config = MermaidConfig::from_value(json!({
            "theme": "forest",
            "securityLevel": "strict"
        }));
        let standalone_json = parse_requirement(text, &meta).unwrap();
        let standalone_editor = parse_requirement_editor_facts(text, &meta);

        reset_requirement_syntax_construction_count();
        let (combined_json, combined_editor) =
            parse_requirement_json_and_editor_facts(text, &meta).unwrap();

        assert_eq!(requirement_syntax_construction_count(), 1);
        assert_eq!(combined_json, standalone_json);
        assert_eq!(combined_editor, standalone_editor);
    }

    #[test]
    fn requirement_typed_projection_matches_compatibility_json() {
        let text = concat!(
            "requirementDiagram\n",
            "accTitle: Login requirements\n",
            "direction LR\n",
            "requirement login {\n",
            "  id: REQ-1\n",
            "  text: Login must work\n",
            "  risk: high\n",
            "  verifymethod: test\n",
            "}\n",
            "element api {\n",
            "  type: service\n",
            "}\n",
            "login - verifies -> api\n",
        );
        let meta = test_meta();
        let compat = parse_requirement(text, &meta).unwrap();
        let typed = parse_requirement_model_for_render(text, &meta).unwrap();

        assert_eq!(render_model_to_compat_json(&typed, &meta).unwrap(), compat);
        assert_eq!(compat["type"], "requirement");
        assert_eq!(compat["config"], meta.effective_config.as_value().clone());
        assert!(compat["accDescr"].is_null());
    }

    #[test]
    fn requirement_malformed_recovery_reuses_partial_statement_facts_and_exact_span() {
        let engine = Engine::new();
        let text = concat!(
            "requirementDiagram\n",
            "requirement login {\n",
            "  id: REQ-1\n",
            "  risk: impossible\n",
            "}\n",
        );
        let bad_value_start = text.find("impossible").unwrap();

        reset_requirement_syntax_construction_count();
        let error = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .expect_err("invalid risk must fail strict parsing");
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("invalid risk returned a non-parse error");
        };
        assert_eq!(requirement_syntax_construction_count(), 1);
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(
                bad_value_start,
                bad_value_start + "impossible".len(),
            ))
        );
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);

        reset_requirement_syntax_construction_count();
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("requirement", text, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(requirement_syntax_construction_count(), 1);
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "login"));
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "REQ-1"));
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.span
                == Some(SourceSpan::new(
                    bad_value_start,
                    bad_value_start + "impossible".len(),
                ))
        }));
    }

    #[test]
    fn requirement_full_requirement_definition_is_parsed() {
        let model = parse(
            r#"requirementDiagram

requirement test_req {
  id: test_id
  text: the test text.
  risk: high
  verifymethod: analysis
}
"#,
        );

        assert_eq!(model["requirements"].as_array().unwrap().len(), 1);
        assert_eq!(model["requirements"][0]["name"], json!("test_req"));
        assert_eq!(model["requirements"][0]["type"], json!("Requirement"));
        assert_eq!(model["requirements"][0]["requirementId"], json!("test_id"));
        assert_eq!(model["requirements"][0]["text"], json!("the test text."));
        assert_eq!(model["requirements"][0]["risk"], json!("High"));
        assert_eq!(model["requirements"][0]["verifyMethod"], json!("Analysis"));
        assert_eq!(model["elements"].as_array().unwrap().len(), 0);
        assert_eq!(model["relationships"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn requirement_full_element_definition_is_parsed() {
        let model = parse(
            r#"requirementDiagram

element test_el {
  type: test_type
  docref: test_ref
}
"#,
        );

        assert_eq!(model["requirements"].as_array().unwrap().len(), 0);
        assert_eq!(model["elements"].as_array().unwrap().len(), 1);
        assert_eq!(model["elements"][0]["name"], json!("test_el"));
        assert_eq!(model["elements"][0]["type"], json!("test_type"));
        assert_eq!(model["elements"][0]["docRef"], json!("test_ref"));
        assert_eq!(model["relationships"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn requirement_editor_payload_spans_point_to_values_when_values_match_keys() {
        let text = r#"requirementDiagram
accTitle: accTitle
accDescr: accDescr
requirement req {
  id: id
  text: text
  risk: risk
  verifymethod: verifymethod
}
element el {
  type: type
  docref: docref
}
"#;
        let facts = parse_requirement_editor_facts(text, &test_meta());

        for (detail, name, needle) in [
            (
                "requirement accessibility title",
                "accTitle",
                "accTitle: accTitle",
            ),
            (
                "requirement accessibility description",
                "accDescr",
                "accDescr: accDescr",
            ),
            ("requirement id", "id", "id: id"),
            ("requirement text", "text", "text: text"),
            ("requirement risk", "risk", "risk: risk"),
            (
                "requirement verify method",
                "verifymethod",
                "verifymethod: verifymethod",
            ),
            ("requirement element type", "type", "type: type"),
            ("requirement doc ref", "docref", "docref: docref"),
        ] {
            let value_start = text.find(needle).unwrap() + needle.rfind(name).unwrap();
            assert_eq!(
                payload_selection(&facts, detail, name),
                SourceSpan::new(value_start, value_start + name.len()),
                "wrong span for {detail}"
            );
        }
    }

    #[test]
    fn requirement_definition_spans_point_to_name_tokens_when_names_prefix_keywords() {
        let text = concat!(
            "requirementDiagram\n",
            "  requirement require {\n",
            "  }\n",
            "  element elem {\n",
            "  }\n",
        );
        let facts = parse_requirement_editor_facts(text, &test_meta());

        for (detail, name, declaration) in [
            ("requirement", "require", "  requirement require {"),
            ("requirement element", "elem", "  element elem {"),
        ] {
            let declaration_start = text.find(declaration).unwrap();
            let name_start = declaration_start + declaration.rfind(name).unwrap();
            let expected = SourceSpan::new(name_start, name_start + name.len());
            let symbol = facts
                .symbols
                .iter()
                .find(|symbol| symbol.detail.as_deref() == Some(detail) && symbol.name == name)
                .unwrap_or_else(|| panic!("missing definition symbol {detail:?} {name:?}"));

            assert_eq!(symbol.role, EditorSemanticRole::Entity);
            assert_eq!(symbol.rename_policy, crate::EditorRenamePolicy::Identifier);
            assert_eq!(symbol.selection, expected);
            assert_eq!(&text[symbol.selection.start..symbol.selection.end], name);
            assert!(facts.expected_syntax.iter().any(|syntax| {
                syntax.kind == EditorExpectedSyntaxKind::NodeIdentifier && syntax.span == expected
            }));
        }
    }

    #[test]
    fn requirement_acc_title_and_acc_descr_are_parsed() {
        let model = parse(
            r#"requirementDiagram
accTitle: test title
accDescr: my chart description
element test_name {
  type: test_type
  docref: test_ref
}
"#,
        );

        assert_eq!(model["accTitle"], json!("test title"));
        assert_eq!(model["accDescr"], json!("my chart description"));
    }

    #[test]
    fn requirement_multiline_acc_descr_is_parsed() {
        let model = parse(
            r#"requirementDiagram
accTitle: test title
accDescr {
  my chart description
line 2
}
element test_name {
  type: test_type
  docref: test_ref
}
"#,
        );

        assert_eq!(model["accTitle"], json!("test title"));
        assert_eq!(model["accDescr"], json!("my chart description\nline 2"));
    }

    #[test]
    fn requirement_relationship_is_parsed() {
        let model = parse(
            r#"requirementDiagram

a - contains -> b
"#,
        );
        assert_eq!(model["relationships"].as_array().unwrap().len(), 1);
        assert_eq!(model["relationships"][0]["type"], json!("contains"));
        assert_eq!(model["relationships"][0]["src"], json!("a"));
        assert_eq!(model["relationships"][0]["dst"], json!("b"));
    }

    #[test]
    fn requirement_relationship_left_arrow_is_parsed() {
        let model = parse(
            r#"requirementDiagram

a <- contains - b
"#,
        );
        assert_eq!(model["relationships"].as_array().unwrap().len(), 1);
        assert_eq!(model["relationships"][0]["type"], json!("contains"));
        assert_eq!(model["relationships"][0]["src"], json!("b"));
        assert_eq!(model["relationships"][0]["dst"], json!("a"));
    }

    #[test]
    fn requirement_proto_and_constructor_ids_are_accepted() {
        for id in ["__proto__", "constructor"] {
            let model = parse(&format!(
                r#"requirementDiagram
requirement {id} {{
  id: 1
  text: the test text.
  risk: high
  verifymethod: test
}}
"#
            ));
            assert_eq!(model["requirements"].as_array().unwrap().len(), 1);
        }

        for id in ["__proto__", "constructor"] {
            let model = parse(&format!(
                r#"requirementDiagram
element {id} {{
  type: simulation
}}
"#
            ));
            assert_eq!(model["elements"].as_array().unwrap().len(), 1);
        }
    }

    #[test]
    fn requirement_style_statement_applies_to_requirement() {
        let model = parse(
            r#"requirementDiagram

requirement test_req {
}
style test_req fill:#f9f,stroke:#333,stroke-width:4px
"#,
        );

        assert_eq!(
            model["requirements"][0]["cssStyles"],
            json!(["fill:#f9f", "stroke:#333", "stroke-width:4px"])
        );
    }

    #[test]
    fn requirement_style_statement_applies_to_element() {
        let model = parse(
            r#"requirementDiagram

element test_element {
}
style test_element fill:#f9f,stroke:#333,stroke-width:4px
"#,
        );

        assert_eq!(
            model["elements"][0]["cssStyles"],
            json!(["fill:#f9f", "stroke:#333", "stroke-width:4px"])
        );
    }

    #[test]
    fn requirement_style_statement_applies_to_multiple_things() {
        let model = parse(
            r#"requirementDiagram

requirement test_requirement {
}
element test_element {
}
style test_requirement,test_element fill:#f9f,stroke:#333,stroke-width:4px
"#,
        );

        assert_eq!(
            model["requirements"][0]["cssStyles"],
            json!(["fill:#f9f", "stroke:#333", "stroke-width:4px"])
        );
        assert_eq!(
            model["elements"][0]["cssStyles"],
            json!(["fill:#f9f", "stroke:#333", "stroke-width:4px"])
        );
    }

    #[test]
    fn requirement_classdef_and_class_statement_are_parsed() {
        let model = parse(
            r#"requirementDiagram

requirement myReq {
}
classDef myClass fill:#f9f,stroke:#333,stroke-width:4px
class myReq myClass
"#,
        );

        assert_eq!(
            model["requirements"][0]["classes"],
            json!(["default", "myClass"])
        );
        assert_eq!(
            model["requirements"][0]["cssStyles"],
            json!(["fill:#f9f", "stroke:#333", "stroke-width:4px"])
        );
        assert_eq!(model["classes"]["myClass"]["id"], json!("myClass"));
        assert_eq!(
            model["classes"]["myClass"]["styles"],
            json!(["fill:#f9f", "stroke:#333", "stroke-width:4px"])
        );
        assert_eq!(model["classes"]["myClass"]["textStyles"], json!([]));
    }

    #[test]
    fn requirement_shorthand_class_statement_is_supported() {
        let model = parse(
            r#"requirementDiagram

requirement myReq {
}
classDef myClass fill:#f9f,stroke:#333,stroke-width:4px
myReq:::myClass
"#,
        );
        assert_eq!(
            model["requirements"][0]["classes"],
            json!(["default", "myClass"])
        );
    }

    #[test]
    fn requirement_shorthand_is_supported_in_definition() {
        let model = parse(
            r#"requirementDiagram

requirement myReq:::class1 {
}
element myElem:::class1,class2 {
}

classDef class1 fill:#f9f,stroke:#333,stroke-width:4px
classDef class2 color:blue
"#,
        );
        assert_eq!(
            model["requirements"][0]["classes"],
            json!(["default", "class1"])
        );
        assert_eq!(
            model["elements"][0]["classes"],
            json!(["default", "class1", "class2"])
        );
    }

    #[test]
    fn requirement_direction_is_parsed() {
        for dir in ["TB", "BT", "LR", "RL"] {
            let model = parse(&format!("requirementDiagram\n\ndirection {dir}\n"));
            assert_eq!(model["direction"], json!(dir));
        }
    }

    #[test]
    fn requirement_top_level_directives_require_exact_first_word() {
        for statement in [
            "styleNode fill:#f00",
            "classify myReq myClass",
            "foo direction LR",
        ] {
            let message = parse_err(&format!("requirementDiagram\n{statement}\n"));
            assert!(
                message.contains("unexpected requirement statement"),
                "unexpected error for {statement:?}: {message}"
            );
        }
    }
}
