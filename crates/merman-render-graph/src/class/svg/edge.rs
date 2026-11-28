#[cfg(feature = "layout-shoal")]
use super::ClassCanonicalRoutes;
use super::ClassSvgRelation;
use super::bounds::{
    include_class_marker_paint_bounds, include_path_bounds_with_outset, include_xywh,
};
use super::context::{ClassEmitCheckpoint, ClassRenderDetails};
use super::defs::{class_canonical_marker, class_marker_name, class_marker_paint_spec};
use super::label::{
    ClassHtmlLabelSpec, class_html_div_style, class_math_html_label, render_class_html_label,
    write_class_svg_edge_text,
};
use super::rough::class_rough_hand_drawn_stroke_path_for_svg_path;
use base64::Engine as _;
use merman_render_core::entities::decode_entities_minimal_cow;
use merman_render_core::html::html_label_overflow_attr;
#[cfg(feature = "layout-shoal")]
use merman_render_core::route_paint::{CanonicalPaintScratch, RouteStyle};
use merman_render_core::svg_bounds::{SvgPathBounds, svg_path_bounds_from_d};
use merman_render_core::svg_css::SvgId as SvgDiagramId;
use merman_render_core::svg_edge_parity::edge_label_geometry::position_edge_label;
use merman_render_core::svg_output::{cutout, timing::RenderTiming};
use merman_render_core::text::{MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX, TextMeasurer, TextStyle};
use merman_render_core::{Bounds, LayoutEdge, LayoutLabel, LayoutPoint};
use merman_theme_contract::v2::{Color, EdgeLabelBackground};
use std::fmt::Write as _;

use super::{SvgOutput, escape_attr_display, escape_xml_into, fmt};
use merman_render_core::svg_edge_parity::path_numbers::json_stringify_points_into;
use rustc_hash::FxHashMap;

const CLASS_HAND_DRAWN_EDGE_STROKE: &str = "#000";
const CLASS_HAND_DRAWN_EDGE_STROKE_WIDTH: f64 = 1.0;

pub(super) struct ClassEdgeGroupsRenderContext<'a> {
    pub edges: &'a [LayoutEdge],
    pub clusters: &'a [merman_render_core::LayoutCluster],
    pub native_style: Option<&'a crate::class::ClassNativeStyle>,
    pub missing_section_points: &'a FxHashMap<&'a str, Vec<LayoutPoint>>,
    pub work_meter: &'a merman_render_core::resources::OperationWorkMeter,
    pub line_hop_paths: &'a std::collections::HashMap<&'a str, String>,
    pub relations_by_id: &'a FxHashMap<&'a str, ClassSvgRelation<'a>>,
    pub relation_index_by_id: &'a FxHashMap<&'a str, usize>,
    pub prepared_labels: &'a crate::class::ClassPreparedLabels,
    #[cfg(feature = "layout-grid")]
    pub grid: Option<merman_render_core::graph_layout::mermaid_grid::GridPaint>,
    #[cfg(feature = "layout-layered")]
    pub layered: Option<super::layered::ClassLayeredSvg<'a>>,
    #[cfg(feature = "layout-shoal")]
    pub canonical_routes: Option<&'a ClassCanonicalRoutes<'a>>,
    pub diagram_marker_class: &'a str,
    pub diagram_id: SvgDiagramId<'a>,
    pub content_tx: f64,
    pub content_ty: f64,
    pub bounds_dx: f64,
    pub bounds_dy: f64,
    pub edge_use_html_labels: bool,
    pub text_measurer: &'a dyn TextMeasurer,
    pub terminal_text_style: &'a TextStyle,
    pub mermaid_config: Option<&'a merman_model::presentation::PresentationConfig>,
    pub math_renderer: Option<&'a (dyn merman_render_core::math::MathRenderer + Send + Sync)>,
    pub look: &'a str,
    pub hand_drawn_seed: roughr::core::RoughRandomness,
    pub timing: RenderTiming,
    pub uses_elk_adapter_dom: bool,
    pub edge_paths_class: &'static str,
    pub relation_theme: &'a crate::class::ClassRelationThemePlan,
    pub emit: ClassEmitCheckpoint<'a>,
}

impl ClassEdgeGroupsRenderContext<'_> {
    fn uses_canonical_routes(&self) -> bool {
        #[cfg(feature = "layout-shoal")]
        {
            self.canonical_routes.is_some()
        }
        #[cfg(not(feature = "layout-shoal"))]
        {
            false
        }
    }

    fn uses_grid(&self) -> bool {
        #[cfg(feature = "layout-grid")]
        return self.grid.is_some();
        #[cfg(not(feature = "layout-grid"))]
        false
    }

    fn uses_flat_edge_labels(&self) -> bool {
        self.uses_elk_adapter_dom || self.uses_grid() || self.uses_layered()
    }

    fn uses_layered(&self) -> bool {
        #[cfg(feature = "layout-layered")]
        return self.layered.is_some();
        #[cfg(not(feature = "layout-layered"))]
        false
    }

    fn translated_label_center(&self, label: &LayoutLabel) -> LayoutPoint {
        let y = label.y + self.content_ty;
        #[cfg(feature = "layout-layered")]
        let y = self.layered.map_or(y, |layered| y + layered.label_y_offset);
        LayoutPoint {
            x: label.x + self.content_tx,
            y,
        }
    }

    fn auto_namespace_fill(&self) -> Option<Color> {
        self.native_style
            .filter(|native| native.edge_label_bg == EdgeLabelBackground::Auto)
            .map(|native| native.container)
    }

    pub(super) fn edge_label_fill(&self, (center_x, center_y): (f64, f64)) -> Option<Color> {
        let default = self.relation_theme.native_edge_label_fill;
        let x = center_x - self.content_tx;
        let y = center_y - self.content_ty;
        self.auto_namespace_fill()
            .filter(|_| {
                self.clusters.iter().any(|cluster| {
                    let hw = cluster.width / 2.0;
                    let hh = cluster.height / 2.0;
                    x >= cluster.x - hw
                        && x <= cluster.x + hw
                        && y >= cluster.y - hh
                        && y <= cluster.y + hh
                })
            })
            .or(default)
    }
}

pub(super) use crate::class::class_arrow_type_for_relation_end;

/// Render order and label centers computed while painting the paths.
pub(super) struct ClassEdgePathEmission<'edges> {
    ordered_edges: Vec<&'edges LayoutEdge>,
    label_centers: FxHashMap<&'edges str, LayoutPoint>,
}

pub(super) fn render_class_edge_paths<'edges, O: SvgOutput>(
    out: &mut O,
    content_bounds: &mut Option<Bounds>,
    detail: &mut ClassRenderDetails,
    ctx: &ClassEdgeGroupsRenderContext<'edges>,
) -> merman_render_core::Result<ClassEdgePathEmission<'edges>> {
    let mut edge_points_json_buf = String::new();
    let mut edge_points_json_ryu = ryu_js::Buffer::new();
    let mut edge_points_b64_buf = String::new();
    let mut edge_raw_points: Vec<LayoutPoint> = Vec::new();
    let mut edge_marker_points: Vec<LayoutPoint> = Vec::new();
    let mut edge_curve_points: Vec<LayoutPoint> = Vec::new();
    #[cfg(feature = "layout-layered")]
    let mut layered_points = Vec::new();
    let mut edge_class_buf = String::with_capacity(64);
    let mut edge_dom_id_buf = String::with_capacity(64);
    let mut edge_style_buf = String::new();
    #[cfg(feature = "layout-shoal")]
    let mut paint_scratch = CanonicalPaintScratch::default();
    let uses_canonical_routes = ctx.uses_canonical_routes();
    let sketch = ctx.relation_theme.sketch_edges;
    let default_label_fill = ctx.relation_theme.native_edge_label_fill;
    let is_transparent = |fill: Color| fill.channels()[3] == 0;
    let has_transparent_label_fill = default_label_fill.is_some_and(is_transparent)
        || ctx.auto_namespace_fill().is_some_and(is_transparent);

    let edge_paths_start = ctx.timing.start();
    let ordered_edges = if ctx.uses_grid() || ctx.uses_layered() {
        if ctx.uses_layered() {
            ctx.work_meter.charge_emit_work(ctx.edges.len())?;
        }
        ctx.edges.iter().collect()
    } else {
        class_edge_render_order(ctx.edges, ctx.relation_index_by_id)
    };
    let mut edge_label_centers =
        FxHashMap::with_capacity_and_hasher(ordered_edges.len(), Default::default());
    let _ = write!(out, r#"<g class="{}">"#, ctx.edge_paths_class);
    out.checkpoint()?;
    #[cfg(feature = "layout-layered")]
    let mut layered_axes = ctx.layered.map(|layered| layered.axes.iter());
    for e in ordered_edges.iter().copied() {
        ctx.emit.checkpoint()?;
        #[cfg(feature = "layout-layered")]
        let axis_tags = layered_axes.as_mut().and_then(Iterator::next);
        let missing_section = ctx.missing_section_points.get(e.id.as_str());
        let source_points = missing_section.map(Vec::as_slice).unwrap_or(&e.points);
        if source_points.is_empty() || (missing_section.is_none() && source_points.len() < 2) {
            continue;
        }

        class_edge_dom_id_into(&mut edge_dom_id_buf, e, ctx.relation_index_by_id);

        if !uses_canonical_routes {
            edge_raw_points.clear();
            if ctx.uses_layered() {
                ctx.work_meter.charge_emit_work(source_points.len())?;
                edge_raw_points
                    .try_reserve(source_points.len())
                    .map_err(|_| merman_render_core::Error::LayoutAllocationFailed {
                        backend: "layered",
                    })?;
            }
            edge_raw_points.reserve(source_points.len());
            for p in source_points {
                edge_raw_points.push(LayoutPoint {
                    x: p.x + ctx.content_tx,
                    y: p.y + ctx.content_ty,
                });
            }
        }

        let curve_start = ctx.timing.start();
        let is_note_edge = e.id.starts_with("edgeNote");
        let use_neo_mask = ctx.look == "neo" && !sketch && !(uses_canonical_routes && is_note_edge);
        let relation = if is_note_edge {
            None
        } else {
            ctx.relations_by_id.get(e.id.as_str()).copied()
        };
        let width_binding = ctx.relation_theme.relation_width();
        let relation_stroke_width = relation
            .and(width_binding.paint_width())
            .or(uses_canonical_routes.then_some(1.0));
        let stroke_outset = relation_stroke_width.map_or(0.0, |width| f64::from(width) / 2.0);
        let mut shaft_markers = [
            relation.and_then(|relation| class_marker_name(relation.relation.type1, true)),
            relation.and_then(|relation| class_marker_name(relation.relation.type2, false)),
        ];
        let start_marker_paint = relation
            .and_then(|rel| class_marker_paint_spec(rel.relation.type1, true, ctx.look == "neo"));
        let end_marker_paint = relation
            .and_then(|rel| class_marker_paint_spec(rel.relation.type2, false, ctx.look == "neo"));
        #[cfg(feature = "layout-shoal")]
        let canonical = ctx
            .canonical_routes
            .map(|routes| {
                if use_neo_mask {
                    paint_scratch.route.measure_length();
                }
                (routes.paint)(
                    e,
                    kurbo::Vec2::new(ctx.content_tx, ctx.content_ty),
                    stroke_outset,
                    RouteStyle::from_interpolate(
                        ctx.relation_theme.native_edge_curve().unwrap_or("basis"),
                        0.0,
                    ),
                    &mut paint_scratch,
                )
            })
            .transpose()?;

        let parity_d;
        let (d, d_pb, marker_points, data_points) = 'geometry: {
            #[cfg(feature = "layout-shoal")]
            if let Some(geometry) = &canonical {
                let bounds = &geometry.path.bounds;
                break 'geometry (
                    geometry.path.d.as_str(),
                    Some(SvgPathBounds {
                        min_x: bounds.min_x,
                        min_y: bounds.min_y,
                        max_x: bounds.max_x,
                        max_y: bounds.max_y,
                    }),
                    geometry.marker_points.as_slice(),
                    geometry.data_points.as_slice(),
                );
            }
            #[cfg(feature = "layout-layered")]
            if let Some(layered) = ctx.layered {
                let axes = axis_tags.ok_or_else(|| {
                    crate::class::layered::invalid_projection("missing Class relation axis tags")
                })?;
                parity_d = super::layered::shaft(
                    &edge_raw_points,
                    axes,
                    layered.curve,
                    super::layered::arrows(relation),
                    &mut layered_points,
                    &mut edge_marker_points,
                    ctx.work_meter,
                )?;
                break 'geometry (
                    parity_d.as_str(),
                    None,
                    edge_marker_points.as_slice(),
                    edge_raw_points.as_slice(),
                );
            }
            #[cfg(feature = "layout-grid")]
            if let Some(settings) = ctx.grid {
                let geom = super::grid::shaft(
                    &edge_raw_points,
                    (
                        relation
                            .and_then(|rel| class_arrow_type_for_relation_end(rel.relation.type1)),
                        relation
                            .and_then(|rel| class_arrow_type_for_relation_end(rel.relation.type2)),
                    ),
                    settings,
                    &mut edge_curve_points,
                    &mut edge_marker_points,
                    ctx.work_meter,
                )?;
                parity_d = geom.d;
                break 'geometry (
                    parity_d.as_str(),
                    geom.bounds,
                    edge_marker_points.as_slice(),
                    edge_raw_points.as_slice(),
                );
            }
            if ctx.uses_elk_adapter_dom && missing_section.is_none() {
                // ELK selects rounded routing after clipping and offsets only the endpoints.
                edge_marker_points =
                    merman_render_core::svg_edge_parity::edge_path::rounded_line_with_marker_offsets_points(
                        &edge_raw_points,
                        relation
                            .and_then(|rel| class_arrow_type_for_relation_end(rel.relation.type1)),
                        relation
                            .and_then(|rel| class_arrow_type_for_relation_end(rel.relation.type2)),
                    );
            } else {
                merman_render_core::svg_edge_parity::edge_path::line_with_offset_points_into(
                    &edge_raw_points,
                    relation.and_then(|rel| class_arrow_type_for_relation_end(rel.relation.type1)),
                    relation.and_then(|rel| class_arrow_type_for_relation_end(rel.relation.type2)),
                    &mut edge_marker_points,
                );
            }
            let edge_curve_source = edge_marker_points.as_slice();
            let native_curve = ctx.relation_theme.native_edge_curve();
            let (path_d, bounds) = if missing_section.is_some() || native_curve == Some("linear") {
                merman_render_core::svg_edge_parity::curve::curve_linear_path_d_and_bounds(
                    edge_curve_source,
                )
            } else if native_curve == Some("step") {
                merman_render_core::svg_edge_parity::curve::curve_step_path_d_and_bounds(
                    edge_curve_source,
                )
            } else if ctx.uses_elk_adapter_dom && native_curve.is_none() {
                (
                    merman_render_core::svg_edge_parity::curve::curve_rounded_path_d(
                        edge_curve_source,
                        5.0,
                    ),
                    None,
                )
            } else if edge_curve_source.len() == 2 {
                edge_curve_points.clear();
                let a = &edge_curve_source[0];
                let b = &edge_curve_source[1];
                edge_curve_points.push(a.clone());
                edge_curve_points.push(LayoutPoint {
                    x: (a.x + b.x) / 2.0,
                    y: (a.y + b.y) / 2.0,
                });
                edge_curve_points.push(b.clone());
                merman_render_core::svg_edge_parity::curve::curve_basis_path_d_and_bounds(
                    &edge_curve_points,
                )
            } else {
                merman_render_core::svg_edge_parity::curve::curve_basis_path_d_and_bounds(
                    edge_curve_source,
                )
            };
            parity_d = path_d;
            break 'geometry (
                parity_d.as_str(),
                bounds,
                edge_curve_source,
                edge_raw_points.as_slice(),
            );
        };
        if let Some(s) = curve_start {
            detail.edge_curve += s.elapsed();
        }
        let rough_d = 'rough: {
            if ctx.look != "handDrawn" && !sketch {
                break 'rough None;
            }
            #[cfg(feature = "layout-shoal")]
            if let (Some(routes), Some(geometry)) = (ctx.canonical_routes, &canonical) {
                ctx.work_meter.charge(d.len().max(1))?;
                break 'rough Some((routes.rough_stroke)(
                    d,
                    geometry.markers,
                    ctx.work_meter,
                    &ctx.hand_drawn_seed,
                )?);
            }
            if sketch {
                let markers = relation.map_or_else(Default::default, |relation| {
                    let neo = ctx.look == "neo";
                    (
                        class_canonical_marker(relation.relation.type1, true, neo),
                        class_canonical_marker(relation.relation.type2, false, neo),
                    )
                });
                break 'rough Some(merman_render_core::rough_svg::constrained_rough_edge_path(
                    d,
                    markers,
                    ctx.work_meter,
                    &ctx.hand_drawn_seed,
                )?);
            }
            class_rough_hand_drawn_stroke_path_for_svg_path(
                d,
                0.3,
                ctx.work_meter,
                &ctx.hand_drawn_seed,
            )
        };
        let paint_d = rough_d.as_deref().unwrap_or(d);
        let hopped_d = ctx.line_hop_paths.get(e.id.as_str());
        let render_d = hopped_d.map(String::as_str).unwrap_or(paint_d);
        let label_center = e.label.as_ref().map(|lbl| {
            if uses_canonical_routes || ctx.uses_grid() || ctx.uses_layered() {
                ctx.translated_label_center(lbl)
            } else {
                let center = if missing_section.is_some() {
                    let first = &edge_raw_points[0];
                    let last = &edge_raw_points[edge_raw_points.len() - 1];
                    LayoutPoint {
                        x: (first.x + last.x) / 2.0,
                        y: (first.y + last.y) / 2.0,
                    }
                } else {
                    class_edge_label_center(
                        Some(&edge_raw_points),
                        &edge_raw_points,
                        paint_d,
                        e.from_cluster.is_some() || e.to_cluster.is_some(),
                        lbl,
                        ctx.content_tx,
                        ctx.content_ty,
                    )
                };
                edge_label_centers.insert(e.id.as_str(), center.clone());
                center
            }
        });
        let path_bounds_start = ctx.timing.start();
        // RoughJS replaces the stable curve endpoints with jittered path data. Use the rendered
        // path bbox plus a rotation-independent marker radius to cover its actual first/final
        // marker anchors without maintaining a second SVG path interpreter. Classic and ELK paths
        // retain one stable endpoint pair and can use the tighter rotated marker boxes below.
        let stable_marker_points = rough_d.is_none() || uses_canonical_routes || sketch;
        let rough_marker_outset = if stable_marker_points {
            0.0
        } else {
            relation_stroke_width.map_or(0.0, |width| {
                [start_marker_paint, end_marker_paint]
                    .into_iter()
                    .flatten()
                    .map(|marker| marker.conservative_radius(f64::from(width)))
                    .fold(0.0, f64::max)
            })
        };
        let path_paint_outset = stroke_outset.max(rough_marker_outset);
        if ctx.uses_layered() {
            ctx.work_meter.charge_emit_work(render_d.len())?;
        }
        let path_bounds = if rough_d.is_none() && hopped_d.is_none() {
            d_pb
        } else {
            None
        }
        .or_else(|| svg_path_bounds_from_d(render_d));
        if let Some(pb) = &path_bounds {
            include_path_bounds_with_outset(
                content_bounds,
                pb,
                ctx.bounds_dx,
                ctx.bounds_dy,
                path_paint_outset,
            );
        }
        if stable_marker_points && let Some(width) = relation_stroke_width.map(f64::from) {
            if let Some(marker) = start_marker_paint {
                include_class_marker_paint_bounds(
                    content_bounds,
                    marker_points,
                    true,
                    marker,
                    width,
                    ctx.bounds_dx,
                    ctx.bounds_dy,
                );
            }
            if let Some(marker) = end_marker_paint {
                include_class_marker_paint_bounds(
                    content_bounds,
                    marker_points,
                    false,
                    marker,
                    width,
                    ctx.bounds_dx,
                    ctx.bounds_dy,
                );
            }
        }
        if let Some(s) = path_bounds_start {
            detail.path_bounds += s.elapsed();
            detail.path_bounds_calls += 1;
        }

        let json_start = ctx.timing.start();
        edge_points_json_buf.clear();
        let axis_json = {
            #[cfg(feature = "layout-layered")]
            if let Some(axes) = axis_tags {
                ctx.work_meter.charge_emit_work(data_points.len())?;
                merman_render_core::svg_edge_parity::path_numbers::json_stringify_axis_points_into(
                    &mut edge_points_json_buf,
                    data_points,
                    axes,
                    &mut edge_points_json_ryu,
                );
                ctx.work_meter
                    .charge_emit_work(edge_points_json_buf.len())?;
                true
            } else {
                false
            }
            #[cfg(not(feature = "layout-layered"))]
            false
        };
        if !axis_json {
            json_stringify_points_into(
                &mut edge_points_json_buf,
                data_points,
                &mut edge_points_json_ryu,
            );
        }
        if let Some(s) = json_start {
            detail.edge_points_json += s.elapsed();
        }

        let b64_start = ctx.timing.start();
        edge_points_b64_buf.clear();
        base64::engine::general_purpose::STANDARD
            .encode_string(edge_points_json_buf.as_bytes(), &mut edge_points_b64_buf);
        if let Some(s) = b64_start {
            detail.edge_points_b64 += s.elapsed();
        }

        edge_class_buf.clear();
        edge_class_buf.push_str("edge-thickness-normal ");
        if is_note_edge {
            edge_class_buf.push_str(class_note_edge_pattern());
        } else if let Some(rel) = relation {
            edge_class_buf.push_str(class_edge_pattern(rel.relation.line_type));
        } else {
            edge_class_buf.push_str("edge-pattern-solid");
        }
        if ctx.look == "handDrawn" {
            edge_class_buf.push_str(" transition");
        }
        edge_class_buf.push_str(" relation");

        let mut separate_markers = [None; 2];
        let label_cutout = if let (Some(bounds), Some(&ordinal)) =
            (&path_bounds, ctx.relation_index_by_id.get(e.id.as_str()))
        {
            let mut gaps = [None; 3];
            if has_transparent_label_fill
                && let (Some(center), Some(label)) = (&label_center, &e.label)
                && label.width > 0.0
                && label.height > 0.0
                && relation.is_some_and(|relation| {
                    !decode_entities_minimal_cow(&relation.source.title)
                        .trim()
                        .is_empty()
                })
                && ctx
                    .edge_label_fill((center.x, center.y))
                    .is_some_and(is_transparent)
            {
                gaps[0] = Some(kurbo::Rect::from_center_size(
                    (center.x, center.y),
                    (label.width, label.height),
                ));
            }
            if uses_canonical_routes {
                let stroke_outset = if ctx.look == "handDrawn" {
                    stroke_outset.max(CLASS_HAND_DRAWN_EDGE_STROKE_WIDTH / 2.0)
                } else {
                    stroke_outset
                };
                let shaft_bounds =
                    kurbo::Rect::new(bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y)
                        .inflate(stroke_outset, stroke_outset);
                let terminals = [&e.start_label_right, &e.end_label_left].map(|label| {
                    label.as_ref().and_then(|label| {
                        if label.width <= 0.0 || label.height <= 0.0 {
                            return None;
                        }
                        let center = ctx.translated_label_center(label);
                        let rect = kurbo::Rect::from_center_size(
                            (center.x, center.y),
                            (label.width, label.height),
                        );
                        (rect.intersect(shaft_bounds).area() > 0.0).then_some(rect)
                    })
                });
                if terminals.iter().any(Option::is_some) {
                    ctx.work_meter.charge_emit_work(render_d.len())?;
                    if let Ok(shaft) = kurbo::BezPath::from_svg(render_d) {
                        for (gap, terminal) in gaps[1..].iter_mut().zip(terminals) {
                            *gap = terminal
                                .and_then(|rect| terminal_label_gap(&shaft, rect, stroke_outset));
                        }
                    }
                }
            }
            if gaps.iter().any(Option::is_some) {
                let width = relation_stroke_width.map_or(1.0, f64::from);
                let radii = [start_marker_paint, end_marker_paint]
                    .map(|marker| marker.map_or(0.0, |marker| marker.conservative_radius(width)));
                let outset = radii.into_iter().fold(width, f64::max);
                let mut holes = [kurbo::Rect::ZERO; 3];
                let mut count = 0;
                for gap in gaps.into_iter().flatten() {
                    holes[count] = gap;
                    count += 1;
                }
                let holes = &holes[..count];
                if uses_canonical_routes {
                    separate_markers = separate_class_edge_markers(
                        holes,
                        marker_points,
                        radii,
                        &mut shaft_markers,
                    );
                }
                let paint =
                    kurbo::Rect::new(bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y)
                        .inflate(outset, outset);
                let id = ctx.diagram_id;
                cutout::write_defs(
                    out,
                    format_args!("{id}_class-edge-label-{ordinal}"),
                    paint,
                    1.0,
                    holes,
                    ctx.work_meter,
                )?;
                Some(ordinal)
            } else {
                None
            }
        } else {
            None
        };

        let _ = write!(out, r#"<path d="{}""#, escape_attr_display(render_d));
        if let Some(ordinal) = label_cutout {
            let id = ctx.diagram_id;
            cutout::write_reference(out, format_args!("{id}_class-edge-label-{ordinal}"));
        }
        if ctx.look == "handDrawn" {
            let _ = write!(
                out,
                r#" stroke="{}" stroke-width="{}" fill="none""#,
                CLASS_HAND_DRAWN_EDGE_STROKE, CLASS_HAND_DRAWN_EDGE_STROKE_WIDTH,
            );
        }
        out.push_str(r#" id=""#);
        let _ = write!(out, "{}", ctx.diagram_id);
        ctx.emit.checkpoint()?;
        let _ = write!(
            out,
            r#"-{}" class="{}" data-edge="true" data-et="edge" data-id="{}" data-points="{}""#,
            escape_attr_display(&edge_dom_id_buf),
            escape_attr_display(&edge_class_buf),
            escape_attr_display(&edge_dom_id_buf),
            edge_points_b64_buf,
        );
        let _ = write!(out, r#" data-look="{}""#, escape_attr_display(ctx.look));
        write_class_marker_references(out, ctx, shaft_markers)?;
        edge_style_buf.clear();
        if use_neo_mask {
            #[cfg(feature = "layout-shoal")]
            let length = canonical.as_ref().and_then(|_| {
                paint_scratch
                    .route
                    .measured_length()
                    .map(|length| f64::from(length as f32))
                    .filter(|length| length.is_finite())
            });
            #[cfg(not(feature = "layout-shoal"))]
            let length: Option<f64> = None;
            if let Some(length) = length
                .or_else(|| merman_render_core::svg_edge_parity::svg_path_length_from_d(paint_d))
            {
                merman_render_core::svg_edge_parity::edge_path::write_neo_edge_mask(
                    &mut edge_style_buf,
                    length,
                    relation.and_then(|rel| class_arrow_type_for_relation_end(rel.relation.type1)),
                    relation.and_then(|rel| class_arrow_type_for_relation_end(rel.relation.type2)),
                    edge_class_buf
                        .split_whitespace()
                        .any(|class| class == "edge-pattern-dashed"),
                    false,
                );
            }
        }
        if uses_canonical_routes {
            if is_note_edge {
                if !edge_style_buf.is_empty() && !edge_style_buf.ends_with(';') {
                    edge_style_buf.push(';');
                }
                edge_style_buf.push_str("fill: none");
            }
        } else {
            edge_style_buf.push_str(class_edge_path_style(
                e.id.as_str(),
                ctx.look == "handDrawn",
            ));
        }
        let path_style = if let Some(hopped_d) = hopped_d {
            merman_render_core::svg_edge_parity::line_hops::rewrite_style_after_line_hop(
                &edge_style_buf,
                hopped_d,
                ctx.work_meter,
            )?
        } else {
            std::borrow::Cow::Borrowed(edge_style_buf.as_str())
        };
        if !uses_canonical_routes || !path_style.is_empty() {
            let _ = write!(out, r#" style="{}""#, escape_attr_display(&path_style));
        }
        out.push_str("/>");
        if separate_markers.iter().any(Option::is_some) {
            ctx.work_meter.charge_emit_work(render_d.len())?;
            let _ = write!(
                out,
                r#"<path d="{}" fill="none" stroke="none""#,
                escape_attr_display(render_d),
            );
            write_class_marker_references(out, ctx, separate_markers)?;
            out.push_str("/>");
        }
        out.checkpoint()?;
    }
    out.push_str("</g>");
    out.checkpoint()?;
    if let Some(s) = edge_paths_start {
        detail.edge_paths += s.elapsed();
    }

    Ok(ClassEdgePathEmission {
        ordered_edges,
        label_centers: edge_label_centers,
    })
}

fn terminal_label_gap(
    shaft: &kurbo::BezPath,
    rect: kurbo::Rect,
    stroke_outset: f64,
) -> Option<kurbo::Rect> {
    use kurbo::ParamCurve as _;

    let ink = rect.inflate(stroke_outset, stroke_outset);
    let corners = [
        ink.origin(),
        (ink.x1, ink.y0).into(),
        (ink.x1, ink.y1).into(),
        (ink.x0, ink.y1).into(),
    ];
    let sides =
        std::array::from_fn::<_, 4, _>(|i| kurbo::Line::new(corners[i], corners[(i + 1) % 4]));
    shaft
        .segments()
        .any(|segment| {
            ink.contains(segment.start())
                || ink.contains(segment.end())
                || sides
                    .iter()
                    .any(|side| !segment.intersect_line(*side).is_empty())
        })
        .then(|| rect.inflate(4.0, 2.0))
}

fn separate_class_edge_markers(
    holes: &[kurbo::Rect],
    points: &[LayoutPoint],
    radii: [f64; 2],
    shaft_markers: &mut [Option<&'static str>; 2],
) -> [Option<&'static str>; 2] {
    // SVG applies the shaft clip to its markers too. Keep intersecting markers on
    // a separate carrier with the same path geometry and marker orientations.
    std::array::from_fn(|end| {
        let point = [points.first(), points.last()][end]?;
        let diameter = 2.0 * radii[end];
        let ink = kurbo::Rect::from_center_size((point.x, point.y), (diameter, diameter));
        if holes.iter().any(|hole| hole.intersect(ink).area() > 0.0) {
            shaft_markers[end].take()
        } else {
            None
        }
    })
}

fn write_class_marker_references(
    out: &mut impl SvgOutput,
    ctx: &ClassEdgeGroupsRenderContext<'_>,
    markers: [Option<&'static str>; 2],
) -> merman_render_core::Result<()> {
    for (attribute, name) in ["marker-start", "marker-end"].into_iter().zip(markers) {
        if let Some(name) = name {
            let _ = write!(out, r#" {attribute}="url(#{}"#, ctx.diagram_id);
            ctx.emit.checkpoint()?;
            let _ = write!(
                out,
                r#"_{}-{}{})""#,
                escape_attr_display(ctx.diagram_marker_class),
                name,
                if ctx.look == "neo" { "-margin" } else { "" },
            );
        }
    }
    Ok(())
}

pub(super) fn render_class_edge_labels<O: SvgOutput>(
    out: &mut O,
    content_bounds: &mut Option<Bounds>,
    detail: &mut ClassRenderDetails,
    ctx: &ClassEdgeGroupsRenderContext<'_>,
    path_emission: &ClassEdgePathEmission<'_>,
) -> merman_render_core::Result<()> {
    let mut edge_dom_id_buf = String::with_capacity(64);
    let edge_labels_start = ctx.timing.start();
    let ordered_edges = &path_emission.ordered_edges;
    out.push_str(r#"<g class="edgeLabels">"#);
    out.checkpoint()?;
    // Mermaid's serialized SVG keeps all `edgeLabel` groups before `edgeTerminals`.
    // ELK awaits each edge label and its terminals; Dagre inserts center labels concurrently.
    for e in ordered_edges.iter().copied() {
        ctx.emit.checkpoint()?;
        class_edge_dom_id_into(&mut edge_dom_id_buf, e, ctx.relation_index_by_id);
        let relation = ctx.relations_by_id.get(e.id.as_str()).copied();
        let label_text = if ctx.uses_grid() {
            // Grid moved the center text to the nodes layer. Terminals retain the empty wrapper.
            ""
        } else {
            relation
                .map(|relation| relation.source.title.as_str())
                .unwrap_or("")
        };

        // The common registered-layout renderer only inserts labels for edges with
        // center or terminal text. Dagre still inserts an empty center wrapper.
        let has_terminal_label = relation.is_some_and(|rel| {
            [&rel.relation_title_1, &rel.relation_title_2]
                .into_iter()
                .flatten()
                .any(|text| !text.is_empty() && text != "none")
        });
        if ctx.uses_flat_edge_labels() && label_text.is_empty() && !has_terminal_label {
            continue;
        }

        let label_center = e.label.as_ref().map(|lbl| {
            path_emission
                .label_centers
                .get(e.id.as_str())
                .cloned()
                .unwrap_or_else(|| ctx.translated_label_center(lbl))
        });
        if !label_text.trim().is_empty()
            && let (Some(lbl), Some(center)) = (e.label.as_ref(), label_center.as_ref())
        {
            include_xywh(
                content_bounds,
                center.x - lbl.width / 2.0 + ctx.bounds_dx,
                center.y - lbl.height / 2.0 + ctx.bounds_dy,
                lbl.width.max(0.0),
                lbl.height.max(0.0),
            );
        }
        render_class_edge_label_group(
            out,
            edge_dom_id_buf.as_str(),
            label_text,
            e.label.as_ref(),
            ctx.prepared_labels.edges_by_id.get(e.id.as_str()),
            label_center
                .as_ref()
                .map_or((0.0, 0.0), |center| (center.x, center.y)),
            ctx,
        )?;
        out.checkpoint()?;
        if ctx.uses_flat_edge_labels() {
            render_class_edge_terminals(out, e, true, content_bounds, ctx)?;
            render_class_edge_terminals(out, e, false, content_bounds, ctx)?;
        }
    }
    if !ctx.uses_flat_edge_labels() {
        for e in ordered_edges.iter().copied() {
            ctx.emit.checkpoint()?;
            render_class_edge_terminals(out, e, true, content_bounds, ctx)?;
        }
        // Dagre starts all insertEdgeLabel futures together. End-only labels precede
        // labels whose start terminal adds another await before their end terminal.
        let mut ordered_end_edges = ordered_edges
            .iter()
            .copied()
            .enumerate()
            .collect::<Vec<_>>();
        ordered_end_edges.sort_by_key(|(idx, edge)| {
            (
                edge.start_label_left.is_some() || edge.start_label_right.is_some(),
                *idx,
            )
        });
        for (_, e) in ordered_end_edges {
            ctx.emit.checkpoint()?;
            render_class_edge_terminals(out, e, false, content_bounds, ctx)?;
        }
    }
    out.push_str("</g>");
    out.checkpoint()?;
    if let Some(s) = edge_labels_start {
        detail.edge_labels += s.elapsed();
    }
    Ok(())
}

pub(super) fn class_edge_label_center(
    original_path_points: Option<&[LayoutPoint]>,
    label_path_points: &[LayoutPoint],
    rendered_d: &str,
    points_were_explicitly_updated: bool,
    label: &LayoutLabel,
    content_tx: f64,
    content_ty: f64,
) -> LayoutPoint {
    position_edge_label(
        LayoutPoint {
            x: label.x + content_tx,
            y: label.y + content_ty,
        },
        original_path_points,
        label_path_points,
        rendered_d,
        points_were_explicitly_updated,
    )
}

fn render_class_edge_label_group(
    out: &mut impl SvgOutput,
    dom_id: &str,
    label_text: &str,
    label: Option<&LayoutLabel>,
    prepared_svg_label: Option<&crate::class::ClassPreparedSvgEdgeLabel>,
    (center_x, center_y): (f64, f64),
    ctx: &ClassEdgeGroupsRenderContext<'_>,
) -> merman_render_core::Result<()> {
    let normalized = if ctx.uses_elk_adapter_dom {
        merman_render_core::text::line_break::mermaid_html_breaks_to_newlines(label_text)
    } else {
        std::borrow::Cow::Borrowed(label_text)
    };
    let decoded = decode_entities_minimal_cow(&normalized);
    let trimmed = decoded.trim();
    let use_html_labels =
        ctx.edge_use_html_labels || merman_render_core::math::contains_delimited_math(trimmed);
    let background_fill = ctx.edge_label_fill((center_x, center_y));
    if use_html_labels {
        let empty_div_style =
            class_html_div_style(0.0, MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX as i64);
        if trimmed.is_empty() {
            let _ = write!(
                out,
                r#"<g class="edgeLabel"><g class="label" data-id="{}" transform="translate(0, 0)"><foreignObject width="0" height="0"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="{}"><span class="edgeLabel"></span></div></foreignObject></g></g>"#,
                escape_attr_display(dom_id),
                escape_attr_display(empty_div_style.as_str())
            );
        } else if let Some(lbl) = label {
            let width = lbl.width.max(0.0);
            let height = lbl.height.max(0.0);
            let mut div_style =
                class_html_div_style(width, MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX as i64);
            let background_style =
                background_fill.map(|fill| format!("background-color:{fill} !important;"));
            if let Some(style) = &background_style {
                div_style.push_str(style);
            }
            let _ = write!(
                out,
                r#"<g class="edgeLabel" transform="translate({}, {})"><g class="label" data-id="{}" transform="translate({}, {})"><foreignObject width="{}" height="{}"{}><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="{}">"#,
                fmt(center_x),
                fmt(center_y),
                escape_attr_display(dom_id),
                fmt(-lbl.width / 2.0),
                fmt(-lbl.height / 2.0),
                fmt(width),
                fmt(height),
                html_label_overflow_attr(ctx.uses_canonical_routes(), width, height),
                escape_attr_display(div_style.as_str()),
            );
            render_class_html_label(
                out,
                &ClassHtmlLabelSpec {
                    span_class: "edgeLabel",
                    text: trimmed,
                    include_p: true,
                    extra_span_class: None,
                    span_style: background_style.as_deref(),
                    prepared_xhtml: None,
                    mermaid_config: ctx.mermaid_config,
                    math_renderer: ctx.math_renderer,
                },
            );
            out.push_str("</div></foreignObject></g></g>");
        } else {
            let _ = write!(
                out,
                r#"<g class="edgeLabel"><g class="label" data-id="{}" transform="translate(0, 0)"><foreignObject width="0" height="0"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="{}"><span class="edgeLabel"></span></div></foreignObject></g></g>"#,
                escape_attr_display(dom_id),
                escape_attr_display(empty_div_style.as_str())
            );
        }
        return Ok(());
    }

    let prepared_svg_label = if trimmed.is_empty() {
        None
    } else {
        Some(
            prepared_svg_label.ok_or_else(|| merman_render_core::Error::InvalidModel {
                message: format!("missing prepared Class SVG relationship label for `{dom_id}`"),
            })?,
        )
    };

    if trimmed.is_empty() {
        const EMPTY_BACKGROUND: &str = r#"<g><rect class="background" style="stroke: none"/></g>"#;
        if !ctx.uses_grid() {
            out.push_str(EMPTY_BACKGROUND);
        }
        let _ = write!(
            out,
            r#"<g class="edgeLabel"><g class="label" data-id="{}" transform="translate(0, 0)">"#,
            escape_attr_display(dom_id)
        );
        write_class_svg_edge_text(out, "", false);
        out.push_str("</g></g>");
        if ctx.uses_grid() {
            out.push_str(EMPTY_BACKGROUND);
        }
    } else if let Some(lbl) = label {
        let prepared = prepared_svg_label.expect("nonempty prepared Class edge label");
        let bounds = &prepared.text_bounds;
        let padding = crate::class::CLASS_SVG_EDGE_LABEL_PADDING_PX;
        let background_style = background_fill
            .map(|fill| format!("fill:{fill} !important;"))
            .unwrap_or_default();
        let _ = write!(
            out,
            r#"<g class="edgeLabel" transform="translate({}, {})"><g class="label" data-id="{}" transform="translate({}, {})"><g><rect class="background" style="{}" x="{}" y="{}" width="{}" height="{}"/>"#,
            fmt(center_x),
            fmt(center_y),
            escape_attr_display(dom_id),
            fmt(-(bounds.min_x + bounds.max_x) / 2.0),
            fmt(-(bounds.min_y + bounds.max_y) / 2.0),
            escape_attr_display(&background_style),
            fmt(bounds.min_x - padding),
            fmt(bounds.min_y - padding),
            fmt(lbl.width.max(0.0)),
            fmt(lbl.height.max(0.0)),
        );
        let prepared = &prepared.label;
        out.push_str(r#"<text y="-10.1" text-anchor="middle">"#);
        super::node::write_class_svg_label_rows(out, prepared.lines(), false, false, true);
        out.push_str("</text>");
        out.push_str("</g></g></g>");
    } else {
        out.push_str(r#"<g><rect class="background" style="stroke: none"/></g>"#);
        let _ = write!(
            out,
            r#"<g class="edgeLabel"><g class="label" data-id="{}" transform="translate(0, 0)">"#,
            escape_attr_display(dom_id)
        );
        out.push_str(r#"<text y="-10.1" text-anchor="middle">"#);
        super::node::write_class_svg_label_rows(
            out,
            prepared_svg_label
                .expect("nonempty prepared Class edge label")
                .label
                .lines(),
            false,
            false,
            true,
        );
        out.push_str("</text>");
        out.push_str("</g></g>");
    }
    Ok(())
}

fn render_class_edge_terminals(
    out: &mut impl SvgOutput,
    edge: &LayoutEdge,
    is_start: bool,
    content_bounds: &mut Option<Bounds>,
    ctx: &ClassEdgeGroupsRenderContext<'_>,
) -> merman_render_core::Result<()> {
    let Some(relation) = ctx.relations_by_id.get(edge.id.as_str()).copied() else {
        return Ok(());
    };
    let (text, labels) = if is_start {
        (
            relation.relation_title_1.as_deref().unwrap_or_default(),
            [&edge.start_label_left, &edge.start_label_right],
        )
    } else {
        (
            relation.relation_title_2.as_deref().unwrap_or_default(),
            [&edge.end_label_left, &edge.end_label_right],
        )
    };
    let text = if ctx.edge_use_html_labels {
        std::borrow::Cow::Borrowed(text)
    } else {
        decode_entities_minimal_cow(text)
    };
    let text = text.trim();
    for label in labels {
        if let Some(label) = label
            .as_ref()
            .filter(|label| !text.is_empty() && label.width > 0.0 && label.height > 0.0)
        {
            // setTerminalWidth retains the legacy CSS width floor, but never clips the
            // measured text width or replaces its measured height with a fixed 12px box.
            let visible_width = if ctx.edge_use_html_labels && !ctx.uses_canonical_routes() {
                label.width.max(text.encode_utf16().count() as f64 * 9.0)
            } else {
                label.width
            };
            include_xywh(
                content_bounds,
                label.x - label.width / 2.0 + ctx.content_tx + ctx.bounds_dx,
                label.y - label.height / 2.0 + ctx.content_ty + ctx.bounds_dy,
                visible_width,
                label.height,
            );
            render_class_edge_terminal_group(out, label, text, visible_width, ctx);
            out.checkpoint()?;
        }
    }
    Ok(())
}

fn render_class_edge_terminal_group<O: SvgOutput>(
    out: &mut O,
    label: &LayoutLabel,
    text: &str,
    style_width: f64,
    ctx: &ClassEdgeGroupsRenderContext<'_>,
) {
    let x = label.x + ctx.content_tx;
    let y = label.y + ctx.content_ty;
    if ctx.edge_use_html_labels {
        let overflow = if ctx.uses_canonical_routes() {
            " overflow: visible;"
        } else {
            ""
        };
        let _ = write!(
            out,
            r#"<g class="edgeTerminals" transform="translate({}, {})"><g class="inner" transform="translate({}, {})"><foreignObject width="{}" height="{}" style="width: {}px; height: {}px;{overflow}"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5;">"#,
            fmt(x),
            fmt(y),
            fmt(-label.width / 2.0),
            fmt(-label.height / 2.0),
            fmt(label.width),
            fmt(label.height),
            fmt(style_width),
            fmt(label.height)
        );
        out.push_str(r#"<span class="edgeLabel">"#);
        render_class_terminal_label(out, text, ctx.mermaid_config, ctx.math_renderer);
        out.push_str("</span></div></foreignObject></g></g>");
    } else {
        let bbox_y = ctx
            .text_measurer
            .measure_svg_create_text_bbox_y_offset_px(text, ctx.terminal_text_style);
        let _ = write!(
            out,
            r#"<g class="edgeTerminals" transform="translate({}, {})"><g class="inner" transform="translate(0, {})"><g><rect class="background" style="stroke: none"/>"#,
            fmt(x),
            fmt(y),
            fmt(-(bbox_y + label.height / 2.0))
        );
        write_class_svg_edge_text(out, text, false);
        out.push_str("</g></g></g>");
    }
}

fn render_class_terminal_label(
    out: &mut impl SvgOutput,
    text: &str,
    mermaid_config: Option<&merman_model::presentation::PresentationConfig>,
    math_renderer: Option<&(dyn merman_render_core::math::MathRenderer + Send + Sync)>,
) {
    let decoded = decode_entities_minimal_cow(text);
    if let Some(math_html) = class_math_html_label(&decoded, mermaid_config, math_renderer) {
        out.push_str(&math_html);
    } else if let Some(config) = mermaid_config {
        let xhtml = crate::class::class_plain_html_fragment(text, config);
        out.push_str(&xhtml);
    } else {
        out.push_str("<p>");
        escape_xml_into(out, text);
        out.push_str("</p>");
    }
}

pub(super) fn class_edge_dom_id_into(
    out: &mut String,
    edge: &LayoutEdge,
    relation_index_by_id: &FxHashMap<&str, usize>,
) {
    out.clear();
    if edge.id.starts_with("edgeNote") {
        if let Some(note_idx) = edge
            .from
            .strip_prefix("note")
            .and_then(|rest| rest.parse::<usize>().ok())
        {
            let _ = write!(out, "edgeNote{note_idx}");
            return;
        }
        out.push_str(edge.id.as_str());
        return;
    }
    // Mermaid uses `getEdgeId` with prefix `id`.
    let idx = relation_index_by_id
        .get(edge.id.as_str())
        .copied()
        .unwrap_or(1);
    crate::class::write_class_relation_dom_id(out, &edge.from, &edge.to, idx);
}

pub(super) fn class_edge_pattern(line_type: i32) -> &'static str {
    // Mermaid class diagram `lineType` uses "dottedLine" for `..` which maps to the dashed pattern.
    if line_type == 1 {
        "edge-pattern-dashed"
    } else {
        "edge-pattern-solid"
    }
}

pub(super) fn class_note_edge_pattern() -> &'static str {
    "edge-pattern-dotted"
}

pub(super) fn class_edge_path_style(edge_id: &str, hand_drawn: bool) -> &'static str {
    if hand_drawn && edge_id.starts_with("edgeNote") {
        ";fill: none"
    } else if hand_drawn {
        ";"
    } else if edge_id.starts_with("edgeNote") {
        "fill: none;;;fill: none"
    } else {
        ";;;"
    }
}

pub(super) fn class_edge_render_order<'a>(
    edges: &'a [LayoutEdge],
    relation_index_by_id: &FxHashMap<&str, usize>,
) -> Vec<&'a LayoutEdge> {
    let mut ordered = edges.iter().collect::<Vec<_>>();
    ordered.sort_by(|a, b| {
        let a_key = if a.id.starts_with("edgeNote") {
            (
                0_u8,
                a.id.trim_start_matches("edgeNote")
                    .parse::<usize>()
                    .unwrap_or(usize::MAX),
                a.id.as_str(),
            )
        } else {
            (
                1_u8,
                relation_index_by_id
                    .get(a.id.as_str())
                    .copied()
                    .unwrap_or(usize::MAX),
                a.id.as_str(),
            )
        };
        let b_key = if b.id.starts_with("edgeNote") {
            (
                0_u8,
                b.id.trim_start_matches("edgeNote")
                    .parse::<usize>()
                    .unwrap_or(usize::MAX),
                b.id.as_str(),
            )
        } else {
            (
                1_u8,
                relation_index_by_id
                    .get(b.id.as_str())
                    .copied()
                    .unwrap_or(usize::MAX),
                b.id.as_str(),
            )
        };
        a_key.cmp(&b_key)
    });
    ordered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_terminal_gap_covers_ink_only_when_the_painted_shaft_hits_it() {
        let shaft = kurbo::BezPath::from_svg("M0 0H30V30").unwrap();
        let in_corner = kurbo::Rect::new(7.0, 9.0, 23.0, 14.0);
        assert_eq!(terminal_label_gap(&shaft, in_corner, 1.0), None);

        let on_shaft = kurbo::Rect::new(7.0, -1.0, 23.0, 4.0);
        assert_eq!(
            terminal_label_gap(&shaft, on_shaft, 1.0),
            Some(kurbo::Rect::new(3.0, -3.0, 27.0, 6.0))
        );
    }

    #[test]
    fn class_terminal_gap_padding_keeps_the_marker_on_an_unclipped_carrier() {
        let shaft = kurbo::BezPath::from_svg("M0 0H50").unwrap();
        let label = kurbo::Rect::new(18.0, -5.0, 25.0, 5.0);
        let gap = terminal_label_gap(&shaft, label, 0.5).unwrap();
        let points = [
            LayoutPoint { x: 0.0, y: 0.0 },
            LayoutPoint { x: 50.0, y: 0.0 },
        ];
        let mut shaft_markers = [Some("compositionStart"), Some("dependencyEnd")];
        let carriers =
            separate_class_edge_markers(&[gap], &points, [17.0, 8.0], &mut shaft_markers);
        assert_eq!(carriers, [Some("compositionStart"), None]);
        assert_eq!(shaft_markers, [None, Some("dependencyEnd")]);
    }

    fn label_at(x: f64, y: f64) -> LayoutLabel {
        LayoutLabel {
            x,
            y,
            width: 20.0,
            height: 16.0,
        }
    }

    #[test]
    fn class_edge_order_preserves_notes_source_indices_and_stable_ties() {
        let edges = [
            ("edge10", "source-second"),
            ("edgeNote10", "note-ten"),
            ("edgeNote2", "note-two-first"),
            ("fallback-z", "fallback-last"),
            ("edge2", "source-first"),
            ("fallback-a", "fallback-first"),
            ("edgeNote2", "note-two-second"),
            ("edge9", "source-second-tie"),
            ("edgeNoteInvalid", "note-invalid"),
            ("edgeNote02", "note-two-id-tie"),
        ]
        .map(|(id, from)| LayoutEdge {
            id: id.to_owned(),
            from: from.to_owned(),
            to: String::new(),
            from_cluster: None,
            to_cluster: None,
            points: Vec::new(),
            label: None,
            start_label_left: None,
            start_label_right: None,
            end_label_left: None,
            end_label_right: None,
            start_marker: None,
            end_marker: None,
            stroke_dasharray: None,
        });
        let relation_indices = FxHashMap::from_iter([("edge2", 0), ("edge10", 1), ("edge9", 1)]);

        let ordered = class_edge_render_order(&edges, &relation_indices);
        assert_eq!(
            ordered
                .iter()
                .map(|edge| edge.from.as_str())
                .collect::<Vec<_>>(),
            [
                "note-two-id-tie",
                "note-two-first",
                "note-two-second",
                "note-ten",
                "note-invalid",
                "source-first",
                "source-second",
                "source-second-tie",
                "fallback-first",
                "fallback-last",
            ]
        );
    }

    #[test]
    fn class_edge_label_uses_pre_curve_points_when_cluster_clipping_updates_the_path() {
        let label_path_points = [
            LayoutPoint { x: 0.0, y: 0.0 },
            LayoutPoint { x: 30.0, y: 0.0 },
            LayoutPoint { x: 100.0, y: 0.0 },
        ];

        let original_path_points = [
            LayoutPoint { x: -20.0, y: 0.0 },
            LayoutPoint { x: 100.0, y: 0.0 },
        ];
        // Preserve the provider's offset from the original midpoint when clipping moves it.
        let center = class_edge_label_center(
            Some(&original_path_points),
            &label_path_points,
            "M17.25,0L30,0L100,0",
            true,
            &label_at(12.0, 8.0),
            0.0,
            0.0,
        );

        assert_eq!((center.x, center.y), (22.0, 8.0));
    }

    #[test]
    fn class_edge_label_checks_the_actual_rendered_path_before_repositioning() {
        let label_path_points = [
            LayoutPoint { x: 0.0, y: 0.0 },
            LayoutPoint { x: 10.0, y: 0.0 },
            LayoutPoint { x: 20.0, y: 0.0 },
        ];
        let label = label_at(4.0, 5.0);

        let unchanged = class_edge_label_center(
            Some(&label_path_points),
            &label_path_points,
            "M0,0L10,0L20,0",
            false,
            &label,
            100.0,
            200.0,
        );
        assert_eq!((unchanged.x, unchanged.y), (104.0, 205.0));

        let rough_path = class_edge_label_center(
            Some(&label_path_points),
            &label_path_points,
            "M1.25,2.5C3.5,4.5,5.5,6.5,7.5,8.5",
            false,
            &label,
            100.0,
            200.0,
        );
        assert_eq!((rough_path.x, rough_path.y), (104.0, 205.0));
    }
}
