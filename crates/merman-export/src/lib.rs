#![forbid(unsafe_code)]

//! Bounded binary export for SVG that has passed Merman's terminal compatibility validation.
//!
//! This crate deliberately accepts [`ResvgCompatibleSvg`] rather than Mermaid source or an
//! arbitrary SVG string. Parsing, semantic construction, layout, SVG production, and terminal
//! SVG validation stay owned by `merman`; this crate only owns allocation-aware encoding.

#[cfg(any(feature = "png", feature = "jpeg"))]
use cssparser::{Delimiter, Parser, ParserInput, Token};
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use merman_render::svg::ResvgCompatibleSvg;
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use std::sync::{Arc, OnceLock};

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("failed to parse SVG")]
    SvgParse,
    #[error("failed to set SVG Document size from tree")]
    SvgDocSize,
    #[error("failed to allocate pixmap for raster rendering")]
    PixmapAlloc,
    #[error("invalid raster scale; expected a finite positive number")]
    InvalidScale,
    #[error("invalid raster sizing option: {0}")]
    InvalidSizing(&'static str),
    #[error("failed to encode PNG")]
    PngEncode,
    #[error("invalid background color for JPG rendering")]
    JpegBackground,
    #[error("JPG rendering requires an opaque background color (e.g. white)")]
    JpegOpaqueBackgroundRequired,
    #[error("failed to encode JPG")]
    JpegEncode,
    #[error("JPG dimensions exceed the 65535-pixel encoder limit")]
    JpegDimensionLimit,
    #[error("failed to convert SVG to PDF")]
    PdfConvert,
    #[error("embedded image resource limit exceeded: {limit_name} is {actual}, maximum is {max}")]
    EmbeddedImageLimit {
        limit_name: &'static str,
        actual: u64,
        max: u64,
    },
    #[error("SVG conversion resource limit exceeded: {limit_name} is {actual}, maximum is {max}")]
    SvgConversionLimit {
        limit_name: &'static str,
        actual: u64,
        max: u64,
    },
    #[error("failed to start the recursive SVG backend worker")]
    BackendWorkerSpawn,
    #[error("the recursive SVG backend worker panicked")]
    BackendWorkerPanic,
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub type Result<T> = std::result::Result<T, ExportError>;

#[cfg(any(feature = "png", feature = "jpeg"))]
pub const DEFAULT_MAX_RASTER_SIDE_LENGTH: u32 = 4096;
#[cfg(any(feature = "png", feature = "jpeg"))]
pub const DEFAULT_MAX_RASTER_PIXELS: u64 =
    (DEFAULT_MAX_RASTER_SIDE_LENGTH as u64) * (DEFAULT_MAX_RASTER_SIDE_LENGTH as u64);
/// Aggregate pixels retained as localized PDF filter images before sampling is reduced.
#[cfg(feature = "pdf")]
pub const DEFAULT_MAX_PDF_FILTER_IMAGE_PIXELS: u64 = 32 * 1024 * 1024;
/// Maximum intrinsic pixels accepted for one embedded raster image by default.
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub const DEFAULT_MAX_DECODED_IMAGE_PIXELS: u64 = 16 * 1024 * 1024;
/// Maximum aggregate intrinsic pixels accepted across embedded raster images by default.
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub const DEFAULT_MAX_TOTAL_DECODED_IMAGE_PIXELS: u64 = 32 * 1024 * 1024;
/// Maximum decoded data-URL bytes accepted for one embedded image before usvg parsing.
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub const DEFAULT_MAX_EMBEDDED_IMAGE_BYTES: u64 = 16 * 1024 * 1024;
/// Maximum aggregate decoded data-URL bytes accepted before usvg parsing.
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub const DEFAULT_MAX_TOTAL_EMBEDDED_IMAGE_BYTES: u64 = 32 * 1024 * 1024;
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub const DEFAULT_MAX_SVG_ISOLATION_DEPTH: usize = 8;
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub const DEFAULT_MAX_FILTER_PRIMITIVES_PER_FILTER: usize = 8;
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub const DEFAULT_MAX_TOTAL_FILTER_PRIMITIVES: usize = 128;
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub const DEFAULT_MAX_SVG_SUBROOTS: usize = 4096;
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub const DEFAULT_MAX_NESTED_SVG_IMAGES: usize = 64;

#[cfg(feature = "pdf")]
const PDF_POINTS_PER_CSS_PIXEL: f32 = 72.0 / 96.0;
#[cfg(feature = "pdf")]
const KRILLA_MAX_FILTER_SIDE_PX: f64 = 5000.0;
#[cfg(all(
    any(feature = "png", feature = "jpeg", feature = "pdf"),
    not(target_arch = "wasm32")
))]
const RECURSIVE_SVG_BACKEND_STACK_BYTES: usize = 8 * 1024 * 1024;

#[cfg(all(
    any(feature = "png", feature = "jpeg", feature = "pdf"),
    target_arch = "wasm32"
))]
const RECURSIVE_SVG_BACKEND_STACK_BYTES: usize = 0;

#[cfg(all(
    any(feature = "png", feature = "jpeg", feature = "pdf"),
    not(target_arch = "wasm32")
))]
fn run_recursive_svg_backend<T, F>(job: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    std::thread::Builder::new()
        .name("merman-svg-backend".to_string())
        .stack_size(RECURSIVE_SVG_BACKEND_STACK_BYTES)
        .spawn(job)
        .map_err(|_| ExportError::BackendWorkerSpawn)?
        .join()
        .map_err(|_| ExportError::BackendWorkerPanic)?
}

#[cfg(all(
    any(feature = "png", feature = "jpeg", feature = "pdf"),
    target_arch = "wasm32"
))]
fn run_recursive_svg_backend<T, F>(job: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    job()
}

/// Optional display box for target-aware rasterization.
///
/// Browser previews typically draw Mermaid SVG as vector content inside a container. A headless
/// rasterizer has to allocate a full pixmap, so UI hosts should pass the visible container size
/// here and use [`RasterOptions::scale`] for device-pixel ratio.
#[cfg(any(feature = "png", feature = "jpeg"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RasterFitBox {
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[cfg(any(feature = "png", feature = "jpeg"))]
impl RasterFitBox {
    pub const fn new(width: Option<u32>, height: Option<u32>) -> Self {
        Self { width, height }
    }

    pub const fn width(width: u32) -> Self {
        Self {
            width: Some(width),
            height: None,
        }
    }

    pub const fn height(height: u32) -> Self {
        Self {
            width: None,
            height: Some(height),
        }
    }

    pub const fn contain(width: u32, height: u32) -> Self {
        Self {
            width: Some(width),
            height: Some(height),
        }
    }
}

/// Resource budget applied before allocating the output pixmap.
#[cfg(any(feature = "png", feature = "jpeg"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RasterSizeLimit {
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    pub max_pixels: Option<u64>,
}

#[cfg(any(feature = "png", feature = "jpeg"))]
impl RasterSizeLimit {
    pub const fn new(
        max_width: Option<u32>,
        max_height: Option<u32>,
        max_pixels: Option<u64>,
    ) -> Self {
        Self {
            max_width,
            max_height,
            max_pixels,
        }
    }

    pub const fn max_side_length(max_side_length: u32) -> Self {
        Self {
            max_width: Some(max_side_length),
            max_height: Some(max_side_length),
            max_pixels: None,
        }
    }

    pub const fn default_safe() -> Self {
        Self {
            max_width: Some(DEFAULT_MAX_RASTER_SIDE_LENGTH),
            max_height: Some(DEFAULT_MAX_RASTER_SIDE_LENGTH),
            max_pixels: Some(DEFAULT_MAX_RASTER_PIXELS),
        }
    }

    pub const fn unbounded() -> Self {
        Self {
            max_width: None,
            max_height: None,
            max_pixels: None,
        }
    }
}

#[cfg(any(feature = "png", feature = "jpeg"))]
impl Default for RasterSizeLimit {
    fn default() -> Self {
        Self::default_safe()
    }
}

#[cfg(any(feature = "png", feature = "jpeg"))]
#[derive(Debug, Clone)]
pub struct RasterOptions {
    pub scale: f32,
    pub background: Option<String>,
    pub jpeg_quality: u8,
    pub fit_to: Option<RasterFitBox>,
    pub size_limit: RasterSizeLimit,
    pub embedded_image_limit: EmbeddedImageLimit,
    pub conversion_limits: SvgConversionLimits,
}

/// Resource budget checked before and after usvg resolves embedded images.
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbeddedImageLimit {
    pub max_bytes_per_image: Option<u64>,
    pub max_total_bytes: Option<u64>,
    pub max_pixels_per_image: Option<u64>,
    pub max_total_pixels: Option<u64>,
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
impl EmbeddedImageLimit {
    pub const fn new(
        max_bytes_per_image: Option<u64>,
        max_total_bytes: Option<u64>,
        max_pixels_per_image: Option<u64>,
        max_total_pixels: Option<u64>,
    ) -> Self {
        Self {
            max_bytes_per_image,
            max_total_bytes,
            max_pixels_per_image,
            max_total_pixels,
        }
    }

    pub const fn default_safe() -> Self {
        Self {
            max_bytes_per_image: Some(DEFAULT_MAX_EMBEDDED_IMAGE_BYTES),
            max_total_bytes: Some(DEFAULT_MAX_TOTAL_EMBEDDED_IMAGE_BYTES),
            max_pixels_per_image: Some(DEFAULT_MAX_DECODED_IMAGE_PIXELS),
            max_total_pixels: Some(DEFAULT_MAX_TOTAL_DECODED_IMAGE_PIXELS),
        }
    }

    pub const fn unbounded() -> Self {
        Self {
            max_bytes_per_image: None,
            max_total_bytes: None,
            max_pixels_per_image: None,
            max_total_pixels: None,
        }
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
impl Default for EmbeddedImageLimit {
    fn default() -> Self {
        Self::default_safe()
    }
}

/// Structural limits checked on the parsed usvg tree before resvg or krilla-svg recurse through
/// it. These limits complement output-pixel and embedded-image budgets; they do not claim to be a
/// byte-exact model of third-party allocator behavior.
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SvgConversionLimits {
    pub max_isolation_depth: Option<usize>,
    pub max_filter_primitives_per_filter: Option<usize>,
    pub max_total_filter_primitives: Option<usize>,
    pub max_subroots: Option<usize>,
    pub max_nested_svg_images: Option<usize>,
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
impl SvgConversionLimits {
    pub const fn default_safe() -> Self {
        Self {
            max_isolation_depth: Some(DEFAULT_MAX_SVG_ISOLATION_DEPTH),
            max_filter_primitives_per_filter: Some(DEFAULT_MAX_FILTER_PRIMITIVES_PER_FILTER),
            max_total_filter_primitives: Some(DEFAULT_MAX_TOTAL_FILTER_PRIMITIVES),
            max_subroots: Some(DEFAULT_MAX_SVG_SUBROOTS),
            max_nested_svg_images: Some(DEFAULT_MAX_NESTED_SVG_IMAGES),
        }
    }

    pub const fn unbounded() -> Self {
        Self {
            max_isolation_depth: None,
            max_filter_primitives_per_filter: None,
            max_total_filter_primitives: None,
            max_subroots: None,
            max_nested_svg_images: None,
        }
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
impl Default for SvgConversionLimits {
    fn default() -> Self {
        Self::default_safe()
    }
}

/// Controls how vector content is placed on a PDF page.
#[cfg(feature = "pdf")]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum PdfPagePolicy {
    /// Use the SVG's intrinsic dimensions as the PDF page dimensions.
    #[default]
    FitSvg,
    /// Scale the SVG uniformly to fit a fixed page and center it without cropping.
    Fixed { width_pt: f32, height_pt: f32 },
    /// Match browser PDF sizing: constrain responsive SVG width in CSS pixels, then convert
    /// CSS pixels to PDF points at 96 CSS pixels per inch.
    FitCssWidth { max_width_px: f32 },
}

/// Aggregate pixel budget for localized filter images retained in a vector PDF.
#[cfg(feature = "pdf")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PdfFilterImageLimit {
    pub max_total_pixels: Option<u64>,
}

#[cfg(feature = "pdf")]
impl PdfFilterImageLimit {
    pub const fn new(max_total_pixels: Option<u64>) -> Self {
        Self { max_total_pixels }
    }

    pub const fn default_safe() -> Self {
        Self {
            max_total_pixels: Some(DEFAULT_MAX_PDF_FILTER_IMAGE_PIXELS),
        }
    }

    pub const fn unbounded() -> Self {
        Self {
            max_total_pixels: None,
        }
    }
}

#[cfg(feature = "pdf")]
impl Default for PdfFilterImageLimit {
    fn default() -> Self {
        Self::default_safe()
    }
}

/// Vector PDF conversion options, intentionally separate from pixel allocation limits.
#[cfg(feature = "pdf")]
#[derive(Debug, Clone, PartialEq)]
pub struct PdfOptions {
    /// PDF page sizing and content placement policy.
    pub page_policy: PdfPagePolicy,
    /// Optional page background color.
    pub background: Option<String>,
    /// Requested sampling scale for SVG filters embedded in the PDF.
    pub filter_scale: f32,
    /// Aggregate budget for localized filter images retained in the PDF.
    pub filter_image_limit: PdfFilterImageLimit,
    /// Header-derived pixel budget for embedded PNG/JPEG/GIF/WebP images.
    pub embedded_image_limit: EmbeddedImageLimit,
    /// Structural budget shared with PNG/JPEG conversion.
    pub conversion_limits: SvgConversionLimits,
}

#[cfg(feature = "pdf")]
impl Default for PdfOptions {
    fn default() -> Self {
        Self {
            page_policy: PdfPagePolicy::FitSvg,
            background: None,
            filter_scale: 4.0,
            filter_image_limit: PdfFilterImageLimit::default(),
            embedded_image_limit: EmbeddedImageLimit::default(),
            conversion_limits: SvgConversionLimits::default(),
        }
    }
}

#[cfg(feature = "pdf")]
impl PdfOptions {
    pub fn with_page_policy(mut self, page_policy: PdfPagePolicy) -> Self {
        self.page_policy = page_policy;
        self
    }

    pub fn with_background(mut self, background: impl Into<String>) -> Self {
        self.background = Some(background.into());
        self
    }

    pub fn with_filter_scale(mut self, filter_scale: f32) -> Self {
        self.filter_scale = filter_scale;
        self
    }

    pub fn with_filter_image_limit(mut self, filter_image_limit: PdfFilterImageLimit) -> Self {
        self.filter_image_limit = filter_image_limit;
        self
    }

    pub fn with_unbounded_filter_images(mut self) -> Self {
        self.filter_image_limit = PdfFilterImageLimit::unbounded();
        self
    }

    pub fn with_embedded_image_limit(mut self, embedded_image_limit: EmbeddedImageLimit) -> Self {
        self.embedded_image_limit = embedded_image_limit;
        self
    }

    pub fn with_conversion_limits(mut self, conversion_limits: SvgConversionLimits) -> Self {
        self.conversion_limits = conversion_limits;
        self
    }
}

#[cfg(any(feature = "png", feature = "jpeg"))]
impl Default for RasterOptions {
    fn default() -> Self {
        Self {
            scale: 1.0,
            background: None,
            jpeg_quality: 90,
            fit_to: None,
            size_limit: RasterSizeLimit::default(),
            embedded_image_limit: EmbeddedImageLimit::default(),
            conversion_limits: SvgConversionLimits::default(),
        }
    }
}

#[cfg(any(feature = "png", feature = "jpeg"))]
impl RasterOptions {
    pub fn with_scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    pub fn with_background(mut self, background: impl Into<String>) -> Self {
        self.background = Some(background.into());
        self
    }

    pub fn with_fit_to(mut self, fit_to: RasterFitBox) -> Self {
        self.fit_to = Some(fit_to);
        self
    }

    pub fn with_size_limit(mut self, size_limit: RasterSizeLimit) -> Self {
        self.size_limit = size_limit;
        self
    }

    pub fn with_unbounded_size(mut self) -> Self {
        self.size_limit = RasterSizeLimit::unbounded();
        self
    }

    pub fn with_embedded_image_limit(mut self, embedded_image_limit: EmbeddedImageLimit) -> Self {
        self.embedded_image_limit = embedded_image_limit;
        self
    }

    pub fn with_conversion_limits(mut self, conversion_limits: SvgConversionLimits) -> Self {
        self.conversion_limits = conversion_limits;
        self
    }
}

#[cfg(any(feature = "png", feature = "jpeg"))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RasterPlan {
    pub requested_width_px: f64,
    pub requested_height_px: f64,
    pub width_px: u32,
    pub height_px: u32,
    pub requested_scale: f64,
    pub effective_scale: f64,
    pub limited: bool,
}

/// Plan for localized SVG filter images embedded in a vector PDF.
#[cfg(feature = "pdf")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PdfFilterImagePlan {
    pub filtered_groups: usize,
    pub requested_scale: f32,
    pub effective_scale: f32,
    pub requested_image_pixels: u64,
    pub effective_image_pixels: u64,
    pub limited: bool,
}

/// Preflight facts for embedded image resources and decoded raster images.
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbeddedImagePlan {
    pub data_resources: usize,
    pub raster_images: usize,
    pub largest_data_bytes: u64,
    pub total_data_bytes: u64,
    pub largest_raster_pixels: u64,
    pub total_pixels: u64,
}

/// Structural work discovered in the usvg tree before a recursive backend is entered.
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SvgConversionPlan {
    /// Maximum resolved `usvg` group depth observed before backend conversion.
    pub max_tree_depth: usize,
    pub max_isolation_depth: usize,
    pub filtered_groups: usize,
    pub filter_primitives: usize,
    pub subroots: usize,
    pub nested_svg_images: usize,
}

/// Parsed raster input shared by sizing, memory scheduling, and image encoding.
#[cfg(any(feature = "png", feature = "jpeg"))]
pub struct PreparedRaster {
    tree: usvg::Tree,
    geometry: RasterGeometry,
    translate_min_to_origin: bool,
    plan: RasterPlan,
    embedded_image_plan: EmbeddedImagePlan,
    conversion_plan: SvgConversionPlan,
    options: RasterOptions,
}

#[cfg(any(feature = "png", feature = "jpeg"))]
impl PreparedRaster {
    /// Returns the allocation plan computed before any output pixmap is created.
    pub const fn plan(&self) -> RasterPlan {
        self.plan
    }

    /// Returns the embedded raster image plan computed from image headers.
    pub const fn embedded_image_plan(&self) -> EmbeddedImagePlan {
        self.embedded_image_plan
    }

    pub const fn conversion_plan(&self) -> SvgConversionPlan {
        self.conversion_plan
    }

    /// Returns an advisory weight for scheduling parallel PNG jobs.
    ///
    /// This is not a hard memory bound for resvg internals.
    pub fn png_scheduling_weight_bytes(&self) -> u64 {
        encoding_scheduling_weight_bytes(self.plan, self.embedded_image_plan, 8)
    }

    /// Returns an advisory weight for scheduling parallel JPEG jobs.
    ///
    /// This is not a hard memory bound for resvg internals.
    pub fn jpeg_scheduling_weight_bytes(&self) -> u64 {
        encoding_scheduling_weight_bytes(self.plan, self.embedded_image_plan, 10)
    }

    /// Allocates and encodes the prepared image as PNG.
    #[cfg(feature = "png")]
    pub fn encode_png(self) -> Result<Vec<u8>> {
        run_recursive_svg_backend(move || {
            self.into_pixmap()?
                .encode_png()
                .map_err(|_| ExportError::PngEncode)
        })
    }

    /// Allocates and encodes the prepared image as JPEG.
    #[cfg(feature = "jpeg")]
    pub fn encode_jpeg(mut self) -> Result<Vec<u8>> {
        run_recursive_svg_backend(move || {
            if self.plan.width_px > u32::from(u16::MAX) || self.plan.height_px > u32::from(u16::MAX)
            {
                return Err(ExportError::JpegDimensionLimit);
            }
            let bg = self.options.background.as_deref().unwrap_or("white");
            let Some(color) = parse_tiny_skia_color(bg) else {
                return Err(ExportError::JpegBackground);
            };
            if color.alpha() != 1.0 {
                return Err(ExportError::JpegOpaqueBackgroundRequired);
            }

            self.options.background = Some(bg.to_string());
            let quality = self.options.jpeg_quality;
            let pixmap = self.into_pixmap()?;
            let (w, h) = (pixmap.width(), pixmap.height());
            let rgba = pixmap.data();
            let mut rgb = vec![0u8; (w as usize) * (h as usize) * 3];
            for (src, dst) in rgba.chunks_exact(4).zip(rgb.chunks_exact_mut(3)) {
                dst[0] = src[0];
                dst[1] = src[1];
                dst[2] = src[2];
            }

            let mut out = Vec::new();
            let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
            enc.encode(&rgb, w, h, image::ExtendedColorType::Rgb8)
                .map_err(|_| ExportError::JpegEncode)?;
            Ok(out)
        })
    }

    fn into_pixmap(self) -> Result<tiny_skia::Pixmap> {
        let mut pixmap = tiny_skia::Pixmap::new(self.plan.width_px, self.plan.height_px)
            .ok_or(ExportError::PixmapAlloc)?;

        if let Some(bg) = self.options.background.as_deref()
            && let Some(color) = parse_tiny_skia_color(bg)
        {
            pixmap.fill(color);
        }

        let scale = self.plan.effective_scale as f32;
        let transform = if self.translate_min_to_origin {
            tiny_skia::Transform::from_row(
                scale,
                0.0,
                0.0,
                scale,
                -self.geometry.min_x * scale,
                -self.geometry.min_y * scale,
            )
        } else {
            tiny_skia::Transform::from_scale(scale, scale)
        };

        resvg::render(&self.tree, transform, &mut pixmap.as_mut());
        Ok(pixmap)
    }
}

/// Parsed vector PDF input shared by page planning, memory scheduling, and encoding.
#[cfg(feature = "pdf")]
pub struct PreparedPdf {
    tree: usvg::Tree,
    options: PdfOptions,
    filter_plan: PdfFilterImagePlan,
    embedded_image_plan: EmbeddedImagePlan,
    conversion_plan: SvgConversionPlan,
}

#[cfg(feature = "pdf")]
impl PreparedPdf {
    /// Returns the localized filter allocation plan computed before PDF encoding.
    pub const fn filter_plan(&self) -> PdfFilterImagePlan {
        self.filter_plan
    }

    /// Returns the embedded raster image plan computed from image headers.
    pub const fn embedded_image_plan(&self) -> EmbeddedImagePlan {
        self.embedded_image_plan
    }

    pub const fn conversion_plan(&self) -> SvgConversionPlan {
        self.conversion_plan
    }

    /// Returns an advisory weight for scheduling parallel PDF jobs.
    ///
    /// This is not a hard memory bound for krilla-svg or resvg internals.
    pub fn scheduling_weight_bytes(&self) -> u64 {
        const PDF_ENCODER_OVERHEAD_BYTES: u64 = 1024 * 1024;
        self.filter_plan
            .effective_image_pixels
            .saturating_mul(8)
            .saturating_add(self.embedded_image_plan.total_pixels.saturating_mul(8))
            .saturating_add(PDF_ENCODER_OVERHEAD_BYTES)
            .saturating_add(RECURSIVE_SVG_BACKEND_STACK_BYTES as u64)
    }

    /// Encodes the prepared tree as vector PDF, rasterizing only SVG filter regions.
    pub fn encode(self) -> Result<Vec<u8>> {
        run_recursive_svg_backend(move || svg_tree_to_pdf(&self.tree, &self.options))
    }
}

/// Parses a sealed SVG once and prepares its bounded raster allocation plan.
#[cfg(any(feature = "png", feature = "jpeg"))]
pub fn prepare_raster(svg: &ResvgCompatibleSvg, options: &RasterOptions) -> Result<PreparedRaster> {
    let source = svg.as_str().to_owned();
    let options = options.clone();
    run_recursive_svg_backend(move || prepare_raster_on_backend_stack(&source, &options))
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn prepare_raster_on_backend_stack(
    source: &str,
    options: &RasterOptions,
) -> Result<PreparedRaster> {
    let root_metadata = parse_root_svg_metadata(source)?;
    let mut usvg_options = usvg::Options::default();
    configure_usvg_options_for_raster(&mut usvg_options, root_metadata);
    let data_plan = plan_embedded_data_resources(source, options.embedded_image_limit)?;
    let tree = usvg::Tree::from_str(source, &usvg_options).map_err(|_| ExportError::SvgParse)?;
    let conversion_plan = plan_svg_conversion(&tree, options.conversion_limits)?;
    let embedded_image_plan = plan_embedded_images(&tree, options.embedded_image_limit, data_plan)?;
    let (geometry, translate_min_to_origin) = raster_geometry_for_svg(root_metadata, &tree);
    let plan = raster_plan_for_geometry(geometry, options)?;

    Ok(PreparedRaster {
        tree,
        geometry,
        translate_min_to_origin,
        plan,
        embedded_image_plan,
        conversion_plan,
        options: options.clone(),
    })
}

/// Parses a sealed SVG once and prepares vector PDF page and filter allocation policy.
#[cfg(feature = "pdf")]
pub fn prepare_pdf(svg: &ResvgCompatibleSvg, options: &PdfOptions) -> Result<PreparedPdf> {
    let source = svg.as_str().to_owned();
    let options = options.clone();
    run_recursive_svg_backend(move || prepare_pdf_on_backend_stack(&source, &options))
}

#[cfg(feature = "pdf")]
fn prepare_pdf_on_backend_stack(source: &str, options: &PdfOptions) -> Result<PreparedPdf> {
    validate_pdf_options(options)?;
    let data_plan = plan_embedded_data_resources(source, options.embedded_image_limit)?;
    let tree = parse_pdf_tree(source)?;
    let conversion_plan = plan_svg_conversion(&tree, options.conversion_limits)?;
    let embedded_image_plan = plan_embedded_images(&tree, options.embedded_image_limit, data_plan)?;
    let svg_size = pdf_svg_size(&tree)?;
    let layout = pdf_page_layout(svg_size, options.page_policy)?;
    let filter_plan = plan_pdf_filter_images(
        &tree,
        layout.drawing_size.width() / svg_size.width(),
        options.filter_scale,
        options.filter_image_limit,
    )?;
    let mut effective_options = options.clone();
    effective_options.filter_scale = filter_plan.effective_scale;

    Ok(PreparedPdf {
        tree,
        options: effective_options,
        filter_plan,
        embedded_image_plan,
        conversion_plan,
    })
}

#[cfg(feature = "pdf")]
fn validate_pdf_options(options: &PdfOptions) -> Result<()> {
    if !(options.filter_scale.is_finite() && options.filter_scale > 0.0) {
        return Err(ExportError::InvalidSizing(
            "PDF filter_scale must be finite and positive",
        ));
    }
    if options.filter_image_limit.max_total_pixels == Some(0) {
        return Err(ExportError::InvalidSizing(
            "PDF filter max_total_pixels must be positive",
        ));
    }
    validate_embedded_image_limit(options.embedded_image_limit)?;
    Ok(())
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn encoding_scheduling_weight_bytes(
    plan: RasterPlan,
    embedded_images: EmbeddedImagePlan,
    bytes_per_pixel: u64,
) -> u64 {
    const ENCODER_OVERHEAD_BYTES: u64 = 1024 * 1024;
    u64::from(plan.width_px)
        .saturating_mul(u64::from(plan.height_px))
        .saturating_mul(bytes_per_pixel)
        .saturating_add(embedded_images.total_pixels.saturating_mul(8))
        .saturating_add(ENCODER_OVERHEAD_BYTES)
        .saturating_add(RECURSIVE_SVG_BACKEND_STACK_BYTES as u64)
}

#[cfg(feature = "png")]
pub fn svg_to_png(svg: &ResvgCompatibleSvg, options: &RasterOptions) -> Result<Vec<u8>> {
    prepare_raster(svg, options)?.encode_png()
}

#[cfg(feature = "jpeg")]
pub fn svg_to_jpeg(svg: &ResvgCompatibleSvg, options: &RasterOptions) -> Result<Vec<u8>> {
    prepare_raster(svg, options)?.encode_jpeg()
}

#[cfg(any(feature = "png", feature = "jpeg"))]
pub fn svg_raster_plan(svg: &ResvgCompatibleSvg, options: &RasterOptions) -> Result<RasterPlan> {
    Ok(prepare_raster(svg, options)?.plan())
}

#[cfg(feature = "pdf")]
pub fn svg_to_pdf(svg: &ResvgCompatibleSvg) -> Result<Vec<u8>> {
    svg_to_pdf_with_options(svg, &PdfOptions::default())
}

#[cfg(feature = "pdf")]
pub fn svg_to_pdf_with_options(svg: &ResvgCompatibleSvg, options: &PdfOptions) -> Result<Vec<u8>> {
    prepare_pdf(svg, options)?.encode()
}

#[cfg(feature = "pdf")]
fn parse_pdf_tree(svg: &str) -> Result<usvg::Tree> {
    let mut opts = usvg::Options::default();
    configure_usvg_options_for_pdf(&mut opts);
    usvg::Tree::from_str(svg, &opts).map_err(|_| ExportError::SvgParse)
}

#[cfg(feature = "pdf")]
fn svg_tree_to_pdf(svg_tree: &usvg::Tree, options: &PdfOptions) -> Result<Vec<u8>> {
    use krilla_svg::SurfaceExt;

    let svg_size = pdf_svg_size(svg_tree)?;
    let layout = pdf_page_layout(svg_size, options.page_policy)?;

    let mut document = krilla::Document::new();
    let mut page = document.start_page_with(krilla::page::PageSettings::new(layout.page_size));
    let mut surface = page.surface();
    draw_pdf_background(
        &mut surface,
        layout.page_size,
        options.background.as_deref(),
    );
    if layout.offset.0 != 0.0 || layout.offset.1 != 0.0 {
        surface.push_transform(&krilla::geom::Transform::from_translate(
            layout.offset.0,
            layout.offset.1,
        ));
    }
    surface.draw_svg(
        svg_tree,
        layout.drawing_size,
        krilla_svg::SvgSettings {
            filter_scale: options.filter_scale,
            ..krilla_svg::SvgSettings::default()
        },
    );
    if layout.offset.0 != 0.0 || layout.offset.1 != 0.0 {
        surface.pop();
    }
    surface.finish();
    page.finish();

    let pdf = document.finish().map_err(|_| ExportError::PdfConvert)?;

    Ok(pdf)
}

#[cfg(feature = "pdf")]
fn pdf_svg_size(svg_tree: &usvg::Tree) -> Result<krilla::geom::Size> {
    krilla::geom::Size::from_wh(svg_tree.size().width(), svg_tree.size().height())
        .ok_or(ExportError::SvgDocSize)
}

#[cfg(feature = "pdf")]
#[derive(Clone, Copy)]
struct PdfPageLayout {
    page_size: krilla::geom::Size,
    drawing_size: krilla::geom::Size,
    offset: (f32, f32),
}

#[cfg(feature = "pdf")]
fn pdf_page_layout(
    svg_size: krilla::geom::Size,
    page_policy: PdfPagePolicy,
) -> Result<PdfPageLayout> {
    match page_policy {
        PdfPagePolicy::FitSvg => Ok(PdfPageLayout {
            page_size: svg_size,
            drawing_size: svg_size,
            offset: (0.0, 0.0),
        }),
        PdfPagePolicy::Fixed {
            width_pt,
            height_pt,
        } => {
            let Some(page_size) = krilla::geom::Size::from_wh(width_pt, height_pt) else {
                return Err(ExportError::InvalidSizing(
                    "fixed PDF page dimensions must be finite and positive",
                ));
            };
            let scale = (width_pt / svg_size.width()).min(height_pt / svg_size.height());
            let Some(drawing_size) =
                krilla::geom::Size::from_wh(svg_size.width() * scale, svg_size.height() * scale)
            else {
                return Err(ExportError::SvgDocSize);
            };
            let offset = (
                (width_pt - drawing_size.width()) / 2.0,
                (height_pt - drawing_size.height()) / 2.0,
            );
            Ok(PdfPageLayout {
                page_size,
                drawing_size,
                offset,
            })
        }
        PdfPagePolicy::FitCssWidth { max_width_px } => {
            if !(max_width_px.is_finite() && max_width_px > 0.0) {
                return Err(ExportError::InvalidSizing(
                    "PDF CSS viewport width must be finite and positive",
                ));
            }
            let displayed_width_px = svg_size.width().min(max_width_px);
            let page_width_pt = displayed_width_px * PDF_POINTS_PER_CSS_PIXEL;
            let page_height_pt = svg_size.height() / svg_size.width() * page_width_pt;
            let Some(page_size) = krilla::geom::Size::from_wh(page_width_pt, page_height_pt) else {
                return Err(ExportError::SvgDocSize);
            };
            Ok(PdfPageLayout {
                page_size,
                drawing_size: page_size,
                offset: (0.0, 0.0),
            })
        }
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn plan_svg_conversion(
    tree: &usvg::Tree,
    limits: SvgConversionLimits,
) -> Result<SvgConversionPlan> {
    let mut plan = SvgConversionPlan::default();
    plan_svg_conversion_group(tree.root(), 0, 0, limits, &mut plan)?;
    Ok(plan)
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn plan_svg_conversion_group(
    root: &usvg::Group,
    parent_isolation_depth: usize,
    tree_depth: usize,
    limits: SvgConversionLimits,
    plan: &mut SvgConversionPlan,
) -> Result<()> {
    let mut stack = vec![(root, parent_isolation_depth, tree_depth)];
    while let Some((group, parent_depth, tree_depth)) = stack.pop() {
        plan.max_tree_depth = plan.max_tree_depth.max(tree_depth);
        check_svg_conversion_limit(
            merman_render::resources::SVG_BACKEND_TREE_DEPTH_HARD_CAP_ID,
            plan.max_tree_depth,
            Some(merman_render::resources::MAX_RESVG_TREE_DEPTH),
        )?;

        let isolation_depth = parent_depth.saturating_add(usize::from(group.should_isolate()));
        plan.max_isolation_depth = plan.max_isolation_depth.max(isolation_depth);
        check_svg_conversion_limit(
            "max_isolation_depth",
            plan.max_isolation_depth,
            limits.max_isolation_depth,
        )?;

        if !group.filters().is_empty() {
            plan.filtered_groups = plan.filtered_groups.saturating_add(1);
        }
        for filter in group.filters() {
            let primitives = filter.primitives().len();
            check_svg_conversion_limit(
                "max_filter_primitives_per_filter",
                primitives,
                limits.max_filter_primitives_per_filter,
            )?;
            plan.filter_primitives = plan.filter_primitives.saturating_add(primitives);
            check_svg_conversion_limit(
                "max_total_filter_primitives",
                plan.filter_primitives,
                limits.max_total_filter_primitives,
            )?;
        }

        for node in group.children() {
            if let usvg::Node::Group(child) = node {
                stack.push((child, isolation_depth, tree_depth.saturating_add(1)));
            }
            if let usvg::Node::Image(image) = node
                && matches!(image.kind(), usvg::ImageKind::SVG(_))
            {
                plan.nested_svg_images = plan.nested_svg_images.saturating_add(1);
                check_svg_conversion_limit(
                    "max_nested_svg_images",
                    plan.nested_svg_images,
                    limits.max_nested_svg_images,
                )?;
            }

            let mut subroot_result = Ok(());
            node.subroots(|subroot| {
                if subroot_result.is_err() {
                    return;
                }
                plan.subroots = plan.subroots.saturating_add(1);
                subroot_result =
                    check_svg_conversion_limit("max_subroots", plan.subroots, limits.max_subroots);
                if subroot_result.is_ok() {
                    subroot_result = plan_svg_conversion_group(
                        subroot,
                        isolation_depth.saturating_add(1),
                        tree_depth.saturating_add(1),
                        limits,
                        plan,
                    );
                }
            });
            subroot_result?;
        }
    }
    Ok(())
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn check_svg_conversion_limit(
    limit_name: &'static str,
    actual: usize,
    max: Option<usize>,
) -> Result<()> {
    let Some(max) = max else {
        return Ok(());
    };
    if actual <= max {
        return Ok(());
    }
    Err(ExportError::SvgConversionLimit {
        limit_name,
        actual: actual as u64,
        max: max as u64,
    })
}

#[cfg(feature = "pdf")]
fn plan_pdf_filter_images(
    tree: &usvg::Tree,
    page_scale: f32,
    requested_scale: f32,
    limit: PdfFilterImageLimit,
) -> Result<PdfFilterImagePlan> {
    let filtered_groups = pdf_filtered_group_bounds(tree);
    let requested_pixels = pdf_filter_pixels(&filtered_groups, page_scale, requested_scale);
    let Some(max_pixels) = limit.max_total_pixels else {
        return Ok(PdfFilterImagePlan {
            filtered_groups: filtered_groups.len(),
            requested_scale,
            effective_scale: requested_scale,
            requested_image_pixels: requested_pixels,
            effective_image_pixels: requested_pixels,
            limited: false,
        });
    };
    if requested_pixels <= max_pixels {
        return Ok(PdfFilterImagePlan {
            filtered_groups: filtered_groups.len(),
            requested_scale,
            effective_scale: requested_scale,
            requested_image_pixels: requested_pixels,
            effective_image_pixels: requested_pixels,
            limited: false,
        });
    }

    let mut accepted = 0.0_f32;
    let mut rejected = requested_scale;
    for _ in 0..48 {
        let candidate = accepted + (rejected - accepted) / 2.0;
        if pdf_filter_pixels(&filtered_groups, page_scale, candidate) <= max_pixels {
            accepted = candidate;
        } else {
            rejected = candidate;
        }
    }
    if !(accepted.is_finite() && accepted > 0.0) {
        return Err(ExportError::InvalidSizing(
            "PDF filter rasterization cannot fit the configured pixel budget",
        ));
    }
    let effective_pixels = pdf_filter_pixels(&filtered_groups, page_scale, accepted);
    Ok(PdfFilterImagePlan {
        filtered_groups: filtered_groups.len(),
        requested_scale,
        effective_scale: accepted,
        requested_image_pixels: requested_pixels,
        effective_image_pixels: effective_pixels,
        limited: true,
    })
}

#[cfg(feature = "pdf")]
fn pdf_filtered_group_bounds(tree: &usvg::Tree) -> Vec<(f64, f64)> {
    let mut bounds = Vec::new();
    let mut stack = vec![(tree.root(), 1.0_f64)];
    while let Some((group, coordinate_scale)) = stack.pop() {
        if !group.filters().is_empty() {
            let bbox = group.abs_layer_bounding_box();
            bounds.push((
                f64::from(bbox.width()) * coordinate_scale,
                f64::from(bbox.height()) * coordinate_scale,
            ));
            // krilla-svg rasterizes this whole group through resvg and returns. Descendant
            // filters contribute to that localized render, but they do not allocate a second
            // krilla-owned PDF image and must not be counted as independent top-level groups.
            continue;
        }
        for node in group.children() {
            match node {
                usvg::Node::Group(child) => stack.push((child, coordinate_scale)),
                usvg::Node::Image(image) => {
                    if let usvg::ImageKind::SVG(nested) = image.kind() {
                        let (scale_x, scale_y) = image.abs_transform().get_scale();
                        let image_scale = f64::from(scale_x.abs().max(scale_y.abs()));
                        stack.push((nested.root(), coordinate_scale * image_scale));
                    }
                }
                _ => {}
            }
        }
    }
    bounds
}

#[cfg(feature = "pdf")]
fn pdf_filter_pixels(bounds: &[(f64, f64)], page_scale: f32, filter_scale: f32) -> u64 {
    let scale = f64::from(page_scale) * f64::from(filter_scale);
    bounds.iter().fold(0_u64, |total, &(width, height)| {
        let requested_width = width * scale;
        let requested_height = height * scale;
        let cap = (KRILLA_MAX_FILTER_SIDE_PX / requested_width)
            .min(KRILLA_MAX_FILTER_SIDE_PX / requested_height)
            .min(1.0);
        let width_px = (requested_width * cap).round().clamp(0.0, 5000.0) as u64;
        let height_px = (requested_height * cap).round().clamp(0.0, 5000.0) as u64;
        total.saturating_add(width_px.saturating_mul(height_px))
    })
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[derive(Debug, Clone, Copy, Default)]
struct EmbeddedDataPlan {
    resources: usize,
    largest_bytes: u64,
    total_bytes: u64,
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn plan_embedded_data_resources(svg: &str, limit: EmbeddedImageLimit) -> Result<EmbeddedDataPlan> {
    use quick_xml::XmlVersion;
    use quick_xml::events::Event;

    validate_embedded_image_limit(limit)?;
    let mut reader = quick_xml::Reader::from_str(svg);
    let mut plan = EmbeddedDataPlan::default();
    loop {
        let event = reader.read_event().map_err(|_| ExportError::SvgParse)?;
        let element = match event {
            Event::Start(element) | Event::Empty(element)
                if is_embedded_image_element(element.local_name().as_ref()) =>
            {
                element
            }
            Event::Eof => break,
            _ => continue,
        };

        for attribute in element.attributes() {
            let attribute = attribute.map_err(|_| ExportError::SvgParse)?;
            if !attribute
                .key
                .local_name()
                .as_ref()
                .eq_ignore_ascii_case(b"href")
            {
                continue;
            }
            let value = attribute
                .normalized_value(XmlVersion::Implicit1_0)
                .map_err(|_| ExportError::SvgParse)?;
            let Ok(data_url) = data_url::DataUrl::process(value.as_ref()) else {
                continue;
            };

            let mut resource_bytes = 0_u64;
            let mut limit_error = None;
            let _ = data_url.decode(|chunk| {
                resource_bytes = resource_bytes.saturating_add(chunk.len() as u64);
                let aggregate = plan.total_bytes.saturating_add(resource_bytes);
                if let Some(max) = limit.max_bytes_per_image
                    && resource_bytes > max
                {
                    limit_error = Some(ExportError::EmbeddedImageLimit {
                        limit_name: "max_bytes_per_image",
                        actual: resource_bytes,
                        max,
                    });
                    return Err(());
                }
                if let Some(max) = limit.max_total_bytes
                    && aggregate > max
                {
                    limit_error = Some(ExportError::EmbeddedImageLimit {
                        limit_name: "max_total_bytes",
                        actual: aggregate,
                        max,
                    });
                    return Err(());
                }
                Ok(())
            });
            if let Some(error) = limit_error {
                return Err(error);
            }

            plan.resources = plan.resources.saturating_add(1);
            plan.largest_bytes = plan.largest_bytes.max(resource_bytes);
            plan.total_bytes = plan.total_bytes.saturating_add(resource_bytes);
        }
    }
    Ok(plan)
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn is_embedded_image_element(local_name: &[u8]) -> bool {
    // usvg resolves both elements through the same image resolver. Match qualified-name aliases
    // conservatively so namespace spelling cannot turn the preflight into a false-negative gate.
    local_name.eq_ignore_ascii_case(b"image") || local_name.eq_ignore_ascii_case(b"feImage")
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn plan_embedded_images(
    tree: &usvg::Tree,
    limit: EmbeddedImageLimit,
    data: EmbeddedDataPlan,
) -> Result<EmbeddedImagePlan> {
    validate_embedded_image_limit(limit)?;
    let mut plan = EmbeddedImagePlan {
        data_resources: data.resources,
        raster_images: 0,
        largest_data_bytes: data.largest_bytes,
        total_data_bytes: data.total_bytes,
        largest_raster_pixels: 0,
        total_pixels: 0,
    };
    plan_embedded_images_in_group(tree.root(), limit, &mut plan)?;
    Ok(plan)
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn plan_embedded_images_in_group(
    root: &usvg::Group,
    limit: EmbeddedImageLimit,
    plan: &mut EmbeddedImagePlan,
) -> Result<()> {
    let mut stack = vec![root];
    while let Some(group) = stack.pop() {
        for node in group.children() {
            match node {
                usvg::Node::Group(child) => stack.push(child),
                usvg::Node::Image(image) => match image.kind() {
                    usvg::ImageKind::SVG(_) => {}
                    usvg::ImageKind::JPEG(_)
                    | usvg::ImageKind::PNG(_)
                    | usvg::ImageKind::GIF(_)
                    | usvg::ImageKind::WEBP(_) => {
                        let pixels = intrinsic_image_pixels(image.size())?;
                        plan.raster_images = plan.raster_images.saturating_add(1);
                        plan.largest_raster_pixels = plan.largest_raster_pixels.max(pixels);
                        plan.total_pixels = plan.total_pixels.saturating_add(pixels);
                        if let Some(max) = limit.max_pixels_per_image
                            && pixels > max
                        {
                            return Err(ExportError::EmbeddedImageLimit {
                                limit_name: "max_pixels_per_image",
                                actual: pixels,
                                max,
                            });
                        }
                        if let Some(max) = limit.max_total_pixels
                            && plan.total_pixels > max
                        {
                            return Err(ExportError::EmbeddedImageLimit {
                                limit_name: "max_total_pixels",
                                actual: plan.total_pixels,
                                max,
                            });
                        }
                    }
                },
                _ => {}
            }

            // SVG images, clip paths, masks, patterns, and filter image primitives can own
            // additional renderable trees. Walk those subroots as well so a hidden definition
            // cannot bypass the decode budget.
            let mut subroot_result = Ok(());
            node.subroots(|subroot| {
                if subroot_result.is_ok() {
                    subroot_result = plan_embedded_images_in_group(subroot, limit, plan);
                }
            });
            subroot_result?;
        }
    }
    Ok(())
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn intrinsic_image_pixels(size: usvg::Size) -> Result<u64> {
    let width = f64::from(size.width()).ceil();
    let height = f64::from(size.height()).ceil();
    if !(width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0) {
        return Err(ExportError::InvalidSizing(
            "embedded image dimensions must be finite and positive",
        ));
    }
    if width > u64::MAX as f64 || height > u64::MAX as f64 {
        return Err(ExportError::InvalidSizing(
            "embedded image dimensions exceed the planner capability",
        ));
    }
    Ok((width as u64).saturating_mul(height as u64))
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn validate_embedded_image_limit(limit: EmbeddedImageLimit) -> Result<()> {
    if limit.max_bytes_per_image == Some(0)
        || limit.max_total_bytes == Some(0)
        || limit.max_pixels_per_image == Some(0)
        || limit.max_total_pixels == Some(0)
    {
        return Err(ExportError::InvalidSizing(
            "embedded image byte and pixel limits must be positive",
        ));
    }
    Ok(())
}

#[cfg(feature = "pdf")]
fn draw_pdf_background(
    surface: &mut krilla::surface::Surface<'_>,
    page_size: krilla::geom::Size,
    background: Option<&str>,
) {
    let Some(background) = background.filter(|value| !value.eq_ignore_ascii_case("transparent"))
    else {
        return;
    };
    let Some(color) = parse_rgba_color(background) else {
        return;
    };
    let Some(opacity) = krilla::num::NormalizedF32::new(f32::from(color.alpha) / 255.0) else {
        return;
    };
    let mut path = krilla::geom::PathBuilder::new();
    let Some(rect) = krilla::geom::Rect::from_xywh(0.0, 0.0, page_size.width(), page_size.height())
    else {
        return;
    };
    path.push_rect(rect);
    let Some(path) = path.finish() else {
        return;
    };
    surface.set_fill(Some(krilla::paint::Fill {
        paint: krilla::color::rgb::Color::new(color.red, color.green, color.blue).into(),
        opacity,
        rule: Default::default(),
    }));
    surface.draw_path(&path);
    surface.set_fill(None);
}

#[cfg(any(feature = "png", feature = "jpeg"))]
#[derive(Debug, Clone, Copy)]
struct ParsedViewBox {
    width: f32,
    height: f32,
}

#[cfg(any(feature = "png", feature = "jpeg"))]
#[derive(Debug, Clone, Copy, Default)]
struct RootSvgMetadata {
    view_box: Option<ParsedViewBox>,
    max_width_px: Option<f32>,
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn parse_root_svg_metadata(svg: &str) -> Result<RootSvgMetadata> {
    use quick_xml::{XmlVersion, events::Event};

    let mut reader = quick_xml::Reader::from_str(svg);
    loop {
        let event = reader.read_event().map_err(|_| ExportError::SvgParse)?;
        let element = match event {
            Event::Start(element) | Event::Empty(element) => element,
            Event::Eof => return Err(ExportError::SvgParse),
            _ => continue,
        };

        if !element.local_name().as_ref().eq_ignore_ascii_case(b"svg") {
            return Err(ExportError::SvgParse);
        }

        let mut metadata = RootSvgMetadata::default();
        for attribute in element.attributes() {
            let attribute = attribute.map_err(|_| ExportError::SvgParse)?;
            let value = attribute
                .normalized_value(XmlVersion::Implicit1_0)
                .map_err(|_| ExportError::SvgParse)?;
            match attribute.key.local_name().as_ref() {
                b"viewBox" => {
                    metadata.view_box = parse_svg_view_box(value.as_ref());
                }
                b"style" => {
                    metadata.max_width_px = parse_inline_max_width_px(value.as_ref());
                }
                _ => {}
            }
        }
        return Ok(metadata);
    }
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn parse_svg_view_box(value: &str) -> Option<ParsedViewBox> {
    let mut input = ParserInput::new(value);
    let mut parser = Parser::new(&mut input);
    let mut values = [0.0_f32; 4];

    for index in 0..values.len() {
        values[index] = parser.expect_number().ok()?;
        if index < values.len() - 1 {
            let _ = parser.try_parse(|parser| parser.expect_comma());
        }
    }
    parser.expect_exhausted().ok()?;

    let [min_x, min_y, width, height] = values;
    if min_x.is_finite()
        && min_y.is_finite()
        && width.is_finite()
        && height.is_finite()
        && width > 0.0
        && height > 0.0
    {
        Some(ParsedViewBox { width, height })
    } else {
        None
    }
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn parse_inline_max_width_px(style: &str) -> Option<f32> {
    let mut input = ParserInput::new(style);
    let mut parser = Parser::new(&mut input);
    let mut max_width = None;

    while !parser.is_exhausted() {
        let declaration = parser.parse_until_after(Delimiter::Semicolon, |declaration| {
            let property = declaration.expect_ident_cloned()?;
            declaration.expect_colon()?;

            if !property.eq_ignore_ascii_case("max-width") {
                declaration.expect_no_error_token()?;
                return Ok::<_, cssparser::ParseError<'_, ()>>(None);
            }

            let token = declaration.next()?.clone();
            declaration.expect_exhausted()?;
            let Token::Dimension { value, unit, .. } = token else {
                return Ok(None);
            };
            if unit.eq_ignore_ascii_case("px") && value.is_finite() && value > 0.0 {
                Ok(Some(value))
            } else {
                Ok(None)
            }
        });

        if let Ok(Some(value)) = declaration {
            max_width = Some(value);
        }
    }
    max_width
}

#[cfg(any(feature = "png", feature = "jpeg"))]
#[derive(Debug, Clone, Copy)]
struct RasterGeometry {
    min_x: f32,
    min_y: f32,
    width: f32,
    height: f32,
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn raster_geometry_for_svg(metadata: RootSvgMetadata, tree: &usvg::Tree) -> (RasterGeometry, bool) {
    if let Some(vb) = metadata.view_box {
        // `usvg`/`resvg` already apply the root viewBox transform (including translating the
        // viewBox min corner to (0,0)) when building/rendering the tree. If we also translate
        // by `-min_x/-min_y` here, diagrams with negative viewBox mins (e.g. kanban, gitGraph)
        // get shifted fully out of the viewport and render as a blank/transparent pixmap.
        return (
            RasterGeometry {
                min_x: 0.0,
                min_y: 0.0,
                width: vb.width,
                height: vb.height,
            },
            false,
        );
    }

    // Some Mermaid diagrams (e.g. `info`) don't emit a viewBox upstream.
    // For raster formats, fall back to the rendered content bounds as computed by usvg.
    let bbox = tree.root().abs_stroke_bounding_box();
    let w = bbox.width().max(1.0);
    let h = bbox.height().max(1.0);
    if w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0 {
        (
            RasterGeometry {
                min_x: bbox.x(),
                min_y: bbox.y(),
                width: w,
                height: h,
            },
            true,
        )
    } else {
        let size = tree.size();
        (
            RasterGeometry {
                min_x: 0.0,
                min_y: 0.0,
                width: size.width(),
                height: size.height(),
            },
            false,
        )
    }
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn raster_plan_for_geometry(geo: RasterGeometry, options: &RasterOptions) -> Result<RasterPlan> {
    if !(options.scale.is_finite() && options.scale > 0.0) {
        return Err(ExportError::InvalidScale);
    }

    validate_fit_box(options.fit_to)?;
    validate_size_limit(options.size_limit)?;

    // Make scaling more intuitive/stable: at scale=1 we already round up to whole pixels, so for
    // scale>1 prefer scaling the *rounded* base size. This avoids surprising off-by-one shrinkage
    // when the viewBox/bounds are fractional (e.g. 342.36 * 2 = 684.72 -> ceil = 685, while
    // ceil(342.36) * 2 = 686).
    let base_width_px = f64::from(geo.width).ceil().max(1.0);
    let base_height_px = f64::from(geo.height).ceil().max(1.0);

    let fit_scale = fit_scale_for_base_size(base_width_px, base_height_px, options.fit_to);
    let requested_scale = fit_scale * f64::from(options.scale);
    let requested_width_px = requested_raster_dim_px(base_width_px * requested_scale)?;
    let requested_height_px = requested_raster_dim_px(base_height_px * requested_scale)?;

    let limit_scale = size_limit_scale(
        base_width_px * requested_scale,
        base_height_px * requested_scale,
        options.size_limit,
    );
    let mut effective_scale = requested_scale * limit_scale;
    let (mut width_px, mut height_px) = raster_limited_dims(
        base_width_px,
        base_height_px,
        effective_scale,
        options.size_limit,
    )?;

    if let Some(max_pixels) = options.size_limit.max_pixels {
        for _ in 0..8 {
            if u64::from(width_px) * u64::from(height_px) <= max_pixels {
                break;
            }
            let pixels = f64::from(width_px) * f64::from(height_px);
            let shrink = ((max_pixels as f64) / pixels).sqrt() * 0.999_999;
            effective_scale *= shrink;
            (width_px, height_px) = raster_limited_dims(
                base_width_px,
                base_height_px,
                effective_scale,
                options.size_limit,
            )?;
        }
    }

    Ok(RasterPlan {
        requested_width_px,
        requested_height_px,
        width_px,
        height_px,
        requested_scale,
        effective_scale,
        limited: f64::from(width_px) != requested_width_px
            || f64::from(height_px) != requested_height_px,
    })
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn validate_fit_box(fit: Option<RasterFitBox>) -> Result<()> {
    let Some(fit) = fit else {
        return Ok(());
    };

    if fit.width.is_none() && fit.height.is_none() {
        return Err(ExportError::InvalidSizing(
            "fit_to must include a positive width or height",
        ));
    }
    if fit.width == Some(0) || fit.height == Some(0) {
        return Err(ExportError::InvalidSizing(
            "fit_to width and height must be positive",
        ));
    }
    Ok(())
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn validate_size_limit(limit: RasterSizeLimit) -> Result<()> {
    if limit.max_width == Some(0) || limit.max_height == Some(0) {
        return Err(ExportError::InvalidSizing(
            "size_limit max_width and max_height must be positive",
        ));
    }
    if limit.max_pixels == Some(0) {
        return Err(ExportError::InvalidSizing(
            "size_limit max_pixels must be positive",
        ));
    }
    Ok(())
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn fit_scale_for_base_size(width: f64, height: f64, fit: Option<RasterFitBox>) -> f64 {
    let Some(fit) = fit else {
        return 1.0;
    };

    let mut scale: f64 = 1.0;
    if let Some(target_width) = fit.width {
        scale = scale.min(f64::from(target_width) / width);
    }
    if let Some(target_height) = fit.height {
        scale = scale.min(f64::from(target_height) / height);
    }
    if scale.is_nan() {
        1.0
    } else {
        scale.clamp(0.0, 1.0)
    }
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn size_limit_scale(width: f64, height: f64, limit: RasterSizeLimit) -> f64 {
    let mut scale: f64 = 1.0;
    if let Some(max_width) = limit.max_width {
        scale = scale.min(f64::from(max_width) / width);
    }
    if let Some(max_height) = limit.max_height {
        scale = scale.min(f64::from(max_height) / height);
    }
    if let Some(max_pixels) = limit.max_pixels {
        let pixels = width * height * scale * scale;
        if pixels > max_pixels as f64 {
            scale *= ((max_pixels as f64) / pixels).sqrt();
        }
    }
    if scale.is_nan() {
        1.0
    } else {
        scale.clamp(0.0, 1.0)
    }
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn raster_limited_dims(
    base_width_px: f64,
    base_height_px: f64,
    scale: f64,
    limit: RasterSizeLimit,
) -> Result<(u32, u32)> {
    Ok((
        raster_dim_px(base_width_px * scale, limit.max_width)?,
        raster_dim_px(base_height_px * scale, limit.max_height)?,
    ))
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn raster_dim_px(value: f64, max: Option<u32>) -> Result<u32> {
    let value = requested_raster_dim_px(value)?;
    let value = max.map_or(value, |max| value.min(f64::from(max)));
    if value > f64::from(u32::MAX) {
        return Err(ExportError::InvalidSizing(
            "final raster dimension exceeds the u32 encoder capability",
        ));
    }
    Ok(value as u32)
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn requested_raster_dim_px(value: f64) -> Result<f64> {
    if !(value.is_finite() && value > 0.0) {
        return Err(ExportError::InvalidSizing(
            "computed raster dimension must be finite and positive",
        ));
    }
    Ok(value.ceil().max(1.0))
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn configure_usvg_options_for_raster(opt: &mut usvg::Options<'_>, metadata: RootSvgMetadata) {
    opt.fontdb = shared_system_fontdb();

    if metadata.view_box.is_none()
        && let Some(max_width) = metadata.max_width_px
        && max_width.is_finite()
        && max_width > 0.0
        && let Some(size) = usvg::Size::from_wh(max_width, opt.default_size.height())
    {
        opt.default_size = size;
    }

    opt.font_family =
        raster_default_font_family(opt.fontdb.as_ref()).unwrap_or_else(|| "Arial".to_string());
    opt.font_resolver = browser_like_font_resolver();
    opt.image_href_resolver = data_url_only_image_href_resolver();
}

#[cfg(feature = "pdf")]
fn configure_usvg_options_for_pdf(opt: &mut usvg::Options<'_>) {
    opt.fontdb = shared_system_fontdb();
    opt.font_family =
        raster_default_font_family(opt.fontdb.as_ref()).unwrap_or_else(|| "Arial".to_string());
    opt.font_resolver = browser_like_pdf_font_resolver();
    opt.image_href_resolver = data_url_only_image_href_resolver();
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn shared_system_fontdb() -> Arc<usvg::fontdb::Database> {
    static FONTDB: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    Arc::clone(FONTDB.get_or_init(|| {
        let mut fontdb = usvg::fontdb::Database::new();
        fontdb.load_system_fonts();
        configure_fontdb_generic_families(&mut fontdb);
        Arc::new(fontdb)
    }))
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn data_url_only_image_href_resolver() -> usvg::ImageHrefResolver<'static> {
    usvg::ImageHrefResolver {
        resolve_data: usvg::ImageHrefResolver::default_data_resolver(),
        resolve_string: Box::new(|_, _| None),
    }
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn browser_like_font_resolver() -> usvg::FontResolver<'static> {
    let default_select = usvg::FontResolver::default_font_selector();

    usvg::FontResolver {
        select_font: Box::new(move |font, fontdb| {
            default_select(font, fontdb)
                .or_else(|| query_browser_like_fallback_font(font, fontdb.as_ref()))
                .or_else(|| fontdb.faces().next().map(|face| face.id))
        }),
        select_fallback: usvg::FontResolver::default_fallback_selector(),
    }
}

#[cfg(feature = "pdf")]
fn browser_like_pdf_font_resolver() -> usvg::FontResolver<'static> {
    let default_select = usvg::FontResolver::default_font_selector();

    usvg::FontResolver {
        select_font: Box::new(move |font, fontdb| {
            default_select(font, fontdb)
                .or_else(|| query_browser_like_pdf_fallback_font(font, fontdb.as_ref()))
                .or_else(|| fontdb.faces().next().map(|face| face.id))
        }),
        select_fallback: usvg::FontResolver::default_fallback_selector(),
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn configure_fontdb_generic_families(fontdb: &mut usvg::fontdb::Database) {
    let sans = first_font_family(fontdb, |face| !face.monospaced)
        .or_else(|| first_font_family(fontdb, |_| true));
    let mono = first_font_family(fontdb, |face| face.monospaced).or_else(|| sans.clone());

    if query_normal_font_family(fontdb, usvg::fontdb::Family::SansSerif).is_none()
        && let Some(family) = sans.as_ref()
    {
        fontdb.set_sans_serif_family(family.clone());
    }
    if query_normal_font_family(fontdb, usvg::fontdb::Family::Serif).is_none()
        && let Some(family) = sans.as_ref()
    {
        fontdb.set_serif_family(family.clone());
    }
    if query_normal_font_family(fontdb, usvg::fontdb::Family::Monospace).is_none()
        && let Some(family) = mono.as_ref()
    {
        fontdb.set_monospace_family(family.clone());
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn raster_default_font_family(fontdb: &usvg::fontdb::Database) -> Option<String> {
    query_normal_font_family(fontdb, usvg::fontdb::Family::SansSerif)
        .or_else(|| query_normal_font_family(fontdb, usvg::fontdb::Family::Serif))
        .or_else(|| first_font_family(fontdb, |_| true))
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn query_browser_like_fallback_font(
    font: &usvg::Font,
    fontdb: &usvg::fontdb::Database,
) -> Option<usvg::fontdb::ID> {
    let mut families = Vec::with_capacity(3);
    if font_requests_monospace(font) {
        families.push(usvg::fontdb::Family::Monospace);
        families.push(usvg::fontdb::Family::SansSerif);
        families.push(usvg::fontdb::Family::Serif);
    } else {
        families.push(usvg::fontdb::Family::SansSerif);
        families.push(usvg::fontdb::Family::Serif);
        families.push(usvg::fontdb::Family::Monospace);
    }

    let query = usvg::fontdb::Query {
        families: &families,
        weight: usvg::fontdb::Weight(font.weight()),
        stretch: font.stretch().into(),
        style: font.style().into(),
    };
    fontdb.query(&query)
}

#[cfg(feature = "pdf")]
fn query_browser_like_pdf_fallback_font(
    font: &usvg::Font,
    fontdb: &usvg::fontdb::Database,
) -> Option<usvg::fontdb::ID> {
    let mut families = Vec::with_capacity(3);
    if pdf_font_requests_monospace(font) {
        families.push(usvg::fontdb::Family::Monospace);
        families.push(usvg::fontdb::Family::SansSerif);
        families.push(usvg::fontdb::Family::Serif);
    } else {
        families.push(usvg::fontdb::Family::SansSerif);
        families.push(usvg::fontdb::Family::Serif);
        families.push(usvg::fontdb::Family::Monospace);
    }

    let query = usvg::fontdb::Query {
        families: &families,
        weight: usvg::fontdb::Weight(font.weight()),
        stretch: font.stretch().into(),
        style: font.style().into(),
    };
    fontdb.query(&query)
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn query_normal_font_family(
    fontdb: &usvg::fontdb::Database,
    family: usvg::fontdb::Family<'_>,
) -> Option<String> {
    let families = [family];
    let query = usvg::fontdb::Query {
        families: &families,
        weight: usvg::fontdb::Weight::NORMAL,
        stretch: usvg::fontdb::Stretch::Normal,
        style: usvg::fontdb::Style::Normal,
    };
    fontdb
        .query(&query)
        .and_then(|id| fontdb.face(id))
        .and_then(face_family_name)
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn first_font_family<F>(fontdb: &usvg::fontdb::Database, mut predicate: F) -> Option<String>
where
    F: FnMut(&usvg::fontdb::FaceInfo) -> bool,
{
    fontdb
        .faces()
        .find(|face| predicate(face))
        .and_then(face_family_name)
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn face_family_name(face: &usvg::fontdb::FaceInfo) -> Option<String> {
    face.families
        .iter()
        .find(|(_, lang)| *lang == usvg::fontdb::Language::English_UnitedStates)
        .or_else(|| face.families.first())
        .map(|(family, _)| family.clone())
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn font_requests_monospace(font: &usvg::Font) -> bool {
    font.families().iter().any(|family| match family {
        usvg::FontFamily::Monospace => true,
        usvg::FontFamily::Named(name) => {
            let name = name.to_ascii_lowercase();
            name.contains("mono")
                || name.contains("courier")
                || name.contains("consolas")
                || name.contains("menlo")
        }
        _ => false,
    })
}

#[cfg(feature = "pdf")]
fn pdf_font_requests_monospace(font: &usvg::Font) -> bool {
    font.families().iter().any(|family| match family {
        usvg::FontFamily::Monospace => true,
        usvg::FontFamily::Named(name) => {
            let name = name.to_ascii_lowercase();
            name.contains("mono")
                || name.contains("courier")
                || name.contains("consolas")
                || name.contains("menlo")
        }
        _ => false,
    })
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[derive(Debug, Clone, Copy)]
struct RgbaColor {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn parse_rgba_color(text: &str) -> Option<RgbaColor> {
    let s = text.trim().to_ascii_lowercase();
    match s.as_str() {
        "transparent" => {
            return Some(RgbaColor {
                red: 0,
                green: 0,
                blue: 0,
                alpha: 0,
            });
        }
        "white" => {
            return Some(RgbaColor {
                red: 255,
                green: 255,
                blue: 255,
                alpha: 255,
            });
        }
        "black" => {
            return Some(RgbaColor {
                red: 0,
                green: 0,
                blue: 0,
                alpha: 255,
            });
        }
        _ => {}
    }

    let hex = s.strip_prefix('#')?;
    fn hex2(b: &[u8]) -> Option<u8> {
        let hi = (*b.first()? as char).to_digit(16)? as u8;
        let lo = (*b.get(1)? as char).to_digit(16)? as u8;
        Some((hi << 4) | lo)
    }
    fn hex1(c: u8) -> Option<u8> {
        let v = (c as char).to_digit(16)? as u8;
        Some((v << 4) | v)
    }

    let bytes = hex.as_bytes();
    match bytes.len() {
        3 => Some(RgbaColor {
            red: hex1(bytes[0])?,
            green: hex1(bytes[1])?,
            blue: hex1(bytes[2])?,
            alpha: 255,
        }),
        4 => Some(RgbaColor {
            red: hex1(bytes[0])?,
            green: hex1(bytes[1])?,
            blue: hex1(bytes[2])?,
            alpha: hex1(bytes[3])?,
        }),
        6 => Some(RgbaColor {
            red: hex2(&bytes[0..2])?,
            green: hex2(&bytes[2..4])?,
            blue: hex2(&bytes[4..6])?,
            alpha: 255,
        }),
        8 => Some(RgbaColor {
            red: hex2(&bytes[0..2])?,
            green: hex2(&bytes[2..4])?,
            blue: hex2(&bytes[4..6])?,
            alpha: hex2(&bytes[6..8])?,
        }),
        _ => None,
    }
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn parse_tiny_skia_color(text: &str) -> Option<tiny_skia::Color> {
    let color = parse_rgba_color(text)?;
    Some(tiny_skia::Color::from_rgba8(
        color.red,
        color.green,
        color.blue,
        color.alpha,
    ))
}

#[cfg(all(test, feature = "png"))]
mod png_feature_tests {
    use super::*;

    fn compatible_svg() -> ResvgCompatibleSvg {
        let session = merman_render::environment::RenderEnvironment::deterministic()
            .begin_session()
            .expect("deterministic render session");
        merman_render::svg::finalize_resvg_svg(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="black"/></svg>"#,
            &session,
        )
        .expect("sealed SVG")
    }

    #[test]
    fn png_leaf_encodes_a_sealed_svg() {
        let bytes = svg_to_png(&compatible_svg(), &RasterOptions::default())
            .expect("PNG export should be callable when its leaf is enabled");

        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    }
}

#[cfg(all(test, feature = "jpeg"))]
mod jpeg_feature_tests {
    use super::*;

    fn compatible_svg() -> ResvgCompatibleSvg {
        let session = merman_render::environment::RenderEnvironment::deterministic()
            .begin_session()
            .expect("deterministic render session");
        merman_render::svg::finalize_resvg_svg(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="black"/></svg>"#,
            &session,
        )
        .expect("sealed SVG")
    }

    #[test]
    fn jpeg_leaf_encodes_a_sealed_svg() {
        let bytes = svg_to_jpeg(&compatible_svg(), &RasterOptions::default())
            .expect("JPEG export should be callable when its leaf is enabled");

        assert!(bytes.starts_with(b"\xff\xd8\xff"));
    }
}

#[cfg(all(test, any(feature = "png", feature = "jpeg")))]
mod root_svg_metadata_tests {
    use super::*;

    #[test]
    fn metadata_reads_only_the_root_svg_attributes() {
        let metadata = parse_root_svg_metadata(
            r#"<svg xmlns="http://www.w3.org/2000/svg" style="content: 'max-width: 9000px'; max-width: 400px"><g viewBox="0 0 9000 9000"/><text>viewBox=&quot;0 0 8000 8000&quot;</text></svg>"#,
        )
        .expect("root SVG metadata");

        assert!(metadata.view_box.is_none());
        assert_eq!(metadata.max_width_px, Some(400.0));
    }

    #[test]
    fn metadata_uses_svg_number_and_css_token_grammar() {
        let metadata = parse_root_svg_metadata(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="-1.5,2,41.5,120" style="max-width: 400px nonsense"/>"#,
        )
        .expect("root SVG metadata");

        let view_box = metadata.view_box.expect("valid root viewBox");
        assert_eq!(view_box.width, 41.5);
        assert_eq!(view_box.height, 120.0);
        assert_eq!(metadata.max_width_px, None);
    }
}

#[cfg(all(test, feature = "pdf"))]
mod pdf_feature_tests {
    use super::*;

    fn compatible_svg() -> ResvgCompatibleSvg {
        let session = merman_render::environment::RenderEnvironment::deterministic()
            .begin_session()
            .expect("deterministic render session");
        merman_render::svg::finalize_resvg_svg(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="black"/></svg>"#,
            &session,
        )
        .expect("sealed SVG")
    }

    #[test]
    fn pdf_leaf_encodes_a_sealed_svg() {
        let bytes = svg_to_pdf(&compatible_svg())
            .expect("PDF export should be callable when its leaf is enabled");

        assert!(bytes.starts_with(b"%PDF-"));
    }
}

#[cfg(all(test, feature = "png", feature = "jpeg", feature = "pdf"))]
mod tests {
    use super::*;
    use base64::Engine as _;

    fn compatible_svg(svg: &str) -> merman_render::svg::ResvgCompatibleSvg {
        let session = merman_render::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        merman_render::svg::finalize_resvg_svg(svg, &session).unwrap()
    }

    fn trusted_compatible_svg(svg: &str) -> merman_render::svg::ResvgCompatibleSvg {
        let session = merman_render::environment::RenderEnvironment::deterministic()
            .with_resource_policy(merman_render::resources::RenderResourcePolicy::trusted_native())
            .begin_session()
            .unwrap();
        merman_render::svg::finalize_resvg_svg(svg, &session).unwrap()
    }

    fn svg_to_png(svg: &str, options: &RasterOptions) -> Result<Vec<u8>> {
        super::svg_to_png(&compatible_svg(svg), options)
    }

    fn svg_to_jpeg(svg: &str, options: &RasterOptions) -> Result<Vec<u8>> {
        super::svg_to_jpeg(&compatible_svg(svg), options)
    }

    fn svg_to_pdf(svg: &str) -> Result<Vec<u8>> {
        super::svg_to_pdf(&compatible_svg(svg))
    }

    fn nested_group_svg(depth: usize) -> String {
        let mut svg =
            String::from(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10">"#);
        svg.push_str(&r#"<g opacity="0.999">"#.repeat(depth - 1));
        svg.push_str(r#"<rect width="10" height="10" fill="black"/>"#);
        svg.push_str(&"</g>".repeat(depth - 1));
        svg.push_str("</svg>");
        svg
    }

    fn expanded_use_chain_svg(depth: usize) -> String {
        let mut svg =
            String::from(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><defs>"#);
        for index in 0..depth {
            if index + 1 == depth {
                svg.push_str(&format!(
                    r#"<g id="use-{index}" opacity="0.999"><rect width="10" height="10"/></g>"#
                ));
            } else {
                svg.push_str(&format!(
                    r##"<g id="use-{index}" opacity="0.999"><use href="#use-{}"/></g>"##,
                    index + 1
                ));
            }
        }
        svg.push_str(r##"</defs><use href="#use-0"/></svg>"##);
        svg
    }

    fn svg_to_pdf_with_options(svg: &str, options: &PdfOptions) -> Result<Vec<u8>> {
        super::svg_to_pdf_with_options(&compatible_svg(svg), options)
    }

    fn prepare_pdf(svg: &str, options: &PdfOptions) -> Result<PreparedPdf> {
        super::prepare_pdf(&compatible_svg(svg), options)
    }

    fn prepare_raster(svg: &str, options: &RasterOptions) -> Result<PreparedRaster> {
        super::prepare_raster(&compatible_svg(svg), options)
    }

    fn svg_raster_plan(svg: &str, options: &RasterOptions) -> Result<RasterPlan> {
        super::svg_raster_plan(&compatible_svg(svg), options)
    }

    #[test]
    fn svg_to_png_produces_png_signature() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="black"/></svg>"#;
        let bytes = svg_to_png(svg, &RasterOptions::default()).unwrap();
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn scheduling_weights_include_the_recursive_backend_stack() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10"/></svg>"#;
        let raster = prepare_raster(svg, &RasterOptions::default()).unwrap();
        let pdf = prepare_pdf(svg, &PdfOptions::default()).unwrap();

        assert!(raster.png_scheduling_weight_bytes() >= RECURSIVE_SVG_BACKEND_STACK_BYTES as u64);
        assert!(pdf.scheduling_weight_bytes() >= RECURSIVE_SVG_BACKEND_STACK_BYTES as u64);
    }

    #[test]
    fn trusted_resvg_backend_handles_the_declared_tree_depth() {
        let depth = merman_render::resources::MAX_RESVG_TREE_DEPTH;
        let svg = trusted_compatible_svg(&nested_group_svg(depth));
        let options =
            RasterOptions::default().with_conversion_limits(SvgConversionLimits::unbounded());
        let prepared = super::prepare_raster(&svg, &options).unwrap();
        assert_eq!(prepared.conversion_plan().max_tree_depth, depth - 1);

        let bytes = prepared.encode_png().unwrap();
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    fn trusted_krilla_backend_handles_the_declared_tree_depth() {
        let depth = merman_render::resources::MAX_RESVG_TREE_DEPTH;
        let svg = trusted_compatible_svg(&nested_group_svg(depth));
        let options =
            PdfOptions::default().with_conversion_limits(SvgConversionLimits::unbounded());
        let prepared = super::prepare_pdf(&svg, &options).unwrap();
        assert_eq!(prepared.conversion_plan().max_tree_depth, depth - 1);

        let bytes = prepared.encode().unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
    }

    #[test]
    fn expanded_use_tree_is_rejected_before_recursive_backends() {
        let svg = trusted_compatible_svg(&expanded_use_chain_svg(
            merman_render::resources::MAX_RESVG_TREE_DEPTH + 2,
        ));
        let options =
            RasterOptions::default().with_conversion_limits(SvgConversionLimits::unbounded());
        let error = super::prepare_raster(&svg, &options)
            .err()
            .expect("the expanded usvg tree must retain the backend depth cap");

        assert!(
            error
                .to_string()
                .contains(merman_render::resources::SVG_BACKEND_TREE_DEPTH_HARD_CAP_ID),
            "{error}"
        );
    }

    #[test]
    fn svg_to_png_does_not_load_local_image_hrefs() {
        let local_image = TempFile::new("png", encode_rgba_png(1, 1, &[255, 0, 0, 255]));
        let href = escape_xml_attr(&local_image.href_path());
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="white"/><image href="{href}" width="10" height="10"/></svg>"#
        );

        let bytes = svg_to_png(&svg, &RasterOptions::default()).unwrap();
        let center = rgba_pixel(&bytes, 5, 5);

        assert!(
            center[0] > 240 && center[1] > 240 && center[2] > 240,
            "expected local image href to be ignored, got center pixel {center:?}"
        );
    }

    #[test]
    fn embedded_image_limits_reject_large_decode_before_png_or_pdf_encoding() {
        let href = png_data_uri_with_declared_size(100_000, 100_000);
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><image href="{href}" width="10" height="10"/></svg>"#
        );

        let png_err = prepare_raster(&svg, &RasterOptions::default())
            .err()
            .expect("PNG preparation should reject the decoded image size");
        let pdf_err = prepare_pdf(&svg, &PdfOptions::default())
            .err()
            .expect("PDF preparation should reject the decoded image size");

        assert!(png_err.to_string().contains("max_pixels_per_image"));
        assert!(pdf_err.to_string().contains("max_pixels_per_image"));
    }

    #[test]
    fn embedded_image_plan_reports_intrinsic_pixels_from_headers() {
        let href = png_data_uri_with_declared_size(2, 3);
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><image href="{href}" width="10" height="10"/></svg>"#
        );
        let prepared = prepare_raster(&svg, &RasterOptions::default()).unwrap();

        assert_eq!(
            prepared.embedded_image_plan(),
            EmbeddedImagePlan {
                data_resources: 1,
                raster_images: 1,
                largest_data_bytes: 68,
                total_data_bytes: 68,
                largest_raster_pixels: 6,
                total_pixels: 6,
            }
        );
    }

    #[test]
    fn namespaced_image_and_filter_image_share_href_byte_limits() {
        let href = png_data_uri_with_declared_size(1, 1);
        let single_svg =
            format!(r#"<svg xmlns="http://www.w3.org/2000/svg"><image href="{href}"/></svg>"#);
        let bytes_per_resource =
            plan_embedded_data_resources(&single_svg, EmbeddedImageLimit::unbounded())
                .unwrap()
                .total_bytes;
        assert!(bytes_per_resource > 0);
        let total_bytes = bytes_per_resource * 2;
        let svg = format!(
            r#"<svg:svg xmlns:svg="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"><svg:image href="{href}"/><svg:defs><svg:filter id="f"><svg:feImage xlink:href="{href}"/></svg:filter></svg:defs></svg:svg>"#
        );

        let plan = plan_embedded_data_resources(
            &svg,
            EmbeddedImageLimit::new(Some(bytes_per_resource), Some(total_bytes), None, None),
        )
        .expect("the exact per-resource and aggregate limits should be accepted");
        assert_eq!(plan.resources, 2);
        assert_eq!(plan.largest_bytes, bytes_per_resource);
        assert_eq!(plan.total_bytes, total_bytes);

        let per_resource_error = plan_embedded_data_resources(
            &svg,
            EmbeddedImageLimit::new(Some(bytes_per_resource - 1), None, None, None),
        )
        .expect_err("both image element kinds must enforce the per-resource byte limit");
        assert!(matches!(
            per_resource_error,
            ExportError::EmbeddedImageLimit {
                limit_name: "max_bytes_per_image",
                actual,
                max,
            } if actual > max
        ));

        let aggregate_error = plan_embedded_data_resources(
            &svg,
            EmbeddedImageLimit::new(Some(bytes_per_resource), Some(total_bytes - 1), None, None),
        )
        .expect_err("feImage must contribute to the aggregate byte limit");
        assert!(matches!(
            aggregate_error,
            ExportError::EmbeddedImageLimit {
                limit_name: "max_total_bytes",
                actual,
                max,
            } if actual > max
        ));
    }

    #[test]
    fn external_image_hrefs_remain_outside_the_data_url_budget() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"><image href="https://example.invalid/image.png"/><defs><filter id="f"><feImage xlink:href="file:///tmp/image.png"/></filter></defs></svg>"#;

        let plan = plan_embedded_data_resources(
            svg,
            EmbeddedImageLimit::new(Some(1), Some(1), None, None),
        )
        .expect("external references are handled by the separate href resolver policy");

        assert_eq!(plan.resources, 0);
        assert_eq!(plan.largest_bytes, 0);
        assert_eq!(plan.total_bytes, 0);
    }

    #[test]
    fn embedded_image_bytes_are_limited_before_usvg_decodes_data_urls() {
        let href = png_data_uri_with_declared_size(1, 1);
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><image href="{href}" width="10" height="10"/></svg>"#
        );
        let options = RasterOptions {
            embedded_image_limit: EmbeddedImageLimit::new(Some(16), None, None, None),
            ..RasterOptions::default()
        };

        let error = prepare_raster(&svg, &options)
            .err()
            .expect("data URL should be rejected before usvg parsing");

        assert!(error.to_string().contains("max_bytes_per_image"));
    }

    #[test]
    fn filter_image_bytes_are_limited_before_usvg_decodes_data_urls() {
        let href = png_data_uri_with_declared_size(1, 1);
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><defs><filter id="f"><feImage href="{href}"/></filter></defs><rect width="10" height="10" filter="url(#f)"/></svg>"#
        );
        let raster_options = RasterOptions {
            embedded_image_limit: EmbeddedImageLimit::new(Some(16), None, None, None),
            ..RasterOptions::default()
        };
        let pdf_options = PdfOptions {
            embedded_image_limit: EmbeddedImageLimit::new(Some(16), None, None, None),
            ..PdfOptions::default()
        };

        let png_error = prepare_raster(&svg, &raster_options)
            .err()
            .expect("filter data URL should be rejected before raster usvg parsing");
        let pdf_error = prepare_pdf(&svg, &pdf_options)
            .err()
            .expect("filter data URL should be rejected before PDF usvg parsing");

        assert!(png_error.to_string().contains("max_bytes_per_image"));
        assert!(pdf_error.to_string().contains("max_bytes_per_image"));
    }

    #[test]
    fn filter_image_pixels_are_limited_after_header_decode() {
        let href = png_data_uri_with_declared_size(100_000, 100_000);
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 10 10"><defs><filter id="f"><feImage xlink:href="{href}"/></filter></defs><rect width="10" height="10" filter="url(#f)"/></svg>"#
        );

        let png_error = prepare_raster(&svg, &RasterOptions::default())
            .err()
            .expect("filter image intrinsic pixels should be bounded for raster output");
        let pdf_error = prepare_pdf(&svg, &PdfOptions::default())
            .err()
            .expect("filter image intrinsic pixels should be bounded for PDF output");

        assert!(png_error.to_string().contains("max_pixels_per_image"));
        assert!(pdf_error.to_string().contains("max_pixels_per_image"));
    }

    #[test]
    fn embedded_image_limits_cover_pattern_subroots() {
        let href = png_data_uri_with_declared_size(100, 100);
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><defs><pattern id="p" patternUnits="userSpaceOnUse" width="1" height="1"><image href="{href}" width="1" height="1"/></pattern></defs><rect width="10" height="10" fill="url(#p)"/></svg>"#
        );

        let options = RasterOptions {
            embedded_image_limit: EmbeddedImageLimit::new(None, None, Some(10), None),
            ..RasterOptions::default()
        };
        let error = prepare_raster(&svg, &options)
            .err()
            .expect("pattern raster image should be checked");

        assert!(error.to_string().contains("max_pixels_per_image"));
    }

    #[test]
    fn svg_to_pdf_produces_pdf_signature() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="black"/></svg>"#;
        let bytes = svg_to_pdf(svg).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
    }

    #[test]
    fn fixed_pdf_page_policy_uses_requested_media_box() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 20"><rect width="10" height="20" fill="black"/></svg>"#;
        let bytes = svg_to_pdf_with_options(
            svg,
            &PdfOptions::default()
                .with_background("white")
                .with_page_policy(PdfPagePolicy::Fixed {
                    width_pt: 612.0,
                    height_pt: 792.0,
                }),
        )
        .unwrap();
        let pdf = String::from_utf8_lossy(&bytes);
        let media_box = pdf
            .find("/MediaBox")
            .map(|start| &pdf[start..pdf.len().min(start + 80)])
            .expect("fixed PDF media box");

        assert!(
            media_box.contains("612") && media_box.contains("792"),
            "{media_box}"
        );
    }

    #[test]
    fn fixed_pdf_page_policy_rejects_invalid_page_dimensions() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"/>"#;
        let err = svg_to_pdf_with_options(
            svg,
            &PdfOptions::default().with_page_policy(PdfPagePolicy::Fixed {
                width_pt: f32::NAN,
                height_pt: 792.0,
            }),
        )
        .unwrap_err();

        assert!(
            err.to_string().contains("fixed PDF page dimensions"),
            "{err}"
        );
    }

    #[test]
    fn css_width_pdf_page_policy_matches_browser_pixel_to_point_sizing() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 9000 9000"><rect width="9000" height="9000" fill="black"/></svg>"#;
        let bytes = svg_to_pdf_with_options(
            svg,
            &PdfOptions::default().with_page_policy(PdfPagePolicy::FitCssWidth {
                max_width_px: 800.0,
            }),
        )
        .unwrap();
        let pdf = String::from_utf8_lossy(&bytes);
        let media_box = pdf
            .find("/MediaBox")
            .map(|start| &pdf[start..pdf.len().min(start + 80)])
            .expect("CSS-sized PDF media box");

        assert!(media_box.contains("600"), "{media_box}");
    }

    #[test]
    fn fixed_pdf_page_scales_large_vector_source_without_pixel_allocation_limits() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 9000 9000"><rect width="9000" height="9000" fill="black"/></svg>"#;
        let bytes = svg_to_pdf_with_options(
            svg,
            &PdfOptions::default().with_page_policy(PdfPagePolicy::Fixed {
                width_pt: 612.0,
                height_pt: 792.0,
            }),
        )
        .unwrap();

        assert!(bytes.starts_with(b"%PDF-"));
    }

    #[test]
    fn svg_to_pdf_preserves_large_intrinsic_vector_page() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 9000 9000"><rect width="9000" height="9000" fill="black"/></svg>"#;
        let bytes = svg_to_pdf(svg).unwrap();
        let pdf = String::from_utf8_lossy(&bytes);
        let media_box = pdf
            .find("/MediaBox")
            .map(|start| &pdf[start..pdf.len().min(start + 80)])
            .expect("large PDF media box");

        assert!(
            media_box.contains("9000"),
            "large vector dimensions should survive PDF conversion: {media_box}"
        );
    }

    #[test]
    fn svg_to_pdf_rejects_invalid_filter_scale() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"/>"#;
        let err = svg_to_pdf_with_options(svg, &PdfOptions::default().with_filter_scale(0.0))
            .unwrap_err();

        assert!(err.to_string().contains("PDF filter_scale"), "{err}");
    }

    #[test]
    fn pdf_filter_plan_bounds_aggregate_localized_rasterization() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10000 10000">
          <defs><filter id="blur"><feGaussianBlur stdDeviation="2"/></filter></defs>
          <g filter="url(#blur)"><rect width="10000" height="10000" fill="black"/></g>
          <g filter="url(#blur)"><rect width="10000" height="10000" fill="white"/></g>
        </svg>"##;
        let prepared = prepare_pdf(svg, &PdfOptions::default()).unwrap();
        let plan = prepared.filter_plan();

        assert_eq!(plan.filtered_groups, 2);
        assert_eq!(plan.requested_image_pixels, 50_000_000);
        assert!(plan.limited, "{plan:?}");
        assert!(
            plan.effective_image_pixels <= DEFAULT_MAX_PDF_FILTER_IMAGE_PIXELS,
            "{plan:?}"
        );
        assert!(plan.effective_scale < plan.requested_scale, "{plan:?}");
    }

    #[test]
    fn pdf_filter_plan_allows_explicit_trusted_unbounded_policy() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10000 10000">
          <defs><filter id="blur"><feGaussianBlur stdDeviation="2"/></filter></defs>
          <g filter="url(#blur)"><rect width="10000" height="10000" fill="black"/></g>
          <g filter="url(#blur)"><rect width="10000" height="10000" fill="white"/></g>
        </svg>"##;
        let prepared =
            prepare_pdf(svg, &PdfOptions::default().with_unbounded_filter_images()).unwrap();
        let plan = prepared.filter_plan();

        assert_eq!(plan.requested_image_pixels, 50_000_000);
        assert_eq!(plan.effective_image_pixels, 50_000_000);
        assert!(!plan.limited, "{plan:?}");
    }

    #[test]
    fn conversion_plan_rejects_filter_primitive_fanout_before_backend_rendering() {
        let primitives = (0..=DEFAULT_MAX_FILTER_PRIMITIVES_PER_FILTER)
            .map(|index| {
                format!(
                    r#"<feColorMatrix in="SourceGraphic" result="p{index}" type="matrix" values="1 0 0 0 0 0 1 0 0 0 0 0 1 0 0 0 0 0 1 0"/>"#
                )
            })
            .collect::<String>();
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><defs><filter id="f">{primitives}</filter></defs><g filter="url(#f)"><rect width="16" height="16"/></g></svg>"#
        );

        let error = prepare_pdf(&svg, &PdfOptions::default())
            .err()
            .expect("filter primitive fanout should be rejected during preparation");

        assert!(
            error
                .to_string()
                .contains("max_filter_primitives_per_filter"),
            "{error}"
        );
    }

    #[test]
    fn conversion_plan_rejects_deep_isolation_before_backend_rendering() {
        let depth = DEFAULT_MAX_SVG_ISOLATION_DEPTH + 1;
        let mut svg =
            String::from(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16">"#);
        svg.push_str(&r#"<g opacity="0.99">"#.repeat(depth));
        svg.push_str(r#"<rect width="16" height="16"/>"#);
        svg.push_str(&"</g>".repeat(depth));
        svg.push_str("</svg>");

        let error = prepare_raster(&svg, &RasterOptions::default())
            .err()
            .expect("deep isolation should be rejected during preparation");

        assert!(error.to_string().contains("max_isolation_depth"), "{error}");
    }

    #[test]
    fn pdf_filter_plan_counts_only_krilla_owned_outer_filtered_groups() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
          <defs><filter id="blur"><feGaussianBlur stdDeviation="2"/></filter></defs>
          <g filter="url(#blur)"><g filter="url(#blur)"><rect width="100" height="100"/></g></g>
        </svg>"##;

        let prepared = prepare_pdf(svg, &PdfOptions::default()).unwrap();

        assert_eq!(prepared.filter_plan().filtered_groups, 1);
        assert_eq!(prepared.conversion_plan().filtered_groups, 2);
        assert_eq!(prepared.conversion_plan().filter_primitives, 2);
    }

    #[test]
    fn svg_to_jpeg_defaults_to_white_background() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8"></svg>"#;
        let bytes = svg_to_jpeg(svg, &RasterOptions::default()).unwrap();
        let img = image::load_from_memory_with_format(&bytes, image::ImageFormat::Jpeg)
            .unwrap()
            .to_rgb8();
        let px = img.get_pixel(0, 0);

        assert!(
            px[0] > 240 && px[1] > 240 && px[2] > 240,
            "expected default JPG background to be white-ish, got {px:?}"
        );
    }

    #[test]
    fn jpeg_encoder_limit_is_checked_before_allocating_the_pixmap() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 65536 1"><rect width="65536" height="1" fill="black"/></svg>"#;
        let err = svg_to_jpeg(svg, &RasterOptions::default().with_unbounded_size()).unwrap_err();

        assert!(matches!(err, ExportError::JpegDimensionLimit));
    }

    #[test]
    fn svg_to_png_keeps_text_visible_when_requested_font_is_missing() {
        let svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="100%" style="max-width: 400px; background-color: white;"><text x="100" y="40" fill="#333333" font-size="32" style="font-family: '__merman_missing_font__'; text-anchor: middle;">v{}</text></svg>"##,
            merman_core::baseline::PINNED_MERMAID_BASELINE_VERSION
        );
        let bytes = svg_to_png(&svg, &RasterOptions::default()).unwrap();
        assert_png_has_visible_non_background_ink(&bytes);
    }

    #[test]
    fn default_plan_downscales_large_intrinsic_svg_without_allocating() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 14544.4375 6565.5"><rect width="14544.4375" height="6565.5" fill="white"/></svg>"#;
        let plan = svg_raster_plan(svg, &RasterOptions::default()).unwrap();

        assert_eq!(plan.requested_width_px, 14545.0);
        assert_eq!(plan.requested_height_px, 6566.0);
        assert_eq!(plan.width_px, DEFAULT_MAX_RASTER_SIDE_LENGTH);
        assert!(plan.height_px < DEFAULT_MAX_RASTER_SIDE_LENGTH);
        assert!(plan.limited);
        assert!(plan.effective_scale < plan.requested_scale);
    }

    #[test]
    fn fit_to_models_browser_preview_container_before_scale() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1000 500"><rect width="1000" height="500" fill="black"/></svg>"#;
        let options = RasterOptions::default()
            .with_fit_to(RasterFitBox::width(250))
            .with_scale(2.0);
        let plan = svg_raster_plan(svg, &options).unwrap();

        assert_eq!(plan.requested_width_px, 500.0);
        assert_eq!(plan.requested_height_px, 250.0);
        assert_eq!(plan.width_px, 500);
        assert_eq!(plan.height_px, 250);
        assert!(!plan.limited);
    }

    #[test]
    fn size_limit_caps_actual_png_dimensions() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1000 500"><rect width="1000" height="500" fill="black"/></svg>"#;
        let options = RasterOptions::default()
            .with_size_limit(RasterSizeLimit::max_side_length(128))
            .with_background("white");
        let bytes = svg_to_png(svg, &options).unwrap();
        let (width, height) = png_size(&bytes);

        assert_eq!((width, height), (128, 64));
    }

    #[test]
    fn size_limit_caps_by_total_pixels() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1000 1000"><rect width="1000" height="1000" fill="black"/></svg>"#;
        let options = RasterOptions::default().with_size_limit(RasterSizeLimit::new(
            None,
            None,
            Some(10_000),
        ));
        let plan = svg_raster_plan(svg, &options).unwrap();

        assert_eq!((plan.width_px, plan.height_px), (100, 100));
        assert!(plan.limited);
    }

    #[test]
    fn unbounded_size_keeps_requested_dimensions() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 9000 4500"><rect width="9000" height="4500" fill="black"/></svg>"#;
        let plan = svg_raster_plan(svg, &RasterOptions::default().with_unbounded_size()).unwrap();

        assert_eq!((plan.width_px, plan.height_px), (9000, 4500));
        assert!(!plan.limited);
    }

    fn png_size(bytes: &[u8]) -> (u32, u32) {
        let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        let reader = decoder.read_info().expect("png read_info");
        let info = reader.info();
        (info.width, info.height)
    }

    fn rgba_pixel(bytes: &[u8], x: u32, y: u32) -> [u8; 4] {
        let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        let mut reader = decoder.read_info().expect("png read_info");
        let size = reader
            .output_buffer_size()
            .expect("invalid png output buffer size");
        let mut buf = vec![0u8; size];
        let info = reader.next_frame(&mut buf).expect("png next_frame");
        assert_eq!(info.color_type, png::ColorType::Rgba);
        assert_eq!(info.bit_depth, png::BitDepth::Eight);
        assert!(x < info.width && y < info.height);
        let offset = ((y * info.width + x) as usize) * 4;
        [
            buf[offset],
            buf[offset + 1],
            buf[offset + 2],
            buf[offset + 3],
        ]
    }

    fn encode_rgba_png(width: u32, height: u32, data: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().expect("png write_header");
            writer.write_image_data(data).expect("png write_image_data");
        }
        bytes
    }

    fn png_data_uri_with_declared_size(width: u32, height: u32) -> String {
        let mut png = encode_rgba_png(1, 1, &[0, 0, 0, 0]);
        png[16..20].copy_from_slice(&width.to_be_bytes());
        png[20..24].copy_from_slice(&height.to_be_bytes());
        format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(png)
        )
    }

    fn escape_xml_attr(value: &str) -> String {
        value
            .replace('&', "&amp;")
            .replace('"', "&quot;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    }

    struct TempFile {
        path: std::path::PathBuf,
    }

    impl TempFile {
        fn new(extension: &str, data: Vec<u8>) -> Self {
            let path = std::env::temp_dir().join(format!(
                "merman-raster-{}-{}.{}",
                std::process::id(),
                line!(),
                extension
            ));
            std::fs::write(&path, data).expect("write temp image");
            Self { path }
        }

        fn href_path(&self) -> String {
            self.path.to_string_lossy().replace('\\', "/")
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    fn assert_png_has_visible_non_background_ink(bytes: &[u8]) {
        let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        let mut reader = decoder.read_info().expect("png read_info");
        let size = reader
            .output_buffer_size()
            .expect("invalid png output buffer size");
        let mut buf = vec![0u8; size];
        let info = reader.next_frame(&mut buf).expect("png next_frame");

        assert_eq!(
            info.color_type,
            png::ColorType::Rgba,
            "expected RGBA PNG output"
        );
        assert_eq!(
            info.bit_depth,
            png::BitDepth::Eight,
            "expected 8-bit PNG output"
        );

        let pixels = &buf[..info.buffer_size()];
        let Some(background) = pixels.chunks_exact(4).next() else {
            panic!("expected at least one PNG pixel");
        };
        let differing_pixels = pixels
            .chunks_exact(4)
            .filter(|px| {
                let alpha_delta = px[3].abs_diff(background[3]) as u16;
                let rgb_delta = px[0].abs_diff(background[0]) as u16
                    + px[1].abs_diff(background[1]) as u16
                    + px[2].abs_diff(background[2]) as u16;
                alpha_delta > 3 || (px[3] > 0 && rgb_delta > 8)
            })
            .take(16)
            .count();
        assert!(
            differing_pixels >= 8,
            "expected visible text ink in rasterized PNG"
        );
    }
}
