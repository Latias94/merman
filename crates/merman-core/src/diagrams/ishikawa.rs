use crate::sanitize::sanitize_text;
use crate::{Error, ParseMetadata, Result};
use serde_json::{Value, json};

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct IshikawaNodeRenderModel {
    pub text: String,
    #[serde(default)]
    pub children: Vec<IshikawaNodeRenderModel>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct IshikawaDiagramRenderModel {
    #[serde(default, rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(default, rename = "accDescr")]
    pub acc_descr: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    pub root: Option<IshikawaNodeRenderModel>,
}

#[derive(Debug, Clone)]
struct FlatNode {
    raw_level: usize,
    text: String,
}

#[derive(Debug, Clone)]
struct ArenaNode {
    text: String,
    children: Vec<usize>,
}

pub fn parse_ishikawa(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let model = parse_ishikawa_model_for_render(code, meta)?;
    let mut nodes = Vec::new();
    if let Some(root) = &model.root {
        flatten_nodes(root, 0, &mut nodes);
    }

    Ok(json!({
        "type": meta.diagram_type,
        "title": model.title,
        "accTitle": model.acc_title,
        "accDescr": model.acc_descr,
        "root": model.root,
        "nodes": nodes,
    }))
}

pub fn parse_ishikawa_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<IshikawaDiagramRenderModel> {
    let nodes = parse_ishikawa_nodes(code, meta)?;
    Ok(nodes_to_render_model(nodes))
}

fn parse_ishikawa_nodes(code: &str, meta: &ParseMetadata) -> Result<Vec<FlatNode>> {
    let mut lines = code.lines();
    let trailing_root = loop {
        let Some(line) = lines.next() else {
            return Err(parse_error(meta, "expected ishikawa"));
        };
        if is_space_or_comment_line(line) {
            continue;
        }
        break parse_header(line, meta)?;
    };

    let mut nodes = Vec::new();
    if let Some(text) = trailing_root {
        nodes.push(FlatNode {
            raw_level: 0,
            text: sanitize_text(&text, &meta.effective_config),
        });
    }

    for raw in lines {
        let raw = raw.trim_end_matches('\r');
        if is_space_or_comment_line(raw) {
            continue;
        }
        let indent = raw
            .chars()
            .take_while(|ch| matches!(ch, ' ' | '\t'))
            .count();
        let text = raw[indent..].trim().to_string();
        if text.is_empty() {
            continue;
        }
        nodes.push(FlatNode {
            raw_level: indent,
            text: sanitize_text(&text, &meta.effective_config),
        });
    }

    Ok(nodes)
}

fn parse_header(line: &str, meta: &ParseMetadata) -> Result<Option<String>> {
    let trimmed = line.trim_start();
    for header in ["ishikawa-beta", "ishikawa"] {
        if !starts_with_ignore_ascii_case(trimmed, header) {
            continue;
        }
        let rest = &trimmed[header.len()..];
        if rest
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        {
            continue;
        }
        let trailing = rest.trim();
        return Ok((!trailing.is_empty()).then(|| trailing.to_string()));
    }

    Err(parse_error(meta, "expected ishikawa"))
}

fn nodes_to_render_model(nodes: Vec<FlatNode>) -> IshikawaDiagramRenderModel {
    let mut iter = nodes.into_iter();
    let Some(root) = iter.next() else {
        return IshikawaDiagramRenderModel::default();
    };

    let mut arena = vec![ArenaNode {
        text: root.text,
        children: Vec::new(),
    }];
    let mut stack = vec![(0usize, 0usize)];
    let mut base_level = None;

    for flat in iter {
        let base = *base_level.get_or_insert(flat.raw_level);
        let mut level = flat.raw_level.saturating_sub(base) + 1;
        if level == 0 {
            level = 1;
        }

        while stack.len() > 1
            && stack
                .last()
                .is_some_and(|(_, top_level)| *top_level >= level)
        {
            stack.pop();
        }

        let parent = stack.last().map(|(idx, _)| *idx).unwrap_or(0);
        let idx = arena.len();
        arena.push(ArenaNode {
            text: flat.text,
            children: Vec::new(),
        });
        arena[parent].children.push(idx);
        stack.push((idx, level));
    }

    let root = arena_node_to_render_model(&arena, 0);
    IshikawaDiagramRenderModel {
        title: Some(root.text.clone()),
        root: Some(root),
        ..Default::default()
    }
}

fn arena_node_to_render_model(arena: &[ArenaNode], idx: usize) -> IshikawaNodeRenderModel {
    let node = &arena[idx];
    IshikawaNodeRenderModel {
        text: node.text.clone(),
        children: node
            .children
            .iter()
            .map(|&child| arena_node_to_render_model(arena, child))
            .collect(),
    }
}

fn flatten_nodes(node: &IshikawaNodeRenderModel, depth: usize, out: &mut Vec<Value>) {
    out.push(json!({
        "text": node.text,
        "depth": depth,
    }));
    for child in &node.children {
        flatten_nodes(child, depth + 1, out);
    }
}

fn is_space_or_comment_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty() || trimmed.starts_with("%%")
}

fn starts_with_ignore_ascii_case(value: &str, prefix: &str) -> bool {
    value
        .get(..prefix.len())
        .is_some_and(|actual| actual.eq_ignore_ascii_case(prefix))
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
            diagram_type: "ishikawa".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: MermaidConfig::empty_object(),
            title: None,
        }
    }

    #[test]
    fn parses_basic_ishikawa_hierarchy() {
        let model = parse_ishikawa_model_for_render(
            r#"ishikawa-beta
    Blurry Photo
        Process
            Out of focus
        User
            Shaky hands
"#,
            &meta(),
        )
        .unwrap();

        let root = model.root.unwrap();
        assert_eq!(root.text, "Blurry Photo");
        assert_eq!(model.title.as_deref(), Some("Blurry Photo"));
        assert_eq!(root.children.len(), 2);
        assert_eq!(root.children[0].text, "Process");
        assert_eq!(root.children[0].children[0].text, "Out of focus");
        assert_eq!(root.children[1].text, "User");
        assert_eq!(root.children[1].children[0].text, "Shaky hands");
    }

    #[test]
    fn handles_effect_indented_more_than_causes() {
        let model = parse_ishikawa_model_for_render(
            r#"ishikawa-beta
    Problem
Cause A
  Subcause A1
Cause B
"#,
            &meta(),
        )
        .unwrap();

        let root = model.root.unwrap();
        assert_eq!(root.text, "Problem");
        assert_eq!(root.children.len(), 2);
        assert_eq!(root.children[0].text, "Cause A");
        assert_eq!(root.children[0].children[0].text, "Subcause A1");
        assert_eq!(root.children[1].text, "Cause B");
    }

    #[test]
    fn detects_plain_header_and_inline_root() {
        let model = parse_ishikawa_model_for_render("ishikawa Problem\n  Cause", &meta()).unwrap();

        let root = model.root.unwrap();
        assert_eq!(root.text, "Problem");
        assert_eq!(root.children[0].text, "Cause");
    }
}
