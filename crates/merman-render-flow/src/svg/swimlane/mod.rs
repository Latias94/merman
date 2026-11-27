use super::*;
#[cfg(test)]
use merman_model::families::flowchart::{FlowEdgeMarker, FlowEdgeStroke, FlowEdgeVisibility};
use rustc_hash::FxHashMap;

#[cfg(feature = "layout-shoal")]
mod canonical_line_hops;
mod cluster;
mod endpoint_clip;
#[cfg(feature = "layout-shoal")]
pub(super) mod label_seating;

#[cfg(feature = "layout-shoal")]
pub(super) use canonical_line_hops::apply_canonical_line_hops;

pub(super) use crate::cluster_shell::swimlane_terminal_geometry;
pub(super) use endpoint_clip::clip_swimlane_endpoints;

pub(super) use cluster::render_swimlane_cluster;

fn is_pool_cluster(ctx: &FlowchartRenderCtx<'_>, id: &str) -> bool {
    ctx.subgraphs_by_id.get(id).is_some_and(|group| {
        group
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get("laneRole"))
            .and_then(serde_json::Value::as_str)
            == Some("pool")
            && ctx
                .swimlane_lanes_by_id
                .values()
                .any(|child| child.parent_id.as_deref() == Some(id))
    })
}

fn uses_pool_shell(ctx: &FlowchartRenderCtx<'_>, id: &str) -> bool {
    is_pool_cluster(ctx, id) && !reuses_default_lane(ctx, id)
}

fn reuses_default_lane(ctx: &FlowchartRenderCtx<'_>, id: &str) -> bool {
    if id != "__swimlane_default__" {
        return false;
    }
    if ctx.layout.route_contract.is_canonical_axis() {
        // Preparation reuses the reserved group as a lane when it must adopt loose cards.
        // The solved parent map already contains those synthetic memberships.
        let authored: FxHashSet<_> = ctx.model.visible_memberships().map(|(id, _)| id).collect();
        ctx.swimlane_layout.is_some_and(|layout| {
            layout.nodes.iter().any(|node| {
                !node.is_edge_label
                    && node.parent_id.as_deref() == Some(id)
                    && !authored.contains(node.id.as_str())
            })
        })
    } else {
        ctx.layout
            .nodes
            .iter()
            .any(|node| !ctx.parent.contains_key(node.id.as_str()))
    }
}

pub(super) fn is_lane_or_pool_cluster(ctx: &FlowchartRenderCtx<'_>, id: &str) -> bool {
    ctx.swimlane_lanes_by_id.get(id).is_some_and(|group| {
        group.parent_id.is_none()
            || is_pool_cluster(ctx, id)
            || group
                .parent_id
                .as_deref()
                .is_some_and(|parent| is_pool_cluster(ctx, parent))
            || (ctx.layout.route_contract.is_canonical_axis() && reuses_default_lane(ctx, id))
    })
}

pub(super) fn apply_swimlane_edge_curves<'a>(
    render_edges: &mut [super::render_input::FlowchartRenderEdge<'a>],
    layout: SwimlaneSvgLayout<'_>,
) {
    let curve_by_owner: FxHashMap<crate::projection::FlowchartEdgeKey, &str> = layout
        .edges
        .iter()
        .zip(layout.edge_owners.iter())
        .map(|(edge, owner)| (owner, edge.curve.as_str()))
        .collect();
    for edge in render_edges {
        let Some(curve) = curve_by_owner.get(&edge.key).copied() else {
            continue;
        };
        edge.edge.to_mut().interpolate = Some(curve.to_string());
    }
}

pub(super) fn apply_line_hops_to_edge_geometries(
    edge_path_cache: &mut FxHashMap<
        crate::projection::FlowchartEdgeKey,
        FlowchartEdgePathCacheEntry,
    >,
    render_edges: &[super::render_input::FlowchartRenderEdge<'_>],
    effective_config: &merman_model::presentation::PresentationConfig,
    work_meter: &merman_render_core::resources::OperationWorkMeter,
    uses_elk_adapter_dom: bool,
    preserve_dash_mask_length: impl Fn(&super::render_input::FlowchartRenderEdge<'_>) -> bool,
) -> Result<()> {
    use merman_render_core::svg_edge_parity::line_hops::{
        LineHopEdge, curve_supports_line_hops, line_hop_overrides,
    };

    let Some(config) = line_hop_config(
        effective_config,
        if uses_elk_adapter_dom {
            "elk"
        } else {
            "swimlane"
        },
    ) else {
        return Ok(());
    };

    let preparation_work = render_edges
        .iter()
        .filter_map(|edge| {
            edge_path_cache
                .get(&edge.key)
                .map(|entry| entry.geom.data_points.len().saturating_add(1))
        })
        .fold(0usize, usize::saturating_add);
    work_meter.charge(preparation_work)?;

    let transport_plan = crate::projection::FlowchartEdgeTransportPlan::for_keyed_edges(
        render_edges
            .iter()
            .map(|edge| (edge.key, edge.edge.as_ref())),
    );
    let (owners, edges): (Vec<_>, Vec<_>) = render_edges
        .iter()
        .filter_map(|edge| {
            let semantic = edge.edge.as_ref();
            let cache_entry = edge_path_cache.get(&edge.key)?;
            let id = transport_plan.id(edge.key)?;
            let (arrow_type_start, arrow_type_end) =
                super::edge_geom::arrow_types_for_edge(semantic.edge_type.as_deref());
            Some((
                edge,
                LineHopEdge {
                    id,
                    points: &cache_entry.geom.data_points,
                    curve: if uses_elk_adapter_dom {
                        Some("rounded")
                    } else {
                        semantic.interpolate.as_deref()
                    },
                    arrow_type_start,
                    arrow_type_end,
                },
            ))
        })
        .unzip();

    let paths = line_hop_overrides(&edges, config, curve_supports_line_hops, work_meter)?;

    for (index, path) in paths {
        let edge = owners[index];
        let Some(cache_entry) = edge_path_cache.get_mut(&edge.key) else {
            continue;
        };
        if preserve_dash_mask_length(edge) {
            cache_entry.geom.pre_hop_dash_mask_length = cache_entry.geom.centerline_length();
        }
        cache_entry.geom.d = path;
        cache_entry.geom.neo_length = None;
        cache_entry.geom.pb = svg_path_bounds_from_d(&cache_entry.geom.d);
        cache_entry.geom.line_hop_applied = true;
    }
    Ok(())
}

fn line_hop_config(
    config: &merman_model::presentation::PresentationConfig,
    family: &str,
) -> Option<merman_render_core::svg_edge_parity::line_hops::LineHopConfig> {
    use merman_render_core::svg_edge_parity::line_hops::{LineHopConfig, LineHopStyle};

    let mode = config.get(family).and_then(|value| value.get("lineHops"));
    if mode.and_then(serde_json::Value::as_bool) == Some(false) {
        return None;
    }
    Some(LineHopConfig {
        jump_radius: 6.0,
        jump_style: if mode.and_then(serde_json::Value::as_str) == Some("gap") {
            LineHopStyle::Gap
        } else {
            LineHopStyle::Arc
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn presentation_config(
        value: serde_json::Value,
    ) -> merman_model::presentation::PresentationConfig {
        use merman_model::{
            MermaidConfig, OperationControl,
            resources::{InputResourcePolicy, ResourceProfile},
        };

        merman_model::presentation::PresentationConfig::from_config_controlled(
            MermaidConfig::from_value(value),
            &InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap()
    }
    use std::borrow::Cow;

    fn point(x: f64, y: f64) -> merman_render_core::LayoutPoint {
        merman_render_core::LayoutPoint { x, y }
    }

    fn assert_points_eq(
        actual: &[merman_render_core::LayoutPoint],
        expected: &[merman_render_core::LayoutPoint],
    ) {
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            assert!((actual.x - expected.x).abs() < 1.0e-9);
            assert!((actual.y - expected.y).abs() < 1.0e-9);
        }
    }

    fn semantic_edge(id: &str) -> merman_model::families::flowchart::FlowEdge {
        merman_model::families::flowchart::FlowEdge {
            id: id.to_string(),
            from: format!("{id}-from"),
            to: format!("{id}-to"),
            label: None,
            label_type: None,
            edge_type: Some("arrow_open".to_string()),
            arrow: String::new(),
            start_marker: FlowEdgeMarker::None,
            end_marker: FlowEdgeMarker::None,
            is_user_defined_id: false,
            stroke: Some("normal".to_string()),
            stroke_kind: FlowEdgeStroke::Normal,
            visibility: FlowEdgeVisibility::Visible,
            interpolate: Some("linear".to_string()),
            classes: Vec::new(),
            style: Vec::new(),
            animate: None,
            animation: None,
            step: None,
            length: 1,
        }
    }

    fn cache_entry(
        d: &str,
        points: Vec<merman_render_core::LayoutPoint>,
    ) -> FlowchartEdgePathCacheEntry {
        let label_path_points = points.clone();
        FlowchartEdgePathCacheEntry {
            origin_x: 0.0,
            origin_y: 0.0,
            abs_top_transform: 0.0,
            geom: FlowchartEdgePathGeom {
                pb: svg_path_bounds_from_d(d),
                d: d.to_string(),
                neo_length: None,
                data_points: points,
                pre_hop_dash_mask_length: None,
                line_hop_applied: false,
                original_label_path_points: label_path_points.clone(),
                label_path_points,
                label_anchor: None,
                label_path_was_explicitly_updated: false,
                emitted_d_for_label: None,
                bounds_skipped_for_viewbox: false,
            },
        }
    }

    #[test]
    fn line_hops_keep_duplicate_raw_ids_bound_to_their_occurrence_keys() {
        let mut vertical = semantic_edge("duplicate");
        vertical.from = "vertical-from".to_string();
        vertical.to = "vertical-to".to_string();
        let mut horizontal = semantic_edge("duplicate");
        horizontal.from = "horizontal-from".to_string();
        horizontal.to = "horizontal-to".to_string();
        let render_edges = vec![
            super::super::render_input::FlowchartRenderEdge {
                key: crate::projection::FlowchartEdgeKey::new(0),
                edge: std::borrow::Cow::Owned(vertical),
            },
            super::super::render_input::FlowchartRenderEdge {
                key: crate::projection::FlowchartEdgeKey::new(1),
                edge: std::borrow::Cow::Owned(horizontal),
            },
        ];
        let vertical_points = vec![point(0.0, -10.0), point(0.0, 10.0)];
        let horizontal_points = vec![point(-10.0, 0.0), point(10.0, 0.0)];
        let mut cache = FxHashMap::default();
        cache.insert(
            render_edges[0].key,
            cache_entry("M0,-10L0,10", vertical_points.clone()),
        );
        cache.insert(
            render_edges[1].key,
            cache_entry("M-10,0L10,0", horizontal_points.clone()),
        );
        let work_meter = merman_render_core::resources::OperationWorkMeter::new(
            merman_render_core::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );

        apply_line_hops_to_edge_geometries(
            &mut cache,
            &render_edges,
            &merman_model::presentation::PresentationConfig::default(),
            &work_meter,
            false,
            |_| false,
        )
        .expect("apply line hops");
        assert_eq!(work_meter.used(), 17);

        let vertical = &cache[&render_edges[0].key].geom;
        assert!(!vertical.line_hop_applied);
        assert_eq!(vertical.d, "M0,-10L0,10");
        assert_points_eq(&vertical.data_points, &vertical_points);
        assert_points_eq(&vertical.label_path_points, &vertical_points);

        let horizontal = &cache[&render_edges[1].key].geom;
        assert!(horizontal.line_hop_applied);
        assert!(horizontal.d.contains("A6,6 0 0 1"), "{}", horizontal.d);
        assert_points_eq(&horizontal.data_points, &horizontal_points);
        assert_points_eq(&horizontal.label_path_points, &horizontal_points);
        assert_eq!(horizontal.pre_hop_dash_mask_length, None);

        assert!(
            svg_path_length_from_d(&horizontal.d).unwrap() > 20.0,
            "a semicircle hop must lengthen the rendered path"
        );
        let bounds = horizontal.pb.expect("post-processed path bounds");
        assert!((bounds.min_y + 6.0).abs() < 1.0e-6, "{bounds:?}");
    }

    #[test]
    fn clamped_crossing_still_rewrites_cached_paths_and_styles() {
        for uses_elk_adapter_dom in [false, true] {
            for mode in [serde_json::json!(true), serde_json::json!("gap")] {
                let render_edges = ["vertical", "horizontal"]
                    .into_iter()
                    .enumerate()
                    .map(
                        |(index, id)| super::super::render_input::FlowchartRenderEdge {
                            key: crate::projection::FlowchartEdgeKey::new(index),
                            edge: Cow::Owned(semantic_edge(id)),
                        },
                    )
                    .collect::<Vec<_>>();
                let vertical_points = vec![point(0.0005, -1.0), point(0.0005, 1.0)];
                let horizontal_points = vec![point(0.0, 0.0), point(10.0, 0.0)];
                let mut cache = FxHashMap::default();
                cache.insert(
                    render_edges[0].key,
                    cache_entry("M0.0005,-1L0.0005,1", vertical_points.clone()),
                );
                cache.insert(
                    render_edges[1].key,
                    cache_entry("M0,0L10,0", horizontal_points.clone()),
                );
                let work_meter = merman_render_core::resources::OperationWorkMeter::new(
                    merman_render_core::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
                );
                let config = presentation_config(serde_json::json!({
                    "elk": { "lineHops": mode }, "swimlane": { "lineHops": mode }
                }));
                apply_line_hops_to_edge_geometries(
                    &mut cache,
                    &render_edges,
                    &config,
                    &work_meter,
                    uses_elk_adapter_dom,
                    |edge| edge.key == render_edges[1].key,
                )
                .unwrap();
                let vertical = &cache[&render_edges[0].key].geom;
                let horizontal = &cache[&render_edges[1].key].geom;
                assert!(!vertical.line_hop_applied);
                assert!(
                    horizontal.line_hop_applied,
                    "the crossing triggers after-paint processing even when the hop cannot fit"
                );
                assert_eq!(horizontal.d, "M0,0 L10,0");
                assert!(!horizontal.d.contains('A'));
                assert_eq!(horizontal.pre_hop_dash_mask_length, Some(10.0));
                assert_eq!(svg_path_length_from_d(&horizontal.d), Some(10.0));
                assert_points_eq(&horizontal.data_points, &horizontal_points);
                assert_points_eq(&horizontal.label_path_points, &horizontal_points);

                // This original dotted Neo mask has a 4px marker clearance, but the
                // upstream rewrite takes its fourth number (2), even without a visible hop.
                let original_style = "stroke-dasharray: 0 0 2 2 4; stroke-dashoffset: 0;";
                let rewritten =
                    merman_render_core::svg_edge_parity::line_hops::rewrite_style_after_line_hop(
                        original_style,
                        &horizontal.d,
                        &work_meter,
                    )
                    .unwrap();
                assert_eq!(
                    rewritten,
                    "stroke-dasharray: 0 0 8 2; stroke-dashoffset: 0;"
                );
            }
        }
    }

    #[test]
    fn disabled_line_hops_leave_cached_geometry_untouched() {
        let render_edges = vec![
            super::super::render_input::FlowchartRenderEdge {
                key: crate::projection::FlowchartEdgeKey::new(0),
                edge: std::borrow::Cow::Owned(semantic_edge("vertical")),
            },
            super::super::render_input::FlowchartRenderEdge {
                key: crate::projection::FlowchartEdgeKey::new(1),
                edge: std::borrow::Cow::Owned(semantic_edge("horizontal")),
            },
        ];
        let mut cache = FxHashMap::default();
        cache.insert(
            render_edges[0].key,
            cache_entry("M0,-10L0,10", vec![point(0.0, -10.0), point(0.0, 10.0)]),
        );
        cache.insert(
            render_edges[1].key,
            cache_entry("M-10,0L10,0", vec![point(-10.0, 0.0), point(10.0, 0.0)]),
        );
        let config = presentation_config(serde_json::json!({
            "swimlane": { "lineHops": false }
        }));
        let work_meter = merman_render_core::resources::OperationWorkMeter::new(
            merman_render_core::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );

        apply_line_hops_to_edge_geometries(
            &mut cache,
            &render_edges,
            &config,
            &work_meter,
            false,
            |_| false,
        )
        .expect("disabled line hops");

        assert_eq!(cache[&render_edges[1].key].geom.d, "M-10,0L10,0");
        assert!(!cache[&render_edges[1].key].geom.line_hop_applied);
        assert_eq!(work_meter.used(), 0);
    }
}
