use crate::diagrams::langium_common::{
    LangiumCommonField, LangiumLexemeTrace, parse_langium_common, push_langium_common_editor_fact,
};
use crate::diagrams::scan::{leading_whitespace_len, physical_line_at, split_ascii_indent};
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol, Error, ParseMetadata, Result, SourceSpan,
};
use serde_json::{Map, Value, json};

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TreemapNodeRenderModel {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<TreemapNodeRenderModel>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    #[serde(
        default,
        rename = "classSelector",
        skip_serializing_if = "Option::is_none"
    )]
    pub class_selector: Option<String>,
    #[serde(
        default,
        rename = "cssCompiledStyles",
        skip_serializing_if = "Option::is_none"
    )]
    pub css_compiled_styles: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TreemapDiagramRenderModel {
    #[serde(rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    pub acc_descr: Option<String>,
    pub title: Option<String>,
    pub root: TreemapNodeRenderModel,
    #[serde(default)]
    pub classes: std::collections::BTreeMap<String, TreemapClassDefRenderModel>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TreemapClassDefRenderModel {
    pub id: String,
    #[serde(default)]
    pub styles: Vec<String>,
    #[serde(default, rename = "textStyles")]
    pub text_styles: Vec<String>,
}

impl TreemapDiagramRenderModel {
    pub(crate) fn sanitize_common_db_fields(&mut self, config: &crate::MermaidConfig) {
        crate::common_db::sanitize_optional_title(&mut self.title, config);
        crate::common_db::sanitize_optional_acc_title(&mut self.acc_title, config);
        crate::common_db::sanitize_optional_acc_descr(&mut self.acc_descr, config);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ItemType {
    Section,
    Leaf,
}

#[derive(Debug, Clone)]
struct ClassDefStatement {
    class_name: SpannedText,
    style_text: Option<SpannedText>,
    span: SourceSpan,
}

#[derive(Debug, Clone)]
struct ItemRow {
    indent: usize,
    name: SpannedText,
    item_type: ItemType,
    value: Option<SpannedValue>,
    class_selector: Option<SpannedText>,
    span: SourceSpan,
}

#[derive(Debug, Clone)]
enum TreemapRow {
    Item(ItemRow),
    ClassDef(ClassDefStatement),
}

type StyleClassDef = TreemapClassDefRenderModel;

#[derive(Debug, Clone)]
struct NodeRecord {
    name: String,
    value: Option<Value>,
    class_selector: Option<String>,
    css_compiled_styles: Option<Vec<String>>,
    children: Option<Vec<usize>>,
}

#[derive(Debug, Clone)]
struct Arena {
    nodes: Vec<NodeRecord>,
}

impl Arena {
    fn push(&mut self, node: NodeRecord) -> usize {
        let idx = self.nodes.len();
        self.nodes.push(node);
        idx
    }
}

struct TreemapParsedInput {
    present: bool,
    title: Option<String>,
    acc_title: Option<String>,
    acc_descr: Option<String>,
    rows: Vec<TreemapRow>,
    editor_facts: EditorSemanticFacts,
}

#[derive(Debug, Clone)]
struct TreemapParseIssue {
    message: String,
    span: Option<SourceSpan>,
}

struct TreemapParseOutcome {
    parsed: TreemapParsedInput,
    first_issue: Option<TreemapParseIssue>,
}

impl TreemapParseOutcome {
    fn record_issue(&mut self, message: impl Into<String>, span: Option<SourceSpan>) {
        let issue = TreemapParseIssue {
            message: message.into(),
            span,
        };
        self.parsed
            .editor_facts
            .mark_recovered_from_parse_error(issue.message.clone(), issue.span);
        if self.first_issue.is_none() {
            self.first_issue = Some(issue);
        }
    }

    fn into_strict(self, meta: &ParseMetadata) -> Result<TreemapParsedInput> {
        match self.first_issue {
            Some(issue) => Err(treemap_error(meta, issue.message, issue.span)),
            None => Ok(self.parsed),
        }
    }

    fn into_editor_facts(self) -> EditorSemanticFacts {
        self.parsed.editor_facts
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SpannedText {
    text: String,
    span: SourceSpan,
}

#[derive(Debug, Clone)]
struct SpannedValue {
    value: Value,
    source: SpannedText,
}

struct TreemapSemanticSource {
    present: bool,
    title: Option<String>,
    acc_title: Option<String>,
    acc_descr: Option<String>,
    class_defs: std::collections::HashMap<String, StyleClassDef>,
    arena: Arena,
    roots: Vec<usize>,
    editor_facts: EditorSemanticFacts,
}

fn push_treemap_entity(
    facts: &mut EditorSemanticFacts,
    text: &SpannedText,
    statement_span: SourceSpan,
    detail: &str,
    kind: EditorSemanticKind,
) {
    if text.text.is_empty() {
        return;
    }
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::NodeIdentifier,
        text.span,
    ));
    facts.push_symbol(EditorSemanticSymbol::new(
        text.text.clone(),
        Some(detail.to_string()),
        kind,
        statement_span,
        text.span,
    ));
}

fn push_treemap_payload(
    facts: &mut EditorSemanticFacts,
    text: &SpannedText,
    detail: &str,
    kind: EditorSemanticKind,
) {
    if text.text.is_empty() {
        return;
    }
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::Payload,
        text.span,
    ));
    facts.push_symbol(EditorSemanticSymbol::payload(
        text.text.clone(),
        Some(detail.to_string()),
        kind,
        text.span,
        text.span,
    ));
}

fn push_treemap_row_editor_facts(facts: &mut EditorSemanticFacts, row: &TreemapRow) {
    match row {
        TreemapRow::ClassDef(class_def) => {
            facts.push_symbol(EditorSemanticSymbol::outline(
                class_def.class_name.text.clone(),
                Some("treemap class definition".to_string()),
                EditorSemanticKind::Class,
                class_def.span,
                class_def.class_name.span,
            ));
            if let Some(style) = class_def.style_text.as_ref() {
                push_treemap_payload(
                    facts,
                    style,
                    "treemap class style",
                    EditorSemanticKind::String,
                );
            }
        }
        TreemapRow::Item(item) => {
            let (detail, kind) = match item.item_type {
                ItemType::Section => ("treemap section", EditorSemanticKind::Namespace),
                ItemType::Leaf => ("treemap leaf", EditorSemanticKind::Variable),
            };
            push_treemap_entity(facts, &item.name, item.span, detail, kind);
            if let Some(class_selector) = item.class_selector.as_ref() {
                push_treemap_payload(
                    facts,
                    class_selector,
                    "treemap class selector",
                    EditorSemanticKind::String,
                );
            }
            if let Some(value) = item.value.as_ref() {
                push_treemap_payload(
                    facts,
                    &value.source,
                    "treemap value",
                    EditorSemanticKind::String,
                );
            }
        }
    }
}

pub fn parse_treemap_editor_facts(code: &str, _meta: &ParseMetadata) -> EditorSemanticFacts {
    parse_treemap_outcome(code).into_editor_facts()
}

pub fn parse_treemap(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let model = parse_treemap_semantic_source(code, meta)?.render_model();
    render_model_to_compat_json(&model, meta)
}

pub(crate) fn parse_treemap_json_and_editor_facts(
    code: &str,
    meta: &ParseMetadata,
) -> Result<(Value, EditorSemanticFacts)> {
    let source = parse_treemap_semantic_source(code, meta)?;
    let model = source.render_model();
    let compat = render_model_to_compat_json(&model, meta)?;
    Ok((compat, source.editor_facts))
}

pub fn parse_treemap_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<TreemapDiagramRenderModel> {
    Ok(parse_treemap_semantic_source(code, meta)?.render_model())
}

pub(crate) fn render_model_to_compat_json(
    model: &TreemapDiagramRenderModel,
    meta: &ParseMetadata,
) -> Result<Value> {
    if model.root.children.is_none() {
        return Ok(json!({}));
    }

    let mut nodes = Vec::new();
    flatten_render_nodes(&model.root, &mut nodes);

    let mut out = Map::new();
    out.insert("type".to_string(), Value::String(meta.diagram_type.clone()));
    out.insert("title".to_string(), json!(&model.title));
    out.insert("accTitle".to_string(), json!(&model.acc_title));
    out.insert("accDescr".to_string(), json!(&model.acc_descr));
    out.insert("root".to_string(), render_node_to_value(&model.root));
    out.insert("nodes".to_string(), Value::Array(nodes));
    out.insert("classes".to_string(), json!(&model.classes));
    out.insert(
        "config".to_string(),
        crate::config::clone_value_nonrecursive(meta.effective_config.as_value()),
    );
    Ok(Value::Object(out))
}

fn render_node_to_map(
    node: &TreemapNodeRenderModel,
    children: Option<Vec<Value>>,
) -> Map<String, Value> {
    let mut out = Map::new();
    out.insert("name".to_string(), Value::String(node.name.clone()));
    if let Some(children) = children {
        out.insert("children".to_string(), Value::Array(children));
    }
    if let Some(value) = &node.value {
        out.insert("value".to_string(), value.clone());
    }
    if let Some(class_selector) = &node.class_selector {
        out.insert(
            "classSelector".to_string(),
            Value::String(class_selector.clone()),
        );
    }
    if let Some(styles) = &node.css_compiled_styles {
        out.insert("cssCompiledStyles".to_string(), json!(styles));
    }
    out
}

fn render_node_to_value(root: &TreemapNodeRenderModel) -> Value {
    let mut completed: std::collections::HashMap<*const TreemapNodeRenderModel, Value> =
        std::collections::HashMap::new();
    let mut stack = vec![(root, false)];

    while let Some((node, visited)) = stack.pop() {
        if visited {
            let children = node.children.as_ref().map(|children| {
                children
                    .iter()
                    .filter_map(|child| completed.remove(&(child as *const TreemapNodeRenderModel)))
                    .collect()
            });
            completed.insert(
                node as *const TreemapNodeRenderModel,
                Value::Object(render_node_to_map(node, children)),
            );
        } else {
            stack.push((node, true));
            if let Some(children) = &node.children {
                for child in children.iter().rev() {
                    stack.push((child, false));
                }
            }
        }
    }

    completed
        .remove(&(root as *const TreemapNodeRenderModel))
        .unwrap_or_else(|| Value::Object(render_node_to_map(root, None)))
}

fn flatten_render_nodes(root: &TreemapNodeRenderModel, out: &mut Vec<Value>) {
    let mut stack = root
        .children
        .as_deref()
        .unwrap_or_default()
        .iter()
        .rev()
        .map(|node| (node, 0_i64))
        .collect::<Vec<_>>();

    while let Some((node, level)) = stack.pop() {
        let mut value = render_node_to_map(node, None);
        value.insert("level".to_string(), Value::Number(level.into()));
        out.push(Value::Object(value));

        if let Some(children) = &node.children {
            for child in children.iter().rev() {
                stack.push((child, level.saturating_add(1)));
            }
        }
    }
}

impl TreemapSemanticSource {
    fn render_model(&self) -> TreemapDiagramRenderModel {
        if !self.present {
            return TreemapDiagramRenderModel::default();
        }
        TreemapDiagramRenderModel {
            title: self.title.clone(),
            acc_title: self.acc_title.clone(),
            acc_descr: self.acc_descr.clone(),
            root: TreemapNodeRenderModel {
                name: String::new(),
                children: Some(
                    self.roots
                        .iter()
                        .map(|&idx| node_to_render_model(&self.arena, idx))
                        .collect(),
                ),
                value: None,
                class_selector: None,
                css_compiled_styles: None,
            },
            classes: self.class_defs.clone().into_iter().collect(),
        }
    }
}

fn parse_treemap_semantic_source(
    code: &str,
    meta: &ParseMetadata,
) -> Result<TreemapSemanticSource> {
    construct_treemap_semantic_source(code, meta)
}

fn construct_treemap_semantic_source(
    code: &str,
    meta: &ParseMetadata,
) -> Result<TreemapSemanticSource> {
    let parsed = parse_treemap_outcome(code).into_strict(meta)?;
    let class_defs = class_defs_from_rows(&parsed.rows);
    let flat_items = flat_items_from_rows(&parsed.rows, &class_defs);
    let (arena, roots) = build_hierarchy(&flat_items);
    Ok(TreemapSemanticSource {
        present: parsed.present,
        title: parsed.title,
        acc_title: parsed.acc_title,
        acc_descr: parsed.acc_descr,
        class_defs,
        arena,
        roots,
        editor_facts: parsed.editor_facts,
    })
}

fn parse_treemap_outcome(code: &str) -> TreemapParseOutcome {
    #[cfg(test)]
    crate::diagrams::langium_common::record_family_syntax_construction("treemap");

    let mut outcome = TreemapParseOutcome {
        parsed: TreemapParsedInput {
            present: false,
            title: None,
            acc_title: None,
            acc_descr: None,
            rows: Vec::new(),
            editor_facts: EditorSemanticFacts::new(),
        },
        first_issue: None,
    };
    let mut lexemes = LangiumLexemeTrace::default();

    let body = match treemap_body_start(code) {
        Ok(Some(body)) => body,
        Ok(None) => {
            lexemes.attach(code, &mut outcome.parsed.editor_facts);
            return outcome;
        }
        Err(issue) => {
            if let Some(span) = issue.span {
                lexemes.literal(span);
            }
            outcome.record_issue(issue.message, issue.span);
            lexemes.attach(code, &mut outcome.parsed.editor_facts);
            return outcome;
        }
    };
    outcome.parsed.present = true;
    lexemes.keyword(body.header_span);

    let mut offset = body.offset;
    let mut saw_statement = false;
    let mut trailing_whitespace_span = None;

    while offset < code.len() {
        if let Some(parsed) = parse_langium_common(code, offset) {
            saw_statement = true;
            trailing_whitespace_span = parsed.trailing_whitespace_span;
            let field = parsed.fact.field;
            let value = parsed.fact.value.clone();
            lexemes.extend(parsed.lexemes.clone());
            push_langium_common_editor_fact(
                &mut outcome.parsed.editor_facts,
                &parsed.fact,
                "treemap",
            );
            match field {
                LangiumCommonField::Title => outcome.parsed.title = Some(value),
                LangiumCommonField::AccTitle => outcome.parsed.acc_title = Some(value),
                LangiumCommonField::AccDescr => outcome.parsed.acc_descr = Some(value),
            }
            if let Some(diagnostic) = parsed.diagnostic {
                outcome.record_issue(diagnostic.message, Some(diagnostic.span));
            }
            offset += parsed.consumed;
            continue;
        }

        let (line, next_offset) = physical_line_at(code, offset);
        if line.trim_start().starts_with("%%") {
            offset = next_offset;
            continue;
        }
        let visible = strip_inline_comment_aware(line);
        if visible.trim().is_empty() {
            if saw_statement && !visible.is_empty() {
                trailing_whitespace_span = Some(SourceSpan::new(offset, offset + line.len()));
            }
            offset = next_offset;
            continue;
        }
        saw_statement = true;
        trailing_whitespace_span = None;

        let (indent, rest_with_trailing) = split_ascii_indent(visible);
        let rest = rest_with_trailing.trim_end();
        let statement_start = offset + visible.len().saturating_sub(rest_with_trailing.len());
        let statement_span = SourceSpan::new(statement_start, statement_start + rest.len());

        match parse_class_def(rest, statement_start, &mut lexemes) {
            Ok(Some(class_def)) => {
                let style_issue = class_def.style_text.as_ref().and_then(|style| {
                    validate_class_def_style(&style.text)
                        .err()
                        .map(|message| (message, style.span))
                });
                let row = TreemapRow::ClassDef(class_def);
                push_treemap_row_editor_facts(&mut outcome.parsed.editor_facts, &row);
                outcome.parsed.rows.push(row);
                if let Some((message, span)) = style_issue {
                    outcome.record_issue(message, Some(span));
                }
            }
            Ok(None) => match parse_item_row(indent, rest, statement_start, &mut lexemes) {
                Ok(item) => {
                    let row = TreemapRow::Item(item);
                    push_treemap_row_editor_facts(&mut outcome.parsed.editor_facts, &row);
                    outcome.parsed.rows.push(row);
                }
                Err(message) => {
                    outcome.record_issue(message, Some(statement_span));
                }
            },
            Err(message) => {
                outcome.record_issue(message, Some(statement_span));
            }
        }
        offset = next_offset;
    }

    if let Some(span) = trailing_whitespace_span {
        outcome.record_issue("unexpected trailing whitespace-only line", Some(span));
    }
    lexemes.attach(code, &mut outcome.parsed.editor_facts);

    outcome
}

#[derive(Debug, Clone, Copy)]
struct TreemapBodyStart {
    offset: usize,
    header_span: SourceSpan,
}

fn treemap_body_start(
    code: &str,
) -> std::result::Result<Option<TreemapBodyStart>, TreemapParseIssue> {
    let mut offset = 0usize;
    while offset < code.len() {
        let (line, next_offset) = physical_line_at(code, offset);
        let visible = strip_inline_comment_aware(line);
        let trimmed = visible.trim();
        if trimmed.is_empty() {
            offset = next_offset;
            continue;
        }
        let leading = visible.len().saturating_sub(visible.trim_start().len());
        let span = SourceSpan::new(offset + leading, offset + leading + trimmed.len());
        if !is_treemap_header(trimmed) {
            return Err(TreemapParseIssue {
                message: "expected treemap".to_string(),
                span: Some(span),
            });
        }
        return Ok(Some(TreemapBodyStart {
            offset: next_offset,
            header_span: SourceSpan::new(offset + leading, offset + leading + trimmed.len()),
        }));
    }
    Ok(None)
}

fn treemap_error(
    meta: &ParseMetadata,
    message: impl Into<String>,
    span: Option<SourceSpan>,
) -> Error {
    let message = message.into();
    match span {
        Some(span) => Error::diagram_parse_exact(meta.diagram_type.clone(), &message, span),
        None => Error::diagram_parse_fallback(meta.diagram_type.clone(), &message),
    }
}

fn class_defs_from_rows(rows: &[TreemapRow]) -> std::collections::HashMap<String, StyleClassDef> {
    let mut class_defs: std::collections::HashMap<String, StyleClassDef> =
        std::collections::HashMap::new();
    for row in rows {
        let TreemapRow::ClassDef(c) = row else {
            continue;
        };
        add_class(
            &mut class_defs,
            &c.class_name.text,
            c.style_text
                .as_ref()
                .map(|style| style.text.as_str())
                .unwrap_or(""),
        );
    }

    class_defs
}

fn flat_items_from_rows(
    rows: &[TreemapRow],
    class_defs: &std::collections::HashMap<String, StyleClassDef>,
) -> Vec<FlatItem> {
    let mut flat_items: Vec<FlatItem> = Vec::new();
    for row in rows {
        let TreemapRow::Item(item) = row else {
            continue;
        };

        let styles = item
            .class_selector
            .as_ref()
            .map(|cls| get_styles_for_class(class_defs, &cls.text))
            .unwrap_or_default();
        let compiled = if !styles.is_empty() {
            Some(styles.join(";"))
        } else {
            None
        };
        let css_compiled_styles = compiled.and_then(|s| if s.is_empty() { None } else { Some(s) });

        flat_items.push(FlatItem {
            level: item.indent,
            name: item.name.text.clone(),
            item_type: item.item_type,
            value: item.value.as_ref().map(|value| value.value.clone()),
            class_selector: item
                .class_selector
                .as_ref()
                .map(|selector| selector.text.clone()),
            css_compiled_styles,
        });
    }

    flat_items
}

#[derive(Debug, Clone)]
struct FlatItem {
    level: usize,
    name: String,
    item_type: ItemType,
    value: Option<Value>,
    class_selector: Option<String>,
    css_compiled_styles: Option<String>,
}

fn build_hierarchy(items: &[FlatItem]) -> (Arena, Vec<usize>) {
    if items.is_empty() {
        return (Arena { nodes: Vec::new() }, Vec::new());
    }

    let mut arena = Arena { nodes: Vec::new() };
    let mut roots: Vec<usize> = Vec::new();
    let mut stack: Vec<(usize, usize)> = Vec::new(); // (node_idx, item.level)

    for item in items {
        let mut node = NodeRecord {
            name: item.name.clone(),
            value: None,
            class_selector: item.class_selector.clone(),
            css_compiled_styles: item.css_compiled_styles.as_ref().map(|s| vec![s.clone()]),
            children: match item.item_type {
                ItemType::Leaf => None,
                ItemType::Section => Some(Vec::new()),
            },
        };
        if item.item_type == ItemType::Leaf {
            node.value = item.value.clone();
        }

        let idx = arena.push(node);

        while stack.last().is_some_and(|(_, lvl)| *lvl >= item.level) {
            stack.pop();
        }

        if stack.is_empty() {
            roots.push(idx);
        } else {
            let parent_idx = stack[stack.len() - 1].0;
            let parent = &mut arena.nodes[parent_idx];
            if let Some(children) = parent.children.as_mut() {
                children.push(idx);
            } else {
                parent.children = Some(vec![idx]);
            }
        }

        if item.item_type != ItemType::Leaf {
            stack.push((idx, item.level));
        }
    }

    (arena, roots)
}

#[cfg(test)]
fn node_to_value(arena: &Arena, idx: usize) -> Value {
    let mut values: Vec<Option<Value>> = vec![None; arena.nodes.len()];
    let mut stack = vec![(idx, false)];

    while let Some((node_idx, visited)) = stack.pop() {
        let Some(node) = arena.nodes.get(node_idx) else {
            continue;
        };

        if visited {
            let mut obj = Map::new();
            obj.insert("name".to_string(), Value::String(node.name.clone()));
            if let Some(v) = &node.value {
                obj.insert("value".to_string(), v.clone());
            }
            if let Some(cls) = &node.class_selector {
                obj.insert("classSelector".to_string(), Value::String(cls.clone()));
            }
            if let Some(css) = &node.css_compiled_styles {
                obj.insert(
                    "cssCompiledStyles".to_string(),
                    Value::Array(css.iter().cloned().map(Value::String).collect()),
                );
            }
            if let Some(children) = &node.children {
                obj.insert(
                    "children".to_string(),
                    Value::Array(
                        children
                            .iter()
                            .filter_map(|&child_idx| {
                                values.get_mut(child_idx).and_then(Option::take)
                            })
                            .collect(),
                    ),
                );
            }
            values[node_idx] = Some(Value::Object(obj));
        } else {
            stack.push((node_idx, true));
            if let Some(children) = &node.children {
                for &child_idx in children.iter().rev() {
                    stack.push((child_idx, false));
                }
            }
        }
    }

    values
        .get_mut(idx)
        .and_then(Option::take)
        .unwrap_or_else(|| json!({ "name": "" }))
}

fn node_to_render_model(arena: &Arena, idx: usize) -> TreemapNodeRenderModel {
    let mut models: Vec<Option<TreemapNodeRenderModel>> = vec![None; arena.nodes.len()];
    let mut stack = vec![(idx, false)];

    while let Some((node_idx, visited)) = stack.pop() {
        let Some(node) = arena.nodes.get(node_idx) else {
            continue;
        };

        if visited {
            let children = node.children.as_ref().map(|children| {
                children
                    .iter()
                    .filter_map(|&child_idx| models.get_mut(child_idx).and_then(Option::take))
                    .collect()
            });
            models[node_idx] = Some(TreemapNodeRenderModel {
                name: node.name.clone(),
                children,
                value: node.value.clone(),
                class_selector: node.class_selector.clone(),
                css_compiled_styles: node.css_compiled_styles.clone(),
            });
        } else {
            stack.push((node_idx, true));
            if let Some(children) = &node.children {
                for &child_idx in children.iter().rev() {
                    stack.push((child_idx, false));
                }
            }
        }
    }

    models
        .get_mut(idx)
        .and_then(Option::take)
        .unwrap_or_default()
}

fn add_class(
    classes: &mut std::collections::HashMap<String, StyleClassDef>,
    id: &str,
    style: &str,
) {
    let mut style_class = classes.get(id).cloned().unwrap_or_else(|| StyleClassDef {
        id: id.to_string(),
        styles: Vec::new(),
        text_styles: Vec::new(),
    });

    const PLACEHOLDER: &str = "ก์ก์ก์";
    let replaced = style.replace("\\,", PLACEHOLDER);
    let replaced = replaced.replace(',', ";");
    let replaced = replaced.replace(PLACEHOLDER, ",");

    for s in replaced.split(';') {
        if is_label_style_bug_compatible(s) {
            style_class.text_styles.push(s.to_string());
        }
        style_class.styles.push(s.to_string());
    }

    classes.insert(id.to_string(), style_class);
}

fn validate_class_def_style(style: &str) -> std::result::Result<(), String> {
    let style = style.trim().trim_end_matches(';').trim();
    if style.is_empty() {
        return Ok(());
    }

    const PLACEHOLDER: &str = "ก์ก์ก์";
    let replaced = style.replace("\\,", PLACEHOLDER);
    let replaced = replaced.replace(',', ";");
    let replaced = replaced.replace(PLACEHOLDER, ",");

    for raw in replaced.split(';') {
        let s = raw.trim();
        if s.is_empty() {
            continue;
        }
        let Some((k, v)) = s.split_once(':') else {
            return Err(format!("invalid classDef style token `{s}`"));
        };
        if k.trim().is_empty() || v.trim().is_empty() {
            return Err(format!("invalid classDef style token `{s}`"));
        }
    }

    Ok(())
}

fn get_styles_for_class(
    classes: &std::collections::HashMap<String, StyleClassDef>,
    class_selector: &str,
) -> Vec<String> {
    classes
        .get(class_selector)
        .map(|c| c.styles.clone())
        .unwrap_or_default()
}

fn is_label_style_bug_compatible(s: &str) -> bool {
    matches!(
        s.trim(),
        "color"
            | "font-size"
            | "font-family"
            | "font-weight"
            | "font-style"
            | "text-decoration"
            | "text-align"
            | "text-transform"
            | "line-height"
            | "letter-spacing"
            | "word-spacing"
            | "text-shadow"
            | "text-overflow"
            | "white-space"
            | "word-wrap"
            | "word-break"
            | "overflow-wrap"
            | "hyphens"
    )
}

fn strip_inline_comment_aware(line: &str) -> &str {
    let mut in_quote: Option<char> = None;

    let mut it = line.char_indices().peekable();
    while let Some((idx, ch)) = it.next() {
        if let Some(q) = in_quote {
            if ch == q {
                in_quote = None;
            }
            continue;
        }

        if ch == '"' || ch == '\'' {
            in_quote = Some(ch);
            continue;
        }

        if ch == '%' && it.peek().is_some_and(|(_, next)| *next == '%') {
            return &line[..idx];
        }
    }

    line
}

fn is_treemap_header(line: &str) -> bool {
    let t = line.trim_start();
    t == "treemap" || t == "treemap-beta"
}

fn parse_class_def(
    line: &str,
    line_start: usize,
    lexemes: &mut LangiumLexemeTrace,
) -> std::result::Result<Option<ClassDefStatement>, String> {
    let Some(after_keyword) = line.strip_prefix("classDef") else {
        return Ok(None);
    };
    if after_keyword
        .chars()
        .next()
        .is_some_and(|character| !character.is_whitespace())
    {
        return Ok(None);
    }
    lexemes.keyword(SourceSpan::new(line_start, line_start + "classDef".len()));
    if after_keyword.is_empty() {
        return Err("expected class name".to_string());
    }

    let class_input = after_keyword.trim_start();
    let class_start = line.len().saturating_sub(class_input.len());
    let (class_name, tail) =
        parse_id2(class_input).ok_or_else(|| "expected class name".to_string())?;
    let class_name_span = SourceSpan::new(
        line_start + class_start,
        line_start + class_start + class_name.len(),
    );
    lexemes.identifier(class_name_span);

    let style_input = tail.trim_start();
    let style_input_start = line.len().saturating_sub(style_input.len());
    let semicolon = style_input.find(';');
    let (style_raw, trailing) = match semicolon {
        Some(semi) => (&style_input[..semi], &style_input[semi + 1..]),
        None => (style_input, ""),
    };
    let style = style_raw.trim();
    let style_text = if style.is_empty() {
        None
    } else {
        let leading = leading_whitespace_len(style_raw);
        let start = line_start + style_input_start + leading;
        Some(SpannedText {
            text: style.to_string(),
            span: SourceSpan::new(start, start + style.len()),
        })
    };
    if let Some(style) = style_text.as_ref() {
        lexemes.style(style.span);
    }
    if let Some(semi) = semicolon {
        let start = line_start + style_input_start + semi;
        lexemes.delimiter(SourceSpan::new(start, start + 1));
    }
    if !trailing.trim().is_empty() {
        return Err("unexpected tokens after classDef".to_string());
    }

    Ok(Some(ClassDefStatement {
        class_name: SpannedText {
            text: class_name,
            span: class_name_span,
        },
        style_text,
        span: SourceSpan::new(line_start, line_start + line.len()),
    }))
}

fn parse_item_row(
    indent: usize,
    line: &str,
    line_start: usize,
    lexemes: &mut LangiumLexemeTrace,
) -> std::result::Result<ItemRow, String> {
    let mut p = Parser::new(line);
    p.skip_ws();
    let name_token_start = p.pos;
    let name_text = p
        .parse_string2()
        .ok_or_else(|| "expected quoted string".to_string())?;
    let name_token_end = p.pos;
    let quote_len = line[name_token_start..]
        .chars()
        .next()
        .map(char::len_utf8)
        .unwrap_or_default();
    let name = SpannedText {
        text: name_text,
        span: SourceSpan::new(
            line_start + name_token_start + quote_len,
            line_start + name_token_end.saturating_sub(quote_len),
        ),
    };
    lexemes.string(SourceSpan::new(
        line_start + name_token_start,
        line_start + name_token_end,
    ));
    p.skip_ws();

    // Section: "Name" (:::class)?
    let class_delimiter_start = p.pos;
    if p.try_consume_str(":::") {
        lexemes.delimiter(SourceSpan::new(
            line_start + class_delimiter_start,
            line_start + class_delimiter_start + 3,
        ));
        p.skip_ws();
        let selector_start = p.pos;
        let (cls, _) = parse_id2(p.rest()).ok_or_else(|| "expected class selector".to_string())?;
        p.pos += cls.len();
        let selector_span = SourceSpan::new(
            line_start + selector_start,
            line_start + selector_start + cls.len(),
        );
        lexemes.identifier(selector_span);
        p.skip_ws();
        if !p.eof() {
            return Err("unexpected tokens after section".to_string());
        }
        return Ok(ItemRow {
            indent,
            name,
            item_type: ItemType::Section,
            value: None,
            class_selector: Some(SpannedText {
                span: selector_span,
                text: cls,
            }),
            span: SourceSpan::new(line_start, line_start + line.len()),
        });
    }

    // Leaf: "Name" : 10 (:::class)?
    let value_delimiter_start = p.pos;
    if p.try_consume(':') || p.try_consume(',') {
        lexemes.delimiter(SourceSpan::new(
            line_start + value_delimiter_start,
            line_start + value_delimiter_start + 1,
        ));
        p.skip_ws();
        let value_start = p.pos;
        let token = p
            .parse_number2_token()
            .ok_or_else(|| "expected number".to_string())?;
        let value_span = SourceSpan::new(
            line_start + value_start,
            line_start + value_start + token.len(),
        );
        let Some(value) = parse_number2_value(&token) else {
            lexemes.literal(value_span);
            return Err("expected number".to_string());
        };
        lexemes.number(value_span);
        p.skip_ws();
        let mut class_selector = None;
        let class_delimiter_start = p.pos;
        if p.try_consume_str(":::") {
            lexemes.delimiter(SourceSpan::new(
                line_start + class_delimiter_start,
                line_start + class_delimiter_start + 3,
            ));
            p.skip_ws();
            let selector_start = p.pos;
            let (cls, _) =
                parse_id2(p.rest()).ok_or_else(|| "expected class selector".to_string())?;
            p.pos += cls.len();
            let selector_span = SourceSpan::new(
                line_start + selector_start,
                line_start + selector_start + cls.len(),
            );
            lexemes.identifier(selector_span);
            class_selector = Some(SpannedText {
                span: selector_span,
                text: cls,
            });
            p.skip_ws();
        }
        if !p.eof() {
            return Err("unexpected tokens after leaf".to_string());
        }
        return Ok(ItemRow {
            indent,
            name,
            item_type: ItemType::Leaf,
            value: Some(SpannedValue {
                value,
                source: SpannedText {
                    text: token.clone(),
                    span: value_span,
                },
            }),
            class_selector,
            span: SourceSpan::new(line_start, line_start + line.len()),
        });
    }

    if p.eof() {
        return Ok(ItemRow {
            indent,
            name,
            item_type: ItemType::Section,
            value: None,
            class_selector: None,
            span: SourceSpan::new(line_start, line_start + line.len()),
        });
    }

    Err("expected ':' or ':::' or end of line".to_string())
}

fn parse_id2(input: &str) -> Option<(String, &str)> {
    let mut chars = input.chars();
    let first = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    let mut idx = first.len_utf8();
    for ch in chars {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            idx += ch.len_utf8();
        } else {
            break;
        }
    }
    Some((input[..idx].to_string(), &input[idx..]))
}

fn parse_number2_value(token: &str) -> Option<Value> {
    let no_commas: String = token.chars().filter(|c| *c != ',').collect();
    let mut saw_dot = false;
    let mut cut = 0usize;
    for ch in no_commas.chars() {
        if ch.is_ascii_digit() {
            cut += ch.len_utf8();
            continue;
        }
        if ch == '.' && !saw_dot {
            saw_dot = true;
            cut += 1;
            continue;
        }
        break;
    }
    if cut == 0 {
        return None;
    }
    let prefix = &no_commas[..cut];

    if saw_dot {
        let frac = prefix.split_once('.').map(|(_, b)| b).unwrap_or("");
        if frac.is_empty() || frac.chars().all(|c| c == '0') {
            let int_part = prefix.split_once('.').map(|(a, _)| a).unwrap_or(prefix);
            let i: i64 = int_part.parse().ok()?;
            return Some(Value::Number(i.into()));
        }
        let f: f64 = prefix.parse().ok()?;
        let n = serde_json::Number::from_f64(f)?;
        return Some(Value::Number(n));
    }

    let i: i64 = prefix.parse().ok()?;
    Some(Value::Number(i.into()))
}

struct Parser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn rest(&self) -> &'a str {
        &self.input[self.pos..]
    }

    fn skip_ws(&mut self) {
        while let Some(ch) = self.rest().chars().next() {
            if ch.is_whitespace() {
                self.pos += ch.len_utf8();
                continue;
            }
            break;
        }
    }

    fn try_consume(&mut self, ch: char) -> bool {
        if self.rest().starts_with(ch) {
            self.pos += ch.len_utf8();
            true
        } else {
            false
        }
    }

    fn try_consume_str(&mut self, s: &str) -> bool {
        if self.rest().starts_with(s) {
            self.pos += s.len();
            true
        } else {
            false
        }
    }

    fn parse_string2(&mut self) -> Option<String> {
        let rest = self.rest();
        let quote = rest.chars().next()?;
        if quote != '"' && quote != '\'' {
            return None;
        }
        let mut idx = 1usize;
        for ch in rest[1..].chars() {
            idx += ch.len_utf8();
            if ch == quote {
                let inner = &rest[1..idx - 1];
                self.pos += idx;
                return Some(inner.to_string());
            }
        }
        None
    }

    fn parse_number2_token(&mut self) -> Option<String> {
        let mut idx = 0usize;
        for ch in self.rest().chars() {
            if ch.is_ascii_digit() || ch == '_' || ch == '.' || ch == ',' {
                idx += ch.len_utf8();
                continue;
            }
            break;
        }
        if idx == 0 {
            return None;
        }
        let token = &self.rest()[..idx];
        self.pos += idx;
        Some(token.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Engine, ParseOptions, RenderSemanticModel};
    use futures::executor::block_on;
    use serde_json::json;

    fn parse(text: &str) -> Value {
        let engine = Engine::new();
        block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap()
            .model
    }

    #[test]
    fn treemap_accepts_treemap_beta_header() {
        let model = parse("treemap-beta\n\"A\"");
        assert_eq!(model["root"]["children"][0]["name"], json!("A"));
    }

    #[test]
    fn treemap_accepts_treemap_header() {
        let model = parse("treemap\n\"A\"");
        assert_eq!(model["root"]["children"][0]["name"], json!("A"));
    }

    #[test]
    fn treemap_render_model_uses_typed_variant_without_changing_json_parse() {
        let engine = Engine::new();
        let input = r#"treemap
title Treemap Title
accTitle: Treemap accTitle
accDescr: Treemap accDescr
"Root"
  "Leaf": 42
"#;

        let parsed = engine
            .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(parsed.meta.diagram_type, "treemap");
        match parsed.model {
            RenderSemanticModel::Treemap(model) => {
                assert_eq!(model.title.as_deref(), Some("Treemap Title"));
                assert_eq!(model.acc_title.as_deref(), Some("Treemap accTitle"));
                assert_eq!(model.acc_descr.as_deref(), Some("Treemap accDescr"));
                assert_eq!(model.root.name, "");
                let root_children = model.root.children.as_ref().unwrap();
                assert_eq!(root_children.len(), 1);
                assert_eq!(root_children[0].name, "Root");
                assert_eq!(root_children[0].children.as_ref().unwrap()[0].name, "Leaf");
            }
            other => panic!("treemap render parse should return typed model, got {other:?}"),
        }

        let parsed_json = engine
            .parse_diagram_sync(input, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(parsed_json.model["type"], json!("treemap"));
        assert_eq!(parsed_json.model["title"], json!("Treemap Title"));
        assert_eq!(parsed_json.model["accTitle"], json!("Treemap accTitle"));
        assert_eq!(
            parsed_json.model["root"]["children"][0]["name"],
            json!("Root")
        );
        assert_eq!(
            parsed_json.model["root"]["children"][0]["children"][0]["value"],
            json!(42)
        );
        assert!(parsed_json.model.get("config").is_some());
    }

    fn parse_error(text: &str) -> String {
        let engine = Engine::new();
        let err = block_on(engine.parse_diagram(text, ParseOptions::default())).unwrap_err();
        err.to_string()
    }

    fn deep_treemap_chain(depth: usize) -> String {
        let mut input = String::from("treemap\n");
        for level in 0..depth {
            input.push_str(&" ".repeat(level));
            input.push('"');
            input.push_str(&format!("section{level}"));
            input.push_str("\"\n");
        }
        input.push_str(&" ".repeat(depth));
        input.push_str("\"leaf\": 1\n");
        input
    }

    fn count_render_nodes(root: &TreemapNodeRenderModel) -> usize {
        let mut count = 0usize;
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            count += 1;
            if let Some(children) = node.children.as_ref() {
                for child in children.iter().rev() {
                    stack.push(child);
                }
            }
        }
        count
    }

    #[test]
    fn treemap_deep_chain_semantic_and_render_model_use_heap_traversal() {
        const DEPTH: usize = 1200;
        let input = deep_treemap_chain(DEPTH);

        let model = parse(&input);
        let nodes = model["nodes"].as_array().expect("nodes array");
        assert_eq!(nodes.len(), DEPTH + 1);
        assert_eq!(nodes[0]["name"], json!("section0"));
        assert_eq!(
            nodes
                .last()
                .and_then(|node| node.get("name"))
                .and_then(Value::as_str),
            Some("leaf")
        );

        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(&input, ParseOptions::strict())
            .unwrap()
            .unwrap();
        match parsed.model {
            RenderSemanticModel::Treemap(model) => {
                assert_eq!(count_render_nodes(&model.root), DEPTH + 2);
            }
            other => panic!("treemap render parse should return typed model, got {other:?}"),
        }
    }

    #[test]
    fn treemap_errors_on_trailing_whitespace_only_line() {
        let msg = parse_error("treemap\n\"A\": 1\n    \n");
        assert!(
            msg.contains("unexpected trailing whitespace-only line"),
            "{msg}"
        );
    }

    #[test]
    fn treemap_trailing_comment_is_not_a_whitespace_only_line() {
        let model = parse("treemap\n\"A\": 1\n    %% trailing comment\n");
        assert_eq!(model["root"]["children"][0]["name"], json!("A"));
    }

    #[test]
    fn treemap_rejects_header_with_colon() {
        let msg = parse_error("treemap:\n\"A\": 1\n");
        assert!(msg.contains("expected treemap"), "{msg}");
    }

    #[test]
    fn treemap_rejects_header_with_suffix_tokens() {
        let msg = parse_error("treemap utilities\n\"A\": 1\n");
        assert!(msg.contains("expected treemap"), "{msg}");
    }

    #[test]
    fn treemap_allows_whitespace_only_lines_in_the_middle() {
        let model = parse("treemap\n\"A\": 1\n    \n\"B\": 2\n");
        assert_eq!(model["root"]["children"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn treemap_does_not_treat_unicode_nbsp_as_indentation() {
        let model = parse("treemap\n\"Root\"\n\u{00A0}\"Leaf\": 1\n");
        let children = model["root"]["children"].as_array().unwrap();
        assert_eq!(children.len(), 2);
        assert_eq!(children[0]["name"], json!("Root"));
        assert_eq!(children[1]["name"], json!("Leaf"));
    }

    #[test]
    fn treemap_parses_basic_hierarchy_from_docs() {
        let model = parse(
            r#"treemap-beta
"Section 1"
    "Leaf 1.1": 12
    "Section 1.2"
      "Leaf 1.2.1": 12
"Section 2"
    "Leaf 2.1": 20
    "Leaf 2.2": 25
"#,
        );

        assert_eq!(model["root"]["children"].as_array().unwrap().len(), 2);
        assert_eq!(model["root"]["children"][0]["name"], json!("Section 1"));
        assert_eq!(
            model["root"]["children"][0]["children"][0]["name"],
            json!("Leaf 1.1")
        );
        assert_eq!(
            model["root"]["children"][0]["children"][0]["value"],
            json!(12)
        );
        assert_eq!(model["root"]["children"][1]["name"], json!("Section 2"));
        assert_eq!(
            model["root"]["children"][1]["children"][1]["value"],
            json!(25)
        );
    }

    #[test]
    fn treemap_classdef_applies_compiled_styles() {
        let model = parse(
            r#"treemap-beta
"Main":::important
  "A": 20

classDef important fill:#f96,stroke:#333,stroke-width:2px;
"#,
        );
        assert_eq!(
            model["classes"]["important"]["styles"][0],
            json!("fill:#f96")
        );
        assert_eq!(
            model["root"]["children"][0]["cssCompiledStyles"][0],
            json!("fill:#f96;stroke:#333;stroke-width:2px")
        );
    }

    #[test]
    fn treemap_classdef_rejects_bare_label_style_tokens_like_mermaid_parser() {
        let msg = parse_error(
            r#"treemap
classDef c fill:#ff0000, stroke:rgb(1\,2\,3), color;
"Root":::c
  "Leaf": 1000.00:::c
"#,
        );
        assert!(
            msg.contains("invalid classDef style token `color`"),
            "{msg}"
        );
    }

    #[test]
    fn treemap_build_hierarchy_matches_upstream_utils_test() {
        let items = vec![
            FlatItem {
                level: 0,
                name: "Root".to_string(),
                item_type: ItemType::Section,
                value: None,
                class_selector: None,
                css_compiled_styles: None,
            },
            FlatItem {
                level: 4,
                name: "Branch 1".to_string(),
                item_type: ItemType::Section,
                value: None,
                class_selector: None,
                css_compiled_styles: None,
            },
            FlatItem {
                level: 8,
                name: "Leaf 1.1".to_string(),
                item_type: ItemType::Leaf,
                value: Some(json!(10)),
                class_selector: None,
                css_compiled_styles: None,
            },
            FlatItem {
                level: 8,
                name: "Leaf 1.2".to_string(),
                item_type: ItemType::Leaf,
                value: Some(json!(15)),
                class_selector: None,
                css_compiled_styles: None,
            },
            FlatItem {
                level: 4,
                name: "Branch 2".to_string(),
                item_type: ItemType::Section,
                value: None,
                class_selector: None,
                css_compiled_styles: None,
            },
            FlatItem {
                level: 8,
                name: "Leaf 2.1".to_string(),
                item_type: ItemType::Leaf,
                value: Some(json!(20)),
                class_selector: None,
                css_compiled_styles: None,
            },
            FlatItem {
                level: 8,
                name: "Leaf 2.2".to_string(),
                item_type: ItemType::Leaf,
                value: Some(json!(25)),
                class_selector: None,
                css_compiled_styles: None,
            },
            FlatItem {
                level: 8,
                name: "Leaf 2.3".to_string(),
                item_type: ItemType::Leaf,
                value: Some(json!(30)),
                class_selector: None,
                css_compiled_styles: None,
            },
        ];

        let (arena, roots) = build_hierarchy(&items);
        let root_value = roots
            .iter()
            .map(|&idx| node_to_value(&arena, idx))
            .collect::<Vec<_>>();
        assert_eq!(
            root_value,
            vec![json!({
                "name": "Root",
                "children": [
                    {
                        "name": "Branch 1",
                        "children": [
                            { "name": "Leaf 1.1", "value": 10 },
                            { "name": "Leaf 1.2", "value": 15 },
                        ]
                    },
                    {
                        "name": "Branch 2",
                        "children": [
                            { "name": "Leaf 2.1", "value": 20 },
                            { "name": "Leaf 2.2", "value": 25 },
                            { "name": "Leaf 2.3", "value": 30 },
                        ]
                    }
                ]
            })]
        );
    }

    fn meta() -> ParseMetadata {
        ParseMetadata {
            diagram_type: "treemap".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::MermaidConfig::empty_object(),
            title: None,
        }
    }

    #[test]
    fn treemap_combined_parse_constructs_source_once_and_matches_standalone() {
        let text = concat!(
            "treemap\n",
            "title Product Map\n",
            "accTitle: Product areas\n",
            "accDescr: Product hierarchy\n",
            "\"Root\":::important\n",
            "  \"Leaf\": 42\n",
            "classDef important fill:#f96,stroke:#333;\n",
        );
        let meta = meta();

        crate::diagrams::langium_common::reset_family_syntax_construction_count("treemap");
        let (combined_json, combined_editor) =
            parse_treemap_json_and_editor_facts(text, &meta).unwrap();
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("treemap"),
            1,
            "one combined request must construct Treemap syntax once"
        );

        let standalone_json = parse_treemap(text, &meta).unwrap();
        let standalone_editor = parse_treemap_editor_facts(text, &meta);
        assert_eq!(combined_json, standalone_json);
        assert_eq!(combined_editor, standalone_editor);
    }

    #[test]
    fn treemap_typed_and_json_projections_share_the_same_semantics() {
        let text = concat!(
            "treemap\n",
            "title Product Map\n",
            "\"Root\":::important\n",
            "  \"Leaf\": 42\n",
            "classDef important fill:#f96,stroke:#333;\n",
            "classDef unused fill:#abc;\n",
        );
        let meta = meta();

        let compat = parse_treemap(text, &meta).unwrap();
        let typed = parse_treemap_model_for_render(text, &meta).unwrap();

        assert_eq!(render_model_to_compat_json(&typed, &meta).unwrap(), compat);
        assert_eq!(compat["title"], json!(typed.title));
        assert_eq!(compat["accTitle"], json!(typed.acc_title));
        assert_eq!(compat["accDescr"], json!(typed.acc_descr));
        assert_eq!(compat["root"], serde_json::to_value(&typed.root).unwrap());
        assert_eq!(compat["type"], json!("treemap"));
        assert!(compat["config"].is_object());
        assert_eq!(compat["accTitle"], Value::Null);
        assert_eq!(compat["accDescr"], Value::Null);
        assert!(compat["classes"].get("unused").is_some());
        assert!(typed.classes.contains_key("unused"));

        let empty = parse_treemap_model_for_render("", &meta).unwrap();
        assert_eq!(
            render_model_to_compat_json(&empty, &meta).unwrap(),
            json!({})
        );
    }

    #[test]
    fn treemap_editor_projection_uses_exact_statement_spans() {
        let text = "treemap\n\"Leaf\": 42 :::hot\nclassDef hot fill:#f96;\n";
        let facts = parse_treemap_editor_facts(text, &meta());

        for (name, detail) in [
            ("Leaf", "treemap leaf"),
            ("42", "treemap value"),
            ("hot", "treemap class selector"),
            ("fill:#f96", "treemap class style"),
        ] {
            let start = text.find(name).unwrap();
            assert!(facts.symbols.iter().any(|symbol| {
                symbol.name == name
                    && symbol.detail.as_deref() == Some(detail)
                    && symbol.selection == SourceSpan::new(start, start + name.len())
            }));
        }
    }

    #[test]
    fn treemap_incomplete_statement_recovers_prior_facts_with_exact_error_span() {
        let text = "treemap\n\"Root\"\n  \"Broken\":\n";
        let meta = meta();

        let error = parse_treemap(text, &meta).expect_err("strict parse must reject the leaf");
        let facts = parse_treemap_editor_facts(text, &meta);

        assert_eq!(
            facts.completeness,
            crate::EditorSemanticCompleteness::Recovered
        );
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "Root"));
        let diagnostic = facts
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.message.contains("expected number"))
            .expect("recovery diagnostic");
        assert!(error.to_string().contains("expected number"));
        assert!(diagnostic.message.contains("expected number"));
        let start = text.find("\"Broken\":").unwrap();
        assert_eq!(
            diagnostic.span,
            Some(SourceSpan::new(start, start + "\"Broken\":".len()))
        );
    }

    fn assert_treemap_lexeme(
        facts: &EditorSemanticFacts,
        source: &str,
        kind: crate::EditorLexemeKind,
        span: SourceSpan,
    ) {
        assert!(
            facts.lexemes().iter().any(|lexeme| {
                lexeme.kind() == kind
                    && lexeme.span() == span
                    && source.get(span.start..span.end).is_some()
            }),
            "missing {kind:?} lexeme for {:?} at {span:?}: {:?}",
            source.get(span.start..span.end),
            facts.lexemes()
        );
    }

    #[test]
    fn treemap_recoverable_outcome_keeps_error_prefix_and_later_safe_lines() {
        let text = concat!(
            "treemap\r\n",
            "\"Before\": 1\r\n",
            "  \"Broken\": 12 :::\r\n",
            "classDef hot fill:#f00; trailing\r\n",
            "\"After\": 2\r\n",
        );
        let meta = meta();

        let error = parse_treemap(text, &meta).expect_err("strict parse must use the first error");
        assert!(error.to_string().contains("expected class selector"));

        let facts = parse_treemap_editor_facts(text, &meta);
        assert_eq!(
            facts.completeness,
            crate::EditorSemanticCompleteness::Recovered
        );
        assert!(
            facts
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("expected class selector") })
        );
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("unexpected tokens after classDef")
        }));

        for (kind, token) in [
            (crate::EditorLexemeKind::String, "\"Broken\""),
            (crate::EditorLexemeKind::Number, "12"),
            (crate::EditorLexemeKind::Keyword, "classDef"),
            (crate::EditorLexemeKind::Identifier, "hot"),
            (crate::EditorLexemeKind::Style, "fill:#f00"),
            (crate::EditorLexemeKind::String, "\"After\""),
        ] {
            let start = text.find(token).expect("test token");
            assert_treemap_lexeme(
                &facts,
                text,
                kind,
                SourceSpan::new(start, start + token.len()),
            );
        }

        let broken_start = text.find("\"Broken\"").expect("broken row");
        let colon = broken_start + "\"Broken\"".len();
        assert_treemap_lexeme(
            &facts,
            text,
            crate::EditorLexemeKind::Delimiter,
            SourceSpan::new(colon, colon + 1),
        );
        let class_delimiter = text.find(":::").expect("class delimiter");
        assert_treemap_lexeme(
            &facts,
            text,
            crate::EditorLexemeKind::Delimiter,
            SourceSpan::new(class_delimiter, class_delimiter + 3),
        );
        let semicolon = text.find(';').expect("classDef terminator");
        assert_treemap_lexeme(
            &facts,
            text,
            crate::EditorLexemeKind::Delimiter,
            SourceSpan::new(semicolon, semicolon + 1),
        );

        assert!(facts.symbols.iter().any(|symbol| symbol.name == "After"));
    }

    #[test]
    fn treemap_crlf_lexemes_keep_original_utf8_byte_spans() {
        let text = "treemap-beta\r\n\"根\": 12:::hot\r\n";
        let facts = parse_treemap_editor_facts(text, &meta());
        assert_eq!(
            facts.completeness,
            crate::EditorSemanticCompleteness::Complete
        );

        for (kind, token) in [
            (crate::EditorLexemeKind::Keyword, "treemap-beta"),
            (crate::EditorLexemeKind::String, "\"根\""),
            (crate::EditorLexemeKind::Number, "12"),
            (crate::EditorLexemeKind::Identifier, "hot"),
        ] {
            let start = text.find(token).expect("test token");
            assert_treemap_lexeme(
                &facts,
                text,
                kind,
                SourceSpan::new(start, start + token.len()),
            );
        }

        assert!(
            facts
                .lexemes()
                .iter()
                .all(|lexeme| { !text[lexeme.span().start..lexeme.span().end].contains('\r') })
        );
    }

    #[test]
    fn treemap_multiline_acc_descr_uses_common_syntax_and_recovers_when_unterminated() {
        let complete = "treemap\naccDescr {\nline one\nline two\n}\n\"Root\"\n";
        let meta = meta();
        let (json, facts) = parse_treemap_json_and_editor_facts(complete, &meta).unwrap();
        assert_eq!(json["accDescr"], json!("line one\nline two"));
        let payload_start = complete.find("line one").unwrap();
        let payload = facts
            .symbols
            .iter()
            .find(|symbol| symbol.name == "line one\nline two")
            .expect("multiline accessibility payload");
        assert_eq!(
            payload.selection,
            SourceSpan::new(payload_start, payload_start + "line one\nline two".len())
        );

        let incomplete = "treemap\naccDescr {\nline one\n";
        assert!(parse_treemap(incomplete, &meta).is_err());
        let recovered = parse_treemap_editor_facts(incomplete, &meta);
        assert_eq!(
            recovered.completeness,
            crate::EditorSemanticCompleteness::Recovered
        );
        assert!(recovered.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("unterminated accDescr block")
                && diagnostic.span == Some(SourceSpan::new(incomplete.len(), incomplete.len()))
        }));
    }
}
