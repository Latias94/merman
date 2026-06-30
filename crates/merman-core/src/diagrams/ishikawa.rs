use crate::diagrams::scan::strip_line_ending;
use crate::sanitize::sanitize_text;
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol, Error, ParseMetadata, Result, SourceSpan,
};
use serde_json::{Map, Value, json};

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

impl IshikawaDiagramRenderModel {
    pub(crate) fn sanitize_common_db_fields(&mut self, config: &crate::MermaidConfig) {
        crate::common_db::sanitize_optional_acc_title(&mut self.acc_title, config);
        crate::common_db::sanitize_optional_acc_descr(&mut self.acc_descr, config);
    }
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

#[derive(Debug, Clone)]
struct IshikawaNodeLine {
    text: String,
    span: SourceSpan,
    selection: SourceSpan,
}

pub fn parse_ishikawa_editor_facts(code: &str, meta: &ParseMetadata) -> EditorSemanticFacts {
    let mut facts = EditorSemanticFacts::new();
    let mut lines = code.split_inclusive('\n').peekable();
    let mut offset = 0usize;
    let mut header_seen = false;
    let mut emitted_root = false;

    while let Some(segment) = lines.next() {
        let line_start = offset;
        offset += segment.len();
        let line = strip_line_ending(segment);
        if is_space_or_comment_line(line) {
            continue;
        }

        if !header_seen {
            match parse_ishikawa_header_line(line, line_start, meta) {
                Ok(Some(root)) => {
                    header_seen = true;
                    push_ishikawa_node_fact(&mut facts, root, true);
                    emitted_root = true;
                }
                Ok(None) => {
                    header_seen = true;
                }
                Err(err) => {
                    facts.mark_recovered_with_diagnostic(
                        format!("ishikawa parser recovered after parse error: {err}"),
                        Some(SourceSpan::new(line_start, line_start + line.len())),
                    );
                    return facts;
                }
            }
            continue;
        }

        match parse_ishikawa_node_line(line, line_start, meta) {
            Ok(node) => {
                let is_root = !emitted_root;
                push_ishikawa_node_fact(&mut facts, node, is_root);
                emitted_root = true;
            }
            Err(err) => {
                facts.mark_recovered_with_diagnostic(
                    format!("ishikawa parser recovered after parse error: {err}"),
                    Some(SourceSpan::new(line_start, line_start + line.len())),
                );
                return facts;
            }
        }
    }

    facts
}

pub fn parse_ishikawa(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let model = parse_ishikawa_model_for_render(code, meta)?;
    let mut nodes = Vec::new();
    let root = if let Some(root) = &model.root {
        flatten_nodes(root, 0, &mut nodes);
        ishikawa_node_to_value(root)
    } else {
        Value::Null
    };

    let mut out = Map::new();
    out.insert("type".to_string(), Value::String(meta.diagram_type.clone()));
    out.insert(
        "title".to_string(),
        model.title.map(Value::String).unwrap_or(Value::Null),
    );
    out.insert(
        "accTitle".to_string(),
        model.acc_title.map(Value::String).unwrap_or(Value::Null),
    );
    out.insert(
        "accDescr".to_string(),
        model.acc_descr.map(Value::String).unwrap_or(Value::Null),
    );
    out.insert("root".to_string(), root);
    out.insert("nodes".to_string(), Value::Array(nodes));
    Ok(Value::Object(out))
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

fn parse_ishikawa_header_line(
    line: &str,
    line_start: usize,
    meta: &ParseMetadata,
) -> Result<Option<IshikawaNodeLine>> {
    let trimmed_start = line.len().saturating_sub(line.trim_start().len());
    let trimmed = &line[trimmed_start..];
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
        let text = rest.trim();
        if text.is_empty() {
            return Ok(None);
        }
        let rel = rest.find(text).unwrap_or(0);
        let start = line_start + trimmed_start + header.len() + rel;
        let end = start + text.len();
        return Ok(Some(IshikawaNodeLine {
            text: text.to_string(),
            span: SourceSpan::new(line_start, line_start + line.len()),
            selection: SourceSpan::new(start, end),
        }));
    }

    Err(parse_error(meta, "expected ishikawa"))
}

fn parse_ishikawa_node_line(
    line: &str,
    line_start: usize,
    meta: &ParseMetadata,
) -> Result<IshikawaNodeLine> {
    let indent = line
        .chars()
        .take_while(|ch| matches!(ch, ' ' | '\t'))
        .count();
    let body = &line[indent..];
    let text = body.trim();
    if text.is_empty() {
        return Err(parse_error(meta, "expected ishikawa node"));
    }

    let rel = body.find(text).unwrap_or(0);
    let start = line_start + indent + rel;
    let end = start + text.len();
    Ok(IshikawaNodeLine {
        text: text.to_string(),
        span: SourceSpan::new(line_start, line_start + line.len()),
        selection: SourceSpan::new(start, end),
    })
}

fn push_ishikawa_node_fact(facts: &mut EditorSemanticFacts, node: IshikawaNodeLine, is_root: bool) {
    let detail = if is_root {
        "ishikawa effect"
    } else {
        "ishikawa cause"
    };
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::NodeIdentifier,
        node.selection,
    ));
    facts.push_symbol(EditorSemanticSymbol::new(
        node.text,
        Some(detail.to_string()),
        EditorSemanticKind::Namespace,
        node.span,
        node.selection,
    ));
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
    if idx >= arena.len() {
        return IshikawaNodeRenderModel::default();
    }

    let mut stack = vec![(idx, false)];
    let mut completed: Vec<Option<IshikawaNodeRenderModel>> =
        (0..arena.len()).map(|_| None).collect();

    while let Some((node_idx, visited)) = stack.pop() {
        let Some(node) = arena.get(node_idx) else {
            continue;
        };

        if visited {
            let children = node
                .children
                .iter()
                .filter_map(|&child_idx| completed.get_mut(child_idx).and_then(Option::take))
                .collect();
            completed[node_idx] = Some(IshikawaNodeRenderModel {
                text: node.text.clone(),
                children,
            });
        } else {
            stack.push((node_idx, true));
            for &child_idx in node.children.iter().rev() {
                stack.push((child_idx, false));
            }
        }
    }

    completed
        .get_mut(idx)
        .and_then(Option::take)
        .unwrap_or_default()
}

fn flatten_nodes(node: &IshikawaNodeRenderModel, depth: usize, out: &mut Vec<Value>) {
    let mut stack = vec![(node, depth)];
    while let Some((node, depth)) = stack.pop() {
        out.push(json!({
            "text": node.text,
            "depth": depth,
        }));
        for child in node.children.iter().rev() {
            stack.push((child, depth + 1));
        }
    }
}

fn ishikawa_node_to_value(node: &IshikawaNodeRenderModel) -> Value {
    let mut stack = vec![(node, false)];
    let mut completed: std::collections::HashMap<*const IshikawaNodeRenderModel, Value> =
        std::collections::HashMap::new();

    while let Some((node, visited)) = stack.pop() {
        if visited {
            let children = node
                .children
                .iter()
                .filter_map(|child| completed.remove(&(child as *const IshikawaNodeRenderModel)))
                .collect();
            let mut obj = Map::new();
            obj.insert("text".to_string(), Value::String(node.text.clone()));
            obj.insert("children".to_string(), Value::Array(children));
            completed.insert(node as *const IshikawaNodeRenderModel, Value::Object(obj));
        } else {
            stack.push((node, true));
            for child in node.children.iter().rev() {
                stack.push((child, false));
            }
        }
    }

    completed
        .remove(&(node as *const IshikawaNodeRenderModel))
        .unwrap_or(Value::Null)
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
    Error::diagram_parse_fallback(meta.diagram_type.clone(), message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EditorExpectedSyntaxKind, EditorSemanticCompleteness, EditorSemanticKind,
        EditorSemanticRole, Engine, MermaidConfig, ParseMetadata, ParseOptions, SourceSpan,
    };

    const DEEP_ISHIKAWA_DEPTH: usize = 1_500;

    fn meta() -> ParseMetadata {
        ParseMetadata {
            diagram_type: "ishikawa".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: MermaidConfig::empty_object(),
            title: None,
        }
    }

    fn deep_ishikawa_source(depth: usize) -> String {
        let mut source = String::from("ishikawa-beta\n  Root\n");
        for i in 0..depth {
            source.push_str(&" ".repeat((i + 2) * 2));
            source.push_str(&format!("Node {i}\n"));
        }
        source
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

    #[test]
    fn parses_deep_hierarchy_without_recursive_stack_growth() {
        let source = deep_ishikawa_source(DEEP_ISHIKAWA_DEPTH);
        let model = parse_ishikawa_model_for_render(&source, &meta()).unwrap();
        let root = model.root.as_ref().unwrap();

        assert_eq!(root.text, "Root");
        let mut node = root;
        for i in 0..DEEP_ISHIKAWA_DEPTH {
            node = &node.children[0];
            assert_eq!(node.text, format!("Node {i}"));
        }
        assert!(node.children.is_empty());

        let semantic = parse_ishikawa(&source, &meta()).unwrap();
        assert_eq!(
            semantic["nodes"].as_array().unwrap().len(),
            DEEP_ISHIKAWA_DEPTH + 1
        );
        assert_eq!(
            semantic["nodes"][DEEP_ISHIKAWA_DEPTH]["depth"].as_u64(),
            Some(DEEP_ISHIKAWA_DEPTH as u64)
        );
        assert_eq!(
            semantic["root"]["children"][0]["children"][0]["text"].as_str(),
            Some("Node 1")
        );
    }

    #[test]
    fn parse_ishikawa_editor_facts_expose_parser_backed_spans() {
        let engine = Engine::new();
        let text = r#"ishikawa-beta
    Problem
Cause A
  Subcause A1
"#;
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("ishikawa", text, ParseOptions::strict())
            .unwrap()
            .expect("ishikawa editor facts");

        assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);

        for name in ["Problem", "Cause A", "Subcause A1"] {
            let start = text.find(name).unwrap();
            assert!(
                facts.expected_syntax.iter().any(|expected| {
                    expected.kind == EditorExpectedSyntaxKind::NodeIdentifier
                        && expected.span == SourceSpan::new(start, start + name.len())
                }),
                "missing expected syntax for {name}"
            );
        }

        let effect = facts
            .symbols
            .iter()
            .find(|symbol| symbol.name == "Problem")
            .expect("missing ishikawa effect");
        assert_eq!(effect.detail.as_deref(), Some("ishikawa effect"));
        assert_eq!(effect.role, EditorSemanticRole::Entity);
        assert_eq!(effect.kind, EditorSemanticKind::Namespace);
        let effect_start = text.find("Problem").unwrap();
        assert_eq!(
            effect.selection,
            SourceSpan::new(effect_start, effect_start + "Problem".len())
        );

        let cause = facts
            .symbols
            .iter()
            .find(|symbol| symbol.name == "Cause A")
            .expect("missing ishikawa cause");
        assert_eq!(cause.detail.as_deref(), Some("ishikawa cause"));
        let cause_start = text.find("Cause A").unwrap();
        assert_eq!(
            cause.selection,
            SourceSpan::new(cause_start, cause_start + "Cause A".len())
        );
    }

    #[test]
    fn parse_ishikawa_editor_facts_support_inline_root() {
        let engine = Engine::new();
        let text = "ishikawa Problem\n  Cause\n";
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("ishikawa", text, ParseOptions::strict())
            .unwrap()
            .expect("ishikawa editor facts");

        let effect = facts
            .symbols
            .iter()
            .find(|symbol| symbol.name == "Problem")
            .expect("missing inline root");
        assert_eq!(effect.detail.as_deref(), Some("ishikawa effect"));
        let effect_start = text.find("Problem").unwrap();
        assert_eq!(
            effect.selection,
            SourceSpan::new(effect_start, effect_start + "Problem".len())
        );
    }
}
