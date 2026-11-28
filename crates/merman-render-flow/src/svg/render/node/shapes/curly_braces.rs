//! Flowchart v2 curly brace / comment shapes.

use crate::node_geometry::brace::{circle_points, curly_brace_comment_intersection_points};

use crate::svg::escape_attr;
#[cfg(feature = "layout-shoal")]
use crate::svg::path_bounds::svg_path_bounds_from_d;
use crate::svg::types::CurlyBracePaint;
use crate::svg::{fmt, fmt_display};

use super::super::geom::path_from_points;
use super::super::roughjs::{RoughGeometryTarget, roughjs_prepare_stroke_path_for_svg_path};
#[cfg(feature = "layout-shoal")]
use merman_render_core::rough_svg::{append_minor_circular_arc, ops_to_compact_svg_path_d};

pub(in crate::svg) struct CurlyBraceCommentGeometry {
    pub(in crate::svg) group_tx: f64,
    pub(in crate::svg) label_dx: f64,
    pub(in crate::svg) paths: Vec<CurlyBraceCommentPath>,
}

pub(in crate::svg) struct CurlyBraceCommentPath {
    pub(in crate::svg) d: String,
    pub(in crate::svg) visible: bool,
}

#[cfg(feature = "layout-shoal")]
impl CurlyBraceCommentPath {
    pub(in crate::svg) fn shoal_stroke_operations(
        &self,
        target: RoughGeometryTarget,
        work_meter: &merman_render_core::resources::OperationWorkMeter,
        randomness: &roughr::core::RoughRandomness,
    ) -> Option<roughr::core::OpSet<f64>> {
        merman_render_core::rough_svg::paths::roughjs_prepare_hand_drawn_stroke_path_for_svg_path(
            target, &self.d, 0.7, work_meter, randomness,
        )
    }
}

enum BraceLeg {
    Point((f64, f64)),
    Arc {
        x: f64,
        y: f64,
        samples: usize,
        start: f64,
        end: f64,
    },
}

#[cfg(feature = "layout-shoal")]
fn shoal_brace_path(legs: &[BraceLeg], radius: f64, negate: bool) -> String {
    use std::fmt::Write as _;

    let mut path = String::new();
    let mut last = None;
    let pen = |point: (f64, f64), last: &mut Option<(f64, f64)>, path: &mut String| {
        if last.is_some_and(|previous| (point.0 - previous.0).hypot(point.1 - previous.1) < 1e-9) {
            return;
        }
        let command = if path.is_empty() { 'M' } else { 'L' };
        let _ = write!(path, "{command}{} {}", fmt(point.0), fmt(point.1));
        *last = Some(point);
    };
    for leg in legs {
        match *leg {
            BraceLeg::Point(point) => pen(point, &mut last, &mut path),
            BraceLeg::Arc {
                x, y, start, end, ..
            } => {
                let point = |angle: f64| {
                    let (sin, cos) = angle.to_radians().sin_cos();
                    let (px, py) = (x + radius * cos, y + radius * sin);
                    if negate { (-px, -py) } else { (px, py) }
                };
                pen(point(start), &mut last, &mut path);
                let end_point = point(end);
                append_minor_circular_arc(&mut path, radius, end_point, end > start);
                last = Some(end_point);
            }
        }
    }
    path
}

fn brace_path(legs: &[BraceLeg], radius: f64, negate: bool) -> String {
    let mut points = Vec::new();
    for leg in legs {
        match *leg {
            BraceLeg::Point(point) => points.push(point),
            BraceLeg::Arc {
                x,
                y,
                samples,
                start,
                end,
            } => {
                points.extend(circle_points(x, y, radius, samples, start, end, negate));
            }
        }
    }
    path_from_points(&points).trim_end_matches('Z').to_owned()
}

pub(in crate::svg) fn curly_brace_comment_geometry(
    shape: &str,
    label_w: f64,
    label_h: f64,
    padding: f64,
    look_is_neo: bool,
    paint: CurlyBracePaint,
) -> CurlyBraceCommentGeometry {
    let padding = padding.max(0.0);
    let (w, h) = crate::node_geometry::flowchart_brace_content_dimensions(
        shape,
        label_w,
        label_h,
        padding,
        look_is_neo,
    );
    let radius = (h * 0.1).max(5.0);

    // Mermaid 12.4 centers the label on its measured box. The left brace still
    // offsets the label horizontally relative to the brace and its padding.
    let (group_tx, label_dx) = match shape {
        "comment" | "brace" | "brace-l" => (radius, (label_w - w) / 2.0 + radius),
        "brace-r" => (-radius, 0.0),
        "braces" => (radius - radius / 4.0, 0.0),
        _ => (0.0, 0.0),
    };

    let rect_points =
        curly_brace_comment_intersection_points(shape, label_w, label_h, padding, look_is_neo);
    let arc = |x, y, samples, start, end| BraceLeg::Arc {
        x,
        y,
        samples,
        start,
        end,
    };
    let point = |x, y| BraceLeg::Point((x, y));
    let brace = |legs: &[BraceLeg], negate| CurlyBraceCommentPath {
        d: match paint {
            CurlyBracePaint::Mermaid => brace_path(legs, radius, negate),
            #[cfg(feature = "layout-shoal")]
            CurlyBracePaint::ShoalHandDrawn => shoal_brace_path(legs, radius, negate),
        },
        visible: true,
    };
    let right = [
        arc(w / 2.0, -h / 2.0, 20, -90.0, 0.0),
        point(w / 2.0 + radius, -radius),
        arc(w / 2.0 + radius * 2.0, -radius, 20, -180.0, -270.0),
        arc(w / 2.0 + radius * 2.0, radius, 20, -90.0, -180.0),
        point(w / 2.0 + radius, h / 2.0),
        arc(w / 2.0, h / 2.0, 20, 0.0, 90.0),
    ];
    let left = [
        arc(w / 2.0, -h / 2.0, 30, -90.0, 0.0),
        point(-w / 2.0 - radius, radius),
        arc(w / 2.0 + radius * 2.0, -radius, 20, -180.0, -270.0),
        arc(w / 2.0 + radius * 2.0, radius, 20, -90.0, -180.0),
        point(-w / 2.0 - radius, -h / 2.0),
        arc(w / 2.0, h / 2.0, 20, 0.0, 90.0),
    ];
    let mut paths = if shape == "braces" {
        let opposing = [
            arc(
                -w / 2.0 + radius + radius / 2.0,
                -h / 2.0,
                20,
                -90.0,
                -180.0,
            ),
            point(w / 2.0 - radius / 2.0, radius),
            arc(-w / 2.0 - radius / 2.0, -radius, 20, 0.0, 90.0),
            arc(-w / 2.0 - radius / 2.0, radius, 20, -90.0, 0.0),
            point(w / 2.0 - radius / 2.0, -radius),
            arc(
                -w / 2.0 + radius + radius / 2.0,
                h / 2.0,
                30,
                -180.0,
                -270.0,
            ),
        ];
        // Upstream inserts each path at :first-child: right, left, then the hidden outline.
        vec![brace(&opposing, true), brace(&left, true)]
    } else if shape == "brace-r" {
        vec![brace(&right, false)]
    } else {
        vec![brace(&left, true)]
    };
    paths.push(CurlyBraceCommentPath {
        d: path_from_points(&rect_points),
        visible: false,
    });

    CurlyBraceCommentGeometry {
        group_tx,
        label_dx,
        paths,
    }
}

pub(in crate::svg::render::node) fn render_curly_brace_comment(
    out: &mut impl crate::svg::SvgOutput,
    ctx: &crate::svg::types::FlowchartRenderCtx<'_>,
    common: &super::super::FlowchartNodeRenderCommon<'_>,
    label: &mut super::super::FlowchartNodeLabelState<'_>,
    details: &mut crate::svg::types::FlowchartRenderDetails,
) {
    let metrics = super::super::helpers::compute_node_label_metrics(
        ctx,
        common.node_id,
        Some(common.layout_node),
        label.text,
        label.label_type,
        common.node_classes,
        common.node_styles,
    );
    let paint = ctx.curly_brace_paint();
    let geometry = curly_brace_comment_geometry(
        common.shape,
        metrics.width,
        metrics.height,
        ctx.node_padding,
        merman_render_core::config::mermaid_config_diagram_look(ctx.config).is_neo(),
        paint,
    );
    label.dx = geometry.label_dx;

    let mut stroke_d = |path: &CurlyBraceCommentPath| {
        super::super::helpers::timed_node_roughjs(common.timing, details, || match paint {
            CurlyBracePaint::Mermaid => roughjs_prepare_stroke_path_for_svg_path(
                RoughGeometryTarget::Svg,
                &path.d,
                common.stroke_width,
                common.stroke_dasharray,
                common.work_meter,
                common.hand_drawn_seed,
            )
            .as_ref()
            .map(merman_render_core::rough_svg::ops_to_svg_path_d),
            #[cfg(feature = "layout-shoal")]
            CurlyBracePaint::ShoalHandDrawn => path
                .shoal_stroke_operations(
                    RoughGeometryTarget::Svg,
                    common.work_meter,
                    common.hand_drawn_seed,
                )
                .as_ref()
                .map(ops_to_compact_svg_path_d),
        })
        .unwrap_or_else(|| "M0,0".to_string())
    };

    let _ = write!(
        out,
        r##"<g class="text" transform="translate({},0)">"##,
        fmt(geometry.group_tx),
    );
    for path in geometry.paths {
        #[cfg(feature = "layout-shoal")]
        if matches!(paint, CurlyBracePaint::ShoalHandDrawn) && !path.visible {
            // The contact polygon sets the hidden bounds. Only the braces need RoughJS paint.
            if let Some(bounds) = svg_path_bounds_from_d(&path.d) {
                let _ = write!(
                    out,
                    r#"<rect x="{}" y="{}" width="{}" height="{}" fill="none" stroke="none" visibility="hidden"/>"#,
                    fmt(bounds.min_x),
                    fmt(bounds.min_y),
                    fmt(bounds.max_x - bounds.min_x),
                    fmt(bounds.max_y - bounds.min_y),
                );
            }
            continue;
        }
        let d = stroke_d(&path);
        if path.visible {
            out.push_str("<g>");
        } else {
            out.push_str(r#"<g stroke-opacity="0">"#);
        }
        let _ = write!(
            out,
            r##"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/>"##,
            escape_attr(&d),
            escape_attr(common.stroke_color),
            fmt_display(common.stroke_width as f64),
            escape_attr(common.stroke_dasharray),
            escape_attr(common.style),
        );
        out.push_str("</g>");
    }
    out.push_str("</g>");
}

pub(super) fn advance_rough_geometry(
    ctx: &crate::svg::types::FlowchartRenderCtx<'_>,
    common: &super::super::FlowchartNodeGeometry<'_>,
    label: &super::super::FlowchartNodeLabelState<'_>,
) {
    let metrics = super::super::helpers::compute_node_label_metrics(
        ctx,
        common.node_id,
        Some(common.layout_node),
        label.text,
        label.label_type,
        common.node_classes,
        common.node_styles,
    );
    let paint = ctx.curly_brace_paint();
    let geometry = curly_brace_comment_geometry(
        common.shape,
        metrics.width,
        metrics.height,
        ctx.node_padding,
        merman_render_core::config::mermaid_config_diagram_look(ctx.config).is_neo(),
        paint,
    );
    for path in geometry.paths {
        match paint {
            CurlyBracePaint::Mermaid => {
                let _ = roughjs_prepare_stroke_path_for_svg_path(
                    RoughGeometryTarget::Query,
                    &path.d,
                    common.stroke_width,
                    common.stroke_dasharray,
                    common.work_meter,
                    common.hand_drawn_seed,
                );
            }
            #[cfg(feature = "layout-shoal")]
            CurlyBracePaint::ShoalHandDrawn => {
                if path.visible {
                    let _ = path.shoal_stroke_operations(
                        RoughGeometryTarget::Query,
                        common.work_meter,
                        common.hand_drawn_seed,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn union_translated_path_bounds(
        geometry: &CurlyBraceCommentGeometry,
    ) -> crate::svg::path_bounds::SvgPathBounds {
        let mut out: Option<crate::svg::path_bounds::SvgPathBounds> = None;
        for path in &geometry.paths {
            let mut pb =
                crate::svg::path_bounds::svg_path_bounds_from_d(&path.d).expect("path bounds");
            pb.min_x += geometry.group_tx;
            pb.max_x += geometry.group_tx;
            out = Some(match out {
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
        out.expect("non-empty geometry")
    }

    #[test]
    fn brace_geometry_matches_layout_and_source_label_transforms() {
        for (shape, neo_dx) in [
            ("brace", -4.0),
            ("brace-l", -4.0),
            ("comment", -4.0),
            ("brace-r", 0.0),
            ("braces", 0.0),
        ] {
            let geometry = curly_brace_comment_geometry(
                shape,
                100.0,
                20.0,
                15.0,
                true,
                CurlyBracePaint::Mermaid,
            );
            assert_eq!(geometry.label_dx, neo_dx, "{shape}");
            let bounds = union_translated_path_bounds(&geometry);
            let layout = crate::node_geometry::flowchart_node_render_dimensions(
                Some(shape),
                merman_render_core::text::TextMetrics {
                    width: 100.0,
                    height: 20.0,
                    line_count: 1,
                },
                15.0,
                true,
            );
            assert!(
                (bounds.max_x - bounds.min_x - layout.0).abs() < 1e-9,
                "{shape}"
            );
            assert!(
                (bounds.max_y - bounds.min_y - layout.1).abs() < 1e-9,
                "{shape}"
            );
            for padding in [0.0, -2.0] {
                let empty = curly_brace_comment_geometry(
                    shape,
                    0.0,
                    0.0,
                    padding,
                    false,
                    CurlyBracePaint::Mermaid,
                );
                let bounds = union_translated_path_bounds(&empty);
                let width = match shape {
                    "brace-r" => 10.0,
                    "braces" => 12.5,
                    _ => 15.0,
                };
                assert!(
                    (bounds.max_x - bounds.min_x - width).abs() < 1e-9,
                    "{shape}"
                );
                assert!((bounds.max_y - bounds.min_y - 10.0).abs() < 1e-9, "{shape}");
            }
        }
        // A tall classic label increases radius beyond the default 5px.
        // Left-brace text follows r - padding/2, not the historical -r/2.
        let tall = curly_brace_comment_geometry(
            "brace",
            100.0,
            85.0,
            15.0,
            false,
            CurlyBracePaint::Mermaid,
        );
        assert_eq!(tall.label_dx, 2.5);
    }

    #[test]
    fn paired_braces_follow_source_dom_insertion_order() {
        let geometry = curly_brace_comment_geometry(
            "braces",
            100.0,
            20.0,
            15.0,
            true,
            CurlyBracePaint::Mermaid,
        );
        assert_eq!(geometry.paths.len(), 3);
        let bounds = |index: usize| {
            crate::svg::path_bounds::svg_path_bounds_from_d(&geometry.paths[index].d)
                .expect("path bounds")
        };
        assert!(geometry.paths[0].visible);
        assert!(geometry.paths[1].visible);
        assert!(!geometry.paths[2].visible);
        assert!(
            bounds(0).min_x > 0.0,
            "the right brace is inserted last and painted first"
        );
        assert!(
            bounds(1).max_x < 0.0,
            "the left brace follows the right brace"
        );
    }

    #[test]
    fn brace_r_geometry_uses_label_box_not_updated_layout_box() {
        let label_w = 198.320_312_5;
        let label_h = 54.2;
        let padding = 15.0;
        let geometry = curly_brace_comment_geometry(
            "brace-r",
            label_w,
            label_h,
            padding,
            false,
            CurlyBracePaint::Mermaid,
        );

        assert_eq!(geometry.paths.len(), 2);
        assert!(geometry.paths[0].visible);
        assert!(!geometry.paths[1].visible);
        assert!((geometry.group_tx + 6.92).abs() < 1e-9);

        let bounds = union_translated_path_bounds(&geometry);
        let expected_width = label_w + padding + 2.0 * 6.92;
        assert!(((bounds.max_x - bounds.min_x) - expected_width).abs() < 1e-9);
    }
}
