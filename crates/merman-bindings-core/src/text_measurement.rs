use crate::{HostTextMeasurement, TextMetrics};

/// Stable result-shape discriminator shared by all host text-measurement transports.
pub use merman::svg::TextMeasurementResultKind as HostTextMeasurementResultKind;

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
    use crate::TextMeasurementOperation;

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
