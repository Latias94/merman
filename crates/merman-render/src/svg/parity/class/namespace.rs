use crate::model::{Bounds, LayoutCluster};
use crate::text::MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX;
use std::collections::HashMap;

use super::super::SvgDiagramId;
use super::super::timing::RenderTiming;
use super::super::{SvgOutput, escape_attr_display, escape_xml_display, fmt};
use super::bounds::include_xywh;
use super::context::ClassEmitCheckpoint;
use super::label::class_math_html_label;
use crate::Result;

#[derive(Clone, Copy)]
pub(super) struct ClassNamespaceClusterGroupContext<'a> {
    pub diagram_id: SvgDiagramId<'a>,
    pub relation_theme: &'a crate::class::ClassRelationThemePlan,
    pub content_tx: f64,
    pub content_ty: f64,
    pub bounds_dx: f64,
    pub bounds_dy: f64,
    pub look: &'a str,
    pub mermaid_config: Option<&'a merman_core::MermaidConfig>,
    pub math_renderer: Option<&'a (dyn crate::math::MathRenderer + Send + Sync)>,
    pub timing: RenderTiming,
    pub emit: ClassEmitCheckpoint<'a>,
}

pub(super) fn render_class_namespace_cluster_group(
    out: &mut impl SvgOutput,
    content_bounds: &mut Option<Bounds>,
    clusters: &[LayoutCluster],
    ctx: ClassNamespaceClusterGroupContext<'_>,
    theme_receipt: &mut crate::class::ClassRelationThemeReceipt,
    typography_receipt: &mut Option<crate::class::ClassTypographyThemeReceipt>,
) -> crate::Result<std::time::Duration> {
    let clusters_start = ctx.timing.start();
    out.push_str(r#"<g class="clusters">"#);
    out.checkpoint()?;
    for c in clusters {
        let typography =
            render_class_namespace_cluster(out, content_bounds, c, ctx, theme_receipt)?;
        if let Some(receipt) = typography_receipt.as_mut() {
            receipt.record_namespace(&c.id, typography);
        }
    }
    out.push_str("</g>");
    out.checkpoint()?;
    Ok(clusters_start
        .map(|start| start.elapsed())
        .unwrap_or_default())
}

fn render_class_namespace_cluster(
    out: &mut impl SvgOutput,
    content_bounds: &mut Option<Bounds>,
    cluster: &LayoutCluster,
    ctx: ClassNamespaceClusterGroupContext<'_>,
    theme_receipt: &mut crate::class::ClassRelationThemeReceipt,
) -> Result<crate::class::ClassTypographyTerminalFacts> {
    render_class_namespace_cluster_at(
        out,
        content_bounds,
        cluster,
        (
            cluster.x - cluster.width.max(1.0) / 2.0 + ctx.content_tx,
            cluster.y - cluster.height.max(1.0) / 2.0 + ctx.content_ty,
        ),
        ctx,
        theme_receipt,
    )
}

fn render_class_namespace_cluster_at(
    out: &mut impl SvgOutput,
    content_bounds: &mut Option<Bounds>,
    cluster: &LayoutCluster,
    (left, top): (f64, f64),
    ctx: ClassNamespaceClusterGroupContext<'_>,
    theme_receipt: &mut crate::class::ClassRelationThemeReceipt,
) -> Result<crate::class::ClassTypographyTerminalFacts> {
    let w = cluster.width.max(1.0);
    let h = cluster.height.max(1.0);
    include_xywh(
        content_bounds,
        left + ctx.bounds_dx,
        top + ctx.bounds_dy,
        w,
        h,
    );

    let label_w = cluster.title_label.width.max(0.0);
    let label_h = 24.0;
    let label_x = left + (w - label_w) / 2.0;
    let label_y = top + cluster.title_margin_top;
    include_xywh(
        content_bounds,
        label_x + ctx.bounds_dx,
        label_y + ctx.bounds_dy,
        label_w,
        label_h,
    );

    let (title_html, typography, inherits_paint) = class_namespace_title_html(&cluster.title, ctx);
    let terminal_style = ctx.relation_theme.cluster_terminal_style();
    out.push_str(r#"<g class="cluster undefined" id=""#);
    let _ = write!(out, "{}", ctx.diagram_id);
    ctx.emit.checkpoint()?;
    let _ = write!(
        out,
        r#"-{}" data-look="{}"><rect x="{}" y="{}" width="{}" height="{}" style="{}"/><g class="cluster-label" transform="translate({}, {})"><foreignObject width="{}" height="24"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {}px; text-align: center;"><span class="nodeLabel""#,
        escape_attr_display(&cluster.id),
        escape_attr_display(ctx.look),
        fmt(left),
        fmt(top),
        fmt(w),
        fmt(h),
        escape_attr_display(terminal_style),
        fmt(label_x),
        fmt(label_y),
        fmt(label_w),
        MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX,
    );
    let title_paint = ctx.relation_theme.namespace_title_terminal();
    if let Some((_, style)) = title_paint {
        let _ = write!(out, r#" style="{}""#, escape_attr_display(style));
    }
    let _ = write!(out, ">{title_html}</span></div></foreignObject></g></g>");
    out.checkpoint()?;
    let (fill_rule, stroke_rule) = ctx.relation_theme.cluster_paint_rule_indices();
    theme_receipt.record_cluster(&cluster.id, fill_rule, stroke_rule, terminal_style);
    if let Some((rule, style)) = title_paint {
        theme_receipt.record_namespace_title(
            &cluster.id,
            rule,
            style,
            label_w > 0.0 && !cluster.title.trim().is_empty(),
            inherits_paint,
        );
    }
    Ok(typography)
}

fn class_namespace_title_html(
    title: &str,
    ctx: ClassNamespaceClusterGroupContext<'_>,
) -> (String, crate::class::ClassTypographyTerminalFacts, bool) {
    if let Some(math_html) = class_math_html_label(title, ctx.mermaid_config, ctx.math_renderer) {
        return (
            math_html,
            crate::class::ClassTypographyTerminalFacts::unverified_text(title),
            false,
        );
    }
    (
        format!("<p>{}</p>", escape_xml_display(title)),
        crate::class::ClassTypographyTerminalFacts::inherited_text(title),
        true,
    )
}

pub(super) fn class_namespace_root_offset(c: &LayoutCluster) -> (f64, f64) {
    let w = c.width.max(1.0);
    let h = c.height.max(1.0);
    (c.x - w / 2.0 - 8.0, c.y - h / 2.0)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_class_namespace_clusters_in_root(
    out: &mut impl SvgOutput,
    content_bounds: &mut Option<Bounds>,
    clusters_by_id: &HashMap<&str, &LayoutCluster>,
    cluster_ids: &[&str],
    ctx: ClassNamespaceClusterGroupContext<'_>,
    root_ns_id: &str,
    root_dx: f64,
    root_dy: f64,
    theme_receipt: &mut crate::class::ClassRelationThemeReceipt,
    typography_receipt: &mut Option<crate::class::ClassTypographyThemeReceipt>,
) -> crate::Result<()> {
    out.push_str(r#"<g class="clusters">"#);
    out.checkpoint()?;
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
        let typography = render_class_namespace_cluster_at(
            out,
            content_bounds,
            c,
            (left, top),
            ClassNamespaceClusterGroupContext {
                bounds_dx: root_dx + ctx.bounds_dx,
                bounds_dy: root_dy + ctx.bounds_dy,
                ..ctx
            },
            theme_receipt,
        )?;
        if let Some(receipt) = typography_receipt.as_mut() {
            receipt.record_namespace(&c.id, typography);
        }
    }
    out.push_str("</g>");
    out.checkpoint()
}
