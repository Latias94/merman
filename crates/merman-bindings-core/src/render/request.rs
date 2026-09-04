#[cfg(test)]
use crate::common::binding_runtime_policy_from;
use crate::common::{
    BindingError, BindingOptions, BindingResourceLimitCause, BindingStatus,
    binding_resource_policy, binding_site_config, css_declaration_value, finite_positive,
    internal_json_error, no_diagram_error, normalize_option, runtime_policy_error,
};
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use crate::common::{BindingExportResourceOptions, binding_export_resource_options};
use crate::theme_execution_evidence::BindingThemeExecutionEvidence;
use merman::svg::{
    LayoutOptions, MeasurementProfileId, RenderCapability, RenderCapabilityPolicy,
    TextMeasurementPhase, TextMeasurementPolicy, TextMeasurementProfileIdentity,
};
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgEnvironment, SvgRequest};

#[derive(Clone)]
pub(super) struct RenderRequestPlan {
    renderer: Renderer,
    svg: SvgRequest,
    theme: Option<merman::svg::DiagramTheme>,
    parse_options: merman::ParseOptions,
    input_resources: merman::resources::InputResourcePolicy,
    resource_profile: merman::resources::ResourceProfile,
    #[cfg(any(feature = "png", feature = "jpeg"))]
    raster_options: merman::svg::export::RasterOptions,
    #[cfg(feature = "pdf")]
    pdf_options: merman::svg::export::PdfOptions,
}

pub(super) struct RenderOperationConfig {
    environment: SvgEnvironment,
    runtime_policy: merman::runtime::RuntimePolicy,
    input_resources: merman::resources::InputResourcePolicy,
    lenient_parsing: bool,
    theme: Option<merman::svg::DiagramTheme>,
    site_config: Option<merman::MermaidConfig>,
    layout: LayoutOptions,
    svg: merman::svg::SvgRenderOptions,
    output: merman::svg::SvgOutputPolicy,
    #[cfg(any(feature = "png", feature = "jpeg"))]
    raster_options: merman::svg::export::RasterOptions,
    #[cfg(feature = "pdf")]
    pdf_options: merman::svg::export::PdfOptions,
}

impl RenderRequestPlan {
    pub(super) fn render_svg_output(
        &self,
        source: &str,
        control: OperationControl,
    ) -> Result<crate::operation::BindingOperationOutput, BindingError> {
        let output = self
            .renderer
            .render(self.request(source, merman::RenderTarget::Svg(self.svg.clone()), control))
            .map_err(|error| classify_render_error(error, self.resource_profile))?;
        let RenderOutput::Svg(svg) = output else {
            return Err(unexpected_render_output("svg"));
        };
        let output = svg.ok_or_else(no_diagram_error)?;
        let evidence =
            BindingThemeExecutionEvidence::project(output.evidence(), output.admission())?;
        Ok(crate::operation::BindingOperationOutput::svg(
            output.into_parts().0.into_bytes(),
            evidence,
        ))
    }

    pub(super) fn layout_json(
        &self,
        source: &str,
        control: OperationControl,
    ) -> Result<Vec<u8>, BindingError> {
        let output = self
            .renderer
            .render(self.request(
                source,
                merman::RenderTarget::LayoutJson(self.svg.clone()),
                control,
            ))
            .map_err(|error| classify_render_error(error, self.resource_profile))?;
        let RenderOutput::LayoutJson(layout_json) = output else {
            return Err(unexpected_render_output("layout-json"));
        };
        let layout_json = layout_json
            .map(|output| output.into_parts().0)
            .ok_or_else(no_diagram_error)?;

        serde_json::to_vec(&layout_json).map_err(internal_json_error)
    }

    pub(super) fn svg_plan_json(
        &self,
        source: &str,
        control: OperationControl,
    ) -> Result<Vec<u8>, BindingError> {
        let output = self
            .renderer
            .render(self.request(
                source,
                merman::RenderTarget::SvgPlan(self.svg.clone()),
                control,
            ))
            .map_err(|error| classify_render_error(error, self.resource_profile))?;
        let RenderOutput::SvgPlan(plan) = output else {
            return Err(unexpected_render_output("svg-plan"));
        };
        let plan = plan.ok_or_else(no_diagram_error)?;

        crate::SvgPlanPayload::from_render_plan(&plan)?.to_json_bytes()
    }

    #[cfg(feature = "png")]
    pub(super) fn render_png_output(
        &self,
        source: &str,
        control: OperationControl,
    ) -> Result<crate::operation::BindingOperationOutput, BindingError> {
        let output = self
            .renderer
            .render(self.request(
                source,
                merman::RenderTarget::Png(merman::PngRequest {
                    svg: self.svg.clone(),
                    options: self.raster_options.clone(),
                }),
                control,
            ))
            .map_err(|error| classify_render_error(error, self.resource_profile))?;
        let RenderOutput::Png(output) = output else {
            return Err(unexpected_render_output("png"));
        };
        let output = output.ok_or_else(no_diagram_error)?;
        let plan = output.plan();
        let evidence =
            BindingThemeExecutionEvidence::project(output.evidence(), output.admission())?;
        Ok(crate::operation::BindingOperationOutput::raster(
            output.into_bytes(),
            plan,
            evidence,
        ))
    }

    #[cfg(feature = "jpeg")]
    pub(super) fn render_jpeg_output(
        &self,
        source: &str,
        control: OperationControl,
    ) -> Result<crate::operation::BindingOperationOutput, BindingError> {
        let output = self
            .renderer
            .render(self.request(
                source,
                merman::RenderTarget::Jpeg(merman::JpegRequest {
                    svg: self.svg.clone(),
                    options: self.raster_options.clone(),
                }),
                control,
            ))
            .map_err(|error| classify_render_error(error, self.resource_profile))?;
        let RenderOutput::Jpeg(output) = output else {
            return Err(unexpected_render_output("jpeg"));
        };
        let output = output.ok_or_else(no_diagram_error)?;
        let plan = output.plan();
        let evidence =
            BindingThemeExecutionEvidence::project(output.evidence(), output.admission())?;
        Ok(crate::operation::BindingOperationOutput::raster(
            output.into_bytes(),
            plan,
            evidence,
        ))
    }

    #[cfg(feature = "pdf")]
    pub(super) fn render_pdf_output(
        &self,
        source: &str,
        control: OperationControl,
    ) -> Result<crate::operation::BindingOperationOutput, BindingError> {
        let output = self
            .renderer
            .render(self.request(
                source,
                merman::RenderTarget::Pdf(merman::PdfRequest {
                    svg: self.svg.clone(),
                    options: self.pdf_options.clone(),
                }),
                control,
            ))
            .map_err(|error| classify_render_error(error, self.resource_profile))?;
        let RenderOutput::Pdf(output) = output else {
            return Err(unexpected_render_output("pdf"));
        };
        let output = output.ok_or_else(no_diagram_error)?;
        let plan = output.plan();
        let evidence =
            BindingThemeExecutionEvidence::project(output.evidence(), output.admission())?;
        Ok(crate::operation::BindingOperationOutput::pdf(
            output.into_bytes(),
            plan,
            evidence,
        ))
    }

    fn request<'a>(
        &self,
        source: &'a str,
        target: merman::RenderTarget,
        control: OperationControl,
    ) -> RenderRequest<'a> {
        let request = RenderRequest::new(source, target, control)
            .with_parse_options(self.parse_options)
            .with_resource_policy(self.input_resources);
        match self.theme.as_ref() {
            Some(theme) => request.with_theme(theme.clone()),
            None => request,
        }
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
    let compiler = merman::svg::DiagramThemeCompiler::new();
    let theme = crate::theme::compile_theme_with(&compiler, options.theme.as_ref())?;
    RenderOperationConfig::compile(
        options,
        runtime_policy,
        RenderCapabilityPolicy::unrestricted(),
        theme,
        compiler.resource_policy().clone(),
    )
}

impl RenderOperationConfig {
    pub(super) fn compile(
        options: &BindingOptions,
        runtime_policy: merman::runtime::RuntimePolicy,
        capability_policy: RenderCapabilityPolicy,
        theme: Option<merman::svg::DiagramTheme>,
        theme_resources: merman::svg::ThemeResourcePolicy,
    ) -> Result<Self, BindingError> {
        let render_resources = binding_resource_policy(options.analysis.resources.as_ref())?;
        let input_resources = *render_resources.input_policy();
        let mut environment =
            SvgEnvironment::deterministic().with_capability_policy(capability_policy);
        environment = environment
            .with_resource_policy(render_resources)
            .with_theme_resource_ceiling(theme_resources);
        if let Some(environment_json) = options.environment.as_ref() {
            if let Some(kind) = environment_json.text_measurement.as_deref() {
                environment = environment.with_text_measurement_policy(
                    match normalize_option(kind).as_str() {
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
            runtime_policy,
            input_resources,
            lenient_parsing,
            theme,
            site_config,
            layout,
            svg: svg_options,
            output,
            #[cfg(any(feature = "png", feature = "jpeg"))]
            raster_options,
            #[cfg(feature = "pdf")]
            pdf_options,
        })
    }

    pub(super) fn materialize(self, services: &crate::BindingEngineServices) -> RenderRequestPlan {
        let mut environment = if let Some(registry) = services.icon_registry() {
            self.environment.with_icon_registry(registry)
        } else {
            self.environment
        };
        if let Some(measurer) = services.host_text_measurer() {
            let identity = TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("merman.binding-host").expect("static profile id"),
                concat!("merman-bindings-core@", env!("CARGO_PKG_VERSION")),
            )
            .expect("static profile identity");
            let policy =
                TextMeasurementPolicy::host_display(identity, measurer, TextMeasurementPhase::ALL);
            environment = environment.with_text_measurement_policy(policy);
        }
        let parse_options = if self.lenient_parsing {
            merman::ParseOptions::lenient()
        } else {
            merman::ParseOptions::strict()
        };
        let input_resources = self.input_resources;
        let resource_profile = input_resources.profile();
        let mut engine = merman::Engine::new().with_runtime_policy(self.runtime_policy);
        if let Some(site_config) = self.site_config {
            engine = engine.with_site_config(site_config);
        }
        RenderRequestPlan {
            renderer: Renderer::new()
                .with_engine(engine)
                .with_parse_options(parse_options)
                .with_resource_policy(input_resources),
            svg: SvgRequest {
                environment,
                layout: self.layout,
                options: self.svg,
                debug: Default::default(),
                pipeline: Some(self.output.pipeline()),
            },
            theme: self.theme,
            parse_options,
            input_resources,
            resource_profile,
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
        if let Some(matte) = raster.matte.as_deref() {
            let matte = css_declaration_value(matte, "raster.matte")?;
            if !merman::svg::export::is_valid_export_color(&matte) {
                return Err(BindingError::new(
                    BindingStatus::InvalidArgument,
                    "raster.matte is not supported by the native exporter",
                ));
            }
            compiled.matte = Some(matte);
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
    if let Some(page_paint) = pdf.page_paint.as_deref() {
        let page_paint = css_declaration_value(page_paint, "pdf.page_paint")?;
        if !merman::svg::export::is_valid_export_color(&page_paint) {
            return Err(BindingError::new(
                BindingStatus::InvalidArgument,
                "pdf.page_paint is not supported by the native exporter",
            ));
        }
        compiled.page_paint = Some(page_paint);
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

fn classify_render_error(
    err: merman::RenderError,
    profile: merman::resources::ResourceProfile,
) -> BindingError {
    match err {
        merman::RenderError::Cancelled(err) => BindingError::cancelled(err),
        merman::RenderError::Parse(err) => crate::common::parse_error(err),
        merman::RenderError::ResourceLimitExceeded(err) => BindingError::resource_limit_with_cause(
            match err.cause {
                merman::render::ResourceLimitCause::Ceiling => BindingResourceLimitCause::Ceiling,
                merman::render::ResourceLimitCause::ArithmeticOverflow => {
                    BindingResourceLimitCause::ArithmeticOverflow
                }
            },
            err.phase,
            err.id,
            err.actual,
            err.maximum,
            profile.id(),
            err.to_string(),
        ),
        merman::RenderError::Svg(err @ merman::svg::RenderError::MissingCapability { .. }) => {
            BindingError::missing_capability(
                err.missing_capability()
                    .expect("matched missing render capability")
                    .id(),
                err.to_string(),
            )
        }
        merman::RenderError::Svg(err @ merman::svg::RenderError::InvalidIconOutput { .. }) => {
            BindingError::new(BindingStatus::InvalidArgument, err.to_string())
        }
        merman::RenderError::Svg(err @ merman::svg::RenderError::IconProcessing { .. }) => {
            BindingError::internal(err.to_string())
        }
        merman::RenderError::Svg(err) => {
            BindingError::new(BindingStatus::RenderError, err.to_string())
        }
        merman::RenderError::SvgEnvironment(err) => {
            BindingError::new(BindingStatus::InvalidArgument, err.to_string())
        }
        merman::RenderError::TargetAdmission(err) => {
            BindingError::new(BindingStatus::RenderError, err.to_string())
        }
        merman::RenderError::PortabilityUnavailableForTarget { target } => {
            BindingError::invalid_argument(format!(
                "render target `{target}` does not support RequirePortable portability admission"
            ))
        }
        merman::RenderError::RuntimePolicy(err) => runtime_policy_error(err),
        #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
        merman::RenderError::Export(err) => match err.resource_limit_details() {
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
        merman::RenderError::UnsupportedTarget(target) => BindingError::internal(format!(
            "renderer returned unsupported target `{target}` for an admitted binding operation"
        )),
        _ => BindingError::internal("unknown canonical renderer failure"),
    }
}

fn unexpected_render_output(target: &str) -> BindingError {
    BindingError::internal(format!(
        "canonical renderer returned the wrong output variant for `{target}`"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_evaluation_limits_keep_resource_status_for_render_outputs() {
        let error = classify_render_error(
            merman::RenderError::Parse(
                merman::Error::ThemeEvaluationLimit(merman::ThemeEvaluationLimitExceeded {
                    limit: "THEME_COLOR_LIMIT",
                    requested: "65".to_string(),
                    max: 64,
                })
                .into(),
            ),
            merman::resources::ResourceProfile::Interactive,
        );

        assert_eq!(error.status(), BindingStatus::ResourceLimitExceeded);
        assert!(error.message().contains("THEME_COLOR_LIMIT"));
        assert_eq!(error.resource_details(), None);
    }

    #[test]
    fn graphical_parse_errors_are_terminal_safe_and_structured() {
        let span = merman::SourceSpan::new(2, 7);
        let diagnostic = merman::ParseDiagnostic::new("bad\u{7}input")
            .with_span(span, merman::ParseDiagnosticSpanKind::Exact)
            .with_code("merman.test\u{1b}");
        let error = classify_render_error(
            merman::RenderError::from(merman::Error::diagram_parse_diagnostic(
                "flow\u{1b}",
                diagnostic,
            )),
            merman::resources::ResourceProfile::Interactive,
        );

        assert_eq!(error.status(), BindingStatus::ParseError);
        assert!(!error.message().contains('\u{1b}'));
        assert!(!error.message().contains('\u{7}'));
        let details = error
            .diagnostic_details()
            .expect("graphical parse errors preserve structured details");
        assert_eq!(details.code, "merman.test\\u{1B}");
        assert_eq!(details.diagram_type.as_deref(), Some("flow\\u{1B}"));
        assert_eq!(
            details.span,
            Some(crate::common::BindingDiagnosticSpan::new(2, 7, "exact"))
        );
    }

    #[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
    #[test]
    fn export_options_compile_through_the_public_json_shape() {
        let options = crate::common::parse_options(
            br##"{
                "raster": {
                    "scale": 1.5,
                    "matte": "#ffffff",
                    "fit_to": {"width": 640}
                },
                "jpeg": {"quality": 82},
                "pdf": {
                    "page_paint": "transparent",
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
        let options = crate::common::parse_options(br#"{"raster":{"matte":"not-a-color"}}"#)
            .expect("JSON shape is valid");
        let error = compile_for_test(&options, merman::runtime::RuntimePolicy::deterministic())
            .err()
            .expect("unsupported backend color");
        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert!(error.message().contains("raster.matte"));
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
            (br#"{"pdf":{"page_paint":"not-a-color"}}"#, "pdf.page_paint"),
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
        let error = classify_render_error(
            merman::RenderError::Export(merman::svg::export::ExportError::EmbeddedImageLimit {
                limit_name: "max_bytes_per_image",
                actual: 5,
                max: 4,
            }),
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

        let error = classify_render_error(
            merman::RenderError::Export(merman::svg::export::ExportError::PdfFilterImageLimit {
                actual: 5,
                max: 4,
            }),
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
    fn terminal_theme_resource_failures_keep_stable_structured_metadata() {
        let policy = merman_render::diagram_theme::ThemeResourcePolicy::constrained();
        let maximum = policy
            .value(merman_render::diagram_theme::ThemeResourceLimitId::MaxThemeEncodedBytes)
            .expect("constrained theme input ceiling");
        let resource = policy
            .check_theme_encoded_bytes(maximum + 1)
            .expect_err("fixture must exceed the constrained theme input ceiling");
        let error = classify_render_error(
            merman::RenderError::from(merman_render::Error::ThemeResourceLimitExceeded(resource)),
            merman::resources::ResourceProfile::Interactive,
        );
        let details = error.resource_details().expect("theme resource details");
        assert_eq!(details.limit_id, "max_theme_encoded_bytes");
        assert_eq!(details.phase, "theme_input");
        assert_eq!(details.actual, (maximum + 1) as u64);
        assert_eq!(details.max, maximum as u64);
        assert_eq!(details.profile, "interactive");
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
    fn fixed_today_freezes_the_binding_clock_across_operations() {
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

        let first = policy.begin_operation().expect("first operation");
        let second = policy.begin_operation().expect("second operation");

        assert_eq!(first, second);
        assert_eq!(first.unix_millis(), 1_781_046_000_000);
        assert_eq!(first.local_time_zone().fixed_offset_minutes(), Some(60));
    }
}
