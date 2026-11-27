//! Shoal paint derived from the visible cluster surfaces and the final edge geometry.

use super::render_input::FlowchartRenderEdgeRef;
use super::*;
use crate::markers::FlowchartMarkerEmissionPlan;
use crate::projection::FlowchartEdgeKey;
use kurbo::Rect;
use merman_theme_contract::v2::Color;

#[derive(Clone, Copy)]
#[cfg_attr(
    not(feature = "layout-shoal"),
    allow(dead_code, reason = "Only Shoal produces fill-aware paint.")
)]
enum EdgeFillPaint {
    Ink(&'static str),
    Casing { color: Color, width: f64 },
}

#[derive(Default)]
pub(super) struct FlowchartEdgeFillPlan {
    paint: FxHashMap<FlowchartEdgeKey, EdgeFillPaint>,
    casing_bounds: Option<Rect>,
}

impl FlowchartEdgeFillPlan {
    pub(super) fn ink(&self, key: FlowchartEdgeKey) -> Option<&'static str> {
        match self.paint.get(&key) {
            Some(&EdgeFillPaint::Ink(ink)) => Some(ink),
            _ => None,
        }
    }

    pub(super) fn casing_color(&self, key: FlowchartEdgeKey) -> Option<Color> {
        match self.paint.get(&key) {
            Some(&EdgeFillPaint::Casing { color, .. }) => Some(color),
            _ => None,
        }
    }

    pub(super) fn casing_bounds(&self) -> Option<Rect> {
        self.casing_bounds
    }

    pub(super) fn write_casings(
        &self,
        out: &mut impl SvgOutput,
        ctx: &FlowchartRenderCtx<'_>,
        marker_plan: &FlowchartMarkerEmissionPlan<'_>,
        edges: &[FlowchartRenderEdgeRef<'_>],
        cache: &FxHashMap<FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
    ) -> Result<()> {
        if self.casing_bounds.is_none() {
            return Ok(());
        }
        ctx.work_meter.charge(edges.len())?;
        out.push_str(r#"<g class="merman-edge-casings" aria-hidden="true" pointer-events="none">"#);
        out.checkpoint()?;
        for edge in edges {
            let Some(&EdgeFillPaint::Casing { color, width }) = self.paint.get(&edge.key) else {
                continue;
            };
            ctx.checkpoint_emit()?;
            let geom = &cache[&edge.key].geom;
            let d = geom.emitted_d_for_label.as_deref().unwrap_or(&geom.d);
            let _ = write!(
                out,
                r#"<path d="{d}" fill="none" stroke="{color}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round""#,
                fmt_display(width),
            );
            // Match Neo's arrowhead shaft setback without emitting a zero-length leading dash,
            // which under `stroke-linecap="round"` would paint an unwanted dot at the start tip.
            if ctx.compatibility.look.is_neo()
                && let Some(len) = geom.centerline_length()
            {
                let (start_arrow, end_arrow) =
                    super::edge_geom::arrow_types_for_edge(edge.edge.edge_type.as_deref());
                let start_offset = super::render::flowchart_neo_marker_mask_offset(start_arrow);
                let end_offset = super::render::flowchart_neo_marker_mask_offset(end_arrow);
                let middle = len - start_offset - end_offset;
                if (start_offset > 0.0 || end_offset > 0.0) && middle > 0.0 {
                    let gap = len + 16.0;
                    if start_offset > 0.0 {
                        let _ = write!(
                            out,
                            r#" style="stroke-dasharray: {} {}; stroke-dashoffset: -{};""#,
                            fmt_display(middle),
                            fmt_display(gap),
                            fmt_display(start_offset),
                        );
                    } else {
                        let _ = write!(
                            out,
                            r#" style="stroke-dasharray: {} {};""#,
                            fmt_display(middle),
                            fmt_display(gap),
                        );
                    }
                }
            }
            marker_plan.push_casing_marker_attributes_for(out, edge.key)?;
            out.push_str("/>");
            out.checkpoint()?;
        }
        out.push_str("</g>");
        out.checkpoint()
    }

    #[cfg(not(feature = "layout-shoal"))]
    pub(in crate::svg) fn applies(_ctx: &FlowchartRenderCtx<'_>) -> bool {
        false
    }

    #[cfg(not(feature = "layout-shoal"))]
    pub(in crate::svg) fn prepare(
        _ctx: &FlowchartRenderCtx<'_>,
        _hierarchy: &FlowchartHierarchyPlan<'_>,
        _cache: &FxHashMap<FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
        _services: &FlowchartSvgServices<'_>,
    ) -> Result<Self> {
        Ok(Self::default())
    }
}

#[cfg(feature = "layout-shoal")]
mod shoal {
    use super::*;
    use crate::markers::CASING_WIDTH_ADD;
    use kurbo::{Point, RoundedRect, Shape, Vec2};
    use merman_model::families::flowchart::FlowEdgeVisibility;
    use merman_render_core::graph_layout::spatial::{Bounds as SpatialBounds, Bvh, BvhEntry};
    use merman_render_core::route_types::{BorderStitch, Endpoint, Side};
    use merman_render_core::theme_svg::edge_label_background::{literal_fill, literal_opacity};
    use merman_theme_contract::v2::{Paint, Texture};
    use std::ops::ControlFlow::{Break, Continue};

    // Treat subtle tint changes as one surface for casing purposes.
    const FILL_CONTRAST: f64 = 1.5;
    // WCAG non-text contrast, used to retain the theme's own connector ink when it reads well.
    const EDGE_CONTRAST: f64 = 3.0;
    const WHITE: Color = Color::rgb(255, 255, 255);
    const BLACK: Color = Color::rgb(0, 0, 0);

    #[derive(Clone, Copy)]
    struct Surface {
        index: usize,
        frame: RoundedRect,
        fill: Option<Color>,
        outside: Option<Color>,
        paint_order: usize,
        depth: usize,
    }

    impl FlowchartEdgeFillPlan {
        pub(in crate::svg) fn applies(ctx: &FlowchartRenderCtx<'_>) -> bool {
            matches!(
                ctx.layout.route_contract,
                FlowchartSvgRoute::CanonicalAxis(_)
            ) && ctx.swimlane_layout.is_none()
                && !ctx.compatibility.look.is_hand_drawn()
                && ctx.recursive_clusters.is_empty()
                && !ctx.layout.clusters.is_empty()
                && !ctx
                    .config
                    .get_str("themeCSS")
                    .is_some_and(|css| !css.trim().is_empty())
                && ![
                    "root",
                    "clusters",
                    "cluster",
                    "flow-cluster",
                    "edgePaths",
                    "marker",
                    "cross",
                    "openArrow",
                    "hollowCircle",
                    ctx.diagram_type,
                ]
                .iter()
                .any(|class| ctx.class_defs.contains_key(*class))
        }

        pub(in crate::svg) fn prepare(
            ctx: &FlowchartRenderCtx<'_>,
            hierarchy: &FlowchartHierarchyPlan<'_>,
            cache: &FxHashMap<FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
            services: &FlowchartSvgServices<'_>,
        ) -> Result<Self> {
            let FlowchartSvgRoute::CanonicalAxis(axis) = ctx.layout.route_contract else {
                return Ok(Self::default());
            };
            let mut plan = Self::default();
            let Some(default_ink) = literal_fill(&ctx.compatibility.line_color)
                .filter(|color| color.channels()[3] == u8::MAX)
            else {
                return Ok(plan);
            };
            let canvas = solid_canvas(ctx, services);
            let groups = ctx.layout.clusters;
            ctx.work_meter.charge(groups.len())?;
            let indices: FxHashMap<_, _> = groups
                .iter()
                .enumerate()
                .map(|(index, group)| (group.id.as_str(), index))
                .collect();
            let mut surfaces: Vec<Option<Surface>> = vec![None; groups.len()];
            for (paint_order, cluster) in hierarchy.root(None)?.clusters().iter().enumerate() {
                ctx.checkpoint_emit()?;
                ctx.work_meter.charge(1)?;
                let index = indices[cluster.id.as_str()];
                let bounds =
                    super::super::cluster_geometry::painted_cluster_frame(ctx, cluster).bounds();
                let radius = super::super::cluster_geometry::cluster_container_radius(
                    ctx,
                    bounds.width(),
                    bounds.height(),
                );
                let frame = RoundedRect::from_rect(bounds, radius);
                let paint = surface_paint(ctx, &cluster.id)?;
                let parent = axis.group_parents[index].map(|parent| parent as usize);
                let (outside, depth) = if let Some(parent) = parent {
                    let Some(parent) = surfaces[parent] else {
                        return Ok(plan);
                    };
                    (
                        contains_rect(parent.frame, bounds)
                            .then_some(parent.fill)
                            .flatten(),
                        parent.depth + 1,
                    )
                } else {
                    (canvas, 0)
                };
                // Isolate a non-allowlisted cluster style (e.g. `rx:8`) by marking only its
                // own surface unproven (`fill = None`) instead of aborting the whole diagram.
                let fill = paint.and_then(|(fill, opacity)| {
                    fill.and_then(|fill| {
                        if opacity == 1.0 && fill.channels()[3] == u8::MAX {
                            Some(fill)
                        } else {
                            outside.map(|outside| fill.composite(outside, opacity))
                        }
                    })
                });
                surfaces[index] = Some(Surface {
                    index,
                    frame,
                    fill,
                    outside,
                    paint_order,
                    depth,
                });
            }
            if !surfaces.iter().flatten().any(|surface| {
                significant_boundary(*surface)
                    || adaptive_ink(*surface, canvas, default_ink).is_some()
            }) {
                return Ok(plan);
            }
            let surface_index = if surfaces
                .iter()
                .flatten()
                .any(|surface| adaptive_ink(*surface, canvas, default_ink).is_some())
            {
                let levels = groups.len().checked_ilog2().unwrap_or(0) as usize + 1;
                let work = groups
                    .len()
                    .checked_mul(levels)
                    .ok_or_else(|| ctx.work_meter.arithmetic_overflow())?;
                ctx.work_meter.charge(work)?;
                Some(
                    Bvh::build(surfaces.iter().flatten().copied().collect(), |surface| {
                        spatial_bounds(surface.frame.rect())
                    })
                    .map_err(|error| Error::InvalidModel {
                        message: format!("could not index Shoal container paint: {error}"),
                    })?,
                )
            } else {
                None
            };
            let endpoint_parent = |endpoint| match endpoint {
                Endpoint::Node(id) => ctx
                    .layout
                    .nodes
                    .get(id as usize)
                    .and_then(|node| ctx.parent.get(node.id.as_str()))
                    .and_then(|parent| indices.get(parent))
                    .copied(),
                Endpoint::Group(id) => {
                    axis.group_parents[id as usize].map(|parent| parent as usize)
                }
            };

            for &edge in hierarchy.ordered_edges() {
                ctx.checkpoint_emit()?;
                ctx.work_meter.charge(1)?;
                let style = ctx.edge_style_plan.edge_for(edge.key)?;
                if edge.visibility == FlowEdgeVisibility::Invisible
                    || !style.has_simple_edge_paint()
                    || ctx.edge_style_plan.animation_for(edge.key)?.is_active()
                {
                    continue;
                }
                let Some(half_width) = ctx
                    .edge_style_plan
                    .stroke_outset_for(edge.key)?
                    .filter(|width| *width > 0.0)
                else {
                    continue;
                };
                let occurrence = edge.key.semantic_index();
                let Some((route, cached)) = axis
                    .routes
                    .get(occurrence)
                    .and_then(Option::as_ref)
                    .zip(cache.get(&edge.key))
                else {
                    continue;
                };
                let Some(bounds) = cached.geom.pb else {
                    continue;
                };
                let offset = Vec2::new(cached.origin_x - ctx.tx, cached.origin_y - ctx.ty);
                let shaft_bounds =
                    Rect::new(bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y) + offset;
                let source_ink = style.edge_marker_color(false);
                let Some(edge_ink) = source_ink
                    .map_or(Some(default_ink), literal_fill)
                    .filter(|ink| ink.channels()[3] == u8::MAX)
                else {
                    continue;
                };
                let first = axis
                    .border_stitches
                    .partition_point(|stitch| stitch.edge < occurrence);
                let stitches = &axis.border_stitches[first..];
                let stitches =
                    &stitches[..stitches.partition_point(|stitch| stitch.edge == occurrence)];
                ctx.work_meter.charge(stitches.len())?;

                if !stitches.is_empty() {
                    if !flowchart_neo_mask_uses_dash_pairs(edge.edge)
                        && let Some(color) = casing_color(
                            stitches,
                            &surfaces,
                            &cached.geom,
                            shaft_bounds,
                            offset,
                            edge_ink,
                        )
                    {
                        let width = 2.0 * half_width + CASING_WIDTH_ADD;
                        let doc_offset = Vec2::new(cached.origin_x, cached.abs_top_transform);
                        let doc_shaft =
                            Rect::new(bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y)
                                + doc_offset;
                        let mut bounds = doc_shaft.inflate(width / 2.0, width / 2.0);
                        let (start_marker, end_marker) = flowchart_marker_metrics(edge.edge);
                        for (marker, point) in [
                            (start_marker, cached.geom.data_points.first()),
                            (end_marker, cached.geom.data_points.last()),
                        ] {
                            if marker.depth > 0.0
                                && let Some(point) = point
                            {
                                let tip = Point::new(point.x, point.y) + doc_offset;
                                let outset = marker.depth + marker.trim + CASING_WIDTH_ADD / 2.0;
                                bounds = bounds
                                    .union(Rect::from_points(tip, tip).inflate(outset, outset));
                            }
                        }
                        plan.casing_bounds =
                            Some(plan.casing_bounds.map_or(bounds, |old| old.union(bounds)));
                        plan.paint
                            .insert(edge.key, EdgeFillPaint::Casing { color, width });
                        continue;
                    }
                    if source_ink.is_none()
                        && let Some(ink) = uniform_stitched_adaptive_ink(
                            route,
                            stitches,
                            &surfaces,
                            &axis.group_parents,
                            endpoint_parent,
                            canvas,
                            default_ink,
                        )
                    {
                        plan.paint.insert(edge.key, EdgeFillPaint::Ink(ink));
                    }
                    continue;
                }
                let Some(index) = endpoint_parent(route.endpoints.0)
                    .filter(|&index| Some(index) == endpoint_parent(route.endpoints.1))
                else {
                    continue;
                };
                if source_ink.is_some() {
                    continue;
                }
                let Some((surface, surface_index)) = surfaces[index].zip(surface_index.as_ref())
                else {
                    continue;
                };
                let Some(ink) = adaptive_ink(surface, canvas, default_ink) else {
                    continue;
                };
                let stroke_bounds =
                    edge_ink_bounds(edge.edge, &cached.geom, shaft_bounds, offset, half_width);
                let endpoint_clusters = [
                    match route.endpoints.0 {
                        Endpoint::Group(id) => Some(id as usize),
                        Endpoint::Node(_) => None,
                    },
                    match route.endpoints.1 {
                        Endpoint::Group(id) => Some(id as usize),
                        Endpoint::Node(_) => None,
                    },
                ];
                if contains_rect(surface.frame, stroke_bounds)
                    && uniform_surface(
                        surface_index,
                        UniformSurfaceQuery {
                            surface,
                            ink,
                            bounds: stroke_bounds,
                            endpoint_clusters,
                            group_parents: &axis.group_parents,
                            data_points: &cached.geom.data_points,
                            offset,
                            half_width,
                            canvas,
                            default_ink,
                        },
                        ctx,
                    )?
                {
                    plan.paint.insert(edge.key, EdgeFillPaint::Ink(ink));
                }
            }
            Ok(plan)
        }
    }

    fn solid_canvas(
        ctx: &FlowchartRenderCtx<'_>,
        services: &FlowchartSvgServices<'_>,
    ) -> Option<Color> {
        if services.native_canvas.is_some_and(|canvas| {
            canvas
                .textures
                .iter()
                .any(|texture| !matches!(texture, Texture::None))
        }) {
            return None;
        }
        services
            .canvas_background
            .or_else(|| {
                if let Some(native) = services.native_family_theme {
                    match &native.roles.canvas {
                        Paint::Solid(color) => Some(*color),
                        Paint::LinearGradient(_) => None,
                    }
                } else {
                    let configured = merman_render_core::config::value_at(
                        ctx.config,
                        &["themeVariables", "background"],
                    )
                    .and_then(serde_json::Value::as_str)
                    .and_then(literal_fill);
                    configured
                        .or_else(|| Some(super::super::render::compat_theme_canvas_fallback(ctx)))
                }
            })
            .filter(|color| color.channels()[3] == u8::MAX)
    }

    fn surface_paint(
        ctx: &FlowchartRenderCtx<'_>,
        id: &str,
    ) -> Result<Option<(Option<Color>, f64)>> {
        let terminal = ctx.prepared_nodes.cluster(id)?;
        ctx.work_meter
            .charge(terminal.rect_style.len().div_ceil(64) + 1)?;
        let mut fill = literal_fill(super::super::render::cluster_base_fill(ctx, terminal, id));
        let mut opacity = Some(1.0);
        let mut fill_opacity = Some(1.0);
        let mut proven_frame = true;
        // The rectangle appends this prepared inline style after the theme fill. Its emitted order
        // is authoritative when source spellings normalize to the same CSS property.
        merman_render_core::mermaid_style::visit_parsed_style_declarations(
            &terminal.rect_style,
            |decl| match decl.property() {
                "fill" => fill = literal_fill(decl.value()),
                "opacity" => opacity = literal_opacity(decl.value()),
                "fill-opacity" => fill_opacity = literal_opacity(decl.value()),
                "stroke" | "stroke-width" | "stroke-opacity" | "stroke-dasharray"
                | "stroke-dashoffset" | "stroke-linejoin" | "stroke-linecap"
                | "stroke-miterlimit" => {}
                _ => proven_frame = false,
            },
        );
        let opacity = opacity.zip(fill_opacity).map(|(a, b)| a * b);
        Ok(proven_frame.then(|| {
            (
                fill.filter(|_| opacity.is_some()),
                opacity.unwrap_or_default(),
            )
        }))
    }

    fn contains_rect(frame: RoundedRect, rect: Rect) -> bool {
        [
            Point::new(rect.x0, rect.y0),
            Point::new(rect.x0, rect.y1),
            Point::new(rect.x1, rect.y0),
            Point::new(rect.x1, rect.y1),
        ]
        .into_iter()
        .all(|point| frame.contains(point))
    }

    fn significant_boundary(surface: Surface) -> bool {
        surface
            .fill
            .zip(surface.outside)
            .is_some_and(|(inside, outside)| inside.contrast(outside) >= FILL_CONTRAST)
    }

    fn adaptive_ink(
        surface: Surface,
        canvas: Option<Color>,
        default_ink: Color,
    ) -> Option<&'static str> {
        let fill = surface.fill?;
        let significant = significant_boundary(surface)
            || canvas.is_none_or(|canvas| fill.contrast(canvas) >= FILL_CONTRAST);
        if !significant || default_ink.composite(fill, 1.0).contrast(fill) >= EDGE_CONTRAST {
            return None;
        }
        Some(if WHITE.contrast(fill) >= BLACK.contrast(fill) {
            "#FFFFFF"
        } else {
            "#000000"
        })
    }

    fn uniform_stitched_adaptive_ink(
        route: &crate::graph::RouteTopology,
        stitches: &[BorderStitch],
        surfaces: &[Option<Surface>],
        group_parents: &[Option<u32>],
        endpoint_parent: impl Fn(Endpoint) -> Option<usize>,
        canvas: Option<Color>,
        default_ink: Color,
    ) -> Option<&'static str> {
        let mut required_ink = None;
        let mut observe_surface = |index: Option<usize>| -> Option<()> {
            let surface = surfaces.get(index?)?.as_ref().copied()?;
            let ink = adaptive_ink(surface, canvas, default_ink)?;
            if let Some(existing) = required_ink {
                if existing != ink {
                    return None;
                }
            } else {
                required_ink = Some(ink);
            }
            Some(())
        };
        observe_surface(endpoint_parent(route.endpoints.0))?;
        observe_surface(endpoint_parent(route.endpoints.1))?;
        for stitch in stitches {
            let group = stitch.group as usize;
            observe_surface(Some(group))?;
            let outside_group = group_parents
                .get(group)
                .copied()
                .flatten()
                .map(|parent| parent as usize);
            observe_surface(outside_group)?;
        }
        required_ink
    }

    fn casing_color(
        stitches: &[BorderStitch],
        surfaces: &[Option<Surface>],
        geom: &FlowchartEdgePathGeom,
        bounds: Rect,
        offset: Vec2,
        edge_ink: Color,
    ) -> Option<Color> {
        // With nested crossings, use the outermost proven surface entered or left by this route.
        stitches
            .iter()
            .filter_map(|stitch| {
                let surface = surfaces.get(stitch.group as usize).copied().flatten()?;
                let point = geom.data_points.get(stitch.point)?;
                let point = Point::new(point.x, point.y) + offset;
                if !(significant_boundary(surface)
                    && crosses_face(surface.frame, point, stitch.side, bounds))
                {
                    return None;
                }
                let fill = surface.fill?;
                let outside = surface.outside?;
                let color = if edge_ink.contrast(outside) >= EDGE_CONTRAST {
                    outside
                } else if edge_ink.contrast(fill) >= EDGE_CONTRAST {
                    fill
                } else if edge_ink.contrast(WHITE) >= edge_ink.contrast(BLACK) {
                    WHITE
                } else {
                    BLACK
                };
                Some((surface.depth, color))
            })
            .min_by_key(|(depth, _)| *depth)
            .map(|(_, color)| color)
    }

    fn crosses_face(frame: RoundedRect, point: Point, side: Side, shaft: Rect) -> bool {
        let rect = frame.rect();
        let radius = frame.radii().top_left;
        let (normal, face, inward, tangent, lo, hi, shaft_lo, shaft_hi) = match side {
            Side::Top => (
                point.y, rect.y0, 1.0, point.x, rect.x0, rect.x1, shaft.y0, shaft.y1,
            ),
            Side::Bottom => (
                point.y, rect.y1, -1.0, point.x, rect.x0, rect.x1, shaft.y0, shaft.y1,
            ),
            Side::Left => (
                point.x, rect.x0, 1.0, point.y, rect.y0, rect.y1, shaft.x0, shaft.x1,
            ),
            Side::Right => (
                point.x, rect.x1, -1.0, point.y, rect.y0, rect.y1, shaft.x0, shaft.x1,
            ),
        };
        // The canonical painter retains a vertex at each stitch. Validate it against the actual
        // frame's straight face or rounded corner quarter-circle arc.
        const EPSILON: f64 = 1e-6;
        if !(shaft_lo + EPSILON < normal && normal + EPSILON < shaft_hi) {
            return false;
        }
        if (normal - face).abs() <= EPSILON && lo + radius < tangent && tangent < hi - radius {
            return true;
        }
        if radius <= 0.0 {
            return false;
        }
        let inward_depth = (normal - face) * inward;
        if !(-EPSILON..=radius + EPSILON).contains(&inward_depth) {
            return false;
        }
        let center_normal = face + inward * radius;
        let on_lo_arc = (lo - EPSILON..=lo + radius + EPSILON).contains(&tangent)
            && ((normal - center_normal).hypot(tangent - (lo + radius)) - radius).abs() <= EPSILON;
        let on_hi_arc = (hi - radius - EPSILON..=hi + EPSILON).contains(&tangent)
            && ((normal - center_normal).hypot(tangent - (hi - radius)) - radius).abs() <= EPSILON;
        on_lo_arc || on_hi_arc
    }

    fn edge_ink_bounds(
        edge: &merman_model::families::flowchart::FlowEdge,
        geom: &FlowchartEdgePathGeom,
        shaft: Rect,
        offset: Vec2,
        half_width: f64,
    ) -> Rect {
        let (start, end) = flowchart_marker_metrics(edge);
        let mut bounds = shaft.inflate(half_width, half_width);
        for (marker, point) in [
            (start, geom.data_points.first()),
            (end, geom.data_points.last()),
        ] {
            if marker.depth > 0.0
                && let Some(point) = point
            {
                let tip = Point::new(point.x, point.y) + offset;
                let outset = marker.depth + marker.trim;
                bounds = bounds.union(Rect::from_points(tip, tip).inflate(outset, outset));
            }
        }
        bounds
    }

    fn spatial_bounds(rect: Rect) -> SpatialBounds {
        SpatialBounds {
            x: [rect.x0, rect.x1],
            y: [rect.y0, rect.y1],
        }
    }

    struct UniformSurfaceQuery<'a> {
        surface: Surface,
        ink: &'static str,
        bounds: Rect,
        endpoint_clusters: [Option<usize>; 2],
        group_parents: &'a [Option<u32>],
        data_points: &'a [merman_render_core::LayoutPoint],
        offset: Vec2,
        half_width: f64,
        canvas: Option<Color>,
        default_ink: Color,
    }

    fn is_endpoint_cluster_or_descendant(
        mut cluster_index: usize,
        endpoint_clusters: [Option<usize>; 2],
        group_parents: &[Option<u32>],
    ) -> bool {
        if endpoint_clusters[0].is_none() && endpoint_clusters[1].is_none() {
            return false;
        }
        loop {
            if endpoint_clusters[0] == Some(cluster_index)
                || endpoint_clusters[1] == Some(cluster_index)
            {
                return true;
            }
            let Some(parent) = group_parents
                .get(cluster_index)
                .copied()
                .flatten()
                .map(|parent| parent as usize)
            else {
                return false;
            };
            cluster_index = parent;
        }
    }

    fn segment_intersects_rect(a: Point, b: Point, rect: Rect) -> bool {
        if rect.x0 > rect.x1 || rect.y0 > rect.y1 {
            return false;
        }
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let mut t0 = 0.0_f64;
        let mut t1 = 1.0_f64;
        for (p, q) in [
            (-dx, a.x - rect.x0),
            (dx, rect.x1 - a.x),
            (-dy, a.y - rect.y0),
            (dy, rect.y1 - a.y),
        ] {
            if p == 0.0 {
                if q < 0.0 {
                    return false;
                }
            } else {
                let r = q / p;
                if p < 0.0 {
                    if r > t1 {
                        return false;
                    }
                    if r > t0 {
                        t0 = r;
                    }
                } else {
                    if r < t0 {
                        return false;
                    }
                    if r < t1 {
                        t1 = r;
                    }
                }
            }
        }
        t0 <= t1
    }

    fn route_enters_rect(
        points: &[merman_render_core::LayoutPoint],
        offset: Vec2,
        rect: Rect,
        margin: f64,
    ) -> bool {
        let test_rect = rect.inflate(margin, margin);
        if test_rect.x0 > test_rect.x1 || test_rect.y0 > test_rect.y1 {
            return false;
        }
        if points.len() < 2 {
            return points
                .first()
                .is_some_and(|p| test_rect.contains(Point::new(p.x, p.y) + offset));
        }
        points.windows(2).any(|pair| {
            let a = Point::new(pair[0].x, pair[0].y) + offset;
            let b = Point::new(pair[1].x, pair[1].y) + offset;
            segment_intersects_rect(a, b, test_rect)
        })
    }

    fn uniform_surface(
        index: &Bvh<Surface>,
        query: UniformSurfaceQuery<'_>,
        ctx: &FlowchartRenderCtx<'_>,
    ) -> Result<bool> {
        let result = index.visit(|entry| -> Result<_> {
            ctx.work_meter.charge(1)?;
            Ok(match entry {
                BvhEntry::Node(node) => {
                    let [x0, x1] = node.x;
                    let [y0, y1] = node.y;
                    Continue(query.bounds.overlaps(Rect::new(x0, y0, x1, y1)))
                }
                BvhEntry::Item(other)
                    if other.paint_order > query.surface.paint_order
                        && other.fill != query.surface.fill
                        && adaptive_ink(*other, query.canvas, query.default_ink)
                            != Some(query.ink)
                        && query.bounds.overlaps(other.frame.rect()) =>
                {
                    // An edge terminating on its own source/target child cluster touches that
                    // boundary (and its inflated marker box extends inward). Ignore boundary
                    // touch unless the route centerline actually enters that cluster's interior.
                    let margin = if is_endpoint_cluster_or_descendant(
                        other.index,
                        query.endpoint_clusters,
                        query.group_parents,
                    ) {
                        -1e-3
                    } else {
                        query.half_width
                    };
                    if route_enters_rect(
                        query.data_points,
                        query.offset,
                        other.frame.rect(),
                        margin,
                    ) {
                        Break(())
                    } else {
                        Continue(true)
                    }
                }
                BvhEntry::Item(_) => Continue(true),
            })
        })?;
        Ok(!result.is_break())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn crosses_face_accepts_rounded_corner_arc_stitch_points() {
            let frame = RoundedRect::from_rect(Rect::new(0.0, 0.0, 100.0, 80.0), 10.0);
            let shaft = Rect::new(0.0, -20.0, 20.0, 40.0);
            // 45° along the top-left corner arc centered at (10, 10) with radius 10.
            let offset = 10.0 - 10.0 * std::f64::consts::FRAC_1_SQRT_2;
            let arc_point = Point::new(offset, offset);
            assert!(crosses_face(frame, arc_point, Side::Top, shaft));
            assert!(crosses_face(
                frame,
                arc_point,
                Side::Left,
                Rect::new(-20.0, 0.0, 40.0, 20.0)
            ));
            // Sharp outer corner (0, 0) lies outside the rounded corner arc and must be rejected.
            assert!(!crosses_face(frame, Point::new(0.0, 0.0), Side::Top, shaft));
        }
    }
}
