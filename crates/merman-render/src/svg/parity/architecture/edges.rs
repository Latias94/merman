use std::fmt::Write as _;

use crate::generated::architecture_text_overrides_11_12_2 as architecture_text_overrides;
use crate::model::Bounds;
use crate::text::{TextMeasurer, VendoredFontMetricsTextMeasurer};

use super::super::{escape_xml, fmt};
use super::geometry::{
    arrow_points, arrow_shift, bounds_from_rect, edge_id, extend_bounds, is_arch_dir_x,
    is_arch_dir_y,
};
use super::labels::{svg_line_plain_text, wrap_svg_words_to_lines, write_svg_text_lines};
use super::model::ArchitectureModelAccess;
use super::settings::ArchitectureRenderSettings;
use crate::model::ArchitectureDiagramLayout;

pub(super) struct ArchitectureEdgeRenderContext<'a, M: ArchitectureModelAccess> {
    pub(super) out: &'a mut String,
    pub(super) layout: &'a ArchitectureDiagramLayout,
    pub(super) model: &'a M,
    pub(super) node_xy: &'a rustc_hash::FxHashMap<&'a str, (f64, f64)>,
    pub(super) settings: &'a ArchitectureRenderSettings,
    pub(super) text_measurer: &'a VendoredFontMetricsTextMeasurer,
    pub(super) content_bounds: &'a mut Option<Bounds>,
    pub(super) junction_bounds: &'a rustc_hash::FxHashMap<&'a str, Bounds>,
}

pub(super) fn push_architecture_edges<M: ArchitectureModelAccess>(
    ctx: &mut ArchitectureEdgeRenderContext<'_, M>,
) {
    let out = &mut *ctx.out;
    let layout = ctx.layout;
    let model = ctx.model;
    let node_xy = ctx.node_xy;
    let settings = ctx.settings;
    let text_measurer = ctx.text_measurer;
    let content_bounds = &mut *ctx.content_bounds;
    let junction_bounds = ctx.junction_bounds;

    let group_edge_shift = settings.padding_px + 4.0;
    let group_edge_label_bottom_px =
        architecture_text_overrides::architecture_service_label_bottom_extension_px();
    let is_junction = |id: &str| junction_bounds.contains_key(id);

    let layout_edge_points: Vec<(f64, f64, f64, f64, f64, f64)> = layout
        .edges
        .iter()
        .map(|e| {
            // Architecture layout edges are expected to be 3-point polylines.
            // Be defensive and fall back to zeros if the snapshot is malformed.
            let p0 = e.points.first().map(|p| (p.x, p.y)).unwrap_or((0.0, 0.0));
            let pm = e.points.get(1).map(|p| (p.x, p.y)).unwrap_or((0.0, 0.0));
            let p2 = e.points.last().map(|p| (p.x, p.y)).unwrap_or((0.0, 0.0));
            (p0.0, p0.1, pm.0, pm.1, p2.0, p2.1)
        })
        .collect();

    let edge_points = |edge_idx: usize,
                       edge: super::model::ArchitectureEdgeRef<'_>|
     -> (f64, f64, f64, f64, f64, f64) {
        // Prefer layout-provided points: this is where we model Mermaid/Cytoscape edge routing.
        //
        // The layout points represent raw Cytoscape endpoints; Mermaid applies group/junction
        // endpoint shifts later, during SVG emission.
        let (raw_start_x, raw_start_y, mid_x, mid_y, raw_end_x, raw_end_y) = layout_edge_points
            .get(edge_idx)
            .copied()
            .unwrap_or_else(|| {
                let (sx, sy) = node_xy.get(edge.lhs_id).copied().unwrap_or((0.0, 0.0));
                let (tx, ty) = node_xy.get(edge.rhs_id).copied().unwrap_or((0.0, 0.0));

                let (sx, sy) = match edge.lhs_dir {
                    'L' => (sx, sy + settings.half_icon),
                    'R' => (sx + settings.icon_size_px, sy + settings.half_icon),
                    'T' => (sx + settings.half_icon, sy),
                    'B' => (sx + settings.half_icon, sy + settings.icon_size_px),
                    _ => (sx + settings.half_icon, sy + settings.half_icon),
                };
                let (tx, ty) = match edge.rhs_dir {
                    'L' => (tx, ty + settings.half_icon),
                    'R' => (tx + settings.icon_size_px, ty + settings.half_icon),
                    'T' => (tx + settings.half_icon, ty),
                    'B' => (tx + settings.half_icon, ty + settings.icon_size_px),
                    _ => (tx + settings.half_icon, ty + settings.half_icon),
                };

                let (mx, my) = if (sx - tx).abs() > 1e-6 && (sy - ty).abs() > 1e-6 {
                    // Match upstream Mermaid: choose the bend based on the *source* dir.
                    if is_arch_dir_y(edge.lhs_dir) {
                        (sx, ty)
                    } else {
                        (tx, sy)
                    }
                } else {
                    ((sx + tx) / 2.0, (sy + ty) / 2.0)
                };
                (sx, sy, mx, my, tx, ty)
            });

        let mut start_x = raw_start_x;
        let mut start_y = raw_start_y;
        let mut end_x = raw_end_x;
        let mut end_y = raw_end_y;

        let lhs_group = edge.lhs_group.unwrap_or(false);
        if lhs_group {
            if is_arch_dir_x(edge.lhs_dir) {
                start_x += if edge.lhs_dir == 'L' {
                    -group_edge_shift
                } else {
                    group_edge_shift
                };
            } else {
                start_y += if edge.lhs_dir == 'T' {
                    -group_edge_shift
                } else {
                    group_edge_shift + group_edge_label_bottom_px
                };
            }
        }
        if !lhs_group && is_junction(edge.lhs_id) {
            if is_arch_dir_x(edge.lhs_dir) {
                start_x += if edge.lhs_dir == 'L' {
                    settings.half_icon
                } else {
                    -settings.half_icon
                };
            } else {
                start_y += if edge.lhs_dir == 'T' {
                    settings.half_icon
                } else {
                    -settings.half_icon
                };
            }
        }

        let rhs_group = edge.rhs_group.unwrap_or(false);
        if rhs_group {
            if is_arch_dir_x(edge.rhs_dir) {
                end_x += if edge.rhs_dir == 'L' {
                    -group_edge_shift
                } else {
                    group_edge_shift
                };
            } else {
                end_y += if edge.rhs_dir == 'T' {
                    -group_edge_shift
                } else {
                    group_edge_shift + group_edge_label_bottom_px
                };
            }
        }
        if !rhs_group && is_junction(edge.rhs_id) {
            if is_arch_dir_x(edge.rhs_dir) {
                end_x += if edge.rhs_dir == 'L' {
                    settings.half_icon
                } else {
                    -settings.half_icon
                };
            } else {
                end_y += if edge.rhs_dir == 'T' {
                    settings.half_icon
                } else {
                    -settings.half_icon
                };
            }
        }

        (start_x, start_y, mid_x, mid_y, end_x, end_y)
    };

    // Edges (including conservative label bounds).
    if model.edges_len() != 0 {
        let arrow_size = settings.icon_size_px / 6.0;
        let half_arrow_size = arrow_size / 2.0;
        for (edge_idx, edge) in model.edges().enumerate() {
            let (start_x, start_y, mid_x, mid_y, end_x, end_y) = edge_points(edge_idx, edge);

            extend_bounds(
                content_bounds,
                Bounds::from_points(vec![(start_x, start_y), (mid_x, mid_y), (end_x, end_y)])
                    .unwrap_or(Bounds {
                        min_x: start_x,
                        min_y: start_y,
                        max_x: end_x,
                        max_y: end_y,
                    }),
            );

            if edge.lhs_into == Some(true) {
                let x_shift = if is_arch_dir_x(edge.lhs_dir) {
                    arrow_shift(edge.lhs_dir, start_x, arrow_size)
                } else {
                    start_x - half_arrow_size
                };
                let y_shift = if is_arch_dir_y(edge.lhs_dir) {
                    arrow_shift(edge.lhs_dir, start_y, arrow_size)
                } else {
                    start_y - half_arrow_size
                };
                extend_bounds(
                    content_bounds,
                    bounds_from_rect(x_shift, y_shift, arrow_size, arrow_size),
                );
            }

            if edge.rhs_into == Some(true) {
                let x_shift = if is_arch_dir_x(edge.rhs_dir) {
                    arrow_shift(edge.rhs_dir, end_x, arrow_size)
                } else {
                    end_x - half_arrow_size
                };
                let y_shift = if is_arch_dir_y(edge.rhs_dir) {
                    arrow_shift(edge.rhs_dir, end_y, arrow_size)
                } else {
                    end_y - half_arrow_size
                };
                extend_bounds(
                    content_bounds,
                    bounds_from_rect(x_shift, y_shift, arrow_size, arrow_size),
                );
            }

            if let Some(label) = edge.title.map(str::trim).filter(|t| !t.is_empty()) {
                let axis = match (is_arch_dir_x(edge.lhs_dir), is_arch_dir_x(edge.rhs_dir)) {
                    (true, true) => "X",
                    (false, false) => "Y",
                    _ => "XY",
                };

                let wrap_width = match axis {
                    "X" => (start_x - end_x).abs(),
                    "Y" => (start_y - end_y).abs() / 1.5,
                    _ => (start_x - end_x).abs() / 2.0,
                };
                let wrap_width = if wrap_width.is_finite() && wrap_width > 0.0 {
                    wrap_width
                } else {
                    architecture_text_overrides::architecture_create_text_default_wrap_width_px()
                };
                let lines =
                    wrap_svg_words_to_lines(label, wrap_width, text_measurer, &settings.text_style);

                let mut bbox_w = 0.0f64;
                for line in &lines {
                    let s = svg_line_plain_text(line);
                    let m = text_measurer.measure_wrapped(
                        s.as_str(),
                        &settings.text_style,
                        None,
                        crate::text::WrapMode::SvgLike,
                    );
                    bbox_w = bbox_w.max(m.width);
                }
                let line_count = lines.len().max(1);
                let bbox_h = architecture_text_overrides::architecture_create_text_bbox_height_px(
                    settings.svg_font_size_px,
                    line_count,
                );

                // AABB for rotated labels (90°/45° variants). Mermaid rotates Architecture edge
                // labels depending on the edge direction; mimic Chromium `getBBox()`-like bounds
                // by projecting the (w,h) label box into the axes.
                let (aabb_w, aabb_h) = match axis {
                    "X" => (bbox_w, bbox_h),
                    "Y" => (bbox_h, bbox_w),
                    _ => {
                        // |cos(45°)| == |sin(45°)| == sqrt(1/2)
                        let a = (bbox_w + bbox_h) * std::f64::consts::FRAC_1_SQRT_2;
                        (a, a)
                    }
                };
                let aabb_w = aabb_w.max(1.0);
                let aabb_h = aabb_h.max(1.0);
                extend_bounds(
                    content_bounds,
                    bounds_from_rect(mid_x - aabb_w / 2.0, mid_y - aabb_h / 2.0, aabb_w, aabb_h),
                );
            }

            out.push_str("<g>");
            let id = edge_id("L", edge.lhs_id, edge.rhs_id, 0);
            let _ = write!(
                out,
                r#"<path d="M {sx},{sy} L {mx},{my} L{ex},{ey} " class="edge" id="{id}"/>"#,
                sx = fmt(start_x),
                sy = fmt(start_y),
                mx = fmt(mid_x),
                my = fmt(mid_y),
                ex = fmt(end_x),
                ey = fmt(end_y),
                id = escape_xml(&id)
            );

            if edge.lhs_into == Some(true) {
                let x_shift = if is_arch_dir_x(edge.lhs_dir) {
                    arrow_shift(edge.lhs_dir, start_x, arrow_size)
                } else {
                    start_x - half_arrow_size
                };
                let y_shift = if is_arch_dir_y(edge.lhs_dir) {
                    arrow_shift(edge.lhs_dir, start_y, arrow_size)
                } else {
                    start_y - half_arrow_size
                };
                let _ = write!(
                    out,
                    r#"<polygon points="{pts}" transform="translate({x},{y})" class="arrow"/>"#,
                    pts = arrow_points(edge.lhs_dir, arrow_size),
                    x = fmt(x_shift),
                    y = fmt(y_shift)
                );
            }

            if edge.rhs_into == Some(true) {
                let x_shift = if is_arch_dir_x(edge.rhs_dir) {
                    arrow_shift(edge.rhs_dir, end_x, arrow_size)
                } else {
                    end_x - half_arrow_size
                };
                let y_shift = if is_arch_dir_y(edge.rhs_dir) {
                    arrow_shift(edge.rhs_dir, end_y, arrow_size)
                } else {
                    end_y - half_arrow_size
                };
                let _ = write!(
                    out,
                    r#"<polygon points="{pts}" transform="translate({x},{y})" class="arrow"/>"#,
                    pts = arrow_points(edge.rhs_dir, arrow_size),
                    x = fmt(x_shift),
                    y = fmt(y_shift)
                );
            }

            if let Some(label) = edge.title.map(str::trim).filter(|t| !t.is_empty()) {
                let axis = match (is_arch_dir_x(edge.lhs_dir), is_arch_dir_x(edge.rhs_dir)) {
                    (true, true) => "X",
                    (false, false) => "Y",
                    _ => "XY",
                };

                // Mermaid@11.12.2 sets the label wrapping width based on the edge axis.
                let wrap_width = match axis {
                    "X" => (start_x - end_x).abs(),
                    "Y" => (start_y - end_y).abs() / 1.5,
                    _ => (start_x - end_x).abs() / 2.0,
                };
                let wrap_width = if wrap_width.is_finite() && wrap_width > 0.0 {
                    wrap_width
                } else {
                    architecture_text_overrides::architecture_create_text_default_wrap_width_px()
                };
                let lines =
                    wrap_svg_words_to_lines(label, wrap_width, text_measurer, &settings.text_style);

                // Mermaid's XY label placement uses `getBoundingClientRect()` in the browser and
                // composes a multi-step transform. Approximate the bbox headlessly so the DOM
                // structure matches the upstream SVG baseline.
                let mut bbox_w = 0.0f64;
                for line in &lines {
                    let s = svg_line_plain_text(line);
                    let w = text_measurer.measure_wrapped(
                        s.as_str(),
                        &settings.text_style,
                        None,
                        crate::text::WrapMode::SvgLike,
                    );
                    bbox_w = bbox_w.max(w.width);
                }
                // Mirror Chromium `getBBox()`-like label height for parity-driven transforms.
                let line_count = lines.len().max(1);
                let bbox_h = architecture_text_overrides::architecture_create_text_bbox_height_px(
                    settings.text_style.font_size,
                    line_count,
                );
                let half_bbox_h = bbox_h / 2.0;

                let (dominant_baseline, transform) = match axis {
                    "Y" => (
                        "middle",
                        format!(r#"translate({}, {}) rotate(-90)"#, fmt(mid_x), fmt(mid_y)),
                    ),
                    "XY" => {
                        let pair = format!("{}{}", edge.lhs_dir, edge.rhs_dir);
                        let (xf, yf): (f64, f64) = match pair.as_str() {
                            "LT" | "TL" => (1.0, 1.0),
                            "BL" | "LB" => (1.0, -1.0),
                            "BR" | "RB" => (-1.0, -1.0),
                            _ => (-1.0, 1.0),
                        };
                        let angle = (-xf * yf * 45.0f64).round() as i64;

                        // Rotated bbox at 45° (w' == h' == (w+h)*sqrt(2)/2).
                        let diag = (bbox_w + bbox_h) * std::f64::consts::FRAC_1_SQRT_2;
                        let t2x = xf * diag / 2.0;
                        let t2y = yf * diag / 2.0;
                        // Mermaid CLI serializes newline characters inside attribute values as
                        // XML entities (`&#10;`). Emit those explicitly so our SVG matches the
                        // upstream baselines.
                        let sep = "&#10;";

                        (
                            "auto",
                            format!(
                                "translate({}, {}){sep}                translate({}, {}){sep}                rotate({}, 0, {})",
                                fmt(mid_x),
                                fmt(mid_y - half_bbox_h),
                                fmt(t2x),
                                fmt(t2y),
                                angle,
                                fmt(half_bbox_h),
                                sep = sep
                            ),
                        )
                    }
                    _ => (
                        "middle",
                        format!(r#"translate({}, {})"#, fmt(mid_x), fmt(mid_y)),
                    ),
                };

                let _ = write!(
                    out,
                    r#"<g dy="1em" alignment-baseline="middle" dominant-baseline="{baseline}" text-anchor="middle" transform="{transform}">"#,
                    baseline = dominant_baseline,
                    transform = transform
                );
                out.push_str(r#"<g><rect class="background" style="stroke: none"/>"#);
                write_svg_text_lines(out, &lines);
                out.push_str("</g></g>");
            }

            out.push_str("</g>");
        }
    }
}
