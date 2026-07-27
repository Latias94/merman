use crate::cli::RenderFormat;
use crate::error::CliError;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone)]
pub(crate) struct MarkdownChart {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) definition: String,
}

#[derive(Debug, Clone)]
pub(crate) struct MarkdownImage {
    pub(crate) url: String,
    pub(crate) title: Option<String>,
    pub(crate) alt: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MarkdownChartLimitExceeded {
    pub(crate) observed: u64,
    pub(crate) max: u64,
}

pub(crate) fn is_markdown_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("md")
                || extension.eq_ignore_ascii_case("markdown")
                || extension.eq_ignore_ascii_case("mdx")
        })
}

pub(crate) fn extract_charts(source: &str) -> Vec<MarkdownChart> {
    extract_charts_with_spans(source)
}

pub(crate) fn extract_charts_with_spans(source: &str) -> Vec<MarkdownChart> {
    extract_charts_limited(source, None).expect("an unbounded scan cannot exceed its chart limit")
}

pub(crate) fn extract_charts_limited(
    source: &str,
    max_charts: Option<u64>,
) -> Result<Vec<MarkdownChart>, MarkdownChartLimitExceeded> {
    let mut charts = Vec::new();
    let mut cursor = 0;

    while cursor < source.len() {
        let line_end = next_line_end(source, cursor);
        let line = trim_line_ending(&source[cursor..line_end]);

        let Some(opening) = markdown_fence_opening(line) else {
            cursor = line_end;
            continue;
        };

        if !opening.is_mermaid {
            cursor = skip_markdown_fence(source, line_end, opening.delimiter);
            continue;
        }

        let body_start = line_end;
        let mut search_start = body_start;
        while search_start < source.len() {
            let closing_end = next_line_end(source, search_start);
            let closing_line = trim_line_ending(&source[search_start..closing_end]);
            if matching_closing_fence(closing_line, opening.delimiter) {
                admit_chart(&charts, max_charts)?;
                charts.push(MarkdownChart {
                    start: cursor,
                    end: closing_end,
                    definition: source[body_start..search_start].to_string(),
                });
                cursor = closing_end;
                break;
            }
            search_start = closing_end;
        }

        if search_start == source.len() {
            admit_chart(&charts, max_charts)?;
            charts.push(MarkdownChart {
                start: cursor,
                end: source.len(),
                definition: source[body_start..].to_string(),
            });
            break;
        }
    }

    Ok(charts)
}

fn admit_chart(
    charts: &[MarkdownChart],
    max_charts: Option<u64>,
) -> Result<(), MarkdownChartLimitExceeded> {
    let observed = u64::try_from(charts.len())
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    if let Some(max) = max_charts
        && observed > max
    {
        return Err(MarkdownChartLimitExceeded { observed, max });
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct FenceDelimiter {
    marker: u8,
    len: usize,
}

#[derive(Debug, Clone, Copy)]
struct MarkdownFenceOpening {
    delimiter: FenceDelimiter,
    is_mermaid: bool,
}

fn markdown_fence_opening(line: &str) -> Option<MarkdownFenceOpening> {
    let trimmed = trim_fence_indent(line)?;
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

    Some(MarkdownFenceOpening {
        delimiter: FenceDelimiter { marker, len },
        is_mermaid,
    })
}

fn matching_closing_fence(line: &str, delimiter: FenceDelimiter) -> bool {
    let Some(trimmed) = trim_fence_indent(line) else {
        return false;
    };
    let len = repeated_marker_len(trimmed.as_bytes(), delimiter.marker);
    len >= delimiter.len && trimmed[len..].chars().all(char::is_whitespace)
}

fn skip_markdown_fence(source: &str, mut cursor: usize, delimiter: FenceDelimiter) -> usize {
    while cursor < source.len() {
        let line_end = next_line_end(source, cursor);
        let line = trim_line_ending(&source[cursor..line_end]);
        if matching_closing_fence(line, delimiter) {
            return line_end;
        }
        cursor = line_end;
    }
    source.len()
}

fn trim_fence_indent(line: &str) -> Option<&str> {
    let mut spaces = 0;
    for (index, byte) in line.bytes().enumerate() {
        match byte {
            b' ' if spaces < 3 => spaces += 1,
            b' ' | b'\t' => return None,
            _ => return Some(&line[index..]),
        }
    }
    Some("")
}

fn repeated_marker_len(bytes: &[u8], marker: u8) -> usize {
    bytes.iter().take_while(|byte| **byte == marker).count()
}

fn trim_line_ending(line: &str) -> &str {
    line.strip_suffix('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .unwrap_or_else(|| line.strip_suffix('\r').unwrap_or(line))
}

fn next_line_end(source: &str, start: usize) -> usize {
    let bytes = source.as_bytes();
    let mut index = start;
    while index < bytes.len() {
        match bytes[index] {
            b'\n' => return index + 1,
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => return index + 2,
            b'\r' => return index + 1,
            _ => index += 1,
        }
    }
    source.len()
}

pub(crate) fn replace_charts_with_images(source: &str, images: &[MarkdownImage]) -> String {
    let charts = extract_charts(source);
    if charts.is_empty() {
        return source.to_string();
    }

    let mut out = String::with_capacity(source.len());
    let mut last = 0;
    for (chart, image) in charts.iter().zip(images) {
        out.push_str(&source[last..chart.start]);
        out.push_str(&markdown_image(image));
        last = chart.end;
    }
    out.push_str(&source[last..]);
    out
}

pub(crate) fn numbered_output_path(
    output_template: &Path,
    index: usize,
    format: RenderFormat,
    artefacts: Option<&Path>,
) -> PathBuf {
    let original_ext = output_template
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_else(|| format.extension());
    let artifact_ext = if is_markdown_path(output_template) {
        format.extension()
    } else {
        original_ext
    };
    let stem = output_template
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("out");
    let file_name = format!("{stem}-{index}.{artifact_ext}");

    match artefacts {
        Some(dir) => dir.join(file_name),
        None => output_template.with_file_name(file_name),
    }
}

pub(crate) fn relative_markdown_url(
    markdown_output: &Path,
    image_output: &Path,
) -> Result<String, CliError> {
    let base_dir = markdown_output.parent().unwrap_or_else(|| Path::new("."));
    let base = absolute_path(base_dir)?;
    let target = absolute_path(image_output)?;
    let relative = relative_path(&base, &target).unwrap_or(target);
    Ok(format!("./{}", path_to_markdown_url(&relative)))
}

fn markdown_image(image: &MarkdownImage) -> String {
    let alt = escape_alt(&image.alt);
    match image.title.as_deref() {
        Some(title) => format!("![{}]({} \"{}\")", alt, image.url, escape_title(title)),
        None => format!("![{}]({})", alt, image.url),
    }
}

fn escape_alt(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if matches!(ch, '[' | ']' | '\\') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

fn escape_title(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if matches!(ch, '"' | '\\') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

fn absolute_path(path: &Path) -> Result<PathBuf, CliError> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn relative_path(base: &Path, target: &Path) -> Option<PathBuf> {
    let base = normalized_components(base);
    let target = normalized_components(target);

    if base.first()? != target.first()? {
        return None;
    }

    let common_len = base
        .iter()
        .zip(target.iter())
        .take_while(|(left, right)| left == right)
        .count();

    let mut out = PathBuf::new();
    for _ in common_len..base.len() {
        out.push("..");
    }
    for component in &target[common_len..] {
        out.push(component);
    }

    if out.as_os_str().is_empty() {
        Some(PathBuf::from("."))
    } else {
        Some(out)
    }
}

fn normalized_components(path: &Path) -> Vec<OsString> {
    path.components()
        .filter_map(|component| match component {
            Component::Prefix(prefix) => Some(prefix.as_os_str().to_os_string()),
            Component::RootDir => Some(OsString::from(std::path::MAIN_SEPARATOR.to_string())),
            Component::CurDir => None,
            Component::ParentDir => Some(OsString::from("..")),
            Component::Normal(value) => Some(value.to_os_string()),
        })
        .collect()
}

fn path_to_markdown_url(path: &Path) -> String {
    percent_encode_markdown_url(&path.to_string_lossy().replace('\\', "/"))
}

fn percent_encode_markdown_url(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'.' | b'-' | b'_' | b'~' => {
                out.push(byte as char)
            }
            _ => {
                use std::fmt::Write as _;
                write!(&mut out, "%{byte:02X}").expect("writing to a String should not fail");
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_backtick_and_colon_mermaid_blocks() {
        let source = "before\n```Mermaid title=Main\nflowchart LR\nA-->B\n```\n~~~ mermaid\nsequenceDiagram\nA->>B: Hi\n~~~\n:::MERMAID extra info\npie title Work\n:::\n```mermaidx\nignored\n```\nafter";
        let charts = extract_charts(source);

        assert_eq!(charts.len(), 3);
        assert!(charts[0].definition.contains("flowchart LR"));
        assert!(charts[1].definition.contains("sequenceDiagram"));
        assert!(charts[2].definition.contains("pie title Work"));
    }

    #[test]
    fn ignores_mermaid_looking_content_inside_another_fence() {
        let source = "````text\n```mermaid\nflowchart LR\nIgnored-->Fence\n```\n````\n\n```mermaid\nflowchart LR\nRendered-->Diagram\n```\n";
        let charts = extract_charts(source);

        assert_eq!(charts.len(), 1);
        assert!(charts[0].definition.contains("Rendered-->Diagram"));
        assert!(!charts[0].definition.contains("Ignored-->Fence"));
    }

    #[test]
    fn preserves_commonmark_fence_boundaries_and_line_endings() {
        let source = concat!(
            "    ```mermaid\n",
            "flowchart LR\n",
            "    ```\n",
            "\t```mermaid\n",
            "flowchart LR\n",
            "```\n",
            "```mermaidx\n",
            "flowchart LR\n",
            "```\n",
            "   ```` mermaid\n",
            "flowchart TD\n",
            "A-->B\n",
            "``````\n",
            "~~~mermaid\r",
            "sequenceDiagram\r",
            "A->>B: Hi\r",
            "~~~~\r",
        );
        let charts = extract_charts(source);

        assert_eq!(charts.len(), 2);
        assert_eq!(charts[0].definition, "flowchart TD\nA-->B\n");
        assert_eq!(
            charts[1].definition, "sequenceDiagram\rA->>B: Hi\r",
            "bare CR line endings must be retained in the rendered definition"
        );
    }

    #[test]
    fn retains_an_unclosed_mermaid_fence_to_match_cli_conversion_behavior() {
        let source = "before\n~~~mermaid\nflowchart LR\nA-->B\n";
        let charts = extract_charts(source);

        assert_eq!(charts.len(), 1);
        assert_eq!(charts[0].start, "before\n".len());
        assert_eq!(charts[0].end, source.len());
        assert_eq!(charts[0].definition, "flowchart LR\nA-->B\n");
    }

    #[test]
    fn replaces_charts_with_escaped_markdown_images() {
        let source = "```mermaid\nflowchart LR\nA-->B\n```";
        let images = [MarkdownImage {
            url: "./out-1.svg".to_string(),
            title: Some(r#"a "title""#.to_string()),
            alt: r"diagram [one]".to_string(),
        }];

        assert_eq!(
            replace_charts_with_images(source, &images),
            r#"![diagram \[one\]](./out-1.svg "a \"title\"")"#
        );
    }

    #[test]
    fn markdown_output_uses_render_format_extension_for_numbered_artefacts() {
        let out = numbered_output_path(Path::new("docs/out.md"), 2, RenderFormat::Svg, None);

        assert_eq!(out, PathBuf::from("docs/out-2.svg"));
    }

    #[test]
    fn markdown_urls_percent_encode_link_destination_delimiters() {
        assert_eq!(
            path_to_markdown_url(Path::new("images/out (final) \"copy\".svg")),
            "images/out%20%28final%29%20%22copy%22.svg"
        );
        assert_eq!(
            path_to_markdown_url(Path::new("images/100% ready #1?.svg")),
            "images/100%25%20ready%20%231%3F.svg"
        );
        assert_eq!(
            path_to_markdown_url(Path::new("images/流程.svg")),
            "images/%E6%B5%81%E7%A8%8B.svg"
        );
    }
}
