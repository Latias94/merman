use merman_render_core::{
    Result,
    svg_css::{CssSelectorDiagramId, PreparedCommonCss, SvgDiagramIdValue},
    svg_output::SvgOutput,
};

use super::SankeyStyle;

pub(super) fn write_sankey_css<I>(
    out: &mut impl SvgOutput,
    diagram_id: I,
    style: &SankeyStyle,
) -> Result<()>
where
    I: SvgDiagramIdValue,
{
    write_sankey_css_values(
        out,
        diagram_id,
        &style.common_css,
        &style.label_background,
        style.palette.link_blend_mode_css(),
        style.palette.link_stroke_opacity(),
    )
}

fn write_sankey_css_values<I>(
    out: &mut impl SvgOutput,
    diagram_id: I,
    values: &PreparedCommonCss,
    label_background: &str,
    link_blend_mode_css: &str,
    link_stroke_opacity: &str,
) -> Result<()>
where
    I: SvgDiagramIdValue,
{
    let id = CssSelectorDiagramId(diagram_id);
    values.write_prefix(out, diagram_id)?;
    let label_font_family_css = values.font_family();
    let _ = write!(
        out,
        r#"#{} .label{{font-family:{};}}#{} .node-labels{{font-family:{};}}#{} .sankey-label-bg{{stroke:{};stroke-width:4px;stroke-linejoin:round;paint-order:stroke;}}#{} .sankey-label-fg{{fill:{};}}#{} .node rect{{shape-rendering:crispEdges;}}#{} .link{{fill:none;stroke-opacity:{link_stroke_opacity};mix-blend-mode:{};}}"#,
        id,
        label_font_family_css,
        id,
        label_font_family_css,
        id,
        label_background,
        id,
        values.text_color(),
        id,
        id,
        link_blend_mode_css
    );
    out.checkpoint()?;
    values.write_root(out, diagram_id, diagram_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_render_core::config::config_string;

    #[test]
    fn sankey_css_honors_mermaid_11_15_theme_options() {
        let cfg = serde_json::json!({
            "fontFamily": "\"source sans\", arial, sans-serif",
            "themeVariables": {
                "fontFamily": "\"ibm plex sans\", arial, sans-serif",
                "textColor": "#123456",
                "mainBkg": "#abcdef",
            }
        });

        let values = PreparedCommonCss::new(&cfg, None);
        let label_background = config_string(&cfg, &["themeVariables", "mainBkg"]).unwrap();
        let mut css = String::new();
        write_sankey_css_values(
            &mut css,
            "sk",
            &values,
            &label_background,
            "multiply",
            "0.5",
        )
        .unwrap();

        assert!(css.contains(r#"#sk .label{font-family:"ibm plex sans",arial,sans-serif;}"#));
        assert!(css.contains(r#"#sk .node-labels{font-family:"ibm plex sans",arial,sans-serif;}"#));
        assert!(css.contains(r#"#sk .sankey-label-bg{stroke:#abcdef;"#));
        assert!(css.contains(r#"#sk .sankey-label-fg{fill:#123456;}"#));
        assert!(
            css.contains(r#"#sk :root{--mermaid-font-family:"source sans",arial,sans-serif;}"#)
        );
    }
}
