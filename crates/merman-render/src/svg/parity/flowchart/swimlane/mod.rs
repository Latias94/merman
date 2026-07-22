use super::*;
use crate::model::{
    FlowchartLayout, LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, SwimlaneLayout,
};
use rustc_hash::FxHashMap;
use std::borrow::Cow;

mod cluster;
pub(super) mod line_hops;

pub(super) use cluster::render_swimlane_cluster;

pub(in crate::svg::parity) fn render_swimlane_svg_model_with_config(
    layout: &SwimlaneLayout,
    model: &crate::flowchart::FlowchartModel,
    effective_config: &merman_core::MermaidConfig,
    diagram_type: &str,
    diagram_title: Option<&str>,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let flowchart_layout = adapt_swimlane_layout(model, layout);
    super::svg_emit::render_flowchart_svg_model_with_swimlane(
        &flowchart_layout,
        layout,
        model,
        effective_config,
        diagram_type,
        diagram_title,
        options,
    )
}

fn adapt_swimlane_layout(
    model: &crate::flowchart::FlowchartModel,
    layout: &SwimlaneLayout,
) -> FlowchartLayout {
    let label_nodes: FxHashMap<&str, &crate::model::SwimlaneNodeLayout> = layout
        .nodes
        .iter()
        .filter(|node| node.is_edge_label)
        .map(|node| (node.id.as_str(), node))
        .collect();

    let nodes = layout
        .nodes
        .iter()
        .filter(|node| !node.is_edge_label)
        .map(|node| LayoutNode {
            id: node.id.clone(),
            x: node.x,
            y: node.y,
            width: node.width,
            height: node.height,
            is_cluster: false,
            label_width: Some(node.label_width),
            label_height: Some(node.label_height),
        })
        .collect();

    let edges = layout
        .edges
        .iter()
        .map(|edge| {
            let label = edge
                .label_node_id
                .as_deref()
                .and_then(|id| label_nodes.get(id).copied())
                .map(|node| LayoutLabel {
                    x: node.x,
                    y: node.y,
                    width: node.label_width,
                    height: node.label_height,
                });
            LayoutEdge {
                id: edge.id.clone(),
                from: edge.from.clone(),
                to: edge.to.clone(),
                from_cluster: None,
                to_cluster: None,
                points: edge.points.clone(),
                label,
                start_label_left: None,
                start_label_right: None,
                end_label_left: None,
                end_label_right: None,
                start_marker: None,
                end_marker: None,
                stroke_dasharray: None,
            }
        })
        .collect();

    let clusters = layout
        .lanes
        .iter()
        .map(|lane| LayoutCluster {
            id: lane.id.clone(),
            x: lane.x,
            y: lane.y,
            width: lane.width,
            height: lane.height,
            diff: 0.0,
            offset_y: 0.0,
            title: lane.title.clone(),
            title_label: LayoutLabel {
                x: lane.x,
                y: lane.y,
                width: 0.0,
                height: 0.0,
            },
            requested_dir: lane.requested_dir.clone(),
            effective_dir: layout.direction.as_str().to_string(),
            padding: lane.padding,
            title_margin_top: 0.0,
            title_margin_bottom: 0.0,
        })
        .collect();

    let mut top_level_order: Vec<String> = model.nodes.iter().map(|node| node.id.clone()).collect();
    top_level_order.extend(
        layout
            .nodes
            .iter()
            .filter(|node| node.is_edge_label)
            .map(|node| node.id.clone()),
    );
    let dom_node_order_by_root =
        std::collections::HashMap::from([(String::new(), top_level_order)]);

    FlowchartLayout {
        nodes,
        edges,
        clusters,
        bounds: layout.bounds.clone(),
        dom_node_order_by_root,
        uses_elk_adapter_dom: false,
    }
}

pub(super) fn apply_swimlane_edge_curves<'a>(
    render_edges: &mut [Cow<'a, crate::flowchart::FlowEdge>],
    layout: &SwimlaneLayout,
) {
    let curve_by_id: FxHashMap<&str, &str> = layout
        .edges
        .iter()
        .map(|edge| (edge.id.as_str(), edge.curve.as_str()))
        .collect();
    for edge in render_edges {
        let Some(curve) = curve_by_id.get(edge.as_ref().id.as_str()).copied() else {
            continue;
        };
        edge.to_mut().interpolate = Some(curve.to_string());
    }
}

pub(super) fn apply_line_hops_to_edge_geometries(
    edge_path_cache: &mut FxHashMap<&str, FlowchartEdgePathCacheEntry>,
    render_edges: &[Cow<'_, crate::flowchart::FlowEdge>],
    effective_config: &merman_core::MermaidConfig,
    resource_limits: crate::resources::RenderResourcePolicy,
) -> Result<()> {
    use line_hops::{LineHopConfig, LineHopEdge, LineHopStyle};

    let line_hops_value = effective_config
        .as_value()
        .get("swimlane")
        .and_then(|value| value.get("lineHops"));
    if line_hops_value.and_then(serde_json::Value::as_bool) == Some(false) {
        return Ok(());
    }
    let jump_style = if line_hops_value.and_then(serde_json::Value::as_str) == Some("gap") {
        LineHopStyle::Gap
    } else {
        LineHopStyle::Arc
    };

    struct OwnedLineHopEdge<'a> {
        semantic: &'a crate::flowchart::FlowEdge,
        points: Vec<crate::model::LayoutPoint>,
        arrow_type_start: Option<&'static str>,
        arrow_type_end: Option<&'static str>,
    }

    let owned_edges: Vec<_> = render_edges
        .iter()
        .filter_map(|edge| {
            let semantic = edge.as_ref();
            let cache_entry = edge_path_cache.get(semantic.id.as_str())?;
            let (arrow_type_start, arrow_type_end) =
                super::edge_geom::arrow_types_for_edge(semantic.edge_type.as_deref());
            Some(OwnedLineHopEdge {
                semantic,
                points: cache_entry.geom.data_points.clone(),
                arrow_type_start,
                arrow_type_end,
            })
        })
        .collect();
    let edges: Vec<_> = owned_edges
        .iter()
        .map(|edge| LineHopEdge {
            id: edge.semantic.id.as_str(),
            points: &edge.points,
            curve: edge.semantic.interpolate.as_deref(),
            arrow_type_start: edge.arrow_type_start,
            arrow_type_end: edge.arrow_type_end,
        })
        .collect();

    let paths = line_hops::process_edges_with_line_hops(
        &edges,
        LineHopConfig {
            enabled: true,
            jump_radius: 6.0,
            jump_style,
        },
        resource_limits,
    )?;

    for path in paths {
        if !path.has_hops
            || !edges
                .iter()
                .find(|edge| edge.id == path.edge_id)
                .is_some_and(|edge| line_hops::curve_supports_line_hops(edge.curve))
        {
            continue;
        }
        let Some(cache_entry) = edge_path_cache.get_mut(path.edge_id) else {
            continue;
        };
        cache_entry.geom.d = path.path;
        cache_entry.geom.pb = svg_path_bounds_from_d(&cache_entry.geom.d);
        cache_entry.geom.path_length = svg_path_length_from_d(&cache_entry.geom.d);
        cache_entry.geom.line_hop_applied = true;
    }
    Ok(())
}

pub(super) fn swimlane_css(
    diagram_id: &str,
    effective_config: &merman_core::MermaidConfig,
) -> String {
    let theme = PresentationTheme::new(effective_config.as_value()).node_diagram();
    format!(
        r#"#{id} .swimlane.cluster rect{{stroke:{border}!important;}}#{id} [data-look="neo"].cluster rect{{filter:none;}}"#,
        id = diagram_id,
        border = theme.cluster_border,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f64, y: f64) -> crate::model::LayoutPoint {
        crate::model::LayoutPoint { x, y }
    }

    fn assert_points_eq(
        actual: &[crate::model::LayoutPoint],
        expected: &[crate::model::LayoutPoint],
    ) {
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            assert!((actual.x - expected.x).abs() < 1.0e-9);
            assert!((actual.y - expected.y).abs() < 1.0e-9);
        }
    }

    fn semantic_edge(id: &str) -> crate::flowchart::FlowEdge {
        crate::flowchart::FlowEdge {
            id: id.to_string(),
            from: format!("{id}-from"),
            to: format!("{id}-to"),
            label: None,
            label_type: None,
            edge_type: Some("arrow_open".to_string()),
            arrow: String::new(),
            is_user_defined_id: false,
            stroke: Some("normal".to_string()),
            interpolate: Some("linear".to_string()),
            classes: Vec::new(),
            style: Vec::new(),
            animate: None,
            animation: None,
            length: 1,
        }
    }

    fn cache_entry(
        d: &str,
        data_points_b64: &str,
        points: Vec<crate::model::LayoutPoint>,
    ) -> FlowchartEdgePathCacheEntry {
        let path_length = svg_path_length_from_d(d);
        FlowchartEdgePathCacheEntry {
            origin_x: 0.0,
            origin_y: 0.0,
            abs_top_transform: 0.0,
            geom: FlowchartEdgePathGeom {
                pb: svg_path_bounds_from_d(d),
                d: d.to_string(),
                data_points: points,
                data_points_b64: data_points_b64.to_string(),
                original_path_length: path_length,
                path_length,
                line_hop_applied: false,
                label_position: None,
                bounds_skipped_for_viewbox: false,
            },
        }
    }

    #[test]
    fn line_hops_post_process_final_cached_geometry_without_changing_data_points() {
        let render_edges: Vec<Cow<'static, crate::flowchart::FlowEdge>> = vec![
            Cow::Owned(semantic_edge("vertical")),
            Cow::Owned(semantic_edge("horizontal")),
        ];
        let vertical_points = vec![point(0.0, -10.0), point(0.0, 10.0)];
        let horizontal_points = vec![point(-10.0, 0.0), point(10.0, 0.0)];
        let mut cache = FxHashMap::default();
        cache.insert(
            render_edges[0].id.as_str(),
            cache_entry("M0,-10L0,10", "vertical-points", vertical_points.clone()),
        );
        cache.insert(
            render_edges[1].id.as_str(),
            cache_entry(
                "M-10,0L10,0",
                "horizontal-points",
                horizontal_points.clone(),
            ),
        );

        apply_line_hops_to_edge_geometries(
            &mut cache,
            &render_edges,
            &merman_core::MermaidConfig::default(),
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .expect("apply line hops");

        let vertical = &cache["vertical"].geom;
        assert!(!vertical.line_hop_applied);
        assert_eq!(vertical.d, "M0,-10L0,10");
        assert_points_eq(&vertical.data_points, &vertical_points);
        assert_eq!(vertical.data_points_b64, "vertical-points");

        let horizontal = &cache["horizontal"].geom;
        assert!(horizontal.line_hop_applied);
        assert!(horizontal.d.contains("A6,6 0 0 1"), "{}", horizontal.d);
        assert_points_eq(&horizontal.data_points, &horizontal_points);
        assert_eq!(horizontal.data_points_b64, "horizontal-points");
        assert!(
            horizontal.path_length.expect("post-processed path length")
                > horizontal
                    .original_path_length
                    .expect("original path length"),
            "a semicircle hop must lengthen the rendered path"
        );
        let bounds = horizontal.pb.expect("post-processed path bounds");
        assert!((bounds.min_y + 6.0).abs() < 1.0e-6, "{bounds:?}");
    }

    #[test]
    fn disabled_line_hops_leave_cached_geometry_untouched() {
        let render_edges: Vec<Cow<'static, crate::flowchart::FlowEdge>> = vec![
            Cow::Owned(semantic_edge("vertical")),
            Cow::Owned(semantic_edge("horizontal")),
        ];
        let mut cache = FxHashMap::default();
        cache.insert(
            render_edges[0].id.as_str(),
            cache_entry(
                "M0,-10L0,10",
                "vertical-points",
                vec![point(0.0, -10.0), point(0.0, 10.0)],
            ),
        );
        cache.insert(
            render_edges[1].id.as_str(),
            cache_entry(
                "M-10,0L10,0",
                "horizontal-points",
                vec![point(-10.0, 0.0), point(10.0, 0.0)],
            ),
        );
        let config = merman_core::MermaidConfig::from_value(serde_json::json!({
            "swimlane": { "lineHops": false }
        }));

        apply_line_hops_to_edge_geometries(
            &mut cache,
            &render_edges,
            &config,
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .expect("disabled line hops");

        assert_eq!(cache["horizontal"].geom.d, "M-10,0L10,0");
        assert!(!cache["horizontal"].geom.line_hop_applied);
    }
}
