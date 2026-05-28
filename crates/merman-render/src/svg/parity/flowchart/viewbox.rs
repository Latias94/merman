//! Flowchart final viewBox/content-bounds preparation.

use std::borrow::Cow;

use rustc_hash::FxHashMap;

use super::render::node::geom::{generate_circle_points, generate_full_sine_wave_points};
use super::*;

const TITLE_FONT_SIZE_PX: f64 = 18.0;

pub(in crate::svg::parity::flowchart) struct FlowchartRenderedBoundsRequest<'view, 'data> {
    pub ctx: &'view FlowchartRenderCtx<'data>,
    pub layout: &'view FlowchartV2Layout,
    pub subgraph_title_y_shift: f64,
}

pub(in crate::svg::parity::flowchart) struct FlowchartViewboxBoundsRequest<
    'borrow,
    'view,
    'data,
    'title,
> {
    pub ctx: &'view FlowchartRenderCtx<'data>,
    pub render_edges: &'view [Cow<'data, crate::flowchart::FlowEdge>],
    pub base_bounds: Bounds,
    pub diagram_title: Option<&'title str>,
    pub font_family: &'borrow str,
    pub title_top_margin: f64,
    pub timing_enabled: bool,
    pub viewbox_edge_curve_bounds: &'borrow mut std::time::Duration,
    pub detail: &'borrow mut FlowchartRenderDetails,
    pub edge_path_cache: &'borrow mut FxHashMap<&'view str, FlowchartEdgePathCacheEntry>,
}

pub(in crate::svg::parity::flowchart) struct FlowchartViewboxBounds {
    pub diagram_title: Option<String>,
    pub title_anchor_x: f64,
    pub bbox_min_x: f64,
    pub bbox_min_y: f64,
    pub bbox_max_x: f64,
    pub bbox_max_y: f64,
}

fn union_svg_path_bounds(paths: &[&str]) -> Option<crate::svg::parity::path_bounds::SvgPathBounds> {
    let mut bounds: Option<crate::svg::parity::path_bounds::SvgPathBounds> = None;
    for d in paths {
        let Some(pb) = crate::svg::parity::path_bounds::svg_path_bounds_from_d(d) else {
            continue;
        };
        bounds = Some(match bounds {
            Some(mut acc) => {
                acc.min_x = acc.min_x.min(pb.min_x);
                acc.min_y = acc.min_y.min(pb.min_y);
                acc.max_x = acc.max_x.max(pb.max_x);
                acc.max_y = acc.max_y.max(pb.max_y);
                acc
            }
            None => pb,
        });
    }
    bounds
}

fn rough_svg_path_bounds(
    path_data: &str,
) -> Option<crate::svg::parity::path_bounds::SvgPathBounds> {
    let (fill_d, stroke_d) =
        crate::svg::parity::flowchart::render::node::roughjs::roughjs_paths_for_svg_path(
            path_data, "#000", "#000", 1.3, "0 0", 0,
        )?;
    union_svg_path_bounds(&[fill_d.as_str(), stroke_d.as_str()])
}

fn rough_stroke_svg_path_bounds(
    path_data: &str,
) -> Option<crate::svg::parity::path_bounds::SvgPathBounds> {
    let stroke_d =
        crate::svg::parity::flowchart::render::node::roughjs::roughjs_stroke_path_for_svg_path(
            path_data, "#000", 1.3, "0 0", 0,
        )?;
    crate::svg::parity::path_bounds::svg_path_bounds_from_d(&stroke_d)
}

pub(in crate::svg::parity::flowchart) fn prepare_flowchart_rendered_bounds<'data, F>(
    request: FlowchartRenderedBoundsRequest<'_, 'data>,
    effective_parent_for_id: &F,
) -> Bounds
where
    F: Fn(&str) -> Option<&'data str>,
{
    let FlowchartRenderedBoundsRequest {
        ctx,
        layout,
        subgraph_title_y_shift,
    } = request;
    let node_padding = ctx.node_padding;

    let mut lca_scratch: Vec<&str> = Vec::new();

    let y_offset_for_root = |root: Option<&str>| -> f64 {
        if root.is_some() && subgraph_title_y_shift.abs() >= 1e-9 {
            -subgraph_title_y_shift
        } else {
            0.0
        }
    };

    // Mermaid's flowchart-v2 renderer draws the self-loop helper nodes (`labelRect`) as
    // `<g class="label edgeLabel" transform="translate(x, y)">` with a `0.1 x 0.1` rect anchored
    // at the translated origin (top-left). Dagre's `x/y` still represent a node center, but the
    // rendered DOM bbox that drives `setupViewPortForSVG(svg, diagramPadding)` is top-left based.
    // Account for that when approximating the final `svg.getBBox()`.
    let mut b: Option<Bounds> = None;
    let mut include_rect = |min_x: f64, min_y: f64, max_x: f64, max_y: f64| {
        if let Some(ref mut cur) = b {
            cur.min_x = cur.min_x.min(min_x);
            cur.min_y = cur.min_y.min(min_y);
            cur.max_x = cur.max_x.max(max_x);
            cur.max_y = cur.max_y.max(max_y);
        } else {
            b = Some(Bounds {
                min_x,
                min_y,
                max_x,
                max_y,
            });
        }
    };

    for c in &layout.clusters {
        let root = if ctx.recursive_clusters.contains(c.id.as_str()) {
            Some(c.id.as_str())
        } else {
            effective_parent_for_id(&c.id)
        };
        let y_off = y_offset_for_root(root);
        let hw = c.width / 2.0;
        let hh = c.height / 2.0;
        include_rect(c.x - hw, c.y + y_off - hh, c.x + hw, c.y + y_off + hh);

        let lhw = c.title_label.width / 2.0;
        let lhh = c.title_label.height / 2.0;
        include_rect(
            c.title_label.x - lhw,
            c.title_label.y + y_off - lhh,
            c.title_label.x + lhw,
            c.title_label.y + y_off + lhh,
        );
    }

    for n in &layout.nodes {
        let is_empty_subgraph_node = ctx
            .subgraphs_by_id
            .get(n.id.as_str())
            .is_some_and(|sg| sg.nodes.is_empty());
        let root = if n.is_cluster && ctx.recursive_clusters.contains(n.id.as_str()) {
            Some(n.id.as_str())
        } else {
            effective_parent_for_id(&n.id)
        };
        let y_off = y_offset_for_root(root);
        if n.is_cluster || ctx.node_dom_index.contains_key(n.id.as_str()) || is_empty_subgraph_node
        {
            let mut left_hw = n.width / 2.0;
            let mut right_hw = left_hw;
            let mut top_hh = n.height / 2.0;
            let mut bottom_hh = top_hh;
            if !n.is_cluster {
                let node_label_metrics =
                    |n: &crate::model::LayoutNode| -> crate::text::TextMetrics {
                        if let (Some(width), Some(height)) = (n.label_width, n.label_height) {
                            return crate::text::TextMetrics {
                                width,
                                height,
                                line_count: 0,
                            };
                        }
                        let Some(flow_node) = ctx.nodes_by_id.get(n.id.as_str()) else {
                            return crate::text::TextMetrics {
                                width: 0.0,
                                height: 0.0,
                                line_count: 0,
                            };
                        };
                        let label = flow_node.label.as_deref().unwrap_or("");
                        let label_type = flow_node
                            .label_type
                            .as_deref()
                            .unwrap_or(if ctx.node_html_labels { "html" } else { "text" });
                        let label_base_style =
                            if ctx.node_wrap_mode == crate::text::WrapMode::HtmlLike {
                                &ctx.html_label_text_style
                            } else {
                                &ctx.text_style
                            };
                        let node_text_style =
                            crate::flowchart::flowchart_effective_text_style_for_node_classes(
                                label_base_style,
                                ctx.class_defs,
                                &flow_node.classes,
                                &flow_node.styles,
                            );
                        crate::flowchart::flowchart_label_metrics_for_layout(
                            crate::flowchart::FlowchartLabelMetricsRequest {
                                measurer: ctx.measurer,
                                raw_label: label,
                                label_type,
                                style: &node_text_style,
                                max_width_px: Some(ctx.wrapping_width),
                                wrap_mode: ctx.node_wrap_mode,
                                config: ctx.config,
                                math_renderer: ctx.math_renderer,
                                preserve_string_whitespace_height: ctx.node_html_labels
                                    && ctx.edge_html_labels,
                            },
                        )
                    };

                if let Some(shape) = ctx
                    .nodes_by_id
                    .get(n.id.as_str())
                    .and_then(|node| node.layout_shape.as_deref())
                {
                    // Mermaid's flowchart-v2 rhombus node renderer offsets the polygon by
                    // `(-width/2 + 0.5, height/2)` so the diamond outline stays on the same
                    // pixel lattice as other nodes. This makes the DOM bbox slightly asymmetric
                    // around the node center and affects the root `getBBox()` width.
                    if shape == "diamond" || shape == "diam" || shape == "rhombus" {
                        left_hw = (left_hw - 0.5).max(0.0);
                        right_hw += 0.5;
                    }

                    // Mermaid `stateEnd.ts` renders the framed-circle using a RoughJS ellipse
                    // path with a slightly asymmetric bbox in Chromium.
                    if matches!(shape, "fr-circ" | "framed-circle" | "stop") {
                        left_hw = 7.0;
                        right_hw = (n.width - 7.0).max(0.0);
                    }

                    // Mermaid `filledCircle.ts` uses a RoughJS circle path (roughness=0) whose
                    // bbox is slightly asymmetric.
                    if matches!(shape, "f-circ") {
                        left_hw = 7.0;
                        right_hw = (n.width - 7.0).max(0.0);
                    }

                    // Mermaid `crossedCircle.ts` uses a RoughJS circle path with radius=30; its
                    // bbox is slightly asymmetric in Chromium.
                    if matches!(shape, "cross-circ" | "summary" | "crossed-circle") {
                        left_hw = 30.0;
                        right_hw = (n.width - 30.0).max(0.0);
                        top_hh = 30.0;
                        bottom_hh = 30.0;
                    }

                    // Mermaid `halfRoundedRectangle.ts` and `curvedTrapezoid.ts` draw their rough
                    // paths from the "theoretical" text+padding width, but Dagre uses the
                    // `updateNodeBounds(...)` bbox which can be slightly narrower.
                    if matches!(
                        shape,
                        "delay" | "curv-trap" | "display" | "curved-trapezoid"
                    ) {
                        if let Some(label_w) = n.label_width {
                            let pre_w = if shape == "delay" {
                                (label_w + 2.0 * node_padding).max(80.0)
                            } else {
                                ((label_w + 2.0 * node_padding) * 1.25).max(80.0)
                            };
                            left_hw = pre_w / 2.0;
                            right_hw = (n.width - left_hw).max(0.0);
                        } else if let Some(flow_node) = ctx.nodes_by_id.get(n.id.as_str()) {
                            let label = flow_node.label.as_deref().unwrap_or("");
                            let label_type = flow_node
                                .label_type
                                .as_deref()
                                .unwrap_or(if ctx.node_html_labels { "html" } else { "text" });
                            let label_base_style =
                                if ctx.node_wrap_mode == crate::text::WrapMode::HtmlLike {
                                    &ctx.html_label_text_style
                                } else {
                                    &ctx.text_style
                                };
                            let node_text_style =
                                crate::flowchart::flowchart_effective_text_style_for_node_classes(
                                    label_base_style,
                                    ctx.class_defs,
                                    &flow_node.classes,
                                    &flow_node.styles,
                                );
                            let metrics = crate::flowchart::flowchart_label_metrics_for_layout(
                                crate::flowchart::FlowchartLabelMetricsRequest {
                                    measurer: ctx.measurer,
                                    raw_label: label,
                                    label_type,
                                    style: &node_text_style,
                                    max_width_px: Some(ctx.wrapping_width),
                                    wrap_mode: ctx.node_wrap_mode,
                                    config: ctx.config,
                                    math_renderer: ctx.math_renderer,
                                    preserve_string_whitespace_height: ctx.node_html_labels
                                        && ctx.edge_html_labels,
                                },
                            );
                            let pre_w = if shape == "delay" {
                                (metrics.width + 2.0 * node_padding).max(80.0)
                            } else {
                                ((metrics.width + 2.0 * node_padding) * 1.25).max(80.0)
                            };
                            left_hw = pre_w / 2.0;
                            right_hw = (n.width - left_hw).max(0.0);
                        }
                    }

                    // Mermaid `waveEdgedRectangle.ts` (document) stores Dagre dimensions from
                    // `updateNodeBounds(...)`, but the final root viewport comes from the rendered
                    // RoughJS path bbox. Rebuild that bbox directly.
                    if matches!(shape, "doc" | "document") {
                        let (label_w, label_h) =
                            if let (Some(w), Some(h)) = (n.label_width, n.label_height) {
                                (w, h)
                            } else if let Some(flow_node) = ctx.nodes_by_id.get(n.id.as_str()) {
                                let label = flow_node.label.as_deref().unwrap_or("");
                                let label_type = flow_node
                                    .label_type
                                    .as_deref()
                                    .unwrap_or(if ctx.node_html_labels { "html" } else { "text" });
                                let label_base_style =
                                    if ctx.node_wrap_mode == crate::text::WrapMode::HtmlLike {
                                        &ctx.html_label_text_style
                                    } else {
                                        &ctx.text_style
                                    };
                                let node_text_style =
                                crate::flowchart::flowchart_effective_text_style_for_node_classes(
                                    label_base_style,
                                    ctx.class_defs,
                                    &flow_node.classes,
                                    &flow_node.styles,
                                );
                                let metrics = crate::flowchart::flowchart_label_metrics_for_layout(
                                    crate::flowchart::FlowchartLabelMetricsRequest {
                                        measurer: ctx.measurer,
                                        raw_label: label,
                                        label_type,
                                        style: &node_text_style,
                                        max_width_px: Some(ctx.wrapping_width),
                                        wrap_mode: ctx.node_wrap_mode,
                                        config: ctx.config,
                                        math_renderer: ctx.math_renderer,
                                        preserve_string_whitespace_height: ctx.node_html_labels
                                            && ctx.edge_html_labels,
                                    },
                                );
                                (metrics.width, metrics.height)
                            } else {
                                (0.0, 0.0)
                            };

                        let w = (label_w + 2.0 * node_padding).max(0.0);
                        let h = (label_h + 2.0 * node_padding).max(0.0);
                        let wave_amplitude = h / 8.0;
                        let final_h = h + wave_amplitude;
                        let extra_w = ((70.0 - w).max(0.0)) / 2.0;
                        let mut points: Vec<(f64, f64)> = Vec::new();
                        points.push((-w / 2.0 - extra_w, final_h / 2.0));
                        points.extend(generate_full_sine_wave_points(
                            -w / 2.0 - extra_w,
                            final_h / 2.0,
                            w / 2.0 + extra_w,
                            final_h / 2.0,
                            wave_amplitude,
                            0.8,
                        ));
                        points.push((w / 2.0 + extra_w, -final_h / 2.0));
                        points.push((-w / 2.0 - extra_w, -final_h / 2.0));

                        let path_data =
                            crate::svg::parity::roughjs_common::closed_path_d_from_points(&points);
                        if let Some(pb) = rough_svg_path_bounds(&path_data) {
                            let y_shift = -wave_amplitude / 2.0;
                            left_hw = (-pb.min_x).max(0.0);
                            right_hw = pb.max_x.max(0.0);
                            top_hh = (-(pb.min_y + y_shift)).max(0.0);
                            bottom_hh = (pb.max_y + y_shift).max(0.0);
                        }
                    }

                    // Mermaid `linedWaveEdgedRect.ts` follows the same split as the other wave
                    // document shapes: Dagre uses the post-`updateNodeBounds(...)` dimensions,
                    // while the rendered root bbox comes from the original label-box path.
                    if matches!(shape, "lin-doc" | "lined-document") {
                        let (label_w, label_h) =
                            if let (Some(w), Some(h)) = (n.label_width, n.label_height) {
                                (w, h)
                            } else if let Some(flow_node) = ctx.nodes_by_id.get(n.id.as_str()) {
                                let label = flow_node.label.as_deref().unwrap_or("");
                                let label_type = flow_node
                                    .label_type
                                    .as_deref()
                                    .unwrap_or(if ctx.node_html_labels { "html" } else { "text" });
                                let label_base_style =
                                    if ctx.node_wrap_mode == crate::text::WrapMode::HtmlLike {
                                        &ctx.html_label_text_style
                                    } else {
                                        &ctx.text_style
                                    };
                                let node_text_style =
                                crate::flowchart::flowchart_effective_text_style_for_node_classes(
                                    label_base_style,
                                    ctx.class_defs,
                                    &flow_node.classes,
                                    &flow_node.styles,
                                );
                                let metrics = crate::flowchart::flowchart_label_metrics_for_layout(
                                    crate::flowchart::FlowchartLabelMetricsRequest {
                                        measurer: ctx.measurer,
                                        raw_label: label,
                                        label_type,
                                        style: &node_text_style,
                                        max_width_px: Some(ctx.wrapping_width),
                                        wrap_mode: ctx.node_wrap_mode,
                                        config: ctx.config,
                                        math_renderer: ctx.math_renderer,
                                        preserve_string_whitespace_height: ctx.node_html_labels
                                            && ctx.edge_html_labels,
                                    },
                                );
                                (metrics.width, metrics.height)
                            } else {
                                (0.0, 0.0)
                            };

                        let w = (label_w + 2.0 * node_padding).max(0.0);
                        let h = (label_h + 2.0 * node_padding).max(0.0);
                        let wave_amplitude = h / 4.0;
                        let final_h = h + wave_amplitude;
                        let extra = (w / 2.0) * 0.1;
                        let mut points: Vec<(f64, f64)> = Vec::new();
                        points.push((-w / 2.0 - extra, -final_h / 2.0));
                        points.push((-w / 2.0 - extra, final_h / 2.0));
                        points.extend(generate_full_sine_wave_points(
                            -w / 2.0 - extra,
                            final_h / 2.0,
                            w / 2.0 + extra,
                            final_h / 2.0,
                            wave_amplitude,
                            0.8,
                        ));
                        points.push((w / 2.0 + extra, -final_h / 2.0));
                        points.push((-w / 2.0 - extra, -final_h / 2.0));
                        points.push((-w / 2.0, -final_h / 2.0));
                        points.push((-w / 2.0, (final_h / 2.0) * 1.1));
                        points.push((-w / 2.0, -final_h / 2.0));

                        let path_data =
                            crate::svg::parity::roughjs_common::closed_path_d_from_points(&points);
                        if let Some(pb) = rough_svg_path_bounds(&path_data) {
                            let y_shift = -wave_amplitude / 2.0;
                            left_hw = (-pb.min_x).max(0.0);
                            right_hw = pb.max_x.max(0.0);
                            top_hh = (-(pb.min_y + y_shift)).max(0.0);
                            bottom_hh = (pb.max_y + y_shift).max(0.0);
                        }
                    }

                    // Mermaid `taggedWaveEdgedRectangle.ts` (tagged-document) renders from the
                    // base label box, then `updateNodeBounds(...)` stores a slightly shorter outer
                    // bbox. The rendered wave is also vertically asymmetric.
                    if matches!(shape, "tag-doc" | "tagged-document") {
                        let label_h = if let Some(h) = n.label_height {
                            h
                        } else if let Some(flow_node) = ctx.nodes_by_id.get(n.id.as_str()) {
                            let label = flow_node.label.as_deref().unwrap_or("");
                            let label_type = flow_node
                                .label_type
                                .as_deref()
                                .unwrap_or(if ctx.node_html_labels { "html" } else { "text" });
                            let label_base_style =
                                if ctx.node_wrap_mode == crate::text::WrapMode::HtmlLike {
                                    &ctx.html_label_text_style
                                } else {
                                    &ctx.text_style
                                };
                            let node_text_style =
                                crate::flowchart::flowchart_effective_text_style_for_node_classes(
                                    label_base_style,
                                    ctx.class_defs,
                                    &flow_node.classes,
                                    &flow_node.styles,
                                );
                            let metrics = crate::flowchart::flowchart_label_metrics_for_layout(
                                crate::flowchart::FlowchartLabelMetricsRequest {
                                    measurer: ctx.measurer,
                                    raw_label: label,
                                    label_type,
                                    style: &node_text_style,
                                    max_width_px: Some(ctx.wrapping_width),
                                    wrap_mode: ctx.node_wrap_mode,
                                    config: ctx.config,
                                    math_renderer: ctx.math_renderer,
                                    preserve_string_whitespace_height: ctx.node_html_labels
                                        && ctx.edge_html_labels,
                                },
                            );
                            metrics.height
                        } else {
                            0.0
                        };

                        let h = (label_h + 2.0 * node_padding).max(0.0);
                        let wave_amplitude = h / 4.0;
                        top_hh = h / 2.0 + wave_amplitude;
                        bottom_hh = (n.height - top_hh).max(0.0);
                    }

                    // Mermaid computes the root viewport from the rendered DOM bbox. Curly
                    // brace/comment shapes emit narrow RoughJS stroke paths plus an invisible
                    // path; using the inflated Dagre `node.width / 2` keeps a phantom edge.
                    if matches!(
                        shape,
                        "comment" | "brace" | "brace-l" | "brace-r" | "braces"
                    ) {
                        let metrics = node_label_metrics(n);
                        let geometry = crate::svg::parity::flowchart::render::node::shapes::curly_brace_comment_geometry(
                            shape,
                            metrics.width,
                            metrics.height,
                            node_padding,
                        );
                        let mut bounds: Option<crate::svg::parity::path_bounds::SvgPathBounds> =
                            None;
                        for path in geometry.paths {
                            if let Some(mut pb) = rough_stroke_svg_path_bounds(&path.d) {
                                pb.min_x += geometry.group_tx;
                                pb.max_x += geometry.group_tx;
                                bounds = Some(match bounds {
                                    Some(mut acc) => {
                                        acc.min_x = acc.min_x.min(pb.min_x);
                                        acc.min_y = acc.min_y.min(pb.min_y);
                                        acc.max_x = acc.max_x.max(pb.max_x);
                                        acc.max_y = acc.max_y.max(pb.max_y);
                                        acc
                                    }
                                    None => pb,
                                });
                            }
                        }
                        if let Some(pb) = bounds {
                            left_hw = (-pb.min_x).max(0.0);
                            right_hw = pb.max_x.max(0.0);
                            top_hh = (-pb.min_y).max(0.0);
                            bottom_hh = pb.max_y.max(0.0);
                        }
                    }

                    // Mermaid `forkJoin.ts` inflates Dagre dimensions but the rendered bar
                    // remains `70x10` (or `10x70` for LR).
                    if matches!(shape, "fork" | "join") {
                        if n.width >= n.height {
                            left_hw = 35.0;
                            right_hw = 35.0;
                            top_hh = 5.0;
                            bottom_hh = 5.0;
                        } else {
                            left_hw = 5.0;
                            right_hw = 5.0;
                            top_hh = 35.0;
                            bottom_hh = 35.0;
                        }
                    }

                    // Mermaid `multiWaveEdgedRectangle.ts` emits a bottom sine wave and then
                    // translates the whole group upward by `waveAmplitude / 2`.
                    if matches!(shape, "docs" | "documents" | "st-doc" | "stacked-document") {
                        let (label_w, label_h) =
                            if let (Some(w), Some(h)) = (n.label_width, n.label_height) {
                                (w, h)
                            } else if let Some(flow_node) = ctx.nodes_by_id.get(n.id.as_str()) {
                                let label = flow_node.label.as_deref().unwrap_or("");
                                let label_type = flow_node
                                    .label_type
                                    .as_deref()
                                    .unwrap_or(if ctx.node_html_labels { "html" } else { "text" });
                                let label_base_style =
                                    if ctx.node_wrap_mode == crate::text::WrapMode::HtmlLike {
                                        &ctx.html_label_text_style
                                    } else {
                                        &ctx.text_style
                                    };
                                let node_text_style =
                                crate::flowchart::flowchart_effective_text_style_for_node_classes(
                                    label_base_style,
                                    ctx.class_defs,
                                    &flow_node.classes,
                                    &flow_node.styles,
                                );
                                let metrics = crate::flowchart::flowchart_label_metrics_for_layout(
                                    crate::flowchart::FlowchartLabelMetricsRequest {
                                        measurer: ctx.measurer,
                                        raw_label: label,
                                        label_type,
                                        style: &node_text_style,
                                        max_width_px: Some(ctx.wrapping_width),
                                        wrap_mode: ctx.node_wrap_mode,
                                        config: ctx.config,
                                        math_renderer: ctx.math_renderer,
                                        preserve_string_whitespace_height: ctx.node_html_labels
                                            && ctx.edge_html_labels,
                                    },
                                );
                                (metrics.width, metrics.height)
                            } else {
                                (0.0, 0.0)
                            };

                        let w = label_w + 2.0 * node_padding;
                        let h = label_h + 2.0 * node_padding;
                        let wave_amplitude = h / 4.0;
                        let final_h = h + wave_amplitude;
                        let rect_offset = 5.0;
                        let y = -final_h / 2.0;
                        let baseline_y = y + final_h + rect_offset;

                        let mut max_wave_y = baseline_y;
                        let delta_x = w;
                        let cycle_length = if delta_x.abs() < 1e-9 {
                            delta_x
                        } else {
                            delta_x / 0.8
                        };
                        let frequency = if cycle_length.abs() < 1e-9 {
                            0.0
                        } else {
                            (2.0 * std::f64::consts::PI) / cycle_length
                        };
                        for i in 0..=50 {
                            let t = i as f64 / 50.0;
                            let x = t * delta_x;
                            let wave_y = baseline_y + wave_amplitude * (frequency * x).sin();
                            max_wave_y = max_wave_y.max(wave_y);
                        }

                        let top_y = y - rect_offset - wave_amplitude / 2.0;
                        let bottom_y = max_wave_y - wave_amplitude / 2.0;
                        top_hh = -top_y;
                        bottom_hh = bottom_y;
                        if left_hw == right_hw {
                            left_hw = w / 2.0 + rect_offset;
                            right_hw = left_hw;
                        }
                    }

                    if matches!(shape, "delay" | "half-rounded-rectangle") {
                        let label_w = n.label_width.unwrap_or(0.0);
                        let label_h = n.label_height.unwrap_or(0.0);
                        let w = (label_w + 2.0 * node_padding).max(80.0);
                        let h = (label_h + 2.0 * node_padding).max(50.0);
                        let radius = h / 2.0;
                        let mut points: Vec<(f64, f64)> = Vec::new();
                        points.push((-w / 2.0, -h / 2.0));
                        points.push((w / 2.0 - radius, -h / 2.0));
                        points.extend(generate_circle_points(
                            -w / 2.0 + radius,
                            0.0,
                            radius,
                            50,
                            90.0,
                            270.0,
                        ));
                        points.push((w / 2.0 - radius, h / 2.0));
                        points.push((-w / 2.0, h / 2.0));

                        let path_data =
                            crate::svg::parity::roughjs_common::closed_path_d_from_points(&points);
                        if let Some(pb) = rough_svg_path_bounds(&path_data) {
                            left_hw = (-pb.min_x).max(0.0);
                            right_hw = pb.max_x.max(0.0);
                            top_hh = (-pb.min_y).max(0.0);
                            bottom_hh = pb.max_y.max(0.0);
                        }
                    }

                    if matches!(shape, "notch-pent" | "loop-limit" | "notched-pentagon") {
                        let label_w = n.label_width.unwrap_or(0.0);
                        let label_h = n.label_height.unwrap_or(0.0);
                        let w = (label_w + 2.0 * node_padding).max(60.0);
                        let h = (label_h + 2.0 * node_padding).max(20.0);
                        let points = vec![
                            ((-w / 2.0) * 0.8, -h / 2.0),
                            ((w / 2.0) * 0.8, -h / 2.0),
                            (w / 2.0, (-h / 2.0) * 0.6),
                            (w / 2.0, h / 2.0),
                            (-w / 2.0, h / 2.0),
                            (-w / 2.0, (-h / 2.0) * 0.6),
                        ];
                        let path_data =
                            crate::svg::parity::roughjs_common::closed_path_d_from_points(&points);
                        if let Some(pb) = rough_svg_path_bounds(&path_data) {
                            left_hw = (-pb.min_x).max(0.0);
                            right_hw = pb.max_x.max(0.0);
                            top_hh = (-pb.min_y).max(0.0);
                            bottom_hh = pb.max_y.max(0.0);
                        }
                    }
                }
            }
            include_rect(
                n.x - left_hw,
                n.y + y_off - top_hh,
                n.x + right_hw,
                n.y + y_off + bottom_hh,
            );
        } else {
            include_rect(n.x, n.y + y_off, n.x + n.width, n.y + y_off + n.height);
        }
    }

    for e in &layout.edges {
        let root = lca_for_ids(
            e.from.as_str(),
            e.to.as_str(),
            effective_parent_for_id,
            &mut lca_scratch,
        );
        let y_off = y_offset_for_root(root);
        for lbl in [
            e.label.as_ref(),
            e.start_label_left.as_ref(),
            e.start_label_right.as_ref(),
            e.end_label_left.as_ref(),
            e.end_label_right.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            let hw = lbl.width / 2.0;
            let hh = lbl.height / 2.0;
            let svg_label_y_offset = if ctx.edge_html_labels { 0.0 } else { 1.0 };
            include_rect(
                lbl.x - hw,
                lbl.y + y_off - hh - svg_label_y_offset,
                lbl.x + hw,
                lbl.y + y_off + hh - svg_label_y_offset,
            );
        }
    }

    b.unwrap_or({
        if layout.nodes.is_empty() && layout.edges.is_empty() && layout.clusters.is_empty() {
            Bounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 0.0,
                max_y: 0.0,
            }
        } else {
            Bounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 100.0,
                max_y: 100.0,
            }
        }
    })
}

pub(in crate::svg::parity::flowchart) fn prepare_flowchart_viewbox_bounds<'data, F>(
    request: FlowchartViewboxBoundsRequest<'_, '_, 'data, '_>,
    effective_parent_for_id: &F,
) -> FlowchartViewboxBounds
where
    F: Fn(&str) -> Option<&'data str>,
{
    let FlowchartViewboxBoundsRequest {
        ctx,
        render_edges,
        base_bounds,
        diagram_title,
        font_family,
        title_top_margin,
        timing_enabled,
        viewbox_edge_curve_bounds,
        detail,
        edge_path_cache,
    } = request;

    // Mermaid computes the final viewport using `svg.getBBox()` after inserting the title, then
    // applies `setupViewPortForSVG(svg, diagramPadding)`.
    let diagram_title = diagram_title
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_owned);

    let mut bbox_min_x = base_bounds.min_x + ctx.tx;
    let mut bbox_min_y = base_bounds.min_y + ctx.ty;
    let mut bbox_max_x = base_bounds.max_x + ctx.tx;
    let mut bbox_max_y = base_bounds.max_y + ctx.ty;

    bbox_max_y += extra_recursive_root_y(ctx);

    // Mermaid derives the final viewport using `svg.getBBox()` (after rendering). For flowcharts
    // this includes the actual curve geometry generated by D3 (which can extend beyond the routed
    // polyline points). Headlessly, approximate that by unioning a tight bbox over each rendered
    // edge path `d` into our base bbox.
    {
        let _g = timing_enabled
            .then(|| super::super::timing::TimingGuard::new(viewbox_edge_curve_bounds));
        let mut lca_scratch: Vec<&str> = Vec::new();
        let mut scratch = FlowchartEdgeDataPointsScratch::default();
        let mut root_offsets: FxHashMap<&str, FlowchartRootOffsets> =
            FxHashMap::with_capacity_and_hasher(8, Default::default());
        root_offsets.insert(
            "",
            FlowchartRootOffsets {
                origin_x: 0.0,
                origin_y: 0.0,
                abs_top_transform: 0.0,
            },
        );
        for e in render_edges {
            let e = e.as_ref();
            let root_id = {
                let _g = detail_guard(timing_enabled, &mut detail.viewbox_edge_curve_lca);
                lca_for_ids(
                    e.from.as_str(),
                    e.to.as_str(),
                    effective_parent_for_id,
                    &mut lca_scratch,
                )
                .unwrap_or("")
            };
            let off = {
                let _g = detail_guard(timing_enabled, &mut detail.viewbox_edge_curve_offsets);
                *root_offsets.entry(root_id).or_insert_with(|| {
                    flowchart_cluster_root_offsets(ctx, root_id).unwrap_or(FlowchartRootOffsets {
                        origin_x: 0.0,
                        origin_y: 0.0,
                        abs_top_transform: 0.0,
                    })
                })
            };

            let Some(geom) = ({
                detail.viewbox_edge_curve_geom_calls += 1;
                let _g = detail_guard(timing_enabled, &mut detail.viewbox_edge_curve_geom);
                flowchart_compute_edge_path_geom(
                    FlowchartEdgePathGeomRequest {
                        ctx,
                        edge: e,
                        origin_x: off.origin_x,
                        origin_y: off.origin_y,
                        abs_top_transform: off.abs_top_transform,
                        trace_enabled: false,
                        viewbox_current_bounds: Some((
                            bbox_min_x, bbox_min_y, bbox_max_x, bbox_max_y,
                        )),
                    },
                    &mut scratch,
                )
            }) else {
                continue;
            };
            if geom.bounds_skipped_for_viewbox {
                detail.viewbox_edge_curve_geom_skipped_bounds += 1;
            }

            {
                let _g = detail_guard(timing_enabled, &mut detail.viewbox_edge_curve_bbox_union);
                if let Some(pb) = geom.pb {
                    bbox_min_x = bbox_min_x.min(pb.min_x + off.origin_x);
                    bbox_min_y = bbox_min_y.min(pb.min_y + off.abs_top_transform);
                    bbox_max_x = bbox_max_x.max(pb.max_x + off.origin_x);
                    bbox_max_y = bbox_max_y.max(pb.max_y + off.abs_top_transform);
                }

                edge_path_cache.insert(
                    e.id.as_str(),
                    FlowchartEdgePathCacheEntry {
                        origin_x: off.origin_x,
                        origin_y: off.origin_y,
                        geom,
                    },
                );
            }
        }
    }

    // Mermaid centers the title using the pre-title `getBBox()` of the rendered root group.
    let title_anchor_x = (bbox_min_x + bbox_max_x) / 2.0;

    if let Some(title) = diagram_title.as_deref() {
        let title_style = TextStyle {
            font_family: Some(font_family.to_string()),
            font_size: TITLE_FONT_SIZE_PX,
            font_weight: None,
        };
        let (title_left, title_right) = ctx.measurer.measure_svg_title_bbox_x(title, &title_style);
        let baseline_y = -title_top_margin;
        let (ascent, descent) = crate::text::svg_title_bbox_vertical_extents_px(&title_style);

        bbox_min_x = bbox_min_x.min(title_anchor_x - title_left);
        bbox_max_x = bbox_max_x.max(title_anchor_x + title_right);
        bbox_min_y = bbox_min_y.min(baseline_y - ascent);
        bbox_max_y = bbox_max_y.max(baseline_y + descent);
    }

    FlowchartViewboxBounds {
        diagram_title,
        title_anchor_x,
        bbox_min_x,
        bbox_min_y,
        bbox_max_x,
        bbox_max_y,
    }
}

fn lca_for_ids<'a, F>(
    a: &str,
    b: &str,
    effective_parent_for_id: &F,
    scratch: &mut Vec<&'a str>,
) -> Option<&'a str>
where
    F: Fn(&str) -> Option<&'a str>,
{
    scratch.clear();
    let mut cur = effective_parent_for_id(a);
    while let Some(p) = cur {
        scratch.push(p);
        cur = effective_parent_for_id(p);
    }

    let mut cur = effective_parent_for_id(b);
    while let Some(p) = cur {
        if scratch.contains(&p) {
            return Some(p);
        }
        cur = effective_parent_for_id(p);
    }
    None
}

fn extra_recursive_root_y(ctx: &FlowchartRenderCtx<'_>) -> f64 {
    fn effective_parent<'a>(
        parent: &'a FxHashMap<&'a str, &'a str>,
        subgraphs_by_id: &'a FxHashMap<&'a str, &'a crate::flowchart::FlowSubgraph>,
        recursive_clusters: &FxHashSet<&'a str>,
        id: &str,
    ) -> Option<&'a str> {
        let mut cur = parent.get(id).copied();
        while let Some(p) = cur {
            if subgraphs_by_id.contains_key(p) && !recursive_clusters.contains(p) {
                cur = parent.get(p).copied();
                continue;
            }
            return Some(p);
        }
        None
    }

    let mut max_y: f64 = 0.0;
    for &cid in &ctx.recursive_clusters {
        let Some(cluster) = ctx.layout_clusters_by_id.get(cid) else {
            continue;
        };
        let my_parent = effective_parent(
            &ctx.parent,
            &ctx.subgraphs_by_id,
            &ctx.recursive_clusters,
            cid,
        );
        let has_empty_sibling = ctx.subgraphs_by_id.iter().any(|(&id, &sg)| {
            id != cid
                && sg.nodes.is_empty()
                && ctx.layout_clusters_by_id.contains_key(id)
                && effective_parent(
                    &ctx.parent,
                    &ctx.subgraphs_by_id,
                    &ctx.recursive_clusters,
                    id,
                ) == my_parent
        });
        if has_empty_sibling {
            max_y = max_y.max(cluster.offset_y.max(0.0) * 2.0);
        }
    }
    max_y
}
