use super::*;
use merman_core::diagrams::cynefin::CynefinDiagramRenderModel;

pub(crate) fn render_cynefin_diagram_svg_model(
    layout: &CynefinDiagramLayout,
    model: &CynefinDiagramRenderModel,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    typography_theme: &crate::cynefin::CynefinTypographyThemePlan,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id.as_deref().unwrap_or("cynefin");
    let diagram_id_esc = escape_xml(diagram_id);
    let acc_title = model.acc_title.as_deref().filter(|value| !value.is_empty());
    let acc_descr = model.acc_descr.as_deref().filter(|value| !value.is_empty());
    let aria_labelledby = acc_title.map(|_| format!("chart-title-{diagram_id}"));
    let aria_describedby = acc_descr.map(|_| format!("chart-desc-{diagram_id}"));
    let root_bounds =
        root_svg::DiagramBounds::from_view_box(0.0, 0.0, layout.total_width, layout.total_height);
    let root_spec = root_svg::RootViewportSpec::mermaid(root_bounds, layout.use_max_width);
    let theme = crate::cynefin::cynefin_theme(effective_config);
    let title = model
        .title
        .as_deref()
        .filter(|value| !value.is_empty())
        .or_else(|| diagram_title.filter(|value| !value.is_empty()));
    let seed = crate::cynefin::resolve_seed(layout.seed, diagram_id);
    let marker_id = format!("cynefin-arrow-{diagram_id}");
    let mut surface_receipt = typography_theme.begin_terminal_receipt(layout, title);

    let mut out = BoundedSvgOutput::new(options.work_meter());
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "cynefin");
    root_chrome.aria_labelledby = aria_labelledby.as_deref();
    root_chrome.aria_describedby = aria_describedby.as_deref();
    root_chrome.dom.trailing_newline = false;
    let root_document =
        root_svg::RootViewportContext::new(crate::DiagramFamilyId::CYNEFIN, diagram_id)
            .write_open(&mut out, root_spec, root_chrome)?;

    if let Some(title) = acc_title {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{}">{}</title>"#,
            diagram_id_esc,
            escape_xml_display(title)
        );
        out.checkpoint()?;
    }
    if let Some(descr) = acc_descr {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{}">{}</desc>"#,
            diagram_id_esc,
            escape_xml_display(descr)
        );
        out.checkpoint()?;
    }

    out.push_str("<style>");
    out.checkpoint()?;
    write_cynefin_css(
        &mut out,
        diagram_id,
        effective_config,
        &theme,
        typography_theme,
        &mut surface_receipt,
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
    push_backgrounds(&mut out, layout, &theme)?;
    push_boundaries(&mut out, layout, seed, &theme)?;
    push_labels(&mut out, layout, &mut surface_receipt)?;
    if layout.show_domain_descriptions {
        push_subtitles(&mut out, layout, &mut surface_receipt)?;
    }
    push_items(&mut out, layout, &theme, &mut surface_receipt)?;
    push_transitions(&mut out, layout, &marker_id, &mut surface_receipt)?;
    if let Some(title) = title {
        let _ = write!(
            &mut out,
            r#"<text class="cynefinTitle" x="{}" y="{}" text-anchor="middle" dominant-baseline="middle">{}</text>"#,
            fmt(layout.width / 2.0),
            fmt(-layout.padding / 2.0),
            escape_xml_display(title)
        );
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
            r#"<defs><marker id="{}" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" class="cynefinArrowHead"></path></marker></defs>"#,
            escape_attr_display(&marker_id)
        );
        out.checkpoint()?;
    }
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
            r#"<rect class="cynefinDomain" x="{}" y="{}" width="{}" height="{}" fill="{}" fill-opacity="0.4" stroke="none"></rect>"#,
            fmt(domain.x),
            fmt(domain.y),
            fmt(domain.width),
            fmt(domain.height),
            escape_attr_display(crate::cynefin::domain_fill(theme, domain_name))
        );
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
    let _ = write!(
        out,
        r#"<path class="cynefinBoundary" d="{}" fill="none"></path><path class="cynefinBoundary" d="{}" fill="none"></path><path class="cynefinCliff" d="{}" fill="none"></path>"#,
        escape_attr_display(&fold_path),
        escape_attr_display(&horizontal_path),
        escape_attr_display(&cliff_path)
    );
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
        r#"<path class="cynefinConfusion" d="{}" fill="{}" fill-opacity="0.5"></path>"#,
        escape_attr_display(&confusion_path),
        escape_attr_display(&theme.confusion_bg)
    );
    out.checkpoint()
}

fn push_labels(
    out: &mut impl SvgOutput,
    layout: &CynefinDiagramLayout,
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
            r#"<text class="cynefinDomainLabel" x="{}" y="{}" text-anchor="middle" dominant-baseline="middle">{}</text>"#,
            fmt(domain.cx),
            fmt(y),
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
        r#"<text class="cynefinDomainLabel" x="{}" y="{}" text-anchor="middle" dominant-baseline="middle">Confusion</text>"#,
        fmt(layout.width / 2.0),
        fmt(y)
    );
    out.push_str("</g>");
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
        let _ = write!(
            out,
            r#"<text class="cynefinSubtitle" x="{}" y="{}" text-anchor="middle" dominant-baseline="middle">{}</text><text class="cynefinSubtitle" x="{}" y="{}" text-anchor="middle" dominant-baseline="middle">{}</text>"#,
            fmt(domain.cx),
            fmt(domain.cy - 10.0),
            escape_xml_display(model),
            fmt(domain.cx),
            fmt(domain.cy + 5.0),
            escape_xml_display(practice)
        );
        out.checkpoint()?;
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
        r#"<text class="cynefinSubtitle" x="{}" y="{}" text-anchor="middle" dominant-baseline="middle">Disorder</text>"#,
        fmt(layout.width / 2.0),
        fmt(layout.height / 2.0 + 8.0)
    );
    out.push_str("</g>");
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
    surface_receipt: &mut crate::cynefin::CynefinSurfaceReceipt,
) -> Result<()> {
    out.push_str(r#"<g class="cynefin-items">"#);
    out.checkpoint()?;
    for item in &layout.items {
        let fill = crate::cynefin::domain_fill(theme, &item.domain);
        let rect_class = if item.overflow {
            "cynefinItemOverflow"
        } else {
            "cynefinItem"
        };
        let _ = write!(
            out,
            r#"<g transform="translate({}, {})"><rect class="{}" x="0" y="0" width="{}" height="{}" rx="4" ry="4" fill="{}" fill-opacity="{}"></rect><text class="cynefinItemText" x="{}" y="{}" text-anchor="middle" dominant-baseline="central">{}</text></g>"#,
            fmt(item.x),
            fmt(item.y),
            rect_class,
            fmt(item.width),
            fmt(item.height),
            escape_attr_display(fill),
            if item.overflow { "0.6" } else { "0.95" },
            fmt(item.text_x),
            fmt(item.text_y),
            escape_xml_display(&item.label)
        );
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
    marker_id: &str,
    surface_receipt: &mut crate::cynefin::CynefinSurfaceReceipt,
) -> Result<()> {
    if layout.transitions.is_empty() {
        return Ok(());
    }
    out.push_str(r#"<g class="cynefin-arrows">"#);
    out.checkpoint()?;
    for transition in &layout.transitions {
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
            r#"<path class="cynefinArrowLine" d="{}" fill="none" marker-end="url(#{})"></path>"#,
            escape_attr_display(&d),
            escape_attr_display(marker_id)
        );
        out.checkpoint()?;
        if let Some(label) = transition
            .label
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            let _ = write!(
                out,
                r#"<text class="cynefinArrowLabel" x="{}" y="{}" text-anchor="middle" dominant-baseline="auto">{}</text>"#,
                fmt(transition.cpx),
                fmt(transition.cpy - 6.0),
                escape_xml_display(label)
            );
            out.checkpoint()?;
            surface_receipt.record_text(
                crate::cynefin::CynefinTextRole::TransitionLabel,
                "cynefinArrowLabel",
                label,
            );
        }
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn write_cynefin_css(
    out: &mut impl SvgOutput,
    diagram_id: &str,
    effective_config: &serde_json::Value,
    theme: &crate::cynefin::CynefinTheme,
    typography_theme: &crate::cynefin::CynefinTypographyThemePlan,
    surface_receipt: &mut crate::cynefin::CynefinSurfaceReceipt,
) -> Result<()> {
    let id = crate::svg::escape_css_identifier(diagram_id);
    let parts = info_css_parts_with_font_family(
        diagram_id,
        effective_config,
        typography_theme.font_family_css(),
    );
    out.push_str(&parts.css_prefix);
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
        theme.text_color,
        theme.boundary_color,
        fmt(theme.item_font_size),
        theme.text_color,
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
        theme.text_color,
        fmt(theme.domain_font_size + 2.0),
        theme.label_color
    );
    out.push_str(&parts.root_rule);
    out.checkpoint()?;
    surface_receipt.record_css_emission(&parts.font_family, &parts.font_family, &parts.font_family);
    Ok(())
}
