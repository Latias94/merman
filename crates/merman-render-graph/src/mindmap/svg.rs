use super::{
    MindmapAppearance, MindmapCssBinding, MindmapDiagramLayout, MindmapEdgeRouting,
    MindmapNativeStyle, MindmapPreparedArtifact,
};
use base64::Engine as _;
use merman_model::families::mindmap::MindmapDiagramRenderModel;
use merman_model::{OperationPhase, presentation::PresentationConfig};
use merman_render_core::{
    Bounds, Error, LayoutNode, LayoutPoint, RenderCapability, Result, config, html, math,
    number_format::canonical_number as fmt,
    resources::{OperationWorkMeter, ResourceLimitId},
    root_svg,
    svg_bounds::svg_path_bounds_from_d,
    svg_css::{CssSelectorDiagramId, SvgDiagramIdValue, SvgId, scoped_drop_shadow},
    svg_edge_parity::{
        curve,
        path_numbers::{fmt_path, json_stringify_points},
    },
    svg_output::{
        BoundedSvgOutput, SvgOutput, XmlTextOutput,
        timing::{RenderTiming, RenderTimings},
    },
    text::{TextMeasurer, TextStyle, mermaid_markdown_svg_plain_text, svg_label},
    xml::{escape_attr, normalize_html_entities_for_xml},
};
use std::fmt::Write as _;

fn fmt_string(value: f64) -> String {
    fmt(value).to_string()
}

fn fmt_into(out: &mut impl SvgOutput, value: f64) {
    let _ = write!(out, "{}", fmt(value));
}

#[derive(Debug, Clone, Copy)]
enum MindmapPathNumberFormat {
    D3Path,
    JsNumber,
}

fn mindmap_path_number(v: f64, number_format: MindmapPathNumberFormat) -> String {
    match number_format {
        MindmapPathNumberFormat::D3Path => fmt_path(v),
        MindmapPathNumberFormat::JsNumber => fmt_string(v),
    }
}

fn mindmap_cloud_path_d(w: f64, h: f64, number_format: MindmapPathNumberFormat) -> String {
    let r1 = 0.15 * w;
    let r2 = 0.25 * w;
    let r3 = 0.35 * w;
    let r4 = 0.2 * w;
    let n = |v| mindmap_path_number(v, number_format);

    format!(
        "M0 0 a{r1},{r1} 0 0,1 {w25},{wn10} a{r3},{r3} 1 0,1 {w40},{wn10} a{r2},{r2} 1 0,1 {w35},{w20} a{r1},{r1} 1 0,1 {w15},{h35} a{r4},{r4} 1 0,1 {wn15},{h65} a{r2},{r1} 1 0,1 {wn25},{w15} a{r3},{r3} 1 0,1 {wn50},0 a{r1},{r1} 1 0,1 {wn25},{wn15} a{r1},{r1} 1 0,1 {wn10},{hn35} a{r4},{r4} 1 0,1 {w10},{hn65} H0 V0 Z",
        r1 = n(r1),
        r2 = n(r2),
        r3 = n(r3),
        r4 = n(r4),
        w25 = n(w * 0.25),
        w40 = n(w * 0.4),
        w35 = n(w * 0.35),
        w20 = n(w * 0.2),
        w15 = n(w * 0.15),
        w10 = n(w * 0.1),
        wn10 = n(-w * 0.1),
        wn15 = n(-w * 0.15),
        wn25 = n(-w * 0.25),
        wn50 = n(-w * 0.5),
        h35 = n(h * 0.35),
        h65 = n(h * 0.65),
        hn35 = n(-h * 0.35),
        hn65 = n(-h * 0.65),
    )
}

pub(crate) fn mindmap_cloud_rendered_bbox_size_px(w: f64, h: f64) -> Option<(f64, f64)> {
    let d = mindmap_cloud_path_d(w, h, MindmapPathNumberFormat::JsNumber);
    let pb = svg_path_bounds_from_d(&d)?;
    Some((pb.max_x - pb.min_x, pb.max_y - pb.min_y))
}

fn mindmap_bang_path_d(
    w_base: f64,
    effective_w: f64,
    effective_h: f64,
    number_format: MindmapPathNumberFormat,
) -> String {
    let r = 0.15 * w_base;
    let n = |v| mindmap_path_number(v, number_format);

    format!(
        "M0 0 a{r},{r} 1 0,0 {w25},{hn10} a{r},{r} 1 0,0 {w25},0 a{r},{r} 1 0,0 {w25},0 a{r},{r} 1 0,0 {w25},{h10} a{r},{r} 1 0,0 {w15},{h33} a{r08},{r08} 1 0,0 0,{h34} a{r},{r} 1 0,0 {wn15},{h33} a{r},{r} 1 0,0 {wn25},{h15} a{r},{r} 1 0,0 {wn25},0 a{r},{r} 1 0,0 {wn25},0 a{r},{r} 1 0,0 {wn25},{hn15} a{r},{r} 1 0,0 {wn10},{hn33} a{r08},{r08} 1 0,0 0,{hn34} a{r},{r} 1 0,0 {w10},{hn33} H0 V0 Z",
        r = n(r),
        r08 = n(r * 0.8),
        w25 = n(effective_w * 0.25),
        w15 = n(effective_w * 0.15),
        w10 = n(effective_w * 0.1),
        wn10 = n(-effective_w * 0.1),
        wn15 = n(-effective_w * 0.15),
        wn25 = n(-effective_w * 0.25),
        h10 = n(effective_h * 0.1),
        hn10 = n(-effective_h * 0.1),
        h15 = n(effective_h * 0.15),
        hn15 = n(-effective_h * 0.15),
        h33 = n(effective_h * 0.33),
        hn33 = n(-effective_h * 0.33),
        h34 = n(effective_h * 0.34),
        hn34 = n(-effective_h * 0.34),
    )
}

fn include_mindmap_rect_bounds(
    bounds: &mut Option<Bounds>,
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
) {
    if let Some(cur) = bounds.as_mut() {
        cur.min_x = cur.min_x.min(min_x);
        cur.min_y = cur.min_y.min(min_y);
        cur.max_x = cur.max_x.max(max_x);
        cur.max_y = cur.max_y.max(max_y);
    } else {
        *bounds = Some(Bounds {
            min_x,
            min_y,
            max_x,
            max_y,
        });
    }
}

fn include_mindmap_node_rect_bounds(bounds: &mut Option<Bounds>, n: &LayoutNode) {
    include_mindmap_rect_bounds(
        bounds,
        n.x - n.width / 2.0,
        n.y - n.height / 2.0,
        n.x + n.width / 2.0,
        n.y + n.height / 2.0,
    );
}

fn include_mindmap_path_bounds(
    bounds: &mut Option<Bounds>,
    d: &str,
    translate_x: f64,
    translate_y: f64,
) -> bool {
    let Some(pb) = svg_path_bounds_from_d(d) else {
        return false;
    };
    include_mindmap_rect_bounds(
        bounds,
        pb.min_x + translate_x,
        pb.min_y + translate_y,
        pb.max_x + translate_x,
        pb.max_y + translate_y,
    );
    true
}

fn mindmap_viewport_bounds_from_layout(
    layout: &MindmapDiagramLayout,
    model: &MindmapDiagramRenderModel,
) -> Option<Bounds> {
    let mut layout_nodes: std::collections::BTreeMap<&str, &LayoutNode> =
        std::collections::BTreeMap::new();
    for n in &layout.nodes {
        layout_nodes.insert(n.id.as_str(), n);
    }

    let mut bounds: Option<Bounds> = None;
    for n in &model.nodes {
        let Some(ln) = layout_nodes.get(n.id.as_str()) else {
            continue;
        };

        let padding = n.padding.max(0.0);
        let half_padding = padding / 2.0;
        match n.shape.as_str() {
            "cloud" => {
                let bbox_w = ln
                    .label_width
                    .unwrap_or_else(|| (ln.width - 2.0 * half_padding).max(1.0));
                let bbox_h = ln
                    .label_height
                    .unwrap_or_else(|| (ln.height - 2.0 * half_padding).max(1.0));
                let w = (bbox_w + 2.0 * half_padding).max(1.0);
                let h = (bbox_h + 2.0 * half_padding).max(1.0);
                let d = mindmap_cloud_path_d(w, h, MindmapPathNumberFormat::JsNumber);
                if !include_mindmap_path_bounds(&mut bounds, &d, ln.x - w / 2.0, ln.y - h / 2.0) {
                    include_mindmap_node_rect_bounds(&mut bounds, ln);
                }
                include_mindmap_rect_bounds(
                    &mut bounds,
                    ln.x - bbox_w / 2.0,
                    ln.y - bbox_h / 2.0,
                    ln.x + bbox_w / 2.0,
                    ln.y + bbox_h / 2.0,
                );
            }
            "bang" => {
                let w = ln.width.max(1.0);
                let h = ln.height.max(1.0);
                let bbox_w = ln
                    .label_width
                    .unwrap_or_else(|| (w - 10.0 * half_padding).max(1.0));
                let bbox_h = ln
                    .label_height
                    .unwrap_or_else(|| (h - 8.0 * half_padding).max(1.0));
                let w_base = bbox_w + 10.0 * half_padding;
                let d = mindmap_bang_path_d(w_base, w, h, MindmapPathNumberFormat::JsNumber);
                if !include_mindmap_path_bounds(&mut bounds, &d, ln.x - w / 2.0, ln.y - h / 2.0) {
                    include_mindmap_node_rect_bounds(&mut bounds, ln);
                }
                include_mindmap_rect_bounds(
                    &mut bounds,
                    ln.x - bbox_w / 2.0,
                    ln.y - bbox_h / 2.0,
                    ln.x + bbox_w / 2.0,
                    ln.y + bbox_h / 2.0,
                );
            }
            _ => include_mindmap_node_rect_bounds(&mut bounds, ln),
        }
    }

    for lane in &layout.swimlane_lanes {
        include_mindmap_rect_bounds(
            &mut bounds,
            lane.x - lane.width / 2.0,
            lane.y - lane.height / 2.0,
            lane.x + lane.width / 2.0,
            lane.y + lane.height / 2.0,
        );
    }
    for e in &layout.edges {
        for p in &e.points {
            include_mindmap_rect_bounds(&mut bounds, p.x, p.y, p.x, p.y);
        }
    }

    bounds
}

fn mindmap_data_look_attr(look: &str) -> String {
    if look.is_empty() {
        String::new()
    } else {
        format!(r#" data-look="{}""#, escape_attr(look))
    }
}

fn mindmap_dom_id(diagram_id: SvgId<'_>, raw_id: &str) -> String {
    format!("{diagram_id}-{raw_id}")
}

fn mindmap_gradient_defs(diagram_id: &str, binding: &MindmapCssBinding) -> String {
    let Some((gradient_start, gradient_stop)) = &binding.gradient else {
        return String::new();
    };

    format!(
        r#"<defs><linearGradient id="{}-gradient" gradientUnits="objectBoundingBox" x1="0%" y1="0%" x2="100%" y2="0%"><stop offset="0%" stop-color="{}" stop-opacity="1"/><stop offset="100%" stop-color="{}" stop-opacity="1"/></linearGradient></defs>"#,
        diagram_id,
        escape_attr(gradient_start),
        escape_attr(gradient_stop)
    )
}

fn push_mindmap_shadow_defs(
    out: &mut impl SvgOutput,
    diagram_id: &str,
    flood_color: &str,
) -> Result<()> {
    let _ = write!(
        out,
        r#"<defs><filter id="{}-drop-shadow" height="130%" width="130%"><feDropShadow dx="4" dy="4" stdDeviation="0" flood-opacity="0.06" flood-color="{}"/></filter></defs><defs><filter id="{}-drop-shadow-small" height="150%" width="150%"><feDropShadow dx="2" dy="2" stdDeviation="0" flood-opacity="0.06" flood-color="{}"/></filter></defs>"#,
        diagram_id, flood_color, diagram_id, flood_color
    );
    out.checkpoint()
}

fn mindmap_css(
    out: &mut impl SvgOutput,
    diagram_id: impl SvgDiagramIdValue,
    binding: &MindmapCssBinding,
) -> Result<()> {
    // Mirrors pinned Mermaid's Mindmap and shared stylesheets, with :root last.
    let theme_color_limit = binding.sections.len();
    let use_gradient = binding.use_gradient;
    let id = CssSelectorDiagramId(diagram_id);
    let fragment_id = diagram_id;
    binding.common.write_prefix(out, diagram_id)?;

    let _ = write!(out, r#"#{} .edge{{stroke-width:3;}}"#, diagram_id);

    // Mermaid default theme resolves `cScale0..11` into this palette for mindmap/kanban/timeline.
    // The first generated section is `section--1` (i=0).
    let theme = binding.theme.as_ref();
    let root_fill = &binding.root_fill;
    let root_label = &binding.root_label;
    let node_border = binding.node_border.as_str();
    let main_bkg = binding.main_background.as_str();
    let stroke_width = &binding.stroke_width;
    let scoped_drop_shadow = scoped_drop_shadow(fragment_id, &binding.drop_shadow);
    for i in 0..theme_color_limit {
        let section = i as i64 - 1;
        let slot = &binding.sections[i];
        let c_scale = &slot.fill;
        let c_scale_inv = &slot.inverse;
        let c_scale_label = &slot.label;
        let sw = if binding.look.as_ref() == "neo" {
            (10_i64 - (section * 2)).max(2)
        } else {
            17_i64 - 3_i64 * (i as i64)
        };
        let neo_node_fill = if theme == "redux" || theme == "redux-dark" || theme == "neutral" {
            main_bkg
        } else {
            c_scale.as_str()
        };
        let neo_node_stroke = if theme == "redux" || theme == "redux-dark" {
            node_border
        } else {
            c_scale.as_str()
        };
        let neo_edge_stroke = if theme.contains("redux") || theme == "neo-dark" {
            node_border
        } else {
            c_scale.as_str()
        };
        let neo_text_label = if theme == "redux" || theme == "redux-dark" {
            node_border
        } else if theme == "neutral" {
            binding.neutral_label.as_str()
        } else {
            c_scale_label.as_str()
        };
        let _ = write!(
            out,
            r#"#{} .section-{} rect,#{} .section-{} path,#{} .section-{} circle,#{} .section-{} polygon,#{} .section-{} path{{fill:{};}}"#,
            diagram_id,
            section,
            diagram_id,
            section,
            diagram_id,
            section,
            diagram_id,
            section,
            diagram_id,
            section,
            c_scale
        );
        let _ = write!(
            out,
            r#"#{} .section-{} text{{fill:{};}}"#,
            diagram_id, section, c_scale_label
        );
        let _ = write!(
            out,
            r#"#{} .section-{} span{{color:{};}}"#,
            diagram_id, section, c_scale_label
        );
        let _ = write!(
            out,
            r#"#{} .node-icon-{}{{font-size:40px;color:{};}}"#,
            diagram_id, section, c_scale_label
        );
        let _ = write!(
            out,
            r#"#{} .section-edge-{}{{stroke:{};}}"#,
            diagram_id, section, c_scale
        );
        let _ = write!(
            out,
            r#"#{} .edge-depth-{}{{stroke-width:{};}}"#,
            diagram_id, section, sw
        );
        let _ = write!(
            out,
            r#"#{} .section-{} line{{stroke:{};stroke-width:3;}}"#,
            diagram_id, section, c_scale_inv
        );
        let _ = write!(
            out,
            r#"#{} .disabled,#{} .disabled circle,#{} .disabled text{{fill:lightgray;}}#{} .disabled text{{fill:#efefef;}}"#,
            diagram_id, diagram_id, diagram_id, diagram_id
        );
        let _ = write!(
            out,
            r#"#{} [data-look="neo"].mindmap-node.section-{} rect,#{} [data-look="neo"].mindmap-node.section-{} path,#{} [data-look="neo"].mindmap-node.section-{} circle,#{} [data-look="neo"].mindmap-node.section-{} polygon{{fill:{};stroke:{};stroke-width:{}px;}}"#,
            diagram_id,
            section,
            diagram_id,
            section,
            diagram_id,
            section,
            diagram_id,
            section,
            neo_node_fill,
            neo_node_stroke,
            stroke_width
        );
        let _ = write!(
            out,
            r#"#{} [data-look="neo"].section-edge-{}{{stroke:{};}}"#,
            diagram_id, section, neo_edge_stroke
        );
        let _ = write!(
            out,
            r#"#{} [data-look="neo"].mindmap-node.section-{} text,#{} [data-look="neo"].mindmap-node.section-{} span{{fill:{};color:{};}}"#,
            diagram_id, section, diagram_id, section, neo_text_label, neo_text_label
        );
        out.checkpoint()?;
    }

    // Root section overrides.
    let root_span = if theme.contains("redux") {
        node_border
    } else {
        root_label.as_str()
    };
    let _ = write!(
        out,
        r#"#{} .section-root rect,#{} .section-root path,#{} .section-root circle,#{} .section-root polygon{{fill:{};}}"#,
        diagram_id, diagram_id, diagram_id, diagram_id, root_fill
    );
    let _ = write!(
        out,
        r#"#{} .section-root text{{fill:{};}}"#,
        diagram_id, root_label
    );
    let _ = write!(
        out,
        r#"#{} .section-root span{{color:{};}}"#,
        diagram_id, root_span
    );
    let _ = write!(
        out,
        r#"#{} .icon-container{{height:100%;display:flex;justify-content:center;align-items:center;}}"#,
        diagram_id
    );
    let _ = write!(out, r#"#{} .edge{{fill:none;}}"#, diagram_id);
    let _ = write!(
        out,
        r#"#{} .mindmap-node-label{{dy:1em;alignment-baseline:middle;text-anchor:middle;dominant-baseline:middle;text-align:center;}}"#,
        diagram_id
    );
    let _ = write!(
        out,
        r#"#{} [data-look="neo"].mindmap-node{{filter:{scoped_drop_shadow};}}"#,
        diagram_id
    );
    let neo_root_fill = if theme.contains("redux") {
        main_bkg
    } else {
        root_fill.as_str()
    };
    let neo_root_text = if theme.contains("redux") {
        node_border
    } else if theme == "neutral" {
        binding.neutral_label.as_str()
    } else {
        binding.sections[0].label.as_str()
    };
    let _ = write!(
        out,
        r#"#{} [data-look="neo"].mindmap-node.section-root rect,#{} [data-look="neo"].mindmap-node.section-root path,#{} [data-look="neo"].mindmap-node.section-root circle,#{} [data-look="neo"].mindmap-node.section-root polygon{{fill:{};}}"#,
        diagram_id, diagram_id, diagram_id, diagram_id, neo_root_fill
    );
    let _ = write!(
        out,
        r#"#{} [data-look="neo"].mindmap-node.section-root .text-inner-tspan,#{} [data-look="neo"].mindmap-node.section-root span{{fill:{};color:{};}}"#,
        diagram_id, diagram_id, neo_root_text, neo_root_text
    );
    if use_gradient {
        for i in 0..theme_color_limit {
            let section = i as i64 - 1;
            let _ = write!(
                out,
                r#"#{} [data-look="neo"].mindmap-node.section-{} rect,#{} [data-look="neo"].mindmap-node.section-{} path,#{} [data-look="neo"].mindmap-node.section-{} circle,#{} [data-look="neo"].mindmap-node.section-{} polygon{{stroke:url(#{}-gradient);fill:{};}}"#,
                id, section, id, section, id, section, id, section, fragment_id, main_bkg
            );
            let _ = write!(
                out,
                r#"#{} .section-{} line{{stroke-width:0;}}"#,
                diagram_id, section
            );
            out.checkpoint()?;
        }
    }

    if let Some(background) = binding.label_background.as_deref() {
        let _ = write!(
            out,
            "#{diagram_id} .labelBkg{{background-color:{background};}}"
        );
    }
    if let Some(text) = binding.label_text.as_deref() {
        let _ = write!(out, "#{diagram_id} .labelBkg span{{color:{text};}}");
    }
    out.checkpoint()?;

    binding.common.write_root(out, diagram_id, fragment_id)?;
    out.checkpoint()
}

fn write_mindmap_edge_label(
    out: &mut impl SvgOutput,
    edge_id: &str,
    config: &PresentationConfig,
) -> Result<()> {
    if !config::config_effective_html_labels(config.as_value()) {
        let _ = write!(
            out,
            r#"<g class="edgeLabel"><g class="label" data-id="{}" transform="translate(0, 0)">"#,
            escape_attr(edge_id),
        );
        out.checkpoint()?;
        svg_label::write_svg_text_centered_from_create_text_source(out, "", false);
        out.checkpoint()?;
        // createText's empty background group remains a sibling of the moved text label.
        out.push_str(r#"</g></g><g><rect class="background" style="stroke: none"/></g>"#);
        return out.checkpoint();
    }
    let _ = write!(
        out,
        r#"<g class="edgeLabel"><g class="label" data-id="{id}" transform="translate(0, 0)"><foreignObject width="0" height="0"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="edgeLabel"></span></div></foreignObject></g></g>"#,
        id = escape_attr(edge_id),
    );
    out.checkpoint()
}

fn write_mindmap_native_css(
    out: &mut impl SvgOutput,
    diagram_id: SvgId<'_>,
    theme: &MindmapNativeStyle,
) {
    let id = CssSelectorDiagramId(diagram_id);
    let _ = write!(
        out,
        r#"#{id}{{font-family:{family};font-size:{size}px;font-weight:{weight};fill:{primary};color:{primary}}}
#{id} p{{margin:0}}
#{id} .mindmap-node>.node-bkg,#{id} .mindmap-node>.label-container{{fill:{node};stroke:{border};stroke-width:{width}}}
#{id} .mindmap-node>.node-line-{{stroke:{border};stroke-width:{width}}}
#{id} .mindmap-node .label text,#{id} .mindmap-node .label span{{fill:inherit;color:inherit}}
#{id} .mindmap-node .label .background{{fill:none}}"#,
        family = theme.label.font_family.as_deref().unwrap_or("sans-serif"),
        size = fmt(theme.label.font_size),
        weight = theme.label.font_weight.as_deref().unwrap_or("400"),
        primary = theme.primary,
        node = theme.node,
        border = theme.border,
        width = fmt(theme.stroke_width),
    );
    let _ = write!(
        out,
        r#"#{id} [data-merman-mindmap-root]>.node-bkg,#{id} [data-merman-mindmap-root]>.label-container{{fill:{}}}"#,
        theme.emphasis,
    );
    for (slot, fill) in theme.data.fill.iter().enumerate() {
        let _ = write!(
            out,
            r#"#{id} [data-merman-mindmap-branch="{slot}"]>.node-bkg,#{id} [data-merman-mindmap-branch="{slot}"]>.label-container{{fill:{fill}}}"#,
        );
    }
}

fn clip_circle_endpoints(
    mut points: Vec<LayoutPoint>,
    start: &LayoutNode,
    end: &LayoutNode,
    start_circle: bool,
    end_circle: bool,
) -> Vec<LayoutPoint> {
    if points.len() < 2 {
        return points;
    }
    let intersect = |node: &LayoutNode, point: &LayoutPoint| {
        let dx = point.x - node.x;
        let dy = point.y - node.y;
        let length = dx.hypot(dy);
        if length == 0.0 {
            return point.clone();
        }
        let radius = node.width / 2.0;
        LayoutPoint {
            x: node.x + radius * dx / length,
            y: node.y + radius * dy / length,
        }
    };
    if start_circle {
        points[0] = intersect(start, &points[1]);
    }
    if end_circle {
        let last = points.len() - 1;
        points[last] = intersect(end, &points[last - 1]);
    }
    points
}

#[cfg(feature = "layout-elk")]
fn sanitize_elk_endpoints(
    points: &[LayoutPoint],
    start: &LayoutNode,
    end: &LayoutNode,
    start_is_circle: bool,
    end_is_circle: bool,
) -> Vec<LayoutPoint> {
    use merman_render_core::elk_edge_geometry::{Outline, Shape, sanitize};
    sanitize(
        points,
        Shape {
            intersection: None,
            node: start,
            outline: if start_is_circle {
                Outline::Ellipse
            } else {
                Outline::Rect
            },
        },
        Shape {
            intersection: None,
            node: end,
            outline: if end_is_circle {
                Outline::Ellipse
            } else {
                Outline::Rect
            },
        },
    )
}

pub struct MindmapSvgContext<'a> {
    pub diagram_id: SvgId<'a>,
    pub viewport: root_svg::RootViewportContext<'a>,
    pub text_measurer: &'a dyn TextMeasurer,
    pub math_renderer: Option<&'a (dyn math::MathRenderer + Send + Sync)>,
    pub work_meter: &'a OperationWorkMeter,
    pub timing: RenderTiming,
}

pub fn render_mindmap_diagram_svg(
    model: &MindmapDiagramRenderModel,
    config: &PresentationConfig,
    prepared: &MindmapPreparedArtifact,
    context: MindmapSvgContext<'_>,
) -> Result<root_svg::RootedSvg> {
    let timing = context.timing;
    let mut timings = RenderTimings::default();
    let total_timer = timing.start();

    let MindmapPreparedArtifact {
        layout,
        appearance,
        prepared_labels,
        routing,
    } = prepared;
    let (binding, native) = match appearance {
        MindmapAppearance::Mermaid(binding) => (Some(binding.as_ref()), None),
        MindmapAppearance::Native(native) => (None, Some(native)),
    };
    let text_style = appearance.text_style();

    struct MindmapLabelSpec<'a> {
        text: &'a str,
        prepared_xhtml: Option<&'a str>,
        label_bkg: bool,
        width: f64,
        height: f64,
        tx: f64,
        ty: f64,
        max_node_width_px: f64,
    }

    fn mk_label(
        out: &mut impl SvgOutput,
        spec: MindmapLabelSpec<'_>,
        config: &PresentationConfig,
        context: &MindmapSvgContext<'_>,
        text_style: &TextStyle,
    ) -> Result<()> {
        let MindmapLabelSpec {
            text,
            prepared_xhtml,
            label_bkg,
            width,
            height,
            tx,
            ty,
            max_node_width_px,
        } = spec;

        if !config::config_effective_html_labels(config.as_value()) {
            let source = super::mindmap_svg_label_source(text);
            let _ = write!(
                out,
                r#"<g class="label" style="" transform="translate({}, {})"><rect/><g>"#,
                fmt(tx),
                fmt(ty)
            );
            out.checkpoint()?;
            if label_bkg {
                let plain = mermaid_markdown_svg_plain_text(source.as_ref());
                let bbox_y = context
                    .text_measurer
                    .measure_svg_create_text_bbox_y_offset_px(&plain, text_style);
                let _ = write!(
                    out,
                    r#"<rect class="background" style="stroke: none" x="-2" y="{}" width="{}" height="{}"/>"#,
                    fmt(bbox_y - 2.0),
                    fmt(width + 4.0),
                    fmt(height + 4.0)
                );
            } else {
                out.push_str(r#"<rect class="background" style="stroke: none"/>"#);
            }
            out.checkpoint()?;
            // Mindmap has no inherited text-anchor rule. Mermaid 12.2's centerLabel offsets
            // the left-aligned createText result by half its width for every node shape.
            svg_label::write_svg_text_markdown_wrapped_from_create_text_source(
                out,
                source.as_ref(),
                true,
                context.text_measurer,
                text_style,
                Some(max_node_width_px),
            );
            out.checkpoint()?;
            out.push_str("</g></g>");
            return out.checkpoint();
        }

        let div_class = if label_bkg {
            r#" class="labelBkg""#
        } else {
            ""
        };

        let max_node_width_px = if max_node_width_px.is_finite() && max_node_width_px > 0.0 {
            max_node_width_px
        } else {
            200.0
        };

        // Mermaid flips the `<div>` to a fixed-width wrapping container when the measured label
        // reaches/exceeds the configured max width (default 200px), even if the emitted
        // `<foreignObject width="...">` reflects the overflow width.
        let wrap_container = width >= max_node_width_px - 1e-3;
        out.push_str(r#"<g class="label" style="" transform="translate("#);
        fmt_into(out, tx);
        out.push_str(", ");
        fmt_into(out, ty);
        out.push_str(r#")"><rect/><foreignObject width=""#);
        fmt_into(out, width.max(1.0));
        out.push_str(r#"" height=""#);
        fmt_into(out, height.max(1.0));
        out.push_str(r#""><div xmlns="http://www.w3.org/1999/xhtml""#);
        out.push_str(div_class);
        out.push_str(r#" style=""#);
        if wrap_container {
            out.push_str(
                "display: table; white-space: break-spaces; line-height: 1.5; max-width: ",
            );
            fmt_into(out, max_node_width_px);
            out.push_str("px; text-align: center; width: ");
            fmt_into(out, max_node_width_px);
            out.push_str("px;");
        } else {
            out.push_str("display: table-cell; white-space: nowrap; line-height: 1.5; max-width: ");
            fmt_into(out, max_node_width_px);
            out.push_str("px; text-align: center;");
        }
        let has_math = math::contains_delimited_math(text);
        out.push_str(r#""><span class="nodeLabel markdown-node-label""#);
        if has_math {
            out.push(' ');
            out.push_str(math::BROWSER_ONLY_MATH_NATIVE_UNAVAILABLE_ATTRIBUTE);
        }
        out.push('>');
        out.checkpoint()?;

        let parse_work = std::cell::Cell::new(0);
        let work_meter = context.work_meter;
        let sink = html::HtmlSanitizeSink::new(
            work_meter
                .policy()
                .value(ResourceLimitId::MaxSvgBytes)
                .unwrap_or(usize::MAX),
            work_meter,
            &parse_work,
            merman_model::OperationPhase::Emit,
        );

        fn escape_amp_preserving_entities(raw: &str) -> String {
            normalize_html_entities_for_xml(raw).into_owned()
        }

        if has_math {
            let html = context
                .math_renderer
                .and_then(|renderer| renderer.render_html_label(text, config))
                .ok_or_else(|| Error::MissingCapability {
                    capability: RenderCapability::Math,
                    diagram_type: "mindmap".to_string(),
                })?;
            let html = merman_render_core::html::resolve_svg_fragment(
                merman_markup::sanitize::sanitize_xhtml_with_sink(&html, config.as_config(), &sink),
                work_meter,
            )?;
            out.push_str(&escape_amp_preserving_entities(&html));
        } else {
            let html = prepared_xhtml.ok_or_else(|| Error::InvalidModel {
                message: "Mindmap HTML label is missing its prepared fragment".to_string(),
            })?;
            out.push_str(html);
        }

        out.push_str("</span></div></foreignObject></g>");
        out.checkpoint()
    }

    let _g_build_ctx = timing.section(&mut timings.build_ctx);

    let diagram_id = context.diagram_id;

    let mut node_by_id: std::collections::BTreeMap<String, &LayoutNode> =
        std::collections::BTreeMap::new();
    for n in &layout.nodes {
        node_by_id.insert(n.id.clone(), n);
    }

    let edge_by_id: std::collections::BTreeMap<_, _> = layout
        .edges
        .iter()
        .map(|edge| (edge.id.as_str(), edge))
        .collect();
    let source_node_by_id: std::collections::BTreeMap<_, _> = model
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    let backend = *routing;

    drop(_g_build_ctx);

    let _g_viewbox = timing.section(&mut timings.viewbox);

    let padding = config::config_f64(config.as_value(), &["mindmap", "padding"]).unwrap_or(10.0);
    let use_max_width =
        config::config_bool(config.as_value(), &["mindmap", "useMaxWidth"]).unwrap_or(true);
    let viewport_bounds =
        mindmap_viewport_bounds_from_layout(layout, model).or_else(|| layout.bounds.clone());
    let (vx, vy, vw, vh) = viewport_bounds
        .as_ref()
        .map(|b| {
            let w = (b.max_x - b.min_x).max(0.0);
            let h = (b.max_y - b.min_y).max(0.0);
            (
                b.min_x - padding,
                b.min_y - padding,
                w + 2.0 * padding,
                h + 2.0 * padding,
            )
        })
        .unwrap_or((0.0, 0.0, 100.0, 100.0));

    let root_spec = root_svg::RootViewportSpec::mermaid(
        root_svg::DiagramBounds::from_view_box(vx, vy, vw, vh),
        use_max_width,
    )
    .with_max_width(root_svg::RootMaxWidth::CssSixSignificant(vw));

    drop(_g_viewbox);

    let _g_render_svg = timing.section(&mut timings.render_svg);

    let mut out = BoundedSvgOutput::new(context.work_meter);
    let root_document = context.viewport.write_open(
        &mut out,
        root_spec,
        root_svg::RootChrome {
            class: Some("mindmapDiagram"),
            dom: root_svg::RootDomProfile {
                trailing_newline: false,
                ..Default::default()
            },
            ..root_svg::RootChrome::new(diagram_id, "mindmap")
        },
    )?;
    out.push_str("<style>");
    out.checkpoint()?;
    {
        let mut css = XmlTextOutput::new(&mut out);
        match appearance {
            MindmapAppearance::Mermaid(binding) => mindmap_css(&mut css, diagram_id, binding)?,
            MindmapAppearance::Native(theme) => {
                write_mindmap_native_css(&mut css, diagram_id, theme)
            }
        }
    }
    out.push_str("</style>");
    out.checkpoint()?;
    if let Some(binding) = binding {
        out.push_str(&mindmap_gradient_defs(diagram_id.semantic_str(), binding));
    }
    out.push_str("<g>");
    out.checkpoint()?;

    let _ = write!(
        &mut out,
        r#"<marker id="{id}_mindmap-pointEnd" class="marker mindmap" viewBox="0 0 10 10" refX="5" refY="5" markerUnits="userSpaceOnUse" markerWidth="8" markerHeight="8" orient="auto"><path d="M 0 0 L 10 5 L 0 10 z" class="arrowMarkerPath" style="stroke-width: 1; stroke-dasharray: 1, 0;"/></marker>"#,
        id = diagram_id
    );
    let _ = write!(
        &mut out,
        r#"<marker id="{id}_mindmap-pointStart" class="marker mindmap" viewBox="0 0 10 10" refX="4.5" refY="5" markerUnits="userSpaceOnUse" markerWidth="8" markerHeight="8" orient="auto"><path d="M 0 5 L 10 10 L 10 0 z" class="arrowMarkerPath" style="stroke-width: 1; stroke-dasharray: 1, 0;"/></marker>"#,
        id = diagram_id
    );
    let _ = write!(
        &mut out,
        r#"<marker id="{id}_mindmap-pointEnd-margin" class="marker mindmap" viewBox="0 0 11.5 14" refX="11.5" refY="7" markerUnits="userSpaceOnUse" markerWidth="10.5" markerHeight="14" orient="auto"><path d="M 0 0 L 11.5 7 L 0 14 z" class="arrowMarkerPath" style="stroke-width: 0; stroke-dasharray: 1, 0;"/></marker>"#,
        id = diagram_id
    );
    let _ = write!(
        &mut out,
        r#"<marker id="{id}_mindmap-pointStart-margin" class="marker mindmap" viewBox="0 0 11.5 14" refX="1" refY="7" markerUnits="userSpaceOnUse" markerWidth="11.5" markerHeight="14" orient="auto"><polygon points="0,7 11.5,14 11.5,0" class="arrowMarkerPath" style="stroke-width: 0; stroke-dasharray: 1, 0;"/></marker>"#,
        id = diagram_id
    );
    out.checkpoint()?;

    if layout.swimlane_lanes.is_empty() {
        out.push_str(r#"<g class="subgraphs"/>"#);
    } else {
        out.push_str(r#"<g class="subgraphs">"#);
        for lane in &layout.swimlane_lanes {
            let x = lane.x - lane.width / 2.0;
            let y = lane.y - lane.height / 2.0;
            let native_fill = native.map(|theme| theme.node.to_string());
            let native_stroke = native.map(|theme| theme.border.to_string());
            let _ = write!(
                out,
                r#"<g class="cluster swimlane" id="{id}" data-id="{id}" data-et="cluster" data-look="{look}"><rect class="swimlane-body" style="" x="{x}" y="{y}" width="{width}" height="{height}" fill="none" stroke="{stroke}"/><rect class="swimlane-title" style="" x="{x}" y="{y}" width="{width}" height="0" fill="{fill}" stroke="{stroke}"/><g class="cluster-label swimlane-label" transform="translate({center}, {y})"><foreignObject width="0" height="0"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {width}px; text-align: center;"><span class="nodeLabel"></span></div></foreignObject></g></g>"#,
                id = escape_attr(&lane.id),
                look = escape_attr(binding.map_or("classic", |style| style.look.as_ref())),
                x = fmt(x),
                y = fmt(y),
                width = fmt(lane.width),
                height = fmt(lane.height),
                center = fmt(lane.x),
                fill = escape_attr(native_fill.as_deref().unwrap_or_else(|| {
                    binding.map_or("none", |style| style.cluster_background.as_ref())
                })),
                stroke = escape_attr(native_stroke.as_deref().unwrap_or_else(|| {
                    binding.map_or("none", |style| style.cluster_border.as_ref())
                }))
            );
        }
        out.push_str("</g>");
    }

    out.push_str(r#"<g class="edgePaths">"#);
    out.checkpoint()?;
    for e in &model.edges {
        let start = node_by_id.get(&e.start).copied();
        let end = node_by_id.get(&e.end).copied();
        let projected_points = || {
            edge_by_id
                .get(e.id.as_str())
                .map(|edge| edge.points.clone())
                .unwrap_or_default()
        };
        let is_circle = |id: &str| {
            source_node_by_id
                .get(id)
                .is_some_and(|node| node.shape == "mindmapCircle")
        };
        let points_for_data_points = match (backend, start, end) {
            #[cfg(feature = "layout-cytoscape")]
            (MindmapEdgeRouting::CoseBilkent, ..) => {
                // Mermaid offsets the endpoints of CoSE edges along the node-to-node ray.
                let (sx, sy, tx, ty) = match (start, end) {
                    (Some(a), Some(b)) => (a.x, a.y, b.x, b.y),
                    _ => (0.0, 0.0, 0.0, 0.0),
                };
                let (vx, vy) = (tx - sx, ty - sy);
                let v_len = (vx * vx + vy * vy).sqrt();
                let (ux, uy) = if v_len == 0.0 {
                    (0.0, 0.0)
                } else {
                    (vx / v_len, vy / v_len)
                };
                let endpoint_offset = 15.0;
                let start_x = sx + endpoint_offset * ux;
                let start_y = sy + endpoint_offset * uy;
                let end_x = tx - endpoint_offset * ux;
                let end_y = ty - endpoint_offset * uy;
                let mid_x = (start_x + end_x) / 2.0;
                let mid_y = (start_y + end_y) / 2.0;
                vec![
                    LayoutPoint {
                        x: start_x,
                        y: start_y,
                    },
                    LayoutPoint { x: mid_x, y: mid_y },
                    LayoutPoint { x: end_x, y: end_y },
                ]
            }
            (backend, Some(start), Some(end)) if backend.clips_circle_endpoints() => {
                clip_circle_endpoints(
                    projected_points(),
                    start,
                    end,
                    is_circle(&e.start),
                    is_circle(&e.end),
                )
            }
            #[cfg(feature = "layout-elk")]
            (MindmapEdgeRouting::Elk, Some(start), Some(end)) => {
                let points = projected_points();
                let mut elk_points = Vec::with_capacity(points.len() + 2);
                elk_points.push(LayoutPoint {
                    x: start.x,
                    y: start.y,
                });
                elk_points.extend(points);
                elk_points.push(LayoutPoint { x: end.x, y: end.y });
                sanitize_elk_endpoints(
                    &elk_points,
                    start,
                    end,
                    is_circle(&e.start),
                    is_circle(&e.end),
                )
            }
            _ => projected_points(),
        };
        let data_points = base64::engine::general_purpose::STANDARD
            .encode(json_stringify_points(&points_for_data_points));

        let d = if matches!(backend, MindmapEdgeRouting::Swimlane) {
            curve::curve_rounded_path_d(&points_for_data_points, 10.0)
        } else if e.curve.trim() == "basis" {
            curve::curve_basis_path_d(&points_for_data_points)
        } else {
            curve::curve_linear_path_d(&points_for_data_points)
        };
        let class = format!(
            "edge-thickness-{} edge-pattern-solid {}",
            e.thickness.trim(),
            e.classes.trim()
        );
        let native_look = super::mindmap_node_look(&e.look, config.as_value());
        let look = binding.map_or_else(|| native_look.as_str(), |style| style.model_look(&e.look));
        let data_look_attr = mindmap_data_look_attr(look);
        let edge_dom_id = mindmap_dom_id(diagram_id, &e.id);
        let _ = write!(
            &mut out,
            r#"<path d="{d}" id="{dom_id}" class="{class}"{look_attr}"#,
            d = escape_attr(&d),
            dom_id = escape_attr(&edge_dom_id),
            class = escape_attr(&class),
            look_attr = data_look_attr,
        );
        if let Some(theme) = native {
            let section = e.section.or_else(|| {
                source_node_by_id
                    .get(e.end.as_str())
                    .and_then(|node| node.section)
            });
            let stroke = theme
                .branch_slot(section)
                .map_or(theme.connector, |slot| theme.data.ink(slot));
            let _ = write!(
                &mut out,
                r#" fill="none" stroke="{stroke}" stroke-width="{}""#,
                fmt(theme.connector_width)
            );
        }
        let _ = write!(
            &mut out,
            r#" data-edge="true" data-et="edge" data-id="{id}" data-points="{pts}"/>"#,
            id = escape_attr(&e.id),
            pts = escape_attr(&data_points),
        );
        out.checkpoint()?;
    }
    out.push_str("</g>");
    out.checkpoint()?;

    out.push_str(r#"<g class="edgeLabels">"#);
    out.checkpoint()?;
    for e in &model.edges {
        write_mindmap_edge_label(&mut out, &e.id, config)?;
    }
    out.push_str("</g>");
    out.checkpoint()?;

    out.push_str(r#"<g class="nodes">"#);
    out.checkpoint()?;
    for n in &model.nodes {
        let max_node_width_px = super::mindmap_label_wrapping_width_px(n, config.as_value());
        let (x, y, w, h, label_w, label_h) = node_by_id
            .get(&n.id)
            .map(|ln| {
                (
                    ln.x,
                    ln.y,
                    ln.width,
                    ln.height,
                    ln.label_width,
                    ln.label_height,
                )
            })
            .unwrap_or((0.0, 0.0, 80.0, 44.0, None, None));
        let padding = n.padding.max(0.0);
        let half_padding = padding / 2.0;
        // The display policy cycles generated sections; authored class tokens stay literal.
        let node_classes = &n.css_classes;
        let class = format!("node {}", node_classes.trim());
        let native_look = super::mindmap_node_look(&n.look, config.as_value());
        let look = binding.map_or_else(|| native_look.as_str(), |style| style.model_look(&n.look));
        let is_root = node_classes
            .split_whitespace()
            .any(|class| class == "section-root");
        let data_look_attr = mindmap_data_look_attr(look);
        let node_dom_id = mindmap_dom_id(diagram_id, &n.dom_id);
        let _ = write!(
            &mut out,
            r#"<g class="{class}" id="{dom_id}""#,
            class = escape_attr(&class),
            dom_id = escape_attr(&node_dom_id),
        );
        if let Some(theme) = native {
            let ink = if is_root {
                out.push_str(r#" data-merman-mindmap-root="true""#);
                theme.on_emphasis
            } else if let Some(slot) = theme.branch_slot(n.section) {
                let _ = write!(&mut out, r#" data-merman-mindmap-branch="{slot}""#);
                theme.data.fill_ink(slot)
            } else {
                theme.primary
            };
            let _ = write!(&mut out, r#" fill="{ink}" color="{ink}""#);
        }
        let _ = write!(
            &mut out,
            r#"{look_attr} transform="translate({x}, {y})">"#,
            look_attr = data_look_attr,
            x = fmt(x),
            y = fmt(y),
        );
        out.checkpoint()?;

        match n.shape.as_str() {
            "defaultMindmapNode" => {
                let rd = native.map_or(5.0, |theme| theme.radius.min(w / 2.0).min(h / 2.0));
                let mut rect_path = format!(
                    "\n    M{} {}\n    v{}\n    q0,-{} {},-{}\n    h{}\n    q{},0 {},{}\n",
                    fmt_path(-(w / 2.0)),
                    fmt_path(h / 2.0 - rd),
                    fmt_path(-h + 2.0 * rd),
                    fmt_path(rd),
                    fmt_path(rd),
                    fmt_path(rd),
                    fmt_path(w - 2.0 * rd),
                    fmt_path(rd),
                    fmt_path(rd),
                    fmt_path(rd),
                );
                if look == "neo" {
                    let _ = write!(
                        &mut rect_path,
                        "    v{}\n    H{}\n    Z\n  ",
                        fmt_path(h - rd),
                        fmt_path(-(w / 2.0)),
                    );
                } else {
                    let _ = write!(
                        &mut rect_path,
                        "    v{}\n    q0,{} -{},{}\n    h{}\n    q-{},0 -{},-{}\n    Z\n  ",
                        fmt_path(h - 2.0 * rd),
                        fmt_path(rd),
                        fmt_path(rd),
                        fmt_path(rd),
                        fmt_path(-w + 2.0 * rd),
                        fmt_path(rd),
                        fmt_path(rd),
                        fmt_path(rd),
                    );
                }

                // Recover label bbox dimensions from the rendered node size + padding rules.
                let bbox_w = label_w.unwrap_or_else(|| (w - 8.0 * half_padding).max(0.0));
                let bbox_h = label_h.unwrap_or_else(|| (h - 2.0 * half_padding).max(0.0));
                let _ = write!(
                    &mut out,
                    r#"<path id="{id}" class="node-bkg node-0" style="" d="{d}"/>"#,
                    id = escape_attr(&mindmap_dom_id(diagram_id, &n.dom_id)),
                    d = escape_attr(&rect_path),
                );
                context.work_meter.checkpoint(OperationPhase::Emit)?;
                let _ = write!(
                    &mut out,
                    r#"<line class="node-line-" x1="{x1}" y1="{y}" x2="{x2}" y2="{y}"/>"#,
                    x1 = fmt(-(w / 2.0)),
                    x2 = fmt(w / 2.0),
                    y = fmt(h / 2.0),
                );
                mk_label(
                    &mut out,
                    MindmapLabelSpec {
                        text: &n.label,
                        prepared_xhtml: prepared_labels.xhtml_for(&n.id),
                        label_bkg: n.icon.is_some(),
                        width: bbox_w,
                        height: bbox_h,
                        tx: -bbox_w / 2.0,
                        ty: -bbox_h / 2.0,
                        max_node_width_px,
                    },
                    config,
                    &context,
                    text_style,
                )?;
            }
            "rect" => {
                let (padding_w, padding_h) = super::mindmap_rect_label_padding_px(
                    super::mindmap_node_look(&n.look, config.as_value()),
                );
                let bbox_w = label_w.unwrap_or_else(|| (w - padding_w).max(0.0));
                let bbox_h = label_h.unwrap_or_else(|| (h - padding_h).max(0.0));
                let _ = write!(
                    &mut out,
                    r#"<rect class="basic label-container" style="" x="{x}" y="{y}" width="{w}" height="{h}"/>"#,
                    x = fmt(-(w / 2.0)),
                    y = fmt(-(h / 2.0)),
                    w = fmt(w.max(1.0)),
                    h = fmt(h.max(1.0)),
                );
                mk_label(
                    &mut out,
                    MindmapLabelSpec {
                        text: &n.label,
                        prepared_xhtml: prepared_labels.xhtml_for(&n.id),
                        label_bkg: n.icon.is_some(),
                        width: bbox_w,
                        height: bbox_h,
                        tx: -bbox_w / 2.0,
                        ty: -bbox_h / 2.0,
                        max_node_width_px,
                    },
                    config,
                    &context,
                    text_style,
                )?;
            }
            "rounded" => {
                let w = w.max(1.0);
                let h = h.max(1.0);
                let _ = write!(
                    &mut out,
                    r#"<rect class="basic label-container" style="" rx="{radius}" ry="{radius}" x="{x}" y="{y}" width="{w}" height="{h}"/>"#,
                    radius = fmt(native.map_or(5.0, |theme| theme.radius)),
                    x = fmt(-(w / 2.0)),
                    y = fmt(-(h / 2.0)),
                    w = fmt(w),
                    h = fmt(h),
                );

                let bbox_w = label_w.unwrap_or_else(|| (w - 2.0 * padding).max(1.0));
                let bbox_h = label_h.unwrap_or_else(|| (h - 2.0 * padding).max(1.0));
                mk_label(
                    &mut out,
                    MindmapLabelSpec {
                        text: &n.label,
                        prepared_xhtml: prepared_labels.xhtml_for(&n.id),
                        label_bkg: n.icon.is_some(),
                        width: bbox_w,
                        height: bbox_h,
                        tx: -bbox_w / 2.0,
                        ty: -bbox_h / 2.0,
                        max_node_width_px,
                    },
                    config,
                    &context,
                    text_style,
                )?;
            }
            "mindmapCircle" => {
                let r = (w.max(h) / 2.0).max(1.0);
                let _ = write!(
                    &mut out,
                    r#"<circle class="basic label-container" style="" r="{r}" cx="0" cy="0"/>"#,
                    r = fmt(r),
                );
                // Shape bounds enclose the label diagonal, while placement uses its true bbox.
                let bbox_w = label_w.unwrap_or_else(|| (w - 2.0 * padding).max(1.0));
                let bbox_h = label_h.unwrap_or_else(|| (h - 2.0 * padding).max(1.0));
                mk_label(
                    &mut out,
                    MindmapLabelSpec {
                        text: &n.label,
                        prepared_xhtml: prepared_labels.xhtml_for(&n.id),
                        label_bkg: n.icon.is_some(),
                        width: bbox_w,
                        height: bbox_h,
                        tx: -bbox_w / 2.0,
                        ty: -bbox_h / 2.0,
                        max_node_width_px,
                    },
                    config,
                    &context,
                    text_style,
                )?;
            }
            "cloud" => {
                let bbox_w = label_w.unwrap_or_else(|| (w - 2.0 * half_padding).max(1.0));
                let bbox_h = label_h.unwrap_or_else(|| (h - 2.0 * half_padding).max(1.0));
                let w = (bbox_w + 2.0 * half_padding).max(1.0);
                let h = (bbox_h + 2.0 * half_padding).max(1.0);

                let cloud_path = mindmap_cloud_path_d(w, h, MindmapPathNumberFormat::D3Path);

                let _ = write!(
                    &mut out,
                    r#"<path class="basic label-container" style="" d="{d}" transform="translate({tx}, {ty})"/>"#,
                    d = escape_attr(&cloud_path),
                    tx = fmt(-(w / 2.0)),
                    ty = fmt(-(h / 2.0)),
                );
                mk_label(
                    &mut out,
                    MindmapLabelSpec {
                        text: &n.label,
                        prepared_xhtml: prepared_labels.xhtml_for(&n.id),
                        label_bkg: n.icon.is_some(),
                        width: bbox_w,
                        height: bbox_h,
                        tx: -bbox_w / 2.0,
                        ty: -bbox_h / 2.0,
                        max_node_width_px,
                    },
                    config,
                    &context,
                    text_style,
                )?;
            }
            "hexagon" => {
                let w = w.max(1.0);
                let h = h.max(1.0);
                let fixed_length = h / 4.0;
                let points = format!(
                    "{},0 {},0 {},{} {},{} {},{} 0,{}",
                    fmt_string(fixed_length),
                    fmt_string(w - fixed_length),
                    fmt_string(w),
                    fmt_string(-h / 2.0),
                    fmt_string(w - fixed_length),
                    fmt_string(-h),
                    fmt_string(fixed_length),
                    fmt_string(-h),
                    fmt_string(-h / 2.0),
                );
                let _ = write!(
                    &mut out,
                    r#"<polygon points="{points}" class="label-container" transform="translate({tx},{ty})"/>"#,
                    points = escape_attr(&points),
                    tx = fmt(-(w / 2.0)),
                    ty = fmt(h / 2.0),
                );
                let label_width = label_w.unwrap_or_else(|| w.max(1.0));
                let label_height = label_h.unwrap_or_else(|| h.max(1.0));
                mk_label(
                    &mut out,
                    MindmapLabelSpec {
                        text: &n.label,
                        prepared_xhtml: prepared_labels.xhtml_for(&n.id),
                        label_bkg: n.icon.is_some(),
                        width: label_width,
                        height: label_height,
                        tx: -label_width / 2.0,
                        ty: -label_height / 2.0,
                        max_node_width_px,
                    },
                    config,
                    &context,
                    text_style,
                )?;
            }
            "bang" => {
                let bbox_w = label_w.unwrap_or_else(|| (w - 10.0 * half_padding).max(1.0));
                let bbox_h = label_h.unwrap_or_else(|| (h - 8.0 * half_padding).max(1.0));

                let w_base = bbox_w + 10.0 * half_padding;
                let effective_w = w.max(1.0);
                let effective_h = h.max(1.0);

                let bang_path = mindmap_bang_path_d(
                    w_base,
                    effective_w,
                    effective_h,
                    MindmapPathNumberFormat::D3Path,
                );

                let _ = write!(
                    &mut out,
                    r#"<path class="basic label-container" style="" d="{d}" transform="translate({tx}, {ty})"/>"#,
                    d = escape_attr(&bang_path),
                    tx = fmt(-(effective_w / 2.0)),
                    ty = fmt(-(effective_h / 2.0)),
                );
                mk_label(
                    &mut out,
                    MindmapLabelSpec {
                        text: &n.label,
                        prepared_xhtml: prepared_labels.xhtml_for(&n.id),
                        label_bkg: n.icon.is_some(),
                        width: bbox_w,
                        height: bbox_h,
                        tx: -bbox_w / 2.0,
                        ty: -bbox_h / 2.0,
                        max_node_width_px,
                    },
                    config,
                    &context,
                    text_style,
                )?;
            }
            _ => {
                let _ = write!(
                    &mut out,
                    r#"<rect class="basic label-container" style="" x="{x}" y="{y}" width="{w}" height="{h}"/>"#,
                    x = fmt(-(w / 2.0)),
                    y = fmt(-(h / 2.0)),
                    w = fmt(w.max(1.0)),
                    h = fmt(h.max(1.0)),
                );
                mk_label(
                    &mut out,
                    MindmapLabelSpec {
                        text: &n.label,
                        prepared_xhtml: prepared_labels.xhtml_for(&n.id),
                        label_bkg: n.icon.is_some(),
                        width: w.max(1.0),
                        height: h.max(1.0),
                        tx: -w / 2.0,
                        ty: -h / 2.0,
                        max_node_width_px,
                    },
                    config,
                    &context,
                    text_style,
                )?;
            }
        }

        out.push_str("</g>");
        out.checkpoint()?;
    }
    out.push_str("</g>");

    out.push_str("</g>");
    if let Some(binding) = binding {
        push_mindmap_shadow_defs(&mut out, diagram_id.semantic_str(), binding.shadow_flood)?;
    }
    out.push_str("</svg>\n");
    out.checkpoint()?;

    drop(_g_render_svg);

    timings.total = total_timer
        .map(merman_model::runtime::OperationTimer::elapsed)
        .unwrap_or_default();
    if timing.is_enabled() {
        eprintln!(
            "[render-timing] diagram=mindmap total={:?} deserialize={:?} build_ctx={:?} viewbox={:?} render_svg={:?} finalize={:?} nodes={} edges={}",
            timings.total,
            timings.deserialize_model,
            timings.build_ctx,
            timings.viewbox,
            timings.render_svg,
            timings.finalize_svg,
            model.nodes.len(),
            model.edges.len(),
        );
    }

    root_document.complete(out.finish()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepared_gradient_preserves_coercion_and_requires_both_raw_endpoints() {
        for enabled in [
            serde_json::json!(true),
            serde_json::json!(1),
            serde_json::json!("on"),
        ] {
            for complete in [false, true] {
                let mut raw = serde_json::json!({"themeVariables": {
                    "useGradient": enabled, "gradientStart": "var(--start)"
                }});
                if complete {
                    raw["themeVariables"]["gradientStop"] = serde_json::json!("currentColor");
                }
                let config = super::super::tests::presentation_config(raw);
                let binding = MindmapCssBinding::new(&config);
                let defs = mindmap_gradient_defs("scoped", &binding);
                if complete {
                    assert!(defs.contains("id=\"scoped-gradient\""));
                    assert!(defs.contains("stop-color=\"var(--start)\""));
                    assert!(defs.contains("stop-color=\"currentColor\""));
                } else {
                    assert!(defs.is_empty());
                }
            }
        }
    }
    use std::fmt;
    use std::ops::Range;

    #[derive(Default)]
    struct RejectAfterFirstWrite {
        write_attempts: usize,
        rejected: bool,
        retained: String,
    }

    impl RejectAfterFirstWrite {
        fn record_write(&mut self, value: &str) -> fmt::Result {
            self.write_attempts += 1;
            if self.write_attempts == 1 {
                self.rejected = true;
                return Err(fmt::Error);
            }
            self.retained.push_str(value);
            Ok(())
        }
    }

    impl fmt::Write for RejectAfterFirstWrite {
        fn write_str(&mut self, value: &str) -> fmt::Result {
            self.record_write(value)
        }
    }

    impl SvgOutput for RejectAfterFirstWrite {
        fn push_str(&mut self, value: &str) {
            let _ = self.record_write(value);
        }

        fn push(&mut self, value: char) {
            let mut encoded = [0u8; 4];
            let _ = self.record_write(value.encode_utf8(&mut encoded));
        }

        fn len(&self) -> usize {
            self.retained.len()
        }

        fn as_str(&self) -> &str {
            self.retained.as_str()
        }

        fn replace_range(&mut self, range: Range<usize>, replacement: &str) -> Result<()> {
            self.retained.replace_range(range, replacement);
            Ok(())
        }

        fn checkpoint(&mut self) -> Result<()> {
            if self.rejected {
                Err(Error::InvalidModel {
                    message: "test SVG sink rejected the first write".to_string(),
                })
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn mindmap_edge_label_stops_after_the_first_svg_sink_failure() {
        let mut out = RejectAfterFirstWrite::default();
        let error =
            write_mindmap_edge_label(&mut out, "edge-0", &PresentationConfig::empty_object())
                .expect_err("the rejecting sink must stop Mindmap edge-label emission");

        assert!(matches!(error, Error::InvalidModel { .. }));
        assert_eq!(
            out.write_attempts, 1,
            "Mindmap edge-label emission must stop at the first failed sink checkpoint"
        );
    }

    #[test]
    fn single_image_paragraph_is_unwrapped_from_the_final_xhtml_shape() {
        assert_eq!(
            super::super::single_image_paragraph_inner(r#"<p><img src="a>b" /></p>"#),
            Some(r#"<img src="a>b" />"#)
        );
        assert_eq!(
            super::super::single_image_paragraph_inner("<p>text</p>"),
            None
        );
        assert_eq!(
            super::super::single_image_paragraph_inner("<p><img src=x><span>extra</span></p>"),
            None
        );
    }

    #[test]
    fn mindmap_css_stops_at_the_first_bounded_sink_checkpoint() {
        let config = merman_model::presentation::PresentationConfig::empty_object();
        let binding = MindmapCssBinding::new(&config);
        let mut out = RejectAfterFirstWrite::default();
        assert!(mindmap_css(&mut out, "bounded", &binding).is_err());
        assert_eq!(
            out.write_attempts, 1,
            "failed prefix admission must stop all section expansion"
        );
        assert!(out.retained.is_empty());
    }

    #[test]
    fn mindmap_css_honors_mermaid_11_15_theme_sections() {
        let cfg = serde_json::json!({
            "theme": "redux",
            "look": "neo",
            "themeVariables": {
                "THEME_COLOR_LIMIT": 3,
                "cScale0": "#101010",
                "cScaleLabel0": "#f0f0f0",
                "cScaleInv0": "#202020",
                "cScale1": "#303030",
                "cScaleLabel1": "#404040",
                "cScaleInv1": "#505050",
                "cScale2": "#606060",
                "cScaleLabel2": "#707070",
                "cScaleInv2": "#808080",
                "git0": "#909090",
                "gitBranchLabel0": "#a0a0a0",
                "nodeBorder": "#b0b0b0"
            }
        });

        let config = super::super::tests::presentation_config(cfg);
        let binding = MindmapCssBinding::new(&config);
        let mut css = String::new();
        mindmap_css(&mut css, "mm", &binding).unwrap();

        assert!(css.contains(r#"#mm .section--1 rect,#mm .section--1 path,#mm .section--1 circle,#mm .section--1 polygon,#mm .section--1 path{fill:#101010;}"#));
        assert!(css.contains(r#"#mm .section--1 span{color:#f0f0f0;}"#));
        assert!(css.contains(r#"#mm .section-0 span{color:#404040;}"#));
        assert!(css.contains(r#"#mm .section-1 line{stroke:#808080;stroke-width:3;}"#));
        assert!(css.contains(r#"#mm .edge-depth--1{stroke-width:12;}"#));
        assert!(css.contains(r#"#mm .edge-depth-0{stroke-width:10;}"#));
        assert!(css.contains(r#"#mm .section-root rect,#mm .section-root path,#mm .section-root circle,#mm .section-root polygon{fill:#909090;}"#));
        assert!(css.contains(r#"#mm .section-root text{fill:#a0a0a0;}"#));
        assert!(css.contains(r#"#mm .section-root span{color:#b0b0b0;}"#));
    }

    #[test]
    fn viewport_bounds_include_cloud_path_bbox() {
        let layout = MindmapDiagramLayout {
            swimlane_lanes: Vec::new(),
            nodes: vec![LayoutNode {
                id: "0".to_string(),
                x: 63.953125,
                y: 32.0,
                width: 97.90625,
                height: 34.0,
                is_cluster: false,
                label_width: Some(87.90625),
                label_height: Some(24.0),
            }],
            edges: Vec::new(),
            bounds: Some(Bounds {
                min_x: 15.0,
                min_y: 15.0,
                max_x: 112.90625,
                max_y: 49.0,
            }),
        };
        let model = merman_model::families::mindmap::MindmapDiagramRenderModel {
            nodes: vec![merman_model::families::mindmap::MindmapDiagramRenderNode {
                id: "0".to_string(),
                dom_id: "node_0".to_string(),
                label: "I am a cloud".to_string(),
                label_type: String::new(),
                is_group: false,
                shape: "cloud".to_string(),
                width: 0.0,
                height: 0.0,
                padding: 10.0,
                css_classes: "mindmap-node section-root section--1".to_string(),
                css_styles: Vec::new(),
                look: String::new(),
                icon: None,
                x: None,
                y: None,
                level: 0,
                node_id: "id".to_string(),
                node_type: -1,
                section: Some(-1),
            }],
            edges: Vec::new(),
        };

        let layout_bounds = layout.bounds.as_ref().expect("layout bounds");
        let bounds = mindmap_viewport_bounds_from_layout(&layout, &model).expect("bounds");

        assert!(bounds.min_x < layout_bounds.min_x);
        assert!(bounds.min_y < layout_bounds.min_y);
        assert!(bounds.max_x > layout_bounds.max_x);
        assert!(bounds.max_y > layout_bounds.max_y);
    }
}
