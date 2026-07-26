use crate::{
    HostTextMeasurement, HostTextMeasurementRequest, TextMeasurementPhase, TextMetrics, WrapMode,
};

const HOST_WRAP_MODE_SVG_LIKE: i32 = 0;
const HOST_WRAP_MODE_SVG_LIKE_SINGLE_RUN: i32 = 1;
const HOST_WRAP_MODE_HTML_LIKE: i32 = 2;
const HOST_TEXT_DIRECTION_AUTO: i32 = 0;
const HOST_TEXT_WHITE_SPACE_NORMAL: i32 = 0;
const HOST_TEXT_WHITE_SPACE_NOWRAP: i32 = 1;
const HOST_TEXT_WHITE_SPACE_BREAK_SPACES: i32 = 2;
const HOST_TEXT_MEASUREMENT_PHASE_LAYOUT: i32 = 0;
const HOST_TEXT_MEASUREMENT_PHASE_WRAP: i32 = 1;
const HOST_TEXT_MEASUREMENT_PHASE_SVG_BBOX: i32 = 2;
const HOST_TEXT_MEASUREMENT_PHASE_COMPUTED_LENGTH: i32 = 3;

/// Stable result-shape discriminator shared by all host text-measurement transports.
pub use merman::svg::TextMeasurementResultKind as HostTextMeasurementResultKind;

/// Transport-neutral stable fields derived from one host text-measurement request.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostTextMeasurementTransportFields {
    pub line_height: f64,
    pub wrap_mode: i32,
    pub direction: i32,
    pub white_space: i32,
    pub phase: i32,
    pub operation: i32,
}

/// Projects renderer types into the numeric protocol shared by native binding transports.
#[must_use]
pub fn host_text_measurement_transport_fields(
    request: HostTextMeasurementRequest<'_>,
) -> HostTextMeasurementTransportFields {
    let wrap_mode = match request.wrap_mode {
        WrapMode::SvgLike => HOST_WRAP_MODE_SVG_LIKE,
        WrapMode::SvgLikeSingleRun => HOST_WRAP_MODE_SVG_LIKE_SINGLE_RUN,
        WrapMode::HtmlLike => HOST_WRAP_MODE_HTML_LIKE,
    };
    let line_height_factor = match request.wrap_mode {
        WrapMode::SvgLike | WrapMode::SvgLikeSingleRun => 1.1,
        WrapMode::HtmlLike => 1.5,
    };
    let white_space = match request.wrap_mode {
        WrapMode::HtmlLike if request.max_width.is_some() => HOST_TEXT_WHITE_SPACE_BREAK_SPACES,
        WrapMode::HtmlLike => HOST_TEXT_WHITE_SPACE_NOWRAP,
        WrapMode::SvgLike | WrapMode::SvgLikeSingleRun => HOST_TEXT_WHITE_SPACE_NORMAL,
    };
    let phase = match request.phase {
        TextMeasurementPhase::Layout => HOST_TEXT_MEASUREMENT_PHASE_LAYOUT,
        TextMeasurementPhase::Wrap => HOST_TEXT_MEASUREMENT_PHASE_WRAP,
        TextMeasurementPhase::SvgBBox => HOST_TEXT_MEASUREMENT_PHASE_SVG_BBOX,
        TextMeasurementPhase::ComputedLength => HOST_TEXT_MEASUREMENT_PHASE_COMPUTED_LENGTH,
    };

    HostTextMeasurementTransportFields {
        line_height: request.style.font_size.max(1.0) * line_height_factor,
        wrap_mode,
        direction: HOST_TEXT_DIRECTION_AUTO,
        white_space,
        phase,
        operation: request.operation.external_code(),
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HostTextMeasurementValues {
    pub width: f64,
    pub height: f64,
    pub line_count: usize,
    pub length: f64,
    pub bbox_left: f64,
    pub bbox_right: f64,
    pub raw_width: Option<f64>,
}

pub fn host_text_measurement_from_values(
    result_kind: Option<HostTextMeasurementResultKind>,
    values: HostTextMeasurementValues,
) -> HostTextMeasurement {
    let metrics = || TextMetrics {
        width: values.width,
        height: values.height,
        line_count: values.line_count,
    };
    match result_kind {
        Some(HostTextMeasurementResultKind::Metrics) => HostTextMeasurement::Metrics(metrics()),
        Some(HostTextMeasurementResultKind::Length) => HostTextMeasurement::Length(values.length),
        Some(HostTextMeasurementResultKind::HorizontalExtents) => {
            HostTextMeasurement::HorizontalExtents {
                left: values.bbox_left,
                right: values.bbox_right,
            }
        }
        Some(HostTextMeasurementResultKind::WrappedWithRawWidth) => {
            HostTextMeasurement::WrappedWithRawWidth {
                metrics: metrics(),
                raw_width: values.raw_width,
            }
        }
        None => HostTextMeasurement::Metrics(TextMetrics {
            width: f64::NAN,
            height: f64::NAN,
            line_count: 0,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TextMeasurementOperation, TextStyle};

    fn values() -> HostTextMeasurementValues {
        HostTextMeasurementValues {
            width: 21.0,
            height: 13.0,
            line_count: 2,
            length: 17.0,
            bbox_left: 3.0,
            bbox_right: 18.0,
            raw_width: Some(34.0),
        }
    }

    #[test]
    fn external_result_kinds_have_stable_mappings() {
        assert_eq!(
            HostTextMeasurementResultKind::ALL
                .map(|kind| (kind.external_code(), kind.external_name())),
            [
                (0, "metrics"),
                (1, "length"),
                (2, "horizontal-extents"),
                (3, "wrapped-with-raw-width"),
            ]
        );
    }

    #[test]
    fn native_transports_share_one_stable_request_projection() {
        let style = TextStyle {
            font_size: 10.0,
            ..TextStyle::default()
        };
        let base = HostTextMeasurementRequest {
            operation: TextMeasurementOperation::Measure,
            phase: TextMeasurementPhase::Layout,
            text: "A",
            style: &style,
            max_width: None,
            wrap_mode: WrapMode::SvgLike,
        };

        assert_eq!(
            host_text_measurement_transport_fields(base),
            HostTextMeasurementTransportFields {
                line_height: 11.0,
                wrap_mode: 0,
                direction: 0,
                white_space: 0,
                phase: 0,
                operation: 0,
            }
        );
        assert_eq!(
            host_text_measurement_transport_fields(HostTextMeasurementRequest {
                phase: TextMeasurementPhase::ComputedLength,
                max_width: Some(30.0),
                wrap_mode: WrapMode::HtmlLike,
                ..base
            }),
            HostTextMeasurementTransportFields {
                line_height: 15.0,
                wrap_mode: 2,
                direction: 0,
                white_space: 2,
                phase: 3,
                operation: 0,
            }
        );
        assert_eq!(
            host_text_measurement_transport_fields(HostTextMeasurementRequest {
                phase: TextMeasurementPhase::SvgBBox,
                wrap_mode: WrapMode::SvgLikeSingleRun,
                ..base
            }),
            HostTextMeasurementTransportFields {
                line_height: 11.0,
                wrap_mode: 1,
                direction: 0,
                white_space: 0,
                phase: 2,
                operation: 0,
            }
        );
        assert_eq!(
            host_text_measurement_transport_fields(HostTextMeasurementRequest {
                phase: TextMeasurementPhase::Wrap,
                wrap_mode: WrapMode::HtmlLike,
                ..base
            })
            .white_space,
            1
        );
    }

    #[test]
    fn operations_declare_their_required_result_shapes() {
        let mappings = TextMeasurementOperation::ALL.map(|operation| {
            (
                operation.external_code(),
                HostTextMeasurementResultKind::expected_for_operation(operation).external_name(),
            )
        });

        assert_eq!(
            mappings,
            [
                (0, "metrics"),
                (1, "length"),
                (2, "horizontal-extents"),
                (3, "horizontal-extents"),
                (4, "horizontal-extents"),
                (5, "length"),
                (6, "length"),
                (7, "length"),
                (8, "length"),
                (9, "length"),
                (10, "length"),
                (11, "metrics"),
                (12, "wrapped-with-raw-width"),
                (13, "length"),
                (14, "length"),
                (15, "metrics"),
                (16, "length"),
                (17, "length"),
                (18, "length"),
            ]
        );
    }

    #[test]
    fn external_values_decode_by_declared_result_kind() {
        let HostTextMeasurement::Metrics(metrics) = host_text_measurement_from_values(
            Some(HostTextMeasurementResultKind::Metrics),
            values(),
        ) else {
            panic!("metrics kind should decode as metrics");
        };
        assert_eq!(
            (metrics.width, metrics.height, metrics.line_count),
            (21.0, 13.0, 2)
        );

        let HostTextMeasurement::Length(length) = host_text_measurement_from_values(
            Some(HostTextMeasurementResultKind::Length),
            values(),
        ) else {
            panic!("length kind should decode as a length");
        };
        assert_eq!(length, 17.0);

        let HostTextMeasurement::HorizontalExtents { left, right } =
            host_text_measurement_from_values(
                Some(HostTextMeasurementResultKind::HorizontalExtents),
                values(),
            )
        else {
            panic!("horizontal-extents kind should decode as extents");
        };
        assert_eq!((left, right), (3.0, 18.0));

        let HostTextMeasurement::WrappedWithRawWidth { metrics, raw_width } =
            host_text_measurement_from_values(
                Some(HostTextMeasurementResultKind::WrappedWithRawWidth),
                values(),
            )
        else {
            panic!("wrapped-with-raw-width kind should preserve both values");
        };
        assert_eq!(
            (metrics.width, metrics.height, metrics.line_count),
            (21.0, 13.0, 2)
        );
        assert_eq!(raw_width, Some(34.0));
    }

    #[test]
    fn unknown_external_result_kind_decodes_to_an_always_invalid_value() {
        let HostTextMeasurement::Metrics(metrics) =
            host_text_measurement_from_values(None, values())
        else {
            panic!("unknown kind should use the invalid metrics sentinel");
        };
        assert!(metrics.width.is_nan());
        assert!(metrics.height.is_nan());
        assert_eq!(metrics.line_count, 0);
    }
}
