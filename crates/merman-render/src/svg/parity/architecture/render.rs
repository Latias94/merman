use super::super::*;
use crate::architecture_metrics::architecture_estimate_service_bounds;
use crate::model::{ArchitectureCytoscapeServiceBounds, ArchitectureDiagramLayout};

use super::edges::{ArchitectureEdgeRenderContext, push_architecture_edges};
use super::geometry::{GroupRect, GroupRectComputer, bounds_from_rect, extend_bounds};
use super::labels::{svg_line_plain_text, wrap_svg_words_to_lines};
use super::model::{ArchitectureModelAccess, ArchitectureServiceRef};
use super::nodes::{
    ArchitectureNodeRenderContext, push_architecture_groups,
    push_architecture_services_and_junctions,
};
use super::root::{architecture_a11y_nodes, begin_architecture_document};
use super::settings::ArchitectureRenderSettings;
use super::viewport::{ArchitectureRootViewportContext, finalize_architecture_root_viewport};

// Architecture diagram SVG renderer implementation (split from parity.rs).

fn architecture_bounds_match_icon_rect(bounds: &Bounds, x: f64, y: f64, icon_size_px: f64) -> bool {
    const EPSILON: f64 = 1e-6;
    (bounds.min_x - x).abs() <= EPSILON
        && (bounds.min_y - y).abs() <= EPSILON
        && (bounds.max_x - (x + icon_size_px)).abs() <= EPSILON
        && (bounds.max_y - (y + icon_size_px)).abs() <= EPSILON
}

fn architecture_cached_service_child_bounds<'a>(
    service_bounds_by_id: &'a rustc_hash::FxHashMap<&str, &'a ArchitectureCytoscapeServiceBounds>,
    service: ArchitectureServiceRef<'_>,
    x: f64,
    y: f64,
    icon_size_px: f64,
) -> Option<&'a ArchitectureCytoscapeServiceBounds> {
    let cached = service_bounds_by_id.get(service.id).copied()?;
    if cached.in_group.as_deref() != service.in_group {
        return None;
    }
    if !architecture_bounds_match_icon_rect(&cached.body_bounds, x, y, icon_size_px) {
        return None;
    }
    Some(cached)
}

struct ArchitectureRenderRequest<'a, M: ArchitectureModelAccess> {
    layout: &'a ArchitectureDiagramLayout,
    model: &'a M,
    effective_config: &'a serde_json::Value,
    sanitize_config: &'a merman_core::MermaidConfig,
    group_theme: &'a crate::architecture::ArchitectureGroupThemePlan,
    options: &'a SvgExecution<'a>,
}

struct ArchitectureTimingState<'a> {
    timing: super::super::timing::RenderTiming,
    timings: &'a mut super::super::timing::RenderTimings,
    total_timer: Option<merman_core::runtime::OperationTimer>,
}

pub(crate) fn render_architecture_diagram_svg_typed_with_config(
    layout: &ArchitectureDiagramLayout,
    model: &merman_core::diagrams::architecture::ArchitectureDiagramRenderModel,
    effective_config: &merman_core::MermaidConfig,
    group_theme: &crate::architecture::ArchitectureGroupThemePlan,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let timing = options.timing();
    let mut timings = super::super::timing::RenderTimings::default();
    let total_timer = timing.start();

    render_architecture_diagram_svg_with_model(
        ArchitectureRenderRequest {
            layout,
            model,
            effective_config: effective_config.as_value(),
            sanitize_config: effective_config,
            group_theme,
            options,
        },
        ArchitectureTimingState {
            timing,
            timings: &mut timings,
            total_timer,
        },
    )
}

fn render_architecture_diagram_svg_with_model<M: ArchitectureModelAccess>(
    req: ArchitectureRenderRequest<'_, M>,
    timing: ArchitectureTimingState<'_>,
) -> Result<root_svg::RootedSvg> {
    let ArchitectureRenderRequest {
        layout,
        model,
        effective_config,
        sanitize_config,
        group_theme,
        options,
    } = req;
    let ArchitectureTimingState {
        timing,
        timings,
        total_timer,
    } = timing;

    let _g_render_svg = timing.section(&mut timings.render_svg);

    let diagram_id = options.diagram_id.as_deref().unwrap_or("architecture");
    let settings = ArchitectureRenderSettings::from_config(diagram_id, effective_config);
    let css = settings.css.as_str();
    let icon_size_px = settings.icon_size_px;
    let half_icon = settings.half_icon;
    let padding_px = settings.padding_px;
    let arch_font_size_px = settings.arch_font_size_px;
    let use_max_width = settings.use_max_width;
    let text_style = &settings.text_style;
    let compound_text_style = &settings.compound_text_style;
    let mut node_xy: rustc_hash::FxHashMap<&str, (f64, f64)> = rustc_hash::FxHashMap::default();
    for n in &layout.nodes {
        node_xy.insert(n.id.as_str(), (n.x, n.y));
    }

    let text_measurer = options.text_measurer();

    let a11y = architecture_a11y_nodes(diagram_id, model.acc_title(), model.acc_descr());

    // Mermaid Architecture uses `setupGraphViewbox()` which expands the viewBox based on the
    // SVG's `getBBox()` plus `architecture.padding`. Reconstruct that effective bbox from the
    // emitted geometry, cached Cytoscape bounds, and operation-owned text measurements.
    let mut content_bounds: Option<Bounds> = None;

    // Service/root text bounds use the exact tspan-height and middle-baseline operations. Compound
    // sizing stays on the separate Cytoscape child-bbox phases cached by layout; the two DOM
    // consumers intentionally do not share a vertical extent.

    let mut cached_service_bounds_by_id: rustc_hash::FxHashMap<
        &str,
        &ArchitectureCytoscapeServiceBounds,
    > = rustc_hash::FxHashMap::default();
    cached_service_bounds_by_id.reserve(layout.cytoscape_service_bounds.len());
    for bounds in &layout.cytoscape_service_bounds {
        cached_service_bounds_by_id.insert(bounds.id.as_str(), bounds);
    }

    let mut service_bounds: rustc_hash::FxHashMap<&str, Bounds> = rustc_hash::FxHashMap::default();
    for svc in model.services() {
        let (x, y) = node_xy.get(svc.id).copied().unwrap_or((0.0, 0.0));
        if svc.in_group.is_some()
            && let Some(cached) = architecture_cached_service_child_bounds(
                &cached_service_bounds_by_id,
                svc,
                x,
                y,
                icon_size_px,
            )
        {
            service_bounds.insert(svc.id, cached.union_bounds.clone());
            extend_bounds(&mut content_bounds, cached.body_bounds.clone());
            continue;
        }

        let estimate = architecture_estimate_service_bounds(
            x,
            y,
            icon_size_px,
            arch_font_size_px,
            svc.title,
            text_measurer,
            text_style,
            compound_text_style,
            wrap_svg_words_to_lines,
            |line| svg_line_plain_text(line.as_slice()),
            |line, style| text_measurer.measure_svg_text_bbox_x(line, style),
        );
        let b_full = if svc.in_group.is_some() {
            estimate
                .cytoscape_group_child_contribution
                .union_bounds
                .clone()
        } else {
            estimate.svg_root_bounds.clone()
        };
        // Group rectangles (compound nodes) are sized by Cytoscape to include service labels, so
        // extending the root `getBBox()` estimate with *in-group* label bounds can double-count
        // and inflate the final `viewBox` / `max-width` in parity-root comparisons.
        //
        // Keep full label bounds for group sizing, but only union label extents into the root
        // viewport bounds when the service is not inside a group.
        service_bounds.insert(svc.id, b_full.clone());
        if svc.in_group.is_none() {
            extend_bounds(&mut content_bounds, estimate.svg_root_bounds);
        } else {
            extend_bounds(&mut content_bounds, estimate.emitted_icon_bounds);
        }
    }

    let mut junction_bounds: rustc_hash::FxHashMap<&str, Bounds> = rustc_hash::FxHashMap::default();
    for junction in model.junctions() {
        let (x, y) = node_xy.get(junction.id).copied().unwrap_or((0.0, 0.0));
        let b = bounds_from_rect(x, y, icon_size_px, icon_size_px);
        junction_bounds.insert(junction.id, b.clone());
        extend_bounds(&mut content_bounds, b);
    }

    // Groups (outer rects, including nested groups).
    let mut child_groups: rustc_hash::FxHashMap<&str, Vec<&str>> = rustc_hash::FxHashMap::default();
    for g in model.groups() {
        if let Some(parent) = g.in_group {
            child_groups.entry(parent).or_default().push(g.id);
        }
    }
    for v in child_groups.values_mut() {
        v.sort_unstable();
    }

    let mut services_in_group: rustc_hash::FxHashMap<&str, Vec<&str>> =
        rustc_hash::FxHashMap::default();
    for svc in model.services() {
        if let Some(parent) = svc.in_group {
            services_in_group.entry(parent).or_default().push(svc.id);
        }
    }
    for v in services_in_group.values_mut() {
        v.sort_unstable();
    }

    let mut junctions_in_group: rustc_hash::FxHashMap<&str, Vec<&str>> =
        rustc_hash::FxHashMap::default();
    for junction in model.junctions() {
        if let Some(parent) = junction.in_group {
            junctions_in_group
                .entry(parent)
                .or_default()
                .push(junction.id);
        }
    }
    for v in junctions_in_group.values_mut() {
        v.sort_unstable();
    }

    let mut group_rects_computer = GroupRectComputer::new(
        icon_size_px,
        padding_px,
        &services_in_group,
        &junctions_in_group,
        &child_groups,
        &service_bounds,
        &junction_bounds,
    );
    for g in model.groups() {
        let _ = group_rects_computer.compute(g.id);
    }

    let mut group_rects: Vec<GroupRect<'_>> = Vec::with_capacity(model.groups_len());
    for g in model.groups() {
        if let Some(b) = group_rects_computer.get(g.id) {
            group_rects.push(GroupRect {
                id: g.id,
                x: b.min_x,
                y: b.min_y,
                w: (b.max_x - b.min_x).max(1.0),
                h: (b.max_y - b.min_y).max(1.0),
                icon: g.icon,
                title: g.title,
            });
            extend_bounds(&mut content_bounds, b.clone());
        }
    }

    let is_empty = model.services().next().is_none()
        && model.junctions().next().is_none()
        && model.groups_len() == 0
        && model.edges_len() == 0;

    let mut out = BoundedSvgOutput::new(options.work_meter());
    let root_viewport =
        root_svg::RootViewportContext::new(crate::DiagramFamilyId::ARCHITECTURE, diagram_id)
            .with_resource_policy(options.resource_policy());
    let root_document = begin_architecture_document(
        &mut out,
        &root_viewport,
        diagram_id,
        css,
        &a11y,
        use_max_width,
    )?;
    let edge_stroke = group_theme.edge_stroke();
    let edge_inline_style = edge_stroke.map(|(_, stroke)| format!("stroke:{stroke};"));
    let mut edge_theme_receipt = group_theme.begin_edge_terminal_receipt(
        model
            .edges()
            .enumerate()
            .map(|(edge_index, edge)| (edge_index, edge.lhs_id, edge.rhs_id)),
    );
    // Edge bounds and DOM emission live in `architecture/edges.rs`.
    {
        let mut edge_render_ctx = ArchitectureEdgeRenderContext {
            out: &mut out,
            diagram_id,
            layout,
            model,
            node_xy: &node_xy,
            settings: &settings,
            text_measurer,
            content_bounds: &mut content_bounds,
            junction_bounds: &junction_bounds,
            edge_stroke,
            edge_inline_style: edge_inline_style.as_deref(),
            terminal_receipt: edge_theme_receipt.as_mut(),
        };
        push_architecture_edges(&mut edge_render_ctx)?;
    }
    out.push_str("</g>");
    out.checkpoint()?;

    let mut group_theme_receipt = group_theme.begin_terminal_receipt();
    {
        let mut node_render_ctx = ArchitectureNodeRenderContext {
            out: &mut out,
            diagram_id,
            model,
            node_xy: &node_xy,
            settings: &settings,
            text_measurer,
            sanitize_config,
            icon_registry: options.icon_registry(),
            work_meter: options.work_meter(),
            content_bounds: &mut content_bounds,
        };
        push_architecture_services_and_junctions(&mut node_render_ctx)?;
        push_architecture_groups(
            &mut node_render_ctx,
            &group_rects,
            group_theme.inline_style(),
            group_theme_receipt.as_mut(),
        )?;
    }

    out.push_str("</svg>\n");
    out.checkpoint()?;

    let root_document = finalize_architecture_root_viewport(ArchitectureRootViewportContext {
        out: &mut out,
        root_viewport: &root_viewport,
        root_document,
        content_bounds,
        padding_px,
        half_icon,
        icon_size_px,
        use_max_width,
        is_empty,
        trust_content_bounds: options.icon_registry().is_none(),
    })?;
    let rooted_svg = root_document.complete(out.finish()?)?;
    if let Some(receipt) = group_theme_receipt {
        let _ = group_theme.record_terminal(receipt);
    }
    if edge_theme_receipt.is_some_and(|receipt| !group_theme.record_edge_terminal(receipt)) {
        return Err(crate::Error::InvalidModel {
            message: "Architecture edge stroke receipt did not match the terminal SVG".to_string(),
        });
    }

    drop(_g_render_svg);

    timings.total = total_timer
        .map(merman_core::runtime::OperationTimer::elapsed)
        .unwrap_or_default();
    if timing.is_enabled() {
        eprintln!(
            "[render-timing] diagram=architecture total={:?} deserialize={:?} build_ctx={:?} viewbox={:?} render_svg={:?} finalize={:?}",
            timings.total,
            timings.deserialize_model,
            timings.build_ctx,
            timings.viewbox,
            timings.render_svg,
            timings.finalize_svg,
        );
    }

    Ok(rooted_svg)
}
