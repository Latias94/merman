//! Facade geometry, backdrop sampling, and text providers for Flow edge labels.

use crate::edge_labels::{
    FlowchartEdgeLabelPlacement, FlowchartEdgeLabelText, OrdinaryEdgeLabel, SwimlaneEdgeLabel,
    SwimlaneEdgeLabelPaint, write_ordinary_edge_label,
};
use kurbo::Size;

use super::super::*;

fn position_flowchart_edge_label(
    dagre_anchor: merman_render_core::LayoutPoint,
    geom: &FlowchartEdgePathGeom,
    always_recompute: bool,
) -> merman_render_core::LayoutPoint {
    let rendered_d = geom
        .emitted_d_for_label
        .as_deref()
        .unwrap_or(geom.d.as_str());
    merman_render_core::svg_edge_parity::edge_label_geometry::position_edge_label(
        geom.label_anchor.clone().unwrap_or(dagre_anchor),
        Some(geom.original_label_path_points.as_slice()),
        &geom.label_path_points,
        rendered_d,
        geom.label_path_was_explicitly_updated || always_recompute,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "The position resolver consumes cached geometry with its edge identity, local origin, and explicit recomputation policy."
)]
pub(in crate::svg) fn resolve_flowchart_edge_label_position(
    ctx: &FlowchartRenderCtx<'_>,
    key: crate::projection::FlowchartEdgeKey,
    layout_edge: FlowchartLayoutEdgeRef<'_>,
    label: &merman_render_core::LayoutLabel,
    origin_x: f64,
    origin_y: f64,
    edge_cache: &FxHashMap<crate::projection::FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
    always_recompute: bool,
) -> merman_render_core::LayoutPoint {
    let layout_anchor = merman_render_core::LayoutPoint {
        x: label.x + ctx.tx - origin_x,
        y: label.y + ctx.ty - origin_y,
    };
    if let Some(geom) = edge_cache
        .get(&key)
        .filter(|entry| {
            (entry.origin_x - origin_x).abs() <= 1e-9 && (entry.origin_y - origin_y).abs() <= 1e-9
        })
        .map(|entry| &entry.geom)
    {
        return position_flowchart_edge_label(layout_anchor, geom, always_recompute);
    }

    let points = layout_edge.points();
    if let Some(midpoint) =
        edge_geom::missing_section_label_position(ctx, key, points, origin_x, origin_y)
    {
        return midpoint;
    }

    // Geometry caching only skips routes with fewer than two points. For that degenerate case,
    // `calcLabelPosition` either keeps the anchor (empty) or returns the sole waypoint.
    if always_recompute
        || layout_edge
            .materialized()
            .is_some_and(|edge| edge.to_cluster.is_some() || edge.from_cluster.is_some())
    {
        let points = points
            .iter()
            .map(|point| merman_render_core::LayoutPoint {
                x: point.x + ctx.tx - origin_x,
                y: point.y + ctx.ty - origin_y,
            })
            .collect::<Vec<_>>();
        return merman_render_core::svg_edge_parity::edge_label_geometry::position_edge_label(
            layout_anchor,
            (!always_recompute).then_some(points.as_slice()),
            &points,
            "",
            true,
        );
    }

    layout_anchor
}

fn render_native_label_background(
    out: &mut impl crate::svg::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    hierarchy: &FlowchartHierarchyPlan<'_>,
    position: merman_render_core::LayoutPoint,
    (origin_x, origin_y): (f64, f64),
    (width, height): (f64, f64),
) -> merman_render_core::Result<Option<merman_theme_contract::v2::Color>> {
    use merman_theme_contract::v2::EdgeLabelBackground;

    let Some(native) = ctx.native else {
        return Ok(None);
    };
    let mode = native.form.edge_label_bg;
    let fill = match mode {
        EdgeLabelBackground::Pill => native.node,
        EdgeLabelBackground::Canvas => native.canvas,
        EdgeLabelBackground::Auto | EdgeLabelBackground::Transparent => {
            let layers = match ctx.container_label_layers.get() {
                Some(layers) => layers,
                None => {
                    let layers = ContainerLabelLayers::prepare(ctx, native, hierarchy)?;
                    ctx.container_label_layers.get_or_init(|| layers)
                }
            };
            let point = kurbo::Point::new(
                position.x + origin_x - ctx.tx,
                position.y + origin_y - ctx.ty,
            );
            layers.fill_at(ctx, point, native.canvas)?
        }
    };
    let bounds = kurbo::Rect::from_center_size((position.x, position.y), (width, height));
    let _ = merman_render_core::theme_svg::edge_label_background::write_backplate(
        out,
        bounds,
        mode,
        fill,
        native.border,
    );
    out.checkpoint()?;
    Ok(Some(fill))
}

pub(in crate::svg) struct ContainerLabelLayers(Vec<ContainerLabelLayer>);

struct ContainerLabelLayer {
    bounds: kurbo::Rect,
    title: Option<kurbo::Rect>,
    paint: std::result::Result<ContainerLabelPaint, String>,
}

struct ContainerLabelPaint {
    body: merman_theme_contract::v2::Color,
    title: merman_theme_contract::v2::Color,
    opacity: f64,
}

impl ContainerLabelLayers {
    fn prepare(
        ctx: &FlowchartRenderCtx<'_>,
        native: &crate::theme::FlowchartNativeStyle,
        hierarchy: &FlowchartHierarchyPlan<'_>,
    ) -> merman_render_core::Result<Self> {
        let meter = ctx.work_meter;
        let clusters = hierarchy.painted_clusters();
        meter.charge_emit_work(clusters.len())?;
        let mut layers = Vec::with_capacity(clusters.len());
        let canonical = ctx.layout.route_contract.group_parents().is_some();
        for (index, cluster) in clusters.iter().enumerate() {
            if index != 0 && index.is_multiple_of(256) {
                meter.charge_emit_work(0)?;
            }
            let id = cluster.id.as_str();
            if ctx.is_subgraph_collapsed(id) {
                continue;
            }
            let (bounds, title) = if let Some(lane) = ctx.swimlane_lanes_by_id.get(id)
                && super::super::swimlane::is_lane_or_pool_cluster(ctx, id)
                && let Some(direction) = ctx.swimlane_direction
            {
                let geometry =
                    super::super::swimlane::swimlane_terminal_geometry(lane, direction, canonical);
                let (body, title) = geometry.frame_parts((lane.x, lane.y), direction, canonical);
                (body.union(title), Some(title))
            } else {
                if !ctx.subgraph_has_children(id)
                    && !super::flowchart_elk_renders_empty_subgraph_as_cluster(ctx)
                {
                    continue;
                }
                (
                    super::super::cluster_geometry::painted_cluster_frame(ctx, cluster).bounds(),
                    None,
                )
            };
            let paint = match ctx.prepared_nodes.cluster(id) {
                Ok(terminal) => {
                    let authored = terminal.compiled.fill.as_deref().and_then(
                        merman_render_core::theme_svg::edge_label_background::literal_fill,
                    );
                    Ok(ContainerLabelPaint {
                        body: authored.unwrap_or(native.container),
                        title: authored.unwrap_or(native.emphasis),
                        opacity: terminal.compiled.fill_opacity,
                    })
                }
                Err(_) => Err(id.to_owned()),
            };
            layers.push(ContainerLabelLayer {
                bounds,
                title,
                paint,
            });
        }
        meter.charge_emit_work(0)?;
        Ok(Self(layers))
    }

    fn fill_at(
        &self,
        ctx: &FlowchartRenderCtx<'_>,
        point: kurbo::Point,
        mut backdrop: merman_theme_contract::v2::Color,
    ) -> merman_render_core::Result<merman_theme_contract::v2::Color> {
        ctx.work_meter.charge_emit_work(self.0.len())?;
        for layer in &self.0 {
            if !layer.bounds.contains(point) {
                continue;
            }
            // A frame without a prepared terminal only matters when it is sampled.
            let paint =
                layer
                    .paint
                    .as_ref()
                    .map_err(|id| merman_render_core::Error::InvalidModel {
                        message: format!("missing prepared Flowchart cluster `{id}`"),
                    })?;
            let fill = if layer.title.is_some_and(|title| title.contains(point)) {
                paint.title
            } else {
                paint.body
            };
            backdrop = fill.composite(backdrop, paint.opacity);
        }
        Ok(backdrop)
    }
}

fn edge_label_text<'a>(ctx: &'a FlowchartRenderCtx<'_>) -> FlowchartEdgeLabelText<'a> {
    FlowchartEdgeLabelText {
        edge_style_plan: ctx.edge_style_plan,
        svg_label_sidecar: ctx.svg_label_sidecar,
        measurer: ctx.measurer,
        math_renderer: ctx.math_renderer,
        text_style: ctx.text_style,
        config: ctx.config,
        default_text_color: &ctx.compatibility.text_color,
    }
}

pub(in crate::svg) fn render_flowchart_edge_label(
    out: &mut impl crate::svg::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    edge_ref: super::super::render_input::FlowchartRenderEdgeRef<'_>,
    origin_x: f64,
    origin_y: f64,
    hierarchy: &FlowchartHierarchyPlan<'_>,
    placements: &super::super::paint::FlowchartEdgeLabels,
) -> merman_render_core::Result<()> {
    let key = edge_ref.key;
    let label_text = ctx
        .model
        .edge_label_for_occurrence(key, edge_ref.edge)
        .unwrap_or_default();
    let source_label_empty = crate::svg::label::flowchart_label_is_empty_for_render(label_text);
    let placement = if source_label_empty && !ctx.edge_html_labels {
        None
    } else {
        ctx.layout_edges_by_key.get(&key).map(|edge| {
            let position = placements.local_position(key, ctx, origin_x, origin_y);
            if let Some(label) = edge.label() {
                FlowchartEdgeLabelPlacement::Layout {
                    position: position.unwrap_or(merman_render_core::LayoutPoint {
                        x: label.x + ctx.tx - origin_x,
                        y: label.y + ctx.ty - origin_y,
                    }),
                    size: Size::new(label.width, label.height),
                }
            } else {
                FlowchartEdgeLabelPlacement::Fallback(position)
            }
        })
    };
    let sampled_backdrop = if ctx.native.is_some()
        && !source_label_empty
        && let Some(FlowchartEdgeLabelPlacement::Layout { position, size }) = &placement
    {
        render_native_label_background(
            out,
            ctx,
            hierarchy,
            position.clone(),
            (origin_x, origin_y),
            (size.width, size.height),
        )?
    } else {
        None
    };

    #[cfg(feature = "layout-shoal")]
    let html_label_inset = if ctx.edge_html_labels
        && !source_label_empty
        && ctx.native.is_none()
        && ctx.layout.route_contract.is_canonical_axis()
    {
        crate::edge_labels::HTML_EDGE_LABEL_INSET_PX
    } else {
        0.0
    };
    #[cfg(not(feature = "layout-shoal"))]
    let html_label_inset = 0.0;

    write_ordinary_edge_label(
        out,
        edge_label_text(ctx),
        OrdinaryEdgeLabel {
            edge: edge_ref,
            label_text,
            placement,
            sampled_backdrop,
            html_labels: ctx.edge_html_labels,
            html_label_inset,
            wrap_mode: ctx.edge_wrap_mode,
        },
    )
}

pub(in crate::svg) fn render_swimlane_edge_label_node(
    out: &mut impl crate::svg::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    node_id: &str,
    edge_ref: super::super::render_input::FlowchartRenderEdgeRef<'_>,
    (origin_x, origin_y): (f64, f64),
    hierarchy: &FlowchartHierarchyPlan<'_>,
    placements: &super::super::paint::FlowchartEdgeLabels,
) -> merman_render_core::Result<()> {
    let key = edge_ref.key;
    let label_text = ctx
        .model
        .edge_label_for_occurrence(key, edge_ref.edge)
        .unwrap_or_default();
    let bound = SwimlaneEdgeLabel::bind(
        edge_label_text(ctx),
        edge_ref,
        label_text,
        ctx.default_edge_style,
    )?;

    let Some(layout_edge) = ctx.layout_edges_by_key.get(&key) else {
        return Ok(());
    };
    let Some(label) = layout_edge.label() else {
        return Ok(());
    };

    // Mermaid 12.2's common renderer uses resolveEdgeLabelPosition: preserve the
    // layout anchor unless insertEdge marks the returned path as updated.
    let position = placements
        .local_position(key, ctx, origin_x, origin_y)
        .ok_or_else(|| merman_render_core::Error::InvalidModel {
            message: "missing prepared Swimlane label position".to_owned(),
        })?;
    let width = label.width.max(0.0);
    let height = label.height.max(0.0);
    let sampled_backdrop = if ctx.native.is_some() {
        render_native_label_background(
            out,
            ctx,
            hierarchy,
            position.clone(),
            (origin_x, origin_y),
            (width, height),
        )?
    } else {
        None
    };
    bound.write(
        out,
        SwimlaneEdgeLabelPaint {
            node_id,
            document_ids: ctx.document_ids(),
            position,
            size: Size::new(width, height),
            sampled_backdrop,
            html_labels: ctx.node_html_labels,
            wrap_mode: ctx.node_wrap_mode,
            wrapping_width: ctx.wrapping_width,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f64, y: f64) -> merman_render_core::LayoutPoint {
        merman_render_core::LayoutPoint { x, y }
    }

    fn geom(d: &str, points: Vec<merman_render_core::LayoutPoint>) -> FlowchartEdgePathGeom {
        FlowchartEdgePathGeom {
            d: d.to_string(),
            pb: None,
            neo_length: None,
            data_points: points.clone(),
            pre_hop_dash_mask_length: None,
            line_hop_applied: false,
            original_label_path_points: points.clone(),
            label_path_points: points,
            label_anchor: None,
            label_path_was_explicitly_updated: false,
            emitted_d_for_label: None,
            bounds_skipped_for_viewbox: false,
        }
    }

    #[test]
    fn ordinary_dagre_keeps_anchor_when_paint_does_not_move_the_route_midpoint() {
        let anchor = point(4.0, 5.0);
        let mut geometry = geom(
            "M0,0L10,0L20,0",
            vec![point(0.0, 0.0), point(10.0, 0.0), point(20.0, 0.0)],
        );

        let unchanged = position_flowchart_edge_label(anchor.clone(), &geometry, false);
        assert_eq!((unchanged.x, unchanged.y), (4.0, 5.0));

        geometry.label_path_was_explicitly_updated = true;
        let updated = position_flowchart_edge_label(anchor, &geometry, false);
        assert_eq!((updated.x, updated.y), (4.0, 5.0));
    }

    #[test]
    fn cluster_cut_moves_the_anchor_by_the_midpoint_delta() {
        let mut geometry = geom("M0,0L100,0", vec![point(0.0, 0.0), point(100.0, 0.0)]);
        geometry.original_label_path_points = vec![point(-20.0, 0.0), point(100.0, 0.0)];
        geometry.label_path_was_explicitly_updated = true;
        let label = position_flowchart_edge_label(point(12.0, 8.0), &geometry, false);
        assert_eq!((label.x, label.y), (22.0, 8.0));
    }

    #[test]
    fn elk_paints_the_projected_anchor_without_recentering_on_the_whole_route() {
        let mut geometry = geom(
            "M0,0L100,0L100,100",
            vec![point(0.0, 0.0), point(100.0, 0.0), point(100.0, 100.0)],
        );
        geometry.label_anchor = Some(point(60.0, 0.0));
        let label = position_flowchart_edge_label(point(60.0, 10.0), &geometry, false);
        assert_eq!((label.x, label.y), (60.0, 0.0));
        assert_ne!((label.x, label.y), (100.0, 0.0));
    }

    #[test]
    fn swimlane_updated_waypoints_still_apply_the_midpoint_delta() {
        let mut geometry = geom(
            "M0,0L10,0L20,0",
            vec![point(0.0, 0.0), point(10.0, 0.0), point(20.0, 0.0)],
        );
        geometry.original_label_path_points =
            vec![point(-20.0, 0.0), point(0.0, 0.0), point(20.0, 0.0)];
        geometry.label_path_was_explicitly_updated = true;
        let positioned = position_flowchart_edge_label(point(4.0, 5.0), &geometry, false);
        assert_eq!((positioned.x, positioned.y), (14.0, 5.0));
    }

    #[test]
    fn swimlane_unchanged_clipped_pins_preserve_the_layout_label_anchor() {
        let mut geometry = geom(
            "M403,212.5L667.929,212.5Q675,212.5 675,219.571L675,561.429Q675,568.5 667.929,568.5L407,568.5",
            vec![
                point(403.0, 212.5),
                point(675.0, 212.5),
                point(675.0, 568.5),
                point(403.0, 568.5),
            ],
        );
        geometry.original_label_path_points = vec![
            point(403.0, 212.5),
            point(403.0, 212.5),
            point(675.0, 212.5),
            point(675.0, 568.5),
            point(403.0, 568.5),
            point(403.0, 568.5),
        ];
        // upstream_7_car_sales_constr: clipping removes the duplicate terminal
        // pins, but insertEdge returns no updatedPath for this rounded route.
        let positioned = position_flowchart_edge_label(point(539.0, 212.5), &geometry, false);
        assert_eq!((positioned.x, positioned.y), (539.0, 212.5));
    }

    #[test]
    fn rough_actual_emitted_path_drives_updated_path_detection() {
        let anchor = point(4.0, 5.0);
        let mut geometry = geom(
            "M0,0L10,20L30,20",
            vec![point(0.0, 0.0), point(10.0, 20.0), point(30.0, 20.0)],
        );

        let logical_curve = position_flowchart_edge_label(anchor.clone(), &geometry, false);
        assert_eq!((logical_curve.x, logical_curve.y), (4.0, 5.0));

        geometry.emitted_d_for_label = Some("M1.1,2.2C3.3,4.4,5.5,6.6,7.7,8.8".to_string());
        let rough_curve = position_flowchart_edge_label(anchor, &geometry, false);
        assert_eq!((rough_curve.x, rough_curve.y), (4.0, 5.0));
    }

    #[test]
    fn shared_geometry_handles_empty_single_degenerate_and_polyline_paths() {
        let anchor = point(4.0, 5.0);
        let empty = geom("", Vec::new());
        let positioned = position_flowchart_edge_label(anchor.clone(), &empty, true);
        assert_eq!((positioned.x, positioned.y), (4.0, 5.0));

        let single = geom("M2,3", vec![point(2.0, 3.0)]);
        let positioned = position_flowchart_edge_label(anchor.clone(), &single, true);
        assert_eq!((positioned.x, positioned.y), (4.0, 5.0));

        let degenerate = geom("M2,3L2,3", vec![point(2.0, 3.0), point(2.0, 3.0)]);
        let positioned = position_flowchart_edge_label(anchor.clone(), &degenerate, true);
        assert_eq!((positioned.x, positioned.y), (4.0, 5.0));

        let polyline = geom(
            "M0,0L6,0L6,8",
            vec![point(0.0, 0.0), point(6.0, 0.0), point(6.0, 8.0)],
        );
        let positioned = position_flowchart_edge_label(anchor, &polyline, true);
        assert_eq!((positioned.x, positioned.y), (4.0, 5.0));
    }
}
