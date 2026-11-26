use super::*;

#[cfg(feature = "diagram-er")]
use merman_render_core::svg_css::CssSelectorDiagramId;
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow",
    feature = "diagram-usecase"
))]
pub(crate) use merman_render_core::svg_css::PreparedCommonNeoCss;
pub(super) use merman_render_core::svg_css::{
    FragmentDiagramId, MermaidBaseCss, write_mermaid_base_css_prefix,
    write_mermaid_base_css_root_rule,
    write_mermaid_base_css_root_rule as write_mermaid_base_css_root_rule_to,
};

pub(super) fn write_mermaid_paragraph_css_to(
    out: &mut impl SvgOutput,
    id: impl std::fmt::Display,
) -> Result<()> {
    let _ = write!(out, "#{id} p{{margin:0;}}");
    out.checkpoint()
}

#[cfg(feature = "diagram-er")]
pub(super) fn er_css_with_resolved_typography<I>(
    diagram_id: I,
    values: &crate::er::ErCssBinding,
) -> Result<String>
where
    I: SvgDiagramIdValue,
{
    // Mirrors pinned Mermaid ER unified renderer stylesheet ordering (see `diagrams/er/styles.ts`
    // and shared base stylesheet).
    // Keep `:root` last (matches upstream fixtures).
    let id = CssSelectorDiagramId(diagram_id);
    let fragment_id = FragmentDiagramId(diagram_id);
    let font = &values.font_family;
    let text_color = &values.text_color;
    let line_color = &values.line_color;
    let main_bkg = &values.main_bkg;
    let node_border = &values.node_border;
    let cluster_bkg = &values.cluster_bkg;
    let cluster_border = &values.cluster_border;
    let title_color = &values.title_color;
    let node_text_color = &values.node_text_color;
    let tertiary_color = &values.tertiary_color;
    let label_background = values.label_background.clone()?;
    let edge_label_background = &values.edge_label_background;
    let stroke_width = &values.stroke_width;
    let mut out = String::new();
    values.common.write_prefix(&mut out, id)?;
    let _ = write!(
        &mut out,
        r#"#{} .entityBox{{fill:{};stroke:{};}}"#,
        id, main_bkg, node_border
    );
    let _ = write!(
        &mut out,
        r#"#{} .relationshipLabelBox{{fill:{};opacity:0.7;background-color:{};}}"#,
        id, tertiary_color, tertiary_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .relationshipLabelBox rect{{opacity:0.5;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .labelBkg{{background-color:{};}}"#,
        id, label_background
    );
    let _ = write!(
        &mut out,
        r#"#{} .edgeLabel{{background-color:{};}}#{} .edgeLabel .label rect{{fill:{};}}#{} .edgeLabel .label text{{fill:{};}}#{} .edgeLabel .label{{fill:{};font-size:14px;}}"#,
        id, edge_label_background, id, edge_label_background, id, text_color, id, node_border
    );
    let _ = write!(
        &mut out,
        r#"#{} .label{{font-family:{};color:{};}}"#,
        id, font, node_text_color
    );
    // Mermaid duplicates `.edge-pattern-dashed` (base rule earlier sets dasharray:3).
    let _ = write!(
        &mut out,
        r#"#{} .edge-pattern-dashed{{stroke-dasharray:8,8;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .node rect,#{} .node circle,#{} .node ellipse,#{} .node polygon{{fill:{};stroke:{};stroke-width:{};}}"#,
        id, id, id, id, main_bkg, node_border, stroke_width
    );
    let _ = write!(
        &mut out,
        r#"#{} .relationshipLine{{stroke:{};stroke-width:{};fill:none;}}"#,
        id, line_color, stroke_width
    );
    // Mermaid duplicates `.marker` (base rule earlier sets fill/stroke from the line color).
    let _ = write!(
        &mut out,
        r#"#{} .marker{{fill:none!important;stroke:{}!important;stroke-width:1;}}"#,
        id, line_color
    );
    let _ = write!(
        &mut out,
        r#"#{} [data-look=neo].labelBkg{{background-color:{};}}"#,
        id,
        values.neo_label_background.clone()?
    );
    let _ = write!(
        &mut out,
        r#"#{} .cluster rect{{fill:{};stroke:{};stroke-width:1px;}}#{} .cluster text{{fill:{};}}#{} .cluster-label text{{fill:{};}}"#,
        id, cluster_bkg, cluster_border, id, title_color, id, title_color
    );
    values.common.write_root(&mut out, id, fragment_id)?;
    Ok(out)
}

#[cfg(all(test, feature = "all-diagrams"))]
mod tests;
