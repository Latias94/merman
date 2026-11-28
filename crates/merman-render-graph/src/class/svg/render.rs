use super::context::ClassEmitCheckpoint;
use super::groups::ClassSplitEdgeGroupsRenderContext;
use super::nodes::{
    ClassNodesRenderContext, ClassNodesRenderState, render_class_flat_adapter_dom,
    render_class_render_tree,
};
use super::root::{CLASS_GRAPH_MARGIN_PX, write_class_svg_document};
use super::viewbox::{ClassViewBoxContext, class_viewbox};
use super::*;
use merman_render_core::svg_output::timing::RenderTimings;

#[cfg(feature = "layout-shoal")]
use merman_render_core::{
    LayoutEdge, LayoutPoint,
    route_paint::{CanonicalPaintScratch, PaintedRoute, RouteStyle},
    route_types::Marker,
};
use merman_render_core::{resources::OperationWorkMeter, svg_output::timing::RenderTiming};
use roughr::core::RoughRandomness;

#[cfg(feature = "layout-shoal")]
pub struct ClassCanonicalEdge {
    pub path: PaintedRoute,
    pub marker_points: [LayoutPoint; 4],
    pub data_points: Vec<LayoutPoint>,
    pub markers: (Marker, Marker),
}

#[cfg(feature = "layout-shoal")]
pub type ClassCanonicalPaint<'a> = dyn Fn(
        &LayoutEdge,
        kurbo::Vec2,
        f64,
        RouteStyle,
        &mut CanonicalPaintScratch,
    ) -> Result<ClassCanonicalEdge>
    + 'a;

#[cfg(feature = "layout-shoal")]
pub type ClassCanonicalRoughStroke =
    fn(&str, (Marker, Marker), &OperationWorkMeter, &RoughRandomness) -> Result<String>;

#[cfg(feature = "layout-shoal")]
pub struct ClassCanonicalRoutes<'a> {
    pub paint: &'a ClassCanonicalPaint<'a>,
    pub rough_stroke: ClassCanonicalRoughStroke,
}

/// Facade-owned root, timing, random key, and optional canonical geometry.
pub struct ClassSvgContext<'a> {
    pub diagram_id: SvgDiagramId<'a>,
    pub viewport: root_svg::RootViewportContext<'a>,
    pub work_meter: &'a OperationWorkMeter,
    pub math_renderer: Option<&'a (dyn merman_render_core::math::MathRenderer + Send + Sync)>,
    pub timing: RenderTiming,
    pub hand_drawn_seed: RoughRandomness,
    pub elk_parity: bool,
    #[cfg(feature = "layout-grid")]
    pub grid: Option<merman_render_core::graph_layout::mermaid_grid::GridPaint>,
    #[cfg(feature = "layout-shoal")]
    pub canonical_routes: Option<&'a ClassCanonicalRoutes<'a>>,
}

#[allow(
    clippy::too_many_arguments,
    reason = "The SVG writer receives prepared geometry, styles, and operation inputs."
)]
pub fn render_class_diagram_svg_model_with_config(
    layout: &ClassDiagramLayout,
    prepared_labels: &crate::class::ClassPreparedLabels,
    model: &ClassSvgModel,
    relation_theme: &crate::class::ClassRelationThemePlan,
    typography_theme: &crate::class::ClassTextThemePlan,
    node_visual_plan: &crate::class::ClassNodeVisualPlan,
    render_config: &crate::class::ClassRenderConfig,
    effective_config: &merman_model::presentation::PresentationConfig,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: ClassSvgContext<'_>,
) -> Result<root_svg::RootedSvg> {
    let mermaid_config = effective_config;
    let effective_config = effective_config.as_value();
    let timing = options.timing;
    let total_timer = timing.start();
    let mut timings = RenderTimings::default();

    let mut detail = ClassRenderDetails::default();
    let diagram_id = options.diagram_id;
    let emit = ClassEmitCheckpoint {
        work_meter: options.work_meter,
    };
    let aria_roledescription = model.model().diagram_type.as_str();

    let build_ctx_guard = timing.section(&mut timings.build_ctx);
    let hand_drawn_seed = options.hand_drawn_seed;
    let settings = render_config;
    #[cfg(feature = "layout-shoal")]
    let canonical_routes = options.canonical_routes;
    #[cfg(feature = "layout-shoal")]
    let uses_canonical_routes = canonical_routes.is_some();
    #[cfg(not(feature = "layout-shoal"))]
    let uses_canonical_routes = false;
    let elk_parity = options.elk_parity;
    #[cfg(feature = "layout-grid")]
    let grid = options.grid.map(|mut paint| {
        if !mermaid_config.explicit_config_owns_path("grid.curve")
            && matches!(relation_theme.native_edge_curve(), Some("step" | "linear"))
        {
            // The native straight/orthogonal/sketch forms can use the Grid shaft vertices.
            paint.curve = "linear";
        }
        paint
    });
    #[cfg(feature = "layout-grid")]
    let uses_grid = grid.is_some();
    #[cfg(not(feature = "layout-grid"))]
    let uses_grid = false;
    #[cfg(feature = "layout-layered")]
    let layered = if let crate::class::layered::Paint::Routed {
        routing,
        axes,
        cluster_order,
        label_y_offset,
    } = &prepared_labels.layered
    {
        use merman_render_core::svg_edge_parity::layered_route::Curve;

        let authored = mermaid_config.get_str("flowchart.curve").unwrap_or("basis");
        let curve = if !mermaid_config.explicit_config_owns_path("layered.edgeRouting")
            && let Some(native) = relation_theme.native_edge_curve()
        {
            Curve::from_mermaid(native)
        } else {
            Curve::for_edge_routing(*routing, authored)
        };
        Some(super::layered::ClassLayeredSvg {
            axes,
            cluster_order,
            curve,
            label_y_offset: *label_y_offset,
        })
    } else {
        None
    };
    #[cfg(feature = "layout-layered")]
    let uses_layered = layered.is_some();
    #[cfg(not(feature = "layout-layered"))]
    let uses_layered = false;
    #[cfg(feature = "layout-layered")]
    if let Some(layered) = layered {
        layered.validate(&layout.edges, layout.clusters.len())?;
    }
    let uses_flat_dom = elk_parity || uses_grid || uses_layered;

    // Mermaid's Dagre renderer applies fixed 8px graph margins. The flat registered renderers
    // emit the layout coordinates directly with the viewport padding as their outer margin.
    let content_tx = if uses_flat_dom || uses_canonical_routes {
        0.0
    } else {
        CLASS_GRAPH_MARGIN_PX
    };
    let content_ty = content_tx;

    // Mermaid derives the final viewport using `svg.getBBox()` (after rendering). We don't have a
    // browser DOM, so approximate the effective bbox by accumulating bounds for the elements we
    // emit (using the exact same `d` strings we output for paths).
    let mut content_bounds: Option<Bounds> = None;

    let render_guard = timing.section(&mut timings.render_svg);
    let mut out = BoundedSvgOutput::new(options.work_meter);
    let root_context = &options.viewport;
    emit.checkpoint()?;

    // Mermaid emits a single `<style>` element with diagram-scoped CSS.
    out.push_str("<style>");
    write_class_css(
        &mut merman_render_core::svg_output::XmlTextOutput::new(&mut out),
        diagram_id.semantic_str(),
        typography_theme,
        relation_theme,
        settings.native_style.as_ref(),
    )?;
    if let Some(native) = &settings.native_style {
        super::css::write_class_native_css(
            &mut merman_render_core::svg_output::XmlTextOutput::new(&mut out),
            diagram_id.semantic_str(),
            native,
            relation_theme,
            typography_theme.css_binding(),
        )?;
    }
    out.push_str("</style>");
    out.checkpoint()?;
    emit.checkpoint()?;

    // Mermaid wraps diagram content (defs + root) in a single `<g>` element.
    out.push_str("<g>");
    out.checkpoint()?;
    // Mermaid 12 shares host markers across registered layouts.
    class_markers(&mut out, diagram_id, aria_roledescription, true)?;
    emit.checkpoint()?;

    let ClassRenderLookups {
        class_nodes_by_id,
        class_color_indices,
        relations_by_id,
        relation_index_by_id,
        note_by_id,
        iface_by_id,
    } = ClassRenderLookups::new(model);

    drop(build_ctx_guard);

    let terminal_text_style = crate::class::class_cardinality_text_style(settings.text_style());
    let mut paint_edges = std::borrow::Cow::Borrowed(layout.edges.as_slice());
    if elk_parity {
        let edges = super::edge::class_edge_render_order(&layout.edges, &relation_index_by_id)
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        paint_edges = std::borrow::Cow::Owned(edges);
    }
    let mut missing_section_points = rustc_hash::FxHashMap::default();
    if elk_parity {
        let nodes_by_id: rustc_hash::FxHashMap<_, _> = layout
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect();
        for edge in &layout.edges {
            if edge.points.is_empty()
                && let (Some(start), Some(end)) = (
                    nodes_by_id.get(edge.from.as_str()),
                    nodes_by_id.get(edge.to.as_str()),
                )
            {
                missing_section_points.insert(
                    edge.id.as_str(),
                    merman_render_core::graph_layout::elk_geometry::missing_rect_section_points(
                        start, end,
                    ),
                );
            }
        }
    }
    let line_hop_edges = if elk_parity && !relation_theme.sketch_edges {
        options.work_meter.charge(paint_edges.len())?;
        paint_edges
            .iter()
            .map(|edge| {
                let relation = relations_by_id.get(edge.id.as_str()).copied();
                let missing = missing_section_points.get(edge.id.as_str());
                merman_render_core::svg_edge_parity::line_hops::LineHopEdge {
                    id: edge.id.as_str(),
                    points: missing.map(Vec::as_slice).unwrap_or(&edge.points),
                    curve: Some(if missing.is_some() {
                        "linear"
                    } else {
                        "rounded"
                    }),
                    arrow_type_start: relation.and_then(|rel| {
                        super::edge::class_arrow_type_for_relation_end(rel.relation.type1)
                    }),
                    arrow_type_end: relation.and_then(|rel| {
                        super::edge::class_arrow_type_for_relation_end(rel.relation.type2)
                    }),
                }
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let line_hop_paths = merman_render_core::svg_edge_parity::line_hops::elk_line_hop_paths(
        effective_config,
        &line_hop_edges,
        options.work_meter,
    )?;
    #[cfg(feature = "layout-grid")]
    let line_hop_paths = if let Some(paint) = grid {
        super::grid::line_hop_paths(&paint_edges, &relations_by_id, paint, options.work_meter)?
    } else {
        line_hop_paths
    };
    #[cfg(feature = "layout-layered")]
    let line_hop_paths = if let Some(layered) = layered {
        let style = if relation_theme.sketch_edges {
            None
        } else {
            merman_render_core::svg_edge_parity::layered_route::line_hop_style(
                mermaid_config
                    .as_config()
                    .get_path(&["layered", "lineHops"]),
            )
        };
        super::layered::line_hop_paths(
            &paint_edges,
            &relations_by_id,
            layered.curve,
            style,
            options.work_meter,
        )?
    } else {
        line_hop_paths
    };
    let group_ctx = ClassSplitEdgeGroupsRenderContext {
        edges: &paint_edges,
        clusters: &layout.clusters,
        native_style: settings.native_style.as_ref(),
        missing_section_points: &missing_section_points,
        work_meter: options.work_meter,
        line_hop_paths: &line_hop_paths,
        relations_by_id: &relations_by_id,
        relation_index_by_id: &relation_index_by_id,
        prepared_labels,
        #[cfg(feature = "layout-grid")]
        grid,
        #[cfg(feature = "layout-layered")]
        layered,
        #[cfg(feature = "layout-shoal")]
        canonical_routes,
        diagram_marker_class: aria_roledescription,
        diagram_id,
        content_tx,
        content_ty,
        edge_use_html_labels: settings.edge_use_html_labels,
        text_measurer: measurer,
        terminal_text_style: &terminal_text_style,
        mermaid_config: Some(mermaid_config),
        math_renderer: options.math_renderer,
        look: settings.look.as_str(),
        hand_drawn_seed: hand_drawn_seed.clone(),
        timing,
        uses_elk_adapter_dom: elk_parity,
        edge_paths_class: if uses_flat_dom {
            "edges edgePaths"
        } else {
            "edgePaths"
        },
        relation_theme,
        emit,
    };

    // The layout-owned render tree preserves the exact recursive Dagre graph that produced these
    // coordinates. Rendering consumes that tree directly instead of inferring namespace extraction
    // and edge ownership a second time from flattened compatibility data.
    let nodes_start = timing.start();
    let mut edge_dom_ids = Vec::new();
    if !layout.clusters.is_empty() {
        options.work_meter.charge(layout.edges.len())?;
        for edge in &layout.edges {
            let mut id = String::new();
            super::edge::class_edge_dom_id_into(&mut id, edge, &relation_index_by_id);
            edge_dom_ids.push(id);
        }
    }
    let namespace_ids = super::namespace::class_namespace_ids(
        &layout.clusters,
        class_nodes_by_id
            .values()
            .map(|class| class.dom_id)
            .chain(note_by_id.keys().copied())
            .chain(iface_by_id.keys().copied())
            .chain(edge_dom_ids.iter().map(String::as_str)),
    );

    let nodes_ctx = ClassNodesRenderContext {
        layout,
        namespace_ids: &namespace_ids,
        prepared_svg_titles: &prepared_labels.titles_by_id,
        class_nodes_by_id: &class_nodes_by_id,
        class_color_indices: &class_color_indices,
        note_by_id: &note_by_id,
        iface_by_id: &iface_by_id,
        settings,
        hand_drawn_seed: &hand_drawn_seed,
        diagram_id,
        measurer,
        mermaid_config,
        math_renderer: options.math_renderer,
        node_visual_plan,
        typography_theme,
        relation_theme,
        content_tx,
        content_ty,
        uses_canonical_routes,
        rough_backing: uses_canonical_routes || relation_theme.rough_backing,
        timing,
        emit,
    };
    if uses_flat_dom {
        out.push_str(r#"<g class="root">"#);
        render_class_flat_adapter_dom(
            ClassNodesRenderState {
                out: &mut out,
                content_bounds: &mut content_bounds,
                detail: &mut detail,
            },
            &nodes_ctx,
            &group_ctx,
        )?;
        out.push_str("</g>"); // root
        out.push_str("</g>"); // wrapper
    } else {
        out.push_str(r#"<g class="root">"#);
        out.checkpoint()?;
        render_class_render_tree(
            ClassNodesRenderState {
                out: &mut out,
                content_bounds: &mut content_bounds,
                detail: &mut detail,
            },
            &nodes_ctx,
            &group_ctx,
        )?;
        out.push_str("</g>"); // root
        out.push_str("</g>"); // wrapper
        out.checkpoint()?;
    }
    if let Some(s) = nodes_start {
        detail.nodes += s.elapsed();
    }

    // Mermaid 12 renderers append shared resources after the graph wrapper. ELK changes
    // only the layout geometry and edge z-order; it does not create a second top-level painter.
    typography_theme
        .css_binding()
        .look_defs
        .write_shadow_defs(&mut out, diagram_id)?;
    typography_theme
        .css_binding()
        .look_defs
        .write_gradient(&mut out, diagram_id)?;
    emit.checkpoint()?;

    drop(render_guard);
    let viewbox_guard = timing.section(&mut timings.viewbox);

    let view_box = class_viewbox(ClassViewBoxContext {
        content_bounds,
        viewport_padding: settings.viewport_padding,
        diagram_title,
        diagram_title_bbox_x: diagram_title
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(|title| {
                if let Some(native) = &settings.native_style {
                    return measurer.measure_svg_title_bbox_x(title, &native.title_style);
                }
                let title_style = TextStyle {
                    font_family: settings.text_style().font_family.clone(),
                    // Mermaid emits `classDiagramTitleText`, while the Class stylesheet's 18px
                    // rule targets `classTitleText`; the diagram title therefore inherits the
                    // root SVG font size.
                    font_size: settings.text_style().font_size,
                    font_weight: None,
                    font_style: None,
                };
                measurer.measure_svg_title_bbox_x(title, &title_style)
            }),
    });

    // Mermaid renders the diagram title as a direct child of `<svg>` (outside the wrapper `<g>`),
    // centered in the root viewport.
    if let Some(title) = view_box.title.as_ref() {
        super::label::write_class_diagram_title(&mut out, title.x, title.y, title.text);
        out.checkpoint()?;
    }

    drop(viewbox_guard);
    let finalize_guard = timing.section(&mut timings.finalize_svg);

    let final_root_spec = root_svg::RootViewportSpec::mermaid(
        root_svg::DiagramBounds::from_view_box(
            view_box.min_x,
            view_box.min_y,
            view_box.width,
            view_box.height,
        ),
        settings.use_max_width,
    )
    .with_max_width(root_svg::RootMaxWidth::CssSixSignificant(view_box.width));
    // Fixed sizing needs the final dimensions when the root is opened. Build its small
    // prefix after measuring content rather than changing a deferred document's sizing mode.
    let mut prefix = BoundedSvgOutput::new(options.work_meter);
    #[cfg(all(feature = "layout-layered", feature = "layout-dagre"))]
    let extra_attrs: &[(&str, &str)] = if matches!(
        prepared_labels.layered,
        crate::class::layered::Paint::DagreFallback
    ) {
        &[("data-layout-fallback", "dagre")]
    } else {
        &[]
    };
    #[cfg(not(all(feature = "layout-layered", feature = "layout-dagre")))]
    let extra_attrs = &[];
    let root_document = write_class_svg_document(
        &mut prefix,
        model,
        diagram_id,
        aria_roledescription,
        root_context,
        final_root_spec,
        extra_attrs,
    )?;
    out.replace_range(0..0, &prefix.finish()?)?;

    out.push_str("</svg>");
    let out = out.finish()?;
    drop(finalize_guard);

    if let Some(s) = total_timer {
        timings.total = s.elapsed();
        emit_class_render_timing(&timings, &detail, layout);
    }
    root_document.complete(out)
}
