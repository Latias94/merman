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
            #[cfg(feature = "native-fonts")]
            Self::NativeFont(error) => {
                let (id, actual, max) = error.resource_limit()?;
                (
                    id,
                    "native-font-export",
                    actual,
                    max,
                    ExportResourceLimitCause::Ceiling,
                )
            }
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
    let previous = control.terminal_checkpoint_at(OperationPhase::Export).err();
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
                previous.unwrap_or_else(|| control.terminate_resource_limit(error.clone())),
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
                previous.unwrap_or_else(|| {
                    control.terminate_resource_overflow(
                        details.limit_id,
                        OperationPhase::Export,
                        details.phase,
                        details.actual,
                        details.max,
                        provenance,
                    )
                }),
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
    T: Send,
    F: FnOnce(&OperationControl) -> Result<T> + Send,
{
    export_checkpoint(control)?;
    let worker_control = control.clone();
    std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .name("merman-svg-backend".to_string())
            .stack_size(RECURSIVE_SVG_BACKEND_STACK_BYTES)
            .spawn_scoped(scope, move || {
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
    })
}

#[cfg(all(
    any(feature = "png", feature = "jpeg", feature = "pdf"),
    target_arch = "wasm32"
))]
fn run_recursive_svg_backend<T, F>(control: &OperationControl, job: F) -> Result<T>
where
    T: Send,
    F: FnOnce(&OperationControl) -> Result<T> + Send,
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
    export_checkpoint(&control)?;
    let backend_control = control.clone();
    run_recursive_svg_backend(&control, move |control| {
        prepare_raster_on_backend_stack(svg, options, control).map(|mut prepared| {
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
    let data_plan = plan_embedded_data_resource_bytes(
        svg.inline_raster_sources(),
        options.embedded_image_limit,
        control,
    )?;
    export_checkpoint(control)?;
    let tree = usvg::Tree::from_str(source, &usvg_options).map_err(|_| ExportError::SvgParse);
    #[cfg(feature = "native-fonts")]
    if let Some(context) = &options.font_context {
        context.check()?;
    }
    export_checkpoint(control)?;
    let tree = tree?;
    let (geometry, translate_min_to_origin) = raster_geometry_for_svg(root_metadata, &tree);
    let font_plan = font_plan.finish_with_tree(&tree, control)?;
    #[cfg(feature = "native-fonts")]
    if let Some(context) = &options.font_context {
        check_export_glyph_coverage(context, font_plan)?;
    }
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
    let backend_control = control.clone();
    run_recursive_svg_backend(&control, move |control| {
        prepare_pdf_on_backend_stack(svg, options, control).map(|mut prepared| {
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
    let data_plan = plan_embedded_data_resource_bytes(
        svg.inline_raster_sources(),
        options.embedded_image_limit,
        control,
    )?;
    let (tree, font_plan) = parse_pdf_tree(svg, options, control)?;
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
    #[cfg(feature = "native-fonts")]
    if let Some(context) = &options.font_context {
        context.check()?;
    }
    export_checkpoint(control)?;
    let tree = tree?;
    let font_plan = font_plan.finish_with_tree(&tree, control)?;
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
fn directional_filter_transforms_supported(
    root: &usvg::Group,
    control: &OperationControl,
) -> Result<bool> {
    let mut stack = vec![(root, usvg::Transform::identity())];
    while let Some((group, parent_transform)) = stack.pop() {
        export_checkpoint(control)?;
        let transform = parent_transform.pre_concat(group.transform());
        // resvg factors rotation and reflection out of the filter layer and restores them
        // during compositing, including when Krilla renders a group into a localized image.
        let keeps_axes = transform.sx != 0.0
            && transform.sy != 0.0
            && transform.ky.abs() <= transform.sx.abs() * f32::EPSILON
            && transform.kx.abs() <= transform.sy.abs() * f32::EPSILON;
        let swaps_axes = transform.ky != 0.0
            && transform.kx != 0.0
            && transform.sx.abs() <= transform.ky.abs() * f32::EPSILON
            && transform.sy.abs() <= transform.kx.abs() * f32::EPSILON;
        if !(keeps_axes || swaps_axes) {
            for primitive in group
                .filters()
                .iter()
                .flat_map(|filter| filter.primitives())
            {
                export_checkpoint(control)?;
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
            // Krilla skips the entire subtree if it cannot localize a layer.
            let Some(bbox) = group.layer_bounding_box() else {
                continue;
            };
            if group.abs_layer_bounding_box().is_none() {
                continue;
            }
            let directional_transform_residual =
                !directional_filter_transforms_supported(group, control)?;
            // Match krilla-svg's composed transform before its f32 side cap and pixel rounding.
            // Absolute bounds are insufficient: isolated PDF streams reset their current CTM.
            let local = bbox
                .transform(group.transform())
                .ok_or(ExportError::SvgDocSize)?;
            let actual = bbox
                .transform(parent_transform.pre_concat(group.transform()))
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

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn plan_embedded_data_resource_bytes<'a>(
    sources: impl Iterator<Item = (&'a str, usize)>,
    limit: EmbeddedImageLimit,
    control: &OperationControl,
) -> Result<EmbeddedDataPlan> {
    validate_embedded_image_limit(limit)?;
    let mut plan = EmbeddedDataPlan::default();
    for (href, occurrences) in sources {
        export_checkpoint(control)?;
        let data_url = data_url::DataUrl::process(href).map_err(|_| ExportError::SvgParse)?;
        let mut resource_bytes = 0_u64;
        let mut limit_error = None;
        let _ = data_url.decode(|chunk| {
            if let Err(error) = export_checkpoint(control) {
                limit_error = Some(error);
                return Err(());
            }
            resource_bytes = resource_bytes.saturating_add(chunk.len() as u64);
            let aggregate = plan
                .total_bytes
                .saturating_add(resource_bytes.saturating_mul(occurrences as u64));
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

        plan.resources = plan.resources.saturating_add(occurrences);
        plan.largest_bytes = plan.largest_bytes.max(resource_bytes);
        plan.total_bytes = plan
            .total_bytes
            .saturating_add(resource_bytes.saturating_mul(occurrences as u64));
    }
    export_checkpoint(control)?;
    Ok(plan)
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
    let Some(bbox) = tree.root().abs_layer_bounding_box() else {
        // Preserve the minimum raster canvas for an empty SVG without an explicit viewport.
        return (
            RasterGeometry {
                min_x: 0.0,
                min_y: 0.0,
                width: 1.0,
                height: 1.0,
            },
            false,
        );
    };
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
#[path = "tests/font_resolver.rs"]
mod font_resolver_tests;

#[cfg(all(test, any(feature = "png", feature = "jpeg", feature = "pdf")))]
#[path = "tests/fixtures.rs"]
mod test_fixtures;

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

#[cfg(all(test, any(feature = "png", feature = "jpeg")))]
#[path = "tests/root_svg_metadata.rs"]
mod root_svg_metadata_tests;

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
