//! Shared Mermaid ELK configuration projection for graph families.
//!
//! Layer assignment keys follow Mermaid 12's `createRootElkGraph`:
//! https://github.com/mermaid-js/mermaid/blob/98a0945418c76238f15df2afaddbba4272656c3b/packages/mermaid/src/rendering-util/layout-algorithms/elk/render.ts

use crate::config::{config_bool, config_string};
use merman_layout_elk as elk;

pub(crate) fn layout_options(effective_config: &serde_json::Value) -> elk::LayoutOptions {
    let (preset_placement, preset_alignment, preset_cycle_breaking) =
        match config_string(effective_config, &["elk", "preset"]).as_deref() {
            Some("legacy") => (
                elk::NodePlacementStrategy::BrandesKoepf,
                elk::NodePlacementAlignment::None,
                elk::CycleBreakingStrategy::Greedy,
            ),
            Some("modelOrder") => (
                elk::NodePlacementStrategy::NetworkSimplex,
                elk::NodePlacementAlignment::None,
                elk::CycleBreakingStrategy::GreedyModelOrder,
            ),
            Some("depthFirst") => (
                elk::NodePlacementStrategy::NetworkSimplex,
                elk::NodePlacementAlignment::None,
                elk::CycleBreakingStrategy::DepthFirst,
            ),
            _ => (
                elk::NodePlacementStrategy::BrandesKoepf,
                elk::NodePlacementAlignment::Balanced,
                elk::CycleBreakingStrategy::DepthFirst,
            ),
        };
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
        .unwrap_or(preset_cycle_breaking);
    let explicit_node_placement =
        config_string(effective_config, &["elk", "nodePlacementStrategy"]).map(|strategy| {
            match strategy.trim().to_ascii_uppercase().as_str() {
                "SIMPLE" => elk::NodePlacementStrategy::Simple,
                "NETWORK_SIMPLEX" => elk::NodePlacementStrategy::NetworkSimplex,
                "LINEAR_SEGMENTS" => elk::NodePlacementStrategy::LinearSegments,
                _ => elk::NodePlacementStrategy::BrandesKoepf,
            }
        });
    let node_placement = explicit_node_placement.unwrap_or(preset_placement);
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
            .unwrap_or(preset_alignment);
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
        algorithm: elk::Algorithm::Layered,
        container: elk::ContainerOptions {
            cycle_breaking,
            node_placement: explicit_node_placement
                .unwrap_or(elk::NodePlacementStrategy::BrandesKoepf),
            node_placement_alignment,
        },
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn elk_layout_options_resolve_presets_for_root_and_container() {
        use elk::NodePlacementStrategy as Placement;
        use elk::{CycleBreakingStrategy as Cycle, NodePlacementAlignment as Align};

        for (preset, placement, alignment, cycle) in [
            (
                "default",
                Placement::BrandesKoepf,
                Align::Balanced,
                Cycle::DepthFirst,
            ),
            (
                "legacy",
                Placement::BrandesKoepf,
                Align::None,
                Cycle::Greedy,
            ),
            (
                "modelOrder",
                Placement::NetworkSimplex,
                Align::None,
                Cycle::GreedyModelOrder,
            ),
            (
                "depthFirst",
                Placement::NetworkSimplex,
                Align::None,
                Cycle::DepthFirst,
            ),
            (
                "unknown",
                Placement::BrandesKoepf,
                Align::Balanced,
                Cycle::DepthFirst,
            ),
        ] {
            let options = layout_options(&json!({"elk": {"preset": preset}}));
            assert_eq!(options.layered.node_placement, placement, "{preset}");
            assert_eq!(
                options.container.node_placement,
                Placement::BrandesKoepf,
                "{preset}"
            );
            assert_eq!(
                options.layered.node_placement_alignment, alignment,
                "{preset}"
            );
            assert_eq!(
                options.container.node_placement_alignment, alignment,
                "{preset}"
            );
            assert_eq!(options.layered.cycle_breaking, cycle, "{preset}");
            assert_eq!(options.container.cycle_breaking, cycle, "{preset}");
        }
    }

    #[test]
    fn elk_layout_options_explicit_values_override_both_preset_scopes() {
        let options = layout_options(&json!({"elk": {
            "preset": "modelOrder",
            "nodePlacementStrategy": "SIMPLE",
            "nodePlacementAlignment": "RIGHTDOWN",
            "cycleBreakingStrategy": "INTERACTIVE"
        }}));
        assert_eq!(
            options.layered.node_placement,
            elk::NodePlacementStrategy::Simple
        );
        assert_eq!(
            options.container.node_placement,
            elk::NodePlacementStrategy::Simple
        );
        assert_eq!(
            options.layered.node_placement_alignment,
            elk::NodePlacementAlignment::RightDown
        );
        assert_eq!(
            options.container.node_placement_alignment,
            elk::NodePlacementAlignment::RightDown
        );
        assert_eq!(
            options.layered.cycle_breaking,
            elk::CycleBreakingStrategy::Interactive
        );
        assert_eq!(
            options.container.cycle_breaking,
            elk::CycleBreakingStrategy::Interactive
        );
    }
}
