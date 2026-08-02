use crate::cli::{ParseCliArgs, RuntimeCliArgs, RuntimePolicyKind};
use crate::error::CliError;
use crate::input::InputLimit;
use crate::io::read_named_text_file;
use crate::resources::ResolvedResourcePolicy;
use merman::runtime::RuntimePolicy;
use merman::{Engine, MermaidConfig, ParseOptions};
use serde_json::Value;
use std::path::Path;

#[cfg(feature = "svg")]
use crate::cli::{MathRendererKind, RenderCliArgs, TextMeasurerKind};
#[cfg(feature = "svg")]
use crate::invocation::ResolvedRenderOptions;
#[cfg(any(feature = "svg", feature = "ascii"))]
use crate::invocation::{ResolvedParseOptions, ResolvedRuntimeOptions};
#[cfg(feature = "svg")]
use merman::svg::{
    HeadlessRenderer, IconRegistry, LayoutOptions, MathRenderer, Presentation, PresentationProfile,
    RenderEnvironment, SvgRenderOptions, TextMeasurementPolicy,
};
#[cfg(feature = "svg")]
use std::sync::Arc;

pub(crate) fn engine_for(
    parse: &ParseCliArgs,
    resources: &ResolvedResourcePolicy,
) -> Result<Engine, CliError> {
    let runtime = ResolvedCliRuntimePolicy::from_cli(&parse.runtime)?;
    let site_config = site_config_for(parse, resources)?;
    Ok(engine_from_config(runtime, site_config))
}

#[cfg(feature = "ascii")]
pub(crate) fn engine_for_resolved(
    parse: &ResolvedParseOptions,
    resources: &ResolvedResourcePolicy,
) -> Result<Engine, CliError> {
    let runtime = ResolvedCliRuntimePolicy::from_resolved(&parse.runtime);
    let site_config = site_config_for_resolved(parse, resources)?;
    Ok(engine_from_config(runtime, site_config))
}

fn engine_from_config(runtime: ResolvedCliRuntimePolicy, site_config: MermaidConfig) -> Engine {
    runtime.apply_engine(Engine::new().with_site_config(site_config))
}

#[derive(Debug, Clone)]
struct ResolvedCliRuntimePolicy {
    runtime_policy: RuntimePolicy,
}

impl ResolvedCliRuntimePolicy {
    fn from_cli(args: &RuntimeCliArgs) -> Result<Self, CliError> {
        let mut runtime_policy = match args.policy {
            RuntimePolicyKind::Deterministic => RuntimePolicy::deterministic(),
            #[cfg(all(
                feature = "system-clock",
                feature = "system-timezone",
                feature = "system-random"
            ))]
            RuntimePolicyKind::Native => RuntimePolicy::try_native().map_err(|err| {
                CliError::InvalidInput(format!("--runtime native is unavailable: {err}"))
            })?,
        };
        #[cfg(feature = "system-clock")]
        if args.system_clock {
            runtime_policy = runtime_policy.try_with_system_clock().map_err(|err| {
                CliError::InvalidInput(format!("--system-clock is unavailable: {err}"))
            })?;
        }
        #[cfg(feature = "system-timezone")]
        if args.system_timezone {
            runtime_policy = runtime_policy.try_with_system_time_zone().map_err(|err| {
                CliError::InvalidInput(format!("--system-timezone is unavailable: {err}"))
            })?;
        }
        #[cfg(feature = "system-random")]
        if args.system_random {
            runtime_policy = runtime_policy.try_with_system_random().map_err(|err| {
                CliError::InvalidInput(format!("--system-random is unavailable: {err}"))
            })?;
        }
        #[cfg(feature = "system-timing")]
        if args.system_timing {
            runtime_policy = runtime_policy.try_with_system_timing().map_err(|err| {
                CliError::InvalidInput(format!("--system-timing is unavailable: {err}"))
            })?;
        }
        if let Some(offset_minutes) = args.fixed_local_offset_minutes {
            runtime_policy = runtime_policy
                .try_with_fixed_local_offset_minutes(offset_minutes)
                .map_err(|err| CliError::InvalidInput(err.to_string()))?;
        }
        if let Some(today) = args.fixed_today {
            runtime_policy = runtime_policy
                .try_with_fixed_today_at_local_midnight(today)
                .map_err(|err| CliError::InvalidInput(err.to_string()))?;
        }
        Ok(Self { runtime_policy })
    }

    #[cfg(any(feature = "svg", feature = "ascii"))]
    fn from_resolved(args: &ResolvedRuntimeOptions) -> Self {
        Self {
            runtime_policy: args.runtime_policy.clone(),
        }
    }

    fn apply_engine(&self, engine: Engine) -> Engine {
        engine.with_runtime_policy(self.runtime_policy.clone())
    }

    #[cfg(feature = "svg")]
    fn apply_environment(&self, environment: RenderEnvironment) -> RenderEnvironment {
        environment.with_runtime_policy(self.runtime_policy.clone())
    }
}

pub(crate) fn resolve_runtime_policy(args: &RuntimeCliArgs) -> Result<RuntimePolicy, CliError> {
    Ok(ResolvedCliRuntimePolicy::from_cli(args)?.runtime_policy)
}

#[cfg(feature = "analysis")]
pub(crate) fn runtime_policy_for(parse: &ParseCliArgs) -> Result<RuntimePolicy, CliError> {
    Ok(ResolvedCliRuntimePolicy::from_cli(&parse.runtime)?.runtime_policy)
}

pub(crate) fn site_config_for(
    parse: &ParseCliArgs,
    resources: &ResolvedResourcePolicy,
) -> Result<MermaidConfig, CliError> {
    site_config_from_parts(
        parse.theme.as_deref(),
        parse.config_file.as_deref(),
        resources,
    )
}

#[cfg(any(feature = "svg", feature = "ascii"))]
pub(crate) fn site_config_for_resolved(
    parse: &ResolvedParseOptions,
    resources: &ResolvedResourcePolicy,
) -> Result<MermaidConfig, CliError> {
    site_config_from_parts(
        parse.theme.as_deref(),
        parse.config_file.as_deref(),
        resources,
    )
}

fn site_config_from_parts(
    theme: Option<&str>,
    config_file: Option<&Path>,
    resources: &ResolvedResourcePolicy,
) -> Result<MermaidConfig, CliError> {
    let mut cfg = MermaidConfig::empty_object();

    if let Some(theme) = theme.map(str::trim).filter(|theme| !theme.is_empty()) {
        cfg.set_value("theme", serde_json::json!(theme));
    }

    if let Some(path) = config_file {
        let text = read_named_text_file(
            path,
            "configuration file",
            InputLimit::new(
                crate::resources::CliResourceLimitId::MaxConfigBytes.as_str(),
                resources.files().config_bytes,
            ),
        )?;
        let value: Value = serde_json::from_str(&text).map_err(|error| {
            CliError::InvalidInput(format!(
                "JSON error while parsing configuration file {}: {error}",
                crate::error::safe_path(path)
            ))
        })?;
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
    parse_options_from_suppress_errors(parse.suppress_errors)
}

#[cfg(any(feature = "svg", feature = "ascii"))]
pub(crate) fn parse_options_for_resolved(parse: &ResolvedParseOptions) -> ParseOptions {
    parse_options_from_suppress_errors(parse.suppress_errors)
}

fn parse_options_from_suppress_errors(suppress_errors: bool) -> ParseOptions {
    ParseOptions { suppress_errors }
}

#[cfg(feature = "svg")]
pub(crate) fn renderer_for(
    parse: &ParseCliArgs,
    render: &RenderCliArgs,
    icon_registry: Option<Arc<IconRegistry>>,
    resources: &ResolvedResourcePolicy,
) -> Result<HeadlessRenderer, CliError> {
    let runtime = ResolvedCliRuntimePolicy::from_cli(&parse.runtime)?;
    let site_config = site_config_for(parse, resources)?;
    renderer_from_config(
        runtime,
        site_config,
        parse_options(parse),
        RendererInputs::from_cli(render),
        icon_registry,
        resources,
    )
}

#[cfg(feature = "svg")]
pub(crate) fn renderer_for_resolved(
    parse: &ResolvedParseOptions,
    render: &ResolvedRenderOptions,
    icon_registry: Option<Arc<IconRegistry>>,
    resources: &ResolvedResourcePolicy,
) -> Result<HeadlessRenderer, CliError> {
    let runtime = ResolvedCliRuntimePolicy::from_resolved(&parse.runtime);
    let site_config = site_config_for_resolved(parse, resources)?;
    renderer_from_config(
        runtime,
        site_config,
        parse_options_for_resolved(parse),
        RendererInputs::from_resolved(render),
        icon_registry,
        resources,
    )
}

#[cfg(feature = "svg")]
#[derive(Debug, Clone, Copy)]
struct RendererInputs<'a> {
    presentation_profile: Option<PresentationProfile>,
    text_measurer: TextMeasurerKind,
    math_renderer: Option<MathRendererKind>,
    container_width: Option<f64>,
    container_height: Option<f64>,
    svg_id: Option<&'a str>,
    hand_drawn_seed: Option<u64>,
}

#[cfg(feature = "svg")]
impl<'a> RendererInputs<'a> {
    fn from_cli(render: &'a RenderCliArgs) -> Self {
        Self {
            presentation_profile: render.presentation_profile,
            text_measurer: render.text_measurer.unwrap_or(TextMeasurerKind::Vendored),
            math_renderer: render.math_renderer,
            container_width: render.container_width,
            container_height: render.container_height,
            svg_id: render.svg_id.as_deref(),
            hand_drawn_seed: render.hand_drawn_seed,
        }
    }

    fn from_resolved(render: &'a ResolvedRenderOptions) -> Self {
        Self {
            presentation_profile: render.presentation_profile,
            text_measurer: render.text_measurer,
            math_renderer: render.math_renderer,
            container_width: render.container_width,
            container_height: render.container_height,
            svg_id: render.svg_id.as_deref(),
            hand_drawn_seed: render.hand_drawn_seed,
        }
    }
}

#[cfg(feature = "svg")]
fn renderer_from_config(
    runtime: ResolvedCliRuntimePolicy,
    mut site_config: MermaidConfig,
    parse_options: ParseOptions,
    render: RendererInputs<'_>,
    icon_registry: Option<Arc<IconRegistry>>,
    resources: &ResolvedResourcePolicy,
) -> Result<HeadlessRenderer, CliError> {
    let mut environment = RenderEnvironment::deterministic()
        .with_text_measurement_policy(text_measurement_policy(render.text_measurer))
        .with_resource_policy(resources.render_policy());
    if let Some(kind) = render.math_renderer {
        environment = match math_renderer(kind)? {
            Some(renderer) => environment.with_math_renderer(renderer),
            None => environment.without_math_renderer(),
        };
    }
    if let Some(registry) = icon_registry {
        environment = environment.with_icon_registry(registry);
    }
    environment = runtime.apply_environment(environment);

    let svg = SvgRenderOptions {
        diagram_id: render.svg_id.map(merman::svg::sanitize_svg_id),
        ..SvgRenderOptions::default()
    };

    if let Some(seed) = render.hand_drawn_seed {
        site_config.set_value("handDrawnSeed", serde_json::json!(seed));
    }

    let engine = runtime.apply_engine(Engine::new());
    let mut renderer = HeadlessRenderer::from_engine_and_environment(engine, environment)
        .with_parse_options(parse_options)
        .with_layout_options(LayoutOptions {
            container_width: render.container_width.unwrap_or(800.0),
            container_height: render.container_height.unwrap_or(600.0),
        })
        .with_svg_options(svg);
    if let Some(profile) = render.presentation_profile {
        renderer = renderer.with_presentation(Presentation::new().with_profile(profile));
    }
    Ok(renderer.with_site_config(site_config))
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
        #[cfg(feature = "math")]
        MathRendererKind::Ratex => Ok(Some(Arc::new(merman::svg::RatexMathRenderer))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "svg")]
    fn default_resources() -> ResolvedResourcePolicy {
        ResolvedResourcePolicy::for_profile(merman::resources::CLI_DEFAULT_RESOURCE_PROFILE)
    }

    #[cfg(feature = "svg")]
    #[test]
    fn none_math_renderer_disables_the_compiled_default() {
        let render = RenderCliArgs {
            math_renderer: Some(MathRendererKind::None),
            ..RenderCliArgs::default()
        };
        let renderer = renderer_for(
            &ParseCliArgs::default(),
            &render,
            None,
            &default_resources(),
        )
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

    #[cfg(all(feature = "svg", feature = "math"))]
    #[test]
    fn unspecified_math_renderer_uses_the_compiled_default() {
        let renderer = renderer_for(
            &ParseCliArgs::default(),
            &RenderCliArgs::default(),
            None,
            &default_resources(),
        )
        .expect("CLI renderer");
        let svg = renderer
            .render_svg_sync("flowchart TD\nA[\"$$x^2$$\"] --> B[Done]")
            .expect("the default CLI renderer should use compiled RaTeX support")
            .expect("successful rendering should return SVG output");

        assert!(
            svg.contains("<path"),
            "expected rendered math glyphs: {svg}"
        );
        assert!(!svg.contains("$$x^2$$"), "math delimiters must be replaced");
    }

    #[test]
    fn default_runtime_policy_is_deterministic_in_every_build() {
        let context = ResolvedCliRuntimePolicy::from_cli(&RuntimeCliArgs::default())
            .expect("deterministic CLI policy")
            .runtime_policy
            .begin_operation()
            .expect("deterministic operation context");

        assert_eq!(
            context.clock_source(),
            merman::runtime::RuntimeValueSource::Fixed
        );
        assert_eq!(
            context.random_source(),
            merman::runtime::RuntimeValueSource::Fixed
        );
        assert_eq!(context.local_time_zone().fixed_offset_minutes(), Some(0));
        assert!(context.timing().is_none());
    }

    #[cfg(all(
        feature = "system-clock",
        feature = "system-timezone",
        feature = "system-random"
    ))]
    #[test]
    fn explicit_native_runtime_uses_system_adapters_without_enabling_timing() {
        let args = RuntimeCliArgs {
            policy: RuntimePolicyKind::Native,
            fixed_local_offset_minutes: Some(480),
            ..Default::default()
        };
        let resolved =
            ResolvedCliRuntimePolicy::from_cli(&args).expect("explicit native CLI runtime policy");
        let context = resolved
            .runtime_policy
            .begin_operation()
            .expect("native operation context");

        assert_eq!(
            context.clock_source(),
            merman::runtime::RuntimeValueSource::System
        );
        assert_eq!(
            context.random_source(),
            merman::runtime::RuntimeValueSource::System
        );
        assert_eq!(context.local_time_zone().fixed_offset_minutes(), Some(480));
        assert!(context.timing().is_none());
    }

    #[cfg(feature = "system-timing")]
    #[test]
    fn system_timing_is_an_independent_explicit_runtime_choice() {
        let args = RuntimeCliArgs {
            system_timing: true,
            ..Default::default()
        };
        let context = ResolvedCliRuntimePolicy::from_cli(&args)
            .expect("compiled timing policy")
            .runtime_policy
            .begin_operation()
            .expect("timed operation context");

        assert_eq!(
            context.clock_source(),
            merman::runtime::RuntimeValueSource::Fixed
        );
        assert!(context.timing().is_some());
    }

    #[test]
    fn boundary_fixed_today_returns_invalid_input_instead_of_panicking() {
        let args = RuntimeCliArgs {
            fixed_today: Some(chrono::NaiveDate::MIN),
            fixed_local_offset_minutes: Some(1439),
            ..Default::default()
        };

        let error = ResolvedCliRuntimePolicy::from_cli(&args)
            .expect_err("boundary instant must be rejected");
        assert!(matches!(error, CliError::InvalidInput(_)));
        assert!(error.to_string().contains("local datetime"));
    }
}
