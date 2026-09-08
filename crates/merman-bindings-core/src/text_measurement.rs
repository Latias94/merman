use crate::{
    HostTextMeasurement, HostTextMeasurementRequest, NormalLineMetrics, TextMeasurementOperation,
    TextMeasurementPhase, TextMetrics, WrapMode,
};
use merman::svg::validate_host_text_measurement;

include!("generated/text_measurement_abi.rs");

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
        WrapMode::SvgLike => HostTextWrapModeCode::SvgLike,
        WrapMode::SvgLikeSingleRun => HostTextWrapModeCode::SvgLikeSingleRun,
        WrapMode::HtmlLike => HostTextWrapModeCode::HtmlLike,
    };
    let line_height_factor = match request.wrap_mode {
        WrapMode::SvgLike | WrapMode::SvgLikeSingleRun => 1.1,
        WrapMode::HtmlLike => 1.5,
    };
    let white_space = match request.wrap_mode {
        WrapMode::HtmlLike if request.max_width.is_some() => HostTextWhiteSpaceCode::BreakSpaces,
        WrapMode::HtmlLike => HostTextWhiteSpaceCode::Nowrap,
        WrapMode::SvgLike | WrapMode::SvgLikeSingleRun => HostTextWhiteSpaceCode::Normal,
    };
    let phase = match request.phase {
        TextMeasurementPhase::Layout => HostTextMeasurementPhaseCode::Layout,
        TextMeasurementPhase::Wrap => HostTextMeasurementPhaseCode::Wrap,
        TextMeasurementPhase::SvgBBox => HostTextMeasurementPhaseCode::SvgBBox,
        TextMeasurementPhase::ComputedLength => HostTextMeasurementPhaseCode::ComputedLength,
    };
    let normal = request.operation == TextMeasurementOperation::NormalLineMetrics;

    HostTextMeasurementTransportFields {
        // Zero is an inapplicable explicit-line-height hint for the normal-metrics operation,
        // not a request to lay out a zero-height line. That operation measures CSS `normal`.
        line_height: if normal {
            0.0
        } else {
            request.style.font_size.max(1.0) * line_height_factor
        },
        wrap_mode: wrap_mode.external_code(),
        direction: HostTextDirectionCode::Auto.external_code(),
        white_space: if normal {
            HostTextWhiteSpaceCode::Normal
        } else {
            white_space
        }
        .external_code(),
        phase: phase.external_code(),
        operation: request.operation.external_code(),
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HostTextMeasurementRecord {
    pub result_kind: Option<HostTextMeasurementResultKind>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub line_count: Option<i128>,
    pub length: Option<f64>,
    pub bbox_left: Option<f64>,
    pub bbox_right: Option<f64>,
    pub raw_width: Option<f64>,
    pub normal_line_metrics: Option<NormalLineMetrics>,
}

pub fn decode_host_text_measurement(
    request: HostTextMeasurementRequest<'_>,
    record: HostTextMeasurementRecord,
) -> Result<HostTextMeasurement, crate::HostTextMeasurementError> {
    if record.bbox_left.is_some() != record.bbox_right.is_some() {
        return Err(invalid_record(
            "bbox_left and bbox_right must either both be present or both be absent",
        ));
    }

    let Some(result_kind) = record.result_kind else {
        return Err(invalid_record(
            "host text measurement result kind is missing or unknown",
        ));
    };
    let required_kind = HostTextMeasurementResultKind::expected_for_operation(request.operation);
    if result_kind != required_kind {
        return Err(invalid_record(format!(
            "operation `{}` requires `{}` but returned `{}`",
            request.operation.external_name(),
            required_kind.external_name(),
            result_kind.external_name(),
        )));
    }
    if record.raw_width.is_some()
        && result_kind != HostTextMeasurementResultKind::WrappedWithRawWidth
    {
        return Err(invalid_record(
            "raw_width is only valid for wrapped-with-raw-width results",
        ));
    }
    if record.normal_line_metrics.is_some()
        && result_kind != HostTextMeasurementResultKind::NormalLineMetrics
    {
        return Err(invalid_record(
            "normal_line_metrics is only valid for normal-line-metrics results",
        ));
    }

    let measurement = match result_kind {
        HostTextMeasurementResultKind::NormalLineMetrics => HostTextMeasurement::NormalLineMetrics(
            required_field(record.normal_line_metrics, "normal_line_metrics")?,
        ),
        HostTextMeasurementResultKind::Metrics => {
            HostTextMeasurement::Metrics(decode_metrics(&record)?)
        }
        HostTextMeasurementResultKind::Length => {
            HostTextMeasurement::Length(required_field(record.length, "length")?)
        }
        HostTextMeasurementResultKind::HorizontalExtents => {
            HostTextMeasurement::HorizontalExtents {
                left: required_field(record.bbox_left, "bbox_left")?,
                right: required_field(record.bbox_right, "bbox_right")?,
            }
        }
        HostTextMeasurementResultKind::WrappedWithRawWidth => {
            HostTextMeasurement::WrappedWithRawWidth {
                metrics: decode_metrics(&record)?,
                raw_width: record.raw_width,
            }
        }
    };
    validate_host_text_measurement(&request, &measurement)?;
    Ok(measurement)
}

fn decode_metrics(
    record: &HostTextMeasurementRecord,
) -> Result<TextMetrics, crate::HostTextMeasurementError> {
    let raw_line_count = required_field(record.line_count, "line_count")?;
    let line_count = usize::try_from(raw_line_count)
        .map_err(|_| invalid_record("line_count cannot be represented losslessly as usize"))?;
    Ok(TextMetrics {
        width: required_field(record.width, "width")?,
        height: required_field(record.height, "height")?,
        line_count,
    })
}

fn required_field<T: Copy>(
    value: Option<T>,
    field: &str,
) -> Result<T, crate::HostTextMeasurementError> {
    value.ok_or_else(|| invalid_record(format!("required field `{field}` is missing")))
}

fn invalid_record(message: impl Into<String>) -> crate::HostTextMeasurementError {
    crate::HostTextMeasurementError::invalid_value(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TextMeasurementOperation, TextStyle};

    fn request<'a>(
        operation: TextMeasurementOperation,
        text: &'a str,
        style: &'a TextStyle,
    ) -> HostTextMeasurementRequest<'a> {
        HostTextMeasurementRequest {
            operation,
            phase: TextMeasurementPhase::Layout,
            text,
            style,
            max_width: None,
            wrap_mode: WrapMode::SvgLike,
        }
    }

    fn record(result_kind: Option<HostTextMeasurementResultKind>) -> HostTextMeasurementRecord {
        HostTextMeasurementRecord {
            result_kind,
            width: Some(21.0),
            height: Some(13.0),
            line_count: Some(2),
            length: Some(17.0),
            bbox_left: Some(3.0),
            bbox_right: Some(18.0),
            raw_width: Some(34.0),
            normal_line_metrics: None,
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
                (4, "normal-line-metrics"),
            ]
        );
    }

    #[test]
    fn normal_line_record_preserves_the_pair_and_has_no_fixed_line_height_hint() {
        let style = TextStyle::default();
        let request = request(TextMeasurementOperation::NormalLineMetrics, "中文", &style);
        let fields = host_text_measurement_transport_fields(request);
        assert_eq!(fields.line_height, 0.0);
        assert_eq!(
            fields.white_space,
            HostTextWhiteSpaceCode::Normal.external_code()
        );
        let metrics = NormalLineMetrics {
            line_height: 28.0,
            baseline_offset: 21.0,
        };
        let mut record = record(Some(HostTextMeasurementResultKind::NormalLineMetrics));
        record.raw_width = None;
        assert!(decode_host_text_measurement(request, record).is_err());
        record.normal_line_metrics = Some(metrics);
        let HostTextMeasurement::NormalLineMetrics(decoded) =
            decode_host_text_measurement(request, record).unwrap()
        else {
            panic!("expected the paired normal-line result")
        };
        assert_eq!(decoded, metrics);
        record.normal_line_metrics.as_mut().unwrap().baseline_offset = f64::INFINITY;
        assert!(decode_host_text_measurement(request, record).is_err());
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
                (19, "normal-line-metrics"),
            ]
        );
    }

    #[test]
    fn external_values_decode_by_declared_result_kind() {
        let style = TextStyle::default();
        let HostTextMeasurement::Metrics(metrics) = decode_host_text_measurement(
            request(TextMeasurementOperation::Measure, "abc", &style),
            HostTextMeasurementRecord {
                raw_width: None,
                ..record(Some(HostTextMeasurementResultKind::Metrics))
            },
        )
        .expect("valid metrics record") else {
            panic!("metrics kind should decode as metrics");
        };
        assert_eq!(
            (metrics.width, metrics.height, metrics.line_count),
            (21.0, 13.0, 2)
        );

        let HostTextMeasurement::Length(length) = decode_host_text_measurement(
            request(TextMeasurementOperation::ComputedLength, "abc", &style),
            HostTextMeasurementRecord {
                raw_width: None,
                ..record(Some(HostTextMeasurementResultKind::Length))
            },
        )
        .expect("valid length record") else {
            panic!("length kind should decode as a length");
        };
        assert_eq!(length, 17.0);

        let HostTextMeasurement::HorizontalExtents { left, right } = decode_host_text_measurement(
            request(TextMeasurementOperation::BBoxX, "abc", &style),
            HostTextMeasurementRecord {
                raw_width: None,
                ..record(Some(HostTextMeasurementResultKind::HorizontalExtents))
            },
        )
        .expect("valid horizontal-extents record") else {
            panic!("horizontal-extents kind should decode as extents");
        };
        assert_eq!((left, right), (3.0, 18.0));

        let HostTextMeasurement::WrappedWithRawWidth { metrics, raw_width } =
            decode_host_text_measurement(
                request(TextMeasurementOperation::WrappedWithRawWidth, "abc", &style),
                record(Some(HostTextMeasurementResultKind::WrappedWithRawWidth)),
            )
            .expect("valid wrapped record")
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
    fn checked_decoder_rejects_unknown_and_operation_mismatched_result_kinds() {
        let style = TextStyle::default();
        let request = request(TextMeasurementOperation::Measure, "abc", &style);

        assert!(decode_host_text_measurement(request, record(None)).is_err());
        assert!(
            decode_host_text_measurement(
                request,
                HostTextMeasurementRecord {
                    raw_width: None,
                    ..record(Some(HostTextMeasurementResultKind::Length))
                },
            )
            .is_err()
        );
    }

    #[test]
    fn checked_decoder_requires_active_fields_and_preserves_presence_rules() {
        let style = TextStyle::default();
        let metrics_request = request(TextMeasurementOperation::Measure, "abc", &style);
        for missing in [
            HostTextMeasurementRecord {
                width: None,
                raw_width: None,
                ..record(Some(HostTextMeasurementResultKind::Metrics))
            },
            HostTextMeasurementRecord {
                height: None,
                raw_width: None,
                ..record(Some(HostTextMeasurementResultKind::Metrics))
            },
            HostTextMeasurementRecord {
                line_count: None,
                raw_width: None,
                ..record(Some(HostTextMeasurementResultKind::Metrics))
            },
        ] {
            assert!(decode_host_text_measurement(metrics_request, missing).is_err());
        }

        let extent_request = request(TextMeasurementOperation::BBoxX, "abc", &style);
        for half_extent in [
            HostTextMeasurementRecord {
                bbox_left: None,
                raw_width: None,
                ..record(Some(HostTextMeasurementResultKind::HorizontalExtents))
            },
            HostTextMeasurementRecord {
                bbox_right: None,
                raw_width: None,
                ..record(Some(HostTextMeasurementResultKind::HorizontalExtents))
            },
        ] {
            assert!(decode_host_text_measurement(extent_request, half_extent).is_err());
        }

        assert!(
            decode_host_text_measurement(
                metrics_request,
                record(Some(HostTextMeasurementResultKind::Metrics)),
            )
            .is_err(),
            "raw_width presence is exclusive to wrapped results"
        );
    }

    #[test]
    fn checked_decoder_rejects_lossy_or_out_of_range_line_counts() {
        let style = TextStyle::default();
        let request = request(TextMeasurementOperation::Measure, "abc", &style);

        for line_count in [-1, i128::MAX, 5] {
            assert!(
                decode_host_text_measurement(
                    request,
                    HostTextMeasurementRecord {
                        line_count: Some(line_count),
                        raw_width: None,
                        ..record(Some(HostTextMeasurementResultKind::Metrics))
                    },
                )
                .is_err()
            );
        }
    }
}
