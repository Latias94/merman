use super::super::*;
use crate::ishikawa::{IshikawaCssBinding, IshikawaTextThemePlan};
use merman_render_structured::ishikawa::{
    IshikawaRoughPaint, write_classic_svg_body, write_hand_drawn_svg_body,
};

pub(crate) fn render_ishikawa_diagram_svg_with_theme(
    layout: &IshikawaDiagramLayout,
    text_theme: &IshikawaTextThemePlan,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id_or("ishikawa");
    let mut out = BoundedSvgOutput::new(options.work_meter());
    let theme = text_theme.css_binding();
    let root_bounds = root_svg::DiagramBounds::from_view_box(
        layout.viewbox_x,
        layout.viewbox_y,
        layout.total_width,
        layout.total_height,
    );
    let root_spec = root_svg::RootViewportSpec::mermaid(root_bounds, layout.use_max_width);
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "ishikawa");
    root_chrome.dom.trailing_newline = false;
    let root_document = options
        .root_viewport(crate::DiagramFamilyId::ISHIKAWA, diagram_id)
        .write_open(&mut out, root_spec, root_chrome)?;
    options.checkpoint_emit()?;

    out.push_str("<style>");
    write_ishikawa_css(
        &mut super::super::output::XmlTextOutput::new(&mut out),
        super::super::util::css_selector_diagram_id(diagram_id),
        theme,
        text_theme,
    );
    out.push_str(r#"</style><g/><g class="ishikawa">"#);
    out.checkpoint()?;
    if theme.look == "handDrawn" {
        let randomness = options.rough_randomness(
            theme.hand_drawn_seed.unwrap_or(options.seed() as f64),
            "render.ishikawa.roughjs",
        );
        write_hand_drawn_svg_body(
            &mut out,
            layout,
            randomness,
            IshikawaRoughPaint {
                line_color: &theme.line_color,
                sub_branch_stroke: theme.sub_branch_stroke(),
                head_fill: theme.head_fill(),
                label_fill: &theme.main_bkg,
                node_stroke: &theme.node_stroke,
                marker_fill: &theme.marker_fill,
                solid_head: theme.native.is_some(),
            },
            options.work_meter(),
        )?;
    } else {
        let marker_id = format!("ishikawa-arrow-{diagram_id}");
        write_classic_svg_body(&mut out, layout, &marker_id, options.work_meter())?;
    }

    out.push_str("</g></svg>\n");
    out.checkpoint()?;
    root_document.complete(out.finish()?)
}

fn write_ishikawa_css(
    css: &mut impl SvgOutput,
    diagram_id: impl Copy + std::fmt::Display,
    theme: &IshikawaCssBinding,
    text_theme: &IshikawaTextThemePlan,
) {
    let _ = write!(
        css,
        "#{diagram_id} .ishikawa .ishikawa-spine,#{diagram_id} .ishikawa .ishikawa-branch,#{diagram_id} .ishikawa .ishikawa-sub-branch {{ stroke: {line_color}; stroke-width: 2; fill: none; }}\
#{diagram_id} .ishikawa .ishikawa-sub-branch {{ stroke-width: 1; }}\
#{diagram_id} .ishikawa .ishikawa-arrow {{ fill: {marker_fill}; }}\
#{diagram_id} .ishikawa .ishikawa-head {{ fill: {head_fill}; stroke: {node_stroke}; stroke-width: 2; }}\
#{diagram_id} .ishikawa .ishikawa-label-box {{ fill: {main_bkg}; stroke: {node_stroke}; stroke-width: 2; }}",
        line_color = theme.line_color,
        head_fill = theme.head_fill(),
        main_bkg = theme.main_bkg,
        node_stroke = theme.node_stroke,
        marker_fill = theme.marker_fill,
    );
    if theme.native.is_some() {
        let _ = write!(
            css,
            "#{diagram_id} .ishikawa .ishikawa-sub-branch {{ stroke: {}; }}",
            theme.sub_branch_stroke(),
        );
    }
    text_theme.write_text_rules(css, diagram_id);
    let _ = write!(
        css,
        "#{diagram_id} .ishikawa .ishikawa-label {{ text-anchor: end; }}\
#{diagram_id} .ishikawa .ishikawa-label.cause {{ text-anchor: middle; dominant-baseline: middle; }}\
#{diagram_id} .ishikawa .ishikawa-label.align {{ text-anchor: end; dominant-baseline: middle; }}\
#{diagram_id} .ishikawa .ishikawa-label.up {{ dominant-baseline: baseline; }}\
#{diagram_id} .ishikawa .ishikawa-label.down {{ dominant-baseline: hanging; }}"
    );
}
