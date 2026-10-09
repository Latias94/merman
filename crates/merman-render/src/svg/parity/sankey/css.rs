use super::super::*;

pub(super) fn write_sankey_css<I>(
    out: &mut impl SvgOutput,
    diagram_id: I,
    theme: &crate::sankey::SankeyTypographyThemePlan,
) -> Result<crate::sankey::SankeyTypographyCssEmission>
where
    I: SvgDiagramIdValue,
{
    write_sankey_css_values(
        out,
        diagram_id,
        theme.common_css(),
        theme.label_background(),
    )
}

fn write_sankey_css_values<I>(
    out: &mut impl SvgOutput,
    diagram_id: I,
    values: &crate::svg::PreparedCommonCss,
    label_background: &str,
) -> Result<crate::sankey::SankeyTypographyCssEmission>
where
    I: SvgDiagramIdValue,
{
    let id = util::css_selector_diagram_id(diagram_id);
    let base_font_emission = values.write_prefix_with_font_emission(out, diagram_id)?;
    let label_font_family_css = values.font_family();
    let _ = write!(
        out,
        r#"#{} .label{{font-family:{};}}#{} .node-labels{{font-family:{};}}#{} .sankey-label-bg{{stroke:{};stroke-width:4px;stroke-linejoin:round;paint-order:stroke;}}#{} .sankey-label-fg{{fill:{};}}#{} .node rect{{shape-rendering:crispEdges;}}#{} .link{{fill:none;stroke-opacity:0.5;mix-blend-mode:multiply;}}"#,
        id,
        label_font_family_css,
        id,
        label_font_family_css,
        id,
        label_background,
        id,
        values.text_color(),
        id,
        id
    );
    out.checkpoint()?;
    let root_font_emission = values.write_root_with_font_emission(out, diagram_id, diagram_id)?;
    let all_font_surfaces_match = [
        base_font_emission.diagram_root_font_family_css(),
        base_font_emission.nested_svg_font_family_css(),
        label_font_family_css,
        label_font_family_css,
        root_font_emission.font_family_css(),
    ]
    .into_iter()
    .all(|family| family == values.font_family());
    Ok(
        crate::sankey::SankeyTypographyCssEmission::from_successful_writes(
            values.font_family().to_owned(),
            all_font_surfaces_match,
            values.text_color().to_owned(),
            values.text_color().to_owned(),
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
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

        let values = crate::svg::PreparedCommonCss::new(&cfg, None);
        let label_background =
            crate::config::config_string(&cfg, &["themeVariables", "mainBkg"]).unwrap();
        let mut css = String::new();
        write_sankey_css_values(&mut css, "sk", &values, &label_background).unwrap();

        assert!(css.contains(r#"#sk .label{font-family:"ibm plex sans",arial,sans-serif;}"#));
        assert!(css.contains(r#"#sk .node-labels{font-family:"ibm plex sans",arial,sans-serif;}"#));
        assert!(css.contains(r#"#sk .sankey-label-bg{stroke:#abcdef;"#));
        assert!(css.contains(r#"#sk .sankey-label-fg{fill:#123456;}"#));
        assert!(
            css.contains(r#"#sk :root{--mermaid-font-family:"source sans",arial,sans-serif;}"#)
        );
    }
}
