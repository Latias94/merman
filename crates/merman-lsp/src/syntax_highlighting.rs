use merman_analysis::AnalysisCancellationToken;
use merman_editor_core::DocumentKind;
use std::ops::{ControlFlow, Range};
use std::sync::{Arc, OnceLock};
use tree_sitter::{
    InputEdit, Language, ParseOptions, Parser, Point, Query, QueryCursor, QueryCursorOptions,
    StreamingIterator, Tree,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub(crate) enum SyntaxTokenKind {
    Comment,
    String,
    Number,
    Keyword,
    Operator,
    Namespace,
    Function,
    Variable,
    Property,
    Type,
    Macro,
    Decorator,
    EnumMember,
}

impl SyntaxTokenKind {
    pub(crate) const ALL: [Self; 13] = [
        Self::Comment,
        Self::String,
        Self::Number,
        Self::Keyword,
        Self::Operator,
        Self::Namespace,
        Self::Function,
        Self::Variable,
        Self::Property,
        Self::Type,
        Self::Macro,
        Self::Decorator,
        Self::EnumMember,
    ];

    pub(crate) const COUNT: usize = Self::ALL.len();

    pub(crate) const fn lsp_name(self) -> &'static str {
        match self {
            Self::Comment => "comment",
            Self::String => "string",
            Self::Number => "number",
            Self::Keyword => "keyword",
            Self::Operator => "operator",
            Self::Namespace => "namespace",
            Self::Function => "function",
            Self::Variable => "variable",
            Self::Property => "property",
            Self::Type => "type",
            Self::Macro => "macro",
            Self::Decorator => "decorator",
            Self::EnumMember => "enumMember",
        }
    }

    pub(crate) const fn index(self) -> usize {
        self as usize
    }

    fn from_capture(mut name: &str) -> Option<Self> {
        loop {
            let kind = match name {
                "namespace" => Some(Self::Namespace),
                "type" => Some(Self::Type),
                "boolean" | "constant" => Some(Self::EnumMember),
                "variable" => Some(Self::Variable),
                "property" | "variable.member" => Some(Self::Property),
                "function" => Some(Self::Function),
                "function.macro" => Some(Self::Macro),
                "keyword" => Some(Self::Keyword),
                "comment" => Some(Self::Comment),
                "string" => Some(Self::String),
                "number" => Some(Self::Number),
                "keyword.operator" | "operator" | "punctuation" => Some(Self::Operator),
                "attribute" => Some(Self::Decorator),
                _ => None,
            };
            if kind.is_some() {
                return kind;
            }
            name = name.rsplit_once('.')?.0;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SyntaxCapture {
    pub(crate) start_byte: usize,
    pub(crate) end_byte: usize,
    pub(crate) kind: SyntaxTokenKind,
    pub(crate) specificity: usize,
    pub(crate) pattern_index: usize,
    pub(crate) capture_name_rank: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct SyntaxDocumentState {
    version: i32,
    kind: DocumentKind,
    source: Arc<str>,
    parsed: ParsedDocument,
}

#[derive(Debug, Clone)]
enum ParsedDocument {
    Diagram(Tree),
    Host(Vec<FenceTree>),
}

#[derive(Debug, Clone)]
struct FenceTree {
    body: Range<usize>,
    tree: Tree,
}

struct HighlightQuery {
    query: Query,
    captures: Vec<Option<CaptureMetadata>>,
}

#[derive(Debug, Clone, Copy)]
struct CaptureMetadata {
    kind: SyntaxTokenKind,
    specificity: usize,
    name_rank: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SyntaxHighlightError {
    Cancelled,
    Language(String),
    Query(String),
    ParseFailed,
}

impl std::fmt::Display for SyntaxHighlightError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => formatter.write_str("Tree-sitter syntax work was cancelled"),
            Self::Language(detail) => {
                write!(
                    formatter,
                    "failed to load the Mermaid Tree-sitter language: {detail}"
                )
            }
            Self::Query(detail) => {
                write!(
                    formatter,
                    "failed to compile the Mermaid highlight query: {detail}"
                )
            }
            Self::ParseFailed => formatter.write_str("Tree-sitter did not produce a syntax tree"),
        }
    }
}

impl std::error::Error for SyntaxHighlightError {}

impl SyntaxHighlightError {
    pub(crate) const fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled)
    }
}

impl SyntaxDocumentState {
    pub(crate) fn parse(
        version: i32,
        kind: DocumentKind,
        source: Arc<str>,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<Self, SyntaxHighlightError> {
        let parsed = parse_document(kind, &source, None, cancellation)?;
        Ok(Self {
            version,
            kind,
            source,
            parsed,
        })
    }

    pub(crate) fn update(
        &self,
        version: i32,
        kind: DocumentKind,
        source: Arc<str>,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<Self, SyntaxHighlightError> {
        let previous = (self.kind == kind).then_some(self);
        let parsed = parse_document(kind, &source, previous, cancellation)?;
        Ok(Self {
            version,
            kind,
            source,
            parsed,
        })
    }

    pub(crate) const fn version(&self) -> i32 {
        self.version
    }

    pub(crate) fn source(&self) -> &str {
        &self.source
    }

    pub(crate) fn captures(
        &self,
        requested: Option<Range<usize>>,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<Vec<SyntaxCapture>, SyntaxHighlightError> {
        if cancellation.is_cancelled() {
            return Err(SyntaxHighlightError::Cancelled);
        }
        let query = highlight_query()?;
        let mut captures = Vec::new();
        match &self.parsed {
            ParsedDocument::Diagram(tree) => collect_tree_captures(
                query,
                tree,
                &self.source,
                0,
                requested.as_ref(),
                cancellation,
                &mut captures,
            )?,
            ParsedDocument::Host(fences) => {
                for fence in fences {
                    if requested
                        .as_ref()
                        .is_some_and(|range| !ranges_overlap(range, &fence.body))
                    {
                        continue;
                    }
                    let body = &self.source[fence.body.clone()];
                    collect_tree_captures(
                        query,
                        &fence.tree,
                        body,
                        fence.body.start,
                        requested.as_ref(),
                        cancellation,
                        &mut captures,
                    )?;
                }
            }
        }
        captures.sort_unstable_by_key(|capture| {
            (
                capture.start_byte,
                capture.end_byte,
                capture.specificity,
                capture.pattern_index,
                capture.capture_name_rank,
            )
        });
        Ok(captures)
    }

    #[cfg(test)]
    fn tree_count(&self) -> usize {
        match &self.parsed {
            ParsedDocument::Diagram(_) => 1,
            ParsedDocument::Host(fences) => fences.len(),
        }
    }
}

fn parse_document(
    kind: DocumentKind,
    source: &str,
    previous: Option<&SyntaxDocumentState>,
    cancellation: &AnalysisCancellationToken,
) -> Result<ParsedDocument, SyntaxHighlightError> {
    if cancellation.is_cancelled() {
        return Err(SyntaxHighlightError::Cancelled);
    }
    match kind {
        DocumentKind::Diagram => {
            let mut parser = parser()?;
            let previous_tree = previous.and_then(|state| match &state.parsed {
                ParsedDocument::Diagram(tree) => Some((state.source.as_ref(), tree)),
                ParsedDocument::Host(_) => None,
            });
            parse_tree(source, previous_tree, &mut parser, cancellation)
                .map(ParsedDocument::Diagram)
        }
        DocumentKind::Markdown | DocumentKind::Mdx => {
            let mut parser = parser()?;
            let bodies = scan_mermaid_fences(source);
            let previous_fences = previous.and_then(|state| match &state.parsed {
                ParsedDocument::Host(fences) => Some((state.source.as_ref(), fences.as_slice())),
                ParsedDocument::Diagram(_) => None,
            });
            let mut fences = Vec::with_capacity(bodies.len());
            for (index, body) in bodies.into_iter().enumerate() {
                if cancellation.is_cancelled() {
                    return Err(SyntaxHighlightError::Cancelled);
                }
                let text = &source[body.clone()];
                let previous_tree = previous_fences.and_then(|(previous_source, previous)| {
                    let previous = previous.get(index)?;
                    Some((&previous_source[previous.body.clone()], &previous.tree))
                });
                fences.push(FenceTree {
                    body,
                    tree: parse_tree(text, previous_tree, &mut parser, cancellation)?,
                });
            }
            Ok(ParsedDocument::Host(fences))
        }
    }
}

fn language() -> Language {
    tree_sitter_mermaid::LANGUAGE.into()
}

fn parser() -> Result<Parser, SyntaxHighlightError> {
    let language = language();
    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .map_err(|error| SyntaxHighlightError::Language(error.to_string()))?;
    Ok(parser)
}

fn parse_tree(
    source: &str,
    previous: Option<(&str, &Tree)>,
    parser: &mut Parser,
    cancellation: &AnalysisCancellationToken,
) -> Result<Tree, SyntaxHighlightError> {
    if let Some((previous_source, previous_tree)) = previous {
        if previous_source == source {
            return Ok(previous_tree.clone());
        }
        let edited = edited_tree(previous_source, source, previous_tree);
        if let Some(tree) = parse_once(source, Some(&edited), parser, cancellation)? {
            return Ok(tree);
        }
    }

    parse_once(source, None, parser, cancellation)?.ok_or(SyntaxHighlightError::ParseFailed)
}

fn parse_once(
    source: &str,
    previous: Option<&Tree>,
    parser: &mut Parser,
    cancellation: &AnalysisCancellationToken,
) -> Result<Option<Tree>, SyntaxHighlightError> {
    if cancellation.is_cancelled() {
        return Err(SyntaxHighlightError::Cancelled);
    }
    let bytes = source.as_bytes();
    let mut read = |offset: usize, _: Point| bytes.get(offset..).unwrap_or_default();
    let mut progress = |_: &tree_sitter::ParseState| {
        if cancellation.is_cancelled() {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    };
    let tree = parser.parse_with_options(
        &mut read,
        previous,
        Some(ParseOptions::new().progress_callback(&mut progress)),
    );
    if cancellation.is_cancelled() {
        Err(SyntaxHighlightError::Cancelled)
    } else {
        Ok(tree)
    }
}

fn edited_tree(previous_source: &str, source: &str, previous_tree: &Tree) -> Tree {
    let edit = minimal_input_edit(previous_source, source);
    let mut tree = previous_tree.clone();
    tree.edit(&edit);
    tree
}

fn minimal_input_edit(previous: &str, current: &str) -> InputEdit {
    let previous_bytes = previous.as_bytes();
    let current_bytes = current.as_bytes();
    let common_len = previous_bytes.len().min(current_bytes.len());
    let mut prefix = previous_bytes
        .iter()
        .zip(current_bytes)
        .take(common_len)
        .take_while(|(left, right)| left == right)
        .count();
    while prefix > 0 && (!previous.is_char_boundary(prefix) || !current.is_char_boundary(prefix)) {
        prefix -= 1;
    }

    let max_suffix = common_len.saturating_sub(prefix);
    let mut suffix = previous_bytes
        .iter()
        .rev()
        .zip(current_bytes.iter().rev())
        .take(max_suffix)
        .take_while(|(left, right)| left == right)
        .count();
    while suffix > 0
        && (!previous.is_char_boundary(previous.len() - suffix)
            || !current.is_char_boundary(current.len() - suffix))
    {
        suffix -= 1;
    }

    let old_end_byte = previous.len() - suffix;
    let new_end_byte = current.len() - suffix;
    InputEdit {
        start_byte: prefix,
        old_end_byte,
        new_end_byte,
        start_position: tree_point(previous, prefix),
        old_end_position: tree_point(previous, old_end_byte),
        new_end_position: tree_point(current, new_end_byte),
    }
}

fn tree_point(source: &str, byte: usize) -> Point {
    let prefix = &source.as_bytes()[..byte];
    let row = prefix.iter().filter(|byte| **byte == b'\n').count();
    let column = prefix
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(prefix.len(), |newline| prefix.len() - newline - 1);
    Point::new(row, column)
}

fn highlight_query() -> Result<&'static HighlightQuery, SyntaxHighlightError> {
    static QUERY: OnceLock<Result<HighlightQuery, String>> = OnceLock::new();
    match QUERY.get_or_init(|| {
        let query = Query::new(&language(), tree_sitter_mermaid::HIGHLIGHTS_QUERY)
            .map_err(|error| error.to_string())?;
        let capture_names = query.capture_names();
        let captures = capture_names
            .iter()
            .map(|name| {
                SyntaxTokenKind::from_capture(name).map(|kind| CaptureMetadata {
                    kind,
                    specificity: name.split('.').count(),
                    name_rank: capture_names
                        .iter()
                        .filter(|candidate| candidate.as_bytes() < name.as_bytes())
                        .count(),
                })
            })
            .collect();
        Ok(HighlightQuery { query, captures })
    }) {
        Ok(query) => Ok(query),
        Err(detail) => Err(SyntaxHighlightError::Query(detail.clone())),
    }
}

fn collect_tree_captures(
    query: &HighlightQuery,
    tree: &Tree,
    source: &str,
    base: usize,
    requested: Option<&Range<usize>>,
    cancellation: &AnalysisCancellationToken,
    output: &mut Vec<SyntaxCapture>,
) -> Result<(), SyntaxHighlightError> {
    let source_range = base..base + source.len();
    let requested = requested
        .map(|range| intersect_ranges(range, &source_range))
        .unwrap_or(Some(source_range));
    let Some(requested) = requested else {
        return Ok(());
    };
    let local_requested = requested.start - base..requested.end - base;

    let mut cursor = QueryCursor::new();
    cursor.set_byte_range(local_requested);
    let mut progress = |_: &tree_sitter::QueryCursorState| {
        if cancellation.is_cancelled() {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    };
    let options = QueryCursorOptions::new().progress_callback(&mut progress);
    let mut captures =
        cursor.captures_with_options(&query.query, tree.root_node(), source.as_bytes(), options);
    while let Some((query_match, capture_index)) = captures.next() {
        let capture = query_match.captures[*capture_index];
        let capture_index = usize::try_from(capture.index).expect("capture index fits usize");
        let Some(metadata) = query.captures.get(capture_index).copied().flatten() else {
            continue;
        };
        let absolute = base + capture.node.start_byte()..base + capture.node.end_byte();
        if ranges_overlap(&absolute, &requested) {
            output.push(SyntaxCapture {
                start_byte: absolute.start,
                end_byte: absolute.end,
                kind: metadata.kind,
                specificity: metadata.specificity,
                pattern_index: query_match.pattern_index,
                capture_name_rank: metadata.name_rank,
            });
        }
    }
    if cancellation.is_cancelled() {
        Err(SyntaxHighlightError::Cancelled)
    } else {
        Ok(())
    }
}

fn ranges_overlap(left: &Range<usize>, right: &Range<usize>) -> bool {
    left.start < right.end && right.start < left.end
}

fn intersect_ranges(left: &Range<usize>, right: &Range<usize>) -> Option<Range<usize>> {
    let start = left.start.max(right.start);
    let end = left.end.min(right.end);
    (start < end).then_some(start..end)
}

#[derive(Debug, Clone, Copy)]
struct FenceDelimiter {
    marker: u8,
    len: usize,
}

#[derive(Debug, Clone, Copy)]
struct FenceOpening {
    delimiter: FenceDelimiter,
    is_mermaid: bool,
}

fn scan_mermaid_fences(source: &str) -> Vec<Range<usize>> {
    let mut bodies = Vec::new();
    let mut cursor = 0usize;
    while cursor < source.len() {
        let line_end = next_line_end(source, cursor);
        let line = trim_line_ending(&source[cursor..line_end]);
        let Some(opening) = fence_opening(line) else {
            cursor = line_end;
            continue;
        };
        if !opening.is_mermaid {
            cursor = skip_fence(source, line_end, opening.delimiter);
            continue;
        }

        let body_start = line_end;
        let mut search_start = body_start;
        let mut closed = false;
        while search_start < source.len() {
            let closing_end = next_line_end(source, search_start);
            let closing_line = trim_line_ending(&source[search_start..closing_end]);
            if matching_closing_fence(closing_line, opening.delimiter) {
                bodies.push(body_start..search_start);
                cursor = closing_end;
                closed = true;
                break;
            }
            search_start = closing_end;
        }
        if !closed {
            bodies.push(body_start..source.len());
            break;
        }
    }
    bodies
}

fn next_line_end(source: &str, start: usize) -> usize {
    let bytes = source.as_bytes();
    let mut cursor = start;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\n' => return cursor + 1,
            b'\r' if bytes.get(cursor + 1) == Some(&b'\n') => return cursor + 2,
            b'\r' => return cursor + 1,
            _ => cursor += 1,
        }
    }
    source.len()
}

fn trim_line_ending(line: &str) -> &str {
    line.strip_suffix("\r\n")
        .or_else(|| line.strip_suffix('\n'))
        .or_else(|| line.strip_suffix('\r'))
        .unwrap_or(line)
}

fn fence_opening(line: &str) -> Option<FenceOpening> {
    let trimmed = trim_fence_indent(line)?;
    let marker = *trimmed.as_bytes().first()?;
    if !matches!(marker, b'`' | b'~' | b':') {
        return None;
    }
    let len = repeated_marker_len(trimmed.as_bytes(), marker);
    if len < 3 {
        return None;
    }
    let rest = trimmed[len..].trim_start_matches(char::is_whitespace);
    if rest.is_empty() {
        return Some(FenceOpening {
            delimiter: FenceDelimiter { marker, len },
            is_mermaid: false,
        });
    }
    let language = "mermaid";
    let Some(prefix) = rest.get(..language.len()) else {
        return Some(FenceOpening {
            delimiter: FenceDelimiter { marker, len },
            is_mermaid: false,
        });
    };
    let tail = &rest[language.len()..];
    Some(FenceOpening {
        delimiter: FenceDelimiter { marker, len },
        is_mermaid: prefix.eq_ignore_ascii_case(language)
            && (tail.is_empty() || tail.chars().next().is_some_and(char::is_whitespace)),
    })
}

fn matching_closing_fence(line: &str, delimiter: FenceDelimiter) -> bool {
    let Some(trimmed) = trim_fence_indent(line) else {
        return false;
    };
    let len = repeated_marker_len(trimmed.as_bytes(), delimiter.marker);
    len >= delimiter.len && trimmed[len..].chars().all(char::is_whitespace)
}

fn skip_fence(source: &str, mut cursor: usize, delimiter: FenceDelimiter) -> usize {
    while cursor < source.len() {
        let line_end = next_line_end(source, cursor);
        let line = trim_line_ending(&source[cursor..line_end]);
        if matching_closing_fence(line, delimiter) {
            return line_end;
        }
        cursor = line_end;
    }
    source.len()
}

fn trim_fence_indent(line: &str) -> Option<&str> {
    let mut spaces = 0usize;
    for (index, byte) in line.bytes().enumerate() {
        match byte {
            b' ' if spaces < 3 => spaces += 1,
            b' ' | b'\t' => return None,
            _ => return Some(&line[index..]),
        }
    }
    Some("")
}

fn repeated_marker_len(bytes: &[u8], marker: u8) -> usize {
    bytes.iter().take_while(|byte| **byte == marker).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cancellation() -> AnalysisCancellationToken {
        AnalysisCancellationToken::new()
    }

    #[test]
    fn incremental_diagram_captures_match_a_fresh_parse() {
        let cancellation = cancellation();
        let initial = SyntaxDocumentState::parse(
            1,
            DocumentKind::Diagram,
            Arc::from("flowchart TD\nA --> B\n"),
            &cancellation,
        )
        .unwrap();
        let updated_source = Arc::<str>::from("flowchart TD\nAlpha --> B\nC --> D\n");
        let incremental = initial
            .update(
                2,
                DocumentKind::Diagram,
                Arc::clone(&updated_source),
                &cancellation,
            )
            .unwrap();
        let fresh =
            SyntaxDocumentState::parse(2, DocumentKind::Diagram, updated_source, &cancellation)
                .unwrap();

        assert_eq!(
            incremental.captures(None, &cancellation).unwrap(),
            fresh.captures(None, &cancellation).unwrap()
        );
    }

    #[test]
    fn markdown_state_owns_one_tree_per_mermaid_fence() {
        let cancellation = cancellation();
        let source = Arc::<str>::from(concat!(
            "````text\n",
            "```mermaid\nflowchart LR\nA-->B\n```\n",
            "````\n",
            "before\n",
            "  ``` Mermaid title=Main\n",
            "flowchart TD\nC-->D\n",
            "  `````\n",
            ":::mermaid\n",
            "pie title Work\n",
        ));
        let state = SyntaxDocumentState::parse(
            1,
            DocumentKind::Markdown,
            Arc::clone(&source),
            &cancellation,
        )
        .unwrap();

        assert_eq!(state.tree_count(), 2);
        let captures = state.captures(None, &cancellation).unwrap();
        assert!(captures.iter().all(|capture| {
            let text = &source[capture.start_byte..capture.end_byte];
            !text.contains("```mermaid")
        }));
        assert!(
            captures
                .iter()
                .any(|capture| { &source[capture.start_byte..capture.end_byte] == "flowchart" })
        );
        assert!(
            captures
                .iter()
                .any(|capture| &source[capture.start_byte..capture.end_byte] == "pie")
        );

        let updated_source = Arc::<str>::from(concat!(
            "before\n",
            "```mermaid\n",
            "flowchart TD\nAlpha-->D\n",
            "```\n",
            "~~~mermaid\n",
            "sequenceDiagram\nA->>B: Hi\n",
            "~~~\n",
        ));
        let incremental = state
            .update(
                2,
                DocumentKind::Markdown,
                Arc::clone(&updated_source),
                &cancellation,
            )
            .unwrap();
        let fresh =
            SyntaxDocumentState::parse(2, DocumentKind::Markdown, updated_source, &cancellation)
                .unwrap();
        assert_eq!(incremental.tree_count(), 2);
        assert_eq!(
            incremental.captures(None, &cancellation).unwrap(),
            fresh.captures(None, &cancellation).unwrap()
        );
    }

    #[test]
    fn cancelled_parse_does_not_publish_a_partial_tree() {
        let cancellation = cancellation();
        cancellation.cancel();

        assert_eq!(
            SyntaxDocumentState::parse(
                1,
                DocumentKind::Diagram,
                Arc::from("flowchart TD\nA-->B\n"),
                &cancellation,
            )
            .unwrap_err(),
            SyntaxHighlightError::Cancelled
        );
    }

    #[test]
    fn minimal_edit_uses_tree_sitter_byte_columns() {
        let edit = minimal_input_edit("flowchart TD\n🤓 --> B\n", "flowchart TD\n🤓 A --> B\n");

        assert_eq!(edit.start_position.row, 1);
        assert_eq!(edit.start_position.column, "🤓 ".len());
        assert_eq!(edit.old_end_position.row, 1);
        assert_eq!(edit.new_end_position.row, 1);
    }

    #[test]
    fn range_queries_preserve_overlapping_capture_boundaries() {
        let cancellation = cancellation();
        for (source, kind) in [
            ("flowchart TD\nA[\"hello\"]\n", SyntaxTokenKind::String),
            (
                "%%{init: {\n  \"theme\": \"dark\"\n}}%%\nflowchart TD\nA-->B\n",
                SyntaxTokenKind::Decorator,
            ),
        ] {
            let state = SyntaxDocumentState::parse(
                1,
                DocumentKind::Diagram,
                Arc::from(source),
                &cancellation,
            )
            .unwrap();
            let full = state.captures(None, &cancellation).unwrap();
            let capture = full
                .iter()
                .find(|capture| {
                    capture.kind == kind
                        && capture.end_byte - capture.start_byte > 2
                        && (kind != SyntaxTokenKind::Decorator
                            || source[capture.start_byte..capture.end_byte].contains('\n'))
                })
                .copied()
                .expect("fixture should contain the requested capture");
            let requested = capture.start_byte + 1..capture.end_byte - 1;

            assert!(
                state
                    .captures(Some(requested), &cancellation)
                    .unwrap()
                    .contains(&capture),
                "range capture should keep its complete syntax-token boundary",
            );
        }
    }
}
