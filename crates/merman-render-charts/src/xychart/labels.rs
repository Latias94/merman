use super::XyChartRectData;

/// Shared rectangle-label selection and sizing, including Mermaid's legend-marker labels.
pub struct XyChartRectDataLabels<'a> {
    pub items: Vec<XyChartRectDataLabel<'a>>,
    pub font_size: f64,
}

pub struct XyChartRectDataLabel<'a> {
    pub rect: &'a XyChartRectData,
    pub label: &'a str,
}

pub const XY_CHART_DATA_LABEL_INSET_PX: f64 = 10.0;

pub fn rect_data_labels<'a>(
    data: &'a [XyChartRectData],
    labels: &'a [String],
    orientation: &str,
) -> XyChartRectDataLabels<'a> {
    let items = data
        .iter()
        .zip(labels)
        .filter_map(|(rect, label)| {
            (rect.width > 0.0 && rect.height > 0.0).then_some(XyChartRectDataLabel { rect, label })
        })
        .collect::<Vec<_>>();
    let mut min_font = f64::INFINITY;
    for item in &items {
        let fs = if orientation == "horizontal" {
            horizontal_label_font_size(
                item.rect.width,
                item.label,
                item.rect.height * 0.7,
                XY_CHART_DATA_LABEL_INSET_PX,
            )
        } else {
            let denominator = javascript_string_length(item.label) * 0.7;
            let initial = if denominator <= 0.0 {
                0.0
            } else {
                item.rect.width / denominator
            };
            vertical_label_font_size(
                item.rect.width,
                item.rect.height,
                item.label,
                initial,
                XY_CHART_DATA_LABEL_INSET_PX,
            )
        };
        min_font = min_font.min(fs);
    }
    let font_size = if min_font.is_finite() { min_font } else { 0.0 }
        .floor()
        .max(0.0);
    XyChartRectDataLabels { items, font_size }
}

/// Mermaid uses JavaScript's `String#length` for XYChart bar labels, which counts UTF-16 code
/// units rather than Unicode scalar values.
fn javascript_string_length(text: &str) -> f64 {
    text.encode_utf16().count() as f64
}

/// Computes the result of Mermaid's one-pixel decrement loop without iterating once per pixel.
///
/// The upstream renderer decrements a candidate font size until it fits. That is observable for
/// ordinary dimensions, but a finite value such as `1e308` cannot make numerical progress when
/// subtracting one, so the browser algorithm never terminates. The fit predicates are monotonic;
/// solving their upper bound directly preserves the same discrete candidate for normal values and
/// makes extreme finite dimensions terminate in constant time.
fn font_size_after_unit_decrements(initial: f64, maximum_that_fits: f64) -> f64 {
    if !(initial.is_finite() && initial > 0.0) || maximum_that_fits.is_nan() {
        return 0.0;
    }
    if maximum_that_fits >= initial {
        return initial;
    }
    if maximum_that_fits <= 0.0 {
        return 0.0;
    }

    let decrements = (initial - maximum_that_fits).ceil();
    let candidate = initial - decrements;
    if candidate.is_finite() && candidate > 0.0 {
        candidate
    } else {
        0.0
    }
}

fn horizontal_label_font_size(item_width: f64, label: &str, initial: f64, inset_px: f64) -> f64 {
    let denominator = javascript_string_length(label) * 0.7;
    let maximum_that_fits = if denominator > 0.0 {
        (item_width - inset_px) / denominator
    } else {
        f64::INFINITY
    };
    font_size_after_unit_decrements(initial, maximum_that_fits)
}

fn vertical_label_font_size(
    item_width: f64,
    item_height: f64,
    label: &str,
    initial: f64,
    y_offset: f64,
) -> f64 {
    let denominator = javascript_string_length(label) * 0.7;
    let horizontal_maximum = if denominator > 0.0 {
        item_width / denominator
    } else {
        f64::INFINITY
    };
    let maximum_that_fits = horizontal_maximum.min(item_height - y_offset);
    font_size_after_unit_decrements(initial, maximum_that_fits)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn upstream_decrement(initial: f64, maximum_that_fits: f64) -> f64 {
        let mut font_size = initial;
        while font_size > maximum_that_fits && font_size > 0.0 {
            font_size -= 1.0;
        }
        font_size
    }

    #[test]
    fn closed_form_font_sizing_matches_mermaid_for_normal_dimensions() {
        for (initial, maximum_that_fits) in [(10.2, 8.0), (10.0, 8.8), (8.0, 8.0), (0.5, 0.0)] {
            assert_eq!(
                font_size_after_unit_decrements(initial, maximum_that_fits)
                    .floor()
                    .max(0.0),
                upstream_decrement(initial, maximum_that_fits)
                    .floor()
                    .max(0.0),
            );
        }
    }

    #[test]
    fn huge_finite_bar_dimensions_complete_without_a_decrement_loop() {
        let font_size = horizontal_label_font_size(1e308, "123", 7e307, 10.0);

        assert!(font_size.is_finite());
        assert!(font_size > 0.0);
    }

    #[test]
    fn bar_label_length_uses_javascript_utf16_code_units() {
        assert_eq!(javascript_string_length("A"), 1.0);
        assert_eq!(javascript_string_length("\u{1F469}\u{200D}\u{1F4BB}"), 5.0);
    }
}
