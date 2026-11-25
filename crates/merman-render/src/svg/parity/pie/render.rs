use super::super::*;
use super::geometry::*;
use crate::pie::{
    PIE_LEGEND_RECT_SIZE_PX, PIE_LEGEND_SPACING_PX, PIE_TITLE_BASELINE_Y_PX, PieLegendPosition,
};
use merman_model::families::pie::PieDiagramModel;

fn pie_legend_rect_style(fill: &str) -> String {
    // Mermaid emits legend colors via inline `style` in rgb() form for default themes.
    // The compare tooling ignores `style`, but we keep this for human inspection parity.
    let color = super::super::util::cssom_color_value(fill);
    format!("fill: {color}; stroke: {color};")
}

fn render_pie_slices(
    out: &mut impl SvgOutput,
    slices: &[crate::model::PieSliceLayout],
    radius: f64,
    inner_radius: f64,
    effective_config: &impl crate::config::ConfigLookup,
) -> Result<()> {
    let highlight = crate::config::config_string(effective_config, &["pie", "highlightSlice"])
        .unwrap_or_default();
    for slice in slices {
        let slice_class = pie_slice_class(&highlight, &slice.label);
        let escaped_fill = escape_xml(&slice.fill);
        let d = pie_slice_path(slice, radius, inner_radius);
        let _ = write!(
            out,
            r#"<path d="{d}" fill="{escaped_fill}" class="{slice_class}"/>"#
        );
        out.checkpoint()?;
    }

    Ok(())
}

#[cfg(test)]
pub(crate) fn render_pie_diagram_svg_model(
    layout: &PieDiagramLayout,
    model: &PieDiagramModel,
    effective_config: &impl crate::config::ConfigLookup,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let paint_plan = crate::pie::PieThemePlan::baseline(model, effective_config);
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
    model: &PieDiagramModel,
    paint_plan: &crate::pie::PieThemePlan,
    effective_config: &impl crate::config::ConfigLookup,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id_or("merman");

    let render_settings = crate::pie::PieConfigView::new(effective_config).render_settings();
    let mut out = BoundedSvgOutput::new(options.work_meter());
    let root_document = open_pie_svg(
        &mut out,
        model,
        diagram_id,
        pie_viewport(layout, model),
        render_settings.use_max_width,
        options,
    )?;

    out.push_str("<style>");
    out.checkpoint()?;
    write_pie_css(
        &mut super::super::output::XmlTextOutput::new(&mut out),
        diagram_id.semantic_str(),
        paint_plan,
    )?;
    out.push_str(r#"</style><g/>"#);
    out.checkpoint()?;

    let content_width = if render_settings.legend_position == PieLegendPosition::Left {
        paint_plan.content_width()?
    } else {
        0.0
    };
    let offset = pie_group_offset(layout, render_settings.legend_position, content_width);
    write_pie_group_open(&mut out, layout, offset);

    let _ = write!(
        &mut out,
        r#"<circle cx="0" cy="0" r="{r}" class="pieOuterCircle"/>"#,
        r = fmt(layout.outer_radius)
    );
    out.checkpoint()?;

    let inner_radius = render_settings.donut_hole * layout.radius;
    render_pie_slices(
        &mut out,
        &layout.slices,
        layout.radius,
        inner_radius,
        effective_config,
    )?;

    for slice in &layout.slices {
        options.checkpoint_emit()?;
        let text = format!("{}%", slice.percent);
        let _ = write!(
            &mut out,
            r#"<text transform="translate({x},{y})" class="slice" style="text-anchor: middle;">{text}</text>"#,
            x = fmt(slice.text_x),
            y = fmt(slice.text_y),
            text = escape_xml(&text)
        );
        out.checkpoint()?;
    }

    out.push_str("</g>");

    let title_text = layout.title.as_deref();
    match title_text {
        Some(t) => {
            options.checkpoint_emit()?;
            let _ = write!(
                &mut out,
                r#"<text x="0" y="{y}" class="{class}">{text}</text>"#,
                y = fmt(PIE_TITLE_BASELINE_Y_PX),
                class = crate::pie::PIE_TITLE_CLASS,
                text = escape_xml(t)
            );
        }
        None => {
            options.checkpoint_emit()?;
            let _ = write!(
                &mut out,
                r#"<text x="0" y="{y}" class="{class}"/>"#,
                y = fmt(PIE_TITLE_BASELINE_Y_PX),
                class = crate::pie::PIE_TITLE_CLASS,
            );
        }
    }
    out.checkpoint()?;

    let legend_rect_size = PIE_LEGEND_RECT_SIZE_PX;
    let legend_text_x = legend_rect_size + PIE_LEGEND_SPACING_PX;

    for item in &layout.legend_items {
        let _ = write!(
            &mut out,
            r#"<g class="legend" transform="translate({x},{y})">"#,
            x = fmt(layout.legend_x),
            y = fmt(item.y)
        );
        out.checkpoint()?;
        let style = pie_legend_rect_style(&item.fill);
        let escaped_style = escape_xml(&style);
        let _ = write!(
            &mut out,
            r#"<rect width="{size}" height="{size}" style="{style}"/>"#,
            size = fmt(legend_rect_size),
            style = escaped_style
        );
        out.checkpoint()?;
        let text = crate::pie::pie_legend_text(&item.label, item.value, model.show_data);
        let _ = write!(
            &mut out,
            r#"<text x="{x}" y="{y}">{text}</text>"#,
            x = fmt(legend_text_x),
            y = fmt(14.0),
            text = escape_xml(&text)
        );
        out.push_str("</g>");
        out.checkpoint()?;
    }

    out.push_str("</g></svg>\n");
    root_document.complete(out.finish()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;
    use merman_model::families::pie::PieDiagramModel;
    use std::fmt;
    use std::ops::Range;

    fn rendered_sector_path(start_angle: f64, end_angle: f64, inner_radius: f64) -> String {
        let slice = crate::model::PieSliceLayout {
            label: "Sector".to_string(),
            value: 1.0,
            start_angle,
            end_angle,
            is_full_circle: false,
            percent: 100,
            text_x: 0.0,
            text_y: 0.0,
            fill: "#ECECFF".to_string(),
        };
        let effective_config = serde_json::json!({});
        let mut out = String::new();
        render_pie_slices(&mut out, &[slice], 185.0, inner_radius, &effective_config)
            .expect("render sector");
        let document = roxmltree::Document::parse(&out).expect("one valid sector path");
        document
            .root_element()
            .attribute("d")
            .expect("sector path data")
            .to_owned()
    }

    #[test]
    fn pie_near_full_arcs_split_coincident_rounded_endpoints() {
        // d3-shape 3.2.1 with d3-path 3.1.1, using its default three-digit precision.
        // Synthetic geometry reaches the arc boundary independently of Pie's 1% filter.
        for (start, inner, expected) in [
            (
                0.0,
                0.0,
                "M0,-185A185,185,0,0,1,0,185A185,185,0,0,1,0,-185L0,0Z",
            ),
            (
                0.0,
                74.0,
                "M0,-185A185,185,0,0,1,0,185A185,185,0,0,1,0,-185L0,-74A74,74,0,0,0,0,74A74,74,0,0,0,0,-74Z",
            ),
            (
                0.3,
                0.0,
                "M54.671,-176.737A185,185,0,0,1,-54.671,176.737A185,185,0,0,1,54.671,-176.737L0,0Z",
            ),
            (
                0.3,
                74.0,
                "M54.671,-176.737A185,185,0,0,1,-54.671,176.737A185,185,0,0,1,54.671,-176.737L21.868,-70.695A74,74,0,0,0,-21.868,70.695A74,74,0,0,0,21.868,-70.695Z",
            ),
        ] {
            assert_eq!(
                rendered_sector_path(start, start + std::f64::consts::TAU - 0.000002, inner),
                expected,
                "start={start}, inner={inner}"
            );
        }
    }

    #[test]
    fn pie_donut_rounding_can_split_only_the_inner_ring() {
        let end = std::f64::consts::TAU - 0.000004;
        assert_eq!(
            rendered_sector_path(0.0, end, 0.0),
            "M0,-185A185,185,0,1,1,-0.001,-185L0,0Z"
        );
        assert_eq!(
            rendered_sector_path(0.0, end, 74.0),
            "M0,-185A185,185,0,1,1,-0.001,-185L0,-74A74,74,0,0,0,0,74A74,74,0,0,0,0,-74Z"
        );
    }

    #[test]
    fn pie_distinct_and_narrow_arcs_keep_single_arc_segments() {
        for (end, inner, expected) in [
            (
                std::f64::consts::TAU - 0.00001,
                0.0,
                "M0,-185A185,185,0,1,1,-0.002,-185L0,0Z",
            ),
            (
                std::f64::consts::TAU - 0.00001,
                74.0,
                "M0,-185A185,185,0,1,1,-0.002,-185L-0.001,-74A74,74,0,1,0,0,-74Z",
            ),
            (0.000002, 0.0, "M0,-185A185,185,0,0,1,0,-185L0,0Z"),
            (
                0.000002,
                74.0,
                "M0,-185A185,185,0,0,1,0,-185L0,-74A74,74,0,0,0,0,-74Z",
            ),
            (
                0.00001,
                74.0,
                "M0,-185A185,185,0,0,1,0.002,-185L0.001,-74A74,74,0,0,0,0,-74Z",
            ),
            (
                std::f64::consts::PI,
                74.0,
                "M0,-185A185,185,0,1,1,0,185L0,74A74,74,0,1,0,0,-74Z",
            ),
            (
                1.5 * std::f64::consts::PI,
                74.0,
                "M0,-185A185,185,0,1,1,-185,0L-74,0A74,74,0,1,0,0,-74Z",
            ),
        ] {
            assert_eq!(
                rendered_sector_path(0.0, end, inner),
                expected,
                "end={end}, inner={inner}"
            );
        }
    }

    #[test]
    fn pie_path_circle_threshold_precedes_rounded_endpoint_splitting() {
        let end = std::f64::consts::TAU - 0.0000005;
        assert_eq!(
            rendered_sector_path(0.0, end, 0.0),
            "M0,-185A185,185,0,1,1,0,185A185,185,0,1,1,0,-185L0,0Z"
        );
        assert_eq!(
            rendered_sector_path(0.0, end, 74.0),
            "M0,-185A185,185,0,1,1,0,185A185,185,0,1,1,0,-185L0,-74A74,74,0,1,0,0,74A74,74,0,1,0,0,-74Z"
        );
    }

    #[test]
    fn pie_path_rounding_uses_javascript_negative_half_ties() {
        for (value, expected) in [
            (0.0005, "0.001"),
            (-0.0005, "0"),
            (0.0015, "0.002"),
            (-0.0015, "-0.001"),
            (1.2345, "1.235"),
            (-1.2345, "-1.234"),
        ] {
            assert_eq!(pie_path_number(value).to_string(), expected, "{value}");
        }
    }

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
        let effective_config = serde_json::json!({});
        let mut out = RejectAfterFirstWrite::default();

        let error = render_pie_slices(&mut out, &slices, 185.0, 0.0, &effective_config)
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
        let model = PieDiagramModel::default();
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
        let model = PieDiagramModel::default();
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
            &PieDiagramModel::default(),
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
        assert!(!root_open.contains("style="), "{root_open}");
        assert!(!root_open.contains("max-width"), "{root_open}");
    }
}
