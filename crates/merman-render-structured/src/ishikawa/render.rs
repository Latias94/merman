use merman_model::{OperationPhase, runtime::OperationContext};
use merman_render_core::{
    Result,
    resources::OperationWorkMeter,
    root_svg::{DiagramBounds, RootChrome, RootViewportContext, RootViewportSpec, RootedSvg},
    svg_css::{CssSelectorDiagramId, SvgId},
    svg_output::{BoundedSvgOutput, SvgOutput, XmlTextOutput},
};
use roughr::core::{RoughJsSeed, RoughMathRandom, RoughRandomness};

use super::{
    IshikawaDiagramLayout, IshikawaRoughPaint, IshikawaTextThemePlan, theme::IshikawaCssBinding,
    write_classic_svg_body, write_hand_drawn_svg_body,
};

/// Emits the complete Ishikawa SVG before shared canvas and terminal admission.
pub fn render_ishikawa_diagram_svg(
    layout: &IshikawaDiagramLayout,
    text_theme: &IshikawaTextThemePlan,
    diagram_id: SvgId<'_>,
    root_viewport: &RootViewportContext<'_>,
    work_meter: &OperationWorkMeter,
    operation: &OperationContext,
) -> Result<RootedSvg> {
    let mut out = BoundedSvgOutput::new(work_meter);
    let theme = text_theme.css_binding();
    let root_bounds = DiagramBounds::from_view_box(
        layout.viewbox_x,
        layout.viewbox_y,
        layout.total_width,
        layout.total_height,
    );
    let root_spec = RootViewportSpec::mermaid(root_bounds, layout.use_max_width);
    let mut root_chrome = RootChrome::new(diagram_id, "ishikawa");
    root_chrome.dom.trailing_newline = false;
    let root_document = root_viewport.write_open(&mut out, root_spec, root_chrome)?;
    work_meter.checkpoint(OperationPhase::Emit)?;

    out.push_str("<style>");
    write_ishikawa_css(
        &mut XmlTextOutput::new(&mut out),
        CssSelectorDiagramId(diagram_id),
        theme,
        text_theme,
    );
    out.push_str(r#"</style><g/><g class="ishikawa">"#);
    out.checkpoint()?;
    if theme.look == "handDrawn" {
        let randomness = rough_randomness(theme.hand_drawn_seed, operation);
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
            work_meter,
        )?;
    } else {
        let marker_id = format!("ishikawa-arrow-{diagram_id}");
        write_classic_svg_body(&mut out, layout, &marker_id, work_meter)?;
    }

    out.push_str("</g></svg>\n");
    out.checkpoint()?;
    root_document.complete(out.finish()?)
}

fn rough_randomness(configured_seed: Option<f64>, operation: &OperationContext) -> RoughRandomness {
    let seed = configured_seed
        .filter(|seed| *seed != 0.0)
        .unwrap_or_else(|| operation.derive_nonzero_u64("render.root", 0).get() as f64);
    RoughRandomness::new(
        RoughJsSeed::new(seed),
        RoughMathRandom::new(operation.derive_u64("render.ishikawa.roughjs", 0)),
    )
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

#[cfg(test)]
mod tests {
    use merman_model::runtime::RuntimePolicy;

    use super::*;

    #[test]
    fn rough_seeds_preserve_the_operation_fallback_and_javascript_number() {
        let operation = RuntimePolicy::deterministic().begin_operation().unwrap();
        let fallback = operation.derive_nonzero_u64("render.root", 0).get() as f64;
        let stream = operation.derive_u64("render.ishikawa.roughjs", 0);
        for (configured, expected) in [
            (None, fallback),
            (Some(0.0), fallback),
            (Some(-0.0), fallback),
            (Some(-1.0), -1.0),
            (Some(1.75), 1.75),
            (Some(4_294_967_297.0), 4_294_967_297.0),
        ] {
            let randomness = rough_randomness(configured, &operation);
            assert_eq!(randomness.seed().number(), expected);
            assert_eq!(randomness.math_random().initial_seed(), stream);
        }
    }
}
