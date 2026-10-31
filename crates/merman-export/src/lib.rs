#![forbid(unsafe_code)]

//! Bounded native font services and binary export for validated SVG.
//!
//! This crate deliberately accepts [`ResvgCompatibleSvg`] rather than Mermaid source or an
//! arbitrary SVG string. Parsing, semantic construction, layout, SVG production, and terminal
//! SVG validation stay owned by `merman`; this crate owns explicit native font contexts and
//! allocation-aware encoding.

#[cfg(any(
    feature = "native-fonts",
    feature = "png",
    feature = "jpeg",
    feature = "pdf"
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
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
mod native_filter_receipt;

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub use font_environment::{ExportFontMode, ExportFontPlan};

#[cfg(any(feature = "png", feature = "jpeg"))]
use cssparser::{Delimiter, Parser, Token};
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use merman_model::{
    OperationCancelled, OperationControl, OperationLedgerError, OperationPhase,
    OperationResourceDomain, OperationResourceLimitExceeded, OperationResourceProvenance,
};
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use merman_render::svg::ResvgCompatibleSvg;
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[cfg(feature = "native-fonts")]
    #[error(transparent)]
    NativeFont(#[from] NativeFontError),
    #[error(transparent)]
    Cancelled(#[from] OperationCancelled),
    #[error("failed to parse SVG")]
    SvgParse,
    #[error("failed to set SVG Document size from tree")]
    SvgDocSize,
    #[error("failed to allocate pixmap for raster rendering")]
    PixmapAlloc,
    #[error("invalid raster scale; expected a finite positive number")]
    InvalidScale,
    #[error("invalid SVG export sizing option: {0}")]
    InvalidSizing(&'static str),
    #[error("failed to encode PNG")]
    PngEncode,
    #[error("invalid raster matte color")]
    InvalidRasterMatte,
    #[error("JPG rendering requires an opaque matte color (e.g. white)")]
    JpegOpaqueMatteRequired,
    #[error("failed to encode JPG")]
    JpegEncode,
    #[error("JPG dimensions exceed the 65535-pixel encoder limit")]
    JpegDimensionLimit,
    #[error("failed to convert SVG to PDF")]
    PdfConvert,
    #[error("invalid PDF page paint color")]
    InvalidPdfPagePaint,
    #[error("retained font catalog and host font-source policy have no usable source")]
    FontSourceUnavailable,
    #[error("failed to load the retained font catalog into the native exporter")]
    FontCatalogLoad,
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
    #[cfg(feature = "pdf")]
    #[error(
        "PDF filter image resource limit exceeded: requested pixels are {actual}, maximum is {max}"
    )]
    PdfFilterImageLimit { actual: u64, max: u64 },
    #[error(
        "operation resource limit `{limit_id}` exceeded during {phase}: actual={actual} maximum={max}"
    )]
    ResourceLimitTerminal {
        limit_id: &'static str,
        phase: &'static str,
        actual: u64,
        max: u64,
    },
    #[error(
        "operation resource `{limit_id}` arithmetic overflow during {phase}: actual={actual} maximum={max}"
    )]
    ResourceArithmeticOverflow {
        limit_id: &'static str,
        phase: &'static str,
        actual: u64,
        max: u64,
    },
    #[doc(hidden)]
    #[error(transparent)]
    OperationResourceTerminal(OperationLedgerError),
    #[error("failed to start the recursive SVG backend worker")]
    BackendWorkerSpawn,
    #[error("the recursive SVG backend worker panicked")]
    BackendWorkerPanic,
}

/// Stable resource metadata for an export failure.
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ExportResourceLimitDetails {
    pub limit_id: &'static str,
    pub phase: &'static str,
    pub actual: u64,
    pub max: u64,
    pub cause: ExportResourceLimitCause,
}

/// Stable reason for an export resource rejection.
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExportResourceLimitCause {
    Ceiling,
    ArithmeticOverflow,
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
impl ExportError {
    /// Returns transport-neutral resource metadata without exposing exporter-internal field names.
    #[must_use]
    pub fn resource_limit_details(&self) -> Option<ExportResourceLimitDetails> {
        let (limit_id, phase, actual, max, cause) = match self {
            Self::EmbeddedImageLimit {
                limit_name,
                actual,
                max,
            } => {
                let limit_id = match *limit_name {
                    "max_bytes_per_image" => MAX_EMBEDDED_IMAGE_BYTES_RESOURCE_LIMIT_ID,
                    "max_total_bytes" => MAX_TOTAL_EMBEDDED_IMAGE_BYTES_RESOURCE_LIMIT_ID,
                    "max_pixels_per_image" => MAX_EMBEDDED_IMAGE_PIXELS_RESOURCE_LIMIT_ID,
                    "max_total_pixels" => MAX_TOTAL_EMBEDDED_IMAGE_PIXELS_RESOURCE_LIMIT_ID,
                    _ => return None,
                };
                (
                    limit_id,
                    "embedded_image_decode",
                    *actual,
                    *max,
                    ExportResourceLimitCause::Ceiling,
                )
            }
            Self::SvgConversionLimit {
                limit_name,
                actual,
                max,
            } => {
                let (limit_id, phase) = match *limit_name {
                    "max_isolation_depth" => (
                        MAX_SVG_CONVERSION_ISOLATION_DEPTH_RESOURCE_LIMIT_ID,
                        "svg_conversion",
                    ),
                    "max_filter_primitives_per_filter" => (
                        MAX_SVG_CONVERSION_FILTER_PRIMITIVES_PER_FILTER_RESOURCE_LIMIT_ID,
                        "svg_conversion",
                    ),
                    "max_total_filter_primitives" => (
                        MAX_TOTAL_SVG_CONVERSION_FILTER_PRIMITIVES_RESOURCE_LIMIT_ID,
                        "svg_conversion",
                    ),
                    "max_subroots" => (
                        MAX_SVG_CONVERSION_SUBROOTS_RESOURCE_LIMIT_ID,
                        "svg_conversion",
                    ),
                    "max_nested_svg_images" => {
                        (MAX_NESTED_SVG_IMAGES_RESOURCE_LIMIT_ID, "svg_conversion")
                    }
                    limit_id @ merman_render::resources::SVG_BACKEND_TREE_NODES_HARD_CAP_ID => (
                        limit_id,
                        merman_render::resources::ResourceLimitPhase::SvgPostprocess.as_str(),
                    ),
                    limit_id @ merman_render::resources::SVG_BACKEND_TREE_DEPTH_HARD_CAP_ID => (
                        limit_id,
                        merman_render::resources::ResourceLimitPhase::SvgPostprocess.as_str(),
                    ),
                    _ => return None,
                };
                (
                    limit_id,
                    phase,
                    *actual,
                    *max,
                    ExportResourceLimitCause::Ceiling,
                )
            }
            #[cfg(feature = "pdf")]
            Self::PdfFilterImageLimit { actual, max } => (
                MAX_PDF_FILTER_IMAGE_PIXELS_RESOURCE_LIMIT_ID,
                "pdf_filter_rasterization",
                *actual,
                *max,
                ExportResourceLimitCause::Ceiling,
            ),
            Self::ResourceLimitTerminal {
                limit_id,
                phase,
                actual,
                max,
            } => (
                *limit_id,
                *phase,
                *actual,
                *max,
                ExportResourceLimitCause::Ceiling,
            ),
            Self::ResourceArithmeticOverflow {
                limit_id,
                phase,
                actual,
                max,
            } => (
                *limit_id,
                *phase,
                *actual,
                *max,
                ExportResourceLimitCause::ArithmeticOverflow,
            ),
            Self::OperationResourceTerminal(error) => {
                return operation_resource_limit_details(error);
            }
            _ => return None,
        };
        Some(ExportResourceLimitDetails {
            limit_id,
            phase,
            actual,
            max,
            cause,
        })
    }

    /// Returns the resource owner recorded by the originating adapter.
    #[must_use]
    pub fn resource_limit_provenance(&self) -> Option<OperationResourceProvenance> {
        match self {
            Self::OperationResourceTerminal(OperationLedgerError::Cancelled(_)) => None,
            Self::OperationResourceTerminal(OperationLedgerError::ResourceLimitExceeded(error)) => {
                Some(error.provenance.clone())
            }
            Self::OperationResourceTerminal(OperationLedgerError::ArithmeticOverflow {
                provenance,
                ..
            }) => Some(provenance.clone()),
            _ => self
                .resource_limit_details()
                .map(|details| export_operation_resource_provenance(details.limit_id)),
        }
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn operation_resource_limit_details(
    error: &OperationLedgerError,
) -> Option<ExportResourceLimitDetails> {
    match error {
        OperationLedgerError::Cancelled(_) => None,
        OperationLedgerError::ResourceLimitExceeded(error) => Some(ExportResourceLimitDetails {
            limit_id: error.id,
            phase: error.resource_phase,
            actual: error.consumed.saturating_add(error.requested),
            max: error.limit,
            cause: ExportResourceLimitCause::Ceiling,
        }),
        OperationLedgerError::ArithmeticOverflow {
            id,
            resource_phase,
            actual,
            maximum,
            ..
        } => Some(ExportResourceLimitDetails {
            limit_id: id,
            phase: resource_phase,
            actual: *actual,
            max: *maximum,
            cause: ExportResourceLimitCause::ArithmeticOverflow,
        }),
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub type Result<T> = std::result::Result<T, ExportError>;

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn export_checkpoint(control: &OperationControl) -> Result<()> {
    control
        .terminal_checkpoint_at(OperationPhase::Export)
        .map_err(export_terminal_error)
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn terminate_export_resource_error(control: &OperationControl, error: ExportError) -> ExportError {
    let Some(details) = error.resource_limit_details() else {
        return error;
    };
    let Some(provenance) = error.resource_limit_provenance() else {
        return error;
    };
    if let Err(terminal) = control.terminal_checkpoint_at(OperationPhase::Export) {
        return export_terminal_error(terminal);
    }
    let (terminal, expected) = match details.cause {
        ExportResourceLimitCause::Ceiling => {
            let error = OperationResourceLimitExceeded {
                id: details.limit_id,
                phase: OperationPhase::Export,
                resource_phase: details.phase,
                limit: details.max,
                consumed: 0,
                requested: details.actual,
                provenance: provenance.clone(),
            };
            (
                control.terminate_resource_limit(error.clone()),
                OperationLedgerError::ResourceLimitExceeded(error),
            )
        }
        ExportResourceLimitCause::ArithmeticOverflow => {
            let expected = OperationLedgerError::ArithmeticOverflow {
                id: details.limit_id,
                phase: OperationPhase::Export,
                resource_phase: details.phase,
                actual: details.actual,
                maximum: details.max,
                provenance: provenance.clone(),
            };
            (
                control.terminate_resource_overflow(
                    details.limit_id,
                    OperationPhase::Export,
                    details.phase,
                    details.actual,
                    details.max,
                    provenance,
                ),
                expected,
            )
        }
    };
    if terminal == expected {
        error
    } else {
        export_terminal_error(terminal)
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn export_operation_resource_provenance(limit_id: &str) -> OperationResourceProvenance {
    let domain = match limit_id {
        merman_render::resources::SVG_BACKEND_TREE_NODES_HARD_CAP_ID
        | merman_render::resources::SVG_BACKEND_TREE_DEPTH_HARD_CAP_ID => {
            OperationResourceDomain::Render
        }
        _ => OperationResourceDomain::Export,
    };
    OperationResourceProvenance::new(domain, None, [])
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn settle_export_result<T>(control: &OperationControl, result: Result<T>) -> Result<T> {
    let result = result.map_err(|error| terminate_export_resource_error(control, error));
    observe_after_export_result(control, result)
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn observe_after_export_result<T>(control: &OperationControl, result: Result<T>) -> Result<T> {
    match result {
        Err(error) if error.resource_limit_details().is_some() => Err(error),
        result => {
            export_checkpoint(control)?;
            result
        }
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn export_terminal_error(error: OperationLedgerError) -> ExportError {
    match error {
        OperationLedgerError::Cancelled(error) => ExportError::Cancelled(error),
        terminal @ (OperationLedgerError::ResourceLimitExceeded(_)
        | OperationLedgerError::ArithmeticOverflow { .. }) => {
            ExportError::OperationResourceTerminal(terminal)
        }
    }
}

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
// Two sRGB shadows lower to at most ten standard SVG primitives. Keep this finite backend cap
// aligned with that bounded composition; the separate total-primitive ceiling still applies.
pub const DEFAULT_MAX_FILTER_PRIMITIVES_PER_FILTER: usize = 10;
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub const DEFAULT_MAX_TOTAL_FILTER_PRIMITIVES: usize = 128;
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub const DEFAULT_MAX_SVG_SUBROOTS: usize = 4096;
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub const DEFAULT_MAX_NESTED_SVG_IMAGES: usize = 64;

/// Stable binding metadata for native export limits owned by this crate.
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ExportResourceLimitDescriptor {
    pub stable_id: &'static str,
    pub phase: &'static str,
    pub description: &'static str,
    pub overridable: bool,
    pub hard_cap: bool,
    pub minimum_value: usize,
}

#[cfg(any(feature = "png", feature = "jpeg"))]
pub const MAX_RASTER_WIDTH_RESOURCE_LIMIT_ID: &str = "max_raster_width";
#[cfg(any(feature = "png", feature = "jpeg"))]
pub const MAX_RASTER_HEIGHT_RESOURCE_LIMIT_ID: &str = "max_raster_height";
#[cfg(any(feature = "png", feature = "jpeg"))]
pub const MAX_RASTER_PIXELS_RESOURCE_LIMIT_ID: &str = "max_raster_pixels";
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub const MAX_EMBEDDED_IMAGE_BYTES_RESOURCE_LIMIT_ID: &str = "max_embedded_image_bytes";
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub const MAX_TOTAL_EMBEDDED_IMAGE_BYTES_RESOURCE_LIMIT_ID: &str = "max_total_embedded_image_bytes";
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub const MAX_EMBEDDED_IMAGE_PIXELS_RESOURCE_LIMIT_ID: &str = "max_embedded_image_pixels";
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub const MAX_TOTAL_EMBEDDED_IMAGE_PIXELS_RESOURCE_LIMIT_ID: &str =
    "max_total_embedded_image_pixels";
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub const MAX_SVG_CONVERSION_ISOLATION_DEPTH_RESOURCE_LIMIT_ID: &str =
    "max_svg_conversion_isolation_depth";
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub const MAX_SVG_CONVERSION_FILTER_PRIMITIVES_PER_FILTER_RESOURCE_LIMIT_ID: &str =
    "max_svg_conversion_filter_primitives_per_filter";
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub const MAX_TOTAL_SVG_CONVERSION_FILTER_PRIMITIVES_RESOURCE_LIMIT_ID: &str =
    "max_total_svg_conversion_filter_primitives";
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub const MAX_SVG_CONVERSION_SUBROOTS_RESOURCE_LIMIT_ID: &str = "max_svg_conversion_subroots";
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub const MAX_NESTED_SVG_IMAGES_RESOURCE_LIMIT_ID: &str = "max_nested_svg_images";
#[cfg(feature = "pdf")]
pub const MAX_PDF_FILTER_IMAGE_PIXELS_RESOURCE_LIMIT_ID: &str = "max_pdf_filter_image_pixels";
#[cfg(any(feature = "png", feature = "jpeg"))]
const RASTER_RESOURCE_LIMIT_DESCRIPTORS: [ExportResourceLimitDescriptor; 3] = [
    ExportResourceLimitDescriptor {
        stable_id: MAX_RASTER_WIDTH_RESOURCE_LIMIT_ID,
        phase: "raster_allocation",
        description: "Maximum final PNG or JPEG width in pixels",
        overridable: true,
        hard_cap: false,
        minimum_value: 1,
    },
    ExportResourceLimitDescriptor {
        stable_id: MAX_RASTER_HEIGHT_RESOURCE_LIMIT_ID,
        phase: "raster_allocation",
        description: "Maximum final PNG or JPEG height in pixels",
        overridable: true,
        hard_cap: false,
        minimum_value: 1,
    },
    ExportResourceLimitDescriptor {
        stable_id: MAX_RASTER_PIXELS_RESOURCE_LIMIT_ID,
        phase: "raster_allocation",
        description: "Maximum final PNG or JPEG pixel count",
        overridable: true,
        hard_cap: false,
        minimum_value: 1,
    },
];

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
const EMBEDDED_IMAGE_RESOURCE_LIMIT_DESCRIPTORS: [ExportResourceLimitDescriptor; 4] = [
    ExportResourceLimitDescriptor {
        stable_id: MAX_EMBEDDED_IMAGE_BYTES_RESOURCE_LIMIT_ID,
        phase: "embedded_image_decode",
        description: "Maximum decoded data-URL bytes for one embedded image",
        overridable: true,
        hard_cap: false,
        minimum_value: 1,
    },
    ExportResourceLimitDescriptor {
        stable_id: MAX_TOTAL_EMBEDDED_IMAGE_BYTES_RESOURCE_LIMIT_ID,
        phase: "embedded_image_decode",
        description: "Maximum aggregate decoded data-URL bytes across embedded images",
        overridable: true,
        hard_cap: false,
        minimum_value: 1,
    },
    ExportResourceLimitDescriptor {
        stable_id: MAX_EMBEDDED_IMAGE_PIXELS_RESOURCE_LIMIT_ID,
        phase: "embedded_image_decode",
        description: "Maximum intrinsic pixels for one embedded raster image",
        overridable: true,
        hard_cap: false,
        minimum_value: 1,
    },
    ExportResourceLimitDescriptor {
        stable_id: MAX_TOTAL_EMBEDDED_IMAGE_PIXELS_RESOURCE_LIMIT_ID,
        phase: "embedded_image_decode",
        description: "Maximum aggregate intrinsic pixels across embedded raster images",
        overridable: true,
        hard_cap: false,
        minimum_value: 1,
    },
];

#[cfg(feature = "pdf")]
const PDF_RESOURCE_LIMIT_DESCRIPTORS: [ExportResourceLimitDescriptor; 1] =
    [ExportResourceLimitDescriptor {
        stable_id: MAX_PDF_FILTER_IMAGE_PIXELS_RESOURCE_LIMIT_ID,
        phase: "pdf_filter_rasterization",
        description: "Maximum aggregate pixels retained for localized PDF filter images",
        overridable: true,
        hard_cap: false,
        minimum_value: 1,
    }];

// These are backend recursion guards, not caller policy knobs. They remain active for the
// trusted-input profile and are exposed only so resource failures have discoverable stable IDs.
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
const SVG_CONVERSION_HARD_CAP_DESCRIPTORS: [ExportResourceLimitDescriptor; 5] = [
    ExportResourceLimitDescriptor {
        stable_id: MAX_SVG_CONVERSION_ISOLATION_DEPTH_RESOURCE_LIMIT_ID,
        phase: "svg_conversion",
        description: "Maximum nested SVG isolation depth accepted by native export",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
    ExportResourceLimitDescriptor {
        stable_id: MAX_SVG_CONVERSION_FILTER_PRIMITIVES_PER_FILTER_RESOURCE_LIMIT_ID,
        phase: "svg_conversion",
        description: "Maximum primitives accepted in one SVG filter",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
    ExportResourceLimitDescriptor {
        stable_id: MAX_TOTAL_SVG_CONVERSION_FILTER_PRIMITIVES_RESOURCE_LIMIT_ID,
        phase: "svg_conversion",
        description: "Maximum aggregate SVG filter primitives accepted by native export",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
    ExportResourceLimitDescriptor {
        stable_id: MAX_SVG_CONVERSION_SUBROOTS_RESOURCE_LIMIT_ID,
        phase: "svg_conversion",
        description: "Maximum resolved SVG subroots accepted by native export",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
    ExportResourceLimitDescriptor {
        stable_id: MAX_NESTED_SVG_IMAGES_RESOURCE_LIMIT_ID,
        phase: "svg_conversion",
        description: "Maximum nested SVG images accepted by native export",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
];

/// Returns the export limits compiled into the concrete feature closure.
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[must_use]
pub fn export_resource_limit_descriptors() -> Vec<ExportResourceLimitDescriptor> {
    let mut descriptors = Vec::new();
    #[cfg(any(feature = "png", feature = "jpeg"))]
    descriptors.extend_from_slice(&RASTER_RESOURCE_LIMIT_DESCRIPTORS);
    descriptors.extend_from_slice(&EMBEDDED_IMAGE_RESOURCE_LIMIT_DESCRIPTORS);
    #[cfg(feature = "pdf")]
    descriptors.extend_from_slice(&PDF_RESOURCE_LIMIT_DESCRIPTORS);
    descriptors.extend_from_slice(&SVG_CONVERSION_HARD_CAP_DESCRIPTORS);
    descriptors
}

/// Returns the outputs that can enforce one export-owned resource limit.
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[must_use]
pub fn export_resource_limit_output_ids(stable_id: &str) -> Option<&'static [&'static str]> {
    #[cfg(any(feature = "png", feature = "jpeg"))]
    if RASTER_RESOURCE_LIMIT_DESCRIPTORS
        .iter()
        .any(|descriptor| descriptor.stable_id == stable_id)
    {
        return Some(&["png", "jpeg"]);
    }
    if EMBEDDED_IMAGE_RESOURCE_LIMIT_DESCRIPTORS
        .iter()
        .any(|descriptor| descriptor.stable_id == stable_id)
    {
        return Some(&["png", "jpeg", "pdf"]);
    }
    #[cfg(feature = "pdf")]
    if PDF_RESOURCE_LIMIT_DESCRIPTORS
        .iter()
        .any(|descriptor| descriptor.stable_id == stable_id)
    {
        return Some(&["pdf"]);
    }
    if SVG_CONVERSION_HARD_CAP_DESCRIPTORS
        .iter()
        .any(|descriptor| descriptor.stable_id == stable_id)
    {
        return Some(&["png", "jpeg", "pdf"]);
    }
    None
}

/// Returns the profile value for an export-owned resource limit.
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[must_use]
pub fn export_resource_profile_value(
    profile: merman_render::resources::RenderResourceProfile,
    stable_id: &str,
) -> Option<Option<usize>> {
    let finite_value = match stable_id {
        #[cfg(any(feature = "png", feature = "jpeg"))]
        MAX_RASTER_WIDTH_RESOURCE_LIMIT_ID | MAX_RASTER_HEIGHT_RESOURCE_LIMIT_ID => {
            DEFAULT_MAX_RASTER_SIDE_LENGTH as usize
        }
        #[cfg(any(feature = "png", feature = "jpeg"))]
        MAX_RASTER_PIXELS_RESOURCE_LIMIT_ID => DEFAULT_MAX_RASTER_PIXELS as usize,
        MAX_EMBEDDED_IMAGE_BYTES_RESOURCE_LIMIT_ID => DEFAULT_MAX_EMBEDDED_IMAGE_BYTES as usize,
        MAX_TOTAL_EMBEDDED_IMAGE_BYTES_RESOURCE_LIMIT_ID => {
            DEFAULT_MAX_TOTAL_EMBEDDED_IMAGE_BYTES as usize
        }
        MAX_EMBEDDED_IMAGE_PIXELS_RESOURCE_LIMIT_ID => DEFAULT_MAX_DECODED_IMAGE_PIXELS as usize,
        MAX_TOTAL_EMBEDDED_IMAGE_PIXELS_RESOURCE_LIMIT_ID => {
            DEFAULT_MAX_TOTAL_DECODED_IMAGE_PIXELS as usize
        }
        #[cfg(feature = "pdf")]
        MAX_PDF_FILTER_IMAGE_PIXELS_RESOURCE_LIMIT_ID => {
            DEFAULT_MAX_PDF_FILTER_IMAGE_PIXELS as usize
        }
        MAX_SVG_CONVERSION_ISOLATION_DEPTH_RESOURCE_LIMIT_ID => DEFAULT_MAX_SVG_ISOLATION_DEPTH,
        MAX_SVG_CONVERSION_FILTER_PRIMITIVES_PER_FILTER_RESOURCE_LIMIT_ID => {
            DEFAULT_MAX_FILTER_PRIMITIVES_PER_FILTER
        }
        MAX_TOTAL_SVG_CONVERSION_FILTER_PRIMITIVES_RESOURCE_LIMIT_ID => {
            DEFAULT_MAX_TOTAL_FILTER_PRIMITIVES
        }
        MAX_SVG_CONVERSION_SUBROOTS_RESOURCE_LIMIT_ID => DEFAULT_MAX_SVG_SUBROOTS,
        MAX_NESTED_SVG_IMAGES_RESOURCE_LIMIT_ID => DEFAULT_MAX_NESTED_SVG_IMAGES,
        _ => return None,
    };
    if SVG_CONVERSION_HARD_CAP_DESCRIPTORS
        .iter()
        .any(|descriptor| descriptor.stable_id == stable_id)
    {
        return Some(Some(finite_value));
    }
    Some(match profile {
        merman_render::resources::RenderResourceProfile::UnboundedForTrustedInput => None,
        merman_render::resources::RenderResourceProfile::Interactive
        | merman_render::resources::RenderResourceProfile::Constrained
        | merman_render::resources::RenderResourceProfile::TrustedNative => Some(finite_value),
    })
}

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
fn run_recursive_svg_backend<T, F>(control: &OperationControl, job: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce(&OperationControl) -> Result<T> + Send + 'static,
{
    export_checkpoint(control)?;
    let worker_control = control.clone();
    let worker = std::thread::Builder::new()
        .name("merman-svg-backend".to_string())
        .stack_size(RECURSIVE_SVG_BACKEND_STACK_BYTES)
        .spawn(move || {
            export_checkpoint(&worker_control)?;
            settle_export_result(&worker_control, job(&worker_control))
        });
    let worker = match worker {
        Ok(worker) => worker,
        Err(_) => {
            export_checkpoint(control)?;
            return Err(ExportError::BackendWorkerSpawn);
        }
    };
    let result = worker
        .join()
        .unwrap_or(Err(ExportError::BackendWorkerPanic));
    observe_after_export_result(control, result)
}

#[cfg(all(
    any(feature = "png", feature = "jpeg", feature = "pdf"),
    target_arch = "wasm32"
))]
fn run_recursive_svg_backend<T, F>(control: &OperationControl, job: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce(&OperationControl) -> Result<T> + Send + 'static,
{
    export_checkpoint(control)?;
    settle_export_result(control, job(control))
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
    native_filter_receipt: Option<merman_render::__private::NativeSvgFilterReceipt>,
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
    #[doc(hidden)]
    pub const fn native_filter_receipt(
        self,
    ) -> Option<merman_render::__private::NativeSvgFilterReceipt> {
        self.native_filter_receipt
    }

    /// Returns whether directional filter offsets may differ from their SVG transform.
    ///
    /// The native raster backend renders these effects with scale magnitudes and can lose
    /// rotation, shear or reflection. Best-effort export still produces the diagram; callers
    /// should disclose this paint-quality limitation or reject it under an exactness policy.
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
    native_filter_receipt: Option<merman_render::__private::NativeSvgFilterReceipt>,
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
    native_filter_receipt: Option<merman_render::__private::NativeSvgFilterReceipt>,
    native_filter_fully_localized: bool,
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
    #[doc(hidden)]
    pub const fn native_filter_receipt(
        self,
    ) -> Option<merman_render::__private::NativeSvgFilterReceipt> {
        self.native_filter_receipt
    }

    #[doc(hidden)]
    pub const fn native_filter_fully_localized(self) -> bool {
        self.native_filter_fully_localized
    }

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
            Some(ExportRgbaColor::WHITE)
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
            native_filter_receipt: self.report.native_filter_receipt,
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
            let matte = self.matte.unwrap_or(ExportRgbaColor::WHITE);
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
            for (src, dst) in rgba.chunks_exact(4).zip(rgb.chunks_exact_mut(3)) {
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
    export_checkpoint(&control)?;
    let svg = svg.clone();
    let options = options.clone();
    let backend_control = control.clone();
    run_recursive_svg_backend(&control, move |control| {
        prepare_raster_on_backend_stack(&svg, &options, control).map(|mut prepared| {
            prepared.control = backend_control;
            prepared
        })
    })
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn prepare_raster_on_backend_stack(
    svg: &ResvgCompatibleSvg,
    options: &RasterOptions,
    control: &OperationControl,
) -> Result<PreparedRaster> {
    prepare_raster_source_on_backend_stack(svg, svg.as_str(), options, control)
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn prepare_raster_source_on_backend_stack(
    svg: &ResvgCompatibleSvg,
    source: &str,
    options: &RasterOptions,
    control: &OperationControl,
) -> Result<PreparedRaster> {
    export_checkpoint(control)?;
    let matte = options
        .matte
        .as_deref()
        .map(parse_export_color)
        .transpose()
        .map_err(|_| ExportError::InvalidRasterMatte)?;
    let root_metadata = parse_root_svg_metadata(source, control)?;
    let mut usvg_options = usvg::Options::default();
    let font_plan =
        configure_usvg_options_for_raster(&mut usvg_options, root_metadata, svg, options, control)?;
    let data_plan = plan_embedded_data_resources_with_occurrences(
        source,
        svg.reference_plan().raw_element_occurrences(),
        options.embedded_image_limit,
        control,
    )?;
    export_checkpoint(control)?;
    let tree = usvg::Tree::from_str(source, &usvg_options).map_err(|_| ExportError::SvgParse);
    export_checkpoint(control)?;
    #[cfg(feature = "native-fonts")]
    if let Some(context) = &options.font_context {
        context.check()?;
    }
    let tree = tree?;
    let (geometry, translate_min_to_origin) = raster_geometry_for_svg(root_metadata, &tree);
    let native_filter_receipt = usvg::Rect::from_xywh(
        geometry.min_x,
        geometry.min_y,
        geometry.width,
        geometry.height,
    )
    .and_then(|viewport| {
        native_filter_receipt::preflight_native_filter_receipt(source, &tree, viewport)
    });
    let font_plan = font_plan.finish_with_tree(&tree);
    #[cfg(feature = "native-fonts")]
    if let Some(context) = &options.font_context {
        check_export_glyph_coverage(context, font_plan)?;
    }
    let fallback_css_residual = svg.finalization_report().fallback_css_residual();
    let conversion_plan = plan_svg_conversion(&tree, options.conversion_limits, control)?;
    let directional_filter_transform_residual =
        !raster_filter_transforms_supported(tree.root(), control)?;
    let embedded_image_plan =
        plan_embedded_images(&tree, options.embedded_image_limit, data_plan, control)?;
    let plan = raster_plan_for_geometry(geometry, options, control)?;
    export_checkpoint(control)?;

    Ok(PreparedRaster {
        tree,
        geometry,
        translate_min_to_origin,
        report: RasterReportSeed {
            native_filter_receipt,
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
        font_context: options.font_context.clone(),
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
    export_checkpoint(&control)?;
    let svg = svg.clone();
    let options = options.clone();
    let backend_control = control.clone();
    run_recursive_svg_backend(&control, move |control| {
        prepare_pdf_on_backend_stack(&svg, &options, control).map(|mut prepared| {
            prepared.control = backend_control;
            prepared
        })
    })
}

#[cfg(feature = "pdf")]
fn prepare_pdf_on_backend_stack(
    svg: &ResvgCompatibleSvg,
    options: &PdfOptions,
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
    let data_plan = plan_embedded_data_resources_with_occurrences(
        source,
        svg.reference_plan().raw_element_occurrences(),
        options.embedded_image_limit,
        control,
    )?;
    let (tree, font_plan) = parse_pdf_tree(svg, options, control)?;
    let fallback_css_residual = svg.finalization_report().fallback_css_residual();
    let metadata = parse_root_svg_metadata(source, control)?;
    let implicit_viewport_clipping_residual =
        if !metadata.has_view_box && !metadata.has_explicit_size {
            let layer = tree.root().abs_layer_bounding_box();
            let viewport = tree.size();
            layer.left() < 0.0
                || layer.top() < 0.0
                || layer.right() > viewport.width()
                || layer.bottom() > viewport.height()
        } else {
            false
        };
    let native_filter_receipt = tree.size().to_rect(0.0, 0.0).and_then(|viewport| {
        native_filter_receipt::preflight_native_filter_receipt(source, &tree, viewport)
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
    let native_filter_fully_localized = pdf_native_filter_fully_localized(
        &tree,
        layout.drawing_size.width() / svg_size.width(),
        options.filter_scale,
        filter_plan,
        native_filter_receipt,
        control,
    )?;
    let report = PdfExportReport {
        native_filter_receipt,
        native_filter_fully_localized,
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
        font_context: options.font_context.clone(),
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

#[cfg(feature = "pdf")]
fn parse_pdf_tree(
    svg: &ResvgCompatibleSvg,
    options: &PdfOptions,
    control: &OperationControl,
) -> Result<(usvg::Tree, ExportFontPlan)> {
    export_checkpoint(control)?;
    let mut opts = usvg::Options::default();
    let font_plan = configure_usvg_options_for_pdf(&mut opts, svg, options, control)?;
    let tree = usvg::Tree::from_str(svg.as_str(), &opts).map_err(|_| ExportError::SvgParse);
    export_checkpoint(control)?;
    #[cfg(feature = "native-fonts")]
    if let Some(context) = &options.font_context {
        context.check()?;
    }
    let tree = tree?;
    let font_plan = font_plan.finish_with_tree(&tree);
    #[cfg(feature = "native-fonts")]
    if let Some(context) = &options.font_context {
        check_export_glyph_coverage(context, font_plan)?;
    }
    Ok((tree, font_plan))
}

#[cfg(all(
    feature = "native-fonts",
    any(feature = "png", feature = "jpeg", feature = "pdf")
))]
fn check_export_glyph_coverage(context: &NativeFontContext, plan: ExportFontPlan) -> Result<()> {
    if context.strict_glyph_coverage()
        && (plan.unresolved_font_request()
            || plan.unresolved_glyph_fallback()
            || plan.notdef_glyph_count() != 0)
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

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn plan_svg_conversion(
    tree: &usvg::Tree,
    limits: SvgConversionLimits,
    control: &OperationControl,
) -> Result<SvgConversionPlan> {
    let mut plan = SvgConversionPlan::default();
    plan_svg_conversion_group(tree.root(), 0, 0, limits, &mut plan, control)?;
    Ok(plan)
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn plan_svg_conversion_group(
    root: &usvg::Group,
    parent_isolation_depth: usize,
    tree_depth: usize,
    limits: SvgConversionLimits,
    plan: &mut SvgConversionPlan,
    control: &OperationControl,
) -> Result<()> {
    let mut stack = vec![(root, parent_isolation_depth, tree_depth)];
    while let Some((group, parent_depth, tree_depth)) = stack.pop() {
        export_checkpoint(control)?;
        charge_svg_conversion_tree_node(plan)?;
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
            export_checkpoint(control)?;
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
            export_checkpoint(control)?;
            if let usvg::Node::Group(child) = node {
                stack.push((child, isolation_depth, tree_depth.saturating_add(1)));
            } else {
                charge_svg_conversion_tree_node(plan)?;
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
                        control,
                    );
                }
            });
            subroot_result?;
        }
    }
    Ok(())
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn charge_svg_conversion_tree_node(plan: &mut SvgConversionPlan) -> Result<()> {
    plan.tree_nodes = plan.tree_nodes.saturating_add(1);
    check_svg_conversion_limit(
        merman_render::resources::SVG_BACKEND_TREE_NODES_HARD_CAP_ID,
        plan.tree_nodes,
        Some(merman_render::resources::MAX_RESVG_TREE_NODES),
    )
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

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn raster_filter_transforms_supported(
    root: &usvg::Group,
    control: &OperationControl,
) -> Result<bool> {
    let mut stack = vec![(root, usvg::Transform::identity())];
    while let Some((group, parent_transform)) = stack.pop() {
        export_checkpoint(control)?;
        let transform = parent_transform.pre_concat(group.transform());
        let preserves_offset_axes =
            transform.kx == 0.0 && transform.ky == 0.0 && transform.sx > 0.0 && transform.sy > 0.0;
        if !preserves_offset_axes {
            for primitive in group
                .filters()
                .iter()
                .flat_map(|filter| filter.primitives())
            {
                export_checkpoint(control)?;
                // resvg 0.47's scale_coordinates uses only scale magnitudes, so directional
                // offsets lose rotation, shear and reflection. Report the residual without
                // preventing best-effort output. PDF uses this rasterizer inside each filtered
                // group, so its ancestor transforms remain outside this validation root.
                let directional = match primitive.kind() {
                    usvg::filter::Kind::Offset(offset) => offset.dx() != 0.0 || offset.dy() != 0.0,
                    usvg::filter::Kind::DropShadow(shadow) => {
                        shadow.dx() != 0.0 || shadow.dy() != 0.0
                    }
                    _ => false,
                };
                if directional {
                    return Ok(false);
                }
            }
        }

        let mut mask = group.mask();
        while let Some(current) = mask {
            export_checkpoint(control)?;
            stack.push((current.root(), transform));
            mask = current.mask();
        }
        for filter in group.filters() {
            for primitive in filter.primitives() {
                export_checkpoint(control)?;
                if let usvg::filter::Kind::Image(image) = primitive.kind() {
                    stack.push((image.root(), transform));
                }
            }
        }
        for node in group.children() {
            export_checkpoint(control)?;
            match node {
                usvg::Node::Group(child) => stack.push((child, transform)),
                usvg::Node::Image(image) => {
                    if let usvg::ImageKind::SVG(nested) = image.kind() {
                        stack.push((nested.root(), transform));
                    }
                }
                // Borrow these owners directly: Node::subroots limits callback borrows to the
                // callback body. resvg samples pattern paints in axis-aligned pixmaps.
                usvg::Node::Path(path) => {
                    for paint in [
                        path.fill().map(|fill| fill.paint()),
                        path.stroke().map(|stroke| stroke.paint()),
                    ]
                    .into_iter()
                    .flatten()
                    {
                        if let usvg::Paint::Pattern(pattern) = paint {
                            stack.push((pattern.root(), usvg::Transform::identity()));
                        }
                    }
                }
                usvg::Node::Text(text) => {
                    stack.push((text.flattened(), usvg::Transform::identity()));
                }
            }
        }
    }
    Ok(true)
}

#[cfg(feature = "pdf")]
fn plan_pdf_filter_images(
    tree: &usvg::Tree,
    page_scale: f32,
    requested_scale: f32,
    limit: PdfFilterImageLimit,
    control: &OperationControl,
) -> Result<(PdfFilterImagePlan, bool)> {
    let filtered_groups = pdf_filtered_group_bounds(tree, page_scale, control)?;
    let directional_residual = filtered_groups
        .iter()
        .any(|geometry| geometry.directional_transform_residual);
    let requested_pixels = pdf_filter_pixels(&filtered_groups, requested_scale, control)?.ok_or(
        ExportError::InvalidSizing(
            "PDF filter scale and backend side cap must leave every image dimension nonzero",
        ),
    )?;
    let Some(max_pixels) = limit.max_total_pixels else {
        return Ok((
            PdfFilterImagePlan {
                filtered_groups: filtered_groups.len(),
                requested_scale,
                effective_scale: requested_scale,
                requested_image_pixels: requested_pixels,
                effective_image_pixels: requested_pixels,
                limited: false,
            },
            directional_residual,
        ));
    };
    if requested_pixels <= max_pixels {
        return Ok((
            PdfFilterImagePlan {
                filtered_groups: filtered_groups.len(),
                requested_scale,
                effective_scale: requested_scale,
                requested_image_pixels: requested_pixels,
                effective_image_pixels: requested_pixels,
                limited: false,
            },
            directional_residual,
        ));
    }

    let mut lower = 0.0_f32;
    let mut accepted = None;
    let mut rejected = requested_scale;
    for _ in 0..48 {
        let candidate = lower + (rejected - lower) / 2.0;
        export_checkpoint(control)?;
        match pdf_filter_pixels(&filtered_groups, candidate, control)? {
            Some(pixels) if pixels <= max_pixels => {
                lower = candidate;
                accepted = Some((candidate, pixels));
            }
            Some(_) => rejected = candidate,
            // A zero-size backend image drops the entire group. It is not a valid budget fit.
            None => lower = candidate,
        }
    }
    let (accepted, effective_pixels) = accepted.ok_or(ExportError::PdfFilterImageLimit {
        actual: requested_pixels,
        max: max_pixels,
    })?;
    Ok((
        PdfFilterImagePlan {
            filtered_groups: filtered_groups.len(),
            requested_scale,
            effective_scale: accepted,
            requested_image_pixels: requested_pixels,
            effective_image_pixels: effective_pixels,
            limited: true,
        },
        directional_residual,
    ))
}

#[cfg(feature = "pdf")]
struct PdfFilterGeometry {
    width: f32,
    height: f32,
    scale: f32,
    directional_transform_residual: bool,
}

#[cfg(feature = "pdf")]
fn pdf_filtered_group_bounds(
    tree: &usvg::Tree,
    page_scale: f32,
    control: &OperationControl,
) -> Result<Vec<PdfFilterGeometry>> {
    let mut bounds = Vec::new();
    let mut stack = vec![(
        tree.root(),
        usvg::Transform::from_scale(page_scale, page_scale),
    )];
    while let Some((group, parent_transform)) = stack.pop() {
        export_checkpoint(control)?;
        if !group.filters().is_empty() {
            // krilla reapplies vector ancestor transforms after rasterization, but a transform
            // on this group or its rasterized descendants still reaches resvg's offset bug.
            let directional_transform_residual =
                !raster_filter_transforms_supported(group, control)?;
            // Match krilla-svg 0.8.1's filter.rs before its f32 side cap and pixel rounding.
            // Absolute bounds are insufficient: isolated PDF streams reset their current CTM.
            let local = group
                .layer_bounding_box()
                .transform(group.transform())
                .ok_or(ExportError::SvgDocSize)?;
            let actual = group
                .layer_bounding_box()
                .transform(parent_transform)
                .and_then(|bbox| bbox.transform(group.transform()))
                .ok_or(ExportError::SvgDocSize)?;
            bounds.push(PdfFilterGeometry {
                width: local.width(),
                height: local.height(),
                scale: (actual.width() / local.width()).max(actual.height() / local.height()),
                directional_transform_residual,
            });
            // krilla-svg rasterizes this whole group through resvg and returns. Descendant
            // filters contribute to that localized render, but they do not allocate a second
            // krilla-owned PDF image and must not be counted as independent top-level groups.
            continue;
        }

        let mut transform = parent_transform.pre_concat(group.transform());
        if let Some(clip) = group.clip_path()
            && pdf_clip_path_uses_mask(clip, control)?
        {
            let mut clip = Some(clip);
            while let Some(current) = clip {
                export_checkpoint(control)?;
                if pdf_clip_path_uses_mask(current, control)? {
                    stack.push((current.root(), current.transform()));
                }
                clip = current.clip_path();
            }
            transform = usvg::Transform::identity();
        }
        let mut mask = group.mask();
        while let Some(current) = mask {
            export_checkpoint(control)?;
            stack.push((current.root(), usvg::Transform::identity()));
            mask = current.mask();
        }
        // These operations start new krilla content streams with an identity CTM.
        if group.isolate()
            || group.opacity() != usvg::Opacity::ONE
            || group.mask().is_some()
            || group.blend_mode() != usvg::BlendMode::Normal
        {
            transform = usvg::Transform::identity();
        }
        for node in group.children() {
            export_checkpoint(control)?;
            match node {
                usvg::Node::Group(child) => stack.push((child, transform)),
                usvg::Node::Image(image) => {
                    if let usvg::ImageKind::SVG(nested) = image.kind() {
                        stack.push((nested.root(), transform));
                    }
                }
                // Pattern paints use separate streams, independent of the consuming path CTM.
                // Keep tree-owner borrows instead of escaping Node::subroots callback borrows.
                usvg::Node::Path(path) => {
                    for paint in [
                        path.fill().map(|fill| fill.paint()),
                        path.stroke().map(|stroke| stroke.paint()),
                    ]
                    .into_iter()
                    .flatten()
                    {
                        if let usvg::Paint::Pattern(pattern) = paint {
                            stack.push((pattern.root(), usvg::Transform::identity()));
                        }
                    }
                }
                usvg::Node::Text(text) => {
                    stack.push((text.flattened(), usvg::Transform::identity()));
                }
            }
        }
    }
    Ok(bounds)
}

#[cfg(feature = "pdf")]
fn pdf_clip_path_uses_mask(clip: &usvg::ClipPath, control: &OperationControl) -> Result<bool> {
    // Match krilla-svg's native-clip admission; complex clips use a new masked content stream.
    let mut stack = vec![clip.root()];
    let mut rules = 0_usize;
    let mut nonzero = 0_usize;
    while let Some(group) = stack.pop() {
        export_checkpoint(control)?;
        for node in group.children() {
            export_checkpoint(control)?;
            match node {
                usvg::Node::Group(child) => {
                    if child.clip_path().is_some() {
                        return Ok(true);
                    }
                    stack.push(child);
                }
                usvg::Node::Text(text) => stack.push(text.flattened()),
                usvg::Node::Path(path) => {
                    if let Some(fill) = path.fill() {
                        rules += 1;
                        nonzero += usize::from(fill.rule() == usvg::FillRule::NonZero);
                    }
                }
                usvg::Node::Image(_) => {}
            }
        }
    }
    Ok(nonzero != rules && !(nonzero == 0 && rules == 1))
}

#[cfg(feature = "pdf")]
fn pdf_native_filter_fully_localized(
    tree: &usvg::Tree,
    page_scale: f32,
    requested_scale: f32,
    filter_plan: PdfFilterImagePlan,
    receipt: Option<merman_render::__private::NativeSvgFilterReceipt>,
    control: &OperationControl,
) -> Result<bool> {
    let Some(receipt) = receipt else {
        return Ok(false);
    };
    let Ok(reference_count) = usize::try_from(receipt.reference_count()) else {
        return Ok(false);
    };
    if filter_plan.limited
        || filter_plan.effective_scale != requested_scale
        || filter_plan.filtered_groups != reference_count
    {
        return Ok(false);
    }

    let bounds = pdf_filtered_group_bounds(tree, page_scale, control)?;
    if bounds.len() != reference_count {
        return Ok(false);
    }
    Ok(bounds.into_iter().all(|geometry| {
        let scale = f64::from(geometry.scale) * f64::from(requested_scale);
        let width = f64::from(geometry.width) * scale;
        let height = f64::from(geometry.height) * scale;
        width.is_finite()
            && height.is_finite()
            && width <= KRILLA_MAX_FILTER_SIDE_PX
            && height <= KRILLA_MAX_FILTER_SIDE_PX
    }))
}

#[cfg(feature = "pdf")]
fn pdf_filter_pixels(
    bounds: &[PdfFilterGeometry],
    filter_scale: f32,
    control: &OperationControl,
) -> Result<Option<u64>> {
    let mut total = 0_u64;
    for geometry in bounds {
        export_checkpoint(control)?;
        let max_scale = ((KRILLA_MAX_FILTER_SIDE_PX as f32) / geometry.width)
            .min((KRILLA_MAX_FILTER_SIDE_PX as f32) / geometry.height);
        let scale = (geometry.scale * filter_scale).min(max_scale);
        let width = geometry.width * scale;
        let height = geometry.height * scale;
        if !width.is_finite() || !height.is_finite() {
            return Err(ExportError::InvalidSizing(
                "PDF filter image dimensions must be finite",
            ));
        }
        let width_px = width.round() as u64;
        let height_px = height.round() as u64;
        if width_px == 0 || height_px == 0 {
            return Ok(None);
        }
        total = total.saturating_add(width_px.saturating_mul(height_px));
    }
    Ok(Some(total))
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[derive(Debug, Clone, Copy, Default)]
struct EmbeddedDataPlan {
    resources: usize,
    largest_bytes: u64,
    total_bytes: u64,
}

#[cfg(all(test, any(feature = "png", feature = "jpeg", feature = "pdf")))]
fn plan_embedded_data_resources(svg: &str, limit: EmbeddedImageLimit) -> Result<EmbeddedDataPlan> {
    plan_embedded_data_resources_with_occurrences(svg, &[], limit, &OperationControl::new())
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn plan_embedded_data_resources_with_occurrences(
    svg: &str,
    raw_element_occurrences: &[usize],
    limit: EmbeddedImageLimit,
    control: &OperationControl,
) -> Result<EmbeddedDataPlan> {
    use quick_xml::XmlVersion;
    use quick_xml::events::Event;

    validate_embedded_image_limit(limit)?;
    let mut reader = quick_xml::Reader::from_str(svg);
    let mut plan = EmbeddedDataPlan::default();
    let mut element_index = 0usize;
    loop {
        export_checkpoint(control)?;
        let event = reader.read_event().map_err(|_| ExportError::SvgParse)?;
        let (element, occurrences) = match event {
            Event::Start(element) | Event::Empty(element) => {
                let occurrences = if raw_element_occurrences.is_empty() {
                    1
                } else {
                    raw_element_occurrences
                        .get(element_index)
                        .copied()
                        .ok_or(ExportError::SvgParse)?
                };
                element_index = element_index.saturating_add(1);
                if is_embedded_image_element(element.local_name().as_ref()) {
                    (element, occurrences)
                } else {
                    continue;
                }
            }
            Event::Eof => break,
            _ => continue,
        };

        for attribute in element.attributes() {
            export_checkpoint(control)?;
            let attribute = attribute.map_err(|_| ExportError::SvgParse)?;
            if !attribute
                .key
                .local_name()
                .as_ref()
                .eq_ignore_ascii_case("href")
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
            let occurrences = occurrences as u64;
            let _ = data_url.decode(|chunk| {
                if let Err(error) = export_checkpoint(control) {
                    limit_error = Some(error);
                    return Err(());
                }
                resource_bytes = resource_bytes.saturating_add(chunk.len() as u64);
                let aggregate = plan
                    .total_bytes
                    .saturating_add(resource_bytes.saturating_mul(occurrences));
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

            plan.resources = plan.resources.saturating_add(occurrences as usize);
            plan.largest_bytes = plan.largest_bytes.max(resource_bytes);
            plan.total_bytes = plan
                .total_bytes
                .saturating_add(resource_bytes.saturating_mul(occurrences));
        }
    }
    if !raw_element_occurrences.is_empty() && element_index != raw_element_occurrences.len() {
        return Err(ExportError::SvgParse);
    }
    Ok(plan)
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn is_embedded_image_element(local_name: &str) -> bool {
    // usvg resolves both elements through the same image resolver. Match qualified-name aliases
    // conservatively so namespace spelling cannot turn the preflight into a false-negative gate.
    local_name.eq_ignore_ascii_case("image") || local_name.eq_ignore_ascii_case("feImage")
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn plan_embedded_images(
    tree: &usvg::Tree,
    limit: EmbeddedImageLimit,
    data: EmbeddedDataPlan,
    control: &OperationControl,
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
    plan_embedded_images_in_group(tree.root(), limit, &mut plan, control)?;
    Ok(plan)
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn plan_embedded_images_in_group(
    root: &usvg::Group,
    limit: EmbeddedImageLimit,
    plan: &mut EmbeddedImagePlan,
    control: &OperationControl,
) -> Result<()> {
    let mut stack = vec![root];
    while let Some(group) = stack.pop() {
        export_checkpoint(control)?;
        for node in group.children() {
            export_checkpoint(control)?;
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
                    subroot_result = plan_embedded_images_in_group(subroot, limit, plan, control);
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

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[derive(Debug, Clone, Copy, Default)]
struct RootSvgMetadata {
    has_view_box: bool,
    has_explicit_size: bool,
    #[cfg(any(feature = "png", feature = "jpeg"))]
    max_width_px: Option<f32>,
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn parse_root_svg_metadata(svg: &str, control: &OperationControl) -> Result<RootSvgMetadata> {
    use quick_xml::{XmlVersion, events::Event, name::ResolveResult, reader::NsReader};

    let mut reader = NsReader::from_str(svg);
    reader.config_mut().enable_all_checks(true);
    loop {
        export_checkpoint(control)?;
        let event = reader.read_event().map_err(|_| ExportError::SvgParse)?;
        let element = match event {
            Event::Start(element) | Event::Empty(element) => element,
            Event::Eof => return Err(ExportError::SvgParse),
            _ => continue,
        };

        let (namespace, local_name) = reader.resolver().resolve_element(element.name());
        let is_svg_namespace = matches!(namespace, ResolveResult::Unbound)
            || matches!(
                namespace,
                ResolveResult::Bound(namespace)
                    if namespace.as_ref() == "http://www.w3.org/2000/svg"
            );
        if !is_svg_namespace || local_name.as_ref() != "svg" {
            return Err(ExportError::SvgParse);
        }

        let mut metadata = RootSvgMetadata::default();
        let mut view_box_seen = false;
        let mut width = None;
        let mut height = None;
        #[cfg(any(feature = "png", feature = "jpeg"))]
        let mut style_seen = false;
        for attribute in element.attributes() {
            export_checkpoint(control)?;
            let attribute = attribute.map_err(|_| ExportError::SvgParse)?;
            if attribute.key.as_namespace_binding().is_some() {
                continue;
            }
            let (namespace, local_name) = reader.resolver().resolve_attribute(attribute.key);
            #[cfg(any(feature = "png", feature = "jpeg"))]
            let is_unbound_attribute = matches!(&namespace, ResolveResult::Unbound);
            let consumed_by_usvg = match namespace {
                ResolveResult::Unknown(_) => return Err(ExportError::SvgParse),
                ResolveResult::Unbound => true,
                ResolveResult::Bound(namespace) => matches!(
                    namespace.as_ref(),
                    "http://www.w3.org/2000/svg"
                        | "http://www.w3.org/1999/xlink"
                        | "http://www.w3.org/XML/1998/namespace"
                ),
            };
            if !consumed_by_usvg {
                continue;
            }

            let value = attribute
                .normalized_value(XmlVersion::Implicit1_0)
                .map_err(|_| ExportError::SvgParse)?;
            match local_name.as_ref() {
                "viewBox" if !view_box_seen => {
                    view_box_seen = true;
                    metadata.has_view_box = has_valid_svg_view_box(value.as_ref());
                }
                "width" if width.is_none() => {
                    width = Some(has_explicit_svg_length(value.as_ref()));
                }
                "height" if height.is_none() => {
                    height = Some(has_explicit_svg_length(value.as_ref()));
                }
                // usvg projects namespaced presentation attributes by local name, but parses the
                // `style` declaration list only from the unbound XML attribute.
                #[cfg(any(feature = "png", feature = "jpeg"))]
                "style" if is_unbound_attribute && !style_seen => {
                    style_seen = true;
                    metadata.max_width_px = parse_inline_max_width_px(value.as_ref(), control)?;
                }
                _ => {}
            }
        }
        metadata.has_explicit_size = width == Some(true) && height == Some(true);
        return Ok(metadata);
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn has_explicit_svg_length(value: &str) -> bool {
    value.parse::<svgtypes::Length>().is_ok_and(|length| {
        length.unit != svgtypes::LengthUnit::Percent
            && length.number.is_finite()
            && length.number > 0.0
    })
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn has_valid_svg_view_box(value: &str) -> bool {
    let Ok(view_box) = value.parse::<svgtypes::ViewBox>() else {
        return false;
    };
    usvg::NonZeroRect::from_xywh(
        view_box.x as f32,
        view_box.y as f32,
        view_box.w as f32,
        view_box.h as f32,
    )
    .is_some()
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn parse_inline_max_width_px(style: &str, control: &OperationControl) -> Result<Option<f32>> {
    let mut parser = Parser::new(style);
    let mut max_width = None;

    while !parser.is_exhausted() {
        export_checkpoint(control)?;
        let declaration = parser.parse_until_after(Delimiter::Semicolon, |declaration| {
            let property = declaration.expect_ident_cloned()?;
            declaration.expect_colon()?;

            if !property.eq_ignore_ascii_case("max-width") {
                declaration.expect_no_error_token()?;
                return Ok::<_, cssparser::ParseError<()>>(None);
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
    Ok(max_width)
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
    if metadata.has_view_box || metadata.has_explicit_size {
        // `usvg`/`resvg` already apply the root viewBox transform (including translating the
        // viewBox min corner to (0,0)) when building/rendering the tree. If we also translate
        // by `-min_x/-min_y` here, diagrams with negative viewBox mins (e.g. kanban, gitGraph)
        // get shifted fully out of the viewport and render as a blank/transparent pixmap.
        let size = tree.size();
        return (
            RasterGeometry {
                min_x: 0.0,
                min_y: 0.0,
                width: size.width(),
                height: size.height(),
            },
            false,
        );
    }

    // Some Mermaid diagrams (e.g. `info`) don't emit a viewBox upstream.
    // For raster formats, fall back to the rendered content bounds as computed by usvg.
    let bbox = tree.root().abs_layer_bounding_box();
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
fn raster_plan_for_geometry(
    geo: RasterGeometry,
    options: &RasterOptions,
    control: &OperationControl,
) -> Result<RasterPlan> {
    export_checkpoint(control)?;
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
            export_checkpoint(control)?;
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
fn configure_usvg_options_for_raster(
    opt: &mut usvg::Options<'_>,
    metadata: RootSvgMetadata,
    svg: &ResvgCompatibleSvg,
    options: &RasterOptions,
    control: &OperationControl,
) -> Result<font_environment::ExportFontPlanSeed> {
    #[cfg(feature = "native-fonts")]
    let plan = if let Some(context) = &options.font_context {
        context.check()?;
        let observer = context.apply_to_usvg_options_for_export(opt, control);
        font_environment::ExportFontPlanSeed::from_native_context(context, observer)
    } else {
        let fonts = font_environment::ExportFontEnvironment::from_svg(svg)?;
        let plan = fonts.plan;
        opt.fontdb = fonts.fontdb;
        opt.font_family = fonts.default_family;
        opt.font_resolver = fonts.resolver;
        plan
    };
    #[cfg(not(feature = "native-fonts"))]
    let plan = {
        let _ = (options, control);
        let fonts = font_environment::ExportFontEnvironment::from_svg(svg)?;
        let plan = fonts.plan;
        opt.fontdb = fonts.fontdb;
        opt.font_family = fonts.default_family;
        opt.font_resolver = fonts.resolver;
        plan
    };

    if !metadata.has_view_box
        && let Some(max_width) = metadata.max_width_px
        && max_width.is_finite()
        && max_width > 0.0
        && let Some(size) = usvg::Size::from_wh(max_width, opt.default_size.height())
    {
        opt.default_size = size;
    }

    opt.image_href_resolver = data_url_only_image_href_resolver();
    Ok(plan)
}

#[cfg(feature = "pdf")]
fn configure_usvg_options_for_pdf(
    opt: &mut usvg::Options<'_>,
    svg: &ResvgCompatibleSvg,
    options: &PdfOptions,
    control: &OperationControl,
) -> Result<font_environment::ExportFontPlanSeed> {
    #[cfg(feature = "native-fonts")]
    let plan = if let Some(context) = &options.font_context {
        context.check()?;
        let observer = context.apply_to_usvg_options_for_export(opt, control);
        font_environment::ExportFontPlanSeed::from_native_context(context, observer)
    } else {
        let fonts = font_environment::ExportFontEnvironment::from_svg(svg)?;
        let plan = fonts.plan;
        opt.fontdb = fonts.fontdb;
        opt.font_family = fonts.default_family;
        opt.font_resolver = fonts.resolver;
        plan
    };
    #[cfg(not(feature = "native-fonts"))]
    let plan = {
        let _ = (options, control);
        let fonts = font_environment::ExportFontEnvironment::from_svg(svg)?;
        let plan = fonts.plan;
        opt.fontdb = fonts.fontdb;
        opt.font_family = fonts.default_family;
        opt.font_resolver = fonts.resolver;
        plan
    };
    opt.image_href_resolver = data_url_only_image_href_resolver();
    Ok(plan)
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn data_url_only_image_href_resolver() -> usvg::ImageHrefResolver<'static> {
    usvg::ImageHrefResolver {
        resolve_data: usvg::ImageHrefResolver::default_data_resolver(),
        resolve_string: Box::new(|_, _| None),
    }
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
    let s = text.trim().to_ascii_lowercase();
    match s.as_str() {
        "transparent" => {
            return Ok(ExportRgbaColor {
                red: 0,
                green: 0,
                blue: 0,
                alpha: 0,
            });
        }
        "white" => {
            return Ok(ExportRgbaColor {
                red: 255,
                green: 255,
                blue: 255,
                alpha: 255,
            });
        }
        "black" => {
            return Ok(ExportRgbaColor {
                red: 0,
                green: 0,
                blue: 0,
                alpha: 255,
            });
        }
        _ => {}
    }

    let hex = s.strip_prefix('#').ok_or(())?;
    fn hex2(b: &[u8]) -> std::result::Result<u8, ()> {
        let hi = (*b.first().ok_or(())? as char).to_digit(16).ok_or(())? as u8;
        let lo = (*b.get(1).ok_or(())? as char).to_digit(16).ok_or(())? as u8;
        Ok((hi << 4) | lo)
    }
    fn hex1(c: u8) -> std::result::Result<u8, ()> {
        let v = (c as char).to_digit(16).ok_or(())? as u8;
        Ok((v << 4) | v)
    }

    let bytes = hex.as_bytes();
    match bytes.len() {
        3 => Ok(ExportRgbaColor {
            red: hex1(bytes[0])?,
            green: hex1(bytes[1])?,
            blue: hex1(bytes[2])?,
            alpha: 255,
        }),
        4 => Ok(ExportRgbaColor {
            red: hex1(bytes[0])?,
            green: hex1(bytes[1])?,
            blue: hex1(bytes[2])?,
            alpha: hex1(bytes[3])?,
        }),
        6 => Ok(ExportRgbaColor {
            red: hex2(&bytes[0..2])?,
            green: hex2(&bytes[2..4])?,
            blue: hex2(&bytes[4..6])?,
            alpha: 255,
        }),
        8 => Ok(ExportRgbaColor {
            red: hex2(&bytes[0..2])?,
            green: hex2(&bytes[2..4])?,
            blue: hex2(&bytes[4..6])?,
            alpha: hex2(&bytes[6..8])?,
        }),
        _ => Err(()),
    }
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
mod font_resolver_tests {
    use super::*;
    use crate::font_environment::shared_system_fontdb;
    use crate::font_selection::{FontSelectionPolicy, face_family_name};
    use std::sync::{Arc, Mutex};

    #[test]
    fn browser_font_resolver_matches_named_families_case_insensitively() {
        let (fontdb, expected_id) = controlled_fontdb();

        assert_eq!(
            selected_font_id(Arc::clone(&fontdb), "'Maße Test Sans'"),
            expected_id,
            "the installed family spelling must select its named face"
        );
        assert_eq!(
            selected_font_id(Arc::clone(&fontdb), "'maße test sans'"),
            expected_id,
            "CSS family matching must ignore ASCII case"
        );
        assert_eq!(
            selected_font_id(Arc::clone(&fontdb), "'MASSE TEST SANS'"),
            expected_id,
            "CSS family matching must use Unicode default case folding"
        );
        assert_eq!(
            selected_font_id(
                Arc::clone(&fontdb),
                "'__merman_missing_font__', 'masse test sans'"
            ),
            expected_id,
            "case-insensitive matching must preserve CSS family stack order"
        );
    }

    #[test]
    fn browser_font_resolver_rejects_a_poisoned_generic_but_preserves_valid_serif() {
        for fixture in [
            include_bytes!("../tests/fonts/MermanSymbolicTest.ttf").as_slice(),
            include_bytes!("../tests/fonts/MermanPictorialTest.ttf").as_slice(),
        ] {
            let mut database = usvg::fontdb::Database::new();
            database.load_font_data(fixture.to_vec());
            let symbol = face_family_name(database.faces().next().unwrap()).unwrap();
            database.load_font_data(
                include_bytes!(
                    "../../merman-render/tests/fixtures/fonts/DejaVuSerif-NativeFilter.ttf"
                )
                .to_vec(),
            );
            let serif = database
                .faces()
                .find(|face| face.post_script_name == "DejaVuSerif")
                .unwrap()
                .id;
            database.set_serif_family(symbol.clone());
            database.set_sans_serif_family(symbol.clone());
            database.set_monospace_family(symbol.clone());
            FontSelectionPolicy::from_database(&database).configure_generics(&mut database);
            for family in ["serif", "sans-serif", "monospace", "__missing__"] {
                assert_eq!(
                    selected_font_id(Arc::new(database.clone()), family),
                    serif,
                    "poisoned {family}"
                );
            }
            database.set_serif_family("DejaVu Serif");
            FontSelectionPolicy::from_database(&database).configure_generics(&mut database);
            assert_eq!(selected_font_id(Arc::new(database), "serif"), serif);
        }
    }

    fn controlled_fontdb() -> (Arc<usvg::fontdb::Database>, usvg::fontdb::ID) {
        let source_face = shared_system_fontdb()
            .faces()
            .next()
            .cloned()
            .expect("native export tests require one valid system font face");

        let mut fontdb = usvg::fontdb::Database::new();
        let mut fallback = source_face.clone();
        fallback.families = vec![(
            "Merman Fallback Serif".to_string(),
            usvg::fontdb::Language::English_UnitedStates,
        )];
        fallback.weight = usvg::fontdb::Weight::NORMAL;
        fallback.stretch = usvg::fontdb::Stretch::Normal;
        fallback.style = usvg::fontdb::Style::Normal;

        let mut target = source_face;
        target.families = vec![(
            "Maße Test Sans".to_string(),
            usvg::fontdb::Language::English_UnitedStates,
        )];
        target.weight = usvg::fontdb::Weight::NORMAL;
        target.stretch = usvg::fontdb::Stretch::Normal;
        target.style = usvg::fontdb::Style::Normal;

        fontdb.set_serif_family("Merman Fallback Serif");
        fontdb.set_sans_serif_family("Merman Fallback Serif");
        fontdb.set_monospace_family("Merman Fallback Serif");
        let _fallback_id = fontdb.push_face_info(fallback);
        let target_id = fontdb.push_face_info(target);
        (Arc::new(fontdb), target_id)
    }

    fn selected_font_id(
        fontdb: Arc<usvg::fontdb::Database>,
        font_family: &str,
    ) -> usvg::fontdb::ID {
        let selected = Arc::new(Mutex::new(Vec::new()));
        let selected_for_resolver = Arc::clone(&selected);
        let resolver =
            browser_like_font_resolver(Arc::new(FontSelectionPolicy::from_database(&fontdb)));
        let select_font = resolver.select_font;
        let options = usvg::Options {
            fontdb,
            font_resolver: usvg::FontResolver {
                select_font: Box::new(move |font, fontdb| {
                    let id = select_font(font, fontdb);
                    if let Some(id) = id {
                        selected_for_resolver
                            .lock()
                            .expect("selected-font recorder lock")
                            .push(id);
                    }
                    id
                }),
                select_fallback: resolver.select_fallback,
            },
            ..Default::default()
        };

        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 60"><text x="10" y="40" font-size="24" style="font-family: {font_family};">A</text></svg>"#
        );
        usvg::Tree::from_str(&svg, &options).expect("parse test SVG with native font resolver");

        selected
            .lock()
            .expect("selected-font recorder lock")
            .first()
            .copied()
            .expect("text parsing must select a primary font")
    }
}

#[cfg(all(
    test,
    feature = "native-fonts",
    any(feature = "png", feature = "jpeg", feature = "pdf")
))]
fn fixture_native_font_context() -> NativeFontContext {
    NativeFontContext::from_font_bytes(
        [include_bytes!("../tests/fonts/Cousine-Regular.ttf").to_vec()],
        NativeFontLimits::default(),
        &OperationControl::new(),
    )
    .expect("explicit export test font")
}

#[cfg(all(test, any(feature = "png", feature = "jpeg")))]
fn fixture_raster_options() -> RasterOptions {
    #[cfg(feature = "native-fonts")]
    if !system_font_discovery_available() {
        return RasterOptions::default().with_font_context(fixture_native_font_context());
    }
    RasterOptions::default()
}

#[cfg(all(test, feature = "pdf"))]
fn fixture_pdf_options() -> PdfOptions {
    #[cfg(feature = "native-fonts")]
    if !system_font_discovery_available() {
        return PdfOptions::default().with_font_context(fixture_native_font_context());
    }
    PdfOptions::default()
}

#[cfg(all(
    test,
    not(all(feature = "system-fonts", not(target_family = "wasm"))),
    any(feature = "png", feature = "jpeg", feature = "pdf")
))]
mod unavailable_source_tests {
    use super::*;

    fn sealed_svg() -> ResvgCompatibleSvg {
        let session = merman_render::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        merman_render::svg::finalize_resvg_svg(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="40"><text x="4" y="24">No implicit fonts</text></svg>"#,
            &session,
        )
        .unwrap()
    }

    #[test]
    fn catalogs_do_not_advertise_uncompiled_system_discovery() {
        assert!(!system_font_discovery_available());
        for format in ["png", "jpeg", "pdf"] {
            if let Some(contract) = output_environment_contract(format) {
                assert!(contract.system_fonts.is_none());
            }
        }
    }

    #[cfg(feature = "png")]
    #[test]
    fn png_without_context_returns_font_source_unavailable() {
        assert!(matches!(
            svg_to_png(&sealed_svg(), &RasterOptions::default()),
            Err(ExportError::FontSourceUnavailable)
        ));
    }

    #[cfg(feature = "jpeg")]
    #[test]
    fn jpeg_without_context_returns_font_source_unavailable() {
        assert!(matches!(
            svg_to_jpeg(&sealed_svg(), &RasterOptions::default()),
            Err(ExportError::FontSourceUnavailable)
        ));
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn pdf_without_context_returns_font_source_unavailable() {
        assert!(matches!(
            svg_to_pdf(&sealed_svg()),
            Err(ExportError::FontSourceUnavailable)
        ));
    }
}

#[cfg(all(test, feature = "png"))]
#[cfg(any(
    feature = "native-fonts",
    all(feature = "system-fonts", not(target_family = "wasm"))
))]
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

    fn assert_typed_color_png_pixel(input: &str, expected: [u8; 4]) {
        let merman_render::diagram_theme::CanvasPaint::Solid(color) =
            merman_render::diagram_theme::CanvasPaint::solid(input).expect("valid typed color")
        else {
            panic!("a solid typed color must remain solid");
        };
        let source = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><rect width="16" height="16" fill="{}"/></svg>"#,
            color.as_css(),
        );
        let session = merman_render::environment::RenderEnvironment::deterministic()
            .begin_session()
            .expect("deterministic color render session");
        let svg = merman_render::svg::finalize_resvg_svg(&source, &session)
            .expect("seal typed color SVG");
        let bytes = svg_to_png(&svg, &fixture_raster_options()).expect("export typed color PNG");
        let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        let mut reader = decoder.read_info().expect("read color PNG");
        let mut pixels = vec![0; reader.output_buffer_size().expect("PNG output size")];
        let info = reader.next_frame(&mut pixels).expect("decode color PNG");
        assert_eq!((info.width, info.height), (16, 16));
        assert_eq!(info.color_type, png::ColorType::Rgba);
        assert_eq!(info.bit_depth, png::BitDepth::Eight);
        let center = ((8 * info.width + 8) * 4) as usize;
        assert_eq!(
            &pixels[center..center + 4],
            &expected,
            "typed color {input}"
        );
    }

    #[test]
    fn typed_rgb_slash_alpha_keeps_native_png_channels() {
        assert_typed_color_png_pixel("rgba(255, 0, 0, .5)", [255, 0, 0, 128]);
        assert_typed_color_png_pixel("rgb(255 0 0 / .5)", [255, 0, 0, 128]);
    }

    #[test]
    fn typed_percentage_alpha_keeps_native_png_channels() {
        for input in ["rgb(100% 0% 0% / 50%)", "rgba(255, 0, 0, 50%)"] {
            assert_typed_color_png_pixel(input, [255, 0, 0, 128]);
        }
    }

    #[test]
    fn typed_hsl_angle_units_keep_native_png_channels() {
        assert_typed_color_png_pixel("hsl(120deg 100% 50%)", [0, 255, 0, 255]);
        for input in [
            "hsl(.5turn 100% 50%)",
            "hsl(200grad 100% 50%)",
            "hsl(3.141592653589793rad 100% 50%)",
            "hsl(.5TURN 100% 50%)",
            "hsl(200GRAD 100% 50%)",
        ] {
            assert_typed_color_png_pixel(input, [0, 255, 255, 255]);
        }
        assert_typed_color_png_pixel("hsl(.5turn 100% 50% / 25%)", [0, 255, 255, 64]);
    }

    #[test]
    fn typed_negative_hue_keeps_native_png_channels() {
        assert_typed_color_png_pixel("hsl(-300deg 50% 50%)", [191, 191, 64, 255]);
    }

    #[test]
    fn typed_percentage_first_mixed_rgb_keeps_native_png_channels() {
        assert_typed_color_png_pixel("rgb(100% 128 0)", [255, 128, 0, 255]);
    }

    #[test]
    fn typed_number_first_mixed_rgb_keeps_native_png_channels() {
        assert_typed_color_png_pixel("rgb(128 100% 0)", [128, 255, 0, 255]);
    }

    #[test]
    fn typed_half_percentage_keeps_native_png_rounding() {
        assert_typed_color_png_pixel("rgb(50% 0% 0% / 1)", [128, 0, 0, 255]);
    }

    #[test]
    fn png_leaf_encodes_a_sealed_svg() {
        let bytes = svg_to_png(&compatible_svg(), &fixture_raster_options())
            .expect("PNG export should be callable when its leaf is enabled");

        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    fn controlled_png_preserves_successful_bytes() {
        let svg = compatible_svg();
        let legacy = svg_to_png(&svg, &fixture_raster_options()).expect("legacy PNG export");
        let controlled =
            svg_to_png_controlled(&svg, &fixture_raster_options(), OperationControl::new())
                .expect("controlled PNG export");

        assert_eq!(controlled, legacy);
    }

    #[test]
    fn pre_cancelled_png_returns_no_prepared_artifact_or_bytes() {
        let svg = compatible_svg();
        let control = OperationControl::new();
        control.cancel();

        let prepared_error =
            match prepare_raster_controlled(&svg, &fixture_raster_options(), control.clone()) {
                Ok(_) => panic!("pre-cancelled preparation must fail"),
                Err(error) => error,
            };
        assert!(matches!(
            prepared_error,
            ExportError::Cancelled(OperationCancelled {
                phase: OperationPhase::Export,
                ..
            })
        ));
        assert!(prepared_error.resource_limit_details().is_none());

        let bytes_error = svg_to_png_controlled(&svg, &fixture_raster_options(), control)
            .expect_err("pre-cancelled encoding must fail");
        assert!(matches!(
            bytes_error,
            ExportError::Cancelled(OperationCancelled {
                phase: OperationPhase::Export,
                ..
            })
        ));
    }

    #[test]
    fn cancellation_after_preparation_returns_no_png_bytes() {
        let svg = compatible_svg();
        let control = OperationControl::new();
        let prepared = prepare_raster_controlled(&svg, &fixture_raster_options(), control.clone())
            .expect("preparation should succeed before cancellation");
        control.cancel();

        let error = prepared
            .encode_png()
            .expect_err("cancelled encoding must not return bytes");
        assert!(matches!(
            error,
            ExportError::Cancelled(OperationCancelled {
                phase: OperationPhase::Export,
                ..
            })
        ));
        assert!(error.resource_limit_details().is_none());
    }
}

#[cfg(all(test, feature = "jpeg"))]
#[cfg(any(
    feature = "native-fonts",
    all(feature = "system-fonts", not(target_family = "wasm"))
))]
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
        let bytes = svg_to_jpeg(&compatible_svg(), &fixture_raster_options())
            .expect("JPEG export should be callable when its leaf is enabled");

        assert!(bytes.starts_with(b"\xff\xd8\xff"));
    }

    #[test]
    fn pre_cancelled_jpeg_returns_no_bytes() {
        let svg = compatible_svg();
        let control = OperationControl::new();
        control.cancel();

        let error = svg_to_jpeg_controlled(&svg, &fixture_raster_options(), control)
            .expect_err("pre-cancelled JPEG encoding must fail");
        assert!(matches!(
            error,
            ExportError::Cancelled(OperationCancelled {
                phase: OperationPhase::Export,
                ..
            })
        ));
        assert!(error.resource_limit_details().is_none());
    }
}

#[cfg(all(test, any(feature = "png", feature = "jpeg")))]
mod root_svg_metadata_tests {
    use super::*;

    #[test]
    fn metadata_reads_only_the_root_svg_attributes() {
        let metadata = parse_root_svg_metadata(
            r#"<svg xmlns="http://www.w3.org/2000/svg" style="content: 'max-width: 9000px'; max-width: 400px"><g viewBox="0 0 9000 9000"/><text>viewBox=&quot;0 0 8000 8000&quot;</text></svg>"#,
            &OperationControl::new(),
        )
        .expect("root SVG metadata");

        assert!(!metadata.has_view_box);
        assert_eq!(metadata.max_width_px, Some(400.0));
    }

    #[test]
    fn metadata_uses_svg_number_and_css_token_grammar() {
        let metadata = parse_root_svg_metadata(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="-1.5-2,41.5,120 trailing" style="max-width: 400px nonsense"/>"#,
            &OperationControl::new(),
        )
        .expect("root SVG metadata");

        assert!(metadata.has_view_box);
        assert_eq!(metadata.max_width_px, None);
    }

    #[test]
    fn metadata_ignores_unknown_namespaces_and_uses_first_usvg_projection() {
        let metadata = parse_root_svg_metadata(
            r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:i="urn:ignored" xmlns:s="http://www.w3.org/2000/svg" i:viewBox="0 0 9000 9000" s:viewBox="invalid" viewBox="0 0 20 10" i:style="max-width: 9000px" s:style="max-width: 200px" style="max-width: 400px"/>"#,
            &OperationControl::new(),
        )
        .expect("root SVG metadata");

        assert!(
            !metadata.has_view_box,
            "the first usvg-projected viewBox is invalid, so a later alias must not replace it"
        );
        assert_eq!(
            metadata.max_width_px,
            Some(400.0),
            "usvg consumes only the unbound style declaration list"
        );
    }
}

#[cfg(all(test, feature = "pdf"))]
#[cfg(any(
    feature = "native-fonts",
    all(feature = "system-fonts", not(target_family = "wasm"))
))]
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
        let bytes = svg_to_pdf_with_options(&compatible_svg(), &fixture_pdf_options())
            .expect("PDF export should be callable when its leaf is enabled");

        assert!(bytes.starts_with(b"%PDF-"));
    }

    #[test]
    fn pre_cancelled_pdf_returns_no_prepared_artifact_or_bytes() {
        let svg = compatible_svg();
        let control = OperationControl::new();
        control.cancel();

        let prepared_error =
            match prepare_pdf_controlled(&svg, &fixture_pdf_options(), control.clone()) {
                Ok(_) => panic!("pre-cancelled PDF preparation must fail"),
                Err(error) => error,
            };
        assert!(matches!(
            prepared_error,
            ExportError::Cancelled(OperationCancelled {
                phase: OperationPhase::Export,
                ..
            })
        ));
        assert!(prepared_error.resource_limit_details().is_none());

        let bytes_error = svg_to_pdf_controlled(&svg, &fixture_pdf_options(), control)
            .expect_err("pre-cancelled PDF encoding must fail");
        assert!(matches!(
            bytes_error,
            ExportError::Cancelled(OperationCancelled {
                phase: OperationPhase::Export,
                ..
            })
        ));
    }
}

#[cfg(all(test, feature = "png", feature = "jpeg", feature = "pdf"))]
#[cfg(any(
    feature = "native-fonts",
    all(feature = "system-fonts", not(target_family = "wasm"))
))]
mod tests {
    use super::*;
    use base64::Engine as _;

    #[test]
    fn backend_tree_rechecks_preserve_the_render_owned_resource_phase() {
        for limit_name in [
            merman_render::resources::SVG_BACKEND_TREE_NODES_HARD_CAP_ID,
            merman_render::resources::SVG_BACKEND_TREE_DEPTH_HARD_CAP_ID,
        ] {
            let details = ExportError::SvgConversionLimit {
                limit_name,
                actual: 2,
                max: 1,
            }
            .resource_limit_details()
            .expect("backend hard cap details");

            assert_eq!(details.limit_id, limit_name);
            assert_eq!(details.phase, "svg_postprocess");
            assert_eq!(details.actual, 2);
            assert_eq!(details.max, 1);
            assert_eq!(
                ExportError::SvgConversionLimit {
                    limit_name,
                    actual: 2,
                    max: 1,
                }
                .resource_limit_provenance()
                .expect("backend hard cap provenance")
                .domain,
                OperationResourceDomain::Render
            );
            assert_eq!(export_resource_limit_output_ids(limit_name), None);
            assert_eq!(
                export_resource_profile_value(
                    merman_render::resources::RenderResourceProfile::Interactive,
                    limit_name,
                ),
                None
            );
        }
    }

    fn compatible_svg(svg: &str) -> merman_render::svg::ResvgCompatibleSvg {
        let session = merman_render::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        merman_render::svg::finalize_resvg_svg(svg, &session).unwrap()
    }

    #[test]
    fn backend_failure_observes_cancellation_before_returning_the_backend_error() {
        let control = OperationControl::new();
        let job_control = control.clone();
        let error = run_recursive_svg_backend(&control, move |_| -> Result<()> {
            job_control.cancel();
            Err(ExportError::SvgParse)
        })
        .expect_err("the post-backend checkpoint must observe cancellation");

        assert!(matches!(
            error,
            ExportError::Cancelled(OperationCancelled {
                phase: OperationPhase::Export,
                ..
            })
        ));
    }

    #[test]
    fn controlled_export_replays_the_first_resource_terminal_after_later_cancellation() {
        let href = png_data_uri_with_declared_size(1, 1);
        let source = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><image href="{href}" width="10" height="10"/></svg>"#
        );
        let svg = compatible_svg(&source);
        let options = RasterOptions {
            embedded_image_limit: EmbeddedImageLimit::new(Some(16), None, None, None),
            ..fixture_raster_options()
        };
        let control = OperationControl::new();
        let first = match super::prepare_raster_controlled(&svg, &options, control.clone()) {
            Ok(_) => panic!("the embedded-image byte limit must reject preparation"),
            Err(error) => error,
        };
        let first_details = first
            .resource_limit_details()
            .expect("the first rejection must expose resource metadata");
        let first_terminal = control
            .terminal_checkpoint_at(OperationPhase::Export)
            .expect_err("the first export resource rejection must latch the operation terminal");
        assert!(matches!(
            &first_terminal,
            OperationLedgerError::ResourceLimitExceeded(error)
                if error.id == MAX_EMBEDDED_IMAGE_BYTES_RESOURCE_LIMIT_ID
                    && error.resource_phase == "embedded_image_decode"
                    && error.limit == first_details.max
                    && error.consumed.saturating_add(error.requested) == first_details.actual
                    && error.provenance.domain == OperationResourceDomain::Export
                    && error.provenance.profile.is_none()
                    && error.provenance.explicit_overrides.is_empty()
        ));

        control.cancel();
        let replay = match super::prepare_raster_controlled(
            &compatible_svg(
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1"><rect width="1" height="1"/></svg>"#,
            ),
            &fixture_raster_options(),
            control,
        ) {
            Ok(_) => panic!("the existing terminal must prevent a second prepared artifact"),
            Err(error) => error,
        };

        assert_eq!(replay.resource_limit_details(), Some(first_details));
        assert!(matches!(
            replay,
            ExportError::OperationResourceTerminal(error) if error == first_terminal
        ));
        assert_eq!(
            first_details.limit_id,
            MAX_EMBEDDED_IMAGE_BYTES_RESOURCE_LIMIT_ID
        );
        assert_eq!(first_details.phase, "embedded_image_decode");
        assert_eq!(first_details.cause, ExportResourceLimitCause::Ceiling);
    }

    #[test]
    fn export_checkpoint_preserves_foreign_resource_terminal_provenance() {
        let control = OperationControl::new();
        let provenance = OperationResourceProvenance::new(
            OperationResourceDomain::Render,
            Some(merman_model::resources::ResourceProfile::Constrained),
            [merman_model::OperationResourceOverride {
                id: "max_svg_bytes",
                value: 17,
            }],
        );
        let terminal = control.terminate_resource_overflow(
            "max_svg_bytes",
            OperationPhase::Postprocess,
            "svg_postprocess",
            u64::MAX,
            17,
            provenance,
        );

        let error = export_checkpoint(&control)
            .expect_err("the export adapter must replay the foreign terminal");

        assert!(matches!(
            error,
            ExportError::OperationResourceTerminal(actual) if actual == terminal
        ));
    }

    #[test]
    fn exporter_image_resolver_never_reads_string_hrefs() {
        let resolver = data_url_only_image_href_resolver();
        let options = usvg::Options::default();

        for href in [
            "/tmp/secret.png",
            "../secret.png",
            r"\\server\share\secret.png",
            r"C:\private\secret.png",
            "https://example.com/remote.png",
        ] {
            assert!(
                (resolver.resolve_string)(href, &options).is_none(),
                "string href unexpectedly resolved: {href}"
            );
        }
    }

    #[test]
    fn export_environment_contract_matches_font_and_image_owners() {
        for output_id in ["jpeg", "pdf", "png"] {
            let contract = output_environment_contract(output_id)
                .expect("each compiled export must disclose its environment");
            assert_eq!(
                contract.system_fonts.is_some(),
                system_font_discovery_available()
            );
            if let Some(system_fonts) = contract.system_fonts {
                assert_eq!(system_fonts.source_id, "host-system");
                assert_eq!(system_fonts.discovery, "first-use");
                assert_eq!(system_fonts.cache_scope, "process-global");
                assert!(system_fonts.host_dependent);
                assert!(!system_fonts.resource_bounded);
            }
            assert_eq!(contract.embedded_images.source_ids, ["data-url"]);
            assert!(!contract.embedded_images.filesystem_access);
            assert!(!contract.embedded_images.network_access);
            assert_eq!(
                contract.embedded_images.default_limits,
                EmbeddedImageLimit::default()
            );
        }
        assert!(output_environment_contract("svg").is_none());
        assert!(output_environment_contract("ascii").is_none());
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

    fn svg_to_png_with_report(
        svg: &str,
        options: &RasterOptions,
    ) -> Result<(Vec<u8>, RasterExportReport)> {
        super::svg_to_png_with_report(&compatible_svg(svg), options)
    }

    fn svg_to_jpeg(svg: &str, options: &RasterOptions) -> Result<Vec<u8>> {
        super::svg_to_jpeg(&compatible_svg(svg), options)
    }

    fn svg_to_jpeg_with_report(
        svg: &str,
        options: &RasterOptions,
    ) -> Result<(Vec<u8>, RasterExportReport)> {
        super::svg_to_jpeg_with_report(&compatible_svg(svg), options)
    }

    fn svg_to_pdf(svg: &str) -> Result<Vec<u8>> {
        super::svg_to_pdf_with_options(&compatible_svg(svg), &fixture_pdf_options())
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

    fn branching_data_image_use_svg(levels: usize, href: &str) -> String {
        let mut svg =
            String::from(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><defs>"#);
        svg.push_str(&format!(
            r#"<g id="leaf"><image href="{href}" width="1" height="1"/></g>"#
        ));
        for index in 0..levels {
            let target = if index + 1 == levels {
                "leaf".to_owned()
            } else {
                format!("use-{}", index + 1)
            };
            svg.push_str(&format!(
                r##"<g id="use-{index}"><use href="#{target}"/><use href="#{target}"/></g>"##
            ));
        }
        svg.push_str(r##"</defs><use href="#use-0"/></svg>"##);
        svg
    }

    fn svg_to_pdf_with_options(svg: &str, options: &PdfOptions) -> Result<Vec<u8>> {
        super::svg_to_pdf_with_options(&compatible_svg(svg), options)
    }

    fn svg_to_pdf_with_report(
        svg: &str,
        options: &PdfOptions,
    ) -> Result<(Vec<u8>, PdfExportReport)> {
        super::svg_to_pdf_with_report(&compatible_svg(svg), options)
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
        let bytes = svg_to_png(svg, &fixture_raster_options()).unwrap();
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    fn invalid_output_compositing_colors_fail_during_preparation() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"/>"#;

        let raster_error = prepare_raster(
            svg,
            &fixture_raster_options().with_matte("not-a-solid-color"),
        )
        .err()
        .expect("invalid raster matte should fail during preparation");
        let pdf_error = prepare_pdf(
            svg,
            &fixture_pdf_options().with_page_paint("not-a-solid-color"),
        )
        .err()
        .expect("invalid PDF page paint should fail during preparation");

        assert!(matches!(raster_error, ExportError::InvalidRasterMatte));
        assert!(matches!(pdf_error, ExportError::InvalidPdfPagePaint));
    }

    #[test]
    fn png_report_retains_dimensions_and_requested_matte() {
        let source = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="black"/></svg>"#;
        let (bytes, report) =
            svg_to_png_with_report(source, &fixture_raster_options().with_matte("#12345678"))
                .unwrap();

        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert_eq!(report.output(), RasterOutputKind::Png);
        assert_eq!(
            report.matte(),
            Some(ExportRgbaColor {
                red: 0x12,
                green: 0x34,
                blue: 0x56,
                alpha: 0x78,
            })
        );
        assert!(!report.matte_defaulted());
        assert_eq!(
            (report.raster().width_px, report.raster().height_px),
            (10, 10)
        );
    }

    #[test]
    fn jpeg_report_exposes_its_effective_default_or_requested_matte() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8"/>"#;
        let (_, default_report) = svg_to_jpeg_with_report(svg, &fixture_raster_options()).unwrap();
        let (_, requested_report) =
            svg_to_jpeg_with_report(svg, &fixture_raster_options().with_matte("#010203")).unwrap();

        assert_eq!(default_report.output(), RasterOutputKind::Jpeg);
        assert_eq!(default_report.matte(), Some(ExportRgbaColor::WHITE));
        assert!(default_report.matte_defaulted());
        assert_eq!(
            requested_report.matte(),
            Some(ExportRgbaColor {
                red: 1,
                green: 2,
                blue: 3,
                alpha: 255,
            })
        );
        assert!(!requested_report.matte_defaulted());
    }

    #[test]
    fn pdf_report_retains_page_filter_plan_and_page_paint() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 20"><rect width="10" height="20" fill="black"/></svg>"#;
        let options = fixture_pdf_options()
            .with_page_policy(PdfPagePolicy::Fixed {
                width_pt: 612.0,
                height_pt: 792.0,
            })
            .with_page_paint("#01020380");
        let (bytes, report) = svg_to_pdf_with_report(svg, &options).unwrap();

        assert!(bytes.starts_with(b"%PDF-"));
        assert_eq!(report.page().page_width_pt(), 612.0);
        assert_eq!(report.page().page_height_pt(), 792.0);
        assert_eq!(report.filters().filtered_groups, 0);
        assert_eq!(
            report.page_paint(),
            Some(ExportRgbaColor {
                red: 1,
                green: 2,
                blue: 3,
                alpha: 128,
            })
        );
    }

    #[test]
    fn rotated_foreign_object_text_and_background_export_at_the_transformed_position() {
        // This is the same translate/rotate/translate shape used by LR Swimlane titles.
        // A long colored label background provides font-independent pixel evidence.
        let source = r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200" viewBox="0 0 200 200"><g transform="translate(60 100) rotate(-90) translate(-40 -10)"><foreignObject width="80" height="20"><div xmlns="http://www.w3.org/1999/xhtml" style="background-color:#ff0000;color:#ffffff"><p>Support</p></div></foreignObject></g></svg>"##;
        let session = merman_render::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let compatible = merman_render::svg::SvgPipeline::resvg_safe()
            .process_resvg_compatible(source, &session)
            .unwrap();
        let bytes = super::svg_to_png(&compatible, &fixture_raster_options()).unwrap();
        assert_eq!(png_size(&bytes), (200, 200));
        for (x, y) in [(51, 70), (51, 100), (51, 130)] {
            assert_eq!(
                rgba_pixel(&bytes, x, y),
                [255, 0, 0, 255],
                "rotated label strip at ({x},{y})"
            );
        }
        assert_eq!(
            rgba_pixel(&bytes, 100, 100)[3],
            0,
            "label must not extend horizontally into nodes"
        );
        assert_eq!(
            rgba_pixel(&bytes, 51, 50)[3],
            0,
            "label stays inside its transformed header strip"
        );
    }

    #[test]
    fn foreign_object_export_preserves_ancestor_clip_opacity_and_local_background() {
        let source = r##"<svg xmlns="http://www.w3.org/2000/svg" id="diagram" width="100" height="100" viewBox="0 0 100 100"><style>#diagram .node rect { fill:blue!important;stroke:green!important;stroke-width:10!important; }</style><defs><clipPath id="clip"><rect width="25" height="100"/></clipPath></defs><g class="node" opacity=".5" clip-path="url(#clip)"><foreignObject x="10" y="10" width="40" height="20"><div xmlns="http://www.w3.org/1999/xhtml" style="background-color:red;color:white"><p>A</p></div></foreignObject></g></svg>"##;
        let session = merman_render::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let compatible = merman_render::svg::SvgPipeline::resvg_safe()
            .process_resvg_compatible(source, &session)
            .unwrap();
        let bytes = super::svg_to_png(&compatible, &fixture_raster_options()).unwrap();
        assert_eq!(rgba_pixel(&bytes, 12, 12), [255, 0, 0, 128]);
        assert_eq!(
            rgba_pixel(&bytes, 35, 12)[3],
            0,
            "ancestor clipping must apply to the generated label"
        );
        assert_eq!(
            rgba_pixel(&bytes, 8, 12)[3],
            0,
            "node shape CSS must not add a label border"
        );
    }

    #[test]
    fn export_reports_retain_sealed_fallback_css_residual() {
        for (css, expected) in [
            ("", false),
            ("<style>tspan{font-size:64px!important}</style>", true),
        ] {
            let source = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100" viewBox="0 0 200 100">{css}<foreignObject x="10" y="10" width="180" height="60"><div xmlns="http://www.w3.org/1999/xhtml"><p>Alpha <b>Beta</b></p></div></foreignObject></svg>"#
            );
            let session = merman_render::environment::RenderEnvironment::deterministic()
                .begin_session()
                .unwrap();
            let svg = merman_render::svg::SvgPipeline::resvg_safe()
                .process_resvg_compatible(&source, &session)
                .unwrap();
            assert_eq!(svg.finalization_report().fallback_css_residual(), expected);

            let raster = super::prepare_raster(&svg, &fixture_raster_options()).unwrap();
            assert_eq!(
                raster
                    .report_for_output(RasterOutputKind::Png)
                    .fallback_css_residual(),
                expected
            );
            assert_eq!(
                raster
                    .report_for_output(RasterOutputKind::Jpeg)
                    .fallback_css_residual(),
                expected
            );
            let (png, report) = raster.encode_png_with_report().unwrap();
            assert_eq!(report.fallback_css_residual(), expected);
            assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));

            let (jpeg, report) =
                super::svg_to_jpeg_with_report(&svg, &fixture_raster_options()).unwrap();
            assert_eq!(report.fallback_css_residual(), expected);
            assert!(jpeg.starts_with(&[0xff, 0xd8]));

            let pdf = super::prepare_pdf(&svg, &fixture_pdf_options()).unwrap();
            assert_eq!(pdf.report().fallback_css_residual(), expected);
            let (pdf, report) = pdf.encode_with_report().unwrap();
            assert_eq!(report.fallback_css_residual(), expected);
            assert!(pdf.starts_with(b"%PDF-"));
        }
    }

    #[test]
    fn root_viewport_dimensions_drive_raster_planning_and_pixels() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100" viewBox="0 0 10 10"><rect width="10" height="10" fill="black"/></svg>"#;
        let options = fixture_raster_options();
        let plan = svg_raster_plan(svg, &options).unwrap();

        assert_eq!(plan.requested_width_px, 200.0);
        assert_eq!(plan.requested_height_px, 100.0);
        assert_eq!((plan.width_px, plan.height_px), (200, 100));

        let bytes = svg_to_png(svg, &options).unwrap();
        assert_eq!(png_size(&bytes), (200, 100));
        let center = rgba_pixel(&bytes, 100, 50);
        let side = rgba_pixel(&bytes, 10, 50);
        assert!(
            center[0] < 8 && center[1] < 8 && center[2] < 8 && center[3] > 247,
            "expected the viewBox content at the viewport center, got {center:?}"
        );
        assert_eq!(
            side,
            [0, 0, 0, 0],
            "preserveAspectRatio should leave transparent side padding"
        );
    }

    #[test]
    fn explicit_root_size_without_viewbox_preserves_canvas() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect x="20" y="30" width="10" height="10" fill="red"/></svg>"#;
        let options = fixture_raster_options();
        let png = svg_to_png(svg, &options).unwrap();

        assert_eq!(png_size(&png), (100, 100));
        assert_eq!(rgba_pixel(&png, 25, 35), [255, 0, 0, 255]);
        assert_eq!(rgba_pixel(&png, 0, 0), [0, 0, 0, 0]);

        let jpeg = svg_to_jpeg(svg, &options).unwrap();
        let jpeg = image::load_from_memory_with_format(&jpeg, image::ImageFormat::Jpeg).unwrap();
        assert_eq!((jpeg.width(), jpeg.height()), (100, 100));
    }

    #[test]
    fn explicit_root_size_without_viewbox_preserves_filter_pixels() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><defs><filter id="shadow" x="-100%" y="-100%" width="400%" height="300%"><feDropShadow dx="15" dy="0" stdDeviation="0" flood-color="blue"/></filter></defs><rect x="20" y="30" width="10" height="10" fill="red" filter="url(#shadow)"/></svg>"##;
        let png = svg_to_png(svg, &fixture_raster_options()).unwrap();

        assert_eq!(png_size(&png), (100, 100));
        assert_eq!(rgba_pixel(&png, 25, 35), [255, 0, 0, 255]);
        assert_eq!(rgba_pixel(&png, 40, 35), [0, 0, 255, 255]);
    }

    #[test]
    fn dimensionless_root_keeps_content_fit() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100%" style="max-width:400px"><rect x="20" y="30" width="10" height="10" fill="red"/></svg>"#;
        let png = svg_to_png(svg, &fixture_raster_options()).unwrap();

        assert_eq!(png_size(&png), (10, 10));
        assert_eq!(rgba_pixel(&png, 5, 5), [255, 0, 0, 255]);
    }

    #[test]
    fn dimensionless_root_content_fit_preserves_filter_pixels() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100%" style="max-width:400px"><defs><filter id="shadow" filterUnits="userSpaceOnUse" x="10" y="20" width="40" height="30"><feDropShadow dx="15" dy="0" stdDeviation="0" flood-color="blue"/></filter></defs><rect x="20" y="30" width="10" height="10" fill="red" filter="url(#shadow)"/></svg>"##;
        let png = svg_to_png(svg, &fixture_raster_options()).unwrap();

        assert_eq!(png_size(&png), (40, 30));
        assert_eq!(rgba_pixel(&png, 15, 15), [255, 0, 0, 255]);
        assert_eq!(rgba_pixel(&png, 30, 15), [0, 0, 255, 255]);
        assert_eq!(rgba_pixel(&png, 0, 0), [0, 0, 0, 0]);
    }

    #[test]
    fn dimensionless_pdf_reports_content_outside_derived_viewport() {
        for body in [
            r#"<rect x="-10" y="10" width="20" height="20" fill="red"/>"#,
            r##"<defs><filter id="shadow" filterUnits="userSpaceOnUse" x="10" y="20" width="40" height="30"><feDropShadow dx="15" dy="0" stdDeviation="0" flood-color="blue"/></filter></defs><rect x="20" y="30" width="10" height="10" fill="red" filter="url(#shadow)"/>"##,
        ] {
            let source = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="100%" style="max-width:400px">{body}</svg>"#
            );
            let prepared = prepare_pdf(&source, &fixture_pdf_options())
                .expect("best-effort PDF reports a derived-viewport limitation and still exports");
            assert!(prepared.report().implicit_viewport_clipping_residual());
            assert!(prepared.encode().unwrap().starts_with(b"%PDF-"));
            let explicit = source.replace(
                r#"width="100%" style="max-width:400px""#,
                r#"width="100" height="100""#,
            );
            let prepared = prepare_pdf(&explicit, &fixture_pdf_options())
                .expect("a caller's explicit viewport keeps intentional source clipping");
            assert!(!prepared.report().implicit_viewport_clipping_residual());
        }
    }

    #[test]
    fn dimensionless_pdf_preserves_visible_info_style_text() {
        let source = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100%" style="max-width:400px"><text x="100" y="40" font-size="32" text-anchor="middle" fill="#333">v0.9.0</text></svg>"##;
        let prepared = prepare_pdf(source, &fixture_pdf_options())
            .expect("ordinary dimensionless Info text stays inside its derived viewport");
        let page = prepared.report().page();
        assert!(page.page_width_pt > 0.0 && page.page_height_pt > 0.0);
        assert!(!prepared.report().implicit_viewport_clipping_residual());
        let pdf = prepared.encode().unwrap();
        let pdf = String::from_utf8_lossy(&pdf);
        assert!(
            pdf.contains("/Type/Font"),
            "Info must contain actual native text"
        );
    }

    #[test]
    fn raster_directional_filters_report_transform_residual_and_encode() {
        for transform in ["rotate(90)", "skewX(30)", "scale(-1 1)"] {
            for primitive in [
                r#"<feDropShadow dx="7" dy="4" stdDeviation="0" flood-color="blue"/>"#,
                r#"<feOffset dx="7" dy="4"/>"#,
            ] {
                let svg = format!(
                    r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><defs><filter id="effect" x="-100%" y="-100%" width="400%" height="400%">{primitive}</filter></defs><g transform="translate(50 50) {transform}"><rect width="10" height="20" fill="red" filter="url(#effect)"/></g></svg>"##
                );
                let prepared = prepare_raster(&svg, &fixture_raster_options()).expect(
                    "best-effort raster export still renders directional filter transforms",
                );
                assert!(
                    prepared
                        .report_for_output(RasterOutputKind::Png)
                        .directional_filter_transform_residual()
                );
                assert!(
                    prepared
                        .report_for_output(RasterOutputKind::Jpeg)
                        .directional_filter_transform_residual()
                );
                let (png, report) = prepared.encode_png_with_report().unwrap();
                assert!(report.directional_filter_transform_residual());
                assert_png_has_visible_non_background_ink(&png);
                let (jpeg, report) =
                    svg_to_jpeg_with_report(&svg, &fixture_raster_options()).unwrap();
                assert!(report.directional_filter_transform_residual());
                assert!(jpeg.starts_with(&[0xff, 0xd8]));
            }
        }
    }

    #[test]
    fn raster_nondirectional_filters_keep_transformed_content() {
        for primitive in [
            r#"<feGaussianBlur stdDeviation="1"/>"#,
            r#"<feDropShadow dx="0" dy="0" stdDeviation="1" flood-color="blue"/>"#,
            r#"<feOffset dx="0" dy="0"/>"#,
        ] {
            let svg = format!(
                r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><defs><filter id="effect" x="-100%" y="-100%" width="400%" height="400%">{primitive}</filter></defs><g transform="translate(50 20) rotate(90)"><rect width="10" height="20" fill="red" filter="url(#effect)"/></g></svg>"##
            );
            let (png, report) = svg_to_png_with_report(&svg, &fixture_raster_options()).unwrap();
            assert!(!report.directional_filter_transform_residual());
            assert!(rgba_pixel(&png, 40, 25)[3] > 200, "{primitive}");
        }
    }

    #[test]
    fn directional_filters_keep_axis_aligned_raster_and_localized_pdf_support() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><defs><filter id="effect" primitiveUnits="objectBoundingBox" x="-100%" y="-100%" width="400%" height="400%"><feDropShadow dx=".5" dy=".5" stdDeviation="0" flood-color="blue"/></filter></defs><g transform="translate(20 20) scale(2)"><rect width="10" height="10" fill="red" filter="url(#effect)"/></g></svg>"##;
        let (png, report) = svg_to_png_with_report(svg, &fixture_raster_options()).unwrap();
        assert!(!report.directional_filter_transform_residual());
        assert_eq!(rgba_pixel(&png, 45, 45), [0, 0, 255, 255]);

        let rotated = svg.replace("translate(20 20) scale(2)", "translate(50 20) rotate(90)");
        let pdf = svg_to_pdf(&rotated).unwrap();
        assert!(String::from_utf8_lossy(&pdf).contains("/Subtype/Image"));
    }

    #[test]
    fn pdf_directional_filters_report_local_transform_residual_and_encode() {
        for transform in ["rotate(90)", "skewX(30)", "scale(-1 1)"] {
            for primitive in [
                r#"<feDropShadow dx="15" dy="0" stdDeviation="0" flood-color="blue"/>"#,
                r#"<feOffset dx="15" dy="0"/>"#,
            ] {
                let svg = format!(
                    r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><defs><filter id="effect" filterUnits="userSpaceOnUse" x="-20" y="-20" width="80" height="80">{primitive}</filter></defs><g transform="translate(60 20) {transform}" filter="url(#effect)"><rect width="20" height="10" fill="red"/></g></svg>"##
                );
                let prepared = prepare_pdf(&svg, &fixture_pdf_options()).expect(
                    "best-effort PDF still exports localized directional filter transforms",
                );
                assert!(prepared.report().directional_filter_transform_residual());
                let (pdf, report) = prepared.encode_with_report().unwrap();
                assert!(report.directional_filter_transform_residual());
                assert!(String::from_utf8_lossy(&pdf).contains("/Subtype/Image"));
            }
        }
    }

    #[test]
    fn pdf_directional_filters_allow_ancestor_transforms() {
        for transform in ["rotate(90)", "skewX(30)", "scale(-1 1)"] {
            let svg = format!(
                r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><defs><filter id="effect" filterUnits="userSpaceOnUse" x="-20" y="-20" width="80" height="80"><feDropShadow dx="15" dy="0" stdDeviation="0" flood-color="blue"/></filter></defs><g transform="translate(60 20) {transform}"><g filter="url(#effect)"><rect width="20" height="10" fill="red"/></g></g></svg>"##
            );
            let prepared = prepare_pdf(&svg, &fixture_pdf_options())
                .expect("vector ancestor transforms remain supported outside the filter image");
            let plan = prepared.filter_plan();
            assert_eq!(plan.filtered_groups, 1);
            assert!(plan.effective_image_pixels > 0, "{transform}: {plan:?}");
            assert!(!plan.limited, "{transform}: {plan:?}");
            assert!(!prepared.report().directional_filter_transform_residual());
            let pdf = prepared.encode().unwrap();
            assert!(pdf.starts_with(b"%PDF-"));
        }
    }

    #[test]
    fn pdf_nondirectional_filters_keep_local_transforms() {
        for primitive in [
            r#"<feGaussianBlur stdDeviation="1"/>"#,
            r#"<feDropShadow dx="0" dy="0" stdDeviation="1" flood-color="blue"/>"#,
            r#"<feOffset dx="0" dy="0"/>"#,
        ] {
            let svg = format!(
                r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><defs><filter id="effect" filterUnits="userSpaceOnUse" x="-20" y="-20" width="80" height="80">{primitive}</filter></defs><g transform="translate(60 20) rotate(90)" filter="url(#effect)"><rect width="20" height="10" fill="red"/></g></svg>"##
            );
            let prepared = prepare_pdf(&svg, &fixture_pdf_options()).unwrap();
            assert_eq!(prepared.filter_plan().filtered_groups, 1);
            assert!(prepared.filter_plan().effective_image_pixels > 0);
            assert!(!prepared.report().directional_filter_transform_residual());
            assert!(prepared.encode().unwrap().starts_with(b"%PDF-"));
        }
    }

    #[test]
    fn pdf_directional_filters_report_nested_local_transform_residual() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><defs><filter id="outer"><feGaussianBlur stdDeviation="0"/></filter><filter id="shadow" filterUnits="userSpaceOnUse" x="-20" y="-20" width="80" height="80"><feDropShadow dx="15" dy="0" stdDeviation="0" flood-color="blue"/></filter></defs><g filter="url(#outer)"><g transform="translate(60 20) rotate(90)" filter="url(#shadow)"><rect width="20" height="10" fill="red"/></g></g></svg>"##;
        let prepared = prepare_pdf(svg, &fixture_pdf_options())
            .expect("descendant transform limitations must not disable best-effort PDF output");
        assert!(prepared.report().directional_filter_transform_residual());
        let (pdf, report) = prepared.encode_with_report().unwrap();
        assert!(report.directional_filter_transform_residual());
        assert!(String::from_utf8_lossy(&pdf).contains("/Subtype/Image"));
    }

    #[test]
    fn ignored_and_later_viewbox_aliases_do_not_change_usvg_geometry() {
        for svg in [
            r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:i="urn:ignored" i:viewBox="0 0 9000 9000"><rect width="12" height="8" fill="black"/></svg>"#,
            r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:s="http://www.w3.org/2000/svg" s:viewBox="invalid" viewBox="0 0 9000 9000"><rect width="12" height="8" fill="black"/></svg>"#,
        ] {
            let plan = svg_raster_plan(svg, &fixture_raster_options()).unwrap();
            assert_eq!(
                (plan.width_px, plan.height_px),
                (12, 8),
                "metadata and usvg must agree on the effective viewBox: {svg}"
            );
        }
    }

    #[test]
    fn non_positive_root_dimensions_cannot_cross_the_sealed_raster_boundary() {
        let session = merman_render::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();

        for svg in [
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="0" height="100" viewBox="0 0 10 10"/>"#,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="-1" viewBox="0 0 10 10"/>"#,
        ] {
            let error = merman_render::svg::finalize_resvg_svg(svg, &session)
                .expect_err("invalid root dimensions must fail before raster preparation");
            assert!(
                error.to_string().contains("must be a positive length"),
                "{error}"
            );
        }
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn scheduling_weights_include_the_recursive_backend_stack() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10"/></svg>"#;
        let raster = prepare_raster(svg, &fixture_raster_options()).unwrap();
        let pdf = prepare_pdf(svg, &fixture_pdf_options()).unwrap();

        assert!(raster.png_scheduling_weight_bytes() >= RECURSIVE_SVG_BACKEND_STACK_BYTES as u64);
        assert!(pdf.scheduling_weight_bytes() >= RECURSIVE_SVG_BACKEND_STACK_BYTES as u64);
    }

    #[test]
    fn trusted_resvg_backend_handles_the_declared_tree_depth() {
        let depth = merman_render::resources::MAX_RESVG_TREE_DEPTH;
        let svg = trusted_compatible_svg(&nested_group_svg(depth));
        let options =
            fixture_raster_options().with_conversion_limits(SvgConversionLimits::unbounded());
        let prepared = super::prepare_raster(&svg, &options).unwrap();
        assert_eq!(prepared.report.conversion.max_tree_depth, depth - 1);

        let bytes = prepared.encode_png().unwrap();
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    fn trusted_krilla_backend_handles_the_declared_tree_depth() {
        let depth = merman_render::resources::MAX_RESVG_TREE_DEPTH;
        let svg = trusted_compatible_svg(&nested_group_svg(depth));
        let options =
            fixture_pdf_options().with_conversion_limits(SvgConversionLimits::unbounded());
        let prepared = super::prepare_pdf(&svg, &options).unwrap();
        assert_eq!(prepared.report().conversion().max_tree_depth, depth - 1);

        let bytes = prepared.encode().unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
    }

    #[test]
    fn expanded_use_tree_is_rejected_before_usvg_parsing() {
        let session = merman_render::environment::RenderEnvironment::deterministic()
            .with_resource_policy(merman_render::resources::RenderResourcePolicy::trusted_native())
            .begin_session()
            .unwrap();
        let error = merman_render::svg::finalize_resvg_svg(
            &expanded_use_chain_svg(merman_render::resources::MAX_RESVG_TREE_DEPTH + 2),
            &session,
        )
        .expect_err("the reference preflight must reject the expanded usvg depth");

        assert!(
            error
                .to_string()
                .contains(merman_render::resources::SVG_BACKEND_TREE_DEPTH_HARD_CAP_ID),
            "{error}"
        );
    }

    #[test]
    fn expanded_use_data_urls_count_toward_the_aggregate_before_usvg_parsing() {
        let href = png_data_uri_with_declared_size(1, 1);
        let svg = compatible_svg(&branching_data_image_use_svg(4, &href));
        let one_source_resource =
            plan_embedded_data_resources(svg.as_str(), EmbeddedImageLimit::unbounded())
                .unwrap()
                .total_bytes;
        let options = RasterOptions {
            embedded_image_limit: EmbeddedImageLimit::new(
                Some(one_source_resource),
                Some(one_source_resource),
                None,
                None,
            ),
            ..fixture_raster_options()
        };

        let error = super::prepare_raster(&svg, &options)
            .err()
            .expect("expanded data URLs must exceed the aggregate preflight budget");

        assert!(error.to_string().contains("max_total_bytes"), "{error}");
    }

    #[test]
    fn filter_and_marker_subroots_count_repeated_data_urls_before_usvg_parsing() {
        let href = png_data_uri_with_declared_size(1, 1);
        let cases = [
            format!(
                r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><defs><image id="source" href="{href}" width="1" height="1"/><filter id="f"><feImage href="#source"/></filter></defs><rect width="10" height="10" filter="url(#f)"/></svg>"##
            ),
            format!(
                r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><defs><marker id="m"><image href="{href}" width="1" height="1"/></marker></defs><path d="M0 0L1 1L2 2L3 3" marker-mid="url(#m)"/></svg>"##
            ),
        ];

        for raw in cases {
            let one_source_resource =
                plan_embedded_data_resources(&raw, EmbeddedImageLimit::unbounded())
                    .unwrap()
                    .total_bytes;
            let svg = compatible_svg(&raw);
            let options = RasterOptions {
                embedded_image_limit: EmbeddedImageLimit::new(
                    Some(one_source_resource),
                    Some(one_source_resource),
                    None,
                    None,
                ),
                ..fixture_raster_options()
            };

            let error = super::prepare_raster(&svg, &options)
                .err()
                .expect("repeated filter or marker data URLs must exceed the preflight budget");
            assert!(error.to_string().contains("max_total_bytes"), "{error}");
        }
    }

    #[test]
    fn css_effect_fanout_counts_filter_and_mask_data_urls_before_usvg_parsing() {
        let href = png_data_uri_with_declared_size(1, 1);
        let one_resource = plan_embedded_data_resources(
            &format!(r#"<svg xmlns="http://www.w3.org/2000/svg"><image href="{href}"/></svg>"#),
            EmbeddedImageLimit::unbounded(),
        )
        .unwrap()
        .total_bytes;
        let raw = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><defs><filter id="f"><feImage href="{href}"/></filter><mask id="m"><image href="{href}" width="1" height="1"/></mask></defs><style>.affected {{ filter: url(#f); mask: url(#m); }}</style><rect class="affected" width="1" height="1"/><rect class="affected" x="2" width="1" height="1"/><rect class="affected" x="4" width="1" height="1"/></svg>"##
        );
        let svg = compatible_svg(&raw);
        let options = RasterOptions {
            embedded_image_limit: EmbeddedImageLimit::new(
                Some(one_resource),
                Some(one_resource.saturating_mul(2)),
                None,
                None,
            ),
            ..fixture_raster_options()
        };

        let error = super::prepare_raster(&svg, &options)
            .err()
            .expect("CSS effect fanout must exceed the aggregate preflight budget");

        assert!(error.to_string().contains("max_total_bytes"), "{error}");
    }

    #[test]
    fn svg_to_png_does_not_load_local_image_hrefs() {
        let local_image = TempFile::new("png", encode_rgba_png(1, 1, &[255, 0, 0, 255]));
        let href = escape_xml_attr(&local_image.href_path());
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="white"/><image href="{href}" width="10" height="10"/></svg>"#
        );

        let bytes = svg_to_png(&svg, &fixture_raster_options()).unwrap();
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

        let png_err = prepare_raster(&svg, &fixture_raster_options())
            .err()
            .expect("PNG preparation should reject the decoded image size");
        let pdf_err = prepare_pdf(&svg, &fixture_pdf_options())
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
        let prepared = prepare_raster(&svg, &fixture_raster_options()).unwrap();

        assert_eq!(
            prepared.report.embedded_images,
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
            ..fixture_raster_options()
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
            ..fixture_raster_options()
        };
        let pdf_options = PdfOptions {
            embedded_image_limit: EmbeddedImageLimit::new(Some(16), None, None, None),
            ..fixture_pdf_options()
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

        let png_error = prepare_raster(&svg, &fixture_raster_options())
            .err()
            .expect("filter image intrinsic pixels should be bounded for raster output");
        let pdf_error = prepare_pdf(&svg, &fixture_pdf_options())
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
            ..fixture_raster_options()
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
            &fixture_pdf_options()
                .with_page_paint("white")
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
            &fixture_pdf_options().with_page_policy(PdfPagePolicy::Fixed {
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
            &fixture_pdf_options().with_page_policy(PdfPagePolicy::FitCssWidth {
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
            &fixture_pdf_options().with_page_policy(PdfPagePolicy::Fixed {
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
        let err = svg_to_pdf_with_options(svg, &fixture_pdf_options().with_filter_scale(0.0))
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
        let prepared = prepare_pdf(svg, &fixture_pdf_options()).unwrap();
        let plan = prepared.report().filters();

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
            prepare_pdf(svg, &fixture_pdf_options().with_unbounded_filter_images()).unwrap();
        let plan = prepared.report().filters();

        assert_eq!(plan.requested_image_pixels, 50_000_000);
        assert_eq!(plan.effective_image_pixels, 50_000_000);
        assert!(!plan.limited, "{plan:?}");
    }

    const TWO_FILTERED_SQUARES: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><defs><filter id="identity"><feGaussianBlur stdDeviation="0"/></filter></defs><rect x="10" y="10" width="20" height="20" fill="red" filter="url(#identity)"/><rect x="50" y="10" width="20" height="20" fill="blue" filter="url(#identity)"/></svg>"##;

    #[test]
    fn pdf_filter_budget_rejects_zero_dimension_images() {
        let error = prepare_pdf(
            TWO_FILTERED_SQUARES,
            &fixture_pdf_options().with_filter_image_limit(PdfFilterImageLimit::new(Some(1))),
        )
        .err()
        .expect("two filtered groups cannot be represented by one retained pixel");

        let details = error
            .resource_limit_details()
            .expect("typed filter budget failure");
        assert_eq!(
            details.limit_id,
            MAX_PDF_FILTER_IMAGE_PIXELS_RESOURCE_LIMIT_ID
        );
        assert_eq!(details.phase, "pdf_filter_rasterization");
        assert!(details.actual >= 2, "{details:?}");
        assert_eq!(details.max, 1);
    }

    #[test]
    fn pdf_filter_cap_rejects_zero_dimension_images() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><defs><filter id="identity" filterUnits="userSpaceOnUse" x="0" y="0" width="1000000" height="1"><feGaussianBlur stdDeviation="0"/></filter></defs><rect width="1000000" height="1" fill="red" filter="url(#identity)"/></svg>"##;
        let error = prepare_pdf(svg, &fixture_pdf_options())
            .err()
            .expect("the backend's side cap must not silently erase a thin filtered group");

        assert!(matches!(error, ExportError::InvalidSizing(_)), "{error}");
    }

    #[test]
    fn pdf_filter_small_requested_scale_rejects_zero_dimension_images() {
        let error = prepare_pdf(
            TWO_FILTERED_SQUARES,
            &fixture_pdf_options().with_filter_scale(0.001),
        )
        .err()
        .expect("a requested scale that erases every filter image must fail preparation");

        assert!(matches!(error, ExportError::InvalidSizing(_)), "{error}");
    }

    #[test]
    fn pdf_filter_budget_retains_nonzero_images_when_feasible() {
        let prepared = prepare_pdf(
            TWO_FILTERED_SQUARES,
            &fixture_pdf_options().with_filter_image_limit(PdfFilterImageLimit::new(Some(2))),
        )
        .unwrap();
        let plan = prepared.filter_plan();
        assert_eq!(plan.filtered_groups, 2);
        assert_eq!(plan.effective_image_pixels, 2);

        let pdf = String::from_utf8_lossy(&prepared.encode().unwrap()).into_owned();
        assert!(
            pdf.contains("/Subtype/Image"),
            "filtered content must survive encoding"
        );
    }

    #[test]
    fn pdf_filter_budget_includes_mask_and_pattern_subroots() {
        for body in [
            r##"<defs><filter id="identity"><feGaussianBlur stdDeviation="0"/></filter><mask id="mask"><rect width="80" height="80" fill="white" filter="url(#identity)"/></mask></defs><rect width="80" height="80" fill="red" mask="url(#mask)"/>"##,
            r##"<defs><filter id="identity"><feGaussianBlur stdDeviation="0"/></filter><pattern id="pattern" patternUnits="userSpaceOnUse" width="80" height="80"><rect width="80" height="80" fill="red" filter="url(#identity)"/></pattern></defs><rect width="80" height="80" fill="url(#pattern)"/>"##,
        ] {
            let svg = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100">{body}</svg>"#
            );
            let prepared = prepare_pdf(
                &svg,
                &fixture_pdf_options().with_filter_image_limit(PdfFilterImageLimit::new(Some(1))),
            )
            .unwrap();
            let plan = prepared.filter_plan();
            assert_eq!(plan.filtered_groups, 1, "{body}");
            assert!(plan.requested_image_pixels > 1, "{plan:?}");
            assert_eq!(plan.effective_image_pixels, 1);
            assert!(plan.limited);
            let pdf = prepared.encode().unwrap();
            assert!(String::from_utf8_lossy(&pdf).contains("/Subtype/Image"));
        }
    }

    #[test]
    fn pdf_filter_plan_matches_isolated_content_stream_scale() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><defs><filter id="identity"><feGaussianBlur stdDeviation="0"/></filter></defs><g transform="scale(5)" opacity=".5"><rect width="16" height="16" fill="red" filter="url(#identity)"/></g></svg>"##;
        let prepared = prepare_pdf(svg, &fixture_pdf_options()).unwrap();
        let plan = prepared.filter_plan();
        // 16px * the default 1.2 filter region * 4 sampling = 76.8, rounded to 77.
        // The opacity content stream is isolated before the parent's scale is reapplied.
        assert_eq!(plan.requested_image_pixels, 77 * 77);
        let pdf = prepared.encode().unwrap();
        let pdf = String::from_utf8_lossy(&pdf);
        assert!(
            pdf.contains("/Width 77") || pdf.contains("/Width77"),
            "{pdf}"
        );
        assert!(
            pdf.contains("/Height 77") || pdf.contains("/Height77"),
            "{pdf}"
        );
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

        let error = prepare_pdf(&svg, &fixture_pdf_options())
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

        let error = prepare_raster(&svg, &fixture_raster_options())
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

        let prepared = prepare_pdf(svg, &fixture_pdf_options()).unwrap();
        let report = prepared.report();

        assert_eq!(report.filters().filtered_groups, 1);
        assert_eq!(report.conversion().filtered_groups, 2);
        assert_eq!(report.conversion().filter_primitives, 2);
    }

    #[test]
    fn svg_to_jpeg_defaults_to_white_background() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8"></svg>"#;
        let bytes = svg_to_jpeg(svg, &fixture_raster_options()).unwrap();
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
        let err = svg_to_jpeg(svg, &fixture_raster_options().with_unbounded_size()).unwrap_err();

        assert!(matches!(err, ExportError::JpegDimensionLimit));
    }

    #[test]
    fn svg_to_png_keeps_text_visible_when_requested_font_is_missing() {
        let svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="100%" style="max-width: 400px; background-color: white;"><text x="100" y="40" fill="#333333" font-size="32" style="font-family: '__merman_missing_font__'; text-anchor: middle;">v{}</text></svg>"##,
            merman_model::baseline::PINNED_MERMAID_BASELINE_VERSION
        );
        let bytes = svg_to_png(&svg, &fixture_raster_options()).unwrap();
        assert_png_has_visible_non_background_ink(&bytes);
    }

    #[test]
    fn default_plan_downscales_large_intrinsic_svg_without_allocating() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 14544.4375 6565.5"><rect width="14544.4375" height="6565.5" fill="white"/></svg>"#;
        let plan = svg_raster_plan(svg, &fixture_raster_options()).unwrap();

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
        let options = fixture_raster_options()
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
        let options = fixture_raster_options()
            .with_size_limit(RasterSizeLimit::max_side_length(128))
            .with_matte("white");
        let bytes = svg_to_png(svg, &options).unwrap();
        let (width, height) = png_size(&bytes);

        assert_eq!((width, height), (128, 64));
    }

    #[test]
    fn size_limit_caps_by_total_pixels() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1000 1000"><rect width="1000" height="1000" fill="black"/></svg>"#;
        let options = fixture_raster_options().with_size_limit(RasterSizeLimit::new(
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
        let plan = svg_raster_plan(svg, &fixture_raster_options().with_unbounded_size()).unwrap();

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

#[cfg(all(test, any(feature = "png", feature = "jpeg", feature = "pdf")))]
mod id_suffix_consumer_tests {
    fn path_fill(svg: &str, id: &str) -> (usvg::Paint, f32) {
        let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).unwrap();
        let usvg::Node::Path(path) = tree.node_by_id(id).unwrap() else {
            panic!("expected path")
        };
        let fill = path.fill().unwrap();
        (fill.paint().clone(), fill.opacity().get())
    }

    #[test]
    fn resvg_safe_retains_hollow_markers_and_selector_specificity() {
        let session = merman_render::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" id="diagram" width="100" height="100"><style>
#diagram [id$="-aggregationEnd"],#diagram .aggregation{fill:transparent!important;stroke:black}
#diagram [id$="-extensionStart"]{fill:transparent!important}
#diagram [id$="-compositionEnd"]{fill:black!important}
#diagram [id$="-arrowhead"] path{fill:red!important}
#diagram [id$="-barbEnd"]{fill:blue!important}
#diagram [id$="-customEnd"]{fill:red!important}
#diagram .override{fill:blue!important}
</style><path id="diagram-aggregationEnd" class="aggregation" d="M0 0H10V10Z"/>
<path id="diagram-aggregationEnd-margin" class="aggregation" d="M20 0H30V10Z"/>
<path id="diagram-extensionStart" d="M0 20H10V30Z"/>
<path id="diagram-compositionEnd" d="M20 20H30V30Z"/>
<g id="diagram-arrowhead"><path id="arrow-paint" d="M40 0H50V10Z"/></g>
<path id="diagram-barbEnd" d="M40 20H50V30Z"/>
<path id="diagram-customEnd" class="override" d="M60 20H70V30Z"/>
</svg>"##;
        // Prove the transformed SVG repairs inheritance and suffix-only rules
        // while retaining the later class override at equal specificity.
        let safe = merman_render::svg::finalize_resvg_svg(svg, &session).unwrap();
        for id in [
            "diagram-aggregationEnd",
            "diagram-aggregationEnd-margin",
            "diagram-extensionStart",
        ] {
            assert_eq!(path_fill(safe.as_str(), id).1, 0.0, "{id}");
        }
        assert_eq!(path_fill(safe.as_str(), "diagram-compositionEnd").1, 1.0);
        for (id, color) in [
            ("arrow-paint", usvg::Color::new_rgb(255, 0, 0)),
            ("diagram-barbEnd", usvg::Color::new_rgb(0, 0, 255)),
            ("diagram-customEnd", usvg::Color::new_rgb(0, 0, 255)),
        ] {
            assert_eq!(
                path_fill(safe.as_str(), id),
                (usvg::Paint::Color(color), 1.0),
                "{id}"
            );
        }
        assert_eq!(
            merman_render::svg::finalize_resvg_svg(safe.as_str(), &session).unwrap(),
            safe
        );
    }
}
