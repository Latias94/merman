use merman_render_core::{
    config::{
        config_bool, config_f64, config_string, config_theme_font_size_css_or_root_number_px,
    },
    number_format::canonicalize_number,
};
use serde_json::Value;

const DEFAULT_CHART_WIDTH: f64 = 500.0;
const DEFAULT_CHART_HEIGHT: f64 = 500.0;
const DEFAULT_TITLE_PADDING: f64 = 10.0;
const DEFAULT_TITLE_FONT_SIZE: f64 = 20.0;
const DEFAULT_QUADRANT_PADDING: f64 = 5.0;
const DEFAULT_X_AXIS_LABEL_PADDING: f64 = 5.0;
const DEFAULT_Y_AXIS_LABEL_PADDING: f64 = 5.0;
const DEFAULT_X_AXIS_LABEL_FONT_SIZE: f64 = 16.0;
const DEFAULT_Y_AXIS_LABEL_FONT_SIZE: f64 = 16.0;
const DEFAULT_QUADRANT_LABEL_FONT_SIZE: f64 = 16.0;
const DEFAULT_QUADRANT_TEXT_TOP_PADDING: f64 = 5.0;
const DEFAULT_POINT_TEXT_PADDING: f64 = 5.0;
const DEFAULT_POINT_LABEL_FONT_SIZE: f64 = 12.0;
const DEFAULT_POINT_RADIUS: f64 = 5.0;
const DEFAULT_X_AXIS_POSITION: &str = "top";
const DEFAULT_Y_AXIS_POSITION: &str = "left";
const DEFAULT_QUADRANT_INTERNAL_BORDER_STROKE_WIDTH: f64 = 1.0;
const DEFAULT_QUADRANT_EXTERNAL_BORDER_STROKE_WIDTH: f64 = 2.0;

#[derive(Debug, Clone)]
pub struct QuadrantChartLayoutSettings {
    pub(super) chart_width: f64,
    pub(super) chart_height: f64,
    pub(super) title_padding: f64,
    pub title_font_size: f64,
    pub(super) quadrant_padding: f64,
    pub(super) x_axis_label_padding: f64,
    pub(super) y_axis_label_padding: f64,
    pub x_axis_label_font_size: f64,
    pub y_axis_label_font_size: f64,
    pub quadrant_label_font_size: f64,
    pub(super) quadrant_text_top_padding: f64,
    pub(super) point_text_padding: f64,
    pub point_label_font_size: f64,
    pub(super) point_radius: f64,
    pub(super) x_axis_position: String,
    pub(super) y_axis_position: String,
    pub quadrant_internal_border_stroke_width: f64,
    pub quadrant_external_border_stroke_width: f64,
}

impl QuadrantChartLayoutSettings {
    pub fn from_effective_config(effective_config: &Value) -> Self {
        let quadrant_config = effective_config
            .get("quadrantChart")
            .unwrap_or(&Value::Null);
        let quadrant_f64 = |key| config_f64(quadrant_config, &[key]);
        let quadrant_string = |key| config_string(quadrant_config, &[key]);
        Self {
            chart_width: quadrant_f64("chartWidth").unwrap_or(DEFAULT_CHART_WIDTH),
            chart_height: quadrant_f64("chartHeight").unwrap_or(DEFAULT_CHART_HEIGHT),
            title_padding: quadrant_f64("titlePadding").unwrap_or(DEFAULT_TITLE_PADDING),
            title_font_size: quadrant_f64("titleFontSize").unwrap_or(DEFAULT_TITLE_FONT_SIZE),
            quadrant_padding: quadrant_f64("quadrantPadding").unwrap_or(DEFAULT_QUADRANT_PADDING),
            x_axis_label_padding: quadrant_f64("xAxisLabelPadding")
                .unwrap_or(DEFAULT_X_AXIS_LABEL_PADDING),
            y_axis_label_padding: quadrant_f64("yAxisLabelPadding")
                .unwrap_or(DEFAULT_Y_AXIS_LABEL_PADDING),
            x_axis_label_font_size: quadrant_f64("xAxisLabelFontSize")
                .unwrap_or(DEFAULT_X_AXIS_LABEL_FONT_SIZE),
            y_axis_label_font_size: quadrant_f64("yAxisLabelFontSize")
                .unwrap_or(DEFAULT_Y_AXIS_LABEL_FONT_SIZE),
            quadrant_label_font_size: quadrant_f64("quadrantLabelFontSize")
                .unwrap_or(DEFAULT_QUADRANT_LABEL_FONT_SIZE),
            quadrant_text_top_padding: quadrant_f64("quadrantTextTopPadding")
                .unwrap_or(DEFAULT_QUADRANT_TEXT_TOP_PADDING),
            point_text_padding: quadrant_f64("pointTextPadding")
                .unwrap_or(DEFAULT_POINT_TEXT_PADDING),
            point_label_font_size: quadrant_f64("pointLabelFontSize")
                .unwrap_or(DEFAULT_POINT_LABEL_FONT_SIZE),
            point_radius: quadrant_f64("pointRadius").unwrap_or(DEFAULT_POINT_RADIUS),
            x_axis_position: quadrant_string("xAxisPosition")
                .unwrap_or_else(|| DEFAULT_X_AXIS_POSITION.to_string()),
            y_axis_position: quadrant_string("yAxisPosition")
                .unwrap_or_else(|| DEFAULT_Y_AXIS_POSITION.to_string()),
            quadrant_internal_border_stroke_width: quadrant_f64(
                "quadrantInternalBorderStrokeWidth",
            )
            .unwrap_or(DEFAULT_QUADRANT_INTERNAL_BORDER_STROKE_WIDTH),
            quadrant_external_border_stroke_width: quadrant_f64(
                "quadrantExternalBorderStrokeWidth",
            )
            .unwrap_or(DEFAULT_QUADRANT_EXTERNAL_BORDER_STROKE_WIDTH),
        }
    }
}

pub(super) fn inherited_font_size_px(effective_config: &Value) -> f64 {
    canonicalize_number(config_theme_font_size_css_or_root_number_px(
        effective_config,
        16.0,
    ))
}

pub(super) fn use_max_width(effective_config: &Value) -> bool {
    effective_config
        .get("quadrantChart")
        .filter(|config| {
            !config
                .as_object()
                .is_some_and(|map| map.contains_key("$ref"))
        })
        .and_then(|config| config_bool(config, &["useMaxWidth"]))
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn quadrantchart_layout_settings_preserve_defaults() {
        let cfg = json!({});
        let settings = QuadrantChartLayoutSettings::from_effective_config(&cfg);

        assert_eq!(settings.chart_width, DEFAULT_CHART_WIDTH);
        assert_eq!(settings.chart_height, DEFAULT_CHART_HEIGHT);
        assert_eq!(settings.title_padding, DEFAULT_TITLE_PADDING);
        assert_eq!(settings.title_font_size, DEFAULT_TITLE_FONT_SIZE);
        assert_eq!(settings.quadrant_padding, DEFAULT_QUADRANT_PADDING);
        assert_eq!(settings.x_axis_label_padding, DEFAULT_X_AXIS_LABEL_PADDING);
        assert_eq!(settings.y_axis_label_padding, DEFAULT_Y_AXIS_LABEL_PADDING);
        assert_eq!(
            settings.x_axis_label_font_size,
            DEFAULT_X_AXIS_LABEL_FONT_SIZE
        );
        assert_eq!(
            settings.y_axis_label_font_size,
            DEFAULT_Y_AXIS_LABEL_FONT_SIZE
        );
        assert_eq!(
            settings.quadrant_label_font_size,
            DEFAULT_QUADRANT_LABEL_FONT_SIZE
        );
        assert_eq!(
            settings.quadrant_text_top_padding,
            DEFAULT_QUADRANT_TEXT_TOP_PADDING
        );
        assert_eq!(settings.point_text_padding, DEFAULT_POINT_TEXT_PADDING);
        assert_eq!(
            settings.point_label_font_size,
            DEFAULT_POINT_LABEL_FONT_SIZE
        );
        assert_eq!(settings.point_radius, DEFAULT_POINT_RADIUS);
        assert_eq!(settings.x_axis_position, DEFAULT_X_AXIS_POSITION);
        assert_eq!(settings.y_axis_position, DEFAULT_Y_AXIS_POSITION);
        assert_eq!(
            settings.quadrant_internal_border_stroke_width,
            DEFAULT_QUADRANT_INTERNAL_BORDER_STROKE_WIDTH
        );
        assert_eq!(
            settings.quadrant_external_border_stroke_width,
            DEFAULT_QUADRANT_EXTERNAL_BORDER_STROKE_WIDTH
        );
    }

    #[test]
    fn quadrantchart_layout_settings_project_configured_values() {
        let cfg = json!({
            "quadrantChart": {
                "chartWidth": 640,
                "chartHeight": "360",
                "titlePadding": 12,
                "titleFontSize": "22",
                "quadrantPadding": 8,
                "xAxisLabelPadding": 9,
                "yAxisLabelPadding": "10",
                "xAxisLabelFontSize": 17,
                "yAxisLabelFontSize": "18",
                "quadrantLabelFontSize": 19,
                "quadrantTextTopPadding": "6",
                "pointTextPadding": 7,
                "pointLabelFontSize": "13",
                "pointRadius": 11,
                "xAxisPosition": "bottom",
                "yAxisPosition": "right",
                "quadrantInternalBorderStrokeWidth": "3",
                "quadrantExternalBorderStrokeWidth": 4
            }
        });
        let settings = QuadrantChartLayoutSettings::from_effective_config(&cfg);

        assert_eq!(settings.chart_width, 640.0);
        assert_eq!(settings.chart_height, 360.0);
        assert_eq!(settings.title_padding, 12.0);
        assert_eq!(settings.title_font_size, 22.0);
        assert_eq!(settings.quadrant_padding, 8.0);
        assert_eq!(settings.x_axis_label_padding, 9.0);
        assert_eq!(settings.y_axis_label_padding, 10.0);
        assert_eq!(settings.x_axis_label_font_size, 17.0);
        assert_eq!(settings.y_axis_label_font_size, 18.0);
        assert_eq!(settings.quadrant_label_font_size, 19.0);
        assert_eq!(settings.quadrant_text_top_padding, 6.0);
        assert_eq!(settings.point_text_padding, 7.0);
        assert_eq!(settings.point_label_font_size, 13.0);
        assert_eq!(settings.point_radius, 11.0);
        assert_eq!(settings.x_axis_position, "bottom");
        assert_eq!(settings.y_axis_position, "right");
        assert_eq!(settings.quadrant_internal_border_stroke_width, 3.0);
        assert_eq!(settings.quadrant_external_border_stroke_width, 4.0);
    }

    #[test]
    fn quadrantchart_layout_settings_project_values_in_ref_objects() {
        let cfg = json!({"quadrantChart": {"$ref": "#/defs/quadrantChart", "chartWidth": 640}});
        let settings = QuadrantChartLayoutSettings::from_effective_config(&cfg);
        assert_eq!(settings.chart_width, 640.0);
    }

    #[test]
    fn quadrantchart_render_settings_project_use_max_width() {
        let cfg = json!({
            "quadrantChart": {
                "useMaxWidth": false
            }
        });
        assert!(!use_max_width(&cfg));
    }

    #[test]
    fn quadrantchart_ref_config_uses_render_default() {
        let cfg = json!({
            "quadrantChart": {
                "$ref": "#/defs/quadrantChart",
                "useMaxWidth": false
            }
        });
        assert!(use_max_width(&cfg));
    }
}
