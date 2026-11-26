//! Shoal paint derived from the visible cluster surfaces and the final edge geometry.

use super::defs::FlowchartMarkerEmissionPlan;
use super::render_input::FlowchartRenderEdgeRef;
use super::*;
use crate::flowchart::FlowchartEdgeKey;
use crate::theme::Color;
use kurbo::Rect;

pub(super) const CASING_WIDTH_ADD: f64 = 3.0;

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
        marker_plan: &FlowchartMarkerEmissionPlan,
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
            marker_plan.push_casing_marker_attributes_for(
                out,
                ctx.document_ids().marker_scope(),
                ctx.diagram_type,
                edge.key,
                ctx.work_meter,
            )?;
            out.push_str("/>");
            out.checkpoint()?;
        }
        out.push_str("</g>");
        out.checkpoint()
    }

    #[cfg(not(feature = "layout-shoal"))]
    pub(in crate::svg::parity::flowchart) fn applies(_ctx: &FlowchartRenderCtx<'_>) -> bool {
        false
    }

    #[cfg(not(feature = "layout-shoal"))]
    pub(in crate::svg::parity::flowchart) fn prepare(
        _ctx: &FlowchartRenderCtx<'_>,
        _hierarchy: &FlowchartHierarchyPlan<'_>,
        _cache: &FxHashMap<FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
        _options: &SvgExecution<'_>,
    ) -> Result<Self> {
        Ok(Self::default())
    }
}

#[cfg(feature = "layout-shoal")]
mod shoal {
    use super::*;
    use crate::graph_layout::RouteContract;
    use crate::graph_layout::ir::{BorderStitch, Endpoint, Side};
    use crate::theme::edge_label_bg::{literal_fill, literal_opacity};
    use kurbo::{Point, RoundedRect, Shape, Vec2};
    use merman_layout_shoal::geom::{Bounds as SpatialBounds, Bvh, BvhEntry};
    use merman_model::families::flowchart::FlowEdgeVisibility;
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
        frame: RoundedRect,
        fill: Option<Color>,
        outside: Option<Color>,
        paint_order: usize,
        depth: usize,
    }

    impl FlowchartEdgeFillPlan {
        pub(in crate::svg::parity::flowchart) fn applies(ctx: &FlowchartRenderCtx<'_>) -> bool {
            matches!(ctx.layout.route_contract, RouteContract::CanonicalAxis(_))
                && ctx.swimlane_layout.is_none()
                && !ctx.compatibility.look.is_hand_drawn()
                && ctx.recursive_clusters.is_empty()
                && !ctx.layout.clusters.is_empty()
                && (ctx.native.is_some()
                    || (ctx.diagram_type != "agentflow" && ctx.palette.color_count() == 0))
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

        pub(in crate::svg::parity::flowchart) fn prepare(
            ctx: &FlowchartRenderCtx<'_>,
            hierarchy: &FlowchartHierarchyPlan<'_>,
            cache: &FxHashMap<FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
            options: &SvgExecution<'_>,
        ) -> Result<Self> {
            let RouteContract::CanonicalAxis(axis) = &ctx.layout.route_contract else {
                return Ok(Self::default());
            };
            let mut plan = Self::default();
            let Some(default_ink) = literal_fill(&ctx.compatibility.line_color)
                .filter(|color| color.channels()[3] == u8::MAX)
            else {
                return Ok(plan);
            };
            let canvas = solid_canvas(options);
            let groups = &ctx.layout.clusters;
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
                let radius = ctx
                    .native
                    .map_or(0.0, |native| native.form.container_radius);
                let frame = RoundedRect::from_rect(bounds, radius);
                let Some((fill, opacity)) = surface_paint(ctx, &cluster.id)? else {
                    // A CSS geometry or compositing override can put paint outside the layout box.
                    return Ok(plan);
                };
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
                let fill = fill.and_then(|fill| {
                    if opacity == 1.0 && fill.channels()[3] == u8::MAX {
                        Some(fill)
                    } else {
                        outside.map(|outside| fill.composite(outside, opacity))
                    }
                });
                surfaces[index] = Some(Surface {
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
                let first = axis
                    .border_stitches
                    .partition_point(|stitch| stitch.edge < occurrence);
                let stitches = &axis.border_stitches[first..];
                let stitches =
                    &stitches[..stitches.partition_point(|stitch| stitch.edge == occurrence)];
                ctx.work_meter.charge(stitches.len())?;

                if !stitches.is_empty() {
                    if flowchart_neo_mask_uses_dash_pairs(edge.edge)
                        || source_ink
                            .map_or(Some(default_ink), literal_fill)
                            .is_none_or(|ink| ink.channels()[3] != u8::MAX)
                    {
                        continue;
                    }
                    if let Some(color) =
                        casing_color(stitches, &surfaces, &cached.geom, shaft_bounds, offset)
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
                                bounds =
                                    bounds.union(Rect::from_points(tip, tip).inflate(outset, outset));
                            }
                        }
                        plan.casing_bounds =
                            Some(plan.casing_bounds.map_or(bounds, |old| old.union(bounds)));
                        plan.paint
                            .insert(edge.key, EdgeFillPaint::Casing { color, width });
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
                if contains_rect(surface.frame, stroke_bounds)
                    && uniform_surface(surface_index, surface, stroke_bounds, ctx)?
                {
                    plan.paint.insert(edge.key, EdgeFillPaint::Ink(ink));
                }
            }
            Ok(plan)
        }
    }

    fn solid_canvas(options: &SvgExecution<'_>) -> Option<Color> {
        let session = options.family.session();
        if session.native_theme().is_some_and(|theme| {
            theme
                .resolved()
                .canvas()
                .textures
                .iter()
                .any(|texture| !matches!(texture, Texture::None))
        }) {
            return None;
        }
        session
            .canvas_background()
            .or_else(|| match &options.native_family_theme()?.roles.canvas {
                Paint::Solid(color) => Some(*color),
                Paint::LinearGradient(_) => None,
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
        let mut fill = literal_fill(&terminal.fill);
        let mut opacity = Some(1.0);
        let mut fill_opacity = Some(1.0);
        let mut proven_frame = true;
        // The rectangle appends this prepared inline style after the theme fill. Its emitted order
        // is authoritative when source spellings normalize to the same CSS property.
        crate::mermaid_style::visit_parsed_style_declarations(&terminal.rect_style, |decl| {
            match decl.property() {
                "fill" => fill = literal_fill(decl.value()),
                "opacity" => opacity = literal_opacity(decl.value()),
                "fill-opacity" => fill_opacity = literal_opacity(decl.value()),
                "stroke" | "stroke-width" | "stroke-opacity" | "stroke-dasharray"
                | "stroke-dashoffset" | "stroke-linejoin" | "stroke-linecap"
                | "stroke-miterlimit" => {}
                _ => proven_frame = false,
            }
        });
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

    fn casing_color(
        stitches: &[BorderStitch],
        surfaces: &[Option<Surface>],
        geom: &FlowchartEdgePathGeom,
        bounds: Rect,
        offset: Vec2,
    ) -> Option<Color> {
        // With nested crossings, use the outermost proven surface entered or left by this route.
        stitches
            .iter()
            .filter_map(|stitch| {
                let surface = surfaces.get(stitch.group as usize).copied().flatten()?;
                let point = geom.data_points.get(stitch.point)?;
                let point = Point::new(point.x, point.y) + offset;
                (significant_boundary(surface)
                    && crosses_face(surface.frame, point, stitch.side, bounds))
                .then_some((surface.depth, surface.outside?))
            })
            .min_by_key(|(depth, _)| *depth)
            .map(|(_, color)| color)
    }

    fn crosses_face(frame: RoundedRect, point: Point, side: Side, shaft: Rect) -> bool {
        let rect = frame.rect();
        let radius = frame.radii().top_left;
        let (normal, face, tangent, lo, hi, shaft_lo, shaft_hi) = match side {
            Side::Top => (
                point.y, rect.y0, point.x, rect.x0, rect.x1, shaft.y0, shaft.y1,
            ),
            Side::Bottom => (
                point.y, rect.y1, point.x, rect.x0, rect.x1, shaft.y0, shaft.y1,
            ),
            Side::Left => (
                point.x, rect.x0, point.y, rect.y0, rect.y1, shaft.x0, shaft.x1,
            ),
            Side::Right => (
                point.x, rect.x1, point.y, rect.y0, rect.y1, shaft.x0, shaft.x1,
            ),
        };
        // The canonical painter retains a vertex at each stitch. Validate it against the actual
        // frame's straight face because native corner rounding can differ from the layout box.
        const EPSILON: f64 = 1e-6;
        (normal - face).abs() <= EPSILON
            && lo + radius < tangent
            && tangent < hi - radius
            && shaft_lo + EPSILON < face
            && face + EPSILON < shaft_hi
    }

    fn edge_ink_bounds(
        edge: &crate::flowchart::FlowEdge,
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

    fn uniform_surface(
        index: &Bvh<Surface>,
        surface: Surface,
        bounds: Rect,
        ctx: &FlowchartRenderCtx<'_>,
    ) -> Result<bool> {
        let result = index.visit(|entry| -> Result<_> {
            ctx.work_meter.charge(1)?;
            Ok(match entry {
                BvhEntry::Node(node) => {
                    let [x0, x1] = node.x;
                    let [y0, y1] = node.y;
                    Continue(bounds.overlaps(Rect::new(x0, y0, x1, y1)))
                }
                BvhEntry::Item(other)
                    if other.paint_order > surface.paint_order
                        && other.fill != surface.fill
                        && bounds.overlaps(other.frame.rect()) =>
                {
                    Break(())
                }
                BvhEntry::Item(_) => Continue(true),
            })
        })?;
        Ok(!result.is_break())
    }
}
