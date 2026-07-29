use crate::snapshot::{DocumentSnapshot, FenceSnapshot};
use crate::types::{Position, Range};
use merman_analysis::{FenceCursorCompletionKind, FenceExpectedSyntaxKind, FenceTextIndexSource};
use merman_core::preprocess::split_frontmatter_block;

#[derive(Debug)]
pub struct CompletionContext<'a> {
    snapshot: &'a DocumentSnapshot,
    fence: &'a FenceSnapshot,
    prefix: String,
    prefix_start_offset: usize,
    cursor_offset: usize,
    source: FenceTextIndexSource,
    source_start: bool,
    directive_prefix: Option<&'static str>,
    comment_or_directive_line: bool,
    expected_syntax: Option<FenceExpectedSyntaxKind>,
    expected_syntax_span: Option<(usize, usize)>,
    completion_kinds: Vec<FenceCursorCompletionKind>,
}

impl<'a> CompletionContext<'a> {
    pub fn from_snapshot(snapshot: &'a DocumentSnapshot, position: Position) -> Option<Self> {
        let fence = snapshot.fence_at_position(position)?;
        let cursor_offset = snapshot.byte_offset_for_position(position)?;
        let body_range = fence.body_range();
        let document_range = fence.document_range();
        if cursor_offset < body_range.start
            || cursor_offset > body_range.end
            || (cursor_offset == body_range.end && document_range.end > body_range.end)
        {
            return None;
        }
        let relative_cursor = cursor_offset
            .saturating_sub(body_range.start)
            .min(fence.text().len());
        let cursor_context = fence
            .text_index()
            .cursor_context(fence.text(), relative_cursor);
        let prefix_start_offset = body_range.start + cursor_context.prefix_start();
        let cursor_offset = body_range.start + cursor_context.cursor();

        Some(Self {
            snapshot,
            fence,
            prefix: cursor_context.prefix().to_string(),
            prefix_start_offset,
            cursor_offset,
            source: cursor_context.source(),
            source_start: cursor_context.is_source_start(),
            directive_prefix: cursor_context.directive_prefix(),
            comment_or_directive_line: cursor_context.is_comment_or_directive_line(),
            expected_syntax: cursor_context.expected_syntax(),
            expected_syntax_span: cursor_context
                .expected_syntax_span()
                .map(|span| (body_range.start + span.start, body_range.start + span.end)),
            completion_kinds: cursor_context.completion_kinds().to_vec(),
        })
    }

    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    pub fn fence(&self) -> &FenceSnapshot {
        self.fence
    }

    pub fn document_uri(&self) -> &str {
        self.snapshot.uri().as_str()
    }

    pub fn has_parser_backed_facts(&self) -> bool {
        self.source.is_parser_backed()
    }

    pub fn fact_source(&self) -> FenceTextIndexSource {
        self.source
    }

    pub fn is_source_start(&self) -> bool {
        self.source_start
    }

    pub fn completion_vocabulary(&self) -> merman_core::EditorCompletionVocabulary {
        self.fence.text_index().completion_vocabulary()
    }

    pub fn prefix_range(&self) -> Option<Range> {
        self.range_for_offsets(self.prefix_start_offset, self.cursor_offset)
    }

    pub fn direction_value_range(&self) -> Option<Range> {
        if matches!(
            self.expected_syntax,
            Some(FenceExpectedSyntaxKind::Direction)
        ) && let Some((start, end)) = self.expected_syntax_span
        {
            return self.range_for_offsets(start, end);
        }

        None
    }

    pub fn operator_range(&self) -> Option<Range> {
        (self.expected_syntax == Some(FenceExpectedSyntaxKind::Operator))
            .then_some(self.expected_syntax_span)
            .flatten()
            .and_then(|(start, end)| self.range_for_offsets(start, end))
    }

    pub fn shape_value_range(&self) -> Option<Range> {
        self.shape_value_edit_parts().map(|(range, _, _)| range)
    }

    pub fn shape_value_edit(&self, value: &str) -> Option<CompletionTextEditParts> {
        let (range, has_separator_space, append_closing_brace) = self.shape_value_edit_parts()?;
        let replacement = if append_closing_brace {
            if has_separator_space {
                format!("{value} }}")
            } else {
                format!(" {value} }}")
            }
        } else if has_separator_space {
            value.to_string()
        } else {
            format!(" {value}")
        };

        Some(CompletionTextEditParts { range, replacement })
    }

    pub fn shape_trigger_range(&self) -> Option<Range> {
        (self.expected_syntax == Some(FenceExpectedSyntaxKind::ShapeTrigger))
            .then_some(self.expected_syntax_span)
            .flatten()
            .and_then(|(start, end)| self.range_for_offsets(start, end))
    }

    pub fn offer_diagram_headers(&self) -> bool {
        self.offers(FenceCursorCompletionKind::DiagramHeader)
    }

    pub fn offer_operator_items(&self) -> bool {
        self.offers(FenceCursorCompletionKind::Operator)
    }

    pub fn offer_directive_items(&self) -> bool {
        if self.expected_syntax.is_some() {
            return false;
        }

        self.offers(FenceCursorCompletionKind::Directive)
    }

    pub fn offer_direction_items(&self) -> bool {
        if let Some(expected) = self.expected_syntax {
            return matches!(expected, FenceExpectedSyntaxKind::Direction);
        }

        self.offers(FenceCursorCompletionKind::Direction)
    }

    pub fn offer_shape_items(&self) -> bool {
        if let Some(expected) = self.expected_syntax {
            return matches!(
                expected,
                FenceExpectedSyntaxKind::Shape | FenceExpectedSyntaxKind::ShapeTrigger
            );
        }

        self.offers(FenceCursorCompletionKind::Shape)
    }

    pub fn offer_node_items(&self) -> bool {
        if let Some(expected) = self.expected_syntax {
            return matches!(
                expected,
                FenceExpectedSyntaxKind::NodeIdentifier | FenceExpectedSyntaxKind::IdList
            );
        }

        if self.has_parser_backed_facts() && self.offer_directive_target_node_items() {
            return true;
        }

        false
    }

    pub fn offer_template_items(&self) -> bool {
        if !self.source_start || self.directive_prefix.is_some() {
            return false;
        }
        let prefix = self.prefix.trim_end();
        !prefix.is_empty()
            && !prefix.chars().any(char::is_whitespace)
            && TEMPLATE_PREFIXES
                .iter()
                .any(|template_prefix| template_prefix.starts_with(prefix))
    }

    pub fn offer_frontmatter_items(&self) -> bool {
        let relative_cursor = self
            .cursor_offset
            .saturating_sub(self.fence.body_range().start)
            .min(self.fence.text().len());
        is_frontmatter_authoring_position(
            self.fence.text(),
            relative_cursor,
            &self.prefix,
            self.source_start,
        )
    }

    pub fn offer_class_name_items(&self) -> bool {
        if self.payload_completion_context() {
            return false;
        }
        if !self.has_parser_backed_facts() {
            return false;
        }
        directive_slot_for_prefix(&self.prefix, self.directive_prefix)
            == DirectiveCompletionSlot::ClassName
    }

    pub fn offer_style_snippet_items(&self) -> bool {
        if self.payload_completion_context() {
            return false;
        }
        if !self.has_parser_backed_facts() {
            return false;
        }
        directive_slot_for_prefix(&self.prefix, self.directive_prefix)
            == DirectiveCompletionSlot::Style
    }

    pub fn offer_interaction_snippet_items(&self) -> bool {
        if self.payload_completion_context() {
            return false;
        }
        if !self.has_parser_backed_facts() {
            return false;
        }
        directive_slot_for_prefix(&self.prefix, self.directive_prefix)
            == DirectiveCompletionSlot::Interaction
    }

    pub fn is_comment_or_directive_line(&self) -> bool {
        self.comment_or_directive_line
    }

    pub fn is_parser_controlled_payload(&self) -> bool {
        self.expected_syntax == Some(FenceExpectedSyntaxKind::Payload)
    }

    pub fn directive_prefix(&self) -> Option<&'static str> {
        self.directive_prefix
    }

    pub fn node_text_edit_range(&self) -> Option<Range> {
        if matches!(
            self.expected_syntax,
            Some(FenceExpectedSyntaxKind::NodeIdentifier | FenceExpectedSyntaxKind::IdList)
        ) && let Some((start, end)) = self.expected_syntax_span
        {
            return self.range_for_offsets(start, end);
        }

        if self.offer_directive_target_node_items() {
            return self.current_token_range(is_directive_target_delimiter);
        }

        if self.offer_operator_items() {
            None
        } else {
            self.prefix_range()
        }
    }

    pub fn class_name_text_edit_range(&self) -> Option<Range> {
        self.current_token_range(is_class_name_delimiter)
    }

    pub fn style_text_edit_range(&self) -> Option<Range> {
        self.current_token_range(is_style_token_delimiter)
    }

    pub fn interaction_text_edit_range(&self) -> Option<Range> {
        self.current_token_range(is_style_token_delimiter)
    }

    pub fn frontmatter_text_edit_range(&self) -> Option<Range> {
        self.current_token_range(is_frontmatter_token_delimiter)
    }

    fn range_for_offsets(&self, start: usize, end: usize) -> Option<Range> {
        let span = self.snapshot.source_map().span(start, end).ok()?;
        Some(Range {
            start: Position {
                line: span.lsp_range.start.line,
                character: span.lsp_range.start.character,
            },
            end: Position {
                line: span.lsp_range.end.line,
                character: span.lsp_range.end.character,
            },
        })
    }

    fn shape_value_edit_parts(&self) -> Option<(Range, bool, bool)> {
        (self.expected_syntax == Some(FenceExpectedSyntaxKind::Shape))
            .then_some(())
            .and_then(|()| self.shape_value_edit_parts_from_expected_span())
    }

    fn shape_value_edit_parts_from_expected_span(&self) -> Option<(Range, bool, bool)> {
        let (start, end) = self.expected_syntax_span?;
        let range = self.range_for_offsets(start, end)?;
        let has_separator_space = self.snapshot.text()[..start]
            .chars()
            .next_back()
            .is_some_and(|ch| ch.is_whitespace());
        let append_closing_brace = self.should_append_shape_closing_brace(end);

        Some((range, has_separator_space, append_closing_brace))
    }

    fn should_append_shape_closing_brace(&self, offset: usize) -> bool {
        let Some(suffix) = self
            .snapshot
            .text()
            .get(offset..self.fence.body_range().end)
        else {
            return false;
        };
        !matches!(
            suffix.chars().find(|ch| !ch.is_whitespace()),
            Some('}' | ',')
        )
    }

    fn offers(&self, kind: FenceCursorCompletionKind) -> bool {
        self.completion_kinds.contains(&kind)
    }

    fn offer_directive_target_node_items(&self) -> bool {
        directive_slot_for_prefix(&self.prefix, self.directive_prefix)
            == DirectiveCompletionSlot::Target
    }

    fn payload_completion_context(&self) -> bool {
        self.is_parser_controlled_payload()
    }

    fn current_token_range(&self, is_delimiter: fn(char) -> bool) -> Option<Range> {
        let prefix = self.prefix.as_str();
        let token_start = prefix
            .char_indices()
            .rev()
            .find_map(|(idx, ch)| is_delimiter(ch).then_some(idx + ch.len_utf8()))
            .unwrap_or(0);

        self.range_for_offsets(self.prefix_start_offset + token_start, self.cursor_offset)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionTextEditParts {
    pub range: Range,
    pub replacement: String,
}

const TEMPLATE_PREFIXES: &[&str] = &[
    "flow", "seq", "icon", "acc", "class", "state", "er", "gantt", "pie", "journey", "mind",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DirectiveCompletionSlot {
    Target,
    ClassName,
    Style,
    Interaction,
    None,
}

fn directive_slot_for_prefix(
    prefix: &str,
    directive_prefix: Option<&str>,
) -> DirectiveCompletionSlot {
    if prefix
        .rfind(":::")
        .is_some_and(|index| index + 3 <= prefix.len())
    {
        return DirectiveCompletionSlot::ClassName;
    }

    let Some(directive_prefix) = directive_prefix else {
        return DirectiveCompletionSlot::None;
    };
    let Some(rest) = rest_after_keyword(prefix, directive_prefix) else {
        return DirectiveCompletionSlot::None;
    };

    match directive_prefix {
        "class" => {
            if first_argument_is_complete(rest) {
                DirectiveCompletionSlot::ClassName
            } else {
                DirectiveCompletionSlot::Target
            }
        }
        "cssClass" => {
            if first_argument_is_complete(rest) {
                DirectiveCompletionSlot::ClassName
            } else {
                DirectiveCompletionSlot::Target
            }
        }
        "classDef" => {
            if first_argument_is_complete(rest) {
                DirectiveCompletionSlot::Style
            } else if first_argument_end(rest).is_some() {
                DirectiveCompletionSlot::ClassName
            } else {
                DirectiveCompletionSlot::None
            }
        }
        "style" => {
            if first_argument_is_complete(rest) {
                DirectiveCompletionSlot::Style
            } else {
                DirectiveCompletionSlot::Target
            }
        }
        "click" | "link" | "callback" => {
            if first_argument_is_complete(rest) {
                DirectiveCompletionSlot::Interaction
            } else {
                DirectiveCompletionSlot::Target
            }
        }
        _ => DirectiveCompletionSlot::None,
    }
}

fn rest_after_keyword<'a>(prefix: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = prefix.strip_prefix(keyword)?;
    if rest.chars().next().is_none_or(|ch| ch.is_whitespace()) {
        Some(rest)
    } else {
        None
    }
}

fn first_argument_is_complete(rest: &str) -> bool {
    let Some(argument_end) = first_argument_end(rest) else {
        return false;
    };

    rest[argument_end..].chars().any(char::is_whitespace)
}

fn first_argument_end(rest: &str) -> Option<usize> {
    let leading = rest
        .chars()
        .take_while(|ch| ch.is_whitespace())
        .map(char::len_utf8)
        .sum::<usize>();
    let body = &rest[leading..];
    if body.is_empty() {
        return None;
    }

    if let Some(quote) = body.chars().next().filter(|ch| matches!(ch, '"' | '\'')) {
        let close = body[quote.len_utf8()..].find(quote)?;
        return Some(leading + quote.len_utf8() + close + quote.len_utf8());
    }

    let body_end = body
        .char_indices()
        .find_map(|(idx, ch)| ch.is_whitespace().then_some(idx))
        .unwrap_or(body.len());
    Some(leading + body_end)
}

fn is_directive_target_delimiter(ch: char) -> bool {
    ch.is_whitespace() || matches!(ch, ',' | '"' | '\'')
}

fn is_class_name_delimiter(ch: char) -> bool {
    is_directive_target_delimiter(ch) || ch == ':'
}

fn is_style_token_delimiter(ch: char) -> bool {
    ch.is_whitespace() || matches!(ch, ',' | '"' | '\'')
}

fn is_frontmatter_token_delimiter(ch: char) -> bool {
    ch.is_whitespace() || ch == ':'
}

fn is_frontmatter_authoring_position(
    text: &str,
    cursor: usize,
    prefix: &str,
    source_start: bool,
) -> bool {
    let trimmed_prefix = prefix.trim_end();
    if let Some(frontmatter) = split_frontmatter_block(text) {
        return cursor <= frontmatter.body.end;
    }
    if starts_with_frontmatter_opening_line(text) {
        return true;
    }
    if !source_start {
        return false;
    }

    cursor == 0
        || trimmed_prefix == "---"
        || (!trimmed_prefix.is_empty()
            && FRONTMATTER_PREFIXES
                .iter()
                .any(|frontmatter_prefix| frontmatter_prefix.starts_with(trimmed_prefix)))
}

fn starts_with_frontmatter_opening_line(text: &str) -> bool {
    let first_line_end = text.find('\n').unwrap_or(text.len());
    let first_line = text[..first_line_end].trim_end_matches('\r');
    first_line.trim_start() == "---"
}

const FRONTMATTER_PREFIXES: &[&str] = &[
    "config",
    "theme",
    "themeCSS",
    "themeVariables",
    "look",
    "layout",
];
