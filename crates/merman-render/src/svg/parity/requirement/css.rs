use super::super::*;
use crate::requirement::RequirementCssBinding;

/// Values emitted by the Requirement stylesheet writer for terminal evidence.
#[derive(Debug)]
pub(super) struct RequirementCssEmission {
    pub(super) css: String,
    font_family: Box<str>,
    font_size: Box<str>,
    typed_relation_color: Option<(usize, Box<str>)>,
}

impl RequirementCssEmission {
    pub(super) fn font_family(&self) -> &str {
        &self.font_family
    }

    pub(super) fn font_size(&self) -> &str {
        &self.font_size
    }

    pub(super) fn typed_relation_color(&self) -> Option<(usize, &str)> {
        self.typed_relation_color
            .as_ref()
            .map(|(index, color)| (*index, color.as_ref()))
    }
}

pub(super) fn write_requirement_css<I>(
    diagram_id: I,
    binding: &RequirementCssBinding,
    mut text_receipt: Option<&mut crate::requirement::RequirementTextPaintReceipt<'_>>,
) -> RequirementCssEmission
where
    I: SvgDiagramIdValue,
{
    let id = super::super::util::css_selector_diagram_id(diagram_id);
    let mut out = String::new();
    binding
        .common
        .write_prefix_with_font_emission(&mut out, id)
        .expect("String-backed Requirement base CSS emission cannot fail");
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
        &mut out,
        r#"#{} marker{{fill:{};stroke:{};}}#{} marker.cross{{stroke:{};}}"#,
        id, relation_color, relation_color, id, line_color
    );
    let _ = write!(
        &mut out,
        r#"#{id} svg{{font-family:{font};font-size:{font_size}}}#{id} .reqBox{{fill:{requirement_background};fill-opacity:1.0;stroke:{requirement_border_color};stroke-width:{requirement_border_size};}}#{id} .reqTitle,#{id} .reqLabel{{fill:{requirement_text_color};}}#{id} .reqLabelBox{{fill:{relation_label_background};fill-opacity:1.0;}}#{id} .req-title-line{{stroke:{requirement_border_color};stroke-width:{requirement_border_size};}}#{id} .relationshipLine{{stroke:{relation_color};stroke-width:{relationship_line_stroke_width};}}#{id} .relationshipLabel{{"#,
    );
    let _ = write!(&mut out, "fill:{relation_label_color};");
    let _ = write!(
        &mut out,
        r#"}}#{id} .edgeLabel{{background-color:{edge_label_background};}}#{id} .edgeLabel .label rect{{fill:{edge_label_background};}}#{id} .edgeLabel .label text{{"#,
    );
    if let Some(receipt) = text_receipt.as_deref_mut() {
        let _ = receipt.write_relation_css(&mut out, relation_label_color);
    } else {
        let _ = write!(&mut out, "fill:{relation_label_color};");
    }
    let _ = write!(
        &mut out,
        r#"}}#{id} .divider{{stroke:{node_border};stroke-width:1;}}#{id} .label{{font-family:{font};"#,
    );
    if let Some(receipt) = text_receipt.as_deref_mut() {
        let _ = receipt.write_node_css(&mut out, node_text_color, false);
    } else {
        let _ = write!(&mut out, "color:{node_text_color};");
    }
    let _ = write!(&mut out, "}}#{id} .label text,#{id} span{{");
    if let Some(receipt) = text_receipt {
        let _ = receipt.write_node_css(&mut out, node_text_color, true);
    } else {
        let _ = write!(&mut out, "fill:{node_text_color};color:{node_text_color};");
    }
    let _ = write!(
        &mut out,
        "}}#{id} .labelBkg{{background-color:{requirement_edge_label_background};}}"
    );
    binding
        .common
        .write_root_with_font_emission(&mut out, id, diagram_id)
        .expect("String-backed Requirement root CSS emission cannot fail");
    RequirementCssEmission {
        css: out,
        font_family: font.into(),
        font_size: font_size.into(),
        typed_relation_color: binding
            .typed_relation_rule
            .map(|index| (index, binding.relation_color.clone().into_boxed_str())),
    }
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

        let config = merman_core::MermaidConfig::from_value(cfg);
        let meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let plan = crate::requirement::RequirementPaintThemePlan::resolve_with_title(
            None,
            &config,
            &merman_core::diagrams::requirement::RequirementDiagramRenderModel {
                acc_title: None,
                acc_descr: None,
                direction: String::new(),
                requirements: Vec::new(),
                elements: Vec::new(),
                relationships: Vec::new(),
                classes: std::collections::BTreeMap::new(),
            },
            None,
            &meter,
        )
        .unwrap();
        let css = write_requirement_css("req", plan.css(), None).css;

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
