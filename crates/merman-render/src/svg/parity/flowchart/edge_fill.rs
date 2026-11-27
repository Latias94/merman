//! Contrast for Shoal edges on the surfaces painted by Flowchart and Swimlane.

use super::*;
use crate::flowchart::FlowchartEdgeKey;
use crate::theme::Color;
use kurbo::RoundedRect;

#[derive(Default)]
pub(super) struct FlowchartEdgeFillPlan {
    inks: FxHashMap<FlowchartEdgeKey, &'static str>,
    layers: FxHashMap<FlowchartEdgeKey, Vec<SurfaceInk>>,
    clips: Vec<Option<RoundedRect>>,
}

#[derive(Clone, Copy)]
pub(super) struct SurfaceInk {
    pub(super) clip: usize,
    pub(super) color: Color,
}

impl FlowchartEdgeFillPlan {
    pub(super) fn ink(&self, key: FlowchartEdgeKey) -> Option<&str> {
        self.inks.get(&key).copied()
    }

    pub(super) fn layers(&self, key: FlowchartEdgeKey) -> &[SurfaceInk] {
        self.layers.get(&key).map_or(&[], Vec::as_slice)
    }

    pub(super) fn write_clip_id(
        out: &mut impl std::fmt::Write,
        ctx: &FlowchartRenderCtx<'_>,
        clip: usize,
    ) -> std::fmt::Result {
        write!(
            out,
            "{}-paint-surface-{clip}",
            escape_attr_display(ctx.document_ids().marker_scope())
        )
    }

    pub(super) fn write_clips(
        &self,
        out: &mut impl SvgOutput,
        ctx: &FlowchartRenderCtx<'_>,
    ) -> Result<()> {
        if self.clips.is_empty() {
            return Ok(());
        }
        out.push_str("<defs>");
        for (clip, frame) in self.clips.iter().enumerate() {
            ctx.work_meter.charge_emit_work(1)?;
            let Some(frame) = frame else { continue };
            let rect = frame.rect();
            out.push_str(r#"<clipPath id=""#);
            let _ = Self::write_clip_id(out, ctx, clip);
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
        _options: &SvgExecution<'_>,
    ) -> Result<Self> {
        Ok(Self::default())
    }
}

#[cfg(feature = "layout-shoal")]
mod shoal {
    use super::*;
    use crate::theme::edge_label_bg::{literal_fill, literal_opacity};
    use kurbo::{BezPath, Line, ParamCurve, ParamCurveExtrema, PathSeg, Point, Rect, Shape, Vec2};
    use merman_layout_shoal::geom::{Bounds as SpatialBounds, Bvh, BvhEntry};
    use merman_model::families::flowchart::FlowEdgeVisibility;
    use merman_theme_contract::v2::{Paint, Texture};
    use std::ops::ControlFlow::Continue;

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

    struct EdgeInk<'a> {
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
                    let palette = swimlane::lane_color_slot(ctx, id)
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
                    BvhEntry::Node(node) => Continue(
                        ink.bounds
                            .overlaps(Rect::new(node.x[0], node.y[0], node.x[1], node.y[1])),
                    ),
                    BvhEntry::Item(surface) => {
                        if ink.bounds.overlaps(surface.frame.rect()) {
                            if ink.markers.iter().flatten().any(|marker| {
                                rounded_rect_overlaps(surface.frame, *marker + ink.offset)
                            }) {
                                hits.push(*surface);
                                return Ok(Continue(true));
                            }
                            let rect = surface.frame.rect().inflate(ink.half_width, ink.half_width)
                                - ink.offset;
                            let radius = surface.frame.radii().top_left;
                            let frame = rect.to_rounded_rect(if radius > 0.0 {
                                radius + ink.half_width
                            } else {
                                0.0
                            });
                            for segment in ink.shaft.segments() {
                                if segment_enters_surface(segment, frame, 0, ctx.work_meter)? {
                                    hits.push(*surface);
                                    break;
                                }
                            }
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
            if let Some(first) = hits
                .iter()
                .rposition(|surface| contains_rect(surface.frame, ink.bounds))
            {
                // A later painted surface fully enclosing the ink occludes every earlier one.
                hits.drain(..first);
            }
            Ok(())
        }
    }

    impl FlowchartEdgeFillPlan {
        pub(in crate::svg::parity::flowchart) fn applies(ctx: &FlowchartRenderCtx<'_>) -> bool {
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

        pub(in crate::svg::parity::flowchart) fn prepare(
            ctx: &FlowchartRenderCtx<'_>,
            hierarchy: &FlowchartHierarchyPlan<'_>,
            cache: &FxHashMap<FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
            options: &SvgExecution<'_>,
        ) -> Result<Self> {
            let mut plan = Self::default();
            if !Self::applies(ctx) {
                return Ok(plan);
            }
            let Some(default_ink) = literal_fill(&ctx.compatibility.line_color) else {
                return Ok(plan);
            };
            let mut hits = Vec::new();
            let surfaces =
                OccludingSurfaceStack::prepare(ctx, hierarchy, solid_canvas(ctx, options))?;
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
                let covers = hits
                    .first()
                    .is_some_and(|surface| contains_rect(surface.frame, ink.bounds));
                let background = if covers {
                    hits[0].fill
                } else {
                    surfaces.canvas
                };
                let base = adapt(source_ink, background);
                if base != source_ink {
                    plan.inks
                        .insert(edge.key, if base == WHITE { "#FFFFFF" } else { "#000000" });
                }
                let mut layers: Vec<SurfaceInk> = Vec::new();
                for surface in &hits {
                    ctx.work_meter.charge(1)?;
                    let ink = adapt(base, surface.fill);
                    // Replay a later surface at the base color when its frame overlaps an
                    // earlier recolored layer. Clipping applies the same paint order as the shells.
                    if ink == base && layers.is_empty() {
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
                        color: ink,
                    });
                }
                if !layers.is_empty() {
                    plan.layers.insert(edge.key, layers);
                }
            }
            Ok(plan)
        }
    }

    fn adapt(ink: Color, fill: Option<Color>) -> Color {
        let Some(fill) = fill.filter(|fill| ink.contrast(*fill) < EDGE_CONTRAST) else {
            return ink;
        };
        if WHITE.contrast(fill) >= BLACK.contrast(fill) {
            WHITE
        } else {
            BLACK
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

    impl<'a> EdgeInk<'a> {
        fn new(
            shaft: &'a BezPath,
            edge: &crate::flowchart::FlowEdge,
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
    }

    fn rounded_rect_overlaps(frame: RoundedRect, rect: Rect) -> bool {
        let center = frame.rect().center();
        frame.contains(Point::new(
            center.x.clamp(rect.x0, rect.x1),
            center.y.clamp(rect.y0, rect.y1),
        ))
    }

    pub(in crate::svg::parity::flowchart) fn spatial_bounds(rect: Rect) -> SpatialBounds {
        SpatialBounds {
            x: [rect.x0, rect.x1],
            y: [rect.y0, rect.y1],
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
        fn fully_enclosed_ink_can_use_the_topmost_surface() {
            let bounds = Rect::new(2.0, 2.0, 18.0, 18.0);
            let frame = Rect::new(0.0, 0.0, 20.0, 20.0);
            assert!(contains_rect(frame.to_rounded_rect(0.0), bounds));
            assert!(!contains_rect(frame.to_rounded_rect(10.0), bounds));
        }
    }
}
