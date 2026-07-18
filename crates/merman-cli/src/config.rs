use crate::cli::{MathRendererKind, ParseCliArgs, RenderCliArgs, TextMeasurerKind};
use crate::error::CliError;
use crate::io::read_named_text_file;
use merman::render::{
    FixedRenderClock, HeadlessRenderer, IconRegistry, LayoutOptions, MathRenderer,
    RenderEnvironment, RenderTimeSnapshot, SvgRenderOptions, TextMeasurementPolicy,
};
use merman::{Engine, MermaidConfig, ParseOptions, time::LocalTimeZone};
use serde_json::Value;
use std::sync::Arc;

pub(crate) fn engine_for(parse: &ParseCliArgs, render: &RenderCliArgs) -> Result<Engine, CliError> {
    let site_config = site_config_for(parse, render)?;
    let time = ResolvedCliTimePolicy::new(parse)?;
    Ok(time.apply_engine(Engine::new().with_site_config(site_config)))
}

#[derive(Debug, Clone)]
struct ResolvedCliTimePolicy {
    today: Option<chrono::NaiveDate>,
    time_zone: LocalTimeZone,
    snapshot: Option<RenderTimeSnapshot>,
    captures_environment: bool,
}

impl ResolvedCliTimePolicy {
    fn new(parse: &ParseCliArgs) -> Result<Self, CliError> {
        let time_zone = match parse.fixed_local_offset_minutes {
            Some(offset_minutes) => LocalTimeZone::fixed(offset_minutes)
                .map_err(|err| CliError::InvalidInput(err.to_string()))?,
            None => LocalTimeZone::system(),
        };
        let snapshot = parse
            .fixed_today
            .map(|today| RenderTimeSnapshot::from_local_midnight(today, &time_zone))
            .transpose()
            .map_err(|err| {
                CliError::InvalidInput(format!("--fixed-today cannot be resolved: {err}"))
            })?;
        Ok(Self {
            today: parse.fixed_today,
            time_zone,
            snapshot,
            captures_environment: parse.fixed_today.is_some()
                || parse.fixed_local_offset_minutes.is_some(),
        })
    }

    fn apply_engine(&self, engine: Engine) -> Engine {
        engine
            .with_fixed_today(self.today)
            .with_local_time_zone(self.time_zone.clone())
    }

    fn apply_environment(&self, mut environment: RenderEnvironment) -> RenderEnvironment {
        if let Some(snapshot) = self.snapshot {
            environment = environment.with_clock(Arc::new(FixedRenderClock::new(snapshot)));
        }
        if self.captures_environment {
            environment = environment.with_local_time_zone(self.time_zone.clone());
        }
        environment
    }
}

pub(crate) fn validate_time_policy(parse: &ParseCliArgs) -> Result<(), CliError> {
    ResolvedCliTimePolicy::new(parse).map(|_| ())
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
    let mut environment = RenderEnvironment::host()
        .with_text_measurement_policy(text_measurement_policy(render.text_measurer))
        .with_resource_limits(merman::render::RenderResourceLimits::for_profile(
            render.resource_profile.into(),
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

    Ok(HeadlessRenderer::new()
        .with_engine(
            time.apply_engine(Engine::new().with_site_config(site_config_for(parse, render)?)),
        )
        .with_parse_options(parse_options(parse))
        .with_environment(environment)
        .with_layout_options(layout_options(render))
        .with_svg_options(svg))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct AdvancingClock(AtomicUsize);

    impl merman::render::RenderClock for AdvancingClock {
        fn unix_millis_and_offset(&self) -> (i64, i32) {
            let tick = self.0.fetch_add(1, Ordering::Relaxed) as i64;
            (tick * 86_400_000, -60)
        }
    }

    #[test]
    fn fixed_offset_only_preserves_an_advancing_cli_clock() {
        let parse = ParseCliArgs {
            fixed_local_offset_minutes: Some(480),
            ..Default::default()
        };
        let environment =
            RenderEnvironment::parity().with_clock(Arc::new(AdvancingClock(AtomicUsize::new(0))));
        let environment = ResolvedCliTimePolicy::new(&parse)
            .expect("CLI time policy")
            .apply_environment(environment);

        let first = environment.begin_session().expect("first session");
        let second = environment.begin_session().expect("second session");

        assert_eq!(first.time().unix_ms(), 0);
        assert_eq!(second.time().unix_ms(), 86_400_000);
        assert_eq!(first.time().local_offset_minutes(), 480);
        assert_eq!(second.time().local_offset_minutes(), 480);
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
