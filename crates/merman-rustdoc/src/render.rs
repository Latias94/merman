use std::fs;
use std::path::PathBuf;

use merman::{MermaidConfig, render::HeadlessRenderer};
use serde_json::Value;

use crate::error::{Error, Result};
use crate::options::{Options, PipelineMode, ThemeMode};

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum RenderedDiagram {
    Single(String),
    RustdocTheme { light: String, dark: String },
}

pub(crate) trait MermaidRenderer {
    fn render_mermaid_diagram(
        &mut self,
        source: &str,
        index: usize,
        options: Options,
    ) -> Result<RenderedDiagram>;
}

impl<F> MermaidRenderer for F
where
    F: FnMut(&str, usize, Options) -> Result<RenderedDiagram>,
{
    fn render_mermaid_diagram(
        &mut self,
        source: &str,
        index: usize,
        options: Options,
    ) -> Result<RenderedDiagram> {
        self(source, index, options)
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
    fn render_mermaid_diagram(
        &mut self,
        source: &str,
        index: usize,
        options: Options,
    ) -> Result<RenderedDiagram> {
        render_mermaid_diagram(source, index, options)
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

fn render_mermaid_diagram(source: &str, index: usize, options: Options) -> Result<RenderedDiagram> {
    let base_id = diagram_id(source, index);
    match options.theme {
        ThemeMode::Rustdoc => {
            let light = render_mermaid_svg(
                source,
                index,
                options.pipeline,
                &format!("{base_id}-light"),
                Some("default"),
                "rustdoc light theme",
            )?;
            let dark = render_mermaid_svg(
                source,
                index,
                options.pipeline,
                &format!("{base_id}-dark"),
                Some("dark"),
                "rustdoc dark theme",
            )?;
            Ok(RenderedDiagram::RustdocTheme { light, dark })
        }
        ThemeMode::Mermaid => {
            let svg = render_mermaid_svg(
                source,
                index,
                options.pipeline,
                &base_id,
                None,
                "Mermaid theme",
            )?;
            Ok(RenderedDiagram::Single(svg))
        }
        ThemeMode::Fixed(theme) => {
            let svg = render_mermaid_svg(
                source,
                index,
                options.pipeline,
                &base_id,
                Some(theme),
                theme,
            )?;
            Ok(RenderedDiagram::Single(svg))
        }
    }
}

fn render_mermaid_svg(
    source: &str,
    index: usize,
    pipeline: PipelineMode,
    diagram_id: &str,
    site_theme: Option<&str>,
    context: &str,
) -> Result<String> {
    let mut renderer = HeadlessRenderer::new().with_diagram_id(diagram_id);
    if let Some(theme) = site_theme {
        let mut config = MermaidConfig::empty_object();
        config.set_value("theme", Value::String(theme.to_string()));
        renderer = renderer.with_site_config(config);
    }

    let rendered = match pipeline {
        PipelineMode::Parity => renderer.render_svg_sync(source),
        PipelineMode::Readable => renderer.render_svg_readable_sync(source),
        PipelineMode::ResvgSafe => renderer.render_svg_resvg_safe_sync(source),
    };
    rendered
        .map_err(|err| {
            Error::new(format!(
                "failed to render Mermaid diagram #{} for rustdoc ({context}): {err}",
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

    #[test]
    fn default_rustdoc_theme_renders_light_and_dark_svgs() {
        let source = "flowchart TD\nA[Plain source] --> B[Themed]";
        let rendered = render_mermaid_diagram(source, 0, Options::default()).unwrap();

        let RenderedDiagram::RustdocTheme { light, dark } = rendered else {
            panic!("expected rustdoc theme variants");
        };
        assert!(light.contains(r#"id="merman-rustdoc-0-"#));
        assert!(light.contains("-light"));
        assert!(dark.contains("-dark"));
        assert_ne!(light, dark);
    }

    #[test]
    fn fixed_theme_renders_single_svg() {
        let source = "flowchart TD\nA[Plain source] --> B[Themed]";
        let rendered = render_mermaid_diagram(
            source,
            0,
            Options {
                theme: ThemeMode::Fixed("dark"),
                ..Options::default()
            },
        )
        .unwrap();

        assert!(matches!(rendered, RenderedDiagram::Single(_)));
    }

    #[test]
    fn source_level_theme_overrides_rustdoc_theme() {
        let source = r#"%%{init: {"theme": "default"}}%%
flowchart TD
A[Source theme] --> B[Rustdoc theme]
"#;
        let source_default = render_mermaid_diagram(
            source,
            0,
            Options {
                theme: ThemeMode::Fixed("default"),
                ..Options::default()
            },
        )
        .unwrap();
        let rustdoc = render_mermaid_diagram(source, 0, Options::default()).unwrap();

        let RenderedDiagram::Single(source_default_svg) = source_default else {
            panic!("expected fixed theme to render one SVG");
        };
        let RenderedDiagram::RustdocTheme { light, dark } = rustdoc else {
            panic!("expected rustdoc theme variants");
        };

        assert_eq!(
            strip_theme_suffixes(&source_default_svg),
            strip_theme_suffixes(&light)
        );
        assert_eq!(
            strip_theme_suffixes(&source_default_svg),
            strip_theme_suffixes(&dark)
        );
    }

    fn strip_theme_suffixes(svg: &str) -> String {
        svg.replace("-light", "").replace("-dark", "")
    }
}
