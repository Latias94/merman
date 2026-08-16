use crate::cli::RenderFormat;
use crate::error::CliError;
use std::ffi::OsString;
use std::ops::Range;
use std::path::{Component, Path, PathBuf};

mod native;
mod strict;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MarkdownFenceLocation {
    pub(crate) line: usize,
    pub(crate) column: usize,
}

#[derive(Debug, Clone)]
#[cfg(any(feature = "rustdoc", test))]
pub(crate) struct MarkdownInclude<'source> {
    source_span: Range<usize>,
    path: &'source str,
    location: MarkdownFenceLocation,
}

#[cfg(any(feature = "rustdoc", test))]
impl<'source> MarkdownInclude<'source> {
    fn new(source_span: Range<usize>, path: &'source str, location: MarkdownFenceLocation) -> Self {
        Self {
            source_span,
            path,
            location,
        }
    }

    pub(crate) fn source_span(&self) -> Range<usize> {
        self.source_span.clone()
    }

    pub(crate) fn path(&self) -> &'source str {
        self.path
    }

    pub(crate) fn location(&self) -> MarkdownFenceLocation {
        self.location
    }
}

#[derive(Debug, Clone)]
#[cfg(any(feature = "rustdoc", test))]
pub(crate) enum MarkdownReplacement<'source> {
    Chart(MarkdownChart<'source>),
    Include(MarkdownInclude<'source>),
}

#[cfg(any(feature = "rustdoc", test))]
impl MarkdownReplacement<'_> {
    pub(crate) fn source_span(&self) -> Range<usize> {
        match self {
            Self::Chart(chart) => chart.source_span(),
            Self::Include(include) => include.source_span(),
        }
    }

    pub(crate) fn location(&self) -> MarkdownFenceLocation {
        match self {
            Self::Chart(chart) => chart.location(),
            Self::Include(include) => include.location(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct MarkdownChart<'source> {
    source_span: Range<usize>,
    #[cfg(test)]
    definition_span: Range<usize>,
    definition: &'source str,
    location: MarkdownFenceLocation,
}

impl<'source> MarkdownChart<'source> {
    fn new(
        source: &'source str,
        source_span: Range<usize>,
        definition_span: Range<usize>,
        location: MarkdownFenceLocation,
    ) -> Self {
        debug_assert!(source_span.start <= definition_span.start);
        debug_assert!(definition_span.end <= source_span.end);
        Self {
            source_span,
            definition: &source[definition_span.clone()],
            #[cfg(test)]
            definition_span,
            location,
        }
    }

    pub(crate) fn source_span(&self) -> Range<usize> {
        self.source_span.clone()
    }

    #[cfg(test)]
    fn definition_span(&self) -> Range<usize> {
        self.definition_span.clone()
    }

    pub(crate) fn definition(&self) -> &'source str {
        self.definition
    }

    pub(crate) fn location(&self) -> MarkdownFenceLocation {
        self.location
    }
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
    pub(crate) location: MarkdownFenceLocation,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[cfg(any(feature = "rustdoc", test))]
pub(crate) enum MarkdownReplacementScanError {
    #[error(
        "Markdown chart limit {max} exceeded by chart {observed} at line {line}, column {column}"
    )]
    ChartLimit {
        observed: u64,
        max: u64,
        line: usize,
        column: usize,
    },
    #[error("unclosed Mermaid fence at line {line}, column {column}")]
    UnclosedMermaidFence { line: usize, column: usize },
    #[error("invalid include_mmd! directive at line {line}, column {column}: {message}")]
    InvalidInclude {
        line: usize,
        column: usize,
        message: String,
    },
}

#[cfg(any(feature = "rustdoc", test))]
impl From<MarkdownChartLimitExceeded> for MarkdownReplacementScanError {
    fn from(error: MarkdownChartLimitExceeded) -> Self {
        Self::ChartLimit {
            observed: error.observed,
            max: error.max,
            line: error.location.line,
            column: error.location.column,
        }
    }
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

#[cfg(test)]
fn scan_native(source: &str) -> Vec<MarkdownChart<'_>> {
    scan_native_limited(source, None).expect("an unbounded scan cannot exceed its chart limit")
}

pub(crate) fn scan_native_limited(
    source: &str,
    max_charts: Option<u64>,
) -> Result<Vec<MarkdownChart<'_>>, MarkdownChartLimitExceeded> {
    native::scan(source, max_charts)
}

#[cfg(any(feature = "rustdoc", test))]
pub(crate) fn scan_rustdoc_replacements_limited(
    source: &str,
    max_charts: Option<u64>,
) -> Result<Vec<MarkdownReplacement<'_>>, MarkdownReplacementScanError> {
    native::scan_rustdoc(source, max_charts)
}

pub(crate) fn scan_mmdc_11_16_0_limited(
    source: &str,
    max_charts: Option<u64>,
) -> Result<Vec<MarkdownChart<'_>>, MarkdownChartLimitExceeded> {
    strict::scan(source, max_charts)
}

fn admit_chart(
    current_count: usize,
    max_charts: Option<u64>,
    location: MarkdownFenceLocation,
) -> Result<(), MarkdownChartLimitExceeded> {
    let observed = u64::try_from(current_count)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    if let Some(max) = max_charts
        && observed > max
    {
        return Err(MarkdownChartLimitExceeded {
            observed,
            max,
            location,
        });
    }
    Ok(())
}

pub(super) fn trim_line_ending(line: &str) -> &str {
    line.strip_suffix('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .unwrap_or_else(|| line.strip_suffix('\r').unwrap_or(line))
}

pub(super) fn next_line_end(source: &str, start: usize) -> usize {
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

pub(crate) fn rewritten_markdown_len(
    source: &str,
    charts: &[MarkdownChart<'_>],
    images: &[MarkdownImage],
) -> Result<usize, CliError> {
    if charts.len() != images.len() {
        return Err(CliError::InvalidOutput(format!(
            "Markdown rewrite expected {} images for {} charts",
            images.len(),
            charts.len()
        )));
    }
    let pair_count = charts.len();
    let removed = charts
        .iter()
        .take(pair_count)
        .try_fold(0_usize, |total, chart| {
            let source_span = chart.source_span();
            let span = source_span
                .end
                .checked_sub(source_span.start)
                .ok_or_else(|| {
                    CliError::InvalidOutput("invalid Markdown chart span".to_string())
                })?;
            total.checked_add(span).ok_or_else(|| {
                CliError::InvalidOutput("rewritten Markdown size overflow".to_string())
            })
        })?;
    let retained = source
        .len()
        .checked_sub(removed)
        .ok_or_else(|| CliError::InvalidOutput("invalid Markdown chart spans".to_string()))?;
    images
        .iter()
        .take(pair_count)
        .try_fold(retained, |total, image| {
            total
                .checked_add(markdown_image_len(image)?)
                .ok_or_else(|| {
                    CliError::InvalidOutput("rewritten Markdown size overflow".to_string())
                })
        })
}

pub(crate) fn replace_known_charts_with_images(
    source: &str,
    charts: &[MarkdownChart<'_>],
    images: &[MarkdownImage],
    capacity: usize,
) -> String {
    if charts.is_empty() {
        return source.to_string();
    }
    debug_assert_eq!(charts.len(), images.len());

    let mut out = String::with_capacity(capacity);
    let mut last = 0;
    for (chart, image) in charts.iter().zip(images) {
        let source_span = chart.source_span();
        out.push_str(&source[last..source_span.start]);
        out.push_str(&markdown_image(image));
        last = source_span.end;
    }
    out.push_str(&source[last..]);
    debug_assert_eq!(out.len(), capacity);
    out
}

#[cfg(test)]
pub(crate) fn numbered_output_path(
    output_template: &Path,
    index: usize,
    format: RenderFormat,
    artefacts: Option<&Path>,
) -> PathBuf {
    NumberedOutputNamespace::new(output_template, format, artefacts).path(index)
}

pub(crate) fn native_manifest_path(output_root: &Path) -> PathBuf {
    output_root.join(".merman-manifest.json")
}

pub(crate) fn strict_manifest_path(output_template: &Path) -> Result<PathBuf, CliError> {
    let file_name = output_template.file_name().ok_or_else(|| {
        CliError::InvalidOutput(format!(
            "Markdown output template {} must name a file",
            crate::error::safe_path(output_template)
        ))
    })?;
    let mut manifest_name = file_name.to_os_string();
    manifest_name.push(".merman-manifest.json");
    Ok(output_template
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(manifest_name))
}

#[derive(Debug, Clone)]
pub(crate) struct NumberedOutputNamespace {
    directory: PathBuf,
    stem: String,
    extension: String,
}

impl NumberedOutputNamespace {
    pub(crate) fn new(
        output_template: &Path,
        format: RenderFormat,
        artefacts: Option<&Path>,
    ) -> Self {
        let original_ext = output_template
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or_else(|| format.extension());
        let extension = if is_markdown_path(output_template) {
            format.extension()
        } else {
            original_ext
        }
        .to_string();
        let stem = output_template
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("out")
            .to_string();
        let directory = artefacts
            .map(Path::to_path_buf)
            .or_else(|| output_template.parent().map(Path::to_path_buf))
            .unwrap_or_default();

        Self {
            directory,
            stem,
            extension,
        }
    }

    pub(crate) fn directory(&self) -> &Path {
        &self.directory
    }

    pub(crate) fn stem(&self) -> &str {
        &self.stem
    }

    pub(crate) fn extension(&self) -> &str {
        &self.extension
    }

    pub(crate) fn path(&self, index: usize) -> PathBuf {
        self.directory
            .join(format!("{}-{index}.{}", self.stem, self.extension))
    }

    pub(crate) fn contains_file_name(&self, file_name: &std::ffi::OsStr) -> bool {
        let Some(file_name) = file_name.to_str() else {
            return false;
        };
        let prefix = format!("{}-", self.stem);
        let suffix = format!(".{}", self.extension);
        let Some(index) = file_name
            .strip_prefix(&prefix)
            .and_then(|rest| rest.strip_suffix(&suffix))
        else {
            return false;
        };
        !index.is_empty()
            && !index.starts_with('0')
            && index.bytes().all(|byte| byte.is_ascii_digit())
    }
}

pub(crate) fn relative_markdown_url(
    markdown_output: &Path,
    image_output: &Path,
    cwd: &Path,
) -> Result<String, CliError> {
    let base_dir = markdown_output.parent().unwrap_or_else(|| Path::new("."));
    let base = absolute_path(base_dir, cwd);
    let target = absolute_path(image_output, cwd);
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

fn markdown_image_len(image: &MarkdownImage) -> Result<usize, CliError> {
    let alt = escaped_markdown_len(&image.alt, |ch| matches!(ch, '[' | ']' | '\\'))?;
    let mut len = 2_usize
        .checked_add(alt)
        .and_then(|len| len.checked_add(2))
        .and_then(|len| len.checked_add(image.url.len()))
        .ok_or_else(|| CliError::InvalidOutput("rewritten Markdown size overflow".to_string()))?;
    if let Some(title) = image.title.as_deref() {
        let title = escaped_markdown_len(title, |ch| matches!(ch, '"' | '\\'))?;
        len = len
            .checked_add(2)
            .and_then(|len| len.checked_add(title))
            .and_then(|len| len.checked_add(1))
            .ok_or_else(|| {
                CliError::InvalidOutput("rewritten Markdown size overflow".to_string())
            })?;
    }
    len.checked_add(1)
        .ok_or_else(|| CliError::InvalidOutput("rewritten Markdown size overflow".to_string()))
}

fn escaped_markdown_len(
    value: &str,
    must_escape: impl Fn(char) -> bool,
) -> Result<usize, CliError> {
    value.chars().try_fold(0_usize, |len, ch| {
        len.checked_add(ch.len_utf8())
            .and_then(|len| len.checked_add(usize::from(must_escape(ch))))
            .ok_or_else(|| CliError::InvalidOutput("rewritten Markdown size overflow".to_string()))
    })
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

pub(crate) fn absolute_path(path: &Path, cwd: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    lexical_normalize(&absolute)
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if normalized.file_name().is_some() {
                    normalized.pop();
                } else if !normalized.has_root() {
                    normalized.push(component.as_os_str());
                }
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    normalized
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
        let charts = scan_native(source);

        assert_eq!(charts.len(), 3);
        assert!(charts[0].definition().contains("flowchart LR"));
        assert!(charts[1].definition().contains("sequenceDiagram"));
        assert!(charts[2].definition().contains("pie title Work"));
    }

    #[test]
    fn ignores_mermaid_looking_content_inside_another_fence() {
        let source = "````text\n```mermaid\nflowchart LR\nIgnored-->Fence\n```\n````\n\n```mermaid\nflowchart LR\nRendered-->Diagram\n```\n";
        let charts = scan_native(source);

        assert_eq!(charts.len(), 1);
        assert!(charts[0].definition().contains("Rendered-->Diagram"));
        assert!(!charts[0].definition().contains("Ignored-->Fence"));
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
        let charts = scan_native(source);

        assert_eq!(charts.len(), 2);
        assert_eq!(charts[0].definition(), "flowchart TD\nA-->B\n");
        assert_eq!(
            charts[1].definition(),
            "sequenceDiagram\rA->>B: Hi\r",
            "bare CR line endings must be retained in the rendered definition"
        );
    }

    #[test]
    fn native_dialect_retains_an_unclosed_mermaid_fence() {
        let source = "before\n~~~mermaid\nflowchart LR\nA-->B\n";
        let charts = scan_native(source);

        assert_eq!(charts.len(), 1);
        assert_eq!(charts[0].source_span().start, "before\n".len());
        assert_eq!(charts[0].source_span().end, source.len());
        assert_eq!(charts[0].definition(), "flowchart LR\nA-->B\n");
    }

    #[test]
    fn rustdoc_scan_rejects_unclosed_mermaid_without_changing_batch_compatibility() {
        let source = "before\n~~~mermaid\nflowchart LR\nA-->B\n";

        let error = scan_rustdoc_replacements_limited(source, None).expect_err("unclosed fence");

        assert_eq!(
            error,
            MarkdownReplacementScanError::UnclosedMermaidFence { line: 2, column: 1 }
        );
        assert_eq!(
            scan_native_limited(source, None).expect("batch scan").len(),
            1
        );
    }

    #[test]
    fn rustdoc_scan_finds_only_standalone_includes_outside_fences() {
        let source = concat!(
            "include_mmd!(\"docs/one.mmd\")\r\n",
            "prose include_mmd!(\"ignored.mmd\")\r\n",
            "```text\r\n",
            "include_mmd!(\"also-ignored.mmd\")\r\n",
            "```\r\n",
            "```mermaid\r\n",
            "flowchart LR\r\n",
            "A-->B\r\n",
            "```\r\n",
        );

        let replacements = scan_rustdoc_replacements_limited(source, None).expect("Rustdoc scan");

        assert_eq!(replacements.len(), 2);
        let MarkdownReplacement::Include(include) = &replacements[0] else {
            panic!("first replacement should be an include");
        };
        assert_eq!(include.path(), "docs/one.mmd");
        assert_eq!(
            include.location(),
            MarkdownFenceLocation { line: 1, column: 1 }
        );
        assert_eq!(
            &source[include.source_span()],
            "include_mmd!(\"docs/one.mmd\")"
        );
        let MarkdownReplacement::Chart(chart) = &replacements[1] else {
            panic!("second replacement should be a chart");
        };
        assert_eq!(chart.definition(), "flowchart LR\r\nA-->B\r\n");
        assert_eq!(
            chart.location(),
            MarkdownFenceLocation { line: 6, column: 1 }
        );
    }

    #[test]
    fn rustdoc_scan_rejects_malformed_complete_include_directives() {
        for source in [
            "include_mmd!(docs/one.mmd)\n",
            "include_mmd!(\"escaped\\\\path.mmd\")\n",
        ] {
            let error = scan_rustdoc_replacements_limited(source, None)
                .expect_err("malformed include must fail closed");
            assert!(
                matches!(error, MarkdownReplacementScanError::InvalidInclude { .. }),
                "{source:?}: {error}"
            );
        }
    }

    #[test]
    fn rustdoc_scan_preserves_include_like_prose_byte_for_byte() {
        for source in [
            "include_mmd! is documented here\n",
            "include_mmd!(\"one.mmd\") trailing prose\n",
            "include_mmd!(\"unfinished.mmd\"\n",
        ] {
            let replacements = scan_rustdoc_replacements_limited(source, None).expect("scan prose");
            assert!(replacements.is_empty(), "{source:?}: {replacements:?}");
        }
    }

    #[test]
    fn replaces_charts_with_escaped_markdown_images() {
        let source = "```mermaid\nflowchart LR\nA-->B\n```";
        let images = [MarkdownImage {
            url: "./out-1.svg".to_string(),
            title: Some(r#"a "title""#.to_string()),
            alt: r"diagram [一]".to_string(),
        }];
        let charts = scan_native(source);
        let expected = r#"![diagram \[一\]](./out-1.svg "a \"title\"")"#;
        let capacity =
            rewritten_markdown_len(source, &charts, &images).expect("valid rewrite length");

        assert_eq!(capacity, expected.len());
        assert_eq!(
            replace_known_charts_with_images(source, &charts, &images, capacity),
            expected
        );
    }

    #[test]
    fn rewriting_preserves_the_closing_fence_line_ending() {
        for source in [
            "```mermaid\nflowchart LR\nA-->B\n```\nafter\n",
            "```mermaid\r\nflowchart LR\r\nA-->B\r\n```\r\nafter\r\n",
        ] {
            let charts = scan_native(source);
            let images = [MarkdownImage {
                url: "./out.svg".to_string(),
                title: None,
                alt: "diagram".to_string(),
            }];
            let capacity =
                rewritten_markdown_len(source, &charts, &images).expect("valid rewrite length");
            let rewritten = replace_known_charts_with_images(source, &charts, &images, capacity);

            assert!(
                rewritten.contains("![diagram](./out.svg)\nafter")
                    || rewritten.contains("![diagram](./out.svg)\r\nafter"),
                "closing line ending must remain after replacement: {rewritten:?}"
            );
        }
    }

    #[test]
    fn scanners_report_the_rejected_fence_location_at_limit_plus_one() {
        let native = "标题\n  ```Mermaid\nflowchart LR\nA-->B\n```\n";
        let native_error = scan_native_limited(native, Some(0)).expect_err("limit");
        assert_eq!(
            native_error.location,
            MarkdownFenceLocation { line: 2, column: 3 }
        );

        let strict = concat!("```mermaid\nA\n```\n", "\u{2028}\t```mermaid\nB\n```\n",);
        let strict_error = scan_mmdc_11_16_0_limited(strict, Some(1)).expect_err("limit");
        assert_eq!(strict_error.observed, 2);
        assert_eq!(
            strict_error.location,
            MarkdownFenceLocation { line: 5, column: 2 }
        );
    }

    #[test]
    fn rewrite_rejects_a_partial_image_set() {
        let source = "```mermaid\nflowchart LR\nA-->B\n```\n";
        let charts = scan_native(source);

        let error = rewritten_markdown_len(source, &charts, &[]).expect_err("image mismatch");

        assert!(error.to_string().contains("0 images for 1 charts"));
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
