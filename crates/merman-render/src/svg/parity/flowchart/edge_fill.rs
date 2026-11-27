//! Shoal paint derived from the visible cluster surfaces and the final edge geometry.

use super::defs::FlowchartMarkerEmissionPlan;
use super::render_input::FlowchartRenderEdgeRef;
use super::*;
use crate::flowchart::FlowchartEdgeKey;
use crate::theme::Color;
use kurbo::Rect;
use std::borrow::Cow;

pub(super) const CASING_WIDTH_ADD: f64 = 3.0;

#[cfg(feature = "layout-shoal")]
pub(super) use shoal::{
    EDGE_CONTRAST, contains_rect, contrasting_casing, rect_surface_paint, segment_enters_surface,
    spatial_bounds,
};

#[cfg_attr(
    not(feature = "layout-shoal"),
    allow(dead_code, reason = "Only Shoal produces fill-aware paint.")
)]
enum EdgeFillPaint {
    Ink(Cow<'static, str>),
    Casing { color: Color, width: f64 },
}

#[derive(Default)]
pub(super) struct FlowchartEdgeFillPlan {
    paint: FxHashMap<FlowchartEdgeKey, EdgeFillPaint>,
    casing_bounds: Option<Rect>,
}

impl FlowchartEdgeFillPlan {
    pub(super) fn ink(&self, key: FlowchartEdgeKey) -> Option<&str> {
        match self.paint.get(&key) {
            Some(EdgeFillPaint::Ink(ink)) => Some(ink),
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
        label_cutouts: &FxHashSet<FlowchartEdgeKey>,
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
            let rough_dash = ctx.compatibility.look.is_hand_drawn()
                && flowchart_neo_mask_uses_dash_pairs(edge.edge);
            let cap = if rough_dash { "butt" } else { "round" };
            let _ = write!(
                out,
                r#"<path d="{d}" fill="none" stroke="{color}" stroke-width="{}" stroke-linecap="{cap}" stroke-linejoin="round""#,
                fmt_display(width),
            );
            if rough_dash {
                let dash = if edge.edge.stroke.as_deref() == Some("dotted") {
                    2
                } else {
                    3
                };
                let _ = write!(out, r#" stroke-dasharray="{dash}""#);
            }
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
            if label_cutouts.contains(&edge.key)
                && let Some(id) = ctx.document_ids().edge(edge.key)
            {
                super::edge_label_cutout::write_attribute(out, id);
            }
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
    use kurbo::{
        BezPath, Line, ParamCurve, ParamCurveExtrema, PathSeg, Point, RoundedRect, Shape, Vec2,
    };
    use merman_layout_shoal::geom::{Bounds as SpatialBounds, Bvh, BvhEntry};
    use merman_model::families::flowchart::FlowEdgeVisibility;
    use merman_theme_contract::v2::{Paint, Texture};
    use std::ops::ControlFlow::{Break, Continue};

    // Treat subtle tint changes as one surface for casing purposes.
    const FILL_CONTRAST: f64 = 1.5;
    // WCAG non-text contrast, used to retain the theme's own connector ink when it reads well.
    pub(in crate::svg::parity::flowchart) const EDGE_CONTRAST: f64 = 3.0;
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
        pub(in crate::svg::parity::flowchart) fn applies(ctx: &FlowchartRenderCtx<'_>) -> bool {
            matches!(ctx.layout.route_contract, RouteContract::CanonicalAxis(_))
                && !(ctx.swimlane_layout.is_some() && ctx.compatibility.look.is_hand_drawn())
                && !(ctx.compatibility.look.is_hand_drawn()
                    && ctx.native.is_none()
                    && ctx.diagram_type == "agentflow")
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
            let canvas = solid_canvas(ctx, options);
            if ctx.swimlane_layout.is_some() {
                return super::super::swimlane::edge_contrast::prepare(
                    ctx,
                    hierarchy,
                    cache,
                    canvas,
                    default_ink,
                );
            }
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
                let offset = Vec2::new(cached.origin_x - ctx.tx, cached.origin_y - ctx.ty);
                let source_ink = style.edge_marker_color(ctx.compatibility.look.is_hand_drawn());
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
                let allow_casing = ctx.compatibility.look.is_hand_drawn()
                    || !flowchart_neo_mask_uses_dash_pairs(edge.edge);
                let candidate = if source_ink.is_some() {
                    None
                } else if stitches.is_empty() {
                    endpoint_parent(route.endpoints.0)
                        .filter(|&index| Some(index) == endpoint_parent(route.endpoints.1))
                        .and_then(|index| surfaces[index])
                        .and_then(|surface| {
                            Some((adaptive_ink(surface, canvas, default_ink)?, surface))
                        })
                } else {
                    uniform_stitched_adaptive_ink(
                        route,
                        stitches,
                        &surfaces,
                        &axis.group_parents,
                        endpoint_parent,
                        canvas,
                        default_ink,
                    )
                };
                if candidate.is_none() && (stitches.is_empty() || !allow_casing) {
                    continue;
                }
                let d = cached
                    .geom
                    .emitted_d_for_label
                    .as_deref()
                    .unwrap_or(&cached.geom.d);
                ctx.work_meter.charge(d.len().div_ceil(16))?;
                let shaft = BezPath::from_svg(d).map_err(|error| Error::InvalidModel {
                    message: format!("could not read painted Shoal edge path: {error}"),
                })?;
                let local_shaft_bounds = shaft.bounding_box();
                let shaft_bounds = local_shaft_bounds + offset;
                let stroke_bounds = edge_ink_bounds(
                    edge.edge,
                    &cached.geom,
                    shaft_bounds,
                    offset,
                    half_width,
                    0.0,
                );
                let endpoint_clusters =
                    [route.endpoints.0, route.endpoints.1].map(|endpoint| match endpoint {
                        Endpoint::Group(id) => Some(id as usize),
                        Endpoint::Node(_) => None,
                    });

                let mut casing = allow_casing
                    .then(|| {
                        casing_color(
                            stitches,
                            &surfaces,
                            &cached.geom,
                            shaft_bounds,
                            offset,
                            edge_ink,
                        )
                    })
                    .flatten();
                if casing.is_none()
                    && let Some(((ink, surface), surface_index)) =
                        candidate.zip(surface_index.as_ref())
                    && (!stitches.is_empty() || contains_rect(surface.frame, stroke_bounds))
                {
                    if uniform_surface(
                        surface_index,
                        UniformSurfaceQuery {
                            paint_order: surface.paint_order,
                            crosses_boundaries: !stitches.is_empty(),
                            ink,
                            bounds: stroke_bounds,
                            endpoint_clusters,
                            group_parents: &axis.group_parents,
                            shaft: &shaft,
                            offset,
                            half_width,
                        },
                        ctx,
                    )? {
                        plan.set_ink(edge.key, ink);
                    } else if allow_casing {
                        casing = Some(contrasting_casing(edge_ink));
                    }
                }
                if let Some(color) = casing {
                    plan.add_casing(edge, cached, local_shaft_bounds, half_width, color);
                }
            }
            Ok(plan)
        }

        pub(in crate::svg::parity::flowchart) fn set_ink(
            &mut self,
            key: FlowchartEdgeKey,
            ink: impl Into<Cow<'static, str>>,
        ) {
            self.paint.insert(key, EdgeFillPaint::Ink(ink.into()));
        }

        pub(in crate::svg::parity::flowchart) fn add_casing(
            &mut self,
            edge: FlowchartRenderEdgeRef<'_>,
            cached: &FlowchartEdgePathCacheEntry,
            local_shaft_bounds: Rect,
            half_width: f64,
            color: Color,
        ) {
            let width = 2.0 * half_width + CASING_WIDTH_ADD;
            let doc_offset = Vec2::new(cached.origin_x, cached.abs_top_transform);
            let bounds = edge_ink_bounds(
                edge.edge,
                &cached.geom,
                local_shaft_bounds + doc_offset,
                doc_offset,
                width / 2.0,
                CASING_WIDTH_ADD / 2.0,
            );
            self.casing_bounds = Some(self.casing_bounds.map_or(bounds, |old| old.union(bounds)));
            self.paint
                .insert(edge.key, EdgeFillPaint::Casing { color, width });
        }
    }

    fn solid_canvas(ctx: &FlowchartRenderCtx<'_>, options: &SvgExecution<'_>) -> Option<Color> {
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
            .or_else(|| {
                if let Some(native) = options.native_family_theme() {
                    match &native.roles.canvas {
                        Paint::Solid(color) => Some(*color),
                        Paint::LinearGradient(_) => None,
                    }
                } else {
                    let configured =
                        crate::config::value_at(ctx.config, &["themeVariables", "background"])
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
        let fill = literal_fill(super::super::render::cluster_base_fill(ctx, terminal, id));
        rect_surface_paint(ctx, fill, &terminal.rect_style)
    }

    pub(in crate::svg::parity::flowchart) fn rect_surface_paint(
        ctx: &FlowchartRenderCtx<'_>,
        mut fill: Option<Color>,
        rect_style: &str,
    ) -> Result<Option<(Option<Color>, f64)>> {
        ctx.work_meter.charge(rect_style.len().div_ceil(64) + 1)?;
        let mut opacity = Some(1.0);
        let mut fill_opacity = Some(1.0);
        let mut proven_frame = true;
        // The rectangle appends this prepared inline style after the theme fill. Its emitted order
        // is authoritative when source spellings normalize to the same CSS property.
        crate::mermaid_style::visit_parsed_style_declarations(rect_style, |decl| {
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

    pub(in crate::svg::parity::flowchart) fn contains_rect(frame: RoundedRect, rect: Rect) -> bool {
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
        route: &crate::graph_layout::RouteTopology,
        stitches: &[BorderStitch],
        surfaces: &[Option<Surface>],
        group_parents: &[Option<u32>],
        endpoint_parent: impl Fn(Endpoint) -> Option<usize>,
        canvas: Option<Color>,
        default_ink: Color,
    ) -> Option<(&'static str, Surface)> {
        let mut required_ink = None;
        let mut first_surface: Option<Surface> = None;
        let mut observe_surface = |index: Option<usize>| -> Option<()> {
            let surface = surfaces.get(index?)?.as_ref().copied()?;
            let ink = adaptive_ink(surface, canvas, default_ink)?;
            if first_surface.is_none_or(|first| surface.paint_order < first.paint_order) {
                first_surface = Some(surface);
            }
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
        required_ink.zip(first_surface)
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
                } else {
                    contrasting_casing(edge_ink)
                };
                Some((surface.depth, color))
            })
            .min_by_key(|(depth, _)| *depth)
            .map(|(_, color)| color)
    }

    pub(in crate::svg::parity::flowchart) fn contrasting_casing(ink: Color) -> Color {
        if ink.contrast(WHITE) >= ink.contrast(BLACK) {
            WHITE
        } else {
            BLACK
        }
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
        edge: &crate::flowchart::FlowEdge,
        geom: &FlowchartEdgePathGeom,
        shaft: Rect,
        offset: Vec2,
        half_width: f64,
        marker_outset: f64,
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
                let outset = marker.depth + marker.trim + marker_outset;
                bounds = bounds.union(Rect::from_points(tip, tip).inflate(outset, outset));
            }
        }
        bounds
    }

    pub(in crate::svg::parity::flowchart) fn spatial_bounds(rect: Rect) -> SpatialBounds {
        SpatialBounds {
            x: [rect.x0, rect.x1],
            y: [rect.y0, rect.y1],
        }
    }

    struct UniformSurfaceQuery<'a> {
        paint_order: usize,
        crosses_boundaries: bool,
        ink: &'static str,
        bounds: Rect,
        endpoint_clusters: [Option<usize>; 2],
        group_parents: &'a [Option<u32>],
        shaft: &'a BezPath,
        offset: Vec2,
        half_width: f64,
    }

    fn is_endpoint_cluster_or_descendant(
        mut cluster_index: usize,
        endpoint_clusters: [Option<usize>; 2],
        group_parents: &[Option<u32>],
        work: &crate::resources::OperationWorkMeter,
    ) -> Result<bool> {
        if endpoint_clusters[0].is_none() && endpoint_clusters[1].is_none() {
            return Ok(false);
        }
        loop {
            work.charge(1)?;
            if endpoint_clusters[0] == Some(cluster_index)
                || endpoint_clusters[1] == Some(cluster_index)
            {
                return Ok(true);
            }
            let Some(parent) = group_parents
                .get(cluster_index)
                .copied()
                .flatten()
                .map(|parent| parent as usize)
            else {
                return Ok(false);
            };
            cluster_index = parent;
        }
    }

    pub(in crate::svg::parity::flowchart) fn segment_enters_surface(
        segment: PathSeg,
        frame: RoundedRect,
        depth: u8,
        work: &crate::resources::OperationWorkMeter,
    ) -> Result<bool> {
        work.charge(1)?;
        let bounds = ParamCurveExtrema::bounding_box(&segment);
        if !bounds.is_finite() {
            return Ok(true);
        }
        let rect = frame.rect();
        if bounds.x1 < rect.x0 || bounds.x0 > rect.x1 || bounds.y1 < rect.y0 || bounds.y0 > rect.y1
        {
            return Ok(false);
        }
        if [segment.start(), segment.end(), segment.eval(0.5)]
            .into_iter()
            .any(|point| frame.contains(point))
        {
            return Ok(true);
        }
        if frame.radii().top_left == 0.0 {
            let corners = [
                (rect.x0, rect.y0),
                (rect.x1, rect.y0),
                (rect.x1, rect.y1),
                (rect.x0, rect.y1),
            ];
            return Ok((0..4).any(|i| {
                !segment
                    .intersect_line(Line::new(corners[i], corners[(i + 1) % 4]))
                    .is_empty()
            }));
        }

        // The point of the tight bounds nearest the center also has the deepest possible
        // penetration into a uniformly rounded rectangle. Prune boxes outside the corner arc.
        let center = rect.center();
        let nearest = Point::new(
            center.x.clamp(bounds.x0, bounds.x1),
            center.y.clamp(bounds.y0, bounds.y1),
        );
        if !frame.contains(nearest) {
            return Ok(false);
        }
        // Resolve any remaining round-corner contact to a subpixel box. Prefer retaining the
        // source ink at the numerical boundary instead of proving that adaptive ink is safe.
        if depth == 32 || bounds.width().max(bounds.height()) <= 1e-3 {
            return Ok(true);
        }
        let (left, right) = segment.subdivide();
        Ok(segment_enters_surface(left, frame, depth + 1, work)?
            || segment_enters_surface(right, frame, depth + 1, work)?)
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
                    if (other.paint_order > query.paint_order
                        || (query.crosses_boundaries
                            && !contains_rect(other.frame, query.bounds)))
                        && other.fill.is_none_or(|fill| {
                            let ink = if query.ink == "#FFFFFF" { WHITE } else { BLACK };
                            ink.contrast(fill) < EDGE_CONTRAST
                        })
                        && query.bounds.overlaps(other.frame.rect()) =>
                {
                    // An edge terminating on its own source/target child cluster touches that
                    // boundary (and its inflated marker box extends inward). Ignore boundary
                    // touch unless the route centerline actually enters that cluster's interior.
                    let margin = if is_endpoint_cluster_or_descendant(
                        other.index,
                        query.endpoint_clusters,
                        query.group_parents,
                        ctx.work_meter,
                    )? {
                        -1e-3
                    } else {
                        query.half_width
                    };
                    let rect = other.frame.rect().inflate(margin, margin) - query.offset;
                    if rect.width() > 0.0 && rect.height() > 0.0 {
                        let frame = RoundedRect::from_rect(
                            rect,
                            (other.frame.radii().top_left + margin).max(0.0),
                        );
                        for segment in query.shaft.segments() {
                            if segment_enters_surface(segment, frame, 0, ctx.work_meter)? {
                                return Ok(Break(()));
                            }
                        }
                    }
                    Continue(true)
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
        fn surface_test_follows_the_curve_instead_of_its_chord_or_control_polygon() {
            let work = crate::resources::OperationWorkMeter::new(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            );
            let path = BezPath::from_svg("M0,0C0,20 20,20 20,0").unwrap();
            let curve = path.segments().next().unwrap();
            for (rect, radius, expected) in [
                (Rect::new(4.0, 10.0, 6.0, 14.0), 0.0, true),
                (Rect::new(9.0, 14.9, 11.0, 15.1), 0.1, true),
                (Rect::new(9.0, 16.0, 11.0, 19.0), 0.0, false),
                (Rect::new(9.0, -0.1, 11.0, 0.1), 0.0, false),
            ] {
                assert_eq!(
                    segment_enters_surface(curve, rect.to_rounded_rect(radius), 0, &work).unwrap(),
                    expected,
                    "rect {rect:?} with radius {radius}"
                );
            }
            let corner = PathSeg::Line(Line::new((0.0, 3.0), (3.0, 0.0)));
            let rounded = Rect::new(0.0, 0.0, 40.0, 40.0).to_rounded_rect(10.0);
            assert!(!segment_enters_surface(corner, rounded, 0, &work).unwrap());
        }

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
