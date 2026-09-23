use crate::cli::{ParseCliArgs, RuntimeCliArgs, RuntimePolicyKind};
use crate::error::CliError;
use crate::input::InputLimit;
#[cfg(any(feature = "svg", feature = "ascii"))]
use crate::io::read_named_text_file_controlled;
use crate::io::{read_named_bytes_file, read_named_text_file};
use crate::resources::ResolvedResourcePolicy;
use merman::runtime::RuntimePolicy;
use merman::{Engine, MermaidConfig, ParseOptions};
use serde_json::Value;
use std::path::Path;

#[cfg(feature = "svg")]
use crate::cli::{MathRendererKind, RenderCliArgs};
#[cfg(feature = "svg")]
use crate::invocation::ResolvedRenderOptions;
#[cfg(any(feature = "svg", feature = "ascii"))]
use crate::invocation::{ResolvedParseOptions, ResolvedRuntimeOptions};
#[cfg(feature = "svg")]
use merman::SvgEnvironment;
#[cfg(feature = "svg")]
use merman::svg::{
    DiagramThemeCompiler, IconRegistry, LayoutOptions, MAX_THEME_ENCODED_BYTES_HARD_CAP,
    SvgRenderOptions, ThemeAdmissionPolicy, ThemePreset, ThemeResourcePolicy, TrustedThemeLane,
    TrustedThemeLanes,
};
#[cfg(feature = "svg")]
use merman_bindings_core::{compile_theme_definition_json_with, compile_theme_selection_json_with};

#[cfg(any(feature = "svg", feature = "ascii"))]
#[derive(Clone)]
pub(crate) struct ConfiguredRenderer {
    pub(crate) renderer: merman::Renderer,
    #[cfg(feature = "svg")]
    pub(crate) svg: merman::SvgRequest,
    #[cfg(feature = "svg")]
    theme: Option<merman::svg::DiagramTheme>,
}

#[cfg(any(feature = "svg", feature = "ascii"))]
impl ConfiguredRenderer {
    #[cfg(all(feature = "svg", feature = "icons"))]
    pub(crate) fn with_svg_environment(mut self, environment: SvgEnvironment) -> Self {
        self.svg.environment = environment;
        self
    }

    pub(crate) fn request<'a>(
        &self,
        source: &'a str,
        target: merman::RenderTarget,
        control: merman::OperationControl,
    ) -> merman::RenderRequest<'a> {
        let request = merman::RenderRequest::new(source, target, control);
        #[cfg(feature = "svg")]
        if let Some(theme) = self.theme.as_ref() {
            return request.with_theme(theme.clone());
        }
        request
    }
}

pub(crate) fn engine_for(
    parse: &ParseCliArgs,
    resources: &ResolvedResourcePolicy,
) -> Result<Engine, CliError> {
    let runtime = ResolvedCliRuntimePolicy::from_cli(&parse.runtime)?;
    let site_config = site_config_for(parse, resources)?;
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
        |path, limit| read_named_text_file(path, "configuration file", limit),
    )
}

#[cfg(any(feature = "svg", feature = "ascii"))]
pub(crate) fn site_config_for_resolved(
    parse: &ResolvedParseOptions,
    resources: &ResolvedResourcePolicy,
    control: &merman::OperationControl,
) -> Result<MermaidConfig, CliError> {
    site_config_from_parts(
        parse.theme.as_deref(),
        parse.config_file.as_deref(),
        resources,
        |path, limit| read_named_text_file_controlled(path, "configuration file", limit, control),
    )
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
    icon_registry: Option<IconRegistry>,
    resources: &ResolvedResourcePolicy,
) -> Result<ConfiguredRenderer, CliError> {
    let runtime = ResolvedCliRuntimePolicy::from_cli(&parse.runtime)?;
    let site_config = site_config_for(parse, resources)?;
    renderer_from_config(
        runtime,
        site_config,
        parse_options(parse),
        RendererInputs::from_cli(render)?,
        icon_registry,
        resources,
    )
}

#[cfg(feature = "svg")]
pub(crate) fn renderer_for_resolved(
    parse: &ResolvedParseOptions,
    render: &ResolvedRenderOptions,
    icon_registry: Option<IconRegistry>,
    resources: &ResolvedResourcePolicy,
    control: &merman::OperationControl,
) -> Result<ConfiguredRenderer, CliError> {
    let runtime = ResolvedCliRuntimePolicy::from_resolved(&parse.runtime);
    let site_config = site_config_for_resolved(parse, resources, control)?;
    renderer_from_config(
        runtime,
        site_config,
        parse_options_for_resolved(parse),
        RendererInputs::from_resolved(render)?,
        icon_registry,
        resources,
    )
}

#[cfg(feature = "rustdoc")]
pub(crate) fn rustdoc_renderer_for_resolved(
    parse: &ResolvedParseOptions,
    render: &ResolvedRenderOptions,
    resources: &ResolvedResourcePolicy,
    control: &merman::OperationControl,
) -> Result<ConfiguredRenderer, CliError> {
    let runtime = ResolvedCliRuntimePolicy::from_resolved(&parse.runtime);
    let mut site_config = site_config_for_resolved(parse, resources, control)?;
    let mut secure = merman::generated::default_site_config()
        .as_value()
        .get("secure")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| {
            CliError::Internal(
                "generated Mermaid site config is missing its secure-key policy".to_string(),
            )
        })?;
    if !secure.iter().any(|value| value.as_str() == Some("theme")) {
        secure.push(Value::String("theme".to_string()));
    }
    site_config.set_value("secure", Value::Array(secure));
    renderer_from_config(
        runtime,
        site_config,
        parse_options_for_resolved(parse),
        RendererInputs::from_resolved(render)?,
        None,
        resources,
    )
}

#[cfg(feature = "svg")]
#[derive(Debug, Clone, Copy)]
enum ThemeInput<'a> {
    Preset(ThemePreset),
    SelectionFile(&'a Path),
    DefinitionFile(&'a Path),
}

#[cfg(feature = "svg")]
impl<'a> ThemeInput<'a> {
    fn resolve(
        preset: Option<ThemePreset>,
        selection_file: Option<&'a Path>,
        definition_file: Option<&'a Path>,
    ) -> Result<Option<Self>, CliError> {
        match (preset, selection_file, definition_file) {
            (Some(_), Some(_), _) | (Some(_), _, Some(_)) | (_, Some(_), Some(_)) => {
                Err(CliError::InvalidInput(
                    "--theme-preset, --theme-file, and --theme-definition are mutually exclusive"
                        .to_string(),
                ))
            }
            (Some(preset), None, None) => Ok(Some(Self::Preset(preset))),
            (None, Some(path), None) => Ok(Some(Self::SelectionFile(path))),
            (None, None, Some(path)) => Ok(Some(Self::DefinitionFile(path))),
            (None, None, None) => Ok(None),
        }
    }

    fn compile(
        self,
        compiler: &DiagramThemeCompiler,
    ) -> Result<merman::svg::DiagramTheme, CliError> {
        if let Self::Preset(preset) = self {
            return compiler
                .compile_preset(preset)
                .map_err(|error| CliError::InvalidInput(format!("invalid theme preset: {error}")));
        }

        let max_bytes = compiler
            .resource_policy()
            .value(merman::svg::ThemeResourceLimitId::MaxThemeEncodedBytes)
            .unwrap_or(MAX_THEME_ENCODED_BYTES_HARD_CAP);
        let (path, input_name) = match self {
            Self::SelectionFile(path) => (path, "theme selection file"),
            Self::DefinitionFile(path) => (path, "theme definition file"),
            Self::Preset(_) => unreachable!("preset returned before file acquisition"),
        };
        let bytes = read_named_bytes_file(
            path,
            input_name,
            InputLimit::new("max_theme_encoded_bytes", Some(max_bytes)),
        )?;
        match self {
            Self::SelectionFile(_) => {
                compile_theme_selection_json_with(compiler, &bytes).map_err(|error| {
                    CliError::InvalidInput(format!(
                        "invalid theme selection file: {}",
                        error.message()
                    ))
                })
            }
            Self::DefinitionFile(_) => compile_theme_definition_json_with(compiler, &bytes)
                .map_err(|error| {
                    CliError::InvalidInput(format!(
                        "invalid theme definition file: {}",
                        error.message()
                    ))
                }),
            Self::Preset(_) => unreachable!("preset returned before file compilation"),
        }
    }
}

#[cfg(feature = "svg")]
#[derive(Debug, Clone, Copy)]
struct RendererInputs<'a> {
    theme: Option<ThemeInput<'a>>,
    math_renderer: Option<MathRendererKind>,
    container_width: Option<f64>,
    container_height: Option<f64>,
    svg_id: Option<&'a str>,
    hand_drawn_seed: Option<u64>,
}

#[cfg(feature = "svg")]
impl<'a> RendererInputs<'a> {
    fn from_cli(render: &'a RenderCliArgs) -> Result<Self, CliError> {
        Ok(Self {
            theme: ThemeInput::resolve(
                render.theme_preset,
                render.theme_file.as_deref(),
                render.theme_definition.as_deref(),
            )?,
            math_renderer: render.math_renderer,
            container_width: render.container_width,
            container_height: render.container_height,
            svg_id: render.svg_id.as_deref(),
            hand_drawn_seed: render.hand_drawn_seed,
        })
    }

    fn from_resolved(render: &'a ResolvedRenderOptions) -> Result<Self, CliError> {
        Ok(Self {
            theme: ThemeInput::resolve(
                render.theme_preset,
                render.theme_file.as_deref(),
                render.theme_definition.as_deref(),
            )?,
            math_renderer: render.math_renderer,
            container_width: render.container_width,
            container_height: render.container_height,
            svg_id: render.svg_id.as_deref(),
            hand_drawn_seed: render.hand_drawn_seed,
        })
    }
}

#[cfg(feature = "svg")]
fn renderer_from_config(
    runtime: ResolvedCliRuntimePolicy,
    mut site_config: MermaidConfig,
    parse_options: ParseOptions,
    render: RendererInputs<'_>,
    icon_registry: Option<IconRegistry>,
    resources: &ResolvedResourcePolicy,
) -> Result<ConfiguredRenderer, CliError> {
    let theme_resources = ThemeResourcePolicy::for_profile(resources.profile());
    let mut environment = SvgEnvironment::deterministic()
        .with_resource_policy(resources.render_policy())
        .with_theme_resource_ceiling(theme_resources.clone())
        .with_theme_admission_policy(ThemeAdmissionPolicy::permissive().with_trusted_lanes(
            TrustedThemeLanes::from_allowed([TrustedThemeLane::RawThemeCss]),
        ));
    if let Some(kind) = render.math_renderer {
        environment = match kind {
            MathRendererKind::None => environment.without_math_renderer(),
            #[cfg(feature = "math")]
            MathRendererKind::Ratex => environment.with_compiled_math_renderer(),
        };
    }
    if let Some(registry) = icon_registry {
        environment = environment.with_icon_registry(registry);
    }
    let svg = SvgRenderOptions {
        diagram_id: render.svg_id.map(merman::svg::sanitize_svg_id),
        ..SvgRenderOptions::default()
    };

    if let Some(seed) = render.hand_drawn_seed {
        site_config.set_value("handDrawnSeed", serde_json::json!(seed));
    }

    let compiler = DiagramThemeCompiler::new().with_resource_policy(theme_resources);
    let selected_theme = render
        .theme
        .map(|input| input.compile(&compiler))
        .transpose()?;
    let renderer = merman::Renderer::new()
        .with_engine(runtime.apply_engine(Engine::new().with_site_config(site_config)))
        .with_parse_options(parse_options)
        .with_resource_policy(*resources.input_policy());
    let svg_request = merman::SvgRequest {
        environment,
        layout: LayoutOptions::default().with_container_size(
            render.container_width.unwrap_or(800.0),
            render.container_height.unwrap_or(600.0),
        ),
        options: svg,
        debug: Default::default(),
        pipeline: None,
    };
    Ok(ConfiguredRenderer {
        renderer,
        svg: svg_request,
        theme: selected_theme,
    })
}

#[cfg(feature = "ascii")]
pub(crate) fn ascii_renderer_for_resolved(
    parse: &ResolvedParseOptions,
    resources: &ResolvedResourcePolicy,
    control: &merman::OperationControl,
) -> Result<ConfiguredRenderer, CliError> {
    let runtime = ResolvedCliRuntimePolicy::from_resolved(&parse.runtime);
    let site_config = site_config_for_resolved(parse, resources, control)?;
    let renderer = merman::Renderer::new()
        .with_engine(engine_from_config(runtime, site_config))
        .with_parse_options(parse_options_for_resolved(parse))
        .with_resource_policy(*resources.input_policy());
    Ok(ConfiguredRenderer {
        renderer,
        #[cfg(feature = "svg")]
        svg: merman::SvgRequest::default(),
        #[cfg(feature = "svg")]
        theme: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "svg")]
    #[test]
    fn cli_rejects_embedded_font_resources() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("theme.json");
        std::fs::write(&path, r#"{"spec":{"assets":{"fonts":[{"id":"caller","format":"woff2","data_base64":"d09GMg=="}]}}}"#).unwrap();
        let render = RenderCliArgs {
            theme_file: Some(path),
            ..Default::default()
        };
        let error = renderer_for(
            &ParseCliArgs::default(),
            &render,
            None,
            &default_resources(),
        )
        .err()
        .expect("CLI must reject retired theme font resources");
        assert!(
            error
                .to_string()
                .contains("embedded theme font resources are not supported"),
            "{error}"
        );
    }

    #[cfg(feature = "svg")]
    fn default_resources() -> ResolvedResourcePolicy {
        ResolvedResourcePolicy::for_profile(merman::resources::CLI_DEFAULT_RESOURCE_PROFILE)
    }

    #[cfg(feature = "svg")]
    fn default_resolved_parse() -> ResolvedParseOptions {
        ResolvedParseOptions {
            suppress_errors: false,
            config_file: None,
            theme: None,
            runtime: ResolvedRuntimeOptions {
                runtime_policy: RuntimePolicy::deterministic(),
            },
        }
    }

    #[cfg(feature = "svg")]
    fn resolved_render(math_renderer: Option<MathRendererKind>) -> ResolvedRenderOptions {
        ResolvedRenderOptions {
            theme_preset: None,
            theme_file: None,
            theme_definition: None,
            math_renderer,
            container_width: None,
            container_height: None,
            svg_id: None,
            hand_drawn_seed: None,
        }
    }

    #[cfg(feature = "svg")]
    #[test]
    fn none_math_renderer_disables_the_compiled_default() {
        let control = merman::OperationControl::new();
        let renderer = renderer_for_resolved(
            &default_resolved_parse(),
            &resolved_render(Some(MathRendererKind::None)),
            None,
            &default_resources(),
            &control,
        )
        .expect("CLI renderer");
        let error = renderer
            .renderer
            .render(renderer.request(
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
            None,
            &default_resources(),
            &control,
        )
        .expect("CLI renderer");
        let output = renderer
            .renderer
            .render(renderer.request(
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

    #[cfg(feature = "svg")]
    #[test]
    fn selected_profile_remains_the_host_ceiling_when_a_theme_is_attached_later() {
        let resources =
            ResolvedResourcePolicy::for_profile(merman::resources::ResourceProfile::Constrained);
        let mut renderer = renderer_for(
            &ParseCliArgs::default(),
            &RenderCliArgs::default(),
            None,
            &resources,
        )
        .expect("CLI renderer without a selected theme");
        assert!(renderer.theme.is_none());

        let limit = merman::svg::ThemeResourceLimitId::MaxEffectGraphs;
        let maximum = ThemeResourcePolicy::constrained()
            .value(limit)
            .expect("constrained effect-graph ceiling");
        let mut effects = merman::svg::DiagramEffectSet::default();
        for ordinal in 0..=maximum {
            let graph = merman::svg::EffectGraph::new(
                format!("host-ceiling-{ordinal}"),
                [merman::svg::EffectPrimitive::GaussianBlur {
                    input: merman::svg::EffectInput::SourceGraphic,
                    std_deviation: 1.0,
                }],
            )
            .expect("valid effect graph");
            effects = effects.with_graph(graph).expect("unique effect graph");
        }
        renderer.theme = Some(
            DiagramThemeCompiler::new()
                .with_resource_policy(ThemeResourcePolicy::unbounded_for_trusted_input())
                .compile(merman::svg::DiagramThemeSpec::new().with_effects(effects))
                .expect("the wider compiler should accept the retained effect graphs"),
        );

        let error = renderer
            .renderer
            .render(renderer.request(
                "flowchart TD\nA --> B",
                merman::RenderTarget::Svg(renderer.svg.clone()),
                merman::OperationControl::new(),
            ))
            .expect_err("the selected CLI profile must remain the session host ceiling");
        let resource = match error {
            merman::RenderError::ResourceLimitExceeded(resource) => resource,
            other => panic!("expected the session host ceiling to reject the theme, got {other:?}"),
        };
        assert_eq!(resource.id, limit.as_str());
        assert_eq!(resource.actual, (maximum + 1) as u64);
        assert_eq!(resource.maximum, maximum as u64);
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
            fixed_today: Some(merman::time::CivilDate::new(i32::MIN, 1, 1).unwrap()),
            fixed_local_offset_minutes: Some(1439),
            ..Default::default()
        };

        let error = ResolvedCliRuntimePolicy::from_cli(&args)
            .expect_err("boundary instant must be rejected");
        assert!(matches!(error, CliError::InvalidInput(_)));
        assert!(error.to_string().contains("local datetime"));
    }
}
