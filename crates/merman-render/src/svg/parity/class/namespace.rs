use crate::model::{Bounds, LayoutCluster};
use crate::text::MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX;
use std::collections::HashMap;
use std::fmt::Write as _;

use super::super::{escape_attr_display, escape_xml_display, fmt};
use super::bounds::include_xywh;

#[derive(Debug, Clone, Copy)]
pub(super) struct ClassNamespaceClusterGroupContext<'a> {
    pub diagram_id: &'a str,
    pub content_tx: f64,
    pub content_ty: f64,
    pub bounds_dx: f64,
    pub bounds_dy: f64,
    pub look: &'a str,
    pub timing_enabled: bool,
}

pub(super) fn render_class_namespace_cluster_group(
    out: &mut String,
    content_bounds: &mut Option<Bounds>,
    clusters: &[LayoutCluster],
    ctx: ClassNamespaceClusterGroupContext<'_>,
) -> web_time::Duration {
    let clusters_start = ctx.timing_enabled.then(web_time::Instant::now);
    out.push_str(r#"<g class="clusters">"#);
    for c in clusters {
        let w = c.width.max(1.0);
        let h = c.height.max(1.0);
        let left = c.x - w / 2.0 + ctx.content_tx;
        let top = c.y - h / 2.0 + ctx.content_ty;
        include_xywh(
            content_bounds,
            left + ctx.bounds_dx,
            top + ctx.bounds_dy,
            w,
            h,
        );

        let label_w = c.title_label.width.max(0.0);
        let label_h = 24.0;
        let label_x = left + (w - label_w) / 2.0;
        let label_y = top + c.title_margin_top;
        include_xywh(
            content_bounds,
            label_x + ctx.bounds_dx,
            label_y + ctx.bounds_dy,
            label_w,
            label_h,
        );

        let _ = write!(
            out,
            r#"<g class="cluster undefined" id="{}-{}" data-look="{}"><rect x="{}" y="{}" width="{}" height="{}" style=""/><g class="cluster-label" transform="translate({}, {})"><foreignObject width="{}" height="24"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {}px; text-align: center;"><span class="nodeLabel"><p>{}</p></span></div></foreignObject></g></g>"#,
            escape_attr_display(ctx.diagram_id),
            escape_attr_display(&c.id),
            escape_attr_display(ctx.look),
            fmt(left),
            fmt(top),
            fmt(w),
            fmt(h),
            fmt(label_x),
            fmt(label_y),
            fmt(label_w),
            MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX,
            escape_xml_display(&c.title)
        );
    }
    out.push_str("</g>");
    clusters_start
        .map(|start| start.elapsed())
        .unwrap_or_default()
}

pub(super) fn class_namespace_root_offset(c: &LayoutCluster) -> (f64, f64) {
    let w = c.width.max(1.0);
    let h = c.height.max(1.0);
    (c.x - w / 2.0 - 8.0, c.y - h / 2.0)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_class_namespace_clusters_in_root(
    out: &mut String,
    content_bounds: &mut Option<Bounds>,
    clusters_by_id: &HashMap<&str, &LayoutCluster>,
    cluster_ids: &[&str],
    ctx: ClassNamespaceClusterGroupContext<'_>,
    root_ns_id: &str,
    root_dx: f64,
    root_dy: f64,
) {
    out.push_str(r#"<g class="clusters">"#);
    for ns_id in cluster_ids {
        let c = clusters_by_id
            .get(ns_id)
            .copied()
            .expect("validated Class render cluster id");

        let w = c.width.max(1.0);
        let h = c.height.max(1.0);
        let (left, top) = if *ns_id == root_ns_id {
            (8.0, 8.0)
        } else {
            (
                c.x - w / 2.0 - root_dx,
                c.y - h / 2.0 + ctx.content_ty - root_dy,
            )
        };
        include_xywh(
            content_bounds,
            left + root_dx + ctx.bounds_dx,
            top + root_dy + ctx.bounds_dy,
            w,
            h,
        );

        let label_w = c.title_label.width.max(0.0);
        let label_h = 24.0;
        let label_x = left + (w - label_w) / 2.0;
        let label_y = top + c.title_margin_top;
        include_xywh(
            content_bounds,
            label_x + root_dx + ctx.bounds_dx,
            label_y + root_dy + ctx.bounds_dy,
            label_w,
            label_h,
        );

        let _ = write!(
            out,
            r#"<g class="cluster undefined" id="{}-{}" data-look="{}"><rect x="{}" y="{}" width="{}" height="{}" style=""/><g class="cluster-label" transform="translate({}, {})"><foreignObject width="{}" height="24"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {}px; text-align: center;"><span class="nodeLabel"><p>{}</p></span></div></foreignObject></g></g>"#,
            escape_attr_display(ctx.diagram_id),
            escape_attr_display(&c.id),
            escape_attr_display(ctx.look),
            fmt(left),
            fmt(top),
            fmt(w),
            fmt(h),
            fmt(label_x),
            fmt(label_y),
            fmt(label_w),
            MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX,
            escape_xml_display(&c.title)
        );
    }
    out.push_str("</g>");
}
