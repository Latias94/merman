use super::*;
use crate::architecture_metrics::{
    ARCHITECTURE_SERVICE_LABEL_BOTTOM_EXTENSION_PX,
    architecture_create_text_compound_label_extra_bottom_px,
    architecture_create_text_root_label_extra_bottom_px,
    architecture_cytoscape_canvas_label_metrics,
};

mod edges;
mod foreign_object;
mod geometry;
mod icons;
mod labels;
mod model;
mod nodes;
mod root;
mod settings;
mod viewport;

use self::edges::{ArchitectureEdgeRenderContext, push_architecture_edges};
use self::geometry::{GroupRect, GroupRectComputer, bounds_from_rect, extend_bounds};
use self::labels::{svg_line_plain_text, wrap_svg_words_to_lines};
use self::model::{ArchitectureModel, ArchitectureModelAccess};
use self::nodes::{
    ArchitectureNodeRenderContext, push_architecture_groups,
    push_architecture_services_and_junctions,
};
use self::root::{
    ArchitectureRootOpenContext, architecture_a11y_nodes, push_architecture_root_open,
};
use self::settings::ArchitectureRenderSettings;
use self::viewport::{ArchitectureRootViewportContext, finalize_architecture_root_viewport};

// Architecture diagram SVG renderer implementation (split from parity.rs).

fn timing_section<'a>(
    enabled: bool,
    dst: &'a mut web_time::Duration,
) -> Option<super::timing::TimingGuard<'a>> {
    enabled.then(|| super::timing::TimingGuard::new(dst))
}

struct ArchitectureRenderRequest<'a, M: ArchitectureModelAccess> {
    layout: &'a ArchitectureDiagramLayout,
    model: &'a M,
    effective_config: &'a serde_json::Value,
    sanitize_config_opt: Option<&'a merman_core::MermaidConfig>,
    options: &'a SvgRenderOptions,
}

struct ArchitectureTimingState<'a> {
    enabled: bool,
    timings: &'a mut super::timing::RenderTimings,
    total_start: web_time::Instant,
}

#[derive(Clone)]
struct ArchitectureServiceBoundsEstimate {
    icon_bounds: Bounds,
    root_bounds: Bounds,
    compound_bounds: Bounds,
}

fn estimate_architecture_service_bounds(
    x: f64,
    y: f64,
    icon_size_px: f64,
    arch_font_size_px: f64,
    svg_font_size_px: f64,
    title: Option<&str>,
    text_measurer: &crate::text::VendoredFontMetricsTextMeasurer,
    text_style: &crate::text::TextStyle,
    compound_text_style: &crate::text::TextStyle,
) -> ArchitectureServiceBoundsEstimate {
    let icon_bounds = bounds_from_rect(x, y, icon_size_px, icon_size_px);
    let mut root_bounds = icon_bounds.clone();
    let mut compound_bounds = icon_bounds.clone();
    let debug_service = std::env::var("MERMAN_ARCH_DEBUG_SERVICE_BOUNDS")
        .ok()
        .filter(|value| !value.is_empty());

    if let Some(title) = title.map(str::trim).filter(|t| !t.is_empty()) {
        let lines =
            wrap_svg_words_to_lines(title, icon_size_px * 1.5, text_measurer, text_style);
        let mut bbox_left_root = 0.0f64;
        let mut bbox_right_root = 0.0f64;
        for line in &lines {
            let s = svg_line_plain_text(line);
            let (l, r) = text_measurer.measure_svg_text_bbox_x(s.as_str(), text_style);
            bbox_left_root = bbox_left_root.max(l);
            bbox_right_root = bbox_right_root.max(r);
        }
        let line_count_root = lines.len().max(1);
        let label_extra_bottom_root =
            architecture_create_text_root_label_extra_bottom_px(svg_font_size_px, line_count_root);

        let metrics = architecture_cytoscape_canvas_label_metrics(
            title,
            text_measurer,
            compound_text_style,
        );
        let compound_half_width = metrics.half_width;
        let bbox_left_compound = compound_half_width;
        let bbox_right_compound = compound_half_width;
        let label_extra_bottom_compound =
            architecture_create_text_compound_label_extra_bottom_px(arch_font_size_px);

        let cx = x + icon_size_px / 2.0;
        let text_left_root = cx - bbox_left_root;
        let text_right_root = cx + bbox_right_root;
        let text_bottom_root = y + icon_size_px + label_extra_bottom_root;

        let text_left_compound = cx - bbox_left_compound;
        let text_right_compound = cx + bbox_right_compound;
        let text_bottom_compound = y + icon_size_px + label_extra_bottom_compound;

        root_bounds = Bounds {
            min_x: root_bounds.min_x.min(text_left_root),
            min_y: root_bounds.min_y,
            max_x: root_bounds.max_x.max(text_right_root),
            max_y: root_bounds.max_y.max(text_bottom_root),
        };
        compound_bounds = Bounds {
            min_x: compound_bounds.min_x.min(text_left_compound),
            min_y: compound_bounds.min_y,
            max_x: compound_bounds.max_x.max(text_right_compound),
            max_y: compound_bounds.max_y.max(text_bottom_compound),
        };

        if debug_service.as_deref() == Some(title) {
            eprintln!(
                "[arch-service-bounds] title={:?} svg_lines={:?} root_lr=({}, {}) root_bottom={} canvas_half={} compound_bottom={} icon_bounds=({}, {})-({}, {}) compound_bounds=({}, {})-({}, {}) root_bounds=({}, {})-({}, {})",
                title,
                lines,
                bbox_left_root,
                bbox_right_root,
                label_extra_bottom_root,
                metrics.half_width,
                label_extra_bottom_compound,
                icon_bounds.min_x,
                icon_bounds.min_y,
                icon_bounds.max_x,
                icon_bounds.max_y,
                compound_bounds.min_x,
                compound_bounds.min_y,
                compound_bounds.max_x,
                compound_bounds.max_y,
                root_bounds.min_x,
                root_bounds.min_y,
                root_bounds.max_x,
                root_bounds.max_y,
            );
        }
    }

    ArchitectureServiceBoundsEstimate {
        icon_bounds,
        root_bounds,
        compound_bounds,
    }
}

pub(super) fn render_architecture_diagram_svg_typed_with_config(
    layout: &ArchitectureDiagramLayout,
    model: &merman_core::diagrams::architecture::ArchitectureDiagramRenderModel,
    effective_config: &merman_core::MermaidConfig,
    options: &SvgRenderOptions,
) -> Result<String> {
    let timing_enabled = super::timing::render_timing_enabled();
    let mut timings = super::timing::RenderTimings::default();
    let total_start = web_time::Instant::now();

    render_architecture_diagram_svg_with_model(
        ArchitectureRenderRequest {
            layout,
            model,
            effective_config: effective_config.as_value(),
            sanitize_config_opt: Some(effective_config),
            options,
        },
        ArchitectureTimingState {
            enabled: timing_enabled,
            timings: &mut timings,
            total_start,
        },
    )
}

pub(super) fn render_architecture_diagram_svg(
    layout: &ArchitectureDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    let timing_enabled = super::timing::render_timing_enabled();
    let mut timings = super::timing::RenderTimings::default();
    let total_start = web_time::Instant::now();
    let model: ArchitectureModel = {
        let _g = timing_section(timing_enabled, &mut timings.deserialize_model);
        crate::json::from_value_ref(semantic)?
    };
    render_architecture_diagram_svg_with_model(
        ArchitectureRenderRequest {
            layout,
            model: &model,
            effective_config,
            sanitize_config_opt: None,
            options,
        },
        ArchitectureTimingState {
            enabled: timing_enabled,
            timings: &mut timings,
            total_start,
        },
    )
}

pub(super) fn render_architecture_diagram_svg_with_config(
    layout: &ArchitectureDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &merman_core::MermaidConfig,
    options: &SvgRenderOptions,
) -> Result<String> {
    let timing_enabled = super::timing::render_timing_enabled();
    let mut timings = super::timing::RenderTimings::default();
    let total_start = web_time::Instant::now();
    let model: ArchitectureModel = {
        let _g = timing_section(timing_enabled, &mut timings.deserialize_model);
        crate::json::from_value_ref(semantic)?
    };
    render_architecture_diagram_svg_with_model(
        ArchitectureRenderRequest {
            layout,
            model: &model,
            effective_config: effective_config.as_value(),
            sanitize_config_opt: Some(effective_config),
            options,
        },
        ArchitectureTimingState {
            enabled: timing_enabled,
            timings: &mut timings,
            total_start,
        },
    )
}

fn render_architecture_diagram_svg_with_model<M: ArchitectureModelAccess>(
    req: ArchitectureRenderRequest<'_, M>,
    timing: ArchitectureTimingState<'_>,
) -> Result<String> {
    let ArchitectureRenderRequest {
        layout,
        model,
        effective_config,
        sanitize_config_opt,
        options,
    } = req;
    let ArchitectureTimingState {
        enabled: timing_enabled,
        timings,
        total_start,
    } = timing;

    fn section<'a>(
        enabled: bool,
        dst: &'a mut web_time::Duration,
    ) -> Option<super::timing::TimingGuard<'a>> {
        enabled.then(|| super::timing::TimingGuard::new(dst))
    }

    let _g_render_svg = section(timing_enabled, &mut timings.render_svg);

    let diagram_id = options.diagram_id.as_deref().unwrap_or("architecture");
    let settings = ArchitectureRenderSettings::from_config(diagram_id, effective_config);
    let ArchitectureRenderSettings {
        css,
        icon_size_px,
        half_icon,
        padding_px,
        arch_font_size_px,
        svg_font_size_px,
        use_max_width,
        text_style,
        compound_text_style,
    } = settings.clone();
    let sanitize_config_owned: merman_core::MermaidConfig;
    let sanitize_config = match sanitize_config_opt {
        Some(cfg) => cfg,
        None => {
            sanitize_config_owned =
                merman_core::MermaidConfig::from_value(effective_config.clone());
            &sanitize_config_owned
        }
    };

    let mut node_xy: rustc_hash::FxHashMap<&str, (f64, f64)> = rustc_hash::FxHashMap::default();
    for n in &layout.nodes {
        node_xy.insert(n.id.as_str(), (n.x, n.y));
    }

    let text_measurer = crate::text::VendoredFontMetricsTextMeasurer::default();

    let a11y = architecture_a11y_nodes(diagram_id, model.acc_title(), model.acc_descr());

    // Mermaid Architecture uses `setupGraphViewbox()` which expands the viewBox based on the
    // SVG's `getBBox()` plus `architecture.padding`. We approximate the effective `getBBox()` by
    // computing a conservative bounds over the elements we emit.
    let mut content_bounds: Option<Bounds> = None;

    // Mermaid `createText()` emits SVG `<text y="-10.1">` + `<tspan y="-0.1em" dy="1.1em">...`.
    //
    // In Chromium, `text.getBBox()` has:
    // - per-line height ~= 19px at 16px font size
    // - additional lines stacked by `dy="1.1em"` (i.e. 17.6px at 16px font size)
    //
    // Model this geometry in a scale-stable way so `setupGraphViewbox(svg.getBBox() + padding)`
    // aligns in `parity-root` comparisons without browser-dependent measurement.
    // Empirical bottom extension (beyond the icon bottom) of Mermaid `createText()` output for a
    // single-line label at 16px in Chromium, as observed in upstream Architecture baselines.
    //
    // This is notably larger than just `fontSize`, due to `createText()` using `<text y="-10.1">`
    // and wrapper attributes like `dy="1em"`; Chromium's `getBBox()` includes that geometry.
    // Cytoscape compound bounds (`node.boundingBox()`) include labels but do *not* match
    // Chromium's `text.getBBox()` exactly. In upstream Mermaid Architecture, group rectangles
    // sized from Cytoscape compound bounds tend to extend below the icon by roughly
    // `(fontSize + 1px)` for single-line service labels.
    //
    // If we reuse the larger root `getBBox()` extension for compounds, nested/group-heavy
    // fixtures get a systematic viewBox height inflation (~7.1875px at 16px).

    // Mermaid singleton top-level `iconText` services render 18px lower than the nominal
    // layout origin; keep the emitted transform and root bbox estimate in sync.
    let groups_len = model.groups_len();
    let edges_len = model.edges_len();
    let service_count = model.services().count();
    let junction_count = model.junctions().count();
    let singleton_icon_text_service_id =
        if groups_len == 0 && service_count == 1 && junction_count == 0 && edges_len == 0 {
            model.services().next().and_then(|service| {
                if service.in_group.is_none()
                    && service
                        .icon_text
                        .map(str::trim)
                        .is_some_and(|text: &str| !text.is_empty())
                {
                    Some(service.id)
                } else {
                    None
                }
            })
        } else {
            None
        };
    let singleton_icon_text_offset_y = |service_id: &str| {
        if singleton_icon_text_service_id == Some(service_id) {
            ARCHITECTURE_SERVICE_LABEL_BOTTOM_EXTENSION_PX
        } else {
            0.0
        }
    };

    let mut service_bounds: rustc_hash::FxHashMap<&str, Bounds> = rustc_hash::FxHashMap::default();
    for svc in model.services() {
        let (x, y) = node_xy.get(svc.id).copied().unwrap_or((0.0, 0.0));
        let y = y + singleton_icon_text_offset_y(svc.id);
        let estimate = estimate_architecture_service_bounds(
            x,
            y,
            icon_size_px,
            arch_font_size_px,
            svg_font_size_px,
            svc.title,
            &text_measurer,
            &text_style,
            &compound_text_style,
        );
        let b_full = if svc.in_group.is_some() {
            estimate.compound_bounds.clone()
        } else {
            estimate.root_bounds.clone()
        };
        // Group rectangles (compound nodes) are sized by Cytoscape to include service labels, so
        // extending the root `getBBox()` estimate with *in-group* label bounds can double-count
        // and inflate the final `viewBox` / `max-width` in parity-root comparisons.
        //
        // Keep full label bounds for group sizing, but only union label extents into the root
        // viewport bounds when the service is not inside a group.
        service_bounds.insert(svc.id, b_full.clone());
        if svc.in_group.is_none() {
            // For top-level services, approximate Chromium `getBBox()` via the root label model.
            extend_bounds(&mut content_bounds, estimate.root_bounds);
        } else {
            extend_bounds(&mut content_bounds, estimate.icon_bounds);
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

    let is_empty = service_count == 0
        && junction_count == 0
        && model.groups_len() == 0
        && model.edges_len() == 0;

    let mut out = String::new();
    push_architecture_root_open(ArchitectureRootOpenContext {
        out: &mut out,
        diagram_id,
        css: css.as_str(),
        a11y: &a11y,
        is_empty,
        use_max_width,
        half_icon,
        icon_size_px,
    });
    // Edge bounds and DOM emission live in `architecture/edges.rs`.
    {
        let mut edge_render_ctx = ArchitectureEdgeRenderContext {
            out: &mut out,
            diagram_id,
            layout,
            model,
            node_xy: &node_xy,
            settings: &settings,
            text_measurer: &text_measurer,
            content_bounds: &mut content_bounds,
            junction_bounds: &junction_bounds,
        };
        push_architecture_edges(&mut edge_render_ctx);
    }
    out.push_str("</g>");

    {
        let mut node_render_ctx = ArchitectureNodeRenderContext {
            out: &mut out,
            diagram_id,
            model,
            node_xy: &node_xy,
            settings: &settings,
            text_measurer: &text_measurer,
            sanitize_config,
            content_bounds: &mut content_bounds,
            singleton_icon_text_service_id,
        };
        push_architecture_services_and_junctions(&mut node_render_ctx);
        push_architecture_groups(&mut node_render_ctx, &group_rects);
    }

    out.push_str("</svg>\n");

    if !is_empty {
        out = finalize_architecture_root_viewport(ArchitectureRootViewportContext {
            out,
            diagram_id,
            model,
            content_bounds,
            padding_px,
            icon_size_px,
            use_max_width,
            apply_root_overrides: options.apply_root_overrides,
        });
    }

    drop(_g_render_svg);

    timings.total = total_start.elapsed();
    if timing_enabled {
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

    Ok(out)
}
