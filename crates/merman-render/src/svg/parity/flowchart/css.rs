//! Projects the facade's prepared Flowchart binding into the stylesheet inputs.

use merman_render_flow::stylesheet::FlowchartStylesheetTheme;

pub(super) use merman_render_flow::stylesheet::flowchart_classdef_cssom_string;

use super::{FlowchartEdgeAnimationResolution, SvgOutput};

pub(super) fn stylesheet_theme(
    theme: &crate::flowchart::FlowchartCompatibilityBinding,
) -> FlowchartStylesheetTheme<'_> {
    FlowchartStylesheetTheme {
        line_color: &theme.line_color,
        arrowhead_color: &theme.arrowhead_color,
        node_border: &theme.node_border,
        main_bkg: &theme.main_bkg,
        text_color: &theme.text_color,
        node_text_color: &theme.node_text_color,
        title_color: &theme.title_color,
        stroke_width: &theme.stroke_width,
        connector_stroke_width: theme.connector_stroke_width,
        error_bkg: &theme.error_bkg,
        error_text: &theme.error_text,
        edge_label_background: &theme.edge_label_background,
        tertiary: &theme.tertiary,
        cluster_bkg: &theme.cluster_bkg,
        cluster_border: &theme.cluster_border,
        border2: &theme.border2,
        drop_shadow: &theme.drop_shadow,
        root_font_family: &theme.root_font_family,
        root_font_weight: theme.root_font_weight,
        neo: &theme.neo,
        html_labels: theme.html_labels,
        hand_drawn: theme.look.is_hand_drawn(),
    }
}

#[cfg(all(
    feature = "native-compact",
    feature = "diagram-flowchart",
    feature = "layout-dagre"
))]
pub(super) fn flowchart_normal_edge_css_width(
    theme: &crate::flowchart::FlowchartCompatibilityBinding,
) -> std::borrow::Cow<'_, str> {
    merman_render_flow::stylesheet::flowchart_normal_edge_css_width(
        &theme.stroke_width,
        theme.connector_stroke_width,
    )
}

#[inline]
pub(super) fn write_flowchart_edge_class_attr(
    out: &mut impl SvgOutput,
    edge: &crate::flowchart::FlowEdge,
    animation: FlowchartEdgeAnimationResolution,
) {
    // Mermaid includes a 2-part class tuple (thickness/pattern) for flowchart edge paths. The
    // second tuple is `edge-thickness-normal edge-pattern-solid` in Mermaid@11.12.2 baselines,
    // even for dotted/thick strokes.
    let (thickness_1, pattern_1) = match edge.stroke.as_deref() {
        Some("thick") => ("edge-thickness-thick", "edge-pattern-solid"),
        Some("invisible") => ("edge-thickness-invisible", "edge-pattern-solid"),
        Some("dotted") => ("edge-thickness-normal", "edge-pattern-dotted"),
        _ => ("edge-thickness-normal", "edge-pattern-solid"),
    };

    if thickness_1 == "edge-thickness-invisible" {
        // Mermaid@11.12.2 does *not* include the second tuple nor `flowchart-link` for invisible
        // edges.
        out.push_str(thickness_1);
        out.push(' ');
        out.push_str(pattern_1);
        return;
    }

    out.push_str(thickness_1);
    out.push(' ');
    out.push_str(pattern_1);
    out.push_str(" edge-thickness-normal edge-pattern-solid flowchart-link");

    if let Some(cls) = animation.class() {
        out.push(' ');
        out.push_str(cls);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};
    use indexmap::IndexMap;
    use merman_render_core::svg_css::{SvgId, escape_css_identifier};
    use merman_render_core::svg_output::BoundedSvgOutput;
    use merman_render_flow::stylesheet::{
        ContainerPalette, FlowchartStylesheetRequest, write_flowchart_stylesheet,
    };
    use serde_json::json;

    fn write_stylesheet(
        out: &mut BoundedSvgOutput<'_>,
        meter: &OperationWorkMeter,
        diagram_id: &str,
        effective_config: &serde_json::Value,
        font_family: &str,
        class_defs: &IndexMap<String, Vec<String>>,
    ) -> crate::Result<()> {
        let binding = crate::flowchart::FlowchartCompatibilityBinding::resolve(effective_config);
        let palette = ContainerPalette::new(effective_config);
        let _ = write_flowchart_stylesheet(
            out,
            meter,
            FlowchartStylesheetRequest {
                diagram_id: SvgId::new(diagram_id).unwrap(),
                diagram_type: "flowchart-v2",
                drop_shadow_id: format!(
                    "{diagram_id}-merman-flowchart-document-filter-drop-shadow"
                ),
                drop_shadow_small_id: format!(
                    "{diagram_id}-merman-flowchart-document-filter-drop-shadow-small"
                ),
                gradient_id: format!("{diagram_id}-merman-flowchart-document-gradient-root"),
                theme: stylesheet_theme(&binding),
                native_title: None,
                palette: &palette,
                font_family,
                font_size: 16.0,
                class_defs,
                swimlane: None,
                allow_deferred_palette: false,
            },
        )?;
        Ok(())
    }

    fn flowchart_css(
        diagram_id: &str,
        effective_config: &serde_json::Value,
        font_family: &str,
        class_defs: &IndexMap<String, Vec<String>>,
    ) -> crate::Result<String> {
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let mut out = BoundedSvgOutput::new(&meter);
        write_stylesheet(
            &mut out,
            &meter,
            diagram_id,
            effective_config,
            font_family,
            class_defs,
        )?;
        out.finish()
    }

    #[test]
    fn bounded_flowchart_css_stops_before_class_catalog_after_prefix_rejection() {
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, 32)
            .expect("valid Flowchart CSS byte ceiling");
        let meter = OperationWorkMeter::new(policy);
        let mut out = BoundedSvgOutput::new(&meter);
        let class_defs =
            IndexMap::from([("late-class".to_string(), vec!["fill:#123456".to_string()])]);
        let config = json!({});

        let error = write_stylesheet(
            &mut out,
            &meter,
            "bounded-css",
            &config,
            "sans-serif",
            &class_defs,
        )
        .expect_err("the fixed Flowchart CSS prefix must exceed the tiny ceiling");

        assert!(matches!(error, crate::Error::ResourceLimitExceeded(_)));
        assert!(
            !out.as_str().contains("late-class"),
            "classDef emission must not continue after the fixed-prefix checkpoint"
        );
    }

    #[test]
    fn flowchart_class_selectors_are_css_encoded_before_terminal_xml_encoding() {
        let class = "class&<:quoted\"Ω";
        let class_defs = IndexMap::from([(class.to_owned(), vec!["fill:#123456".to_owned()])]);
        let config = json!({"htmlLabels": true});
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let mut out = BoundedSvgOutput::new(&meter);
        out.push_str("<svg>");
        write_stylesheet(
            &mut out,
            &meter,
            "selector-scope",
            &config,
            "sans-serif",
            &class_defs,
        )
        .unwrap();
        out.push_str("</svg>");
        let svg = out.finish().unwrap();
        let document = crate::xml_dom::Document::parse(&svg).unwrap();
        let css = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .unwrap()
            .text()
            .unwrap();
        let escaped = escape_css_identifier(class);
        assert!(
            css.contains(&format!("#selector-scope .{escaped}>*{{")),
            "{css}"
        );
        assert!(!css.contains("&amp;"));
        assert!(!css.contains("&gt;"));
    }

    #[test]
    fn khroma_named_edge_label_background_preserves_channels() {
        let css = flowchart_css(
            "theme_named_color",
            &json!({
                "themeVariables": {
                    "edgeLabelBackground": "rebeccapurple"
                }
            }),
            "\"trebuchet ms\",verdana,arial,sans-serif",
            &IndexMap::new(),
        )
        .expect("valid khroma color");

        assert!(
            css.contains("#theme_named_color .labelBkg{background-color:rgba(102, 51, 153, 0.5);}")
        );
    }

    #[test]
    fn unsupported_edge_label_background_returns_color_error() {
        let error = flowchart_css(
            "theme_unknown_color",
            &json!({
                "themeVariables": {
                    "edgeLabelBackground": "not-a-css-color"
                }
            }),
            "\"trebuchet ms\",verdana,arial,sans-serif",
            &IndexMap::new(),
        )
        .expect_err("unsupported khroma color must fail");

        assert!(error.to_string().contains("not-a-css-color"));
    }

    #[test]
    fn ordinary_neo_without_profile_keeps_mermaid_common_css_only() {
        let css = flowchart_css(
            "ordinary_neo",
            &json!({"look": "neo"}),
            "\"trebuchet ms\",verdana,arial,sans-serif",
            &IndexMap::new(),
        )
        .expect("valid Mermaid CSS");

        assert!(!css.contains(
            r#".flowchart-link[data-look="neo"]{stroke-linecap:round;stroke-linejoin:round;}"#
        ));
    }

    #[test]
    fn deferred_flowchart_stylesheet_preserves_bytes_and_the_svg_ceiling() {
        use merman_render_core::svg_css::SvgDiagramIdProjection;

        let config = json!({
            "theme": "redux-color", "look": "neo",
            "themeVariables": {"borderColorArray": ["#446688"], "bkgColorArray": ["#ccddee"]}
        });
        let binding = crate::flowchart::FlowchartCompatibilityBinding::resolve(&config);
        let palette = ContainerPalette::new(&config);
        let class_defs = IndexMap::new();
        let render = |defer, ceiling| -> crate::Result<String> {
            let policy = RenderResourcePolicy::unbounded_for_trusted_input();
            let policy = match ceiling {
                Some(value) => policy
                    .with_limit(ResourceLimitId::MaxSvgBytes, value)
                    .unwrap(),
                None => policy,
            };
            let meter = OperationWorkMeter::new(policy);
            let projection = SvgDiagramIdProjection::new(&meter);
            let diagram_id = SvgId::new("palette-scope")
                .unwrap()
                .with_projection(&projection);
            let mut out = BoundedSvgOutput::new(&meter);
            out.push_str("<svg>");
            let deferred = write_flowchart_stylesheet(
                &mut out,
                &meter,
                FlowchartStylesheetRequest {
                    diagram_id,
                    diagram_type: "flowchart-v2",
                    drop_shadow_id: "shadow",
                    drop_shadow_small_id: "shadow-small",
                    gradient_id: "gradient",
                    theme: stylesheet_theme(&binding),
                    native_title: None,
                    palette: &palette,
                    font_family: "sans-serif",
                    font_size: 16.0,
                    class_defs: &class_defs,
                    swimlane: None,
                    allow_deferred_palette: defer,
                },
            )?;
            assert_eq!(deferred.is_some(), defer);
            out.push_str(r#"<g data-color-id="color-0"/></svg>"#);
            out.checkpoint()?;
            if let Some(deferred) = deferred {
                deferred.insert(&mut out)?;
            }
            projection.finish()?;
            out.finish()
        };
        let eager = render(false, None).unwrap();
        assert_eq!(render(true, Some(eager.len())).unwrap(), eager);
        assert!(matches!(
            render(true, Some(eager.len() - 1)),
            Err(crate::Error::ResourceLimitExceeded(_))
        ));
    }
}
