//! Controlled Requirement statement scanning and semantic construction.

use merman_core_preprocess::scan::{LineCursor, starts_with_case_insensitive};
use merman_model::families::requirement::{
    RequirementClass, RequirementDiagramModel, RequirementElement, RequirementNode,
    RequirementRelationship,
};
use merman_model::text::is_ecmascript_whitespace;
use merman_model::{OperationControl, OperationControlResult, SourceSpan};
use std::collections::{BTreeMap, HashMap};

type SyntaxResult<T> = Result<T, String>;

#[derive(Debug)]
pub struct RequirementSourceIssue {
    pub message: String,
    pub location: RequirementIssueLocation,
}

#[derive(Debug)]
pub enum RequirementIssueLocation {
    Exact(SourceSpan),
    InsertionPoint(usize),
}

impl RequirementSourceIssue {
    fn exact(message: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            message: message.into(),
            location: RequirementIssueLocation::Exact(span),
        }
    }

    fn insertion_point(message: impl Into<String>, offset: usize) -> Self {
        Self {
            message: message.into(),
            location: RequirementIssueLocation::InsertionPoint(offset),
        }
    }
}

#[cfg(feature = "editor-facts")]
#[derive(Clone, Copy)]
pub enum RequirementFieldKind<'a> {
    Requirement(&'a str),
    Element,
    Class,
    ClassTarget,
    ClassDefinition,
    StyleTarget,
    Style,
    ClassStyle,
    Relationship,
    RelationshipSource,
    RelationshipTarget,
    Id,
    Text,
    Risk,
    VerifyMethod,
    ElementType,
    DocRef,
}

#[cfg(feature = "editor-facts")]
pub enum RequirementObservation<'a> {
    CaptureStarted,
    Direction {
        value: Option<&'static str>,
        span: SourceSpan,
        selection: SourceSpan,
    },
    Accessibility {
        directive: &'static str,
        value: Option<(&'a str, SourceSpan)>,
        span: SourceSpan,
        complete: bool,
    },
    Field {
        kind: RequirementFieldKind<'a>,
        text: &'a str,
        span: SourceSpan,
        selection: SourceSpan,
    },
    IncompleteAccessibility(SourceSpan),
    ResolveReferences(RequirementEntities<'a>),
}

#[cfg(feature = "editor-facts")]
type Observer<'a> = dyn FnMut(RequirementObservation<'_>) + 'a;

#[cfg(feature = "editor-facts")]
struct RequirementObserver<'a> {
    capture_fields: bool,
    observe: Option<&'a mut Observer<'a>>,
}

#[cfg(feature = "editor-facts")]
impl<'a> RequirementObserver<'a> {
    fn rich(&mut self) -> Option<&mut Observer<'a>> {
        if self.capture_fields {
            self.observe.as_deref_mut()
        } else {
            None
        }
    }
}

#[cfg(feature = "editor-facts")]
pub struct RequirementEntities<'a>(&'a RequirementDb);

#[cfg(feature = "editor-facts")]
pub enum RequirementEntityKind {
    Requirement,
    Element,
}

#[cfg(feature = "editor-facts")]
impl RequirementEntities<'_> {
    pub fn get(&self, name: &str) -> Option<RequirementEntityKind> {
        if self.0.requirements.contains_key(name) {
            Some(RequirementEntityKind::Requirement)
        } else if self.0.elements.contains_key(name) {
            Some(RequirementEntityKind::Element)
        } else {
            None
        }
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

#[derive(Debug, Clone, Copy)]
struct SpannedValue<'a> {
    text: &'a str,
    start: usize,
}

fn requirement_statement_start(line: &str, line_start: usize) -> usize {
    line_start + line.len() - line.trim_start_matches(is_ecmascript_whitespace).len()
}

fn leading_whitespace_len(input: &str) -> usize {
    input.len() - input.trim_start().len()
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

/// Scans one preprocessed source after the caller's entry checkpoint.
pub fn parse_requirement_source_controlled(
    code: &str,
    control: &OperationControl,
) -> OperationControlResult<Result<RequirementDiagramModel, RequirementSourceIssue>> {
    parse_requirement_source(
        code,
        #[cfg(feature = "editor-facts")]
        false,
        #[cfg(feature = "editor-facts")]
        None,
        control,
    )
}

/// Scans the same source with optional editor observation.
///
/// Spans are relative to `code`. An observer receives incomplete accessibility events even
/// with rich capture disabled. The draft is retained through reference binding on failure.
#[cfg(feature = "editor-facts")]
pub fn parse_requirement_source_observed_controlled(
    code: &str,
    capture_fields: bool,
    observe: Option<&mut dyn FnMut(RequirementObservation<'_>)>,
    control: &OperationControl,
) -> OperationControlResult<Result<RequirementDiagramModel, RequirementSourceIssue>> {
    parse_requirement_source(code, capture_fields, observe, control)
}

fn parse_requirement_source(
    code: &str,
    #[cfg(feature = "editor-facts")] capture_fields: bool,
    #[cfg(feature = "editor-facts")] observe: Option<&mut dyn FnMut(RequirementObservation<'_>)>,
    control: &OperationControl,
) -> OperationControlResult<Result<RequirementDiagramModel, RequirementSourceIssue>> {
    control.checkpoint()?;
    let mut db = RequirementDb::new();
    let mut acc_title: Option<String> = None;
    let mut acc_descr: Option<String> = None;
    #[cfg(feature = "editor-facts")]
    let mut observer = RequirementObserver {
        capture_fields,
        observe,
    };
    #[cfg(feature = "editor-facts")]
    if let Some(observe) = observer.rich() {
        observe(RequirementObservation::CaptureStarted);
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
                if let Some(observe) = observer.rich() {
                    observe(direction.observation(SourceSpan::new(line_start, end)));
                }
                if let Some(value) = direction.value {
                    db.set_direction(value);
                }
            } else {
                first_error.get_or_insert(RequirementSourceIssue::exact(
                    "expected requirementDiagram",
                    SourceSpan::new(line_start, end),
                ));
            }
            continue;
        }
        if let Some(parsed) = accessibility {
            #[cfg(feature = "editor-facts")]
            if let Some(observe) = observer.rich() {
                observe(parsed.observation());
            }
            if !parsed.complete {
                #[cfg(feature = "editor-facts")]
                if let Some(observe) = observer.observe.as_deref_mut() {
                    observe(RequirementObservation::IncompleteAccessibility(
                        parsed.statement_span,
                    ));
                }
                first_error.get_or_insert_with(|| {
                    RequirementSourceIssue::insertion_point(
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
            first_error.get_or_insert(RequirementSourceIssue::exact(
                "expected requirementDiagram",
                requirement_statement_span(raw, line_start),
            ));
            continue;
        }

        if let Some(ch) = t.chars().next().filter(|ch| ch.is_whitespace()) {
            first_error.get_or_insert(RequirementSourceIssue::exact(
                "unexpected non-ECMAScript whitespace at requirement statement",
                SourceSpan::new(statement_start, statement_start + ch.len_utf8()),
            ));
            continue;
        }

        if let Some(direction) = parse_direction(t, statement_start) {
            #[cfg(feature = "editor-facts")]
            if let Some(observe) = observer.rich() {
                observe(direction.observation(requirement_statement_span(raw, line_start)));
            }
            if let Some(value) = direction.value {
                db.set_direction(value);
            } else {
                first_error.get_or_insert(RequirementSourceIssue::exact(
                    "invalid requirement direction",
                    direction.selection,
                ));
            }
            continue;
        }

        let requirement_open = match parse_requirement_def_open(t, statement_start) {
            Ok(open) => open,
            Err(error) => {
                first_error.get_or_insert(RequirementSourceIssue::exact(
                    error,
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
            if let Some(observe) = observer.rich() {
                let span = requirement_statement_span(raw, line_start);
                emit_field(
                    observe,
                    RequirementFieldKind::Requirement(&ty),
                    &name.value,
                    span,
                    name.selection,
                );
                if let Some(classes) = &classes {
                    emit_ids(observe, classes, RequirementFieldKind::Class, span);
                }
            }
            let body = parse_requirement_body(
                code,
                &mut lines,
                #[cfg(feature = "editor-facts")]
                observer.rich(),
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
                first_error.get_or_insert(RequirementSourceIssue::exact(
                    error,
                    requirement_statement_span(raw, line_start),
                ));
                continue;
            }
        };
        if let Some(ElementDefOpen { name, classes }) = element_open {
            #[cfg(feature = "editor-facts")]
            if let Some(observe) = observer.rich() {
                let span = requirement_statement_span(raw, line_start);
                emit_field(
                    observe,
                    RequirementFieldKind::Element,
                    &name.value,
                    span,
                    name.selection,
                );
                if let Some(classes) = &classes {
                    emit_ids(observe, classes, RequirementFieldKind::Class, span);
                }
            }
            let body = parse_element_body(
                code,
                &mut lines,
                #[cfg(feature = "editor-facts")]
                observer.rich(),
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
                first_error.get_or_insert(RequirementSourceIssue::exact(
                    error,
                    requirement_statement_span(raw, line_start),
                ));
                continue;
            }
        };
        if let Some(statement) = shorthand {
            #[cfg(feature = "editor-facts")]
            if let Some(observe) = observer.rich() {
                let span = requirement_statement_span(raw, line_start);
                emit_field(
                    observe,
                    RequirementFieldKind::ClassTarget,
                    &statement.target.value,
                    span,
                    statement.target.selection,
                );
                emit_ids(
                    observe,
                    &statement.classes,
                    RequirementFieldKind::Class,
                    span,
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
                first_error.get_or_insert(RequirementSourceIssue::exact(
                    error,
                    requirement_statement_span(raw, line_start),
                ));
                continue;
            }
        };
        if let Some(statement) = style {
            #[cfg(feature = "editor-facts")]
            if let Some(observe) = observer.rich() {
                let span = requirement_statement_span(raw, line_start);
                emit_ids(
                    observe,
                    &statement.ids,
                    RequirementFieldKind::StyleTarget,
                    span,
                );
                emit_styles(observe, &statement.styles, RequirementFieldKind::Style);
            }
            db.set_css_style(&statement.ids.values(), &statement.styles.values());
            continue;
        }

        let class_def = match parse_classdef_stmt(t, statement_start) {
            Ok(statement) => statement,
            Err(error) => {
                first_error.get_or_insert(RequirementSourceIssue::exact(
                    error,
                    requirement_statement_span(raw, line_start),
                ));
                continue;
            }
        };
        if let Some(statement) = class_def {
            #[cfg(feature = "editor-facts")]
            if let Some(observe) = observer.rich() {
                let span = requirement_statement_span(raw, line_start);
                emit_ids(
                    observe,
                    &statement.ids,
                    RequirementFieldKind::ClassDefinition,
                    span,
                );
                emit_styles(observe, &statement.styles, RequirementFieldKind::ClassStyle);
            }
            db.define_class(&statement.ids.values(), &statement.styles.values());
            continue;
        }

        let class = match parse_class_stmt(t, statement_start) {
            Ok(statement) => statement,
            Err(error) => {
                first_error.get_or_insert(RequirementSourceIssue::exact(
                    error,
                    requirement_statement_span(raw, line_start),
                ));
                continue;
            }
        };
        if let Some(statement) = class {
            #[cfg(feature = "editor-facts")]
            if let Some(observe) = observer.rich() {
                let span = requirement_statement_span(raw, line_start);
                emit_ids(
                    observe,
                    &statement.classes,
                    RequirementFieldKind::Class,
                    span,
                );
                emit_ids(
                    observe,
                    &statement.targets,
                    RequirementFieldKind::ClassTarget,
                    span,
                );
            }
            db.set_class(&statement.targets.values(), &statement.classes.values());
            continue;
        }

        let relationship = match parse_relationship_stmt(t, statement_start) {
            Ok(statement) => statement,
            Err(error) => {
                first_error.get_or_insert(RequirementSourceIssue::exact(
                    error,
                    requirement_statement_span(raw, line_start),
                ));
                continue;
            }
        };
        if let Some(relationship) = relationship {
            #[cfg(feature = "editor-facts")]
            if let Some(observe) = observer.rich() {
                let span = requirement_statement_span(raw, line_start);
                emit_field(
                    observe,
                    RequirementFieldKind::Relationship,
                    &relationship.kind,
                    span,
                    relationship.kind_span,
                );
                emit_field(
                    observe,
                    RequirementFieldKind::RelationshipSource,
                    &relationship.source.value,
                    span,
                    relationship.source.selection,
                );
                emit_field(
                    observe,
                    RequirementFieldKind::RelationshipTarget,
                    &relationship.target.value,
                    span,
                    relationship.target.selection,
                );
            }
            db.add_relationship(
                &relationship.kind,
                &relationship.source.value,
                &relationship.target.value,
            );
            continue;
        }

        first_error.get_or_insert(RequirementSourceIssue::exact(
            format!("unexpected requirement statement: {t}"),
            requirement_statement_span(raw, line_start),
        ));
    }

    if !saw_header {
        first_error.get_or_insert(RequirementSourceIssue::insertion_point(
            "expected requirementDiagram",
            code.len(),
        ));
    }

    #[cfg(feature = "editor-facts")]
    if let Some(observe) = observer.rich() {
        observe(RequirementObservation::ResolveReferences(
            RequirementEntities(&db),
        ));
    }

    if let Some(error) = first_error {
        return Ok(Err(error));
    }

    control.checkpoint()?;
    Ok(Ok(db.to_render_model(acc_title, acc_descr)))
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

#[cfg(feature = "editor-facts")]
impl RequirementAccessibility<'_> {
    fn observation(&self) -> RequirementObservation<'_> {
        RequirementObservation::Accessibility {
            directive: self.key,
            value: self.value.map(|value| {
                let selection = SourceSpan::new(value.start, value.start + value.text.len());
                (value.text, selection)
            }),
            span: self.statement_span,
            complete: self.complete,
        }
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
fn emit_field(
    observe: &mut Observer<'_>,
    kind: RequirementFieldKind<'_>,
    text: &str,
    span: SourceSpan,
    selection: SourceSpan,
) {
    observe(RequirementObservation::Field {
        kind,
        text,
        span,
        selection,
    });
}

#[cfg(feature = "editor-facts")]
fn emit_ids(
    observe: &mut Observer<'_>,
    ids: &ParsedRequirementIdList,
    kind: RequirementFieldKind<'_>,
    span: SourceSpan,
) {
    for id in &ids.items {
        emit_field(observe, kind, &id.value, span, id.selection);
    }
}

#[cfg(feature = "editor-facts")]
fn emit_styles(
    observe: &mut Observer<'_>,
    styles: &ParsedRequirementStyles,
    kind: RequirementFieldKind<'_>,
) {
    for style in &styles.items {
        emit_field(observe, kind, &style.value, style.span, style.span);
    }
}

#[cfg(feature = "editor-facts")]
fn emit_body_value(
    observe: &mut Observer<'_>,
    kind: RequirementFieldKind<'_>,
    text: &str,
    start: usize,
) {
    let text = text.trim();
    if !text.is_empty() {
        let span = SourceSpan::new(start, start + text.len());
        emit_field(observe, kind, text, span, span);
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

#[cfg(feature = "editor-facts")]
impl ParsedRequirementDirection {
    fn observation(&self, span: SourceSpan) -> RequirementObservation<'_> {
        RequirementObservation::Direction {
            value: self.value,
            span,
            selection: self.selection,
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
) -> SyntaxResult<Option<RequirementDefOpen>> {
    let t = t.trim();
    if !t.ends_with('{') {
        return Ok(None);
    }

    let without_brace = t[..t.len() - 1].trim_end();
    let (ty_raw, rest) = split_first_word(without_brace)
        .ok_or_else(|| "invalid requirement definition".to_string())?;

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
        return Err("requirement name is empty".to_string());
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

fn parse_element_def_open(t: &str, statement_start: usize) -> SyntaxResult<Option<ElementDefOpen>> {
    let t = t.trim();
    if !t.ends_with('{') {
        return Ok(None);
    }

    let without_brace = t[..t.len() - 1].trim_end();
    let (kw, rest) =
        split_first_word(without_brace).ok_or_else(|| "invalid element definition".to_string())?;
    if !kw.eq_ignore_ascii_case("element") {
        return Ok(None);
    }

    let rest_start = without_brace.len() - rest.len();
    let name_and_classes = split_name_and_classes(rest, statement_start + rest_start)?;
    if name_and_classes.name.value.is_empty() {
        return Err("element name is empty".to_string());
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

fn split_name_and_classes(input: &str, input_start: usize) -> SyntaxResult<DefinitionName> {
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

fn parse_id_or_name(input: &str) -> SyntaxResult<(String, &str)> {
    let input = input.trim_start();
    if input.starts_with('"') {
        if let Some((val, rest)) = parse_quoted_prefix(input) {
            return Ok((val, rest));
        }
        return Err("unterminated string".to_string());
    }
    Ok((input.trim().to_string(), ""))
}

fn parse_quoted_prefix(input: &str) -> Option<(String, &str)> {
    let end = requirement_quoted_end(input)?;
    Some((input[1..end - 1].to_string(), &input[end..]))
}

struct RequirementBodyParse<T> {
    value: T,
    error: Option<RequirementSourceIssue>,
}

fn parse_requirement_body<'a>(
    code: &'a str,
    lines: &mut LineCursor<'a>,
    #[cfg(feature = "editor-facts")] mut observe: Option<&mut Observer<'_>>,
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
            error.get_or_insert(RequirementSourceIssue::exact(
                format!("invalid requirement body line: {t}"),
                requirement_statement_span(raw, line_start),
            ));
            continue;
        };
        let k = key_value.key.text;
        let key = k.to_ascii_lowercase();
        let Some(value) = key_value.value else {
            error.get_or_insert(RequirementSourceIssue::exact(
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
            error.get_or_insert(RequirementSourceIssue::exact(
                format!("unexpected requirement body key: {k}"),
                requirement_statement_span(raw, line_start),
            ));
            continue;
        }
        #[cfg(feature = "editor-facts")]
        if let Some(observe) = observe.as_deref_mut() {
            let kind = match key.as_str() {
                "id" => RequirementFieldKind::Id,
                "text" => RequirementFieldKind::Text,
                "risk" => RequirementFieldKind::Risk,
                "verifymethod" => RequirementFieldKind::VerifyMethod,
                _ => unreachable!("requirement key was validated above"),
            };
            emit_body_value(observe, kind, value.text, value_span.start);
        }
        let parsed = match parse_simple_value(value.text) {
            Ok(parsed) => parsed,
            Err(parse_error) => {
                error.get_or_insert(RequirementSourceIssue::exact(parse_error, value_span));
                continue;
            }
        };
        match key.as_str() {
            "id" => b.requirement_id = parsed,
            "text" => b.text = parsed,
            "risk" => match normalize_risk(&parsed) {
                Ok(risk) => b.risk = risk,
                Err(parse_error) => {
                    error.get_or_insert(RequirementSourceIssue::exact(parse_error, value_span));
                }
            },
            "verifymethod" => match normalize_verify_method(&parsed) {
                Ok(method) => b.verify_method = method,
                Err(parse_error) => {
                    error.get_or_insert(RequirementSourceIssue::exact(parse_error, value_span));
                }
            },
            _ => unreachable!("body key was validated above"),
        }
    }

    error.get_or_insert(RequirementSourceIssue::insertion_point(
        "unterminated requirement block",
        lines.offset(),
    ));
    Ok(RequirementBodyParse { value: b, error })
}

fn parse_element_body<'a>(
    code: &'a str,
    lines: &mut LineCursor<'a>,
    #[cfg(feature = "editor-facts")] mut observe: Option<&mut Observer<'_>>,
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
            error.get_or_insert(RequirementSourceIssue::exact(
                format!("invalid element body line: {t}"),
                requirement_statement_span(raw, line_start),
            ));
            continue;
        };
        let k = key_value.key.text;
        let key = k.to_ascii_lowercase();
        let Some(value) = key_value.value else {
            error.get_or_insert(RequirementSourceIssue::exact(
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
            error.get_or_insert(RequirementSourceIssue::exact(
                format!("unexpected element body key: {k}"),
                requirement_statement_span(raw, line_start),
            ));
            continue;
        }
        #[cfg(feature = "editor-facts")]
        if let Some(observe) = observe.as_deref_mut() {
            let kind = match key.as_str() {
                "type" => RequirementFieldKind::ElementType,
                "docref" => RequirementFieldKind::DocRef,
                _ => unreachable!("element key was validated above"),
            };
            emit_body_value(observe, kind, value.text, value_span.start);
        }
        let parsed = match parse_simple_value(value.text) {
            Ok(parsed) => parsed,
            Err(parse_error) => {
                error.get_or_insert(RequirementSourceIssue::exact(parse_error, value_span));
                continue;
            }
        };
        match key.as_str() {
            "type" => b.element_type = parsed,
            "docref" => b.doc_ref = parsed,
            _ => unreachable!("element key was validated above"),
        }
    }

    error.get_or_insert(RequirementSourceIssue::insertion_point(
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

fn parse_simple_value(input: &str) -> SyntaxResult<String> {
    let input = input.trim();
    if input.starts_with('"') {
        if let Some((val, rest)) = parse_quoted_prefix(input) {
            if !rest.trim().is_empty() {
                return Err(format!(
                    "unexpected trailing tokens after string: {}",
                    rest.trim()
                ));
            }
            return Ok(val.trim().to_string());
        }
        return Err("unterminated string".to_string());
    }
    Ok(input.trim().to_string())
}

fn normalize_risk(input: &str) -> SyntaxResult<String> {
    match input.trim().to_ascii_lowercase().as_str() {
        "low" => Ok("Low".to_string()),
        "medium" => Ok("Medium".to_string()),
        "high" => Ok("High".to_string()),
        other => Err(format!("invalid risk level: {other}")),
    }
}

fn normalize_verify_method(input: &str) -> SyntaxResult<String> {
    match input.trim().to_ascii_lowercase().as_str() {
        "analysis" => Ok("Analysis".to_string()),
        "demonstration" => Ok("Demonstration".to_string()),
        "inspection" => Ok("Inspection".to_string()),
        "test" => Ok("Test".to_string()),
        other => Err(format!("invalid verify method: {other}")),
    }
}

fn parse_requirement_id_spanned(
    input: &str,
    _input_start: usize,
) -> SyntaxResult<ParsedRequirementId> {
    #[cfg(feature = "editor-facts")]
    let leading = leading_whitespace_len(input);
    let input = input.trim();
    #[cfg(feature = "editor-facts")]
    let token_start = _input_start + leading;
    let (value, rest) = parse_id_or_name(input)?;
    if !rest.trim().is_empty() {
        return Err(format!(
            "unexpected trailing tokens after identifier: {}",
            rest.trim()
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
) -> SyntaxResult<Option<ParsedRequirementClassShorthand>> {
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
        return Err(format!("invalid class shorthand statement: {t}"));
    }
    let target = parse_requirement_id_spanned(left, statement_start)?;
    let classes = parse_id_list_all_spanned(right, statement_start + pos + 3 + right_leading)?;
    Ok(Some(ParsedRequirementClassShorthand { target, classes }))
}

fn parse_style_stmt(
    t: &str,
    statement_start: usize,
) -> SyntaxResult<Option<ParsedRequirementStyleStatement>> {
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
        return Err(format!("invalid style statement: {t}"));
    }
    Ok(Some(ParsedRequirementStyleStatement {
        ids: id_tokens,
        styles,
    }))
}

fn parse_classdef_stmt(
    t: &str,
    statement_start: usize,
) -> SyntaxResult<Option<ParsedRequirementStyleStatement>> {
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
        return Err(format!("invalid classDef statement: {t}"));
    }
    Ok(Some(ParsedRequirementStyleStatement {
        ids: id_tokens,
        styles,
    }))
}

fn parse_class_stmt(
    t: &str,
    statement_start: usize,
) -> SyntaxResult<Option<ParsedRequirementClassStatement>> {
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
        return Err(format!("invalid class statement: {t}"));
    }
    Ok(Some(ParsedRequirementClassStatement {
        targets: id_tokens,
        classes: class_tokens,
    }))
}

fn parse_relationship_stmt(
    t: &str,
    statement_start: usize,
) -> SyntaxResult<Option<ParsedRequirementRelationship>> {
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
            return Err(format!("invalid relationship statement: {rest}"));
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
            return Err(format!("invalid relationship statement: {left_part}"));
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

fn normalize_relationship(input: &str) -> SyntaxResult<String> {
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
) -> SyntaxResult<(ParsedRequirementIdList, &str, usize)> {
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
            let (value, rest) =
                parse_quoted_prefix(cur).ok_or_else(|| "unterminated string".to_string())?;
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
                return Err("expected identifier".to_string());
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

fn parse_id_list_all_spanned(
    input: &str,
    input_start: usize,
) -> SyntaxResult<ParsedRequirementIdList> {
    let mut out = Vec::new();
    let mut cur = input.trim_start();
    let mut cursor = input.len() - cur.len();
    while !cur.is_empty() {
        let leading = cur.len() - cur.trim_start().len();
        cursor += leading;
        cur = cur.trim_start();
        let (item, rest, _selection) = if cur.starts_with('"') {
            let (item, rest) =
                parse_quoted_prefix(cur).ok_or_else(|| "unterminated string".to_string())?;
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

#[cfg(test)]
mod tests;
