//! Export diagnostics, terminal resource state, and backend execution.

use merman_model::{
    OperationCancelled, OperationControl, OperationLedgerError, OperationPhase,
    OperationResourceDomain, OperationResourceLimitExceeded, OperationResourceProvenance,
};

#[cfg(feature = "pdf")]
use crate::MAX_PDF_FILTER_IMAGE_PIXELS_RESOURCE_LIMIT_ID;
#[cfg(feature = "native-fonts")]
use crate::NativeFontError;
use crate::{
    MAX_EMBEDDED_IMAGE_BYTES_RESOURCE_LIMIT_ID, MAX_EMBEDDED_IMAGE_PIXELS_RESOURCE_LIMIT_ID,
    MAX_NESTED_SVG_IMAGES_RESOURCE_LIMIT_ID,
    MAX_SVG_CONVERSION_FILTER_PRIMITIVES_PER_FILTER_RESOURCE_LIMIT_ID,
    MAX_SVG_CONVERSION_ISOLATION_DEPTH_RESOURCE_LIMIT_ID,
    MAX_SVG_CONVERSION_SUBROOTS_RESOURCE_LIMIT_ID,
    MAX_TOTAL_EMBEDDED_IMAGE_BYTES_RESOURCE_LIMIT_ID,
    MAX_TOTAL_EMBEDDED_IMAGE_PIXELS_RESOURCE_LIMIT_ID,
    MAX_TOTAL_SVG_CONVERSION_FILTER_PRIMITIVES_RESOURCE_LIMIT_ID,
};

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExportResourceLimitCause {
    Ceiling,
    ArithmeticOverflow,
}

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

pub type Result<T> = std::result::Result<T, ExportError>;

pub(super) fn export_checkpoint(control: &OperationControl) -> Result<()> {
    control
        .terminal_checkpoint_at(OperationPhase::Export)
        .map_err(export_terminal_error)
}

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

fn settle_export_result<T>(control: &OperationControl, result: Result<T>) -> Result<T> {
    let result = result.map_err(|error| terminate_export_resource_error(control, error));
    observe_after_export_result(control, result)
}

fn observe_after_export_result<T>(control: &OperationControl, result: Result<T>) -> Result<T> {
    match result {
        Err(error) if error.resource_limit_details().is_some() => Err(error),
        result => {
            export_checkpoint(control)?;
            result
        }
    }
}

fn export_terminal_error(error: OperationLedgerError) -> ExportError {
    match error {
        OperationLedgerError::Cancelled(error) => ExportError::Cancelled(error),
        terminal @ (OperationLedgerError::ResourceLimitExceeded(_)
        | OperationLedgerError::ArithmeticOverflow { .. }) => {
            ExportError::OperationResourceTerminal(terminal)
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) const RECURSIVE_SVG_BACKEND_STACK_BYTES: usize = 8 * 1024 * 1024;

#[cfg(target_arch = "wasm32")]
pub(super) const RECURSIVE_SVG_BACKEND_STACK_BYTES: usize = 0;

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn run_recursive_svg_backend<T, F>(control: &OperationControl, job: F) -> Result<T>
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

#[cfg(target_arch = "wasm32")]
pub(super) fn run_recursive_svg_backend<T, F>(control: &OperationControl, job: F) -> Result<T>
where
    T: Send,
    F: FnOnce(&OperationControl) -> Result<T> + Send,
{
    export_checkpoint(control)?;
    settle_export_result(control, job(control))
}
