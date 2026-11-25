use serde::{Serialize, Serializer};

use super::BINDING_JSON_SAFE_INTEGER_MAX;
#[cfg(feature = "svg")]
use super::icon_registry_error_status;

mod transport_envelope;

pub use transport_envelope::{
    BindingErrorBody, binding_error_details_json, binding_error_js_details_json,
    binding_error_js_payload_json_bytes, binding_error_payload_json_bytes,
    error_payload_json_bytes,
};

/// Stable machine-readable classification carried by binding errors.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BindingErrorKind {
    #[default]
    Generic,
    UnknownOperation,
    MissingCapability,
    Busy,
    ReentrantCall,
}

impl BindingErrorKind {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Generic => "generic",
            Self::UnknownOperation => "unknown-operation",
            Self::MissingCapability => "missing-capability",
            Self::Busy => "busy",
            Self::ReentrantCall => "reentrant-call",
        }
    }
}

/// Stable machine-readable reason for a resource-limit failure.
#[repr(u8)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BindingResourceLimitCause {
    #[default]
    Ceiling,
    ArithmeticOverflow,
    /// A cause added by a newer engine that this binding layer does not yet classify.
    Unknown,
}

impl BindingResourceLimitCause {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ceiling => "ceiling",
            Self::ArithmeticOverflow => "arithmetic_overflow",
            Self::Unknown => "unknown",
        }
    }
}

/// Structured cooperative-cancellation details carried by the additive error payload.
///
/// The binding layer projects the core operation error to stable strings so transports do not
/// need to depend on the non-exhaustive core phase enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct BindingCancellationErrorDetails {
    pub reason: &'static str,
    pub phase: &'static str,
}

impl BindingCancellationErrorDetails {
    #[must_use]
    pub const fn from_operation(error: merman::OperationCancelled) -> Self {
        Self {
            reason: error.reason.as_str(),
            phase: error.phase.as_str(),
        }
    }
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingStatus {
    Ok = 0,
    InvalidArgument = 1,
    Utf8Error = 2,
    OptionsJsonError = 3,
    NoDiagram = 4,
    ParseError = 5,
    RenderError = 6,
    UnsupportedOperation = 7,
    Panic = 8,
    InternalError = 9,
    ResourceLimitExceeded = 10,
    Busy = 11,
    Cancelled = 12,
}

impl BindingStatus {
    pub const fn code(self) -> i32 {
        self as i32
    }

    pub const fn code_name(self) -> &'static str {
        match self {
            Self::Ok => "MERMAN_OK",
            Self::InvalidArgument => "MERMAN_INVALID_ARGUMENT",
            Self::Utf8Error => "MERMAN_UTF8_ERROR",
            Self::OptionsJsonError => "MERMAN_OPTIONS_JSON_ERROR",
            Self::NoDiagram => "MERMAN_NO_DIAGRAM",
            Self::ParseError => "MERMAN_PARSE_ERROR",
            Self::RenderError => "MERMAN_RENDER_ERROR",
            Self::UnsupportedOperation => "MERMAN_UNSUPPORTED_OPERATION",
            Self::Panic => "MERMAN_PANIC",
            Self::InternalError => "MERMAN_INTERNAL_ERROR",
            Self::ResourceLimitExceeded => "MERMAN_RESOURCE_LIMIT_EXCEEDED",
            Self::Busy => "MERMAN_BUSY",
            Self::Cancelled => "MERMAN_CANCELLED",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingError {
    status: BindingStatus,
    kind: BindingErrorKind,
    capability_id: Option<&'static str>,
    details: Option<Box<BindingErrorDetails>>,
    message: Box<str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct BindingErrorDetails {
    resource: Option<BindingResourceErrorDetails>,
    diagnostic: Option<BindingDiagnosticErrorDetails>,
    cancellation: Option<BindingCancellationErrorDetails>,
    icon_registry: Option<BindingIconRegistryErrorDetails>,
}

/// Structured resource failure details carried by the additive error JSON payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct BindingResourceErrorDetails {
    /// Stable reason for the failure: `ceiling` or `arithmetic_overflow`.
    pub cause: BindingResourceLimitCause,
    pub limit_id: &'static str,
    pub phase: &'static str,
    pub actual: u64,
    pub max: u64,
    pub profile: &'static str,
}

/// Byte span carried by a structured parser or renderer diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct BindingDiagnosticSpan {
    pub start: u64,
    pub end: u64,
    pub kind: &'static str,
}

/// Stable machine-readable context for parse and ASCII render failures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct BindingDiagnosticErrorDetails {
    pub code: String,
    pub span: Option<BindingDiagnosticSpan>,
    pub field: Option<String>,
    pub diagram_type: Option<String>,
    pub requested_max_width: Option<u64>,
    pub actual_width: Option<u64>,
    pub width_profile: Option<String>,
    pub fallback_reason: Option<String>,
}

impl BindingDiagnosticSpan {
    pub const fn new(start: u64, end: u64, kind: &'static str) -> Self {
        Self { start, end, kind }
    }
}

impl BindingDiagnosticErrorDetails {
    pub fn new(code: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            span: None,
            field: None,
            diagram_type: None,
            requested_max_width: None,
            actual_width: None,
            width_profile: None,
            fallback_reason: None,
        }
    }

    #[must_use]
    pub const fn with_span(mut self, span: BindingDiagnosticSpan) -> Self {
        self.span = Some(span);
        self
    }

    #[must_use]
    pub fn with_field(mut self, field: impl Into<String>) -> Self {
        self.field = Some(field.into());
        self
    }

    #[must_use]
    pub fn with_diagram_type(mut self, diagram_type: impl Into<String>) -> Self {
        self.diagram_type = Some(diagram_type.into());
        self
    }
}

impl BindingResourceErrorDetails {
    /// Projects exact resource counts into the lossless JSON representation used by JavaScript
    /// transports without changing the native binding payload contract.
    #[doc(hidden)]
    #[must_use]
    pub const fn js_safe_json(self) -> BindingJsSafeResourceErrorDetails {
        BindingJsSafeResourceErrorDetails {
            cause: self.cause,
            limit_id: self.limit_id,
            phase: self.phase,
            actual: self.actual,
            max: self.max,
            profile: self.profile,
        }
    }
}

/// JavaScript-safe serialization projection for [`BindingResourceErrorDetails`].
///
/// Values through `Number.MAX_SAFE_INTEGER` remain JSON numbers. Wider `u64` values serialize as
/// canonical decimal strings so Node and WebAssembly hosts retain the exact count.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BindingJsSafeResourceErrorDetails {
    cause: BindingResourceLimitCause,
    limit_id: &'static str,
    phase: &'static str,
    #[serde(serialize_with = "serialize_binding_resource_count")]
    actual: u64,
    #[serde(serialize_with = "serialize_binding_resource_count")]
    max: u64,
    profile: &'static str,
}

fn serialize_binding_resource_count<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if *value <= BINDING_JSON_SAFE_INTEGER_MAX {
        serializer.serialize_u64(*value)
    } else {
        serializer.collect_str(value)
    }
}

/// Structured failure details for transactional Iconify registry construction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct BindingIconRegistryErrorDetails {
    pub kind_id: &'static str,
    pub pack_index: Option<u64>,
    pub registration_name: Option<String>,
}

impl BindingError {
    fn classified(
        status: BindingStatus,
        kind: BindingErrorKind,
        capability_id: Option<&'static str>,
        details: Option<BindingErrorDetails>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            status,
            kind,
            capability_id,
            details: details.map(Box::new),
            message: message.into().into_boxed_str(),
        }
    }

    pub fn new(status: BindingStatus, message: impl Into<String>) -> Self {
        Self::classified(status, BindingErrorKind::Generic, None, None, message)
    }

    /// Creates a caller-owned invalid-argument failure without requiring status reconstruction.
    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self::new(BindingStatus::InvalidArgument, message)
    }

    /// Creates a caller-owned options-JSON failure without requiring status reconstruction.
    pub fn invalid_options_json(message: impl Into<String>) -> Self {
        Self::new(BindingStatus::OptionsJsonError, message)
    }

    /// Creates an internal implementation failure without requiring status reconstruction.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(BindingStatus::InternalError, message)
    }

    pub fn unknown_operation(message: impl Into<String>) -> Self {
        Self::classified(
            BindingStatus::UnsupportedOperation,
            BindingErrorKind::UnknownOperation,
            None,
            None,
            message,
        )
    }

    pub fn missing_capability(capability_id: &'static str, message: impl Into<String>) -> Self {
        Self::classified(
            BindingStatus::UnsupportedOperation,
            BindingErrorKind::MissingCapability,
            Some(capability_id),
            None,
            message,
        )
    }

    /// Creates a caller-owned failure for a known operation or metadata endpoint that this
    /// transport intentionally does not expose.
    pub fn unsupported_operation(message: impl Into<String>) -> Self {
        Self::new(BindingStatus::UnsupportedOperation, message)
    }

    pub fn reentrant_call(message: impl Into<String>) -> Self {
        Self::classified(
            BindingStatus::InvalidArgument,
            BindingErrorKind::ReentrantCall,
            None,
            None,
            message,
        )
    }

    pub fn busy(message: impl Into<String>) -> Self {
        Self::classified(
            BindingStatus::Busy,
            BindingErrorKind::Busy,
            None,
            None,
            message,
        )
    }

    pub fn resource_limit(
        phase: &'static str,
        limit_id: &'static str,
        actual: u64,
        max: u64,
        profile: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self::resource_limit_with_cause(
            BindingResourceLimitCause::Ceiling,
            phase,
            limit_id,
            actual,
            max,
            profile,
            message,
        )
    }

    /// Constructs a resource-limit error with an explicit stable failure cause.
    pub fn resource_limit_with_cause(
        cause: BindingResourceLimitCause,
        phase: &'static str,
        limit_id: &'static str,
        actual: u64,
        max: u64,
        profile: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self::classified(
            BindingStatus::ResourceLimitExceeded,
            BindingErrorKind::Generic,
            None,
            Some(BindingErrorDetails {
                resource: Some(BindingResourceErrorDetails {
                    cause,
                    limit_id,
                    phase,
                    actual,
                    max,
                    profile,
                }),
                diagnostic: None,
                cancellation: None,
                icon_registry: None,
            }),
            message,
        )
    }

    /// Creates the canonical structured failure for UTF-8 rejected before registry ingestion.
    #[cfg(feature = "svg")]
    pub fn icon_registry_invalid_utf8(pack_index: usize, message: impl Into<String>) -> Self {
        Self::icon_registry_preflight_failure(
            merman::svg::IconRegistryBuildErrorKind::InvalidUtf8,
            Some(pack_index),
            None,
            message,
        )
    }

    /// Creates the canonical structured failure for a registry limit rejected before ingestion.
    #[cfg(feature = "svg")]
    pub fn icon_registry_resource_limit(
        limit: crate::IconRegistryResourceLimitId,
        actual: u64,
        pack_index: Option<usize>,
        message: impl Into<String>,
    ) -> Self {
        let descriptor = limit.descriptor();
        Self::icon_registry_preflight_failure(
            merman::svg::IconRegistryBuildErrorKind::ResourceLimitExceeded,
            pack_index,
            Some(BindingResourceErrorDetails {
                cause: BindingResourceLimitCause::Ceiling,
                limit_id: descriptor.stable_id,
                phase: descriptor.phase,
                actual,
                max: limit.fixed_value(),
                profile: "constructor-fixed",
            }),
            message,
        )
    }

    #[cfg(feature = "svg")]
    fn icon_registry_preflight_failure(
        kind: merman::svg::IconRegistryBuildErrorKind,
        pack_index: Option<usize>,
        resource: Option<BindingResourceErrorDetails>,
        message: impl Into<String>,
    ) -> Self {
        Self::classified(
            icon_registry_error_status(kind),
            BindingErrorKind::Generic,
            None,
            Some(BindingErrorDetails {
                resource,
                diagnostic: None,
                cancellation: None,
                icon_registry: Some(BindingIconRegistryErrorDetails {
                    kind_id: kind.stable_id(),
                    pack_index: pack_index.and_then(|index| u64::try_from(index).ok()),
                    registration_name: None,
                }),
            }),
            message,
        )
    }

    pub const fn status(&self) -> BindingStatus {
        self.status
    }

    pub const fn kind(&self) -> BindingErrorKind {
        self.kind
    }

    pub const fn capability_id(&self) -> Option<&'static str> {
        self.capability_id
    }

    pub fn resource_details(&self) -> Option<BindingResourceErrorDetails> {
        self.details.as_deref().and_then(|details| details.resource)
    }

    pub fn with_diagnostic_details(mut self, details: BindingDiagnosticErrorDetails) -> Self {
        self.details
            .get_or_insert_with(|| Box::new(BindingErrorDetails::default()))
            .diagnostic = Some(details);
        self
    }

    pub fn diagnostic_details(&self) -> Option<&BindingDiagnosticErrorDetails> {
        self.details
            .as_deref()
            .and_then(|details| details.diagnostic.as_ref())
    }

    #[must_use]
    pub fn cancellation_details(&self) -> Option<BindingCancellationErrorDetails> {
        match self.details.as_deref() {
            Some(details) => details.cancellation,
            None => None,
        }
    }

    /// Creates the canonical structured failure for cooperative cancellation or deadline expiry.
    pub fn cancelled(error: merman::OperationCancelled) -> Self {
        Self::classified(
            BindingStatus::Cancelled,
            BindingErrorKind::Generic,
            None,
            Some(BindingErrorDetails {
                resource: None,
                diagnostic: None,
                cancellation: Some(BindingCancellationErrorDetails::from_operation(error)),
                icon_registry: None,
            }),
            error.to_string(),
        )
    }

    pub fn icon_registry_details(&self) -> Option<&BindingIconRegistryErrorDetails> {
        self.details
            .as_deref()
            .and_then(|details| details.icon_registry.as_ref())
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

#[cfg(feature = "svg")]
impl From<merman::svg::IconRegistryBuildError> for BindingError {
    fn from(error: merman::svg::IconRegistryBuildError) -> Self {
        let error_kind = error.kind();
        let limit = error.limit();
        let message = error.to_string();
        let details = BindingIconRegistryErrorDetails {
            kind_id: error.kind().stable_id(),
            pack_index: error
                .pack_index()
                .and_then(|index| u64::try_from(index).ok()),
            registration_name: error.registration_name().map(str::to_owned),
        };
        let resource = match (limit, error.actual(), error.maximum()) {
            (Some(limit), Some(actual), Some(max)) => {
                let descriptor = limit.descriptor();
                Some(BindingResourceErrorDetails {
                    cause: match error_kind {
                        merman::svg::IconRegistryBuildErrorKind::ArithmeticOverflow => {
                            BindingResourceLimitCause::ArithmeticOverflow
                        }
                        _ => BindingResourceLimitCause::Ceiling,
                    },
                    limit_id: descriptor.stable_id,
                    phase: descriptor.phase,
                    actual,
                    max,
                    profile: "constructor-fixed",
                })
            }
            _ => None,
        };
        Self::classified(
            icon_registry_error_status(error_kind),
            BindingErrorKind::Generic,
            None,
            Some(BindingErrorDetails {
                resource,
                diagnostic: None,
                cancellation: None,
                icon_registry: Some(details),
            }),
            message,
        )
    }
}

#[cfg(feature = "analysis")]
impl From<merman_analysis::AnalysisOptionsJsonError> for BindingError {
    fn from(error: merman_analysis::AnalysisOptionsJsonError) -> Self {
        BindingError::new(BindingStatus::InvalidArgument, error.to_string())
    }
}
