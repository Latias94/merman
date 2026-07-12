use crate::diagrams::scan::{split_ascii_indent, strip_line_ending};
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol, Error, MAX_DIAGRAM_NESTING_DEPTH, ParseMetadata, Result, SourceSpan,
};
use serde_json::{Value, json};
use std::collections::HashMap;

const TREE_VIEW_FILE_NODE_TYPE: &str = "file";
const TREE_VIEW_DIRECTORY_NODE_TYPE: &str = "directory";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TreeViewNodeRenderModel {
    pub id: i64,
    pub level: i64,
    pub name: String,
    #[serde(rename = "nodeType")]
    pub node_type: String,
    #[serde(default, rename = "cssClass", skip_serializing_if = "Option::is_none")]
    pub css_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub children: Vec<TreeViewNodeRenderModel>,
}

impl Default for TreeViewNodeRenderModel {
    fn default() -> Self {
        Self {
            id: 0,
            level: -1,
            name: "/".to_string(),
            node_type: TREE_VIEW_DIRECTORY_NODE_TYPE.to_string(),
            css_class: None,
            icon: None,
            description: None,
            children: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TreeViewDiagramRenderModel {
    #[serde(default, rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(default, rename = "accDescr")]
    pub acc_descr: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    pub root: TreeViewNodeRenderModel,
}

impl TreeViewDiagramRenderModel {
    pub(crate) fn sanitize_common_db_fields(&mut self, config: &crate::MermaidConfig) {
        crate::common_db::sanitize_optional_title(&mut self.title, config);
        crate::common_db::sanitize_optional_acc_title(&mut self.acc_title, config);
        crate::common_db::sanitize_optional_acc_descr(&mut self.acc_descr, config);
    }
}

#[derive(Debug, Clone)]
struct FlatNode {
    level: i64,
    name: String,
    node_type: String,
    css_class: Option<String>,
    icon: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Clone)]
struct ParsedTreeViewInput {
    title: Option<String>,
    acc_title: Option<String>,
    acc_descr: Option<String>,
    nodes: Vec<FlatNode>,
}

#[derive(Debug, Clone)]
struct TreeViewNodeLineDetails {
    indent: usize,
    name: String,
    node_type: String,
    css_class: Option<TreeViewSpannedValue>,
    icon: Option<TreeViewSpannedValue>,
    description: Option<TreeViewSpannedValue>,
    span: SourceSpan,
    selection: SourceSpan,
}

#[derive(Debug, Clone)]
struct TreeViewSpannedValue {
    value: String,
    span: SourceSpan,
}

#[derive(Debug, Clone, Default)]
struct TreeViewAnnotations {
    css_class: Option<TreeViewSpannedValue>,
    icon: Option<TreeViewSpannedValue>,
    description: Option<TreeViewSpannedValue>,
}

#[derive(Debug, Clone, Copy)]
struct TreeViewLineFormat {
    box_drawing: bool,
    segment_width: usize,
}

#[derive(Debug, Clone, Copy)]
struct TreeViewLineView<'a> {
    indent: usize,
    content: &'a str,
    content_offset: usize,
}

#[derive(Debug, Clone)]
struct ArenaNode {
    id: i64,
    level: i64,
    name: String,
    node_type: String,
    css_class: Option<String>,
    icon: Option<String>,
    description: Option<String>,
    children: Vec<usize>,
}

pub fn parse_tree_view(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let model = parse_tree_view_model_for_render(code, meta)?;
    let mut nodes = Vec::new();
    flatten_nodes(&model.root, &mut nodes);
    let root = tree_view_node_to_value(&model.root);

    Ok(json!({
        "type": meta.diagram_type,
        "title": model.title,
        "accTitle": model.acc_title,
        "accDescr": model.acc_descr,
        "root": root,
        "nodes": nodes,
    }))
}

pub fn parse_tree_view_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<TreeViewDiagramRenderModel> {
    let parsed = parse_tree_view_input(code, meta)?;
    tree_view_input_to_render_model(parsed, meta)
}

pub fn parse_tree_view_editor_facts(code: &str, meta: &ParseMetadata) -> EditorSemanticFacts {
    let mut facts = EditorSemanticFacts::new();
    let line_format = TreeViewLineFormat::from_code(code);
    let mut lines = code.split_inclusive('\n').peekable();
    let mut offset = 0usize;
    let mut saw_header = false;
    let mut saw_node = false;

    while let Some(segment) = lines.next() {
        let line_start = offset;
        offset += segment.len();
        let line = strip_line_ending(segment);
        let stripped = strip_inline_comment_aware(line);
        let trimmed = stripped.trim();
        if trimmed.is_empty() {
            continue;
        }

        if !saw_header {
            let Some(rest) = trimmed.strip_prefix("treeView-beta") else {
                facts.mark_recovered_with_diagnostic(
                    "expected treeView-beta header",
                    Some(SourceSpan::new(line_start, line_start + trimmed.len())),
                );
                return facts;
            };
            if !rest.trim().is_empty() {
                facts.mark_recovered_with_diagnostic(
                    "unexpected tokens after treeView-beta",
                    Some(SourceSpan::new(line_start, line_start + trimmed.len())),
                );
                return facts;
            }
            saw_header = true;
            continue;
        }

        if !saw_node {
            if let Some(value) = parse_title(stripped) {
                facts.push_directive_prefix("title");
                if !value.is_empty() {
                    push_tree_view_payload_fact(
                        &mut facts,
                        stripped,
                        line_start,
                        &value,
                        "tree view title",
                    );
                }
                continue;
            }
            if let Some(value) = parse_acc_title(stripped) {
                facts.push_directive_prefix("accTitle");
                if !value.is_empty() {
                    push_tree_view_payload_fact(
                        &mut facts,
                        stripped,
                        line_start,
                        &value,
                        "tree view accessibility title",
                    );
                }
                continue;
            }
            if let Some(value) = parse_acc_descr(stripped) {
                facts.push_directive_prefix("accDescr");
                if !value.is_empty() {
                    push_tree_view_payload_fact(
                        &mut facts,
                        stripped,
                        line_start,
                        &value,
                        "tree view accessibility description",
                    );
                }
                continue;
            }
        }

        match parse_node_line_details(stripped, line_start, line_format, meta) {
            Ok(Some(node)) => {
                saw_node = true;
                facts.push_expected_syntax(EditorExpectedSyntax::new(
                    EditorExpectedSyntaxKind::NodeIdentifier,
                    node.selection,
                ));
                facts.push_symbol(EditorSemanticSymbol::new(
                    node.name,
                    Some("tree view node".to_string()),
                    EditorSemanticKind::Namespace,
                    node.span,
                    node.selection,
                ));
                for (value, detail) in [
                    (node.css_class.as_ref(), "tree view class"),
                    (node.icon.as_ref(), "tree view icon"),
                    (node.description.as_ref(), "tree view description"),
                ] {
                    if let Some(value) = value {
                        push_tree_view_spanned_payload_fact(&mut facts, value, detail);
                    }
                }
            }
            Ok(None) => {
                continue;
            }
            Err(err) => {
                facts.mark_recovered_with_diagnostic(
                    format!("treeView parser recovered after parse error: {err}"),
                    Some(SourceSpan::new(line_start, line_start + trimmed.len())),
                );
                return facts;
            }
        }
    }

    facts
}

fn parse_tree_view_input(code: &str, meta: &ParseMetadata) -> Result<ParsedTreeViewInput> {
    let raw_lines = code
        .lines()
        .map(|line| line.trim_end_matches('\r'))
        .collect::<Vec<_>>();
    let mut header_index = None;
    let mut header = String::new();
    for (idx, line) in raw_lines.iter().enumerate() {
        let t = strip_inline_comment_aware(line).trim();
        if t.is_empty() {
            continue;
        }
        header_index = Some(idx);
        header = t.to_string();
        break;
    }
    let Some(header_index) = header_index else {
        return Err(parse_error(meta, "expected treeView-beta"));
    };

    let Some(rest) = header.strip_prefix("treeView-beta") else {
        return Err(parse_error(meta, "expected treeView-beta"));
    };
    if !rest.trim().is_empty() {
        return Err(parse_error(meta, "unexpected tokens after treeView-beta"));
    }

    let mut title = None;
    let mut acc_title = None;
    let mut acc_descr = None;
    let mut nodes = Vec::new();
    let mut saw_node = false;
    let line_format = TreeViewLineFormat::from_lines(&raw_lines[header_index + 1..]);

    for raw in &raw_lines[header_index + 1..] {
        let t = strip_inline_comment_aware(raw);
        if t.trim().is_empty() {
            continue;
        }

        if !saw_node {
            if let Some(v) = parse_title(t) {
                title = Some(v);
                continue;
            }
            if let Some(v) = parse_acc_title(t) {
                acc_title = Some(v);
                continue;
            }
            if let Some(v) = parse_acc_descr(t) {
                acc_descr = Some(v);
                continue;
            }
        }

        saw_node = true;
        if let Some(node) = parse_node_line(t, line_format, meta)? {
            nodes.push(node);
        }
    }

    Ok(ParsedTreeViewInput {
        title,
        acc_title,
        acc_descr,
        nodes,
    })
}

fn tree_view_input_to_render_model(
    parsed: ParsedTreeViewInput,
    meta: &ParseMetadata,
) -> Result<TreeViewDiagramRenderModel> {
    let mut arena = vec![ArenaNode {
        id: 0,
        level: -1,
        name: "/".to_string(),
        node_type: TREE_VIEW_DIRECTORY_NODE_TYPE.to_string(),
        css_class: None,
        icon: None,
        description: None,
        children: Vec::new(),
    }];
    let mut stack = vec![0usize];
    for (next_id, flat) in (1i64..).zip(parsed.nodes) {
        while stack
            .last()
            .and_then(|&idx| arena.get(idx))
            .is_some_and(|node| flat.level <= node.level)
        {
            stack.pop();
        }

        let parent = stack.last().copied().unwrap_or(0);
        let idx = arena.len();
        arena.push(ArenaNode {
            id: next_id,
            level: flat.level,
            name: flat.name,
            node_type: flat.node_type,
            css_class: flat.css_class,
            icon: flat.icon,
            description: flat.description,
            children: Vec::new(),
        });
        arena[parent].children.push(idx);
        stack.push(idx);
        if stack.len().saturating_sub(1) > MAX_DIAGRAM_NESTING_DEPTH {
            return Err(parse_error(
                meta,
                format!("treeView nesting depth exceeds {MAX_DIAGRAM_NESTING_DEPTH}"),
            ));
        }
    }

    Ok(TreeViewDiagramRenderModel {
        title: parsed.title,
        acc_title: parsed.acc_title,
        acc_descr: parsed.acc_descr,
        root: arena_node_to_render_model(&arena, 0),
    })
}

fn arena_node_to_render_model(arena: &[ArenaNode], idx: usize) -> TreeViewNodeRenderModel {
    let mut models: Vec<Option<TreeViewNodeRenderModel>> = vec![None; arena.len()];
    let mut stack = vec![(idx, false)];

    while let Some((node_idx, visited)) = stack.pop() {
        let Some(node) = arena.get(node_idx) else {
            continue;
        };

        if visited {
            let children = node
                .children
                .iter()
                .filter_map(|&child_idx| models.get_mut(child_idx).and_then(Option::take))
                .collect();
            models[node_idx] = Some(TreeViewNodeRenderModel {
                id: node.id,
                level: node.level,
                name: node.name.clone(),
                node_type: node.node_type.clone(),
                css_class: node.css_class.clone(),
                icon: node.icon.clone(),
                description: node.description.clone(),
                children,
            });
        } else {
            stack.push((node_idx, true));
            for &child_idx in node.children.iter().rev() {
                stack.push((child_idx, false));
            }
        }
    }

    models
        .get_mut(idx)
        .and_then(Option::take)
        .unwrap_or_default()
}

fn flatten_nodes(node: &TreeViewNodeRenderModel, out: &mut Vec<Value>) {
    let mut stack = vec![node];
    while let Some(current) = stack.pop() {
        out.push(tree_view_flat_node_to_value(current));
        for child in current.children.iter().rev() {
            stack.push(child);
        }
    }
}

fn tree_view_node_to_value(root: &TreeViewNodeRenderModel) -> Value {
    let mut values: HashMap<*const TreeViewNodeRenderModel, Value> = HashMap::new();
    let mut stack = vec![(root, false)];

    while let Some((node, visited)) = stack.pop() {
        let node_ptr = std::ptr::from_ref(node);
        if visited {
            let children = node
                .children
                .iter()
                .filter_map(|child| values.remove(&std::ptr::from_ref(child)))
                .collect::<Vec<_>>();
            values.insert(
                node_ptr,
                tree_view_node_with_children_to_value(node, children),
            );
        } else {
            stack.push((node, true));
            for child in node.children.iter().rev() {
                stack.push((child, false));
            }
        }
    }

    values.remove(&std::ptr::from_ref(root)).unwrap_or_else(|| {
        json!({
            "id": 0,
            "level": -1,
            "name": "/",
            "nodeType": TREE_VIEW_DIRECTORY_NODE_TYPE,
            "children": [],
        })
    })
}

fn tree_view_flat_node_to_value(node: &TreeViewNodeRenderModel) -> Value {
    let mut value = serde_json::Map::new();
    value.insert("id".to_string(), json!(node.id));
    value.insert("level".to_string(), json!(node.level));
    value.insert("name".to_string(), json!(node.name));
    value.insert("nodeType".to_string(), json!(node.node_type));
    if let Some(css_class) = &node.css_class {
        value.insert("cssClass".to_string(), json!(css_class));
    }
    if let Some(icon) = &node.icon {
        value.insert("icon".to_string(), json!(icon));
    }
    if let Some(description) = &node.description {
        value.insert("description".to_string(), json!(description));
    }
    Value::Object(value)
}

fn tree_view_node_with_children_to_value(
    node: &TreeViewNodeRenderModel,
    children: Vec<Value>,
) -> Value {
    let mut value = match tree_view_flat_node_to_value(node) {
        Value::Object(value) => value,
        _ => serde_json::Map::new(),
    };
    value.insert("children".to_string(), Value::Array(children));
    Value::Object(value)
}

fn parse_node_line(
    line: &str,
    line_format: TreeViewLineFormat,
    meta: &ParseMetadata,
) -> Result<Option<FlatNode>> {
    let Some(details) = parse_node_line_details(line, 0, line_format, meta)? else {
        return Ok(None);
    };
    Ok(Some(FlatNode {
        level: details.indent as i64,
        name: details.name,
        node_type: details.node_type,
        css_class: details.css_class.map(|value| value.value),
        icon: details.icon.map(|value| value.value),
        description: details.description.map(|value| value.value),
    }))
}

fn parse_node_line_details(
    line: &str,
    line_start: usize,
    line_format: TreeViewLineFormat,
    meta: &ParseMetadata,
) -> Result<Option<TreeViewNodeLineDetails>> {
    let Some(line_view) = tree_view_line_view(line, line_format, meta)? else {
        return Ok(None);
    };
    parse_node_content(line_view, line_start, meta).map(Some)
}

impl TreeViewLineFormat {
    fn from_code(code: &str) -> Self {
        let lines = code
            .lines()
            .map(|line| line.trim_end_matches('\r'))
            .collect::<Vec<_>>();
        let Some(header_index) = lines
            .iter()
            .position(|line| strip_inline_comment_aware(line).trim() == "treeView-beta")
        else {
            return Self {
                box_drawing: false,
                segment_width: 4,
            };
        };
        Self::from_lines(&lines[header_index + 1..])
    }

    fn from_lines(lines: &[&str]) -> Self {
        let mut content_lines = Vec::new();
        for line in lines {
            if should_skip_tree_view_box_detection_line(line) {
                continue;
            }
            content_lines.push(line.replace('\t', "    "));
        }
        let box_drawing = content_lines
            .iter()
            .any(|line| line.chars().any(is_tree_view_box_char));
        let segment_width = if box_drawing {
            infer_tree_view_box_segment_width(&content_lines)
        } else {
            4
        };
        Self {
            box_drawing,
            segment_width,
        }
    }
}

fn tree_view_line_view<'a>(
    line: &'a str,
    line_format: TreeViewLineFormat,
    meta: &ParseMetadata,
) -> Result<Option<TreeViewLineView<'a>>> {
    if !line_format.box_drawing {
        let (indent, rest) = split_ascii_indent(line);
        let content = rest.trim_end();
        if content.is_empty() {
            return Ok(None);
        }
        let content_offset = line.len().saturating_sub(rest.len());
        return Ok(Some(TreeViewLineView {
            indent,
            content,
            content_offset,
        }));
    }

    if is_tree_view_decoration_only(line) {
        return Ok(None);
    }

    if let Some((branch_byte, branch_col, branch_char)) = find_tree_view_branch_char(line) {
        let depth = ((branch_col as f64 / line_format.segment_width as f64).round() as usize) + 1;
        let mut content_offset = branch_byte + branch_char.len_utf8();
        while let Some(ch) = line[content_offset..].chars().next() {
            if is_tree_view_dash_char(ch) {
                content_offset += ch.len_utf8();
            } else {
                break;
            }
        }
        while let Some(ch) = line[content_offset..].chars().next() {
            if ch == ' ' || ch == '\t' {
                content_offset += ch.len_utf8();
            } else {
                break;
            }
        }
        let content = line[content_offset..].trim_end();
        if content.is_empty() {
            return Err(parse_error(
                meta,
                "empty tree node after box-drawing prefix",
            ));
        }
        return Ok(Some(TreeViewLineView {
            indent: depth * 4,
            content,
            content_offset,
        }));
    }

    if is_tree_view_box_drawing_only(line) {
        return Ok(None);
    }

    if line.chars().any(is_tree_view_box_char) {
        let content = line.trim_end();
        return Ok(Some(TreeViewLineView {
            indent: 0,
            content,
            content_offset: 0,
        }));
    }

    if line.chars().next().is_some_and(char::is_whitespace) {
        return Err(parse_error(
            meta,
            "unexpected indentation without box-drawing prefix in treeView box-drawing input",
        ));
    }

    let content = line.trim_end();
    Ok(Some(TreeViewLineView {
        indent: 0,
        content,
        content_offset: 0,
    }))
}

fn parse_node_content(
    line_view: TreeViewLineView<'_>,
    line_start: usize,
    meta: &ParseMetadata,
) -> Result<TreeViewNodeLineDetails> {
    let content = line_view.content;
    let content_abs = line_start + line_view.content_offset;
    let span = SourceSpan::new(content_abs, content_abs + content.len());

    let (raw_name, name_start, name_end, suffix_start) =
        if let Some((_, quote @ ('"' | '\''))) = content.char_indices().next() {
            let mut end = None;
            for (idx, ch) in content[quote.len_utf8()..].char_indices() {
                if ch == quote {
                    end = Some(quote.len_utf8() + idx);
                    break;
                }
            }
            let Some(end_idx) = end else {
                return Err(parse_error(meta, "unterminated quoted tree node name"));
            };
            (
                content[quote.len_utf8()..end_idx].to_string(),
                quote.len_utf8(),
                end_idx,
                end_idx + quote.len_utf8(),
            )
        } else {
            let annotation_start =
                find_next_tree_view_annotation_start(content, 0).unwrap_or(content.len());
            let name_end = trim_end_byte_index(&content[..annotation_start]);
            if name_end == 0 {
                return Err(parse_error(meta, "expected tree node name"));
            }
            (content[..name_end].to_string(), 0, name_end, name_end)
        };

    let suffix = &content[suffix_start..];
    let suffix_abs = content_abs + suffix_start;
    let annotations = parse_tree_view_annotations(suffix, suffix_abs, meta)?;
    let (name, node_type, selection_end) = normalize_tree_view_node_name(raw_name, name_end);
    let selection = SourceSpan::new(content_abs + name_start, content_abs + selection_end);

    Ok(TreeViewNodeLineDetails {
        indent: line_view.indent,
        name,
        node_type,
        css_class: annotations.css_class,
        icon: annotations.icon,
        description: annotations.description,
        span,
        selection,
    })
}

fn normalize_tree_view_node_name(
    raw_name: String,
    raw_selection_end: usize,
) -> (String, String, usize) {
    if raw_name.ends_with('/') {
        let mut name = raw_name;
        name.pop();
        (
            name,
            TREE_VIEW_DIRECTORY_NODE_TYPE.to_string(),
            raw_selection_end.saturating_sub('/'.len_utf8()),
        )
    } else {
        (
            raw_name,
            TREE_VIEW_FILE_NODE_TYPE.to_string(),
            raw_selection_end,
        )
    }
}

fn parse_tree_view_annotations(
    suffix: &str,
    abs_base: usize,
    meta: &ParseMetadata,
) -> Result<TreeViewAnnotations> {
    let mut annotations = TreeViewAnnotations::default();
    let mut pos = 0usize;
    while pos < suffix.len() {
        pos = skip_ascii_whitespace(suffix, pos);
        if pos >= suffix.len() {
            break;
        }

        if suffix[pos..].starts_with(":::") && is_annotation_token_boundary(suffix, pos) {
            let value_start = skip_ascii_whitespace(suffix, pos + 3);
            let value_end =
                find_next_tree_view_annotation_start(suffix, value_start).unwrap_or(suffix.len());
            let (trimmed_start, trimmed_end) = trim_ascii_span(suffix, value_start, value_end);
            if trimmed_start == trimmed_end {
                return Err(parse_error(meta, "expected tree node class after :::"));
            }
            annotations.css_class = Some(TreeViewSpannedValue {
                value: suffix[trimmed_start..trimmed_end].to_string(),
                span: SourceSpan::new(abs_base + trimmed_start, abs_base + trimmed_end),
            });
            pos = value_end;
            continue;
        }

        if suffix[pos..].starts_with("icon(") && is_annotation_token_boundary(suffix, pos) {
            let value_start = pos + "icon(".len();
            let Some(close_rel) = suffix[value_start..].find(')') else {
                return Err(parse_error(meta, "unterminated tree node icon annotation"));
            };
            let value_end = value_start + close_rel;
            let (trimmed_start, trimmed_end) = trim_ascii_span(suffix, value_start, value_end);
            let value = if trimmed_start == trimmed_end {
                "none".to_string()
            } else {
                suffix[trimmed_start..trimmed_end].to_string()
            };
            annotations.icon = Some(TreeViewSpannedValue {
                value,
                span: SourceSpan::new(abs_base + trimmed_start, abs_base + trimmed_end),
            });
            pos = value_end + ')'.len_utf8();
            continue;
        }

        if suffix[pos..].starts_with("##") && is_annotation_token_boundary(suffix, pos) {
            let value_start = skip_ascii_whitespace(suffix, pos + 2);
            let (trimmed_start, trimmed_end) = trim_ascii_span(suffix, value_start, suffix.len());
            if trimmed_start != trimmed_end {
                annotations.description = Some(TreeViewSpannedValue {
                    value: suffix[trimmed_start..trimmed_end].to_string(),
                    span: SourceSpan::new(abs_base + trimmed_start, abs_base + trimmed_end),
                });
            }
            break;
        }

        return Err(parse_error(meta, "unexpected tokens after tree node name"));
    }
    Ok(annotations)
}

fn push_tree_view_spanned_payload_fact(
    facts: &mut EditorSemanticFacts,
    value: &TreeViewSpannedValue,
    detail: &'static str,
) {
    if value.value.is_empty() || value.span.start == value.span.end {
        return;
    }
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::Payload,
        value.span,
    ));
    facts.push_symbol(EditorSemanticSymbol::payload(
        value.value.clone(),
        Some(detail.to_string()),
        EditorSemanticKind::String,
        value.span,
        value.span,
    ));
}

fn push_tree_view_payload_fact(
    facts: &mut EditorSemanticFacts,
    line: &str,
    line_start: usize,
    value: &str,
    detail: &'static str,
) {
    if value.is_empty() {
        return;
    }
    if let Some(rel) = line.find(value) {
        let span = SourceSpan::new(line_start + rel, line_start + rel + value.len());
        facts.push_expected_syntax(EditorExpectedSyntax::new(
            EditorExpectedSyntaxKind::Payload,
            span,
        ));
        facts.push_symbol(EditorSemanticSymbol::payload(
            value.to_string(),
            Some(detail.to_string()),
            EditorSemanticKind::String,
            span,
            span,
        ));
    }
}

fn should_skip_tree_view_box_detection_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty()
        || is_tree_view_comment_line(line)
        || is_tree_view_metadata_line(line)
        || is_tree_view_decoration_only(line)
}

fn is_tree_view_comment_line(line: &str) -> bool {
    line.trim_start().starts_with("%%")
}

fn is_tree_view_metadata_line(line: &str) -> bool {
    let t = line.trim_start();
    if t.starts_with("title ") || t.starts_with("title\t") {
        return true;
    }
    if let Some(rest) = t.strip_prefix("accTitle") {
        return rest.trim_start().starts_with(':');
    }
    if let Some(rest) = t.strip_prefix("accDescr") {
        let rest = rest.trim_start();
        return rest.starts_with(':') || rest.starts_with('{');
    }
    false
}

fn infer_tree_view_box_segment_width(content_lines: &[String]) -> usize {
    for line in content_lines {
        if let Some((col, _)) = line
            .chars()
            .enumerate()
            .find(|(_, ch)| is_tree_view_branch_char(*ch))
            && col > 0
        {
            return col;
        }
    }
    4
}

fn find_tree_view_branch_char(line: &str) -> Option<(usize, usize, char)> {
    let mut col = 0usize;
    for (idx, ch) in line.char_indices() {
        if is_tree_view_branch_char(ch) {
            return Some((idx, col, ch));
        }
        col += if ch == '\t' { 4 } else { 1 };
    }
    None
}

fn is_tree_view_box_char(ch: char) -> bool {
    matches!(ch, '─' | '━' | '│' | '┃' | '└' | '┗' | '├' | '┣')
}

fn is_tree_view_branch_char(ch: char) -> bool {
    matches!(ch, '└' | '┗' | '├' | '┣')
}

fn is_tree_view_dash_char(ch: char) -> bool {
    matches!(ch, '─' | '━')
}

fn is_tree_view_decoration_only(line: &str) -> bool {
    !line.is_empty()
        && line
            .chars()
            .all(|ch| ch.is_whitespace() || matches!(ch, '│' | '┃'))
}

fn is_tree_view_box_drawing_only(line: &str) -> bool {
    !line.is_empty()
        && line
            .chars()
            .all(|ch| ch.is_whitespace() || is_tree_view_box_char(ch))
}

fn find_next_tree_view_annotation_start(s: &str, from: usize) -> Option<usize> {
    for (idx, _) in s.char_indices().filter(|(idx, _)| *idx >= from) {
        if (s[idx..].starts_with(":::")
            || s[idx..].starts_with("##")
            || s[idx..].starts_with("icon("))
            && ((from == 0 && idx == 0) || is_annotation_token_boundary(s, idx))
        {
            return Some(idx);
        }
    }
    None
}

fn is_annotation_token_boundary(s: &str, idx: usize) -> bool {
    idx > 0
        && s[..idx]
            .chars()
            .next_back()
            .is_some_and(char::is_whitespace)
}

fn skip_ascii_whitespace(s: &str, mut idx: usize) -> usize {
    while let Some(ch) = s[idx..].chars().next() {
        if ch == ' ' || ch == '\t' {
            idx += ch.len_utf8();
        } else {
            break;
        }
        if idx >= s.len() {
            break;
        }
    }
    idx
}

fn trim_ascii_span(s: &str, start: usize, end: usize) -> (usize, usize) {
    let mut start = start;
    let mut end = end;
    while start < end {
        let Some(ch) = s[start..end].chars().next() else {
            break;
        };
        if ch == ' ' || ch == '\t' {
            start += ch.len_utf8();
        } else {
            break;
        }
    }
    while start < end {
        let Some(ch) = s[start..end].chars().next_back() else {
            break;
        };
        if ch == ' ' || ch == '\t' {
            end -= ch.len_utf8();
        } else {
            break;
        }
    }
    (start, end)
}

fn trim_end_byte_index(s: &str) -> usize {
    let mut end = s.len();
    while end > 0 {
        let Some(ch) = s[..end].chars().next_back() else {
            break;
        };
        if ch == ' ' || ch == '\t' {
            end -= ch.len_utf8();
        } else {
            break;
        }
    }
    end
}

fn parse_title(line: &str) -> Option<String> {
    let t = line.trim_start();
    if t == "title" {
        return Some(String::new());
    }
    let rest = t.strip_prefix("title")?;
    rest.chars()
        .next()
        .is_some_and(char::is_whitespace)
        .then(|| rest.trim().to_string())
}

fn parse_acc_title(line: &str) -> Option<String> {
    let t = line.trim_start();
    let rest = t.strip_prefix("accTitle")?.trim_start();
    let rest = rest.strip_prefix(':')?;
    Some(rest.trim().to_string())
}

fn parse_acc_descr(line: &str) -> Option<String> {
    let t = line.trim_start();
    let rest = t.strip_prefix("accDescr")?.trim_start();
    if let Some(rest) = rest.strip_prefix(':') {
        return Some(rest.trim().to_string());
    }
    let rest = rest.strip_prefix('{')?;
    let rest = rest.strip_suffix('}')?;
    Some(rest.trim().to_string())
}

fn strip_inline_comment_aware(line: &str) -> &str {
    let mut in_single = false;
    let mut in_double = false;
    let mut iter = line.char_indices().peekable();
    while let Some((idx, ch)) = iter.next() {
        match ch {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '%' if !in_single
                && !in_double
                && iter.peek().is_some_and(|(_, next)| *next == '%') =>
            {
                return &line[..idx];
            }
            _ => {}
        }
    }
    line
}

fn parse_error(meta: &ParseMetadata, message: impl Into<String>) -> Error {
    Error::diagram_parse_fallback(meta.diagram_type.clone(), message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EditorExpectedSyntaxKind, Engine, MermaidConfig, ParseMetadata, ParseOptions, SourceSpan,
    };

    fn meta() -> ParseMetadata {
        ParseMetadata {
            diagram_type: "treeView".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: MermaidConfig::empty_object(),
            title: None,
        }
    }

    #[test]
    fn builds_virtual_root_and_indentation_tree() {
        let model = parse_tree_view_model_for_render(
            r#"treeView-beta
"Root"
    "Child1"
    "Child2"
        "Grandchild"
"Sibling""#,
            &meta(),
        )
        .unwrap();

        assert_eq!(model.root.name, "/");
        assert_eq!(model.root.children.len(), 2);
        assert_eq!(model.root.children[0].name, "Root");
        assert_eq!(model.root.children[0].children.len(), 2);
        assert_eq!(
            model.root.children[0].children[1].children[0].name,
            "Grandchild"
        );
        assert_eq!(model.root.children[1].name, "Sibling");
    }

    #[test]
    fn parses_title_and_accessibility_before_nodes() {
        let model = parse_tree_view_model_for_render(
            r#"treeView-beta
title My Tree
accTitle: Accessible Title
accDescr: Accessible Description
"Root""#,
            &meta(),
        )
        .unwrap();

        assert_eq!(model.title.as_deref(), Some("My Tree"));
        assert_eq!(model.acc_title.as_deref(), Some("Accessible Title"));
        assert_eq!(model.acc_descr.as_deref(), Some("Accessible Description"));
    }

    #[test]
    fn parses_mermaid_11_16_node_annotations_and_bare_names() {
        let semantic = parse_tree_view(
            r#"treeView-beta
src/ :::highlight icon(folder) ## source directory
App.tsx icon(logos:react)
index.js icon()
".gitignore"
'My Documents/' :::important
plain file.ts ## entry point
"#,
            &meta(),
        )
        .unwrap();
        let nodes = semantic["nodes"].as_array().expect("nodes array");

        assert_eq!(nodes[1]["name"], json!("src"));
        assert_eq!(nodes[1]["nodeType"], json!("directory"));
        assert_eq!(nodes[1]["cssClass"], json!("highlight"));
        assert_eq!(nodes[1]["icon"], json!("folder"));
        assert_eq!(nodes[1]["description"], json!("source directory"));
        assert_eq!(nodes[2]["name"], json!("App.tsx"));
        assert_eq!(nodes[2]["nodeType"], json!("file"));
        assert_eq!(nodes[2]["icon"], json!("logos:react"));
        assert_eq!(nodes[3]["icon"], json!("none"));
        assert_eq!(nodes[4]["name"], json!(".gitignore"));
        assert_eq!(nodes[5]["name"], json!("My Documents"));
        assert_eq!(nodes[5]["nodeType"], json!("directory"));
        assert_eq!(nodes[5]["cssClass"], json!("important"));
        assert_eq!(nodes[6]["name"], json!("plain file.ts"));
        assert_eq!(nodes[6]["description"], json!("entry point"));
    }

    #[test]
    fn annotation_markers_inside_bare_node_names_remain_literal() {
        let semantic = parse_tree_view(
            r#"treeView-beta
foo:::bar
file##notes
"#,
            &meta(),
        )
        .unwrap();
        let nodes = semantic["nodes"].as_array().expect("nodes array");

        assert_eq!(nodes[1]["name"], json!("foo:::bar"));
        assert!(nodes[1].get("cssClass").is_none());
        assert_eq!(nodes[2]["name"], json!("file##notes"));
        assert!(nodes[2].get("description").is_none());
    }

    #[test]
    fn annotation_tokens_cannot_replace_a_tree_node_name() {
        for annotation in [":::highlight", "icon(file)", "## description"] {
            let input = format!("treeView-beta\n{annotation}\n");
            let err = parse_tree_view(&input, &meta()).expect_err("node name is required");

            assert!(err.to_string().contains("expected tree node name"), "{err}");
        }
    }

    #[test]
    fn parses_mermaid_11_16_box_drawing_as_indented_tree() {
        let indent = parse_tree_view(
            r#"treeView-beta
my-project/
    src/ :::highlight
        App.tsx icon(react) ## main component
        index.ts ## entry point
    package.json
    README.md ## project docs
"#,
            &meta(),
        )
        .unwrap();
        let box_draw = parse_tree_view(
            r#"treeView-beta
my-project/
├── src/ :::highlight
│   ├── App.tsx icon(react) ## main component
│   └── index.ts ## entry point
├── package.json
└── README.md ## project docs
"#,
            &meta(),
        )
        .unwrap();

        assert_eq!(box_draw["root"], indent["root"]);
    }

    #[test]
    fn rejects_mixed_indentation_inside_box_drawing_tree() {
        let err = parse_tree_view_model_for_render(
            r#"treeView-beta
root/
├── src/
    mixed.txt
"#,
            &meta(),
        )
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("unexpected indentation without box-drawing prefix"),
            "{err}"
        );
    }

    #[test]
    fn rejects_tree_view_input_beyond_nesting_limit() {
        let mut input = String::from("treeView-beta\n");
        for depth in 0..=crate::MAX_DIAGRAM_NESTING_DEPTH {
            input.push_str(&" ".repeat(depth));
            input.push('"');
            input.push_str(&format!("n{depth}"));
            input.push_str("\"\n");
        }

        let err = parse_tree_view_model_for_render(&input, &meta()).unwrap_err();
        assert!(
            err.to_string().contains("treeView nesting depth exceeds"),
            "{err}"
        );
    }

    #[test]
    fn parse_tree_view_projects_max_allowed_chain() {
        let mut input = String::from("treeView-beta\n");
        for depth in 0..crate::MAX_DIAGRAM_NESTING_DEPTH {
            input.push_str(&" ".repeat(depth));
            input.push('"');
            input.push_str(&format!("n{depth}"));
            input.push_str("\"\n");
        }

        let semantic = parse_tree_view(&input, &meta()).unwrap();
        let nodes = semantic
            .get("nodes")
            .and_then(Value::as_array)
            .expect("nodes array");

        assert_eq!(nodes.len(), crate::MAX_DIAGRAM_NESTING_DEPTH + 1);
        assert_eq!(nodes[0].get("name").and_then(Value::as_str), Some("/"));
        assert_eq!(nodes[1].get("name").and_then(Value::as_str), Some("n0"));
        let expected_last = format!("n{}", crate::MAX_DIAGRAM_NESTING_DEPTH - 1);
        assert_eq!(
            nodes
                .last()
                .and_then(|node| node.get("name"))
                .and_then(Value::as_str),
            Some(expected_last.as_str())
        );
    }

    #[test]
    fn parse_tree_view_editor_facts_expose_parser_backed_spans() {
        let engine = Engine::new();
        let text = r#"
treeView-beta
title My Tree
accTitle: Accessible Title
accDescr: Accessible Description
"Root"
  "Child 1"
"#;
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("treeView", text, ParseOptions::strict())
            .unwrap()
            .unwrap();

        assert!(facts.directive_prefixes.iter().any(|p| p == "title"));
        assert!(facts.directive_prefixes.iter().any(|p| p == "accTitle"));
        assert!(facts.directive_prefixes.iter().any(|p| p == "accDescr"));
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "Root"));
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "Child 1"));

        let root_start = text.find("Root").unwrap();
        assert!(facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::NodeIdentifier
                && expected.span == SourceSpan::new(root_start, root_start + "Root".len())
        }));
    }

    #[test]
    fn parse_tree_view_editor_facts_preserve_box_drawing_annotation_spans() {
        let engine = Engine::new();
        let text = r#"
treeView-beta
├── App.tsx :::highlight icon(react) ## main component
"#;
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("treeView", text, ParseOptions::strict())
            .unwrap()
            .unwrap();

        for (payload, detail) in [
            ("App.tsx", "tree view node"),
            ("highlight", "tree view class"),
            ("react", "tree view icon"),
            ("main component", "tree view description"),
        ] {
            let start = text.find(payload).unwrap();
            assert!(
                facts.symbols.iter().any(|symbol| {
                    symbol.name == payload
                        && symbol.detail.as_deref() == Some(detail)
                        && symbol.selection == SourceSpan::new(start, start + payload.len())
                }),
                "missing {detail} payload {payload:?}"
            );
        }
    }
}
