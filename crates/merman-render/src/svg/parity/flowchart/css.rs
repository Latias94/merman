//! Flowchart CSS generation.

use super::*;

/// One Flowchart CSS selector identity that preserves the operation-owned SVG ID projection.
///
/// Production callers pass the already normalized [`SvgDiagramId`], so every selector occurrence
/// is formatted through its projection. Test-only raw strings retain the historical CSS escaping
/// behavior without weakening the production boundary.
#[derive(Clone, Copy)]
pub(in crate::svg::parity::flowchart) struct FlowchartCssSelectorDiagramId<I>(I);

impl<I> FlowchartCssSelectorDiagramId<I> {
    pub(in crate::svg::parity::flowchart) const fn new(value: I) -> Self {
        Self(value)
    }
}

impl<I> std::fmt::Display for FlowchartCssSelectorDiagramId<I>
where
    I: SvgDiagramIdValue,
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let semantic = self.0.semantic_value();
        let normalized = semantic
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic)
            && semantic
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
        if normalized {
            std::fmt::Display::fmt(&self.0, formatter)
        } else {
            formatter.write_str(&crate::svg::escape_css_identifier(semantic))
        }
    }
}

#[derive(Debug)]
struct FlowchartClassDefCssDeclaration {
    property_css: String,
    value: String,
}

fn flowchart_classdef_css_declarations(declarations: &[String]) -> (String, String) {
    let mut shape_cssom = IndexMap::<String, FlowchartClassDefCssDeclaration>::new();
    let mut text_cssom = IndexMap::<String, FlowchartClassDefCssDeclaration>::new();

    for raw in declarations {
        flowchart_classdef_cssom_set(&mut shape_cssom, raw);

        // FlowDB builds `textStyles` before CSSOM by selecting source declarations that contain
        // the exact lowercase substring `color`, first replacing the first source `fill` with
        // `bgFill`. `createCssStyles()` later replaces the first `color` with `fill`. Keep both
        // source-sensitive steps separate from the canonical CSSOM declaration map.
        if raw.contains("color") {
            let text_raw = raw
                .replacen("fill", "bgFill", 1)
                .replacen("color", "fill", 1);
            flowchart_classdef_cssom_set(&mut text_cssom, &text_raw);
        }
    }

    // This is deliberately a safe headless projection, not a complete browser CSS engine:
    // `PreparedSourceStyleDeclaration` rejects resource-bearing values that CSSOM would retain.
    (
        flowchart_classdef_cssom_string(shape_cssom),
        flowchart_classdef_cssom_string(text_cssom),
    )
}

/// SVG cluster labels omit inline label styles, but assigned classes still reach their tspans.
pub(in crate::svg::parity::flowchart) fn cluster_title_class_foreground(
    class_defs: &IndexMap<String, Vec<String>>,
    classes: &[String],
    work: &crate::resources::OperationWorkMeter,
) -> Result<crate::flowchart::FlowchartSourceFacetStatus> {
    use crate::flowchart::FlowchartSourceFacetStatus;
    let mut status = FlowchartSourceFacetStatus::Absent;
    for class in classes {
        work.charge(1)?;
        let Some(declarations) = class_defs.get(class) else {
            continue;
        };
        for declaration in declarations {
            work.charge(declaration.len())?;
        }
        let (_, text_style) = flowchart_classdef_css_declarations(declarations);
        for raw in crate::flowchart::flowchart_split_mermaid_style_decls(&text_style) {
            if let Some(declaration) =
                crate::diagram_theme::PreparedSourceStyleDeclaration::parse(raw)
                && declaration.property() == "fill"
            {
                status = status.merge(FlowchartSourceFacetStatus::from_parts(
                    true,
                    crate::mermaid_style::is_supported_css_color_value(declaration.value()),
                ));
            }
        }
    }
    Ok(status)
}

fn flowchart_classdef_cssom_set(
    cssom: &mut IndexMap<String, FlowchartClassDefCssDeclaration>,
    raw: &str,
) {
    let Some(declaration) = crate::diagram_theme::PreparedSourceStyleDeclaration::parse(raw) else {
        return;
    };
    let property = declaration.property().to_string();
    let value = if !property.starts_with("--")
        && (matches!(property.as_str(), "fill" | "stroke" | "color")
            || property.ends_with("-color"))
    {
        super::super::util::cssom_color_value(declaration.value())
    } else {
        declaration.value().to_string()
    };

    // `createCssStyles()` crosses a browser CSSStyleSheet boundary. Ordinary CSS property names
    // therefore share decoded, lowercase identity; custom properties retain decoded case-sensitive
    // identity. Replacing either kind moves the surviving property to its final declaration
    // position, and CSSOM serializes the decoded property spelling.
    cssom.shift_remove(&property);
    cssom.insert(
        property.clone(),
        FlowchartClassDefCssDeclaration {
            property_css: property,
            value,
        },
    );
}

fn flowchart_classdef_cssom_string(
    cssom: IndexMap<String, FlowchartClassDefCssDeclaration>,
) -> String {
    let mut style = String::new();
    for declaration in cssom.values() {
        let _ = write!(
            &mut style,
            "{}:{}!important;",
            declaration.property_css, declaration.value
        );
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

#[allow(
    clippy::too_many_arguments,
    reason = "The streaming CSS writer keeps independently typed SVG definition IDs and theme inputs explicit."
)]
pub(in crate::svg::parity) fn write_flowchart_css<
    DiagramId,
    DropShadowId,
    DropShadowSmallId,
    GradientId,
>(
    out: &mut impl crate::svg::parity::SvgOutput,
    diagram_id: DiagramId,
    diagram_type: &str,
    drop_shadow_id: DropShadowId,
    drop_shadow_small_id: DropShadowSmallId,
    gradient_id: GradientId,
    effective_config: &serde_json::Value,
    font_family: &str,
    font_size: f64,
    class_defs: &IndexMap<String, Vec<String>>,
    text_surface_paint: Option<&crate::flowchart::FlowchartTextSurfacePaintPlan>,
) -> Result<()>
where
    DiagramId: SvgDiagramIdValue,
    DropShadowId: std::fmt::Display,
    DropShadowSmallId: std::fmt::Display,
    GradientId: std::fmt::Display,
{
    let id = FlowchartCssSelectorDiagramId(diagram_id);
    let theme = MermaidThemeAdapter::new(effective_config).node_diagram();
    let stroke = theme.common.line_color.as_str();
    let arrowhead_color = theme.arrowhead_color.as_str();
    let node_border = theme.node_border.as_str();
    let main_bkg = theme.main_bkg.as_str();
    let text_color = text_surface_paint.map_or(theme.common.text_color.as_str(), |plan| {
        plan.generic_text.color(
            crate::flowchart::FlowchartTextPaintChannel::DiagramTitle,
            theme.common.text_color.as_str(),
        )
    });
    let node_text_color = text_surface_paint.map_or(theme.node_text_color.as_str(), |plan| {
        plan.generic_text.color(
            crate::flowchart::FlowchartTextPaintChannel::Node,
            theme.node_text_color.as_str(),
        )
    });
    let title_color = theme.title_color.as_str();
    let stroke_width = theme.stroke_width.as_str();
    let error_bkg = theme.common.error_bkg.as_str();
    let error_text = theme.common.error_text.as_str();
    let edge_label_background = text_surface_paint
        .map_or(theme.edge_label_background.as_str(), |plan| {
            plan.background.color(theme.edge_label_background.as_str())
        });
    let tertiary = theme.tertiary.as_str();
    let cluster_bkg = theme.cluster_bkg.as_str();
    let cluster_border = theme.cluster_border.as_str();
    let tooltip_border = theme_token(effective_config, "border2", cluster_border);
    let drop_shadow = theme_token(effective_config, "dropShadow", "none");

    let typed_background =
        text_surface_paint.is_some_and(|plan| plan.background.supplies_background());
    // Typed paint owns one layer, including its alpha. Mermaid configuration retains
    // the historical paragraph/div compositing and native rectangle fade.
    let background_underlay = if typed_background {
        "transparent"
    } else {
        edge_label_background
    };
    let background_opacity = if typed_background { "1" } else { "0.5" };
    let label_bkg = if typed_background {
        edge_label_background.to_owned()
    } else {
        css_rgba_fade(edge_label_background, 0.5)?
    };
    let scoped_drop_shadow = ScopedFlowchartDropShadow {
        source: &drop_shadow,
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
    out.push_str(
        r#"@keyframes edge-animation-frame{from{stroke-dashoffset:0;}}@keyframes dash{to{stroke-dashoffset:0;}}"#,
    );
    let _ = write!(
        &mut *out,
        r#"#{} .edge-animation-slow{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 50s linear infinite;stroke-linecap:round;}}#{} .edge-animation-fast{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 20s linear infinite;stroke-linecap:round;}}"#,
        id, id
    );
    let _ = write!(
        &mut *out,
        r#"#{} .error-icon{{fill:{};}}#{} .error-text{{fill:{};stroke:{};}}"#,
        id, error_bkg, id, error_text, error_text
    );
    let _ = write!(
        &mut *out,
        r#"#{} .edge-thickness-normal{{stroke-width:{}px;}}#{} .edge-thickness-thick{{stroke-width:3.5px;}}#{} .edge-pattern-solid{{stroke-dasharray:0;}}#{} .edge-thickness-invisible{{stroke-width:0;fill:none;}}#{} .edge-pattern-dashed{{stroke-dasharray:3;}}#{} .edge-pattern-dotted{{stroke-dasharray:2;}}"#,
        id, stroke_width, id, id, id, id, id
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
    if diagram_type != "agentflow" {
        super::agentflow::write_flowchart_container_css(out, id, effective_config)?;
    }
    let _ = write!(
        &mut *out,
        r#"#{} .label{{font-family:{};color:{};}}"#,
        id, font_family, node_text_color
    );
    let _ = write!(out, "#{id} .cluster-label text{{");
    write_cluster_title_paint(out, text_surface_paint, title_color, false);
    let _ = write!(out, "}}#{id} .cluster-label span{{");
    write_cluster_title_paint(out, text_surface_paint, title_color, true);
    let _ = write!(
        out,
        "}}#{id} .cluster-label span p{{background-color:transparent;}}#{id} .label text,#{id} span{{fill:{node_text_color};color:{node_text_color};}}"
    );
    if let Some(plan) = text_surface_paint.filter(|plan| plan.generic_text.requested()) {
        let edge_text_color = plan.generic_text.color(
            crate::flowchart::FlowchartTextPaintChannel::Edge,
            theme.node_text_color.as_str(),
        );
        let _ = write!(
            out,
            "#{id} .edgeLabel .label,#{id} .edgeLabel .label text,#{id} .edgeLabel span{{fill:{edge_text_color};color:{edge_text_color};}}"
        );
    }
    let _ = write!(
        &mut *out,
        r#"#{id} .node rect,#{id} .node circle,#{id} .node ellipse,#{id} .node polygon,#{id} .node path{{fill:{main_bkg};stroke:{node_border};stroke-width:{stroke_width}px;}}#{id} .rough-node .label text,#{id} .node .label text,#{id} .image-shape .label,#{id} .icon-shape .label{{text-anchor:middle;}}#{id} .node .katex path{{fill:#000;stroke:#000;stroke-width:1px;}}#{id} .rough-node .label,#{id} .node .label,#{id} .image-shape .label,#{id} .icon-shape .label{{text-align:center;}}#{id} .node.clickable{{cursor:pointer;}}"#
    );
    let _ = write!(
        &mut *out,
        r#"#{} .root .anchor path{{fill:{}!important;stroke-width:0;stroke:{};}}#{} .arrowheadPath{{fill:{};}}#{} .edgePaths .path{{stroke:{};stroke-width:{}px;}}#{} .flowchart-link{{stroke:{};fill:none;}}"#,
        id, stroke, stroke, id, arrowhead_color, id, stroke, stroke_width, id, stroke
    );
    let _ = write!(
        &mut *out,
        r#"#{} .edgeLabel{{background-color:{};text-align:center;}}#{} .edgeLabel p{{background-color:{};}}#{} .edgeLabel rect{{opacity:{background_opacity};background-color:{};fill:{};}}#{} .labelBkg{{background-color:{};}}"#,
        id,
        background_underlay,
        id,
        background_underlay,
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
    write_cluster_title_paint(out, text_surface_paint, title_color, false);
    let _ = write!(out, "}}#{id} .cluster span{{");
    write_cluster_title_paint(out, text_surface_paint, title_color, true);
    out.push('}');
    let _ = write!(
        &mut *out,
        r#"#{id} .node .collapsed-indicator{{fill:{cluster_border};stroke:none;opacity:0.6;}}#{id} .node .collapsed-separator{{stroke:{cluster_border};stroke-width:0.75px;}}"#,
    );
    let _ = write!(
        &mut *out,
        "#{} div.mermaidTooltip{{position:absolute;text-align:center;max-width:200px;padding:2px;font-family:{};font-size:12px;background:{};border:1px solid {};border-radius:2px;pointer-events:none;z-index:100;}}#{} .{}{{text-anchor:middle;font-size:18px;fill:{};}}#{} rect.text{{fill:none;stroke-width:0;}}",
        id,
        font_family,
        tertiary,
        tooltip_border,
        id,
        title_css_class(diagram_type),
        text_color,
        id
    );
    let _ = write!(
        &mut *out,
        r#"#{} .icon-shape,#{} .image-shape{{background-color:{};text-align:center;}}#{} .icon-shape p,#{} .image-shape p{{background-color:{};padding:2px;}}#{} .icon-shape .label rect,#{} .image-shape .label rect{{opacity:{background_opacity};background-color:{};fill:{};}}#{} .label-icon{{display:inline-block;height:1em;overflow:visible;vertical-align:-0.125em;}}#{} .node .label-icon path{{fill:currentColor;stroke:revert;stroke-width:revert;}}"#,
        id,
        id,
        background_underlay,
        id,
        id,
        background_underlay,
        id,
        id,
        edge_label_background,
        edge_label_background,
        id,
        id,
    );
    crate::svg::parity::css::write_mermaid_common_neo_css_with_ids(
        out,
        id,
        gradient_id,
        scoped_drop_shadow,
        effective_config,
    )?;
    let _ = crate::svg::parity::css::write_mermaid_base_css_root_rule_to(
        &mut *out,
        id,
        &crate::config::config_root_font_family_css(effective_config),
    );

    // Mermaid `createCssStyles(...)` chooses different selectors based on `htmlLabels`.
    // - HTML labels: `.classDef > *` + `.classDef span`
    // - SVG labels: `.classDef rect|polygon|ellipse|circle|path`
    let html_labels =
        crate::flowchart::FlowchartConfigView::new(effective_config).effective_html_labels();
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
            // Mermaid (via Stylis) ends up serializing the `>` combinator inside `<style>` as
            // `&gt;` in the final SVG string (see upstream baselines).
            let _ = write!(
                &mut *out,
                r#"#{} .{}&gt;*{{{}}}#{} .{} span{{{}}}"#,
                id,
                escape_xml(class),
                style,
                id,
                escape_xml(class),
                style
            );
        } else {
            for css_element in shape_elements {
                let _ = write!(
                    &mut *out,
                    r#"#{} .{} {}{{{}}}"#,
                    id,
                    escape_xml(class),
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
                escape_xml(class),
                text_style
            );
        }
        out.checkpoint()?;
    }

    Ok(())
}

fn write_cluster_title_paint(
    out: &mut impl std::fmt::Write,
    plan: Option<&crate::flowchart::FlowchartTextSurfacePaintPlan>,
    configured: &str,
    html: bool,
) {
    if let Some(plan) = plan {
        let _ = plan.write_css(out, configured, html);
    } else {
        let property = if html { "color" } else { "fill" };
        let _ = write!(out, "{property}:{configured};");
    }
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
        &format!("{diagram_id}-merman-flowchart-document-filter-drop-shadow"),
        &format!("{diagram_id}-merman-flowchart-document-filter-drop-shadow-small"),
        &format!("{diagram_id}-merman-flowchart-document-gradient-root"),
        effective_config,
        font_family,
        font_size,
        class_defs,
        None,
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

        let error = write_flowchart_css(
            &mut out,
            "bounded-css",
            "flowchart-v2",
            "bounded-css-merman-flowchart-document-filter-drop-shadow",
            "bounded-css-merman-flowchart-document-filter-drop-shadow-small",
            "bounded-css-merman-flowchart-document-gradient-root",
            &json!({}),
            "sans-serif",
            16.0,
            &class_defs,
            None,
        )
        .expect_err("the fixed Flowchart CSS prefix must exceed the tiny ceiling");

        assert!(matches!(error, crate::Error::ResourceLimitExceeded(_)));
        assert!(
            !out.as_str().contains("late-class"),
            "classDef emission must not continue after the fixed-prefix checkpoint"
        );
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
