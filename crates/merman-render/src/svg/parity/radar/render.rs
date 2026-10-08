use super::super::theme::RadarTheme;
use super::super::*;
use merman_core::diagrams::radar::RadarDiagramRenderModel;

// Radar diagram SVG renderer implementation (split from parity.rs).

#[allow(
    clippy::too_many_arguments,
    reason = "The SVG writer takes geometry, resolved styles, and terminal evidence separately."
)]
fn write_radar_css<'a, I>(
    out: &mut impl SvgOutput,
    diagram_id: I,
    theme: &'a RadarTheme,
    axis_paint: &'a crate::radar::RadarAxisPaintPlan,
    text_paint: &'a crate::radar::RadarTextPaintPlan,
    mut text_receipt: Option<&mut crate::radar::RadarTextPaintReceipt>,
    mut axis_receipt: Option<&mut crate::radar::RadarAxisPaintReceipt>,
    typography: &'a crate::radar::RadarTypographyThemePlan,
    series_colors: &[String],
    mut record_series_rule: impl FnMut(usize, &str),
) -> Result<crate::radar::RadarTypographyCssEmission<'a>>
where
    I: SvgDiagramIdValue,
{
    // Keep `:root` last (matches upstream Mermaid radar SVG baselines).
    let diagram_id = super::super::util::css_selector_diagram_id(diagram_id);
    let font_family_css = typography.font_family_css();
    let font_size_css = typography.font_size_css();
    let title_fill_css = text_paint.title_color();
    let base_font_emission = write_mermaid_base_css_prefix_with_font_emission(
        out,
        diagram_id,
        MermaidBaseCss {
            font_family: font_family_css,
            font_size_css,
            normal_edge_stroke_width_css: "1px",
            text_color: text_paint.text_color(),
            line_color: axis_paint.line_color(),
            error_bkg: &theme.error_bkg_color,
            error_text: &theme.error_text_color,
        },
    )?;

    if let Some(receipt) = text_receipt.as_deref_mut() {
        receipt.record_base_css(text_paint.text_color());
    }
    if let Some(receipt) = axis_receipt.as_deref_mut() {
        receipt.record_base_line_css(axis_paint.line_color());
    }

    let _ = write!(
        out,
        r#"#{} .radarTitle{{font-size:{};color:{};fill:{};dominant-baseline:hanging;text-anchor:middle;}}"#,
        diagram_id, font_size_css, title_fill_css, title_fill_css
    );
    out.checkpoint()?;
    if let Some(receipt) = text_receipt {
        receipt.record_title_css("radarTitle", title_fill_css, title_fill_css);
    }
    let _ = write!(
        out,
        r#"#{} .radarAxisLine{{stroke:{};stroke-width:{};}}"#,
        diagram_id,
        axis_paint.axis_color(),
        fmt(theme.axis_stroke_width)
    );
    out.checkpoint()?;
    if let Some(receipt) = axis_receipt.as_deref_mut() {
        receipt.record_axis_line_css("radarAxisLine", axis_paint.axis_color());
    }
    let _ = write!(
        out,
        r#"#{} .radarAxisLabel{{font-size:{}px;color:{};}}"#,
        diagram_id,
        fmt(theme.axis_label_font_size),
        axis_paint.axis_color()
    );
    out.checkpoint()?;
    if let Some(receipt) = axis_receipt {
        receipt.record_axis_label_css("radarAxisLabel", axis_paint.axis_color());
    }
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
        r#"#{} .radarLegendText{{text-anchor:start;font-size:{}px;dominant-baseline:hanging;}}"#,
        diagram_id,
        fmt(theme.legend_font_size)
    );
    out.checkpoint()?;

    for (i, c) in series_colors.iter().enumerate() {
        let _ = write!(
            out,
            r#"#{} .radarCurve-{}{{color:{};fill:{};fill-opacity:{};stroke:{};stroke-width:{};}}#{} .radarLegendBox-{}{{fill:{};fill-opacity:{};stroke:{};}}"#,
            diagram_id,
            i,
            c,
            c,
            fmt(theme.curve_opacity),
            c,
            fmt(theme.curve_stroke_width),
            diagram_id,
            i,
            c,
            fmt(theme.curve_opacity),
            c
        );
        out.checkpoint()?;
        record_series_rule(i, c);
    }

    let root_font_emission =
        write_mermaid_base_css_root_rule_with_font_emission(out, diagram_id, font_family_css)?;

    Ok(
        crate::radar::RadarTypographyCssEmission::from_successful_writes(
            base_font_emission.diagram_root_font_family_css(),
            base_font_emission.nested_svg_font_family_css(),
            root_font_emission.font_family_css(),
            base_font_emission.diagram_root_font_size_css(),
            base_font_emission.nested_svg_font_size_css(),
            font_size_css,
        ),
    )
}

fn write_radar_axes(
    out: &mut impl SvgOutput,
    axes: &[crate::model::RadarAxisLayout],
    mut record_axis_label: impl FnMut(usize, &str),
) -> Result<()> {
    for (axis_index, axis) in axes.iter().enumerate() {
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
            x2 = fmt_display(axis.line_x2),
            y2 = fmt_display(axis.line_y2),
            x = fmt_display(axis.label_x + label_padding * cos_a),
            y = fmt_display(axis.label_y + label_padding * sin_a),
            label = escape_xml(&axis.label)
        );
        out.checkpoint()?;
        record_axis_label(axis_index, &axis.label);
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn render_radar_diagram_svg_model(
    layout: &RadarDiagramLayout,
    model: &RadarDiagramRenderModel,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let effective_config = merman_core::MermaidConfig::from_value(effective_config.clone());
    let series_paint = crate::radar::RadarSeriesPaintPlan::baseline(&effective_config, model);
    let typography = crate::radar::RadarTypographyThemePlan::resolve(None, &effective_config);
    let title_theme = crate::radar::RadarTitleThemePlan::baseline();
    let axis_paint =
        crate::radar::RadarAxisPaintPlan::baseline(&effective_config, layout.axes.len());
    let text_paint =
        crate::radar::RadarTextPaintPlan::baseline(&effective_config, layout, model, diagram_title);
    render_radar_diagram_svg_model_with_theme_plans(
        layout,
        model,
        &series_paint,
        &title_theme,
        &axis_paint,
        &text_paint,
        &typography,
        effective_config.as_value(),
        diagram_title,
        options,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "The SVG writer takes geometry, resolved styles, and terminal evidence separately."
)]
pub(crate) fn render_radar_diagram_svg_model_with_theme_plans(
    layout: &RadarDiagramLayout,
    model: &RadarDiagramRenderModel,
    series_paint: &crate::radar::RadarSeriesPaintPlan,
    title_theme: &crate::radar::RadarTitleThemePlan,
    axis_paint: &crate::radar::RadarAxisPaintPlan,
    text_paint: &crate::radar::RadarTextPaintPlan,
    typography: &crate::radar::RadarTypographyThemePlan,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id_or("radar");

    let has_acc_title = model
        .acc_title
        .as_deref()
        .is_some_and(|s| !s.trim().is_empty());
    let has_acc_descr = model
        .acc_descr
        .as_deref()
        .is_some_and(|s| !s.trim().is_empty());

    let render_settings = crate::radar::RadarConfigView::new(effective_config).render_settings();

    let aria_describedby = has_acc_descr.then(|| format!("chart-desc-{diagram_id}"));
    let aria_labelledby = has_acc_title.then(|| format!("chart-title-{diagram_id}"));
    let root_extra_attrs: [(&str, &str); 1] = [("overflow", "visible")];

    let mut out = BoundedSvgOutput::new(options.work_meter());
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
    let root_document = root_svg::RootViewportContext::new(
        crate::DiagramFamilyId::RADAR,
        diagram_id,
    )
    .write_open(
        &mut out,
        root_svg::RootViewportSpec::mermaid(
            root_svg::DiagramBounds::from_view_box(0.0, 0.0, layout.svg_width, layout.svg_height),
            render_settings.use_max_width,
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

    let theme = MermaidThemeAdapter::new(effective_config).radar();
    out.push_str("<style>");
    out.checkpoint()?;
    let mut series_paint_receipt = series_paint.begin_terminal_receipt();
    let mut title_theme_receipt = title_theme.begin_terminal_receipt();
    let mut axis_paint_receipt = axis_paint.begin_terminal_receipt();
    let mut text_paint_receipt = text_paint.begin_terminal_receipt();
    let mut typography_receipt =
        typography.begin_terminal_receipt(layout.axes.len(), layout.legend_items.len());
    let typography_css_emission = write_radar_css(
        &mut out,
        diagram_id,
        &theme,
        axis_paint,
        text_paint,
        text_paint_receipt.as_mut(),
        axis_paint_receipt.as_mut(),
        typography,
        series_paint.colors(),
        |rule_index, color| {
            if rule_index < series_paint.curve_count()
                && let Some(receipt) = series_paint_receipt.as_mut()
            {
                receipt.record_series_rule(series_paint, rule_index, color);
            }
        },
    )?;
    out.push_str("</style><g/>");
    out.checkpoint()?;
    if let Some(receipt) = typography_receipt.as_mut() {
        receipt.record_css_emission(typography_css_emission);
    }
    if let Some(receipt) = title_theme_receipt.as_mut() {
        receipt.record_stylesheet("radarTitle", text_paint.title_color());
    }

    let _ = write!(
        &mut out,
        r#"<g transform="translate({x}, {y})">"#,
        x = fmt_display(layout.center_x),
        y = fmt_display(layout.center_y)
    );
    out.checkpoint()?;

    for graticule in &layout.graticules {
        if graticule.kind == "polygon" {
            if graticule.points.is_empty() {
                out.push_str(r#"<polygon points="" class="radarGraticule"/>"#);
            } else {
                let points = fmt_points(&graticule.points);
                let _ = write!(
                    &mut out,
                    r#"<polygon points="{points}" class="radarGraticule"/>"#,
                    points = escape_xml(&points)
                );
            }
        } else if let Some(r) = graticule.r {
            let _ = write!(
                &mut out,
                r#"<circle r="{r}" class="radarGraticule"/>"#,
                r = fmt_display(r)
            );
        }
        out.checkpoint()?;
    }
    write_radar_axes(&mut out, &layout.axes, |axis_index, label| {
        if let Some(receipt) = text_paint_receipt.as_mut() {
            receipt.record_axis(axis_index, "radarAxisLabel", label);
        }
        if let Some(receipt) = axis_paint_receipt.as_mut() {
            receipt.record_axis_line(axis_index, "radarAxisLine");
            receipt.record_axis_label(axis_index, "radarAxisLabel");
        }
        if let Some(receipt) = typography_receipt.as_mut() {
            receipt.record_axis_label(axis_index, label);
        }
    })?;

    let polygon_curves = layout
        .graticules
        .first()
        .is_some_and(|g| g.kind.trim() == "polygon");
    for (curve_index, curve) in layout.curves.iter().enumerate() {
        if polygon_curves && !curve.points.is_empty() {
            let points = fmt_points(&curve.points);
            let _ = write!(
                &mut out,
                r#"<polygon points="{points}" class="radarCurve-{idx}"/>"#,
                points = escape_xml(&points),
                idx = curve.class_index
            );
        } else {
            let _ = write!(
                &mut out,
                r#"<path d="{d}" class="radarCurve-{idx}"/>"#,
                d = escape_xml(&curve.path_d),
                idx = curve.class_index
            );
        }
        out.checkpoint()?;
        if let Some(receipt) = series_paint_receipt.as_mut() {
            receipt.record_curve(series_paint, curve_index, curve.class_index);
        }
    }

    for (legend_index, item) in layout.legend_items.iter().enumerate() {
        let _ = write!(
            &mut out,
            r#"<g transform="translate({x}, {y})">"#,
            x = fmt_display(item.x),
            y = fmt_display(item.y)
        );
        out.checkpoint()?;
        let _ = write!(
            &mut out,
            r#"<rect width="{size}" height="{size}" class="radarLegendBox-{idx}"/>"#,
            size = fmt_display(12.0),
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
            x = fmt_display(16.0),
            y = fmt_display(0.0),
            text = escape_xml(label)
        );
        out.checkpoint()?;
        out.push_str("</g>");
        out.checkpoint()?;
        if let Some(receipt) = series_paint_receipt.as_mut() {
            receipt.record_legend(series_paint, legend_index, item.class_index);
        }
        if let Some(receipt) = text_paint_receipt.as_mut() {
            receipt.record_legend(legend_index, "radarLegendText", label);
        }
        if let Some(receipt) = typography_receipt.as_mut() {
            receipt.record_legend_label(legend_index, label);
        }
    }

    let title = model
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .or_else(|| {
            diagram_title
                .map(str::trim)
                .filter(|title| !title.is_empty())
        });
    let title_visible = match title {
        Some(t) => {
            let _ = write!(
                &mut out,
                r#"<text class="radarTitle" x="0" y="{y}">{text}</text>"#,
                y = fmt(layout.title_y),
                text = escape_xml(t)
            );
            true
        }
        None => {
            let _ = write!(
                &mut out,
                r#"<text class="radarTitle" x="0" y="{y}"/>"#,
                y = fmt(layout.title_y)
            );
            false
        }
    };
    out.checkpoint()?;
    if let Some(receipt) = text_paint_receipt.as_mut() {
        receipt.record_title("radarTitle", title_visible);
    }
    if let Some(receipt) = typography_receipt.as_mut() {
        receipt.record_title(title_visible);
    }
    if let Some(receipt) = title_theme_receipt.as_mut() {
        receipt.record_title_text("radarTitle");
    }

    out.push_str("</g></svg>\n");
    let rooted = root_document.complete(out.finish()?)?;
    if let Some(receipt) = series_paint_receipt
        && !series_paint.record_terminal(receipt)
    {
        return Err(crate::Error::InvalidModel {
            message: "Radar series paint receipt did not match the terminal SVG".to_string(),
        });
    }
    if let Some(receipt) = typography_receipt
        && !typography.record_terminal(receipt)
    {
        return Err(crate::Error::InvalidModel {
            message: "Radar typography receipt did not match the terminal SVG".to_string(),
        });
    }
    if let Some(receipt) = title_theme_receipt
        && !title_theme.record_terminal(receipt)
    {
        return Err(crate::Error::InvalidModel {
            message: "Radar title paint receipt did not match the terminal SVG".to_string(),
        });
    }
    if let Some(receipt) = axis_paint_receipt
        && !axis_paint.record_terminal(receipt)
    {
        return Err(crate::Error::InvalidModel {
            message: "Radar axis paint receipt did not match the terminal SVG".to_string(),
        });
    }
    if let Some(receipt) = text_paint_receipt
        && !text_paint.record_terminal(receipt)
    {
        return Err(crate::Error::InvalidModel {
            message: "Radar text paint receipt did not match the terminal SVG".to_string(),
        });
    }
    Ok(rooted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;
    use crate::model::RadarAxisLayout;
    use std::fmt;
    use std::ops::Range;

    fn radar_css_for_test(config: &serde_json::Value) -> String {
        let effective_config = merman_core::MermaidConfig::from_value(config.clone());
        let adapter = MermaidThemeAdapter::new(effective_config.as_value());
        let theme = adapter.radar();
        let series_colors = adapter.radar_series_colors();
        let typography = crate::radar::RadarTypographyThemePlan::resolve(None, &effective_config);
        let mut css = String::new();
        write_radar_css(
            &mut css,
            "radar",
            &theme,
            &crate::radar::RadarAxisPaintPlan::baseline(&effective_config, 0),
            &crate::radar::RadarTextPaintPlan::baseline(
                &effective_config,
                &crate::model::RadarDiagramLayout {
                    bounds: None,
                    svg_width: 0.0,
                    svg_height: 0.0,
                    center_x: 0.0,
                    center_y: 0.0,
                    radius: 0.0,
                    axis_label_factor: 1.0,
                    title_y: 0.0,
                    axes: Vec::new(),
                    graticules: Vec::new(),
                    curves: Vec::new(),
                    legend_items: Vec::new(),
                },
                &RadarDiagramRenderModel::default(),
                None,
            ),
            None,
            None,
            &typography,
            &series_colors,
            |_, _| {},
        )
        .expect("String-backed Radar CSS writer");
        css
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

        let error = write_radar_axes(&mut out, &axes, |_, _| {})
            .expect_err("the rejecting sink must stop Radar axis emission");

        assert!(matches!(error, crate::Error::InvalidModel { .. }));
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
        let options = SvgRenderOptions {
            diagram_id: Some("radarRoot".to_string()),
            ..SvgRenderOptions::default()
        };

        let svg = with_test_svg_execution(DiagramFamilyId::RADAR, &options, |options| {
            render_radar_diagram_svg_model(
                &layout,
                &RadarDiagramRenderModel::default(),
                &serde_json::json!({}),
                None,
                options,
            )
        })
        .unwrap();
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
        let options = SvgRenderOptions {
            diagram_id: Some("radarFixed".to_string()),
            ..SvgRenderOptions::default()
        };

        let svg = with_test_svg_execution(DiagramFamilyId::RADAR, &options, |options| {
            render_radar_diagram_svg_model(
                &layout,
                &RadarDiagramRenderModel::default(),
                &serde_json::json!({"radar": {"useMaxWidth": false}}),
                None,
                options,
            )
        })
        .unwrap();
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

        let request = SvgRenderOptions::default();
        let svg = with_test_svg_execution(DiagramFamilyId::RADAR, &request, |options| {
            render_radar_diagram_svg_model(
                &layout,
                &RadarDiagramRenderModel::default(),
                &serde_json::json!({}),
                None,
                options,
            )
        })
        .unwrap();
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
