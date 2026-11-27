use kurbo::Shape;
use merman_model::{
    MermaidConfig, OperationControl,
    families::xy_chart::{
        XyChartAxisModel, XyChartDiagramRenderModel, XyChartPlotModel, XyChartPlotType,
    },
    family::DiagramFamilyId,
    presentation::PresentationConfig,
    resources::{InputResourcePolicy, ResourceProfile},
};
use merman_render_core::{
    resources::{OperationWorkMeter, RenderResourcePolicy},
    root_svg::RootViewportContext,
    svg_css::SvgId,
    text::{TextMeasurer, TextMetrics, TextStyle},
};
use serde_json::{Value, json};

use super::{
    XyChartDiagramLayout, XyChartDrawableElem, XyChartSvgContext, XyChartTextData,
    palette::mermaid_plot_palette, prepare_xychart_diagram, render_xychart_svg,
    theme::XyChartTypographyThemePlan,
};

fn presentation_config(value: Value) -> PresentationConfig {
    PresentationConfig::from_config_controlled(
        MermaidConfig::from_value(value),
        &InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput),
        &OperationControl::new(),
    )
    .unwrap()
    .unwrap()
}

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

#[test]
fn xychart_background_preserves_the_authored_paint_attribute() {
    let fill = "url(\"#swatch-#333;\")\nred";
    let config = presentation_config(json!({"themeVariables": {"xyChart": {
        "backgroundColor": fill
    }}}));
    let model: XyChartDiagramRenderModel = serde_json::from_value(json!({
        "orientation": "vertical",
        "xAxis": {"type": "band"},
        "yAxis": {"type": "linear"},
        "plots": []
    }))
    .unwrap();
    let (layout, style) =
        prepare_xychart_diagram(&model, None, &config, None, false, &UserUnitMeasurer).unwrap();
    let work_meter = OperationWorkMeter::new(RenderResourcePolicy::interactive());
    let diagram_id = SvgId::new("xychart").unwrap();
    let svg = render_xychart_svg(
        &layout,
        &model,
        &style,
        &XyChartSvgContext {
            diagram_id,
            viewport: RootViewportContext::new(DiagramFamilyId::XY_CHART, diagram_id),
            work_meter: &work_meter,
            native_theme: None,
        },
    )
    .unwrap();
    let doc = roxmltree::Document::parse(&svg).unwrap();
    let background = doc
        .descendants()
        .find(|node| node.attribute("class") == Some("background"))
        .unwrap();
    assert_eq!(background.attribute("fill"), Some(fill));
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
    let config = presentation_config(json!({ "xyChart": {
        "width": 700, "height": height, "showLegend": legend
    }}));
    prepare_xychart_diagram(&model, None, &config, None, false, &UserUnitMeasurer)
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
fn xychart_common_css_keeps_source_colors_with_its_resolved_font_stack() {
    let config = presentation_config(json!({
        "fontFamily": "Inter, sans-serif",
        "themeVariables": { "textColor": "#123456", "lineColor": "#abcdef" }
    }));
    let typography = XyChartTypographyThemePlan::resolve(&config);
    let mut css = String::new();
    typography.common_css().write(&mut css, "chart").unwrap();
    for (selector, expected) in [
        ("#chart{", ["font-family:Inter,sans-serif", "fill:#123456"]),
        ("#chart .marker{", ["fill:#abcdef", "stroke:#abcdef"]),
    ] {
        let (_, rule) = css.split_once(selector).unwrap();
        let (declarations, _) = rule.split_once('}').unwrap();
        for property in expected {
            assert!(
                declarations
                    .split(';')
                    .any(|declaration| declaration == property),
                "{selector} {property}"
            );
        }
    }
}
