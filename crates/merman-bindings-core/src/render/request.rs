use crate::common::{
    BindingError, BindingOptions, BindingStatus, HostThemeOptionsJson,
    binding_fixed_local_offset_minutes, binding_fixed_today, binding_site_config,
    css_declaration_value, finite_positive, internal_json_error, no_diagram_error,
    normalize_option,
};
use chrono::TimeZone;
use merman::render::{
    DeterministicTextMeasurer, FlowchartElkBackend, HeadlessRenderer, HostThemeAppearance,
    HostThemePipelinePreset, HostThemePreset, HostThemeProfile, HostThemeRoles,
    HostThemeRootBackground, LayoutOptions, VendoredFontMetricsTextMeasurer,
};
use std::sync::Arc;

#[derive(Clone)]
pub(super) struct RenderRequestPlan {
    renderer: HeadlessRenderer,
    pipeline: merman::render::SvgPipeline,
}

impl RenderRequestPlan {
    pub(super) fn from_options(options: &BindingOptions) -> Result<Self, BindingError> {
        let (renderer, pipeline) = build_renderer(options)?;
        Ok(Self {
            renderer,
            pipeline: pipeline.into_pipeline(),
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

    pub(super) fn with_text_measurer(
        &self,
        measurer: Arc<dyn merman::render::TextMeasurer + Send + Sync>,
    ) -> Self {
        Self {
            renderer: self.renderer.clone().with_text_measurer(measurer),
            pipeline: self.pipeline.clone(),
        }
    }

    pub(super) fn parse_json(&self, source: &str) -> Result<Vec<u8>, BindingError> {
        self.renderer
            .layout
            .resource_limits
            .check_source_bytes(source)
            .map_err(|err| {
                BindingError::new(BindingStatus::ResourceLimitExceeded, err.to_string())
            })?;
        let parsed = self
            .renderer
            .parse_diagram_sync(source)
            .map_err(classify_render_error)?
            .ok_or_else(no_diagram_error)?;

        serde_json::to_vec(&parsed.model).map_err(internal_json_error)
    }

    pub(super) fn layout_json(&self, source: &str) -> Result<Vec<u8>, BindingError> {
        let layouted = self
            .renderer
            .layout_diagram_sync(source)
            .map_err(classify_render_error)?
            .ok_or_else(no_diagram_error)?;

        serde_json::to_vec(&layouted).map_err(internal_json_error)
    }
}

#[cfg(test)]
pub(super) fn pipeline_for_options(
    options: &BindingOptions,
) -> Result<merman::render::SvgPipeline, BindingError> {
    Ok(build_renderer(options)?.1.into_pipeline())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum PipelineKind {
    #[default]
    Parity,
    Readable,
    ResvgSafe,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SvgPipelineOptions {
    kind: PipelineKind,
    scoped_css: Option<String>,
    css_override_policy: merman::render::CssOverridePolicy,
    strip_existing_important: bool,
    root_background_color: Option<String>,
    drop_native_duplicate_fallbacks: bool,
}

impl Default for SvgPipelineOptions {
    fn default() -> Self {
        Self {
            kind: PipelineKind::default(),
            scoped_css: None,
            css_override_policy: merman::render::CssOverridePolicy::Preserve,
            strip_existing_important: false,
            root_background_color: None,
            drop_native_duplicate_fallbacks: false,
        }
    }
}

impl SvgPipelineOptions {
    fn apply_compiled_host_output(&mut self, output: merman::render::CompiledHostThemeOutput) {
        self.kind = match output.preset {
            merman::render::SvgPipelinePreset::Parity => PipelineKind::Parity,
            merman::render::SvgPipelinePreset::Readable => PipelineKind::Readable,
            merman::render::SvgPipelinePreset::ResvgSafe => PipelineKind::ResvgSafe,
        };
        self.css_override_policy = output.css_override_policy;
        self.strip_existing_important = matches!(
            output.css_override_policy,
            merman::render::CssOverridePolicy::StripExistingImportant
        );
        if output.root_background_color.is_some() {
            self.root_background_color = output.root_background_color;
        }
        self.drop_native_duplicate_fallbacks = output.drop_native_duplicate_fallbacks;
        if output.scoped_css.is_some() {
            self.scoped_css = output.scoped_css;
        }
    }

    fn into_pipeline(self) -> merman::render::SvgPipeline {
        let mut pipeline = match self.kind {
            PipelineKind::Parity => merman::render::SvgPipeline::parity(),
            PipelineKind::Readable => merman::render::SvgPipeline::readable(),
            PipelineKind::ResvgSafe => merman::render::SvgPipeline::resvg_safe(),
        };

        if self.strip_existing_important {
            pipeline.push_postprocessor(
                merman::render::CssOverridePostprocessor::strip_existing_important(),
            );
        }

        if self.drop_native_duplicate_fallbacks {
            pipeline.push_postprocessor(merman::render::DropNativeDuplicateFallbacksPostprocessor);
        }

        if let Some(root_background_color) = self.root_background_color {
            pipeline.push_postprocessor(merman::render::RootBackgroundPostprocessor::new(
                root_background_color,
            ));
        }

        if let Some(scoped_css) = self.scoped_css.filter(|css| !css.trim().is_empty()) {
            pipeline.push_postprocessor(
                merman::render::ScopedCssPostprocessor::new(scoped_css)
                    .with_override_policy(self.css_override_policy),
            );
            if matches!(self.kind, PipelineKind::ResvgSafe) {
                pipeline.push_postprocessor(merman::render::SanitizeCssPostprocessor);
            }
        }

        pipeline
    }
}

fn build_renderer(
    options: &BindingOptions,
) -> Result<(HeadlessRenderer, SvgPipelineOptions), BindingError> {
    let fixed_today = binding_fixed_today(options)?;
    let fixed_local_offset_minutes = binding_fixed_local_offset_minutes(options)?;
    let mut renderer = HeadlessRenderer::new()
        .with_fixed_today(fixed_today)
        .with_fixed_local_offset_minutes(fixed_local_offset_minutes);
    if let Some(now_ms) = fixed_today_marker_ms(fixed_today, fixed_local_offset_minutes) {
        renderer.svg.now_ms_override = Some(now_ms);
    }

    if options
        .analysis
        .parse
        .as_ref()
        .and_then(|parse| parse.suppress_errors)
        .unwrap_or(false)
    {
        renderer = renderer.with_lenient_parsing();
    } else {
        renderer = renderer.with_strict_parsing();
    }

    let mut pipeline = SvgPipelineOptions::default();
    if let Some(host_theme) = options.host_theme.as_ref() {
        let compiled = binding_host_theme(host_theme)?;
        pipeline.apply_compiled_host_output(compiled.output);
        renderer = renderer.with_site_config(compiled.site_config);
    }

    if let Some(site_config) = binding_site_config(options)? {
        renderer = renderer.with_site_config(site_config);
    }

    let mut layout = LayoutOptions::headless_svg_defaults();
    if let Some(resources) = options.analysis.resources.as_ref() {
        layout.resource_limits = binding_resource_limits(resources)?;
    }
    if let Some(layout_json) = options.layout.as_ref() {
        if let Some(width) = layout_json.viewport_width {
            layout.viewport_width = finite_positive(width, "layout.viewport_width")?;
        }
        if let Some(height) = layout_json.viewport_height {
            layout.viewport_height = finite_positive(height, "layout.viewport_height")?;
        }
        if let Some(kind) = layout_json.text_measurer.as_deref() {
            match normalize_option(kind).as_str() {
                "vendored" => {
                    layout.text_measurer = Arc::new(VendoredFontMetricsTextMeasurer::default());
                }
                "deterministic" => {
                    layout.text_measurer = Arc::new(DeterministicTextMeasurer::default());
                }
                other => {
                    return Err(BindingError::new(
                        BindingStatus::InvalidArgument,
                        format!("unsupported layout.text_measurer: {other}"),
                    ));
                }
            }
        }
        if let Some(backend) = layout_json.flowchart_elk_backend.as_deref() {
            layout.flowchart_elk_backend = match normalize_option(backend).as_str() {
                "source-ported" | "source_ported" | "source" => FlowchartElkBackend::SourcePorted,
                "compat" => FlowchartElkBackend::Compat,
                other => {
                    return Err(BindingError::new(
                        BindingStatus::InvalidArgument,
                        format!("unsupported layout.flowchart_elk_backend: {other}"),
                    ));
                }
            };
        }
    }
    renderer = renderer.with_layout_options(layout);

    if let Some(math_renderer) = options
        .layout
        .as_ref()
        .and_then(|layout| layout.math_renderer.as_deref())
    {
        match normalize_option(math_renderer).as_str() {
            "none" => {}
            "ratex" => {
                #[cfg(feature = "ratex-math")]
                {
                    renderer = renderer
                        .with_math_renderer(Arc::new(merman_render::math::RatexMathRenderer));
                }
                #[cfg(not(feature = "ratex-math"))]
                {
                    return Err(BindingError::new(
                        BindingStatus::UnsupportedFormat,
                        "layout.math_renderer=ratex requires the ratex-math feature",
                    ));
                }
            }
            other => {
                return Err(BindingError::new(
                    BindingStatus::InvalidArgument,
                    format!("unsupported layout.math_renderer: {other}"),
                ));
            }
        }
    }

    if let Some(svg) = options.svg.as_ref() {
        if let Some(diagram_id) = svg.diagram_id.as_deref() {
            renderer = renderer.with_diagram_id(diagram_id);
        }
        if let Some(raw_pipeline) = svg.pipeline.as_deref() {
            pipeline.kind = match normalize_option(raw_pipeline).as_str() {
                "parity" => PipelineKind::Parity,
                "readable" => PipelineKind::Readable,
                "resvg-safe" | "resvg_safe" => PipelineKind::ResvgSafe,
                other => {
                    return Err(BindingError::new(
                        BindingStatus::InvalidArgument,
                        format!("unsupported svg.pipeline: {other}"),
                    ));
                }
            };
        }
        if let Some(raw_policy) = svg.css_override_policy.as_deref() {
            pipeline.css_override_policy = match normalize_option(raw_policy).as_str() {
                "preserve" => {
                    pipeline.strip_existing_important = false;
                    merman::render::CssOverridePolicy::Preserve
                }
                "strip-existing-important" | "strip_existing_important" => {
                    pipeline.strip_existing_important = true;
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
            pipeline.scoped_css = Some(scoped_css.to_string());
        }
        if let Some(root_background_color) = svg.root_background_color.as_deref() {
            pipeline.root_background_color = Some(css_declaration_value(
                root_background_color,
                "svg.root_background_color",
            )?);
        }
        pipeline.drop_native_duplicate_fallbacks =
            svg.drop_native_duplicate_fallbacks.unwrap_or(false);
    }

    Ok((renderer, pipeline))
}

fn fixed_today_marker_ms(
    today: Option<chrono::NaiveDate>,
    offset_minutes: Option<i32>,
) -> Option<i64> {
    let today = today?;
    let offset_minutes = offset_minutes?;
    let offset = chrono::FixedOffset::east_opt(offset_minutes.checked_mul(60)?)?;
    let midnight = today.and_hms_opt(0, 0, 0)?;
    let dt = offset
        .from_local_datetime(&midnight)
        .single()
        .unwrap_or_else(|| {
            chrono::DateTime::<chrono::FixedOffset>::from_naive_utc_and_offset(midnight, offset)
        });
    Some(dt.timestamp_millis())
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
                "resvg-safe" | "resvg_safe" => HostThemePipelinePreset::ResvgSafe,
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
                "strip-existing-important" | "strip_existing_important" => {
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

fn binding_resource_limits(
    resources: &merman_analysis::ResourceOptionsJson,
) -> Result<merman::render::RenderResourceLimits, BindingError> {
    let mut limits = match resources.profile.as_deref() {
        None => merman::render::RenderResourceLimits::interactive(),
        Some(profile) => match normalize_option(profile).as_str() {
            "interactive" => merman::render::RenderResourceLimits::interactive(),
            "typst-package" | "typst_package" | "typst" => {
                merman::render::RenderResourceLimits::typst_package()
            }
            "trusted-native" | "trusted_native" | "trusted" => {
                merman::render::RenderResourceLimits::trusted_native()
            }
            "unbounded-for-trusted-input" | "unbounded_for_trusted_input" | "unbounded" => {
                merman::render::RenderResourceLimits::unbounded_for_trusted_input()
            }
            other => {
                return Err(BindingError::new(
                    BindingStatus::InvalidArgument,
                    format!("unsupported resources.profile: {other}"),
                ));
            }
        },
    };

    apply_usize_override(
        &mut limits.max_source_bytes,
        resources.max_source_bytes,
        "resources.max_source_bytes",
    )?;
    apply_usize_override(
        &mut limits.max_svg_bytes,
        resources.max_svg_bytes,
        "resources.max_svg_bytes",
    )?;
    apply_usize_override(
        &mut limits.max_flowchart_nodes,
        resources.max_flowchart_nodes,
        "resources.max_flowchart_nodes",
    )?;
    apply_usize_override(
        &mut limits.max_flowchart_edges,
        resources.max_flowchart_edges,
        "resources.max_flowchart_edges",
    )?;
    apply_usize_override(
        &mut limits.max_flowchart_subgraphs,
        resources.max_flowchart_subgraphs,
        "resources.max_flowchart_subgraphs",
    )?;
    apply_usize_override(
        &mut limits.max_class_nodes,
        resources.max_class_nodes,
        "resources.max_class_nodes",
    )?;
    apply_usize_override(
        &mut limits.max_class_edges,
        resources.max_class_edges,
        "resources.max_class_edges",
    )?;
    apply_usize_override(
        &mut limits.max_class_namespaces,
        resources.max_class_namespaces,
        "resources.max_class_namespaces",
    )?;
    apply_usize_override(
        &mut limits.max_label_bytes,
        resources.max_label_bytes,
        "resources.max_label_bytes",
    )?;

    Ok(limits)
}

fn apply_usize_override(
    target: &mut Option<usize>,
    value: Option<usize>,
    name: &'static str,
) -> Result<(), BindingError> {
    if let Some(value) = value {
        if value == 0 {
            return Err(BindingError::new(
                BindingStatus::InvalidArgument,
                format!("{name} must be a positive integer"),
            ));
        }
        *target = Some(value);
    }
    Ok(())
}

fn binding_host_theme_preset(value: &str) -> Result<HostThemePreset, BindingError> {
    match normalize_option(value).as_str() {
        "editor-light" | "editor_light" => Ok(HostThemePreset::EditorLight),
        "editor-dark" | "editor_dark" => Ok(HostThemePreset::EditorDark),
        "one-dark" | "one_dark" | "onedark" => Ok(HostThemePreset::OneDark),
        "gruvbox-light" | "gruvbox_light" => Ok(HostThemePreset::GruvboxLight),
        "gruvbox-dark" | "gruvbox_dark" => Ok(HostThemePreset::GruvboxDark),
        "ayu-light" | "ayu_light" => Ok(HostThemePreset::AyuLight),
        "ayu-dark" | "ayu_dark" => Ok(HostThemePreset::AyuDark),
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_today_marker_ms_uses_fixed_local_offset() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 6, 10).unwrap();

        assert_eq!(
            fixed_today_marker_ms(Some(today), Some(0)),
            Some(1_781_049_600_000)
        );
        assert_eq!(
            fixed_today_marker_ms(Some(today), Some(60)),
            Some(1_781_046_000_000)
        );
    }

    #[test]
    fn fixed_today_marker_ms_requires_explicit_offset() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 6, 10).unwrap();

        assert_eq!(fixed_today_marker_ms(Some(today), None), None);
    }
}
