use crate::Result;
use crate::model::{Bounds, PieDiagramLayout, PieLegendItemLayout, PieSliceLayout};
use crate::text::TextMeasurer;
use merman_model::families::pie::{PieDiagramModel, PieSection};

pub(crate) const PIE_LEGEND_RECT_SIZE_PX: f64 = 18.0;
pub(crate) const PIE_LEGEND_SPACING_PX: f64 = 4.0;

mod config;
mod theme;

pub(crate) use config::{PieConfigView, PieLegendPosition};
pub(crate) use theme::prepared_text::{PieTextCssEmission, PieTextRole};
pub(crate) use theme::{PIE_TITLE_CLASS, PieThemePlan, PieThemeReceipt};

fn visible_pie_sections(model: &PieDiagramModel) -> (f64, Vec<&PieSection>) {
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
            .filter(|section| section.value.is_finite() && section.value > 0.0)
            .filter(|section| (section.value / total) * 100.0 >= 1.0)
            .collect()
    } else {
        Vec::new()
    };
    (total, sections)
}

pub(crate) fn pie_legend_text(label: &str, value: f64, show_data: bool) -> String {
    if show_data {
        format!(
            "{label} [{}]",
            crate::number_format::canonical_number(value)
        )
    } else {
        label.to_owned()
    }
}

fn polar_xy(radius: f64, angle: f64) -> (f64, f64) {
    // Mermaid pie charts use a "12 o'clock is zero" convention with y increasing downwards.
    let x = radius * angle.sin();
    let y = -radius * angle.cos();
    (x, y)
}

fn fmt_number(v: f64) -> String {
    if !v.is_finite() {
        return "0".to_string();
    }
    if v.abs() < 0.0005 {
        return "0".to_string();
    }
    let mut r = (v * 1000.0).round() / 1000.0;
    if r.abs() < 0.0005 {
        r = 0.0;
    }
    let mut s = format!("{r:.3}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" { "0".to_string() } else { s }
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
    let _ = (
        model.title.as_deref(),
        model.acc_title.as_deref(),
        model.acc_descr.as_deref(),
    );

    // Mermaid@11.16 `packages/mermaid/src/diagrams/pie/pieRenderer.ts` constants.
    let margin: f64 = 40.0;
    let legend_rect_size = PIE_LEGEND_RECT_SIZE_PX;
    let legend_spacing = PIE_LEGEND_SPACING_PX;

    let center: f64 = 225.0;
    let radius: f64 = 185.0;
    let outer_radius = radius + 1.0;
    let cfg = PieConfigView::new(effective_config).layout_settings();
    let title = model.title.as_deref().or(diagram_title);
    let label_radius = radius.max(0.0) * cfg.text_position;
    let legend_step_y: f64 = legend_rect_size + legend_spacing;
    let legend_position = cfg.legend_position;
    let total_legend_height = (model.sections.len() as f64) * legend_step_y;
    let centered_legend_start_y = -(legend_step_y * (model.sections.len().max(1) as f64)) / 2.0;
    let legend_start_y = match legend_position {
        PieLegendPosition::Top => -radius,
        PieLegendPosition::Bottom => radius + legend_step_y,
        _ => centered_legend_start_y,
    };

    let (total, pie_sections) = visible_pie_sections(model);

    let mut slices: Vec<PieSliceLayout> = Vec::new();
    if total.is_finite() && total > 0.0 {
        // Mermaid@11.16 `packages/mermaid/src/diagrams/pie/pieRenderer.ts`:
        //
        // - filter out values < 1% (based on the original total)
        // - preserve input order before D3 pie() computes angles (`sort(null)`)
        // - angles are normalized over the filtered set (so drawn slices fill the whole circle)
        // - percentage labels are still computed using the original total
        let pie_total: f64 = pie_sections.iter().map(|s| s.value).sum();
        if !pie_sections.is_empty() && pie_total.is_finite() && pie_total > 0.0 {
            if pie_sections.len() == 1 {
                let s = pie_sections[0];
                let fill = pie_fill_for(paint_plan, &s.label)?;
                let (tx, ty) = polar_xy(label_radius, std::f64::consts::PI);
                let percent = ((100.0 * (s.value / total)).max(0.0)).round() as i64;
                slices.push(PieSliceLayout {
                    label: s.label.clone(),
                    value: s.value,
                    start_angle: 0.0,
                    end_angle: std::f64::consts::TAU,
                    is_full_circle: true,
                    percent,
                    text_x: tx,
                    text_y: ty,
                    fill,
                });
            } else {
                let mut start = 0.0;
                for s in pie_sections {
                    let frac = (s.value / pie_total).max(0.0);
                    let delta = (frac * std::f64::consts::TAU).max(0.0);
                    let end = start + delta;
                    let mid = (start + end) / 2.0;
                    let (tx, ty) = polar_xy(label_radius, mid);
                    let fill = pie_fill_for(paint_plan, &s.label)?;
                    let percent = ((100.0 * (s.value / total)).max(0.0)).round() as i64;
                    if percent != 0 {
                        slices.push(PieSliceLayout {
                            label: s.label.clone(),
                            value: s.value,
                            start_angle: start,
                            end_angle: end,
                            is_full_circle: false,
                            percent,
                            text_x: tx,
                            text_y: ty,
                            fill,
                        });
                    }
                    start = end;
                }
            }
        }
    }

    // Lock the color scale domain based on the drawn slices first, then compute legend colors in
    // the original section order (this matches Mermaid's zero-slice behavior).
    let mut legend_items: Vec<PieLegendItemLayout> = Vec::new();
    for (i, sec) in model.sections.iter().enumerate() {
        let y = legend_start_y + (i as f64) * legend_step_y;
        let fill = pie_fill_for(paint_plan, &sec.label)?;
        legend_items.push(PieLegendItemLayout {
            label: sec.label.clone(),
            value: sec.value,
            fill,
            y,
        });
    }

    let legend_style = paint_plan.css_binding().legend_measurement_style();
    let title_style = paint_plan.css_binding().title_measurement_style();
    let mut max_legend_width: f64 = 0.0;
    for sec in &model.sections {
        let label = if paint_plan.text_measurement_requested() {
            pie_legend_text(&sec.label, sec.value, model.show_data)
        } else if model.show_data {
            format!("{} [{}]", sec.label, fmt_number(sec.value))
        } else {
            sec.label.clone()
        };
        let trimmed = label.trim_end();
        // Mermaid 11.16 measures pie legend text via a single SVG `<text>` node's
        // `getBoundingClientRect().width`.
        let w = if trimmed.is_empty() {
            0.0
        } else {
            let width =
                measurer.measure_svg_text_bounding_client_rect_width_px(trimmed, &legend_style);
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
            let width =
                measurer.measure_svg_text_bounding_client_rect_width_px(title, &title_style);
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

    let chart_and_legend_width = match legend_position {
        PieLegendPosition::Top => (base_w + margin).max(1.0),
        PieLegendPosition::Bottom => (base_w + margin).max(1.0),
        PieLegendPosition::Left => (base_w + margin + legend_extra_width).max(1.0),
        PieLegendPosition::Center => (base_w + margin).max(1.0),
        PieLegendPosition::Right => {
            if model.sections.is_empty() {
                f64::NEG_INFINITY
            } else {
                (base_w + margin + legend_extra_width).max(1.0)
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
    let min_x = title_left.min(0.0);
    let max_x = chart_and_legend_width.max(title_right);
    let max_x = if model.sections.is_empty() {
        max_x.max(base_w)
    } else {
        max_x
    };

    Ok(PieDiagramLayout {
        bounds: Some(Bounds {
            min_x,
            min_y: 0.0,
            max_x,
            max_y: height,
        }),
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
    })
}

fn pie_fill_for(paint_plan: &PieThemePlan, label: &str) -> Result<String> {
    paint_plan
        .fill_for(label)
        .map(str::to_string)
        .ok_or_else(|| crate::Error::InvalidModel {
            message: format!("Pie theme plan is missing slice label `{label}`"),
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
        let mut model = PieDiagramModel::default();
        model.title = Some(title.to_owned());
        model.sections = vec![PieSection {
            label: "Legend".to_owned(),
            value: 1.0,
        }];
        for (title_css, legend_css, title_size, legend_size) in [
            ("60px", "40px", 60.0, 40.0),
            ("2em", "150%", 40.0, 30.0),
            ("0", "0px", 0.0, 0.0),
            ("60", "invalid", 20.0, 20.0),
            ("inherit", "unset", 20.0, 20.0),
        ] {
            let measurer = SizeSensitiveMeasurer(Mutex::new(Vec::new()));
            let config = serde_json::json!({"themeVariables": {
                "fontFamily": "Bound Pie",
                "fontSize": "20px",
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
        let mut legend_model = PieDiagramModel::default();
        legend_model.sections = vec![PieSection {
            label: "Legend".to_string(),
            value: 1.0,
        }];
        let legend_layout =
            super::layout_pie_diagram_typed(&legend_model, None, &serde_json::json!({}), &measurer)
                .expect("legend layout");
        let legend_max_x = legend_layout.bounds.expect("legend bounds").max_x;
        assert!((legend_max_x - (512.0 + 123.456_789)).abs() < 1e-12);

        let mut title_model = PieDiagramModel::default();
        title_model.title = Some("Title".to_string());
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

    #[test]
    fn pie_layout_measurement_uses_the_resolved_font_stack() {
        use crate::DiagramFamilyId;
        use crate::diagram_theme::{
            DiagramThemeCompiler, DiagramThemeSpec, FontStack, ThemeTextStyle, TypographySpec,
        };
        use crate::resources::RenderResourcePolicy;
        use merman_model::families::pie::PieSection;
        use merman_model::presentation::PresentationConfig;

        let mut model = PieDiagramModel::default();
        model.title = Some("Title".to_string());
        model.sections = ["Legend"]
            .into_iter()
            .map(|label| PieSection {
                label: label.to_string(),
                value: 1.0,
            })
            .collect();
        let font_stack =
            FontStack::new(["Pie Layout", "sans-serif"]).expect("valid Pie layout font stack");
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_family_style(
                    DiagramFamilyId::PIE,
                    ThemeTextStyle::default().with_font_stack(font_stack.clone()),
                ),
            ))
            .expect("compile Pie typography theme")
            .resolve(DiagramFamilyId::PIE);
        let config = PresentationConfig::empty_object();
        let work_meter = crate::resources::OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let plan = super::PieThemePlan::resolve_with_title(
            &model,
            &config,
            Some(&theme),
            model.title.as_deref(),
            &work_meter,
        )
        .expect("resolve Pie typography theme");
        let measurer = RecordingBoundingClientRectMeasurer::default();

        let _ = super::layout_pie_diagram_typed_with_paint_plan(
            &model,
            None,
            config.as_value(),
            &plan,
            &measurer,
        )
        .expect("layout Pie typography theme");

        assert_eq!(
            *measurer
                .font_families
                .lock()
                .expect("font family measurements"),
            [Some(font_stack.as_css()), Some(font_stack.as_css())]
        );
    }

    #[test]
    fn pie_role_measurement_uses_final_sizes_weights_and_styles_with_source_precedence() {
        use crate::DiagramFamilyId;
        use crate::diagram_theme::{
            DiagramThemeCompiler, DiagramThemeSpec, FontStyle, Specified, ThemeRule, ThemeRuleSet,
            ThemeStylePatch, ThemeTarget, ThemeTextStyle, TypographySpec,
        };
        use crate::resources::{OperationWorkMeter, RenderResourcePolicy};

        let mut model = PieDiagramModel::default();
        model.title = Some("Title".to_owned());
        model.sections = vec![PieSection {
            label: "Legend".to_owned(),
            value: 1.0,
        }];
        let mut rules = ThemeRuleSet::default();
        for (target, size, weight, style) in [
            (ThemeTarget::Title, 30.0, 650, FontStyle::Italic),
            (ThemeTarget::Legend, 12.0, 500, FontStyle::Normal),
        ] {
            let mut patch = ThemeStylePatch::default();
            patch.typography.font_size_px = Specified::Value(size);
            patch.typography.font_weight = Specified::Value(weight);
            patch.typography.font_style = Specified::Value(style);
            rules = rules.with_rule(ThemeRule::new(target, patch).for_family(DiagramFamilyId::PIE));
        }
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(rules).with_typography(
                    TypographySpec::default()
                        .with_default(ThemeTextStyle::default().with_font_size_px(20.0).unwrap())
                        .with_family_style(
                            DiagramFamilyId::PIE,
                            ThemeTextStyle::default().with_font_size_px(24.0).unwrap(),
                        ),
                ),
            )
            .unwrap()
            .resolve(DiagramFamilyId::PIE);
        let config = merman_core::MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "%%{init: {\"themeVariables\": {\"pieLegendTextSize\": \"1.5em\"}}}%%\npie title Title\n  \"Legend\" : 1\n",
                merman_core::ParseOptions::strict(),
            )
            .unwrap()
            .unwrap().into_presentation()
            .metadata()
            .effective_config
            .clone();
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let plan = super::PieThemePlan::resolve(&model, &config, Some(&theme), &meter).unwrap();
        let measurer = RecordingBoundingClientRectMeasurer::default();
        super::layout_pie_diagram_typed_with_paint_plan(
            &model,
            None,
            config.as_value(),
            &plan,
            &measurer,
        )
        .unwrap();
        let styles = measurer.styles.lock().unwrap();
        assert_eq!(styles[0].font_size, 36.0);
        assert_eq!(styles[0].font_weight.as_deref(), Some("500"));
        assert_eq!(styles[0].font_style.as_deref(), Some("normal"));
        assert_eq!(styles[1].font_size, 30.0);
        assert_eq!(styles[1].font_weight.as_deref(), Some("650"));
        assert_eq!(styles[1].font_style.as_deref(), Some("italic"));
        assert_eq!(plan.css_binding().legend_text_size, "1.5em");
        assert_eq!(plan.css_binding().title_text_size, "30px");
    }
}
