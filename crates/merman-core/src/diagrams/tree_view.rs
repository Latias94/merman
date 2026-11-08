#[cfg(feature = "presentation")]
use crate::SourcePresentationMetadata;
#[cfg(feature = "editor-facts")]
use crate::diagrams::langium_common::push_langium_common_editor_fact;
use crate::diagrams::langium_common::{LangiumCommonField, parse_langium_common};
use crate::diagrams::scan::{physical_line_at, split_ascii_indent};
#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol,
};
use crate::{Error, MAX_DIAGRAM_NESTING_DEPTH, Result, SourceSpan};
use merman_model::families::tree_view::{TREE_VIEW_DIRECTORY_NODE_TYPE, TREE_VIEW_FILE_NODE_TYPE};
pub(crate) use merman_model::families::tree_view::{TreeViewDiagramModel, TreeViewNode};
#[cfg(feature = "compat-json")]
use serde_json::Value;
#[cfg(all(test, feature = "compat-json"))]
use serde_json::json;

#[cfg(test)]
thread_local! {
    static TREE_VIEW_BODY_LINE_SCAN_BYTES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(crate) fn sanitize_common_db_fields(
    model: &mut TreeViewDiagramModel,
    config: &crate::MermaidConfig,
) {
    crate::common_db::sanitize_optional_title(&mut model.title, config);
    crate::common_db::sanitize_optional_acc_title(&mut model.acc_title, config);
    crate::common_db::sanitize_optional_acc_descr(&mut model.acc_descr, config);
}

#[derive(Debug, Clone)]
struct ParsedTreeViewInput {
    title: Option<String>,
    acc_title: Option<String>,
    acc_descr: Option<String>,
    nodes: Vec<TreeViewNodeDetails>,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<EditorSemanticFacts>,
}

#[derive(Debug, Clone)]
struct TreeViewParseIssue {
    message: String,
    span: Option<SourceSpan>,
}

struct TreeViewParseOutcome {
    snapshot: ParsedTreeViewInput,
    first_issue: Option<TreeViewParseIssue>,
}

impl TreeViewParseOutcome {
    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    fn into_combined(
        self,
        meta: &SourcePresentationMetadata,
    ) -> crate::family::CombinedSemanticParse {
        let Self {
            snapshot,
            first_issue,
        } = self;
        let construction = match first_issue {
            Some(issue) => Err(crate::family::CombinedSemanticFailure::new(
                issue.into_error(&meta.diagram_type),
                Some(
                    snapshot
                        .editor_facts
                        .expect("combined parsing requests editor facts"),
                ),
            )),
            None => Ok(snapshot),
        };
        crate::family::CombinedSemanticParse::from_construction(
            construction,
            |snapshot| {
                let ParsedTreeViewInput {
                    title,
                    acc_title,
                    acc_descr,
                    nodes,
                    editor_facts,
                } = snapshot;
                let mut editor_facts =
                    editor_facts.expect("combined parsing requests editor facts");
                let model = tree_view_input_to_model(
                    title,
                    acc_title,
                    acc_descr,
                    nodes,
                    meta.effective_config.as_config(),
                )
                .map_err(|message| {
                    editor_facts.mark_recovered_from_parse_error(message.clone(), None);
                    Error::diagram_parse_fallback(meta.diagram_type.clone(), message)
                })
                .and_then(|model| render_model_to_compat_json(&model, meta));
                (model, editor_facts)
            },
            crate::family::CombinedSemanticFailure::into_parts,
        )
    }
}

impl TreeViewParseIssue {
    fn new(message: impl Into<String>, span: Option<SourceSpan>) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }

    fn into_error(self, diagram_type: &str) -> Error {
        match self.span {
            Some(span) => Error::diagram_parse_exact(diagram_type.to_string(), &self.message, span),
            None => Error::diagram_parse_fallback(diagram_type.to_string(), &self.message),
        }
    }
}

#[derive(Debug, Clone)]
struct TreeViewNodeDetails {
    indent: usize,
    name: String,
    node_type: String,
    css_class: Option<TreeViewSpannedValue>,
    icon: Option<TreeViewSpannedValue>,
    description: Option<TreeViewSpannedValue>,
    #[cfg(feature = "editor-facts")]
    span: SourceSpan,
    #[cfg(feature = "editor-facts")]
    selection: SourceSpan,
}

#[derive(Debug, Clone)]
struct TreeViewSpannedValue {
    value: String,
    #[cfg(feature = "editor-facts")]
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
    expand_content_tabs: bool,
}

struct TreeViewSourceSegment {
    text: SourceSpan,
    source: SourceSpan,
}

/// Family preprocessing keeps token boundaries tied to the original box-drawing/CRLF input.
struct TreeViewSource {
    text: String,
    segments: Vec<TreeViewSourceSegment>,
    source_end: usize,
}

impl TreeViewSource {
    fn push(&mut self, text: &str, source: SourceSpan) {
        if text.is_empty() {
            return;
        }
        let start = self.text.len();
        self.text.push_str(text);
        if let Some(last) = self.segments.last_mut()
            && last.source.end == source.start
            && last.text.end - last.text.start == last.source.end - last.source.start
            && text.len() == source.end - source.start
        {
            last.text.end = self.text.len();
            last.source.end = source.end;
        } else {
            self.segments.push(TreeViewSourceSegment {
                text: SourceSpan::new(start, self.text.len()),
                source,
            });
        }
    }

    fn source_offset(&self, offset: usize, end: bool) -> usize {
        // EOF insertions use the original EOF; token ends use the last retained segment.
        if offset == self.text.len() && !end {
            return self.source_end;
        }
        let index = if end {
            self.segments
                .partition_point(|segment| segment.text.start < offset)
                .saturating_sub(1)
        } else {
            self.segments
                .partition_point(|segment| segment.text.end <= offset)
        };
        let Some(segment) = self.segments.get(index) else {
            return self.source_end;
        };
        if offset <= segment.text.start {
            segment.source.start
        } else if offset >= segment.text.end {
            segment.source.end
        } else if segment.text.end - segment.text.start == segment.source.end - segment.source.start
        {
            segment.source.start + offset - segment.text.start
        } else if end {
            segment.source.end
        } else {
            segment.source.start
        }
    }

    fn source_span(&self, span: SourceSpan) -> SourceSpan {
        let start = self.source_offset(span.start, false);
        let end = if span.start == span.end {
            start
        } else {
            self.source_offset(span.end, true)
        };
        SourceSpan::new(start, end)
    }
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

#[cfg(feature = "compat-json")]
pub(crate) fn parse_tree_view(code: &str, meta: &SourcePresentationMetadata) -> Result<Value> {
    let model =
        parse_tree_view_model_for_render_controlled(code, meta, &crate::OperationControl::new())
            .expect("a private parse control cannot be cancelled")?;
    render_model_to_compat_json(&model, meta)
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub(crate) fn parse_tree_view_json_and_editor_facts(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<crate::family::CombinedSemanticParse> {
    let parsed = parse_tree_view_input_controlled(
        code,
        &meta.diagram_type,
        EditorFactsDemand::Capture,
        control,
    )?
    .into_combined(meta);
    control.checkpoint()?;
    Ok(parsed)
}

#[cfg(all(test, feature = "presentation"))]
pub(crate) fn parse_tree_view_model_for_render(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<TreeViewDiagramModel> {
    parse_tree_view_model_for_render_controlled(code, meta, &crate::OperationControl::new())
        .expect("a private parse control cannot be cancelled")
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_tree_view_model_for_render_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<TreeViewDiagramModel>> {
    Ok(construct_tree_view_model_controlled(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )?
    .model)
}

pub(crate) fn parse_tree_view_semantic_model_controlled(
    code: &str,
    meta: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<crate::family::SemanticConstruction<TreeViewDiagramModel>> {
    Ok(construct_tree_view_model_controlled(
        code,
        &meta.diagram_type,
        &meta.semantic_config,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?
    .map_model(|mut model| {
        sanitize_common_db_fields(&mut model, &meta.semantic_config);
        model
    }))
}

fn construct_tree_view_model_controlled(
    code: &str,
    diagram_type: &str,
    config: &crate::MermaidConfig,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<crate::family::SemanticConstruction<TreeViewDiagramModel>> {
    let TreeViewParseOutcome {
        snapshot,
        first_issue,
    } = parse_tree_view_input_controlled(
        code,
        diagram_type,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?;
    let ParsedTreeViewInput {
        title,
        acc_title,
        acc_descr,
        nodes,
        #[cfg(feature = "editor-facts")]
        mut editor_facts,
    } = snapshot;
    if let Some(issue) = first_issue {
        return Ok(crate::family::SemanticConstruction {
            model: Err(issue.into_error(diagram_type)),
            #[cfg(feature = "editor-facts")]
            editor_facts,
        });
    }
    let model =
        tree_view_input_to_model(title, acc_title, acc_descr, nodes, config).map_err(|message| {
            #[cfg(feature = "editor-facts")]
            if let Some(facts) = &mut editor_facts {
                facts.mark_recovered_from_parse_error(message.clone(), None);
            }
            Error::diagram_parse_fallback(diagram_type.to_string(), message)
        });
    control.checkpoint()?;
    Ok(crate::family::SemanticConstruction {
        model,
        #[cfg(feature = "editor-facts")]
        editor_facts,
    })
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &TreeViewDiagramModel,
    meta: &SourcePresentationMetadata,
) -> Result<Value> {
    merman_model::families::tree_view::compat::project(model, &meta.compat_json_metadata())
        .map_err(Into::into)
}

fn parse_tree_view_input_controlled(
    code: &str,
    diagram_type: &str,
    #[cfg(feature = "editor-facts")] demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<TreeViewParseOutcome> {
    control.checkpoint()?;
    #[cfg(test)]
    crate::diagrams::langium_common::record_family_syntax_construction("treeView");
    #[cfg(feature = "editor-facts")]
    let mut editor_facts = demand.needs_recovery().then(EditorSemanticFacts::new);
    let mut first_issue = None;
    let body = match tree_view_body_start_controlled(code, control)? {
        Ok(body) => body,
        Err(issue) => {
            mark_tree_view_recovery(
                #[cfg(feature = "editor-facts")]
                &mut editor_facts,
                &mut first_issue,
                issue,
            );
            return Ok(TreeViewParseOutcome {
                snapshot: ParsedTreeViewInput {
                    title: None,
                    acc_title: None,
                    acc_descr: None,
                    nodes: Vec::new(),
                    #[cfg(feature = "editor-facts")]
                    editor_facts,
                },
                first_issue,
            });
        }
    };
    let (source, issues) = preprocess_tree_view_body(code, body.offset, diagram_type, control)?;
    for issue in issues {
        mark_tree_view_recovery(
            #[cfg(feature = "editor-facts")]
            &mut editor_facts,
            &mut first_issue,
            issue,
        );
    }
    let code = source.text.as_str();
    let mut offset = 0;
    let mut title = None;
    let mut acc_title = None;
    let mut acc_descr = None;
    let mut nodes = Vec::new();
    let mut saw_node = false;
    let mut line_end = 0;
    let mut next_line_offset = 0;

    while offset < code.len() {
        control.checkpoint()?;
        if let Some(parsed) = parse_langium_common(code, offset) {
            if saw_node {
                let span = parsed.fact.raw_span;
                mark_tree_view_recovery(
                    #[cfg(feature = "editor-facts")]
                    &mut editor_facts,
                    &mut first_issue,
                    TreeViewParseIssue::new(
                        "tree view title and accessibility fields must precede every node",
                        Some(source.source_span(span)),
                    ),
                );
                offset += parsed.consumed;
                continue;
            }
            let field = parsed.fact.field;
            let value = parsed.fact.value.clone();
            #[cfg(feature = "editor-facts")]
            if let Some(facts) = editor_facts.as_mut().filter(|_| demand.captures_rich()) {
                let mut fact = parsed.fact.clone();
                fact.raw_span = source.source_span(fact.raw_span);
                fact.value_span = source.source_span(fact.value_span);
                push_langium_common_editor_fact(facts, &fact, "tree view");
            }
            match field {
                LangiumCommonField::Title => title = Some(value),
                LangiumCommonField::AccTitle => acc_title = Some(value),
                LangiumCommonField::AccDescr => acc_descr = Some(value),
            }
            if let Some(diagnostic) = parsed.diagnostic {
                mark_tree_view_recovery(
                    #[cfg(feature = "editor-facts")]
                    &mut editor_facts,
                    &mut first_issue,
                    TreeViewParseIssue::new(
                        diagnostic.message,
                        Some(source.source_span(diagnostic.span)),
                    ),
                );
            }
            offset += parsed.consumed;
            continue;
        }

        // Adjacent quoted nodes can resume within one physical line.
        if offset >= next_line_offset {
            let (line, next_offset) = physical_line_at(code, offset);
            line_end = offset + line.len();
            next_line_offset = next_offset;
            #[cfg(test)]
            TREE_VIEW_BODY_LINE_SCAN_BYTES.with(|bytes| {
                bytes.set(bytes.get() + next_offset - offset);
            });
        }
        let line = &code[offset..line_end];
        let starts_physical_line = offset == 0 || code.as_bytes()[offset - 1] == b'\n';
        if line.is_empty()
            || (starts_physical_line
                && (is_tree_view_cleaned_comment_line(line) || line.starts_with("%%")))
        {
            offset = next_line_offset;
            continue;
        }
        saw_node = true;
        let (parsed, next_offset) = parse_tree_view_node(code, offset, control)?;
        offset = next_offset;
        match parsed {
            Ok(node) => {
                #[cfg(feature = "editor-facts")]
                let mut node = node;
                #[cfg(feature = "editor-facts")]
                {
                    node.span = source.source_span(node.span);
                    node.selection = source.source_span(node.selection);
                    for value in [&mut node.css_class, &mut node.icon, &mut node.description]
                        .into_iter()
                        .flatten()
                    {
                        value.span = source.source_span(value.span);
                    }
                    if let Some(facts) = editor_facts.as_mut().filter(|_| demand.captures_rich()) {
                        push_tree_view_node_editor_facts(facts, &node);
                    }
                }
                nodes.push(node);
            }
            Err(mut issue) => {
                issue.span = issue.span.map(|span| source.source_span(span));
                mark_tree_view_recovery(
                    #[cfg(feature = "editor-facts")]
                    &mut editor_facts,
                    &mut first_issue,
                    issue,
                );
            }
        }
    }

    control.checkpoint()?;
    Ok(TreeViewParseOutcome {
        snapshot: ParsedTreeViewInput {
            title,
            acc_title,
            acc_descr,
            nodes,
            #[cfg(feature = "editor-facts")]
            editor_facts,
        },
        first_issue,
    })
}

#[derive(Debug, Clone, Copy)]
struct TreeViewBodyStart {
    offset: usize,
}

fn tree_view_body_start_controlled(
    code: &str,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<std::result::Result<TreeViewBodyStart, TreeViewParseIssue>> {
    let mut offset = 0usize;
    while offset < code.len() {
        control.checkpoint()?;
        let (line, next_offset) = physical_line_at(code, offset);
        let trimmed = line.trim();
        if trimmed.is_empty() || is_tree_view_comment_line(line) {
            offset = next_offset;
            continue;
        }

        let leading = line.len().saturating_sub(line.trim_start().len());
        let span = SourceSpan::new(offset + leading, offset + leading + trimmed.len());
        let Some(trailing) = trimmed.strip_prefix("treeView-beta") else {
            return Ok(Err(TreeViewParseIssue::new(
                "expected treeView-beta",
                Some(span),
            )));
        };
        if !trailing.trim().is_empty() && !trailing.trim_start().starts_with("%%") {
            return Ok(Err(TreeViewParseIssue::new(
                "unexpected tokens after treeView-beta",
                Some(span),
            )));
        }
        return Ok(Ok(TreeViewBodyStart {
            offset: next_offset,
        }));
    }

    Ok(Err(TreeViewParseIssue::new("expected treeView-beta", None)))
}

fn mark_tree_view_recovery(
    #[cfg(feature = "editor-facts")] editor_facts: &mut Option<EditorSemanticFacts>,
    first_issue: &mut Option<TreeViewParseIssue>,
    issue: TreeViewParseIssue,
) {
    if first_issue.is_none() {
        *first_issue = Some(issue.clone());
    }
    #[cfg(feature = "editor-facts")]
    if let Some(editor_facts) = editor_facts {
        editor_facts.mark_recovered_from_parse_error(
            format!(
                "treeView parser recovered after parse error: {}",
                issue.message
            ),
            issue.span,
        );
    }
}

fn tree_view_error_message(error: &Error) -> String {
    match error {
        Error::DiagramParse { diagnostic, .. } => diagnostic.message().to_string(),
        _ => error.to_string(),
    }
}

fn tree_view_input_to_model(
    title: Option<String>,
    acc_title: Option<String>,
    acc_descr: Option<String>,
    nodes: Vec<TreeViewNodeDetails>,
    config: &crate::MermaidConfig,
) -> std::result::Result<TreeViewDiagramModel, String> {
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
    for (next_id, flat) in (1i64..).zip(nodes) {
        let level = flat.indent as i64;
        while stack
            .last()
            .and_then(|&idx| arena.get(idx))
            .is_some_and(|node| level <= node.level)
        {
            stack.pop();
        }

        let parent = stack.last().copied().unwrap_or(0);
        let idx = arena.len();
        arena.push(ArenaNode {
            id: next_id,
            level,
            name: flat.name,
            node_type: flat.node_type,
            css_class: flat.css_class.map(|value| value.value),
            icon: flat.icon.map(|value| value.value),
            description: flat
                .description
                .map(|value| crate::sanitize::sanitize_text(&value.value, config)),
            children: Vec::new(),
        });
        arena[parent].children.push(idx);
        stack.push(idx);
        if stack.len().saturating_sub(1) > MAX_DIAGRAM_NESTING_DEPTH {
            return Err(format!(
                "treeView nesting depth exceeds {MAX_DIAGRAM_NESTING_DEPTH}"
            ));
        }
    }

    Ok(TreeViewDiagramModel {
        title,
        acc_title,
        acc_descr,
        root: arena_node_to_model(&arena, 0),
    })
}

fn arena_node_to_model(arena: &[ArenaNode], idx: usize) -> TreeViewNode {
    let mut models: Vec<Option<TreeViewNode>> = vec![None; arena.len()];
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
            models[node_idx] = Some(TreeViewNode {
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

fn preprocess_tree_view_body(
    code: &str,
    body_start: usize,
    diagram_type: &str,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<(TreeViewSource, Vec<TreeViewParseIssue>)> {
    let lines = code[body_start..].lines().collect::<Vec<_>>();
    let line_format = TreeViewLineFormat::from_lines(&lines);
    let mut source = TreeViewSource {
        text: String::new(),
        segments: Vec::new(),
        source_end: code.len(),
    };
    let mut issues = Vec::new();
    let mut offset = body_start;
    while offset < code.len() {
        control.checkpoint()?;
        let (line, next_offset) = physical_line_at(code, offset);
        let passthrough = !line_format.box_drawing
            || line.trim().is_empty()
            || is_tree_view_comment_line(line)
            || is_tree_view_metadata_line(line);
        if passthrough {
            source.push(line, SourceSpan::new(offset, offset + line.len()));
        } else {
            match tree_view_line_view(line, line_format, diagram_type) {
                Ok(Some(view)) if view.expand_content_tabs => {
                    let content_start = offset + view.content_offset;
                    source.push(
                        &" ".repeat(view.indent),
                        SourceSpan::new(offset, content_start),
                    );
                    let mut start = 0;
                    for (index, ch) in view.content.char_indices() {
                        if index % 128 == 0 {
                            control.checkpoint()?;
                        }
                        if ch == '\t' {
                            source.push(
                                &view.content[start..index],
                                SourceSpan::new(content_start + start, content_start + index),
                            );
                            source.push(
                                "    ",
                                SourceSpan::new(content_start + index, content_start + index + 1),
                            );
                            start = index + 1;
                        }
                    }
                    source.push(
                        &view.content[start..],
                        SourceSpan::new(content_start + start, content_start + view.content.len()),
                    );
                }
                Ok(Some(_)) => {
                    source.push(line, SourceSpan::new(offset, offset + line.len()));
                }
                Ok(None) => {
                    offset = next_offset;
                    continue;
                }
                Err(error) => {
                    let leading = line.len() - line.trim_start().len();
                    issues.push(TreeViewParseIssue::new(
                        tree_view_error_message(&error),
                        Some(SourceSpan::new(
                            offset + leading,
                            offset + line.trim_end().len(),
                        )),
                    ));
                    offset = next_offset;
                    continue;
                }
            }
        }
        let line_end = offset + line.len();
        if next_offset > line_end {
            source.push("\n", SourceSpan::new(line_end, next_offset));
        }
        offset = next_offset;
    }
    control.checkpoint()?;
    Ok((source, issues))
}

impl TreeViewLineFormat {
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
    diagram_type: &str,
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
            expand_content_tabs: false,
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
                diagram_type,
                "empty tree node after box-drawing prefix",
            ));
        }
        return Ok(Some(TreeViewLineView {
            indent: depth * 4,
            content,
            content_offset,
            expand_content_tabs: true,
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
            expand_content_tabs: false,
        }));
    }

    if line.chars().next().is_some_and(char::is_whitespace) {
        return Err(parse_error(
            diagram_type,
            "unexpected indentation without box-drawing prefix in treeView box-drawing input",
        ));
    }

    let content = line.trim_end();
    Ok(Some(TreeViewLineView {
        indent: 0,
        content,
        content_offset: 0,
        expand_content_tabs: false,
    }))
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

fn parse_tree_view_node(
    code: &str,
    offset: usize,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<(
    std::result::Result<TreeViewNodeDetails, TreeViewParseIssue>,
    usize,
)> {
    let (indent, content) = split_ascii_indent(&code[offset..]);
    let mut node_start = code.len() - content.len();
    // INDENTATION is a grammar token, while NL and comments are hidden. An indent
    // after a completed node can therefore belong to a name on a later line.
    loop {
        control.checkpoint()?;
        if code.as_bytes().get(node_start) == Some(&b'\n') {
            node_start += 1;
            continue;
        }
        if code[node_start..].starts_with("%%") {
            node_start = physical_line_at(code, node_start).1;
            continue;
        }
        if node_start == 0 || code.as_bytes().get(node_start - 1) == Some(&b'\n') {
            let (line, next_offset) = physical_line_at(code, node_start);
            if is_tree_view_cleaned_comment_line(line) {
                node_start = next_offset;
                continue;
            }
        }
        break;
    }
    let content = &code[node_start..];
    if let Some(common) = parse_langium_common(code, node_start) {
        return Ok((
            Err(TreeViewParseIssue::new(
                "tree view title and accessibility fields must precede every node",
                Some(common.fact.raw_span),
            )),
            node_start + common.consumed,
        ));
    }
    if content.starts_with([' ', '\t']) {
        return Ok(tree_view_node_failure(
            code,
            node_start,
            node_start,
            "expected tree node name after indentation",
        ));
    }
    let (raw_name, _name_start, name_end, mut pos) = if let Some(quote @ (b'"' | b'\'')) =
        content.as_bytes().first().copied()
    {
        // QUOTED_NAME has no escapes or line-ending restriction. Only its matching quote
        // ends the token; comment and metadata spellings inside it are ordinary payload.
        let mut close = None;
        for (index, byte) in content.as_bytes()[1..].iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            if *byte == quote {
                close = Some(node_start + index + 1);
                break;
            }
        }
        let Some(close) = close else {
            return Ok(tree_view_node_failure(
                code,
                node_start,
                node_start,
                "unterminated quoted tree node name",
            ));
        };
        (
            code[node_start + 1..close].to_string(),
            node_start + 1,
            close,
            close + 1,
        )
    } else {
        // BARE_NAME excludes these prefixes even when they cannot form an annotation.
        if content.starts_with(":::") || content.starts_with("icon(") || content.starts_with("##") {
            return Ok(tree_view_node_failure(
                code,
                node_start,
                node_start,
                "expected tree node name",
            ));
        }
        let (line, _) = physical_line_at(code, node_start);
        let annotation_start = find_next_tree_view_annotation_start(line, 0);
        let end = trim_end_byte_index(&line[..annotation_start.unwrap_or(line.len())]);
        if end == 0 {
            return Ok(tree_view_node_failure(
                code,
                node_start,
                node_start,
                "expected tree node name",
            ));
        }
        (
            line[..end].to_string(),
            node_start,
            node_start + end,
            // A bare token owns its trailing blanks unless an annotation starts there.
            node_start
                + if annotation_start.is_some() {
                    end
                } else {
                    line.len()
                },
        )
    };
    let (name, node_type, _selection_end) = normalize_tree_view_node_name(raw_name, name_end);
    let mut annotations = TreeViewAnnotations::default();
    let mut _node_end = pos;
    loop {
        control.checkpoint()?;
        let token_start = skip_ascii_whitespace(code, pos);
        if token_start >= code.len() {
            break;
        }
        if code.as_bytes()[token_start] == b'\n' {
            if token_start != pos {
                break;
            }
            pos = token_start + 1;
            continue;
        }
        // Hidden ML_COMMENT is recognized between tokens, never within BARE_NAME or DESC.
        if code[token_start..].starts_with("%%") {
            if token_start != pos {
                break;
            }
            pos = physical_line_at(code, token_start).1;
            continue;
        }
        if token_start == pos {
            break;
        }

        if code[token_start..].starts_with(":::") {
            let value_start = skip_ascii_whitespace(code, token_start + 3);
            let Some(value_end) = tree_view_class_name_end(code, value_start) else {
                return Ok(tree_view_node_failure(
                    code,
                    node_start,
                    token_start,
                    "expected tree node class after :::",
                ));
            };
            annotations.css_class = Some(TreeViewSpannedValue {
                value: code[value_start..value_end].to_string(),
                #[cfg(feature = "editor-facts")]
                span: SourceSpan::new(value_start, value_end),
            });
            pos = value_end;
        } else if code[token_start..].starts_with("icon(") {
            let value_start = token_start + "icon(".len();
            let (payload_line, _) = physical_line_at(code, value_start);
            let Some(close_rel) = payload_line.find(')') else {
                return Ok(tree_view_node_failure(
                    code,
                    node_start,
                    token_start,
                    "unterminated tree node icon annotation",
                ));
            };
            let value_end = value_start + close_rel;
            let payload = &code[value_start..value_end];
            if !is_tree_view_icon_name(payload) {
                return Ok(tree_view_node_failure(
                    code,
                    node_start,
                    token_start,
                    "invalid tree node icon annotation",
                ));
            }
            let value = if payload.is_empty() {
                "none".to_string()
            } else {
                payload.to_string()
            };
            annotations.icon = Some(TreeViewSpannedValue {
                value,
                #[cfg(feature = "editor-facts")]
                span: SourceSpan::new(value_start, value_end),
            });
            pos = value_end + ')'.len_utf8();
        } else if code[token_start..].starts_with("##") {
            let value_start = token_start + 2;
            let (line, _) = physical_line_at(code, value_start);
            let end = value_start + line.len();
            let (trimmed_start, trimmed_end) = trim_ascii_span(code, value_start, end);
            annotations.description =
                (trimmed_start != trimmed_end).then(|| TreeViewSpannedValue {
                    value: code[trimmed_start..trimmed_end].to_string(),
                    #[cfg(feature = "editor-facts")]
                    span: SourceSpan::new(trimmed_start, trimmed_end),
                });
            pos = end;
        } else {
            break;
        }
        _node_end = pos;
    }

    Ok((
        Ok(TreeViewNodeDetails {
            indent,
            name,
            node_type,
            css_class: annotations.css_class,
            icon: annotations.icon,
            description: annotations.description,
            #[cfg(feature = "editor-facts")]
            span: SourceSpan::new(node_start, _node_end),
            #[cfg(feature = "editor-facts")]
            selection: SourceSpan::new(_name_start, _selection_end),
        }),
        pos,
    ))
}

fn tree_view_node_failure(
    code: &str,
    node_start: usize,
    error_offset: usize,
    message: &str,
) -> (
    std::result::Result<TreeViewNodeDetails, TreeViewParseIssue>,
    usize,
) {
    let (line, next_offset) = physical_line_at(code, error_offset);
    (
        Err(TreeViewParseIssue::new(
            message,
            Some(SourceSpan::new(
                node_start,
                error_offset + line.trim_end().len(),
            )),
        )),
        next_offset,
    )
}

#[cfg(feature = "editor-facts")]
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

#[cfg(feature = "editor-facts")]
fn push_tree_view_node_editor_facts(facts: &mut EditorSemanticFacts, node: &TreeViewNodeDetails) {
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::NodeIdentifier,
        node.selection,
    ));
    facts.push_symbol(EditorSemanticSymbol::new(
        node.name.clone(),
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
            push_tree_view_spanned_payload_fact(facts, value, detail);
        }
    }
}

fn should_skip_tree_view_box_detection_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty()
        || is_tree_view_comment_line(line)
        || is_tree_view_metadata_line(line)
        || is_tree_view_decoration_only(line)
}

fn is_tree_view_metadata_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    if let Some(rest) = trimmed.strip_prefix("title") {
        return rest.starts_with([' ', '\t']);
    }
    if let Some(rest) = trimmed.strip_prefix("accTitle") {
        return rest.trim_start_matches([' ', '\t']).starts_with(':');
    }
    if let Some(rest) = trimmed.strip_prefix("accDescr") {
        return rest.trim_start_matches([' ', '\t']).starts_with(':')
            || rest.trim_start().starts_with('{');
    }
    false
}

fn is_tree_view_comment_line(line: &str) -> bool {
    line.trim_start().starts_with("%%")
}

fn is_tree_view_cleaned_comment_line(line: &str) -> bool {
    // Shared cleanupComments removes only complete non-directive lines with a
    // nonempty payload. Empty %% is still hidden grammar text, after any indent.
    line.trim_start()
        .strip_prefix("%%")
        .is_some_and(|payload| !payload.is_empty() && !payload.starts_with('{'))
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
        let valid_class = s[idx..].starts_with(":::")
            && tree_view_class_name_end(s, skip_ascii_whitespace(s, idx + 3)).is_some();
        if (valid_class || s[idx..].starts_with("##") || s[idx..].starts_with("icon("))
            && ((from == 0 && idx == 0) || is_annotation_token_boundary(s, idx))
        {
            return Some(idx);
        }
    }
    None
}

fn tree_view_class_name_end(s: &str, start: usize) -> Option<usize> {
    let mut chars = s.get(start..)?.char_indices();
    let (_, first) = chars.next()?;
    if !first.is_ascii_alphabetic() && first != '_' {
        return None;
    }
    let mut end = start + first.len_utf8();
    for (relative, ch) in chars {
        if !ch.is_ascii_alphanumeric() && ch != '_' && ch != '-' {
            break;
        }
        end = start + relative + ch.len_utf8();
    }
    Some(end)
}

fn is_tree_view_icon_name(value: &str) -> bool {
    fn is_component(value: &str, allow_empty: bool) -> bool {
        (allow_empty || !value.is_empty())
            && value
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    }

    match value.split_once(':') {
        Some((pack, icon)) => {
            !icon.contains(':') && is_component(pack, true) && is_component(icon, false)
        }
        None => is_component(value, true),
    }
}

fn is_annotation_token_boundary(s: &str, idx: usize) -> bool {
    idx > 0
        && s[..idx]
            .chars()
            .next_back()
            .is_some_and(|ch| ch == ' ' || ch == '\t')
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

fn parse_error(diagram_type: &str, message: impl Into<String>) -> Error {
    Error::diagram_parse_fallback(diagram_type.to_string(), message.into())
}

#[cfg(test)]
mod semantic_tests {
    use super::*;

    #[test]
    fn indentation_between_completed_nodes_and_hidden_tokens_belongs_to_the_next_name() {
        for newline in ["\n", "\r\n"] {
            for first in ["\"file\"", "'file'", "file :::highlight", "file icon(file)"] {
                for (gap, level) in [("", 0), (" ", 1), ("   ", 3), ("\t", 1)] {
                    let text = format!(
                        "treeView-beta{newline}{first}{gap}%% inline{newline}  %% full-line{newline}{newline}next{newline}"
                    );
                    let parsed = crate::SemanticParser::new()
                        .parse_diagram_sync(&text)
                        .expect("hidden tokens preserve the next node's indentation");
                    let crate::SemanticDiagramModel::TreeView(model) = parsed.model else {
                        panic!("expected TreeView semantics");
                    };
                    let next = if level == 0 {
                        assert_eq!(model.root.children.len(), 2);
                        &model.root.children[1]
                    } else {
                        assert_eq!(model.root.children.len(), 1);
                        assert_eq!(model.root.children[0].children.len(), 1);
                        &model.root.children[0].children[0]
                    };
                    assert_eq!(next.name, "next");
                    assert_eq!(next.level, level);

                    #[cfg(feature = "presentation")]
                    {
                        let rendered = crate::MermaidEngine::new()
                            .parse_diagram_for_render_model_sync(
                                &text,
                                crate::ParseOptions::strict(),
                            )
                            .unwrap()
                            .unwrap();
                        let crate::RenderSemanticModel::TreeView(render_model) = rendered.model()
                        else {
                            panic!("expected TreeView render model");
                        };
                        assert_eq!(
                            serde_json::to_value(&model).unwrap(),
                            serde_json::to_value(render_model).unwrap()
                        );
                    }
                    #[cfg(all(feature = "presentation", feature = "editor-facts"))]
                    {
                        let facts = crate::MermaidEngine::new()
                            .parse_editor_semantic_facts_with_type_sync("treeView", &text)
                            .unwrap()
                            .unwrap();
                        assert_eq!(
                            facts.completeness,
                            crate::EditorSemanticCompleteness::Complete
                        );
                        let symbol = facts
                            .symbols
                            .iter()
                            .find(|symbol| symbol.name == "next")
                            .unwrap();
                        let start = text.rfind("next").unwrap();
                        assert_eq!(symbol.selection, SourceSpan::new(start, start + 4));
                    }
                }
            }
        }

        for text in [
            "treeView-beta\n\"file\" \nnext\n",
            "treeView-beta\n\"file\"\n \nnext\n",
        ] {
            let parsed = crate::SemanticParser::new()
                .parse_diagram_sync(text)
                .unwrap();
            let crate::SemanticDiagramModel::TreeView(model) = parsed.model else {
                panic!("expected TreeView semantics");
            };
            assert_eq!(model.root.children[0].children[0].name, "next");
            assert_eq!(model.root.children[0].children[0].level, 1);
        }
        for (text, level) in [
            ("treeView-beta\n\"file\"\n  %%\nnext\n", 2),
            ("treeView-beta\n\"file\" %% first\n  %% \nnext\n", 1),
        ] {
            let parsed = crate::SemanticParser::new()
                .parse_diagram_sync(text)
                .unwrap();
            let crate::SemanticDiagramModel::TreeView(model) = parsed.model else {
                panic!("expected TreeView semantics");
            };
            assert_eq!(model.root.children.len(), 1);
            assert_eq!(model.root.children[0].children[0].name, "next");
            assert_eq!(model.root.children[0].children[0].level, level);
        }
        for text in [
            "treeView-beta\nfile   \nnext\n",
            "treeView-beta\nfile ## description   \nnext\n",
        ] {
            let parsed = crate::SemanticParser::new()
                .parse_diagram_sync(text)
                .unwrap();
            let crate::SemanticDiagramModel::TreeView(model) = parsed.model else {
                panic!("expected TreeView semantics");
            };
            assert_eq!(model.root.children.len(), 2);
            assert_eq!(model.root.children[0].name, "file");
            assert_eq!(model.root.children[1].name, "next");
            assert_eq!(model.root.children[1].level, 0);
        }
    }

    #[test]
    fn dangling_or_repeated_indentation_tokens_reject_missing_names() {
        for text in [
            "treeView-beta\n\"file\" %% comment\n",
            "treeView-beta\n\"file\"   ",
            "treeView-beta\n\"file\"\n  \n",
            "treeView-beta\n\"file\" %% comment\n  next\n",
            "treeView-beta\n\"file\" %% first\n  %%\nnext\n",
        ] {
            assert!(
                crate::SemanticParser::new()
                    .parse_diagram_sync(text)
                    .is_err(),
                "{text:?}"
            );
            #[cfg(feature = "presentation")]
            assert!(
                crate::MermaidEngine::new()
                    .parse_diagram_for_render_model_sync(text, crate::ParseOptions::strict())
                    .is_err(),
                "{text:?}"
            );
        }
        let parsed = crate::SemanticParser::new()
            .parse_diagram_sync("treeView-beta\n\"file\"%% comment\n")
            .expect("a comment without an indentation token is hidden at EOF");
        let crate::SemanticDiagramModel::TreeView(model) = parsed.model else {
            panic!("expected TreeView semantics");
        };
        assert_eq!(model.root.children.len(), 1);
        assert_eq!(model.root.children[0].name, "file");
    }

    #[test]
    fn pending_indentation_keeps_metadata_terminal_priority_and_recovery_ranges() {
        for newline in ["\n", "\r\n"] {
            for first in ["\"file\"", "file :::highlight", "file icon(file)"] {
                for gap in [" ", " %% comment"] {
                    for metadata in ["title Late", "accTitle: Late", "accDescr: Late"] {
                        let text = format!(
                            "treeView-beta{newline}{first}{gap}{newline}{metadata}{newline}after{newline}"
                        );
                        let start = text.find(metadata).unwrap();
                        let expected = SourceSpan::new(start, start + metadata.len());
                        let error = crate::SemanticParser::new()
                            .parse_diagram_sync(&text)
                            .unwrap_err();
                        let Error::DiagramParse { diagnostic, .. } = error else {
                            panic!("expected late metadata diagnostic");
                        };
                        assert!(diagnostic.message().contains("must precede every node"));
                        assert_eq!(diagnostic.span(), Some(expected));
                        #[cfg(feature = "presentation")]
                        assert!(
                            crate::MermaidEngine::new()
                                .parse_diagram_for_render_model_sync(
                                    &text,
                                    crate::ParseOptions::strict()
                                )
                                .is_err()
                        );
                        #[cfg(all(feature = "presentation", feature = "editor-facts"))]
                        {
                            let facts = crate::MermaidEngine::new()
                                .parse_editor_semantic_facts_with_type_sync("treeView", &text)
                                .unwrap()
                                .unwrap();
                            assert_eq!(
                                facts.completeness,
                                crate::EditorSemanticCompleteness::Recovered
                            );
                            assert!(
                                facts
                                    .diagnostics
                                    .iter()
                                    .any(|diagnostic| diagnostic.span == Some(expected))
                            );
                            for name in ["file", "after"] {
                                assert!(facts.symbols.iter().any(|symbol| symbol.name == name));
                            }
                            assert!(
                                !facts
                                    .symbols
                                    .iter()
                                    .any(|symbol| symbol.name.contains("Late"))
                            );
                        }
                    }
                }
            }
        }
        let parsed = crate::SemanticParser::new()
            .parse_diagram_sync("treeView-beta\n\"file\" \n\"accTitle: Late\"\nafter\n")
            .expect("metadata spellings within quoted names remain payloads");
        let crate::SemanticDiagramModel::TreeView(model) = parsed.model else {
            panic!("expected TreeView semantics");
        };
        assert_eq!(model.root.children[0].children[0].name, "accTitle: Late");
    }

    #[test]
    fn quoted_names_reject_annotation_prefixes_as_adjacent_bare_names() {
        #[cfg(feature = "presentation")]
        let metadata = SourcePresentationMetadata {
            diagram_type: "treeView".into(),
            config: crate::MermaidConfig::default(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        };
        for suffix in [":::9bad", "icon(file)", "## description"] {
            for tail in ["", "after.txt\n"] {
                let source = format!("treeView-beta\n\"x\"{suffix}\n{tail}");
                let start = "treeView-beta\n\"x\"".len();
                let span = SourceSpan::new(start, start + suffix.len());
                let errors = [
                    crate::SemanticParser::new()
                        .parse_diagram_sync(&source)
                        .expect_err("annotation prefixes cannot start bare names"),
                    #[cfg(feature = "presentation")]
                    parse_tree_view_model_for_render(&source, &metadata)
                        .expect_err("typed rendering must reject the suffix"),
                    #[cfg(feature = "compat-json")]
                    parse_tree_view(&source, &metadata)
                        .expect_err("compat parsing must reject the suffix"),
                ];
                for error in errors {
                    let Error::DiagramParse { diagnostic, .. } = error else {
                        panic!("expected a TreeView parse diagnostic");
                    };
                    assert!(diagnostic.message().contains("expected tree node name"));
                    assert_eq!(diagnostic.span(), Some(span));
                }

                #[cfg(feature = "editor-facts")]
                {
                    let outcome = parse_tree_view_input_controlled(
                        &source,
                        "treeView",
                        EditorFactsDemand::Capture,
                        &crate::OperationControl::new(),
                    )
                    .unwrap();
                    let facts = outcome.snapshot.editor_facts.unwrap();
                    assert_eq!(
                        facts.completeness,
                        crate::EditorSemanticCompleteness::Recovered
                    );
                    assert_eq!(facts.diagnostics.len(), 1);
                    assert_eq!(facts.diagnostics[0].span, Some(span));
                    let names = facts
                        .symbols
                        .iter()
                        .map(|symbol| symbol.name.as_str())
                        .collect::<Vec<_>>();
                    if tail.is_empty() {
                        assert_eq!(names, ["x"]);
                    } else {
                        assert_eq!(names, ["x", "after.txt"]);
                    }
                }
            }
        }

        let parsed = crate::SemanticParser::new()
            .parse_diagram_sync("treeView-beta\nfile :::9bad\n")
            .expect("invalid class markers inside an existing bare name remain literal");
        let crate::SemanticDiagramModel::TreeView(model) = parsed.model else {
            panic!("expected TreeView semantics");
        };
        assert_eq!(model.root.children[0].name, "file :::9bad");
        assert!(model.root.children[0].css_class.is_none());
    }

    #[test]
    fn box_drawing_eof_mapping_preserves_token_ends_and_eof_insertions() {
        for newline in ["\n", "\r\n"] {
            for class_suffix in ["", " :::ready"] {
                for ending in ["", newline] {
                    let text =
                        format!("treeView-beta{newline}├── file.txt{class_suffix}   {ending}");
                    let (source, issues) = preprocess_tree_view_body(
                        &text,
                        "treeView-beta".len() + newline.len(),
                        "treeView",
                        &crate::OperationControl::new(),
                    )
                    .unwrap();
                    assert!(issues.is_empty());
                    let eof = source.text.len();
                    assert_eq!(
                        source.source_span(SourceSpan::new(eof, eof)),
                        SourceSpan::new(text.len(), text.len())
                    );

                    let crlf_extra = newline.len() - 1;
                    let mut expected = vec![(
                        "file.txt",
                        SourceSpan::new(24 + crlf_extra, 32 + crlf_extra),
                    )];
                    if !class_suffix.is_empty() {
                        expected.push(("ready", SourceSpan::new(36 + crlf_extra, 41 + crlf_extra)));
                    }
                    for (payload, span) in &expected {
                        let start = source.text.find(*payload).unwrap();
                        assert_eq!(
                            source.source_span(SourceSpan::new(start, start + payload.len())),
                            *span
                        );
                        assert_eq!(&text[span.start..span.end], *payload);
                    }

                    #[cfg(feature = "editor-facts")]
                    {
                        let outcome = parse_tree_view_input_controlled(
                            &text,
                            "treeView",
                            EditorFactsDemand::Capture,
                            &crate::OperationControl::new(),
                        )
                        .unwrap();
                        assert!(outcome.first_issue.is_none());
                        let captures = [
                            outcome.snapshot.editor_facts.unwrap(),
                            #[cfg(feature = "presentation")]
                            crate::MermaidEngine::new()
                                .parse_editor_semantic_facts_with_type_sync("treeView", &text)
                                .unwrap()
                                .unwrap(),
                        ];
                        for facts in captures {
                            assert_eq!(
                                facts.completeness,
                                crate::EditorSemanticCompleteness::Complete
                            );
                            assert!(facts.diagnostics.is_empty());
                            for (payload, span) in &expected {
                                let symbol = facts
                                    .symbols
                                    .iter()
                                    .find(|symbol| symbol.name == *payload)
                                    .expect("retained box node or class fact");
                                assert_eq!(symbol.selection, *span);
                                assert_eq!(
                                    &text[symbol.selection.start..symbol.selection.end],
                                    *payload
                                );
                                assert!(
                                    facts
                                        .expected_syntax
                                        .iter()
                                        .any(|syntax| syntax.span == *span)
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn adjacent_quoted_nodes_scan_each_physical_line_once() {
        const HEADER: &str = "treeView-beta\n";
        for count in [8, 64, 1_024] {
            for ending in ["", "\n"] {
                let source = format!("{HEADER}{}{ending}", "\"x\"".repeat(count));
                TREE_VIEW_BODY_LINE_SCAN_BYTES.with(|bytes| bytes.set(0));
                let outcome = parse_tree_view_input_controlled(
                    &source,
                    "treeView",
                    #[cfg(feature = "editor-facts")]
                    EditorFactsDemand::Capture,
                    &crate::OperationControl::new(),
                )
                .unwrap();
                let scanned_bytes = TREE_VIEW_BODY_LINE_SCAN_BYTES.with(|bytes| bytes.get());
                assert_eq!(scanned_bytes, source.len() - HEADER.len());
                assert!(outcome.first_issue.is_none());
                let snapshot = outcome.snapshot;
                assert_eq!(snapshot.nodes.len(), count);
                #[cfg(feature = "editor-facts")]
                {
                    let facts = snapshot.editor_facts.as_ref().unwrap();
                    assert_eq!(
                        facts.completeness,
                        crate::EditorSemanticCompleteness::Complete
                    );
                    assert!(facts.diagnostics.is_empty());
                    assert_eq!(facts.symbols.len(), count);
                    assert_eq!(facts.expected_syntax.len(), count);
                    for (index, symbol) in facts.symbols.iter().enumerate() {
                        let start = HEADER.len() + index * "\"x\"".len();
                        let selection = SourceSpan::new(start + 1, start + 2);
                        assert_eq!(symbol.name, "x");
                        assert_eq!(symbol.span, SourceSpan::new(start, start + 3));
                        assert_eq!(symbol.selection, selection);
                        assert_eq!(facts.expected_syntax[index].span, selection);
                        assert_eq!(
                            facts.expected_syntax[index].kind,
                            EditorExpectedSyntaxKind::NodeIdentifier
                        );
                    }
                }
                let model = tree_view_input_to_model(
                    snapshot.title,
                    snapshot.acc_title,
                    snapshot.acc_descr,
                    snapshot.nodes,
                    &crate::MermaidConfig::default(),
                )
                .unwrap();
                assert_eq!(model.root.children.len(), count);
                assert!(model.root.children.iter().all(|node| {
                    node.name == "x" && node.level == 0 && node.children.is_empty()
                }));
            }
        }
    }

    #[test]
    fn canonical_tree_preserves_percent_tokens_and_normalizes_multiline_names() {
        for newline in ["\n", "\r\n"] {
            let source = [
                "treeView-beta",
                "file%%name.txt",
                "file.txt ## progress100%%complete",
                "\"alpha",
                "beta\" :::highlight icon(file) ## multiline%%description",
                "after.txt",
                "",
            ]
            .join(newline);
            let parsed = crate::SemanticParser::new()
                .parse_diagram_sync(&source)
                .expect("canonical TreeView tokens");
            let crate::SemanticDiagramModel::TreeView(model) = parsed.model else {
                panic!("expected TreeView semantics");
            };
            let nodes = &model.root.children;
            assert_eq!(nodes.len(), 4);
            assert_eq!(nodes[0].name, "file%%name.txt");
            assert_eq!(
                nodes[1].description.as_deref(),
                Some("progress100%%complete")
            );
            assert_eq!(nodes[2].name, "alpha\nbeta");
            assert_eq!(nodes[2].css_class.as_deref(), Some("highlight"));
            assert_eq!(nodes[2].icon.as_deref(), Some("file"));
            assert_eq!(
                nodes[2].description.as_deref(),
                Some("multiline%%description")
            );
            assert_eq!(nodes[3].name, "after.txt");

            #[cfg(feature = "presentation")]
            {
                let full = crate::MermaidEngine::new()
                    .parse_diagram_for_render_model_sync(&source, crate::ParseOptions::strict())
                    .unwrap()
                    .expect("full TreeView parse");
                let crate::RenderSemanticModel::TreeView(render_model) = full.model() else {
                    panic!("expected typed TreeView render model");
                };
                assert_eq!(
                    serde_json::to_value(&model).unwrap(),
                    serde_json::to_value(render_model).unwrap()
                );
            }
            #[cfg(feature = "compat-json")]
            {
                let metadata = SourcePresentationMetadata {
                    diagram_type: "treeView".into(),
                    config: crate::MermaidConfig::default(),
                    effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
                    title: None,
                };
                let compat = parse_tree_view(&source, &metadata).expect("compat TreeView tokens");
                let typed = parse_tree_view_model_for_render(&source, &metadata)
                    .expect("family TreeView render parse");
                assert_eq!(compat["root"], serde_json::to_value(&model.root).unwrap());
                assert_eq!(
                    serde_json::to_value(&model).unwrap(),
                    serde_json::to_value(typed).unwrap()
                );
            }
        }
    }

    #[test]
    fn canonical_capture_preserves_spanless_model_build_recovery() {
        let mut source = String::from("treeView-beta\n");
        for depth in 0..=MAX_DIAGRAM_NESTING_DEPTH {
            source.push_str(&" ".repeat(depth));
            source.push_str(&format!("\"n{depth}\"\n"));
        }
        let metadata = crate::SemanticMetadata {
            diagram_type: "treeView".into(),
            config: crate::MermaidConfig::empty_object(),
            semantic_config: crate::MermaidConfig::empty_object(),
            title: None,
        };
        crate::diagrams::langium_common::reset_family_syntax_construction_count("treeView");
        let parsed = parse_tree_view_semantic_model_controlled(
            &source,
            &metadata,
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Capture,
            &crate::OperationControl::new(),
        )
        .unwrap();
        assert!(
            parsed
                .model
                .unwrap_err()
                .to_string()
                .contains("nesting depth exceeds")
        );
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("treeView"),
            1
        );
        #[cfg(feature = "editor-facts")]
        {
            let facts = parsed.editor_facts.expect("requested facts");
            assert_eq!(
                facts.completeness,
                crate::EditorSemanticCompleteness::Recovered
            );
            assert_eq!(facts.diagnostics.len(), 1);
            assert_eq!(facts.diagnostics[0].span, None);
            assert!(
                facts.diagnostics[0]
                    .message
                    .contains("nesting depth exceeds")
            );
        }
    }

    #[test]
    fn canonical_tree_keeps_authored_attributes_without_editor_work() {
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        crate::diagrams::langium_common::reset_family_syntax_construction_count("treeView");
        let source = "treeView-beta\ntitle Files\nsrc/ :::important icon(folder) ## source directory\n    main.rs\n";
        let parsed = crate::SemanticParser::new()
            .parse_diagram_sync(source)
            .unwrap();
        assert_eq!(parsed.metadata.diagram_type, "treeView");
        let model = match parsed.model {
            crate::SemanticDiagramModel::TreeView(model) => model,
            #[allow(unreachable_patterns)]
            _ => panic!("expected TreeView semantics"),
        };
        assert_eq!(model.title.as_deref(), Some("Files"));
        let directory = &model.root.children[0];
        assert_eq!(directory.name, "src");
        assert_eq!(directory.node_type, "directory");
        assert_eq!(directory.css_class.as_deref(), Some("important"));
        assert_eq!(directory.icon.as_deref(), Some("folder"));
        assert_eq!(directory.description.as_deref(), Some("source directory"));
        assert_eq!(directory.children[0].name, "main.rs");
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("treeView"),
            1
        );

        #[cfg(feature = "presentation")]
        {
            let full = crate::MermaidEngine::new()
                .parse_diagram_for_render_model_with_type_sync(
                    "treeView",
                    source,
                    crate::ParseOptions::strict(),
                )
                .unwrap()
                .unwrap();
            let crate::RenderSemanticModel::TreeView(full) = full.model() else {
                panic!("expected full TreeView model");
            };
            assert_eq!(
                serde_json::to_value(&model).unwrap(),
                serde_json::to_value(full).unwrap()
            );
            #[cfg(feature = "editor-facts")]
            assert_eq!(crate::editor::test_work::read(), Default::default());
        }
    }

    #[test]
    fn canonical_tree_preserves_strict_diagnostics_and_cancellation() {
        let source = "treeView-beta\nroot\nfile icon(invalid icon)\n";
        let error = crate::SemanticParser::new()
            .parse_diagram_sync(source)
            .unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected TreeView parse diagnostic");
        };
        let start = source.find("file icon").unwrap();
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(start, source.len() - 1))
        );
        let control = crate::OperationControl::new();
        control.cancel();
        let error = crate::SemanticParser::new()
            .parse_diagram_controlled_sync(source, &control)
            .unwrap_err();
        assert_eq!(error.reason, crate::CancelReason::Requested);
    }
}

#[cfg(all(test, feature = "compat-json"))]
mod tests {
    use super::*;
    #[cfg(feature = "editor-facts")]
    use crate::EditorExpectedSyntaxKind;
    #[cfg(feature = "editor-facts")]
    use crate::MermaidEngine;
    use crate::{MermaidConfig, SourcePresentationMetadata, SourceSpan};

    fn meta() -> SourcePresentationMetadata {
        SourcePresentationMetadata {
            diagram_type: "treeView".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
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
    fn percent_payloads_and_hidden_comments_follow_tree_view_token_boundaries() {
        let source = concat!(
            "treeView-beta%% header comment\n",
            "file%%name.txt\n",
            "file.txt %% literal suffix\n",
            "%% full-line comment\n",
            "\"file%%name.txt\"%% hidden quoted comment\n",
            "class.txt :::highlight%% hidden class comment\n",
            "icon.txt icon(file) %% hidden icon comment\n",
            "desc.txt ## progress100%%complete\n",
            "\"continued\"%% hidden before annotation\n",
            "  :::highlight icon() ## final%%text\n",
        );
        let semantic = parse_tree_view(source, &meta()).expect("terminal-aware comments");
        let nodes = semantic["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 8);
        assert_eq!(nodes[1]["name"], "file%%name.txt");
        assert_eq!(nodes[2]["name"], "file.txt %% literal suffix");
        assert_eq!(nodes[3]["name"], "file%%name.txt");
        assert_eq!(nodes[4]["name"], "class.txt");
        assert_eq!(nodes[4]["cssClass"], "highlight");
        assert_eq!(nodes[5]["name"], "icon.txt");
        assert_eq!(nodes[5]["icon"], "file");
        assert_eq!(nodes[6]["description"], "progress100%%complete");
        assert_eq!(nodes[7]["name"], "continued");
        assert_eq!(nodes[7]["cssClass"], "highlight");
        assert_eq!(nodes[7]["icon"], "none");
        assert_eq!(nodes[7]["description"], "final%%text");

        #[cfg(feature = "editor-facts")]
        {
            let (combined, facts) =
                crate::family::test_support::into_result(parse_tree_view_json_and_editor_facts(
                    source,
                    &meta(),
                    &crate::OperationControl::new(),
                ))
                .unwrap();
            assert_eq!(combined, semantic);
            assert_eq!(
                facts.completeness,
                crate::EditorSemanticCompleteness::Complete
            );
            assert!(facts.diagnostics.is_empty());
            for payload in [
                "file.txt %% literal suffix",
                "progress100%%complete",
                "final%%text",
            ] {
                let start = source.find(payload).unwrap();
                assert!(facts.symbols.iter().any(|symbol| {
                    symbol.name == payload
                        && symbol.selection == SourceSpan::new(start, start + payload.len())
                }));
            }
            assert!(
                facts
                    .symbols
                    .iter()
                    .all(|symbol| !symbol.name.contains("hidden"))
            );
        }
    }

    #[test]
    fn multiline_quoted_names_keep_normalized_values_and_authored_fact_ranges() {
        for newline in ["\n", "\r\n"] {
            for quote in ['"', '\''] {
                let source = format!(
                    "treeView-beta{newline}{quote}alpha{newline}title is name{newline}beta/{quote} :::highlight icon(file) ## progress100%%complete{newline}after.txt{newline}"
                );
                let semantic = parse_tree_view(&source, &meta()).expect("multiline quoted name");
                let node = &semantic["nodes"][1];
                assert_eq!(semantic["nodes"].as_array().unwrap().len(), 3);
                assert_eq!(node["name"], "alpha\ntitle is name\nbeta");
                assert_eq!(node["nodeType"], "directory");
                assert_eq!(node["cssClass"], "highlight");
                assert_eq!(node["icon"], "file");
                assert_eq!(node["description"], "progress100%%complete");
                assert_eq!(semantic["nodes"][2]["name"], "after.txt");

                #[cfg(feature = "editor-facts")]
                {
                    let (combined, family_facts) = crate::family::test_support::into_result(
                        parse_tree_view_json_and_editor_facts(
                            &source,
                            &meta(),
                            &crate::OperationControl::new(),
                        ),
                    )
                    .unwrap();
                    assert_eq!(combined, semantic);
                    let public_facts = MermaidEngine::new()
                        .parse_editor_semantic_facts_with_type_sync("treeView", &source)
                        .unwrap()
                        .unwrap();
                    for facts in [family_facts, public_facts] {
                        assert_eq!(
                            facts.completeness,
                            crate::EditorSemanticCompleteness::Complete
                        );
                        assert!(facts.diagnostics.is_empty());
                        let start = source.find("alpha").unwrap();
                        let end = source.find("beta/").unwrap() + "beta".len();
                        let selection = SourceSpan::new(start, end);
                        let symbol = facts
                            .symbols
                            .iter()
                            .find(|symbol| symbol.name == "alpha\ntitle is name\nbeta")
                            .expect("one multiline directory symbol");
                        assert_eq!(symbol.selection, selection);
                        assert_eq!(
                            &source[selection.start..selection.end],
                            format!("alpha{newline}title is name{newline}beta")
                        );
                        assert!(facts.expected_syntax.iter().any(|expected| {
                            expected.kind == EditorExpectedSyntaxKind::NodeIdentifier
                                && expected.span == selection
                        }));
                        for payload in ["highlight", "progress100%%complete", "after.txt"] {
                            let start = source.find(payload).unwrap();
                            assert!(facts.symbols.iter().any(|symbol| {
                                symbol.name == payload
                                    && symbol.selection
                                        == SourceSpan::new(start, start + payload.len())
                            }));
                        }
                        let start = source.find("icon(file)").unwrap() + "icon(".len();
                        assert!(facts.symbols.iter().any(|symbol| {
                            symbol.name == "file"
                                && symbol.selection == SourceSpan::new(start, start + "file".len())
                        }));
                    }
                }
            }
        }
    }

    #[test]
    fn multiline_box_names_map_expanded_tabs_and_annotations_to_raw_source() {
        for newline in ["\n", "\r\n"] {
            let source = [
                "treeView-beta",
                "root/",
                "├── \"alpha\tone",
                "│   └── βeta\tend\" :::highlight icon(file) ## progress\t100%%complete",
                "└── after.txt icon()",
                "",
            ]
            .join(newline);
            let semantic = parse_tree_view(&source, &meta()).expect("multiline box name");
            let nodes = semantic["nodes"].as_array().unwrap();
            assert_eq!(nodes.len(), 4);
            assert_eq!(nodes[2]["name"], "alpha    one\n        βeta    end");
            assert_eq!(nodes[2]["level"], 4);
            assert_eq!(nodes[2]["cssClass"], "highlight");
            assert_eq!(nodes[2]["icon"], "file");
            assert_eq!(nodes[2]["description"], "progress    100%%complete");
            assert_eq!(nodes[3]["name"], "after.txt");
            assert_eq!(nodes[3]["icon"], "none");

            #[cfg(feature = "editor-facts")]
            {
                let (combined, family_facts) = crate::family::test_support::into_result(
                    parse_tree_view_json_and_editor_facts(
                        &source,
                        &meta(),
                        &crate::OperationControl::new(),
                    ),
                )
                .unwrap();
                assert_eq!(combined, semantic);
                let public_facts = MermaidEngine::new()
                    .parse_editor_semantic_facts_with_type_sync("treeView", &source)
                    .unwrap()
                    .unwrap();
                for facts in [family_facts, public_facts] {
                    assert_eq!(
                        facts.completeness,
                        crate::EditorSemanticCompleteness::Complete
                    );
                    assert!(facts.diagnostics.is_empty());
                    let start = source.find("alpha").unwrap();
                    let end = source.find("end\"").unwrap() + "end".len();
                    assert!(facts.symbols.iter().any(|symbol| {
                        symbol.name == "alpha    one\n        βeta    end"
                            && symbol.selection == SourceSpan::new(start, end)
                    }));
                    for (raw, value) in [
                        ("highlight", "highlight"),
                        ("progress\t100%%complete", "progress    100%%complete"),
                        ("after.txt", "after.txt"),
                    ] {
                        let start = source.find(raw).unwrap();
                        assert!(facts.symbols.iter().any(|symbol| {
                            symbol.name == value
                                && symbol.selection == SourceSpan::new(start, start + raw.len())
                        }));
                    }
                    let start = source.find("icon(file)").unwrap() + "icon(".len();
                    assert!(facts.symbols.iter().any(|symbol| {
                        symbol.name == "file"
                            && symbol.selection == SourceSpan::new(start, start + "file".len())
                    }));
                }
            }
        }
    }

    #[test]
    fn unmatched_quoted_name_preserves_later_nodes_and_the_authored_error_range() {
        for newline in ["\n", "\r\n"] {
            let source = [
                "treeView-beta",
                "root/",
                "  \"unfinished",
                "  after.txt :::ready icon(file) ## retained%%payload",
                "next.txt",
                "",
            ]
            .join(newline);
            let error = parse_tree_view(&source, &meta()).expect_err("truly unmatched quote");
            assert!(
                error
                    .to_string()
                    .contains("unterminated quoted tree node name")
            );
            let start = source.find("\"unfinished").unwrap();
            let span = SourceSpan::new(start, start + "\"unfinished".len());
            let Error::DiagramParse { diagnostic, .. } = error else {
                panic!("expected a TreeView diagnostic");
            };
            assert_eq!(diagnostic.span(), Some(span));
            let error = crate::SemanticParser::new()
                .parse_diagram_sync(&source)
                .expect_err("canonical unmatched quote");
            let Error::DiagramParse { diagnostic, .. } = error else {
                panic!("expected a canonical TreeView diagnostic");
            };
            assert_eq!(diagnostic.span(), Some(span));

            #[cfg(feature = "editor-facts")]
            {
                let facts = crate::family::test_support::editor_facts(
                    parse_tree_view_json_and_editor_facts,
                    &source,
                    &meta(),
                );
                assert_eq!(
                    facts.completeness,
                    crate::EditorSemanticCompleteness::Recovered
                );
                assert!(facts.diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .message
                        .contains("unterminated quoted tree node name")
                        && diagnostic.span == Some(span)
                }));
                for payload in [
                    "root",
                    "after.txt",
                    "ready",
                    "file",
                    "retained%%payload",
                    "next.txt",
                ] {
                    assert!(facts.symbols.iter().any(|symbol| symbol.name == payload));
                }
                assert!(
                    facts
                        .symbols
                        .iter()
                        .all(|symbol| !symbol.name.contains("unfinished"))
                );
            }
        }
    }

    #[test]
    fn sanitizes_descriptions_before_projecting_tree_view_semantics() {
        let mut meta = meta();
        meta.effective_config = crate::presentation::SourcePresentationConfig::from_value(json!({
            "securityLevel": "strict"
        }));
        let source = "treeView-beta\nfile.txt ## <style>.bad{display:none}</style><b>safe</b>\n";

        let semantic = parse_tree_view(source, &meta).unwrap();
        let typed = parse_tree_view_model_for_render(source, &meta).unwrap();

        assert_eq!(semantic["nodes"][1]["description"], json!("<b>safe</b>"));
        assert_eq!(
            typed.root.children[0].description.as_deref(),
            Some("<b>safe</b>")
        );
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
    fn non_breaking_space_does_not_start_an_annotation() {
        let semantic = parse_tree_view(
            "treeView-beta\nclass\u{a0}:::literal\nicon\u{a0}icon(literal)\ndesc\u{a0}## literal\n",
            &meta(),
        )
        .unwrap();
        let nodes = semantic["nodes"].as_array().expect("nodes array");

        assert_eq!(nodes[1]["name"], json!("class\u{a0}:::literal"));
        assert_eq!(nodes[2]["name"], json!("icon\u{a0}icon(literal)"));
        assert_eq!(nodes[3]["name"], json!("desc\u{a0}## literal"));
        for node in &nodes[1..] {
            assert!(node.get("cssClass").is_none());
            assert!(node.get("icon").is_none());
            assert!(node.get("description").is_none());
        }
    }

    #[test]
    fn class_annotation_uses_the_upstream_langium_identifier_terminal() {
        for class_name in ["a", "_", "Z9_-", "_ready-2"] {
            let input = format!("treeView-beta\nfile ::: {class_name}\n");
            let semantic = parse_tree_view(&input, &meta()).expect("valid class must parse");
            assert_eq!(semantic["nodes"][1]["name"], json!("file"));
            assert_eq!(semantic["nodes"][1]["cssClass"], json!(class_name));
        }

        for rejected_class in ["9bad", "-bad", "éclair"] {
            let input = format!("treeView-beta\nfile :::{rejected_class}\n");
            let semantic = parse_tree_view(&input, &meta())
                .expect("an invalid class marker remains part of the bare name upstream");
            assert_eq!(
                semantic["nodes"][1]["name"],
                json!(format!("file :::{rejected_class}"))
            );
            assert!(semantic["nodes"][1].get("cssClass").is_none());
        }
    }

    #[test]
    fn icon_annotation_uses_the_upstream_iconify_reference_terminal() {
        for (payload, expected) in [
            ("", "none"),
            ("foo", "foo"),
            ("-", "-"),
            (":valid-name", ":valid-name"),
            ("pack:icon", "pack:icon"),
            ("_-:9-a", "_-:9-a"),
        ] {
            let input = format!("treeView-beta\nfile icon({payload})\n");
            let semantic = parse_tree_view(&input, &meta()).expect("valid icon must parse");
            assert_eq!(semantic["nodes"][1]["icon"], json!(expected));
        }

        for payload in [
            "foo bar",
            "foo:",
            "foo:bar:baz",
            "foo::bar",
            "foo/bar",
            "emoji:face🙂",
        ] {
            let input = format!("treeView-beta\nfile icon({payload})\n");
            let error = parse_tree_view(&input, &meta()).expect_err("invalid icon must fail");
            assert!(
                error
                    .to_string()
                    .contains("invalid tree node icon annotation"),
                "{payload}: {error}"
            );
        }
    }

    #[test]
    fn common_metadata_after_a_node_is_rejected_and_not_reinterpreted_as_a_node() {
        for late_metadata in [
            "title Late\n",
            "accTitle: Late\n",
            "accDescr: Late\n",
            "accDescr {\nLate\n}\n",
        ] {
            let source = format!("treeView-beta\nroot\n{late_metadata}after\n");
            let error =
                parse_tree_view(&source, &meta()).expect_err("late common metadata must fail");
            assert!(
                error.to_string().contains("must precede every node"),
                "{late_metadata:?}: {error}"
            );

            #[cfg(feature = "editor-facts")]
            {
                let facts = crate::family::test_support::editor_facts(
                    parse_tree_view_json_and_editor_facts,
                    &source,
                    &meta(),
                );
                assert!(
                    facts
                        .diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.message.contains("must precede every node")),
                    "{late_metadata:?}: {:?}",
                    facts.diagnostics
                );
                assert!(facts.symbols.iter().any(|symbol| symbol.name == "root"));
                assert!(facts.symbols.iter().any(|symbol| symbol.name == "after"));
                assert!(
                    !facts
                        .symbols
                        .iter()
                        .any(|symbol| symbol.name.contains("Late"))
                );
            }
        }
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
    fn box_drawing_preprocessing_expands_tabs_across_the_branch_line() {
        let semantic = parse_tree_view(
            "treeView-beta\nroot/\n├── feature\tflag ## alpha\tbeta\n",
            &meta(),
        )
        .unwrap();
        let node = &semantic["nodes"][2];

        assert_eq!(node["name"], json!("feature    flag"));
        assert_eq!(node["description"], json!("alpha    beta"));
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
    #[cfg(feature = "editor-facts")]
    fn parse_tree_view_editor_facts_expose_parser_backed_spans() {
        let engine = MermaidEngine::new();
        let text = r#"
treeView-beta
title My Tree
accTitle: Accessible Title
accDescr: Accessible Description
"Root"
  "Child 1"
"#;
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("treeView", text)
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
    #[cfg(feature = "editor-facts")]
    fn parse_tree_view_editor_facts_preserve_box_drawing_annotation_spans() {
        let engine = MermaidEngine::new();
        let text = r#"
treeView-beta
├── App.tsx :::highlight icon(react) ## main component
"#;
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("treeView", text)
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

    #[test]
    #[cfg(feature = "editor-facts")]
    fn tree_view_combined_parse_constructs_source_once_and_preserves_projections() {
        let text = concat!(
            "treeView-beta\n",
            "title Project Tree\n",
            "accTitle: Project files\n",
            "accDescr: Source hierarchy\n",
            "src/ :::highlight icon(folder) ## source directory\n",
            "  App.tsx icon(react)\n",
        );
        let meta = meta();

        crate::diagrams::langium_common::reset_family_syntax_construction_count("treeView");
        let (combined_json, combined_editor) = crate::family::test_support::into_result(
            parse_tree_view_json_and_editor_facts(text, &meta, &crate::OperationControl::new()),
        )
        .unwrap();
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("treeView"),
            1,
            "one combined request must construct TreeView syntax once"
        );

        let standalone_json = parse_tree_view(text, &meta).unwrap();
        assert_eq!(combined_json, standalone_json);
        assert!(!combined_editor.symbols.is_empty());
    }

    #[test]
    fn tree_view_typed_and_json_projections_share_the_same_semantics() {
        let text = concat!(
            "treeView-beta\n",
            "title Project Tree\n",
            "root/ :::root icon(folder) ## project root\n",
            "  child.ts icon(typescript)\n",
        );
        let meta = meta();

        let compat = parse_tree_view(text, &meta).unwrap();
        let typed = parse_tree_view_model_for_render(text, &meta).unwrap();

        assert_eq!(render_model_to_compat_json(&typed, &meta).unwrap(), compat);
        assert_eq!(compat["title"], json!(typed.title));
        assert_eq!(compat["accTitle"], json!(typed.acc_title));
        assert_eq!(compat["accDescr"], json!(typed.acc_descr));
        assert_eq!(compat["root"], serde_json::to_value(&typed.root).unwrap());
    }

    #[test]
    fn tree_view_incomplete_node_recovers_prior_facts_with_exact_error_span() {
        let text = "treeView-beta\nroot/\n  \"unfinished\n";
        let meta = meta();

        let error = parse_tree_view(text, &meta).expect_err("strict parse must reject the node");
        assert!(
            error
                .to_string()
                .contains("unterminated quoted tree node name")
        );
        #[cfg(feature = "editor-facts")]
        {
            let facts = crate::family::test_support::editor_facts(
                parse_tree_view_json_and_editor_facts,
                text,
                &meta,
            );

            assert_eq!(
                facts.completeness,
                crate::EditorSemanticCompleteness::Recovered
            );
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "root"));
            let diagnostic = facts
                .diagnostics
                .iter()
                .find(|diagnostic| {
                    diagnostic
                        .message
                        .contains("unterminated quoted tree node name")
                })
                .expect("recovery diagnostic");
            assert!(
                diagnostic
                    .message
                    .contains("unterminated quoted tree node name")
            );
            let start = text.find("\"unfinished").unwrap();
            assert_eq!(
                diagnostic.span,
                Some(SourceSpan::new(start, start + "\"unfinished".len()))
            );
        }
    }

    #[test]
    fn tree_view_recovery_keeps_partial_and_later_semantic_facts_with_crlf_spans() {
        let text = concat!(
            "treeView-beta\r\n",
            "root/\r\n",
            "  broken.txt icon(unclosed\r\n",
            "  later.txt :::ready icon(file)\r\n",
        );
        let meta = meta();

        let error =
            parse_tree_view(text, &meta).expect_err("strict parse must stop at first error");
        assert!(
            error
                .to_string()
                .contains("unterminated tree node icon annotation"),
            "{error}"
        );

        #[cfg(feature = "editor-facts")]
        {
            let facts = crate::family::test_support::editor_facts(
                parse_tree_view_json_and_editor_facts,
                text,
                &meta,
            );
            assert_eq!(
                facts.completeness,
                crate::EditorSemanticCompleteness::Recovered
            );
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "root"));
            assert!(
                facts
                    .symbols
                    .iter()
                    .any(|symbol| symbol.name == "later.txt")
            );
            assert!(
                !facts
                    .symbols
                    .iter()
                    .any(|symbol| symbol.name == "broken.txt")
            );

            let malformed_line = "broken.txt icon(unclosed";
            let malformed_start = text.find(malformed_line).unwrap();
            assert!(facts.diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("unterminated tree node icon annotation")
                    && diagnostic.span
                        == Some(SourceSpan::new(
                            malformed_start,
                            malformed_start + malformed_line.len(),
                        ))
            }));
        }
    }

    #[test]
    fn tree_view_strict_projections_report_the_first_recoverable_issue() {
        let text = concat!(
            "treeView-beta\n",
            "first.txt icon(unclosed\n",
            "\"second.txt\n",
            "after.txt\n",
        );
        let meta = meta();

        let errors = [
            parse_tree_view(text, &meta).unwrap_err(),
            #[cfg(feature = "editor-facts")]
            {
                crate::family::test_support::into_result(parse_tree_view_json_and_editor_facts(
                    text,
                    &meta,
                    &crate::OperationControl::new(),
                ))
                .unwrap_err()
            },
            parse_tree_view_model_for_render(text, &meta).unwrap_err(),
        ];
        for error in errors {
            let message = error.to_string();
            assert!(
                message.contains("unterminated tree node icon annotation"),
                "{message}"
            );
            assert!(!message.contains("unterminated quoted tree node name"));
        }

        #[cfg(feature = "editor-facts")]
        {
            let facts = crate::family::test_support::editor_facts(
                parse_tree_view_json_and_editor_facts,
                text,
                &meta,
            );
            assert!(facts.diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("unterminated quoted tree node name")
            }));
            assert!(
                facts
                    .symbols
                    .iter()
                    .any(|symbol| symbol.name == "after.txt")
            );
        }
    }

    #[test]
    fn tree_view_rejects_tokens_after_header_with_compatible_diagnostic() {
        let error = parse_tree_view("treeView-beta junk\nroot\n", &meta()).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("unexpected tokens after treeView-beta")
        );
    }

    #[test]
    fn tree_view_multiline_acc_descr_uses_common_syntax_and_recovers_when_unterminated() {
        let complete = "treeView-beta\naccDescr {\nline one\nline two\n}\nroot/\n";
        let meta = meta();
        #[cfg(feature = "editor-facts")]
        let (json, facts) = crate::family::test_support::into_result(
            parse_tree_view_json_and_editor_facts(complete, &meta, &crate::OperationControl::new()),
        )
        .unwrap();
        #[cfg(not(feature = "editor-facts"))]
        let json = parse_tree_view(complete, &meta).unwrap();
        assert_eq!(json["accDescr"], json!("line one\nline two"));
        #[cfg(feature = "editor-facts")]
        {
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
        }
        let incomplete = "treeView-beta\naccDescr {\nline one\n";
        assert!(parse_tree_view(incomplete, &meta).is_err());
        #[cfg(feature = "editor-facts")]
        {
            let recovered = crate::family::test_support::editor_facts(
                parse_tree_view_json_and_editor_facts,
                incomplete,
                &meta,
            );
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
}
