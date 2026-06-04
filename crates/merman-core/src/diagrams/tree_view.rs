use crate::{Error, ParseMetadata, Result};
use serde_json::{Value, json};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TreeViewNodeRenderModel {
    pub id: i64,
    pub level: i64,
    pub name: String,
    #[serde(default)]
    pub children: Vec<TreeViewNodeRenderModel>,
}

impl Default for TreeViewNodeRenderModel {
    fn default() -> Self {
        Self {
            id: 0,
            level: -1,
            name: "/".to_string(),
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

#[derive(Debug, Clone)]
struct FlatNode {
    level: i64,
    name: String,
}

#[derive(Debug, Clone)]
struct ParsedTreeViewInput {
    title: Option<String>,
    acc_title: Option<String>,
    acc_descr: Option<String>,
    nodes: Vec<FlatNode>,
}

#[derive(Debug, Clone)]
struct ArenaNode {
    id: i64,
    level: i64,
    name: String,
    children: Vec<usize>,
}

pub fn parse_tree_view(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let model = parse_tree_view_model_for_render(code, meta)?;
    let mut nodes = Vec::new();
    flatten_nodes(&model.root, &mut nodes);

    Ok(json!({
        "type": meta.diagram_type,
        "title": model.title,
        "accTitle": model.acc_title,
        "accDescr": model.acc_descr,
        "root": model.root,
        "nodes": nodes,
    }))
}

pub fn parse_tree_view_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<TreeViewDiagramRenderModel> {
    let parsed = parse_tree_view_input(code, meta)?;
    Ok(tree_view_input_to_render_model(parsed))
}

fn parse_tree_view_input(code: &str, meta: &ParseMetadata) -> Result<ParsedTreeViewInput> {
    let mut lines = code.lines();
    let header = loop {
        let Some(line) = lines.next() else {
            return Err(parse_error(meta, "expected treeView-beta"));
        };
        let t = strip_inline_comment_aware(line).trim();
        if t.is_empty() {
            continue;
        }
        break t.to_string();
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

    for raw in lines {
        let raw = raw.trim_end_matches('\r');
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
        nodes.push(parse_node_line(t, meta)?);
    }

    Ok(ParsedTreeViewInput {
        title,
        acc_title,
        acc_descr,
        nodes,
    })
}

fn tree_view_input_to_render_model(parsed: ParsedTreeViewInput) -> TreeViewDiagramRenderModel {
    let mut arena = vec![ArenaNode {
        id: 0,
        level: -1,
        name: "/".to_string(),
        children: Vec::new(),
    }];
    let mut stack = vec![0usize];
    let mut next_id = 1i64;

    for flat in parsed.nodes {
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
            children: Vec::new(),
        });
        next_id += 1;
        arena[parent].children.push(idx);
        stack.push(idx);
    }

    TreeViewDiagramRenderModel {
        title: parsed.title,
        acc_title: parsed.acc_title,
        acc_descr: parsed.acc_descr,
        root: arena_node_to_render_model(&arena, 0),
    }
}

fn arena_node_to_render_model(arena: &[ArenaNode], idx: usize) -> TreeViewNodeRenderModel {
    let node = &arena[idx];
    TreeViewNodeRenderModel {
        id: node.id,
        level: node.level,
        name: node.name.clone(),
        children: node
            .children
            .iter()
            .map(|&child| arena_node_to_render_model(arena, child))
            .collect(),
    }
}

fn flatten_nodes(node: &TreeViewNodeRenderModel, out: &mut Vec<Value>) {
    out.push(json!({
        "id": node.id,
        "level": node.level,
        "name": node.name,
    }));
    for child in &node.children {
        flatten_nodes(child, out);
    }
}

fn parse_node_line(line: &str, meta: &ParseMetadata) -> Result<FlatNode> {
    let indent = line
        .chars()
        .take_while(|ch| matches!(ch, ' ' | '\t'))
        .count();
    let rest = &line[indent..];
    let mut chars = rest.char_indices();
    let Some((_, quote @ ('"' | '\''))) = chars.next() else {
        return Err(parse_error(meta, "expected quoted tree node name"));
    };

    let mut end = None;
    for (idx, ch) in chars {
        if ch == quote {
            end = Some(idx);
            break;
        }
    }
    let Some(end_idx) = end else {
        return Err(parse_error(meta, "unterminated quoted tree node name"));
    };

    if !rest[end_idx + quote.len_utf8()..].trim().is_empty() {
        return Err(parse_error(meta, "unexpected tokens after tree node name"));
    }

    Ok(FlatNode {
        level: indent as i64,
        name: rest[quote.len_utf8()..end_idx].to_string(),
    })
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
            '%' if !in_single && !in_double => {
                if iter.peek().is_some_and(|(_, next)| *next == '%') {
                    return &line[..idx];
                }
            }
            _ => {}
        }
    }
    line
}

fn parse_error(meta: &ParseMetadata, message: impl Into<String>) -> Error {
    Error::DiagramParse {
        diagram_type: meta.diagram_type.clone(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MermaidConfig, ParseMetadata};

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
}
