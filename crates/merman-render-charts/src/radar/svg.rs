use std::fmt::Write as _;

use merman_model::families::radar::RadarDiagramModel;
use merman_render_core::{
    LayoutPoint, Result,
    number_format::canonical_number as fmt,
    resources::OperationWorkMeter,
    root_svg,
    svg_css::{
        CssSelectorDiagramId, MermaidBaseCss, SvgDiagramIdValue, SvgId,
        write_mermaid_base_css_prefix, write_mermaid_base_css_root_rule,
    },
    svg_output::{BoundedSvgOutput, SvgOutput, XmlTextOutput},
    xml::{escape_attr_display, escape_xml},
};

use super::{
    RadarAxisLayout, RadarDiagramLayout, RadarStyle,
    config::use_max_width,
    theme::{RadarSeriesPaintPlan, RadarTypographyThemePlan},
};

fn effective_title<'a>(
    model: &'a RadarDiagramModel,
    diagram_title: Option<&'a str>,
) -> Option<&'a str> {
    model
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .or_else(|| {
            diagram_title
                .map(str::trim)
                .filter(|title| !title.is_empty())
        })
}

fn format_points(points: &[LayoutPoint]) -> String {
    let mut out = String::new();
    let mut buffer = ryu_js::Buffer::new();
    for (index, point) in points.iter().enumerate() {
        if index > 0 {
            out.push(' ');
        }
        out.push_str(buffer.format(point.x));
        out.push(',');
        out.push_str(buffer.format(point.y));
    }
    out
}

fn write_radar_css<I>(
    out: &mut impl SvgOutput,
    diagram_id: I,
    typography: &RadarTypographyThemePlan,
    series_paint: &RadarSeriesPaintPlan,
) -> Result<()>
where
    I: SvgDiagramIdValue,
{
    let theme = typography.css_binding();
    // Keep `:root` last (matches upstream Mermaid radar SVG baselines).
    let diagram_id = CssSelectorDiagramId(diagram_id);
    let font_family_css = typography.font_family_css();
    let font_size_css = typography.font_size_css();
    let title_fill_css = &theme.title_color;
    write_mermaid_base_css_prefix(
        out,
        diagram_id,
        MermaidBaseCss {
            font_family: font_family_css,
            font_size_css,
            normal_edge_stroke_width_css: "1px",
            text_color: &theme.text_color,
            line_color: &theme.line_color,
            error_bkg: &theme.error_bkg_color,
            error_text: &theme.error_text_color,
        },
    )?;

    let _ = write!(
        out,
        r#"#{} .radarTitle{{font-size:{};color:{};fill:{};dominant-baseline:hanging;text-anchor:middle;}}"#,
        diagram_id, font_size_css, title_fill_css, title_fill_css
    );
    out.checkpoint()?;
    let _ = write!(
        out,
        r#"#{} .radarAxisLine{{stroke:{};stroke-width:{};}}"#,
        diagram_id,
        theme.axis_color,
        fmt(theme.axis_stroke_width)
    );
    out.checkpoint()?;
    let _ = write!(
        out,
        r#"#{} .radarAxisLabel{{font-size:{};color:{};}}"#,
        diagram_id,
        theme.axis_label_font_size.css(),
        theme.axis_color
    );
    if let Some(color) = &theme.native_axis_label_color {
        let _ = write!(
            out,
            "#{diagram_id} .radarAxisLabel{{color:{color};fill:{color};}}"
        );
    }
    out.checkpoint()?;
    let _ = write!(
        out,
        r#"#{} .radarGraticule{{fill:{};fill-opacity:{};stroke:{};stroke-width:{};}}"#,
        diagram_id,
        theme.graticule_color,
        fmt(theme.graticule_opacity),
        theme.graticule_color,
        fmt(theme.graticule_stroke_width)
    );
    out.checkpoint()?;
    let _ = write!(
        out,
        r#"#{} .radarLegendText{{text-anchor:start;font-size:{};dominant-baseline:hanging;}}"#,
        diagram_id,
        theme.legend_font_size.css()
    );
    if let Some((title_family, title_weight, label_weight)) = typography.native_text_style() {
        let _ = write!(
            out,
            "#{diagram_id} .radarTitle{{font-family:{title_family};font-weight:{title_weight};}}\
            #{diagram_id} .radarAxisLabel,#{diagram_id} .radarLegendText{{font-weight:{label_weight};}}"
        );
    }
    out.checkpoint()?;

    for (i, c) in series_paint.colors().iter().enumerate() {
        let fill = series_paint.fill_css(i);
        let _ = write!(
            out,
            r#"#{} .radarCurve-{}{{color:{};fill:{};fill-opacity:{};stroke:{};stroke-width:{};}}#{} .radarLegendBox-{}{{fill:{};fill-opacity:{};stroke:{};}}"#,
            diagram_id,
            i,
            c,
            fill,
            fmt(theme.curve_opacity),
            c,
            fmt(theme.curve_stroke_width),
            diagram_id,
            i,
            fill,
            fmt(theme.curve_opacity),
            c
        );
        out.checkpoint()?;
    }
    write_mermaid_base_css_root_rule(out, diagram_id, font_family_css)
}

fn write_radar_axes(out: &mut impl SvgOutput, axes: &[RadarAxisLayout]) -> Result<()> {
    for axis in axes {
        let cos_a = axis.angle.cos();
        let sin_a = axis.angle.sin();
        let text_anchor = if cos_a > 0.01 {
            "start"
        } else if cos_a < -0.01 {
            "end"
        } else {
            "middle"
        };
        let dominant_baseline = if sin_a > 0.01 {
            "hanging"
        } else if sin_a < -0.01 {
            "auto"
        } else {
            "central"
        };
        let label_padding = 4.0;
        let _ = write!(
            out,
            r#"<line x1="0" y1="0" x2="{x2}" y2="{y2}" class="radarAxisLine"/><text x="{x}" y="{y}" text-anchor="{text_anchor}" dominant-baseline="{dominant_baseline}" class="radarAxisLabel">{label}</text>"#,
            x2 = fmt(axis.line_x2),
            y2 = fmt(axis.line_y2),
            x = fmt(axis.label_x + label_padding * cos_a),
            y = fmt(axis.label_y + label_padding * sin_a),
            label = escape_xml(&axis.label)
        );
        out.checkpoint()?;
    }
    Ok(())
}

/// Host-owned SVG ID, viewport, and work meter for a Radar render.
pub struct RadarSvgContext<'a> {
    /// The projected ID used in the root, ARIA references, and stylesheet.
    pub diagram_id: SvgId<'a>,
    pub viewport: root_svg::RootViewportContext<'a>,
    pub work_meter: &'a OperationWorkMeter,
}

/// Emits the complete Radar document before shared terminal processing.
pub fn render_radar_svg(
    layout: &RadarDiagramLayout,
    model: &RadarDiagramModel,
    style: &RadarStyle,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    context: &RadarSvgContext<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = context.diagram_id;
    let RadarStyle {
        typography,
        series_paint,
    } = style;

    let has_acc_title = model.acc_title.as_deref().is_some_and(|s| !s.is_empty());
    let has_acc_descr = model.acc_descr.as_deref().is_some_and(|s| !s.is_empty());

    let use_max_width = use_max_width(effective_config);

    let aria_describedby = has_acc_descr.then(|| format!("chart-desc-{diagram_id}"));
    let aria_labelledby = has_acc_title.then(|| format!("chart-title-{diagram_id}"));
    let root_extra_attrs: [(&str, &str); 1] = [("overflow", "visible")];

    let mut out = BoundedSvgOutput::new(context.work_meter);
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "radar");
    root_chrome.extra_attrs = &root_extra_attrs;
    root_chrome.aria_labelledby = aria_labelledby.as_deref();
    root_chrome.aria_describedby = aria_describedby.as_deref();
    root_chrome.dom = root_svg::RootDomProfile {
        fixed_height_placement: root_svg::SvgRootFixedHeightPlacement::AfterXmlns,
        fixed_style_placement: root_svg::RootStylePlacement::Tail,
        trailing_newline: false,
        ..root_svg::RootDomProfile::default()
    };
    let root_document = context.viewport.write_open(
        &mut out,
        root_svg::RootViewportSpec::mermaid(
            root_svg::DiagramBounds::from_view_box(0.0, 0.0, layout.svg_width, layout.svg_height),
            use_max_width,
        )
        .with_max_width(root_svg::RootMaxWidth::CssSixSignificant(layout.svg_width)),
        root_chrome,
    )?;

    if has_acc_title {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{id}">{text}</title>"#,
            id = diagram_id,
            text = escape_xml(model.acc_title.as_deref().unwrap_or_default())
        );
        out.checkpoint()?;
    }
    if has_acc_descr {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{id}">{text}</desc>"#,
            id = diagram_id,
            text = escape_xml(model.acc_descr.as_deref().unwrap_or_default())
        );
        out.checkpoint()?;
    }

    out.push_str("<style>");
    out.checkpoint()?;
    write_radar_css(
        &mut XmlTextOutput::new(&mut out),
        diagram_id,
        typography,
        series_paint,
    )?;
    out.push_str("</style><g/>");
    out.checkpoint()?;

    let _ = write!(
        &mut out,
        r#"<g transform="translate({x}, {y})">"#,
        x = fmt(layout.center_x),
        y = fmt(layout.center_y)
    );
    out.checkpoint()?;

    for graticule in layout.graticules.iter().filter(|_| typography.show_grid()) {
        if graticule.kind == "polygon" {
            if graticule.points.is_empty() {
                out.push_str(r#"<polygon points="" class="radarGraticule"/>"#);
            } else {
                let points = format_points(&graticule.points);
                let _ = write!(
                    &mut out,
                    r#"<polygon points="{points}" class="radarGraticule"/>"#
                );
            }
        } else if let Some(r) = graticule.r {
            let _ = write!(
                &mut out,
                r#"<circle r="{r}" class="radarGraticule"/>"#,
                r = ryu_js::Buffer::new().format(r)
            );
        }
        out.checkpoint()?;
    }
    write_radar_axes(&mut out, &layout.axes)?;

    let polygon_curves = model.options.graticule.trim() == "polygon";
    for curve in &layout.curves {
        if polygon_curves && !curve.points.is_empty() {
            let points = format_points(&curve.points);
            let _ = write!(
                &mut out,
                r#"<polygon points="{points}" class="radarCurve-{idx}"/>"#,
                idx = curve.class_index
            );
        } else {
            let _ = write!(
                &mut out,
                r#"<path d="{d}" class="radarCurve-{idx}"/>"#,
                d = escape_attr_display(&curve.path_d),
                idx = curve.class_index
            );
        }
        out.checkpoint()?;
    }

    for item in &layout.legend_items {
        let _ = write!(
            &mut out,
            r#"<g transform="translate({x}, {y})">"#,
            x = fmt(item.x),
            y = fmt(item.y)
        );
        out.checkpoint()?;
        let _ = write!(
            &mut out,
            r#"<rect width="{size}" height="{size}" class="radarLegendBox-{idx}"/>"#,
            size = fmt(12.0),
            idx = item.class_index
        );
        out.checkpoint()?;
        let label = model
            .curves
            .get(item.class_index as usize)
            .map(|curve| curve.label.as_str())
            .unwrap_or("");
        let _ = write!(
            &mut out,
            r#"<text x="{x}" y="{y}" class="radarLegendText">{text}</text>"#,
            x = fmt(16.0),
            y = fmt(0.0),
            text = escape_xml(label)
        );
        out.checkpoint()?;
        out.push_str("</g>");
        out.checkpoint()?;
    }

    let title = effective_title(model, diagram_title);
    match title {
        Some(t) => {
            let _ = write!(
                &mut out,
                r#"<text class="radarTitle" x="0" y="{y}">{text}</text>"#,
                y = fmt(layout.title_y),
                text = escape_xml(t)
            );
        }
        None => {
            let _ = write!(
                &mut out,
                r#"<text class="radarTitle" x="0" y="{y}"/>"#,
                y = fmt(layout.title_y)
            );
        }
    };
    out.checkpoint()?;
    out.push_str("</g></svg>\n");
    root_document.complete(out.finish()?)
}

#[cfg(test)]
mod tests {
    use std::fmt;
    use std::ops::Range;

    use merman_model::family::DiagramFamilyId;
    use merman_render_core::{
        Error, resources::RenderResourcePolicy, svg_css::SvgDiagramIdProjection,
    };

    use super::super::test_support::{FALLBACK_C_SCALE, presentation_config};
    use super::*;

    fn radar_css_for_test(config: &serde_json::Value) -> String {
        let effective_config = presentation_config(config.clone());
        let typography = RadarTypographyThemePlan::resolve(None, &effective_config);
        let mut css = String::new();
        write_radar_css(
            &mut css,
            "radar",
            &typography,
            &RadarSeriesPaintPlan::baseline(config, &FALLBACK_C_SCALE),
        )
        .expect("String-backed Radar CSS writer");
        css
    }

    fn render_test_svg(
        layout: &RadarDiagramLayout,
        effective_config: serde_json::Value,
        id: &str,
    ) -> root_svg::RootedSvg {
        let config = presentation_config(effective_config);
        let model = RadarDiagramModel::default();
        let style = RadarStyle::resolve(None, &config, &model, &FALLBACK_C_SCALE);
        let meter = OperationWorkMeter::new(RenderResourcePolicy::interactive());
        let projection = SvgDiagramIdProjection::new(&meter);
        let diagram_id = SvgId::new(id).unwrap().with_projection(&projection);
        let svg = render_radar_svg(
            layout,
            &model,
            &style,
            config.as_value(),
            None,
            &RadarSvgContext {
                diagram_id,
                viewport: root_svg::RootViewportContext::new(DiagramFamilyId::RADAR, diagram_id),
                work_meter: &meter,
            },
        )
        .unwrap();
        projection.finish().unwrap();
        svg
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
                Err(Error::InvalidModel {
                    message: "test SVG sink rejected the first write".to_string(),
                })
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn radar_axes_stop_after_the_first_svg_sink_failure() {
        let axes = (0..4)
            .map(|index| RadarAxisLayout {
                label: format!("axis-{index}"),
                angle: index as f64,
                line_x2: index as f64,
                line_y2: index as f64,
                label_x: index as f64,
                label_y: index as f64,
            })
            .collect::<Vec<_>>();
        let mut out = RejectAfterFirstWrite::default();

        let error = write_radar_axes(&mut out, &axes)
            .expect_err("the rejecting sink must stop Radar axis emission");

        assert!(matches!(error, Error::InvalidModel { .. }));
        assert_eq!(
            out.write_attempts, 1,
            "Radar axis emission must stop at the first failed sink checkpoint"
        );
    }

    #[test]
    fn radar_css_honors_top_level_style_overrides() {
        let cfg = serde_json::json!({
            "themeVariables": {
                "fontFamily": "\"ibm plex sans\", arial, sans-serif",
                "fontSize": "18px",
                "textColor": "#101010",
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

        let css = radar_css_for_test(&cfg);

        assert!(css.contains(r#"#radar .radarTitle{font-size:18px;color:#202020;"#));
        assert!(css.contains(r#"#radar .radarAxisLine{stroke:#606060;stroke-width:4;}"#));
        assert!(css.contains(r#"#radar .radarAxisLabel{font-size:14px;color:#606060;}"#));
        assert!(css.contains(
            r#"#radar .radarGraticule{fill:#707070;fill-opacity:0.8;stroke:#707070;stroke-width:5;}"#
        ));
        assert!(css.contains(
            r#"#radar .radarLegendText{text-anchor:start;font-size:16px;dominant-baseline:hanging;}"#
        ));
        assert!(css.contains(
            r#"#radar .radarCurve-0{color:#303030;fill:#303030;fill-opacity:0.9;stroke:#303030;stroke-width:6;}"#
        ));
    }

    #[test]
    fn radar_css_uses_scoped_theme_variables_when_top_level_is_missing() {
        let cfg = serde_json::json!({
            "themeVariables": {
                "fontFamily": "\"ibm plex sans\", arial, sans-serif",
                "fontSize": "18px",
                "textColor": "#101010",
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
            }
        });

        let css = radar_css_for_test(&cfg);

        assert!(css.contains(r#"#radar .radarAxisLine{stroke:#404040;stroke-width:2;}"#));
        assert!(css.contains(r#"#radar .radarAxisLabel{font-size:12px;color:#404040;}"#));
        assert!(css.contains(
            r#"#radar .radarGraticule{fill:#505050;fill-opacity:0.3;stroke:#505050;stroke-width:1;}"#
        ));
        assert!(css.contains(
            r#"#radar .radarLegendText{text-anchor:start;font-size:12px;dominant-baseline:hanging;}"#
        ));
        assert!(css.contains(
            r#"#radar .radarCurve-0{color:#303030;fill:#303030;fill-opacity:0.5;stroke:#303030;stroke-width:2;}"#
        ));
    }

    #[test]
    fn radar_root_uses_responsive_width_and_max_width_style() {
        let layout = RadarDiagramLayout {
            bounds: None,
            svg_width: 700.0,
            svg_height: 700.0,
            center_x: 350.0,
            center_y: 350.0,
            radius: 300.0,
            axis_label_factor: 1.05,
            title_y: -350.0,
            axes: Vec::new(),
            graticules: Vec::new(),
            curves: Vec::new(),
            legend_items: Vec::new(),
        };
        let svg = render_test_svg(&layout, serde_json::json!({}), "radarRoot");
        let root_open = svg.split_once('>').expect("root svg open tag").0;

        assert!(root_open.contains(r#"width="100%""#), "{root_open}");
        assert!(
            root_open.contains(r#"style="max-width: 700px;" viewBox="0 0 700 700""#),
            "{root_open}"
        );
        assert!(
            !root_open.contains(r#"height=""#),
            "radar root should not emit fixed height: {root_open}"
        );
    }

    #[test]
    fn radar_root_honors_disabled_max_width() {
        let layout = RadarDiagramLayout {
            bounds: None,
            svg_width: 700.0,
            svg_height: 700.0,
            center_x: 350.0,
            center_y: 350.0,
            radius: 300.0,
            axis_label_factor: 1.05,
            title_y: -350.0,
            axes: Vec::new(),
            graticules: Vec::new(),
            curves: Vec::new(),
            legend_items: Vec::new(),
        };
        let svg = render_test_svg(
            &layout,
            serde_json::json!({"radar": {"useMaxWidth": false}}),
            "radarFixed",
        );
        let root_open = svg.split_once('>').expect("root svg open tag").0;

        assert!(root_open.contains(r#"width="700""#), "{root_open}");
        assert!(root_open.contains(r#"height="700""#), "{root_open}");
        assert!(
            root_open.contains(r#"viewBox="0 0 700 700""#),
            "{root_open}"
        );
        assert!(!root_open.contains("style="), "{root_open}");
        assert!(!root_open.contains("max-width"), "{root_open}");
    }

    #[test]
    fn radar_root_and_axis_labels_match_mermaid_11_16() {
        let axes = [
            ("Top", -std::f64::consts::FRAC_PI_2, 0.0, -100.0),
            ("Right", 0.0, 100.0, 0.0),
            ("Bottom", std::f64::consts::FRAC_PI_2, 0.0, 100.0),
            ("Left", std::f64::consts::PI, -100.0, 0.0),
        ]
        .into_iter()
        .map(|(label, angle, label_x, label_y)| RadarAxisLayout {
            label: label.to_string(),
            angle,
            line_x2: label_x,
            line_y2: label_y,
            label_x,
            label_y,
        })
        .collect();
        let layout = RadarDiagramLayout {
            bounds: None,
            svg_width: 240.0,
            svg_height: 240.0,
            center_x: 120.0,
            center_y: 120.0,
            radius: 100.0,
            axis_label_factor: 1.0,
            title_y: -120.0,
            axes,
            graticules: Vec::new(),
            curves: Vec::new(),
            legend_items: Vec::new(),
        };

        let svg = render_test_svg(&layout, serde_json::json!({}), "radar");
        let root_open = svg.split_once('>').expect("root svg open tag").0;

        assert!(root_open.contains(r#"overflow="visible""#), "{root_open}");
        assert!(
            svg.contains(
                r#"<text x="0" y="-104" text-anchor="middle" dominant-baseline="auto" class="radarAxisLabel">Top</text>"#
            ),
            "{svg}"
        );
        assert!(
            svg.contains(
                r#"<text x="104" y="0" text-anchor="start" dominant-baseline="central" class="radarAxisLabel">Right</text>"#
            ),
            "{svg}"
        );
        assert!(
            svg.contains(
                r#"<text x="0" y="104" text-anchor="middle" dominant-baseline="hanging" class="radarAxisLabel">Bottom</text>"#
            ),
            "{svg}"
        );
        assert!(
            svg.contains(
                r#"<text x="-104" y="0" text-anchor="end" dominant-baseline="central" class="radarAxisLabel">Left</text>"#
            ),
            "{svg}"
        );
        assert!(
            svg.contains(".radarAxisLabel{font-size:12px;color:#333333;}"),
            "radar CSS must leave per-axis alignment attributes in control: {svg}"
        );
    }
}
