use crate::common::{
    BindingError, BindingOptions, BindingStatus, binding_fixed_local_offset_minutes,
    binding_fixed_today, binding_site_config, css_declaration_value, finite_positive,
    internal_json_error, no_diagram_error, normalize_option,
};
use merman::render::{
    DeterministicTextMeasurer, HeadlessRenderer, LayoutOptions, VendoredFontMetricsTextMeasurer,
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

    pub(super) fn parse_json(&self, source: &str) -> Result<Vec<u8>, BindingError> {
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

    pub(super) fn validate(&self, source: &str) -> Result<(), BindingError> {
        self.parse_json(source).map(|_| ())
    }
}

#[cfg(test)]
pub(super) fn pipeline_for_options(
    options: &BindingOptions,
) -> Result<merman::render::SvgPipeline, BindingError> {
    Ok(build_renderer(options)?.1.into_pipeline())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PipelineKind {
    Parity,
    Readable,
    ResvgSafe,
}

impl Default for PipelineKind {
    fn default() -> Self {
        Self::Parity
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SvgPipelineOptions {
    kind: PipelineKind,
    scoped_css: Option<String>,
    css_override_policy: merman::render::CssOverridePolicy,
    root_background_color: Option<String>,
    drop_native_duplicate_fallbacks: bool,
}

impl Default for SvgPipelineOptions {
    fn default() -> Self {
        Self {
            kind: PipelineKind::default(),
            scoped_css: None,
            css_override_policy: merman::render::CssOverridePolicy::Preserve,
            root_background_color: None,
            drop_native_duplicate_fallbacks: false,
        }
    }
}

impl SvgPipelineOptions {
    fn into_pipeline(self) -> merman::render::SvgPipeline {
        let mut pipeline = match self.kind {
            PipelineKind::Parity => merman::render::SvgPipeline::parity(),
            PipelineKind::Readable => merman::render::SvgPipeline::readable(),
            PipelineKind::ResvgSafe => merman::render::SvgPipeline::resvg_safe(),
        };

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
    let mut renderer = HeadlessRenderer::new()
        .with_fixed_today(binding_fixed_today(options)?)
        .with_fixed_local_offset_minutes(binding_fixed_local_offset_minutes(options)?);

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

    if let Some(site_config) = binding_site_config(options)? {
        renderer = renderer.with_site_config(site_config);
    }

    let mut layout = LayoutOptions::headless_svg_defaults();
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

    let mut pipeline = SvgPipelineOptions::default();
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
                "preserve" => merman::render::CssOverridePolicy::Preserve,
                "strip-existing-important" | "strip_existing_important" => {
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

fn classify_render_error(err: merman::render::HeadlessError) -> BindingError {
    match err {
        merman::render::HeadlessError::Parse(err) => {
            BindingError::new(BindingStatus::ParseError, err.to_string())
        }
        merman::render::HeadlessError::Render(err) => {
            BindingError::new(BindingStatus::RenderError, err.to_string())
        }
    }
}
