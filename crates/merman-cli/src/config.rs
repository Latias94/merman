use crate::cli::{MathRendererKind, ParseCliArgs, RenderCliArgs, TextMeasurerKind};
use crate::error::CliError;
use crate::io::read_named_text_file;
use merman::render::{
    HeadlessRenderer, IconRegistry, LayoutOptions, MathRenderer, RenderEnvironment, RuntimePolicy,
    SvgRenderOptions, TextMeasurementPolicy,
};
use merman::{Engine, MermaidConfig, ParseOptions};
use serde_json::Value;
use std::sync::Arc;

pub(crate) fn engine_for(parse: &ParseCliArgs, render: &RenderCliArgs) -> Result<Engine, CliError> {
    let site_config = site_config_for(parse, render)?;
    let time = ResolvedCliTimePolicy::new(parse)?;
    Ok(time.apply_engine(Engine::new().with_site_config(site_config)))
}

#[derive(Debug, Clone)]
struct ResolvedCliTimePolicy {
    runtime_policy: RuntimePolicy,
}

impl ResolvedCliTimePolicy {
    fn new(parse: &ParseCliArgs) -> Result<Self, CliError> {
        let mut runtime_policy = RuntimePolicy::try_native()
            .map_err(|err| CliError::InvalidInput(format!("native runtime unavailable: {err}")))?;
        if let Some(offset_minutes) = parse.fixed_local_offset_minutes {
            runtime_policy = runtime_policy
                .try_with_fixed_local_offset_minutes(offset_minutes)
                .map_err(|err| CliError::InvalidInput(err.to_string()))?;
        }
        if let Some(today) = parse.fixed_today {
            let local_midnight = today
                .and_hms_opt(0, 0, 0)
                .expect("every valid date has a valid midnight");
            let local = runtime_policy
                .local_time_zone()
                .datetime_from_naive_local(local_midnight)
                .ok_or_else(|| {
                    CliError::InvalidInput(format!(
                        "--fixed-today cannot be resolved in the selected timezone: {today}"
                    ))
                })?;
            runtime_policy = runtime_policy
                .with_fixed_unix_millis(local.timestamp_millis())
                .with_fixed_today(Some(today));
        }
        Ok(Self { runtime_policy })
    }

    fn apply_engine(&self, engine: Engine) -> Engine {
        engine.with_runtime_policy(self.runtime_policy.clone())
    }

    fn apply_environment(&self, environment: RenderEnvironment) -> RenderEnvironment {
        environment.with_runtime_policy(self.runtime_policy.clone())
    }
}

pub(crate) fn runtime_policy_for(parse: &ParseCliArgs) -> Result<RuntimePolicy, CliError> {
    Ok(ResolvedCliTimePolicy::new(parse)?.runtime_policy)
}

pub(crate) fn site_config_for(
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

pub(crate) fn layout_options(render: &RenderCliArgs) -> LayoutOptions {
    LayoutOptions {
        container_width: render.container_width.unwrap_or(800.0),
        container_height: render.container_height.unwrap_or(600.0),
    }
}

fn text_measurement_policy(kind: TextMeasurerKind) -> TextMeasurementPolicy {
    match kind {
        TextMeasurerKind::Deterministic => TextMeasurementPolicy::deterministic(),
        TextMeasurerKind::Vendored => TextMeasurementPolicy::parity(),
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

pub(crate) fn renderer_for(
    parse: &ParseCliArgs,
    render: &RenderCliArgs,
    icon_registry: Option<Arc<IconRegistry>>,
) -> Result<HeadlessRenderer, CliError> {
    let time = ResolvedCliTimePolicy::new(parse)?;
    let mut environment = RenderEnvironment::deterministic()
        .with_text_measurement_policy(text_measurement_policy(render.text_measurer))
        .with_resource_policy(merman::render::RenderResourcePolicy::for_profile(
            render.resource_profile,
        ));
    if let Some(renderer) = math_renderer(render.math_renderer)? {
        environment = environment.with_math_renderer(renderer);
    }
    if let Some(registry) = icon_registry {
        environment = environment.with_icon_registry(registry);
    }
    environment = time.apply_environment(environment);

    let svg = SvgRenderOptions {
        diagram_id: render
            .svg_id
            .as_deref()
            .map(merman::render::sanitize_svg_id),
        ..SvgRenderOptions::default()
    };

    let engine = time.apply_engine(Engine::new().with_site_config(site_config_for(parse, render)?));
    Ok(
        HeadlessRenderer::from_engine_and_environment(engine, environment)
            .with_parse_options(parse_options(parse))
            .with_layout_options(layout_options(render))
            .with_svg_options(svg),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_offset_only_preserves_the_native_cli_clock() {
        let parse = ParseCliArgs {
            fixed_local_offset_minutes: Some(480),
            ..Default::default()
        };
        let resolved = ResolvedCliTimePolicy::new(&parse).expect("CLI time policy");
        let context = resolved
            .runtime_policy
            .begin_operation()
            .expect("native operation context");

        assert_eq!(
            context.clock_source(),
            merman::render::RuntimeValueSource::System
        );
        assert_eq!(context.local_time_zone().fixed_offset_minutes(), Some(480));
    }

    #[test]
    fn boundary_fixed_today_returns_invalid_input_instead_of_panicking() {
        let parse = ParseCliArgs {
            fixed_today: Some(chrono::NaiveDate::MIN),
            fixed_local_offset_minutes: Some(1439),
            ..Default::default()
        };

        let error =
            ResolvedCliTimePolicy::new(&parse).expect_err("boundary instant must be rejected");
        assert!(matches!(error, CliError::InvalidInput(_)));
        assert!(error.to_string().contains("local datetime"));
    }
}
