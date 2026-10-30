pub use merman_model::source::SourceSpan;

pub(crate) fn line_content_end(source: &str, end: usize) -> usize {
    let end = end.min(source.len());
    end.checked_sub(1)
        .filter(|index| source.as_bytes().get(*index) == Some(&b'\r'))
        .unwrap_or(end)
}

#[allow(
    dead_code,
    reason = "Shared parser facilities have different consumers in each family selection."
)]
pub(crate) fn has_ascii_separator(source: &str, start: usize, end: usize) -> bool {
    source.get(start..end).is_some_and(|slice| {
        !slice.is_empty() && slice.as_bytes().iter().all(u8::is_ascii_whitespace)
    })
}

#[allow(
    dead_code,
    reason = "Shared parser facilities have different consumers in each family selection."
)]
pub(crate) fn trailing_ascii_whitespace_slot(
    source: &str,
    start: usize,
    end: usize,
) -> Option<SourceSpan> {
    let end = line_content_end(source, end);
    let slice = source.get(start..end)?;
    (!slice.is_empty() && slice.as_bytes().last().is_some_and(u8::is_ascii_whitespace))
        .then_some(SourceSpan::new(end, end))
}

#[allow(
    dead_code,
    reason = "Shared parser facilities have different consumers in each family selection."
)]
pub(crate) fn source_value_span(source: &str, span: SourceSpan, value: &str) -> Option<SourceSpan> {
    let slice = source.get(span.start..span.end)?;
    let relative_start = slice.find(value)?;
    Some(SourceSpan::new(
        span.start + relative_start,
        span.start + relative_start + value.len(),
    ))
}

#[allow(
    dead_code,
    reason = "Shared parser facilities have different consumers in each family selection."
)]
pub(crate) fn keyword_value_span(
    source: &str,
    statement_start: usize,
    statement_end: usize,
    keyword: &str,
) -> Option<SourceSpan> {
    let raw = source.get(statement_start..statement_end)?;
    let trimmed = raw.trim_start();
    let leading = raw.len().saturating_sub(trimmed.len());
    let keyword_source = trimmed.get(..keyword.len())?;
    if !keyword_source.eq_ignore_ascii_case(keyword) {
        return None;
    }
    let after_keyword = trimmed.get(keyword.len()..)?;
    if after_keyword
        .chars()
        .next()
        .is_some_and(|ch| !ch.is_whitespace())
    {
        return None;
    }
    let whitespace = after_keyword
        .chars()
        .take_while(|ch| ch.is_whitespace())
        .map(char::len_utf8)
        .sum::<usize>();
    let value_start = statement_start + leading + keyword.len() + whitespace;
    let value_len = after_keyword[whitespace..]
        .chars()
        .take_while(|ch| !ch.is_whitespace())
        .map(char::len_utf8)
        .sum::<usize>();
    Some(SourceSpan::new(value_start, value_start + value_len))
}

#[cfg(all(
    feature = "editor-facts",
    any(feature = "diagram-packet", feature = "diagram-radar")
))]
pub(crate) fn recovery_fallback_span(source: &str) -> SourceSpan {
    let mut line_start = 0;
    for segment in source.split_inclusive('\n') {
        let line = segment.strip_suffix('\n').unwrap_or(segment);
        let line = line.strip_suffix('\r').unwrap_or(line);
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            let start = line_start + line.find(trimmed).unwrap_or_default();
            return SourceSpan::new(start, start + trimmed.len());
        }
        line_start += segment.len();
    }
    SourceSpan::new(source.len(), source.len())
}
