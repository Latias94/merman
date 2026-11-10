#[cfg(feature = "presentation")]
use crate::SourcePresentationMetadata;
use crate::diagrams::scan::{
    LineCursor, is_ecmascript_whitespace, leading_whitespace_len, starts_with_case_insensitive,
};
#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticRole, EditorSemanticSymbol,
};
use crate::{
    Error, OperationControl, OperationControlResult, Result, SourceSpan,
    family::CombinedSemanticFailure,
};
pub(crate) use merman_model::families::requirement::{
    RequirementClass, RequirementDiagramModel, RequirementElement, RequirementNode,
    RequirementRelationship,
};
#[cfg(feature = "compat-json")]
use serde_json::Value;
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

pub(crate) fn sanitize_common_db_fields(
    model: &mut RequirementDiagramModel,
    config: &crate::MermaidConfig,
) {
    crate::common_db::sanitize_optional_acc_title(&mut model.acc_title, config);
    crate::common_db::sanitize_optional_acc_descr(&mut model.acc_descr, config);
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
    relations: Vec<RequirementRelationship>,

    requirements: HashMap<String, RequirementNode>,
    requirement_order: Vec<String>,

    elements: HashMap<String, RequirementElement>,
    element_order: Vec<String>,

    classes: HashMap<String, RequirementClass>,
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
            RequirementNode {
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
            RequirementElement {
                name: name.to_string(),
                element_type: b.element_type,
                doc_ref: b.doc_ref,
                css_styles: Vec::new(),
                classes: vec!["default".to_string()],
            },
        );
    }

    fn add_relationship(&mut self, relationship_type: &str, src: &str, dst: &str) {
        self.relations.push(RequirementRelationship {
            rel_type: relationship_type.to_string(),
            src: src.to_string(),
            dst: dst.to_string(),
        });
    }

    #[cfg(feature = "editor-facts")]
    fn editor_entity_kind(&self, name: &str) -> Option<EditorSemanticKind> {
        if self.requirements.contains_key(name) {
            Some(EditorSemanticKind::Struct)
        } else if self.elements.contains_key(name) {
            Some(EditorSemanticKind::Object)
        } else {
            None
        }
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
                    .or_insert_with(|| RequirementClass {
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
    ) -> RequirementDiagramModel {
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

        RequirementDiagramModel {
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

#[cfg(feature = "compat-json")]
pub(crate) fn parse_requirement(code: &str, meta: &SourcePresentationMetadata) -> Result<Value> {
    let model = parse_requirement_semantic_source(code, meta)?.model;
    render_model_to_compat_json(&model, meta)
}

#[cfg(all(test, feature = "presentation"))]
pub(crate) fn parse_requirement_model_for_render(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<RequirementDiagramModel> {
    Ok(parse_requirement_semantic_source(code, meta)?.model)
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_requirement_model_for_render_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<RequirementDiagramModel>> {
    let construction = construct_requirement_semantic_source_controlled(
        code,
        &meta.diagram_type,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )?;
    match construction {
        Ok(source) => Ok(Ok(source.model)),
        Err(failure) => Ok(Err(failure.into_error())),
    }
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub(crate) fn parse_requirement_json_and_editor_facts(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<crate::family::CombinedSemanticParse> {
    let construction = construct_requirement_semantic_source_controlled(
        code,
        &meta.diagram_type,
        EditorFactsDemand::Capture,
        control,
    )?;
    Ok(crate::family::CombinedSemanticParse::from_construction(
        construction,
        |RequirementSemanticSource {
             model,
             editor_facts,
         }| (render_model_to_compat_json(&model, meta), editor_facts),
        CombinedSemanticFailure::into_parts,
    ))
}

pub(crate) fn parse_requirement_semantic_model_controlled(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<crate::family::SemanticConstruction<RequirementDiagramModel>> {
    let construction = construct_requirement_semantic_source_controlled(
        code,
        &metadata.diagram_type,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?;
    Ok(match construction {
        Ok(source) => {
            #[cfg(feature = "editor-facts")]
            let editor_facts = editor_demand
                .needs_recovery()
                .then_some(source.editor_facts);
            let mut model = source.model;
            sanitize_common_db_fields(&mut model, &metadata.semantic_config);
            crate::family::SemanticConstruction {
                model: Ok(model),
                #[cfg(feature = "editor-facts")]
                editor_facts,
            }
        }
        Err(failure) => {
            #[cfg(feature = "editor-facts")]
            {
                match editor_demand {
                    EditorFactsDemand::Capture | EditorFactsDemand::Recovery => {
                        let (error, facts) = failure.into_parts();
                        crate::family::SemanticConstruction {
                            model: Err(error),
                            editor_facts: Some(facts),
                        }
                    }
                    EditorFactsDemand::Omit => crate::family::SemanticConstruction {
                        model: Err(failure.into_error()),
                        editor_facts: None,
                    },
                }
            }
            #[cfg(not(feature = "editor-facts"))]
            {
                crate::family::SemanticConstruction {
                    model: Err(failure.into_error()),
                }
            }
        }
    })
}

struct RequirementSemanticSource {
    model: RequirementDiagramModel,
    #[cfg(feature = "editor-facts")]
    editor_facts: EditorSemanticFacts,
}

#[derive(Debug, Clone, Copy)]
struct SpannedValue<'a> {
    text: &'a str,
    start: usize,
}

fn requirement_statement_start(line: &str, line_start: usize) -> usize {
    line_start + line.len() - line.trim_start_matches(is_ecmascript_whitespace).len()
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

#[cfg(feature = "editor-facts")]
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

#[cfg(feature = "editor-facts")]
fn emit_parsed_requirement_ids(
    facts: &mut EditorSemanticFacts,
    ids: &ParsedRequirementIdList,
    statement_span: SourceSpan,
    detail: &'static str,
    kind: EditorSemanticKind,
    role: EditorSemanticRole,
) {
    for id in &ids.items {
        facts.push_symbol(EditorSemanticSymbol::with_role(
            id.value.clone(),
            Some(detail.to_string()),
            kind,
            role,
            statement_span,
            id.selection,
        ));
    }
}

#[cfg(feature = "editor-facts")]
fn emit_parsed_requirement_styles(
    facts: &mut EditorSemanticFacts,
    styles: &ParsedRequirementStyles,
    detail: &'static str,
) {
    for style in &styles.items {
        facts.push_symbol(EditorSemanticSymbol::payload(
            style.value.clone(),
            Some(detail.to_string()),
            EditorSemanticKind::Property,
            style.span,
            style.span,
        ));
    }
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &RequirementDiagramModel,
    meta: &SourcePresentationMetadata,
) -> Result<Value> {
    merman_model::families::requirement::compat::project(model, &meta.compat_json_metadata())
        .map_err(crate::Error::from)
}

#[cfg(any(feature = "compat-json", all(test, feature = "presentation")))]
fn parse_requirement_semantic_source(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<RequirementSemanticSource> {
    construct_requirement_semantic_source(code, meta).map_err(CombinedSemanticFailure::into_error)
}

#[cfg(any(feature = "compat-json", all(test, feature = "presentation")))]
fn construct_requirement_semantic_source(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> std::result::Result<RequirementSemanticSource, CombinedSemanticFailure> {
    construct_requirement_semantic_source_controlled(
        code,
        &meta.diagram_type,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        &OperationControl::new(),
    )
    .expect("a private parse control cannot be cancelled")
}

fn construct_requirement_semantic_source_controlled(
    code: &str,
    diagram_type: &str,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<RequirementSemanticSource, CombinedSemanticFailure>>
{
    control.checkpoint()?;
    #[cfg(test)]
    REQUIREMENT_SYNTAX_CONSTRUCTION_COUNT.set(REQUIREMENT_SYNTAX_CONSTRUCTION_COUNT.get() + 1);

    parse_requirement_semantic_source_once(
        code,
        diagram_type,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )
}

fn parse_requirement_semantic_source_once(
    code: &str,
    diagram_type: &str,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<RequirementSemanticSource, CombinedSemanticFailure>>
{
    control.checkpoint()?;
    let mut db = RequirementDb::new();
    let mut acc_title: Option<String> = None;
    let mut acc_descr: Option<String> = None;
    #[cfg(feature = "editor-facts")]
    let mut editor_facts = EditorSemanticFacts::new();
    #[cfg(feature = "editor-facts")]
    if editor_demand == EditorFactsDemand::Capture {
        #[cfg(test)]
        crate::editor::test_work::record_collection();
    }
    let mut lines = LineCursor::new(code);
    let mut direction_lookahead = RequirementDirectionLookahead::default();
    let mut saw_header = false;
    let mut first_error = None;

    while let Some((raw, line_start)) = lines.next_line() {
        control.checkpoint()?;
        let statement_start = requirement_statement_start(raw, line_start);
        if raw.is_empty() {
            continue;
        }
        let accessibility = parse_requirement_accessibility(&mut lines, line_start, control)?;
        if accessibility.is_none()
            && !starts_requirement_title_token(&code[line_start..])
            && let Some((direction, end)) =
                direction_lookahead.parse(code, line_start, line_start + raw.len(), control)?
        {
            lines.resume_at(end);
            if saw_header {
                #[cfg(feature = "editor-facts")]
                if editor_demand == EditorFactsDemand::Capture {
                    direction.emit_editor_fact(&mut editor_facts, SourceSpan::new(line_start, end));
                }
                if let Some(value) = direction.value {
                    db.set_direction(value);
                }
            } else {
                first_error.get_or_insert(Error::diagram_parse_exact(
                    diagram_type.to_string(),
                    "expected requirementDiagram",
                    SourceSpan::new(line_start, end),
                ));
            }
            continue;
        }
        if let Some(parsed) = accessibility {
            #[cfg(feature = "editor-facts")]
            if editor_demand == EditorFactsDemand::Capture {
                editor_facts.push_directive_prefix(parsed.key);
                parsed.emit_editor_fact(&mut editor_facts);
            }
            if !parsed.complete {
                #[cfg(feature = "editor-facts")]
                if editor_demand.needs_recovery() {
                    editor_facts.mark_recovered_from_parse_error(
                        "requirement parser recovered from incomplete accessibility directive",
                        Some(parsed.statement_span),
                    );
                }
                first_error.get_or_insert_with(|| {
                    Error::diagram_parse_insertion_point(
                        diagram_type.to_string(),
                        if parsed.block {
                            "unterminated accDescr block"
                        } else if parsed.key == "accTitle" {
                            "expected requirement accessibility title value"
                        } else {
                            "expected requirement accessibility description value"
                        },
                        parsed.statement_span.end,
                    )
                });
            }
            if let Some(value) = parsed.value {
                if parsed.key == "accTitle" {
                    acc_title = Some(value.text.to_string());
                } else {
                    acc_descr = Some(value.text.to_string());
                }
            }
            continue;
        }

        if code[line_start..]
            .chars()
            .next()
            .is_some_and(is_ecmascript_whitespace)
        {
            let end = scan_requirement_accessibility_while(
                code,
                line_start,
                is_ecmascript_whitespace,
                control,
            )?;
            lines.resume_at(end);
            continue;
        }

        let stripped = split_requirement_line(raw);
        let t = stripped.trim_matches(is_ecmascript_whitespace);
        if t.is_empty() {
            continue;
        }

        if !saw_header {
            if t.eq_ignore_ascii_case("requirementDiagram") {
                saw_header = true;
                continue;
            }
            first_error.get_or_insert(Error::diagram_parse_exact(
                diagram_type.to_string(),
                "expected requirementDiagram",
                requirement_statement_span(raw, line_start),
            ));
            continue;
        }

        if let Some(ch) = t.chars().next().filter(|ch| ch.is_whitespace()) {
            first_error.get_or_insert(Error::diagram_parse_exact(
                diagram_type.to_string(),
                "unexpected non-ECMAScript whitespace at requirement statement",
                SourceSpan::new(statement_start, statement_start + ch.len_utf8()),
            ));
            continue;
        }

        if let Some(direction) = parse_direction(t, statement_start) {
            #[cfg(feature = "editor-facts")]
            if editor_demand == EditorFactsDemand::Capture {
                direction.emit_editor_fact(
                    &mut editor_facts,
                    requirement_statement_span(raw, line_start),
                );
            }
            if let Some(value) = direction.value {
                db.set_direction(value);
            } else {
                first_error.get_or_insert(Error::diagram_parse_exact(
                    diagram_type.to_string(),
                    "invalid requirement direction",
                    direction.selection,
                ));
            }
            continue;
        }

        let requirement_open = match parse_requirement_def_open(t, statement_start) {
            Ok(open) => open,
            Err(error) => {
                first_error.get_or_insert(requirement_exact_error(
                    error,
                    diagram_type,
                    requirement_statement_span(raw, line_start),
                ));
                continue;
            }
        };
        if let Some(RequirementDefOpen {
            name,
            requirement_type: ty,
            classes,
        }) = requirement_open
        {
            #[cfg(feature = "editor-facts")]
            let statement_span = requirement_statement_span(raw, line_start);
            #[cfg(feature = "editor-facts")]
            if editor_demand == EditorFactsDemand::Capture {
                emit_requirement_definition(
                    &mut editor_facts,
                    &name,
                    classes.as_ref(),
                    statement_span,
                    &ty.to_lowercase(),
                    EditorSemanticKind::Struct,
                );
            }
            let body = parse_requirement_body(
                code,
                &mut lines,
                diagram_type,
                #[cfg(feature = "editor-facts")]
                &mut editor_facts,
                #[cfg(feature = "editor-facts")]
                editor_demand,
                control,
            )?;
            if let Some(error) = body.error {
                first_error.get_or_insert(error);
            }
            db.add_requirement(&name.value, &ty, body.value);
            if let Some(classes) = classes.as_ref().map(ParsedRequirementIdList::values) {
                db.set_class(std::slice::from_ref(&name.value), &classes);
            }
            continue;
        }

        let element_open = match parse_element_def_open(t, statement_start) {
            Ok(open) => open,
            Err(error) => {
                first_error.get_or_insert(requirement_exact_error(
                    error,
                    diagram_type,
                    requirement_statement_span(raw, line_start),
                ));
                continue;
            }
        };
        if let Some(ElementDefOpen { name, classes }) = element_open {
            #[cfg(feature = "editor-facts")]
            let statement_span = requirement_statement_span(raw, line_start);
            #[cfg(feature = "editor-facts")]
            if editor_demand == EditorFactsDemand::Capture {
                emit_requirement_definition(
                    &mut editor_facts,
                    &name,
                    classes.as_ref(),
                    statement_span,
                    "requirement element",
                    EditorSemanticKind::Object,
                );
            }
            let body = parse_element_body(
                code,
                &mut lines,
                diagram_type,
                #[cfg(feature = "editor-facts")]
                &mut editor_facts,
                #[cfg(feature = "editor-facts")]
                editor_demand,
                control,
            )?;
            if let Some(error) = body.error {
                first_error.get_or_insert(error);
            }
            db.add_element(&name.value, body.value);
            if let Some(classes) = classes.as_ref().map(ParsedRequirementIdList::values) {
                db.set_class(std::slice::from_ref(&name.value), &classes);
            }
            continue;
        }

        let shorthand = match parse_shorthand_class_stmt(t, statement_start) {
            Ok(statement) => statement,
            Err(error) => {
                first_error.get_or_insert(requirement_exact_error(
                    error,
                    diagram_type,
                    requirement_statement_span(raw, line_start),
                ));
                continue;
            }
        };
        if let Some(statement) = shorthand {
            #[cfg(feature = "editor-facts")]
            if editor_demand == EditorFactsDemand::Capture {
                emit_requirement_shorthand_class(
                    &mut editor_facts,
                    &statement,
                    requirement_statement_span(raw, line_start),
                );
            }
            db.set_class(
                std::slice::from_ref(&statement.target.value),
                &statement.classes.values(),
            );
            continue;
        }

        let style = match parse_style_stmt(t, statement_start) {
            Ok(statement) => statement,
            Err(error) => {
                first_error.get_or_insert(requirement_exact_error(
                    error,
                    diagram_type,
                    requirement_statement_span(raw, line_start),
                ));
                continue;
            }
        };
        if let Some(statement) = style {
            #[cfg(feature = "editor-facts")]
            if editor_demand == EditorFactsDemand::Capture {
                emit_requirement_style(
                    &mut editor_facts,
                    &statement,
                    requirement_statement_span(raw, line_start),
                );
            }
            db.set_css_style(&statement.ids.values(), &statement.styles.values());
            continue;
        }

        let class_def = match parse_classdef_stmt(t, statement_start) {
            Ok(statement) => statement,
            Err(error) => {
                first_error.get_or_insert(requirement_exact_error(
                    error,
                    diagram_type,
                    requirement_statement_span(raw, line_start),
                ));
                continue;
            }
        };
        if let Some(statement) = class_def {
            #[cfg(feature = "editor-facts")]
            if editor_demand == EditorFactsDemand::Capture {
                emit_requirement_class_def(
                    &mut editor_facts,
                    &statement,
                    requirement_statement_span(raw, line_start),
                );
            }
            db.define_class(&statement.ids.values(), &statement.styles.values());
            continue;
        }

        let class = match parse_class_stmt(t, statement_start) {
            Ok(statement) => statement,
            Err(error) => {
                first_error.get_or_insert(requirement_exact_error(
                    error,
                    diagram_type,
                    requirement_statement_span(raw, line_start),
                ));
                continue;
            }
        };
        if let Some(statement) = class {
            #[cfg(feature = "editor-facts")]
            if editor_demand == EditorFactsDemand::Capture {
                emit_requirement_class(
                    &mut editor_facts,
                    &statement,
                    requirement_statement_span(raw, line_start),
                );
            }
            db.set_class(&statement.targets.values(), &statement.classes.values());
            continue;
        }

        let relationship = match parse_relationship_stmt(t, statement_start) {
            Ok(statement) => statement,
            Err(error) => {
                first_error.get_or_insert(requirement_exact_error(
                    error,
                    diagram_type,
                    requirement_statement_span(raw, line_start),
                ));
                continue;
            }
        };
        if let Some(relationship) = relationship {
            #[cfg(feature = "editor-facts")]
            if editor_demand == EditorFactsDemand::Capture {
                emit_requirement_relationship(
                    &mut editor_facts,
                    &relationship,
                    requirement_statement_span(raw, line_start),
                );
            }
            db.add_relationship(
                &relationship.kind,
                &relationship.source.value,
                &relationship.target.value,
            );
            continue;
        }

        first_error.get_or_insert(Error::diagram_parse_exact(
            diagram_type.to_string(),
            format!("unexpected requirement statement: {t}"),
            requirement_statement_span(raw, line_start),
        ));
    }

    if !saw_header {
        first_error.get_or_insert(Error::diagram_parse_insertion_point(
            diagram_type.to_string(),
            "expected requirementDiagram",
            code.len(),
        ));
    }

    #[cfg(feature = "editor-facts")]
    if editor_demand == EditorFactsDemand::Capture {
        resolve_requirement_reference_kinds(&mut editor_facts, &db);
    }

    if let Some(error) = first_error {
        return Ok(Err(CombinedSemanticFailure::parser_recovery(
            #[cfg(feature = "editor-facts")]
            "requirement",
            error,
            #[cfg(feature = "editor-facts")]
            editor_demand.needs_recovery().then_some(editor_facts),
        )));
    }

    control.checkpoint()?;
    Ok(Ok(RequirementSemanticSource {
        model: db.to_render_model(acc_title, acc_descr),
        #[cfg(feature = "editor-facts")]
        editor_facts,
    }))
}

fn requirement_exact_error(error: Error, diagram_type: &str, span: SourceSpan) -> Error {
    let message = match error {
        Error::DiagramParse { diagnostic, .. } => diagnostic.message().to_string(),
        error => error.to_string(),
    };
    Error::diagram_parse_exact(diagram_type.to_string(), message, span)
}

fn requirement_statement_span(line: &str, line_start: usize) -> SourceSpan {
    let start = requirement_statement_start(line, line_start);
    let end = line_start + line.trim_end_matches(is_ecmascript_whitespace).len();
    SourceSpan::new(start, end.max(start))
}

struct RequirementAccessibility<'a> {
    key: &'static str,
    value: Option<SpannedValue<'a>>,
    statement_span: SourceSpan,
    complete: bool,
    block: bool,
}

impl RequirementAccessibility<'_> {
    #[cfg(feature = "editor-facts")]
    fn emit_editor_fact(&self, facts: &mut EditorSemanticFacts) {
        let Some(value) = self.value else {
            if !self.complete {
                facts.push_expected_syntax(EditorExpectedSyntax::new(
                    EditorExpectedSyntaxKind::Payload,
                    SourceSpan::new(self.statement_span.end, self.statement_span.end),
                ));
            }
            return;
        };
        let selection = SourceSpan::new(value.start, value.start + value.text.len());
        facts.push_expected_syntax(EditorExpectedSyntax::new(
            EditorExpectedSyntaxKind::Payload,
            selection,
        ));
        facts.push_symbol(EditorSemanticSymbol::payload(
            value.text.to_string(),
            Some(
                if self.key == "accTitle" {
                    "requirement accessibility title"
                } else {
                    "requirement accessibility description"
                }
                .to_string(),
            ),
            EditorSemanticKind::String,
            self.statement_span,
            selection,
        ));
    }
}

fn scan_requirement_accessibility_while(
    source: &str,
    start: usize,
    mut accepts: impl FnMut(char) -> bool,
    control: &OperationControl,
) -> OperationControlResult<usize> {
    for (count, (relative, ch)) in source[start..].char_indices().enumerate() {
        if count.is_multiple_of(128) {
            control.checkpoint()?;
        }
        if !accepts(ch) {
            return Ok(start + relative);
        }
    }
    control.checkpoint()?;
    Ok(source.len())
}

fn parse_requirement_accessibility<'a>(
    cursor: &mut LineCursor<'a>,
    start: usize,
    control: &OperationControl,
) -> OperationControlResult<Option<RequirementAccessibility<'a>>> {
    let source = cursor.source();
    let key = if starts_with_case_insensitive(&source[start..], "accTitle") {
        "accTitle"
    } else if starts_with_case_insensitive(&source[start..], "accDescr") {
        "accDescr"
    } else {
        return Ok(None);
    };
    let delimiter = scan_requirement_accessibility_while(
        source,
        start + key.len(),
        is_ecmascript_whitespace,
        control,
    )?;
    let block = match source.as_bytes().get(delimiter) {
        Some(b':') => false,
        Some(b'{') if key == "accDescr" => true,
        _ => return Ok(None),
    };
    let value_start = scan_requirement_accessibility_while(
        source,
        delimiter + 1,
        is_ecmascript_whitespace,
        control,
    )?;
    let value_end = scan_requirement_accessibility_while(
        source,
        value_start,
        |ch| ch != if block { '}' } else { '\n' },
        control,
    )?;
    let complete = if block {
        value_end < source.len()
    } else {
        value_start < value_end
    };
    let end = value_end + usize::from(block && complete);
    let text = source[value_start..value_end].trim_end_matches(is_ecmascript_whitespace);
    cursor.resume_at(end);
    Ok(Some(RequirementAccessibility {
        key,
        value: (!text.is_empty()).then_some(SpannedValue {
            text,
            start: value_start,
        }),
        statement_span: SourceSpan::new(start, end),
        complete,
        block,
    }))
}

#[cfg(feature = "editor-facts")]
fn emit_requirement_definition(
    facts: &mut EditorSemanticFacts,
    name: &ParsedRequirementId,
    classes: Option<&ParsedRequirementIdList>,
    statement_span: SourceSpan,
    detail: &str,
    kind: EditorSemanticKind,
) {
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::NodeIdentifier,
        name.selection,
    ));
    facts.push_symbol(EditorSemanticSymbol::new(
        name.value.clone(),
        Some(detail.to_string()),
        kind,
        statement_span,
        name.selection,
    ));
    if let Some(classes) = classes {
        emit_parsed_requirement_ids(
            facts,
            classes,
            statement_span,
            "requirement class",
            EditorSemanticKind::Property,
            EditorSemanticRole::Payload,
        );
    }
}

#[cfg(feature = "editor-facts")]
fn emit_requirement_shorthand_class(
    facts: &mut EditorSemanticFacts,
    statement: &ParsedRequirementClassShorthand,
    statement_span: SourceSpan,
) {
    facts.push_symbol(EditorSemanticSymbol::reference(
        statement.target.value.clone(),
        Some("requirement class target".to_string()),
        EditorSemanticKind::Struct,
        statement_span,
        statement.target.selection,
    ));
    emit_parsed_requirement_ids(
        facts,
        &statement.classes,
        statement_span,
        "requirement class",
        EditorSemanticKind::Property,
        EditorSemanticRole::Payload,
    );
}

#[cfg(feature = "editor-facts")]
fn emit_requirement_style(
    facts: &mut EditorSemanticFacts,
    statement: &ParsedRequirementStyleStatement,
    statement_span: SourceSpan,
) {
    emit_parsed_requirement_ids(
        facts,
        &statement.ids,
        statement_span,
        "requirement style target",
        EditorSemanticKind::Struct,
        EditorSemanticRole::Reference,
    );
    emit_parsed_requirement_styles(facts, &statement.styles, "requirement style");
}

#[cfg(feature = "editor-facts")]
fn emit_requirement_class_def(
    facts: &mut EditorSemanticFacts,
    statement: &ParsedRequirementStyleStatement,
    statement_span: SourceSpan,
) {
    emit_parsed_requirement_ids(
        facts,
        &statement.ids,
        statement_span,
        "requirement class definition",
        EditorSemanticKind::Property,
        EditorSemanticRole::ClassDefinition,
    );
    emit_parsed_requirement_styles(facts, &statement.styles, "requirement class style");
}

#[cfg(feature = "editor-facts")]
fn emit_requirement_class(
    facts: &mut EditorSemanticFacts,
    statement: &ParsedRequirementClassStatement,
    statement_span: SourceSpan,
) {
    emit_parsed_requirement_ids(
        facts,
        &statement.classes,
        statement_span,
        "requirement class",
        EditorSemanticKind::Property,
        EditorSemanticRole::Payload,
    );
    emit_parsed_requirement_ids(
        facts,
        &statement.targets,
        statement_span,
        "requirement class target",
        EditorSemanticKind::Struct,
        EditorSemanticRole::Reference,
    );
}

#[cfg(feature = "editor-facts")]
fn emit_requirement_relationship(
    facts: &mut EditorSemanticFacts,
    relationship: &ParsedRequirementRelationship,
    statement_span: SourceSpan,
) {
    facts.push_symbol(EditorSemanticSymbol::payload(
        relationship.kind.clone(),
        Some("requirement relationship".to_string()),
        EditorSemanticKind::String,
        statement_span,
        relationship.kind_span,
    ));
    facts.push_symbol(EditorSemanticSymbol::reference(
        relationship.source.value.clone(),
        Some("requirement relationship source".to_string()),
        EditorSemanticKind::Struct,
        statement_span,
        relationship.source.selection,
    ));
    facts.push_symbol(EditorSemanticSymbol::reference(
        relationship.target.value.clone(),
        Some("requirement relationship target".to_string()),
        EditorSemanticKind::Struct,
        statement_span,
        relationship.target.selection,
    ));
}

#[cfg(feature = "editor-facts")]
fn resolve_requirement_reference_kinds(facts: &mut EditorSemanticFacts, db: &RequirementDb) {
    for symbol in &mut facts.symbols {
        if symbol.role == EditorSemanticRole::Reference
            && let Some(kind) = db.editor_entity_kind(&symbol.name)
        {
            symbol.kind = kind;
        }
    }
}

/// Body values beginning with a quoted token may contain physical newlines. A quote inside
/// an unquoted value is ordinary text in the selected grammar's `unqString` state.
fn next_requirement_line<'a>(
    code: &'a str,
    lines: &mut LineCursor<'a>,
    control: &OperationControl,
) -> OperationControlResult<Option<(&'a str, usize)>> {
    let Some((mut line, start)) = lines.next_line() else {
        return Ok(None);
    };
    let Some(value) =
        split_key_value_spanned(split_requirement_line(line)).and_then(|pair| pair.value)
    else {
        return Ok(Some((line, start)));
    };
    let Some(content) = value.text.strip_prefix('"') else {
        return Ok(Some((line, start)));
    };
    let mut content = content;
    let mut line_start = start;
    loop {
        control.checkpoint()?;
        let mut closed = false;
        for (index, byte) in content.bytes().enumerate() {
            if index.is_multiple_of(4096) {
                control.checkpoint()?;
            }
            if byte == b'"' {
                closed = true;
                break;
            }
        }
        let end = line_start + line.len();
        if closed {
            return Ok(Some((&code[start..end], start)));
        }
        let Some((next, next_start)) = lines.next_line() else {
            return Ok(Some((&code[start..end], start)));
        };
        line = next;
        content = next;
        line_start = next_start;
    }
}

fn requirement_quoted_end(input: &str) -> Option<usize> {
    input.strip_prefix('"')?.find('"').map(|end| end + 2)
}

/// Searches default-state boundaries without interpreting the contents of string tokens.
fn find_requirement_boundary(input: &str, mut accepts: impl FnMut(&str) -> bool) -> Option<usize> {
    // These ordered Jison literals precede unqString and require an ASCII word boundary.
    // In particular, comments after a header or a risk/verify-method token are still comments.
    const KEYWORDS: &[&str] = &[
        "requirementDiagram",
        "id",
        "text",
        "risk",
        "verifyMethod",
        "requirement",
        "functionalRequirement",
        "interfaceRequirement",
        "performanceRequirement",
        "physicalRequirement",
        "designConstraint",
        "low",
        "medium",
        "high",
        "analysis",
        "demonstration",
        "inspection",
        "test",
        "element",
        "contains",
        "copies",
        "derives",
        "satisfies",
        "verifies",
        "refines",
        "traces",
        "type",
        "docref",
        "style",
        "classDef",
        "class",
    ];

    let mut cursor = 0;
    while let Some(ch) = input[cursor..].chars().next() {
        let rest = &input[cursor..];
        if accepts(rest) {
            return Some(cursor);
        }
        if matches!(ch, '#' | '%') {
            return None;
        }
        cursor += if ch == '"' {
            requirement_quoted_end(rest).unwrap_or(rest.len())
        } else if ch.is_ascii_alphanumeric() || ch == '_' {
            if let Some(keyword) = KEYWORDS.iter().find(|keyword| {
                starts_with_case_insensitive(rest, keyword)
                    && rest
                        .as_bytes()
                        .get(keyword.len())
                        .is_none_or(|byte| !byte.is_ascii_alphanumeric() && *byte != b'_')
            }) {
                keyword.len()
            } else {
                // unqString may contain spaces, quotes, #, and % after its initial ASCII \w.
                rest.find([':', ',', '\r', '\n', '{', '<', '>', '-', '='])
                    .unwrap_or(rest.len())
            }
        } else {
            ch.len_utf8()
        };
    }
    None
}

fn find_requirement_token(input: &str, token: &str) -> Option<usize> {
    find_requirement_boundary(input, |rest| rest.starts_with(token))
}

fn split_requirement_line(line: &str) -> &str {
    let lowered = line
        .trim_start_matches(is_ecmascript_whitespace)
        .to_ascii_lowercase();
    if lowered.starts_with("style")
        || lowered.starts_with("classdef")
        || lowered.starts_with("class ")
        || lowered == "class"
    {
        return line;
    }

    let end =
        find_requirement_boundary(line, |rest| rest.starts_with(['#', '%'])).unwrap_or(line.len());
    &line[..end]
}

struct ParsedRequirementDirection {
    value: Option<&'static str>,
    selection: SourceSpan,
}

fn starts_requirement_title_token(source: &str) -> bool {
    if !starts_with_case_insensitive(source, "title") {
        return false;
    }
    let after = &source["title".len()..];
    let Some(separator) = after
        .chars()
        .next()
        .filter(|ch| is_ecmascript_whitespace(*ch))
    else {
        return false;
    };
    after[separator.len_utf8()..]
        .chars()
        .next()
        .is_some_and(|ch| !matches!(ch, '#' | ';' | '\n'))
}

#[derive(Default)]
struct RequirementDirectionLookahead {
    line: Option<RequirementDirectionLine>,
}

struct RequirementDirectionLine {
    start: usize,
    prefix_end: usize,
    candidates: [Option<RequirementDirectionCandidate>; 4],
}

#[derive(Clone, Copy)]
struct RequirementDirectionCandidate {
    keyword_start: usize,
    value_start: usize,
    end: usize,
}

impl RequirementDirectionLookahead {
    fn parse(
        &mut self,
        source: &str,
        start: usize,
        line_end: usize,
        control: &OperationControl,
    ) -> OperationControlResult<Option<(ParsedRequirementDirection, usize)>> {
        if self
            .line
            .as_ref()
            .is_none_or(|line| start < line.start || start > line.prefix_end)
        {
            let mut line = RequirementDirectionLine {
                start,
                prefix_end: source.len(),
                candidates: [None; 4],
            };
            for (count, (offset, ch)) in source[start..].char_indices().enumerate() {
                if count.is_multiple_of(128) {
                    control.checkpoint()?;
                }
                if matches!(ch, '\n' | '\r' | '\u{2028}' | '\u{2029}') {
                    line.prefix_end = start + offset;
                    break;
                }
                let keyword_start = start + offset;
                if !starts_with_case_insensitive(&source[keyword_start..], "direction") {
                    continue;
                }
                let after_keyword = keyword_start + "direction".len();
                let value = scan_requirement_accessibility_while(
                    source,
                    after_keyword,
                    is_ecmascript_whitespace,
                    control,
                )?;
                if value == after_keyword {
                    continue;
                }
                for (index, direction) in ["TB", "BT", "RL", "LR"].into_iter().enumerate() {
                    if !starts_with_case_insensitive(&source[value..], direction) {
                        continue;
                    }
                    let value_end = value + direction.len();
                    let end = if value_end <= line_end {
                        line_end
                    } else {
                        scan_requirement_accessibility_while(
                            source,
                            value_end,
                            |ch| ch != '\n',
                            control,
                        )?
                    };
                    line.candidates[index] = Some(RequirementDirectionCandidate {
                        keyword_start,
                        value_start: value,
                        end,
                    });
                    break;
                }
            }
            self.line = Some(line);
        }
        let Some(line) = &self.line else {
            return Ok(None);
        };
        for (candidate, direction) in line.candidates.iter().zip(["TB", "BT", "RL", "LR"]) {
            let Some(candidate) = candidate.filter(|candidate| candidate.keyword_start >= start)
            else {
                continue;
            };
            return Ok(Some((
                ParsedRequirementDirection {
                    value: Some(direction),
                    selection: SourceSpan::new(
                        candidate.value_start,
                        candidate.value_start + direction.len(),
                    ),
                },
                candidate.end,
            )));
        }
        Ok(None)
    }
}

impl ParsedRequirementDirection {
    #[cfg(feature = "editor-facts")]
    fn emit_editor_fact(&self, facts: &mut EditorSemanticFacts, statement_span: SourceSpan) {
        facts.push_expected_syntax(EditorExpectedSyntax::new(
            EditorExpectedSyntaxKind::CardinalDirectionValue,
            self.selection,
        ));
        if let Some(value) = self.value {
            facts.push_symbol(EditorSemanticSymbol::payload(
                value,
                Some("requirement direction".to_string()),
                EditorSemanticKind::String,
                statement_span,
                self.selection,
            ));
        }
    }
}

fn parse_direction(t: &str, statement_start: usize) -> Option<ParsedRequirementDirection> {
    let (keyword, rest) = split_first_word(t)?;
    if !keyword.eq_ignore_ascii_case("direction") {
        return None;
    }
    let rest_start = t.len() - rest.len();
    let value_start = statement_start + rest_start + leading_whitespace_len(rest);
    let dir = split_first_word(rest).map_or("", |(dir, _)| dir);
    let value = match dir.to_ascii_uppercase().as_str() {
        "TB" => Some("TB"),
        "BT" => Some("BT"),
        "LR" => Some("LR"),
        "RL" => Some("RL"),
        _ => None,
    };
    Some(ParsedRequirementDirection {
        value,
        selection: SourceSpan::new(value_start, value_start + dir.len()),
    })
}

fn parse_requirement_def_open(
    t: &str,
    statement_start: usize,
) -> Result<Option<RequirementDefOpen>> {
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
    let name_and_classes = split_name_and_classes(rest, statement_start + rest_start)?;
    if name_and_classes.name.value.is_empty() {
        return Err(Error::diagram_parse_fallback(
            "requirement".to_string(),
            "requirement name is empty".to_string(),
        ));
    }
    Ok(Some(RequirementDefOpen {
        name: name_and_classes.name,
        requirement_type,
        classes: name_and_classes.class_tokens,
    }))
}

struct RequirementDefOpen {
    name: ParsedRequirementId,
    requirement_type: String,
    classes: Option<ParsedRequirementIdList>,
}

fn parse_element_def_open(t: &str, statement_start: usize) -> Result<Option<ElementDefOpen>> {
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
    let name_and_classes = split_name_and_classes(rest, statement_start + rest_start)?;
    if name_and_classes.name.value.is_empty() {
        return Err(Error::diagram_parse_fallback(
            "requirement".to_string(),
            "element name is empty".to_string(),
        ));
    }
    Ok(Some(ElementDefOpen {
        name: name_and_classes.name,
        classes: name_and_classes.class_tokens,
    }))
}

struct ElementDefOpen {
    name: ParsedRequirementId,
    classes: Option<ParsedRequirementIdList>,
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

fn split_name_and_classes(input: &str, input_start: usize) -> Result<DefinitionName> {
    let leading = leading_whitespace_len(input);
    let input = input.trim();
    if input.is_empty() {
        return Ok(DefinitionName {
            name: ParsedRequirementId {
                value: String::new(),
                #[cfg(feature = "editor-facts")]
                selection: SourceSpan::new(input_start + leading, input_start + leading),
            },
            class_tokens: None,
        });
    }

    if let Some(pos) = find_requirement_token(input, ":::") {
        let name_raw = input[..pos].trim_end();
        let classes_raw = &input[pos + 3..];
        let name = parse_requirement_id_spanned(name_raw, input_start + leading)?;
        let classes_start = input_start + leading + pos + 3;
        let class_tokens = parse_id_list_all_spanned(classes_raw, classes_start)?;
        return Ok(DefinitionName {
            name,
            class_tokens: Some(class_tokens),
        });
    }

    let name = parse_requirement_id_spanned(input, input_start + leading)?;
    Ok(DefinitionName {
        name,
        class_tokens: None,
    })
}

struct DefinitionName {
    name: ParsedRequirementId,
    class_tokens: Option<ParsedRequirementIdList>,
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
    let end = requirement_quoted_end(input)?;
    Some((input[1..end - 1].to_string(), &input[end..]))
}

struct RequirementBodyParse<T> {
    value: T,
    error: Option<Error>,
}

fn parse_requirement_body<'a>(
    code: &'a str,
    lines: &mut LineCursor<'a>,
    diagram_type: &str,
    #[cfg(feature = "editor-facts")] facts: &mut EditorSemanticFacts,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<RequirementBodyParse<RequirementBuilder>> {
    control.checkpoint()?;
    let mut b = RequirementBuilder::new();
    let mut error = None;
    while let Some((raw, line_start)) = next_requirement_line(code, lines, control)? {
        control.checkpoint()?;
        let line = split_requirement_line(raw);
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if t == "}" {
            return Ok(RequirementBodyParse { value: b, error });
        }

        let Some(key_value) = split_key_value_spanned(line) else {
            error.get_or_insert(Error::diagram_parse_exact(
                diagram_type.to_string(),
                format!("invalid requirement body line: {t}"),
                requirement_statement_span(raw, line_start),
            ));
            continue;
        };
        let k = key_value.key.text;
        let key = k.to_ascii_lowercase();
        let Some(value) = key_value.value else {
            error.get_or_insert(Error::diagram_parse_exact(
                diagram_type.to_string(),
                format!("invalid requirement body line: {t}"),
                requirement_statement_span(raw, line_start),
            ));
            continue;
        };
        let value_span = SourceSpan::new(
            line_start + value.start,
            line_start + value.start + value.text.len(),
        );
        if !matches!(key.as_str(), "id" | "text" | "risk" | "verifymethod") {
            error.get_or_insert(Error::diagram_parse_exact(
                diagram_type.to_string(),
                format!("unexpected requirement body key: {k}"),
                requirement_statement_span(raw, line_start),
            ));
            continue;
        }
        #[cfg(feature = "editor-facts")]
        let detail = match key.as_str() {
            "id" => "requirement id",
            "text" => "requirement text",
            "risk" => "requirement risk",
            "verifymethod" => "requirement verify method",
            _ => unreachable!("requirement key was validated above"),
        };
        #[cfg(feature = "editor-facts")]
        if editor_demand == EditorFactsDemand::Capture {
            push_requirement_payload_fact(
                facts,
                value.text,
                value_span.start,
                detail,
                EditorSemanticKind::String,
            );
        }
        let parsed = match parse_simple_value(value.text) {
            Ok(parsed) => parsed,
            Err(parse_error) => {
                error.get_or_insert(requirement_exact_error(
                    parse_error,
                    diagram_type,
                    value_span,
                ));
                continue;
            }
        };
        match key.as_str() {
            "id" => b.requirement_id = parsed,
            "text" => b.text = parsed,
            "risk" => match normalize_risk(&parsed) {
                Ok(risk) => b.risk = risk,
                Err(parse_error) => {
                    error.get_or_insert(requirement_exact_error(
                        parse_error,
                        diagram_type,
                        value_span,
                    ));
                }
            },
            "verifymethod" => match normalize_verify_method(&parsed) {
                Ok(method) => b.verify_method = method,
                Err(parse_error) => {
                    error.get_or_insert(requirement_exact_error(
                        parse_error,
                        diagram_type,
                        value_span,
                    ));
                }
            },
            _ => unreachable!("body key was validated above"),
        }
    }

    error.get_or_insert(Error::diagram_parse_insertion_point(
        diagram_type.to_string(),
        "unterminated requirement block",
        lines.offset(),
    ));
    Ok(RequirementBodyParse { value: b, error })
}

fn parse_element_body<'a>(
    code: &'a str,
    lines: &mut LineCursor<'a>,
    diagram_type: &str,
    #[cfg(feature = "editor-facts")] facts: &mut EditorSemanticFacts,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<RequirementBodyParse<ElementBuilder>> {
    control.checkpoint()?;
    let mut b = ElementBuilder::new();
    let mut error = None;
    while let Some((raw, line_start)) = next_requirement_line(code, lines, control)? {
        control.checkpoint()?;
        let line = split_requirement_line(raw);
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if t == "}" {
            return Ok(RequirementBodyParse { value: b, error });
        }

        let Some(key_value) = split_key_value_spanned(line) else {
            error.get_or_insert(Error::diagram_parse_exact(
                diagram_type.to_string(),
                format!("invalid element body line: {t}"),
                requirement_statement_span(raw, line_start),
            ));
            continue;
        };
        let k = key_value.key.text;
        let key = k.to_ascii_lowercase();
        let Some(value) = key_value.value else {
            error.get_or_insert(Error::diagram_parse_exact(
                diagram_type.to_string(),
                format!("invalid element body line: {t}"),
                requirement_statement_span(raw, line_start),
            ));
            continue;
        };
        let value_span = SourceSpan::new(
            line_start + value.start,
            line_start + value.start + value.text.len(),
        );
        if !matches!(key.as_str(), "type" | "docref") {
            error.get_or_insert(Error::diagram_parse_exact(
                diagram_type.to_string(),
                format!("unexpected element body key: {k}"),
                requirement_statement_span(raw, line_start),
            ));
            continue;
        }
        #[cfg(feature = "editor-facts")]
        let detail = match key.as_str() {
            "type" => "requirement element type",
            "docref" => "requirement doc ref",
            _ => unreachable!("element key was validated above"),
        };
        #[cfg(feature = "editor-facts")]
        if editor_demand == EditorFactsDemand::Capture {
            push_requirement_payload_fact(
                facts,
                value.text,
                value_span.start,
                detail,
                EditorSemanticKind::String,
            );
        }
        let parsed = match parse_simple_value(value.text) {
            Ok(parsed) => parsed,
            Err(parse_error) => {
                error.get_or_insert(requirement_exact_error(
                    parse_error,
                    diagram_type,
                    value_span,
                ));
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
        diagram_type.to_string(),
        "unterminated element block",
        lines.offset(),
    ));
    Ok(RequirementBodyParse { value: b, error })
}

struct SpannedKeyValue<'a> {
    key: SpannedValue<'a>,
    value: Option<SpannedValue<'a>>,
}

fn split_key_value_spanned(input: &str) -> Option<SpannedKeyValue<'_>> {
    let idx = input.find(':')?;
    let key = trim_spanned_value(&input[..idx], 0)?;
    let value = trim_spanned_value(&input[idx + 1..], idx + 1);
    Some(SpannedKeyValue { key, value })
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

fn parse_requirement_id_spanned(input: &str, _input_start: usize) -> Result<ParsedRequirementId> {
    #[cfg(feature = "editor-facts")]
    let leading = leading_whitespace_len(input);
    let input = input.trim();
    #[cfg(feature = "editor-facts")]
    let token_start = _input_start + leading;
    let (value, rest) = parse_id_or_name(input)?;
    if !rest.trim().is_empty() {
        return Err(Error::diagram_parse_fallback(
            "requirement".to_string(),
            format!(
                "unexpected trailing tokens after identifier: {}",
                rest.trim()
            ),
        ));
    }
    if input.starts_with('"') {
        #[cfg(feature = "editor-facts")]
        let selection = SourceSpan::new(token_start + 1, token_start + 1 + value.len());
        return Ok(ParsedRequirementId {
            value,
            #[cfg(feature = "editor-facts")]
            selection,
        });
    }
    Ok(ParsedRequirementId {
        #[cfg(feature = "editor-facts")]
        selection: SourceSpan::new(token_start, token_start + input.len()),
        value,
    })
}

#[derive(Debug)]
struct ParsedRequirementStyle {
    value: String,
    #[cfg(feature = "editor-facts")]
    span: SourceSpan,
}

#[derive(Debug)]
struct ParsedRequirementStyles {
    items: Vec<ParsedRequirementStyle>,
}

impl ParsedRequirementStyles {
    fn values(&self) -> Vec<String> {
        self.items.iter().map(|item| item.value.clone()).collect()
    }
}

fn parse_requirement_styles(input: &str, _input_start: usize) -> ParsedRequirementStyles {
    let mut items = Vec::new();
    let mut _cursor = 0usize;
    for part in input.split_inclusive(',') {
        let raw = part.strip_suffix(',').unwrap_or(part);
        #[cfg(feature = "editor-facts")]
        let leading = leading_whitespace_len(raw);
        let value = raw.trim();
        if !value.is_empty() {
            items.push(ParsedRequirementStyle {
                value: value.to_string(),
                #[cfg(feature = "editor-facts")]
                span: SourceSpan::new(
                    _input_start + _cursor + leading,
                    _input_start + _cursor + leading + value.len(),
                ),
            });
        }
        _cursor += part.len();
    }
    ParsedRequirementStyles { items }
}

struct ParsedRequirementClassShorthand {
    target: ParsedRequirementId,
    classes: ParsedRequirementIdList,
}

struct ParsedRequirementStyleStatement {
    ids: ParsedRequirementIdList,
    styles: ParsedRequirementStyles,
}

struct ParsedRequirementClassStatement {
    targets: ParsedRequirementIdList,
    classes: ParsedRequirementIdList,
}

struct ParsedRequirementRelationship {
    kind: String,
    #[cfg(feature = "editor-facts")]
    kind_span: SourceSpan,
    source: ParsedRequirementId,
    target: ParsedRequirementId,
}

fn parse_shorthand_class_stmt(
    t: &str,
    statement_start: usize,
) -> Result<Option<ParsedRequirementClassShorthand>> {
    let t = t.trim();
    if t.is_empty() || t.ends_with('{') {
        return Ok(None);
    }
    let Some(pos) = find_requirement_token(t, ":::") else {
        return Ok(None);
    };
    let left = t[..pos].trim_end();
    let right_raw = &t[pos + 3..];
    let right_leading = leading_whitespace_len(right_raw);
    let right = right_raw.trim_start();
    if left.is_empty() || right.is_empty() {
        return Err(Error::diagram_parse_fallback(
            "requirement".to_string(),
            format!("invalid class shorthand statement: {t}"),
        ));
    }
    let target = parse_requirement_id_spanned(left, statement_start)?;
    let classes = parse_id_list_all_spanned(right, statement_start + pos + 3 + right_leading)?;
    Ok(Some(ParsedRequirementClassShorthand { target, classes }))
}

fn parse_style_stmt(
    t: &str,
    statement_start: usize,
) -> Result<Option<ParsedRequirementStyleStatement>> {
    let t = t.trim_start();
    let Some((keyword, rest)) = split_first_word(t) else {
        return Ok(None);
    };
    if !keyword.eq_ignore_ascii_case("style") {
        return Ok(None);
    }
    let rest_leading = leading_whitespace_len(rest);
    let rest_start = t.len() - rest.len() + rest_leading;
    let rest = rest.trim_start();
    let (id_tokens, styles_str, styles_start) =
        split_list_and_rest_spanned(rest, statement_start + rest_start)?;
    let styles = parse_requirement_styles(styles_str, styles_start);
    if id_tokens.items.is_empty() || styles.items.is_empty() {
        return Err(Error::diagram_parse_fallback(
            "requirement".to_string(),
            format!("invalid style statement: {t}"),
        ));
    }
    Ok(Some(ParsedRequirementStyleStatement {
        ids: id_tokens,
        styles,
    }))
}

fn parse_classdef_stmt(
    t: &str,
    statement_start: usize,
) -> Result<Option<ParsedRequirementStyleStatement>> {
    let t = t.trim_start();
    let Some((keyword, rest)) = split_first_word(t) else {
        return Ok(None);
    };
    if !keyword.eq_ignore_ascii_case("classdef") {
        return Ok(None);
    }
    let rest_leading = leading_whitespace_len(rest);
    let rest_start = t.len() - rest.len() + rest_leading;
    let rest = rest.trim_start();
    let (id_tokens, styles_str, styles_start) =
        split_list_and_rest_spanned(rest, statement_start + rest_start)?;
    let styles = parse_requirement_styles(styles_str, styles_start);
    if id_tokens.items.is_empty() || styles.items.is_empty() {
        return Err(Error::diagram_parse_fallback(
            "requirement".to_string(),
            format!("invalid classDef statement: {t}"),
        ));
    }
    Ok(Some(ParsedRequirementStyleStatement {
        ids: id_tokens,
        styles,
    }))
}

fn parse_class_stmt(
    t: &str,
    statement_start: usize,
) -> Result<Option<ParsedRequirementClassStatement>> {
    let t = t.trim_start();
    let Some((keyword, rest)) = split_first_word(t) else {
        return Ok(None);
    };
    if !keyword.eq_ignore_ascii_case("class") {
        return Ok(None);
    }
    let rest_leading = leading_whitespace_len(rest);
    let rest_start = t.len() - rest.len() + rest_leading;
    let rest = rest.trim_start();
    let (id_tokens, classes_str, classes_start) =
        split_list_and_rest_spanned(rest, statement_start + rest_start)?;
    let class_tokens = parse_id_list_all_spanned(classes_str, classes_start)?;
    if id_tokens.items.is_empty() || class_tokens.items.is_empty() {
        return Err(Error::diagram_parse_fallback(
            "requirement".to_string(),
            format!("invalid class statement: {t}"),
        ));
    }
    Ok(Some(ParsedRequirementClassStatement {
        targets: id_tokens,
        classes: class_tokens,
    }))
}

fn parse_relationship_stmt(
    t: &str,
    statement_start: usize,
) -> Result<Option<ParsedRequirementRelationship>> {
    let t = t.trim();
    if t.is_empty() {
        return Ok(None);
    }

    if let Some(pos) = find_requirement_token(t, "<-") {
        let left = t[..pos].trim_end();
        let after_arrow = &t[pos + 2..];
        let rest_leading = leading_whitespace_len(after_arrow);
        let rest_start = pos + 2 + rest_leading;
        let rest = after_arrow.trim_start();
        let Some(dash) = find_requirement_token(rest, "-") else {
            return Err(Error::diagram_parse_fallback(
                "requirement".to_string(),
                format!("invalid relationship statement: {rest}"),
            ));
        };
        let rel = rest[..dash].trim();
        let right_raw = &rest[dash + 1..];
        let right_leading = leading_whitespace_len(right_raw);
        let right = right_raw.trim();
        #[cfg(feature = "editor-facts")]
        let rel_leading = leading_whitespace_len(&rest[..dash]);
        #[cfg(feature = "editor-facts")]
        let rel_start = rest_start + rel_leading;
        let right_start = rest_start + dash + 1 + right_leading;
        let relationship = normalize_relationship(rel)?;
        let mut left_token = parse_requirement_id_spanned(left, statement_start)?;
        let mut right_token = parse_requirement_id_spanned(right, statement_start + right_start)?;
        left_token.value = left_token.value.trim().to_string();
        right_token.value = right_token.value.trim().to_string();
        if relationship.is_empty() {
            return Ok(None);
        }
        return Ok(Some(ParsedRequirementRelationship {
            kind: relationship,
            #[cfg(feature = "editor-facts")]
            kind_span: SourceSpan::new(
                statement_start + rel_start,
                statement_start + rel_start + rel.len(),
            ),
            source: right_token,
            target: left_token,
        }));
    }

    if let Some(pos) = find_requirement_token(t, "->") {
        let right_raw = &t[pos + 2..];
        let right_leading = leading_whitespace_len(right_raw);
        let right_start = pos + 2 + right_leading;
        let right = right_raw.trim();
        let left_part = t[..pos].trim_end();
        let Some(dash) = find_requirement_token(left_part, "-") else {
            return Err(Error::diagram_parse_fallback(
                "requirement".to_string(),
                format!("invalid relationship statement: {left_part}"),
            ));
        };
        let src_raw = left_part[..dash].trim_end();
        let rel_raw = &left_part[dash + 1..];
        #[cfg(feature = "editor-facts")]
        let rel_leading = leading_whitespace_len(rel_raw);
        let rel = rel_raw.trim();
        #[cfg(feature = "editor-facts")]
        let rel_start = dash + 1 + rel_leading;
        let relationship = normalize_relationship(rel)?;
        let mut src_token = parse_requirement_id_spanned(src_raw, statement_start)?;
        let mut dst_token = parse_requirement_id_spanned(right, statement_start + right_start)?;
        src_token.value = src_token.value.trim().to_string();
        dst_token.value = dst_token.value.trim().to_string();
        if relationship.is_empty() {
            return Ok(None);
        }
        return Ok(Some(ParsedRequirementRelationship {
            kind: relationship,
            #[cfg(feature = "editor-facts")]
            kind_span: SourceSpan::new(
                statement_start + rel_start,
                statement_start + rel_start + rel.len(),
            ),
            source: src_token,
            target: dst_token,
        }));
    }

    Ok(None)
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

fn split_list_and_rest_spanned(
    input: &str,
    input_start: usize,
) -> Result<(ParsedRequirementIdList, &str, usize)> {
    let mut cur = input;
    let mut cursor = 0usize;
    let mut items = Vec::new();

    loop {
        let leading = leading_whitespace_len(cur);
        cursor += leading;
        cur = cur.trim_start();
        if cur.is_empty() {
            break;
        }

        let (item, rest) = if cur.starts_with('"') {
            let (value, rest) = parse_quoted_prefix(cur).ok_or_else(|| {
                Error::diagram_parse_fallback(
                    "requirement".to_string(),
                    "unterminated string".to_string(),
                )
            })?;
            #[cfg(feature = "editor-facts")]
            let consumed = cur.len() - rest.len();
            (
                ParsedRequirementId {
                    value,
                    #[cfg(feature = "editor-facts")]
                    selection: SourceSpan::new(
                        input_start + cursor + 1,
                        input_start + cursor + consumed - 1,
                    ),
                },
                rest,
            )
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
            (
                ParsedRequirementId {
                    value: cur[..end].to_string(),
                    #[cfg(feature = "editor-facts")]
                    selection: SourceSpan::new(input_start + cursor, input_start + cursor + end),
                },
                &cur[end..],
            )
        };

        items.push(item);
        cursor += cur.len() - rest.len();
        cur = rest;
        let separator_leading = leading_whitespace_len(cur);
        cursor += separator_leading;
        cur = cur.trim_start();
        if cur.starts_with(',') {
            cursor += 1;
            cur = &cur[1..];
            continue;
        }
        break;
    }

    let rest_leading = leading_whitespace_len(cur);
    cursor += rest_leading;
    let rest = cur.trim_start();
    Ok((
        ParsedRequirementIdList { items },
        rest,
        input_start + cursor,
    ))
}

#[derive(Debug, Clone)]
struct ParsedRequirementId {
    value: String,
    #[cfg(feature = "editor-facts")]
    selection: SourceSpan,
}

#[derive(Debug, Clone)]
struct ParsedRequirementIdList {
    items: Vec<ParsedRequirementId>,
}

impl ParsedRequirementIdList {
    fn values(&self) -> Vec<String> {
        self.items.iter().map(|item| item.value.clone()).collect()
    }
}

fn parse_id_list_all_spanned(input: &str, input_start: usize) -> Result<ParsedRequirementIdList> {
    let mut out = Vec::new();
    let mut cur = input.trim_start();
    let mut cursor = input.len() - cur.len();
    while !cur.is_empty() {
        let leading = cur.len() - cur.trim_start().len();
        cursor += leading;
        cur = cur.trim_start();
        let (item, rest, _selection) = if cur.starts_with('"') {
            let (item, rest) = parse_quoted_prefix(cur).ok_or_else(|| {
                Error::diagram_parse_fallback(
                    "requirement".to_string(),
                    "unterminated string".to_string(),
                )
            })?;
            let consumed = cur.len() - rest.len();
            (
                item,
                rest,
                SourceSpan::new(
                    input_start + cursor + 1,
                    input_start + cursor + consumed - 1,
                ),
            )
        } else {
            let mut end = cur.len();
            for (i, c) in cur.char_indices() {
                if c == ',' {
                    end = i;
                    break;
                }
            }
            let raw = &cur[..end];
            let item_leading = leading_whitespace_len(raw);
            let item = raw.trim().to_string();
            let selection = SourceSpan::new(
                input_start + cursor + item_leading,
                input_start + cursor + item_leading + item.len(),
            );
            (item, &cur[end..], selection)
        };
        if !item.is_empty() {
            out.push(ParsedRequirementId {
                value: item,
                #[cfg(feature = "editor-facts")]
                selection: _selection,
            });
        }
        cursor += cur.len() - rest.len();
        cur = rest;
        let separator_leading = cur.len() - cur.trim_start().len();
        cursor += separator_leading;
        cur = cur.trim_start();
        if cur.starts_with(',') {
            cursor += 1;
            cur = &cur[1..];
            continue;
        }
        break;
    }
    Ok(ParsedRequirementIdList { items: out })
}

#[cfg(all(test, feature = "compat-json"))]
mod tests {
    use super::*;
    #[cfg(feature = "editor-facts")]
    use crate::EditorSemanticCompleteness;
    use crate::{MermaidConfig, MermaidEngine, ParseDiagnosticSpanKind, ParseOptions};
    use futures::executor::block_on;
    use serde_json::json;

    fn parse(text: &str) -> crate::ManagedSemanticJson {
        let engine = MermaidEngine::new();
        block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap()
            .model
    }

    fn parse_err(text: &str) -> String {
        let engine = MermaidEngine::new();
        block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap_err()
            .to_string()
    }

    fn test_meta() -> SourcePresentationMetadata {
        SourcePresentationMetadata {
            diagram_type: "requirement".to_string(),
            config: MermaidConfig::default(),
            effective_config: crate::presentation::SourcePresentationConfig::default(),
            title: None,
        }
    }

    #[test]
    fn requirement_quoted_names_keep_relationship_and_class_delimiters_literal() {
        let source = concat!(
            "requirementDiagram\n",
            "element \"a-b:::c<-d->e\" {\n}\n",
            "requirement req {\n}\n",
            "\"a-b:::c<-d->e\" - traces -> req\n",
            "req <- traces - \"a-b:::c<-d->e\"\n",
        );
        let model = parse(source);
        assert_eq!(model["elements"][0]["name"], "a-b:::c<-d->e");
        for relation in model["relationships"].as_array().unwrap() {
            assert_eq!(relation["src"], "a-b:::c<-d->e");
            assert_eq!(relation["dst"], "req");
        }
    }

    #[test]
    fn requirement_unquoted_tokens_keep_embedded_comment_markers() {
        let source = concat!(
            "requirementDiagram\r\n",
            "requirement R#tag {\r\n",
            "  id: REQ#7%%rev\r\n",
            "  text: Guide 雪#章 %%literal\r\n",
            "}\r\n",
            "element Doc%%rev {\r\n",
            "  type: 5\" 雪#screen%%rev\r\n",
            "  docRef: guide#section%%tail\r\n",
            "}\r\n",
            "R#tag - traces -> Doc%%rev\r\n",
        );
        let parser = crate::SemanticParser::new();
        let parsed = parser.parse_diagram_sync(source).unwrap();
        let merman_model::semantic::SemanticDiagramModel::Requirement(model) = parsed.model else {
            panic!("expected Requirement model");
        };
        assert_eq!(model.requirements[0].name, "R#tag");
        assert_eq!(model.requirements[0].requirement_id, "REQ#7%%rev");
        assert_eq!(model.requirements[0].text, "Guide 雪#章 %%literal");
        assert_eq!(model.elements[0].name, "Doc%%rev");
        assert_eq!(model.elements[0].element_type, "5\" 雪#screen%%rev");
        assert_eq!(model.elements[0].doc_ref, "guide#section%%tail");
        assert_eq!(model.relationships[0].src, "R#tag");
        assert_eq!(model.relationships[0].dst, "Doc%%rev");

        #[cfg(feature = "editor-facts")]
        {
            let crate::SemanticSnapshotCapture::Snapshot(snapshot) =
                parser.capture_diagram_snapshot_sync(source)
            else {
                panic!("expected Requirement snapshot");
            };
            let (_, outcome, editor_facts) = snapshot.into_parts();
            assert!(matches!(outcome, crate::SemanticParseOutcome::Parsed(_)));
            let crate::ParsedEditorFacts::Available(facts) = editor_facts else {
                panic!("expected Requirement editor facts");
            };
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
            assert!(facts.diagnostics.is_empty());
            for (detail, value) in [
                ("requirement id", "REQ#7%%rev"),
                ("requirement text", "Guide 雪#章 %%literal"),
                ("requirement element type", "5\" 雪#screen%%rev"),
                ("requirement doc ref", "guide#section%%tail"),
            ] {
                let selection = payload_selection(&facts, detail, value);
                let start = source.find(value).unwrap();
                assert_eq!(selection, SourceSpan::new(start, start + value.len()));
                assert_eq!(&source[selection.start..selection.end], value);
                assert!(facts.expected_syntax.iter().any(|syntax| {
                    syntax.kind == EditorExpectedSyntaxKind::Payload && syntax.span == selection
                }));
            }
            for name in ["R#tag", "Doc%%rev"] {
                let symbols: Vec<_> = facts
                    .symbols
                    .iter()
                    .filter(|symbol| symbol.name == name)
                    .collect();
                assert_eq!(symbols.len(), 2);
                assert_eq!(symbols[0].role, EditorSemanticRole::Entity);
                assert_eq!(symbols[1].role, EditorSemanticRole::Reference);
                for symbol in symbols {
                    assert_eq!(&source[symbol.selection.start..symbol.selection.end], name);
                }
            }
        }
    }

    #[test]
    fn requirement_quoted_body_values_preserve_physical_line_endings_and_comments() {
        let source = concat!(
            "requirementDiagram # header comment\r\n",
            "# boundary comment: \"\r\n",
            "requirement req {\r\n",
            " id: \"REQ#%%\" # trailing comment: \"\r\n",
            " text: \"First\r\nSecond %% # café\" %% trailing comment: \"\r\n",
            " risk: high# boundary comment\r\n",
            " verifyMethod: test%% boundary comment\r\n",
            "} # closing comment\r\n",
            "element doc {\r\n",
            " type: \"One\r\nTwo\" # trailing comment: \"\r\n",
            " docRef: \"guide#section%%rev\" %% trailing comment\r\n",
            "}\r\n",
            "style doc fill:#fff\r\n",
        );
        let model = parse(source);
        assert_eq!(model["requirements"][0]["requirementId"], "REQ#%%");
        assert_eq!(model["requirements"][0]["text"], "First\nSecond %% # café");
        assert_eq!(model["requirements"][0]["risk"], "High");
        assert_eq!(model["requirements"][0]["verifyMethod"], "Test");
        assert_eq!(model["elements"][0]["type"], "One\nTwo");
        assert_eq!(model["elements"][0]["docRef"], "guide#section%%rev");
        assert_eq!(model["elements"][0]["cssStyles"], json!(["fill:#fff"]));
        #[cfg(feature = "editor-facts")]
        {
            let facts = crate::family::test_support::editor_facts(
                parse_requirement_json_and_editor_facts,
                source,
                &test_meta(),
            );
            let literal = "\"First\r\nSecond %% # café\"";
            let selection = payload_selection(&facts, "requirement text", literal);
            assert_eq!(&source[selection.start..selection.end], literal);
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
            assert!(facts.diagnostics.is_empty());
            assert!(
                !facts
                    .symbols
                    .iter()
                    .any(|symbol| symbol.name.contains("comment"))
            );
        }
        assert!(
            parse_err("requirementDiagram\nrequirement req {\ntext: \"First\nSecond\n}\n")
                .contains("unterminated")
        );
    }

    #[test]
    fn requirement_unquoted_inches_do_not_start_a_multiline_string() {
        let model = parse(
            "requirementDiagram\nrequirement r {\n text: 5\" display\n}\nelement next {\n}\n",
        );
        assert_eq!(model["requirements"][0]["text"], "5\" display");
        assert_eq!(model["elements"][0]["name"], "next");

        let model = parse(
            "requirementDiagram\nelement a\"b:::hot {\n}\nrequirement r {\n}\na\"b - traces -> r\n",
        );
        assert_eq!(model["elements"][0]["name"], "a\"b");
        assert_eq!(model["elements"][0]["classes"], json!(["default", "hot"]));
        assert_eq!(model["relationships"][0]["src"], "a\"b");

        for comment in ["# note: \"", "%% note: \""] {
            let model = parse(&format!(
                "requirementDiagram\nrequirement r {{\n{comment}\ntext: retained\n}}\nelement next {{\n}}\n"
            ));
            assert_eq!(model["requirements"][0]["text"], "retained");
            assert_eq!(model["elements"][0]["name"], "next");
        }
    }

    #[test]
    fn requirement_keyword_prefixes_keep_unquoted_token_boundaries() {
        let source = concat!(
            "requirementDiagram\n",
            "requirement textid\":::critical {\n",
            "id: DocRefId#7\n",
            "text: testid#section\n",
            "}\n",
            "element testid\" {\n",
            "docRef: textid%%anchor\n",
            "}\n",
            "testid\" - traces -> textid\"\n",
        );
        let model = parse(source);
        assert_eq!(model["requirements"][0]["name"], "textid\"");
        assert_eq!(
            model["requirements"][0]["classes"],
            json!(["default", "critical"])
        );
        assert_eq!(model["requirements"][0]["requirementId"], "DocRefId#7");
        assert_eq!(model["requirements"][0]["text"], "testid#section");
        assert_eq!(model["elements"][0]["name"], "testid\"");
        assert_eq!(model["elements"][0]["docRef"], "textid%%anchor");
        assert_eq!(model["relationships"][0]["src"], "testid\"");
        assert_eq!(model["relationships"][0]["dst"], "textid\"");

        #[cfg(feature = "editor-facts")]
        {
            let facts = crate::family::test_support::editor_facts(
                parse_requirement_json_and_editor_facts,
                source,
                &test_meta(),
            );
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
            assert!(facts.diagnostics.is_empty());
            for (detail, value) in [
                ("requirement id", "DocRefId#7"),
                ("requirement text", "testid#section"),
                ("requirement doc ref", "textid%%anchor"),
            ] {
                let selection = payload_selection(&facts, detail, value);
                assert_eq!(&source[selection.start..selection.end], value);
            }
        }
    }

    #[cfg(feature = "editor-facts")]
    fn payload_selection(facts: &EditorSemanticFacts, detail: &str, name: &str) -> SourceSpan {
        facts
            .symbols
            .iter()
            .find(|symbol| symbol.detail.as_deref() == Some(detail) && symbol.name == name)
            .unwrap_or_else(|| panic!("missing payload symbol {detail:?} {name:?}"))
            .selection
    }

    fn statement_occurrences(source: &str, statement: &str, needle: &str) -> Vec<SourceSpan> {
        let statement_start = source
            .find(statement)
            .unwrap_or_else(|| panic!("missing statement {statement:?}"));
        statement
            .match_indices(needle)
            .map(|(start, value)| {
                SourceSpan::new(
                    statement_start + start,
                    statement_start + start + value.len(),
                )
            })
            .collect()
    }

    #[cfg(feature = "editor-facts")]
    fn statement_symbol_selections(
        facts: &EditorSemanticFacts,
        source: &str,
        statement: &str,
        detail: &str,
    ) -> Vec<SourceSpan> {
        let statement_start = source
            .find(statement)
            .unwrap_or_else(|| panic!("missing statement {statement:?}"));
        let statement_span = SourceSpan::new(statement_start, statement_start + statement.len());
        let mut selections = facts
            .symbols
            .iter()
            .filter(|symbol| {
                symbol.detail.as_deref() == Some(detail)
                    && symbol.selection.start >= statement_span.start
                    && symbol.selection.end <= statement_span.end
            })
            .map(|symbol| symbol.selection)
            .collect::<Vec<_>>();
        selections.sort_by_key(|span| (span.start, span.end));
        selections
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn requirement_semantic_symbols_use_parser_spans_when_values_repeat() {
        let source = concat!(
            "requirementDiagram\n",
            "requirement same:::same,same {\n",
            "}\n",
            "short:::short,short\n",
            "style paint,paint paint:paint,paint:paint\n",
            "classDef defc,defc defc:defc,defc:defc\n",
            "class cls,cls cls,cls\n",
            "verifies - verifies -> verifies\n",
        );
        let facts = crate::family::test_support::editor_facts(
            parse_requirement_json_and_editor_facts,
            source,
            &test_meta(),
        );

        let definition = "requirement same:::same,same {";
        let definition_same = statement_occurrences(source, definition, "same");
        assert_eq!(
            statement_symbol_selections(&facts, source, definition, "requirement"),
            vec![definition_same[0]],
        );
        assert_eq!(
            statement_symbol_selections(&facts, source, definition, "requirement class"),
            definition_same[1..].to_vec(),
        );

        let shorthand = "short:::short,short";
        let shorthand_same = statement_occurrences(source, shorthand, "short");
        assert_eq!(
            statement_symbol_selections(&facts, source, shorthand, "requirement class target"),
            vec![shorthand_same[0]],
        );
        assert_eq!(
            statement_symbol_selections(&facts, source, shorthand, "requirement class"),
            shorthand_same[1..].to_vec(),
        );

        let style = "style paint,paint paint:paint,paint:paint";
        let style_same = statement_occurrences(source, style, "paint");
        assert_eq!(
            statement_symbol_selections(&facts, source, style, "requirement style target"),
            style_same[..2].to_vec(),
        );
        assert_eq!(
            statement_symbol_selections(&facts, source, style, "requirement style"),
            statement_occurrences(source, style, "paint:paint"),
        );

        let class_def = "classDef defc,defc defc:defc,defc:defc";
        let class_def_same = statement_occurrences(source, class_def, "defc");
        assert_eq!(
            statement_symbol_selections(&facts, source, class_def, "requirement class definition",),
            class_def_same[..2].to_vec(),
        );
        assert_eq!(
            statement_symbol_selections(&facts, source, class_def, "requirement class style",),
            statement_occurrences(source, class_def, "defc:defc"),
        );

        let class = "class cls,cls cls,cls";
        let class_same = statement_occurrences(source, class, "cls");
        assert_eq!(
            statement_symbol_selections(&facts, source, class, "requirement class target"),
            class_same[..2].to_vec(),
        );
        assert_eq!(
            statement_symbol_selections(&facts, source, class, "requirement class"),
            class_same[2..].to_vec(),
        );

        let relationship = "verifies - verifies -> verifies";
        let verifies = statement_occurrences(source, relationship, "verifies");
        assert_eq!(
            statement_symbol_selections(
                &facts,
                source,
                relationship,
                "requirement relationship source",
            ),
            vec![verifies[0]],
        );
        assert_eq!(
            statement_symbol_selections(&facts, source, relationship, "requirement relationship",),
            vec![verifies[1]],
        );
        assert_eq!(
            statement_symbol_selections(
                &facts,
                source,
                relationship,
                "requirement relationship target",
            ),
            vec![verifies[2]],
        );
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn requirement_targets_use_reference_roles_and_resolve_forward_definition_kinds() {
        let source = concat!(
            "requirementDiagram\n",
            "future - verifies -> later\n",
            "style future,later fill:#fff\n",
            "class future,later important\n",
            "future:::important\n",
            "element future {\n",
            "}\n",
            "requirement later {\n",
            "}\n",
            "classDef important fill:#fff\n",
        );
        let facts = crate::family::test_support::editor_facts(
            parse_requirement_json_and_editor_facts,
            source,
            &test_meta(),
        );
        let symbols_for = |name: &str| {
            facts
                .symbols
                .iter()
                .filter(|symbol| symbol.name == name)
                .collect::<Vec<_>>()
        };

        let future = symbols_for("future");
        assert_eq!(future.len(), 5);
        assert!(
            future[..4]
                .iter()
                .all(|symbol| symbol.role == EditorSemanticRole::Reference)
        );
        assert!(
            future
                .iter()
                .all(|symbol| symbol.kind == EditorSemanticKind::Object)
        );
        assert_eq!(future[4].role, EditorSemanticRole::Entity);

        let later = symbols_for("later");
        assert_eq!(later.len(), 4);
        assert!(
            later[..3]
                .iter()
                .all(|symbol| symbol.role == EditorSemanticRole::Reference)
        );
        assert!(
            later
                .iter()
                .all(|symbol| symbol.kind == EditorSemanticKind::Struct)
        );
        assert_eq!(later[3].role, EditorSemanticRole::Entity);
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn requirement_reference_kind_resolution_does_not_inspect_display_detail() {
        let mut db = RequirementDb::new();
        db.add_element("node", ElementBuilder::new());
        let span = SourceSpan::new(0, "node".len());
        let mut facts = EditorSemanticFacts::new();
        facts.push_symbol(EditorSemanticSymbol::reference(
            "node",
            Some("definition-looking display text".to_string()),
            EditorSemanticKind::Struct,
            span,
            span,
        ));

        resolve_requirement_reference_kinds(&mut facts, &db);

        assert_eq!(facts.symbols[0].role, EditorSemanticRole::Reference);
        assert_eq!(facts.symbols[0].kind, EditorSemanticKind::Object);
    }

    #[test]
    fn requirement_entrypoints_construct_one_semantic_source() {
        let engine = MermaidEngine::new();
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

        #[cfg(feature = "editor-facts")]
        {
            reset_requirement_syntax_construction_count();
            engine
                .parse_editor_semantic_facts_with_type_sync("requirement", text)
                .unwrap()
                .unwrap();
            assert_eq!(requirement_syntax_construction_count(), 1);
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
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
        meta.effective_config = crate::presentation::SourcePresentationConfig::from_value(json!({
            "theme": "forest",
            "securityLevel": "strict"
        }));
        let standalone_json = parse_requirement(text, &meta).unwrap();
        reset_requirement_syntax_construction_count();
        let (combined_json, combined_editor) = crate::family::test_support::into_result(
            parse_requirement_json_and_editor_facts(text, &meta, &OperationControl::new()),
        )
        .unwrap();

        assert_eq!(requirement_syntax_construction_count(), 1);
        assert_eq!(combined_json, standalone_json);
        assert!(!combined_editor.symbols.is_empty());
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
        let engine = MermaidEngine::new();
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

        #[cfg(feature = "editor-facts")]
        {
            reset_requirement_syntax_construction_count();
            let facts = engine
                .parse_editor_semantic_facts_with_type_sync("requirement", text)
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
    #[cfg(feature = "editor-facts")]
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
        let facts = crate::family::test_support::editor_facts(
            parse_requirement_json_and_editor_facts,
            text,
            &test_meta(),
        );

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
    #[cfg(feature = "editor-facts")]
    fn requirement_definition_spans_point_to_name_tokens_when_names_prefix_keywords() {
        let text = concat!(
            "requirementDiagram\n",
            "  requirement require {\n",
            "  }\n",
            "  element elem {\n",
            "  }\n",
        );
        let facts = crate::family::test_support::editor_facts(
            parse_requirement_json_and_editor_facts,
            text,
            &test_meta(),
        );

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
    fn requirement_style_and_class_directives_require_exact_first_word() {
        for statement in ["styleNode fill:#f00", "classify myReq myClass"] {
            let message = parse_err(&format!("requirementDiagram\n{statement}\n"));
            assert!(
                message.contains("unexpected requirement statement"),
                "unexpected error for {statement:?}: {message}"
            );
        }
    }
}

#[cfg(test)]
mod semantic_tests {
    use super::*;

    #[test]
    fn requirement_default_whitespace_keeps_its_greedy_crossline_endpoint() {
        for ending in ["\n", "\r\n"] {
            for (blank, direction_wins) in
                [("", true), (" ", false), ("\t", false), ("\u{feff}", false)]
            {
                for statement in [
                    "accTitle: direction LR",
                    "accDescr: direction LR",
                    "accDescr{direction LR}",
                ] {
                    let source =
                        format!("requirementDiagram{ending}{blank}{ending} {statement}{ending}");
                    let parsed = crate::SemanticParser::new()
                        .parse_diagram_sync(&source)
                        .unwrap();
                    let merman_model::semantic::SemanticDiagramModel::Requirement(model) =
                        parsed.model
                    else {
                        panic!("expected Requirement model");
                    };
                    assert_eq!(model.direction, if direction_wins { "LR" } else { "TB" });
                    let payload = if statement.starts_with("accTitle") {
                        model.acc_title
                    } else {
                        model.acc_descr
                    };
                    assert_eq!(
                        payload.as_deref(),
                        if direction_wins {
                            None
                        } else {
                            Some("direction LR")
                        }
                    );
                }
            }
        }
    }

    #[test]
    fn requirement_repeated_same_line_blocks_keep_total_checkpoint_work_linear() {
        for count in [128, 2048] {
            for block in [" accDescr{}", " accDescr{界}"] {
                for capture in [false, true] {
                    let source = format!("requirementDiagram\n{}\n", block.repeat(count));
                    let control = OperationControl::new();
                    control.cancel_after_checkpoints(count * 10 + 64);
                    let result = parse_requirement_semantic_model_controlled(
                        &source,
                        &metadata(),
                        #[cfg(feature = "editor-facts")]
                        if capture {
                            EditorFactsDemand::Capture
                        } else {
                            EditorFactsDemand::Omit
                        },
                        &control,
                    )
                    .expect("same-line token consumption must stay within linear checkpoint work");
                    let model = result.model.unwrap();
                    assert_eq!(
                        model.acc_descr.as_deref(),
                        if block.contains('界') {
                            Some("界")
                        } else {
                            None
                        }
                    );
                    #[cfg(feature = "editor-facts")]
                    if capture && block.contains('界') {
                        assert_eq!(result.editor_facts.unwrap().symbols.len(), count);
                    }
                    #[cfg(not(feature = "editor-facts"))]
                    let _ = capture;
                }
            }
        }
        let source = " prefix direction LR accDescr{} direction BT";
        let mut lookahead = RequirementDirectionLookahead::default();
        let first = lookahead
            .parse(source, 0, source.len(), &OperationControl::new())
            .unwrap()
            .unwrap();
        assert_eq!(first.0.value, Some("BT"));
        let after = source.find("direction BT").unwrap() + "direction BT".len();
        assert!(
            lookahead
                .parse(source, after, source.len(), &OperationControl::new())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn requirement_default_direction_tokens_keep_ordered_lexer_priority() {
        for ending in ["\n", "\r\n"] {
            for prefix in ["", " ", "\t", "\u{feff}", "\u{85}"] {
                for (statement, title, description) in [
                    ("accTitle: direction LR", Some("direction LR"), None),
                    ("accDescr: direction LR", None, Some("direction LR")),
                    ("accDescr{direction LR}", None, Some("direction LR")),
                ] {
                    let source = format!("requirementDiagram{ending}{prefix}{statement}{ending}");
                    let parsed = crate::SemanticParser::new()
                        .parse_diagram_sync(&source)
                        .unwrap();
                    let merman_model::semantic::SemanticDiagramModel::Requirement(model) =
                        parsed.model
                    else {
                        panic!("expected Requirement model");
                    };
                    assert_eq!(model.direction, if prefix.is_empty() { "TB" } else { "LR" });
                    assert_eq!(
                        model.acc_title.as_deref(),
                        if prefix.is_empty() { title } else { None }
                    );
                    assert_eq!(
                        model.acc_descr.as_deref(),
                        if prefix.is_empty() { description } else { None }
                    );
                }
            }
            for (statement, expected) in [
                ("prefix direction LR suffix", "LR"),
                ("direction\n\u{feff}LR", "LR"),
                ("prefix direction BT direction LR", "BT"),
                ("prefix direction LR direction TB", "TB"),
                ("prefix direction LRX", "LR"),
            ] {
                let source = format!("requirementDiagram\n{statement}\n").replace('\n', ending);
                let parsed = crate::SemanticParser::new()
                    .parse_diagram_sync(&source)
                    .unwrap();
                let merman_model::semantic::SemanticDiagramModel::Requirement(model) = parsed.model
                else {
                    panic!("expected Requirement model");
                };
                assert_eq!(model.direction, expected);
            }
            for statement in ["accTitle\n:direction LR", "accDescr\n{direction LR}"] {
                let source = format!("requirementDiagram\n{statement}\n").replace('\n', ending);
                let parsed = crate::SemanticParser::new()
                    .parse_diagram_sync(&source)
                    .unwrap();
                let merman_model::semantic::SemanticDiagramModel::Requirement(model) = parsed.model
                else {
                    panic!("expected Requirement model");
                };
                assert_eq!(model.direction, "TB");
            }
            for statement in [
                "title direction LR",
                "title\ndirection LR",
                "prefix\u{2028}direction LR",
            ] {
                let source = format!("requirementDiagram\n{statement}\n").replace('\n', ending);
                assert!(
                    crate::SemanticParser::new()
                        .parse_diagram_sync(&source)
                        .is_err()
                );
            }
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn requirement_literal_nel_root_prefixes_never_publish_shifted_identity_ranges() {
        for ending in ["\n", "\r\n"] {
            for statement in [
                "element Ghost {\n}\n",
                "requirement Ghost {\n}\n",
                "style Ghost fill:red\n",
                "class Ghost hot\n",
                "classDef Ghost fill:red\n",
                "Ghost - satisfies -> Target\n",
            ] {
                let source = format!(
                    "requirementDiagram\n\u{85}{statement}element Target {{\n}}\nelement Source {{\n}}\nSource - satisfies -> Target\n"
                ).replace('\n', ending);
                let parser = crate::SemanticParser::new();
                assert!(parser.parse_diagram_sync(&source).is_err());
                let crate::SemanticSnapshotCapture::Snapshot(snapshot) =
                    parser.capture_diagram_snapshot_sync(&source)
                else {
                    panic!("expected Requirement snapshot");
                };
                let crate::ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                    panic!("expected recovered Requirement facts");
                };
                assert!(!facts.symbols.iter().any(|symbol| symbol.name == "Ghost"));
                for symbol in facts
                    .symbols
                    .iter()
                    .filter(|symbol| matches!(symbol.name.as_str(), "Source" | "Target"))
                {
                    assert_eq!(
                        &source[symbol.selection.start..symbol.selection.end],
                        symbol.name
                    );
                }
                let nel = source.find('\u{85}').unwrap();
                assert!(facts.diagnostics.iter().any(|diagnostic| diagnostic.span
                    == Some(SourceSpan::new(nel, nel + '\u{85}'.len_utf8()))));
            }
        }
    }

    #[test]
    fn canonical_capture_resolves_forward_reference_kinds() {
        let source = concat!(
            "requirementDiagram\n",
            "future - verifies -> later\n",
            "style future,later fill:#fff\n",
            "class future,later important\n",
            "future:::important\n",
            "element future {\n",
            "}\n",
            "requirement later {\n",
            "}\n",
            "classDef important fill:#fff\n",
        );
        reset_requirement_syntax_construction_count();
        let parsed = parse_requirement_semantic_model_controlled(
            source,
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Capture,
            &OperationControl::new(),
        )
        .unwrap();
        assert!(parsed.model.is_ok());
        assert_eq!(requirement_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        {
            let facts = parsed.editor_facts.expect("requested facts");
            for (name, kind, count) in [
                ("future", EditorSemanticKind::Object, 5),
                ("later", EditorSemanticKind::Struct, 4),
            ] {
                let symbols = facts
                    .symbols
                    .iter()
                    .filter(|symbol| symbol.name == name)
                    .collect::<Vec<_>>();
                assert_eq!(symbols.len(), count);
                assert!(symbols.iter().all(|symbol| symbol.kind == kind));
                assert!(
                    symbols[..count - 1]
                        .iter()
                        .all(|symbol| symbol.role == EditorSemanticRole::Reference)
                );
                assert_eq!(symbols[count - 1].role, EditorSemanticRole::Entity);
            }
        }
    }

    fn metadata() -> crate::SemanticMetadata {
        crate::SemanticMetadata {
            diagram_type: "requirement".to_string(),
            config: crate::MermaidConfig::empty_object(),
            semantic_config: crate::MermaidConfig::from_value(serde_json::json!({
                "securityLevel": "strict"
            })),
            title: None,
        }
    }

    #[test]
    fn requirement_multiline_accessibility_scalars_keep_punctuation_and_js_whitespace() {
        let parser = crate::SemanticParser::new();
        for ending in ["\n", "\r\n"] {
            let source = format!(
                concat!(
                    "requirementDiagram{ending}",
                    "\u{feff}accTitle{ending}\u{00a0}\u{2028}:{ending}{ending}",
                    "\u{feff}\u{0085}Title;#%%literal\u{0085}\u{feff}{ending}",
                    "accDescr{ending}%% Removed{ending}\u{feff}:{ending}",
                    "\u{00a0}\u{0085}Help;#%%literal\u{0085}\u{feff}{ending}",
                    "requirement R {{{ending} id: 1{ending} text: \"Body\"{ending}}}{ending}",
                ),
                ending = ending,
            );
            let parsed = parser.parse_diagram_sync(&source).unwrap();
            let merman_model::semantic::SemanticDiagramModel::Requirement(model) = parsed.model
            else {
                panic!("expected Requirement model");
            };
            assert_eq!(
                model.acc_title.as_deref(),
                Some("\u{0085}Title;#%%literal\u{0085}")
            );
            assert_eq!(
                model.acc_descr.as_deref(),
                Some("\u{0085}Help;#%%literal\u{0085}")
            );
            assert_eq!(model.requirements.len(), 1);
            assert_eq!(model.requirements[0].name, "R");
            assert_eq!(model.requirements[0].text, "Body");
        }
    }

    #[test]
    fn requirement_root_accessibility_structural_whitespace_matches_ecmascript() {
        let parser = crate::SemanticParser::new();
        for whitespace in ["\u{feff}", "\u{00a0}", "\u{2028}"] {
            let source = format!(
                "{whitespace}requirementDiagram\n{whitespace}accTitle{whitespace}:{whitespace}Value\n"
            );
            let parsed = parser.parse_diagram_sync(&source).unwrap();
            let merman_model::semantic::SemanticDiagramModel::Requirement(model) = parsed.model
            else {
                panic!("expected Requirement model");
            };
            assert_eq!(model.acc_title.as_deref(), Some("Value"));
        }
        for statement in [
            "\u{0085}accTitle: Value",
            "accTitle\u{0085}: Value",
            "\u{0085}accDescr: Value",
            "accDescr\u{0085}{Help}",
        ] {
            let source = format!("requirementDiagram\n{statement}\n");
            assert!(parser.parse_diagram_sync(&source).is_err(), "{source:?}");
        }
    }

    #[test]
    fn requirement_empty_blocks_preserve_description_and_replay_closed_suffixes() {
        let parser = crate::SemanticParser::new();
        for ending in ["\n", "\r\n"] {
            let source = format!(
                concat!(
                    "requirementDiagram{ending}",
                    "accDescr: Old{ending}",
                    "accDescr{{}}{ending}",
                    "accDescr{ending}\u{feff}{{ {ending}\u{00a0}\u{2028}}}",
                    "accTitle{ending}:Value{ending}",
                    "element E {{{ending}}}{ending}",
                ),
                ending = ending,
            );
            let parsed = parser.parse_diagram_sync(&source).unwrap();
            let merman_model::semantic::SemanticDiagramModel::Requirement(model) = parsed.model
            else {
                panic!("expected Requirement model");
            };
            assert_eq!(model.acc_title.as_deref(), Some("Value"));
            assert_eq!(model.acc_descr.as_deref(), Some("Old"));
            assert_eq!(model.elements.len(), 1);
            assert_eq!(model.elements[0].name, "E");
        }
    }

    #[test]
    fn requirement_accessibility_states_own_directive_text_and_same_line_replacements() {
        let parser = crate::SemanticParser::new();
        for ending in ["\n", "\r\n"] {
            for (directives, title, description) in [
                ("accTitle:\ndirection LR\n", Some("direction LR"), None),
                (
                    "accDescr:\naccTitle: Literal\n",
                    None,
                    Some("accTitle: Literal"),
                ),
                (
                    "accDescr\n{\naccTitle: Keep\ndirection LR\n}\n",
                    None,
                    Some("accTitle: Keep\ndirection LR"),
                ),
                (
                    "accDescr: Old\naccDescr{\u{0085}}\n",
                    None,
                    Some("\u{0085}"),
                ),
                ("accDescr{First}accDescr{Second}\n", None, Some("Second")),
                (
                    "accDescr{Help}\u{feff}accTitle\n:Value\n",
                    Some("Value"),
                    Some("Help"),
                ),
            ] {
                let source = format!(
                    "requirementDiagram{ending}{}element E {{{ending}}}{ending}",
                    directives.replace('\n', ending),
                );
                let parsed = parser.parse_diagram_sync(&source).unwrap();
                let merman_model::semantic::SemanticDiagramModel::Requirement(model) = parsed.model
                else {
                    panic!("expected Requirement model");
                };
                assert_eq!(model.acc_title.as_deref(), title, "{source:?}");
                assert_eq!(model.acc_descr.as_deref(), description, "{source:?}");
                assert_eq!(model.direction, "TB");
                assert_eq!(model.elements.len(), 1);
            }
        }
    }

    #[test]
    fn requirement_accessibility_block_borrows_original_content_and_preserves_suffix() {
        let source = "accDescr\r\n\u{feff}{\r\n First  \r\n  Next #%%;\r\n}accTitle\n:Value\n";
        let mut cursor = LineCursor::new(source);
        cursor.next_line();
        let parsed = parse_requirement_accessibility(&mut cursor, 0, &OperationControl::new())
            .unwrap()
            .unwrap();
        let value = parsed.value.unwrap();
        assert_eq!(value.text, "First  \r\n  Next #%%;");
        assert_eq!(value.start, source.find("First").unwrap());
        assert_eq!(value.text.as_ptr(), source[value.start..].as_ptr());
        assert_eq!(parsed.statement_span.end, source.find('}').unwrap() + 1);
        assert!(parsed.complete);
        let suffix_start = parsed.statement_span.end;
        assert_eq!(cursor.next_line(), Some(("accTitle", suffix_start)));
        let suffix =
            parse_requirement_accessibility(&mut cursor, suffix_start, &OperationControl::new())
                .unwrap()
                .unwrap();
        assert_eq!(suffix.value.unwrap().text, "Value");
        assert_eq!(cursor.next_line(), Some(("", source.len() - 1)));
        assert!(cursor.next_line().is_none());
    }

    #[test]
    fn requirement_accessibility_prefix_grammar_stays_family_local_and_title_is_unadmitted() {
        let source = concat!(
            "accTitle\n:\nAccessible\n",
            "accDescr: Old\naccDescr { }accDescr\n{Help}\n",
            "requirementDiagram\nrequirement R {\n id: 1\n}\n",
        );
        let construction = parse_requirement_semantic_model_controlled(
            source,
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap();
        let model = construction.model.unwrap();
        assert_eq!(model.acc_title.as_deref(), Some("Accessible"));
        assert_eq!(model.acc_descr.as_deref(), Some("Help"));
        assert_eq!(model.requirements.len(), 1);

        let parser = crate::SemanticParser::new();
        assert!(parser.parse_diagram_sync(source).is_err());
        assert!(
            parser
                .parse_diagram_sync("requirementDiagram\ntitle Unsupported\n")
                .is_err()
        );
    }

    #[test]
    fn requirement_missing_accessibility_values_reject_at_eof_and_keep_prior_facts() {
        let parser = crate::SemanticParser::new();
        for ending in ["\n", "\r\n"] {
            for suffix in [
                "accTitle:".to_string(),
                format!("accTitle{ending}:\u{feff}{ending}"),
                "accDescr:".to_string(),
                format!("accDescr:{ending}{ending}"),
                "accDescr{".to_string(),
                format!("accDescr{ending}{{\u{feff}{ending}"),
                format!("accDescr{ending}{{Help{ending}unfinished"),
            ] {
                let source =
                    format!("requirementDiagram{ending}element E {{{ending}}}{ending}{suffix}");
                assert!(parser.parse_diagram_sync(&source).is_err(), "{source:?}");
                #[cfg(feature = "editor-facts")]
                {
                    let crate::SemanticSnapshotCapture::Snapshot(snapshot) =
                        parser.capture_diagram_snapshot_sync(&source)
                    else {
                        panic!("expected recovered Requirement snapshot");
                    };
                    let crate::ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                        panic!("expected Requirement editor facts");
                    };
                    assert_eq!(
                        facts.completeness,
                        crate::EditorSemanticCompleteness::Recovered
                    );
                    let entity = facts
                        .symbols
                        .iter()
                        .find(|symbol| symbol.name == "E")
                        .unwrap();
                    assert_eq!(&source[entity.selection.start..entity.selection.end], "E");
                    assert!(
                        facts.diagnostics.iter().any(|diagnostic| {
                            diagnostic.kind == crate::EditorSemanticDiagnosticKind::ParserRecovery
                                && diagnostic.span
                                    == Some(SourceSpan::new(source.len(), source.len()))
                        }),
                        "{source:?}"
                    );
                }
            }
        }
    }

    #[cfg(feature = "editor-facts")]
    #[test]
    fn requirement_multiline_accessibility_facts_keep_exact_original_ranges() {
        for ending in ["\n", "\r\n"] {
            let source = format!(
                concat!(
                    "requirementDiagram{ending}",
                    "\u{feff}accDescr{ending} \u{feff}{{{ending}",
                    "Help  {ending}  Next #%%;{ending}}}",
                    "accTitle{ending}%% Removed{ending}:{ending}\u{0085}Accessible\u{0085}\u{feff}{ending}",
                    "element E {{{ending}}}{ending}",
                ),
                ending = ending,
            );
            let crate::SemanticSnapshotCapture::Snapshot(snapshot) =
                crate::SemanticParser::new().capture_diagram_snapshot_sync(&source)
            else {
                panic!("expected Requirement snapshot");
            };
            let (_, outcome, editor_facts) = snapshot.into_parts();
            let crate::SemanticParseOutcome::Parsed(
                merman_model::semantic::SemanticDiagramModel::Requirement(model),
            ) = outcome
            else {
                panic!("expected Requirement model");
            };
            assert_eq!(
                model.acc_title.as_deref(),
                Some("\u{0085}Accessible\u{0085}")
            );
            assert_eq!(model.acc_descr.as_deref(), Some("Help  \nNext #%%;"));
            let crate::ParsedEditorFacts::Available(facts) = editor_facts else {
                panic!("expected Requirement editor facts");
            };
            assert!(facts.diagnostics.is_empty());
            let block = facts
                .symbols
                .iter()
                .find(|symbol| {
                    symbol.detail.as_deref() == Some("requirement accessibility description")
                })
                .unwrap();
            let content = format!("Help  {ending}  Next #%%;");
            let content_start = source.find("Help").unwrap();
            assert_eq!(block.name, "Help  \n  Next #%%;");
            assert_eq!(
                block.selection,
                SourceSpan::new(content_start, content_start + content.len())
            );
            assert_eq!(&source[block.selection.start..block.selection.end], content);
            assert_eq!(
                block.span,
                SourceSpan::new(
                    source.find("accDescr").unwrap(),
                    source.find('}').unwrap() + 1
                )
            );

            let title = facts
                .symbols
                .iter()
                .find(|symbol| symbol.detail.as_deref() == Some("requirement accessibility title"))
                .unwrap();
            let value_start = source.find('\u{0085}').unwrap();
            assert_eq!(title.name, "\u{0085}Accessible\u{0085}");
            assert_eq!(
                title.selection,
                SourceSpan::new(value_start, value_start + title.name.len())
            );
            assert_eq!(
                &source[title.selection.start..title.selection.end],
                title.name
            );
            assert_eq!(title.span.start, source.find("accTitle").unwrap());
            assert!(source[title.span.start..title.span.end].contains("%% Removed"));
            assert!(facts.expected_syntax.iter().any(|syntax| {
                syntax.kind == EditorExpectedSyntaxKind::Payload && syntax.span == title.selection
            }));
            let entity = facts
                .symbols
                .iter()
                .find(|symbol| symbol.name == "E")
                .unwrap();
            assert_eq!(&source[entity.selection.start..entity.selection.end], "E");
        }
    }

    #[test]
    fn requirement_accessibility_scans_observe_cancellation_inside_long_tokens() {
        for source in [
            format!("accTitle{}:Value", "\u{feff}\n".repeat(4096)),
            format!("accTitle:{}", "\u{0085}".repeat(8192)),
            format!("accDescr{{{}", "accDescr{\n".repeat(1024)),
            format!("accDescr{{{}}}", "\u{0085}".repeat(8192)),
        ] {
            let control = OperationControl::new();
            control.cancel_after_checkpoints(5);
            assert!(
                parse_requirement_accessibility(&mut LineCursor::new(&source), 0, &control)
                    .is_err()
            );
        }
    }

    #[test]
    fn requirement_unclosed_accessibility_owns_recovery_tail_once() {
        let source = format!("accDescr{{\n{}", "accDescr{\n".repeat(512));
        let control = OperationControl::new();
        control.cancel_after_checkpoints(128);
        let mut cursor = LineCursor::new(&source);
        cursor.next_line();
        let parsed = parse_requirement_accessibility(&mut cursor, 0, &control)
            .unwrap()
            .unwrap();
        assert!(!parsed.complete);
        assert_eq!(parsed.statement_span, SourceSpan::new(0, source.len()));
        assert_eq!(cursor.offset(), source.len());
        assert!(cursor.next_line().is_none());
        assert!(
            parsed
                .value
                .unwrap()
                .text
                .starts_with("accDescr{\naccDescr{")
        );
    }

    #[cfg(feature = "editor-facts")]
    #[test]
    fn requirement_closed_block_suffix_errors_recover_at_the_original_range() {
        for ending in ["\n", "\r\n"] {
            let source = format!(
                concat!(
                    "requirementDiagram{ending}",
                    "accDescr{ending}{{Help}}unexpected{ending}",
                    "element E {{{ending}}}{ending}",
                ),
                ending = ending,
            );
            let parser = crate::SemanticParser::new();
            assert!(parser.parse_diagram_sync(&source).is_err());
            let crate::SemanticSnapshotCapture::Snapshot(snapshot) =
                parser.capture_diagram_snapshot_sync(&source)
            else {
                panic!("expected recovered Requirement snapshot");
            };
            let crate::ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                panic!("expected Requirement editor facts");
            };
            assert_eq!(
                facts.completeness,
                crate::EditorSemanticCompleteness::Recovered
            );
            let bad_start = source.find("unexpected").unwrap();
            assert!(facts.diagnostics.iter().any(|diagnostic| {
                diagnostic.span == Some(SourceSpan::new(bad_start, bad_start + "unexpected".len()))
            }));
            for name in ["Help", "E"] {
                let symbol = facts
                    .symbols
                    .iter()
                    .find(|symbol| symbol.name == name)
                    .unwrap();
                assert_eq!(&source[symbol.selection.start..symbol.selection.end], name);
            }
        }
    }

    #[test]
    fn semantic_model_preserves_domain_records_without_editor_work() {
        let metadata = metadata();
        let source = concat!(
            "requirementDiagram\n",
            "accTitle: <script>secret</script>Public\n",
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
        reset_requirement_syntax_construction_count();
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let model = parse_requirement_semantic_model_controlled(
            source,
            &metadata,
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();
        assert_eq!(model.acc_title.as_deref(), Some("Public"));
        assert_eq!(model.direction, "LR");
        assert_eq!(model.requirements[0].requirement_id, "REQ-1");
        assert_eq!(model.requirements[0].risk, "High");
        assert_eq!(model.requirements[0].verify_method, "Test");
        assert!(
            model.requirements[0]
                .classes
                .iter()
                .any(|name| name == "critical")
        );
        assert_eq!(model.elements[0].element_type, "service");
        assert_eq!(model.relationships[0].src, "login");
        assert_eq!(model.relationships[0].dst, "api");
        assert_eq!(requirement_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        #[cfg(feature = "presentation")]
        {
            let full_metadata = crate::SourcePresentationMetadata {
                diagram_type: metadata.diagram_type.clone(),
                config: metadata.config.clone(),
                effective_config: crate::presentation::SourcePresentationConfig::from_config(
                    metadata.semantic_config.clone(),
                ),
                title: metadata.title.clone(),
            };
            parse_requirement_model_for_render_controlled(
                source,
                &full_metadata,
                &crate::OperationControl::new(),
            )
            .unwrap()
            .unwrap();
            #[cfg(feature = "editor-facts")]
            assert_eq!(crate::editor::test_work::read(), Default::default());

            #[cfg(feature = "compat-json")]
            {
                parse_requirement(source, &full_metadata).unwrap();
                #[cfg(feature = "editor-facts")]
                assert_eq!(crate::editor::test_work::read(), Default::default());
            }
        }
    }

    #[test]
    fn semantic_errors_keep_exact_spans_and_observe_cancellation() {
        let source = "requirementDiagram\nrequirement login {\n  risk: impossible\n}\n";
        let metadata = metadata();
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let error = parse_requirement_semantic_model_controlled(
            source,
            &metadata,
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected structured parse diagnostic");
        };
        let start = source.find("impossible").unwrap();
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(start, start + "impossible".len())),
        );
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        let control = crate::OperationControl::new();
        control.cancel();
        assert!(
            parse_requirement_semantic_model_controlled(
                source,
                &metadata,
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &control
            )
            .is_err()
        );
    }
}
