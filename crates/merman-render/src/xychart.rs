use merman_model::families::xy_chart::XyChartDiagramRenderModel;
use merman_render_charts::xychart::XyChartSeriesPaintPlan;
use serde_json::Value;

use crate::Result;
use crate::chart_palette::{plot_color_from_palette, resolve_xychart_plot_palette};
use crate::config::{ConfigLookup, selected_config_string};
use crate::model::XyChartDiagramLayout;
use crate::text::TextMeasurer;
use crate::theme::FamilyTheme;

mod theme;

pub(crate) use merman_render_charts::xychart::{
    XY_CHART_DATA_LABEL_INSET_PX, XyChartTextRole, rect_data_labels,
};
pub(crate) use theme::XyChartTypographyThemePlan;

fn mermaid_plot_palette(config: &impl ConfigLookup) -> Vec<String> {
    let palette = selected_config_string(config, &["themeVariables.xyChart.plotColorPalette"]);
    resolve_xychart_plot_palette(
        config
            .get("theme")
            .and_then(Value::as_str)
            .unwrap_or("default"),
        palette.map(|(_, value)| value),
    )
}

pub(crate) fn resolve_series_paint(
    config: &impl ConfigLookup,
    model: &XyChartDiagramRenderModel,
    native: Option<&FamilyTheme>,
) -> XyChartSeriesPaintPlan {
    let palette = if native.is_none() {
        mermaid_plot_palette(config)
    } else {
        Vec::new()
    };
    XyChartSeriesPaintPlan::new(config, model, native, |index| {
        plot_color_from_palette(&palette, index)
    })
}

pub(crate) fn layout_xychart_diagram_typed(
    model: &XyChartDiagramRenderModel,
    diagram_title: Option<&str>,
    series_paint: &XyChartSeriesPaintPlan,
    typography_theme: &XyChartTypographyThemePlan,
    native: Option<&FamilyTheme>,
    text_measurer: &dyn TextMeasurer,
) -> Result<(XyChartDiagramLayout, Vec<usize>)> {
    merman_render_charts::xychart::layout_xychart_diagram_typed(
        model,
        diagram_title,
        series_paint,
        typography_theme.layout_typography(),
        native,
        text_measurer,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{XyChartDrawableElem, XyChartTextData};
    use crate::text::{TextMetrics, TextStyle};
    use kurbo::Shape;
    use merman_model::families::xy_chart::{XyChartAxisModel, XyChartPlotModel, XyChartPlotType};
    use serde_json::json;

    #[test]
    fn missing_and_empty_palette_keep_named_theme_fallback() {
        let missing = mermaid_plot_palette(&json!({ "theme": "dark" }));
        let empty = mermaid_plot_palette(&json!({
            "theme": "dark", "themeVariables": { "xyChart": { "plotColorPalette": " , " } }
        }));
        assert_eq!(missing, empty);
        assert_eq!(missing[0], "#3498db");
        let explicit = mermaid_plot_palette(&json!({
            "theme": "dark", "themeVariables": { "xyChart": { "plotColorPalette": "var(--series), currentColor" } }
        }));
        assert_eq!(explicit, ["var(--series)", "currentColor"]);
        let nested = mermaid_plot_palette(&json!({
            "theme": "neo", "themeVariables": {
                "xyChart": { "plotColorPalette": "#001122, #334455" }
            }
        }));
        assert_eq!(nested, ["#001122", "#334455"]);
    }

    struct UserUnitMeasurer;

    impl TextMeasurer for UserUnitMeasurer {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            TextMetrics {
                width: text.encode_utf16().count() as f64 * style.font_size,
                height: style.font_size,
                line_count: 1,
            }
        }
    }

    fn layout(title: &str, orientation: &str, height: f64, legend: bool) -> XyChartDiagramLayout {
        let model = XyChartDiagramRenderModel {
            semantic: merman_model::families::xy_chart::XyChartDiagramModel {
                orientation: orientation.to_string(),
                title: Some(title.to_string()),
                acc_title: None,
                acc_descr: None,
                x_axis: XyChartAxisModel::Band {
                    title: String::new(),
                    categories: vec!["a".to_string(), "b".to_string(), "c".to_string()],
                },
                y_axis: XyChartAxisModel::Linear {
                    title: "A fairly wide y-axis title".to_string(),
                    min: Some(0.0),
                    max: Some(100000.0),
                },
                plots: vec![XyChartPlotModel {
                    plot_type: XyChartPlotType::Bar,
                    title: Some("series".to_string()),
                    values: vec![10.0, 20.0, 30.0],
                    data: vec![
                        ("a".to_string(), Some(10.0)),
                        ("b".to_string(), Some(20.0)),
                        ("c".to_string(), Some(30.0)),
                    ],
                    point_labels: Vec::new(),
                }],
            },
            display: Default::default(),
        };
        let config = crate::test_support::presentation_config(json!({ "xyChart": {
            "width": 700, "height": height, "showLegend": legend
        }}));
        let series_paint = resolve_series_paint(config.as_value(), &model, None);
        let typography = XyChartTypographyThemePlan::resolve(&config);
        layout_xychart_diagram_typed(
            &model,
            None,
            &series_paint,
            &typography,
            None,
            &UserUnitMeasurer,
        )
        .unwrap()
        .0
    }

    fn title(layout: &XyChartDiagramLayout) -> Option<&XyChartTextData> {
        layout.drawables.iter().find_map(|drawable| match drawable {
            XyChartDrawableElem::Text { group_texts, data } if group_texts == &["chart-title"] => {
                data.first()
            }
            _ => None,
        })
    }

    fn plot_span(layout: &XyChartDiagramLayout) -> (f64, f64) {
        let axis = if layout.chart_orientation == "horizontal" {
            "top-axis"
        } else {
            "bottom-axis"
        };
        let path = layout
            .drawables
            .iter()
            .find_map(|drawable| match drawable {
                XyChartDrawableElem::Path { group_texts, data }
                    if group_texts == &[axis, "axis-line"] =>
                {
                    data.first()
                }
                _ => None,
            })
            .expect("axis spanning the plot");
        let bounds = kurbo::BezPath::from_svg(&path.path).unwrap().bounding_box();
        (bounds.x0, bounds.x1)
    }

    #[test]
    fn xychart_title_centers_on_plot_for_both_orientations_and_legend_modes() {
        for orientation in ["vertical", "horizontal"] {
            for legend in [false, true] {
                let layout = layout("Title", orientation, 500.0, legend);
                let (start, end) = plot_span(&layout);
                assert!(start > 0.0);
                assert!((title(&layout).unwrap().x - (start + end) / 2.0).abs() < 1e-9);
                if legend {
                    assert!(end < layout.width);
                } else {
                    assert_eq!(end, layout.width);
                }
            }
        }
    }

    #[test]
    fn xychart_wide_title_clamps_to_chart_and_can_span_legend_columns() {
        let wide = "x".repeat(32);
        let without_legend = layout(&wide, "vertical", 500.0, false);
        assert_eq!(title(&without_legend).unwrap().x, 380.0);

        let with_legend = layout(&wide, "vertical", 500.0, true);
        let (start, end) = plot_span(&with_legend);
        let label = title(&with_legend).unwrap();
        assert_eq!(label.x, (start + end) / 2.0);
        assert!(label.x + 320.0 > end);
        assert!(label.x + 320.0 <= with_legend.width);
        let legend_marker = with_legend
            .drawables
            .iter()
            .find_map(|drawable| match drawable {
                XyChartDrawableElem::Rect { group_texts, data }
                    if group_texts == &["legend", "markers"] =>
                {
                    data.first()
                }
                _ => None,
            })
            .unwrap();
        assert!(label.y + 10.0 <= legend_marker.y - 10.0);

        let oversized = layout(&"x".repeat(40), "vertical", 500.0, false);
        assert_eq!(title(&oversized).unwrap().x, oversized.width / 2.0);
    }

    #[test]
    fn xychart_title_needs_space_after_the_plot_reservation() {
        for orientation in ["vertical", "horizontal"] {
            let short = layout("Title", orientation, 60.0, false);
            assert!(title(&short).is_none());
            let mut bars = 0;
            for drawable in &short.drawables {
                if let XyChartDrawableElem::Rect { group_texts, data } = drawable
                    && group_texts == &["plot", "bar-plot-0"]
                {
                    bars += data.len();
                    for bar in data {
                        assert!(bar.y + bar.height <= short.height + 1e-9);
                    }
                }
            }
            assert_eq!(bars, 3);
            assert!(title(&layout("Title", orientation, 100.0, false)).is_some());
        }
    }

    #[test]
    fn xychart_measures_title_axes_and_legend_with_the_configured_font() {
        use std::sync::Mutex;
        struct RecordingFont(Mutex<Vec<String>>);
        impl TextMeasurer for RecordingFont {
            fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
                assert_eq!(style.font_family.as_deref(), Some("Inter,sans-serif"));
                self.0.lock().unwrap().push(text.to_owned());
                UserUnitMeasurer.measure(text, style)
            }
        }
        let parsed = merman_core::MermaidEngine::new().parse_diagram_for_render_model_sync(
            "xychart\ntitle Chart\nx-axis Categories [One, Two, Three]\ny-axis Amount 0 --> 40\nline Series [10, 30, 20]",
            merman_core::ParseOptions::strict(),
        ).unwrap().unwrap().into_presentation();
        let merman_model::presentation::RenderSemanticModel::XyChart(model) = parsed.model() else {
            panic!("expected XYChart render model");
        };
        let measurer = RecordingFont(Mutex::new(Vec::new()));
        let config = crate::test_support::presentation_config(json!({
            "fontFamily":"Inter, sans-serif", "xyChart":{"showLegend":true},
        }));
        let series = resolve_series_paint(config.as_value(), model, None);
        let typography = XyChartTypographyThemePlan::resolve(&config);
        layout_xychart_diagram_typed(model, None, &series, &typography, None, &measurer).unwrap();
        let text = measurer.0.lock().unwrap();
        for expected in ["Chart", "Categories", "Amount", "One", "Series"] {
            assert!(
                text.iter().any(|text| text == expected),
                "missing measurement: {expected}"
            );
        }
    }
}
