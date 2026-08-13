use crate::diagrams::scan::strip_line_ending;
use crate::sanitize::sanitize_text;
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorLexemeKind, EditorLexemeModifiers,
    EditorSemanticFacts, EditorSemanticKind, EditorSemanticSymbol, Error, ParseMetadata, Result,
    SourceSpan, editor::EditorLexemeJournal,
};
use serde_json::{Map, Value, json};
#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    static ISHIKAWA_SYNTAX_CONSTRUCTION_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_ishikawa_syntax_construction_count() {
    ISHIKAWA_SYNTAX_CONSTRUCTION_COUNT.set(0);
}

#[cfg(test)]
pub(crate) fn ishikawa_syntax_construction_count() -> usize {
    ISHIKAWA_SYNTAX_CONSTRUCTION_COUNT.get()
}

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
    span: SourceSpan,
    selection: SourceSpan,
}

#[derive(Debug, Clone)]
struct ArenaNode {
    text: String,
    children: Vec<usize>,
}

struct IshikawaSemanticSource {
    nodes: Vec<FlatNode>,
    editor_facts: EditorSemanticFacts,
}

struct IshikawaParseFailure {
    error: Box<Error>,
    editor_facts: Box<EditorSemanticFacts>,
}

impl IshikawaSemanticSource {
    fn editor_facts(&self) -> EditorSemanticFacts {
        self.editor_facts.clone()
    }

    fn into_render_model(mut self, meta: &ParseMetadata) -> IshikawaDiagramRenderModel {
        for node in &mut self.nodes {
            node.text = sanitize_text(&node.text, &meta.effective_config);
        }
        nodes_to_render_model(self.nodes)
    }

    fn into_compat_json(self, meta: &ParseMetadata) -> Result<Value> {
        let model = self.into_render_model(meta);
        render_model_to_compat_json(&model, meta)
    }
}

pub(crate) fn parse_ishikawa(code: &str, meta: &ParseMetadata) -> Result<Value> {
    construct_ishikawa_semantic_source(code, meta)
        .map_err(|failure| *failure.error)?
        .into_compat_json(meta)
}

pub(crate) fn parse_ishikawa_json_and_editor_facts(
    code: &str,
    meta: &ParseMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<crate::family::CombinedSemanticParse> {
    let construction = construct_ishikawa_semantic_source_controlled(code, meta, control)?;
    let parsed = crate::family::CombinedSemanticParse::from_construction(
        construction,
        |source| {
            let editor_facts = source.editor_facts();
            (source.into_compat_json(meta), editor_facts)
        },
        IshikawaParseFailure::into_error_and_editor_facts,
    );
    control.checkpoint()?;
    Ok(parsed)
}

pub(crate) fn render_model_to_compat_json(
    model: &IshikawaDiagramRenderModel,
    meta: &ParseMetadata,
) -> Result<Value> {
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
        model
            .title
            .as_ref()
            .cloned()
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    out.insert(
        "accTitle".to_string(),
        model
            .acc_title
            .as_ref()
            .cloned()
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    out.insert(
        "accDescr".to_string(),
        model
            .acc_descr
            .as_ref()
            .cloned()
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    out.insert("root".to_string(), root);
    out.insert("nodes".to_string(), Value::Array(nodes));
    Ok(Value::Object(out))
}

pub(crate) fn parse_ishikawa_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<IshikawaDiagramRenderModel> {
    Ok(construct_ishikawa_semantic_source(code, meta)
        .map_err(|failure| *failure.error)?
        .into_render_model(meta))
}

impl IshikawaParseFailure {
    fn into_error_and_editor_facts(self) -> (Error, EditorSemanticFacts) {
        (*self.error, *self.editor_facts)
    }
}

struct IshikawaHeader {
    keyword_span: SourceSpan,
    root: Option<FlatNode>,
}

fn construct_ishikawa_semantic_source(
    code: &str,
    meta: &ParseMetadata,
) -> std::result::Result<IshikawaSemanticSource, IshikawaParseFailure> {
    construct_ishikawa_semantic_source_controlled(code, meta, &crate::OperationControl::new())
        .expect("a private parse control cannot be cancelled")
}

fn construct_ishikawa_semantic_source_controlled(
    code: &str,
    meta: &ParseMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<std::result::Result<IshikawaSemanticSource, IshikawaParseFailure>>
{
    control.checkpoint()?;
    #[cfg(test)]
    ISHIKAWA_SYNTAX_CONSTRUCTION_COUNT.set(ISHIKAWA_SYNTAX_CONSTRUCTION_COUNT.get() + 1);

    let mut nodes = Vec::new();
    let mut lexemes = EditorLexemeJournal::family_parser(code);
    let mut offset = 0usize;
    let mut header_seen = false;
    let mut first_error = None;

    for segment in code.split_inclusive('\n') {
        control.checkpoint()?;
        let line_start = offset;
        offset += segment.len();
        let line = strip_line_ending(segment);
        if is_space_or_comment_line(line) {
            continue;
        }

        if !header_seen {
            match parse_ishikawa_header_line(line, line_start, meta) {
                Ok(header) => {
                    header_seen = true;
                    lexemes.push(
                        EditorLexemeKind::Keyword,
                        EditorLexemeModifiers::NONE,
                        header.keyword_span,
                    );
                    if let Some(root) = header.root {
                        lexemes.push(
                            EditorLexemeKind::String,
                            EditorLexemeModifiers::NONE,
                            root.selection,
                        );
                        nodes.push(root);
                    }
                }
                Err(error) => {
                    let span = SourceSpan::new(line_start, line_start + line.len());
                    lexemes.push(EditorLexemeKind::Literal, EditorLexemeModifiers::NONE, span);
                    first_error.get_or_insert((error, span));
                }
            }
            continue;
        }

        match parse_ishikawa_node_line(line, line_start, meta) {
            Ok(node) => {
                lexemes.push(
                    EditorLexemeKind::String,
                    EditorLexemeModifiers::NONE,
                    node.selection,
                );
                nodes.push(node);
            }
            Err(error) => {
                let span = SourceSpan::new(line_start, line_start + line.len());
                lexemes.push(EditorLexemeKind::Literal, EditorLexemeModifiers::NONE, span);
                first_error.get_or_insert((error, span));
            }
        }
    }

    if !header_seen {
        first_error.get_or_insert((
            Error::diagram_parse_insertion_point(meta.diagram_type.clone(), "expected ishikawa", 0),
            SourceSpan::new(0, 0),
        ));
    }

    let mut editor_facts = EditorSemanticFacts::new();
    for (index, node) in nodes.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        push_ishikawa_node_fact(&mut editor_facts, node, index == 0);
    }
    if let Some((error, span)) = first_error {
        editor_facts.mark_recovered_from_parse_error(
            format!("ishikawa parser recovered after parse error: {error}"),
            Some(span),
        );
        editor_facts.replace_family_lexemes(lexemes.finish());
        return Ok(Err(IshikawaParseFailure {
            error: Box::new(error),
            editor_facts: Box::new(editor_facts),
        }));
    }
    editor_facts.replace_family_lexemes(lexemes.finish());
    control.checkpoint()?;
    Ok(Ok(IshikawaSemanticSource {
        nodes,
        editor_facts,
    }))
}

fn parse_ishikawa_header_line(
    line: &str,
    line_start: usize,
    meta: &ParseMetadata,
) -> Result<IshikawaHeader> {
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
        let keyword_span = SourceSpan::new(
            line_start + trimmed_start,
            line_start + trimmed_start + header.len(),
        );
        if text.is_empty() {
            return Ok(IshikawaHeader {
                keyword_span,
                root: None,
            });
        }
        let rel = rest.len().saturating_sub(rest.trim_start().len());
        let start = line_start + trimmed_start + header.len() + rel;
        let end = start + text.len();
        return Ok(IshikawaHeader {
            keyword_span,
            root: Some(FlatNode {
                raw_level: 0,
                text: text.to_string(),
                span: SourceSpan::new(line_start, line_start + line.len()),
                selection: SourceSpan::new(start, end),
            }),
        });
    }

    Err(Error::diagram_parse_exact(
        meta.diagram_type.clone(),
        "expected ishikawa",
        SourceSpan::new(line_start, line_start + line.len()),
    ))
}

fn parse_ishikawa_node_line(
    line: &str,
    line_start: usize,
    meta: &ParseMetadata,
) -> Result<FlatNode> {
    let indent = line
        .chars()
        .take_while(|ch| matches!(ch, ' ' | '\t'))
        .count();
    let body = &line[indent..];
    let text = body.trim();
    if text.is_empty() {
        return Err(Error::diagram_parse_exact(
            meta.diagram_type.clone(),
            "expected ishikawa node",
            SourceSpan::new(line_start, line_start + line.len()),
        ));
    }

    let rel = body.len().saturating_sub(body.trim_start().len());
    let start = line_start + indent + rel;
    let end = start + text.len();
    Ok(FlatNode {
        raw_level: indent,
        text: text.to_string(),
        span: SourceSpan::new(line_start, line_start + line.len()),
        selection: SourceSpan::new(start, end),
    })
}

fn push_ishikawa_node_fact(facts: &mut EditorSemanticFacts, node: &FlatNode, is_root: bool) {
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
        node.text.clone(),
        Some(detail.to_string()),
        EditorSemanticKind::Namespace,
        node.span,
        node.selection,
    ));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EditorExpectedSyntaxKind, EditorLexemeProducerKind, EditorSemanticCompleteness,
        EditorSemanticKind, EditorSemanticRole, Engine, MermaidConfig, ParseDiagnosticSpanKind,
        ParseMetadata, SourceSpan,
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
    fn controlled_parse_can_cancel_between_ishikawa_lines() {
        let control = crate::OperationControl::new();
        control.cancel_after_checkpoints(2);

        assert!(matches!(
            construct_ishikawa_semantic_source_controlled(
                "ishikawa-beta Problem\n  Cause A\n  Cause B\n",
                &meta(),
                &control,
            ),
            Err(crate::OperationCancelled { .. })
        ));
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
    fn combined_parse_constructs_syntax_once_and_preserves_all_projections() {
        let text = "ishikawa-beta Problem\r\n  Cause A\r\n    Cause A1\r\n  Cause B\r\n";
        let expected_json = parse_ishikawa(text, &meta()).unwrap();
        let expected_model = parse_ishikawa_model_for_render(text, &meta()).unwrap();

        reset_ishikawa_syntax_construction_count();
        let (json, facts) = crate::family::test_support::into_result(
            parse_ishikawa_json_and_editor_facts(text, &meta(), &crate::OperationControl::new()),
        )
        .unwrap();

        assert_eq!(ishikawa_syntax_construction_count(), 1);
        assert_eq!(json, expected_json);
        assert_eq!(
            render_model_to_compat_json(&expected_model, &meta()).unwrap(),
            expected_json
        );
        assert_eq!(
            json["root"],
            serde_json::to_value(&expected_model.root).unwrap()
        );
        assert_eq!(json["title"].as_str(), expected_model.title.as_deref());

        for name in ["Problem", "Cause A", "Cause A1", "Cause B"] {
            let start = text.find(name).unwrap();
            assert!(facts.symbols.iter().any(|symbol| {
                symbol.name == name
                    && symbol.selection == SourceSpan::new(start, start + name.len())
            }));
        }
    }

    #[test]
    fn editor_recovery_reports_invalid_or_incomplete_headers() {
        for (text, span, span_kind) in [
            (
                "not-ishikawa\n  Cause\n",
                SourceSpan::new(0, 12),
                ParseDiagnosticSpanKind::Exact,
            ),
            (
                "",
                SourceSpan::new(0, 0),
                ParseDiagnosticSpanKind::InsertionPoint,
            ),
        ] {
            let Error::DiagramParse { diagnostic, .. } = parse_ishikawa(text, &meta()).unwrap_err()
            else {
                panic!("expected ishikawa parse error");
            };
            assert_eq!(diagnostic.span(), Some(span));
            assert_eq!(diagnostic.span_kind(), span_kind);

            reset_ishikawa_syntax_construction_count();
            let facts = crate::family::test_support::editor_facts(
                parse_ishikawa_json_and_editor_facts,
                text,
                &meta(),
            );
            assert_eq!(ishikawa_syntax_construction_count(), 1);
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            assert_eq!(facts.diagnostics.len(), 1);
            assert_eq!(facts.diagnostics[0].span, Some(span));
        }
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
            .parse_editor_semantic_facts_with_type_sync("ishikawa", text)
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
            .parse_editor_semantic_facts_with_type_sync("ishikawa", text)
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

    #[test]
    fn ishikawa_line_parser_emits_crlf_and_unicode_lexemes() {
        let source = "  ishikawa-beta 主要问题\r\n    原因一\r\n\t子原因\r\n";
        let facts = Engine::new()
            .parse_editor_semantic_facts_with_type_sync("ishikawa", source)
            .unwrap()
            .expect("ishikawa facts");

        assert_eq!(facts.lexeme_failure(), None);
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
        assert!(facts.lexemes().iter().all(|lexeme| {
            lexeme.producer().kind() == EditorLexemeProducerKind::FamilyParser
                && lexeme.producer().family().map(|family| family.as_str()) == Some("ishikawa")
        }));
        for (kind, text) in [
            (EditorLexemeKind::Keyword, "ishikawa-beta"),
            (EditorLexemeKind::String, "主要问题"),
            (EditorLexemeKind::String, "原因一"),
            (EditorLexemeKind::String, "子原因"),
        ] {
            assert!(facts.lexemes().iter().any(|lexeme| {
                let span = lexeme.span();
                lexeme.kind() == kind && &source[span.start..span.end] == text
            }));
        }
    }

    #[test]
    fn ishikawa_recovery_keeps_lexemes_after_an_invalid_leading_statement() {
        let source = "not-ishikawa\r\nishikawa-beta Problem\r\n  Later cause\r\n";
        let facts = Engine::new()
            .parse_editor_semantic_facts_with_type_sync("ishikawa", source)
            .unwrap()
            .expect("ishikawa recovery facts");

        assert_eq!(facts.lexeme_failure(), None);
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(facts.lexemes().iter().all(|lexeme| {
            lexeme.producer().kind() == EditorLexemeProducerKind::FamilyRecovery
                && lexeme.producer().family().map(|family| family.as_str()) == Some("ishikawa")
        }));
        for (kind, text) in [
            (EditorLexemeKind::Literal, "not-ishikawa"),
            (EditorLexemeKind::Keyword, "ishikawa-beta"),
            (EditorLexemeKind::String, "Problem"),
            (EditorLexemeKind::String, "Later cause"),
        ] {
            assert!(facts.lexemes().iter().any(|lexeme| {
                let span = lexeme.span();
                lexeme.kind() == kind && &source[span.start..span.end] == text
            }));
        }
    }
}
