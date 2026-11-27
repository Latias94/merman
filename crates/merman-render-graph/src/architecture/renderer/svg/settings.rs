use merman_render_core::text::TextStyle;

use super::super::{
    ArchitectureCssBinding, ArchitectureNativeStyle, ArchitectureTypographyThemePlan,
};
use super::{SvgDiagramId, SvgOutput};
use merman_render_core::Result;

pub(super) const OPEN_ARROW_STROKE_WIDTH: f64 = 1.5;

#[derive(Clone)]
pub(super) struct ArchitectureRenderSettings {
    pub(super) icon_size_px: f64,
    pub(super) half_icon: f64,
    pub(super) padding_px: f64,
    pub(super) arch_font_size_px: f64,
    pub(super) edge_stroke_outset: f64,
    pub(super) use_max_width: bool,
    pub(super) text_style: TextStyle,
    pub(super) compound_text_style: TextStyle,
}

impl ArchitectureRenderSettings {
    pub(super) fn write_stylesheet(
        out: &mut impl SvgOutput,
        diagram_id: SvgDiagramId<'_>,
        binding: &ArchitectureCssBinding,
        native: Option<&ArchitectureNativeStyle>,
    ) -> Result<()> {
        binding.common.write_prefix(out, diagram_id)?;
        let _ = write!(
            out,
            r#"#{} .edge{{stroke-width:{};stroke:{};fill:none;}}#{} .arrow{{fill:{};}}#{} .node-bkg{{fill:none;stroke:{};stroke-width:{};stroke-dasharray:8;}}#{} .node-icon-text{{display:flex;align-items:center;}}#{} .node-icon-text>div{{color:#fff;margin:1px;height:fit-content;text-align:center;overflow:hidden;display:-webkit-box;-webkit-box-orient:vertical;}}"#,
            diagram_id,
            binding.edge_width,
            binding.edge_color,
            diagram_id,
            binding.arrow_color,
            diagram_id,
            binding.group_border_color,
            binding.group_border_width,
            diagram_id,
            diagram_id,
        );
        binding.common.write_root(out, diagram_id, diagram_id)?;
        if let Some(native) = native {
            let _ = write!(
                out,
                "#{diagram_id} .architecture-services text,#{diagram_id} .architecture-groups text,#{diagram_id} .architecture-edges text{{fill:{};}}\
                 #{diagram_id} .node-icon-text>div{{color:{};}}\
                 #{diagram_id} .arrow-open{{fill:none;stroke:{};stroke-width:{OPEN_ARROW_STROKE_WIDTH}px;stroke-linejoin:round;stroke-linecap:round;}}",
                native.text_ink, native.icon_ink, binding.arrow_color,
            );
        }
        out.checkpoint()
    }

    pub(super) fn from_prepared(
        typography: &ArchitectureTypographyThemePlan,
        native: Option<&ArchitectureNativeStyle>,
    ) -> Self {
        let binding = typography.css_binding();
        Self {
            icon_size_px: binding.icon_size_px,
            half_icon: binding.icon_size_px / 2.0,
            padding_px: binding.padding_px,
            arch_font_size_px: binding.arch_font_size_px,
            edge_stroke_outset: binding.edge_stroke_outset(),
            use_max_width: binding.use_max_width,
            text_style: typography.svg_text_style(),
            compound_text_style: native.map_or_else(
                || super::super::architecture_cytoscape_text_style(binding.arch_font_size_px),
                |native| native.layout_text.clone(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prepared_settings(
        id: &str,
        config: &serde_json::Value,
    ) -> (ArchitectureRenderSettings, String) {
        let config = crate::architecture::renderer::presentation_config(config.clone());
        let typography =
            crate::architecture::renderer::ArchitectureTypographyThemePlan::resolve(&config, None);
        crate::architecture::renderer::with_test_svg_id(id, |diagram_id, _| {
            let settings = ArchitectureRenderSettings::from_prepared(&typography, None);
            let mut css = String::new();
            ArchitectureRenderSettings::write_stylesheet(
                &mut css,
                diagram_id,
                typography.css_binding(),
                None,
            )
            .unwrap();
            (settings, css)
        })
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

        let (settings, css) = prepared_settings("arch", &cfg);

        assert_eq!(
            settings.text_style.font_family.as_deref(),
            Some(r#""IBM Plex Sans",Arial,sans-serif"#)
        );
        assert_eq!(
            settings.compound_text_style.font_family.as_deref(),
            Some("Helvetica Neue,Helvetica,sans-serif")
        );
        assert!(css.contains(
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
        let (prepared, css) = prepared_settings("architecture", &cfg);
        assert!(css.contains(".edge{stroke-width:2em;stroke:var(--edge);fill:none;}"));
        assert!(css.contains(".arrow{fill:none;}"));
        assert!(css.contains(".node-bkg{fill:none;stroke:;stroke-width:0;stroke-dasharray:8;}"));
        assert!(css.contains(".edge-thickness-normal{stroke-width:2px;}"));
        assert!(css.contains("stroke:url(#architecture-gradient)"));
        assert!(css.contains("filter:url(#custom)"));
        assert_eq!(prepared.icon_size_px, 1.0);
        assert_eq!(prepared.padding_px, 0.0);
        assert_eq!(prepared.arch_font_size_px, 18.0);
        assert!(!prepared.use_max_width);
        assert_eq!(
            prepared.text_style.font_family.as_deref(),
            Some(r#""IBM Plex Sans",Arial,sans-serif"#)
        );
        assert_eq!(prepared.text_style.font_size, 19.0);
        assert_eq!(prepared.edge_stroke_outset, 19.0);
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

        let (_, css) = prepared_settings("diag", &cfg);

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

        let (_, css) = prepared_settings("diag", &cfg);

        assert!(css.contains(
            r#"#diag{font-family:"IBM Plex Sans",Arial,sans-serif;font-size:16px;fill:#333;}"#
        ));
        assert_eq!(
            merman_render_core::config::config_root_font_family_css(&cfg),
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
        let (_, css) = prepared_settings("diag", &serde_json::json!({}));
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
