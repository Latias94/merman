use merman_model::families::pie::PieDiagramModel;
use merman_render_charts::pie::layout_mermaid_pie;

use crate::Result;
use crate::config::ConfigLookup;
use crate::model::PieDiagramLayout;
use crate::text::{TextMeasurer, TextStyle, svg_title_bbox_vertical_extents_px};
use crate::theme::FamilyTheme;

mod theme;

pub(crate) use merman_render_charts::pie::PieNativeStyle;
pub(crate) use theme::PieThemePlan;

#[derive(Debug)]
#[allow(
    clippy::large_enum_variant,
    reason = "The boxed family artifact already owns the layout and its appearance."
)]
pub(crate) enum PieAppearance {
    Mermaid(PieThemePlan),
    Native(PieNativeStyle),
}

pub(crate) fn layout_pie_diagram_typed_with_paint_plan(
    model: &PieDiagramModel,
    diagram_title: Option<&str>,
    effective_config: &impl ConfigLookup,
    paint_plan: &PieThemePlan,
    measurer: &dyn TextMeasurer,
) -> Result<PieDiagramLayout> {
    layout_mermaid_pie(
        model,
        diagram_title,
        effective_config,
        &paint_plan.style,
        paint_plan.common.font_family(),
        &client_rect_width(measurer),
    )
}

pub(crate) fn prepare_native_pie(
    theme: &FamilyTheme,
    model: &PieDiagramModel,
    diagram_title: Option<&str>,
    effective_config: &impl ConfigLookup,
    measurer: &dyn TextMeasurer,
) -> Result<(PieDiagramLayout, PieNativeStyle)> {
    PieNativeStyle::prepare(
        theme,
        model,
        diagram_title,
        effective_config,
        &client_rect_width(measurer),
        svg_title_bbox_vertical_extents_px,
    )
}

fn client_rect_width(measurer: &dyn TextMeasurer) -> impl Fn(&str, &TextStyle) -> f64 + '_ {
    move |text, style| measurer.measure_svg_text_bounding_client_rect_width_px(text, style)
}

#[cfg(test)]
pub(crate) fn layout_pie_diagram_typed(
    model: &PieDiagramModel,
    diagram_title: Option<&str>,
    effective_config: &impl ConfigLookup,
    measurer: &dyn TextMeasurer,
) -> Result<PieDiagramLayout> {
    let paint_plan = PieThemePlan::baseline(model, effective_config);
    layout_pie_diagram_typed_with_paint_plan(
        model,
        diagram_title,
        effective_config,
        &paint_plan,
        measurer,
    )
}

#[cfg(test)]
mod tests {
    use crate::text::{TextMeasurer, TextMetrics, TextStyle};
    use merman_model::families::pie::{PieDiagramModel, PieSection};
    use std::sync::Mutex;

    #[derive(Default)]
    struct RecordingBoundingClientRectMeasurer {
        calls: Mutex<Vec<String>>,
        font_families: Mutex<Vec<Option<String>>>,
        styles: Mutex<Vec<TextStyle>>,
    }

    impl TextMeasurer for RecordingBoundingClientRectMeasurer {
        fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
            panic!("pie legend and title must use the source-backed browser primitive")
        }

        fn measure_svg_simple_text_bbox_width_px(&self, _text: &str, _style: &TextStyle) -> f64 {
            panic!("getBBox must not stand in for getBoundingClientRect")
        }

        fn measure_svg_text_bounding_client_rect_width_px(
            &self,
            text: &str,
            style: &TextStyle,
        ) -> f64 {
            self.calls
                .lock()
                .expect("measurement calls")
                .push(text.to_string());
            self.font_families
                .lock()
                .expect("font family measurements")
                .push(style.font_family.clone());
            self.styles.lock().unwrap().push(style.clone());
            match text {
                "Legend" => 123.456_789,
                "Title" => 1_000.123_456,
                "  Title  " => 1_100.123_456,
                "\u{a0}Title\u{a0}" => 1_200.123_456,
                other => panic!("unexpected measurement: {other}"),
            }
        }
    }

    #[test]
    fn pie_legend_geometry_constants_match_mermaid() {
        assert_eq!(merman_render_charts::pie::PIE_LEGEND_RECT_SIZE_PX, 18.0);
        assert_eq!(merman_render_charts::pie::PIE_LEGEND_SPACING_PX, 4.0);
    }

    #[test]
    fn native_pie_measures_with_the_emitted_title_and_label_typography() {
        use crate::theme::{BuiltinPreset, ThemeCompiler};

        let theme = ThemeCompiler::new()
            .compile_preset(BuiltinPreset::Paper, None)
            .unwrap();
        let model = PieDiagramModel {
            sections: vec![PieSection {
                label: "Legend".to_owned(),
                value: 1.0,
            }],
            ..Default::default()
        };
        let measurer = RecordingBoundingClientRectMeasurer::default();
        let (layout, style) = super::prepare_native_pie(
            theme.family("pie"),
            &model,
            Some("Title"),
            &serde_json::json!({}),
            &measurer,
        )
        .unwrap();

        let measured = measurer.styles.lock().unwrap();
        assert_eq!(measured.len(), 2);
        assert_ne!(style.label.font_family, style.title.font_family);
        for (measured, emitted) in measured.iter().zip([&style.label, &style.title]) {
            assert_eq!(measured.font_family, emitted.font_family);
            assert_eq!(measured.font_size, emitted.font_size);
            assert_eq!(measured.font_weight, emitted.font_weight);
        }
        assert_eq!(layout.bounds.unwrap().min_x, 225.0 - 1_000.123_456 / 2.0);
    }

    #[test]
    fn pie_outer_radius_uses_the_consumed_config_width() {
        let model = PieDiagramModel::default();
        let measurer = RecordingBoundingClientRectMeasurer::default();
        for (width, radius) in [
            (serde_json::json!(20.5), 195.25),
            (serde_json::json!("20.5px"), 195.0),
            (serde_json::json!("5em"), 187.5),
            (serde_json::json!(0), 185.0),
            (serde_json::json!("calc(1px + 1em)"), 186.0),
        ] {
            let config = serde_json::json!({"themeVariables": {"pieOuterStrokeWidth": width}});
            let layout = super::layout_pie_diagram_typed(&model, None, &config, &measurer)
                .expect("configured Pie radius");
            assert_eq!(layout.outer_radius, radius, "{config}");
            assert_eq!(
                layout.radius, 185.0,
                "slice radius is independent of its outer stroke"
            );
        }
    }

    #[test]
    fn pie_prepared_content_width_excludes_title_expansion() {
        for title in [None, Some("Title")] {
            let model = PieDiagramModel {
                title: title.map(str::to_owned),
                sections: vec![PieSection {
                    label: "Legend".to_owned(),
                    value: 1.0,
                }],
                ..Default::default()
            };
            let config = serde_json::json!({"pie": {"legendPosition": "left"}});
            let plan = super::PieThemePlan::baseline(&model, &config);
            let layout = super::layout_pie_diagram_typed_with_paint_plan(
                &model,
                None,
                &config,
                &plan,
                &RecordingBoundingClientRectMeasurer::default(),
            )
            .expect("prepared Pie geometry");
            let content_width = 512.0 + 123.456_789;
            assert!((plan.content_width().unwrap() - content_width).abs() < 1e-9);
            let bounds = layout.bounds.unwrap();
            if title.is_some() {
                assert_eq!(bounds.min_x, 225.0 - 1_000.123_456 / 2.0);
                assert_eq!(bounds.max_x, 225.0 + 1_000.123_456 / 2.0);
                assert!(bounds.max_x - bounds.min_x > content_width);
            } else {
                assert_eq!(bounds.min_x, 0.0);
                assert!((bounds.max_x - content_width).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn pie_configured_text_sizes_reach_client_rect_measurement_and_bounds() {
        struct SizeSensitiveMeasurer(Mutex<Vec<(String, TextStyle)>>);

        impl TextMeasurer for SizeSensitiveMeasurer {
            fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
                panic!("Pie must retain its client-rect measurement primitive")
            }

            fn measure_svg_text_bounding_client_rect_width_px(
                &self,
                text: &str,
                style: &TextStyle,
            ) -> f64 {
                self.0
                    .lock()
                    .unwrap()
                    .push((text.to_owned(), style.clone()));
                text.chars().count() as f64 * style.font_size
            }
        }

        let title = "TitleTitleTitleTitle";
        let model = PieDiagramModel {
            title: Some(title.to_owned()),
            sections: vec![PieSection {
                label: "Legend".to_owned(),
                value: 1.0,
            }],
            ..Default::default()
        };
        for (base_css, title_css, legend_css, title_size, legend_size) in [
            ("20px", "60px", "40px", 60.0, 40.0),
            ("20px", "24pt", "18pt", 32.0, 24.0),
            ("20px", "2PC", "1pc", 32.0, 16.0),
            ("20px", "1in", "0.5in", 96.0, 48.0),
            ("20px", "2.54cm", "1.27cm", 96.0, 48.0),
            ("20px", "25.4mm", "12.7mm", 96.0, 48.0),
            ("20px", "101.6q", "50.8q", 96.0, 48.0),
            ("20px", "2em", "150%", 40.0, 30.0),
            ("20px", "0", "0px", 0.0, 0.0),
            ("20px", "60", "invalid", 20.0, 20.0),
            ("20px", "inherit", "unset", 20.0, 20.0),
            ("0px", "inherit", "100%", 0.0, 0.0),
            ("0px", "-1px", "potato", 0.0, 0.0),
            ("0px", "24", "auto", 0.0, 0.0),
            ("24pt", "-1px", "potato", 32.0, 32.0),
            ("0.5px", "inherit", "100%", 0.5, 0.5),
            ("0px", "14px", "17px", 14.0, 17.0),
        ] {
            let measurer = SizeSensitiveMeasurer(Mutex::new(Vec::new()));
            let config = serde_json::json!({"themeVariables": {
                "fontFamily": "Bound Pie",
                "fontSize": base_css,
                "pieTitleTextSize": title_css,
                "pieLegendTextSize": legend_css
            }});
            let layout = super::layout_pie_diagram_typed(&model, None, &config, &measurer)
                .expect("configured Pie");
            let calls = measurer.0.lock().unwrap();
            assert_eq!(calls.len(), 2);
            assert_eq!(calls[0].0, "Legend");
            assert_eq!(calls[0].1.font_size, legend_size, "{legend_css}");
            assert_eq!(calls[1].0, title);
            assert_eq!(calls[1].1.font_size, title_size, "{title_css}");
            assert!(
                calls
                    .iter()
                    .all(|(_, style)| { style.font_family.as_deref() == Some("Bound Pie") })
            );
            let bounds = layout.bounds.unwrap();
            let title_width = title.len() as f64 * title_size;
            let legend_width = "Legend".len() as f64 * legend_size;
            assert_eq!(bounds.min_x, (225.0 - title_width / 2.0).min(0.0));
            assert_eq!(
                bounds.max_x,
                (512.0 + legend_width).max(225.0 + title_width / 2.0)
            );
        }
    }

    #[test]
    fn pie_legend_and_title_use_exact_bounding_client_rect_results() {
        let measurer = RecordingBoundingClientRectMeasurer::default();
        let legend_model = PieDiagramModel {
            sections: vec![PieSection {
                label: "Legend".to_string(),
                value: 1.0,
            }],
            ..Default::default()
        };
        let legend_layout =
            super::layout_pie_diagram_typed(&legend_model, None, &serde_json::json!({}), &measurer)
                .expect("legend layout");
        let legend_max_x = legend_layout.bounds.expect("legend bounds").max_x;
        assert!((legend_max_x - (512.0 + 123.456_789)).abs() < 1e-12);

        let title_model = PieDiagramModel {
            title: Some("Title".to_string()),
            ..Default::default()
        };
        let title_layout = super::layout_pie_diagram_typed(
            &title_model,
            None,
            &serde_json::json!({"pie": {"legendPosition": "top"}}),
            &measurer,
        )
        .expect("title layout");
        let title_max_x = title_layout.bounds.expect("title bounds").max_x;
        assert!((title_max_x - (225.0 + 1_000.123_456 / 2.0)).abs() < 1e-12);
        assert_eq!(
            *measurer.calls.lock().expect("measurement calls"),
            ["Legend".to_string(), "Title".to_string()]
        );
    }

    #[test]
    fn empty_pie_bounds_still_contain_the_painted_circle() {
        let measurer = RecordingBoundingClientRectMeasurer::default();
        for config in [
            serde_json::json!({}),
            serde_json::json!({"pie": {"legendPosition": "top"}}),
        ] {
            let layout = super::layout_pie_diagram_typed(
                &PieDiagramModel::default(),
                None,
                &config,
                &measurer,
            )
            .expect("empty pie layout");
            let bounds = layout.bounds.expect("empty pie bounds");
            assert!(bounds.min_x <= layout.center_x - layout.outer_radius);
            assert!(bounds.max_x >= layout.center_x * 2.0);
        }
    }

    #[test]
    fn pie_frontmatter_title_preserves_boundary_whitespace_for_layout_measurement() {
        for (title, expected_max_x) in [
            ("  Title  ", 225.0 + 1_100.123_456 / 2.0),
            ("\u{a0}Title\u{a0}", 225.0 + 1_200.123_456 / 2.0),
        ] {
            let measurer = RecordingBoundingClientRectMeasurer::default();
            let layout = super::layout_pie_diagram_typed(
                &PieDiagramModel::default(),
                Some(title),
                &serde_json::json!({"pie": {"legendPosition": "top"}}),
                &measurer,
            )
            .expect("title layout");

            assert_eq!(layout.title.as_deref(), Some(title));
            let max_x = layout.bounds.expect("title bounds").max_x;
            assert!((max_x - expected_max_x).abs() < 1e-12);
            assert_eq!(
                *measurer.calls.lock().expect("measurement calls"),
                [title.to_string()]
            );
        }
    }
}
