use super::*;
use crate::cynefin::{CynefinStyle, CynefinTextRole, CynefinTheme};
use merman_model::families::cynefin::CynefinDiagramModel;

pub(crate) fn render_cynefin_diagram_svg_model(
    layout: &CynefinDiagramLayout,
    model: &CynefinDiagramModel,
    diagram_title: Option<&str>,
    style: &CynefinStyle,
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
    let theme = style.colors();
    let title = model
        .title
        .as_deref()
        .filter(|value| !value.is_empty())
        .or_else(|| diagram_title.filter(|value| !value.is_empty()));
    let seed = crate::cynefin::resolve_seed(layout.seed, diagram_id.semantic_str());
    let mut out = BoundedSvgOutput::new(options.work_meter());
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "cynefin");
    root_chrome.aria_labelledby = aria_labelledby.as_deref();
    root_chrome.aria_describedby = aria_describedby.as_deref();
    root_chrome.dom.trailing_newline = false;
    let root_document = options
        .root_viewport(crate::DiagramFamilyId::CYNEFIN, diagram_id)
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
        style,
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
    push_backgrounds(&mut out, layout, theme)?;
    push_boundaries(&mut out, layout, seed, diagram_id, theme)?;
    push_labels(&mut out, layout)?;
    if layout.show_domain_descriptions {
        push_subtitles(&mut out, layout)?;
    }
    push_items(&mut out, layout, theme)?;
    push_transitions(&mut out, layout, diagram_id, options)?;
    if let Some(title) = title {
        let _ = write!(
            &mut out,
            r#"<text class="cynefinTitle" x="{}" y="{}" text-anchor="middle" dominant-baseline="middle""#,
            fmt(layout.width / 2.0),
            fmt(-layout.padding / 2.0)
        );
        let _ = write!(&mut out, ">{}</text>", escape_xml_display(title));
        out.checkpoint()?;
    }
    out.push_str("</g>");
    out.checkpoint()?;

    if !layout.transitions.is_empty() {
        let _ = write!(
            &mut out,
            r#"<defs><marker id="cynefin-arrow-{diagram_id}" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" class="cynefinArrowHead"></path></marker></defs>"#,
        );
        out.checkpoint()?;
    }
    out.push_str("</svg>\n");
    root_document.complete(out.finish()?)
}

fn push_backgrounds(
    out: &mut impl SvgOutput,
    layout: &CynefinDiagramLayout,
    theme: &CynefinTheme,
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
        if let Some(native) = &theme.native {
            let _ = write!(
                out,
                r#"<rect class="cynefinDomainAccent" x="{}" y="{}" width="{}" height="{}" fill="{}"/>"#,
                fmt(domain.x),
                fmt(domain.y),
                fmt(domain.width),
                fmt(4.0_f64.min(domain.height * 0.25)),
                escape_attr_display(native.domain_accent(domain_name))
            );
            out.checkpoint()?;
        }
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn push_boundaries(
    out: &mut impl SvgOutput,
    layout: &CynefinDiagramLayout,
    seed: f64,
    diagram_id: SvgDiagramId<'_>,
    theme: &CynefinTheme,
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
    for (class, path) in [
        ("cynefinBoundary", &fold_path),
        ("cynefinBoundary", &horizontal_path),
        ("cynefinCliff", &cliff_path),
    ] {
        let _ = write!(
            out,
            r#"<path class="{class}" d="{}" fill="none"></path>"#,
            escape_attr_display(path)
        );
        out.checkpoint()?;
    }
    out.push_str("</g>");
    out.checkpoint()?;

    let cx = layout.width / 2.0;
    let cy = layout.height / 2.0;
    let rx = layout.width * 0.15;
    let ry = layout.height * 0.15;
    let confusion_path = crate::cynefin::generate_confusion_path(cx, cy, rx, ry);
    let _ = write!(
        out,
        r#"<path class="cynefinConfusion" d="{}" fill="{}" fill-opacity="0.5"></path>"#,
        escape_attr_display(&confusion_path),
        escape_attr_display(&theme.confusion_bg)
    );
    out.checkpoint()?;
    if let Some(native) = &theme.native {
        let _ = write!(
            out,
            r#"<defs><clipPath id="cynefin-confusion-clip-{diagram_id}"><path d="{}"/></clipPath></defs><rect class="cynefinDomainAccent" x="{}" y="{}" width="{}" height="{}" fill="{}" clip-path="url(#cynefin-confusion-clip-{diagram_id})"/>"#,
            escape_attr_display(&confusion_path),
            fmt(cx - rx),
            fmt(cy - ry),
            fmt(rx * 2.0),
            fmt(4.0_f64.min(ry * 0.25)),
            escape_attr_display(native.domain_accent("confusion"))
        );
    }
    out.checkpoint()
}

fn push_labels(out: &mut impl SvgOutput, layout: &CynefinDiagramLayout) -> Result<()> {
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
        let _ = write!(
            out,
            ">{}</text>",
            escape_xml_display(crate::cynefin::domain_title(domain_name))
        );
        out.checkpoint()?;
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
    out.push_str(">Confusion</text></g>");
    out.checkpoint()
}

fn push_subtitles(out: &mut impl SvgOutput, layout: &CynefinDiagramLayout) -> Result<()> {
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
            let _ = write!(out, ">{}</text>", escape_xml_display(text));
            out.checkpoint()?;
        }
    }
    let _ = write!(
        out,
        r#"<text class="cynefinSubtitle" x="{}" y="{}" text-anchor="middle" dominant-baseline="middle""#,
        fmt(layout.width / 2.0),
        fmt(layout.height / 2.0 + 8.0)
    );
    out.push_str(">Disorder</text></g>");
    out.checkpoint()
}

fn push_items(
    out: &mut impl SvgOutput,
    layout: &CynefinDiagramLayout,
    theme: &CynefinTheme,
) -> Result<()> {
    out.push_str(r#"<g class="cynefin-items">"#);
    out.checkpoint()?;
    for item in &layout.items {
        let domain_fill = crate::cynefin::domain_fill(theme, &item.domain);
        let fill = theme.native.as_deref().map_or(domain_fill, |native| {
            native.item_fill(&item.domain, domain_fill)
        });
        let rect_class = if item.overflow {
            "cynefinItemOverflow"
        } else {
            "cynefinItem"
        };
        let _ = write!(
            out,
            r#"<g transform="translate({}, {})"><rect class="{}" x="0" y="0" width="{}" height="{}" rx="4" ry="4" fill="{}" fill-opacity="{}"></rect>"#,
            fmt(item.x),
            fmt(item.y),
            rect_class,
            fmt(item.width),
            fmt(item.height),
            escape_attr_display(fill),
            if item.overflow { "0.6" } else { "0.95" }
        );
        let _ = write!(
            out,
            r#"<text class="cynefinItemText" x="{}" y="{}" text-anchor="middle" dominant-baseline="central">{}</text></g>"#,
            fmt(item.text_x),
            fmt(item.text_y),
            escape_xml_display(&item.label)
        );
        out.checkpoint()?;
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn push_transitions(
    out: &mut impl SvgOutput,
    layout: &CynefinDiagramLayout,
    diagram_id: SvgDiagramId<'_>,
    options: &SvgExecution<'_>,
) -> Result<()> {
    if layout.transitions.is_empty() {
        return Ok(());
    }
    out.push_str(r#"<g class="cynefin-arrows">"#);
    out.checkpoint()?;
    for transition in &layout.transitions {
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
            r#"<path class="cynefinArrowLine" d="{}" fill="none" marker-end="url(#cynefin-arrow-{diagram_id})"></path>"#,
            escape_attr_display(&d),
        );
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
            let _ = write!(out, ">{}</text>", escape_xml_display(label));
            out.checkpoint()?;
        }
        options.checkpoint_emit()?;
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn write_cynefin_css(
    out: &mut impl SvgOutput,
    diagram_id: SvgDiagramId<'_>,
    style: &CynefinStyle,
) -> Result<()> {
    let id = crate::svg::escape_css_identifier(diagram_id.semantic_str());
    let theme = style.colors();
    let text_color = &theme.text_color;
    let item_border_color = theme
        .native
        .as_deref()
        .map_or(theme.boundary_color.as_str(), |native| {
            &native.item_border_color
        });
    let common_css = style.common_css();
    common_css.write_prefix(out, util::css_selector_diagram_id(diagram_id))?;
    out.checkpoint()?;
    let _ = write!(
        out,
        "#{id} .cynefinDomain{{stroke:none;}}\
#{id} .cynefinDomainLabel{{font-size:{};font-weight:bold;fill:{};}}\
#{id} .cynefinSubtitle{{font-size:{};fill:{};font-style:italic;}}\
#{id} .cynefinItem{{fill-opacity:0.95;stroke:{};stroke-width:1;}}\
#{id} .cynefinItemText{{font-size:{};fill:{};}}\
#{id} .cynefinItemOverflow{{fill-opacity:0.6;stroke:{};stroke-width:1;stroke-dasharray:3 2;}}\
#{id} .cynefinBoundary{{stroke:{};stroke-width:{};stroke-dasharray:6 3;}}\
#{id} .cynefinCliff{{stroke:{};stroke-width:{};}}\
#{id} .cynefinConfusion{{stroke:{};stroke-width:1.5;stroke-dasharray:4 2;}}\
#{id} .cynefinArrowLine{{stroke:{};stroke-width:{};fill:none;}}\
#{id} .cynefinArrowHead{{fill:{};stroke:none;}}\
#{id} .cynefinArrowLabel{{font-size:{};fill:{};}}\
#{id} .cynefinTitle{{font-size:{};font-weight:bold;fill:{};}}",
        theme.font_binding.css(CynefinTextRole::DomainLabel),
        theme.label_color,
        theme.font_binding.css(CynefinTextRole::Subtitle),
        text_color,
        item_border_color,
        theme.font_binding.css(CynefinTextRole::Item),
        text_color,
        item_border_color,
        theme.boundary_color,
        fmt(theme.boundary_width),
        theme.cliff_color,
        fmt(theme.cliff_width),
        theme.boundary_color,
        theme.arrow_color,
        fmt(theme.arrow_width),
        theme.arrow_color,
        theme.font_binding.css(CynefinTextRole::TransitionLabel),
        text_color,
        theme.font_binding.css(CynefinTextRole::Title),
        theme.label_color
    );
    if let Some(native) = &theme.native {
        let _ = write!(
            out,
            "#{id} .cynefinDomainLabel,#{id} .cynefinTitle{{font-weight:{};",
            native.title_weight
        );
        if let Some(family) = &native.title_family {
            let _ = write!(out, "font-family:{family};");
        }
        let _ = write!(
            out,
            "}}#{id} .cynefinItemText,#{id} .cynefinArrowLabel{{font-weight:{};}}",
            native.label_weight
        );
    }
    common_css.write_root(out, util::css_selector_diagram_id(diagram_id), diagram_id)?;
    out.checkpoint()
}
