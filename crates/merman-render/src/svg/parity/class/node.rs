use crate::model::{Bounds, LayoutNode};
use std::fmt::Write as _;
use std::time::Duration;

use super::super::{escape_attr_display, fmt, fmt_into};
use super::ClassSvgNode;
use super::bounds::{include_path_bounds, include_xywh};
use super::rough::{class_rough_rect_stroke_path_and_bounds, class_rough_seed};

#[derive(Debug, Clone, Copy)]
pub(super) struct ClassNodeRenderPosition {
    pub node_tx: f64,
    pub node_ty: f64,
    pub node_bounds_tx: f64,
    pub node_bounds_ty: f64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ClassNodeBoxGeometry {
    pub w: f64,
    pub h: f64,
    pub left: f64,
    pub rough_seed: u64,
}

pub(super) struct ClassNodeRenderState<'a> {
    pub out: &'a mut String,
    pub content_bounds: &'a mut Option<Bounds>,
}

pub(super) struct ClassNodeBasicContainerContext<'a> {
    pub diagram_id: &'a str,
    pub node_style_attr: &'a str,
    pub node_fill: &'a str,
    pub node_stroke: &'a str,
    pub node_stroke_width: &'a str,
    pub node_stroke_dasharray: &'a str,
    pub timing_enabled: bool,
}

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct ClassNodeRenderStats {
    pub path_bounds: Duration,
    pub path_bounds_calls: usize,
}

pub(super) struct ClassNodeBasicContainerResult {
    pub geometry: ClassNodeBoxGeometry,
    pub stats: ClassNodeRenderStats,
}

pub(super) fn render_class_node_shell_open(
    out: &mut String,
    node: &ClassSvgNode,
    position: ClassNodeRenderPosition,
) -> bool {
    let tooltip = node.tooltip.as_deref().unwrap_or("").trim();
    let has_tooltip = !tooltip.is_empty();

    let link = node
        .link
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let include_href = link.is_some_and(|s| {
        let lower = s.to_ascii_lowercase();
        !lower.starts_with("javascript:") && lower != "about:blank"
    });
    let have_callback = node.have_callback;

    if let Some(link) = link {
        out.push_str("<a");
        if include_href {
            out.push_str(r#" xlink:href=""#);
            super::super::util::escape_attr_into(out, link);
            out.push('"');
        }
        if have_callback {
            out.push_str(r#" class="null clickable""#);
        }
        out.push_str(r#" transform="translate("#);
        fmt_into(out, position.node_tx);
        out.push_str(", ");
        fmt_into(out, position.node_ty);
        out.push_str(r#")">"#);
    }

    out.push_str(r#"<g class=""#);
    out.push_str("node ");
    super::super::util::escape_attr_into(out, node.css_classes.trim());
    out.push_str(r#"" id=""#);
    super::super::util::escape_attr_into(out, &node.dom_id);
    out.push('"');
    if has_tooltip {
        out.push_str(r#" title=""#);
        super::super::util::escape_attr_into(out, tooltip);
        out.push('"');
    }
    if link.is_none() {
        out.push_str(r#" transform="translate("#);
        fmt_into(out, position.node_tx);
        out.push_str(", ");
        fmt_into(out, position.node_ty);
        out.push_str(r#")""#);
    }
    out.push('>');

    link.is_some()
}

pub(super) fn render_class_node_basic_container(
    state: ClassNodeRenderState<'_>,
    node: &ClassSvgNode,
    layout_node: &LayoutNode,
    position: ClassNodeRenderPosition,
    ctx: &ClassNodeBasicContainerContext<'_>,
) -> ClassNodeBasicContainerResult {
    let out = &mut *state.out;
    let content_bounds = &mut *state.content_bounds;
    let mut stats = ClassNodeRenderStats::default();

    out.push_str(r#"<g class="basic label-container">"#);
    let w = layout_node.width.max(1.0);
    let h = layout_node.height.max(1.0);
    let left = -w / 2.0;
    let top = -h / 2.0;
    let rough_seed = class_rough_seed(ctx.diagram_id, &node.dom_id);
    let _ = write!(
        out,
        r#"<path d="M{} {} L{} {} L{} {} L{} {}" stroke="none" stroke-width="0" fill="{}" style="{}"/>"#,
        fmt(left),
        fmt(top),
        fmt(left + w),
        fmt(top),
        fmt(left + w),
        fmt(top + h),
        fmt(left),
        fmt(top + h),
        escape_attr_display(ctx.node_fill),
        escape_attr_display(ctx.node_style_attr)
    );
    let (stroke_d, stroke_pb) =
        class_rough_rect_stroke_path_and_bounds(left, top, w, h, rough_seed);
    include_xywh(
        content_bounds,
        position.node_bounds_tx + left,
        position.node_bounds_ty + top,
        w,
        h,
    );
    let path_bounds_start = ctx.timing_enabled.then(std::time::Instant::now);
    include_path_bounds(
        content_bounds,
        &stroke_pb,
        position.node_bounds_tx,
        position.node_bounds_ty,
    );
    if let Some(s) = path_bounds_start {
        stats.path_bounds += s.elapsed();
        stats.path_bounds_calls += 1;
    }
    let _ = write!(
        out,
        r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/>"#,
        escape_attr_display(&stroke_d),
        escape_attr_display(ctx.node_stroke),
        escape_attr_display(ctx.node_stroke_width),
        escape_attr_display(ctx.node_stroke_dasharray),
        escape_attr_display(ctx.node_style_attr),
    );
    out.push_str("</g>");

    ClassNodeBasicContainerResult {
        geometry: ClassNodeBoxGeometry {
            w,
            h,
            left,
            rough_seed,
        },
        stats,
    }
}
