#[cfg(test)]
use crate::common::binding_runtime_policy_from;
use crate::common::{
    BindingError, BindingOptions, BindingResourceLimitCause, BindingStatus, LightSvgOverride,
    binding_resource_policy, binding_site_config, css_declaration_value, finite_positive,
    internal_json_error, no_diagram_error, normalize_option, runtime_policy_error,
};
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use crate::common::{BindingExportResourceOptions, binding_export_resource_options};
use merman::SourcePresentationConfig;
use merman::svg::{
    LayoutOptions, MeasurementProfileId, RenderCapability, RenderCapabilityPolicy, SvgGeneration,
    SvgPipeline, TextMeasurementPhase, TextMeasurementPolicy, TextMeasurementProfileIdentity,
};
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgEnvironment, SvgRequest};

#[derive(Clone)]
pub(super) struct RenderRequestPlan {
    renderer: Renderer,
    runtime_policy: merman::model::runtime::RuntimePolicy,
    svg: SvgRequest,
    parse_options: merman::ParseOptions,
    input_resources: merman::model::resources::InputResourcePolicy,
    resource_profile: merman::model::resources::ResourceProfile,
    render_resources: merman::svg::RenderResourcePolicy,
    #[cfg(feature = "native-fonts")]
    native_font_context: Option<merman::svg::export::NativeFontContext>,
    #[cfg(any(feature = "png", feature = "jpeg"))]
    raster_options: merman::svg::export::RasterOptions,
    #[cfg(feature = "pdf")]
    pdf_options: merman::svg::export::PdfOptions,
}

#[derive(Clone)]
pub(super) struct RenderOperationConfig {
    environment: SvgEnvironment,
    runtime_policy: merman::model::runtime::RuntimePolicy,
    input_resources: merman::model::resources::InputResourcePolicy,
    render_resources: merman::svg::RenderResourcePolicy,
    lenient_parsing: bool,
    theme: Option<merman::theme::ThemeSelection>,
    background: Option<merman::theme::Color>,
    site_config: Option<merman::MermaidConfig>,
    layout: LayoutOptions,
    svg: merman::svg::SvgRenderOptions,
    pipeline: SvgPipeline,
    #[cfg(any(feature = "png", feature = "jpeg"))]
    raster_options: merman::svg::export::RasterOptions,
    #[cfg(feature = "pdf")]
    pdf_options: merman::svg::export::PdfOptions,
}

#[derive(Clone, Copy)]
pub(super) enum RenderInput<'a> {
    Source(&'a str),
    Prepared(&'a merman::PreparedSource),
}

impl RenderRequestPlan {
    pub(super) fn prepare_source(
        &self,
        source: &str,
        control: OperationControl,
    ) -> Result<merman::PreparedSource, BindingError> {
        let request = merman::PrepareSourceRequest::new(source, control)
            .with_parse_options(self.parse_options)
            .with_resource_policy(self.input_resources);
        self.renderer
            .prepare_source(request)
            .map_err(|error| classify_render_error(error, self.resource_profile))?
            .ok_or_else(no_diagram_error)
    }

    pub(super) fn runtime_policy(&self) -> merman::model::runtime::RuntimePolicy {
        self.runtime_policy.clone()
    }

    fn render_input(
        &self,
        input: RenderInput<'_>,
        target: merman::RenderTarget,
        control: OperationControl,
    ) -> Result<RenderOutput, BindingError> {
        match input {
            RenderInput::Source(source) => {
                self.renderer.render(self.request(source, target, control))
            }
            RenderInput::Prepared(prepared) => self.renderer.render_prepared_with_resource_policy(
                prepared,
                target,
                control,
                self.input_resources,
            ),
        }
        .map_err(|error| classify_render_error(error, self.resource_profile))
    }

    pub(super) fn render_svg_output(
        &self,
        input: RenderInput<'_>,
        control: OperationControl,
        include_metadata: bool,
        overlay: Option<LightSvgOverride>,
    ) -> Result<crate::operation::BindingOperationOutput, BindingError> {
        let mut request = self.svg_request()?;
        if let Some(overlay) = overlay {
            request.options.diagram_id = overlay.diagram_id;
        }
        if !include_metadata {
            let svg = match input {
                RenderInput::Source(source) => self
                    .renderer
                    .render_svg_data(self.request(
                        source,
                        merman::RenderTarget::Svg(request),
                        control,
                    ))
                    .map_err(|error| classify_render_error(error, self.resource_profile))?
                    .ok_or_else(no_diagram_error)?,
                RenderInput::Prepared(prepared) => self
                    .renderer
                    .render_prepared_svg_data(prepared, request, control)
                    .map_err(|error| classify_render_error(error, self.resource_profile))?,
            };
            return Ok(crate::operation::BindingOperationOutput::plain(
                svg.into_bytes(),
            ));
        }
        let RenderOutput::Svg(svg) =
            self.render_input(input, merman::RenderTarget::Svg(request), control)?
        else {
            return Err(unexpected_render_output("svg"));
        };
        let output = svg.ok_or_else(no_diagram_error)?;
        let layout_adaptation = output.layout_adaptation();
        let theme_outcome =
            serde_json::to_value(output.theme_outcome()).map_err(internal_json_error)?;
        #[cfg(feature = "layout-shoal")]
        let shoal_layout_frame = match output.shoal_layout_frame() {
            Some(frame) => project_shoal_layout_frame(frame)?,
            None => None,
        };
        let output = crate::operation::BindingOperationOutput::svg(
            output.into_parts().0.into_bytes(),
            theme_outcome,
        )
        .with_layout_adaptation(layout_adaptation);
        #[cfg(feature = "layout-shoal")]
        let output = output.with_shoal_layout_frame(shoal_layout_frame);
        Ok(output)
    }

    pub(super) fn layout_json(
        &self,
        input: RenderInput<'_>,
        control: OperationControl,
    ) -> Result<Vec<u8>, BindingError> {
        let output = self.render_input(
            input,
            merman::RenderTarget::LayoutSnapshot(self.svg_request()?),
            control.clone(),
        )?;
        let RenderOutput::LayoutJson(layout_json) = output else {
            return Err(unexpected_render_output("layout-json"));
        };
        let layout_json = layout_json
            .map(|output| output.into_parts().0)
            .ok_or_else(no_diagram_error)?;
        let mut bytes = Vec::new();
        layout_json
            .write_json_controlled(&mut bytes, &control.for_phase(merman::OperationPhase::Emit))
            .map_err(BindingError::cancelled)?
            .map_err(internal_json_error)?;
        Ok(bytes)
    }

    pub(super) fn edge_geometry_json(
        &self,
        input: RenderInput<'_>,
        control: OperationControl,
    ) -> Result<Vec<u8>, BindingError> {
        let output = self.render_input(
            input,
            merman::RenderTarget::EdgeGeometryJson(self.svg_request()?),
            control.clone(),
        )?;
        let RenderOutput::EdgeGeometryJson(edge_geometry_json) = output else {
            return Err(unexpected_render_output("edge-geometry-json"));
        };
        let mut bytes = Vec::new();
        edge_geometry_json
            .ok_or_else(no_diagram_error)?
            .write_json_controlled(&mut bytes, self.render_resources, &control)
            .map_err(|error| classify_render_error(error.into(), self.resource_profile))?;
        Ok(bytes)
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
                merman::RenderTarget::SvgPlan(self.svg_request()?),
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
        include_metadata: bool,
    ) -> Result<crate::operation::BindingOperationOutput, BindingError> {
        let output = self
            .renderer
            .render(self.request(
                source,
                merman::RenderTarget::Png(merman::PngRequest {
                    svg: self.svg_request()?,
                    options: self.raster_options.clone(),
                }),
                control,
            ))
            .map_err(|error| classify_render_error(error, self.resource_profile))?;
        let RenderOutput::Png(output) = output else {
            return Err(unexpected_render_output("png"));
        };
        let output = output.ok_or_else(no_diagram_error)?;
        if !include_metadata {
            return Ok(crate::operation::BindingOperationOutput::plain(
                output.into_bytes(),
            ));
        }
        let plan = output.plan();
        let layout_adaptation = output.layout_adaptation();
        let theme_outcome =
            serde_json::to_value(output.theme_outcome()).map_err(internal_json_error)?;
        Ok(crate::operation::BindingOperationOutput::raster(
            output.into_bytes(),
            plan,
            theme_outcome,
        )
        .with_layout_adaptation(layout_adaptation))
    }

    #[cfg(feature = "jpeg")]
    pub(super) fn render_jpeg_output(
        &self,
        source: &str,
        control: OperationControl,
        include_metadata: bool,
    ) -> Result<crate::operation::BindingOperationOutput, BindingError> {
        let output = self
            .renderer
            .render(self.request(
                source,
                merman::RenderTarget::Jpeg(merman::JpegRequest {
                    svg: self.svg_request()?,
                    options: self.raster_options.clone(),
                }),
                control,
            ))
            .map_err(|error| classify_render_error(error, self.resource_profile))?;
        let RenderOutput::Jpeg(output) = output else {
            return Err(unexpected_render_output("jpeg"));
        };
        let output = output.ok_or_else(no_diagram_error)?;
        if !include_metadata {
            return Ok(crate::operation::BindingOperationOutput::plain(
                output.into_bytes(),
            ));
        }
        let plan = output.plan();
        let layout_adaptation = output.layout_adaptation();
        let theme_outcome =
            serde_json::to_value(output.theme_outcome()).map_err(internal_json_error)?;
        Ok(crate::operation::BindingOperationOutput::raster(
            output.into_bytes(),
            plan,
            theme_outcome,
        )
        .with_layout_adaptation(layout_adaptation))
    }

    #[cfg(feature = "pdf")]
    pub(super) fn render_pdf_output(
        &self,
        source: &str,
        control: OperationControl,
        include_metadata: bool,
    ) -> Result<crate::operation::BindingOperationOutput, BindingError> {
        let output = self
            .renderer
            .render(self.request(
                source,
                merman::RenderTarget::Pdf(merman::PdfRequest {
                    svg: self.svg_request()?,
                    options: self.pdf_options.clone(),
                }),
                control,
            ))
            .map_err(|error| classify_render_error(error, self.resource_profile))?;
        let RenderOutput::Pdf(output) = output else {
            return Err(unexpected_render_output("pdf"));
        };
        let output = output.ok_or_else(no_diagram_error)?;
        if !include_metadata {
            return Ok(crate::operation::BindingOperationOutput::plain(
                output.into_bytes(),
            ));
        }
        let plan = output.plan();
        let layout_adaptation = output.layout_adaptation();
        let theme_outcome =
            serde_json::to_value(output.theme_outcome()).map_err(internal_json_error)?;
        Ok(
            crate::operation::BindingOperationOutput::pdf(output.into_bytes(), plan, theme_outcome)
                .with_layout_adaptation(layout_adaptation),
        )
    }

    fn svg_request(&self) -> Result<SvgRequest, BindingError> {
        let svg = self.svg.clone();
        #[cfg(feature = "native-fonts")]
        if let Some(context) = &self.native_font_context {
            // Select reusable assets here; facade target admission owns the operation-local fork.
            let mut svg = svg;
            svg.environment = svg
                .environment
                .with_native_font_context(context.clone())
                .map_err(|error| classify_render_error(error.into(), self.resource_profile))?;
            return Ok(svg);
        }
        Ok(svg)
    }

    fn request<'a>(
        &self,
        source: &'a str,
        target: merman::RenderTarget,
        control: OperationControl,
    ) -> RenderRequest<'a> {
        RenderRequest::new(source, target, control)
            .with_parse_options(self.parse_options)
            .with_resource_policy(self.input_resources)
    }
}

#[cfg(test)]
pub(super) fn pipeline_for_options(
    options: &BindingOptions,
) -> Result<merman::svg::SvgPipeline, BindingError> {
    Ok(compile_for_test(
        options,
        merman::model::runtime::RuntimePolicy::deterministic(),
    )?
    .pipeline)
}

#[cfg(test)]
fn compile_for_test(
    options: &BindingOptions,
    runtime_policy: merman::model::runtime::RuntimePolicy,
) -> Result<RenderOperationConfig, BindingError> {
    RenderOperationConfig::compile(
        options,
        runtime_policy,
        RenderCapabilityPolicy::unrestricted(),
        options.theme.clone(),
    )
}

impl RenderOperationConfig {
    pub(super) fn compile(
        options: &BindingOptions,
        runtime_policy: merman::model::runtime::RuntimePolicy,
        capability_policy: RenderCapabilityPolicy,
        theme: Option<merman::theme::ThemeSelection>,
    ) -> Result<Self, BindingError> {
        let render_resources = binding_resource_policy(options.analysis.resources.as_ref())?;
        let input_resources = *render_resources.input_policy();
        let mut environment = SvgEnvironment::deterministic()
            .with_capability_policy(capability_policy)
            .with_resource_policy(render_resources);
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
            if let Some(adaptation) = layout_json.adaptation.as_deref() {
                layout.adaptation = match normalize_option(adaptation).as_str() {
                    "none" => merman::svg::LayoutAdaptation::None,
                    "fit-width" => merman::svg::LayoutAdaptation::FitWidth,
                    _ => {
                        return Err(BindingError::invalid_argument(format!(
                            "unsupported layout.adaptation: {adaptation}"
                        )));
                    }
                };
            }
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
            let capture_shoal_frame = layout_json.capture_shoal_frame == Some(true)
                || layout_json.previous_shoal_frame.is_some();
            if (capture_shoal_frame || layout_json.fold_wide_ranks == Some(true))
                && (!cfg!(feature = "layout-shoal")
                    || !capability_policy.allows(RenderCapability::LayoutShoal))
            {
                return Err(BindingError::missing_capability(
                    "layout-shoal",
                    "Shoal frame options and fold_wide_ranks require the layout-shoal capability",
                ));
            }
            #[cfg(feature = "layout-shoal")]
            {
                if let Some(fold) = layout_json.fold_wide_ranks {
                    layout = layout.with_fold_wide_ranks(fold);
                }
                if capture_shoal_frame {
                    let previous = layout_json
                        .previous_shoal_frame
                        .as_deref()
                        .map(parse_previous_shoal_frame)
                        .transpose()?;
                    layout = layout.with_shoal_layout_frame(previous.map(std::sync::Arc::new));
                }
            }
        }

        let mut svg_options = merman::svg::SvgRenderOptions::default();
        let mut static_generation = false;
        if let Some(svg) = options.svg.as_ref() {
            svg_options.diagram_id.clone_from(&svg.diagram_id);
            if let Some(viewbox_padding) = svg.viewbox_padding {
                svg_options.viewbox_padding =
                    finite_nonnegative(viewbox_padding, "svg.viewbox_padding")?;
            }
            if let Some(raw_pipeline) = svg.pipeline.as_deref() {
                output.preset = match normalize_option(raw_pipeline).as_str() {
                    "parity" => merman::svg::SvgPipelinePreset::Parity,
                    "static" => {
                        static_generation = true;
                        merman::svg::SvgPipelinePreset::Parity
                    }
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
                let color =
                    css_declaration_value(root_background_color, "svg.root_background_color")?;
                if options.background.is_none() {
                    output.root_background_color = Some(color);
                }
            }
            if let Some(drop_native_duplicate_fallbacks) = svg.drop_native_duplicate_fallbacks {
                output.drop_native_duplicate_fallbacks = drop_native_duplicate_fallbacks;
            }
        }
        let pipeline = output.pipeline();
        let pipeline = if static_generation {
            pipeline.with_generation(SvgGeneration::Static)
        } else {
            pipeline
        };

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
            render_resources,
            lenient_parsing,
            theme,
            background: options.background,
            site_config,
            layout,
            svg: svg_options,
            pipeline,
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
        let runtime_policy = self.runtime_policy;
        let mut engine = merman::MermaidEngine::new().with_runtime_policy(runtime_policy.clone());
        if let Some(site_config) = self.site_config {
            engine = engine.with_site_config(SourcePresentationConfig::from_config(site_config));
        }
        let mut svg = SvgRequest {
            environment,
            layout: self.layout,
            options: self.svg,
            debug: Default::default(),
            pipeline: Some(self.pipeline),
            output_profile: Default::default(),
        };
        if let Some(theme) = self.theme {
            svg = svg.with_native_theme(theme);
        }
        if let Some(background) = self.background {
            svg = svg.with_background(background);
        }
        RenderRequestPlan {
            runtime_policy,
            renderer: Renderer::new()
                .with_presentation_engine(engine)
                .with_parse_options(parse_options)
                .with_resource_policy(input_resources),
            svg,
            parse_options,
            input_resources,
            resource_profile,
            render_resources: self.render_resources,
            #[cfg(feature = "native-fonts")]
            native_font_context: services.native_font_context(),
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
        matte: options.export_matte.map(|color| color.to_string()),
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
            if compiled.matte.is_none() {
                compiled.matte = Some(matte);
            }
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
        page_paint: options.export_matte.map(|color| color.to_string()),
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
        if compiled.page_paint.is_none() {
            compiled.page_paint = Some(page_paint);
        }
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

#[cfg(feature = "layout-shoal")]
fn project_shoal_layout_frame(
    frame: &impl serde::Serialize,
) -> Result<Option<serde_json::Value>, BindingError> {
    let mut budget = ShoalFrameJsonBudget(crate::BINDING_OPTIONS_JSON_MAX_BYTES);
    match serde_json::to_writer(&mut budget, frame) {
        Ok(()) => serde_json::to_value(frame)
            .map(Some)
            .map_err(internal_json_error),
        Err(error) if error.io_error_kind() == Some(std::io::ErrorKind::StorageFull) => Ok(None),
        Err(error) => Err(internal_json_error(error)),
    }
}

#[cfg(feature = "layout-shoal")]
struct ShoalFrameJsonBudget(usize);

#[cfg(feature = "layout-shoal")]
impl std::io::Write for ShoalFrameJsonBudget {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 = self
            .0
            .checked_sub(bytes.len())
            .ok_or(std::io::ErrorKind::StorageFull)?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(feature = "layout-shoal")]
fn parse_previous_shoal_frame(
    raw: &serde_json::value::RawValue,
) -> Result<merman::svg::ShoalLayoutFrame, BindingError> {
    if !raw.get().trim_start().starts_with('{') {
        return Err(BindingError::invalid_argument(
            "layout.previous_shoal_frame must be an object",
        ));
    }
    serde_json::from_str(raw.get()).map_err(|_| {
        BindingError::invalid_argument("layout.previous_shoal_frame is not a valid Shoal frame")
    })
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
    profile: merman::model::resources::ResourceProfile,
) -> BindingError {
    match err {
        #[cfg(feature = "native-fonts")]
        merman::RenderError::NativeFont(merman::svg::export::NativeFontError::Operation(error)) => {
            classify_render_error(
                merman::svg::RenderError::OperationResourceTerminal(error).into(),
                profile,
            )
        }
        #[cfg(feature = "native-fonts")]
        merman::RenderError::NativeFont(error) => {
            if let Some((id, actual, limit)) = error.resource_limit() {
                BindingError::resource_limit(
                    "native-font-measurement",
                    id,
                    actual,
                    limit,
                    profile.id(),
                    error.to_string(),
                )
            } else {
                BindingError::new(BindingStatus::RenderError, error.to_string())
            }
        }
        merman::RenderError::Cancelled(err) => BindingError::cancelled(err),
        merman::RenderError::Parse(err) => crate::common::parse_error(err),
        merman::RenderError::Theme(err) => crate::native_theme::compile_error("theme", err),
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
        merman::RenderError::Svg(
            err @ (merman::svg::RenderError::UnsupportedLayoutBackend { .. }
            | merman::svg::RenderError::UnsupportedLayoutConstruct { .. }),
        ) => BindingError::unsupported_operation(err.to_string()),
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

    #[cfg(feature = "layout-shoal")]
    #[test]
    fn shoal_capture_omits_frames_beyond_the_options_byte_limit() {
        #[derive(serde::Serialize)]
        struct Frame<'a> {
            node_id: &'a str,
        }

        let mut node_id = String::from("界\"");
        let overhead = serde_json::to_vec(&Frame { node_id: &node_id })
            .unwrap()
            .len();
        node_id.extend(std::iter::repeat_n(
            'x',
            crate::BINDING_OPTIONS_JSON_MAX_BYTES - overhead,
        ));
        let frame = project_shoal_layout_frame(&Frame { node_id: &node_id })
            .unwrap()
            .unwrap();
        assert_eq!(frame["node_id"].as_str(), Some(node_id.as_str()));
        drop(frame);

        node_id.push('x');
        assert!(
            project_shoal_layout_frame(&Frame { node_id: &node_id })
                .unwrap()
                .is_none()
        );
    }

    #[cfg(feature = "native-fonts")]
    #[test]
    fn native_font_errors_preserve_structured_operation_terminal_details() {
        use merman::model::{
            CancelReason, OperationCancelled, OperationLedgerError, OperationPhase,
            OperationResourceDomain, OperationResourceLimitExceeded, OperationResourceProvenance,
        };
        let profile = merman::model::resources::ResourceProfile::Interactive;
        let cancelled = classify_render_error(
            merman::svg::export::NativeFontError::Operation(OperationLedgerError::Cancelled(
                OperationCancelled {
                    phase: OperationPhase::Layout,
                    reason: CancelReason::Requested,
                },
            ))
            .into(),
            profile,
        );
        assert_eq!(cancelled.status(), BindingStatus::Cancelled);
        assert!(cancelled.cancellation_details().is_some());
        assert!(cancelled.resource_details().is_none());

        let limited = classify_render_error(
            merman::svg::export::NativeFontError::Operation(
                OperationResourceLimitExceeded {
                    id: "native_font_measurements",
                    phase: OperationPhase::Layout,
                    resource_phase: "native-font-measurement",
                    limit: 64,
                    consumed: 64,
                    requested: 1,
                    provenance: OperationResourceProvenance::new(
                        OperationResourceDomain::Render,
                        None,
                        [],
                    ),
                }
                .into(),
            )
            .into(),
            profile,
        );
        assert_eq!(limited.status(), BindingStatus::ResourceLimitExceeded);
        let details = limited.resource_details().expect("native resource details");
        assert_eq!(details.limit_id, "native_font_measurements");
        assert_eq!(details.phase, "native-font-measurement");
        assert_eq!(details.actual, 65);
        assert_eq!(details.max, 64);
        assert!(limited.cancellation_details().is_none());

        let font_limit = classify_render_error(
            merman::svg::export::NativeFontError::Limit {
                resource: "label-bytes",
                actual: 97,
                limit: 96,
            }
            .into(),
            profile,
        );
        assert_eq!(font_limit.status(), BindingStatus::ResourceLimitExceeded);
        let details = font_limit
            .resource_details()
            .expect("native font ceiling details");
        assert_eq!(details.limit_id, "native_font_label_bytes");
        assert_eq!(details.actual, 97);
        assert_eq!(details.max, 96);

        let measurement = classify_render_error(
            merman::svg::export::NativeFontError::Measurement(
                "unsupported native typography".into(),
            )
            .into(),
            profile,
        );
        assert_eq!(measurement.status(), BindingStatus::RenderError);
        assert!(
            measurement
                .message()
                .contains("unsupported native typography")
        );
    }

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
            merman::model::resources::ResourceProfile::Interactive,
        );

        assert_eq!(error.status(), BindingStatus::ResourceLimitExceeded);
        assert!(error.message().contains("THEME_COLOR_LIMIT"));
        assert_eq!(error.resource_details(), None);
    }

    #[test]
    fn graphical_parse_errors_are_terminal_safe_and_structured() {
        let span = merman::model::SourceSpan::new(2, 7);
        let diagnostic = merman::ParseDiagnostic::new("bad\u{7}input")
            .with_span(span, merman::ParseDiagnosticSpanKind::Exact)
            .with_code("merman.test\u{1b}");
        let error = classify_render_error(
            merman::RenderError::from(merman::Error::diagram_parse_diagnostic(
                "flow\u{1b}",
                diagnostic,
            )),
            merman::model::resources::ResourceProfile::Interactive,
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
                "background": "transparent",
                "export_matte": "#123456",
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
        let config = compile_for_test(
            &options,
            merman::model::runtime::RuntimePolicy::deterministic(),
        )
        .expect("export options compile");
        assert_eq!(config.background, Some(merman::theme::Color::TRANSPARENT));
        assert_eq!(config.raster_options.matte.as_deref(), Some("#123456"));
        assert_eq!(config.pdf_options.page_paint.as_deref(), Some("#123456"));
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
        let config = compile_for_test(
            &options,
            merman::model::runtime::RuntimePolicy::deterministic(),
        )
        .expect("SVG options compile");

        assert_eq!(config.svg.diagram_id.as_deref(), Some("docs-flow"));
        assert_eq!(config.svg.viewbox_padding, 12.5);
    }

    #[cfg(feature = "svg")]
    #[test]
    fn svg_viewbox_padding_rejects_negative_or_unrepresentable_values() {
        let error = crate::common::parse_options(br#"{"svg":{"viewbox_padding":-1}}"#)
            .and_then(|options| {
                compile_for_test(
                    &options,
                    merman::model::runtime::RuntimePolicy::deterministic(),
                )
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
        let error = compile_for_test(
            &options,
            merman::model::runtime::RuntimePolicy::deterministic(),
        )
        .err()
        .expect("unsupported backend color");
        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert!(error.message().contains("raster.matte"));

        let error = crate::common::parse_options(br##"{"export_matte":"#12345678"}"##)
            .expect_err("export matte must be opaque");
        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(error.message().contains("export_matte must be opaque"));
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
            let error = compile_for_test(
                &options,
                merman::model::runtime::RuntimePolicy::deterministic(),
            )
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
            merman::model::resources::ResourceProfile::Constrained,
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
            merman::model::resources::ResourceProfile::Constrained,
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
                merman::model::runtime::RuntimePolicy::deterministic()
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
                merman::model::runtime::RuntimePolicy::deterministic(),
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
        let error = binding_runtime_policy_from(
            &options,
            merman::model::runtime::RuntimePolicy::deterministic(),
        )
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
            merman::model::runtime::RuntimePolicy::deterministic()
                .with_fixed_unix_millis(86_400_000),
        )
        .expect("binding time policy");
        let context = policy.begin_operation().expect("operation context");

        assert_eq!(context.unix_millis(), 86_400_000);
        assert_eq!(context.local_time_zone().fixed_offset_minutes(), Some(480));
    }

    #[test]
    fn fixed_unix_millis_derives_the_local_day_from_the_precise_instant() {
        let options = crate::common::parse_options(
            br#"{"fixed_unix_millis":-1,"fixed_local_offset_minutes":480}"#,
        )
        .expect("binding options");
        let previous = merman::model::runtime::RuntimePolicy::deterministic()
            .try_with_fixed_today_at_local_midnight("2026-06-10".parse().unwrap())
            .unwrap();
        let policy = binding_runtime_policy_from(&options, previous).expect("host clock policy");
        let context = policy.begin_operation().expect("host clock context");

        assert_eq!(context.unix_millis(), -1);
        assert_eq!(context.today_local().to_string(), "1970-01-01");
        assert!(!context.today_is_fixed());
        assert_eq!(context, policy.begin_operation().unwrap());
    }

    #[test]
    fn fixed_unix_millis_accepts_the_inclusive_javascript_date_range() {
        for millis in [-8_640_000_000_000_000_i64, 0, 8_640_000_000_000_000] {
            let options = crate::common::parse_options(
                format!(r#"{{"fixed_unix_millis":{millis}}}"#).as_bytes(),
            )
            .expect("TimeClip endpoint is accepted");
            let policy = binding_runtime_policy_from(
                &options,
                merman::model::runtime::RuntimePolicy::deterministic(),
            )
            .unwrap();
            assert_eq!(policy.begin_operation().unwrap().unix_millis(), millis);
        }
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
        let policy = binding_runtime_policy_from(
            &options,
            merman::model::runtime::RuntimePolicy::deterministic(),
        )
        .expect("binding time policy");

        let first = policy.begin_operation().expect("first operation");
        let second = policy.begin_operation().expect("second operation");

        assert_eq!(first, second);
        assert_eq!(first.unix_millis(), 1_781_046_000_000);
        assert_eq!(first.local_time_zone().fixed_offset_minutes(), Some(60));
    }
}
