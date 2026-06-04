use crate::cli::RenderFormat;
use crate::error::CliError;
use regex::Regex;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

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

pub(crate) fn is_markdown_path(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("md" | "markdown")
    )
}

pub(crate) fn extract_charts(source: &str) -> Vec<MarkdownChart> {
    chart_regex()
        .captures_iter(source)
        .filter_map(|captures| {
            let whole = captures.get(0)?;
            let definition = captures.get(2)?;
            Some(MarkdownChart {
                start: whole.start(),
                end: whole.end(),
                definition: definition.as_str().to_string(),
            })
        })
        .collect()
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

fn chart_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?ms)^[^\S\n]*[`:]{3}(?:mermaid)([^\S\n]*\r?\n(.*?))[`:]{3}[^\S\n]*$")
            .expect("valid Mermaid Markdown chart regex")
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
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_backtick_and_colon_mermaid_blocks() {
        let source = "before\n```mermaid\nflowchart LR\nA-->B\n```\n:::mermaid\nsequenceDiagram\nA->>B: Hi\n:::\nafter";
        let charts = extract_charts(&source);

        assert_eq!(charts.len(), 2);
        assert!(charts[0].definition.contains("flowchart LR"));
        assert!(charts[1].definition.contains("sequenceDiagram"));
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
        let out = numbered_output_path(Path::new("docs/out.md"), 2, RenderFormat::Png, None);

        assert_eq!(out, PathBuf::from("docs/out-2.png"));
    }
}
