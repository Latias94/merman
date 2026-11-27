//! Tree View syntax, source-relative spans, and bounded tree construction.

#[cfg(feature = "editor-facts")]
use merman_core_preprocess::scan::langium::LangiumCommonFact;
use merman_core_preprocess::scan::{
    langium::{LangiumCommonField, parse_langium_common},
    physical_line_at,
};
use merman_markup::sanitize::sanitize_text;
use merman_model::families::tree_view::{
    TREE_VIEW_DIRECTORY_NODE_TYPE, TREE_VIEW_FILE_NODE_TYPE, TreeViewDiagramModel, TreeViewNode,
};
use merman_model::{
    MAX_DIAGRAM_NESTING_DEPTH, MermaidConfig, OperationControl, OperationControlResult, SourceSpan,
};

#[cfg(test)]
thread_local! {
    static TREE_VIEW_BODY_LINE_SCAN_BYTES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[derive(Debug)]
pub struct ParsedTreeView {
    title: Option<String>,
    acc_title: Option<String>,
    acc_descr: Option<String>,
    nodes: Vec<TreeViewNodeDetails>,
}

#[derive(Debug, Clone)]
pub struct TreeViewParseIssue {
    pub message: String,
    pub span: Option<SourceSpan>,
}

impl TreeViewParseIssue {
    fn new(message: impl Into<String>, span: Option<SourceSpan>) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

#[cfg(feature = "editor-facts")]
pub enum TreeViewObservation<'source> {
    Common(&'source LangiumCommonFact),
    Issue(&'source TreeViewParseIssue),
    Node {
        name: &'source str,
        span: SourceSpan,
        selection: SourceSpan,
    },
    Annotation {
        kind: TreeViewAnnotationKind,
        text: &'source str,
        span: SourceSpan,
    },
}

#[cfg(feature = "editor-facts")]
pub enum TreeViewAnnotationKind {
    Class,
    Icon,
    Description,
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

/// Parses one preprocessed source after the caller's entry checkpoint.
pub fn parse_tree_view_source_controlled(
    code: &str,
    control: &OperationControl,
) -> OperationControlResult<Result<ParsedTreeView, TreeViewParseIssue>> {
    parse_tree_view_source(
        code,
        #[cfg(feature = "editor-facts")]
        false,
        #[cfg(feature = "editor-facts")]
        None,
        control,
    )
}

/// Parses one source and streams recovery and optional rich observations in parse order.
/// All spans refer to the supplied source before Tree View normalizes its box drawing.
#[cfg(feature = "editor-facts")]
pub fn parse_tree_view_source_with_observer_controlled(
    code: &str,
    capture_fields: bool,
    observe: &mut dyn FnMut(TreeViewObservation<'_>),
    control: &OperationControl,
) -> OperationControlResult<Result<ParsedTreeView, TreeViewParseIssue>> {
    parse_tree_view_source(code, capture_fields, Some(observe), control)
}

fn parse_tree_view_source(
    code: &str,
    #[cfg(feature = "editor-facts")] capture_fields: bool,
    #[cfg(feature = "editor-facts")] mut observe: Option<&mut dyn FnMut(TreeViewObservation<'_>)>,
    control: &OperationControl,
) -> OperationControlResult<Result<ParsedTreeView, TreeViewParseIssue>> {
    let mut first_issue = None;
    let body = match tree_view_body_start_controlled(code, control)? {
        Ok(body) => body,
        Err(issue) => {
            #[cfg(feature = "editor-facts")]
            if let Some(observe) = &mut observe {
                observe(TreeViewObservation::Issue(&issue));
            }
            return Ok(Err(issue));
        }
    };
    let (source, issues) = preprocess_tree_view_body(code, body.offset, control)?;
    for issue in issues {
        mark_tree_view_recovery(
            #[cfg(feature = "editor-facts")]
            &mut observe,
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
                    &mut observe,
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
            if let Some(observe) = observe.as_deref_mut().filter(|_| capture_fields) {
                let mut fact = parsed.fact.clone();
                fact.raw_span = source.source_span(fact.raw_span);
                fact.value_span = source.source_span(fact.value_span);
                observe(TreeViewObservation::Common(&fact));
            }
            match field {
                LangiumCommonField::Title => title = Some(value),
                LangiumCommonField::AccTitle => acc_title = Some(value),
                LangiumCommonField::AccDescr => acc_descr = Some(value),
            }
            if let Some(diagnostic) = parsed.diagnostic {
                mark_tree_view_recovery(
                    #[cfg(feature = "editor-facts")]
                    &mut observe,
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
                    if let Some(observe) = observe.as_deref_mut().filter(|_| capture_fields) {
                        observe_tree_view_node(observe, &node);
                    }
                }
                nodes.push(node);
            }
            Err(mut issue) => {
                issue.span = issue.span.map(|span| source.source_span(span));
                mark_tree_view_recovery(
                    #[cfg(feature = "editor-facts")]
                    &mut observe,
                    &mut first_issue,
                    issue,
                );
            }
        }
    }

    control.checkpoint()?;
    Ok(match first_issue {
        Some(issue) => Err(issue),
        None => Ok(ParsedTreeView {
            title,
            acc_title,
            acc_descr,
            nodes,
        }),
    })
}

#[derive(Debug, Clone, Copy)]
struct TreeViewBodyStart {
    offset: usize,
}

fn tree_view_body_start_controlled(
    code: &str,
    control: &OperationControl,
) -> OperationControlResult<Result<TreeViewBodyStart, TreeViewParseIssue>> {
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
    #[cfg(feature = "editor-facts")] observe: &mut Option<&mut dyn FnMut(TreeViewObservation<'_>)>,
    first_issue: &mut Option<TreeViewParseIssue>,
    issue: TreeViewParseIssue,
) {
    if first_issue.is_none() {
        *first_issue = Some(issue.clone());
    }
    #[cfg(feature = "editor-facts")]
    if let Some(observe) = observe {
        observe(TreeViewObservation::Issue(&issue));
    }
}

impl ParsedTreeView {
    /// Builds the ordered tree after the complete statement pass has succeeded.
    pub fn into_model(self, config: &MermaidConfig) -> Result<TreeViewDiagramModel, String> {
        let Self {
            title,
            acc_title,
            acc_descr,
            nodes,
        } = self;
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
                    .map(|value| sanitize_text(&value.value, config)),
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
    control: &OperationControl,
) -> OperationControlResult<(TreeViewSource, Vec<TreeViewParseIssue>)> {
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
            match tree_view_line_view(line, line_format) {
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
                        error,
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
) -> Result<Option<TreeViewLineView<'a>>, &'static str> {
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
            return Err("empty tree node after box-drawing prefix");
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
        return Err(
            "unexpected indentation without box-drawing prefix in treeView box-drawing input",
        );
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
    control: &OperationControl,
) -> OperationControlResult<(Result<TreeViewNodeDetails, TreeViewParseIssue>, usize)> {
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
) -> (Result<TreeViewNodeDetails, TreeViewParseIssue>, usize) {
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
fn observe_tree_view_node(
    observe: &mut dyn FnMut(TreeViewObservation<'_>),
    node: &TreeViewNodeDetails,
) {
    observe(TreeViewObservation::Node {
        name: &node.name,
        span: node.span,
        selection: node.selection,
    });
    for (value, kind) in [
        (node.css_class.as_ref(), TreeViewAnnotationKind::Class),
        (node.icon.as_ref(), TreeViewAnnotationKind::Icon),
        (
            node.description.as_ref(),
            TreeViewAnnotationKind::Description,
        ),
    ] {
        if let Some(value) = value
            && !value.value.is_empty()
            && value.span.start != value.span.end
        {
            observe(TreeViewObservation::Annotation {
                kind,
                text: &value.value,
                span: value.span,
            });
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

fn split_ascii_indent(line: &str) -> (usize, &str) {
    let rest = line.trim_start_matches([' ', '\t']);
    (line.len() - rest.len(), rest)
}

#[cfg(test)]
mod tests;
