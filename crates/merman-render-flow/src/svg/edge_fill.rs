//! Contrast for Shoal edges on the surfaces painted by Flowchart and Swimlane.

use super::*;
use crate::markers::FlowchartMarkerSurface as SurfaceInk;
use crate::projection::FlowchartEdgeKey;
use kurbo::RoundedRect;

#[cfg(feature = "layout-shoal")]
pub(super) use shoal::{EdgeInk, segment_enters_surface, spatial_bounds};

#[derive(Default)]
pub(crate) struct FlowchartEdgeFillPlan {
    inks: FxHashMap<FlowchartEdgeKey, &'static str>,
    layers: FxHashMap<FlowchartEdgeKey, Vec<SurfaceInk>>,
    clips: Vec<Option<RoundedRect>>,
}

impl FlowchartEdgeFillPlan {
    pub(super) fn ink(&self, key: FlowchartEdgeKey) -> Option<&'static str> {
        self.inks.get(&key).copied()
    }

    pub(super) fn layers(&self, key: FlowchartEdgeKey) -> &[SurfaceInk] {
        self.layers.get(&key).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn write_clip_id(
        out: &mut impl std::fmt::Write,
        scope: &str,
        clip: usize,
    ) -> std::fmt::Result {
        write!(out, "{}-paint-surface-{clip}", escape_attr_display(scope))
    }

    pub(super) fn write_clips(
        &self,
        out: &mut impl SvgOutput,
        ctx: &FlowchartRenderCtx<'_>,
    ) -> Result<()> {
        if self.clips.is_empty() {
            return Ok(());
        }
        let scope = ctx.document_ids().scope().as_str();
        out.push_str("<defs>");
        for (clip, frame) in self.clips.iter().enumerate() {
            ctx.work_meter.charge_emit_work(1)?;
            let Some(frame) = frame else { continue };
            let rect = frame.rect();
            out.push_str(r#"<clipPath id=""#);
            let _ = Self::write_clip_id(out, scope, clip);
            let _ = write!(
                out,
                r#"" clipPathUnits="userSpaceOnUse"><rect x="{}" y="{}" width="{}" height="{}" rx="{}"/></clipPath>"#,
                fmt_display(rect.x0),
                fmt_display(rect.y0),
                fmt_display(rect.width()),
                fmt_display(rect.height()),
                fmt_display(frame.radii().top_left),
            );
            out.checkpoint()?;
        }
        out.push_str("</defs>");
        out.checkpoint()
    }

    #[cfg(not(feature = "layout-shoal"))]
    pub(super) fn applies(_ctx: &FlowchartRenderCtx<'_>) -> bool {
        false
    }

    #[cfg(not(feature = "layout-shoal"))]
    pub(super) fn prepare(
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
    use kurbo::{BezPath, Line, ParamCurve, ParamCurveExtrema, PathSeg, Point, Rect, Shape, Vec2};
    use merman_model::families::flowchart::{FlowEdge, FlowEdgeVisibility};
    use merman_render_core::graph_layout::spatial::{Bounds as SpatialBounds, Bvh, BvhEntry};
    use merman_render_core::theme_svg::edge_label_background::{literal_fill, literal_opacity};
    use merman_theme_contract::v2::Texture;
    use std::ops::ControlFlow::{Break, Continue};

    const EDGE_CONTRAST: f64 = 3.0;
    const WHITE: Color = Color::rgb(255, 255, 255);
    const BLACK: Color = Color::rgb(0, 0, 0);

    #[derive(Clone, Copy)]
    struct Surface {
        order: usize,
        frame: RoundedRect,
        fill: Option<Color>,
    }

    struct OccludingSurfaceStack {
        canvas: Option<Color>,
        count: usize,
        index: Bvh<Surface>,
    }

    pub(in crate::svg) struct EdgeInk<'a> {
        shaft: &'a BezPath,
        markers: [Option<Rect>; 2],
        offset: Vec2,
        bounds: Rect,
        half_width: f64,
    }

    impl OccludingSurfaceStack {
        fn prepare(
            ctx: &FlowchartRenderCtx<'_>,
            hierarchy: &FlowchartHierarchyPlan<'_>,
            canvas: Option<Color>,
        ) -> Result<Self> {
            let mut backdrop = FxHashMap::default();
            let clusters = hierarchy.root(None)?.clusters();
            ctx.work_meter.charge(clusters.len())?;
            let mut surfaces = Vec::new();
            for cluster in clusters {
                ctx.checkpoint_emit()?;
                let id = cluster.id.as_str();
                let terminal = ctx.prepared_nodes.cluster(id)?;
                let outside = ctx
                    .parent
                    .get(id)
                    .map_or(canvas, |parent| backdrop.get(parent).copied().flatten());
                let fill = |base| -> Result<Option<Color>> {
                    Ok(
                        rect_surface_paint(ctx, base, &terminal.rect_style)?.and_then(
                            |(paint, opacity)| {
                                let color = paint?;
                                if color.channels()[3] == u8::MAX && opacity == 1.0 {
                                    Some(color)
                                } else {
                                    outside.map(|under| color.composite(under, opacity))
                                }
                            },
                        ),
                    )
                };
                if let Some((direction, lane)) =
                    ctx.swimlane_direction.zip(ctx.swimlane_lanes_by_id.get(id))
                    && super::super::swimlane::is_lane_or_pool_cluster(ctx, id)
                {
                    use super::super::swimlane;
                    let base = literal_fill(&ctx.compatibility.cluster_bkg);
                    let palette = super::super::render::cluster::lane_color_slot(ctx, id)
                        .and_then(|slot| ctx.palette.background_color(slot))
                        .and_then(literal_fill);
                    let body = fill(
                        palette
                            .or_else(|| ctx.native.map(|native| native.container))
                            .or(base),
                    )?;
                    let header = fill(
                        palette
                            .or_else(|| ctx.native.map(|native| native.emphasis))
                            .or(base),
                    )?;
                    let frames = swimlane::swimlane_terminal_geometry(lane, direction, true)
                        .frame_parts((lane.x, lane.y), direction, true);
                    backdrop.insert(id, body);
                    for (frame, fill) in [(frames.0, body), (frames.1, header)] {
                        if frame.area() > 0.0 {
                            surfaces.push(Surface {
                                order: surfaces.len(),
                                frame: frame.to_rounded_rect(0.0),
                                fill,
                            });
                        }
                    }
                } else if ctx.subgraph_has_children(id) {
                    let bounds =
                        super::super::cluster_geometry::painted_cluster_frame(ctx, cluster)
                            .bounds();
                    let radius = super::super::cluster_geometry::cluster_container_radius(
                        ctx,
                        bounds.width(),
                        bounds.height(),
                    );
                    let fill = fill(literal_fill(super::super::render::cluster_base_fill(
                        ctx, terminal, id,
                    )))?;
                    backdrop.insert(id, fill);
                    surfaces.push(Surface {
                        order: surfaces.len(),
                        frame: bounds.to_rounded_rect(radius),
                        fill,
                    });
                }
            }
            let count = surfaces.len();
            let visits = count
                .checked_mul(count.checked_ilog2().unwrap_or(0) as usize + 1)
                .ok_or_else(|| ctx.work_meter.arithmetic_overflow())?;
            ctx.work_meter.charge(visits)?;
            let index = Bvh::build(surfaces, |surface| spatial_bounds(surface.frame.rect()))
                .map_err(|error| Error::InvalidModel {
                    message: format!("could not index Shoal surface paint: {error}"),
                })?;
            Ok(Self {
                canvas,
                count,
                index,
            })
        }

        fn touches(
            &self,
            ink: &EdgeInk<'_>,
            hits: &mut Vec<Surface>,
            ctx: &FlowchartRenderCtx<'_>,
        ) -> Result<()> {
            hits.clear();
            let _ = self.index.visit(|entry| -> Result<_> {
                ctx.work_meter.charge(1)?;
                Ok(match entry {
                    BvhEntry::Node(node) => Continue(ink.bounds.overlaps(bounds_rect(node))),
                    BvhEntry::Item(surface) => {
                        if ink.touches(surface.frame, ctx.work_meter)? {
                            hits.push(*surface);
                        }
                        Continue(true)
                    }
                })
            })?;
            ctx.work_meter.charge(
                hits.len()
                    .saturating_mul(hits.len().checked_ilog2().unwrap_or(0) as usize + 1),
            )?;
            hits.sort_unstable_by_key(|a| a.order);
            if let Some(first) = hits.iter().rposition(|surface| {
                surface.fill.is_some() && contains_rect(surface.frame, ink.bounds)
            }) {
                // An enclosing surface with resolved paint supplies a base for the whole edge.
                hits.drain(..first);
            }
            Ok(())
        }

        fn overlaps_recolored_ink(
            &self,
            ink: &EdgeInk<'_>,
            frame: RoundedRect,
            recolored: &FxHashSet<usize>,
            work: &merman_render_core::resources::OperationWorkMeter,
        ) -> Result<bool> {
            if recolored.is_empty() {
                return Ok(false);
            }
            let query = frame.rect().intersect(ink.bounds);
            let hit = self.index.visit(|entry| -> Result<_> {
                work.charge(1)?;
                Ok(match entry {
                    BvhEntry::Node(node) => Continue(query.overlaps(bounds_rect(node))),
                    BvhEntry::Item(surface) => {
                        if recolored.contains(&surface.order)
                            && ink.touches_overlap(frame, surface.frame, work)?
                        {
                            return Ok(Break(()));
                        }
                        Continue(true)
                    }
                })
            })?;
            Ok(hit.is_break())
        }
    }

    impl FlowchartEdgeFillPlan {
        pub(in crate::svg) fn applies(ctx: &FlowchartRenderCtx<'_>) -> bool {
            ctx.layout.route_contract.is_canonical_axis()
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
            let mut plan = Self::default();
            if !Self::applies(ctx) {
                return Ok(plan);
            }
            let Some(default_ink) = literal_fill(&ctx.compatibility.line_color) else {
                return Ok(plan);
            };
            let mut hits = Vec::new();
            let mut recolored = FxHashSet::default();
            let surfaces =
                OccludingSurfaceStack::prepare(ctx, hierarchy, solid_canvas(ctx, services))?;
            let default_checks_surface = (&surfaces.index)
                .into_iter()
                .filter_map(|surface| surface.fill)
                .chain(surfaces.canvas)
                .any(|fill| default_ink.contrast(fill) < EDGE_CONTRAST);
            let hand_drawn = ctx.compatibility.look.is_hand_drawn();
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
                let source = style.source_edge_stroke(hand_drawn);
                if source.is_none() && !default_checks_surface {
                    continue;
                }
                let Some(source_ink) = source
                    .map_or(Some(default_ink), literal_fill)
                    .filter(|color| color.channels()[3] == u8::MAX)
                else {
                    continue;
                };
                let Some(half_width) = ctx
                    .edge_style_plan
                    .stroke_outset_for(edge.key)?
                    .filter(|width| *width > 0.0)
                else {
                    continue;
                };
                let Some(cached) = cache.get(&edge.key) else {
                    continue;
                };
                let d = cached
                    .geom
                    .emitted_d_for_label
                    .as_deref()
                    .unwrap_or(&cached.geom.d);
                ctx.work_meter.charge(d.len().div_ceil(16))?;
                let shaft = BezPath::from_svg(d).map_err(|error| Error::InvalidModel {
                    message: format!("could not read painted Shoal edge path: {error}"),
                })?;
                let offset = Vec2::new(cached.origin_x - ctx.tx, cached.origin_y - ctx.ty);
                let ink = EdgeInk::new(&shaft, edge.edge, &cached.geom, offset, half_width);
                surfaces.touches(&ink, &mut hits, ctx)?;
                let background = hits
                    .first()
                    .filter(|surface| contains_rect(surface.frame, ink.bounds))
                    .and_then(|surface| surface.fill)
                    .or(surfaces.canvas);
                let base = background.map_or(source_ink, |fill| adapt(source_ink, fill));
                if base != source_ink {
                    plan.inks
                        .insert(edge.key, if base == WHITE { "#FFFFFF" } else { "#000000" });
                }
                let mut layers = Vec::new();
                recolored.clear();
                for surface in &hits {
                    ctx.work_meter.charge(1)?;
                    let Some(fill) = surface.fill else { continue };
                    let color = adapt(source_ink, fill);
                    if color != base {
                        recolored.insert(surface.order);
                    } else if !surfaces.overlaps_recolored_ink(
                        &ink,
                        surface.frame,
                        &recolored,
                        ctx.work_meter,
                    )? {
                        continue;
                    }
                    if plan.clips.is_empty() {
                        plan.clips.resize(surfaces.count, None);
                    }
                    // Canonical routes share the non-recursive root's coordinate system.
                    let frame = surface.frame.rect() - offset;
                    plan.clips[surface.order] = Some(frame.to_rounded_rect(surface.frame.radii()));
                    layers.push(SurfaceInk {
                        clip: surface.order,
                        color,
                    });
                }
                if !layers.is_empty() {
                    plan.layers.insert(edge.key, layers);
                }
            }
            Ok(plan)
        }
    }

    fn adapt(ink: Color, fill: Color) -> Color {
        if ink.contrast(fill) >= EDGE_CONTRAST {
            return ink;
        }
        if WHITE.contrast(fill) >= BLACK.contrast(fill) {
            WHITE
        } else {
            BLACK
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
        ctx.canvas.filter(|color| color.channels()[3] == u8::MAX)
    }

    fn rect_surface_paint(
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
        merman_render_core::mermaid_style::visit_parsed_style_declarations(rect_style, |decl| {
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

    pub(in crate::svg) fn contains_rect(frame: RoundedRect, rect: Rect) -> bool {
        [
            Point::new(rect.x0, rect.y0),
            Point::new(rect.x0, rect.y1),
            Point::new(rect.x1, rect.y0),
            Point::new(rect.x1, rect.y1),
        ]
        .into_iter()
        .all(|point| frame.contains(point))
    }

    impl<'a> EdgeInk<'a> {
        pub(in crate::svg) fn new(
            shaft: &'a BezPath,
            edge: &FlowEdge,
            geom: &FlowchartEdgePathGeom,
            offset: Vec2,
            half_width: f64,
        ) -> Self {
            let (start, end) = flowchart_marker_metrics(edge);
            let mut segments = shaft.segments();
            let first = segments.next();
            let last = segments.last().or(first);
            let markers = [
                (start, geom.data_points.first(), first, true),
                (end, geom.data_points.last(), last, false),
            ]
            .map(|(marker, tip, segment, from_start)| {
                if marker.depth == 0.0 || marker.width == 0.0 {
                    return None;
                }
                let segment = segment?;
                let tip = tip.map(|p| Point::new(p.x, p.y))?;
                let cubic = segment.to_cubic();
                let (terminal, controls) = if from_start {
                    (cubic.p0, [cubic.p1, cubic.p2, cubic.p3])
                } else {
                    (cubic.p3, [cubic.p2, cubic.p1, cubic.p0])
                };
                let tangent = controls
                    .into_iter()
                    .find(|&point| point != terminal)
                    .map_or(Vec2::ZERO, |point| point - terminal);
                let reach = marker.trim + marker.depth;
                let length = tangent.hypot();
                if length == 0.0 {
                    return Some(Rect::from_points(tip, tip).inflate(reach, reach));
                }
                let outward = tangent / length;
                let wing = Vec2::new(outward.y.abs(), outward.x.abs()) * (marker.width / 2.0);
                Some(Rect::from_points(tip, tip + outward * reach).inflate(wing.x, wing.y))
            });
            let mut bounds = shaft.bounding_box().inflate(half_width, half_width);
            for marker in markers.iter().flatten() {
                bounds = bounds.union(*marker);
            }
            Self {
                shaft,
                markers,
                offset,
                bounds: bounds + offset,
                half_width,
            }
        }

        pub(in crate::svg) fn marker_bounds(&self) -> impl Iterator<Item = Rect> {
            self.markers
                .into_iter()
                .flatten()
                .map(|rect| rect + self.offset)
        }

        fn touches(
            &self,
            frame: RoundedRect,
            work: &merman_render_core::resources::OperationWorkMeter,
        ) -> Result<bool> {
            if !self.bounds.overlaps(frame.rect()) {
                return Ok(false);
            }
            if self
                .markers
                .iter()
                .flatten()
                .any(|marker| rounded_rect_overlaps(frame, *marker + self.offset))
            {
                return Ok(true);
            }
            let rect = frame.rect().inflate(self.half_width, self.half_width) - self.offset;
            let radius = frame.radii().top_left;
            let frame = rect.to_rounded_rect(if radius > 0.0 {
                radius + self.half_width
            } else {
                0.0
            });
            for segment in self.shaft.segments() {
                if segment_enters_surface(segment, frame, 0, work)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }

        fn touches_overlap(
            &self,
            first: RoundedRect,
            second: RoundedRect,
            work: &merman_render_core::resources::OperationWorkMeter,
        ) -> Result<bool> {
            let overlap = first.rect().intersect(second.rect()).intersect(self.bounds);
            if overlap.area() <= 0.0 {
                return Ok(false);
            }
            self.touches_shared_rect(overlap, [first, second], 0, work)
        }

        fn touches_shared_rect(
            &self,
            rect: Rect,
            frames: [RoundedRect; 2],
            depth: u8,
            work: &merman_render_core::resources::OperationWorkMeter,
        ) -> Result<bool> {
            work.charge(1)?;
            if frames.iter().all(|frame| contains_rect(*frame, rect)) {
                return self.touches(rect.to_rounded_rect(0.0), work);
            }
            if !frames
                .iter()
                .all(|frame| rounded_rect_overlaps(*frame, rect))
                || !self.touches(rect.to_rounded_rect(0.0), work)?
            {
                return Ok(false);
            }
            // Only the partially covered corner boxes need finer inspection.
            if depth == 32 || rect.width().max(rect.height()) <= 1e-3 {
                return Ok(false);
            }
            let center = rect.center();
            let (left, right) = if rect.width() >= rect.height() {
                (
                    Rect::new(rect.x0, rect.y0, center.x, rect.y1),
                    Rect::new(center.x, rect.y0, rect.x1, rect.y1),
                )
            } else {
                (
                    Rect::new(rect.x0, rect.y0, rect.x1, center.y),
                    Rect::new(rect.x0, center.y, rect.x1, rect.y1),
                )
            };
            Ok(self.touches_shared_rect(left, frames, depth + 1, work)?
                || self.touches_shared_rect(right, frames, depth + 1, work)?)
        }
    }

    fn rounded_rect_overlaps(frame: RoundedRect, rect: Rect) -> bool {
        let center = frame.rect().center();
        frame.contains(Point::new(
            center.x.clamp(rect.x0, rect.x1),
            center.y.clamp(rect.y0, rect.y1),
        ))
    }

    fn bounds_rect(bounds: SpatialBounds) -> Rect {
        Rect::new(bounds.x[0], bounds.y[0], bounds.x[1], bounds.y[1])
    }

    pub(in crate::svg) fn spatial_bounds(rect: Rect) -> SpatialBounds {
        SpatialBounds {
            x: [rect.x0, rect.x1],
            y: [rect.y0, rect.y1],
        }
    }

    pub(in crate::svg) fn segment_enters_surface(
        segment: PathSeg,
        frame: RoundedRect,
        depth: u8,
        work: &merman_render_core::resources::OperationWorkMeter,
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

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn surface_test_follows_the_curve_instead_of_its_chord_or_control_polygon() {
            let work = merman_render_core::resources::OperationWorkMeter::new(
                merman_render_core::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
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
        fn restoration_requires_common_shaft_or_marker_ink() {
            let work = merman_render_core::resources::OperationWorkMeter::new(
                merman_render_core::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            );
            let shaft = BezPath::from_svg("M0,0H20V20").unwrap();
            let mut ink = EdgeInk {
                shaft: &shaft,
                markers: [None, None],
                offset: Vec2::ZERO,
                bounds: Rect::new(-0.5, -0.5, 20.5, 20.5),
                half_width: 0.5,
            };
            let first = Rect::new(0.0, -1.0, 15.0, 15.0).to_rounded_rect(0.0);
            let second = Rect::new(10.0, 5.0, 22.0, 18.0).to_rounded_rect(0.0);
            assert!(ink.touches(first, &work).unwrap());
            assert!(ink.touches(second, &work).unwrap());
            assert!(!ink.touches_overlap(first, second, &work).unwrap());
            let crosses_shaft = Rect::new(10.0, -1.0, 22.0, 18.0).to_rounded_rect(0.0);
            assert!(ink.touches_overlap(first, crosses_shaft, &work).unwrap());
            ink.markers[1] = Some(Rect::new(11.0, 6.0, 12.0, 7.0));
            assert!(ink.touches_overlap(first, second, &work).unwrap());
        }

        #[test]
        fn fully_enclosed_ink_can_use_the_topmost_surface() {
            let bounds = Rect::new(2.0, 2.0, 18.0, 18.0);
            let frame = Rect::new(0.0, 0.0, 20.0, 20.0);
            assert!(contains_rect(frame.to_rounded_rect(0.0), bounds));
            assert!(!contains_rect(frame.to_rounded_rect(10.0), bounds));
        }
    }
}
