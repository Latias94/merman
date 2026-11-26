//! Treemap row syntax and controlled construction of the semantic arena.

#[cfg(feature = "editor-facts")]
use merman_core_preprocess::scan::langium::LangiumCommonFact;
use merman_core_preprocess::scan::langium::{LangiumCommonField, parse_langium_common};
use merman_model::families::treemap::{TreemapClassDef, TreemapDiagramModel, TreemapNode};
use merman_model::{ManagedJson, OperationControl, OperationControlResult, SourceSpan};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemType {
    Section,
    Leaf,
}

#[derive(Debug, Clone)]
pub struct ClassDefStatement {
    pub class_name: SpannedText,
    pub style_text: Option<SpannedText>,
    #[cfg(feature = "editor-facts")]
    pub span: SourceSpan,
}

#[derive(Debug, Clone)]
pub struct ItemRow {
    indent: usize,
    pub name: SpannedText,
    pub item_type: ItemType,
    pub value: Option<SpannedValue>,
    pub class_selector: Option<SpannedText>,
    #[cfg(feature = "editor-facts")]
    pub span: SourceSpan,
}

#[derive(Debug, Clone)]
pub enum TreemapRow {
    Item(ItemRow),
    ClassDef(ClassDefStatement),
}

type StyleClassDef = TreemapClassDef;

#[derive(Debug, Clone)]
struct Arena {
    nodes: Vec<TreemapNode>,
}

impl Arena {
    fn push(&mut self, node: TreemapNode) -> usize {
        let idx = self.nodes.len();
        self.nodes.push(node);
        idx
    }
}

pub struct TreemapParsedInput {
    present: bool,
    title: Option<String>,
    acc_title: Option<String>,
    acc_descr: Option<String>,
    rows: Vec<TreemapRow>,
}

#[derive(Debug, Clone)]
pub struct TreemapParseIssue {
    pub message: String,
    pub span: Option<SourceSpan>,
}

#[cfg(feature = "editor-facts")]
pub enum TreemapSyntaxEvent<'a> {
    Common(&'a LangiumCommonFact),
    Row(&'a TreemapRow),
    Issue(&'a TreemapParseIssue),
}

#[cfg(feature = "editor-facts")]
pub struct TreemapSyntaxObserver<'a> {
    pub captures_rich: bool,
    pub observe: &'a mut dyn FnMut(TreemapSyntaxEvent<'_>),
}

pub struct TreemapSyntax {
    parsed: TreemapParsedInput,
    first_issue: Option<TreemapParseIssue>,
    #[cfg(test)]
    string2_scanned_bytes: usize,
}

impl TreemapSyntax {
    fn record_issue(
        &mut self,
        message: impl Into<String>,
        span: Option<SourceSpan>,
        #[cfg(feature = "editor-facts")] observer: Option<&mut TreemapSyntaxObserver<'_>>,
    ) {
        let issue = TreemapParseIssue {
            message: message.into(),
            span,
        };
        #[cfg(feature = "editor-facts")]
        if let Some(observer) = observer {
            (observer.observe)(TreemapSyntaxEvent::Issue(&issue));
        }
        if self.first_issue.is_none() {
            self.first_issue = Some(issue);
        }
    }

    pub fn into_strict(self) -> Result<TreemapParsedInput, TreemapParseIssue> {
        match self.first_issue {
            Some(issue) => Err(issue),
            None => Ok(self.parsed),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpannedText {
    pub text: String,
    pub span: SourceSpan,
}

#[derive(Debug, Clone)]
pub struct SpannedValue {
    value: Value,
    #[cfg(feature = "editor-facts")]
    pub source: SpannedText,
}

pub struct TreemapSemanticSource {
    present: bool,
    title: Option<String>,
    acc_title: Option<String>,
    acc_descr: Option<String>,
    class_defs: std::collections::HashMap<String, StyleClassDef>,
    arena: Arena,
    roots: Vec<usize>,
}

impl TreemapSemanticSource {
    pub const fn is_present(&self) -> bool {
        self.present
    }

    pub fn into_semantic_model_controlled(
        self,
        control: &OperationControl,
    ) -> OperationControlResult<TreemapDiagramModel> {
        let mut classes = std::collections::BTreeMap::new();
        for (id, class) in self.class_defs {
            control.checkpoint()?;
            classes.insert(id, class);
        }
        control.checkpoint()?;
        Ok(TreemapDiagramModel {
            title: self.title,
            acc_title: self.acc_title,
            acc_descr: self.acc_descr,
            root: TreemapNode {
                children: Some(self.roots),
                ..Default::default()
            },
            nodes: self.arena.nodes,
            classes,
        })
    }
}

pub fn treemap_semantic_source_from_parsed_controlled(
    parsed: TreemapParsedInput,
    control: &OperationControl,
) -> OperationControlResult<TreemapSemanticSource> {
    let class_defs = class_defs_from_rows_controlled(&parsed.rows, control)?;
    let flat_items = flat_items_from_rows_controlled(&parsed.rows, control)?;
    let (arena, roots) = build_hierarchy_controlled(&flat_items, control)?;
    control.checkpoint()?;
    Ok(TreemapSemanticSource {
        present: parsed.present,
        title: parsed.title,
        acc_title: parsed.acc_title,
        acc_descr: parsed.acc_descr,
        class_defs,
        arena,
        roots,
    })
}

/// Scans a Treemap source after the caller's operation-entry checkpoint.
pub fn scan_treemap_controlled(
    code: &str,
    #[cfg(feature = "editor-facts")] mut observer: Option<TreemapSyntaxObserver<'_>>,
    control: &OperationControl,
) -> OperationControlResult<TreemapSyntax> {
    let mut outcome = TreemapSyntax {
        parsed: TreemapParsedInput {
            present: false,
            title: None,
            acc_title: None,
            acc_descr: None,
            rows: Vec::new(),
        },
        first_issue: None,
        #[cfg(test)]
        string2_scanned_bytes: 0,
    };
    let body = match treemap_body_start_controlled(code, control)? {
        Ok(Some(body)) => body,
        Ok(None) => return Ok(outcome),
        Err(issue) => {
            outcome.record_issue(
                issue.message,
                issue.span,
                #[cfg(feature = "editor-facts")]
                observer.as_mut(),
            );
            return Ok(outcome);
        }
    };
    outcome.parsed.present = true;

    let mut offset = body.offset;
    let mut saw_statement = false;
    let mut trailing_whitespace_span = None;

    while offset < code.len() {
        control.checkpoint()?;
        if let Some(parsed) = parse_langium_common(code, offset) {
            saw_statement = true;
            trailing_whitespace_span = parsed.trailing_whitespace_span;
            let field = parsed.fact.field;
            let value = parsed.fact.value.clone();
            #[cfg(feature = "editor-facts")]
            if let Some(observer) = observer.as_mut().filter(|observer| observer.captures_rich) {
                (observer.observe)(TreemapSyntaxEvent::Common(&parsed.fact));
            }
            match field {
                LangiumCommonField::Title => outcome.parsed.title = Some(value),
                LangiumCommonField::AccTitle => outcome.parsed.acc_title = Some(value),
                LangiumCommonField::AccDescr => outcome.parsed.acc_descr = Some(value),
            }
            if let Some(diagnostic) = parsed.diagnostic {
                outcome.record_issue(
                    diagnostic.message,
                    Some(diagnostic.span),
                    #[cfg(feature = "editor-facts")]
                    observer.as_mut(),
                );
            }
            offset += parsed.consumed;
            continue;
        }

        let (line, next_offset) = physical_line_at(code, offset);
        if line.trim_start().starts_with("%%") {
            offset = next_offset;
            continue;
        }
        let visible = strip_inline_comment_aware(line);
        if visible.trim().is_empty() {
            if saw_statement && !visible.is_empty() {
                trailing_whitespace_span = Some(SourceSpan::new(offset, offset + line.len()));
            }
            offset = next_offset;
            continue;
        }
        saw_statement = true;
        trailing_whitespace_span = None;

        let (indent, rest_with_trailing) = split_ascii_indent(visible);
        let statement_start = offset + visible.len().saturating_sub(rest_with_trailing.len());
        let (statement, statement_next_offset) = treemap_item_statement_controlled(
            code,
            statement_start,
            rest_with_trailing,
            next_offset,
            control,
            #[cfg(test)]
            &mut outcome.string2_scanned_bytes,
        )?;
        let rest = strip_inline_comment_aware(statement).trim_end();
        let statement_span = SourceSpan::new(statement_start, statement_start + rest.len());
        let mut parsed_next_offset = next_offset;

        match parse_class_def(rest, statement_start) {
            Ok(Some(class_def)) => {
                let style_issue = class_def.style_text.as_ref().and_then(|style| {
                    validate_class_def_style(&style.text)
                        .err()
                        .map(|message| (message, style.span))
                });
                let row = TreemapRow::ClassDef(class_def);
                #[cfg(feature = "editor-facts")]
                if let Some(observer) = observer.as_mut().filter(|observer| observer.captures_rich)
                {
                    (observer.observe)(TreemapSyntaxEvent::Row(&row));
                }
                outcome.parsed.rows.push(row);
                if let Some((message, span)) = style_issue {
                    outcome.record_issue(
                        message,
                        Some(span),
                        #[cfg(feature = "editor-facts")]
                        observer.as_mut(),
                    );
                }
            }
            Ok(None) => match parse_item_row(indent, rest, statement_start) {
                Ok(item) => {
                    let row = TreemapRow::Item(item);
                    #[cfg(feature = "editor-facts")]
                    if let Some(observer) =
                        observer.as_mut().filter(|observer| observer.captures_rich)
                    {
                        (observer.observe)(TreemapSyntaxEvent::Row(&row));
                    }
                    outcome.parsed.rows.push(row);
                    parsed_next_offset = statement_next_offset;
                }
                Err(message) => {
                    outcome.record_issue(
                        message,
                        Some(statement_span),
                        #[cfg(feature = "editor-facts")]
                        observer.as_mut(),
                    );
                }
            },
            Err(message) => {
                outcome.record_issue(
                    message,
                    Some(statement_span),
                    #[cfg(feature = "editor-facts")]
                    observer.as_mut(),
                );
            }
        }
        offset = parsed_next_offset;
    }

    if let Some(span) = trailing_whitespace_span {
        outcome.record_issue(
            "unexpected trailing whitespace-only line",
            Some(span),
            #[cfg(feature = "editor-facts")]
            observer.as_mut(),
        );
    }

    control.checkpoint()?;
    Ok(outcome)
}

fn treemap_item_statement_controlled<'a>(
    source: &'a str,
    statement_start: usize,
    first_line: &'a str,
    next_offset: usize,
    control: &OperationControl,
    #[cfg(test)] scanned_bytes: &mut usize,
) -> OperationControlResult<(&'a str, usize)> {
    let name = first_line.trim_start();
    let Some(quote @ (b'\'' | b'"')) = name.as_bytes().first().copied() else {
        return Ok((first_line, next_offset));
    };
    let content_start = statement_start + first_line.len() - name.len() + 1;
    const SCAN_CHUNK_BYTES: usize = 4096;
    for (chunk_index, chunk) in source.as_bytes()[content_start..]
        .chunks(SCAN_CHUNK_BYTES)
        .enumerate()
    {
        control.checkpoint()?;
        let relative_close = chunk.iter().position(|&byte| byte == quote);
        #[cfg(test)]
        {
            *scanned_bytes += relative_close.map_or(chunk.len(), |index| index + 1);
        }
        if let Some(relative_close) = relative_close {
            let close_end = content_start + chunk_index * SCAN_CHUNK_BYTES + relative_close + 1;
            let (tail, next_offset) = physical_line_at(source, close_end);
            return Ok((
                &source[statement_start..close_end + tail.len()],
                next_offset,
            ));
        }
    }

    Ok((first_line, next_offset))
}

#[derive(Debug, Clone, Copy)]
struct TreemapBodyStart {
    offset: usize,
}

fn treemap_body_start_controlled(
    code: &str,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<Option<TreemapBodyStart>, TreemapParseIssue>> {
    let mut offset = 0usize;
    while offset < code.len() {
        control.checkpoint()?;
        let (line, next_offset) = physical_line_at(code, offset);
        let visible = strip_inline_comment_aware(line);
        let trimmed = visible.trim();
        if trimmed.is_empty() {
            offset = next_offset;
            continue;
        }
        let leading = visible.len().saturating_sub(visible.trim_start().len());
        let span = SourceSpan::new(offset + leading, offset + leading + trimmed.len());
        if !is_treemap_header(trimmed) {
            return Ok(Err(TreemapParseIssue {
                message: "expected treemap".to_string(),
                span: Some(span),
            }));
        }
        return Ok(Ok(Some(TreemapBodyStart {
            offset: next_offset,
        })));
    }
    Ok(Ok(None))
}

fn class_defs_from_rows_controlled(
    rows: &[TreemapRow],
    control: &OperationControl,
) -> OperationControlResult<std::collections::HashMap<String, StyleClassDef>> {
    let mut class_defs: std::collections::HashMap<String, StyleClassDef> =
        std::collections::HashMap::new();
    for (index, row) in rows.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        let TreemapRow::ClassDef(c) = row else {
            continue;
        };
        add_class(
            &mut class_defs,
            &c.class_name.text,
            c.style_text
                .as_ref()
                .map(|style| style.text.as_str())
                .unwrap_or(""),
        );
    }

    control.checkpoint()?;
    Ok(class_defs)
}

fn flat_items_from_rows_controlled(
    rows: &[TreemapRow],
    control: &OperationControl,
) -> OperationControlResult<Vec<FlatItem>> {
    let mut flat_items: Vec<FlatItem> = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        let TreemapRow::Item(item) = row else {
            continue;
        };

        flat_items.push(FlatItem {
            level: item.indent,
            name: item.name.text.clone(),
            item_type: item.item_type,
            value: item.value.as_ref().map(|value| value.value.clone()),
            class_selector: item
                .class_selector
                .as_ref()
                .map(|selector| selector.text.clone()),
        });
    }

    control.checkpoint()?;
    Ok(flat_items)
}

#[derive(Debug, Clone)]
struct FlatItem {
    level: usize,
    name: String,
    item_type: ItemType,
    value: Option<Value>,
    class_selector: Option<String>,
}

fn build_hierarchy_controlled(
    items: &[FlatItem],
    control: &OperationControl,
) -> OperationControlResult<(Arena, Vec<usize>)> {
    if items.is_empty() {
        return Ok((Arena { nodes: Vec::new() }, Vec::new()));
    }

    let mut arena = Arena { nodes: Vec::new() };
    let mut roots: Vec<usize> = Vec::new();
    let mut stack: Vec<(usize, usize)> = Vec::new(); // (node_idx, item.level)

    for (index, item) in items.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        let mut node = TreemapNode {
            name: item.name.clone(),
            value: None,
            class_selector: item.class_selector.clone(),
            children: match item.item_type {
                ItemType::Leaf => None,
                ItemType::Section => Some(Vec::new()),
            },
        };
        if item.item_type == ItemType::Leaf {
            node.value = item.value.as_ref().map(ManagedJson::from);
        }

        let idx = arena.push(node);

        while stack.last().is_some_and(|(_, lvl)| *lvl >= item.level) {
            control.checkpoint()?;
            stack.pop();
        }

        if stack.is_empty() {
            roots.push(idx);
        } else {
            let parent_idx = stack[stack.len() - 1].0;
            let parent = &mut arena.nodes[parent_idx];
            if let Some(children) = parent.children.as_mut() {
                children.push(idx);
            } else {
                parent.children = Some(vec![idx]);
            }
        }

        if item.item_type != ItemType::Leaf {
            stack.push((idx, item.level));
        }
    }

    control.checkpoint()?;
    Ok((arena, roots))
}

fn add_class(
    classes: &mut std::collections::HashMap<String, StyleClassDef>,
    id: &str,
    style: &str,
) {
    let style_class = classes
        .entry(id.to_string())
        .or_insert_with(|| StyleClassDef {
            id: id.to_string(),
            styles: Vec::new(),
            text_styles: Vec::new(),
        });

    const PLACEHOLDER: &str = "ก์ก์ก์";
    let replaced = style.replace("\\,", PLACEHOLDER);
    let replaced = replaced.replace(',', ";");
    let replaced = replaced.replace(PLACEHOLDER, ",");

    for s in replaced.split(';') {
        if is_label_style_bug_compatible(s) {
            style_class.text_styles.push(s.to_string());
        }
        style_class.styles.push(s.to_string());
    }
}

fn validate_class_def_style(style: &str) -> std::result::Result<(), String> {
    let style = style.trim().trim_end_matches(';').trim();
    if style.is_empty() {
        return Ok(());
    }

    const PLACEHOLDER: &str = "ก์ก์ก์";
    let replaced = style.replace("\\,", PLACEHOLDER);
    let replaced = replaced.replace(',', ";");
    let replaced = replaced.replace(PLACEHOLDER, ",");

    for raw in replaced.split(';') {
        let s = raw.trim();
        if s.is_empty() {
            continue;
        }
        let Some((k, v)) = s.split_once(':') else {
            return Err(format!("invalid classDef style token `{s}`"));
        };
        if k.trim().is_empty() || v.trim().is_empty() {
            return Err(format!("invalid classDef style token `{s}`"));
        }
    }

    Ok(())
}

fn is_label_style_bug_compatible(s: &str) -> bool {
    matches!(
        s.trim(),
        "color"
            | "font-size"
            | "font-family"
            | "font-weight"
            | "font-style"
            | "text-decoration"
            | "text-align"
            | "text-transform"
            | "line-height"
            | "letter-spacing"
            | "word-spacing"
            | "text-shadow"
            | "text-overflow"
            | "white-space"
            | "word-wrap"
            | "word-break"
            | "overflow-wrap"
            | "hyphens"
    )
}

fn physical_line_at(source: &str, offset: usize) -> (&str, usize) {
    let rest = &source[offset..];
    let (segment, next_offset) = match rest.find('\n') {
        Some(relative_newline) => (&rest[..=relative_newline], offset + relative_newline + 1),
        None => (rest, source.len()),
    };
    let line = segment.strip_suffix('\n').unwrap_or(segment);
    (line.strip_suffix('\r').unwrap_or(line), next_offset)
}

fn split_ascii_indent(line: &str) -> (usize, &str) {
    let indent = line
        .bytes()
        .take_while(|&byte| matches!(byte, b' ' | b'\t'))
        .count();
    (indent, &line[indent..])
}

fn strip_inline_comment_aware(line: &str) -> &str {
    let mut in_quote: Option<char> = None;

    let mut it = line.char_indices().peekable();
    while let Some((idx, ch)) = it.next() {
        if let Some(q) = in_quote {
            if ch == q {
                in_quote = None;
            }
            continue;
        }

        if ch == '"' || ch == '\'' {
            in_quote = Some(ch);
            continue;
        }

        if ch == '%' && it.peek().is_some_and(|(_, next)| *next == '%') {
            return &line[..idx];
        }
    }

    line
}

fn is_treemap_header(line: &str) -> bool {
    let t = line.trim_start();
    t == "treemap" || t == "treemap-beta"
}

fn parse_class_def(
    line: &str,
    line_start: usize,
) -> std::result::Result<Option<ClassDefStatement>, String> {
    let Some(after_keyword) = line.strip_prefix("classDef") else {
        return Ok(None);
    };
    if after_keyword
        .chars()
        .next()
        .is_some_and(|character| !character.is_whitespace())
    {
        return Ok(None);
    }
    if after_keyword.is_empty() {
        return Err("expected class name".to_string());
    }

    let class_input = after_keyword.trim_start();
    let class_start = line.len().saturating_sub(class_input.len());
    let (class_name, tail) =
        parse_id2(class_input).ok_or_else(|| "expected class name".to_string())?;
    let class_name_span = SourceSpan::new(
        line_start + class_start,
        line_start + class_start + class_name.len(),
    );

    let style_input = tail.trim_start();
    let style_input_start = line.len().saturating_sub(style_input.len());
    let semicolon = style_input.find(';');
    let (style_raw, trailing) = match semicolon {
        Some(semi) => (&style_input[..semi], &style_input[semi + 1..]),
        None => (style_input, ""),
    };
    let style = style_raw.trim();
    let style_text = if style.is_empty() {
        None
    } else {
        let leading = style_raw.len() - style_raw.trim_start().len();
        let start = line_start + style_input_start + leading;
        Some(SpannedText {
            text: style.to_string(),
            span: SourceSpan::new(start, start + style.len()),
        })
    };
    if !trailing.trim().is_empty() {
        return Err("unexpected tokens after classDef".to_string());
    }

    Ok(Some(ClassDefStatement {
        class_name: SpannedText {
            text: class_name,
            span: class_name_span,
        },
        style_text,
        #[cfg(feature = "editor-facts")]
        span: SourceSpan::new(line_start, line_start + line.len()),
    }))
}

fn parse_item_row(
    indent: usize,
    line: &str,
    line_start: usize,
) -> std::result::Result<ItemRow, String> {
    let mut p = Parser::new(line);
    p.skip_ws();
    let name_token_start = p.pos;
    let name_text = p
        .parse_string2()
        .ok_or_else(|| "expected quoted string".to_string())?;
    let name_token_end = p.pos;
    let quote_len = line[name_token_start..]
        .chars()
        .next()
        .map(char::len_utf8)
        .unwrap_or_default();
    let name = SpannedText {
        text: name_text,
        span: SourceSpan::new(
            line_start + name_token_start + quote_len,
            line_start + name_token_end.saturating_sub(quote_len),
        ),
    };
    p.skip_ws();

    // Section: "Name" (:::class)?
    if p.try_consume_str(":::") {
        p.skip_ws();
        let selector_start = p.pos;
        let (cls, _) = parse_id2(p.rest()).ok_or_else(|| "expected class selector".to_string())?;
        p.pos += cls.len();
        let selector_span = SourceSpan::new(
            line_start + selector_start,
            line_start + selector_start + cls.len(),
        );
        p.skip_ws();
        if !p.eof() {
            return Err("unexpected tokens after section".to_string());
        }
        return Ok(ItemRow {
            indent,
            name,
            item_type: ItemType::Section,
            value: None,
            class_selector: Some(SpannedText {
                span: selector_span,
                text: cls,
            }),
            #[cfg(feature = "editor-facts")]
            span: SourceSpan::new(line_start, line_start + line.len()),
        });
    }

    // Leaf: "Name" : 10 (:::class)?
    if p.try_consume(':') || p.try_consume(',') {
        p.skip_ws();
        #[cfg(feature = "editor-facts")]
        let value_start = p.pos;
        let token = p
            .parse_number2_token()
            .ok_or_else(|| "expected number".to_string())?;
        #[cfg(feature = "editor-facts")]
        let value_span = SourceSpan::new(
            line_start + value_start,
            line_start + value_start + token.len(),
        );
        let Some(value) = parse_number2_value(&token) else {
            return Err("expected number".to_string());
        };
        p.skip_ws();
        let mut class_selector = None;
        if p.try_consume_str(":::") {
            p.skip_ws();
            let selector_start = p.pos;
            let (cls, _) =
                parse_id2(p.rest()).ok_or_else(|| "expected class selector".to_string())?;
            p.pos += cls.len();
            let selector_span = SourceSpan::new(
                line_start + selector_start,
                line_start + selector_start + cls.len(),
            );
            class_selector = Some(SpannedText {
                span: selector_span,
                text: cls,
            });
            p.skip_ws();
        }
        if !p.eof() {
            return Err("unexpected tokens after leaf".to_string());
        }
        return Ok(ItemRow {
            indent,
            name,
            item_type: ItemType::Leaf,
            value: Some(SpannedValue {
                value,
                #[cfg(feature = "editor-facts")]
                source: SpannedText {
                    text: token.clone(),
                    span: value_span,
                },
            }),
            class_selector,
            #[cfg(feature = "editor-facts")]
            span: SourceSpan::new(line_start, line_start + line.len()),
        });
    }

    if p.eof() {
        return Ok(ItemRow {
            indent,
            name,
            item_type: ItemType::Section,
            value: None,
            class_selector: None,
            #[cfg(feature = "editor-facts")]
            span: SourceSpan::new(line_start, line_start + line.len()),
        });
    }

    Err("expected ':' or ':::' or end of line".to_string())
}

fn parse_id2(input: &str) -> Option<(String, &str)> {
    let mut chars = input.chars();
    let first = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    let mut idx = first.len_utf8();
    for ch in chars {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            idx += ch.len_utf8();
        } else {
            break;
        }
    }
    Some((input[..idx].to_string(), &input[idx..]))
}

fn parse_number2_value(token: &str) -> Option<Value> {
    let no_commas: String = token.chars().filter(|c| *c != ',').collect();
    let mut saw_dot = false;
    let mut cut = 0usize;
    for ch in no_commas.chars() {
        if ch.is_ascii_digit() {
            cut += ch.len_utf8();
            continue;
        }
        if ch == '.' && !saw_dot {
            saw_dot = true;
            cut += 1;
            continue;
        }
        break;
    }
    if cut == 0 {
        return None;
    }
    let prefix = &no_commas[..cut];
    let value: f64 = prefix.parse().ok()?;
    let number = serde_json::Number::from_f64(value)?;
    if value.fract() == 0.0 && value < i64::MAX as f64 {
        return Some(Value::Number((value as i64).into()));
    }
    Some(Value::Number(number))
}

struct Parser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn rest(&self) -> &'a str {
        &self.input[self.pos..]
    }

    fn skip_ws(&mut self) {
        while let Some(ch) = self.rest().chars().next() {
            if ch.is_whitespace() {
                self.pos += ch.len_utf8();
                continue;
            }
            break;
        }
    }

    fn try_consume(&mut self, ch: char) -> bool {
        if self.rest().starts_with(ch) {
            self.pos += ch.len_utf8();
            true
        } else {
            false
        }
    }

    fn try_consume_str(&mut self, s: &str) -> bool {
        if self.rest().starts_with(s) {
            self.pos += s.len();
            true
        } else {
            false
        }
    }

    fn parse_string2(&mut self) -> Option<String> {
        let rest = self.rest();
        let quote = rest.chars().next()?;
        if quote != '"' && quote != '\'' {
            return None;
        }
        let mut idx = 1usize;
        for ch in rest[1..].chars() {
            idx += ch.len_utf8();
            if ch == quote {
                let inner = &rest[1..idx - 1];
                self.pos += idx;
                return Some(inner.to_string());
            }
        }
        None
    }

    fn parse_number2_token(&mut self) -> Option<String> {
        let mut idx = 0usize;
        for ch in self.rest().chars() {
            if ch.is_ascii_digit() || ch == '_' || ch == '.' || ch == ',' {
                idx += ch.len_utf8();
                continue;
            }
            break;
        }
        if idx == 0 {
            return None;
        }
        let token = &self.rest()[..idx];
        self.pos += idx;
        Some(token.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(source: &str) -> TreemapSyntax {
        let control = OperationControl::new();
        control.checkpoint().unwrap();
        scan_treemap_controlled(
            source,
            #[cfg(feature = "editor-facts")]
            None,
            &control,
        )
        .unwrap()
    }

    fn item_names(syntax: &TreemapSyntax) -> Vec<&str> {
        syntax
            .parsed
            .rows
            .iter()
            .filter_map(|row| match row {
                TreemapRow::Item(item) => Some(item.name.text.as_str()),
                TreemapRow::ClassDef(_) => None,
            })
            .collect()
    }

    #[test]
    fn treemap_malformed_multiline_string2_keeps_following_safe_rows() {
        for eol in ["\n", "\r\n"] {
            for quote in ['"', '\''] {
                let broken = format!("{quote}Broken{eol}  suffix{quote}: _");
                let source = format!(
                    "treemap{eol}{quote}Before{quote}: 1{eol}{broken}{eol}{quote}After{quote}: 2{eol}"
                );
                let outcome = scan(&source);
                let issue = outcome.first_issue.as_ref().unwrap();
                assert_eq!(issue.message, "expected number");
                let start = source.find(&broken).unwrap();
                assert_eq!(
                    issue.span,
                    Some(SourceSpan::new(start, start + broken.len()))
                );
                assert_eq!(item_names(&outcome), ["Before", "After"]);
            }
        }
    }

    #[test]
    fn treemap_failed_string2_candidates_keep_nodes_with_linear_scan_work() {
        const ROWS: usize = 128;
        for quote in ['"', '\''] {
            let mut source = String::from("treemap\n");
            for index in 0..ROWS {
                source.push_str(&format!(
                    "{quote}Broken{index}\n{quote}After{index}{quote}: 1\n"
                ));
            }
            let other_quote = if quote == '"' { '\'' } else { '"' };
            source.push_str(&format!(
                "{quote}unclosed\n{other_quote}SafeSuffix{other_quote}: 2\n{other_quote}unclosed other\n"
            ));
            for index in 0..ROWS {
                source.push_str(&format!("classDef hot{index} fill:#f96;\n"));
            }
            source.push_str("title Recovered title\n");
            let outcome = scan(&source);
            assert!(outcome.first_issue.is_some());
            let names = item_names(&outcome);
            assert_eq!(names.len(), ROWS + 1);
            for (index, name) in names[..ROWS].iter().enumerate() {
                assert_eq!(*name, format!("After{index}"));
            }
            assert_eq!(names[ROWS], "SafeSuffix");
            assert_eq!(outcome.parsed.rows.len(), ROWS * 2 + 1);
            assert_eq!(outcome.parsed.title.as_deref(), Some("Recovered title"));
            assert!(
                outcome.string2_scanned_bytes <= source.len() * 2,
                "two quote kinds must inspect at most twice the source bytes: {} > {}",
                outcome.string2_scanned_bytes,
                source.len() * 2
            );
        }
    }
}
