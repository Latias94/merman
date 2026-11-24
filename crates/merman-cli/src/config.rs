use crate::cli::{MAX_FIXED_UNIX_MILLIS, ParseCliArgs, RuntimeCliArgs, RuntimePolicyKind};
use crate::error::CliError;
use crate::input::InputLimit;
use crate::io::read_named_text_file;
#[cfg(any(feature = "svg", feature = "ascii"))]
use crate::io::read_named_text_file_controlled;
use crate::resources::ResolvedResourcePolicy;
use merman::SourcePresentationConfig;
use merman::model::runtime::RuntimePolicy;
use merman::{MermaidConfig, MermaidEngine, ParseOptions};
use serde_json::Value;
use std::path::Path;

#[cfg(feature = "svg")]
use crate::cli::MathRendererKind;
#[cfg(feature = "svg")]
use crate::invocation::ResolvedRenderOptions;
#[cfg(any(feature = "svg", feature = "ascii"))]
use crate::invocation::{ResolvedParseOptions, ResolvedRuntimeOptions};
#[cfg(feature = "svg")]
use merman::SvgEnvironment;
#[cfg(feature = "svg")]
use merman::svg::{LayoutOptions, SvgRenderOptions};
#[cfg(any(feature = "svg", feature = "ascii"))]
use merman::theme::ThemePrecedence;

#[cfg(feature = "svg")]
#[derive(Clone)]
pub(crate) struct ConfiguredRenderer {
    pub(crate) renderer: merman::Renderer,
    pub(crate) svg: merman::SvgRequest,
}

#[cfg(all(feature = "svg", any(feature = "icons", feature = "native-fonts")))]
impl ConfiguredRenderer {
    pub(crate) fn with_svg_environment(mut self, environment: SvgEnvironment) -> Self {
        self.svg.environment = environment;
        self
    }
}

pub(crate) fn engine_for(
    parse: &ParseCliArgs,
    resources: &ResolvedResourcePolicy,
) -> Result<MermaidEngine, CliError> {
    let runtime = ResolvedCliRuntimePolicy::from_cli(&parse.source.runtime)?;
    let site_config = site_config_for(parse, resources)?;
    Ok(engine_from_config(runtime, site_config))
}

fn engine_from_config(
    runtime: ResolvedCliRuntimePolicy,
    site_config: MermaidConfig,
) -> MermaidEngine {
    runtime.apply_engine(
        MermaidEngine::new().with_site_config(SourcePresentationConfig::from_config(site_config)),
    )
}

#[derive(Debug, Clone)]
struct ResolvedCliRuntimePolicy {
    runtime_policy: RuntimePolicy,
}

impl ResolvedCliRuntimePolicy {
    fn from_cli(args: &RuntimeCliArgs) -> Result<Self, CliError> {
        if args.fixed_today.is_some() && args.fixed_unix_millis.is_some() {
            return Err(CliError::InvalidInput(
                "--fixed-today cannot be combined with --fixed-unix-millis".to_string(),
            ));
        }
        if let Some(unix_millis) = args.fixed_unix_millis
            && !(-MAX_FIXED_UNIX_MILLIS..=MAX_FIXED_UNIX_MILLIS).contains(&unix_millis)
        {
            return Err(CliError::InvalidInput(format!(
                "--fixed-unix-millis must be between -{MAX_FIXED_UNIX_MILLIS} and {MAX_FIXED_UNIX_MILLIS}"
            )));
        }
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
        if let Some(unix_millis) = args.fixed_unix_millis {
            runtime_policy = runtime_policy
                .with_fixed_today(None)
                .with_fixed_unix_millis(unix_millis);
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

    fn apply_engine(&self, engine: MermaidEngine) -> MermaidEngine {
        engine.with_runtime_policy(self.runtime_policy.clone())
    }
}

pub(crate) fn resolve_runtime_policy(args: &RuntimeCliArgs) -> Result<RuntimePolicy, CliError> {
    Ok(ResolvedCliRuntimePolicy::from_cli(args)?.runtime_policy)
}

#[cfg(feature = "analysis")]
pub(crate) fn runtime_policy_for(parse: &ParseCliArgs) -> Result<RuntimePolicy, CliError> {
    Ok(ResolvedCliRuntimePolicy::from_cli(&parse.source.runtime)?.runtime_policy)
}

pub(crate) fn site_config_for(
    parse: &ParseCliArgs,
    resources: &ResolvedResourcePolicy,
) -> Result<MermaidConfig, CliError> {
    site_config_from_parts(
        parse.theme.as_deref(),
        parse.source.config_file.as_deref(),
        resources,
        |path, limit| read_named_text_file(path, "configuration file", limit),
    )
}

#[cfg(any(feature = "svg", feature = "ascii"))]
pub(crate) fn site_config_for_resolved(
    parse: &ResolvedParseOptions,
    resources: &ResolvedResourcePolicy,
    control: &merman::OperationControl,
) -> Result<MermaidConfig, CliError> {
    let fallback = parse
        .theme
        .as_deref()
        .filter(|_| parse.theme_precedence.is_none());
    let mut site_config = site_config_from_parts(
        fallback,
        parse.config_file.as_deref(),
        resources,
        |path, limit| read_named_text_file_controlled(path, "configuration file", limit, control),
    )?;
    if let (Some(theme), Some(precedence)) = (&parse.theme, parse.theme_precedence) {
        let value = Value::String(theme.clone());
        site_config.set_value("theme", value.clone());
        if let Some(root) = site_config.as_value_mut().as_object_mut() {
            for family in merman::model::family::FAMILY_CATALOG {
                if let Some(config) = family.config
                    && let Some(existing) = root
                        .get_mut(config.namespace)
                        .and_then(|section| section.get_mut("theme"))
                {
                    *existing = value.clone();
                }
            }
        }
        if precedence == ThemePrecedence::Host {
            lock_site_theme(&mut site_config);
        }
    }
    Ok(site_config)
}

#[cfg(any(feature = "svg", feature = "ascii"))]
fn lock_site_theme(site_config: &mut MermaidConfig) {
    let defaults = merman::default_site_config();
    let mut secure = site_config
        .as_value()
        .get("secure")
        .and_then(Value::as_array)
        .or_else(|| defaults.as_value().get("secure").and_then(Value::as_array))
        .cloned()
        .unwrap_or_default();
    if !secure.iter().any(|value| value.as_str() == Some("theme")) {
        secure.push(Value::String("theme".to_owned()));
        site_config.set_value("secure", Value::Array(secure));
    }
}

fn site_config_from_parts(
    theme: Option<&str>,
    config_file: Option<&Path>,
    resources: &ResolvedResourcePolicy,
    mut read_config: impl FnMut(&Path, InputLimit) -> Result<String, CliError>,
) -> Result<MermaidConfig, CliError> {
    let mut cfg = MermaidConfig::empty_object();

    if let Some(theme) = theme.map(str::trim).filter(|theme| !theme.is_empty()) {
        cfg.set_value("theme", serde_json::json!(theme));
    }

    if let Some(path) = config_file {
        let limit = InputLimit::new(
            crate::resources::CliResourceLimitId::MaxConfigBytes.as_str(),
            resources.files().config_bytes,
        );
        let text = read_config(path, limit)?;
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
    parse_options_from_suppress_errors(parse.source.suppress_errors)
}

#[cfg(any(feature = "svg", feature = "ascii"))]
pub(crate) fn parse_options_for_resolved(parse: &ResolvedParseOptions) -> ParseOptions {
    parse_options_from_suppress_errors(parse.suppress_errors)
}

fn parse_options_from_suppress_errors(suppress_errors: bool) -> ParseOptions {
    ParseOptions { suppress_errors }
}

#[cfg(feature = "svg")]
pub(crate) fn renderer_for_resolved(
    parse: &ResolvedParseOptions,
    render: &ResolvedRenderOptions,
    resources: &ResolvedResourcePolicy,
    control: &merman::OperationControl,
) -> Result<ConfiguredRenderer, CliError> {
    let site_config = site_config_for_resolved(parse, resources, control)?;
    Ok(renderer_from_config(parse, site_config, render, resources))
}

#[cfg(feature = "rustdoc")]
pub(crate) fn rustdoc_renderer_for_resolved(
    parse: &ResolvedParseOptions,
    render: &ResolvedRenderOptions,
    resources: &ResolvedResourcePolicy,
    control: &merman::OperationControl,
) -> Result<ConfiguredRenderer, CliError> {
    let mut site_config = site_config_for_resolved(parse, resources, control)?;
    lock_site_theme(&mut site_config);
    Ok(renderer_from_config(parse, site_config, render, resources))
}

#[cfg(feature = "svg")]
fn renderer_from_config(
    parse: &ResolvedParseOptions,
    mut site_config: MermaidConfig,
    render: &ResolvedRenderOptions,
    resources: &ResolvedResourcePolicy,
) -> ConfiguredRenderer {
    let runtime = ResolvedCliRuntimePolicy::from_resolved(&parse.runtime);
    let mut environment = SvgEnvironment::deterministic()
        .with_resource_policy(resources.render_policy())
        .with_trusted_theme_css(true);
    #[cfg(not(feature = "layout-shoal"))]
    {
        use merman::svg::{RenderCapability, RenderCapabilityPolicy};
        environment = environment.with_capability_policy(
            RenderCapabilityPolicy::unrestricted().with_denied(RenderCapability::LayoutShoal),
        );
    }
    #[cfg(not(feature = "math"))]
    {
        environment = environment.without_math_renderer();
    }
    if let Some(kind) = render.math_renderer {
        environment = match kind {
            MathRendererKind::None => environment.without_math_renderer(),
            #[cfg(feature = "math")]
            MathRendererKind::Ratex => environment.with_compiled_math_renderer(),
        };
    }
    let svg = SvgRenderOptions {
        diagram_id: render.svg_id.as_deref().map(merman::svg::sanitize_svg_id),
        ..SvgRenderOptions::default()
    };

    if let Some(seed) = render.hand_drawn_seed {
        site_config.set_value("handDrawnSeed", serde_json::json!(seed));
    }

    let renderer = merman::Renderer::new()
        .with_presentation_engine(
            runtime.apply_engine(
                MermaidEngine::new()
                    .with_site_config(SourcePresentationConfig::from_config(site_config)),
            ),
        )
        .with_parse_options(parse_options_for_resolved(parse))
        .with_resource_policy(*resources.input_policy());
    let svg_request = merman::SvgRequest {
        environment,
        layout: LayoutOptions::default()
            .with_adaptation_if_supported(render.adaptation)
            .with_container_size(
                render.container_width.unwrap_or(800.0),
                render.container_height.unwrap_or(600.0),
            ),
        options: svg,
        debug: Default::default(),
        pipeline: None,
    };
    ConfiguredRenderer {
        renderer,
        svg: svg_request,
    }
}

#[cfg(feature = "ascii")]
pub(crate) fn ascii_renderer_for_resolved(
    parse: &ResolvedParseOptions,
    resources: &ResolvedResourcePolicy,
    control: &merman::OperationControl,
) -> Result<merman::Renderer, CliError> {
    let runtime = ResolvedCliRuntimePolicy::from_resolved(&parse.runtime);
    let site_config = site_config_for_resolved(parse, resources, control)?;
    Ok(merman::Renderer::new()
        .with_semantic_parser(
            merman::SemanticParser::new()
                .with_site_config(site_config)
                .with_runtime_policy(runtime.runtime_policy),
        )
        .with_parse_options(parse_options_for_resolved(parse))
        .with_resource_policy(*resources.input_policy()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "svg")]
    fn default_resources() -> ResolvedResourcePolicy {
        ResolvedResourcePolicy::for_profile(merman::model::resources::CLI_DEFAULT_RESOURCE_PROFILE)
    }

    #[cfg(feature = "svg")]
    fn default_resolved_parse() -> ResolvedParseOptions {
        ResolvedParseOptions {
            suppress_errors: false,
            config_file: None,
            theme: None,
            theme_precedence: None,
            runtime: ResolvedRuntimeOptions {
                runtime_policy: RuntimePolicy::deterministic(),
            },
        }
    }

    #[cfg(feature = "svg")]
    fn resolved_render(math_renderer: Option<MathRendererKind>) -> ResolvedRenderOptions {
        ResolvedRenderOptions {
            math_renderer,
            ..Default::default()
        }
    }

    #[cfg(feature = "svg")]
    #[test]
    fn none_math_renderer_disables_the_compiled_default() {
        let control = merman::OperationControl::new();
        let renderer = renderer_for_resolved(
            &default_resolved_parse(),
            &resolved_render(Some(MathRendererKind::None)),
            &default_resources(),
            &control,
        )
        .expect("CLI renderer");
        let error = renderer
            .renderer
            .render(merman::RenderRequest::new(
                "flowchart TD\nA[\"$$x^2$$\"] --> B[Done]",
                merman::RenderTarget::Svg(renderer.svg.clone()),
                merman::OperationControl::new(),
            ))
            .expect_err("explicitly disabling math must reject math labels");

        match error {
            merman::RenderError::Svg(merman::svg::RenderError::MissingCapability {
                capability,
                diagram_type: _,
            }) => assert_eq!(capability, merman::svg::RenderCapability::Math),
            other => panic!("expected a missing math capability error, got {other:?}"),
        }
    }

    #[cfg(all(feature = "svg", feature = "math"))]
    #[test]
    fn unspecified_math_renderer_uses_the_compiled_default() {
        let control = merman::OperationControl::new();
        let renderer = renderer_for_resolved(
            &default_resolved_parse(),
            &resolved_render(None),
            &default_resources(),
            &control,
        )
        .expect("CLI renderer");
        let output = renderer
            .renderer
            .render(merman::RenderRequest::new(
                "flowchart TD\nA[\"$$x^2$$\"] --> B[Done]",
                merman::RenderTarget::Svg(renderer.svg.clone()),
                merman::OperationControl::new(),
            ))
            .expect("the default CLI renderer should use compiled RaTeX support");
        let merman::RenderOutput::Svg(Some(svg)) = output else {
            panic!("successful rendering should return SVG output");
        };

        assert!(
            svg.svg().contains("<path"),
            "expected rendered math glyphs: {}",
            svg.svg()
        );
        assert!(
            !svg.svg().contains("$$x^2$$"),
            "math delimiters must be replaced"
        );
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
            merman::model::runtime::RuntimeValueSource::Fixed
        );
        assert_eq!(
            context.random_source(),
            merman::model::runtime::RuntimeValueSource::Fixed
        );
        assert_eq!(context.unix_millis(), 0);
        assert_eq!(context.local_time_zone().fixed_offset_minutes(), Some(0));
        assert!(context.timing().is_none());
    }

    #[test]
    fn fixed_unix_millis_preserves_the_instant_and_fixed_timezone() {
        for unix_millis in [-MAX_FIXED_UNIX_MILLIS, -1, 0, 375, MAX_FIXED_UNIX_MILLIS] {
            let args = RuntimeCliArgs {
                fixed_unix_millis: Some(unix_millis),
                fixed_local_offset_minutes: Some(-60),
                ..Default::default()
            };
            let resolved = ResolvedCliRuntimePolicy::from_cli(&args).unwrap();
            let first = resolved.runtime_policy.begin_operation().unwrap();
            let second = resolved.runtime_policy.begin_operation().unwrap();

            assert_eq!(first.unix_millis(), unix_millis);
            assert_eq!(first.local_time_zone().fixed_offset_minutes(), Some(-60));
            assert_eq!(first, second);
            assert_eq!(resolved.runtime_policy.fixed_today(), None);
            if unix_millis == 375 {
                assert_eq!(first.today_local().to_string(), "1969-12-31");
            }
        }
    }

    #[test]
    fn fixed_unix_millis_rejects_programmatic_conflicts_and_out_of_range_instants() {
        for unix_millis in [
            i64::MIN,
            -MAX_FIXED_UNIX_MILLIS - 1,
            MAX_FIXED_UNIX_MILLIS + 1,
            i64::MAX,
        ] {
            let args = RuntimeCliArgs {
                fixed_unix_millis: Some(unix_millis),
                ..Default::default()
            };
            let error = ResolvedCliRuntimePolicy::from_cli(&args).unwrap_err();

            assert!(matches!(error, CliError::InvalidInput(_)));
            assert!(error.to_string().contains("--fixed-unix-millis"));
        }

        let args = RuntimeCliArgs {
            fixed_today: Some("2026-01-15".parse().unwrap()),
            fixed_unix_millis: Some(0),
            ..Default::default()
        };
        let error = ResolvedCliRuntimePolicy::from_cli(&args).unwrap_err();

        assert!(matches!(error, CliError::InvalidInput(_)));
        assert!(error.to_string().contains("--fixed-today"));
        assert!(error.to_string().contains("--fixed-unix-millis"));
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
            merman::model::runtime::RuntimeValueSource::System
        );
        assert_eq!(
            context.random_source(),
            merman::model::runtime::RuntimeValueSource::System
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
            merman::model::runtime::RuntimeValueSource::Fixed
        );
        assert!(context.timing().is_some());
    }

    #[test]
    fn boundary_fixed_today_returns_invalid_input_instead_of_panicking() {
        let args = RuntimeCliArgs {
            fixed_today: Some(merman::model::time::CivilDate::new(i32::MIN, 1, 1).unwrap()),
            fixed_local_offset_minutes: Some(1439),
            ..Default::default()
        };

        let error = ResolvedCliRuntimePolicy::from_cli(&args)
            .expect_err("boundary instant must be rejected");
        assert!(matches!(error, CliError::InvalidInput(_)));
        assert!(error.to_string().contains("local datetime"));
    }
}
