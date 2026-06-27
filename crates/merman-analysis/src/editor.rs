use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteSpan {
    pub start: usize,
    pub end: usize,
}

impl ByteSpan {
    pub fn contains(self, offset: usize) -> bool {
        offset >= self.start && offset <= self.end
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EditorSymbolKind {
    Class,
    Event,
    Function,
    Module,
    Namespace,
    Object,
    Package,
    Property,
    String,
    Struct,
    Variable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FenceLineItem {
    pub name: String,
    pub detail: Option<String>,
    pub kind: EditorSymbolKind,
    pub span: ByteSpan,
    pub selection: ByteSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FenceSemanticRole {
    Entity,
    Outline,
    Payload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FenceSemanticItem {
    pub name: String,
    pub detail: Option<String>,
    pub kind: EditorSymbolKind,
    pub role: FenceSemanticRole,
    pub span: ByteSpan,
    pub selection: ByteSpan,
}

impl FenceSemanticItem {
    fn to_line_item(&self) -> FenceLineItem {
        FenceLineItem {
            name: self.name.clone(),
            detail: self.detail.clone(),
            kind: self.kind,
            span: self.span,
            selection: self.selection,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FenceReferenceGroup {
    pub name: String,
    pub kind: EditorSymbolKind,
}

impl FenceReferenceGroup {
    pub fn new(name: impl Into<String>, kind: EditorSymbolKind) -> Self {
        Self {
            name: name.into(),
            kind,
        }
    }

    pub fn from_semantic_item(item: &FenceSemanticItem) -> Self {
        Self::new(item.name.clone(), item.kind)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FenceTextIndexSource {
    #[default]
    TextScan,
    ParserComplete,
    ParserRecovered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FenceCursorCompletionKind {
    DiagramHeader,
    Operator,
    Directive,
    Direction,
    Shape,
    NodeIdentifier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FenceExpectedSyntaxKind {
    IdList,
    NodeIdentifier,
    Shape,
    ShapeTrigger,
    Direction,
    Payload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FenceExpectedSyntax {
    kind: FenceExpectedSyntaxKind,
    span: ByteSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FenceCursorContext {
    prefix: String,
    prefix_start: usize,
    cursor: usize,
    directive_prefix: Option<&'static str>,
    comment_or_directive_line: bool,
    expected_syntax: Option<FenceExpectedSyntaxKind>,
    expected_syntax_span: Option<ByteSpan>,
    completion_kinds: Vec<FenceCursorCompletionKind>,
}

impl FenceCursorContext {
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    pub fn prefix_start(&self) -> usize {
        self.prefix_start
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn directive_prefix(&self) -> Option<&'static str> {
        self.directive_prefix
    }

    pub fn is_comment_or_directive_line(&self) -> bool {
        self.comment_or_directive_line
    }

    pub fn expected_syntax(&self) -> Option<FenceExpectedSyntaxKind> {
        self.expected_syntax
    }

    pub fn expected_syntax_span(&self) -> Option<ByteSpan> {
        self.expected_syntax_span
    }

    pub fn completion_kinds(&self) -> &[FenceCursorCompletionKind] {
        &self.completion_kinds
    }

    pub fn offers(&self, kind: FenceCursorCompletionKind) -> bool {
        self.completion_kinds.contains(&kind)
    }
}

#[derive(Debug, Clone, Default)]
pub struct FenceTextIndex {
    node_ids: BTreeSet<String>,
    directive_prefixes: BTreeSet<String>,
    references: BTreeMap<FenceReferenceGroup, Vec<ByteSpan>>,
    outline_items: Vec<FenceLineItem>,
    semantic_items: Vec<FenceSemanticItem>,
    expected_syntax: Vec<FenceExpectedSyntax>,
    source: FenceTextIndexSource,
}

impl FenceTextIndex {
    pub fn from_text(text: &str, diagram_type: Option<&str>) -> Self {
        let mut index = Self::default();
        let mut relative_start = 0usize;

        for line in text.split_inclusive('\n') {
            let line_end = relative_start + line.len();
            let line_no_newline = line.strip_suffix('\n').unwrap_or(line);
            let trimmed = line_no_newline.trim_start();
            let leading = line_no_newline.len().saturating_sub(trimmed.len());
            let abs_start = relative_start + leading;
            let abs_end = line_end;

            index.record_line(diagram_type, line_no_newline, trimmed, abs_start, abs_end);
            relative_start = line_end;
        }

        if !text.ends_with('\n') && relative_start < text.len() {
            let line_no_newline = &text[relative_start..];
            let trimmed = line_no_newline.trim_start();
            let leading = line_no_newline.len().saturating_sub(trimmed.len());
            index.record_line(
                diagram_type,
                line_no_newline,
                trimmed,
                relative_start + leading,
                text.len(),
            );
        }

        index.outline_items.sort_by(|left, right| {
            (
                left.span.start,
                left.span.end,
                left.name.as_str(),
                left.selection.start,
                left.selection.end,
            )
                .cmp(&(
                    right.span.start,
                    right.span.end,
                    right.name.as_str(),
                    right.selection.start,
                    right.selection.end,
                ))
        });
        index.outline_items.dedup_by(|left, right| {
            left.span.start == right.span.start
                && left.span.end == right.span.end
                && left.name == right.name
        });

        index
    }

    pub fn from_core_facts(facts: merman_core::EditorSemanticFacts) -> Self {
        let mut index = Self::default();

        index.source = match facts.completeness {
            merman_core::EditorSemanticCompleteness::Complete => {
                FenceTextIndexSource::ParserComplete
            }
            merman_core::EditorSemanticCompleteness::Recovered => {
                FenceTextIndexSource::ParserRecovered
            }
        };
        index.directive_prefixes.extend(facts.directive_prefixes);
        index
            .expected_syntax
            .extend(
                facts
                    .expected_syntax
                    .into_iter()
                    .map(|expected| FenceExpectedSyntax {
                        kind: expected_syntax_kind_from_core(expected.kind),
                        span: ByteSpan {
                            start: expected.span.start,
                            end: expected.span.end,
                        },
                    }),
            );

        for symbol in facts.symbols {
            let role = symbol.role;
            let item = FenceSemanticItem {
                name: symbol.name,
                detail: symbol.detail,
                kind: editor_kind_from_core(symbol.kind),
                role: semantic_role_from_core(role),
                span: ByteSpan {
                    start: symbol.span.start,
                    end: symbol.span.end,
                },
                selection: ByteSpan {
                    start: symbol.selection.start,
                    end: symbol.selection.end,
                },
            };
            if role.contributes_references() {
                index
                    .references
                    .entry(FenceReferenceGroup::from_semantic_item(&item))
                    .or_default()
                    .push(item.selection);
            }
            if role.contributes_completion() {
                index.node_ids.insert(item.name.clone());
            }
            if role.contributes_outline() {
                index.outline_items.push(item.to_line_item());
            }
            index.semantic_items.push(item);
        }

        index.outline_items.sort_by(|left, right| {
            (
                left.span.start,
                left.span.end,
                left.name.as_str(),
                left.selection.start,
                left.selection.end,
            )
                .cmp(&(
                    right.span.start,
                    right.span.end,
                    right.name.as_str(),
                    right.selection.start,
                    right.selection.end,
                ))
        });
        index.semantic_items.sort_by(|left, right| {
            (
                left.span.start,
                left.span.end,
                left.name.as_str(),
                left.selection.start,
                left.selection.end,
            )
                .cmp(&(
                    right.span.start,
                    right.span.end,
                    right.name.as_str(),
                    right.selection.start,
                    right.selection.end,
                ))
        });
        index
    }

    pub fn node_ids(&self) -> impl Iterator<Item = &String> {
        self.node_ids.iter()
    }

    pub fn directive_prefixes(&self) -> impl Iterator<Item = &String> {
        self.directive_prefixes.iter()
    }

    pub fn has_directive_prefix(&self, prefix: &str) -> bool {
        self.directive_prefixes.contains(prefix)
    }

    pub fn first_reference_span(&self, name: &str) -> Option<ByteSpan> {
        self.references
            .iter()
            .find(|(group, _)| group.name == name)
            .map(|(_, spans)| spans)
            .and_then(|spans| spans.first().copied())
    }

    pub fn reference_spans(&self, name: &str) -> &[ByteSpan] {
        self.references
            .iter()
            .find(|(group, _)| group.name == name)
            .map(|(_, spans)| spans.as_slice())
            .unwrap_or(&[])
    }

    pub fn first_reference_span_for_item(&self, item: &FenceSemanticItem) -> Option<ByteSpan> {
        self.first_reference_span_in_group(&FenceReferenceGroup::from_semantic_item(item))
    }

    pub fn reference_spans_for_item(&self, item: &FenceSemanticItem) -> &[ByteSpan] {
        self.reference_spans_in_group(&FenceReferenceGroup::from_semantic_item(item))
    }

    pub fn first_reference_span_in_group(&self, group: &FenceReferenceGroup) -> Option<ByteSpan> {
        self.references
            .get(group)
            .and_then(|spans| spans.first().copied())
    }

    pub fn reference_spans_in_group(&self, group: &FenceReferenceGroup) -> &[ByteSpan] {
        self.references.get(group).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn symbol_at_offset(&self, offset: usize) -> Option<(String, ByteSpan)> {
        self.references.iter().find_map(|(group, spans)| {
            spans
                .iter()
                .copied()
                .find(|span| span.contains(offset))
                .map(|span| (group.name.clone(), span))
        })
    }

    pub fn semantic_item_at_offset(&self, offset: usize) -> Option<&FenceSemanticItem> {
        self.semantic_items
            .iter()
            .filter(|item| item.span.contains(offset))
            .min_by(|left, right| {
                let left_len = left.span.end.saturating_sub(left.span.start);
                let right_len = right.span.end.saturating_sub(right.span.start);
                (
                    left_len,
                    left.selection.start,
                    left.selection.end,
                    left.name.as_str(),
                )
                    .cmp(&(
                        right_len,
                        right.selection.start,
                        right.selection.end,
                        right.name.as_str(),
                    ))
            })
    }

    pub fn entity_item_at_offset(&self, offset: usize) -> Option<&FenceSemanticItem> {
        self.semantic_item_at_offset(offset)
            .filter(|item| item.role == FenceSemanticRole::Entity)
    }

    pub fn outline_items(&self) -> &[FenceLineItem] {
        &self.outline_items
    }

    pub fn semantic_items(&self) -> &[FenceSemanticItem] {
        &self.semantic_items
    }

    pub fn source(&self) -> FenceTextIndexSource {
        self.source
    }

    pub fn cursor_context(&self, text: &str, cursor_offset: usize) -> FenceCursorContext {
        let cursor = clamp_to_char_boundary(text, cursor_offset);
        let (prefix_start, prefix) = current_line_prefix(text, cursor);
        let directive_prefix = directive_prefix(&prefix);
        let comment_or_directive_line =
            prefix.trim_start().starts_with("%%") || directive_prefix.is_some();
        let mut completion_kinds = Vec::new();

        if offer_diagram_headers(&prefix) {
            completion_kinds.push(FenceCursorCompletionKind::DiagramHeader);
        }
        if offer_operator_items(&prefix) {
            completion_kinds.push(FenceCursorCompletionKind::Operator);
        }
        if offer_direction_items(&prefix) {
            completion_kinds.push(FenceCursorCompletionKind::Direction);
        }
        if offer_directive_items(&prefix, directive_prefix) {
            completion_kinds.push(FenceCursorCompletionKind::Directive);
        }
        if offer_shape_items(&prefix) {
            completion_kinds.push(FenceCursorCompletionKind::Shape);
        }
        if offer_node_items(&prefix, comment_or_directive_line) {
            completion_kinds.push(FenceCursorCompletionKind::NodeIdentifier);
        }
        let expected_syntax = self.expected_syntax_at_offset(cursor).copied();
        let expected_syntax_kind = expected_syntax.map(|expected| expected.kind);
        let expected_syntax_span = expected_syntax.map(|expected| expected.span);
        if let Some(expected_syntax) = expected_syntax_kind {
            apply_expected_syntax_to_completion(expected_syntax, &mut completion_kinds);
        }

        FenceCursorContext {
            prefix,
            prefix_start,
            cursor,
            directive_prefix,
            comment_or_directive_line,
            expected_syntax: expected_syntax_kind,
            expected_syntax_span,
            completion_kinds,
        }
    }

    fn expected_syntax_at_offset(&self, offset: usize) -> Option<&FenceExpectedSyntax> {
        self.expected_syntax
            .iter()
            .filter(|expected| expected.span.contains(offset))
            .min_by(|left, right| {
                let left_len = left.span.end.saturating_sub(left.span.start);
                let right_len = right.span.end.saturating_sub(right.span.start);
                (left_len, left.span.start, left.span.end).cmp(&(
                    right_len,
                    right.span.start,
                    right.span.end,
                ))
            })
    }

    fn record_line(
        &mut self,
        diagram_type: Option<&str>,
        line_no_newline: &str,
        trimmed: &str,
        abs_start: usize,
        abs_end: usize,
    ) {
        if let Some(prefix) = directive_prefix(line_no_newline) {
            self.directive_prefixes.insert(prefix.to_string());
            if is_payload_only_text_scan_prefix(prefix) {
                return;
            }
        }

        collect_node_ids(line_no_newline, &mut self.node_ids);

        if let Some(item) = classify_line_item(diagram_type, trimmed, abs_start, abs_end) {
            self.references
                .entry(FenceReferenceGroup::new(item.name.clone(), item.kind))
                .or_default()
                .push(item.selection);
            self.outline_items.push(item);
        }
    }
}

fn clamp_to_char_boundary(text: &str, offset: usize) -> usize {
    let mut cursor = offset.min(text.len());
    while cursor > 0 && !text.is_char_boundary(cursor) {
        cursor -= 1;
    }
    cursor
}

fn current_line_prefix(text: &str, cursor: usize) -> (usize, String) {
    let before = &text[..cursor];
    let line_start = before.rfind('\n').map(|index| index + 1).unwrap_or(0);
    let raw_prefix = &before[line_start..];
    let trimmed = raw_prefix.trim_start();
    let prefix_start = line_start + raw_prefix.len().saturating_sub(trimmed.len());

    (prefix_start, trimmed.to_string())
}

fn offer_diagram_headers(prefix: &str) -> bool {
    let prefix = prefix.trim_end();

    prefix.is_empty() || diagram_header_prefix_matches(prefix)
}

fn offer_operator_items(prefix: &str) -> bool {
    let prefix = prefix.trim_end();

    prefix.ends_with("--") || prefix.ends_with("->")
}

fn offer_directive_items(prefix: &str, directive_prefix: Option<&str>) -> bool {
    let prefix = prefix.trim_end();

    prefix.trim_start().starts_with("%%")
        || matches!(
            directive_prefix,
            Some("classDef") | Some("class") | Some("style") | Some("linkStyle") | Some(":::")
        )
}

fn offer_direction_items(prefix: &str) -> bool {
    prefix.trim_end() == "direction"
}

fn offer_shape_items(prefix: &str) -> bool {
    let prefix = prefix.trim_end();

    prefix.contains("@{ shape:")
        || prefix.ends_with("((")
        || prefix.ends_with("{{")
        || prefix.ends_with('[')
        || prefix.ends_with("[/")
        || prefix.ends_with("[\\")
        || prefix.ends_with('>')
}

fn offer_node_items(prefix: &str, comment_or_directive_line: bool) -> bool {
    let prefix = prefix.trim_end();

    !diagram_header_prefix_matches(prefix)
        && !offer_direction_items(prefix)
        && !comment_or_directive_line
        && !offer_shape_items(prefix)
}

fn diagram_header_prefix_matches(prefix: &str) -> bool {
    if prefix.is_empty() {
        return false;
    }

    [
        "flowchart TD",
        "sequenceDiagram",
        "stateDiagram-v2",
        "gantt",
        "mindmap",
    ]
    .iter()
    .any(|candidate| candidate.starts_with(prefix))
}

fn is_payload_only_text_scan_prefix(prefix: &str) -> bool {
    matches!(
        prefix,
        "init"
            | "initialize"
            | "wrap"
            | "linkStyle"
            | "click"
            | "accTitle"
            | "accDescr"
            | "accDescription"
            | "title"
    )
}

fn editor_kind_from_core(kind: merman_core::EditorSemanticKind) -> EditorSymbolKind {
    match kind {
        merman_core::EditorSemanticKind::Class => EditorSymbolKind::Class,
        merman_core::EditorSemanticKind::Event => EditorSymbolKind::Event,
        merman_core::EditorSemanticKind::Function => EditorSymbolKind::Function,
        merman_core::EditorSemanticKind::Module => EditorSymbolKind::Module,
        merman_core::EditorSemanticKind::Namespace => EditorSymbolKind::Namespace,
        merman_core::EditorSemanticKind::Object => EditorSymbolKind::Object,
        merman_core::EditorSemanticKind::Package => EditorSymbolKind::Package,
        merman_core::EditorSemanticKind::Property => EditorSymbolKind::Property,
        merman_core::EditorSemanticKind::String => EditorSymbolKind::String,
        merman_core::EditorSemanticKind::Struct => EditorSymbolKind::Struct,
        merman_core::EditorSemanticKind::Variable => EditorSymbolKind::Variable,
    }
}

fn semantic_role_from_core(role: merman_core::EditorSemanticRole) -> FenceSemanticRole {
    match role {
        merman_core::EditorSemanticRole::Entity => FenceSemanticRole::Entity,
        merman_core::EditorSemanticRole::Outline => FenceSemanticRole::Outline,
        merman_core::EditorSemanticRole::Payload => FenceSemanticRole::Payload,
    }
}

fn expected_syntax_kind_from_core(
    kind: merman_core::EditorExpectedSyntaxKind,
) -> FenceExpectedSyntaxKind {
    match kind {
        merman_core::EditorExpectedSyntaxKind::IdList => FenceExpectedSyntaxKind::IdList,
        merman_core::EditorExpectedSyntaxKind::NodeIdentifier => {
            FenceExpectedSyntaxKind::NodeIdentifier
        }
        merman_core::EditorExpectedSyntaxKind::ShapeValue => FenceExpectedSyntaxKind::Shape,
        merman_core::EditorExpectedSyntaxKind::ShapeTrigger => {
            FenceExpectedSyntaxKind::ShapeTrigger
        }
        merman_core::EditorExpectedSyntaxKind::DirectionValue => FenceExpectedSyntaxKind::Direction,
        merman_core::EditorExpectedSyntaxKind::Payload => FenceExpectedSyntaxKind::Payload,
    }
}

fn apply_expected_syntax_to_completion(
    expected: FenceExpectedSyntaxKind,
    completion_kinds: &mut Vec<FenceCursorCompletionKind>,
) {
    match expected {
        FenceExpectedSyntaxKind::IdList => {
            completion_kinds.clear();
            completion_kinds.push(FenceCursorCompletionKind::NodeIdentifier);
        }
        FenceExpectedSyntaxKind::NodeIdentifier => {
            completion_kinds.clear();
            completion_kinds.push(FenceCursorCompletionKind::NodeIdentifier);
        }
        FenceExpectedSyntaxKind::Shape => {
            completion_kinds.clear();
            completion_kinds.push(FenceCursorCompletionKind::Shape);
        }
        FenceExpectedSyntaxKind::ShapeTrigger => {
            completion_kinds.clear();
            completion_kinds.push(FenceCursorCompletionKind::Shape);
        }
        FenceExpectedSyntaxKind::Direction => {
            completion_kinds.clear();
            completion_kinds.push(FenceCursorCompletionKind::Direction);
        }
        FenceExpectedSyntaxKind::Payload => completion_kinds.clear(),
    }
}

fn directive_prefix(line: &str) -> Option<&'static str> {
    let trimmed = line.trim_start();

    if let Some(rest) = trimmed.strip_prefix("%%{") {
        let name = rest
            .split(|ch: char| ch.is_whitespace() || matches!(ch, ':' | '}'))
            .next()
            .filter(|name| !name.is_empty())?;

        return matches!(name, "init" | "initialize" | "wrap").then_some(match name {
            "init" => "init",
            "initialize" => "initialize",
            "wrap" => "wrap",
            _ => unreachable!(),
        });
    }

    if trimmed.starts_with(":::") {
        return Some(":::");
    }

    for prefix in [
        "classDef",
        "class",
        "style",
        "linkStyle",
        "click",
        "accTitle",
        "accDescr",
        "accDescription",
        "title",
    ] {
        if has_word_boundary(trimmed, prefix) {
            return Some(prefix);
        }
    }

    None
}

fn has_word_boundary(text: &str, prefix: &str) -> bool {
    text.strip_prefix(prefix).is_some_and(|rest| {
        rest.is_empty()
            || rest
                .chars()
                .next()
                .is_some_and(|ch| ch.is_whitespace() || matches!(ch, ':' | '{'))
    })
}

fn is_candidate_node_id(token: &str) -> bool {
    if token.is_empty() || token.starts_with('%') {
        return false;
    }

    !matches!(
        token,
        "flowchart"
            | "graph"
            | "sequenceDiagram"
            | "stateDiagram"
            | "stateDiagram-v2"
            | "mindmap"
            | "TD"
            | "TB"
            | "BT"
            | "LR"
            | "RL"
            | "classDef"
            | "class"
            | "style"
            | "linkStyle"
            | "init"
            | "initialize"
            | "wrap"
    )
}

fn collect_node_ids(text: &str, ids: &mut BTreeSet<String>) {
    for token in text.split(|ch: char| {
        ch.is_whitespace()
            || matches!(
                ch,
                '[' | ']'
                    | '('
                    | ')'
                    | '{'
                    | '}'
                    | '-'
                    | '='
                    | '.'
                    | '<'
                    | '>'
                    | '|'
                    | ':'
                    | ','
                    | ';'
            )
    }) {
        if is_candidate_node_id(token) {
            ids.insert(token.to_string());
        }
    }
}

fn classify_line_item(
    diagram_type: Option<&str>,
    trimmed: &str,
    abs_start: usize,
    abs_end: usize,
) -> Option<FenceLineItem> {
    if trimmed.is_empty()
        || is_header_line(trimmed)
        || trimmed.starts_with("%%")
        || trimmed.starts_with(":::")
    {
        return None;
    }

    if let Some(rest) = trimmed.strip_prefix("subgraph ") {
        let (name, selection) = token_after_prefix(trimmed, "subgraph", abs_start)?;
        return Some(FenceLineItem {
            name: if rest.trim().is_empty() {
                "subgraph".to_string()
            } else {
                name
            },
            detail: Some("subgraph".to_string()),
            kind: EditorSymbolKind::Namespace,
            span: ByteSpan {
                start: abs_start,
                end: abs_end,
            },
            selection,
        });
    }

    if let Some((keyword, kind, detail)) = [
        (
            "participant",
            EditorSymbolKind::Variable,
            "sequence participant",
        ),
        ("actor", EditorSymbolKind::Variable, "sequence actor"),
        ("box", EditorSymbolKind::Package, "sequence box"),
        ("note", EditorSymbolKind::Event, "note"),
        ("state", EditorSymbolKind::Class, "state"),
        ("classDef", EditorSymbolKind::Property, "class definition"),
        ("class", EditorSymbolKind::Class, "class assignment"),
        ("style", EditorSymbolKind::Property, "style"),
        ("click", EditorSymbolKind::Function, "interaction"),
        ("linkStyle", EditorSymbolKind::Property, "link style"),
        ("accTitle", EditorSymbolKind::String, "accessibility title"),
        (
            "accDescr",
            EditorSymbolKind::String,
            "accessibility description",
        ),
        ("title", EditorSymbolKind::String, "title"),
    ]
    .into_iter()
    .find_map(|(keyword, kind, detail)| {
        trimmed
            .strip_prefix(keyword)
            .map(|_| (keyword, kind, detail))
    }) {
        let (name, selection) = token_after_prefix(trimmed, keyword, abs_start)?;
        return Some(FenceLineItem {
            name,
            detail: Some(detail.to_string()),
            kind,
            span: ByteSpan {
                start: abs_start,
                end: abs_end,
            },
            selection,
        });
    }

    if matches!(diagram_type, Some("mindmap")) {
        let (name, selection) = first_symbol_token(trimmed, abs_start)?;
        return Some(FenceLineItem {
            name,
            detail: Some("mindmap node".to_string()),
            kind: EditorSymbolKind::String,
            span: ByteSpan {
                start: abs_start,
                end: abs_end,
            },
            selection,
        });
    }

    let (name, selection) = first_symbol_token(trimmed, abs_start)?;
    Some(FenceLineItem {
        name,
        detail: Some("diagram element".to_string()),
        kind: generic_kind(diagram_type),
        span: ByteSpan {
            start: abs_start,
            end: abs_end,
        },
        selection,
    })
}

fn first_symbol_token(trimmed: &str, abs_start: usize) -> Option<(String, ByteSpan)> {
    let mut token_end = 0usize;
    for (idx, ch) in trimmed.char_indices() {
        if idx == 0 && matches!(ch, '[' | '(' | '{' | '<' | ':' | '%' | ';') {
            token_end = ch.len_utf8();
            break;
        }
        if ch.is_whitespace()
            || matches!(
                ch,
                '[' | ']'
                    | '('
                    | ')'
                    | '{'
                    | '}'
                    | '-'
                    | '='
                    | '.'
                    | '<'
                    | '>'
                    | '|'
                    | ':'
                    | ','
                    | ';'
            )
        {
            token_end = idx;
            break;
        }
        token_end = idx + ch.len_utf8();
    }

    if token_end == 0 {
        token_end = trimmed.len();
    }

    let token = trimmed[..token_end].trim_matches(|ch: char| matches!(ch, '[' | ']' | '(' | ')'));
    if token.is_empty() || token.starts_with('%') || is_header_line(token) {
        return None;
    }

    let leading = trimmed.len().saturating_sub(trimmed.trim_start().len());
    Some((
        token.to_string(),
        ByteSpan {
            start: abs_start + leading,
            end: abs_start + leading + token.len(),
        },
    ))
}

fn token_after_prefix(trimmed: &str, prefix: &str, abs_start: usize) -> Option<(String, ByteSpan)> {
    let rest = trimmed.strip_prefix(prefix)?.trim_start();
    let rest_offset = trimmed.len().saturating_sub(rest.len());
    let token = rest
        .split(|ch: char| ch.is_whitespace() || matches!(ch, ':' | '{' | '(' | '['))
        .next()
        .filter(|token| !token.is_empty())?;

    Some((
        token.to_string(),
        ByteSpan {
            start: abs_start + rest_offset,
            end: abs_start + rest_offset + token.len(),
        },
    ))
}

fn is_header_line(trimmed: &str) -> bool {
    matches!(
        trimmed,
        "flowchart"
            | "flowchart TD"
            | "flowchart TB"
            | "flowchart BT"
            | "flowchart LR"
            | "flowchart RL"
            | "graph"
            | "graph TD"
            | "graph TB"
            | "graph BT"
            | "graph LR"
            | "graph RL"
            | "sequenceDiagram"
            | "stateDiagram"
            | "stateDiagram-v2"
            | "mindmap"
            | "classDiagram"
            | "erDiagram"
            | "gantt"
            | "block-beta"
            | "journey"
            | "timeline"
            | "pie"
            | "quadrantChart"
            | "xychart-beta"
            | "C4Context"
            | "C4Container"
            | "C4Component"
            | "C4Dynamic"
    ) || trimmed.starts_with("flowchart ")
        || trimmed.starts_with("graph ")
        || trimmed.starts_with("sequenceDiagram ")
        || trimmed.starts_with("stateDiagram ")
        || trimmed.starts_with("stateDiagram-v2 ")
        || trimmed.starts_with("classDiagram ")
        || trimmed.starts_with("erDiagram ")
        || trimmed.starts_with("mindmap ")
        || trimmed.starts_with("gantt ")
        || trimmed.starts_with("block-beta ")
        || trimmed.starts_with("journey ")
        || trimmed.starts_with("timeline ")
        || trimmed.starts_with("pie ")
        || trimmed.starts_with("quadrantChart ")
        || trimmed.starts_with("xychart-beta ")
        || trimmed.starts_with("C4")
}

fn generic_kind(diagram_type: Option<&str>) -> EditorSymbolKind {
    match diagram_type {
        Some("sequence") => EditorSymbolKind::Event,
        Some("state") => EditorSymbolKind::Class,
        Some("mindmap") => EditorSymbolKind::Namespace,
        Some("class") => EditorSymbolKind::Class,
        Some("er") => EditorSymbolKind::Struct,
        Some("block") => EditorSymbolKind::Object,
        Some("flowchart-v2") | Some("flowchart-elk") => EditorSymbolKind::Module,
        _ => EditorSymbolKind::Variable,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ByteSpan, EditorSymbolKind, FenceCursorCompletionKind, FenceExpectedSyntaxKind,
        FenceSemanticRole, FenceTextIndex, FenceTextIndexSource, is_candidate_node_id,
    };
    use merman_core::{
        EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
        EditorSemanticSymbol, SourceSpan,
    };

    #[test]
    fn text_index_collects_node_ids() {
        let index = FenceTextIndex::from_text("flowchart TD\nA-->B\nB-->C\n", Some("flowchart-v2"));
        let ids = index.node_ids().cloned().collect::<Vec<_>>();

        assert_eq!(ids, vec!["A", "B", "C"]);
    }

    #[test]
    fn node_id_filter_skips_keywords_and_empty_tokens() {
        assert!(!is_candidate_node_id("flowchart"));
        assert!(!is_candidate_node_id("%comment"));
        assert!(is_candidate_node_id("node_1"));
    }

    #[test]
    fn text_index_tracks_directive_prefixes() {
        let index = FenceTextIndex::from_text(
            "%%{init: {\"theme\": \"dark\"}}%%\nclassDef foo fill:#f00\n:::className\n",
            None,
        );

        assert!(index.has_directive_prefix("init"));
        assert!(index.has_directive_prefix("classDef"));
        assert!(index.has_directive_prefix(":::"));
    }

    #[test]
    fn text_scan_records_payload_directive_prefixes_without_projecting_payload_symbols() {
        let index = FenceTextIndex::from_text(
            concat!(
                "flowchart TD\n",
                "click A href \"https://example.com\" \"Open user\" _blank\n",
                "linkStyle 0 stroke:#111,stroke-width:2px\n",
                "accTitle: Chart title\n",
                "accDescr: Chart description\n",
                "title Roadmap\n",
            ),
            Some("flowchart-v2"),
        );

        for prefix in ["click", "linkStyle", "accTitle", "accDescr", "title"] {
            assert!(index.has_directive_prefix(prefix));
        }
        for leaked in [
            "A", "href", "https", "example", "Open", "user", "_blank", "stroke", "Chart", "Roadmap",
        ] {
            assert!(
                !index.node_ids().any(|id| id == leaked),
                "text-scan payload directive leaked {leaked:?} as a node id"
            );
        }
        assert!(index.outline_items().is_empty());
    }

    #[test]
    fn text_scan_skips_non_symbol_directive_prefixes() {
        let index = FenceTextIndex::from_text(
            concat!(
                "%%{initialize: {\"theme\": \"dark\"}}%%\n",
                "%%{wrap}%%\n",
                "flowchart TD\n",
                "A-->B\n",
            ),
            Some("flowchart-v2"),
        );

        assert!(index.has_directive_prefix("initialize"));
        assert!(index.has_directive_prefix("wrap"));
        assert_eq!(
            index.node_ids().cloned().collect::<Vec<_>>(),
            vec!["A", "B"]
        );
        assert!(
            !index
                .outline_items()
                .iter()
                .any(|item| matches!(item.name.as_str(), "initialize" | "wrap"))
        );
    }

    #[test]
    fn cursor_context_classifies_header_operator_directive_and_nodes() {
        let index = FenceTextIndex::from_text("flowchart TD\nA-->B\n", Some("flowchart-v2"));

        let header = index.cursor_context("flow", 4);
        assert_eq!(header.prefix(), "flow");
        assert_eq!(header.prefix_start(), 0);
        assert!(header.offers(FenceCursorCompletionKind::DiagramHeader));
        assert!(!header.offers(FenceCursorCompletionKind::NodeIdentifier));

        let ambiguous = index.cursor_context("flowchart TD\nA-", "flowchart TD\nA-".len());
        assert!(!ambiguous.offers(FenceCursorCompletionKind::Operator));
        assert!(ambiguous.offers(FenceCursorCompletionKind::NodeIdentifier));

        let operator = index.cursor_context("flowchart TD\nA-->B", "flowchart TD\nA--".len());
        assert!(operator.offers(FenceCursorCompletionKind::Operator));
        assert!(operator.offers(FenceCursorCompletionKind::NodeIdentifier));

        let directive = index.cursor_context("classDef foo fill:#f00", "classDef foo".len());
        assert_eq!(directive.directive_prefix(), Some("classDef"));
        assert!(directive.is_comment_or_directive_line());
        assert!(directive.offers(FenceCursorCompletionKind::Directive));
        assert!(!directive.offers(FenceCursorCompletionKind::NodeIdentifier));

        let node = index.cursor_context("node_1", "node_1".len());
        assert!(node.offers(FenceCursorCompletionKind::NodeIdentifier));
    }

    #[test]
    fn cursor_context_uses_fence_local_offsets_and_shape_context() {
        let index = FenceTextIndex::from_text("A@{ shape: ", Some("flowchart-v2"));
        let context = index.cursor_context("  A@{ shape: ", "  A@{ shape: ".len());

        assert_eq!(context.prefix(), "A@{ shape: ");
        assert_eq!(context.prefix_start(), 2);
        assert_eq!(context.cursor(), "  A@{ shape: ".len());
        assert!(context.offers(FenceCursorCompletionKind::Shape));
        assert!(!context.offers(FenceCursorCompletionKind::NodeIdentifier));
    }

    #[test]
    fn cursor_context_clamps_to_utf8_char_boundaries() {
        let text = "\u{8282}\u{70b9}";
        let index = FenceTextIndex::from_text(text, Some("flowchart-v2"));
        let context = index.cursor_context(text, 1);

        assert_eq!(context.cursor(), 0);
        assert_eq!(context.prefix(), "");
        assert!(context.offers(FenceCursorCompletionKind::DiagramHeader));
    }

    #[test]
    fn cursor_context_uses_parser_expected_payload_to_suppress_generic_completion() {
        let mut facts = EditorSemanticFacts::new();
        facts.push_symbol(EditorSemanticSymbol::new(
            "Alice",
            Some("sequence participant".to_string()),
            EditorSemanticKind::Event,
            SourceSpan::new(16, 21),
            SourceSpan::new(16, 21),
        ));
        facts.push_expected_syntax(merman_core::EditorExpectedSyntax::new(
            merman_core::EditorExpectedSyntaxKind::Payload,
            SourceSpan::new(28, 33),
        ));
        let index = FenceTextIndex::from_core_facts(facts);
        let context = index.cursor_context("sequenceDiagram\nAlice->Bob: Hello", 31);

        assert_eq!(
            context.expected_syntax(),
            Some(FenceExpectedSyntaxKind::Payload)
        );
        assert!(context.completion_kinds().is_empty());
        assert!(!context.offers(FenceCursorCompletionKind::NodeIdentifier));
        assert!(!context.offers(FenceCursorCompletionKind::DiagramHeader));
    }

    #[test]
    fn cursor_context_uses_parser_expected_node_identifier_to_override_generic_completion() {
        let mut facts = EditorSemanticFacts::new();
        facts.push_symbol(EditorSemanticSymbol::new(
            "A",
            Some("flowchart node".to_string()),
            EditorSemanticKind::Module,
            SourceSpan::new(13, 14),
            SourceSpan::new(13, 14),
        ));
        facts.push_expected_syntax(merman_core::EditorExpectedSyntax::new(
            merman_core::EditorExpectedSyntaxKind::NodeIdentifier,
            SourceSpan::new(17, 18),
        ));
        let index = FenceTextIndex::from_core_facts(facts);
        let context = index.cursor_context("flowchart TD\nA--> ", 17);

        assert_eq!(
            context.expected_syntax(),
            Some(FenceExpectedSyntaxKind::NodeIdentifier)
        );
        assert_eq!(
            context.completion_kinds(),
            vec![FenceCursorCompletionKind::NodeIdentifier]
        );
        assert!(context.offers(FenceCursorCompletionKind::NodeIdentifier));
        assert!(!context.offers(FenceCursorCompletionKind::Operator));
    }

    #[test]
    fn cursor_context_uses_parser_expected_shape_value_to_override_generic_completion() {
        let mut facts = EditorSemanticFacts::new();
        let text = "flowchart TD\nA@{\n  shape: rou\n}\n";
        let value_start = text.find("rou").unwrap();
        facts.push_expected_syntax(merman_core::EditorExpectedSyntax::new(
            merman_core::EditorExpectedSyntaxKind::ShapeValue,
            SourceSpan::new(value_start, value_start + "rou".len()),
        ));
        let index = FenceTextIndex::from_core_facts(facts);
        let context = index.cursor_context(text, value_start + 2);

        assert_eq!(
            context.expected_syntax(),
            Some(FenceExpectedSyntaxKind::Shape)
        );
        assert_eq!(
            context.completion_kinds(),
            vec![FenceCursorCompletionKind::Shape]
        );
        assert!(context.offers(FenceCursorCompletionKind::Shape));
        assert!(!context.offers(FenceCursorCompletionKind::NodeIdentifier));
    }

    #[test]
    fn cursor_context_uses_parser_expected_shape_trigger_to_override_generic_completion() {
        let mut facts = EditorSemanticFacts::new();
        let text = "flowchart TD\nA((\n";
        let trigger_start = text.find("((").unwrap();
        facts.push_expected_syntax(merman_core::EditorExpectedSyntax::new(
            merman_core::EditorExpectedSyntaxKind::ShapeTrigger,
            SourceSpan::new(trigger_start, trigger_start + 2),
        ));
        let index = FenceTextIndex::from_core_facts(facts);
        let context = index.cursor_context(text, trigger_start + 2);

        assert_eq!(
            context.expected_syntax(),
            Some(FenceExpectedSyntaxKind::ShapeTrigger)
        );
        assert_eq!(
            context.completion_kinds(),
            vec![FenceCursorCompletionKind::Shape]
        );
        assert!(context.offers(FenceCursorCompletionKind::Shape));
        assert!(!context.offers(FenceCursorCompletionKind::NodeIdentifier));
    }

    #[test]
    fn cursor_context_uses_parser_expected_direction_value_to_override_generic_completion() {
        let mut facts = EditorSemanticFacts::new();
        let text = "flowchart TD\nsubgraph group\ndirection LR\nend\n";
        let value_start = text.find("LR").unwrap();
        facts.push_expected_syntax(merman_core::EditorExpectedSyntax::new(
            merman_core::EditorExpectedSyntaxKind::DirectionValue,
            SourceSpan::new(value_start, value_start + "LR".len()),
        ));
        let index = FenceTextIndex::from_core_facts(facts);
        let context = index.cursor_context(text, value_start + 1);

        assert_eq!(
            context.expected_syntax(),
            Some(FenceExpectedSyntaxKind::Direction)
        );
        assert_eq!(
            context.completion_kinds(),
            vec![FenceCursorCompletionKind::Direction]
        );
        assert!(context.offers(FenceCursorCompletionKind::Direction));
        assert!(!context.offers(FenceCursorCompletionKind::NodeIdentifier));
    }

    #[test]
    fn cursor_context_uses_parser_expected_id_list_to_override_directive_completion() {
        let mut facts = EditorSemanticFacts::new();
        let text = "erDiagram\nclassDef pink fill:#f9f";
        let expected_start = text.find("pink").unwrap();
        facts.push_expected_syntax(EditorExpectedSyntax::new(
            EditorExpectedSyntaxKind::IdList,
            SourceSpan::new(expected_start, expected_start + "pink".len()),
        ));
        let index = FenceTextIndex::from_core_facts(facts);
        let context = index.cursor_context(text, expected_start);

        assert_eq!(
            context.expected_syntax(),
            Some(FenceExpectedSyntaxKind::IdList)
        );
        assert_eq!(
            context.completion_kinds(),
            vec![FenceCursorCompletionKind::NodeIdentifier]
        );
        assert!(context.offers(FenceCursorCompletionKind::NodeIdentifier));
        assert!(!context.offers(FenceCursorCompletionKind::Directive));
    }

    #[test]
    fn text_index_projects_core_editor_facts() {
        let mut facts = EditorSemanticFacts::new();
        facts.push_directive_prefix("classDef");
        facts.push_symbol(EditorSemanticSymbol::new(
            "A",
            Some("flowchart node".to_string()),
            EditorSemanticKind::Module,
            SourceSpan::new(13, 14),
            SourceSpan::new(13, 14),
        ));

        let index = FenceTextIndex::from_core_facts(facts);

        assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
        assert!(index.node_ids().any(|id| id == "A"));
        assert_eq!(index.first_reference_span("A").unwrap().start, 13);
        assert_eq!(
            index.outline_items()[0].detail.as_deref(),
            Some("flowchart node")
        );
        assert!(index.has_directive_prefix("classDef"));
    }

    #[test]
    fn typed_reference_groups_separate_same_name_different_kinds() {
        let mut facts = EditorSemanticFacts::new();
        facts.push_symbol(EditorSemanticSymbol::new(
            "Shared",
            Some("module entity".to_string()),
            EditorSemanticKind::Module,
            SourceSpan::new(0, 6),
            SourceSpan::new(0, 6),
        ));
        facts.push_symbol(EditorSemanticSymbol::new(
            "Shared",
            Some("property entity".to_string()),
            EditorSemanticKind::Property,
            SourceSpan::new(7, 13),
            SourceSpan::new(7, 13),
        ));

        let index = FenceTextIndex::from_core_facts(facts);
        let module_item = index
            .semantic_items()
            .iter()
            .find(|item| item.kind == EditorSymbolKind::Module)
            .unwrap();
        let property_item = index
            .semantic_items()
            .iter()
            .find(|item| item.kind == EditorSymbolKind::Property)
            .unwrap();

        assert_eq!(
            index.reference_spans_for_item(module_item),
            &[ByteSpan { start: 0, end: 6 }]
        );
        assert_eq!(
            index.reference_spans_for_item(property_item),
            &[ByteSpan { start: 7, end: 13 }]
        );
        assert_eq!(
            index.first_reference_span_for_item(module_item),
            Some(ByteSpan { start: 0, end: 6 })
        );
        assert_eq!(
            index.first_reference_span_for_item(property_item),
            Some(ByteSpan { start: 7, end: 13 })
        );
        assert_eq!(index.reference_spans("Shared").len(), 1);
    }

    #[test]
    fn text_index_skips_payload_only_core_facts_for_completion() {
        let mut facts = EditorSemanticFacts::new();
        facts.push_symbol(EditorSemanticSymbol::outline(
            "section",
            Some("gantt section".to_string()),
            EditorSemanticKind::Namespace,
            SourceSpan::new(0, 7),
            SourceSpan::new(0, 7),
        ));
        facts.push_symbol(EditorSemanticSymbol::payload(
            "PK",
            Some("er attribute key".to_string()),
            EditorSemanticKind::Property,
            SourceSpan::new(8, 10),
            SourceSpan::new(8, 10),
        ));

        let index = FenceTextIndex::from_core_facts(facts);

        assert!(!index.node_ids().any(|id| id == "PK"));
        assert!(!index.node_ids().any(|id| id == "section"));
        assert!(
            index
                .semantic_items()
                .iter()
                .any(|item| item.name == "section" && item.role == FenceSemanticRole::Outline)
        );
        assert!(
            index
                .semantic_items()
                .iter()
                .any(|item| item.name == "PK" && item.role == FenceSemanticRole::Payload)
        );
        assert_eq!(
            index
                .semantic_item_at_offset(9)
                .map(|item| item.name.as_str()),
            Some("PK")
        );
        assert_eq!(index.entity_item_at_offset(9), None);
        assert_eq!(index.symbol_at_offset(9), None);
        assert!(
            index
                .outline_items()
                .iter()
                .any(|item| item.name == "section")
        );
        assert!(!index.outline_items().iter().any(|item| item.name == "PK"));
    }
}
