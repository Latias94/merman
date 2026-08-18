use super::super::*;
use crate::pie::{PIE_LEGEND_RECT_SIZE_PX, PIE_LEGEND_SPACING_PX};
use merman_core::diagrams::pie::PieDiagramRenderModel;
const EMPTY_PIE_WIDTH: f64 = 225.0;
const EMPTY_PIE_HEIGHT: f64 = 450.0;

fn pie_legend_rect_style(fill: &str) -> String {
    // Mermaid emits legend colors via inline `style` in rgb() form for default themes.
    // The compare tooling ignores `style`, but we keep this for human inspection parity.
    let color = super::super::util::cssom_color_value(fill);
    format!("fill: {color}; stroke: {color};")
}

fn pie_polar_xy(radius: f64, angle: f64) -> (f64, f64) {
    let x = radius * angle.sin();
    let y = -radius * angle.cos();
    (x, y)
}

fn pie_slice_class(effective_config: &serde_json::Value, label: &str) -> String {
    let highlight = crate::config::config_string(effective_config, &["pie", "highlightSlice"])
        .unwrap_or_default();
    let mut class_name = "pieCircle".to_string();
    if highlight == "hover" {
        class_name.push_str(" highlightedOnHover");
    } else if highlight == label {
        class_name.push_str(" highlighted");
    }
    class_name
}

fn emitted_attribute_matches(
    output: &str,
    element_start: usize,
    attribute_prefix: &str,
    expected_value: &str,
) -> bool {
    output
        .get(element_start..)
        .and_then(|element| element.split_once(attribute_prefix))
        .and_then(|(_, value)| value.split_once('"'))
        .is_some_and(|(value, _)| value == expected_value)
}

fn empty_pie_root_viewport_fallback(
    model: &PieDiagramRenderModel,
    min_x: f64,
    min_y: f64,
    width: f64,
    height: f64,
) -> Option<(root_svg::DiagramBounds, root_svg::RootMaxWidth)> {
    if !model.sections.is_empty() {
        return None;
    }

    let computed_root_is_finite =
        min_x.is_finite() && min_y.is_finite() && width.is_finite() && height.is_finite();
    if computed_root_is_finite {
        return None;
    }

    // Mermaid can produce an invalid `-Infinity` viewport when no sections are drawn. Root
    // Viewport requires finite bounds, but valid title-widened layout bounds remain authoritative.
    Some((
        root_svg::DiagramBounds::from_view_box(0.0, 0.0, EMPTY_PIE_WIDTH, EMPTY_PIE_HEIGHT),
        root_svg::RootMaxWidth::SvgNumber(EMPTY_PIE_WIDTH),
    ))
}

fn render_pie_slices(
    out: &mut impl SvgOutput,
    slices: &[crate::model::PieSliceLayout],
    radius: f64,
    inner_radius: f64,
    effective_config: &serde_json::Value,
    paint_plan: &crate::pie::PieSlicePaintPlan,
    mut paint_receipt: Option<&mut crate::pie::PieSlicePaintReceipt>,
) -> Result<()> {
    for (slice_index, slice) in slices.iter().enumerate() {
        let slice_class = pie_slice_class(effective_config, &slice.label);
        let emitted_fill = slice.fill.as_str();
        let escaped_fill = escape_xml(emitted_fill);
        let element_start = out.len();
        if slice.is_full_circle {
            let d = if inner_radius > 0.0 {
                format!(
                    "M0,-{r}A{r},{r},0,1,1,0,{r}A{r},{r},0,1,1,0,-{r}M0,-{ir}A{ir},{ir},0,1,0,0,{ir}A{ir},{ir},0,1,0,0,-{ir}Z",
                    r = fmt(radius),
                    ir = fmt(inner_radius)
                )
            } else {
                format!(
                    "M0,-{r}A{r},{r},0,1,1,0,{r}A{r},{r},0,1,1,0,-{r}Z",
                    r = fmt(radius)
                )
            };
            let _ = write!(
                out,
                r#"<path d="{d}" fill="{fill}" class="{class}"/>"#,
                d = d,
                fill = &escaped_fill,
                class = escape_xml(&slice_class)
            );
        } else {
            let (x0, y0) = pie_polar_xy(radius, slice.start_angle);
            let (x1, y1) = pie_polar_xy(radius, slice.end_angle);
            let large = if (slice.end_angle - slice.start_angle) > std::f64::consts::PI {
                1
            } else {
                0
            };
            let d = if inner_radius > 0.0 {
                let (ix0, iy0) = pie_polar_xy(inner_radius, slice.start_angle);
                let (ix1, iy1) = pie_polar_xy(inner_radius, slice.end_angle);
                format!(
                    "M{x0},{y0}A{r},{r},0,{large},1,{x1},{y1}L{ix1},{iy1}A{ir},{ir},0,{large},0,{ix0},{iy0}Z",
                    x0 = fmt(x0),
                    y0 = fmt(y0),
                    r = fmt(radius),
                    large = large,
                    x1 = fmt(x1),
                    y1 = fmt(y1),
                    ix1 = fmt(ix1),
                    iy1 = fmt(iy1),
                    ir = fmt(inner_radius),
                    ix0 = fmt(ix0),
                    iy0 = fmt(iy0)
                )
            } else {
                format!(
                    "M{x0},{y0}A{r},{r},0,{large},1,{x1},{y1}L0,0Z",
                    x0 = fmt(x0),
                    y0 = fmt(y0),
                    r = fmt(radius),
                    large = large,
                    x1 = fmt(x1),
                    y1 = fmt(y1)
                )
            };
            let _ = write!(
                out,
                r#"<path d="{d}" fill="{fill}" class="{class}"/>"#,
                d = d,
                fill = &escaped_fill,
                class = escape_xml(&slice_class)
            );
        }
        out.checkpoint()?;
        if let Some(receipt) = paint_receipt.as_deref_mut() {
            let terminal_fill =
                emitted_attribute_matches(out.as_str(), element_start, r#" fill=""#, &escaped_fill)
                    .then_some(emitted_fill);
            receipt.record_slice(paint_plan, slice_index, slice.label.as_str(), terminal_fill);
        }
    }

    Ok(())
}

#[cfg(test)]
pub(crate) fn render_pie_diagram_svg_model(
    layout: &PieDiagramLayout,
    model: &PieDiagramRenderModel,
    effective_config: &serde_json::Value,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let paint_plan = crate::pie::PieSlicePaintPlan::baseline(model, effective_config);
    render_pie_diagram_svg_model_with_paint_plan(
        layout,
        model,
        &paint_plan,
        effective_config,
        options,
    )
}

pub(crate) fn render_pie_diagram_svg_model_with_paint_plan(
    layout: &PieDiagramLayout,
    model: &PieDiagramRenderModel,
    paint_plan: &crate::pie::PieSlicePaintPlan,
    effective_config: &serde_json::Value,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");
    let diagram_id_esc = escape_xml(diagram_id);

    let bounds = layout.bounds.clone().unwrap_or(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 450.0,
        max_y: 450.0,
    });
    let vb_min_x = bounds.min_x;
    let vb_min_y = bounds.min_y;
    let vb_w = (bounds.max_x - bounds.min_x).max(1.0);
    let vb_h = (bounds.max_y - bounds.min_y).max(1.0);

    let render_settings = crate::pie::PieConfigView::new(effective_config).render_settings();
    let (root_bounds, root_max_width) = empty_pie_root_viewport_fallback(
        model, vb_min_x, vb_min_y, vb_w, vb_h,
    )
    .unwrap_or_else(|| {
        (
            root_svg::DiagramBounds::from_view_box(vb_min_x, vb_min_y, vb_w, vb_h),
            root_svg::RootMaxWidth::CssSixSignificant(vb_w),
        )
    });
    let root_spec = root_svg::RootViewportSpec::mermaid(root_bounds, render_settings.use_max_width)
        .with_max_width(root_max_width);

    let mut out = BoundedSvgOutput::new(options.work_meter());
    let aria_labelledby = model
        .acc_title
        .as_deref()
        .map(|_| format!("chart-title-{diagram_id}"));
    let aria_describedby = model
        .acc_descr
        .as_deref()
        .map(|_| format!("chart-desc-{diagram_id}"));
    let root_document = root_svg::RootViewportContext::new(crate::DiagramFamilyId::PIE, diagram_id)
        .write_open(
            &mut out,
            root_spec,
            root_svg::RootChrome {
                aria_labelledby: aria_labelledby.as_deref(),
                aria_describedby: aria_describedby.as_deref(),
                dom: root_svg::RootDomProfile {
                    style_viewbox_order: root_svg::SvgRootStyleViewBoxOrder::ViewBoxThenStyle,
                    fixed_height_placement: root_svg::SvgRootFixedHeightPlacement::AfterXmlns,
                    fixed_style_placement: root_svg::RootStylePlacement::Tail,
                    trailing_newline: false,
                    ..Default::default()
                },
                ..root_svg::RootChrome::new(diagram_id, "pie")
            },
        )?;

    if let Some(t) = model.acc_title.as_deref() {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{id}">{text}</title>"#,
            id = diagram_id_esc,
            text = escape_xml(t)
        );
    }
    if let Some(d) = model.acc_descr.as_deref() {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{id}">{text}</desc>"#,
            id = diagram_id_esc,
            text = escape_xml(d)
        );
    }
    out.checkpoint()?;

    out.push_str("<style>");
    out.checkpoint()?;
    let css = pie_css(diagram_id, effective_config);
    out.push_str(&css);
    drop(css);
    out.checkpoint()?;
    out.push_str(r#"</style><g/>"#);
    out.checkpoint()?;

    let _ = write!(
        &mut out,
        r#"<g transform="translate({x},{y})">"#,
        x = fmt(layout.center_x),
        y = fmt(layout.center_y)
    );

    let legend_position = render_settings.legend_position;
    let pie_offset_x = if legend_position == crate::pie::PieLegendPosition::Left {
        (vb_w - 490.0).max(0.0)
    } else {
        0.0
    };
    let pie_offset_y = if legend_position == crate::pie::PieLegendPosition::Top {
        layout.legend_step_y * ((layout.legend_items.len() as f64) + 1.0)
    } else {
        0.0
    };
    let has_pie_offset = pie_offset_x != 0.0 || pie_offset_y != 0.0;
    if has_pie_offset {
        let _ = write!(
            &mut out,
            r#"<g transform="translate({x},{y})">"#,
            x = fmt(pie_offset_x),
            y = fmt(pie_offset_y)
        );
    } else {
        out.push_str("<g>");
    }

    let _ = write!(
        &mut out,
        r#"<circle cx="0" cy="0" r="{r}" class="pieOuterCircle"/>"#,
        r = fmt(layout.outer_radius)
    );
    out.checkpoint()?;

    let inner_radius = render_settings.donut_hole * layout.radius;
    let mut paint_receipt =
        paint_plan.begin_terminal_receipt(layout.slices.iter().map(|slice| slice.label.as_str()));
    render_pie_slices(
        &mut out,
        &layout.slices,
        layout.radius,
        inner_radius,
        effective_config,
        paint_plan,
        paint_receipt.as_mut(),
    )?;

    for slice in &layout.slices {
        let _ = write!(
            &mut out,
            r#"<text transform="translate({x},{y})" class="slice" style="text-anchor: middle;">{text}</text>"#,
            x = fmt(slice.text_x),
            y = fmt(slice.text_y),
            text = escape_xml(&format!("{}%", slice.percent))
        );
        out.checkpoint()?;
    }

    out.push_str("</g>");

    match layout.title.as_deref() {
        Some(t) => {
            let _ = write!(
                &mut out,
                r#"<text x="0" y="{y}" class="pieTitleText">{text}</text>"#,
                y = fmt(-200.0),
                text = escape_xml(t)
            );
        }
        None => {
            let _ = write!(
                &mut out,
                r#"<text x="0" y="{y}" class="pieTitleText"/>"#,
                y = fmt(-200.0)
            );
        }
    }
    out.checkpoint()?;

    let legend_rect_size = PIE_LEGEND_RECT_SIZE_PX;
    let legend_text_x = legend_rect_size + PIE_LEGEND_SPACING_PX;

    for (legend_index, item) in layout.legend_items.iter().enumerate() {
        let _ = write!(
            &mut out,
            r#"<g class="legend" transform="translate({x},{y})">"#,
            x = fmt(layout.legend_x),
            y = fmt(item.y)
        );
        out.checkpoint()?;
        let emitted_fill = item.fill.as_str();
        let style = pie_legend_rect_style(emitted_fill);
        let escaped_style = escape_xml(&style);
        let element_start = out.len();
        let _ = write!(
            &mut out,
            r#"<rect width="{size}" height="{size}" style="{style}"/>"#,
            size = fmt(legend_rect_size),
            style = &escaped_style
        );
        out.checkpoint()?;
        let terminal_fill =
            emitted_attribute_matches(out.as_str(), element_start, r#" style=""#, &escaped_style)
                .then_some(emitted_fill);
        let text = if model.show_data {
            format!("{} [{}]", item.label, fmt(item.value))
        } else {
            item.label.clone()
        };
        let _ = write!(
            &mut out,
            r#"<text x="{x}" y="{y}">{text}</text>"#,
            x = fmt(legend_text_x),
            y = fmt(14.0),
            text = escape_xml(&text)
        );
        out.push_str("</g>");
        out.checkpoint()?;
        if let Some(receipt) = paint_receipt.as_mut() {
            receipt.record_legend(paint_plan, legend_index, item.label.as_str(), terminal_fill);
        }
    }

    out.push_str("</g></svg>\n");
    let rooted_svg = root_document.complete(out.finish()?)?;
    if paint_receipt.is_some_and(|receipt| !paint_plan.record_terminal(receipt)) {
        return Err(crate::Error::InvalidModel {
            message: "Pie slice paint receipt did not match the terminal SVG".to_string(),
        });
    }
    Ok(rooted_svg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;
    use merman_core::diagrams::pie::PieDiagramRenderModel;
    use std::fmt;
    use std::ops::Range;

    #[derive(Default)]
    struct RejectAfterFirstWrite {
        write_attempts: usize,
        rejected: bool,
        retained: String,
    }

    impl RejectAfterFirstWrite {
        fn record_write(&mut self, value: &str) -> fmt::Result {
            self.write_attempts += 1;
            if self.write_attempts == 1 {
                self.rejected = true;
                return Err(fmt::Error);
            }
            self.retained.push_str(value);
            Ok(())
        }
    }

    impl fmt::Write for RejectAfterFirstWrite {
        fn write_str(&mut self, value: &str) -> fmt::Result {
            self.record_write(value)
        }
    }

    impl SvgOutput for RejectAfterFirstWrite {
        fn push_str(&mut self, value: &str) {
            let _ = self.record_write(value);
        }

        fn push(&mut self, value: char) {
            let mut encoded = [0u8; 4];
            let _ = self.record_write(value.encode_utf8(&mut encoded));
        }

        fn len(&self) -> usize {
            self.retained.len()
        }

        fn as_str(&self) -> &str {
            self.retained.as_str()
        }

        fn replace_range(&mut self, range: Range<usize>, replacement: &str) -> crate::Result<()> {
            self.retained.replace_range(range, replacement);
            Ok(())
        }

        fn checkpoint(&mut self) -> crate::Result<()> {
            if self.rejected {
                Err(crate::Error::InvalidModel {
                    message: "test SVG sink rejected the first write".to_string(),
                })
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn pie_slices_stop_after_the_first_svg_sink_failure() {
        let slices = (0..4)
            .map(|index| crate::model::PieSliceLayout {
                label: format!("slice-{index}"),
                value: 1.0,
                start_angle: 0.0,
                end_angle: std::f64::consts::TAU,
                is_full_circle: true,
                percent: 100,
                text_x: 0.0,
                text_y: 0.0,
                fill: "#ECECFF".to_string(),
            })
            .collect::<Vec<_>>();
        let mut model = PieDiagramRenderModel::default();
        model.sections = slices
            .iter()
            .map(|slice| merman_core::diagrams::pie::PieRenderSection {
                label: slice.label.clone(),
                value: slice.value,
            })
            .collect();
        let effective_config = serde_json::json!({});
        let paint_plan = crate::pie::PieSlicePaintPlan::baseline(&model, &effective_config);
        let mut out = RejectAfterFirstWrite::default();

        let error = render_pie_slices(
            &mut out,
            &slices,
            185.0,
            0.0,
            &effective_config,
            &paint_plan,
            None,
        )
        .expect_err("the rejecting sink must stop Pie slice rendering");

        assert!(matches!(error, crate::Error::InvalidModel { .. }));
        assert_eq!(
            out.write_attempts, 1,
            "Pie slice rendering must stop after the first failed slice write"
        );
    }

    #[test]
    fn pie_legend_rect_style_serializes_default_palette_colors_as_rgb() {
        assert_eq!(
            pie_legend_rect_style("hsl(60, 100%, 63.5294117647%)"),
            "fill: rgb(255, 255, 69); stroke: rgb(255, 255, 69);"
        );
        assert_eq!(
            pie_legend_rect_style("#ECECFF"),
            "fill: rgb(236, 236, 255); stroke: rgb(236, 236, 255);"
        );
        assert_eq!(
            pie_legend_rect_style("hsla(210 65.3846153846% 20.3921568627% / .5)"),
            "fill: rgba(18, 52, 86, 0.5); stroke: rgba(18, 52, 86, 0.5);"
        );
        assert_eq!(
            pie_legend_rect_style("ReBeccAPurple"),
            "fill: rebeccapurple; stroke: rebeccapurple;"
        );
    }

    #[test]
    fn empty_pie_root_viewport_fallback_only_repairs_non_finite_roots() {
        let model = PieDiagramRenderModel::default();
        let (bounds, max_width) =
            empty_pie_root_viewport_fallback(&model, 0.0, 0.0, f64::INFINITY, 450.0)
                .expect("non-finite empty pie should use fallback");

        assert_eq!(
            bounds,
            root_svg::DiagramBounds::from_view_box(0.0, 0.0, EMPTY_PIE_WIDTH, EMPTY_PIE_HEIGHT,)
        );
        assert_eq!(
            max_width,
            root_svg::RootMaxWidth::SvgNumber(EMPTY_PIE_WIDTH)
        );
    }

    #[test]
    fn empty_pie_root_viewport_fallback_preserves_finite_title_bounds() {
        let model = PieDiagramRenderModel::default();
        assert_eq!(
            empty_pie_root_viewport_fallback(&model, 0.0, 0.0, 292.400390625, 450.0),
            None
        );
    }

    #[test]
    fn pie_root_honors_disabled_max_width() {
        let layout = PieDiagramLayout {
            bounds: Some(crate::model::Bounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 490.0,
                max_y: 450.0,
            }),
            title: None,
            center_x: 225.0,
            center_y: 225.0,
            radius: 185.0,
            outer_radius: 186.0,
            legend_x: 216.0,
            legend_start_y: 0.0,
            legend_step_y: 22.0,
            slices: Vec::new(),
            legend_items: Vec::new(),
        };
        let options = SvgRenderOptions {
            diagram_id: Some("pieFixed".to_string()),
            ..SvgRenderOptions::default()
        };
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .expect("render session");
        let debug = SvgDebugOptions::default();
        let execution =
            SvgExecution::unthemed_for_test(&options, &debug, &session, DiagramFamilyId::PIE)
                .expect("SVG execution");

        let svg = render_pie_diagram_svg_model(
            &layout,
            &PieDiagramRenderModel::default(),
            &serde_json::json!({"pie": {"useMaxWidth": false}}),
            &execution,
        )
        .unwrap();
        let root_open = svg.split_once('>').expect("root svg open tag").0;

        assert!(root_open.contains(r#"width="490""#), "{root_open}");
        assert!(root_open.contains(r#"height="450""#), "{root_open}");
        assert!(
            root_open.contains(r#"viewBox="0 0 490 450""#),
            "{root_open}"
        );
        assert!(
            root_open.contains(r#"style="background-color: white;""#),
            "{root_open}"
        );
        assert!(!root_open.contains("max-width"), "{root_open}");
    }
}
