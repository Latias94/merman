use super::super::*;
use crate::treemap::TreemapStyle;

pub(super) fn treemap_css(
    diagram_id: impl SvgDiagramIdValue,
    style: &TreemapStyle,
) -> Result<String> {
    // Mermaid's treemap styles merge `treemap.*` options with theme title/text colors. Keep
    // `:root` last to match upstream SVG baselines.
    let id = super::super::util::css_selector_diagram_id(diagram_id);
    let theme = style.css_binding();
    let label_color = &theme.label_color;
    let value_color = &theme.value_color;
    let mut out = String::new();
    theme.common.write_prefix(&mut out, diagram_id)?;

    let _ = write!(
        &mut out,
        r#"#{} .treemapNode.section{{stroke:{};stroke-width:{};fill:{};}}#{} .treemapNode.leaf{{stroke:{};stroke-width:{};fill:{};}}#{} .treemapLabel{{fill:{};font-size:{};}}#{} .treemapValue{{fill:{};font-size:{};}}"#,
        id,
        theme.section_stroke_color,
        theme.section_stroke_width,
        theme.section_fill_color,
        id,
        theme.leaf_stroke_color,
        theme.leaf_stroke_width,
        theme.leaf_fill_color,
        id,
        label_color,
        theme.label_font_size,
        id,
        value_color,
        theme.value_font_size
    );
    let title_class = crate::treemap::TREEMAP_TITLE_CLASS;
    let title_fill = theme.title_color.as_str();
    let _ = write!(
        &mut out,
        r#"#{} .{}{{fill:{};font-size:{};}}"#,
        id, title_class, title_fill, theme.title_font_size
    );
    theme.common.write_root(&mut out, diagram_id, diagram_id)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn treemap_css_honors_mermaid_11_15_style_options() {
        let cfg = serde_json::json!({
            "themeVariables": {
                "fontFamily": "\"ibm plex sans\", arial, sans-serif",
                "textColor": "#123456",
                "titleColor": "#654321"
            },
            "treemap": {
                "sectionStrokeColor": "#111111",
                "sectionStrokeWidth": 2,
                "sectionFillColor": "#222222",
                "leafStrokeColor": "#333333",
                "leafStrokeWidth": "3",
                "leafFillColor": "#444444",
                "labelColor": "#555555",
                "valueColor": "#666666",
                "titleColor": "#777777",
                "labelFontSize": "13px",
                "valueFontSize": "11px",
                "titleFontSize": "15px"
            }
        });

        let config = crate::test_support::presentation_config(cfg);
        let work = std::sync::Arc::new(crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let layout = crate::treemap::layout_treemap_diagram_typed_with_work_meter(
            &merman_model::families::treemap::TreemapDiagramRenderModel::default(),
            None,
            config.as_value(),
            &work,
        )
        .unwrap();
        let style = TreemapStyle::resolve(&config, &layout, None, None, &work).unwrap();
        let css = treemap_css("tm", &style).unwrap();

        assert!(
            css.contains("#tm .treemapNode.section{stroke:#111111;stroke-width:2;fill:#222222;}")
        );
        assert!(css.contains("#tm .treemapNode.leaf{stroke:#333333;stroke-width:3;fill:#444444;}"));
        assert!(css.contains("#tm .treemapLabel{fill:#555555;font-size:13px;}"));
        assert!(css.contains("#tm .treemapValue{fill:#666666;font-size:11px;}"));
        assert!(css.contains("#tm .treemapTitle{fill:#777777;font-size:15px;}"));
    }
}
