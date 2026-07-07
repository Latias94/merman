use crate::snapshot::{DocumentSnapshot, FenceSnapshot};
use crate::types::{Position, Range};
use merman_analysis::{
    FenceCursorCompletionKind, FenceExpectedSyntaxKind, FenceTextIndexSource,
    shape_object_value_prefix,
};
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
        if cursor_offset < fence.body_start
            || cursor_offset > fence.body_end
            || (cursor_offset == fence.body_end && fence.end > fence.body_end)
        {
            return None;
        }
        let relative_cursor = cursor_offset
            .saturating_sub(fence.body_start)
            .min(fence.text.len());
        let cursor_context = fence
            .text_index
            .cursor_context(&fence.text, relative_cursor);
        let prefix_start_offset = fence.body_start + cursor_context.prefix_start();
        let cursor_offset = fence.body_start + cursor_context.cursor();

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
                .map(|span| (fence.body_start + span.start, fence.body_start + span.end)),
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
        self.snapshot.uri.as_str()
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

    pub fn prefix_range(&self) -> Option<Range> {
        self.range_for_offsets(self.prefix_start_offset, self.cursor_offset)
    }

    pub fn operator_range(&self) -> Option<Range> {
        let suffix_start = operator_suffix_start(&self.prefix)?;
        self.range_for_offsets(self.prefix_start_offset + suffix_start, self.cursor_offset)
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
        if matches!(
            self.expected_syntax,
            Some(FenceExpectedSyntaxKind::ShapeTrigger)
        ) && let Some((start, end)) = self.expected_syntax_span
        {
            return self.range_for_offsets(start, end);
        }

        let prefix = self.prefix.trim_end();
        let trigger_len = if prefix.ends_with("((")
            || prefix.ends_with("{{")
            || prefix.ends_with("[/")
            || prefix.ends_with("[\\")
        {
            2
        } else if prefix.ends_with('[') || prefix.ends_with('>') {
            1
        } else {
            return None;
        };

        self.range_for_offsets(
            self.prefix_start_offset + prefix.len().saturating_sub(trigger_len),
            self.cursor_offset,
        )
    }

    pub fn offer_diagram_headers(&self) -> bool {
        self.offers(FenceCursorCompletionKind::DiagramHeader)
    }

    pub fn offer_operator_items(&self) -> bool {
        self.offers(FenceCursorCompletionKind::Operator)
            && !self.degraded_flowchart_payload_context()
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

        if self.degraded_flowchart_payload_context()
            && !flowchart_top_level_shape_completion_context(&self.prefix)
        {
            return false;
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

        if self.degraded_flowchart_target_range().is_some() {
            return true;
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
            .saturating_sub(self.fence.body_start)
            .min(self.fence.text.len());
        is_frontmatter_authoring_position(
            &self.fence.text,
            relative_cursor,
            &self.prefix,
            self.source_start,
        )
    }

    pub fn offer_class_name_items(&self) -> bool {
        if !self.has_parser_backed_facts() {
            return false;
        }
        directive_slot_for_prefix(&self.prefix, self.directive_prefix)
            == DirectiveCompletionSlot::ClassName
    }

    pub fn offer_style_snippet_items(&self) -> bool {
        if !self.has_parser_backed_facts() {
            return false;
        }
        directive_slot_for_prefix(&self.prefix, self.directive_prefix)
            == DirectiveCompletionSlot::Style
    }

    pub fn offer_interaction_snippet_items(&self) -> bool {
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

        if let Some(range) = self.degraded_flowchart_target_range() {
            return Some(range);
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
        let span = self.snapshot.source_map.span(start, end).ok()?;
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
        if self.expected_syntax == Some(FenceExpectedSyntaxKind::Shape) {
            return self.shape_value_edit_parts_from_expected_span();
        }

        let prefix = self.prefix.as_str();
        if let Some((range, has_separator_space)) = self.shape_value_edit_parts_from_prefix(prefix)
        {
            return Some((
                range,
                has_separator_space,
                self.should_append_shape_closing_brace(self.cursor_offset),
            ));
        }

        None
    }

    fn shape_value_edit_parts_from_prefix(&self, prefix: &str) -> Option<(Range, bool)> {
        let shape_prefix = shape_object_value_prefix(prefix)?;
        let range = self.range_for_offsets(
            self.prefix_start_offset + shape_prefix.value_start,
            self.cursor_offset,
        )?;

        Some((range, shape_prefix.has_separator_space))
    }

    fn shape_value_edit_parts_from_expected_span(&self) -> Option<(Range, bool, bool)> {
        let (start, end) = self.expected_syntax_span?;
        let range = self.range_for_offsets(start, end)?;
        let has_separator_space = self.snapshot.text[..start]
            .chars()
            .next_back()
            .is_some_and(|ch| ch.is_whitespace());
        let append_closing_brace = self.should_append_shape_closing_brace(end);

        Some((range, has_separator_space, append_closing_brace))
    }

    fn should_append_shape_closing_brace(&self, offset: usize) -> bool {
        let Some(suffix) = self.snapshot.text.get(offset..self.fence.body_end) else {
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

    fn degraded_flowchart_target_range(&self) -> Option<Range> {
        if self.expected_syntax.is_some()
            || self.comment_or_directive_line
            || !self.source.is_parser_backed()
            || self.source.has_source_mapped_spans()
            || !is_flowchart_diagram_type(self.fence.diagram_type.as_deref())
        {
            return None;
        }

        let token_start = flowchart_target_token_start(&self.prefix)?;
        self.range_for_offsets(self.prefix_start_offset + token_start, self.cursor_offset)
    }

    fn degraded_flowchart_payload_context(&self) -> bool {
        self.expected_syntax.is_none()
            && self.source.is_parser_backed()
            && !self.source.has_source_mapped_spans()
            && is_flowchart_diagram_type(self.fence.diagram_type.as_deref())
            && flowchart_scan_line(&self.prefix).inside_payload
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

fn operator_suffix_start(prefix: &str) -> Option<usize> {
    let mut start = prefix.len();
    let mut seen_operator = false;

    for (idx, ch) in prefix.char_indices().rev() {
        if matches!(ch, '-' | '>' | '.' | '=') {
            start = idx;
            seen_operator = true;
        } else {
            break;
        }
    }

    seen_operator.then_some(start)
}

fn is_flowchart_diagram_type(diagram_type: Option<&str>) -> bool {
    diagram_type.is_some_and(|diagram_type| diagram_type.starts_with("flowchart"))
}

fn flowchart_target_token_start(prefix: &str) -> Option<usize> {
    let trimmed_end = prefix.trim_end().len();
    let trimmed = &prefix[..trimmed_end];
    let (_, operator_end) = last_flowchart_operator(trimmed)?;
    let mut target_start = operator_end;

    target_start += leading_whitespace_len(&prefix[target_start..]);
    let mut replacement_start = target_start;
    let operator_tail = prefix.get(target_start..)?;
    if operator_tail.starts_with('|') {
        target_start += flowchart_edge_label_len(operator_tail)?;
        replacement_start = target_start;
        target_start += leading_whitespace_len(&prefix[target_start..]);
    }

    let target = &prefix[target_start..];
    if target.chars().any(char::is_whitespace) {
        return None;
    }
    if !target.chars().all(is_flowchart_target_fragment_char) {
        return None;
    }

    Some(replacement_start)
}

fn last_flowchart_operator(prefix: &str) -> Option<(usize, usize)> {
    flowchart_scan_line(prefix).last_operator
}

fn flowchart_top_level_shape_completion_context(prefix: &str) -> bool {
    flowchart_top_level_shape_trigger(prefix) || flowchart_top_level_shape_object_value(prefix)
}

fn flowchart_top_level_shape_trigger(prefix: &str) -> bool {
    let prefix = prefix.trim_end();
    let trigger_len = if prefix.ends_with("((")
        || prefix.ends_with("{{")
        || prefix.ends_with("[/")
        || prefix.ends_with("[\\")
    {
        2
    } else if prefix.ends_with('[') || prefix.ends_with('>') {
        1
    } else {
        return false;
    };
    let trigger_start = prefix.len().saturating_sub(trigger_len);

    !flowchart_scan_line(&prefix[..trigger_start]).inside_payload
}

fn flowchart_top_level_shape_object_value(prefix: &str) -> bool {
    let prefix = prefix.trim_end();
    let mut search_end = prefix.len();

    while let Some(marker) = prefix[..search_end].rfind("@{") {
        if shape_object_value_prefix(&prefix[marker..]).is_some()
            && !flowchart_scan_line(&prefix[..marker]).inside_payload
        {
            return true;
        }
        search_end = marker;
    }

    false
}

#[derive(Debug, Default)]
struct FlowchartLineScan {
    last_operator: Option<(usize, usize)>,
    inside_payload: bool,
}

fn flowchart_scan_line(prefix: &str) -> FlowchartLineScan {
    let mut scan = FlowchartLineScan::default();
    let mut state = FlowchartOperatorScanState::default();
    let mut cursor = 0usize;

    while cursor < prefix.len() {
        if state.is_operator_site()
            && let Some(operator) = flowchart_operator_at(prefix, cursor)
        {
            let start = cursor;
            cursor += operator.len();
            scan.last_operator = Some((start, cursor));

            let label_start = cursor + leading_whitespace_len(&prefix[cursor..]);
            if prefix[label_start..].starts_with('|') {
                let Some(label_len) = flowchart_edge_label_len(&prefix[label_start..]) else {
                    scan.inside_payload = true;
                    return scan;
                };
                cursor = label_start + label_len;
            }
            continue;
        }

        let Some(ch) = prefix[cursor..].chars().next() else {
            break;
        };
        state.accept(ch);
        cursor += ch.len_utf8();
    }

    scan.inside_payload = !state.is_operator_site();
    scan
}

#[derive(Debug, Default)]
struct FlowchartOperatorScanState {
    bracket_depth: usize,
    paren_depth: usize,
    brace_depth: usize,
    quote: Option<char>,
    escaped: bool,
}

impl FlowchartOperatorScanState {
    fn is_operator_site(&self) -> bool {
        self.quote.is_none()
            && self.bracket_depth == 0
            && self.paren_depth == 0
            && self.brace_depth == 0
    }

    fn accept(&mut self, ch: char) {
        if let Some(quote) = self.quote {
            if self.escaped {
                self.escaped = false;
                return;
            }
            if ch == '\\' {
                self.escaped = true;
                return;
            }
            if ch == quote {
                self.quote = None;
            }
            return;
        }

        match ch {
            '"' | '\'' | '`' => self.quote = Some(ch),
            '[' => self.bracket_depth += 1,
            ']' => self.bracket_depth = self.bracket_depth.saturating_sub(1),
            '(' => self.paren_depth += 1,
            ')' => self.paren_depth = self.paren_depth.saturating_sub(1),
            '{' => self.brace_depth += 1,
            '}' => self.brace_depth = self.brace_depth.saturating_sub(1),
            _ => {}
        }
    }
}

fn flowchart_operator_at(prefix: &str, offset: usize) -> Option<&'static str> {
    let tail = prefix.get(offset..)?;
    FLOWCHART_TARGET_OPERATORS
        .iter()
        .copied()
        .find(|operator| tail.starts_with(operator))
}

fn flowchart_edge_label_len(tail: &str) -> Option<usize> {
    let label = tail.strip_prefix('|')?;
    let close = label.find('|')?;
    Some(1 + close + 1)
}

fn leading_whitespace_len(input: &str) -> usize {
    input
        .chars()
        .take_while(|ch| ch.is_whitespace())
        .map(char::len_utf8)
        .sum()
}

fn is_flowchart_target_fragment_char(ch: char) -> bool {
    ch == '_' || ch == '-' || ch == '.' || ch == ':' || ch.is_alphanumeric()
}

const FLOWCHART_TARGET_OPERATORS: &[&str] = &[
    "-.->", "<-->", "-->", "---", "==>", "<--", "--x", "--o", "x--", "o--", "*--",
];

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
