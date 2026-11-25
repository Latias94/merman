use super::super::*;
use merman_model::families::quadrant_chart::QuadrantChartDiagramModel;

pub(crate) fn render_quadrantchart_diagram_svg(
    layout: &QuadrantChartDiagramLayout,
    model: &QuadrantChartDiagramModel,
    point_theme: &crate::quadrantchart::QuadrantChartPointThemePlan,
    effective_config: &serde_json::Value,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    if point_theme.point_count() != layout.points.len() || model.points.len() != layout.points.len()
    {
        return Err(crate::Error::InvalidModel {
            message: "Quadrant Chart point theme plan does not match the terminal layout"
                .to_string(),
        });
    }
    let diagram_id = options.diagram_id_or("quadrantchart");
    let acc_title = model.acc_title.as_deref().filter(|title| !title.is_empty());
    let acc_descr = model
        .acc_descr
        .as_deref()
        .filter(|description| !description.is_empty());
    let aria_labelledby = acc_title.map(|_| format!("chart-title-{diagram_id}"));
    let aria_describedby = acc_descr.map(|_| format!("chart-desc-{diagram_id}"));
    let use_max_width = crate::quadrantchart::QuadrantChartConfigView::new(effective_config)
        .render_settings()
        .use_max_width;

    let mut out = BoundedSvgOutput::new(options.work_meter());
    let root_bounds = root_svg::DiagramBounds::from_extents(
        0.0,
        0.0,
        layout.width.max(1.0),
        layout.height.max(1.0),
        0.0,
    );
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "quadrantChart");
    root_chrome.aria_labelledby = aria_labelledby.as_deref();
    root_chrome.aria_describedby = aria_describedby.as_deref();
    root_chrome.dom = root_svg::RootDomProfile {
        style_viewbox_order: root_svg::SvgRootStyleViewBoxOrder::ViewBoxThenStyle,
        fixed_height_placement: root_svg::SvgRootFixedHeightPlacement::AfterXmlns,
        fixed_style_placement: root_svg::RootStylePlacement::Tail,
        trailing_newline: false,
        ..root_svg::RootDomProfile::default()
    };
    let root_document = options
        .root_viewport(crate::DiagramFamilyId::QUADRANT_CHART, diagram_id)
        .write_open(
            &mut out,
            root_svg::RootViewportSpec::mermaid(root_bounds, use_max_width),
            root_chrome,
        )?;

    if let Some(title) = acc_title {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{diagram_id}">{}</title>"#,
            escape_xml(title)
        );
        out.checkpoint()?;
    }
    if let Some(description) = acc_descr {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{diagram_id}">{}</desc>"#,
            escape_xml(description)
        );
        out.checkpoint()?;
    }

    out.push_str("<style>");
    out.checkpoint()?;
    let common_css = point_theme.common_css();
    let selector = super::super::util::css_selector_diagram_id(diagram_id);
    {
        let mut css_out = super::super::output::XmlTextOutput::new(&mut out);
        common_css.write_prefix(&mut css_out, selector)?;
        common_css.write_root(&mut css_out, selector, diagram_id)?;
    }
    if let Some((title_family, title_weight, label_weight)) = point_theme.native_title_style() {
        let id = crate::svg::escape_css_identifier(diagram_id.semantic_str());
        let _ = write!(
            &mut super::super::output::XmlTextOutput::new(&mut out),
            "#{id} .quadrants text,#{id} .data-points text,#{id} .labels text{{font-weight:{label_weight};}}\
             #{id} .title text{{font-family:{title_family};font-weight:{title_weight};}}",
        );
    }
    out.checkpoint()?;
    // Mermaid always includes an empty `<g/>` placeholder after `<style>`.
    out.push_str(r#"</style><g/>"#);
    out.checkpoint()?;

    out.push_str(r#"<g class="main">"#);
    out.checkpoint()?;

    merman_render_charts::quadrantchart::write_svg_body(&mut out, layout, &model.points)?;

    out.push_str("</g></svg>\n");
    root_document.complete(out.finish()?)
}
