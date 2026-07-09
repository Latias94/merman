use super::*;
use merman_core::diagrams::cynefin::CynefinDiagramRenderModel;

pub(crate) fn render_cynefin_diagram_svg(
    layout: &CynefinDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    let model: CynefinDiagramRenderModel = crate::json::from_value_ref(semantic)?;
    render_cynefin_diagram_svg_model(layout, &model, effective_config, options)
}

pub(crate) fn render_cynefin_diagram_svg_model(
    layout: &CynefinDiagramLayout,
    model: &CynefinDiagramRenderModel,
    effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    let diagram_id = options.diagram_id.as_deref().unwrap_or("cynefin");
    let diagram_id_esc = escape_xml(diagram_id);
    let acc_title = model
        .acc_title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let acc_descr = model
        .acc_descr
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let aria_labelledby = acc_title.map(|_| format!("chart-title-{diagram_id_esc}"));
    let aria_describedby = acc_descr.map(|_| format!("chart-desc-{diagram_id_esc}"));
    let root_bounds =
        root_svg::DiagramBounds::from_view_box(0.0, 0.0, layout.total_width, layout.total_height);
    let viewport_plan = root_svg::build_root_viewport_plan(root_bounds, None, layout.use_max_width);
    let theme = crate::cynefin::cynefin_theme(effective_config);
    let seed = crate::cynefin::resolve_seed(layout.seed, diagram_id);
    let marker_id = format!("cynefin-arrow-{diagram_id}");

    let mut out = String::new();
    root_svg::push_svg_root_open_with_viewport_plan(
        &mut out,
        root_svg::SvgRootAttrs {
            aria_labelledby: aria_labelledby.as_deref(),
            aria_describedby: aria_describedby.as_deref(),
            trailing_newline: false,
            ..root_svg::SvgRootAttrs::new(diagram_id, "cynefin")
        },
        &viewport_plan,
    );

    if let Some(title) = acc_title {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{}">{}</title>"#,
            diagram_id_esc,
            escape_xml_display(title)
        );
    }
    if let Some(descr) = acc_descr {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{}">{}</desc>"#,
            diagram_id_esc,
            escape_xml_display(descr)
        );
    }

    let _ = write!(&mut out, "<style>{}</style>", cynefin_css(&theme));
    if !layout.transitions.is_empty() {
        let _ = write!(
            &mut out,
            r#"<defs><marker id="{}" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" class="cynefinArrowHead"></path></marker></defs>"#,
            escape_attr_display(&marker_id)
        );
    }

    let _ = write!(
        &mut out,
        r#"<g transform="translate({}, {})">"#,
        fmt(layout.padding),
        fmt(layout.padding)
    );
    push_backgrounds(&mut out, layout, &theme);
    push_boundaries(&mut out, layout, seed, &theme);
    push_labels(&mut out, layout);
    if layout.show_domain_descriptions {
        push_subtitles(&mut out, layout);
    }
    push_items(&mut out, layout, &theme);
    push_transitions(&mut out, layout, &marker_id);
    if let Some(title) = model
        .title
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        let _ = write!(
            &mut out,
            r#"<text class="cynefinTitle" x="{}" y="{}" text-anchor="middle" dominant-baseline="middle">{}</text>"#,
            fmt(layout.width / 2.0),
            fmt(-layout.padding / 2.0),
            escape_xml_display(title)
        );
    }
    out.push_str("</g></svg>\n");
    Ok(out)
}

fn push_backgrounds(
    out: &mut String,
    layout: &CynefinDiagramLayout,
    theme: &crate::cynefin::CynefinTheme,
) {
    out.push_str(r#"<g class="cynefin-backgrounds">"#);
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
    }
    out.push_str("</g>");
}

fn push_boundaries(
    out: &mut String,
    layout: &CynefinDiagramLayout,
    seed: i32,
    theme: &crate::cynefin::CynefinTheme,
) {
    out.push_str(r#"<g class="cynefin-boundaries">"#);
    let fold_path = crate::cynefin::generate_fold_path(
        layout.width,
        layout.height,
        seed,
        Some(layout.boundary_amplitude),
    );
    let horizontal_path = crate::cynefin::generate_horizontal_boundary(
        layout.width,
        layout.height,
        seed.wrapping_add(100),
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
}

fn push_labels(out: &mut String, layout: &CynefinDiagramLayout) {
    out.push_str(r#"<g class="cynefin-labels">"#);
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
}

fn push_subtitles(out: &mut String, layout: &CynefinDiagramLayout) {
    out.push_str(r#"<g class="cynefin-subtitles">"#);
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
    }
    let _ = write!(
        out,
        r#"<text class="cynefinSubtitle" x="{}" y="{}" text-anchor="middle" dominant-baseline="middle">Disorder</text>"#,
        fmt(layout.width / 2.0),
        fmt(layout.height / 2.0 + 8.0)
    );
    out.push_str("</g>");
}

fn push_items(
    out: &mut String,
    layout: &CynefinDiagramLayout,
    theme: &crate::cynefin::CynefinTheme,
) {
    out.push_str(r#"<g class="cynefin-items">"#);
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
    }
    out.push_str("</g>");
}

fn push_transitions(out: &mut String, layout: &CynefinDiagramLayout, marker_id: &str) {
    if layout.transitions.is_empty() {
        return;
    }
    out.push_str(r#"<g class="cynefin-arrows">"#);
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
        if let Some(label) = transition
            .label
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            let _ = write!(
                out,
                r#"<text class="cynefinArrowLabel" x="{}" y="{}" text-anchor="middle" dominant-baseline="auto">{}</text>"#,
                fmt(transition.cpx),
                fmt(transition.cpy - 6.0),
                escape_xml_display(label)
            );
        }
    }
    out.push_str("</g>");
}

fn cynefin_css(theme: &crate::cynefin::CynefinTheme) -> String {
    format!(
        ".cynefinDomain{{stroke:none;}}\
.cynefinDomainLabel{{font-size:{}px;font-weight:bold;fill:{};}}\
.cynefinSubtitle{{font-size:{}px;fill:{};font-style:italic;}}\
.cynefinItem{{fill-opacity:0.95;stroke:{};stroke-width:1;}}\
.cynefinItemText{{font-size:{}px;fill:{};}}\
.cynefinItemOverflow{{fill-opacity:0.6;stroke:{};stroke-width:1;stroke-dasharray:3 2;}}\
.cynefinBoundary{{stroke:{};stroke-width:{};stroke-dasharray:6 3;}}\
.cynefinCliff{{stroke:{};stroke-width:{};}}\
.cynefinConfusion{{stroke:{};stroke-width:1.5;stroke-dasharray:4 2;}}\
.cynefinArrowLine{{stroke:{};stroke-width:{};fill:none;}}\
.cynefinArrowHead{{fill:{};stroke:none;}}\
.cynefinArrowLabel{{font-size:{}px;fill:{};}}\
.cynefinTitle{{font-size:{}px;font-weight:bold;fill:{};}}",
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
    )
}
