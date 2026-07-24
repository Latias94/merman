use crate::cli::ParseCliArgs;
use crate::error::CliError;
use crate::io::read_named_text_file;
use merman::runtime::RuntimePolicy;
use merman::{Engine, MermaidConfig, ParseOptions};
use serde_json::Value;

#[cfg(feature = "svg")]
use crate::cli::{MathRendererKind, RenderCliArgs, TextMeasurerKind};
#[cfg(feature = "svg")]
use merman::svg::{
    HeadlessRenderer, IconRegistry, LayoutOptions, MathRenderer, RenderEnvironment,
    SvgRenderOptions, TextMeasurementPolicy,
};
#[cfg(feature = "svg")]
use std::sync::Arc;

pub(crate) fn engine_for(parse: &ParseCliArgs) -> Result<Engine, CliError> {
    let time = ResolvedCliTimePolicy::new(parse)?;
    Ok(time.apply_engine(Engine::new().with_site_config(site_config_for(parse)?)))
}

#[derive(Debug, Clone)]
struct ResolvedCliTimePolicy {
    runtime_policy: RuntimePolicy,
}

impl ResolvedCliTimePolicy {
    fn new(parse: &ParseCliArgs) -> Result<Self, CliError> {
        let mut runtime_policy = default_runtime_policy()?;
        if let Some(offset_minutes) = parse.fixed_local_offset_minutes {
            runtime_policy = runtime_policy
                .try_with_fixed_local_offset_minutes(offset_minutes)
                .map_err(|err| CliError::InvalidInput(err.to_string()))?;
        }
        if let Some(today) = parse.fixed_today {
            runtime_policy = runtime_policy
                .try_with_fixed_today_at_local_midnight(today)
                .map_err(|err| CliError::InvalidInput(err.to_string()))?;
        }
        Ok(Self { runtime_policy })
    }

    fn apply_engine(&self, engine: Engine) -> Engine {
        engine.with_runtime_policy(self.runtime_policy.clone())
    }

    #[cfg(feature = "svg")]
    fn apply_environment(&self, environment: RenderEnvironment) -> RenderEnvironment {
        environment.with_runtime_policy(self.runtime_policy.clone())
    }
}

#[cfg(all(
    feature = "system-clock",
    feature = "system-timezone",
    feature = "system-random",
    feature = "system-timing"
))]
fn default_runtime_policy() -> Result<RuntimePolicy, CliError> {
    RuntimePolicy::try_native()
        .map_err(|err| CliError::InvalidInput(format!("native runtime unavailable: {err}")))
}

#[cfg(not(all(
    feature = "system-clock",
    feature = "system-timezone",
    feature = "system-random",
    feature = "system-timing"
)))]
fn default_runtime_policy() -> Result<RuntimePolicy, CliError> {
    Ok(RuntimePolicy::deterministic())
}

#[cfg(feature = "analysis")]
pub(crate) fn runtime_policy_for(parse: &ParseCliArgs) -> Result<RuntimePolicy, CliError> {
    Ok(ResolvedCliTimePolicy::new(parse)?.runtime_policy)
}

pub(crate) fn site_config_for(parse: &ParseCliArgs) -> Result<MermaidConfig, CliError> {
    let mut cfg = MermaidConfig::empty_object();

    if let Some(theme) = parse
        .theme
        .as_deref()
        .map(str::trim)
        .filter(|theme| !theme.is_empty())
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

    Ok(cfg)
}

pub(crate) fn parse_options(parse: &ParseCliArgs) -> ParseOptions {
    ParseOptions {
        suppress_errors: parse.suppress_errors,
    }
}

#[cfg(feature = "svg")]
pub(crate) fn renderer_for(
    parse: &ParseCliArgs,
    render: &RenderCliArgs,
    icon_registry: Option<Arc<IconRegistry>>,
) -> Result<HeadlessRenderer, CliError> {
    let time = ResolvedCliTimePolicy::new(parse)?;
    let mut environment = RenderEnvironment::deterministic()
        .with_text_measurement_policy(text_measurement_policy(render.text_measurer))
        .with_resource_policy(merman::svg::RenderResourcePolicy::for_profile(
            render.resource_profile,
        ));
    environment = match math_renderer(render.math_renderer)? {
        Some(renderer) => environment.with_math_renderer(renderer),
        None => environment.without_math_renderer(),
    };
    if let Some(registry) = icon_registry {
        environment = environment.with_icon_registry(registry);
    }
    environment = time.apply_environment(environment);

    let svg = SvgRenderOptions {
        diagram_id: render.svg_id.as_deref().map(merman::svg::sanitize_svg_id),
        ..SvgRenderOptions::default()
    };

    let mut site_config = site_config_for(parse)?;
    if let Some(seed) = render.hand_drawn_seed {
        site_config.set_value("handDrawnSeed", serde_json::json!(seed));
    }

    let engine = time.apply_engine(Engine::new().with_site_config(site_config));
    Ok(
        HeadlessRenderer::from_engine_and_environment(engine, environment)
            .with_parse_options(parse_options(parse))
            .with_layout_options(LayoutOptions {
                container_width: render.container_width.unwrap_or(800.0),
                container_height: render.container_height.unwrap_or(600.0),
            })
            .with_svg_options(svg),
    )
}

#[cfg(feature = "svg")]
fn text_measurement_policy(kind: TextMeasurerKind) -> TextMeasurementPolicy {
    match kind {
        TextMeasurerKind::Deterministic => TextMeasurementPolicy::deterministic(),
        TextMeasurerKind::Vendored => TextMeasurementPolicy::parity(),
    }
}

#[cfg(feature = "svg")]
fn math_renderer(
    kind: MathRendererKind,
) -> Result<Option<Arc<dyn MathRenderer + Send + Sync>>, CliError> {
    match kind {
        MathRendererKind::None => Ok(None),
        MathRendererKind::Ratex => {
            #[cfg(feature = "math")]
            {
                Ok(Some(Arc::new(merman::svg::RatexMathRenderer)))
            }

            #[cfg(not(feature = "math"))]
            {
                Err(CliError::InvalidInput(
                    "RaTeX math rendering requires building merman-cli with --features math."
                        .to_string(),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "svg")]
    #[test]
    fn none_math_renderer_disables_the_compiled_default() {
        let renderer = renderer_for(&ParseCliArgs::default(), &RenderCliArgs::default(), None)
            .expect("CLI renderer");
        let error = renderer
            .render_svg_sync("flowchart TD\nA[\"$$x^2$$\"] --> B[Done]")
            .expect_err("explicitly disabling math must reject math labels");

        match error {
            merman::svg::HeadlessError::Render(merman::svg::RenderError::MissingCapability {
                capability,
                diagram_type: _,
            }) => assert_eq!(capability, merman::svg::RenderCapability::Math),
            other => panic!("expected a missing math capability error, got {other:?}"),
        }
    }

    #[cfg(all(
        feature = "system-clock",
        feature = "system-timezone",
        feature = "system-random",
        feature = "system-timing"
    ))]
    #[test]
    fn native_profile_preserves_the_system_clock() {
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
            merman::runtime::RuntimeValueSource::System
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

    #[cfg(not(all(
        feature = "system-clock",
        feature = "system-timezone",
        feature = "system-random",
        feature = "system-timing"
    )))]
    #[test]
    fn lean_profiles_use_the_deterministic_runtime_policy() {
        let context = ResolvedCliTimePolicy::new(&ParseCliArgs::default())
            .expect("deterministic CLI policy")
            .runtime_policy
            .begin_operation()
            .expect("deterministic operation context");

        assert_eq!(
            context.clock_source(),
            merman::runtime::RuntimeValueSource::Fixed
        );
    }
}
