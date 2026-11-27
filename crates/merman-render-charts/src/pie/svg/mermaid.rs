use std::fmt::Write as _;

use merman_model::families::pie::PieDiagramModel;
use merman_render_core::{
    Result,
    number_format::canonical_number as fmt,
    svg_css::SvgId,
    svg_output::{SvgOutput, XmlTextOutput},
    theme_svg::cssom_color_value,
    xml::{escape_attr_display, escape_xml},
};

use super::write_pie_group_open;
use crate::pie::{
    PIE_LEGEND_RECT_SIZE_PX, PIE_LEGEND_SPACING_PX, PIE_TITLE_BASELINE_Y_PX, PIE_TITLE_CLASS,
    PieDiagramLayout, PieLegendPosition, PieMermaidCss, PieMermaidStyle, PieRenderSettings,
    PieSliceLayout, PieTextRole, pie_group_offset, pie_legend_text, pie_slice_class,
    pie_slice_path,
};

/// Writes Pie's rules between the common Mermaid stylesheet prefix and suffix.
pub fn write_mermaid_css_rules(
    out: &mut XmlTextOutput<'_, impl SvgOutput>,
    id: SvgId<'_>,
    values: &PieMermaidCss,
    font_family: &str,
) -> Result<()> {
    let _ = write!(
        out,
        r#"#{} .pieCircle{{stroke:{};stroke-width:{};opacity:{};}}#{} .pieCircle.highlighted{{scale:1.05;opacity:1;}}#{} .pieCircle.highlightedOnHover:hover{{transition-duration:250ms;scale:1.05;opacity:1;}}#{} .pieOuterCircle{{stroke:{};stroke-width:{};fill:none;}}"#,
        id,
        values.slice_stroke,
        values.slice_stroke_width,
        values.slice_opacity,
        id,
        id,
        id,
        values.outer_stroke,
        values.outer_stroke_width,
    );
    for role in PieTextRole::ALL {
        // Preserve Mermaid's baseline declaration order and bytes for each independent role.
        let _ = write!(out, "#{} .{}{{", id, role.selector());
        match role {
            PieTextRole::Title => {
                let _ = write!(
                    out,
                    "text-anchor:middle;font-size:{};fill:{};font-family:{};",
                    values.text_size(role),
                    values.text_fill(role),
                    font_family
                );
            }
            PieTextRole::Section => {
                let _ = write!(
                    out,
                    "font-family:{};fill:{};font-size:{};",
                    font_family,
                    values.text_fill(role),
                    values.text_size(role)
                );
            }
            PieTextRole::Legend => {
                let _ = write!(
                    out,
                    "fill:{};font-family:{};font-size:{};",
                    values.text_fill(role),
                    font_family,
                    values.text_size(role)
                );
            }
        }
        out.push('}');
    }
    out.checkpoint()
}

/// Writes the classic chart and legend groups after CSS. The caller closes the outer group.
pub fn write_mermaid_svg_body(
    out: &mut impl SvgOutput,
    layout: &PieDiagramLayout,
    model: &PieDiagramModel,
    style: &PieMermaidStyle,
    settings: PieRenderSettings<'_>,
    checkpoint_emit: impl Fn() -> Result<()>,
) -> Result<()> {
    let content_width = if settings.legend_position == PieLegendPosition::Left {
        style.content_width()?
    } else {
        0.0
    };
    let offset = pie_group_offset(layout, settings.legend_position, content_width);
    write_pie_group_open(out, layout, offset);

    let _ = write!(
        out,
        r#"<circle cx="0" cy="0" r="{r}" class="pieOuterCircle"/>"#,
        r = fmt(layout.outer_radius)
    );
    out.checkpoint()?;

    let inner_radius = settings.donut_hole * layout.radius;
    write_mermaid_slices(
        out,
        &layout.slices,
        layout.radius,
        inner_radius,
        settings.highlight_slice,
    )?;

    for slice in &layout.slices {
        checkpoint_emit()?;
        let text = format!("{}%", slice.percent);
        let _ = write!(
            out,
            r#"<text transform="translate({x},{y})" class="slice" style="text-anchor: middle;">{text}</text>"#,
            x = fmt(slice.text_x),
            y = fmt(slice.text_y),
            text = escape_xml(&text)
        );
        out.checkpoint()?;
    }

    out.push_str("</g>");

    // As in the renderer, observe operation terminals before formatting text.
    let title_text = layout.title.as_deref();
    match title_text {
        Some(t) => {
            checkpoint_emit()?;
            let _ = write!(
                out,
                r#"<text x="0" y="{y}" class="{class}">{text}</text>"#,
                y = fmt(PIE_TITLE_BASELINE_Y_PX),
                class = PIE_TITLE_CLASS,
                text = escape_xml(t)
            );
        }
        None => {
            checkpoint_emit()?;
            let _ = write!(
                out,
                r#"<text x="0" y="{y}" class="{class}"/>"#,
                y = fmt(PIE_TITLE_BASELINE_Y_PX),
                class = PIE_TITLE_CLASS,
            );
        }
    }
    out.checkpoint()?;

    let legend_rect_size = PIE_LEGEND_RECT_SIZE_PX;
    let legend_text_x = legend_rect_size + PIE_LEGEND_SPACING_PX;

    for item in &layout.legend_items {
        let _ = write!(
            out,
            r#"<g class="legend" transform="translate({x},{y})">"#,
            x = fmt(layout.legend_x),
            y = fmt(item.y)
        );
        out.checkpoint()?;
        let style = mermaid_legend_rect_style(&item.fill);
        let escaped_style = escape_attr_display(&style);
        let _ = write!(
            out,
            r#"<rect width="{size}" height="{size}" style="{style}"/>"#,
            size = fmt(legend_rect_size),
            style = escaped_style
        );
        out.checkpoint()?;
        let text = pie_legend_text(&item.label, item.value, model.show_data);
        let _ = write!(
            out,
            r#"<text x="{x}" y="{y}">{text}</text>"#,
            x = fmt(legend_text_x),
            y = fmt(14.0),
            text = escape_xml(&text)
        );
        out.push_str("</g>");
        out.checkpoint()?;
    }

    Ok(())
}

fn mermaid_legend_rect_style(fill: &str) -> String {
    // Mermaid emits legend colors via inline `style` in rgb() form for default themes.
    // The compare tooling ignores `style`, but we keep this for human inspection parity.
    let color = cssom_color_value(fill);
    format!("fill: {color}; stroke: {color};")
}

fn write_mermaid_slices(
    out: &mut impl SvgOutput,
    slices: &[PieSliceLayout],
    radius: f64,
    inner_radius: f64,
    highlight: &str,
) -> Result<()> {
    for slice in slices {
        let slice_class = pie_slice_class(highlight, &slice.label);
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
mod tests {
    use super::*;
    use std::fmt;
    use std::ops::Range;

    #[test]
    fn pie_stylesheet_encodes_authored_css_for_xml_once() {
        let model = PieDiagramModel::default();
        let config =
            serde_json::json!({"themeVariables": {"pieStrokeColor": r"var(--brand\&ink)"}});
        let style = PieMermaidStyle::baseline(&model, &config);
        let mut text = String::new();
        write_mermaid_css_rules(
            &mut XmlTextOutput::new(&mut text),
            SvgId::new("pie").unwrap(),
            style.css_binding(),
            r#""A&B Serif", serif"#,
        )
        .unwrap();

        assert!(text.contains(r"stroke:var(--brand\&amp;ink);"));
        assert!(text.contains(r#"font-family:"A&amp;B Serif", serif;"#));
    }

    fn rendered_sector_path(start_angle: f64, end_angle: f64, inner_radius: f64) -> String {
        let slice = PieSliceLayout {
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
        let mut out = String::new();
        write_mermaid_slices(&mut out, &[slice], 185.0, inner_radius, "").expect("render sector");
        out.strip_prefix(r#"<path d=""#)
            .and_then(|attributes| attributes.split_once('"'))
            .expect("one sector path with a d attribute")
            .0
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

        fn replace_range(&mut self, range: Range<usize>, replacement: &str) -> Result<()> {
            self.retained.replace_range(range, replacement);
            Ok(())
        }

        fn checkpoint(&mut self) -> Result<()> {
            if self.rejected {
                Err(merman_render_core::Error::InvalidModel {
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
            .map(|index| PieSliceLayout {
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
        let mut out = RejectAfterFirstWrite::default();

        let error = write_mermaid_slices(&mut out, &slices, 185.0, 0.0, "")
            .expect_err("the rejecting sink must stop Pie slice rendering");

        assert!(matches!(
            error,
            merman_render_core::Error::InvalidModel { .. }
        ));
        assert_eq!(
            out.write_attempts, 1,
            "Pie slice rendering must stop after the first failed slice write"
        );
    }
}
