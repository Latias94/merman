//! Shared Mermaid ELK configuration projection for graph families.
//!
//! Layer assignment keys follow Mermaid 12's `createRootElkGraph`:
//! https://github.com/mermaid-js/mermaid/blob/98a0945418c76238f15df2afaddbba4272656c3b/packages/mermaid/src/rendering-util/layout-algorithms/elk/render.ts

use crate::config::{config_bool, config_string};
use merman_layout_elk as elk;

pub(crate) fn layout_options(effective_config: &serde_json::Value) -> elk::LayoutOptions {
    let model_order = config_string(effective_config, &["elk", "considerModelOrder"])
        .map(
            |strategy| match strategy.trim().to_ascii_uppercase().as_str() {
                "NONE" => elk::ModelOrderStrategy::None,
                "PREFER_EDGES" => elk::ModelOrderStrategy::PreferEdges,
                "PREFER_NODES" => elk::ModelOrderStrategy::PreferNodes,
                _ => elk::ModelOrderStrategy::NodesAndEdges,
            },
        )
        .unwrap_or_default();
    let cycle_breaking = config_string(effective_config, &["elk", "cycleBreakingStrategy"])
        .map(
            |strategy| match strategy.trim().to_ascii_uppercase().as_str() {
                "DEPTH_FIRST" => elk::CycleBreakingStrategy::DepthFirst,
                "INTERACTIVE" => elk::CycleBreakingStrategy::Interactive,
                "MODEL_ORDER" => elk::CycleBreakingStrategy::ModelOrder,
                "GREEDY_MODEL_ORDER" => elk::CycleBreakingStrategy::GreedyModelOrder,
                _ => elk::CycleBreakingStrategy::Greedy,
            },
        )
        .unwrap_or_default();
    let node_placement = config_string(effective_config, &["elk", "nodePlacementStrategy"])
        .map(
            |strategy| match strategy.trim().to_ascii_uppercase().as_str() {
                "SIMPLE" => elk::NodePlacementStrategy::Simple,
                "NETWORK_SIMPLEX" => elk::NodePlacementStrategy::NetworkSimplex,
                "LINEAR_SEGMENTS" => elk::NodePlacementStrategy::LinearSegments,
                _ => elk::NodePlacementStrategy::BrandesKoepf,
            },
        )
        .unwrap_or_default();
    let node_placement_alignment =
        config_string(effective_config, &["elk", "nodePlacementAlignment"])
            .map(
                |alignment| match alignment.trim().to_ascii_uppercase().as_str() {
                    "LEFTUP" => elk::NodePlacementAlignment::LeftUp,
                    "LEFTDOWN" => elk::NodePlacementAlignment::LeftDown,
                    "RIGHTUP" => elk::NodePlacementAlignment::RightUp,
                    "RIGHTDOWN" => elk::NodePlacementAlignment::RightDown,
                    "BALANCED" => elk::NodePlacementAlignment::Balanced,
                    _ => elk::NodePlacementAlignment::None,
                },
            )
            .unwrap_or_default();
    let self_loop_ordering = config_string(
        effective_config,
        &["elk", "layered", "edgeRouting", "selfLoopOrdering"],
    )
    .map(
        |strategy| match strategy.trim().to_ascii_uppercase().as_str() {
            "REVERSE_STACKED" => elk::SelfLoopOrderingStrategy::ReverseStacked,
            "SEQUENCED" => elk::SelfLoopOrderingStrategy::Sequenced,
            _ => elk::SelfLoopOrderingStrategy::Stacked,
        },
    )
    .unwrap_or_default();

    let layering = match config_string(effective_config, &["elk", "layeringStrategy"]).as_deref() {
        Some("LONGEST_PATH") => elk::LayeringStrategy::LongestPath,
        Some("LONGEST_PATH_SOURCE") => elk::LayeringStrategy::LongestPathSource,
        Some("COFFMAN_GRAHAM") => elk::LayeringStrategy::CoffmanGraham,
        Some("MIN_WIDTH") => elk::LayeringStrategy::MinWidth,
        Some("STRETCH_WIDTH") => elk::LayeringStrategy::StretchWidth,
        Some("INTERACTIVE") => elk::LayeringStrategy::Interactive,
        _ => elk::LayeringStrategy::NetworkSimplex,
    };
    let layering_layer_bound = effective_config
        .pointer("/elk/layeringLayerBound")
        .and_then(serde_json::Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
        .unwrap_or(4);

    elk::LayoutOptions {
        layered: elk::LayeredOptions {
            merge_edges: config_bool(effective_config, &["elk", "mergeEdges"]).unwrap_or(false),
            merge_hierarchy_edges: true,
            unnecessary_bendpoints: true,
            inside_self_loops_activate: config_bool(
                effective_config,
                &["elk", "insideSelfLoops", "activate"],
            )
            .unwrap_or(false),
            self_loop_distribution: elk::SelfLoopDistributionStrategy::Equally,
            self_loop_ordering,
            force_node_model_order: config_bool(effective_config, &["elk", "forceNodeModelOrder"])
                .unwrap_or(false),
            consider_model_order: model_order != elk::ModelOrderStrategy::None,
            model_order,
            cycle_breaking,
            layering,
            layering_layer_bound,
            node_placement,
            node_placement_alignment,
            ..Default::default()
        },
    }
}
