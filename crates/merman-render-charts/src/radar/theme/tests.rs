use serde_json::json;

use super::super::test_support::FALLBACK_C_SCALE;
use super::{RadarCssBinding, RadarSeriesPaintPlan};

#[test]
fn radar_binding_resolves_style_roles() {
    let cfg = json!({
        "fontFamily": "Ignored, sans-serif",
        "themeVariables": {
            "fontFamily": "\"ibm plex sans\", arial, sans-serif",
            "fontSize": 18,
            "textColor": "#101010",
            "lineColor": "#111111",
            "errorBkgColor": "#121212",
            "errorTextColor": "#131313",
            "titleColor": "#202020",
            "cScale0": "#303030",
            "radar": {
                "axisColor": "#404040",
                "axisStrokeWidth": 2,
                "axisLabelFontSize": 12,
                "graticuleColor": "#505050",
                "graticuleOpacity": 0.3,
                "graticuleStrokeWidth": 1,
                "legendFontSize": 12,
                "curveOpacity": 0.5,
                "curveStrokeWidth": 2
            }
        },
        "radar": {
            "axisColor": "#606060",
            "axisStrokeWidth": 4,
            "axisLabelFontSize": 14,
            "graticuleColor": "#707070",
            "graticuleOpacity": 0.8,
            "graticuleStrokeWidth": 5,
            "legendFontSize": 16,
            "curveOpacity": 0.9,
            "curveStrokeWidth": 6
        }
    });

    let radar = RadarCssBinding::resolve(&cfg);

    assert_eq!(radar.text_color, "#101010");
    assert_eq!(radar.line_color, "#111111");
    assert_eq!(radar.error_bkg_color, "#121212");
    assert_eq!(radar.error_text_color, "#131313");
    assert_eq!(radar.title_color, "#202020");
    assert_eq!(radar.axis_color, "#606060");
    assert_eq!(radar.axis_stroke_width, 4.0);
    assert_eq!(radar.axis_label_font_size.css(), "14px");
    assert_eq!(radar.graticule_color, "#707070");
    assert_eq!(radar.graticule_opacity, 0.8);
    assert_eq!(radar.graticule_stroke_width, 5.0);
    assert_eq!(radar.legend_font_size.css(), "16px");
    assert_eq!(radar.curve_opacity, 0.9);
    assert_eq!(radar.curve_stroke_width, 6.0);
    let series = RadarSeriesPaintPlan::baseline(&cfg, &FALLBACK_C_SCALE);
    let series_colors = series.colors();
    assert_eq!(series_colors[0], "#303030");
    assert_eq!(series_colors[11], "hsl(210, 100%, 76.2745098039%)");
}

#[test]
fn radar_binding_uses_default_style_roles() {
    let cfg = json!({});

    let radar = RadarCssBinding::resolve(&cfg);

    assert_eq!(radar.text_color, "#333");
    assert_eq!(radar.line_color, "#333333");
    assert_eq!(radar.error_bkg_color, "#552222");
    assert_eq!(radar.error_text_color, "#552222");
    assert_eq!(radar.title_color, "#333");
    assert_eq!(radar.axis_color, "#333333");
    assert_eq!(radar.axis_stroke_width, 2.0);
    assert_eq!(radar.axis_label_font_size.css(), "12px");
    assert_eq!(radar.graticule_color, "#DEDEDE");
    assert_eq!(radar.graticule_opacity, 0.3);
    assert_eq!(radar.graticule_stroke_width, 1.0);
    assert_eq!(radar.legend_font_size.css(), "12px");
    assert_eq!(radar.curve_opacity, 0.5);
    assert_eq!(radar.curve_stroke_width, 2.0);
    let series = RadarSeriesPaintPlan::baseline(&cfg, &FALLBACK_C_SCALE);
    let series_colors = series.colors();
    assert_eq!(series_colors[0], "hsl(240, 100%, 76.2745098039%)");
}
