use crate::common::{
    BindingError, BindingOptions, BindingStatus, HostThemeOptionsJson, binding_resource_policy,
    binding_runtime_policy_from, binding_site_config, css_declaration_value, finite_positive,
    internal_json_error, no_diagram_error, normalize_option,
};
use merman::render::{
    HeadlessRenderer, HostThemeAppearance, HostThemePipelinePreset, HostThemePreset,
    HostThemeProfile, HostThemeRoles, HostThemeRootBackground, LayoutOptions, MeasurementProfileId,
    RenderEnvironment, TextMeasurementPhase, TextMeasurementPolicy, TextMeasurementProfileIdentity,
};
use std::sync::Arc;

#[derive(Clone)]
pub(super) struct RenderRequestPlan {
    renderer: HeadlessRenderer,
    pipeline: merman::render::SvgPipeline,
}

impl RenderRequestPlan {
    pub(super) fn from_options_with_runtime_policy(
        options: &BindingOptions,
        runtime_policy: merman::runtime::RuntimePolicy,
    ) -> Result<Self, BindingError> {
        let (renderer, pipeline) = build_renderer(options, runtime_policy)?;
        Ok(Self {
            renderer,
            pipeline: pipeline.pipeline(),
        })
    }

    pub(super) fn render_svg(&self, source: &str) -> Result<Vec<u8>, BindingError> {
        let svg = self
            .renderer
            .render_svg_with_pipeline_sync(source, &self.pipeline)
            .map_err(classify_render_error)?;

        match svg {
            Some(svg) => Ok(svg.into_bytes()),
            None => Err(no_diagram_error()),
        }
    }

    pub(super) fn with_host_text_measurer(
        &self,
        measurer: Arc<dyn merman::render::HostTextMeasurer>,
    ) -> Self {
        let identity = TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("merman.binding-host").expect("static profile id"),
            concat!("merman-bindings-core@", env!("CARGO_PKG_VERSION")),
        )
        .expect("static profile identity");
        let policy =
            TextMeasurementPolicy::host_display(identity, measurer, TextMeasurementPhase::ALL);
        Self {
            renderer: self.renderer.clone().with_text_measurement_policy(policy),
            pipeline: self.pipeline.clone(),
        }
    }

    pub(super) fn parse_json(&self, source: &str) -> Result<Vec<u8>, BindingError> {
        let parsed = self
            .renderer
            .parse_diagram_sync(source)
            .map_err(classify_render_error)?
            .ok_or_else(no_diagram_error)?;

        serde_json::to_vec(&parsed.model).map_err(internal_json_error)
    }

    pub(super) fn layout_json(&self, source: &str) -> Result<Vec<u8>, BindingError> {
        let layout_json = self
            .renderer
            .layout_json_sync(source)
            .map_err(classify_render_error)?
            .ok_or_else(no_diagram_error)?;

        serde_json::to_vec(&layout_json).map_err(internal_json_error)
    }
}

#[cfg(test)]
pub(super) fn pipeline_for_options(
    options: &BindingOptions,
) -> Result<merman::render::SvgPipeline, BindingError> {
    Ok(
        build_renderer(options, merman::runtime::RuntimePolicy::deterministic())?
            .1
            .pipeline(),
    )
}

fn build_renderer(
    options: &BindingOptions,
    runtime_policy: merman::runtime::RuntimePolicy,
) -> Result<(HeadlessRenderer, merman::render::SvgOutputPolicy), BindingError> {
    let runtime_policy = binding_runtime_policy_from(options, runtime_policy)?;
    let mut environment = RenderEnvironment::deterministic().with_runtime_policy(runtime_policy);
    environment = environment.with_resource_policy(binding_resource_policy(
        options.analysis.resources.as_ref(),
    )?);
    if let Some(environment_json) = options.environment.as_ref() {
        if let Some(kind) = environment_json.text_measurement.as_deref() {
            environment =
                environment.with_text_measurement_policy(match normalize_option(kind).as_str() {
                    "vendored" | "parity" => TextMeasurementPolicy::parity(),
                    "deterministic" => TextMeasurementPolicy::deterministic(),
                    other => {
                        return Err(BindingError::new(
                            BindingStatus::InvalidArgument,
                            format!("unsupported environment.text_measurement: {other}"),
                        ));
                    }
                });
        }
        if let Some(math_renderer) = environment_json.math_renderer.as_deref() {
            match normalize_option(math_renderer).as_str() {
                "none" => {}
                "ratex" => {
                    #[cfg(feature = "ratex-math")]
                    {
                        environment = environment
                            .with_math_renderer(Arc::new(merman_render::math::RatexMathRenderer));
                    }
                    #[cfg(not(feature = "ratex-math"))]
                    {
                        return Err(BindingError::new(
                            BindingStatus::UnsupportedFormat,
                            "environment.math_renderer=ratex requires the ratex-math feature",
                        ));
                    }
                }
                other => {
                    return Err(BindingError::new(
                        BindingStatus::InvalidArgument,
                        format!("unsupported environment.math_renderer: {other}"),
                    ));
                }
            }
        }
    }
    let mut renderer = HeadlessRenderer::new().with_environment(environment);

    if options
        .parse
        .as_ref()
        .and_then(|parse| parse.suppress_errors)
        .unwrap_or(false)
    {
        renderer = renderer.with_lenient_parsing();
    } else {
        renderer = renderer.with_strict_parsing();
    }

    let mut output = merman::render::SvgOutputPolicy::default();
    if let Some(host_theme) = options.host_theme.as_ref() {
        let compiled = binding_host_theme(host_theme)?;
        output = compiled.output;
        renderer = renderer.with_site_config(compiled.site_config);
    }

    if let Some(site_config) = binding_site_config(options)? {
        renderer = renderer.with_site_config(site_config);
    }

    let mut layout = LayoutOptions::headless_svg_defaults();
    if let Some(layout_json) = options.layout.as_ref() {
        if let Some(width) = layout_json.container_width {
            layout.container_width = finite_positive(width, "layout.container_width")?;
        }
        if let Some(height) = layout_json.container_height {
            layout.container_height = finite_positive(height, "layout.container_height")?;
        }
    }
    renderer = renderer.with_layout_options(layout);

    if let Some(svg) = options.svg.as_ref() {
        if let Some(diagram_id) = svg.diagram_id.as_deref() {
            renderer = renderer.with_diagram_id(diagram_id);
        }
        if let Some(raw_pipeline) = svg.pipeline.as_deref() {
            output.preset = match normalize_option(raw_pipeline).as_str() {
                "parity" => merman::render::SvgPipelinePreset::Parity,
                "readable" => merman::render::SvgPipelinePreset::Readable,
                "resvg-safe" => merman::render::SvgPipelinePreset::ResvgSafe,
                other => {
                    return Err(BindingError::new(
                        BindingStatus::InvalidArgument,
                        format!("unsupported svg.pipeline: {other}"),
                    ));
                }
            };
        }
        if let Some(raw_policy) = svg.css_override_policy.as_deref() {
            output.css_override_policy = match normalize_option(raw_policy).as_str() {
                "preserve" => merman::render::CssOverridePolicy::Preserve,
                "strip-existing-important" => {
                    merman::render::CssOverridePolicy::StripExistingImportant
                }
                other => {
                    return Err(BindingError::new(
                        BindingStatus::InvalidArgument,
                        format!("unsupported svg.css_override_policy: {other}"),
                    ));
                }
            };
        }
        if let Some(scoped_css) = svg.scoped_css.as_deref() {
            output.scoped_css = Some(scoped_css.to_string());
        }
        if let Some(root_background_color) = svg.root_background_color.as_deref() {
            output.root_background_color = Some(css_declaration_value(
                root_background_color,
                "svg.root_background_color",
            )?);
        }
        if let Some(drop_native_duplicate_fallbacks) = svg.drop_native_duplicate_fallbacks {
            output.drop_native_duplicate_fallbacks = drop_native_duplicate_fallbacks;
        }
    }

    Ok((renderer, output))
}

fn binding_host_theme(
    host_theme: &HostThemeOptionsJson,
) -> Result<merman::render::CompiledHostTheme, BindingError> {
    let mut profile = if let Some(preset) = host_theme.preset.as_deref() {
        HostThemeProfile::from_preset(binding_host_theme_preset(preset)?)
    } else {
        HostThemeProfile::default()
    };

    if let Some(appearance) = host_theme.appearance.as_deref() {
        profile.appearance = match normalize_option(appearance).as_str() {
            "light" => HostThemeAppearance::Light,
            "dark" => HostThemeAppearance::Dark,
            other => {
                return Err(BindingError::new(
                    BindingStatus::InvalidArgument,
                    format!("unsupported host_theme.appearance: {other}"),
                ));
            }
        };
    }

    if let Some(font_family) = host_theme
        .font_family
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        profile.font_family = Some(font_family.to_string());
    }
    if let Some(font_size) = host_theme
        .font_size
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        profile.font_size = Some(css_declaration_value(font_size, "host_theme.font_size")?);
    }

    if let Some(roles) = host_theme.roles.as_ref() {
        apply_host_theme_roles(&mut profile.roles, roles)?;
    }

    if let Some(palette) = host_theme.series_palette.as_ref() {
        let mut parsed = Vec::with_capacity(palette.len());
        for (index, color) in palette.iter().enumerate() {
            parsed.push(
                css_declaration_value(color, "host_theme.series_palette").map_err(|err| {
                    BindingError::new(err.status(), format!("{} at index {index}", err.message()))
                })?,
            );
        }
        profile.series_palette = parsed;
    }

    if let Some(output) = host_theme.output.as_ref() {
        if let Some(pipeline) = output.pipeline.as_deref() {
            profile.output.pipeline = match normalize_option(pipeline).as_str() {
                "parity" => HostThemePipelinePreset::Parity,
                "readable" => HostThemePipelinePreset::Readable,
                "resvg-safe" => HostThemePipelinePreset::ResvgSafe,
                other => {
                    return Err(BindingError::new(
                        BindingStatus::InvalidArgument,
                        format!("unsupported host_theme.output.pipeline: {other}"),
                    ));
                }
            };
        }
        if let Some(policy) = output.css_override_policy.as_deref() {
            profile.output.css_override_policy = match normalize_option(policy).as_str() {
                "preserve" => merman::render::CssOverridePolicy::Preserve,
                "strip-existing-important" => {
                    merman::render::CssOverridePolicy::StripExistingImportant
                }
                other => {
                    return Err(BindingError::new(
                        BindingStatus::InvalidArgument,
                        format!("unsupported host_theme.output.css_override_policy: {other}"),
                    ));
                }
            };
        }
        if let Some(root_background) = output.root_background.as_deref() {
            profile.output.root_background = match normalize_option(root_background).as_str() {
                "none" => HostThemeRootBackground::None,
                "canvas" => HostThemeRootBackground::Canvas,
                _ => HostThemeRootBackground::Color(css_declaration_value(
                    root_background,
                    "host_theme.output.root_background",
                )?),
            };
        }
        if let Some(drop) = output.drop_native_duplicate_fallbacks {
            profile.output.drop_native_duplicate_fallbacks = drop;
        }
        if let Some(scoped_css) = output.scoped_css.as_deref() {
            profile.output.scoped_css = Some(scoped_css.to_string());
        }
    }

    if let Some(theme_variables) = host_theme.theme_variables.as_ref() {
        profile.theme_variables = theme_variables.clone();
    }
    if let Some(site_config) = host_theme.site_config.as_ref() {
        let Some(object) = site_config.as_object() else {
            return Err(BindingError::new(
                BindingStatus::InvalidArgument,
                "host_theme.site_config must be a JSON object",
            ));
        };
        profile.site_config = object.clone();
    }

    Ok(profile.compile())
}

fn binding_host_theme_preset(value: &str) -> Result<HostThemePreset, BindingError> {
    match normalize_option(value).as_str() {
        "editor-light" => Ok(HostThemePreset::EditorLight),
        "editor-dark" => Ok(HostThemePreset::EditorDark),
        "one-dark" => Ok(HostThemePreset::OneDark),
        "gruvbox-light" => Ok(HostThemePreset::GruvboxLight),
        "gruvbox-dark" => Ok(HostThemePreset::GruvboxDark),
        "ayu-light" => Ok(HostThemePreset::AyuLight),
        "ayu-dark" => Ok(HostThemePreset::AyuDark),
        other => Err(BindingError::new(
            BindingStatus::InvalidArgument,
            format!("unsupported host_theme.preset: {other}"),
        )),
    }
}

fn apply_host_theme_roles(
    target: &mut HostThemeRoles,
    roles: &crate::common::HostThemeRolesJson,
) -> Result<(), BindingError> {
    set_role(
        &mut target.canvas,
        roles.canvas.as_deref(),
        "host_theme.roles.canvas",
    )?;
    set_role(
        &mut target.surface,
        roles.surface.as_deref(),
        "host_theme.roles.surface",
    )?;
    set_role(
        &mut target.surface_alt,
        roles.surface_alt.as_deref(),
        "host_theme.roles.surface_alt",
    )?;
    set_role(
        &mut target.surface_muted,
        roles.surface_muted.as_deref(),
        "host_theme.roles.surface_muted",
    )?;
    set_role(
        &mut target.text,
        roles.text.as_deref(),
        "host_theme.roles.text",
    )?;
    set_role(
        &mut target.subtle_text,
        roles.subtle_text.as_deref(),
        "host_theme.roles.subtle_text",
    )?;
    set_role(
        &mut target.border,
        roles.border.as_deref(),
        "host_theme.roles.border",
    )?;
    set_role(
        &mut target.line,
        roles.line.as_deref(),
        "host_theme.roles.line",
    )?;
    set_role(
        &mut target.edge_label_background,
        roles.edge_label_background.as_deref(),
        "host_theme.roles.edge_label_background",
    )?;
    set_role(
        &mut target.cluster_background,
        roles.cluster_background.as_deref(),
        "host_theme.roles.cluster_background",
    )?;
    set_role(
        &mut target.cluster_border,
        roles.cluster_border.as_deref(),
        "host_theme.roles.cluster_border",
    )?;
    set_role(
        &mut target.note_background,
        roles.note_background.as_deref(),
        "host_theme.roles.note_background",
    )?;
    set_role(
        &mut target.note_border,
        roles.note_border.as_deref(),
        "host_theme.roles.note_border",
    )?;
    set_role(
        &mut target.note_text,
        roles.note_text.as_deref(),
        "host_theme.roles.note_text",
    )?;
    set_role(
        &mut target.actor_background,
        roles.actor_background.as_deref(),
        "host_theme.roles.actor_background",
    )?;
    set_role(
        &mut target.actor_border,
        roles.actor_border.as_deref(),
        "host_theme.roles.actor_border",
    )?;
    set_role(
        &mut target.actor_text,
        roles.actor_text.as_deref(),
        "host_theme.roles.actor_text",
    )?;
    set_role(
        &mut target.activation_background,
        roles.activation_background.as_deref(),
        "host_theme.roles.activation_background",
    )?;
    set_role(
        &mut target.activation_border,
        roles.activation_border.as_deref(),
        "host_theme.roles.activation_border",
    )?;
    set_role(
        &mut target.error,
        roles.error.as_deref(),
        "host_theme.roles.error",
    )?;
    set_role(
        &mut target.warning,
        roles.warning.as_deref(),
        "host_theme.roles.warning",
    )?;
    set_role(
        &mut target.success,
        roles.success.as_deref(),
        "host_theme.roles.success",
    )?;
    Ok(())
}

fn set_role(
    target: &mut Option<String>,
    value: Option<&str>,
    name: &str,
) -> Result<(), BindingError> {
    if value.is_some() {
        *target = css_role_value(value, name)?;
    }
    Ok(())
}

fn css_role_value(value: Option<&str>, name: &str) -> Result<Option<String>, BindingError> {
    value
        .map(|value| css_declaration_value(value, name))
        .transpose()
}

fn classify_render_error(err: merman::render::HeadlessError) -> BindingError {
    match err {
        merman::render::HeadlessError::Parse(err) => {
            BindingError::new(BindingStatus::ParseError, err.to_string())
        }
        merman::render::HeadlessError::Render(
            merman::render::RenderError::ResourceLimitExceeded(err),
        ) => BindingError::new(BindingStatus::ResourceLimitExceeded, err.to_string()),
        merman::render::HeadlessError::Render(err) => {
            BindingError::new(BindingStatus::RenderError, err.to_string())
        }
        merman::render::HeadlessError::RuntimePolicy(err) => {
            BindingError::new(BindingStatus::RenderError, err.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_local_midnight_uses_fixed_local_offset() {
        let utc_options = crate::common::parse_options(
            br#"{ "fixed_today": "2026-06-10", "fixed_local_offset_minutes": 0 }"#,
        )
        .expect("UTC options");
        assert_eq!(
            binding_runtime_policy_from(
                &utc_options,
                merman::runtime::RuntimePolicy::deterministic()
            )
            .expect("valid UTC midnight")
            .begin_operation()
            .expect("UTC operation")
            .unix_millis(),
            1_781_049_600_000
        );
        let east_one_options = crate::common::parse_options(
            br#"{ "fixed_today": "2026-06-10", "fixed_local_offset_minutes": 60 }"#,
        )
        .expect("fixed-offset options");
        assert_eq!(
            binding_runtime_policy_from(
                &east_one_options,
                merman::runtime::RuntimePolicy::deterministic(),
            )
            .expect("valid fixed-offset midnight")
            .begin_operation()
            .expect("fixed-offset operation")
            .unix_millis(),
            1_781_046_000_000
        );
    }

    #[test]
    fn boundary_fixed_today_returns_invalid_argument_instead_of_panicking() {
        let options = crate::common::parse_options(
            br#"{
                "fixed_today": "-262143-01-01",
                "fixed_local_offset_minutes": 1439
            }"#,
        )
        .expect("binding options JSON");
        let error =
            binding_runtime_policy_from(&options, merman::runtime::RuntimePolicy::deterministic())
                .expect_err("boundary instant must be rejected");

        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert!(error.message().contains("fixed_today"));
    }

    #[test]
    fn fixed_offset_only_preserves_the_selected_binding_clock() {
        let options = crate::common::parse_options(br#"{ "fixed_local_offset_minutes": 480 }"#)
            .expect("binding options");
        let policy = binding_runtime_policy_from(
            &options,
            merman::runtime::RuntimePolicy::deterministic().with_fixed_unix_millis(86_400_000),
        )
        .expect("binding time policy");
        let context = policy.begin_operation().expect("operation context");

        assert_eq!(context.unix_millis(), 86_400_000);
        assert_eq!(context.local_time_zone().fixed_offset_minutes(), Some(480));
    }

    #[test]
    fn fixed_today_freezes_the_binding_clock_across_sessions() {
        let options = crate::common::parse_options(
            br#"{
                "fixed_today": "2026-06-10",
                "fixed_local_offset_minutes": 60
            }"#,
        )
        .expect("binding options");
        let policy =
            binding_runtime_policy_from(&options, merman::runtime::RuntimePolicy::deterministic())
                .expect("binding time policy");
        let environment = RenderEnvironment::deterministic().with_runtime_policy(policy);

        let first = environment.begin_session().expect("first session");
        let second = environment.begin_session().expect("second session");

        assert_eq!(first.operation_context(), second.operation_context());
        assert_eq!(first.unix_millis(), 1_781_046_000_000);
        assert_eq!(first.local_time_zone().fixed_offset_minutes(), Some(60));
    }
}
