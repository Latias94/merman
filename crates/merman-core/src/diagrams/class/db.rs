use crate::MermaidConfig;
#[cfg(feature = "compat-json")]
use crate::SourcePresentationMetadata;
use crate::common::parse_generic_types;
use crate::sanitize::sanitize_text;
use crate::utils::format_url;
use indexmap::IndexMap;
use merman_model::families::class as class_typed;
use merman_model::text::is_ecmascript_whitespace;
use serde_json::Value;
use std::collections::BTreeMap;

use super::ast::{Action, RelationData};
use super::{
    LINE_DOTTED, LINE_SOLID, MERMAID_DOM_ID_PREFIX, REL_AGGREGATION, REL_COMPOSITION,
    REL_DEPENDENCY, REL_EXTENSION, REL_LOLLIPOP, REL_NONE,
};

#[derive(Debug, Clone)]
struct ClassMember {
    member_type: String,
    visibility: String,
    id: String,
    classifier: String,
    parameters: String,
    return_type: String,
    display_text: String,
    css_style: String,
}

impl ClassMember {
    fn new(input: &str, member_type: &str, config: &MermaidConfig) -> Result<Self, String> {
        let input = sanitize_text(input, config);
        let mut m = Self {
            member_type: member_type.to_string(),
            visibility: String::new(),
            id: String::new(),
            classifier: String::new(),
            parameters: String::new(),
            return_type: String::new(),
            display_text: String::new(),
            css_style: String::new(),
        };
        m.parse_member(&input, member_type)?;
        Ok(m)
    }

    fn parse_method_signature_like_upstream(input: &str) -> Option<(&str, &str, &str, &str, &str)> {
        // The selected unanchored regex uses greedy, non-dotall name/parameter groups.
        // Scan each line once; only its optional post-parenthesis separator can cross a line.
        let mut offset = 0;
        for chunk in input.split_inclusive(is_js_regex_line_terminator) {
            let line = chunk.trim_end_matches(is_js_regex_line_terminator);
            let start = offset;
            offset += chunk.len();
            let Some(close) = line.rfind(')') else {
                continue;
            };
            let Some(open) = line[..close].rfind('(') else {
                continue;
            };
            if open == 0 {
                continue;
            }
            let (visibility, name) =
                if open > 1 && matches!(line.as_bytes()[0], b'#' | b'+' | b'~' | b'-') {
                    (&line[..1], &line[1..open])
                } else {
                    ("", &line[..open])
                };
            let params = line[open + 1..close].trim_matches(is_ecmascript_whitespace);
            let after_paren = &input[start + close + 1..];
            let (classifier, tail) = match after_paren.chars().next() {
                Some(ch @ ('$' | '*')) => (&after_paren[..1], &after_paren[ch.len_utf8()..]),
                Some(ch) if is_ecmascript_whitespace(ch) => ("", &after_paren[ch.len_utf8()..]),
                _ => ("", after_paren),
            };
            let end = tail.find(is_js_regex_line_terminator).unwrap_or(tail.len());
            let return_type = tail[..end].trim_matches(is_ecmascript_whitespace);
            return Some((visibility, name, params, classifier, return_type));
        }
        None
    }

    fn parse_member(&mut self, input: &str, member_type: &str) -> Result<(), String> {
        // AddMember trims authored text before sanitization. A second trim here
        // would reinterpret a trailing classifier after removed markup exposes space.
        if member_type == "method" {
            let (visibility, id, params, classifier, return_type) =
                Self::parse_method_signature_like_upstream(input)
                    .ok_or_else(|| "Invalid class method signature".to_string())?;
            self.visibility = visibility.to_string();
            self.id = id.to_string();
            self.parameters = params.to_string();
            self.classifier = classifier.to_string();
            self.return_type = return_type.to_string();

            if self.classifier.is_empty()
                && let Some(last) = self.return_type.chars().last()
                && (last == '$' || last == '*')
            {
                self.classifier = last.to_string();
                self.return_type.pop();
            }
        } else {
            let first = input.chars().next().unwrap_or('\0');
            let last = input.chars().last().unwrap_or('\0');
            let mut start = 0usize;
            let mut end = input.len();
            if matches!(first, '#' | '+' | '~' | '-') {
                self.visibility = first.to_string();
                start = first.len_utf8();
            }
            if last == '$' || last == '*' {
                self.classifier = last.to_string();
                end = input.len() - last.len_utf8();
            }
            self.id = input[start..end].to_string();
        }

        if self.id.starts_with(' ') {
            self.id = format!(" {}", self.id.trim_matches(is_ecmascript_whitespace));
        } else {
            self.id = self.id.trim_matches(is_ecmascript_whitespace).to_string();
        }

        self.css_style = match self.classifier.as_str() {
            "*" => "font-style:italic;".to_string(),
            "$" => "text-decoration:underline;".to_string(),
            _ => String::new(),
        };

        let mut display = format!("{}{}", self.visibility, parse_generic_types(&self.id));
        if member_type == "method" {
            display.push('(');
            display.push_str(&parse_generic_types(
                self.parameters.trim_matches(is_ecmascript_whitespace),
            ));
            display.push(')');
            if !self.return_type.is_empty() {
                display.push_str(" : ");
                display.push_str(&parse_generic_types(&self.return_type));
            }
        }
        self.display_text = display.trim_matches(is_ecmascript_whitespace).to_string();
        Ok(())
    }

    fn into_typed(self) -> class_typed::ClassMember {
        class_typed::ClassMember {
            member_type: self.member_type,
            visibility: self.visibility,
            id: self.id,
            classifier: self.classifier,
            parameters: self.parameters,
            return_type: self.return_type,
            display_text: self.display_text,
            css_style: self.css_style,
        }
    }
}

fn is_js_regex_line_terminator(ch: char) -> bool {
    matches!(ch, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

#[derive(Debug, Clone)]
struct ClassNode {
    id: String,
    type_param: String,
    label: String,
    text: String,
    css_classes: String,
    methods: Vec<ClassMember>,
    members: Vec<ClassMember>,
    annotations: Vec<String>,
    styles: Vec<String>,
    parent: Option<String>,
    link: Option<String>,
    link_target: Option<String>,
    tooltip: Option<String>,
    have_callback: bool,
    callback: Option<serde_json::Map<String, Value>>,
    callback_effective: bool,
    relation_endpoint_facade_candidate: bool,
}

impl ClassNode {
    fn into_typed(self) -> class_typed::ClassNode {
        class_typed::ClassNode {
            id: self.id,
            type_param: self.type_param,
            label: self.label,
            text: self.text,
            css_classes: self.css_classes,
            methods: self
                .methods
                .into_iter()
                .map(ClassMember::into_typed)
                .collect(),
            members: self
                .members
                .into_iter()
                .map(ClassMember::into_typed)
                .collect(),
            annotations: self.annotations,
            styles: self.styles,
            parent: self.parent,
            link: self.link,
            link_target: self.link_target,
            tooltip: self.tooltip,
            have_callback: self.have_callback,
            callback: self.callback,
            callback_effective: self.callback_effective,
        }
    }
}

#[derive(Debug, Clone)]
struct ClassNote {
    id: String,
    class_id: Option<String>,
    text: String,
    parent: Option<String>,
}

impl ClassNote {
    fn into_typed(self) -> class_typed::ClassNote {
        class_typed::ClassNote {
            id: self.id,
            class_id: self.class_id,
            text: self.text,
            parent: self.parent,
        }
    }
}

#[derive(Debug, Clone)]
struct Interface {
    id: String,
    label: String,
    class_id: String,
}

impl Interface {
    fn into_typed(self) -> class_typed::ClassInterface {
        class_typed::ClassInterface {
            id: self.id,
            label: self.label,
            class_id: self.class_id,
        }
    }
}

#[derive(Debug, Clone)]
struct Namespace {
    id: String,
    label: String,
    class_ids: Vec<String>,
    note_ids: Vec<String>,
    parent: Option<String>,
    explicit: bool,
}

impl Namespace {
    fn into_typed(self) -> class_typed::Namespace {
        class_typed::Namespace {
            id: self.id,
            label: self.label,
            class_ids: self.class_ids,
            note_ids: self.note_ids,
            parent: self.parent,
            explicit: self.explicit,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct StyleClass {
    id: String,
    styles: Vec<String>,
    text_styles: Vec<String>,
}

impl StyleClass {
    fn into_typed(self) -> class_typed::StyleClass {
        class_typed::StyleClass {
            id: self.id,
            styles: self.styles,
            text_styles: self.text_styles,
        }
    }
}

#[derive(Debug, Clone)]
struct ExplicitCssAssignment {
    target: String,
    style_event_ordinal: usize,
}

#[derive(Debug, Clone)]
struct PaintClassDefinition {
    styles: Vec<String>,
    style_event_ordinal: usize,
}

#[derive(Debug, Default)]
struct StylePrecedenceObservations {
    assignment_before_definition_copy: Option<class_typed::ClassStylePrecedenceWitness>,
    definition_before_assignment_no_backfill: Option<class_typed::ClassStylePrecedenceWitness>,
    inline_paint: Option<class_typed::ClassStyleDeclarationWitness>,
    classdef_typography: Option<class_typed::ClassStyleDeclarationWitness>,
}

#[derive(Debug)]
pub(super) struct ClassDb<'a> {
    direction: String,
    classes: IndexMap<String, ClassNode>,
    relations: Vec<RelationData>,
    notes: Vec<ClassNote>,
    interfaces: Vec<Interface>,
    namespaces: IndexMap<String, Namespace>,
    namespace_stack: Vec<String>,
    style_classes: IndexMap<String, StyleClass>,
    style_precedence_facts: StylePrecedenceObservations,
    explicit_css_assignments: BTreeMap<String, ExplicitCssAssignment>,
    paint_class_definitions: BTreeMap<String, PaintClassDefinition>,
    next_style_event_ordinal: usize,
    acc_title: Option<String>,
    acc_descr: Option<String>,
    security_level: Option<&'a str>,
    config: &'a MermaidConfig,
}

impl<'a> ClassDb<'a> {
    pub(super) fn new(config: &'a MermaidConfig) -> Self {
        Self {
            direction: "TB".to_string(),
            classes: IndexMap::new(),
            relations: Vec::new(),
            notes: Vec::new(),
            interfaces: Vec::new(),
            namespaces: IndexMap::new(),
            namespace_stack: Vec::new(),
            style_classes: IndexMap::new(),
            style_precedence_facts: StylePrecedenceObservations::default(),
            explicit_css_assignments: BTreeMap::new(),
            paint_class_definitions: BTreeMap::new(),
            next_style_event_ordinal: 0,
            acc_title: None,
            acc_descr: None,
            security_level: config.get_str("securityLevel"),
            config,
        }
    }
}

impl<'a> ClassDb<'a> {
    fn split_class_name_and_type(&self, id: &str) -> (String, String) {
        let id = sanitize_text(id, self.config);
        let (left, right) =
            if let Some((left, right)) = id.split_once('~').filter(|(left, _)| !left.is_empty()) {
                (
                    left.to_string(),
                    right.split('~').next().unwrap_or("").to_string(),
                )
            } else {
                (id, String::new())
            };

        let class_name = sanitize_text(&left, self.config);
        let type_param = if right.is_empty() {
            right
        } else {
            sanitize_text(&right, self.config)
        };

        (class_name, type_param)
    }

    fn add_class_with_origin(&mut self, id: &str, relation_endpoint_facade_candidate: bool) {
        let (class_name, type_param) = self.split_class_name_and_type(id);
        if let Some(class) = self.classes.get_mut(&class_name) {
            if !relation_endpoint_facade_candidate {
                class.relation_endpoint_facade_candidate = false;
            }
            return;
        }
        let text = if type_param.is_empty() {
            class_name.clone()
        } else {
            format!("{class_name}&lt;{type_param}&gt;")
        };
        self.classes.insert(
            class_name.clone(),
            ClassNode {
                id: class_name.clone(),
                type_param: type_param.clone(),
                label: class_name.clone(),
                text,
                css_classes: "default".to_string(),
                methods: Vec::new(),
                members: Vec::new(),
                annotations: Vec::new(),
                styles: Vec::new(),
                parent: None,
                link: None,
                link_target: None,
                tooltip: None,
                have_callback: false,
                callback: None,
                callback_effective: false,
                relation_endpoint_facade_candidate,
            },
        );
    }

    pub(super) fn add_class(&mut self, id: &str) {
        self.add_class_with_origin(id, false);
    }

    pub(super) fn set_class_label(&mut self, id: &str, label: &str) {
        let (class_name, _) = self.split_class_name_and_type(id);
        self.add_class(&class_name);
        let Some(c) = self.classes.get_mut(&class_name) else {
            return;
        };
        let label = sanitize_text(label, self.config);
        c.label = label.clone();
        c.text = if c.type_param.is_empty() {
            label
        } else {
            format!("{label}<{}>", c.type_param)
        };
    }

    pub(super) fn set_direction(&mut self, dir: &str) {
        self.direction = dir.to_string();
    }

    fn cleanup_label(&self, label: &str) -> String {
        let t = label.strip_prefix(':').unwrap_or(label);
        sanitize_text(t.trim_matches(is_ecmascript_whitespace), self.config)
    }

    pub(super) fn add_member(&mut self, class_name: &str, member: &str) -> Result<(), String> {
        self.add_class(class_name);
        let (class_name, _) = self.split_class_name_and_type(class_name);
        let Some(c) = self.classes.get_mut(&class_name) else {
            return Ok(());
        };

        let member_string = member.trim_matches(is_ecmascript_whitespace);
        if member_string.is_empty() {
            return Ok(());
        }
        if member_string.starts_with("<<") && member_string.ends_with(">>") {
            c.annotations.push(sanitize_text(
                member_string
                    .trim_start_matches("<<")
                    .trim_end_matches(">>"),
                self.config,
            ));
            return Ok(());
        }
        if member_string.find(')').is_some_and(|index| index > 0) {
            c.methods
                .push(ClassMember::new(member_string, "method", self.config)?);
            return Ok(());
        }
        c.members
            .push(ClassMember::new(member_string, "attribute", self.config)?);
        Ok(())
    }

    fn add_members(&mut self, class_name: &str, mut members: Vec<String>) -> Result<(), String> {
        members.reverse();
        for m in members {
            self.add_member(class_name, &m)?;
        }
        Ok(())
    }

    fn add_annotation(&mut self, class_name: &str, annotation: &str) {
        self.add_class(class_name);
        let (class_name, _) = self.split_class_name_and_type(class_name);
        if let Some(c) = self.classes.get_mut(&class_name) {
            c.annotations.push(sanitize_text(annotation, self.config));
        }
    }

    pub(super) fn set_css_class(
        &mut self,
        ids: &str,
        css_class: &str,
        style_event_ordinal: Option<usize>,
    ) {
        let prior_definition = style_event_ordinal
            .and_then(|_| self.paint_class_definitions.get(css_class))
            .cloned();
        // Mermaid preserves whitespace inside each quoted cssClass target.
        for id in ids.split(',') {
            if id.is_empty() {
                continue;
            }
            let (class_name, _) = if id.as_bytes().first().is_some_and(u8::is_ascii_digit) {
                self.split_class_name_and_type(&format!("{MERMAID_DOM_ID_PREFIX}{id}"))
            } else {
                self.split_class_name_and_type(id)
            };
            if let Some(c) = self.classes.get_mut(&class_name) {
                c.relation_endpoint_facade_candidate = false;
                let is_first_explicit_assignment = style_event_ordinal.is_some()
                    && !class_list_contains(&c.css_classes, css_class);
                if let Some(style_event_ordinal) =
                    style_event_ordinal.filter(|_| is_first_explicit_assignment)
                {
                    if let Some(definition) = prior_definition.as_ref()
                        && self
                            .style_precedence_facts
                            .definition_before_assignment_no_backfill
                            .is_none()
                    {
                        self.style_precedence_facts
                            .definition_before_assignment_no_backfill =
                            Some(class_typed::ClassStylePrecedenceWitness::new(
                                css_class.to_string(),
                                class_name.clone(),
                                definition.styles.clone(),
                                definition.style_event_ordinal,
                                style_event_ordinal,
                            ));
                    }
                    self.explicit_css_assignments
                        .entry(css_class.to_string())
                        .or_insert_with(|| ExplicitCssAssignment {
                            target: class_name.clone(),
                            style_event_ordinal,
                        });
                }
                c.css_classes.push(' ');
                c.css_classes.push_str(css_class);
            }
        }
    }

    fn set_tooltip(&mut self, ids: &str, tooltip: &str) {
        for id in ids.split(',') {
            let (class_name, _) = self.split_class_name_and_type(id);
            if let Some(c) = self.classes.get_mut(&class_name) {
                c.relation_endpoint_facade_candidate = false;
                c.tooltip = Some(sanitize_text(tooltip, self.config));
            }
        }
    }

    fn set_link(&mut self, ids: &str, url: &str, target: Option<String>) {
        for id in ids.split(',') {
            let (class_name, _) = if id.as_bytes().first().is_some_and(u8::is_ascii_digit) {
                self.split_class_name_and_type(&format!("{MERMAID_DOM_ID_PREFIX}{id}"))
            } else {
                self.split_class_name_and_type(id)
            };
            if let Some(c) = self.classes.get_mut(&class_name) {
                c.relation_endpoint_facade_candidate = false;
                c.link = format_url(url, self.config);

                let final_target = if self.security_level == Some("sandbox") {
                    "_top".to_string()
                } else if let Some(target) = target.as_deref() {
                    sanitize_text(target, self.config)
                } else {
                    "_blank".to_string()
                };
                c.link_target = Some(final_target);
            }
        }
        self.set_css_class(ids, "clickable", None);
    }

    fn set_click_event(&mut self, ids: &str, function: &str, args: Option<String>) {
        let args = args
            .map(|args| args.trim().to_string())
            .filter(|args| !args.is_empty());
        for id in ids.split(',') {
            let (class_name, _) = self.split_class_name_and_type(id);
            if let Some(c) = self.classes.get_mut(&class_name) {
                c.relation_endpoint_facade_candidate = false;
                c.have_callback = true;
                let mut map = serde_json::Map::new();
                map.insert("function".to_string(), Value::String(function.to_string()));
                if let Some(args) = &args {
                    map.insert("args".to_string(), Value::String(args.clone()));
                }
                c.callback = Some(map);
                c.callback_effective = self.security_level == Some("loose");
            }
        }
        self.set_css_class(ids, "clickable", None);
    }

    fn parse_styles(raw: &str) -> Vec<String> {
        raw.split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect()
    }

    fn take_style_event_ordinal(&mut self) -> usize {
        let ordinal = self.next_style_event_ordinal;
        self.next_style_event_ordinal = self.next_style_event_ordinal.saturating_add(1);
        ordinal
    }

    fn set_css_style(&mut self, id: &str, styles: Vec<String>, style_event_ordinal: usize) {
        let paint_styles = styles
            .iter()
            .filter(|style| crate::style::is_safe_paint_declaration(style))
            .cloned()
            .collect::<Vec<_>>();
        if !paint_styles.is_empty()
            && self.style_precedence_facts.inline_paint.is_none()
            && self.classes.contains_key(id)
        {
            self.style_precedence_facts.inline_paint =
                Some(class_typed::ClassStyleDeclarationWitness::new(
                    id.to_string(),
                    paint_styles,
                    style_event_ordinal,
                ));
        }
        let Some(c) = self.classes.get_mut(id) else {
            return;
        };
        c.relation_endpoint_facade_candidate = false;
        for s in styles {
            for part in s.split(',') {
                let t = part.trim();
                if !t.is_empty() {
                    c.styles.push(t.to_string());
                }
            }
        }
    }

    fn define_class(&mut self, id: &str, styles: Vec<String>, style_event_ordinal: usize) {
        let paint_styles = styles
            .iter()
            .filter(|style| crate::style::is_safe_paint_declaration(style))
            .cloned()
            .collect::<Vec<_>>();
        let typography_styles = styles
            .iter()
            .filter(|style| crate::style::is_safe_typography_declaration(style))
            .cloned()
            .collect::<Vec<_>>();
        if !typography_styles.is_empty()
            && self.style_precedence_facts.classdef_typography.is_none()
        {
            self.style_precedence_facts.classdef_typography =
                Some(class_typed::ClassStyleDeclarationWitness::new(
                    id.to_string(),
                    typography_styles,
                    style_event_ordinal,
                ));
        }
        if !paint_styles.is_empty()
            && self
                .style_precedence_facts
                .assignment_before_definition_copy
                .is_none()
            && let Some(assignment) = self.explicit_css_assignments.get(id)
        {
            self.style_precedence_facts
                .assignment_before_definition_copy =
                Some(class_typed::ClassStylePrecedenceWitness::new(
                    id.to_string(),
                    assignment.target.clone(),
                    paint_styles.clone(),
                    assignment.style_event_ordinal,
                    style_event_ordinal,
                ));
        }
        if !paint_styles.is_empty() {
            self.paint_class_definitions.insert(
                id.to_string(),
                PaintClassDefinition {
                    styles: paint_styles,
                    style_event_ordinal,
                },
            );
        }
        let entry = self
            .style_classes
            .entry(id.to_string())
            .or_insert_with(|| StyleClass {
                id: id.to_string(),
                ..Default::default()
            });

        for s in &styles {
            if s.contains("color") {
                entry.text_styles.push(s.replace("fill", "bgFill"));
            }
            entry.styles.push(s.to_string());
        }

        for c in self.classes.values_mut() {
            if !c.css_classes.contains(id) {
                continue;
            }
            for s in &styles {
                for part in s.split(',') {
                    let t = part.trim();
                    if !t.is_empty() {
                        c.styles.push(t.to_string());
                    }
                }
            }
        }
    }

    fn current_namespace(&self) -> Option<&str> {
        self.namespace_stack.last().map(String::as_str)
    }

    fn resolve_qualified_namespace_id(&self, id: &str) -> String {
        if let Some(prefix) = self.current_namespace() {
            format!("{prefix}.{id}")
        } else {
            id.to_string()
        }
    }

    fn namespace_ancestor_ids(qualified_id: &str) -> Vec<String> {
        let parts: Vec<&str> = qualified_id.split('.').collect();
        let mut ids = Vec::with_capacity(parts.len());
        let mut current = String::new();
        for part in parts {
            if current.is_empty() {
                current.push_str(part);
            } else {
                current.push('.');
                current.push_str(part);
            }
            ids.push(current.clone());
        }
        ids
    }

    fn add_note(&mut self, class_id: Option<String>, text: &str) {
        let note_id = format!("note{}", self.notes.len());
        let parent = self.current_namespace().map(str::to_string);
        self.notes.push(ClassNote {
            id: note_id.clone(),
            class_id,
            text: text.to_string(),
            parent: parent.clone(),
        });
        if let Some(parent) = parent
            && let Some(ns) = self.namespaces.get_mut(&parent)
        {
            ns.note_ids.push(note_id);
        }
    }

    fn add_namespace(&mut self, id: &str, label: Option<String>) {
        let qualified_id = self.resolve_qualified_namespace_id(id);
        self.namespace_stack.push(qualified_id.clone());

        if self.namespaces.contains_key(&qualified_id) {
            if let Some(ns) = self.namespaces.get_mut(&qualified_id) {
                ns.explicit = true;
                if let Some(label) = label {
                    ns.label = sanitize_text(&label, self.config);
                }
            }
            return;
        }

        let parts: Vec<&str> = qualified_id.split('.').collect();
        let ancestor_ids = Self::namespace_ancestor_ids(&qualified_id);
        for (idx, current_id) in ancestor_ids.iter().enumerate() {
            let parent = idx
                .checked_sub(1)
                .and_then(|i| ancestor_ids.get(i))
                .cloned();
            let is_leaf = idx == ancestor_ids.len() - 1;
            let node_label = if is_leaf {
                label
                    .as_deref()
                    .map(|s| sanitize_text(s, self.config))
                    .unwrap_or_else(|| parts[idx].to_string())
            } else {
                parts[idx].to_string()
            };

            if let Some(ns) = self.namespaces.get_mut(current_id) {
                if is_leaf {
                    ns.explicit = true;
                    ns.label = node_label;
                }
                if ns.parent.is_none() {
                    ns.parent = parent;
                }
                continue;
            }

            self.namespaces.insert(
                current_id.clone(),
                Namespace {
                    id: current_id.clone(),
                    label: node_label,
                    class_ids: Vec::new(),
                    note_ids: Vec::new(),
                    parent,
                    explicit: is_leaf,
                },
            );
        }
    }

    fn pop_namespace(&mut self) {
        self.namespace_stack.pop();
    }

    fn add_classes_to_namespace(&mut self, namespace: &str, class_names: &[String]) {
        let namespace = self
            .current_namespace()
            .map(str::to_string)
            .unwrap_or_else(|| namespace.to_string());
        if !self.namespaces.contains_key(&namespace) {
            return;
        }
        let mut ids = Vec::new();
        for name in class_names {
            let (class_name, _) = self.split_class_name_and_type(name);
            self.add_class(name);
            if let Some(c) = self.classes.get_mut(&class_name) {
                c.parent = Some(namespace.clone());
            }
            ids.push(class_name);
        }
        if let Some(ns) = self.namespaces.get_mut(&namespace) {
            for id in ids {
                if !ns.class_ids.contains(&id) {
                    ns.class_ids.push(id);
                }
            }
        }
    }

    fn existing_relation_class_id(&self, class_name: &str) -> Option<String> {
        if self.classes.contains_key(class_name) {
            return Some(class_name.to_string());
        }

        None
    }

    fn add_relation_class(&mut self, id: &str) -> String {
        let (class_name, _) = self.split_class_name_and_type(id);
        if let Some(existing_id) = self.existing_relation_class_id(&class_name) {
            return existing_id;
        }

        self.add_class_with_origin(id, true);
        class_name
    }

    fn namespace_facade_aliases(&self) -> BTreeMap<String, String> {
        let mut qualified_members = BTreeMap::<String, (usize, String)>::new();
        for namespace in self.namespaces.values() {
            for class_id in &namespace.class_ids {
                let qualified_id = format!("{}.{}", namespace.id, class_id);
                let namespace_len = namespace.id.len();
                match qualified_members.get(&qualified_id) {
                    Some((existing_len, _)) if *existing_len >= namespace_len => {}
                    _ => {
                        qualified_members
                            .insert(qualified_id, (namespace_len, class_id.to_string()));
                    }
                }
            }
        }

        self.classes
            .values()
            .filter(|class| class.relation_endpoint_facade_candidate)
            .filter_map(|class| {
                qualified_members
                    .get(class.id.as_str())
                    .map(|(_, target)| (class.id.clone(), target.clone()))
            })
            .collect()
    }

    fn add_relation(&mut self, mut rel: RelationData) {
        let original_id1 = rel.id1.clone();
        let original_id2 = rel.id2.clone();

        let invalid_types = [
            REL_LOLLIPOP,
            REL_AGGREGATION,
            REL_COMPOSITION,
            REL_DEPENDENCY,
            REL_EXTENSION,
        ];

        if rel.relation.type1 == REL_LOLLIPOP && !invalid_types.contains(&rel.relation.type2) {
            let id2_name = self.add_relation_class(&original_id2);
            let iface_id = format!("interface{}", self.interfaces.len());
            self.interfaces.push(Interface {
                id: iface_id.clone(),
                label: rel.id1.clone(),
                class_id: id2_name.clone(),
            });
            rel.id1 = iface_id;
            rel.id2 = id2_name;
        } else if rel.relation.type2 == REL_LOLLIPOP && !invalid_types.contains(&rel.relation.type1)
        {
            let id1_name = self.add_relation_class(&original_id1);
            let iface_id = format!("interface{}", self.interfaces.len());
            self.interfaces.push(Interface {
                id: iface_id.clone(),
                label: rel.id2.clone(),
                class_id: id1_name.clone(),
            });
            rel.id1 = id1_name;
            rel.id2 = iface_id;
        } else {
            let id1_name = self.add_relation_class(&original_id1);
            let id2_name = self.add_relation_class(&original_id2);
            rel.id1 = id1_name;
            rel.id2 = id2_name;
        }

        self.relations.push(rel);
    }

    pub(super) fn apply(&mut self, action: Action) -> std::result::Result<(), String> {
        match action {
            Action::SetDirection(d) => {
                self.set_direction(&d);
                Ok(())
            }
            Action::SetAccTitle(t) => {
                self.acc_title = Some(t);
                Ok(())
            }
            Action::SetAccDescr(t) => {
                self.acc_descr = Some(t);
                Ok(())
            }

            Action::AddNamespace { id, label } => {
                self.add_namespace(&id, label);
                Ok(())
            }
            Action::AddClassesToNamespace {
                namespace,
                class_ids,
            } => {
                self.add_classes_to_namespace(&namespace, &class_ids);
                Ok(())
            }
            Action::PopNamespace => {
                self.pop_namespace();
                Ok(())
            }

            Action::AddClass { id } => {
                self.add_class(&id);
                Ok(())
            }
            Action::SetClassLabel { id, label } => {
                self.set_class_label(&id, &label);
                Ok(())
            }
            Action::SetCssClass { ids, css_class } => {
                let style_event_ordinal = self.take_style_event_ordinal();
                self.set_css_class(&ids, &css_class, Some(style_event_ordinal));
                Ok(())
            }
            Action::SetCssStyle { id, raw } => {
                let styles = Self::parse_styles(&raw);
                let style_event_ordinal = self.take_style_event_ordinal();
                self.set_css_style(&id, styles, style_event_ordinal);
                Ok(())
            }
            Action::DefineClass { id, raw } => {
                let styles = Self::parse_styles(&raw);
                let style_event_ordinal = self.take_style_event_ordinal();
                self.define_class(&id, styles, style_event_ordinal);
                Ok(())
            }
            Action::SetLink { id, url, target } => {
                self.set_link(&id, &url, target);
                Ok(())
            }
            Action::SetTooltip { id, tooltip } => {
                self.set_tooltip(&id, &tooltip);
                Ok(())
            }
            Action::SetClickEvent { id, function, args } => {
                self.set_click_event(&id, &function, args);
                Ok(())
            }
            Action::AddMembers { id, members } => self.add_members(&id, members),
            Action::AddMember { id, member } => {
                let cleaned = self.cleanup_label(&member);
                self.add_member(&id, &cleaned)
            }
            Action::AddAnnotation { id, annotation } => {
                self.add_annotation(&id, &annotation);
                Ok(())
            }
            Action::AddRelation { mut data } => {
                if let Some(t) = data.title.take() {
                    data.title = Some(self.cleanup_label(&t));
                }
                if let Some(t) = data.relation_title1.take() {
                    data.relation_title1 = Some(sanitize_text(
                        t.trim_matches(is_ecmascript_whitespace),
                        self.config,
                    ));
                }
                if let Some(t) = data.relation_title2.take() {
                    data.relation_title2 = Some(sanitize_text(
                        t.trim_matches(is_ecmascript_whitespace),
                        self.config,
                    ));
                }
                self.add_relation(data);
                Ok(())
            }
            Action::AddNote { class_id, text } => {
                self.add_note(class_id, &text);
                Ok(())
            }
        }
    }

    fn rebuild_namespace_memberships(&mut self) {
        for ns in self.namespaces.values_mut() {
            ns.class_ids.clear();
            ns.note_ids.clear();
        }
        for (id, class_node) in &self.classes {
            let Some(parent) = class_node.parent.as_deref() else {
                continue;
            };
            // `classes` is an IndexMap keyed by unique class IDs, so rebuilding
            // membership cannot introduce a duplicate class entry.
            if let Some(ns) = self.namespaces.get_mut(parent) {
                ns.class_ids.push(id.clone());
            }
        }
        for note in &self.notes {
            let Some(parent) = note.parent.as_deref() else {
                continue;
            };
            // Notes receive monotonic synthetic IDs at insertion time, so the
            // rebuild pass can append directly without an O(N^2) membership scan.
            if let Some(ns) = self.namespaces.get_mut(parent) {
                ns.note_ids.push(note.id.clone());
            }
        }
    }

    #[cfg(feature = "compat-json")]
    pub(super) fn into_model(self, meta: &SourcePresentationMetadata) -> Value {
        let model = class_typed::ClassDiagram::new(
            self.into_semantic_model(&meta.diagram_type),
            meta.effective_config.as_config(),
        );
        super::render_model_to_compat_json(&model, meta)
            .expect("Class typed model must remain JSON-serializable")
    }

    pub(super) fn into_semantic_model(
        mut self,
        diagram_type: &str,
    ) -> class_typed::ClassDiagramModel {
        self.rebuild_namespace_memberships();
        let namespace_facade_aliases = self.namespace_facade_aliases();

        let classes = self
            .classes
            .into_iter()
            .map(|(id, c)| (id, c.into_typed()))
            .collect();

        let relations = self
            .relations
            .into_iter()
            .enumerate()
            .map(|(idx, r)| class_typed::ClassRelation {
                id: idx.to_string(),
                id1: r.id1,
                id2: r.id2,
                relation_title_1: r.relation_title1,
                relation_title_2: r.relation_title2,
                title: r.title.unwrap_or_default(),
                relation: class_typed::RelationShape {
                    type1: r.relation.type1,
                    type2: r.relation.type2,
                    line_type: r.relation.line_type,
                },
            })
            .collect();

        let notes = self.notes.into_iter().map(ClassNote::into_typed).collect();
        let interfaces = self
            .interfaces
            .into_iter()
            .map(Interface::into_typed)
            .collect();
        let namespaces = self
            .namespaces
            .into_iter()
            .map(|(k, ns)| (k, ns.into_typed()))
            .collect();
        let style_classes = self
            .style_classes
            .into_iter()
            .map(|(k, sc)| (k, sc.into_typed()))
            .collect();

        class_typed::ClassDiagramModel {
            diagram_type: diagram_type.to_string(),
            direction: self.direction,
            acc_title: self.acc_title,
            acc_descr: self.acc_descr,
            classes,
            relations,
            notes,
            interfaces,
            namespaces,
            namespace_facade_aliases,
            style_classes,
            style_precedence_facts: class_typed::ClassStylePrecedenceFacts::from_witnesses(
                self.style_precedence_facts
                    .assignment_before_definition_copy,
                self.style_precedence_facts
                    .definition_before_assignment_no_backfill,
                self.style_precedence_facts.inline_paint,
                self.style_precedence_facts.classdef_typography,
            ),
            constants: class_typed::ClassConstants {
                line_type: class_typed::ClassLineTypeConstants {
                    line: LINE_SOLID,
                    dotted_line: LINE_DOTTED,
                },
                relation_type: class_typed::ClassRelationTypeConstants {
                    none: REL_NONE,
                    aggregation: REL_AGGREGATION,
                    extension: REL_EXTENSION,
                    composition: REL_COMPOSITION,
                    dependency: REL_DEPENDENCY,
                    lollipop: REL_LOLLIPOP,
                },
            },
        }
    }
}

fn class_list_contains(classes: &str, expected: &str) -> bool {
    classes
        .split(|ch: char| ch.is_ascii_whitespace() || ch == ',')
        .any(|class_name| class_name == expected)
}
