#[cfg(test)]
use crate::common::binding_runtime_policy_from;
use crate::common::{
    BindingError, BindingOptions, BindingStatus, PresentationOptionsJson,
    PresentationThemeOptionsJson, binding_resource_policy, binding_site_config,
    css_declaration_value, finite_positive, internal_json_error, no_diagram_error,
    normalize_option, runtime_policy_error,
};
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use crate::common::{BindingExportResourceOptions, binding_export_resource_options};
use merman::svg::{
    HeadlessRenderer, HostTheme, HostThemeAppearance, HostThemePreset, LayoutOptions,
    MeasurementProfileId, Presentation, PresentationProfile, RenderCapability,
    RenderCapabilityPolicy, RenderEnvironment, TextMeasurementPhase, TextMeasurementPolicy,
    TextMeasurementProfileIdentity, ThemeRole,
};

#[derive(Clone)]
pub(super) struct RenderRequestPlan {
    renderer: HeadlessRenderer,
    pipeline: merman::svg::SvgPipeline,
    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    export_resource_profile: merman::resources::ResourceProfile,
    #[cfg(any(feature = "png", feature = "jpeg"))]
    raster_options: merman::svg::export::RasterOptions,
    #[cfg(feature = "pdf")]
    pdf_options: merman::svg::export::PdfOptions,
}

pub(super) struct RenderOperationConfig {
    environment: RenderEnvironment,
    lenient_parsing: bool,
    presentation: Option<Presentation>,
    site_config: Option<merman::MermaidConfig>,
    layout: LayoutOptions,
    svg: merman::svg::SvgRenderOptions,
    output: merman::svg::SvgOutputPolicy,
    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    export_resource_profile: merman::resources::ResourceProfile,
    #[cfg(any(feature = "png", feature = "jpeg"))]
    raster_options: merman::svg::export::RasterOptions,
    #[cfg(feature = "pdf")]
    pdf_options: merman::svg::export::PdfOptions,
}

impl RenderRequestPlan {
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

    pub(super) fn layout_json(&self, source: &str) -> Result<Vec<u8>, BindingError> {
        let layout_json = self
            .renderer
            .layout_json_sync(source)
            .map_err(classify_render_error)?
            .ok_or_else(no_diagram_error)?;

        serde_json::to_vec(&layout_json).map_err(internal_json_error)
    }

    pub(super) fn svg_plan_json(&self, source: &str) -> Result<Vec<u8>, BindingError> {
        let plan = self
            .renderer
            .plan_svg_sync(source)
            .map_err(classify_render_error)?
            .ok_or_else(no_diagram_error)?;

        crate::SvgPlanPayload::from_render_plan(&plan)?.to_json_bytes()
    }

    #[cfg(feature = "png")]
    pub(super) fn render_png_output(
        &self,
        source: &str,
    ) -> Result<crate::operation::BindingOperationOutput, BindingError> {
        let (data, plan) = self
            .renderer
            .render_png_with_plan_sync(source, &self.raster_options)
            .map_err(|error| classify_output_error(error, self.export_resource_profile))?
            .ok_or_else(no_diagram_error)?;
        Ok(crate::operation::BindingOperationOutput::raster(data, plan))
    }

    #[cfg(feature = "jpeg")]
    pub(super) fn render_jpeg_output(
        &self,
        source: &str,
    ) -> Result<crate::operation::BindingOperationOutput, BindingError> {
        let (data, plan) = self
            .renderer
            .render_jpeg_with_plan_sync(source, &self.raster_options)
            .map_err(|error| classify_output_error(error, self.export_resource_profile))?
            .ok_or_else(no_diagram_error)?;
        Ok(crate::operation::BindingOperationOutput::raster(data, plan))
    }

    #[cfg(feature = "pdf")]
    pub(super) fn render_pdf_output(
        &self,
        source: &str,
    ) -> Result<crate::operation::BindingOperationOutput, BindingError> {
        let (data, plan) = self
            .renderer
            .render_pdf_with_plan_sync(source, &self.pdf_options)
            .map_err(|error| classify_output_error(error, self.export_resource_profile))?
            .ok_or_else(no_diagram_error)?;
        Ok(crate::operation::BindingOperationOutput::pdf(data, plan))
    }
}

#[cfg(test)]
pub(super) fn pipeline_for_options(
    options: &BindingOptions,
) -> Result<merman::svg::SvgPipeline, BindingError> {
    Ok(
        compile_for_test(options, merman::runtime::RuntimePolicy::deterministic())?
            .output
            .pipeline(),
    )
}

#[cfg(test)]
fn compile_for_test(
    options: &BindingOptions,
    runtime_policy: merman::runtime::RuntimePolicy,
) -> Result<RenderOperationConfig, BindingError> {
    RenderOperationConfig::compile(
        options,
        runtime_policy,
        RenderCapabilityPolicy::unrestricted(),
    )
}

impl RenderOperationConfig {
    pub(super) fn compile(
        options: &BindingOptions,
        runtime_policy: merman::runtime::RuntimePolicy,
        capability_policy: RenderCapabilityPolicy,
    ) -> Result<Self, BindingError> {
        let mut environment = RenderEnvironment::deterministic()
            .with_capability_policy(capability_policy)
            .with_runtime_policy(runtime_policy);
        environment = environment.with_resource_policy(binding_resource_policy(
            options.analysis.resources.as_ref(),
        )?);
        if let Some(environment_json) = options.environment.as_ref() {
            if let Some(kind) = environment_json.text_measurement.as_deref() {
                environment = environment.with_text_measurement_policy(
                    match normalize_option(kind).as_str() {
                        "vendored" | "parity" => TextMeasurementPolicy::parity(),
                        "deterministic" => TextMeasurementPolicy::deterministic(),
                        other => {
                            return Err(BindingError::new(
                                BindingStatus::InvalidArgument,
                                format!("unsupported environment.text_measurement: {other}"),
                            ));
                        }
                    },
                );
            }
            if let Some(math_renderer) = environment_json.math_renderer.as_deref() {
                match normalize_option(math_renderer).as_str() {
                    "none" => {
                        environment = environment.without_math_renderer();
                    }
                    "ratex" => {
                        if !capability_policy.allows(RenderCapability::Math)
                            || !merman::svg::math_available()
                        {
                            return Err(BindingError::missing_capability(
                                "math",
                                "environment.math_renderer=ratex requires the artifact-owned math capability",
                            ));
                        }
                        environment = environment.with_compiled_math_renderer();
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

        let lenient_parsing = options
            .parse
            .as_ref()
            .and_then(|parse| parse.suppress_errors)
            .unwrap_or(false);
        let mut output = merman::svg::SvgOutputPolicy::default();
        let presentation = options
            .presentation
            .as_ref()
            .map(binding_presentation)
            .transpose()?;
        let site_config = binding_site_config(options)?;

        let mut layout = LayoutOptions::headless_svg_defaults();
        if let Some(layout_json) = options.layout.as_ref() {
            if let Some(width) = layout_json.container_width {
                layout.container_width = finite_positive(width, "layout.container_width")?;
            }
            if let Some(height) = layout_json.container_height {
                layout.container_height = finite_positive(height, "layout.container_height")?;
            }
            if let Some(width) = layout_json.screen_available_width {
                layout.screen_available_width =
                    Some(finite_positive(width, "layout.screen_available_width")?);
            }
        }

        let mut svg_options = merman::svg::SvgRenderOptions::default();
        if let Some(svg) = options.svg.as_ref() {
            svg_options.diagram_id.clone_from(&svg.diagram_id);
            if let Some(viewbox_padding) = svg.viewbox_padding {
                svg_options.viewbox_padding =
                    finite_nonnegative(viewbox_padding, "svg.viewbox_padding")?;
            }
            if let Some(raw_pipeline) = svg.pipeline.as_deref() {
                output.preset = match normalize_option(raw_pipeline).as_str() {
                    "parity" => merman::svg::SvgPipelinePreset::Parity,
                    "readable" => merman::svg::SvgPipelinePreset::Readable,
                    "resvg-safe" => merman::svg::SvgPipelinePreset::ResvgSafe,
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
                    "preserve" => merman::svg::CssOverridePolicy::Preserve,
                    "strip-existing-important" => {
                        merman::svg::CssOverridePolicy::StripExistingImportant
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

        #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
        let export_resources =
            binding_export_resource_options(options.analysis.resources.as_ref())?;
        #[cfg(any(feature = "png", feature = "jpeg"))]
        let raster_options = binding_raster_options(options, &export_resources)?;
        #[cfg(feature = "pdf")]
        let pdf_options = binding_pdf_options(options, &export_resources)?;

        Ok(Self {
            environment,
            lenient_parsing,
            presentation,
            site_config,
            layout,
            svg: svg_options,
            output,
            #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
            export_resource_profile: export_resources.profile,
            #[cfg(any(feature = "png", feature = "jpeg"))]
            raster_options,
            #[cfg(feature = "pdf")]
            pdf_options,
        })
    }

    pub(super) fn materialize(self, services: &crate::BindingEngineServices) -> RenderRequestPlan {
        let environment = if let Some(registry) = services.icon_registry() {
            self.environment.with_icon_registry(registry)
        } else {
            self.environment
        };
        let mut renderer = HeadlessRenderer::new().with_environment(environment);
        if let Some(measurer) = services.host_text_measurer() {
            let identity = TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("merman.binding-host").expect("static profile id"),
                concat!("merman-bindings-core@", env!("CARGO_PKG_VERSION")),
            )
            .expect("static profile identity");
            let policy =
                TextMeasurementPolicy::host_display(identity, measurer, TextMeasurementPhase::ALL);
            renderer = renderer.with_text_measurement_policy(policy);
        }
        renderer = if self.lenient_parsing {
            renderer.with_lenient_parsing()
        } else {
            renderer.with_strict_parsing()
        };
        if let Some(presentation) = self.presentation {
            renderer = renderer.with_presentation(presentation);
        }
        if let Some(site_config) = self.site_config {
            renderer = renderer.with_site_config(site_config);
        }
        renderer = renderer.with_layout_options(self.layout);
        renderer = renderer.with_svg_options(self.svg);
        RenderRequestPlan {
            renderer,
            pipeline: self.output.pipeline(),
            #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
            export_resource_profile: self.export_resource_profile,
            #[cfg(any(feature = "png", feature = "jpeg"))]
            raster_options: self.raster_options,
            #[cfg(feature = "pdf")]
            pdf_options: self.pdf_options,
        }
    }
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn binding_raster_options(
    options: &BindingOptions,
    resources: &BindingExportResourceOptions,
) -> Result<merman::svg::export::RasterOptions, BindingError> {
    let mut compiled = merman::svg::export::RasterOptions {
        size_limit: resources.raster_size_limit,
        embedded_image_limit: resources.embedded_image_limit,
        conversion_limits: resources.conversion_limits,
        ..Default::default()
    };
    if let Some(raster) = options.raster.as_ref() {
        if let Some(scale) = raster.scale {
            compiled.scale = finite_positive_f32(scale, "raster.scale")?;
        }
        if let Some(background) = raster.background.as_deref() {
            let background = css_declaration_value(background, "raster.background")?;
            if !merman::svg::export::is_valid_export_background_color(&background) {
                return Err(BindingError::new(
                    BindingStatus::InvalidArgument,
                    "raster.background is not supported by the native exporter",
                ));
            }
            compiled.background = Some(background);
        }
        if let Some(fit) = raster.fit_to.as_ref() {
            if fit.width.is_none() && fit.height.is_none() {
                return Err(BindingError::new(
                    BindingStatus::InvalidArgument,
                    "raster.fit_to must include width or height",
                ));
            }
            if fit.width == Some(0) || fit.height == Some(0) {
                return Err(BindingError::new(
                    BindingStatus::InvalidArgument,
                    "raster.fit_to width and height must be positive",
                ));
            }
            compiled.fit_to = Some(merman::svg::export::RasterFitBox::new(
                fit.width, fit.height,
            ));
        }
    }
    #[cfg(feature = "jpeg")]
    if let Some(quality) = options.jpeg.as_ref().and_then(|jpeg| jpeg.quality) {
        if !(1..=100).contains(&quality) {
            return Err(BindingError::new(
                BindingStatus::InvalidArgument,
                "jpeg.quality must be between 1 and 100",
            ));
        }
        compiled.jpeg_quality = quality;
    }
    Ok(compiled)
}

#[cfg(feature = "pdf")]
fn binding_pdf_options(
    options: &BindingOptions,
    resources: &BindingExportResourceOptions,
) -> Result<merman::svg::export::PdfOptions, BindingError> {
    let mut compiled = merman::svg::export::PdfOptions {
        embedded_image_limit: resources.embedded_image_limit,
        filter_image_limit: resources.pdf_filter_image_limit,
        conversion_limits: resources.conversion_limits,
        ..Default::default()
    };
    let Some(pdf) = options.pdf.as_ref() else {
        return Ok(compiled);
    };
    if let Some(page) = pdf.page_policy.as_ref() {
        compiled.page_policy = match page {
            crate::common::PdfPageOptionsJson::FitSvg => merman::svg::export::PdfPagePolicy::FitSvg,
            crate::common::PdfPageOptionsJson::Fixed {
                width_pt,
                height_pt,
            } => merman::svg::export::PdfPagePolicy::Fixed {
                width_pt: finite_positive_f32(*width_pt, "pdf.page_policy.width_pt")?,
                height_pt: finite_positive_f32(*height_pt, "pdf.page_policy.height_pt")?,
            },
            crate::common::PdfPageOptionsJson::FitCssWidth { max_width_px } => {
                merman::svg::export::PdfPagePolicy::FitCssWidth {
                    max_width_px: finite_positive_f32(
                        *max_width_px,
                        "pdf.page_policy.max_width_px",
                    )?,
                }
            }
        };
    }
    if let Some(filter_scale) = pdf.filter_scale {
        compiled.filter_scale = finite_positive_f32(filter_scale, "pdf.filter_scale")?;
    }
    if let Some(background) = pdf.background.as_deref() {
        let background = css_declaration_value(background, "pdf.background")?;
        if !merman::svg::export::is_valid_export_background_color(&background) {
            return Err(BindingError::new(
                BindingStatus::InvalidArgument,
                "pdf.background is not supported by the native exporter",
            ));
        }
        compiled.background = Some(background);
    }
    Ok(compiled)
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn finite_positive_f32(value: f64, name: &'static str) -> Result<f32, BindingError> {
    let value = finite_positive(value, name)?;
    let narrowed = value as f32;
    if narrowed.is_finite() {
        Ok(narrowed)
    } else {
        Err(BindingError::new(
            BindingStatus::InvalidArgument,
            format!("{name} exceeds the f32 export boundary"),
        ))
    }
}

fn finite_nonnegative(value: f64, name: &'static str) -> Result<f64, BindingError> {
    if value.is_finite() && value >= 0.0 {
        Ok(value)
    } else {
        Err(BindingError::new(
            BindingStatus::InvalidArgument,
            format!("{name} must be finite and non-negative"),
        ))
    }
}

fn binding_presentation(options: &PresentationOptionsJson) -> Result<Presentation, BindingError> {
    let mut presentation = Presentation::new();
    if let Some(profile) = options.profile.as_deref() {
        let profile = PresentationProfile::from_id(profile.trim()).map_err(|_| {
            BindingError::new(
                BindingStatus::InvalidArgument,
                format!("unsupported presentation.profile: {profile}"),
            )
        })?;
        presentation = presentation.with_profile(profile);
    }
    if let Some(theme) = options.theme.as_ref() {
        presentation = presentation.with_theme(binding_presentation_theme(theme)?);
    }
    Ok(presentation)
}

fn binding_presentation_theme(
    options: &PresentationThemeOptionsJson,
) -> Result<HostTheme, BindingError> {
    let mut theme = if let Some(preset) = options.preset.as_deref() {
        let preset = HostThemePreset::from_id(preset.trim()).map_err(|_| {
            BindingError::new(
                BindingStatus::InvalidArgument,
                format!("unsupported presentation.theme.preset: {preset}"),
            )
        })?;
        HostTheme::from_preset(preset)
    } else {
        HostTheme::new()
    };

    if let Some(appearance) = options.appearance.as_deref() {
        theme = theme.with_appearance(match appearance.trim() {
            "light" => HostThemeAppearance::Light,
            "dark" => HostThemeAppearance::Dark,
            other => {
                return Err(BindingError::new(
                    BindingStatus::InvalidArgument,
                    format!("unsupported presentation.theme.appearance: {other}"),
                ));
            }
        });
    }
    if let Some(font_family) = options.font_family.as_deref() {
        theme = theme
            .try_with_font_family(font_family)
            .map_err(binding_presentation_error)?;
    }
    if let Some(font_size) = options.font_size.as_deref() {
        theme = theme
            .try_with_font_size(font_size)
            .map_err(binding_presentation_error)?;
    }
    if let Some(roles) = options.roles.as_ref() {
        for (id, value) in roles {
            let role = ThemeRole::from_id(id).map_err(|_| {
                BindingError::new(
                    BindingStatus::InvalidArgument,
                    format!("unsupported presentation.theme.roles key: {id}"),
                )
            })?;
            theme = theme
                .try_with_role(role, value)
                .map_err(binding_presentation_error)?;
        }
    }
    if let Some(palette) = options.series_palette.as_ref() {
        theme = theme
            .try_with_series_palette(palette.iter().cloned())
            .map_err(binding_presentation_error)?;
    }
    Ok(theme)
}

fn binding_presentation_error(error: merman::svg::PresentationError) -> BindingError {
    BindingError::new(
        BindingStatus::InvalidArgument,
        format!("invalid presentation.{error}"),
    )
}

fn classify_render_error(err: merman::svg::HeadlessError) -> BindingError {
    match err {
        merman::svg::HeadlessError::Parse(err) => {
            BindingError::new(BindingStatus::ParseError, err.to_string())
        }
        merman::svg::HeadlessError::Render(merman::svg::RenderError::ResourceLimitExceeded(
            err,
        )) => BindingError::resource_limit(
            err.phase.as_str(),
            err.limit,
            err.actual as u64,
            err.max as u64,
            err.profile.id(),
            err.to_string(),
        ),
        merman::svg::HeadlessError::Render(
            err @ merman::svg::RenderError::MissingCapability { .. },
        ) => BindingError::missing_capability(
            err.missing_capability()
                .expect("matched missing render capability")
                .id(),
            err.to_string(),
        ),
        merman::svg::HeadlessError::Render(
            err @ merman::svg::RenderError::InvalidIconOutput { .. },
        ) => BindingError::new(BindingStatus::InvalidArgument, err.to_string()),
        merman::svg::HeadlessError::Render(
            err @ merman::svg::RenderError::IconProcessing { .. },
        ) => BindingError::internal(err.to_string()),
        merman::svg::HeadlessError::Render(err) => {
            BindingError::new(BindingStatus::RenderError, err.to_string())
        }
        merman::svg::HeadlessError::RuntimePolicy(err) => runtime_policy_error(err),
        _ => BindingError::internal("unknown headless renderer failure"),
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn classify_output_error(
    err: merman::svg::OutputError,
    profile: merman::resources::ResourceProfile,
) -> BindingError {
    match err {
        merman::svg::OutputError::Headless(err) => classify_render_error(err),
        merman::svg::OutputError::Export(err) => match err.resource_limit_details() {
            Some(details) => BindingError::resource_limit(
                details.phase,
                details.limit_id,
                details.actual,
                details.max,
                profile.id(),
                err.to_string(),
            ),
            None => BindingError::new(BindingStatus::RenderError, err.to_string()),
        },
        _ => BindingError::internal("unknown SVG output failure"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
    #[test]
    fn export_options_compile_through_the_public_json_shape() {
        let options = crate::common::parse_options(
            br##"{
                "raster": {
                    "scale": 1.5,
                    "background": "#ffffff",
                    "fit_to": {"width": 640}
                },
                "jpeg": {"quality": 82},
                "pdf": {
                    "background": "transparent",
                    "filter_scale": 2.5,
                    "page_policy": {"kind": "fixed", "width_pt": 612, "height_pt": 792}
                },
                "resources": {
                    "limits": {"max_pdf_filter_image_pixels": 1234}
                }
            }"##,
        )
        .expect("valid export options");
        let config = compile_for_test(&options, merman::runtime::RuntimePolicy::deterministic())
            .expect("export options compile");
        assert_eq!(config.raster_options.scale, 1.5);
        assert_eq!(config.raster_options.jpeg_quality, 82);
        let fit = config.raster_options.fit_to.expect("fit box");
        assert_eq!(fit.width, Some(640));
        assert_eq!(fit.height, None);
        assert_eq!(
            config.pdf_options.page_policy,
            merman::svg::export::PdfPagePolicy::Fixed {
                width_pt: 612.0,
                height_pt: 792.0
            }
        );
        assert_eq!(
            config.pdf_options.filter_image_limit.max_total_pixels,
            Some(1234)
        );
        assert_eq!(config.pdf_options.filter_scale, 2.5);
    }

    #[cfg(feature = "svg")]
    #[test]
    fn svg_viewbox_padding_compiles_through_the_public_json_shape() {
        let options = crate::common::parse_options(
            br#"{"svg":{"diagram_id":"docs-flow","viewBoxPadding":12.5}}"#,
        )
        .expect("valid SVG options");
        let config = compile_for_test(&options, merman::runtime::RuntimePolicy::deterministic())
            .expect("SVG options compile");

        assert_eq!(config.svg.diagram_id.as_deref(), Some("docs-flow"));
        assert_eq!(config.svg.viewbox_padding, 12.5);
    }

    #[cfg(feature = "svg")]
    #[test]
    fn svg_viewbox_padding_rejects_negative_or_unrepresentable_values() {
        let error = crate::common::parse_options(br#"{"svg":{"viewbox_padding":-1}}"#)
            .and_then(|options| {
                compile_for_test(&options, merman::runtime::RuntimePolicy::deterministic())
            })
            .err()
            .expect("negative SVG padding");
        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert!(error.message().contains("svg.viewbox_padding"), "{error:?}");

        let error = crate::common::parse_options(br#"{"svg":{"viewbox_padding":1e400}}"#)
            .expect_err("unrepresentable JSON number");
        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
    }

    #[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
    #[test]
    fn export_options_reject_backend_colors_that_would_be_ignored() {
        let options = crate::common::parse_options(br#"{"raster":{"background":"not-a-color"}}"#)
            .expect("JSON shape is valid");
        let error = compile_for_test(&options, merman::runtime::RuntimePolicy::deterministic())
            .err()
            .expect("unsupported backend color");
        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert!(error.message().contains("raster.background"));
    }

    #[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
    #[test]
    fn export_options_reject_invalid_numeric_and_shape_boundaries() {
        let cases: &[(&[u8], &str)] = &[
            (br#"{"raster":{"scale":0}}"#, "raster.scale"),
            (br#"{"raster":{"fit_to":{}}}"#, "raster.fit_to"),
            (br#"{"raster":{"fit_to":{"width":0}}}"#, "raster.fit_to"),
            (br#"{"jpeg":{"quality":0}}"#, "jpeg.quality"),
            (br#"{"jpeg":{"quality":101}}"#, "jpeg.quality"),
            (
                br#"{"pdf":{"page_policy":{"kind":"fixed","width_pt":0,"height_pt":10}}}"#,
                "pdf.page_policy.width_pt",
            ),
            (
                br#"{"pdf":{"page_policy":{"kind":"fixed","width_pt":10,"height_pt":0}}}"#,
                "pdf.page_policy.height_pt",
            ),
            (
                br#"{"pdf":{"page_policy":{"kind":"fit-css-width","max_width_px":0}}}"#,
                "pdf.page_policy.max_width_px",
            ),
            (br#"{"pdf":{"background":"not-a-color"}}"#, "pdf.background"),
        ];

        for &(options_json, expected_field) in cases {
            let options = crate::common::parse_options(options_json).expect("valid JSON shape");
            let error = compile_for_test(&options, merman::runtime::RuntimePolicy::deterministic())
                .err()
                .expect("invalid export option must fail before backend work");
            assert_eq!(error.status(), BindingStatus::InvalidArgument);
            assert!(
                error.message().contains(expected_field),
                "{expected_field}: {error:?}"
            );
        }
    }

    #[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
    #[test]
    fn export_resource_failures_keep_stable_structured_metadata() {
        let error = classify_output_error(
            merman::svg::OutputError::Export(
                merman::svg::export::ExportError::EmbeddedImageLimit {
                    limit_name: "max_bytes_per_image",
                    actual: 5,
                    max: 4,
                },
            ),
            merman::resources::ResourceProfile::Constrained,
        );
        let details = error.resource_details().expect("resource details");
        assert_eq!(
            details.limit_id,
            merman::svg::export::MAX_EMBEDDED_IMAGE_BYTES_RESOURCE_LIMIT_ID
        );
        assert_eq!(details.phase, "embedded_image_decode");
        assert_eq!(details.actual, 5);
        assert_eq!(details.max, 4);
        assert_eq!(details.profile, "constrained");

        let error = classify_output_error(
            merman::svg::OutputError::Export(
                merman::svg::export::ExportError::PdfFilterImageLimit { actual: 5, max: 4 },
            ),
            merman::resources::ResourceProfile::Constrained,
        );
        let details = error.resource_details().expect("PDF resource details");
        assert_eq!(
            details.limit_id,
            merman::svg::export::MAX_PDF_FILTER_IMAGE_PIXELS_RESOURCE_LIMIT_ID
        );
        assert_eq!(details.phase, "pdf_filter_rasterization");
        assert_eq!(details.actual, 5);
        assert_eq!(details.max, 4);
        assert_eq!(details.profile, "constrained");
    }

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
                "fixed_today": "-2147483648-01-01",
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
