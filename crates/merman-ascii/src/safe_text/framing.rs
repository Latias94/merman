use super::{BudgetedTextDocument, BudgetedTextLine, BudgetedWrappedText};
use crate::Result;

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
    line: &mut BudgetedWrappedText<'_, '_>,
    separator: &str,
    key: &str,
    value: &str,
) -> Result<()> {
    line.write_fmt(format_args!("{separator}{key}(bytes={})=", value.len()))?;
    line.push_quoted_text(value)
}

/// Writes one length-framed authored list to a wrapping StructuredText row.
pub(crate) fn push_wrapped_list<'a>(
    line: &mut BudgetedWrappedText<'_, '_>,
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
