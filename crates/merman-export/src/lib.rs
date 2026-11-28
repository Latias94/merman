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
#[cfg(feature = "pdf")]
mod pdf;
#[cfg(any(feature = "png", feature = "jpeg"))]
mod raster;
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
mod viewport;

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use merman_model::OperationControl;
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use merman_render::svg::ResvgCompatibleSvg;
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use operation::export_checkpoint;
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub use operation::{ExportError, ExportResourceLimitCause, ExportResourceLimitDetails, Result};
#[cfg(all(feature = "native-fonts", feature = "pdf"))]
pub use pdf::prepare_pdf_with_font_loader_controlled;
#[cfg(feature = "pdf")]
pub use pdf::{
    PdfExportReport, PdfFilterImageLimit, PdfFilterImagePlan, PdfOptions, PdfPagePlan,
    PdfPagePolicy, PreparedPdf, prepare_pdf, prepare_pdf_controlled, svg_to_pdf,
    svg_to_pdf_controlled, svg_to_pdf_with_options, svg_to_pdf_with_plan,
    svg_to_pdf_with_plan_controlled, svg_to_pdf_with_report,
};
#[cfg(all(feature = "native-fonts", any(feature = "png", feature = "jpeg")))]
pub use raster::prepare_raster_with_font_loader_controlled;
#[cfg(any(feature = "png", feature = "jpeg"))]
pub use raster::{
    PreparedRaster, RasterExportReport, RasterOptions, RasterOutputKind, RasterPlan,
    prepare_raster, prepare_raster_controlled, svg_raster_plan, svg_raster_plan_controlled,
};
#[cfg(feature = "jpeg")]
pub use raster::{
    svg_to_jpeg, svg_to_jpeg_controlled, svg_to_jpeg_with_plan, svg_to_jpeg_with_plan_controlled,
    svg_to_jpeg_with_report,
};
#[cfg(feature = "png")]
pub use raster::{
    svg_to_png, svg_to_png_controlled, svg_to_png_with_plan, svg_to_png_with_plan_controlled,
    svg_to_png_with_report,
};
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
#[cfg(any(feature = "png", feature = "jpeg"))]
pub use viewport::{RasterFitBox, RasterSizeLimit};

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

#[cfg(any(
    feature = "native-fonts",
    all(
        test,
        feature = "system-fonts",
        not(target_family = "wasm"),
        any(feature = "png", feature = "jpeg")
    )
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

#[cfg(all(
    test,
    feature = "system-fonts",
    not(target_family = "wasm"),
    any(feature = "png", feature = "jpeg")
))]
#[path = "tests/font_resolver.rs"]
mod font_resolver_tests;

#[cfg(all(test, any(feature = "png", feature = "jpeg", feature = "pdf")))]
#[cfg(any(
    feature = "native-fonts",
    all(feature = "system-fonts", not(target_family = "wasm"))
))]
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
