use crate::cli::RenderFormat;
use crate::error::CliError;
use merman_analysis::{DocumentSource, source_descriptor_for_markdown_path};
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

pub(crate) fn is_markdown_path(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("md" | "markdown" | "mdx")
    )
}

pub(crate) fn extract_charts(source: &str) -> Vec<MarkdownChart> {
    extract_charts_with_spans(source)
}

pub(crate) fn extract_charts_with_spans(source: &str) -> Vec<MarkdownChart> {
    let document = DocumentSource::new(source, source_descriptor_for_markdown_path(None));
    document
        .diagrams()
        .iter()
        .map(|diagram| MarkdownChart {
            start: diagram.start,
            end: diagram.end,
            definition: diagram.text.to_owned_text(),
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
        let source = "before\n```Mermaid title=Main\nflowchart LR\nA-->B\n```\n~~~ mermaid\nsequenceDiagram\nA->>B: Hi\n~~~\n:::MERMAID extra info\npie title Work\n:::\n```mermaidx\nignored\n```\nafter";
        let charts = extract_charts(source);

        assert_eq!(charts.len(), 3);
        assert!(charts[0].definition.contains("flowchart LR"));
        assert!(charts[1].definition.contains("sequenceDiagram"));
        assert!(charts[2].definition.contains("pie title Work"));
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
