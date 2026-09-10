use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use std::borrow::Cow;
use std::fmt;
use std::ops::Range;

/// One-based position in the original Markdown source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Location {
    pub line: usize,
    pub column: usize,
}

/// A replaceable block, including a failed directive that may be kept as source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub span: Range<usize>,
    pub location: Location,
    pub kind: BlockKind,
    pub embedding: Embedding,
}

/// Markdown container prefixes used to insert a generated HTML block.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Embedding {
    first_prefix: String,
    continuation_prefix: String,
    explicit: bool,
    opens_container: bool,
    task_marker: bool,
    at_document_end: bool,
}

impl Embedding {
    /// Retain a trailing separator when the scanned source is only a document fragment.
    pub fn with_trailing_boundary(mut self) -> Self {
        self.at_document_end = false;
        self
    }

    /// Write generated diagram HTML in its original Markdown container.
    pub fn write_html(&self, output: &mut impl fmt::Write, html: &str) -> fmt::Result {
        if self.continuation_prefix.is_empty() {
            return output.write_str(if self.at_document_end {
                html.trim_end_matches(['\r', '\n'])
            } else {
                html
            });
        }
        output.write_str(&self.first_prefix)?;
        if !self.opens_container || self.task_marker {
            output.write_char('\n')?;
            output.write_str(&self.continuation_prefix)?;
        }
        for (index, line) in html.trim_matches(['\r', '\n']).lines().enumerate() {
            if index != 0 {
                output.write_char('\n')?;
                output.write_str(&self.continuation_prefix)?;
            }
            output.write_str(line)?;
        }
        if !self.at_document_end {
            output.write_char('\n')?;
            output.write_str(&self.continuation_prefix)?;
        }
        Ok(())
    }

    /// Return the exact UTF-8 byte count written by [`Self::write_html`].
    pub fn html_len(&self, html: &str) -> Option<usize> {
        struct Counter(Option<usize>);
        impl fmt::Write for Counter {
            fn write_str(&mut self, text: &str) -> fmt::Result {
                self.0 = self.0.and_then(|count| count.checked_add(text.len()));
                Ok(())
            }
        }
        let mut counter = Counter(Some(0));
        self.write_html(&mut counter, html).ok()?;
        counter.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlockKind {
    Mermaid(String),
    Include(String),
    Invalid(DocError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocError {
    UnclosedMermaidFence,
    InvalidInclude(&'static str),
    UnsupportedContainer,
}

impl fmt::Display for DocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnclosedMermaidFence => f.write_str("unclosed Mermaid fence"),
            Self::InvalidInclude(message) => write!(f, "invalid include_mmd! directive: {message}"),
            Self::UnsupportedContainer => f.write_str(
                "Mermaid include requires an explicit container prefix; indent it within its list or footnote and retain every block quote marker",
            ),
        }
    }
}

impl std::error::Error for DocError {}

/// Find diagram blocks while keeping per-block errors recoverable.
pub fn scan(source: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let result: Result<(), std::convert::Infallible> = visit_blocks(
        source,
        || Ok(()),
        |block| {
            blocks.push(block);
            Ok(())
        },
    );
    match result {
        Ok(()) => blocks,
        Err(never) => match never {},
    }
}

/// Visit blocks in source order, with caller-owned cancellation and admission.
///
/// `checkpoint` runs before parsing and between parser events. `visit` can reject
/// a block before it is retained or rendered. Individual Markdown errors are
/// represented by [`BlockKind::Invalid`], not by this function's return value.
/// The synchronous parser cannot be interrupted within a single parser operation.
pub fn visit_blocks<E>(
    source: &str,
    mut checkpoint: impl FnMut() -> Result<(), E>,
    mut visit: impl FnMut(Block) -> Result<(), E>,
) -> Result<(), E> {
    checkpoint()?;
    let lines = SourceLines::new(source, &mut checkpoint)?;
    let content_end = source.trim_end_matches([' ', '\t', '\r', '\n']).len();
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;
    let mut parents = Vec::new();
    let mut code: Option<CodeBlock> = None;
    let mut last_include_line = None;
    let parser_source = normalize_bare_cr(source);
    for (event, span) in Parser::new_ext(&parser_source, options).into_offset_iter() {
        checkpoint()?;
        match event {
            Event::Start(Tag::CodeBlock(kind)) => {
                let mermaid = matches!(&kind, CodeBlockKind::Fenced(info)
                    if info.trim_start_matches('{').split(|ch: char| ch.is_whitespace() || ch == '}' || ch == ',').next().is_some_and(|language| language.eq_ignore_ascii_case("mermaid")));
                code = Some(CodeBlock {
                    mermaid,
                    body_end: None,
                    embedding: embedding(source, &lines, &parents, span.start),
                    span,
                    body: String::new(),
                });
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some(mut code) = code.take().filter(|code| code.mermaid) {
                    code.embedding.at_document_end = code.span.end >= content_end;
                    let location = lines.location(source, code.span.start);
                    let kind = if !has_closing_fence(source, &code) {
                        BlockKind::Invalid(DocError::UnclosedMermaidFence)
                    } else {
                        BlockKind::Mermaid(code.body)
                    };
                    visit(Block {
                        span: lines.start(code.span.start)..code.span.end,
                        location,
                        kind,
                        embedding: code.embedding,
                    })?;
                }
            }
            Event::Text(text) if code.is_some() => {
                if let Some(code) = code.as_mut().filter(|code| code.mermaid) {
                    code.body.push_str(&text);
                    code.body_end = Some(span.end);
                }
            }
            Event::Text(_) if allows_include(&parents) => {
                // Only text events can introduce a directive. Inline code and
                // HTML comments never enter this branch. Read the original line
                // because inline Markdown processing may split or unescape it.
                for offset in lines.intersecting_starts(span.clone()) {
                    let line_start = lines.start(offset);
                    if last_include_line == Some(line_start) {
                        continue;
                    }
                    let line_end = lines.end(source, offset);
                    let nested = parents
                        .iter()
                        .any(|parent| parent.container_prefix.is_some());
                    let candidate_start = if nested { offset } else { line_start };
                    let line = &source[candidate_start..line_end];
                    let trimmed = line.trim();
                    let directive_start = candidate_start + line.len() - line.trim_start().len();
                    if directive_start < span.start || directive_start >= span.end {
                        continue;
                    }
                    let Some(kind) = parse_include(trimmed) else {
                        continue;
                    };
                    last_include_line = Some(line_start);
                    let mut embedding = embedding(source, &lines, &parents, directive_start);
                    embedding.at_document_end = line_end >= content_end;
                    let prefix =
                        continuation_prefix(source, &lines, &parents, directive_start, None);
                    let content = prefix.trim_matches(|character: char| {
                        character.is_whitespace() || character == '>'
                    });
                    if !(content.is_empty()
                        || embedding.task_marker && matches!(content, "[ ]" | "[x]" | "[X]"))
                    {
                        continue;
                    }
                    let explicit = embedding.explicit;
                    visit(Block {
                        span: line_start..line_end,
                        location: lines.location(source, directive_start),
                        kind: if nested && !explicit {
                            BlockKind::Invalid(DocError::UnsupportedContainer)
                        } else {
                            kind
                        },
                        embedding,
                    })?;
                }
            }
            Event::Start(tag) => {
                let end = tag.to_end();
                let marker_end = container_marker_end(source, span.start, end);
                let container_prefix = marker_end.map(|marker_end| {
                    continuation_prefix(
                        source,
                        &lines,
                        &parents,
                        marker_end,
                        Some((span.start, end)),
                    )
                });
                parents.push(Parent {
                    end,
                    start: span.start,
                    container_prefix,
                    task_marker: None,
                });
            }
            Event::TaskListMarker(_) => {
                if let Some(item) = parents
                    .iter_mut()
                    .rev()
                    .find(|parent| parent.end == TagEnd::Item)
                {
                    item.task_marker = Some(span.end);
                }
            }
            Event::End(_) => {
                parents.pop();
            }
            _ => {}
        }
    }
    checkpoint()
}

// Replacing bare CR with LF keeps every UTF-8 offset stable. The parser handles
// CRLF itself, but does not treat a standalone CR as a line boundary.
fn normalize_bare_cr(source: &str) -> Cow<'_, str> {
    let bytes = source.as_bytes();
    let Some(first) = bytes.iter().enumerate().find_map(|(index, byte)| {
        (*byte == b'\r' && bytes.get(index + 1) != Some(&b'\n')).then_some(index)
    }) else {
        return Cow::Borrowed(source);
    };
    let mut normalized = bytes.to_vec();
    for index in first..normalized.len() {
        if normalized[index] == b'\r' && normalized.get(index + 1) != Some(&b'\n') {
            normalized[index] = b'\n';
        }
    }
    Cow::Owned(String::from_utf8(normalized).expect("ASCII newline replacement preserves UTF-8"))
}

fn allows_include(parents: &[Parent]) -> bool {
    !parents.is_empty()
        && parents.iter().all(|parent| {
            matches!(
                parent.end,
                TagEnd::Paragraph
                    | TagEnd::Item
                    | TagEnd::List(_)
                    | TagEnd::BlockQuote(_)
                    | TagEnd::FootnoteDefinition
            )
        })
}

struct Parent {
    end: TagEnd,
    start: usize,
    container_prefix: Option<String>,
    task_marker: Option<usize>,
}

fn container_marker_end(source: &str, start: usize, tag: TagEnd) -> Option<usize> {
    let remaining = &source[start..];
    let marker_len = match tag {
        TagEnd::Item => remaining
            .bytes()
            .position(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))?,
        TagEnd::BlockQuote(_) => 1,
        TagEnd::FootnoteDefinition => remaining.find("]:")? + 2,
        _ => return None,
    };
    let whitespace = remaining[marker_len..]
        .bytes()
        .take_while(|byte| matches!(byte, b' ' | b'\t'))
        .count();
    Some(start + marker_len + whitespace)
}

fn continuation_prefix(
    source: &str,
    lines: &SourceLines,
    parents: &[Parent],
    content_start: usize,
    current: Option<(usize, TagEnd)>,
) -> String {
    let line_start = lines.start(content_start);
    let mut prefix = String::new();
    let mut copied = line_start;
    for (start, tag) in parents
        .iter()
        .map(|parent| (parent.start, parent.end))
        .chain(current)
        .filter(|(start, tag)| {
            *start >= line_start
                && *start < content_start
                && matches!(tag, TagEnd::Item | TagEnd::FootnoteDefinition)
        })
    {
        let Some(end) = container_marker_end(source, start, tag) else {
            continue;
        };
        let end = end.min(content_start);
        prefix.push_str(&source[copied..start]);
        match tag {
            TagEnd::FootnoteDefinition => prefix.push_str("    "),
            TagEnd::Item => {
                for character in source[start..end].chars() {
                    if character == '\t' {
                        prefix.push('\t');
                    } else {
                        prefix.push(' ');
                    }
                }
            }
            _ => unreachable!(),
        }
        copied = end;
    }
    prefix.push_str(&source[copied..content_start]);
    prefix
}

fn embedding(source: &str, lines: &SourceLines, parents: &[Parent], start: usize) -> Embedding {
    let Some(continuation) = parents
        .iter()
        .rev()
        .find_map(|parent| parent.container_prefix.as_ref())
    else {
        return Embedding::default();
    };
    let first_prefix = source[lines.start(start)..start].to_owned();
    let normalized = continuation_prefix(source, lines, parents, start, None);
    let task_marker = parents.iter().any(|parent| {
        parent
            .task_marker
            .is_some_and(|end| end <= start && lines.start(end) == lines.start(start))
    });
    let explicit = prefix_shape(&normalized)
        .strip_prefix(&prefix_shape(continuation))
        .is_some_and(|rest| {
            rest.trim().is_empty() || task_marker && matches!(rest.trim(), "[ ]" | "[x]" | "[X]")
        });
    Embedding {
        first_prefix,
        continuation_prefix: continuation.clone(),
        explicit,
        opens_container: parents.iter().any(|parent| {
            parent.start >= lines.start(start)
                && matches!(parent.end, TagEnd::Item | TagEnd::FootnoteDefinition)
        }),
        task_marker,
        at_document_end: false,
    }
}

fn prefix_shape(prefix: &str) -> String {
    let mut shape = String::new();
    let mut after_quote = false;
    let mut column = 0;
    for character in prefix.chars() {
        match character {
            ' ' if after_quote => {}
            '\t' => {
                let width = 4 - column % 4;
                shape.extend(std::iter::repeat_n(' ', width));
                column += width;
            }
            _ => {
                shape.push(character);
                column += 1;
            }
        }
        after_quote = character == '>';
    }
    shape
}

struct CodeBlock {
    span: Range<usize>,
    mermaid: bool,
    body_end: Option<usize>,
    embedding: Embedding,
    body: String,
}

fn has_closing_fence(source: &str, code: &CodeBlock) -> bool {
    let block = &source[code.span.clone()];
    let Some(first_end) = block.find(['\r', '\n']) else {
        return false;
    };
    let opening = &block[..first_end];
    let marker = opening.as_bytes()[0];
    let count = opening.bytes().take_while(|byte| *byte == marker).count();
    let closing_start = code.body_end.unwrap_or(code.span.start + first_end);
    let final_line = source[closing_start..code.span.end]
        .trim_end_matches(['\r', '\n'])
        .rsplit(['\r', '\n'])
        .next()
        .unwrap_or("")
        .trim_start_matches([' ', '\t', '>'])
        .trim_end_matches([' ', '\t']);
    final_line.len() >= count && final_line.bytes().all(|byte| byte == marker)
}

fn parse_include(line: &str) -> Option<BlockKind> {
    let rest = line.strip_prefix("include_mmd!")?.trim();
    let arguments = rest.strip_prefix('(')?.trim_start();
    let mut literal = serde_json::Deserializer::from_str(arguments).into_iter::<String>();
    let path = match literal.next() {
        Some(Ok(path)) => path,
        _ => {
            return Some(BlockKind::Invalid(DocError::InvalidInclude(
                "path must be a double-quoted string with JSON-compatible escapes; raw Rust strings are not supported",
            )));
        }
    };
    let suffix = arguments[literal.byte_offset()..].trim();
    if suffix == ")" {
        Some(BlockKind::Include(path))
    } else if suffix.starts_with(')') {
        // A complete directive followed by prose is an example, not an include.
        None
    } else {
        Some(BlockKind::Invalid(DocError::InvalidInclude(
            "expected a closing ')' after the path literal",
        )))
    }
}

struct SourceLines(Vec<usize>);

impl SourceLines {
    fn new<E>(source: &str, checkpoint: &mut impl FnMut() -> Result<(), E>) -> Result<Self, E> {
        let mut starts = vec![0];
        let mut previous_cr = false;
        for (offset, byte) in source.bytes().enumerate() {
            if offset % (64 * 1024) == 0 {
                checkpoint()?;
            }
            if byte == b'\n' && previous_cr {
                *starts.last_mut().expect("initial source line") = offset + 1;
            } else if matches!(byte, b'\n' | b'\r') {
                starts.push(offset + 1);
            }
            previous_cr = byte == b'\r';
        }
        Ok(Self(starts))
    }

    fn index(&self, offset: usize) -> usize {
        self.0.partition_point(|start| *start <= offset) - 1
    }

    fn start(&self, offset: usize) -> usize {
        self.0[self.index(offset)]
    }

    fn end(&self, source: &str, offset: usize) -> usize {
        let end = self
            .0
            .get(self.index(offset) + 1)
            .copied()
            .unwrap_or(source.len());
        self.start(offset)
            + source[self.start(offset)..end]
                .trim_end_matches(['\r', '\n'])
                .len()
    }

    fn location(&self, source: &str, offset: usize) -> Location {
        Location {
            line: self.index(offset) + 1,
            column: source[self.start(offset)..offset].chars().count() + 1,
        }
    }

    fn intersecting_starts(&self, span: Range<usize>) -> impl Iterator<Item = usize> + '_ {
        std::iter::once(span.start).chain(
            self.0[self.index(span.start) + 1..]
                .iter()
                .copied()
                .take_while(move |start| *start < span.end),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_real_blocks_and_preserves_original_ranges() {
        let source = "before\r\n  ```` Mermaid title=x\r\n  flowchart LR\r\n  A-->B\r\n  `````\r\ninclude_mmd!(\"one.mmd\")\r\nafter";
        let blocks = scan(source);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].location, Location { line: 2, column: 3 });
        assert_eq!(
            blocks[0].kind,
            BlockKind::Mermaid("flowchart LR\nA-->B\n".into())
        );
        assert_eq!(
            &source[blocks[0].span.clone()],
            "  ```` Mermaid title=x\r\n  flowchart LR\r\n  A-->B\r\n  `````"
        );
        assert_eq!(blocks[1].kind, BlockKind::Include("one.mmd".into()));
        assert_eq!(&source[blocks[1].span.clone()], "include_mmd!(\"one.mmd\")");
    }

    #[test]
    fn ignores_examples_comments_and_non_directive_prose() {
        for source in [
            "    ```mermaid\n    flowchart LR\n    A-->B\n    ```\n",
            "    include_mmd!(\"ignored.mmd\")\n",
            "<!--\ninclude_mmd!(\"ignored.mmd\")\n```mermaid\nA-->B\n```\n-->\n",
            "before <!--\ninclude_mmd!(\"ignored.mmd\")\n--> after\n",
            "````text\n```mermaid\nA-->B\n```\ninclude_mmd!(\"ignored.mmd\")\n````\n",
            "`include_mmd!(\"ignored.mmd\")`\n",
            "include_mmd! is documented here\n",
            "include_mmd!(\"one.mmd\") trailing prose\n",
            "- **Example** include_mmd!(\"ignored.mmd\")\n",
            "> `Example` include_mmd!(\"ignored.mmd\")\n",
            "- [ ] **Example** include_mmd!(\"ignored.mmd\")\n",
            ":::mermaid\nA-->B\n:::\n",
        ] {
            assert!(scan(source).is_empty(), "{source:?}");
        }
    }

    #[test]
    fn keeps_errors_local_to_each_block() {
        let source = "include_mmd!(bad)\n\n~~~mermaid\nA-->B\n~~~\n\n```mermaid\nunfinished\n";
        let blocks = scan(source);
        assert_eq!(blocks.len(), 3);
        assert!(matches!(
            blocks[0].kind,
            BlockKind::Invalid(DocError::InvalidInclude(_))
        ));
        assert_eq!(blocks[1].kind, BlockKind::Mermaid("A-->B\n".into()));
        assert_eq!(
            blocks[2].kind,
            BlockKind::Invalid(DocError::UnclosedMermaidFence)
        );
        let mut rewritten = String::new();
        let mut cursor = 0;
        for block in blocks {
            rewritten.push_str(&source[cursor..block.span.start]);
            if matches!(block.kind, BlockKind::Mermaid(_)) {
                rewritten.push_str("<svg/>");
            } else {
                rewritten.push_str(&source[block.span.clone()]);
            }
            cursor = block.span.end;
        }
        rewritten.push_str(&source[cursor..]);
        assert_eq!(
            rewritten,
            "include_mmd!(bad)\n\n<svg/>\n\n```mermaid\nunfinished\n"
        );
    }

    #[test]
    fn preserves_containers_and_neighboring_markdown() {
        for source in [
            "> ```mermaid\n> A-->B\n> ```\n> **after**\n",
            "> include_mmd!(\"one.mmd\")\n> **after**\n",
            "- include_mmd!(\"one.mmd\")\n  **after**\n",
            "- item\n\n  ```mermaid\n  A-->B\n  ```\n  **after**\n",
            "1.  > ```mermaid\n    > A-->B\n    > ```\n    > **after**\n",
            "> - include_mmd!(\"one.mmd\")\n>   **after**\n",
            "- > include_mmd!(\"one.mmd\")\n  > **after**\n",
            "- outer\n  - include_mmd!(\"one.mmd\")\n    **after**\n",
            "9. include_mmd!(\"one.mmd\")\n   **after**\n",
            "9.\tinclude_mmd!(\"one.mmd\")\n\t**after**\n",
            "-\t> include_mmd!(\"one.mmd\")\n\t> **after**\n",
            "- [ ] include_mmd!(\"one.mmd\")\n  **after**\n",
            "[^diagram]:\n    ```mermaid\n    A-->B\n    ```\n    **after**\n",
            "[^long-label]: include_mmd!(\"one.mmd\")\n    **after**\n",
            "[^long-label]:   include_mmd!(\"one.mmd\")\n    **after**\n",
            "[^a]: > include_mmd!(\"one.mmd\")\n    > **after**\n",
            "[^a]: - include_mmd!(\"one.mmd\")\n      **after**\n",
            "> ```mermaid\r\n> A-->B\r\n> ```\r\n> **after**\r\n",
        ] {
            let blocks = scan(source);
            assert_eq!(blocks.len(), 1, "{source:?}");
            assert!(
                !matches!(blocks[0].kind, BlockKind::Invalid(_)),
                "{source:?}: {blocks:?}"
            );
            let block = &blocks[0];
            let replacement = "\n\n<div data-diagram=\"true\">\n<svg></svg>\n</div>\n\n";
            let mut embedded = String::new();
            block
                .embedding
                .write_html(&mut embedded, replacement)
                .unwrap();
            assert_eq!(block.embedding.html_len(replacement), Some(embedded.len()));
            let rewritten = format!(
                "{}{}{}",
                &source[..block.span.start],
                embedded,
                &source[block.span.end..]
            );
            let mut html = String::new();
            pulldown_cmark::html::push_html(
                &mut html,
                Parser::new_ext(
                    &rewritten,
                    Options::ENABLE_FOOTNOTES | Options::ENABLE_TASKLISTS,
                ),
            );
            assert!(
                html.contains("<strong>after</strong>"),
                "{source:?}\n{rewritten}\n{html}"
            );
            assert!(
                html.contains("<svg></svg>"),
                "{source:?}\n{rewritten}\n{html}"
            );
            if source.contains("[ ]") {
                assert!(html.contains("type=\"checkbox\""), "{rewritten}\n{html}");
                assert!(!html.contains("&lt;div"), "{rewritten}\n{html}");
            }
            let containers = |text: &str| {
                Parser::new_ext(text, Options::ENABLE_FOOTNOTES)
                    .filter_map(|event| match event {
                        Event::Start(
                            tag @ (Tag::Item
                            | Tag::List(_)
                            | Tag::BlockQuote(_)
                            | Tag::FootnoteDefinition(_)),
                        ) => Some(tag.to_end()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                containers(source),
                containers(&rewritten),
                "{source:?}\n{rewritten}\n{html}"
            );
            let mut active = Vec::new();
            let mut diagram_containers = None;
            for event in Parser::new_ext(
                &rewritten,
                Options::ENABLE_FOOTNOTES | Options::ENABLE_TASKLISTS,
            ) {
                match event {
                    Event::Start(
                        tag @ (Tag::Item
                        | Tag::List(_)
                        | Tag::BlockQuote(_)
                        | Tag::FootnoteDefinition(_)),
                    ) => active.push(tag.to_end()),
                    Event::End(
                        TagEnd::Item
                        | TagEnd::List(_)
                        | TagEnd::BlockQuote(_)
                        | TagEnd::FootnoteDefinition,
                    ) => {
                        active.pop();
                    }
                    Event::Html(html) if html.contains("data-diagram") => {
                        diagram_containers = Some(active.clone())
                    }
                    _ => {}
                }
            }
            assert_eq!(
                diagram_containers,
                Some(containers(source)),
                "{source:?}\n{rewritten}"
            );
        }
    }

    #[test]
    fn final_diagrams_preserve_source_line_endings_without_added_blank_lines() {
        for block_source in [
            "```mermaid\nA-->B\n```",
            "include_mmd!(\"one.mmd\")",
            "> include_mmd!(\"one.mmd\")",
            "- include_mmd!(\"one.mmd\")",
            "[^diagram]: include_mmd!(\"one.mmd\")",
        ] {
            for ending in ["", "\n", "\r\n", "\n\n"] {
                let source = format!("{block_source}{ending}");
                let blocks = scan(&source);
                assert_eq!(blocks.len(), 1);
                let block = &blocks[0];
                let html = "\n\n<div><svg></svg></div>\n\n";
                let mut embedded = String::new();
                block.embedding.write_html(&mut embedded, html).unwrap();
                assert_eq!(block.embedding.html_len(html), Some(embedded.len()));
                assert!(embedded.ends_with("</div>"), "{source:?}: {embedded:?}");
                let rewritten = format!("{embedded}{}", &source[block.span.end..]);
                assert!(rewritten.ends_with(&format!("</div>{ending}")));
            }
        }
    }

    #[test]
    fn lazy_include_continuations_require_explicit_container_prefixes() {
        for source in [
            "- before\ninclude_mmd!(\"one.mmd\")\n",
            "> before\ninclude_mmd!(\"one.mmd\")\n",
        ] {
            assert_eq!(
                scan(source)[0].kind,
                BlockKind::Invalid(DocError::UnsupportedContainer)
            );
        }
    }

    #[test]
    fn container_fence_boundaries_use_parser_body_ranges() {
        for (source, expected) in [
            ("> ```mermaid\n> ```\n", BlockKind::Mermaid(String::new())),
            (
                "```mermaid\nA-->B\n```   \n",
                BlockKind::Mermaid("A-->B\n".into()),
            ),
            (
                "> ```mermaid\r\n> A-->B\r\n> ```   \r\n",
                BlockKind::Mermaid("A-->B\n".into()),
            ),
            ("- ```mermaid\n  ```\n", BlockKind::Mermaid(String::new())),
            (
                "```mermaid\nA-->B\n```\t\n",
                BlockKind::Invalid(DocError::UnclosedMermaidFence),
            ),
            (
                "> ```mermaid\n> >not-a-close\n> ```\n",
                BlockKind::Mermaid(">not-a-close\n".into()),
            ),
            (
                "> ```mermaid\n> ``` trailing\n",
                BlockKind::Invalid(DocError::UnclosedMermaidFence),
            ),
            (
                "> ```mermaid\n> unfinished\n\nOutside\n",
                BlockKind::Invalid(DocError::UnclosedMermaidFence),
            ),
        ] {
            assert_eq!(scan(source)[0].kind, expected, "{source:?}");
        }
    }

    #[test]
    fn rejects_invalid_include_literals() {
        for source in [
            "include_mmd!(bad)",
            "include_mmd!(\"unfinished.mmd\"",
            "include_mmd!(r\"one.mmd\")",
        ] {
            assert!(matches!(
                scan(source)[0].kind,
                BlockKind::Invalid(DocError::InvalidInclude(_))
            ));
        }
    }

    #[test]
    fn decodes_escaped_paths_without_reinterpreting_markdown() {
        let blocks = scan(
            r#"include_mmd!("docs\\windows\\diagram.mmd")
include_mmd!("docs/quoted\"diagram.mmd")
include_mmd!("docs/\u6d41\u7a0b.mmd")"#,
        );
        assert_eq!(blocks.len(), 3);
        assert_eq!(
            blocks[0].kind,
            BlockKind::Include(r"docs\windows\diagram.mmd".into())
        );
        assert_eq!(
            blocks[1].kind,
            BlockKind::Include("docs/quoted\"diagram.mmd".into())
        );
        assert_eq!(blocks[2].kind, BlockKind::Include("docs/流程.mmd".into()));
    }

    #[test]
    fn keeps_existing_fence_info_aliases() {
        for info in [
            "mermaid",
            "Mermaid",
            "{mermaid}",
            "mermaid,ignore",
            "{mermaid, theme=dark}",
            "mermaid extra",
        ] {
            let source = format!("```{info}\nA-->B\n```\n");
            assert_eq!(scan(&source)[0].kind, BlockKind::Mermaid("A-->B\n".into()));
        }
    }

    #[test]
    fn admission_and_cancellation_stop_visiting() {
        let source = "```mermaid\nA-->B\n```\n```mermaid\nB-->C\n```\n";
        let mut visited = 0;
        let result = visit_blocks(
            source,
            || Ok(()),
            |_| {
                visited += 1;
                Err("limit")
            },
        );
        assert_eq!(result, Err("limit"));
        assert_eq!(visited, 1);
        let result = visit_blocks(
            source,
            || Err("cancelled"),
            |_| panic!("no blocks after cancellation"),
        );
        assert_eq!(result, Err("cancelled"));
    }

    #[test]
    fn handles_bare_cr_and_multibyte_positions() {
        let blocks = scan("中文\r\r   ~~~mermaid\rA-->B\r~~~\r");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].location, Location { line: 3, column: 4 });
        assert_eq!(blocks[0].kind, BlockKind::Mermaid("A-->B\n".into()));
    }
}
