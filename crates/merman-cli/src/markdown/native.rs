use super::{
    MarkdownChart, MarkdownChartLimitExceeded, MarkdownFenceLocation, admit_chart, next_line_end,
    trim_line_ending,
};

#[derive(Debug, Clone, Copy)]
struct FenceDelimiter {
    marker: u8,
    len: usize,
}

#[derive(Debug, Clone, Copy)]
struct FenceOpening {
    delimiter: FenceDelimiter,
    marker_offset: usize,
    is_mermaid: bool,
}

pub(super) fn scan<'source>(
    source: &'source str,
    max_charts: Option<u64>,
) -> Result<Vec<MarkdownChart<'source>>, MarkdownChartLimitExceeded> {
    let mut charts = Vec::new();
    let mut cursor = 0;
    let mut line = 1;

    while cursor < source.len() {
        let line_end = next_line_end(source, cursor);
        let line_source = trim_line_ending(&source[cursor..line_end]);

        let Some(opening) = fence_opening(line_source) else {
            cursor = line_end;
            line += 1;
            continue;
        };

        if !opening.is_mermaid {
            let (next_cursor, lines_consumed) = skip_fence(source, line_end, opening.delimiter);
            cursor = next_cursor;
            line += 1 + lines_consumed;
            continue;
        }

        let location = MarkdownFenceLocation {
            line,
            column: line_source[..opening.marker_offset].chars().count() + 1,
        };
        let body_start = line_end;
        let mut search_start = body_start;
        let mut body_lines = 0;
        while search_start < source.len() {
            let closing_end = next_line_end(source, search_start);
            let closing_line = trim_line_ending(&source[search_start..closing_end]);
            if matching_closing_fence(closing_line, opening.delimiter) {
                admit_chart(charts.len(), max_charts, location)?;
                charts.push(MarkdownChart::new(
                    source,
                    cursor..search_start + closing_line.len(),
                    body_start..search_start,
                    location,
                ));
                cursor = closing_end;
                line += 2 + body_lines;
                break;
            }
            search_start = closing_end;
            body_lines += 1;
        }

        if search_start == source.len() {
            admit_chart(charts.len(), max_charts, location)?;
            charts.push(MarkdownChart::new(
                source,
                cursor..source.len(),
                body_start..source.len(),
                location,
            ));
            break;
        }
    }

    Ok(charts)
}

fn fence_opening(line: &str) -> Option<FenceOpening> {
    let (trimmed, marker_offset) = trim_fence_indent(line)?;
    let marker = *trimmed.as_bytes().first()?;
    if !matches!(marker, b'`' | b'~' | b':') {
        return None;
    }

    let len = repeated_marker_len(trimmed.as_bytes(), marker);
    if len < 3 {
        return None;
    }

    let rest = trimmed[len..].trim_start();
    let is_mermaid = rest
        .get(.."mermaid".len())
        .is_some_and(|language| language.eq_ignore_ascii_case("mermaid"))
        && (rest.len() == "mermaid".len()
            || rest["mermaid".len()..].starts_with(char::is_whitespace));

    Some(FenceOpening {
        delimiter: FenceDelimiter { marker, len },
        marker_offset,
        is_mermaid,
    })
}

fn matching_closing_fence(line: &str, delimiter: FenceDelimiter) -> bool {
    let Some((trimmed, _)) = trim_fence_indent(line) else {
        return false;
    };
    let len = repeated_marker_len(trimmed.as_bytes(), delimiter.marker);
    len >= delimiter.len && trimmed[len..].chars().all(char::is_whitespace)
}

fn skip_fence(source: &str, mut cursor: usize, delimiter: FenceDelimiter) -> (usize, usize) {
    let mut lines_consumed = 0;
    while cursor < source.len() {
        let line_end = next_line_end(source, cursor);
        let line = trim_line_ending(&source[cursor..line_end]);
        lines_consumed += 1;
        if matching_closing_fence(line, delimiter) {
            return (line_end, lines_consumed);
        }
        cursor = line_end;
    }
    (source.len(), lines_consumed)
}

fn trim_fence_indent(line: &str) -> Option<(&str, usize)> {
    let mut spaces = 0;
    for (index, byte) in line.bytes().enumerate() {
        match byte {
            b' ' if spaces < 3 => spaces += 1,
            b' ' | b'\t' => return None,
            _ => return Some((&line[index..], index)),
        }
    }
    Some(("", line.len()))
}

fn repeated_marker_len(bytes: &[u8], marker: u8) -> usize {
    bytes.iter().take_while(|byte| **byte == marker).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_borrowable_ranges_for_native_extensions() {
        let source = concat!(
            "before\r\n",
            "   ~~~~ Mermaid title=Main\r\n",
            "flowchart LR\r\n",
            "A-->B\r\n",
            "~~~~~~\r\n",
            "after\r\n",
        );

        let charts = scan(source, None).expect("scan");

        assert_eq!(charts.len(), 1);
        assert_eq!(charts[0].definition(), "flowchart LR\r\nA-->B\r\n");
        assert_eq!(
            charts[0].location(),
            MarkdownFenceLocation { line: 2, column: 4 }
        );
        let definition = charts[0].definition();
        let source_start = source.as_ptr() as usize;
        let source_end = source_start + source.len();
        let definition_start = definition.as_ptr() as usize;
        assert!(definition_start >= source_start);
        assert!(definition_start < source_end);
    }

    #[test]
    fn protects_mermaid_markers_inside_other_fences() {
        let source = concat!(
            "````text\n",
            "```mermaid\n",
            "flowchart LR\n",
            "Ignored-->Fence\n",
            "```\n",
            "````\n",
            "~~~mermaid\n",
            "flowchart LR\n",
            "Rendered-->Diagram\n",
        );

        let charts = scan(source, None).expect("scan");

        assert_eq!(charts.len(), 1);
        assert!(charts[0].definition().contains("Rendered-->Diagram"));
    }
}
