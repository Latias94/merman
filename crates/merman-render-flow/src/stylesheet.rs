//! Flowchart document stylesheet and deferred container palette emission.

use std::fmt::Write as _;
use std::{borrow::Cow, fmt};

use indexmap::IndexMap;
use merman_markup::theme_color::{ColorChannel, ThemeColor, rgba};
use merman_render_core::{
    Error, Result,
    number_format::canonical_number,
    resources::OperationWorkMeter,
    svg_css::{
        CssSelectorDiagramId, PreparedCommonNeoCss, SvgDiagramIdValue, SvgId,
        escape_css_identifier, write_mermaid_base_css_root_rule,
    },
    svg_output::{BoundedSvgOutput, SvgOutput, XmlTextOutput},
    text::TextStyle,
    theme_svg::cssom_color_value,
    write_static_css,
};
use merman_theme_contract::v2::Color;

use crate::{class_style::FlowchartClassDefStyles, source_style::FlowchartStyleDeclaration};

mod palette;

pub use palette::ContainerPalette;

pub const TITLE_FONT_SIZE_PX: f64 = 18.0;

/// Shared by the title element and its stylesheet; Agentflow owns its title class.
pub fn title_css_class(diagram_type: &str) -> &'static str {
    if diagram_type == "agentflow" {
        "agentflowTitleText"
    } else {
        "flowchartTitleText"
    }
}

/// Borrows the stylesheet inputs from the already prepared compatibility and theme binding.
pub struct FlowchartStylesheetTheme<'a> {
    pub line_color: &'a str,
    pub arrowhead_color: &'a str,
    pub node_border: &'a str,
    pub main_bkg: &'a str,
    pub text_color: &'a str,
    pub node_text_color: &'a str,
    pub title_color: &'a str,
    pub stroke_width: &'a str,
    pub connector_stroke_width: Option<f32>,
    pub error_bkg: &'a str,
    pub error_text: &'a str,
    pub edge_label_background: &'a str,
    pub tertiary: &'a str,
    pub cluster_bkg: &'a str,
    pub cluster_border: &'a str,
    pub border2: &'a str,
    pub drop_shadow: &'a str,
    pub root_font_family: &'a str,
    pub root_font_weight: Option<u16>,
    pub neo: &'a PreparedCommonNeoCss,
    pub html_labels: bool,
    pub hand_drawn: bool,
}

pub enum SwimlaneStylesheet {
    Mermaid,
    Native {
        container: Color,
        guide: Color,
        emphasis: Color,
        on_emphasis: Color,
    },
}

pub struct FlowchartStylesheetRequest<'a, DefinitionId> {
    pub diagram_id: SvgId<'a>,
    pub diagram_type: &'a str,
    pub drop_shadow_id: DefinitionId,
    pub drop_shadow_small_id: DefinitionId,
    pub gradient_id: DefinitionId,
    pub theme: FlowchartStylesheetTheme<'a>,
    pub native_title: Option<&'a TextStyle>,
    pub palette: &'a ContainerPalette<'a>,
    pub font_family: &'a str,
    pub font_size: f64,
    pub class_defs: &'a IndexMap<String, Vec<String>>,
    pub swimlane: Option<SwimlaneStylesheet>,
    pub allow_deferred_palette: bool,
}

pub struct DeferredFlowchartPalette<'a> {
    offset: usize,
    body_start: usize,
    diagram_id: SvgId<'a>,
    diagram_type: &'a str,
    palette: &'a ContainerPalette<'a>,
    work_meter: &'a OperationWorkMeter,
}

/// Writes the complete stylesheet element to the admitted document sink.
/// Supply the same operation meter used to construct `out`.
pub fn write_flowchart_stylesheet<'a, DefinitionId: fmt::Display>(
    out: &mut BoundedSvgOutput<'_>,
    work_meter: &'a OperationWorkMeter,
    request: FlowchartStylesheetRequest<'a, DefinitionId>,
) -> Result<Option<DeferredFlowchartPalette<'a>>> {
    out.push_str("<style>");
    out.checkpoint()?;
    // The palette scan observes the operation between items, independently of sink append errors.
    let defer_palette = request.allow_deferred_palette && request.palette.can_defer(work_meter)?;
    let palette_offset = {
        let mut css_out = XmlTextOutput::new(out);
        let flowchart_palette_offset = write_flowchart_css(&mut css_out, &request, defer_palette)?;
        let palette_offset = if request.diagram_type == "agentflow" {
            palette::write_agentflow_css(
                &mut css_out,
                request.diagram_id,
                request.palette,
                defer_palette,
            )?
        } else {
            flowchart_palette_offset
        };
        if let Some(swimlane) = request.swimlane {
            write_swimlane_css(
                &mut css_out,
                request.diagram_id,
                request.theme.cluster_border,
                swimlane,
            );
        }
        palette_offset
    };
    out.push_str("</style>");
    out.checkpoint()?;
    Ok(palette_offset.map(|offset| DeferredFlowchartPalette {
        offset,
        body_start: out.len(),
        diagram_id: request.diagram_id,
        diagram_type: request.diagram_type,
        palette: request.palette,
        work_meter,
    }))
}

impl DeferredFlowchartPalette<'_> {
    /// Inserts the final palette after the body has been appended and checked on the same sink.
    pub fn insert(self, out: &mut BoundedSvgOutput<'_>) -> Result<()> {
        let svg = out.as_str();
        let body = svg
            .as_bytes()
            .get(self.body_start..)
            .filter(|_| svg.is_char_boundary(self.offset))
            .ok_or_else(|| Error::InvalidModel {
                message: "deferred Flowchart palette requires its original SVG prefix".to_owned(),
            })?;
        let slots =
            palette::emitted_palette_slots(svg, body, self.palette.color_count(), self.work_meter)?;
        if slots.as_ref().is_none_or(|slots| !slots.is_empty()) {
            out.write_deferred_at(self.offset, |out| {
                let mut css_out = XmlTextOutput::new(out);
                if self.diagram_type == "agentflow" {
                    palette::write_agentflow_container_css(
                        &mut css_out,
                        self.diagram_id,
                        self.palette,
                        slots.as_ref(),
                    )
                } else {
                    let id = CssSelectorDiagramId(self.diagram_id);
                    match slots {
                        Some(slots) => palette::write_flowchart_container_css(
                            &mut css_out,
                            id,
                            self.palette,
                            slots,
                        ),
                        None => palette::write_flowchart_container_css(
                            &mut css_out,
                            id,
                            self.palette,
                            0..self.palette.color_count(),
                        ),
                    }
                }
            })?;
        }
        Ok(())
    }
}

fn flowchart_classdef_css_declarations(declarations: &[String]) -> (String, String) {
    let styles = FlowchartClassDefStyles::prepare(declarations);

    // This is deliberately a safe headless projection, not a complete browser CSS engine:
    // `FlowchartStyleDeclaration` rejects resource-bearing values that CSSOM would retain.
    (
        flowchart_classdef_cssom_string(&styles.shape),
        flowchart_classdef_cssom_string(&styles.text),
    )
}

pub fn flowchart_classdef_cssom_string(
    cssom: &IndexMap<String, std::sync::Arc<FlowchartStyleDeclaration>>,
) -> String {
    let mut style = String::new();
    for (property, declaration) in cssom {
        let value = if !property.starts_with("--")
            && (matches!(property.as_str(), "fill" | "stroke" | "color")
                || property.ends_with("-color"))
        {
            cssom_color_value(declaration.value())
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

pub fn flowchart_normal_edge_css_width(
    stroke_width: &str,
    connector_width: Option<f32>,
) -> Cow<'_, str> {
    connector_width.map_or(Cow::Borrowed(stroke_width), |width| {
        Cow::Owned(width.to_string())
    })
}

fn write_flowchart_css(
    out: &mut impl SvgOutput,
    request: &FlowchartStylesheetRequest<'_, impl fmt::Display>,
    defer_palette: bool,
) -> Result<Option<usize>> {
    let FlowchartStylesheetRequest {
        diagram_id,
        diagram_type,
        drop_shadow_id,
        drop_shadow_small_id,
        gradient_id,
        theme,
        native_title,
        palette,
        font_family,
        font_size,
        class_defs,
        ..
    } = request;
    let id = CssSelectorDiagramId(*diagram_id);
    let stroke = theme.line_color;
    let arrowhead_color = theme.arrowhead_color;
    let node_border = theme.node_border;
    let main_bkg = theme.main_bkg;
    let text_color = theme.text_color;
    let node_text_color = theme.node_text_color;
    let title_color = theme.title_color;
    let stroke_width = theme.stroke_width;
    let connector_width =
        flowchart_normal_edge_css_width(stroke_width, theme.connector_stroke_width);
    let connector_width: &str = connector_width.as_ref();
    let error_bkg = theme.error_bkg;
    let error_text = theme.error_text;
    let edge_label_background = theme.edge_label_background;
    let tertiary = theme.tertiary;
    let cluster_bkg = theme.cluster_bkg;
    let cluster_border = theme.cluster_border;
    let tooltip_border = theme.border2;
    let drop_shadow = theme.drop_shadow;

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
        canonical_number(*font_size),
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
        canonical_number(*font_size),
        id
    );
    out.checkpoint()?;
    let palette_offset = if *diagram_type == "agentflow" {
        None
    } else if defer_palette {
        Some(out.len())
    } else {
        palette::write_flowchart_container_css(out, id, palette, 0..palette.color_count())?;
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
        let size = TITLE_FONT_SIZE_PX;
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
    let _ = write_mermaid_base_css_root_rule(&mut *out, id, theme.root_font_family);

    // Mermaid `createCssStyles(...)` chooses different selectors based on `htmlLabels`.
    // - HTML labels: `.classDef > *` + `.classDef span`
    // - SVG labels: `.classDef rect|polygon|ellipse|circle|path`
    let html_labels = theme.html_labels;
    let shape_elements: &[&str] = &["rect", "polygon", "ellipse", "circle", "path"];

    // Flush the fixed stylesheet prefix before processing attacker-controlled class catalogs.
    out.checkpoint()?;

    for (class, decls) in *class_defs {
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
                escape_css_identifier(class),
                style,
                id,
                escape_css_identifier(class),
                style
            );
        } else if theme.hand_drawn {
            // Hand-drawn nodes render as `.rough-node` and bake classDef fill/stroke directly onto
            // their RoughJS paths. Restricting SVG classDef shape selectors to `.node` and
            // `.cluster` preserves classic/cluster styling in both browsers and `usvg` without
            // overwriting rough hachure fill strokes or open cylinder rims.
            for css_element in shape_elements {
                let _ = write!(
                    &mut *out,
                    r#"#{} .node.{} {},#{} .cluster.{} {}{{{}}}"#,
                    id,
                    escape_css_identifier(class),
                    css_element,
                    id,
                    escape_css_identifier(class),
                    css_element,
                    style
                );
            }
        } else {
            for css_element in shape_elements {
                let _ = write!(
                    &mut *out,
                    r#"#{} .{} {}{{{}}}"#,
                    id,
                    escape_css_identifier(class),
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
                escape_css_identifier(class),
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

fn css_rgba_fade(color: &str, opacity: f64) -> Result<String> {
    let color = ThemeColor::parse(color.trim())?;
    let faded = rgba(
        color.channel(ColorChannel::Red),
        color.channel(ColorChannel::Green),
        color.channel(ColorChannel::Blue),
        opacity,
    )?;
    Ok(faded)
}

fn write_swimlane_css<DiagramId>(
    out: &mut impl SvgOutput,
    diagram_id: DiagramId,
    cluster_border: &str,
    style: SwimlaneStylesheet,
) where
    DiagramId: SvgDiagramIdValue,
{
    let id = CssSelectorDiagramId(diagram_id);
    let _ = write!(
        out,
        r#"#{id} [data-look="neo"].cluster rect{{filter:none;}}"#,
    );
    if let SwimlaneStylesheet::Native {
        container,
        guide,
        emphasis,
        on_emphasis,
    } = style
    {
        let _ = write!(
            out,
            "#{id} .swimlane.cluster rect.swimlane-body{{fill:{};stroke:{}}}#{id} .swimlane.cluster rect.swimlane-title{{fill:{};stroke:{}}}#{id} .swimlane-label text{{fill:{}}}#{id} .swimlane-label span{{color:{}}}",
            container, guide, emphasis, guide, on_emphasis, on_emphasis,
        );
    } else {
        let _ = write!(
            out,
            "#{id} .swimlane.cluster:not([data-color-id]) rect{{stroke:{cluster_border}!important;}}"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn css_rgba_fade_matches_khroma_color_semantics() {
        assert_eq!(
            css_rgba_fade("#8090a0", 0.5).expect("valid hex color"),
            "rgba(128, 144, 160, 0.5)"
        );
        assert_eq!(
            css_rgba_fade("rebeccapurple", 0.5).expect("valid named color"),
            "rgba(102, 51, 153, 0.5)"
        );
        assert_eq!(
            css_rgba_fade("rgb(20% 40% 60% / 25%)", 0.2).expect("valid modern RGB color"),
            "rgba(51, 102, 153, 0.2)"
        );
        assert_eq!(
            css_rgba_fade("hsl(80, 100%, 96%)", 0.5).expect("valid HSL color"),
            "rgba(248.2, 255, 234.6, 0.5)"
        );
        assert_eq!(
            css_rgba_fade("#8090a080", 0.5).expect("valid alpha hex color"),
            "rgba(128, 144, 160, 0.5)"
        );
    }

    #[test]
    fn css_rgba_fade_rejects_unsupported_css_color() {
        let error = css_rgba_fade("var(--not-runtime-resolved)", 0.5)
            .expect_err("unresolved CSS variables are not khroma colors");

        assert!(error.to_string().contains("var(--not-runtime-resolved)"));
    }

    #[test]
    fn swimlane_css_escapes_the_diagram_id_as_a_css_identifier() {
        let mut css = String::new();
        write_swimlane_css(
            &mut css,
            "seq:prod.v1",
            "#aaaa33",
            SwimlaneStylesheet::Mermaid,
        );

        assert!(css.contains(r#"#seq\:prod\.v1 [data-look="neo"].cluster"#));
        assert!(!css.contains(r#"#seq:prod.v1 [data-look="neo"].cluster"#));
    }
}
