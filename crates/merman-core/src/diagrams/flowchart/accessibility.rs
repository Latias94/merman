#[cfg(test)]
use std::cell::Cell;

use crate::{EditorLexemeKind, SourceSpan};

#[cfg(test)]
thread_local! {
    static FLOWCHART_ACCESSIBILITY_SCAN_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(super) fn reset_flowchart_accessibility_scan_count() {
    FLOWCHART_ACCESSIBILITY_SCAN_COUNT.set(0);
}

#[cfg(test)]
pub(super) fn flowchart_accessibility_scan_count() -> usize {
    FLOWCHART_ACCESSIBILITY_SCAN_COUNT.get()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FlowchartAccessibilityDirective {
    Title,
    Description,
}

impl FlowchartAccessibilityDirective {
    pub(super) const fn prefix(self) -> &'static str {
        match self {
            Self::Title => "accTitle",
            Self::Description => "accDescr",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct FlowchartAccessibilityLexeme {
    pub(super) kind: EditorLexemeKind,
    pub(super) span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FlowchartAccessibilityStatement {
    pub(super) directive: FlowchartAccessibilityDirective,
    pub(super) complete: bool,
    pub(super) lexemes: Vec<FlowchartAccessibilityLexeme>,
}

/// One source-backed interpretation of Flowchart accessibility statements.
///
/// The masked parser input preserves byte offsets and newlines. Semantic values and editor
/// directive facts are projections of the same recognized statements, so they cannot drift.
pub(super) struct FlowchartAccessibilityScan {
    pub(super) parser_input: String,
    pub(super) title: Option<String>,
    pub(super) description: Option<String>,
    pub(super) statements: Vec<FlowchartAccessibilityStatement>,
}

pub(super) fn scan_flowchart_accessibility(code: &str) -> FlowchartAccessibilityScan {
    #[cfg(test)]
    FLOWCHART_ACCESSIBILITY_SCAN_COUNT
        .set(FLOWCHART_ACCESSIBILITY_SCAN_COUNT.get().saturating_add(1));

    let mut masked = code.as_bytes().to_vec();
    let mut title = None;
    let mut description = None;
    let mut statements = Vec::new();
    let mut start = 0usize;

    while start < code.len() {
        let line_end = next_line_end(code, start);
        let line = &code[start..line_end];
        let trimmed = line.trim_start();
        let prefix_start = start + line.len().saturating_sub(trimmed.len());

        if let Some(after_prefix) = trimmed.strip_prefix("accTitle") {
            let whitespace = after_prefix
                .len()
                .saturating_sub(after_prefix.trim_start().len());
            let rest = after_prefix.trim_start();
            if let Some(value) = rest.strip_prefix(':') {
                title = Some(value.trim().to_string());
                statements.push(inline_statement(
                    code,
                    FlowchartAccessibilityDirective::Title,
                    prefix_start,
                    prefix_start + "accTitle".len() + whitespace,
                    line_end,
                ));
                mask_range_preserving_newlines(&mut masked, start, line_end);
                start = line_end;
                continue;
            }
        }

        let description_directive = trimmed.strip_prefix("accDescr").map(|after_prefix| {
            let whitespace = after_prefix
                .len()
                .saturating_sub(after_prefix.trim_start().len());
            (
                FlowchartAccessibilityDirective::Description,
                "accDescr",
                whitespace,
                after_prefix.trim_start(),
            )
        });

        let Some((directive, prefix, whitespace, rest)) = description_directive else {
            start = line_end;
            continue;
        };
        let delimiter_start = prefix_start + prefix.len() + whitespace;

        if let Some(value) = rest.strip_prefix(':') {
            description = Some(value.trim().to_string());
            statements.push(inline_statement(
                code,
                directive,
                prefix_start,
                delimiter_start,
                line_end,
            ));
            mask_range_preserving_newlines(&mut masked, start, line_end);
            start = line_end;
            continue;
        }

        let Some(after_open) = rest.strip_prefix('{') else {
            start = line_end;
            continue;
        };
        let content_start = delimiter_start + 1;
        debug_assert_eq!(content_start, line_end - after_open.len());
        let closing_brace = code[content_start..]
            .find('}')
            .map(|relative| content_start + relative);
        let content_end = closing_brace.unwrap_or(code.len());
        if closing_brace.is_some() {
            description = Some(code[content_start..content_end].trim().to_string());
        }

        let statement_end = closing_brace.map_or(code.len(), |position| position + 1);
        let mut lexemes = vec![
            FlowchartAccessibilityLexeme {
                kind: EditorLexemeKind::Keyword,
                span: SourceSpan::new(prefix_start, prefix_start + prefix.len()),
            },
            FlowchartAccessibilityLexeme {
                kind: EditorLexemeKind::Delimiter,
                span: SourceSpan::new(delimiter_start, delimiter_start + 1),
            },
        ];
        if let Some(span) = trimmed_nonempty_span(code, content_start, content_end) {
            lexemes.push(FlowchartAccessibilityLexeme {
                kind: EditorLexemeKind::String,
                span,
            });
        }
        if let Some(closing_brace) = closing_brace {
            lexemes.push(FlowchartAccessibilityLexeme {
                kind: EditorLexemeKind::Delimiter,
                span: SourceSpan::new(closing_brace, closing_brace + 1),
            });
        }
        statements.push(FlowchartAccessibilityStatement {
            directive,
            complete: closing_brace.is_some(),
            lexemes,
        });
        mask_range_preserving_newlines(&mut masked, start, statement_end);
        start = statement_end;
    }

    let parser_input = String::from_utf8(masked)
        .expect("replacing non-newline source bytes with ASCII spaces preserves UTF-8");
    FlowchartAccessibilityScan {
        parser_input,
        title,
        description,
        statements,
    }
}

fn inline_statement(
    code: &str,
    directive: FlowchartAccessibilityDirective,
    prefix_start: usize,
    delimiter_start: usize,
    line_end: usize,
) -> FlowchartAccessibilityStatement {
    let mut lexemes = vec![
        FlowchartAccessibilityLexeme {
            kind: EditorLexemeKind::Keyword,
            span: SourceSpan::new(prefix_start, prefix_start + directive.prefix().len()),
        },
        FlowchartAccessibilityLexeme {
            kind: EditorLexemeKind::Delimiter,
            span: SourceSpan::new(delimiter_start, delimiter_start + 1),
        },
    ];
    if let Some(span) = trimmed_nonempty_span(code, delimiter_start + 1, line_end) {
        lexemes.push(FlowchartAccessibilityLexeme {
            kind: EditorLexemeKind::String,
            span,
        });
    }
    FlowchartAccessibilityStatement {
        directive,
        complete: true,
        lexemes,
    }
}

fn trimmed_nonempty_span(code: &str, start: usize, end: usize) -> Option<SourceSpan> {
    let raw = code.get(start..end)?;
    let leading = raw.len().saturating_sub(raw.trim_start().len());
    let trailing = raw.len().saturating_sub(raw.trim_end().len());
    let span = SourceSpan::new(start + leading, end.saturating_sub(trailing));
    (span.start < span.end).then_some(span)
}

fn next_line_end(code: &str, start: usize) -> usize {
    code[start..]
        .find('\n')
        .map_or(code.len(), |relative| start + relative + 1)
}

fn mask_range_preserving_newlines(bytes: &mut [u8], start: usize, end: usize) {
    for byte in &mut bytes[start..end] {
        if *byte != b'\n' && *byte != b'\r' {
            *byte = b' ';
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_projects_values_directives_and_offset_preserving_input_once() {
        reset_flowchart_accessibility_scan_count();
        let source = concat!(
            "flowchart TD\n",
            "accTitle: Checkout\n",
            "accDescr {\n",
            "  First line\n",
            "  second line\n",
            "} A --> B\n",
            "A --> B\n",
        );

        let scan = scan_flowchart_accessibility(source);

        assert_eq!(flowchart_accessibility_scan_count(), 1);
        assert_eq!(scan.title.as_deref(), Some("Checkout"));
        assert_eq!(
            scan.description.as_deref(),
            Some("First line\n  second line")
        );
        assert_eq!(
            scan.statements
                .iter()
                .map(|statement| statement.directive)
                .collect::<Vec<_>>(),
            vec![
                FlowchartAccessibilityDirective::Title,
                FlowchartAccessibilityDirective::Description,
            ]
        );
        assert_eq!(scan.parser_input.len(), source.len());
        assert_eq!(
            scan.parser_input.match_indices('\n').collect::<Vec<_>>(),
            source.match_indices('\n').collect::<Vec<_>>()
        );
        assert!(scan.parser_input.contains("flowchart TD"));
        assert!(scan.parser_input.contains("A --> B"));
        assert!(!scan.parser_input.contains("Checkout"));
        assert!(!scan.parser_input.contains("First line"));

        let lexemes = scan
            .statements
            .iter()
            .flat_map(|statement| statement.lexemes.iter())
            .map(|lexeme| (lexeme.kind, &source[lexeme.span.start..lexeme.span.end]))
            .collect::<Vec<_>>();
        assert!(lexemes.contains(&(EditorLexemeKind::Keyword, "accTitle")));
        assert!(lexemes.contains(&(EditorLexemeKind::Keyword, "accDescr")));
        assert!(lexemes.contains(&(EditorLexemeKind::Delimiter, ":")));
        assert!(lexemes.contains(&(EditorLexemeKind::Delimiter, "{")));
        assert!(lexemes.contains(&(EditorLexemeKind::Delimiter, "}")));
        assert!(lexemes.contains(&(EditorLexemeKind::String, "Checkout")));
        assert!(lexemes.contains(&(EditorLexemeKind::String, "First line\n  second line")));
        assert!(scan.parser_input.contains("A --> B"));
    }

    #[test]
    fn scan_uses_utf8_byte_spans_and_ignores_unterminated_block_semantics() {
        let source = concat!(
            "flowchart TD\n",
            "  accTitle : 结账流程\n",
            "accDescr {\n  第一行\n  第二行",
        );

        let scan = scan_flowchart_accessibility(source);
        let strings = scan
            .statements
            .iter()
            .flat_map(|statement| statement.lexemes.iter())
            .filter(|lexeme| lexeme.kind == EditorLexemeKind::String)
            .map(|lexeme| &source[lexeme.span.start..lexeme.span.end])
            .collect::<Vec<_>>();

        assert_eq!(strings, ["结账流程", "第一行\n  第二行"]);
        assert_eq!(scan.title.as_deref(), Some("结账流程"));
        assert_eq!(scan.description, None);
        assert!(!scan.statements.last().unwrap().complete);
    }
}
