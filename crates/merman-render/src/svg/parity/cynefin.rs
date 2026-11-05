use super::*;
use crate::diagram_theme::{ResolvedStyleProperty, ThemeTarget};
use crate::family::{TerminalPaint, TerminalPaintPlan};
use merman_model::families::cynefin::CynefinDiagramModel;

fn domain_ordinal(name: &str) -> usize {
    match name {
        "complex" => 1,
        "complicated" => 2,
        "chaotic" => 3,
        "clear" => 4,
        _ => 5,
    }
}

fn domain_fill_path(name: &str) -> &'static str {
    match name {
        "complex" => "themeVariables.cynefin.complexBg",
        "complicated" => "themeVariables.cynefin.complicatedBg",
        "chaotic" => "themeVariables.cynefin.chaoticBg",
        "clear" => "themeVariables.cynefin.clearBg",
        _ => "themeVariables.cynefin.confusionBg",
    }
}

fn write_paint_style(
    out: &mut impl SvgOutput,
    plan: &mut TerminalPaintPlan<'_>,
    fill: Option<TerminalPaint>,
    stroke: Option<TerminalPaint>,
) -> Result<()> {
    if fill.is_none() && stroke.is_none() {
        return Ok(());
    }
    out.push_str(r#" style=""#);
    for (property, paint) in [("fill", fill), ("stroke", stroke)] {
        if let Some(paint) = paint {
            let _ = write!(out, "{property}:{};", escape_attr_display(paint.css()));
            out.checkpoint()?;
            plan.record(Some(&paint), paint.css());
        }
    }
    out.push_str(r#"""#);
    out.checkpoint()
}

fn write_label_paint(
    out: &mut impl SvgOutput,
    plan: &mut TerminalPaintPlan<'_>,
    target: ThemeTarget,
    ordinal: usize,
    domain_label: bool,
    base: &str,
) -> Result<bool> {
    let style = plan.style(target, ordinal, !domain_label)?;
    let paths: &[&str] = if domain_label {
        &[
            "themeVariables.cynefin.labelColor",
            "themeVariables.primaryTextColor",
        ]
    } else {
        &[
            "themeVariables.cynefin.textColor",
            "themeVariables.textColor",
        ]
    };
    let fill = plan.paint(
        target,
        ordinal,
        style.as_ref(),
        ResolvedStyleProperty::Fill,
        paths,
        base,
    );
    plan.unsupported_property(target, style.as_ref(), ResolvedStyleProperty::Stroke);
    let overridden = fill.is_some();
    write_paint_style(out, plan, fill, None)?;
    Ok(overridden)
}

pub(crate) fn render_cynefin_diagram_svg_model(
    layout: &CynefinDiagramLayout,
    model: &CynefinDiagramModel,
    diagram_title: Option<&str>,
    typography_theme: &crate::cynefin::CynefinTypographyThemePlan,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id_or("cynefin");
    let acc_title = model.acc_title.as_deref().filter(|value| !value.is_empty());
    let acc_descr = model.acc_descr.as_deref().filter(|value| !value.is_empty());
    let aria_labelledby = acc_title.map(|_| format!("chart-title-{diagram_id}"));
    let aria_describedby = acc_descr.map(|_| format!("chart-desc-{diagram_id}"));
    let root_bounds =
        root_svg::DiagramBounds::from_view_box(0.0, 0.0, layout.total_width, layout.total_height);
    let root_spec = root_svg::RootViewportSpec::mermaid(root_bounds, layout.use_max_width);
    let theme = typography_theme.colors();
    let title = model
        .title
        .as_deref()
        .filter(|value| !value.is_empty())
        .or_else(|| diagram_title.filter(|value| !value.is_empty()));
    let seed = crate::cynefin::resolve_seed(layout.seed, diagram_id.semantic_str());
    let mut surface_receipt = typography_theme.begin_terminal_receipt();
    let mut paint_plan = TerminalPaintPlan::new(
        options.resolved_theme(),
        typography_theme.config(),
        crate::cynefin::PAINT_TARGETS,
        options.work_meter(),
    )?;
    let base = typography_theme.base_colors();

    let mut out = BoundedSvgOutput::new(options.work_meter());
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "cynefin");
    root_chrome.aria_labelledby = aria_labelledby.as_deref();
    root_chrome.aria_describedby = aria_describedby.as_deref();
    root_chrome.dom.trailing_newline = false;
    let root_document =
        root_svg::RootViewportContext::new(crate::DiagramFamilyId::CYNEFIN, diagram_id)
            .write_open(&mut out, root_spec, root_chrome)?;
    options.checkpoint_emit()?;

    if let Some(title) = acc_title {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{diagram_id}">{}</title>"#,
            escape_xml_display(title)
        );
        out.checkpoint()?;
    }
    if let Some(descr) = acc_descr {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{diagram_id}">{}</desc>"#,
            escape_xml_display(descr)
        );
        out.checkpoint()?;
    }

    out.push_str("<style>");
    out.checkpoint()?;
    write_cynefin_css(
        &mut super::output::XmlTextOutput::new(&mut out),
        diagram_id,
        theme,
        typography_theme,
    )?;
    out.push_str("</style><g/>");
    out.checkpoint()?;
    if let Some(title) = acc_title {
        let _ = write!(&mut out, "<title>{}</title>", escape_xml_display(title));
        out.checkpoint()?;
    }
    if let Some(descr) = acc_descr {
        let _ = write!(&mut out, "<desc>{}</desc>", escape_xml_display(descr));
        out.checkpoint()?;
    }

    let _ = write!(
        &mut out,
        r#"<g transform="translate({}, {})">"#,
        fmt(layout.padding),
        fmt(layout.padding)
    );
    out.checkpoint()?;
    push_backgrounds(&mut out, layout, theme, &mut paint_plan)?;
    push_boundaries(&mut out, layout, seed, theme, &mut paint_plan)?;
    push_labels(
        &mut out,
        layout,
        base,
        &mut paint_plan,
        &mut surface_receipt,
    )?;
    if layout.show_domain_descriptions {
        push_subtitles(
            &mut out,
            layout,
            base,
            &mut paint_plan,
            &mut surface_receipt,
        )?;
    }
    push_items(
        &mut out,
        layout,
        theme,
        base,
        &mut paint_plan,
        &mut surface_receipt,
    )?;
    push_transitions(
        &mut out,
        layout,
        diagram_id,
        options,
        base,
        &mut paint_plan,
        &mut surface_receipt,
    )?;
    if let Some(title) = title {
        let _ = write!(
            &mut out,
            r#"<text class="cynefinTitle" x="{}" y="{}" text-anchor="middle" dominant-baseline="middle""#,
            fmt(layout.width / 2.0),
            fmt(-layout.padding / 2.0)
        );
        write_label_paint(
            &mut out,
            &mut paint_plan,
            ThemeTarget::Title,
            1,
            true,
            &base.label_color,
        )?;
        let _ = write!(&mut out, ">{}</text>", escape_xml_display(title));
        out.checkpoint()?;
        surface_receipt.record_text(
            crate::cynefin::CynefinTextRole::Title,
            "cynefinTitle",
            title,
        );
    }
    out.push_str("</g>");
    out.checkpoint()?;

    if !layout.transitions.is_empty() {
        let _ = write!(
            &mut out,
            r#"<defs><marker id="cynefin-arrow-{diagram_id}" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" class="cynefinArrowHead""#,
        );
        let style = paint_plan.style(ThemeTarget::Marker, 1, false)?;
        let paths = &[
            "themeVariables.cynefin.arrowColor",
            "themeVariables.lineColor",
        ];
        let mut fill = paint_plan.paint(
            ThemeTarget::Marker,
            1,
            style.as_ref(),
            ResolvedStyleProperty::Fill,
            paths,
            &base.arrow_color,
        );
        if style
            .as_ref()
            .is_some_and(|style| style.fill_resolution().specified().is_unspecified())
        {
            let edge_style = paint_plan.style(ThemeTarget::Edge, 1, false)?;
            fill = paint_plan.paint(
                ThemeTarget::Edge,
                1,
                edge_style.as_ref(),
                ResolvedStyleProperty::Stroke,
                paths,
                &base.arrow_color,
            );
        }
        let stroke = paint_plan.paint(
            ThemeTarget::Marker,
            1,
            style.as_ref(),
            ResolvedStyleProperty::Stroke,
            paths,
            "none",
        );
        write_paint_style(&mut out, &mut paint_plan, fill, stroke)?;
        out.push_str("></path></marker></defs>");
        out.checkpoint()?;
    }
    surface_receipt.paint_evidence = paint_plan.finish();
    out.push_str("</svg>\n");
    let rooted_svg = root_document.complete(out.finish()?)?;
    if !typography_theme.record_terminal(surface_receipt) {
        return Err(crate::Error::InvalidModel {
            message: "Cynefin typography receipt was recorded more than once".to_string(),
        });
    }
    Ok(rooted_svg)
}

fn push_backgrounds(
    out: &mut impl SvgOutput,
    layout: &CynefinDiagramLayout,
    theme: &crate::cynefin::CynefinTheme,
    paint_plan: &mut TerminalPaintPlan<'_>,
) -> Result<()> {
    out.push_str(r#"<g class="cynefin-backgrounds">"#);
    out.checkpoint()?;
    for domain_name in crate::cynefin::quadrant_domains() {
        let Some(domain) = layout
            .domain_layouts
            .iter()
            .find(|item| item.name == *domain_name)
        else {
            continue;
        };
        let _ = write!(
            out,
            r#"<rect class="cynefinDomain" x="{}" y="{}" width="{}" height="{}" fill="{}" fill-opacity="0.4" stroke="none""#,
            fmt(domain.x),
            fmt(domain.y),
            fmt(domain.width),
            fmt(domain.height),
            escape_attr_display(crate::cynefin::domain_fill(theme, domain_name))
        );
        let ordinal = domain_ordinal(domain_name);
        let style = paint_plan.style(ThemeTarget::Node, ordinal, false)?;
        let fill = paint_plan.paint(
            ThemeTarget::Node,
            ordinal,
            style.as_ref(),
            ResolvedStyleProperty::Fill,
            &[domain_fill_path(domain_name)],
            crate::cynefin::domain_fill(theme, domain_name),
        );
        let stroke = paint_plan.paint(
            ThemeTarget::Node,
            ordinal,
            style.as_ref(),
            ResolvedStyleProperty::Stroke,
            &[
                "themeVariables.cynefin.boundaryColor",
                "themeVariables.lineColor",
            ],
            "none",
        );
        write_paint_style(out, paint_plan, fill, stroke)?;
        out.push_str("></rect>");
        out.checkpoint()?;
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn push_boundaries(
    out: &mut impl SvgOutput,
    layout: &CynefinDiagramLayout,
    seed: f64,
    theme: &crate::cynefin::CynefinTheme,
    paint_plan: &mut TerminalPaintPlan<'_>,
) -> Result<()> {
    out.push_str(r#"<g class="cynefin-boundaries">"#);
    out.checkpoint()?;
    let fold_path = crate::cynefin::generate_fold_path(
        layout.width,
        layout.height,
        seed,
        Some(layout.boundary_amplitude),
    );
    let horizontal_path = crate::cynefin::generate_horizontal_boundary(
        layout.width,
        layout.height,
        seed + 100.0,
        Some(layout.boundary_amplitude),
    );
    let cliff_path = crate::cynefin::generate_cliff_path(layout.width, layout.height);
    for (class, path, ordinal, paths, base) in [
        (
            "cynefinBoundary",
            &fold_path,
            1,
            &[
                "themeVariables.cynefin.boundaryColor",
                "themeVariables.lineColor",
            ][..],
            &theme.boundary_color,
        ),
        (
            "cynefinBoundary",
            &horizontal_path,
            1,
            &[
                "themeVariables.cynefin.boundaryColor",
                "themeVariables.lineColor",
            ][..],
            &theme.boundary_color,
        ),
        (
            "cynefinCliff",
            &cliff_path,
            4,
            &["themeVariables.cynefin.cliffColor"][..],
            &theme.cliff_color,
        ),
    ] {
        let _ = write!(
            out,
            r#"<path class="{class}" d="{}" fill="none""#,
            escape_attr_display(path)
        );
        let style = paint_plan.style(ThemeTarget::Node, ordinal, false)?;
        let stroke = paint_plan.paint(
            ThemeTarget::Node,
            ordinal,
            style.as_ref(),
            ResolvedStyleProperty::Stroke,
            paths,
            base,
        );
        write_paint_style(out, paint_plan, None, stroke)?;
        out.push_str("></path>");
        out.checkpoint()?;
    }
    out.push_str("</g>");
    out.checkpoint()?;

    let confusion_path = crate::cynefin::generate_confusion_path(
        layout.width / 2.0,
        layout.height / 2.0,
        layout.width * 0.15,
        layout.height * 0.15,
    );
    let _ = write!(
        out,
        r#"<path class="cynefinConfusion" d="{}" fill="{}" fill-opacity="0.5""#,
        escape_attr_display(&confusion_path),
        escape_attr_display(&theme.confusion_bg)
    );
    let style = paint_plan.style(ThemeTarget::Node, 5, false)?;
    let fill = paint_plan.paint(
        ThemeTarget::Node,
        5,
        style.as_ref(),
        ResolvedStyleProperty::Fill,
        &[domain_fill_path("confusion")],
        &theme.confusion_bg,
    );
    let stroke = paint_plan.paint(
        ThemeTarget::Node,
        5,
        style.as_ref(),
        ResolvedStyleProperty::Stroke,
        &[
            "themeVariables.cynefin.boundaryColor",
            "themeVariables.lineColor",
        ],
        &theme.boundary_color,
    );
    write_paint_style(out, paint_plan, fill, stroke)?;
    out.push_str("></path>");
    out.checkpoint()
}

fn push_labels(
    out: &mut impl SvgOutput,
    layout: &CynefinDiagramLayout,
    base: &crate::cynefin::CynefinTheme,
    paint_plan: &mut TerminalPaintPlan<'_>,
    surface_receipt: &mut crate::cynefin::CynefinSurfaceReceipt,
) -> Result<()> {
    out.push_str(r#"<g class="cynefin-labels">"#);
    out.checkpoint()?;
    for domain_name in crate::cynefin::quadrant_domains() {
        let Some(domain) = layout
            .domain_layouts
            .iter()
            .find(|item| item.name == *domain_name)
        else {
            continue;
        };
        let y = if layout.show_domain_descriptions {
            domain.cy - 30.0
        } else {
            domain.cy
        };
        let _ = write!(
            out,
            r#"<text class="cynefinDomainLabel" x="{}" y="{}" text-anchor="middle" dominant-baseline="middle""#,
            fmt(domain.cx),
            fmt(y)
        );
        write_label_paint(
            out,
            paint_plan,
            ThemeTarget::NodeLabel,
            domain_ordinal(domain_name),
            true,
            &base.label_color,
        )?;
        let _ = write!(
            out,
            ">{}</text>",
            escape_xml_display(crate::cynefin::domain_title(domain_name))
        );
        out.checkpoint()?;
        surface_receipt.record_text(
            crate::cynefin::CynefinTextRole::DomainLabel,
            "cynefinDomainLabel",
            crate::cynefin::domain_title(domain_name),
        );
    }
    let y = if layout.show_domain_descriptions {
        layout.height / 2.0 - 10.0
    } else {
        layout.height / 2.0
    };
    let _ = write!(
        out,
        r#"<text class="cynefinDomainLabel" x="{}" y="{}" text-anchor="middle" dominant-baseline="middle""#,
        fmt(layout.width / 2.0),
        fmt(y)
    );
    write_label_paint(
        out,
        paint_plan,
        ThemeTarget::NodeLabel,
        5,
        true,
        &base.label_color,
    )?;
    out.push_str(">Confusion</text></g>");
    out.checkpoint()?;
    surface_receipt.record_text(
        crate::cynefin::CynefinTextRole::DomainLabel,
        "cynefinDomainLabel",
        "Confusion",
    );
    Ok(())
}

fn push_subtitles(
    out: &mut impl SvgOutput,
    layout: &CynefinDiagramLayout,
    base: &crate::cynefin::CynefinTheme,
    paint_plan: &mut TerminalPaintPlan<'_>,
    surface_receipt: &mut crate::cynefin::CynefinSurfaceReceipt,
) -> Result<()> {
    out.push_str(r#"<g class="cynefin-subtitles">"#);
    out.checkpoint()?;
    for domain_name in crate::cynefin::quadrant_domains() {
        let Some(domain) = layout
            .domain_layouts
            .iter()
            .find(|item| item.name == *domain_name)
        else {
            continue;
        };
        let (model, practice) = crate::cynefin::domain_model_and_practice(domain_name);
        for (text, y) in [(model, domain.cy - 10.0), (practice, domain.cy + 5.0)] {
            let _ = write!(
                out,
                r#"<text class="cynefinSubtitle" x="{}" y="{}" text-anchor="middle" dominant-baseline="middle""#,
                fmt(domain.cx),
                fmt(y)
            );
            if write_label_paint(
                out,
                paint_plan,
                ThemeTarget::NodeLabel,
                domain_ordinal(domain_name),
                false,
                &base.text_color,
            )? {
                surface_receipt.record_text_fill_override();
            }
            let _ = write!(out, ">{}</text>", escape_xml_display(text));
            out.checkpoint()?;
        }
        surface_receipt.record_text(
            crate::cynefin::CynefinTextRole::Subtitle,
            "cynefinSubtitle",
            model,
        );
        surface_receipt.record_text(
            crate::cynefin::CynefinTextRole::Subtitle,
            "cynefinSubtitle",
            practice,
        );
    }
    let _ = write!(
        out,
        r#"<text class="cynefinSubtitle" x="{}" y="{}" text-anchor="middle" dominant-baseline="middle""#,
        fmt(layout.width / 2.0),
        fmt(layout.height / 2.0 + 8.0)
    );
    if write_label_paint(
        out,
        paint_plan,
        ThemeTarget::NodeLabel,
        5,
        false,
        &base.text_color,
    )? {
        surface_receipt.record_text_fill_override();
    }
    out.push_str(">Disorder</text></g>");
    out.checkpoint()?;
    surface_receipt.record_text(
        crate::cynefin::CynefinTextRole::Subtitle,
        "cynefinSubtitle",
        "Disorder",
    );
    Ok(())
}

fn push_items(
    out: &mut impl SvgOutput,
    layout: &CynefinDiagramLayout,
    theme: &crate::cynefin::CynefinTheme,
    base: &crate::cynefin::CynefinTheme,
    paint_plan: &mut TerminalPaintPlan<'_>,
    surface_receipt: &mut crate::cynefin::CynefinSurfaceReceipt,
) -> Result<()> {
    out.push_str(r#"<g class="cynefin-items">"#);
    out.checkpoint()?;
    for item in &layout.items {
        let fill = crate::cynefin::domain_fill(theme, &item.domain);
        let ordinal = domain_ordinal(&item.domain);
        let rect_class = if item.overflow {
            "cynefinItemOverflow"
        } else {
            "cynefinItem"
        };
        let _ = write!(
            out,
            r#"<g transform="translate({}, {})"><rect class="{}" x="0" y="0" width="{}" height="{}" rx="4" ry="4" fill="{}" fill-opacity="{}""#,
            fmt(item.x),
            fmt(item.y),
            rect_class,
            fmt(item.width),
            fmt(item.height),
            escape_attr_display(fill),
            if item.overflow { "0.6" } else { "0.95" }
        );
        let style = paint_plan.style(ThemeTarget::Node, ordinal, false)?;
        let typed_fill = paint_plan.paint(
            ThemeTarget::Node,
            ordinal,
            style.as_ref(),
            ResolvedStyleProperty::Fill,
            &[domain_fill_path(&item.domain)],
            fill,
        );
        let typed_stroke = paint_plan.paint(
            ThemeTarget::Node,
            ordinal,
            style.as_ref(),
            ResolvedStyleProperty::Stroke,
            &[
                "themeVariables.cynefin.boundaryColor",
                "themeVariables.lineColor",
            ],
            &base.boundary_color,
        );
        write_paint_style(out, paint_plan, typed_fill, typed_stroke)?;
        let _ = write!(
            out,
            r#"></rect><text class="cynefinItemText" x="{}" y="{}" text-anchor="middle" dominant-baseline="central""#,
            fmt(item.text_x),
            fmt(item.text_y)
        );
        if !item.label.trim().is_empty() {
            if write_label_paint(
                out,
                paint_plan,
                ThemeTarget::NodeLabel,
                ordinal,
                false,
                &base.text_color,
            )? {
                surface_receipt.record_text_fill_override();
            }
        }
        let _ = write!(out, ">{}</text></g>", escape_xml_display(&item.label));
        out.checkpoint()?;
        surface_receipt.record_text(
            crate::cynefin::CynefinTextRole::Item,
            "cynefinItemText",
            &item.label,
        );
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn push_transitions(
    out: &mut impl SvgOutput,
    layout: &CynefinDiagramLayout,
    diagram_id: SvgDiagramId<'_>,
    options: &SvgExecution<'_>,
    base: &crate::cynefin::CynefinTheme,
    paint_plan: &mut TerminalPaintPlan<'_>,
    surface_receipt: &mut crate::cynefin::CynefinSurfaceReceipt,
) -> Result<()> {
    if layout.transitions.is_empty() {
        return Ok(());
    }
    out.push_str(r#"<g class="cynefin-arrows">"#);
    out.checkpoint()?;
    for (index, transition) in layout.transitions.iter().enumerate() {
        options.checkpoint_emit()?;
        let d = format!(
            "M{},{} Q{},{} {},{}",
            fmt(transition.x1),
            fmt(transition.y1),
            fmt(transition.cpx),
            fmt(transition.cpy),
            fmt(transition.x2),
            fmt(transition.y2)
        );
        let _ = write!(
            out,
            r#"<path class="cynefinArrowLine" d="{}" fill="none" marker-end="url(#cynefin-arrow-{diagram_id})""#,
            escape_attr_display(&d),
        );
        let style = paint_plan.style(ThemeTarget::Edge, index + 1, false)?;
        let stroke = paint_plan.paint(
            ThemeTarget::Edge,
            index + 1,
            style.as_ref(),
            ResolvedStyleProperty::Stroke,
            &[
                "themeVariables.cynefin.arrowColor",
                "themeVariables.lineColor",
            ],
            &base.arrow_color,
        );
        paint_plan.unsupported_property(
            ThemeTarget::Edge,
            style.as_ref(),
            ResolvedStyleProperty::Fill,
        );
        write_paint_style(out, paint_plan, None, stroke)?;
        out.push_str("></path>");
        out.checkpoint()?;
        if let Some(label) = transition
            .label
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            let _ = write!(
                out,
                r#"<text class="cynefinArrowLabel" x="{}" y="{}" text-anchor="middle" dominant-baseline="auto""#,
                fmt(transition.cpx),
                fmt(transition.cpy - 6.0)
            );
            if write_label_paint(
                out,
                paint_plan,
                ThemeTarget::EdgeLabel,
                index + 1,
                false,
                &base.text_color,
            )? {
                surface_receipt.record_text_fill_override();
            }
            let _ = write!(out, ">{}</text>", escape_xml_display(label));
            out.checkpoint()?;
            surface_receipt.record_text(
                crate::cynefin::CynefinTextRole::TransitionLabel,
                "cynefinArrowLabel",
                label,
            );
        }
        options.checkpoint_emit()?;
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn write_cynefin_css(
    out: &mut impl SvgOutput,
    diagram_id: SvgDiagramId<'_>,
    theme: &crate::cynefin::CynefinTheme,
    typography_theme: &crate::cynefin::CynefinTypographyThemePlan,
) -> Result<()> {
    let id = crate::svg::escape_css_identifier(diagram_id.semantic_str());
    let text_color = &theme.text_color;
    let common_css = typography_theme.common_css();
    common_css.write_prefix_with_font_emission(out, util::css_selector_diagram_id(diagram_id))?;
    out.checkpoint()?;
    let _ = write!(
        out,
        "#{id} .cynefinDomain{{stroke:none;}}\
#{id} .cynefinDomainLabel{{font-size:{}px;font-weight:bold;fill:{};}}\
#{id} .cynefinSubtitle{{font-size:{}px;fill:{};font-style:italic;}}\
#{id} .cynefinItem{{fill-opacity:0.95;stroke:{};stroke-width:1;}}\
#{id} .cynefinItemText{{font-size:{}px;fill:{};}}\
#{id} .cynefinItemOverflow{{fill-opacity:0.6;stroke:{};stroke-width:1;stroke-dasharray:3 2;}}\
#{id} .cynefinBoundary{{stroke:{};stroke-width:{};stroke-dasharray:6 3;}}\
#{id} .cynefinCliff{{stroke:{};stroke-width:{};}}\
#{id} .cynefinConfusion{{stroke:{};stroke-width:1.5;stroke-dasharray:4 2;}}\
#{id} .cynefinArrowLine{{stroke:{};stroke-width:{};fill:none;}}\
#{id} .cynefinArrowHead{{fill:{};stroke:none;}}\
#{id} .cynefinArrowLabel{{font-size:{}px;fill:{};}}\
#{id} .cynefinTitle{{font-size:{}px;font-weight:bold;fill:{};}}",
        fmt(theme.domain_font_size),
        theme.label_color,
        fmt((theme.item_font_size - 1.0).max(1.0)),
        text_color,
        theme.boundary_color,
        fmt(theme.item_font_size),
        text_color,
        theme.boundary_color,
        theme.boundary_color,
        fmt(theme.boundary_width),
        theme.cliff_color,
        fmt(theme.cliff_width),
        theme.boundary_color,
        theme.arrow_color,
        fmt(theme.arrow_width),
        theme.arrow_color,
        fmt((theme.item_font_size - 1.0).max(1.0)),
        text_color,
        fmt(theme.domain_font_size + 2.0),
        theme.label_color
    );
    common_css.write_root_with_font_emission(
        out,
        util::css_selector_diagram_id(diagram_id),
        diagram_id,
    )?;
    out.checkpoint()?;
    Ok(())
}
