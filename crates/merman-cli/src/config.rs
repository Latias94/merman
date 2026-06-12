use crate::cli::{MathRendererKind, ParseCliArgs, RenderCliArgs, TextMeasurerKind};
use crate::error::CliError;
use crate::io::read_named_text_file;
use merman::render::{
    DeterministicTextMeasurer, LayoutOptions, MathRenderer, TextMeasurer,
    VendoredFontMetricsTextMeasurer,
};
use merman::{Engine, MermaidConfig, ParseOptions};
use serde_json::Value;
use std::sync::Arc;

pub(crate) fn engine_for(parse: &ParseCliArgs, render: &RenderCliArgs) -> Result<Engine, CliError> {
    let site_config = site_config_for(parse, render)?;
    Ok(Engine::new()
        .with_fixed_today(parse.fixed_today)
        .with_fixed_local_offset_minutes(parse.fixed_local_offset_minutes)
        .with_site_config(site_config))
}

fn site_config_for(
    parse: &ParseCliArgs,
    render: &RenderCliArgs,
) -> Result<MermaidConfig, CliError> {
    let mut cfg = MermaidConfig::empty_object();

    if let Some(theme) = parse
        .theme
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        cfg.set_value("theme", serde_json::json!(theme));
    }

    if let Some(path) = parse.config_file.as_deref() {
        let text = read_named_text_file(path, "configuration file")?;
        let value: Value = serde_json::from_str(&text)?;
        if !value.is_object() {
            return Err(CliError::InvalidInput(
                "configuration file must contain a JSON object".to_string(),
            ));
        }
        cfg.deep_merge(&value);
    }

    if let Some(seed) = render.hand_drawn_seed {
        cfg.set_value("handDrawnSeed", serde_json::json!(seed));
    }

    Ok(cfg)
}

pub(crate) fn parse_options(parse: &ParseCliArgs) -> ParseOptions {
    ParseOptions {
        suppress_errors: parse.suppress_errors,
    }
}

pub(crate) fn layout_options(
    render: &RenderCliArgs,
    math_renderer: Option<Arc<dyn MathRenderer + Send + Sync>>,
) -> LayoutOptions {
    LayoutOptions {
        viewport_width: render.width.unwrap_or(800.0),
        viewport_height: render.height.unwrap_or(600.0),
        text_measurer: text_measurer(render.text_measurer),
        math_renderer,
        // Mermaid parity for some diagrams relies on manatee-backed layout engines.
        use_manatee_layout: true,
    }
}

fn text_measurer(kind: TextMeasurerKind) -> Arc<dyn TextMeasurer + Send + Sync> {
    match kind {
        TextMeasurerKind::Deterministic => Arc::new(DeterministicTextMeasurer::default()),
        TextMeasurerKind::Vendored => Arc::new(VendoredFontMetricsTextMeasurer::default()),
    }
}

pub(crate) fn math_renderer(
    kind: MathRendererKind,
) -> Result<Option<Arc<dyn MathRenderer + Send + Sync>>, CliError> {
    match kind {
        MathRendererKind::None => Ok(None),
        MathRendererKind::Ratex => {
            #[cfg(feature = "ratex-math")]
            {
                Ok(Some(Arc::new(merman::render::RatexMathRenderer)))
            }

            #[cfg(not(feature = "ratex-math"))]
            {
                Err(CliError::InvalidInput(
                    "RaTeX math rendering requires building merman-cli with --features ratex-math."
                        .to_string(),
                ))
            }
        }
    }
}
