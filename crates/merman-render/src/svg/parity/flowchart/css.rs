//! Flowchart CSS generation.

use super::*;
use indexmap::IndexMap;
use merman_render_core::write_static_css;
use std::borrow::Cow;

pub(in crate::svg::parity::flowchart) fn flowchart_classdef_css_declarations(
    declarations: &[String],
) -> (String, String) {
    let styles = crate::flowchart::FlowchartClassDefStyles::prepare(declarations);

    // This is deliberately a safe headless projection, not a complete browser CSS engine:
    // `FlowchartStyleDeclaration` rejects resource-bearing values that CSSOM would retain.
    (
        flowchart_classdef_cssom_string(&styles.shape),
        flowchart_classdef_cssom_string(&styles.text),
    )
}

pub(super) fn flowchart_classdef_cssom_string(
    cssom: &IndexMap<String, std::sync::Arc<crate::flowchart::FlowchartStyleDeclaration>>,
) -> String {
    let mut style = String::new();
    for (property, declaration) in cssom {
        let value = if !property.starts_with("--")
            && (matches!(property.as_str(), "fill" | "stroke" | "color")
                || property.ends_with("-color"))
        {
            super::super::util::cssom_color_value(declaration.value())
        } else {
            declaration.value().to_owned()
        };
        let _ = write!(&mut style, "{}:{}!important;", property, value);
    }
    style
}

struct ScopedFlowchartDropShadow<'a, DropShadowId, DropShadowSmallId> {
    source: &'a str,
    drop_shadow_id: DropShadowId,
    drop_shadow_small_id: DropShadowSmallId,
}

impl<DropShadowId, DropShadowSmallId> std::fmt::Display
    for ScopedFlowchartDropShadow<'_, DropShadowId, DropShadowSmallId>
where
    DropShadowId: std::fmt::Display,
    DropShadowSmallId: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        const PREFIX: &str = "url(#drop-shadow";
        const SMALL_SUFFIX: &str = "-small)";

        let mut remaining = self.source;
        loop {
            let Some(offset) = remaining.find(PREFIX) else {
                return f.write_str(remaining);
            };

            f.write_str(&remaining[..offset])?;
            let suffix = &remaining[offset + PREFIX.len()..];
            if let Some(next) = suffix.strip_prefix(SMALL_SUFFIX) {
                f.write_str("url(#")?;
                std::fmt::Display::fmt(&self.drop_shadow_small_id, f)?;
                f.write_str(")")?;
                remaining = next;
            } else if let Some(next) = suffix.strip_prefix(')') {
                f.write_str("url(#")?;
                std::fmt::Display::fmt(&self.drop_shadow_id, f)?;
                f.write_str(")")?;
                remaining = next;
            } else {
                // Preserve near-matches byte-for-byte and continue after the common prefix. Each
                // input byte therefore belongs to at most one subsequent `find` range.
                f.write_str(PREFIX)?;
                remaining = suffix;
            }
        }
    }
}

pub(super) fn flowchart_normal_edge_css_width(
    theme: &crate::flowchart::FlowchartCompatibilityBinding,
) -> Cow<'_, str> {
    theme
        .connector_stroke_width
        .map_or(Cow::Borrowed(theme.stroke_width.as_str()), |width| {
            Cow::Owned(width.to_string())
        })
}

#[allow(
    clippy::too_many_arguments,
    reason = "The streaming CSS writer keeps independently typed SVG definition IDs and theme inputs explicit."
)]
pub(super) fn write_flowchart_css<DiagramId, DropShadowId, DropShadowSmallId, GradientId>(
    out: &mut impl crate::svg::parity::SvgOutput,
    diagram_id: DiagramId,
    diagram_type: &str,
    drop_shadow_id: DropShadowId,
    drop_shadow_small_id: DropShadowSmallId,
    gradient_id: GradientId,
    theme: &crate::flowchart::FlowchartCompatibilityBinding,
    native_title: Option<&crate::text::TextStyle>,
    palette: &super::agentflow::ContainerPalette<'_>,
    font_family: &str,
    font_size: f64,
    class_defs: &IndexMap<String, Vec<String>>,
    defer_palette: bool,
) -> Result<Option<usize>>
where
    DiagramId: SvgDiagramIdValue,
    DropShadowId: std::fmt::Display,
    DropShadowSmallId: std::fmt::Display,
    GradientId: std::fmt::Display,
{
    let id = css_selector_diagram_id(diagram_id);
    let stroke = theme.line_color.as_str();
    let arrowhead_color = theme.arrowhead_color.as_str();
    let node_border = theme.node_border.as_str();
    let main_bkg = theme.main_bkg.as_str();
    let text_color = theme.text_color.as_str();
    let node_text_color = theme.node_text_color.as_str();
    let title_color = theme.title_color.as_str();
    let stroke_width = theme.stroke_width.as_str();
    let connector_width = flowchart_normal_edge_css_width(theme);
    let connector_width: &str = connector_width.as_ref();
    let error_bkg = theme.error_bkg.as_str();
    let error_text = theme.error_text.as_str();
    let edge_label_background = theme.edge_label_background.as_str();
    let tertiary = theme.tertiary.as_str();
    let cluster_bkg = theme.cluster_bkg.as_str();
    let cluster_border = theme.cluster_border.as_str();
    let tooltip_border = &theme.border2;
    let drop_shadow = &theme.drop_shadow;

    let faded_label_background;
    let label_bkg = if native_title.is_some() {
        // Native labels paint their own backplate, including the transparent case.
        edge_label_background
    } else {
        faded_label_background = css_rgba_fade(edge_label_background, 0.5)?;
        &faded_label_background
    };
    let scoped_drop_shadow = ScopedFlowchartDropShadow {
        source: drop_shadow,
        drop_shadow_id,
        drop_shadow_small_id,
    };

    let _ = write!(
        &mut *out,
        r#"#{}{{font-family:{};font-size:{}px;fill:{};}}"#,
        id,
        font_family,
        fmt(font_size),
        text_color
    );
    if let Some(weight) = theme.root_font_weight {
        let _ = write!(out, "#{id}{{font-weight:{weight};}}");
    }
    out.push_str(
        r#"@keyframes edge-animation-frame{from{stroke-dashoffset:0;}}@keyframes dash{to{stroke-dashoffset:0;}}"#,
    );
    let _ = write_static_css!(
        out,
        "#",
        id => " .edge-animation-slow{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 50s linear infinite;stroke-linecap:round;}#",
        id => " .edge-animation-fast{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 20s linear infinite;stroke-linecap:round;}",
    );
    let _ = write!(
        &mut *out,
        r#"#{} .error-icon{{fill:{};}}#{} .error-text{{fill:{};stroke:{};}}"#,
        id, error_bkg, id, error_text, error_text
    );
    let _ = write_static_css!(
        out,
        "#",
        id => " .edge-thickness-normal{stroke-width:",
        connector_width => "px;}#",
        id => " .edge-thickness-thick{stroke-width:3.5px;}#",
        id => " .edge-pattern-solid{stroke-dasharray:0;}#",
        id => " .edge-thickness-invisible{stroke-width:0;fill:none;}#",
        id => " .edge-pattern-dashed{stroke-dasharray:3;}#",
        id => " .edge-pattern-dotted{stroke-dasharray:2;}",
    );
    let _ = write!(
        &mut *out,
        r#"#{} .marker{{fill:{};stroke:{};}}#{} .marker.cross{{stroke:{};}}"#,
        id, stroke, stroke, id, stroke
    );
    let _ = write!(
        &mut *out,
        r#"#{} svg{{font-family:{};font-size:{}px;}}#{} p{{margin:0;}}"#,
        id,
        font_family,
        fmt(font_size),
        id
    );
    out.checkpoint()?;
    let palette_offset = if diagram_type == "agentflow" {
        None
    } else if defer_palette {
        Some(out.len())
    } else {
        super::agentflow::write_flowchart_container_css(
            out,
            id,
            palette,
            0..palette.color_count(),
        )?;
        None
    };
    let _ = write!(
        &mut *out,
        r#"#{} .label{{font-family:{};color:{};}}"#,
        id, font_family, node_text_color
    );
    let _ = write!(out, "#{id} .cluster-label text{{");
    write_cluster_title_paint(out, title_color, false);
    let _ = write!(out, "}}#{id} .cluster-label span{{");
    write_cluster_title_paint(out, title_color, true);
    let _ = write!(
        out,
        "}}#{id} .cluster-label span p{{background-color:transparent;}}#{id} .label text,#{id} span{{fill:{node_text_color};color:{node_text_color};}}"
    );
    let _ = write_static_css!(
        out,
        "#",
        id => " .node rect,#",
        id => " .node circle,#",
        id => " .node ellipse,#",
        id => " .node polygon,#",
        id => " .node path{fill:",
        main_bkg => ";stroke:",
        node_border => ";stroke-width:",
        stroke_width => "px;}#",
        id => " .rough-node .label text,#",
        id => " .node .label text,#",
        id => " .image-shape .label,#",
        id => " .icon-shape .label{text-anchor:middle;}#",
        id => " .node .katex path{fill:#000;stroke:#000;stroke-width:1px;}#",
        id => " .rough-node .label,#",
        id => " .node .label,#",
        id => " .image-shape .label,#",
        id => " .icon-shape .label{text-align:center;}#",
        id => " .node.clickable{cursor:pointer;}",
    );
    let _ = write!(
        &mut *out,
        r#"#{} .root .anchor path{{fill:{}!important;stroke-width:0;stroke:{};}}#{} .arrowheadPath{{fill:{};}}#{} .edgePaths .path{{stroke:{};stroke-width:{}px;}}#{} .flowchart-link{{stroke:{};fill:none;}}"#,
        id, stroke, stroke, id, arrowhead_color, id, stroke, connector_width, id, stroke
    );
    let _ = write!(
        &mut *out,
        r#"#{} .edgeLabel{{background-color:{};text-align:center;}}#{} .edgeLabel p{{background-color:{};}}#{} .edgeLabel rect{{opacity:0.5;background-color:{};fill:{};}}#{} .labelBkg{{background-color:{};}}"#,
        id,
        edge_label_background,
        id,
        edge_label_background,
        id,
        edge_label_background,
        edge_label_background,
        id,
        label_bkg
    );
    let _ = write!(
        out,
        "#{id} .cluster rect{{fill:{cluster_bkg};stroke:{cluster_border};stroke-width:1px;}}#{id} .cluster text{{"
    );
    write_cluster_title_paint(out, title_color, false);
    let _ = write!(out, "}}#{id} .cluster span{{");
    write_cluster_title_paint(out, title_color, true);
    out.push('}');
    let _ = write!(
        &mut *out,
        r#"#{id} .node .collapsed-indicator{{fill:{cluster_border};stroke:none;opacity:0.6;}}#{id} .node .collapsed-separator{{stroke:{cluster_border};stroke-width:0.75px;}}"#,
    );
    let _ = write_static_css!(
        out,
        "#",
        id => " div.mermaidTooltip{position:absolute;text-align:center;max-width:200px;padding:2px;font-family:",
        font_family => ";font-size:12px;background:",
        tertiary => ";border:1px solid ",
        tooltip_border => ";border-radius:2px;pointer-events:none;z-index:100;}#",
        id => " .",
        title_css_class(diagram_type) => "{text-anchor:middle;",
    );
    if let Some(style) = native_title {
        let _ = write!(out, "font-size:{}px;fill:{title_color};", style.font_size);
        if let Some(family) = style.font_family.as_deref() {
            let _ = write!(out, "font-family:{family};");
        }
        if let Some(weight) = style.font_weight.as_deref() {
            let _ = write!(out, "font-weight:{weight};");
        }
    } else {
        let size = super::viewbox::TITLE_FONT_SIZE_PX;
        let _ = write!(out, "font-size:{size}px;fill:{text_color};");
    }
    let _ = write!(out, "}}#{id} rect.text{{fill:none;stroke-width:0;}}");
    let _ = write_static_css!(
        out,
        "#",
        id => " .icon-shape,#",
        id => " .image-shape{background-color:",
        edge_label_background => ";text-align:center;}#",
        id => " .icon-shape p,#",
        id => " .image-shape p{background-color:",
        edge_label_background => ";padding:2px;}#",
        id => " .icon-shape .label rect,#",
        id => " .image-shape .label rect{opacity:0.5;background-color:",
        edge_label_background => ";fill:",
        edge_label_background => ";}#",
        id => " .label-icon{display:inline-block;height:1em;overflow:visible;vertical-align:-0.125em;}#",
        id => " .node .label-icon path{fill:currentColor;stroke:revert;stroke-width:revert;}",
    );
    theme
        .neo
        .write_with_ids(out, id, gradient_id, scoped_drop_shadow)?;
    let _ = crate::svg::parity::css::write_mermaid_base_css_root_rule_to(
        &mut *out,
        id,
        &theme.root_font_family,
    );

    // Mermaid `createCssStyles(...)` chooses different selectors based on `htmlLabels`.
    // - HTML labels: `.classDef > *` + `.classDef span`
    // - SVG labels: `.classDef rect|polygon|ellipse|circle|path`
    let html_labels = theme.html_labels;
    let shape_elements: &[&str] = &["rect", "polygon", "ellipse", "circle", "path"];

    // Flush the fixed stylesheet prefix before processing attacker-controlled class catalogs.
    out.checkpoint()?;

    for (class, decls) in class_defs {
        if decls.is_empty() {
            continue;
        }
        let (style, text_style) = flowchart_classdef_css_declarations(decls);
        if style.is_empty() {
            continue;
        }
        if html_labels {
            let _ = write!(
                &mut *out,
                r#"#{} .{}>*{{{}}}#{} .{} span{{{}}}"#,
                id,
                crate::svg::escape_css_identifier(class),
                style,
                id,
                crate::svg::escape_css_identifier(class),
                style
            );
        } else {
            for css_element in shape_elements {
                let _ = write!(
                    &mut *out,
                    r#"#{} .{} {}{{{}}}"#,
                    id,
                    crate::svg::escape_css_identifier(class),
                    css_element,
                    style
                );
            }
        }
        if !text_style.is_empty() {
            let _ = write!(
                &mut *out,
                r#"#{} .{} tspan{{{}}}"#,
                id,
                crate::svg::escape_css_identifier(class),
                text_style
            );
        }
        out.checkpoint()?;
    }

    Ok(palette_offset)
}

fn write_cluster_title_paint(out: &mut impl std::fmt::Write, configured: &str, html: bool) {
    let property = if html { "color" } else { "fill" };
    let _ = write!(out, "{property}:{configured};");
}

#[cfg(test)]
fn flowchart_css(
    diagram_id: &str,
    diagram_type: &str,
    effective_config: &serde_json::Value,
    font_family: &str,
    font_size: f64,
    class_defs: &IndexMap<String, Vec<String>>,
) -> Result<String> {
    let mut out = String::new();
    write_flowchart_css(
        &mut out,
        diagram_id,
        diagram_type,
        format!("{diagram_id}-merman-flowchart-document-filter-drop-shadow"),
        format!("{diagram_id}-merman-flowchart-document-filter-drop-shadow-small"),
        format!("{diagram_id}-merman-flowchart-document-gradient-root"),
        &crate::flowchart::FlowchartCompatibilityBinding::resolve(effective_config),
        None,
        &super::agentflow::ContainerPalette::new(effective_config),
        font_family,
        font_size,
        class_defs,
        false,
    )?;
    Ok(out)
}

#[inline]
pub(super) fn write_flowchart_edge_class_attr(
    out: &mut impl crate::svg::parity::SvgOutput,
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
    use serde_json::json;

    fn scoped_drop_shadow(source: &str) -> String {
        ScopedFlowchartDropShadow {
            source,
            drop_shadow_id: "scope-filter-drop-shadow",
            drop_shadow_small_id: "scope-filter-drop-shadow-small",
        }
        .to_string()
    }

    #[test]
    fn scoped_drop_shadow_rewrites_repeated_exact_tokens_in_one_pass_order() {
        let source = "url(#drop-shadow)|url(#drop-shadow-small)|url(#drop-shadow)|".repeat(64);
        let expected = "url(#scope-filter-drop-shadow)|url(#scope-filter-drop-shadow-small)|url(#scope-filter-drop-shadow)|"
            .repeat(64);

        assert_eq!(scoped_drop_shadow(&source), expected);
    }

    #[test]
    fn scoped_drop_shadow_preserves_near_matches_byte_for_byte() {
        let source = concat!(
            "url(#drop-shadowx) ",
            "url(#drop-shadow-smallx) ",
            "url(#drop-shadow-small ) ",
            "url(#drop-shadow ",
            "url(#drop-shadow-SMALL) ",
            "URL(#drop-shadow)"
        );

        assert_eq!(scoped_drop_shadow(source), source);
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

        let error = write_flowchart_css(
            &mut out,
            "bounded-css",
            "flowchart-v2",
            "bounded-css-merman-flowchart-document-filter-drop-shadow",
            "bounded-css-merman-flowchart-document-filter-drop-shadow-small",
            "bounded-css-merman-flowchart-document-gradient-root",
            &crate::flowchart::FlowchartCompatibilityBinding::resolve(&config),
            None,
            &super::super::agentflow::ContainerPalette::new(&config),
            "sans-serif",
            16.0,
            &class_defs,
            false,
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
        let mut svg = String::from("<svg><style>");
        write_flowchart_css(
            &mut super::super::super::output::XmlTextOutput::new(&mut svg),
            "selector-scope",
            "flowchart-v2",
            "shadow",
            "shadow-small",
            "gradient",
            &crate::flowchart::FlowchartCompatibilityBinding::resolve(&config),
            None,
            &super::super::agentflow::ContainerPalette::new(&config),
            "sans-serif",
            16.0,
            &class_defs,
            false,
        )
        .unwrap();
        svg.push_str("</style></svg>");
        let document = crate::xml_dom::Document::parse(&svg).unwrap();
        let css = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .unwrap()
            .text()
            .unwrap();
        let escaped = crate::svg::escape_css_identifier(class);
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
            "flowchart-v2",
            &json!({
                "themeVariables": {
                    "edgeLabelBackground": "rebeccapurple"
                }
            }),
            "\"trebuchet ms\",verdana,arial,sans-serif",
            16.0,
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
            "flowchart-v2",
            &json!({
                "themeVariables": {
                    "edgeLabelBackground": "not-a-css-color"
                }
            }),
            "\"trebuchet ms\",verdana,arial,sans-serif",
            16.0,
            &IndexMap::new(),
        )
        .expect_err("unsupported khroma color must fail");

        assert!(error.to_string().contains("not-a-css-color"));
    }

    #[test]
    fn ordinary_neo_without_profile_keeps_mermaid_common_css_only() {
        let css = flowchart_css(
            "ordinary_neo",
            "flowchart-v2",
            &json!({"look": "neo"}),
            "\"trebuchet ms\",verdana,arial,sans-serif",
            16.0,
            &IndexMap::new(),
        )
        .expect("valid Mermaid CSS");

        assert!(!css.contains(
            r#".flowchart-link[data-look="neo"]{stroke-linecap:round;stroke-linejoin:round;}"#
        ));
    }
}
