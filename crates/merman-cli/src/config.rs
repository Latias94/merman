use crate::cli::{MathRendererKind, ParseCliArgs, RenderCliArgs, TextMeasurerKind};
use crate::error::CliError;
use crate::io::read_named_text_file;
use merman::render::{
    HeadlessRenderer, IconRegistry, LayoutOptions, MathRenderer, RenderEnvironment,
    RenderTimeSnapshot, SvgRenderOptions, TextMeasurementPolicy,
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
        viewport_width: render.width.unwrap_or(800.0),
        viewport_height: render.height.unwrap_or(600.0),
        // Mermaid parity for some diagrams relies on manatee-backed layout engines.
        use_manatee_layout: true,
        flowchart_elk_backend: render.flowchart_elk_backend.into(),
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
    environment = apply_render_time_policy(environment, parse)?;

    let svg = SvgRenderOptions {
        diagram_id: render
            .svg_id
            .as_deref()
            .map(merman::render::sanitize_svg_id),
        ..SvgRenderOptions::default()
    };

    Ok(HeadlessRenderer::new()
        .with_engine(engine_for(parse, render)?)
        .with_parse_options(parse_options(parse))
        .with_environment(environment)
        .with_layout_options(layout_options(render))
        .with_svg_options(svg))
}

fn apply_render_time_policy(
    environment: RenderEnvironment,
    parse: &ParseCliArgs,
) -> Result<RenderEnvironment, CliError> {
    match (parse.fixed_today, parse.fixed_local_offset_minutes) {
        (Some(today), requested_offset) => {
            let offset_minutes = requested_offset
                .unwrap_or_else(|| chrono::Local::now().offset().local_minus_utc() / 60);
            let local_midnight = today.and_hms_opt(0, 0, 0).expect("valid midnight");
            let utc_midnight =
                local_midnight - chrono::TimeDelta::minutes(i64::from(offset_minutes));
            let unix_ms = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
                utc_midnight,
                chrono::Utc,
            )
            .timestamp_millis();
            let snapshot = RenderTimeSnapshot::from_unix_millis(unix_ms, offset_minutes)
                .map_err(|err| CliError::InvalidInput(err.to_string()))?;
            Ok(environment.with_time_snapshot(snapshot))
        }
        (None, Some(offset_minutes)) => environment
            .with_fixed_local_offset_minutes(offset_minutes)
            .map_err(|err| CliError::InvalidInput(err.to_string())),
        (None, None) => Ok(environment),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::FlowchartElkBackend;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct AdvancingClock(AtomicUsize);

    impl merman::render::RenderClock for AdvancingClock {
        fn unix_millis_and_offset(&self) -> (i64, i32) {
            let tick = self.0.fetch_add(1, Ordering::Relaxed) as i64;
            (tick * 86_400_000, -60)
        }
    }

    #[test]
    fn layout_options_default_to_source_ported_flowchart_elk_backend() {
        let layout = layout_options(&RenderCliArgs::default());

        assert_eq!(
            layout.flowchart_elk_backend,
            merman::render::FlowchartElkBackend::SourcePorted
        );
    }

    #[test]
    fn layout_options_preserve_explicit_compat_flowchart_elk_backend() {
        let render = RenderCliArgs {
            flowchart_elk_backend: FlowchartElkBackend::Compat,
            ..Default::default()
        };
        let layout = layout_options(&render);

        assert_eq!(
            layout.flowchart_elk_backend,
            merman::render::FlowchartElkBackend::Compat
        );
    }

    #[test]
    fn fixed_offset_only_preserves_an_advancing_cli_clock() {
        let parse = ParseCliArgs {
            fixed_local_offset_minutes: Some(480),
            ..Default::default()
        };
        let environment =
            RenderEnvironment::parity().with_clock(Arc::new(AdvancingClock(AtomicUsize::new(0))));
        let environment = apply_render_time_policy(environment, &parse).expect("CLI time policy");

        let first = environment.begin_session().expect("first session");
        let second = environment.begin_session().expect("second session");

        assert_eq!(first.time().unix_ms(), 0);
        assert_eq!(second.time().unix_ms(), 86_400_000);
        assert_eq!(first.time().local_offset_minutes(), 480);
        assert_eq!(second.time().local_offset_minutes(), 480);
    }
}
