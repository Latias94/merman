use super::normalization::visible_escape;
use super::{BudgetedTextDocument, BudgetedTextLine, BudgetedWrappedText};
use crate::Result;
use crate::resource::ResourceContext;
use unicode_segmentation::UnicodeSegmentation;

/// Visits one injective, terminal-safe quoted field value without allocating an escaped copy.
pub(crate) fn visit_quoted_terminal_text(
    value: &str,
    resources: &ResourceContext,
    mut visit: impl FnMut(&str) -> Result<()>,
) -> Result<()> {
    visit_quoted_terminal_text_with(value, |event| match event {
        QuotedTerminalTextEvent::SourceGrapheme(grapheme) => {
            resources.charge_layout_work(1)?;
            resources.check_grapheme_bytes(grapheme.len())
        }
        QuotedTerminalTextEvent::OutputFragment(fragment) => visit(fragment),
    })
}

pub(super) enum QuotedTerminalTextEvent<'a> {
    SourceGrapheme(&'a str),
    OutputFragment(&'a str),
}

pub(super) fn visit_quoted_terminal_text_with(
    value: &str,
    mut visit: impl for<'a> FnMut(QuotedTerminalTextEvent<'a>) -> Result<()>,
) -> Result<()> {
    visit(QuotedTerminalTextEvent::OutputFragment("\""))?;
    for grapheme in value.graphemes(true) {
        // Check the source grapheme before quoting can split it into fragments and bypass the byte limit.
        visit(QuotedTerminalTextEvent::SourceGrapheme(grapheme))?;
        if !grapheme
            .chars()
            .any(|ch| ch == '\\' || ch == '"' || (ch != ' ' && ch.is_whitespace()))
        {
            visit(QuotedTerminalTextEvent::OutputFragment(grapheme))?;
            continue;
        }

        for ch in grapheme.chars() {
            match ch {
                '\\' => visit(QuotedTerminalTextEvent::OutputFragment("\\\\"))?,
                '"' => visit(QuotedTerminalTextEvent::OutputFragment("\\\""))?,
                ' ' => visit(QuotedTerminalTextEvent::OutputFragment(" "))?,
                '\t' => visit(QuotedTerminalTextEvent::OutputFragment("\\t"))?,
                '\n' => visit(QuotedTerminalTextEvent::OutputFragment("\\n"))?,
                '\r' => visit(QuotedTerminalTextEvent::OutputFragment("\\r"))?,
                ch if ch.is_whitespace() => {
                    let mut buffer = [0u8; 10];
                    visit(QuotedTerminalTextEvent::OutputFragment(visible_escape(
                        ch,
                        &mut buffer,
                    )))?;
                }
                ch => {
                    let mut buffer = [0u8; 4];
                    visit(QuotedTerminalTextEvent::OutputFragment(
                        ch.encode_utf8(&mut buffer),
                    ))?;
                }
            }
        }
    }
    visit(QuotedTerminalTextEvent::OutputFragment("\""))
}

/// Writes one length-framed authored field to a non-wrapping StructuredText row.
pub(crate) fn push_line_field(
    line: &mut BudgetedTextLine<'_>,
    separator: &str,
    key: &str,
    value: &str,
) -> Result<()> {
    line.write_fmt(format_args!("{separator}{key}(bytes={})=", value.len()))?;
    line.push_quoted_text(value)
}

/// Writes one length-framed authored list to a non-wrapping StructuredText row.
pub(crate) fn push_line_list<'a>(
    line: &mut BudgetedTextLine<'_>,
    separator: &str,
    key: &str,
    values: impl IntoIterator<Item = &'a str>,
) -> Result<()> {
    line.write_fmt(format_args!("{separator}{key}=["))?;
    for (index, value) in values.into_iter().enumerate() {
        if index > 0 {
            line.push_str(", ")?;
        }
        line.write_fmt(format_args!("bytes={} ", value.len()))?;
        line.push_quoted_text(value)?;
    }
    line.push_str("]")
}

/// Writes one length-framed authored field to a wrapping StructuredText row.
pub(crate) fn push_wrapped_field(
    line: &mut BudgetedWrappedText<'_>,
    separator: &str,
    key: &str,
    value: &str,
) -> Result<()> {
    line.write_fmt(format_args!("{separator}{key}(bytes={})=", value.len()))?;
    line.push_quoted_text(value)
}

/// Writes one length-framed authored list to a wrapping StructuredText row.
pub(crate) fn push_wrapped_list<'a>(
    line: &mut BudgetedWrappedText<'_>,
    separator: &str,
    key: &str,
    values: impl IntoIterator<Item = &'a str>,
) -> Result<()> {
    line.write_fmt(format_args!("{separator}{key}=["))?;
    for (index, value) in values.into_iter().enumerate() {
        if index > 0 {
            line.push_str(", ")?;
        }
        line.write_fmt(format_args!("bytes={} ", value.len()))?;
        line.push_quoted_text(value)?;
    }
    line.push_str("]")
}

pub(crate) fn push_document_field(
    document: &mut BudgetedTextDocument,
    key: &str,
    value: &str,
) -> Result<()> {
    document.push_line_with(|line| push_line_field(line, "", key, value))
}

pub(crate) fn push_optional_document_field(
    document: &mut BudgetedTextDocument,
    key: &str,
    value: Option<&str>,
) -> Result<()> {
    let Some(value) = value else {
        return Ok(());
    };
    push_document_field(document, key, value)
}

pub(crate) fn push_document_list<'a>(
    document: &mut BudgetedTextDocument,
    key: &str,
    values: impl IntoIterator<Item = &'a str>,
) -> Result<()> {
    document.push_line_with(|line| push_line_list(line, "", key, values))
}
