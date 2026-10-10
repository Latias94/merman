use crate::text::TextStyle;

use super::super::SvgDiagramId;

#[derive(Clone)]
pub(super) struct ArchitectureRenderSettings {
    pub(super) css: String,
    pub(super) font_family_css: String,
    pub(super) typography_css_emission: crate::architecture::ArchitectureTypographyCssEmission,
    pub(super) icon_size_px: f64,
    pub(super) half_icon: f64,
    pub(super) padding_px: f64,
    pub(super) arch_font_size_px: f64,
    pub(super) use_max_width: bool,
    pub(super) text_style: TextStyle,
    pub(super) compound_text_style: TextStyle,
}

impl ArchitectureRenderSettings {
    pub(super) fn from_prepared(
        diagram_id: SvgDiagramId<'_>,
        typography: &crate::architecture::ArchitectureTypographyThemePlan,
    ) -> Self {
        let binding = typography.css_binding();
        let mut css = String::new();
        let base = binding
            .common
            .write_prefix_with_font_emission(&mut css, diagram_id)
            .expect("String-backed Architecture base CSS emission cannot fail");
        let base_fonts = (
            base.diagram_root_font_family_css(),
            base.nested_svg_font_family_css(),
            base.diagram_root_font_size_css(),
            base.nested_svg_font_size_css(),
        );
        use std::fmt::Write;
        write!(css,
            r#"#{} .edge{{stroke-width:{};stroke:{};fill:none;}}#{} .arrow{{fill:{};}}#{} .node-bkg{{fill:none;stroke:{};stroke-width:{};stroke-dasharray:8;}}#{} .node-icon-text{{display:flex;align-items:center;}}#{} .node-icon-text>div{{color:#fff;margin:1px;height:fit-content;text-align:center;overflow:hidden;display:-webkit-box;-webkit-box-orient:vertical;}}"#,
            diagram_id, binding.edge_width, binding.edge_color,
            diagram_id, binding.arrow_color,
            diagram_id, binding.group_border_color, binding.group_border_width,
            diagram_id, diagram_id,
        ).expect("String-backed Architecture visual CSS emission cannot fail");
        let root = binding
            .common
            .write_root_with_font_emission(&mut css, diagram_id, diagram_id)
            .expect("String-backed Architecture root CSS emission cannot fail");
        let typography_css_emission = crate::architecture::ArchitectureTypographyCssEmission::new(
            base_fonts.0,
            base_fonts.1,
            base_fonts.2,
            base_fonts.3,
            root.font_family_css(),
        );
        Self {
            css,
            font_family_css: typography.font_family_css().to_owned(),
            typography_css_emission,
            icon_size_px: binding.icon_size_px,
            half_icon: binding.icon_size_px / 2.0,
            padding_px: binding.padding_px,
            arch_font_size_px: binding.arch_font_size_px,
            use_max_width: binding.use_max_width,
            text_style: TextStyle {
                font_family: Some(typography.font_family_css().to_owned()),
                font_size: typography.font_size_px(),
                font_weight: None,
                font_style: None,
            },
            compound_text_style: crate::architecture::architecture_cytoscape_text_style(
                binding.arch_font_size_px,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prepared_settings(id: &str, config: &serde_json::Value) -> ArchitectureRenderSettings {
        let config = merman_core::MermaidConfig::from_value(config.clone());
        let typography = crate::architecture::ArchitectureTypographyThemePlan::resolve(
            None,
            &config,
            crate::architecture::ArchitectureTypographyTerminalInventory::new(0, 0).unwrap(),
        );
        let request = crate::svg::SvgRenderOptions {
            diagram_id: Some(id.to_owned()),
            ..Default::default()
        };
        super::super::super::with_test_svg_execution(
            crate::DiagramFamilyId::ARCHITECTURE,
            &request,
            |options| {
                ArchitectureRenderSettings::from_prepared(
                    options.diagram_id_or("diag"),
                    &typography,
                )
            },
        )
    }

    #[test]
    fn architecture_render_settings_keep_cytoscape_measurement_owner() {
        let cfg = serde_json::json!({
            "fontFamily": "Courier, monospace",
            "themeVariables": {
                "fontFamily": "\"IBM Plex Sans\", Arial, sans-serif"
            },
            "architecture": {
                "fontSize": 18
            }
        });

        let settings = prepared_settings("arch", &cfg);

        assert_eq!(
            settings.text_style.font_family.as_deref(),
            Some(r#""IBM Plex Sans",Arial,sans-serif"#)
        );
        assert_eq!(
            settings.compound_text_style.font_family.as_deref(),
            Some("Helvetica Neue,Helvetica,sans-serif")
        );
        assert!(settings.css.contains(
            r#"#arch{font-family:"IBM Plex Sans",Arial,sans-serif;font-size:16px;fill:#333;}"#
        ));
    }

    #[test]
    fn prepared_css_matches_raw_visual_width_and_neo_contracts() {
        let cfg = serde_json::json!({
            "fontFamily": "Courier, monospace",
            "themeVariables": {
                "fontFamily": "\"IBM Plex Sans\", Arial, sans-serif",
                "fontSize": 19,
                "textColor": "var(--text)", "lineColor": "currentColor",
                "strokeWidth": " 2 ", "nodeBorder": "var(--node)",
                "useGradient": true, "dropShadow": "url(#custom)",
                "archEdgeColor": "var(--edge)", "archEdgeArrowColor": "none",
                "archEdgeWidth": "2em", "archGroupBorderWidth": 0,
                "archGroupBorderColor": ""
            },
            "architecture": {"fontSize": 18, "iconSize": 0, "padding": -1, "useMaxWidth": false}
        });
        let prepared = prepared_settings("architecture", &cfg);
        assert!(
            prepared
                .css
                .contains(".edge{stroke-width:2em;stroke:var(--edge);fill:none;}")
        );
        assert!(prepared.css.contains(".arrow{fill:none;}"));
        assert!(
            prepared
                .css
                .contains(".node-bkg{fill:none;stroke:;stroke-width:0;stroke-dasharray:8;}")
        );
        assert!(
            prepared
                .css
                .contains(".edge-thickness-normal{stroke-width:2px;}")
        );
        assert!(prepared.css.contains("stroke:url(#architecture-gradient)"));
        assert!(prepared.css.contains("filter:url(#custom)"));
        assert_eq!(prepared.icon_size_px, 1.0);
        assert_eq!(prepared.padding_px, 0.0);
        assert_eq!(prepared.arch_font_size_px, 18.0);
        assert!(!prepared.use_max_width);
        assert_eq!(
            prepared.text_style.font_family.as_deref(),
            Some(r#""IBM Plex Sans",Arial,sans-serif"#)
        );
        assert_eq!(prepared.text_style.font_size, 19.0);
        assert_eq!(
            prepared.compound_text_style.font_family.as_deref(),
            Some("Helvetica Neue,Helvetica,sans-serif")
        );
    }
    #[test]
    fn architecture_prepared_css_honors_font_and_theme_colors() {
        let cfg = serde_json::json!({
            "fontFamily": "\"courier new\", courier, monospace;",
            "fontSize": 18,
            "themeVariables": {
                "textColor": "#112233",
                "lineColor": "#445566",
                "primaryBorderColor": "#778899",
                "archEdgeColor": "#010203",
                "archEdgeArrowColor": "#040506",
                "archEdgeWidth": 7,
                "archGroupBorderColor": "#070809",
                "archGroupBorderWidth": "6px",
            }
        });

        let css = prepared_settings("diag", &cfg).css;

        assert!(css.contains(
            r#"#diag{font-family:"courier new",courier,monospace;font-size:18px;fill:#112233;}"#
        ));
        assert!(css.contains(r#"#diag .edge{stroke-width:7;stroke:#010203;fill:none;}"#));
        assert!(css.contains(r#"#diag .arrow{fill:#040506;}"#));
        assert!(css.contains(
            r#"#diag .node-bkg{fill:none;stroke:#070809;stroke-width:6px;stroke-dasharray:8;}"#
        ));
        assert!(
            css.contains(r#"#diag :root{--mermaid-font-family:"courier new",courier,monospace;}"#)
        );
    }

    #[test]
    fn architecture_css_prefers_theme_font_family_over_legacy_root() {
        let cfg = serde_json::json!({
            "fontFamily": "Courier, monospace",
            "themeVariables": {
                "fontFamily": "\"IBM Plex Sans\", Arial, sans-serif"
            }
        });

        let css = prepared_settings("diag", &cfg).css;

        assert!(css.contains(
            r#"#diag{font-family:"IBM Plex Sans",Arial,sans-serif;font-size:16px;fill:#333;}"#
        ));
        assert_eq!(
            crate::config::config_root_font_family_css(&cfg),
            "Courier,monospace"
        );
        assert!(
            css.contains(r#"#diag :root{--mermaid-font-family:"IBM Plex Sans",Arial,sans-serif;}"#)
        );
    }

    #[test]
    fn architecture_prepared_css_keeps_base_parity_order() {
        let base_fragments = [
            r#"#diag{font-family:"trebuchet ms",verdana,arial,sans-serif;font-size:16px;fill:#333;}"#,
            r#"@keyframes edge-animation-frame{from{stroke-dashoffset:0;}}@keyframes dash{to{stroke-dashoffset:0;}}"#,
            r#"#diag .edge-animation-slow{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 50s linear infinite;stroke-linecap:round;}#diag .edge-animation-fast{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 20s linear infinite;stroke-linecap:round;}"#,
            r#"#diag .error-icon{fill:#552222;}#diag .error-text{fill:#552222;stroke:#552222;}"#,
            r#"#diag .edge-thickness-normal{stroke-width:1px;}#diag .edge-thickness-thick{stroke-width:3.5px;}#diag .edge-pattern-solid{stroke-dasharray:0;}#diag .edge-thickness-invisible{stroke-width:0;fill:none;}#diag .edge-pattern-dashed{stroke-dasharray:3;}#diag .edge-pattern-dotted{stroke-dasharray:2;}"#,
            r#"#diag .marker{fill:#333333;stroke:#333333;}#diag .marker.cross{stroke:#333333;}"#,
            r#"#diag svg{font-family:"trebuchet ms",verdana,arial,sans-serif;font-size:16px;}#diag p{margin:0;}"#,
        ];
        let css = prepared_settings("diag", &serde_json::json!({})).css;
        let fragments = [
            &base_fragments[..],
            &[r#"#diag .edge{stroke-width:3;stroke:#333333;fill:none;}"#],
        ]
        .concat();
        let mut cursor = 0;
        for fragment in fragments {
            let offset = css[cursor..]
                .find(fragment)
                .unwrap_or_else(|| panic!("missing CSS fragment: {fragment}"));
            cursor += offset + fragment.len();
        }
        assert!(css.ends_with(
            r#"#diag :root{--mermaid-font-family:"trebuchet ms",verdana,arial,sans-serif;}"#
        ));
    }
}
