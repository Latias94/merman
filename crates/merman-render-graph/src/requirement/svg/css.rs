use crate::requirement::RequirementCssBinding;
use merman_render_core::{
    Result,
    svg_css::{CssSelectorDiagramId, FragmentDiagramId, SvgDiagramIdValue},
    svg_output::SvgOutput,
};

pub(super) fn write_requirement_css<I>(
    out: &mut impl SvgOutput,
    diagram_id: I,
    binding: &RequirementCssBinding,
    canonical: bool,
) -> Result<()>
where
    I: SvgDiagramIdValue,
{
    let id = CssSelectorDiagramId(diagram_id);
    let font = binding.common.font_family();
    let font_size = binding.common.font_size_css();
    let node_text_color = &binding.node_text_color;
    let relation_color = &binding.relation_color;
    let line_color = &binding.line_color;
    let requirement_background = &binding.requirement_background;
    let requirement_border_color = &binding.requirement_border_color;
    let requirement_border_size = &binding.requirement_border_size;
    let requirement_text_color = &binding.requirement_text_color;
    let relation_label_background = &binding.relation_label_background;
    let relation_label_color = &binding.relation_label_color;
    let edge_label_background = &binding.edge_label_background;
    let requirement_edge_label_background = &binding.requirement_edge_label_background;
    let node_border = &binding.node_border;
    let relationship_line_stroke_width = &binding.relationship_line_stroke_width;
    let _ = write!(
        out,
        r#"#{} marker{{fill:{};stroke:{};}}#{} marker.cross{{stroke:{};}}"#,
        id, relation_color, relation_color, id, line_color
    );
    let _ = write!(
        out,
        r#"#{id} svg{{font-family:{font};font-size:{font_size}}}#{id} .reqBox{{fill:{requirement_background};fill-opacity:1.0;stroke:{requirement_border_color};stroke-width:{requirement_border_size};}}#{id} .reqTitle,#{id} .reqLabel{{fill:{requirement_text_color};}}#{id} .reqLabelBox{{fill:{relation_label_background};fill-opacity:1.0;}}#{id} .req-title-line{{stroke:{requirement_border_color};stroke-width:{requirement_border_size};}}#{id} .relationshipLine{{stroke:{relation_color};stroke-width:{relationship_line_stroke_width};}}#{id} .relationshipLabel{{"#,
    );
    let _ = write!(out, "fill:{relation_label_color};");
    let _ = write!(
        out,
        r#"}}#{id} .edgeLabel{{background-color:{edge_label_background};}}#{id} .edgeLabel .label rect{{fill:{edge_label_background};}}#{id} .edgeLabel .label text{{"#,
    );
    let _ = write!(out, "fill:{relation_label_color};");
    let _ = write!(
        out,
        r#"}}#{id} .divider{{stroke:{node_border};stroke-width:1;}}#{id} .label{{font-family:{font};"#,
    );
    let _ = write!(out, "color:{node_text_color};");
    let _ = write!(out, "}}#{id} .label text,#{id} span{{");
    let _ = write!(out, "fill:{node_text_color};color:{node_text_color};");
    let _ = write!(
        out,
        "}}#{id} .labelBkg{{background-color:{requirement_edge_label_background};}}"
    );
    if canonical || binding.native.is_some() {
        let _ = write!(
            out,
            "#{id} .edgeLabel span{{color:{relation_label_color};}}"
        );
    }
    out.checkpoint()?;
    binding
        .common
        .write_root(out, id, FragmentDiagramId(diagram_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn requirement_css_honors_mermaid_11_15_theme_options() {
        let cfg = serde_json::json!({
            "look": "neo",
            "themeVariables": {
                "fontFamily": "\"ibm plex sans\", arial, sans-serif",
                "fontSize": "18px",
                "textColor": "#101010",
                "nodeTextColor": "#111111",
                "relationColor": "#222222",
                "lineColor": "#333333",
                "requirementBackground": "#444444",
                "requirementBorderColor": "#555555",
                "requirementBorderSize": 2,
                "requirementTextColor": "#666666",
                "relationLabelBackground": "#777777",
                "relationLabelColor": "#888888",
                "edgeLabelBackground": "#999999",
                "requirementEdgeLabelBackground": "#aaaaaa",
                "nodeBorder": "#bbbbbb",
                "strokeWidth": 3
            }
        });

        let config = crate::requirement::test_config(cfg);
        let meter = merman_render_core::resources::OperationWorkMeter::new(
            merman_render_core::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let plan = crate::requirement::RequirementPaintThemePlan::resolve(
            None,
            &config,
            &merman_model::families::requirement::RequirementDiagramModel {
                acc_title: None,
                acc_descr: None,
                direction: String::new(),
                requirements: Vec::new(),
                elements: Vec::new(),
                relationships: Vec::new(),
                classes: std::collections::BTreeMap::new(),
            },
            &meter,
        )
        .unwrap();
        let mut css = String::new();
        write_requirement_css(&mut css, "req", plan.css(), false).unwrap();

        assert!(css.contains(r#"#req marker{fill:#222222;stroke:#222222;}"#));
        assert!(css.contains(r#"#req marker.cross{stroke:#333333;}"#));
        assert!(css.contains(
            r#"#req .reqBox{fill:#444444;fill-opacity:1.0;stroke:#555555;stroke-width:2;}"#
        ));
        assert!(css.contains(r#"#req .reqTitle,#req .reqLabel{fill:#666666;}"#));
        assert!(css.contains(r#"#req .reqLabelBox{fill:#777777;fill-opacity:1.0;}"#));
        assert!(css.contains(r#"#req .relationshipLine{stroke:#222222;stroke-width:3;}"#));
        assert!(css.contains(r#"#req .relationshipLabel{fill:#888888;}"#));
        assert!(css.contains(r#"#req .edgeLabel .label rect{fill:#999999;}"#));
        assert!(css.contains(r#"#req .labelBkg{background-color:#aaaaaa;}"#));
        assert!(
            css.contains(r#"#req [data-look="neo"].node path{stroke:#bbbbbb;stroke-width:3px;}"#)
        );
    }
}
