use std::fs;
use std::path::PathBuf;

use merman::{
    MermaidConfig, OperationControl, RenderError, RenderOutput, RenderRequest, Renderer,
    SvgEnvironment, SvgRequest,
    svg::{RenderCapability, SvgOutputPolicy, SvgPipelinePreset},
};
use serde_json::Value;

use crate::error::{Error, Result};
use crate::options::{Options, PipelineMode, SanitizeMode, ThemeMode};

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum RenderedDiagram {
    Single(String),
    RustdocTheme { light: String, dark: String },
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

pub(crate) fn render_mermaid_diagram(
    source: &str,
    index: usize,
    options: &Options,
    namespace: &str,
) -> Result<RenderedDiagram> {
    let base_id = match &options.id_prefix {
        Some(prefix) => format!("{prefix}-{namespace}-{index}"),
        None => format!("{namespace}-{index}"),
    };
    match options.theme {
        ThemeMode::Rustdoc => {
            let light = render_mermaid_svg(
                source,
                index,
                options,
                &format!("{base_id}-light"),
                Some("default"),
                "rustdoc light theme",
            )?;
            let dark = render_mermaid_svg(
                source,
                index,
                options,
                &format!("{base_id}-dark"),
                Some("dark"),
                "rustdoc dark theme",
            )?;
            Ok(RenderedDiagram::RustdocTheme { light, dark })
        }
        ThemeMode::Mermaid => {
            let svg = render_mermaid_svg(source, index, options, &base_id, None, "Mermaid theme")?;
            Ok(RenderedDiagram::Single(svg))
        }
        ThemeMode::Fixed(theme) => {
            let svg = render_mermaid_svg(source, index, options, &base_id, Some(theme), theme)?;
            Ok(RenderedDiagram::Single(svg))
        }
    }
}

fn render_mermaid_svg(
    source: &str,
    index: usize,
    options: &Options,
    diagram_id: &str,
    site_theme: Option<&str>,
    context: &str,
) -> Result<String> {
    let mut engine = merman::Engine::new();
    if let Some(theme) = site_theme {
        let mut config = MermaidConfig::empty_object();
        config.set_value("theme", Value::String(theme.to_string()));
        engine = engine.with_site_config(config);
    }

    let policy = SvgOutputPolicy {
        preset: match options.pipeline {
            PipelineMode::Parity => SvgPipelinePreset::Parity,
            PipelineMode::Readable => SvgPipelinePreset::Readable,
            PipelineMode::ResvgSafe => SvgPipelinePreset::ResvgSafe,
        },
        root_background_color: Some(options.background.clone()),
        ..Default::default()
    };
    let pipeline = match options.sanitize {
        SanitizeMode::Strict => policy.pipeline().with_browser_inline_contract(diagram_id),
        SanitizeMode::Off => policy.pipeline().with_rebased_ids(diagram_id),
    };
    let request = SvgRequest {
        environment: SvgEnvironment::deterministic(),
        pipeline: Some(pipeline),
        options: merman::svg::SvgRenderOptions {
            diagram_id: Some(diagram_id.to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
    let rendered = Renderer::new()
        .with_engine(engine)
        .render(RenderRequest::svg(source, OperationControl::new(), request))
        .map_err(|err| {
            let hint = match &err {
                RenderError::Svg(error)
                    if error.missing_capability() == Some(RenderCapability::Math) =>
                {
                    "; enable the `math` or `complete-svg` feature on the `merman-rustdoc` dependency"
                }
                _ => "",
            };
            Error::new(format!(
                "failed to render Mermaid diagram #{} for rustdoc ({context}): {err}{hint}",
                index + 1
            ))
        })?;
    let RenderOutput::Svg(svg) = rendered else {
        unreachable!("SVG request must return SVG output");
    };
    svg.map(|output| output.into_parts().0).ok_or_else(|| {
        Error::new(format!(
            "Mermaid diagram #{} did not produce SVG output",
            index + 1
        ))
    })
}

pub(crate) fn read_include_mmd(path: &str) -> Result<String> {
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

pub(crate) fn stable_hash(bytes: &[u8]) -> u64 {
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

    fn render(source: &str, index: usize, options: Options) -> Result<RenderedDiagram> {
        render_mermaid_diagram(source, index, &options, "merman-rustdoc-test")
    }

    #[test]
    fn default_rustdoc_theme_renders_light_and_dark_svgs() {
        let source = "flowchart TD\nA[Plain source] --> B[Themed]";
        let rendered = render(source, 0, Options::default()).unwrap();

        let RenderedDiagram::RustdocTheme { light, dark } = rendered else {
            panic!("expected rustdoc theme variants");
        };
        assert!(light.contains(r#"id="merman-rustdoc-test-0"#));
        assert!(light.contains("-light"));
        assert!(dark.contains("-dark"));
        assert_ne!(light, dark);
    }

    #[test]
    fn fixed_theme_renders_single_svg() {
        let source = "flowchart TD\nA[Plain source] --> B[Themed]";
        let rendered = render(
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
    fn default_pipeline_keeps_one_browser_label_representation() {
        let source = r#"classDiagram
    EventConsumer <|.. OnClickConsumer
    OnClick <.. OnClickConsumer : uses
"#;
        let rendered = render(
            source,
            0,
            Options {
                theme: ThemeMode::Fixed("default"),
                ..Options::default()
            },
        )
        .unwrap();

        let RenderedDiagram::Single(svg) = rendered else {
            panic!("expected fixed theme to render one SVG");
        };
        assert!(svg.contains("<foreignObject"), "{svg}");
        assert!(
            !svg.contains(r#"data-merman-foreignobject="fallback""#),
            "default rustdoc output must not add a second visible label representation: {svg}"
        );
        assert!(
            svg.contains(r#"class="edge-thickness-normal edge-pattern-dashed relation""#),
            "expected dotted Class relations to keep Mermaid's dashed edge pattern: {svg}"
        );
        assert!(
            svg.contains(".edge-pattern-dashed{stroke-dasharray:3;}"),
            "expected Class SVG to include Mermaid's shared dashed-edge CSS: {svg}"
        );
    }

    #[test]
    fn readable_pipeline_remains_an_explicit_fallback_overlay_option() {
        let rendered = render(
            "flowchart TD\nA[Start] --> B[Done]",
            0,
            Options {
                pipeline: PipelineMode::Readable,
                theme: ThemeMode::Fixed("default"),
                ..Options::default()
            },
        )
        .unwrap();

        let RenderedDiagram::Single(svg) = rendered else {
            panic!("expected fixed theme to render one SVG");
        };
        assert!(
            svg.contains(r#"data-merman-foreignobject="fallback""#),
            "explicit readable output should retain SVG text fallbacks: {svg}"
        );
    }

    #[test]
    fn source_level_theme_overrides_rustdoc_theme() {
        let source = r#"%%{init: {"theme": "default"}}%%
flowchart TD
A[Source theme] --> B[Rustdoc theme]
"#;
        let source_default = render(
            source,
            0,
            Options {
                theme: ThemeMode::Fixed("default"),
                ..Options::default()
            },
        )
        .unwrap();
        let rustdoc = render(source, 0, Options::default()).unwrap();

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
