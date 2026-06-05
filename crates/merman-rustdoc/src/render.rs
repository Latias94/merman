use std::fs;
use std::path::PathBuf;

use merman::render::HeadlessRenderer;

use crate::error::{Error, Result};
use crate::options::PipelineMode;

pub(crate) trait MermaidRenderer {
    fn render_mermaid_svg(
        &mut self,
        source: &str,
        index: usize,
        pipeline: PipelineMode,
    ) -> Result<String>;
}

impl<F> MermaidRenderer for F
where
    F: FnMut(&str, usize, PipelineMode) -> Result<String>,
{
    fn render_mermaid_svg(
        &mut self,
        source: &str,
        index: usize,
        pipeline: PipelineMode,
    ) -> Result<String> {
        self(source, index, pipeline)
    }
}

pub(crate) trait IncludeResolver {
    fn read_include_mmd(&mut self, path: &str) -> Result<String>;
}

impl<F> IncludeResolver for F
where
    F: FnMut(&str) -> Result<String>,
{
    fn read_include_mmd(&mut self, path: &str) -> Result<String> {
        self(path)
    }
}

pub(crate) struct HeadlessMermaidRenderer;

impl MermaidRenderer for HeadlessMermaidRenderer {
    fn render_mermaid_svg(
        &mut self,
        source: &str,
        index: usize,
        pipeline: PipelineMode,
    ) -> Result<String> {
        render_mermaid_svg(source, index, pipeline)
    }
}

pub(crate) struct ManifestIncludeResolver;

impl IncludeResolver for ManifestIncludeResolver {
    fn read_include_mmd(&mut self, path: &str) -> Result<String> {
        read_include_mmd(path)
    }
}

pub(crate) fn source_preview(source: &str) -> String {
    let preview = source
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("<empty>");
    const MAX_PREVIEW_CHARS: usize = 80;
    if preview.chars().count() <= MAX_PREVIEW_CHARS {
        return preview.to_string();
    }

    let mut out = preview.chars().take(MAX_PREVIEW_CHARS).collect::<String>();
    out.push_str("...");
    out
}

fn render_mermaid_svg(source: &str, index: usize, pipeline: PipelineMode) -> Result<String> {
    let diagram_id = diagram_id(source, index);
    let renderer = HeadlessRenderer::new().with_diagram_id(&diagram_id);
    let rendered = match pipeline {
        PipelineMode::Parity => renderer.render_svg_sync(source),
        PipelineMode::Readable => renderer.render_svg_readable_sync(source),
        PipelineMode::ResvgSafe => renderer.render_svg_resvg_safe_sync(source),
    };
    rendered
        .map_err(|err| {
            Error::new(format!(
                "failed to render Mermaid diagram #{} for rustdoc: {err}",
                index + 1
            ))
        })?
        .ok_or_else(|| {
            Error::new(format!(
                "Mermaid diagram #{} did not produce SVG output",
                index + 1
            ))
        })
}

fn read_include_mmd(path: &str) -> Result<String> {
    let base = std::env::var_os("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let full_path = base.join(path);
    fs::read_to_string(&full_path).map_err(|err| {
        Error::new(format!(
            "failed to read Mermaid include `{path}` at `{}`: {err}",
            full_path.display()
        ))
    })
}

fn diagram_id(source: &str, index: usize) -> String {
    format!("merman-rustdoc-{index}-{:016x}", fnv1a64(source.as_bytes()))
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagram_ids_are_stable_and_indexed() {
        assert_eq!(
            diagram_id("flowchart TD\nA-->B", 0),
            diagram_id("flowchart TD\nA-->B", 0)
        );
        assert_ne!(
            diagram_id("flowchart TD\nA-->B", 0),
            diagram_id("flowchart TD\nA-->B", 1)
        );
        assert_ne!(
            diagram_id("flowchart TD\nA-->B", 0),
            diagram_id("flowchart TD\nA-->C", 0)
        );
    }
}
