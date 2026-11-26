use merman_model::MermaidConfig;

#[cfg(feature = "jpeg")]
use super::JpegRequest;
#[cfg(feature = "png")]
use super::PngRequest;
#[cfg(any(feature = "png", feature = "jpeg"))]
use super::RasterOutput;
use super::Renderer;
#[cfg(feature = "ascii")]
use super::{AsciiOutput, AsciiRequest};
#[cfg(any(feature = "svg", feature = "ascii"))]
use super::{OperationControl, RenderError, RenderOutput, RenderRequest, RenderTarget};
#[cfg(feature = "pdf")]
use super::{PdfOutput, PdfRequest};
#[cfg(feature = "svg")]
use super::{SvgEnvironment, SvgOutput, SvgRequest};
#[cfg(feature = "ascii")]
use crate::ascii::AsciiRenderOptions;
#[cfg(feature = "svg")]
use crate::svg::{RenderResourcePolicy, TextMeasurementPolicy};
#[cfg(any(feature = "svg", feature = "ascii"))]
use crate::theme::{Appearance, BuiltinPreset, ThemeSelection};
#[cfg(any(feature = "svg", feature = "ascii"))]
use merman_model::resources::{InputResourcePolicy, ResourceProfile};
#[cfg(any(feature = "svg", feature = "ascii"))]
use std::sync::Arc;

/// A default layout selection for families that accept a configurable backend.
///
/// The family still determines which backends it supports. Selection and capability admission
/// use the same rules as the Mermaid site config, including source overrides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LayoutBackend {
    Dagre,
    Elk,
    Cytoscape,
    Shoal,
}

impl LayoutBackend {
    const fn config_value(self) -> &'static str {
        match self {
            Self::Dagre => "dagre",
            Self::Elk => "elk",
            Self::Cytoscape => "cose-bilkent",
            Self::Shoal => "shoal",
        }
    }
}

/// Invalid defaults for the zero-configuration rendering methods.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RendererBuilderError {
    #[error("unbounded input limits must be selected on an explicit render operation")]
    UnboundedInputPolicy,
    #[cfg(feature = "svg")]
    #[error("unbounded graphical limits must be selected on an explicit render operation")]
    UnboundedRenderPolicy,
}

/// Configures site defaults and the zero-configuration methods on [`Renderer`].
///
/// Site configuration and the default layout apply to the renderer's parser and presentation
/// engine. One-argument typed methods use the configured target and input defaults. To keep the
/// target defaults when supplying an explicit request, start from the renderer's default request
/// for that target. `*_with` and [`Renderer::render`] take their target settings from the request.
#[derive(Debug, Default)]
#[must_use]
pub struct RendererBuilder {
    site_config: Option<MermaidConfig>,
    default_layout: Option<LayoutBackend>,
    #[cfg(any(feature = "svg", feature = "ascii"))]
    input_resources: Option<InputResourcePolicy>,
    #[cfg(any(feature = "svg", feature = "ascii"))]
    theme: Option<ThemeSelection>,
    #[cfg(feature = "svg")]
    svg_environment: Option<SvgEnvironment>,
    #[cfg(feature = "svg")]
    render_resources: Option<RenderResourcePolicy>,
    #[cfg(feature = "svg")]
    text_measurement: Option<TextMeasurementPolicy>,
    #[cfg(feature = "ascii")]
    ascii_options: Option<AsciiRenderOptions>,
}

#[cfg(any(feature = "svg", feature = "ascii"))]
#[derive(Debug)]
pub(super) struct ErgonomicDefaults {
    input_resources: Option<InputResourcePolicy>,
    #[cfg(feature = "svg")]
    svg: Option<Box<SvgRequest>>,
    #[cfg(feature = "ascii")]
    ascii: Option<Box<AsciiRequest>>,
}

#[cfg(any(feature = "svg", feature = "ascii"))]
impl std::panic::RefUnwindSafe for ErgonomicDefaults {}

#[cfg(any(feature = "svg", feature = "ascii"))]
impl ErgonomicDefaults {
    fn into_shared(self) -> Option<Arc<Self>> {
        let configured = self.input_resources.is_some();
        #[cfg(feature = "svg")]
        let configured = configured || self.svg.is_some();
        #[cfg(feature = "ascii")]
        let configured = configured || self.ascii.is_some();
        configured.then(|| Arc::new(self))
    }
}

impl RendererBuilder {
    /// Sets the host site configuration used by both parsing paths.
    pub fn with_site_config(mut self, config: MermaidConfig) -> Self {
        self.site_config = Some(config);
        self
    }

    /// Sets the root `layout` in the host site config, independent of setter order.
    ///
    /// A family-specific site setting or an allowed source setting can still override it.
    pub fn with_default_layout(mut self, backend: LayoutBackend) -> Self {
        self.default_layout = Some(backend);
        self
    }

    /// Sets input admission for the one-argument methods, ahead of any target environment.
    #[cfg(any(feature = "svg", feature = "ascii"))]
    pub fn with_input_resource_policy(mut self, policy: InputResourcePolicy) -> Self {
        self.input_resources = Some(policy);
        self
    }

    /// Selects a native theme for graphical and colored terminal output, including precompiled
    /// themes passed through [`ThemeSelection::compiled`].
    #[cfg(any(feature = "svg", feature = "ascii"))]
    pub fn with_native_theme(mut self, theme: ThemeSelection) -> Self {
        self.theme = Some(theme);
        self
    }

    /// Selects a built-in native theme and its default or explicit appearance.
    #[cfg(any(feature = "svg", feature = "ascii"))]
    pub fn with_theme_preset(self, preset: BuiltinPreset, appearance: Option<Appearance>) -> Self {
        self.with_native_theme(ThemeSelection::preset(preset, appearance))
    }

    /// Supplies reusable host services for the one-argument graphical methods.
    ///
    /// Target defaults are cloned into each operation. Hosts remain responsible for the
    /// panic recovery of their own callbacks and shared state.
    #[cfg(feature = "svg")]
    pub fn with_svg_environment(mut self, environment: SvgEnvironment) -> Self {
        self.svg_environment = Some(environment);
        self
    }

    /// Overrides the SVG environment's resource profile and limits.
    ///
    /// Its input policy also supplies graphical input limits unless overridden separately.
    #[cfg(feature = "svg")]
    pub fn with_render_resource_policy(mut self, policy: RenderResourcePolicy) -> Self {
        self.render_resources = Some(policy);
        self
    }

    /// Overrides the SVG environment's text measurement with an explicit host or built-in policy.
    #[cfg(feature = "svg")]
    pub fn with_text_measurement_policy(mut self, policy: TextMeasurementPolicy) -> Self {
        self.text_measurement = Some(policy);
        self
    }

    /// Sets terminal options for the one-argument ASCII method, including color mode.
    #[cfg(feature = "ascii")]
    pub fn with_ascii_options(mut self, options: AsciiRenderOptions) -> Self {
        self.ascii_options = Some(options);
        self
    }

    /// Builds one reusable renderer without initializing its presentation engine.
    ///
    /// Unbounded defaults are not accepted. Use an explicit request to render trusted input
    /// without those limits. Preset and layout admission still occur when rendering.
    pub fn build(mut self) -> Result<Renderer, RendererBuilderError> {
        #[cfg(any(feature = "svg", feature = "ascii"))]
        if self
            .input_resources
            .is_some_and(|policy| policy.profile() == ResourceProfile::UnboundedForTrustedInput)
        {
            return Err(RendererBuilderError::UnboundedInputPolicy);
        }

        #[cfg(feature = "svg")]
        let svg = if self.svg_environment.is_some()
            || self.render_resources.is_some()
            || self.text_measurement.is_some()
            || self.theme.is_some()
            || self.input_resources.is_some()
        {
            let mut request = SvgRequest::default();
            if let Some(environment) = self.svg_environment {
                request.environment = environment;
            }
            if let Some(policy) = self.render_resources {
                request.environment = request.environment.with_resource_policy(policy);
            }
            if request.environment.resource_policy().profile()
                == ResourceProfile::UnboundedForTrustedInput
            {
                return Err(RendererBuilderError::UnboundedRenderPolicy);
            }
            if let Some(policy) = self.input_resources {
                request.environment = request.environment.with_input_resource_policy(policy);
            }
            if request
                .environment
                .resource_policy()
                .input_policy()
                .profile()
                == ResourceProfile::UnboundedForTrustedInput
            {
                return Err(RendererBuilderError::UnboundedInputPolicy);
            }
            if let Some(policy) = self.text_measurement {
                request.environment = request.environment.with_text_measurement_policy(policy);
            }
            if let Some(theme) = &self.theme {
                request.environment.native_theme = Some(theme.clone());
            }
            Some(Box::new(request))
        } else {
            None
        };

        #[cfg(feature = "ascii")]
        let ascii = if self.ascii_options.is_some() || self.theme.is_some() {
            Some(Box::new(AsciiRequest {
                options: self.ascii_options.unwrap_or_default(),
                native_theme: self.theme,
                ..AsciiRequest::default()
            }))
        } else {
            None
        };

        if let Some(backend) = self.default_layout {
            self.site_config
                .get_or_insert_with(MermaidConfig::empty_object)
                .set_value("layout", backend.config_value().into());
        }
        let mut renderer = Renderer::new();
        if let Some(config) = self.site_config {
            renderer.parser = renderer.parser.with_site_config(config);
        }
        #[cfg(any(feature = "svg", feature = "ascii"))]
        {
            renderer.ergonomic_defaults = ErgonomicDefaults {
                input_resources: self.input_resources,
                #[cfg(feature = "svg")]
                svg,
                #[cfg(feature = "ascii")]
                ascii,
            }
            .into_shared();
        }
        Ok(renderer)
    }
}

#[cfg(any(feature = "svg", feature = "ascii"))]
macro_rules! require_output {
    ($result:expr, $variant:ident, $target:literal) => {
        match $result? {
            RenderOutput::$variant(output) => output.ok_or(RenderError::NoDiagram),
            _ => Err(RenderError::UnsupportedTarget($target)),
        }
    };
}

impl Renderer {
    /// Starts configuring a renderer. The default policies are bounded.
    pub fn builder() -> RendererBuilder {
        RendererBuilder::default()
    }

    #[cfg(any(feature = "svg", feature = "ascii"))]
    fn render_typed_target(
        &self,
        mut request: RenderRequest<'_>,
    ) -> Result<RenderOutput, RenderError> {
        request.allow_missing_diagram = true;
        self.render(request)
    }

    #[cfg(any(feature = "svg", feature = "ascii"))]
    fn render_default_target(
        &self,
        source: &str,
        target: RenderTarget,
    ) -> Result<RenderOutput, RenderError> {
        let policy = self
            .ergonomic_defaults
            .as_deref()
            .and_then(|defaults| defaults.input_resources)
            .or_else(|| {
                if self.resources.profile() != ResourceProfile::UnboundedForTrustedInput {
                    return None;
                }
                #[cfg(feature = "svg")]
                let bounded = target.svg_input_resource_policy().unwrap_or_default();
                #[cfg(not(feature = "svg"))]
                let bounded = InputResourcePolicy::default();
                Some(bounded.meet(&self.resources))
            });
        let mut request = RenderRequest::new(source, target, OperationControl::new());
        if let Some(policy) = policy {
            request = request.with_resource_policy(policy);
        }
        self.render_typed_target(request)
    }

    /// Copies the SVG target used by [`Self::render_svg`], including the configured theme and
    /// environment. Change only the fields you want to override before calling
    /// [`Self::render_svg_with`]. The copy also supplies the SVG settings for raster and PDF requests.
    #[cfg(feature = "svg")]
    pub fn default_svg_request(&self) -> SvgRequest {
        self.ergonomic_defaults
            .as_deref()
            .and_then(|defaults| defaults.svg.as_deref())
            .cloned()
            .unwrap_or_default()
    }

    /// Renders SVG with bounded defaults. Missing diagrams return [`RenderError::NoDiagram`].
    #[cfg(feature = "svg")]
    pub fn render_svg(&self, source: &str) -> Result<SvgOutput, RenderError> {
        let target = RenderTarget::Svg(self.default_svg_request());
        require_output!(self.render_default_target(source, target), Svg, "svg")
    }

    /// Renders SVG using the supplied target request and operation control.
    /// Use [`Self::default_svg_request`] to keep the builder's target defaults.
    #[cfg(feature = "svg")]
    pub fn render_svg_with(
        &self,
        source: &str,
        request: SvgRequest,
        control: OperationControl,
    ) -> Result<SvgOutput, RenderError> {
        require_output!(
            self.render_typed_target(RenderRequest::svg(source, control, request)),
            Svg,
            "svg"
        )
    }

    /// Copies the ASCII target used by [`Self::render_ascii`], including the configured terminal
    /// options and theme. For explicit operation-level input budgets, use [`RenderRequest`].
    #[cfg(feature = "ascii")]
    pub fn default_ascii_request(&self) -> AsciiRequest {
        self.ergonomic_defaults
            .as_deref()
            .and_then(|defaults| defaults.ascii.as_deref())
            .cloned()
            .unwrap_or_default()
    }

    /// Renders terminal text and its report with bounded defaults.
    #[cfg(feature = "ascii")]
    pub fn render_ascii(&self, source: &str) -> Result<AsciiOutput, RenderError> {
        let request = self.default_ascii_request();
        require_output!(
            self.render_default_target(source, RenderTarget::Ascii(request)),
            Ascii,
            "ascii"
        )
    }

    /// Renders terminal text using the supplied target request and operation control.
    /// Use [`Self::default_ascii_request`] to keep the builder's target defaults.
    #[cfg(feature = "ascii")]
    pub fn render_ascii_with(
        &self,
        source: &str,
        request: AsciiRequest,
        control: OperationControl,
    ) -> Result<AsciiOutput, RenderError> {
        require_output!(
            self.render_typed_target(RenderRequest::ascii(source, control, request)),
            Ascii,
            "ascii"
        )
    }

    /// Renders PNG with bounded graphical and raster export defaults.
    #[cfg(feature = "png")]
    pub fn render_png(&self, source: &str) -> Result<RasterOutput, RenderError> {
        let target = RenderTarget::Png(PngRequest {
            svg: self.default_svg_request(),
            options: Default::default(),
        });
        require_output!(self.render_default_target(source, target), Png, "png")
    }

    /// Renders PNG using the supplied target request and operation control.
    #[cfg(feature = "png")]
    pub fn render_png_with(
        &self,
        source: &str,
        request: PngRequest,
        control: OperationControl,
    ) -> Result<RasterOutput, RenderError> {
        require_output!(
            self.render_typed_target(RenderRequest::png(source, control, request)),
            Png,
            "png"
        )
    }

    /// Renders JPEG with bounded graphical and raster export defaults.
    #[cfg(feature = "jpeg")]
    pub fn render_jpeg(&self, source: &str) -> Result<RasterOutput, RenderError> {
        let target = RenderTarget::Jpeg(JpegRequest {
            svg: self.default_svg_request(),
            options: Default::default(),
        });
        require_output!(self.render_default_target(source, target), Jpeg, "jpeg")
    }

    /// Renders JPEG using the supplied target request and operation control.
    #[cfg(feature = "jpeg")]
    pub fn render_jpeg_with(
        &self,
        source: &str,
        request: JpegRequest,
        control: OperationControl,
    ) -> Result<RasterOutput, RenderError> {
        require_output!(
            self.render_typed_target(RenderRequest::jpeg(source, control, request)),
            Jpeg,
            "jpeg"
        )
    }

    /// Renders vector PDF with bounded graphical and PDF export defaults.
    #[cfg(feature = "pdf")]
    pub fn render_pdf(&self, source: &str) -> Result<PdfOutput, RenderError> {
        let target = RenderTarget::Pdf(PdfRequest {
            svg: self.default_svg_request(),
            options: Default::default(),
        });
        require_output!(self.render_default_target(source, target), Pdf, "pdf")
    }

    /// Renders PDF using the supplied target request and operation control.
    #[cfg(feature = "pdf")]
    pub fn render_pdf_with(
        &self,
        source: &str,
        request: PdfRequest,
        control: OperationControl,
    ) -> Result<PdfOutput, RenderError> {
        require_output!(
            self.render_typed_target(RenderRequest::pdf(source, control, request)),
            Pdf,
            "pdf"
        )
    }
}
