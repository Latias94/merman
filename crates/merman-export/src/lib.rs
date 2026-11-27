#![deny(unsafe_code)]

//! Bounded native font services and binary export for validated SVG.
//!
//! This crate deliberately accepts [`ResvgCompatibleSvg`] rather than Mermaid source or an
//! arbitrary SVG string. Parsing, semantic construction, layout, SVG production, and terminal
//! SVG validation stay owned by `merman`; this crate owns explicit native font contexts and
//! allocation-aware encoding.

#![cfg_attr(
    feature = "native-fonts",
    doc = "
Use the re-exported [`usvg::Options`] with [`NativeFontContext::apply_to_usvg_options`]
to share Merman's SVG parser types."
)]

#[cfg(any(
    feature = "native-fonts",
    all(
        any(feature = "png", feature = "jpeg", feature = "pdf"),
        any(test, all(feature = "system-fonts", not(target_family = "wasm")))
    )
))]
mod font_selection;
#[cfg(all(
    feature = "experimental-system-font-query",
    not(target_family = "wasm")
))]
use font_selection::{query_font_family, query_named_font_family_case_insensitively};

#[cfg(feature = "native-fonts")]
pub mod fonts;
#[cfg(feature = "native-fonts")]
pub use fonts::{
    FontSource, NativeFontContext, NativeFontError, NativeFontLimits, NativeFontOptions,
};

#[cfg(feature = "native-fonts")]
pub use usvg;

/// Returns production Cousine regular, bold, and italic font bytes in that order.
///
/// Chain caller fonts before this iterator to give them precedence over bundled faces.
/// The fonts' OFL license and provenance are retained in `assets/fonts/`.
#[cfg(feature = "bundled-fonts")]
pub fn bundled_font_data() -> impl Iterator<Item = Vec<u8>> {
    [
        include_bytes!("../assets/fonts/Cousine-Regular.ttf").as_slice(),
        include_bytes!("../assets/fonts/Cousine-Bold.ttf").as_slice(),
        include_bytes!("../assets/fonts/Cousine-Italic.ttf").as_slice(),
    ]
    .into_iter()
    .map(<[u8]>::to_vec)
}

/// Reports the actual feature-unified system-font discovery capability of this dependency.
#[cfg(any(
    feature = "native-fonts",
    feature = "system-fonts",
    feature = "png",
    feature = "jpeg",
    feature = "pdf"
))]
#[must_use]
pub const fn system_font_discovery_available() -> bool {
    cfg!(all(feature = "system-fonts", not(target_family = "wasm")))
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
mod font_environment;

#[cfg(feature = "resource-metadata")]
pub mod resource_metadata;

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use font_environment::{ExportFontEnvironment, ExportFontPlanSeed};
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub use font_environment::{ExportFontMode, ExportFontPlan};

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
mod conversion;
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
mod operation;
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
mod viewport;

#[cfg(any(feature = "png", feature = "jpeg"))]
use conversion::directional_filter_transforms_supported;
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use conversion::{plan_embedded_data_resource_bytes, plan_embedded_images, plan_svg_conversion};
#[cfg(feature = "pdf")]
use conversion::{plan_pdf_filter_images, validate_embedded_image_limit};
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use merman_model::OperationControl;
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use merman_render::svg::ResvgCompatibleSvg;
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub use operation::{ExportError, ExportResourceLimitCause, ExportResourceLimitDetails, Result};
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use operation::{RECURSIVE_SVG_BACKEND_STACK_BYTES, export_checkpoint, run_recursive_svg_backend};
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub use resource_metadata::{
    DEFAULT_MAX_DECODED_IMAGE_PIXELS, DEFAULT_MAX_EMBEDDED_IMAGE_BYTES,
    DEFAULT_MAX_FILTER_PRIMITIVES_PER_FILTER, DEFAULT_MAX_NESTED_SVG_IMAGES,
    DEFAULT_MAX_SVG_ISOLATION_DEPTH, DEFAULT_MAX_SVG_SUBROOTS,
    DEFAULT_MAX_TOTAL_DECODED_IMAGE_PIXELS, DEFAULT_MAX_TOTAL_EMBEDDED_IMAGE_BYTES,
    DEFAULT_MAX_TOTAL_FILTER_PRIMITIVES, ExportResourceLimitDescriptor,
    MAX_EMBEDDED_IMAGE_BYTES_RESOURCE_LIMIT_ID, MAX_EMBEDDED_IMAGE_PIXELS_RESOURCE_LIMIT_ID,
    MAX_NESTED_SVG_IMAGES_RESOURCE_LIMIT_ID,
    MAX_SVG_CONVERSION_FILTER_PRIMITIVES_PER_FILTER_RESOURCE_LIMIT_ID,
    MAX_SVG_CONVERSION_ISOLATION_DEPTH_RESOURCE_LIMIT_ID,
    MAX_SVG_CONVERSION_SUBROOTS_RESOURCE_LIMIT_ID,
    MAX_TOTAL_EMBEDDED_IMAGE_BYTES_RESOURCE_LIMIT_ID,
    MAX_TOTAL_EMBEDDED_IMAGE_PIXELS_RESOURCE_LIMIT_ID,
    MAX_TOTAL_SVG_CONVERSION_FILTER_PRIMITIVES_RESOURCE_LIMIT_ID,
};
#[cfg(feature = "pdf")]
pub use resource_metadata::{
    DEFAULT_MAX_PDF_FILTER_IMAGE_PIXELS, MAX_PDF_FILTER_IMAGE_PIXELS_RESOURCE_LIMIT_ID,
};
#[cfg(any(feature = "png", feature = "jpeg"))]
pub use resource_metadata::{
    DEFAULT_MAX_RASTER_PIXELS, DEFAULT_MAX_RASTER_SIDE_LENGTH, MAX_RASTER_HEIGHT_RESOURCE_LIMIT_ID,
    MAX_RASTER_PIXELS_RESOURCE_LIMIT_ID, MAX_RASTER_WIDTH_RESOURCE_LIMIT_ID,
};
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use viewport::parse_root_svg_metadata;
#[cfg(any(feature = "png", feature = "jpeg"))]
pub use viewport::{RasterFitBox, RasterSizeLimit};
#[cfg(any(feature = "png", feature = "jpeg"))]
use viewport::{
    RasterGeometry, raster_geometry_for_svg, raster_plan_for_geometry, raster_usvg_options,
};

/// Returns the export limits compiled into the concrete feature closure.
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[must_use]
pub fn export_resource_limit_descriptors() -> Vec<ExportResourceLimitDescriptor> {
    resource_metadata::ExportResourceLimits::COMPILED.descriptors()
}

/// Returns the outputs that can enforce one export-owned resource limit.
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[must_use]
pub fn export_resource_limit_output_ids(stable_id: &str) -> Option<&'static [&'static str]> {
    resource_metadata::ExportResourceLimits::COMPILED.output_ids(stable_id)
}

/// Returns the profile value for an export-owned resource limit.
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[must_use]
pub fn export_resource_profile_value(
    profile: merman_render::resources::RenderResourceProfile,
    stable_id: &str,
) -> Option<Option<usize>> {
    resource_metadata::ExportResourceLimits::COMPILED.profile_value(profile, stable_id)
}

#[cfg(feature = "pdf")]
const PDF_POINTS_PER_CSS_PIXEL: f32 = 72.0 / 96.0;
#[cfg(any(feature = "png", feature = "jpeg"))]
#[derive(Debug, Clone)]
pub struct RasterOptions {
    pub scale: f32,
    /// Optional solid color composited behind SVG pixels. This is not the diagram theme canvas.
    pub matte: Option<String>,
    pub jpeg_quality: u8,
    pub fit_to: Option<RasterFitBox>,
    pub size_limit: RasterSizeLimit,
    pub embedded_image_limit: EmbeddedImageLimit,
    pub conversion_limits: SvgConversionLimits,
    #[cfg(feature = "native-fonts")]
    pub font_context: Option<NativeFontContext>,
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

/// Optional host-system font capability shared by native PNG, JPEG, and PDF exporters.
///
/// This static contract reports what a compiled backend can do. A concrete export consults this
/// source only when its sealed font-source policy permits it; [`ExportFontPlan`] reports whether
/// usvg actually selected a host face.
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct SystemFontEnvironmentContract {
    pub source_id: &'static str,
    pub discovery: &'static str,
    pub cache_scope: &'static str,
    pub host_dependent: bool,
    pub resource_bounded: bool,
}

/// Embedded-image behavior shared by the native PNG, JPEG, and PDF exporters.
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct EmbeddedImageEnvironmentContract {
    pub source_ids: &'static [&'static str],
    pub filesystem_access: bool,
    pub network_access: bool,
    pub default_limits: EmbeddedImageLimit,
}

/// Runtime environment facts owned by a compiled export backend.
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ExportEnvironmentContract {
    /// System font discovery available to this target, or `None` when it cannot discover fonts.
    /// Availability does not imply that every operation consults or selects a system font.
    pub system_fonts: Option<SystemFontEnvironmentContract>,
    pub embedded_images: EmbeddedImageEnvironmentContract,
}

/// Returns the runtime environment contract for a compiled native export output.
///
/// SVG and ASCII do not pass through this exporter and therefore return `None`.
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[must_use]
pub fn output_environment_contract(output_id: &str) -> Option<ExportEnvironmentContract> {
    let compiled = match output_id {
        #[cfg(feature = "png")]
        "png" => true,
        #[cfg(feature = "jpeg")]
        "jpeg" => true,
        #[cfg(feature = "pdf")]
        "pdf" => true,
        _ => false,
    };
    compiled.then_some(ExportEnvironmentContract {
        system_fonts: system_font_discovery_available().then_some(SystemFontEnvironmentContract {
            source_id: "host-system",
            discovery: "first-use",
            cache_scope: "process-global",
            host_dependent: true,
            resource_bounded: false,
        }),
        embedded_images: EmbeddedImageEnvironmentContract {
            source_ids: &["data-url"],
            filesystem_access: false,
            network_access: false,
            default_limits: EmbeddedImageLimit::default_safe(),
        },
    })
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
    /// Optional solid paint behind SVG content on the PDF page.
    pub page_paint: Option<String>,
    /// Requested sampling scale for SVG filters embedded in the PDF.
    pub filter_scale: f32,
    /// Aggregate budget for localized filter images retained in the PDF.
    pub filter_image_limit: PdfFilterImageLimit,
    /// Header-derived pixel budget for embedded PNG/JPEG/GIF/WebP images.
    pub embedded_image_limit: EmbeddedImageLimit,
    /// Structural budget shared with PNG/JPEG conversion.
    pub conversion_limits: SvgConversionLimits,
    #[cfg(feature = "native-fonts")]
    pub font_context: Option<NativeFontContext>,
}

#[cfg(feature = "pdf")]
impl Default for PdfOptions {
    fn default() -> Self {
        Self {
            page_policy: PdfPagePolicy::FitSvg,
            page_paint: None,
            filter_scale: 4.0,
            filter_image_limit: PdfFilterImageLimit::default(),
            embedded_image_limit: EmbeddedImageLimit::default(),
            conversion_limits: SvgConversionLimits::default(),
            #[cfg(feature = "native-fonts")]
            font_context: None,
        }
    }
}

#[cfg(feature = "pdf")]
impl PdfOptions {
    #[cfg(feature = "native-fonts")]
    pub fn with_font_context(mut self, font_context: NativeFontContext) -> Self {
        self.font_context = Some(font_context);
        self
    }

    pub fn with_page_policy(mut self, page_policy: PdfPagePolicy) -> Self {
        self.page_policy = page_policy;
        self
    }

    pub fn with_page_paint(mut self, page_paint: impl Into<String>) -> Self {
        self.page_paint = Some(page_paint.into());
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
            matte: None,
            jpeg_quality: 90,
            fit_to: None,
            size_limit: RasterSizeLimit::default(),
            embedded_image_limit: EmbeddedImageLimit::default(),
            conversion_limits: SvgConversionLimits::default(),
            #[cfg(feature = "native-fonts")]
            font_context: None,
        }
    }
}

#[cfg(any(feature = "png", feature = "jpeg"))]
impl RasterOptions {
    /// The opaque matte used for JPEG when the caller supplies no matte.
    pub const DEFAULT_JPEG_MATTE: ExportRgbaColor = ExportRgbaColor::WHITE;

    #[cfg(feature = "native-fonts")]
    pub fn with_font_context(mut self, font_context: NativeFontContext) -> Self {
        self.font_context = Some(font_context);
        self
    }

    pub fn with_scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    pub fn with_matte(mut self, matte: impl Into<String>) -> Self {
        self.matte = Some(matte.into());
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
    /// Total resolved usvg nodes traversed before backend conversion.
    pub tree_nodes: usize,
    /// Maximum resolved `usvg` group depth observed before backend conversion.
    pub max_tree_depth: usize,
    pub max_isolation_depth: usize,
    pub filtered_groups: usize,
    pub filter_primitives: usize,
    pub subroots: usize,
    pub nested_svg_images: usize,
}

/// Solid RGBA color accepted by native export compositing.
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportRgbaColor {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
impl ExportRgbaColor {
    pub const WHITE: Self = Self {
        red: 255,
        green: 255,
        blue: 255,
        alpha: 255,
    };

    /// Reads a solid color using the native export color grammar.
    pub fn parse(text: &str) -> Option<Self> {
        parse_export_color(text).ok()
    }

    pub const fn red(self) -> u8 {
        self.red
    }

    pub const fn green(self) -> u8 {
        self.green
    }

    pub const fn blue(self) -> u8 {
        self.blue
    }

    pub const fn alpha(self) -> u8 {
        self.alpha
    }

    pub const fn is_opaque(self) -> bool {
        self.alpha == 255
    }
}

/// Concrete pixel output selected from one prepared raster tree.
#[cfg(any(feature = "png", feature = "jpeg"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RasterOutputKind {
    Png,
    Jpeg,
}

/// Frozen resource, allocation, font, and compositing evidence for one raster export.
#[cfg(any(feature = "png", feature = "jpeg"))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RasterExportReport {
    directional_filter_transform_residual: bool,
    fallback_css_residual: bool,
    output: RasterOutputKind,
    raster: RasterPlan,
    embedded_images: EmbeddedImagePlan,
    conversion: SvgConversionPlan,
    fonts: ExportFontPlan,
    matte: Option<ExportRgbaColor>,
    matte_defaulted: bool,
}

#[cfg(any(feature = "png", feature = "jpeg"))]
impl RasterExportReport {
    /// Returns whether directional filter offsets may differ from their SVG transform.
    ///
    /// Raster export supports transforms that preserve or swap the coordinate axes. Best-effort
    /// export reports the residual for other transforms that can resample the filtered layer.
    pub const fn directional_filter_transform_residual(self) -> bool {
        self.directional_filter_transform_residual
    }

    /// Returns whether converted HTML label styles can differ in the native CSS backend.
    ///
    /// This quality observation is retained from the sealed SVG's terminal validation. It does
    /// not authorize resources, relax SVG safety, or prevent best-effort image encoding.
    pub const fn fallback_css_residual(self) -> bool {
        self.fallback_css_residual
    }

    pub const fn output(self) -> RasterOutputKind {
        self.output
    }

    pub const fn raster(self) -> RasterPlan {
        self.raster
    }

    pub const fn embedded_images(self) -> EmbeddedImagePlan {
        self.embedded_images
    }

    pub const fn conversion(self) -> SvgConversionPlan {
        self.conversion
    }

    pub const fn fonts(self) -> ExportFontPlan {
        self.fonts
    }

    /// Returns the effective output matte. JPEG always reports an opaque value.
    pub const fn matte(self) -> Option<ExportRgbaColor> {
        self.matte
    }

    /// Returns whether the output format supplied the matte rather than the caller.
    pub const fn matte_defaulted(self) -> bool {
        self.matte_defaulted
    }
}

#[cfg(any(feature = "png", feature = "jpeg"))]
#[derive(Debug, Clone, Copy)]
struct RasterReportSeed {
    directional_filter_transform_residual: bool,
    fallback_css_residual: bool,
    raster: RasterPlan,
    embedded_images: EmbeddedImagePlan,
    conversion: SvgConversionPlan,
    fonts: ExportFontPlan,
}

/// PDF page geometry frozen before filter planning and encoding.
#[cfg(feature = "pdf")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PdfPagePlan {
    page_width_pt: f32,
    page_height_pt: f32,
    drawing_width_pt: f32,
    drawing_height_pt: f32,
    offset_x_pt: f32,
    offset_y_pt: f32,
}

#[cfg(feature = "pdf")]
impl PdfPagePlan {
    pub const fn page_width_pt(self) -> f32 {
        self.page_width_pt
    }

    pub const fn page_height_pt(self) -> f32 {
        self.page_height_pt
    }

    pub const fn drawing_width_pt(self) -> f32 {
        self.drawing_width_pt
    }

    pub const fn drawing_height_pt(self) -> f32 {
        self.drawing_height_pt
    }

    pub const fn offset_x_pt(self) -> f32 {
        self.offset_x_pt
    }

    pub const fn offset_y_pt(self) -> f32 {
        self.offset_y_pt
    }
}

/// Frozen resource, page, filter, font, and compositing evidence for one PDF export.
#[cfg(feature = "pdf")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PdfExportReport {
    directional_filter_transform_residual: bool,
    implicit_viewport_clipping_residual: bool,
    fallback_css_residual: bool,
    page: PdfPagePlan,
    filters: PdfFilterImagePlan,
    embedded_images: EmbeddedImagePlan,
    conversion: SvgConversionPlan,
    fonts: ExportFontPlan,
    page_paint: Option<ExportRgbaColor>,
}

#[cfg(feature = "pdf")]
impl PdfExportReport {
    /// Returns whether offsets inside a localized filter image may paint in a different direction.
    ///
    /// Vector ancestor transforms remain supported. A transform on the rasterized group or its
    /// descendants can expose the raster backend's offset limitation. Best-effort export still
    /// produces the diagram and reports this paint-quality residual.
    pub const fn directional_filter_transform_residual(self) -> bool {
        self.directional_filter_transform_residual
    }

    /// Returns whether a dimensionless SVG's derived PDF viewport can clip its layer bounds.
    ///
    /// This reports best-effort output without changing the sealed SVG. An explicit viewBox or
    /// width and height define a caller-owned viewport and do not produce this residual.
    pub const fn implicit_viewport_clipping_residual(self) -> bool {
        self.implicit_viewport_clipping_residual
    }

    /// Returns whether converted HTML label styles can differ in the native CSS backend.
    ///
    /// This quality observation is retained from the sealed SVG's terminal validation. It does
    /// not authorize resources, relax SVG safety, or prevent best-effort PDF encoding.
    pub const fn fallback_css_residual(self) -> bool {
        self.fallback_css_residual
    }

    pub const fn page(self) -> PdfPagePlan {
        self.page
    }

    pub const fn filters(self) -> PdfFilterImagePlan {
        self.filters
    }

    pub const fn embedded_images(self) -> EmbeddedImagePlan {
        self.embedded_images
    }

    pub const fn conversion(self) -> SvgConversionPlan {
        self.conversion
    }

    pub const fn fonts(self) -> ExportFontPlan {
        self.fonts
    }

    pub const fn page_paint(self) -> Option<ExportRgbaColor> {
        self.page_paint
    }
}

/// Parsed raster input shared by sizing, memory scheduling, and image encoding.
#[cfg(any(feature = "png", feature = "jpeg"))]
pub struct PreparedRaster {
    tree: usvg::Tree,
    geometry: RasterGeometry,
    translate_min_to_origin: bool,
    report: RasterReportSeed,
    matte: Option<ExportRgbaColor>,
    #[cfg(feature = "jpeg")]
    jpeg_quality: u8,
    control: OperationControl,
    #[cfg(feature = "native-fonts")]
    font_context: Option<NativeFontContext>,
}

#[cfg(any(feature = "png", feature = "jpeg"))]
impl PreparedRaster {
    /// Returns the allocation plan computed before any output pixmap is created.
    pub const fn plan(&self) -> RasterPlan {
        self.report.raster
    }

    /// Returns the embedded raster image plan computed from image headers.
    pub const fn embedded_image_plan(&self) -> EmbeddedImagePlan {
        self.report.embedded_images
    }

    pub const fn conversion_plan(&self) -> SvgConversionPlan {
        self.report.conversion
    }

    /// Returns the complete report that would be used for the selected raster output.
    ///
    /// The report is frozen during preparation. JPEG's implicit white matte is reflected here;
    /// the encoder still performs its format-specific dimension and opacity checks before bytes
    /// are emitted.
    pub fn report_for_output(&self, output: RasterOutputKind) -> RasterExportReport {
        let matte_defaulted = matches!(output, RasterOutputKind::Jpeg) && self.matte.is_none();
        let matte = if matte_defaulted {
            Some(RasterOptions::DEFAULT_JPEG_MATTE)
        } else {
            self.matte
        };
        self.report_for(output, matte, matte_defaulted)
    }

    fn report_for(
        &self,
        output: RasterOutputKind,
        matte: Option<ExportRgbaColor>,
        matte_defaulted: bool,
    ) -> RasterExportReport {
        RasterExportReport {
            directional_filter_transform_residual: self
                .report
                .directional_filter_transform_residual,
            fallback_css_residual: self.report.fallback_css_residual,
            output,
            raster: self.report.raster,
            embedded_images: self.report.embedded_images,
            conversion: self.report.conversion,
            fonts: self.report.fonts,
            matte,
            matte_defaulted,
        }
    }

    /// Returns an advisory weight for scheduling parallel PNG jobs.
    ///
    /// This is not a hard memory bound for resvg internals.
    pub fn png_scheduling_weight_bytes(&self) -> u64 {
        encoding_scheduling_weight_bytes(self.report.raster, self.report.embedded_images, 8)
    }

    /// Returns an advisory weight for scheduling parallel JPEG jobs.
    ///
    /// This is not a hard memory bound for resvg internals.
    pub fn jpeg_scheduling_weight_bytes(&self) -> u64 {
        encoding_scheduling_weight_bytes(self.report.raster, self.report.embedded_images, 10)
    }

    /// Allocates and encodes the prepared image as PNG.
    #[cfg(feature = "png")]
    pub fn encode_png(self) -> Result<Vec<u8>> {
        self.encode_png_with_report().map(|(bytes, _)| bytes)
    }

    /// Allocates and encodes PNG together with the exact resources and compositing evidence used.
    #[cfg(feature = "png")]
    pub fn encode_png_with_report(self) -> Result<(Vec<u8>, RasterExportReport)> {
        let control = self.control.clone();
        #[cfg(feature = "native-fonts")]
        let font_context = self.font_context.clone();
        run_recursive_svg_backend(&control, move |control| {
            export_checkpoint(control)?;
            let matte = self.matte;
            let report = self.report_for_output(RasterOutputKind::Png);
            let pixmap = self.into_pixmap(matte, control)?;
            export_checkpoint(control)?;
            let bytes = pixmap.encode_png().map_err(|_| ExportError::PngEncode);
            export_checkpoint(control)?;
            let bytes = bytes?;
            #[cfg(feature = "native-fonts")]
            if let Some(context) = &font_context {
                context.check()?;
            }
            Ok((bytes, report))
        })
    }

    /// Allocates and encodes the prepared image as JPEG.
    #[cfg(feature = "jpeg")]
    pub fn encode_jpeg(self) -> Result<Vec<u8>> {
        self.encode_jpeg_with_report().map(|(bytes, _)| bytes)
    }

    /// Allocates and encodes JPEG together with the exact resources and compositing evidence used.
    #[cfg(feature = "jpeg")]
    pub fn encode_jpeg_with_report(self) -> Result<(Vec<u8>, RasterExportReport)> {
        let control = self.control.clone();
        #[cfg(feature = "native-fonts")]
        let font_context = self.font_context.clone();
        run_recursive_svg_backend(&control, move |control| {
            export_checkpoint(control)?;
            if self.report.raster.width_px > u32::from(u16::MAX)
                || self.report.raster.height_px > u32::from(u16::MAX)
            {
                return Err(ExportError::JpegDimensionLimit);
            }
            let matte = self.matte.unwrap_or(RasterOptions::DEFAULT_JPEG_MATTE);
            if !matte.is_opaque() {
                return Err(ExportError::JpegOpaqueMatteRequired);
            }

            let report = self.report_for_output(RasterOutputKind::Jpeg);
            let quality = self.jpeg_quality;
            let pixmap = self.into_pixmap(Some(matte), control)?;
            export_checkpoint(control)?;
            let (w, h) = (pixmap.width(), pixmap.height());
            let rgba = pixmap.data();
            let mut rgb = vec![0u8; (w as usize) * (h as usize) * 3];
            for (src, dst) in rgba
                .as_chunks::<4>()
                .0
                .iter()
                .zip(rgb.as_chunks_mut::<3>().0.iter_mut())
            {
                export_checkpoint(control)?;
                dst[0] = src[0];
                dst[1] = src[1];
                dst[2] = src[2];
            }

            let mut out = Vec::new();
            let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
            export_checkpoint(control)?;
            let encoded = enc
                .encode(&rgb, w, h, image::ExtendedColorType::Rgb8)
                .map_err(|_| ExportError::JpegEncode);
            export_checkpoint(control)?;
            encoded?;
            #[cfg(feature = "native-fonts")]
            if let Some(context) = &font_context {
                context.check()?;
            }
            Ok((out, report))
        })
    }

    fn render_pixmap(
        &self,
        matte: Option<ExportRgbaColor>,
        control: &OperationControl,
    ) -> Result<tiny_skia::Pixmap> {
        export_checkpoint(control)?;
        let mut pixmap =
            tiny_skia::Pixmap::new(self.report.raster.width_px, self.report.raster.height_px)
                .ok_or(ExportError::PixmapAlloc)?;

        if let Some(matte) = matte {
            pixmap.fill(tiny_skia_color(matte));
        }

        let scale = self.report.raster.effective_scale as f32;
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

        // resvg is one opaque synchronous backend call. Cooperative cancellation is observed at
        // its boundaries; hosts that need hard preemption must isolate the worker or process.
        export_checkpoint(control)?;
        resvg::render(&self.tree, transform, &mut pixmap.as_mut());
        export_checkpoint(control)?;
        #[cfg(feature = "native-fonts")]
        if let Some(context) = &self.font_context {
            context.check()?;
        }
        Ok(pixmap)
    }

    fn into_pixmap(
        self,
        matte: Option<ExportRgbaColor>,
        control: &OperationControl,
    ) -> Result<tiny_skia::Pixmap> {
        self.render_pixmap(matte, control)
    }
}

/// Parsed vector PDF input shared by page planning, memory scheduling, and encoding.
#[cfg(feature = "pdf")]
pub struct PreparedPdf {
    tree: usvg::Tree,
    layout: PdfPageLayout,
    filter_scale: f32,
    page_paint: Option<ExportRgbaColor>,
    report: PdfExportReport,
    control: OperationControl,
    #[cfg(feature = "native-fonts")]
    font_context: Option<NativeFontContext>,
}

#[cfg(feature = "pdf")]
impl PreparedPdf {
    /// Returns the complete report frozen before PDF encoding.
    pub const fn report(&self) -> PdfExportReport {
        self.report
    }

    /// Returns the localized filter allocation plan computed before PDF encoding.
    pub const fn filter_plan(&self) -> PdfFilterImagePlan {
        self.report.filters
    }

    /// Returns the embedded raster image plan computed from image headers.
    pub const fn embedded_image_plan(&self) -> EmbeddedImagePlan {
        self.report.embedded_images
    }

    pub const fn conversion_plan(&self) -> SvgConversionPlan {
        self.report.conversion
    }

    /// Returns an advisory weight for scheduling parallel PDF jobs.
    ///
    /// This is not a hard memory bound for krilla-svg or resvg internals.
    pub fn scheduling_weight_bytes(&self) -> u64 {
        const PDF_ENCODER_OVERHEAD_BYTES: u64 = 1024 * 1024;
        self.report
            .filters
            .effective_image_pixels
            .saturating_mul(8)
            .saturating_add(self.report.embedded_images.total_pixels.saturating_mul(8))
            .saturating_add(PDF_ENCODER_OVERHEAD_BYTES)
            .saturating_add(RECURSIVE_SVG_BACKEND_STACK_BYTES as u64)
    }

    /// Encodes the prepared tree as vector PDF, rasterizing only SVG filter regions.
    pub fn encode(self) -> Result<Vec<u8>> {
        self.encode_with_report().map(|(bytes, _)| bytes)
    }

    /// Encodes PDF together with the exact resources, page, and filter evidence used.
    pub fn encode_with_report(self) -> Result<(Vec<u8>, PdfExportReport)> {
        let control = self.control.clone();
        #[cfg(feature = "native-fonts")]
        let font_context = self.font_context.clone();
        run_recursive_svg_backend(&control, move |control| {
            let bytes = svg_tree_to_pdf(
                &self.tree,
                self.layout,
                self.filter_scale,
                self.page_paint,
                control,
            )?;
            #[cfg(feature = "native-fonts")]
            if let Some(context) = &font_context {
                context.check()?;
            }
            Ok((bytes, self.report))
        })
    }
}

/// Parses a sealed SVG once and prepares its bounded raster allocation plan.
#[cfg(any(feature = "png", feature = "jpeg"))]
pub fn prepare_raster(svg: &ResvgCompatibleSvg, options: &RasterOptions) -> Result<PreparedRaster> {
    prepare_raster_controlled(svg, options, OperationControl::new())
}

/// Parses a sealed SVG using caller-owned cooperative cancellation/deadline state.
#[cfg(any(feature = "png", feature = "jpeg"))]
pub fn prepare_raster_controlled(
    svg: &ResvgCompatibleSvg,
    options: &RasterOptions,
    control: OperationControl,
) -> Result<PreparedRaster> {
    let fonts = SvgExportFonts::from_options(
        svg,
        #[cfg(feature = "native-fonts")]
        options.font_context.as_ref(),
    );
    prepare_raster_with_fonts(svg, options, control, fonts)
}

/// Prepares raster export with a caller-supplied font policy instead of `options.font_context`.
///
/// Calls `load` at most once. A `true` argument requires a context for actual converted text.
/// For a successfully parsed fontless SVG, passes `false` so explicitly supplied sources can
/// still be validated without discovering system fonts. Returning `None` is allowed only then.
#[cfg(all(feature = "native-fonts", any(feature = "png", feature = "jpeg")))]
pub fn prepare_raster_with_font_loader_controlled(
    svg: &ResvgCompatibleSvg,
    options: &RasterOptions,
    control: OperationControl,
    load: impl FnOnce(
        bool,
        &OperationControl,
    ) -> std::result::Result<Option<NativeFontContext>, NativeFontError>
    + Send,
) -> Result<PreparedRaster> {
    prepare_raster_with_fonts(
        svg,
        options,
        control,
        SvgExportFonts::Deferred(Box::new(load)),
    )
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn prepare_raster_with_fonts(
    svg: &ResvgCompatibleSvg,
    options: &RasterOptions,
    control: OperationControl,
    fonts: SvgExportFonts<'_>,
) -> Result<PreparedRaster> {
    export_checkpoint(&control)?;
    let backend_control = control.clone();
    run_recursive_svg_backend(&control, move |control| {
        prepare_raster_on_backend_stack(svg, options, fonts, control).map(|mut prepared| {
            prepared.control = backend_control;
            prepared
        })
    })
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn prepare_raster_on_backend_stack(
    svg: &ResvgCompatibleSvg,
    options: &RasterOptions,
    fonts: SvgExportFonts<'_>,
    control: &OperationControl,
) -> Result<PreparedRaster> {
    export_checkpoint(control)?;
    let matte = options
        .matte
        .as_deref()
        .map(parse_export_color)
        .transpose()
        .map_err(|_| ExportError::InvalidRasterMatte)?;
    let root_metadata = parse_root_svg_metadata(svg.as_str(), control)?;
    let data_plan = plan_embedded_data_resource_bytes(
        svg.inline_raster_sources(),
        options.embedded_image_limit,
        control,
    )?;
    let ParsedExportSvg {
        tree,
        font_plan,
        #[cfg(feature = "native-fonts")]
        font_context,
    } = parse_svg_tree(
        svg.as_str(),
        raster_usvg_options(root_metadata),
        fonts,
        control,
    )?;
    let (geometry, translate_min_to_origin) = raster_geometry_for_svg(root_metadata, &tree);
    let fallback_css_residual = svg.finalization_report().fallback_css_residual();
    let conversion_plan = plan_svg_conversion(&tree, options.conversion_limits, control)?;
    let directional_filter_transform_residual =
        !directional_filter_transforms_supported(tree.root(), control)?;
    let embedded_image_plan =
        plan_embedded_images(&tree, options.embedded_image_limit, data_plan, control)?;
    let plan = raster_plan_for_geometry(geometry, options, control)?;
    export_checkpoint(control)?;

    Ok(PreparedRaster {
        tree,
        geometry,
        translate_min_to_origin,
        report: RasterReportSeed {
            directional_filter_transform_residual,
            fallback_css_residual,
            raster: plan,
            embedded_images: embedded_image_plan,
            conversion: conversion_plan,
            fonts: font_plan,
        },
        matte,
        #[cfg(feature = "jpeg")]
        jpeg_quality: options.jpeg_quality,
        control: control.clone(),
        #[cfg(feature = "native-fonts")]
        font_context,
    })
}

/// Parses a sealed SVG once and prepares vector PDF page and filter allocation policy.
#[cfg(feature = "pdf")]
pub fn prepare_pdf(svg: &ResvgCompatibleSvg, options: &PdfOptions) -> Result<PreparedPdf> {
    prepare_pdf_controlled(svg, options, OperationControl::new())
}

/// Parses a sealed SVG for PDF using caller-owned cooperative cancellation/deadline state.
#[cfg(feature = "pdf")]
pub fn prepare_pdf_controlled(
    svg: &ResvgCompatibleSvg,
    options: &PdfOptions,
    control: OperationControl,
) -> Result<PreparedPdf> {
    let fonts = SvgExportFonts::from_options(
        svg,
        #[cfg(feature = "native-fonts")]
        options.font_context.as_ref(),
    );
    prepare_pdf_with_fonts(svg, options, control, fonts)
}

/// Prepares PDF export with a caller-supplied font policy instead of `options.font_context`.
///
/// The loader runs at most once and receives the same actual text-demand decision as raster
/// export. It may return `None` only when the successfully parsed SVG needs no fonts.
#[cfg(all(feature = "native-fonts", feature = "pdf"))]
pub fn prepare_pdf_with_font_loader_controlled(
    svg: &ResvgCompatibleSvg,
    options: &PdfOptions,
    control: OperationControl,
    load: impl FnOnce(
        bool,
        &OperationControl,
    ) -> std::result::Result<Option<NativeFontContext>, NativeFontError>
    + Send,
) -> Result<PreparedPdf> {
    prepare_pdf_with_fonts(
        svg,
        options,
        control,
        SvgExportFonts::Deferred(Box::new(load)),
    )
}

#[cfg(feature = "pdf")]
fn prepare_pdf_with_fonts(
    svg: &ResvgCompatibleSvg,
    options: &PdfOptions,
    control: OperationControl,
    fonts: SvgExportFonts<'_>,
) -> Result<PreparedPdf> {
    export_checkpoint(&control)?;
    let backend_control = control.clone();
    run_recursive_svg_backend(&control, move |control| {
        prepare_pdf_on_backend_stack(svg, options, fonts, control).map(|mut prepared| {
            prepared.control = backend_control;
            prepared
        })
    })
}

#[cfg(feature = "pdf")]
fn prepare_pdf_on_backend_stack(
    svg: &ResvgCompatibleSvg,
    options: &PdfOptions,
    fonts: SvgExportFonts<'_>,
    control: &OperationControl,
) -> Result<PreparedPdf> {
    export_checkpoint(control)?;
    let source = svg.as_str();
    validate_pdf_options(options)?;
    let page_paint = options
        .page_paint
        .as_deref()
        .map(parse_export_color)
        .transpose()
        .map_err(|_| ExportError::InvalidPdfPagePaint)?;
    let data_plan = plan_embedded_data_resource_bytes(
        svg.inline_raster_sources(),
        options.embedded_image_limit,
        control,
    )?;
    let ParsedExportSvg {
        tree,
        font_plan,
        #[cfg(feature = "native-fonts")]
        font_context,
    } = parse_svg_tree(source, usvg::Options::default(), fonts, control)?;
    let fallback_css_residual = svg.finalization_report().fallback_css_residual();
    let metadata = parse_root_svg_metadata(source, control)?;
    let implicit_viewport_clipping_residual = !metadata.has_view_box
        && !metadata.has_explicit_size
        && tree.root().abs_layer_bounding_box().is_some_and(|layer| {
            let viewport = tree.size();
            layer.left() < 0.0
                || layer.top() < 0.0
                || layer.right() > viewport.width()
                || layer.bottom() > viewport.height()
        });
    let conversion_plan = plan_svg_conversion(&tree, options.conversion_limits, control)?;
    let embedded_image_plan =
        plan_embedded_images(&tree, options.embedded_image_limit, data_plan, control)?;
    let svg_size = pdf_svg_size(&tree)?;
    let layout = pdf_page_layout(svg_size, options.page_policy)?;
    let (filter_plan, directional_filter_transform_residual) = plan_pdf_filter_images(
        &tree,
        layout.drawing_size.width() / svg_size.width(),
        options.filter_scale,
        options.filter_image_limit,
        control,
    )?;
    let report = PdfExportReport {
        directional_filter_transform_residual,
        implicit_viewport_clipping_residual,
        fallback_css_residual,
        page: layout.plan(),
        filters: filter_plan,
        embedded_images: embedded_image_plan,
        conversion: conversion_plan,
        fonts: font_plan,
        page_paint,
    };

    Ok(PreparedPdf {
        tree,
        layout,
        filter_scale: filter_plan.effective_scale,
        page_paint,
        report,
        control: control.clone(),
        #[cfg(feature = "native-fonts")]
        font_context,
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
    svg_to_png_controlled(svg, options, OperationControl::new())
}

/// Encodes a sealed SVG as PNG using caller-owned cooperative cancellation/deadline state.
#[cfg(feature = "png")]
pub fn svg_to_png_controlled(
    svg: &ResvgCompatibleSvg,
    options: &RasterOptions,
    control: OperationControl,
) -> Result<Vec<u8>> {
    prepare_raster_controlled(svg, options, control)?.encode_png()
}

/// Encodes a sealed SVG as PNG and returns the allocation plan used for the output pixmap.
#[cfg(feature = "png")]
pub fn svg_to_png_with_plan_controlled(
    svg: &ResvgCompatibleSvg,
    options: &RasterOptions,
    control: OperationControl,
) -> Result<(Vec<u8>, RasterPlan)> {
    let prepared = prepare_raster_controlled(svg, options, control)?;
    let plan = prepared.plan();
    let bytes = prepared.encode_png()?;
    Ok((bytes, plan))
}

/// Encodes a sealed SVG as PNG and returns its allocation plan using a fresh control.
#[cfg(feature = "png")]
pub fn svg_to_png_with_plan(
    svg: &ResvgCompatibleSvg,
    options: &RasterOptions,
) -> Result<(Vec<u8>, RasterPlan)> {
    svg_to_png_with_plan_controlled(svg, options, OperationControl::new())
}

#[cfg(feature = "png")]
pub fn svg_to_png_with_report(
    svg: &ResvgCompatibleSvg,
    options: &RasterOptions,
) -> Result<(Vec<u8>, RasterExportReport)> {
    prepare_raster(svg, options)?.encode_png_with_report()
}

#[cfg(feature = "jpeg")]
pub fn svg_to_jpeg(svg: &ResvgCompatibleSvg, options: &RasterOptions) -> Result<Vec<u8>> {
    svg_to_jpeg_controlled(svg, options, OperationControl::new())
}

/// Encodes a sealed SVG as JPEG using caller-owned cooperative cancellation/deadline state.
#[cfg(feature = "jpeg")]
pub fn svg_to_jpeg_controlled(
    svg: &ResvgCompatibleSvg,
    options: &RasterOptions,
    control: OperationControl,
) -> Result<Vec<u8>> {
    prepare_raster_controlled(svg, options, control)?.encode_jpeg()
}

/// Encodes a sealed SVG as JPEG and returns the allocation plan used for the output pixmap.
#[cfg(feature = "jpeg")]
pub fn svg_to_jpeg_with_plan_controlled(
    svg: &ResvgCompatibleSvg,
    options: &RasterOptions,
    control: OperationControl,
) -> Result<(Vec<u8>, RasterPlan)> {
    let prepared = prepare_raster_controlled(svg, options, control)?;
    let plan = prepared.plan();
    let bytes = prepared.encode_jpeg()?;
    Ok((bytes, plan))
}

/// Encodes a sealed SVG as JPEG and returns its allocation plan using a fresh control.
#[cfg(feature = "jpeg")]
pub fn svg_to_jpeg_with_plan(
    svg: &ResvgCompatibleSvg,
    options: &RasterOptions,
) -> Result<(Vec<u8>, RasterPlan)> {
    svg_to_jpeg_with_plan_controlled(svg, options, OperationControl::new())
}

#[cfg(feature = "jpeg")]
pub fn svg_to_jpeg_with_report(
    svg: &ResvgCompatibleSvg,
    options: &RasterOptions,
) -> Result<(Vec<u8>, RasterExportReport)> {
    prepare_raster(svg, options)?.encode_jpeg_with_report()
}

#[cfg(any(feature = "png", feature = "jpeg"))]
pub fn svg_raster_plan(svg: &ResvgCompatibleSvg, options: &RasterOptions) -> Result<RasterPlan> {
    svg_raster_plan_controlled(svg, options, OperationControl::new())
}

/// Computes a sealed SVG raster plan using caller-owned cooperative cancellation/deadline state.
#[cfg(any(feature = "png", feature = "jpeg"))]
pub fn svg_raster_plan_controlled(
    svg: &ResvgCompatibleSvg,
    options: &RasterOptions,
    control: OperationControl,
) -> Result<RasterPlan> {
    Ok(prepare_raster_controlled(svg, options, control)?.plan())
}

#[cfg(feature = "pdf")]
pub fn svg_to_pdf(svg: &ResvgCompatibleSvg) -> Result<Vec<u8>> {
    svg_to_pdf_controlled(svg, &PdfOptions::default(), OperationControl::new())
}

#[cfg(feature = "pdf")]
pub fn svg_to_pdf_with_options(svg: &ResvgCompatibleSvg, options: &PdfOptions) -> Result<Vec<u8>> {
    svg_to_pdf_controlled(svg, options, OperationControl::new())
}

/// Encodes a sealed SVG as PDF using caller-owned cooperative cancellation/deadline state.
#[cfg(feature = "pdf")]
pub fn svg_to_pdf_controlled(
    svg: &ResvgCompatibleSvg,
    options: &PdfOptions,
    control: OperationControl,
) -> Result<Vec<u8>> {
    prepare_pdf_controlled(svg, options, control)?.encode()
}

/// Encodes a sealed SVG as PDF and returns the filter-image plan used by the encoder.
#[cfg(feature = "pdf")]
pub fn svg_to_pdf_with_plan_controlled(
    svg: &ResvgCompatibleSvg,
    options: &PdfOptions,
    control: OperationControl,
) -> Result<(Vec<u8>, PdfFilterImagePlan)> {
    let prepared = prepare_pdf_controlled(svg, options, control)?;
    let plan = prepared.filter_plan();
    let bytes = prepared.encode()?;
    Ok((bytes, plan))
}

/// Encodes a sealed SVG as PDF and returns its filter-image plan using a fresh control.
#[cfg(feature = "pdf")]
pub fn svg_to_pdf_with_plan(
    svg: &ResvgCompatibleSvg,
    options: &PdfOptions,
) -> Result<(Vec<u8>, PdfFilterImagePlan)> {
    svg_to_pdf_with_plan_controlled(svg, options, OperationControl::new())
}

#[cfg(feature = "pdf")]
pub fn svg_to_pdf_with_report(
    svg: &ResvgCompatibleSvg,
    options: &PdfOptions,
) -> Result<(Vec<u8>, PdfExportReport)> {
    prepare_pdf(svg, options)?.encode_with_report()
}

#[cfg(all(
    feature = "native-fonts",
    any(feature = "png", feature = "jpeg", feature = "pdf")
))]
type ExportFontLoader<'a> = Box<
    dyn FnOnce(
            bool,
            &OperationControl,
        ) -> std::result::Result<Option<NativeFontContext>, NativeFontError>
        + Send
        + 'a,
>;

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
enum SvgExportFonts<'a> {
    Retained(&'a ResvgCompatibleSvg),
    #[cfg(feature = "native-fonts")]
    Context(&'a NativeFontContext),
    #[cfg(feature = "native-fonts")]
    Deferred(ExportFontLoader<'a>),
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
struct ConfiguredExportFonts {
    plan: ExportFontPlanSeed,
    #[cfg(feature = "native-fonts")]
    context: Option<NativeFontContext>,
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
impl<'a> SvgExportFonts<'a> {
    fn from_options(
        svg: &'a ResvgCompatibleSvg,
        #[cfg(feature = "native-fonts")] context: Option<&'a NativeFontContext>,
    ) -> Self {
        #[cfg(feature = "native-fonts")]
        if let Some(context) = context {
            return Self::Context(context);
        }
        Self::Retained(svg)
    }

    fn configure(
        self,
        needs_fonts: bool,
        options: &mut usvg::Options<'_>,
        control: &OperationControl,
    ) -> Result<ConfiguredExportFonts> {
        export_checkpoint(control)?;
        let plan = match self {
            Self::Retained(svg) if needs_fonts => {
                ExportFontEnvironment::from_svg(svg)?.install(options)
            }
            #[cfg(feature = "native-fonts")]
            Self::Context(context) => {
                return ConfiguredExportFonts::from_context(context.clone(), options, control);
            }
            #[cfg(feature = "native-fonts")]
            Self::Deferred(load) => match load(needs_fonts, control)? {
                Some(context) => {
                    return ConfiguredExportFonts::from_context(context, options, control);
                }
                None if needs_fonts => return Err(ExportError::FontSourceUnavailable),
                None => ExportFontEnvironment::fontless_plan(options),
            },
            Self::Retained(_) => ExportFontEnvironment::fontless_plan(options),
        };
        Ok(ConfiguredExportFonts {
            plan,
            #[cfg(feature = "native-fonts")]
            context: None,
        })
    }
}

#[cfg(all(
    feature = "native-fonts",
    any(feature = "png", feature = "jpeg", feature = "pdf")
))]
impl ConfiguredExportFonts {
    fn from_context(
        context: NativeFontContext,
        options: &mut usvg::Options<'_>,
        control: &OperationControl,
    ) -> Result<Self> {
        context.check()?;
        let observer = context.apply_to_usvg_options_for_export(options, control);
        Ok(Self {
            plan: ExportFontPlanSeed::from_native_context(&context, observer),
            context: Some(context),
        })
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
struct ParsedExportSvg {
    tree: usvg::Tree,
    font_plan: ExportFontPlan,
    #[cfg(feature = "native-fonts")]
    font_context: Option<NativeFontContext>,
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn parse_svg_tree(
    source: &str,
    mut options: usvg::Options<'static>,
    fonts: SvgExportFonts<'_>,
    control: &OperationControl,
) -> Result<ParsedExportSvg> {
    export_checkpoint(control)?;
    #[cfg(feature = "native-fonts")]
    if let SvgExportFonts::Context(context) = &fonts {
        context.check()?;
    }
    let mut fonts = Some(fonts);
    let mut configured = None;
    let tree = usvg::Tree::try_from_str_with_text_fonts(
        source,
        &options,
        usvg::ImageHrefPolicy::DataOnly,
        |options| {
            configured = fonts
                .take()
                .map(|fonts| fonts.configure(true, options, control));
            configured.as_ref().is_some_and(Result::is_ok)
        },
    );
    #[cfg(feature = "native-fonts")]
    if let Some(Ok(configured)) = &configured
        && let Some(context) = &configured.context
    {
        context.check()?;
    }
    export_checkpoint(control)?;
    let configured = configured.transpose()?;
    let tree = tree
        .map_err(|_| ExportError::SvgParse)?
        .ok_or(ExportError::SvgParse)?;
    let configured = match configured {
        Some(configured) => configured,
        None => fonts
            .ok_or(ExportError::SvgParse)?
            .configure(false, &mut options, control)?,
    };
    let font_plan = configured.plan.finish_with_tree(&tree, control)?;
    #[cfg(feature = "native-fonts")]
    if let Some(context) = &configured.context {
        check_export_glyph_coverage(context, font_plan)?;
    }
    Ok(ParsedExportSvg {
        tree,
        font_plan,
        #[cfg(feature = "native-fonts")]
        font_context: configured.context,
    })
}

#[cfg(all(
    feature = "native-fonts",
    any(feature = "png", feature = "jpeg", feature = "pdf")
))]
fn check_export_glyph_coverage(context: &NativeFontContext, plan: ExportFontPlan) -> Result<()> {
    if context.strict_glyph_coverage()
        && (plan.unresolved_font_request() || plan.unresolved_glyph_fallback())
    {
        return Err(NativeFontError::MissingGlyphCoverage.into());
    }
    Ok(())
}

#[cfg(feature = "pdf")]
fn svg_tree_to_pdf(
    svg_tree: &usvg::Tree,
    layout: PdfPageLayout,
    filter_scale: f32,
    page_paint: Option<ExportRgbaColor>,
    control: &OperationControl,
) -> Result<Vec<u8>> {
    use krilla_svg::SurfaceExt;

    export_checkpoint(control)?;

    let mut document = krilla::Document::new();
    let mut page = document.start_page_with(krilla::page::PageSettings::new(layout.page_size));
    let mut surface = page.surface();
    draw_pdf_page_paint(&mut surface, layout.page_size, page_paint);
    if layout.offset.0 != 0.0 || layout.offset.1 != 0.0 {
        surface.push_transform(&krilla::geom::Transform::from_translate(
            layout.offset.0,
            layout.offset.1,
        ));
    }
    // krilla-svg performs one opaque synchronous draw. Cooperative cancellation is observed at
    // the call boundaries; hard interruption requires host-level worker/process isolation.
    export_checkpoint(control)?;
    surface.draw_svg(
        svg_tree,
        layout.drawing_size,
        krilla_svg::SvgSettings {
            filter_scale,
            ..krilla_svg::SvgSettings::default()
        },
    );
    export_checkpoint(control)?;
    if layout.offset.0 != 0.0 || layout.offset.1 != 0.0 {
        surface.pop();
    }
    surface.finish();
    page.finish();

    export_checkpoint(control)?;
    let pdf = document.finish().map_err(|_| ExportError::PdfConvert);
    export_checkpoint(control)?;
    pdf
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
impl PdfPageLayout {
    fn plan(self) -> PdfPagePlan {
        PdfPagePlan {
            page_width_pt: self.page_size.width(),
            page_height_pt: self.page_size.height(),
            drawing_width_pt: self.drawing_size.width(),
            drawing_height_pt: self.drawing_size.height(),
            offset_x_pt: self.offset.0,
            offset_y_pt: self.offset.1,
        }
    }
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

#[cfg(feature = "pdf")]
fn draw_pdf_page_paint(
    surface: &mut krilla::surface::Surface<'_>,
    page_size: krilla::geom::Size,
    page_paint: Option<ExportRgbaColor>,
) {
    let Some(color) = page_paint.filter(|color| color.alpha() != 0) else {
        return;
    };
    let Some(opacity) = krilla::num::NormalizedF32::new(f32::from(color.alpha()) / 255.0) else {
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
        paint: krilla::color::rgb::Color::new(color.red(), color.green(), color.blue()).into(),
        opacity,
        rule: Default::default(),
    }));
    surface.draw_path(&path);
    surface.set_fill(None);
}

#[cfg(any(
    feature = "native-fonts",
    all(test, any(feature = "png", feature = "jpeg", feature = "pdf"))
))]
fn browser_like_font_resolver(
    selection: std::sync::Arc<font_selection::FontSelectionPolicy>,
) -> usvg::FontResolver<'static> {
    let fallback_selection = std::sync::Arc::clone(&selection);
    usvg::FontResolver {
        select_font: Box::new(move |font, fontdb| {
            selection
                .requested(font, fontdb)
                .or_else(|| selection.fallback(font, fontdb))
        }),
        select_fallback: Box::new(move |character, excluded, database| {
            fallback_selection.glyph_fallback(character, excluded, database)
        }),
    }
}

/// Returns whether the native exporters can interpret a background color.
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[must_use]
pub fn is_valid_export_color(text: &str) -> bool {
    parse_export_color(text).is_ok()
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn parse_export_color(text: &str) -> std::result::Result<ExportRgbaColor, ()> {
    let [red, green, blue, alpha] = merman_markup::theme_color::ThemeColor::parse(text.trim())
        .map_err(|_| ())?
        .rgba_channels()
        .to_rgba8();
    Ok(ExportRgbaColor {
        red,
        green,
        blue,
        alpha,
    })
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn tiny_skia_color(color: ExportRgbaColor) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(color.red(), color.green(), color.blue(), color.alpha())
}

#[cfg(all(
    test,
    feature = "system-fonts",
    not(target_family = "wasm"),
    any(feature = "png", feature = "jpeg")
))]
#[path = "tests/font_resolver.rs"]
mod font_resolver_tests;

#[cfg(all(test, any(feature = "png", feature = "jpeg", feature = "pdf")))]
#[path = "tests/fixtures.rs"]
mod test_fixtures;

#[cfg(all(test, any(feature = "png", feature = "jpeg", feature = "pdf")))]
#[path = "tests/svg_font_demand.rs"]
mod svg_font_demand_tests;

#[cfg(all(
    test,
    not(all(feature = "system-fonts", not(target_family = "wasm"))),
    any(feature = "png", feature = "jpeg", feature = "pdf")
))]
#[path = "tests/unavailable_source.rs"]
mod unavailable_source_tests;

#[cfg(all(test, feature = "png"))]
#[cfg(any(
    feature = "native-fonts",
    all(feature = "system-fonts", not(target_family = "wasm"))
))]
#[path = "tests/png_feature.rs"]
mod png_feature_tests;

#[cfg(all(test, feature = "jpeg"))]
#[cfg(any(
    feature = "native-fonts",
    all(feature = "system-fonts", not(target_family = "wasm"))
))]
#[path = "tests/jpeg_feature.rs"]
mod jpeg_feature_tests;

#[cfg(all(test, feature = "pdf"))]
#[cfg(any(
    feature = "native-fonts",
    all(feature = "system-fonts", not(target_family = "wasm"))
))]
#[path = "tests/pdf_feature.rs"]
mod pdf_feature_tests;

#[cfg(all(test, feature = "png", feature = "jpeg", feature = "pdf"))]
#[cfg(any(
    feature = "native-fonts",
    all(feature = "system-fonts", not(target_family = "wasm"))
))]
mod tests;

#[cfg(all(test, any(feature = "png", feature = "jpeg", feature = "pdf")))]
#[path = "tests/id_suffix_consumer.rs"]
mod id_suffix_consumer_tests;
