//! Host measurement requests and validation shared by routing and bindings.

use super::{TextMetrics, TextStyle, WrapMode};
use merman_model::OperationControl;
use std::sync::Arc;

#[rustfmt::skip]
mod abi;
#[cfg(feature = "resources")]
mod route;
#[cfg(all(test, feature = "svg-parse"))]
pub(super) mod test_support;
mod trusted;

#[cfg(feature = "resources")]
pub use route::{
    InvalidMeasurementProfileIdentity, MeasurementProfileId, RoutedTextMeasurer,
    TextMeasurementPolicy, TextMeasurementProfile, TextMeasurementProfileIdentity,
    TextMeasurementProvenance, TextMeasurementReport, TextMeasurementRoute, TextMeasurementSession,
    TextMeasurementSource, TextMeasurementSummary,
};
#[cfg(feature = "svg-output")]
pub(super) use trusted::BuiltinSvgComputedLength;
#[cfg(feature = "resources")]
pub(super) use trusted::BuiltinTextMeasurementOperationCarrier;
#[cfg(feature = "svg-parse")]
pub(super) use trusted::InlineHtmlMeasurementCarrier;
pub(super) use trusted::{BuiltinHtmlLineWidth, TRUSTED_MEASUREMENT, TrustedMeasurementQuery};
#[cfg(feature = "resources")]
pub use trusted::{MeasurementReuseKey, measurement_reuse_key};

/// The exact operation requested from a text measurer and its required host result shape.
/// Both types are generated from the independently versioned host text-measurement protocol.
pub use abi::{
    TEXT_MEASUREMENT_PROTOCOL_VERSION, TextMeasurementOperation, TextMeasurementResultKind,
};

/// A render phase that may select a distinct complete text-measurement profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextMeasurementPhase {
    Layout,
    Wrap,
    SvgBBox,
    ComputedLength,
}

impl TextMeasurementPhase {
    pub const ALL: [Self; 4] = [
        Self::Layout,
        Self::Wrap,
        Self::SvgBBox,
        Self::ComputedLength,
    ];
}

#[cfg(feature = "resources")]
const fn phase_index(phase: TextMeasurementPhase) -> usize {
    match phase {
        TextMeasurementPhase::Layout => 0,
        TextMeasurementPhase::Wrap => 1,
        TextMeasurementPhase::SvgBBox => 2,
        TextMeasurementPhase::ComputedLength => 3,
    }
}

#[doc(hidden)]
pub const fn operation_index(operation: TextMeasurementOperation) -> usize {
    operation.index()
}

/// Why a configured host attempt used its named fallback profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HostFallbackReason {
    Missing,
    Invalid,
    Error,
}

/// A host callback failure or invalid result converted to explicit fallback provenance.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct HostTextMeasurementError {
    message: Arc<str>,
    fallback_reason: HostFallbackReason,
}

impl HostTextMeasurementError {
    /// Creates an error reported by the host callback or its transport.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: Arc::from(message.into()),
            fallback_reason: HostFallbackReason::Error,
        }
    }

    /// Creates an error for a callback value that violates the measurement contract.
    #[doc(hidden)]
    pub fn invalid_value(message: impl Into<String>) -> Self {
        Self {
            message: Arc::from(message.into()),
            fallback_reason: HostFallbackReason::Invalid,
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HostTextMeasurementRequest<'a> {
    pub operation: TextMeasurementOperation,
    pub phase: TextMeasurementPhase,
    pub text: &'a str,
    pub style: &'a TextStyle,
    pub max_width: Option<f64>,
    pub wrap_mode: WrapMode,
}

#[derive(Debug, Clone, Copy)]
pub enum HostTextMeasurement {
    Metrics(TextMetrics),
    Length(f64),
    HorizontalExtents {
        left: f64,
        right: f64,
    },
    WrappedWithRawWidth {
        metrics: TextMetrics,
        raw_width: Option<f64>,
    },
}

pub type HostMeasurementResult = Result<Option<HostTextMeasurement>, HostTextMeasurementError>;

/// Fallible, operation-aware host counterpart of the infallible render text measurer.
///
/// Returning `Ok(None)` declines exactly the requested operation. Returning a result variant that
/// does not match `request.operation`, or an invalid value, uses the configured fallback and is
/// recorded as [`HostFallbackReason::Invalid`].
pub trait HostTextMeasurer: Send + Sync {
    fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult;

    /// Measures within the active render operation. Existing host adapters may ignore control.
    /// Native adapters use it to checkpoint bounded shaping and wrapping work.
    fn measure_with_control(
        &self,
        request: HostTextMeasurementRequest<'_>,
        _control: &OperationControl,
    ) -> HostMeasurementResult {
        self.measure(request)
    }
}

/// Checks a host callback value against the complete operation request.
///
/// The validator is the single authority for result shape and numeric bounds across direct
/// renderer hosts and every binding transport.
pub fn validate_host_text_measurement(
    request: &HostTextMeasurementRequest<'_>,
    measurement: &HostTextMeasurement,
) -> Result<(), HostTextMeasurementError> {
    let result_kind = match measurement {
        HostTextMeasurement::Metrics(_) => TextMeasurementResultKind::Metrics,
        HostTextMeasurement::Length(_) => TextMeasurementResultKind::Length,
        HostTextMeasurement::HorizontalExtents { .. } => {
            TextMeasurementResultKind::HorizontalExtents
        }
        HostTextMeasurement::WrappedWithRawWidth { .. } => {
            TextMeasurementResultKind::WrappedWithRawWidth
        }
    };
    let required_kind = request.operation.required_result_kind();
    if result_kind != required_kind {
        return Err(HostTextMeasurementError::invalid_value(format!(
            "host text measurement operation `{}` requires `{}` but returned `{}`",
            request.operation.external_name(),
            required_kind.external_name(),
            result_kind.external_name(),
        )));
    }

    let valid = match measurement {
        HostTextMeasurement::Metrics(metrics) => valid_metrics(request, metrics),
        HostTextMeasurement::Length(value) => {
            value.is_finite() && (request.operation.accepts_signed_length() || *value >= 0.0)
        }
        HostTextMeasurement::HorizontalExtents { left, right } => valid_extents(*left, *right),
        HostTextMeasurement::WrappedWithRawWidth { metrics, raw_width } => {
            valid_metrics(request, metrics)
                && raw_width.is_none_or(|value| value.is_finite() && value >= 0.0)
        }
    };
    if valid {
        Ok(())
    } else {
        Err(HostTextMeasurementError::invalid_value(format!(
            "host text measurement operation `{}` returned an invalid `{}` value",
            request.operation.external_name(),
            result_kind.external_name(),
        )))
    }
}

fn valid_metrics(request: &HostTextMeasurementRequest<'_>, metrics: &TextMetrics) -> bool {
    metrics.width.is_finite()
        && metrics.height.is_finite()
        && metrics.width >= 0.0
        && metrics.height >= 0.0
        && metrics.line_count > 0
        && metrics.line_count <= request.text.len().saturating_add(1)
}

fn valid_extents(left: f64, right: f64) -> bool {
    left.is_finite()
        && right.is_finite()
        && left >= 0.0
        && right >= 0.0
        && (left + right).is_finite()
}
