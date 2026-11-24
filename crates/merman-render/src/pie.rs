use crate::Result;
use crate::model::{Bounds, PieDiagramLayout, PieLegendItemLayout, PieSliceLayout};
use crate::text::{TextMeasurer, TextStyle, svg_title_bbox_vertical_extents_px};
use merman_model::families::pie::{PieDiagramModel, PieSection};
use merman_theme_contract::v2::DataPalette;

pub(crate) const PIE_LEGEND_RECT_SIZE_PX: f64 = 18.0;
pub(crate) const PIE_LEGEND_SPACING_PX: f64 = 4.0;
pub(crate) const PIE_TITLE_BASELINE_Y_PX: f64 = -200.0;
pub(crate) const PIE_MARGIN_PX: f64 = 40.0;
pub(crate) const PIE_TITLE_CLASS: &str = "pieTitleText";

mod config;
mod native_theme;
mod theme;

pub(crate) use config::{PieConfigView, PieLegendPosition};
pub(crate) use native_theme::PieNativeStyle;
pub(crate) use theme::{PieTextRole, PieThemePlan};

#[derive(Debug)]
#[allow(
    clippy::large_enum_variant,
    reason = "The boxed family artifact already owns the layout and its appearance."
)]
pub(crate) enum PieAppearance {
    Mermaid(PieThemePlan),
    Native(PieNativeStyle),
}

fn visible_pie_sections(model: &PieDiagramModel) -> (f64, Vec<(usize, &PieSection)>) {
    let total: f64 = model
        .sections
        .iter()
        .filter(|section| section.value.is_finite() && section.value >= 0.0)
        .map(|section| section.value)
        .sum();
    let sections = if total.is_finite() && total > 0.0 {
        model
            .sections
            .iter()
            .enumerate()
            .filter(|(_, section)| section.value.is_finite() && section.value > 0.0)
            .filter(|(_, section)| (section.value / total) * 100.0 >= 1.0)
            .collect()
    } else {
        Vec::new()
    };
    (total, sections)
}

pub(crate) fn pie_legend_text(label: &str, value: f64, show_data: bool) -> String {
    if show_data {
        format!("{label} [{}]", ryu_js::Buffer::new().format(value))
    } else {
        label.to_owned()
    }
}

pub(crate) fn native_legend_baseline_y(style: &TextStyle) -> f64 {
    let (ascent, descent) = svg_title_bbox_vertical_extents_px(style);
    (PIE_LEGEND_RECT_SIZE_PX + ascent - descent) / 2.0
}

fn polar_xy(radius: f64, angle: f64) -> (f64, f64) {
    // Mermaid pie charts use a "12 o'clock is zero" convention with y increasing downwards.
    let x = radius * angle.sin();
    let y = -radius * angle.cos();
    (x, y)
}

#[cfg(test)]
pub(crate) fn layout_pie_diagram_typed(
    model: &PieDiagramModel,
    diagram_title: Option<&str>,
    effective_config: &serde_json::Value,
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

pub(crate) fn layout_pie_diagram_typed_with_paint_plan(
    model: &PieDiagramModel,
    diagram_title: Option<&str>,
    effective_config: &serde_json::Value,
    paint_plan: &PieThemePlan,
    measurer: &dyn TextMeasurer,
) -> Result<PieDiagramLayout> {
    let geometry = layout_pie(
        model,
        diagram_title,
        effective_config,
        PieLayoutPaint::Mermaid(paint_plan),
        measurer,
    )?;
    paint_plan.prepare_content_width(geometry.content_width)?;
    Ok(geometry.layout)
}

#[derive(Clone, Copy)]
enum PieLayoutPaint<'a> {
    Mermaid(&'a PieThemePlan),
    Native {
        data: &'a DataPalette,
        title: &'a TextStyle,
        label: &'a TextStyle,
    },
}

impl PieLayoutPaint<'_> {
    fn fill(self, ordinal: usize, label: &str) -> Result<String> {
        match self {
            Self::Mermaid(plan) => {
                plan.fill_for(label)
                    .map(str::to_owned)
                    .ok_or_else(|| crate::Error::InvalidModel {
                        message: format!("Pie theme plan is missing slice label `{label}`"),
                    })
            }
            Self::Native { data, .. } => Ok(data.fill(ordinal).to_string()),
        }
    }
}

struct PieLayoutGeometry {
    layout: PieDiagramLayout,
    content_width: f64,
    slice_ordinals: Vec<usize>,
}

fn layout_pie(
    model: &PieDiagramModel,
    diagram_title: Option<&str>,
    effective_config: &serde_json::Value,
    paint: PieLayoutPaint<'_>,
    measurer: &dyn TextMeasurer,
) -> Result<PieLayoutGeometry> {
    // Mermaid@11.16 `packages/mermaid/src/diagrams/pie/pieRenderer.ts` constants.
    let legend_rect_size = PIE_LEGEND_RECT_SIZE_PX;
    let legend_spacing = PIE_LEGEND_SPACING_PX;
    let center: f64 = 225.0;
    let radius: f64 = 185.0;

    let (mermaid_legend_style, mermaid_title_style);
    let (outer_radius, legend_style, title_style) = match paint {
        PieLayoutPaint::Mermaid(plan) => {
            let css = plan.css_binding();
            mermaid_legend_style = css.legend_measurement_style();
            mermaid_title_style = css.title_measurement_style();
            (
                radius + css.outer_stroke_width_number / 2.0,
                &mermaid_legend_style,
                &mermaid_title_style,
            )
        }
        PieLayoutPaint::Native { title, label, .. } => (radius, label, title),
    };
    let native = matches!(paint, PieLayoutPaint::Native { .. });
    let cfg = PieConfigView::new(effective_config).layout_settings();
    let title = model.title.as_deref().or(diagram_title);
    let label_radius = radius.max(0.0) * cfg.text_position;
    let mut legend_step_y = legend_rect_size + legend_spacing;
    if native {
        let (ascent, descent) = svg_title_bbox_vertical_extents_px(legend_style);
        legend_step_y = legend_step_y.max((ascent + descent).ceil() + legend_spacing);
    }
    let legend_position = cfg.legend_position;
    let total_legend_height = (model.sections.len() as f64) * legend_step_y;
    let centered_legend_start_y = -(legend_step_y * (model.sections.len().max(1) as f64)) / 2.0;
    let legend_start_y = match legend_position {
        PieLegendPosition::Top => -radius,
        PieLegendPosition::Bottom => radius + legend_step_y,
        _ => centered_legend_start_y,
    };

    let (total, pie_sections) = visible_pie_sections(model);

    // D3 normalizes angles over the visible sectors in input order. The displayed percentages
    // retain the original total, including values that fall below Mermaid's 1% threshold.
    let mut slices = Vec::new();
    let mut slice_ordinals = Vec::new();
    let pie_total: f64 = pie_sections.iter().map(|(_, section)| section.value).sum();
    let is_full_circle = pie_sections.len() == 1;
    if pie_total.is_finite() && pie_total > 0.0 {
        let mut start_angle = 0.0;
        for (ordinal, section) in pie_sections {
            let end_angle = if is_full_circle {
                std::f64::consts::TAU
            } else {
                start_angle + (section.value / pie_total) * std::f64::consts::TAU
            };
            let (text_x, text_y) = polar_xy(label_radius, (start_angle + end_angle) / 2.0);
            slices.push(PieSliceLayout {
                label: section.label.clone(),
                value: section.value,
                start_angle,
                end_angle,
                is_full_circle,
                percent: (100.0 * (section.value / total)).round() as i64,
                text_x,
                text_y,
                fill: paint.fill(ordinal, &section.label)?,
            });
            if native {
                slice_ordinals.push(ordinal);
            }
            start_angle = end_angle;
        }
    }

    // The legend retains the original category order, including categories without a sector.
    let mut legend_items = Vec::with_capacity(model.sections.len());
    for (i, sec) in model.sections.iter().enumerate() {
        let y = legend_start_y + (i as f64) * legend_step_y;
        let fill = paint.fill(i, &sec.label)?;
        legend_items.push(PieLegendItemLayout {
            label: sec.label.clone(),
            value: sec.value,
            fill,
            y,
        });
    }

    let mut max_legend_width: f64 = 0.0;
    for sec in &model.sections {
        let label = pie_legend_text(&sec.label, sec.value, model.show_data);
        let trimmed = label.trim_end();
        // Mermaid 11.16 measures pie legend text via a single SVG `<text>` node's
        // `getBoundingClientRect().width`.
        let w = if trimmed.is_empty() {
            0.0
        } else {
            let width =
                measurer.measure_svg_text_bounding_client_rect_width_px(trimmed, legend_style);
            if legend_style.font_size == 0.0 {
                0.0
            } else {
                width
            }
        };
        max_legend_width = max_legend_width.max(w);
    }

    let title_width = title
        .map(|title| {
            let width = measurer.measure_svg_text_bounding_client_rect_width_px(title, title_style);
            if title_style.font_size == 0.0 {
                0.0
            } else {
                width
            }
        })
        .unwrap_or(0.0);

    let base_w: f64 = center * 2.0;
    let legend_extra_width = legend_rect_size + legend_spacing + max_legend_width;
    let centered_legend_x = -max_legend_width / 2.0 - (legend_rect_size + legend_spacing);

    let padded_chart_width = base_w + PIE_MARGIN_PX;
    let chart_and_legend_width = match legend_position {
        PieLegendPosition::Top | PieLegendPosition::Bottom | PieLegendPosition::Center => {
            padded_chart_width
        }
        PieLegendPosition::Left => padded_chart_width + legend_extra_width,
        PieLegendPosition::Right => {
            if model.sections.is_empty() {
                f64::NEG_INFINITY
            } else {
                padded_chart_width + legend_extra_width
            }
        }
    };
    let height = match legend_position {
        PieLegendPosition::Top | PieLegendPosition::Bottom => {
            (base_w + total_legend_height).max(1.0)
        }
        PieLegendPosition::Left | PieLegendPosition::Center | PieLegendPosition::Right => {
            base_w.max(1.0)
        }
    };

    let legend_x = match legend_position {
        PieLegendPosition::Top | PieLegendPosition::Bottom | PieLegendPosition::Center => {
            centered_legend_x
        }
        PieLegendPosition::Left => -radius - (legend_rect_size + legend_spacing),
        PieLegendPosition::Right => 12.0 * legend_rect_size,
    };

    let title_left = base_w / 2.0 - title_width / 2.0;
    let title_right = base_w / 2.0 + title_width / 2.0;
    let mut bounds = Bounds {
        min_x: title_left.min(0.0),
        min_y: 0.0,
        max_x: chart_and_legend_width.max(title_right),
        max_y: height,
    };
    if model.sections.is_empty() {
        bounds.max_x = bounds.max_x.max(base_w);
    }
    if native {
        if title.is_some_and(|title| !title.is_empty()) {
            let (ascent, _) = svg_title_bbox_vertical_extents_px(title_style);
            bounds.min_y = bounds.min_y.min(center + PIE_TITLE_BASELINE_Y_PX - ascent);
        }
        if let Some(last) = legend_items.last() {
            let (ascent, descent) = svg_title_bbox_vertical_extents_px(legend_style);
            let text_baseline = native_legend_baseline_y(legend_style);
            let left = center + legend_x;
            bounds.min_x = bounds.min_x.min(left);
            bounds.max_x = bounds.max_x.max(left + legend_extra_width);
            bounds.min_y = bounds
                .min_y
                .min(center + legend_start_y + (text_baseline - ascent).min(0.0));
            bounds.max_y = bounds
                .max_y
                .max(center + last.y + (text_baseline + descent).max(legend_rect_size));
        }
    }

    Ok(PieLayoutGeometry {
        layout: PieDiagramLayout {
            bounds: Some(bounds),
            title: title.map(str::to_owned),
            center_x: center,
            center_y: center,
            radius,
            outer_radius,
            legend_x,
            legend_start_y,
            legend_step_y,
            slices,
            legend_items,
        },
        content_width: chart_and_legend_width,
        slice_ordinals,
    })
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
        assert_eq!(super::PIE_LEGEND_RECT_SIZE_PX, 18.0);
        assert_eq!(super::PIE_LEGEND_SPACING_PX, 4.0);
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
        let (layout, style) = super::PieNativeStyle::prepare(
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
