//! ELK layered option model.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/Layered.melk
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.core/src/org/eclipse/elk/core/Core.melk
//! - https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid-layout-elk/src/render.ts

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ElkDirection {
    Left,
    #[default]
    Right,
    Up,
    Down,
    Undefined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HierarchyHandling {
    Inherit,
    IncludeChildren,
    #[default]
    SeparateChildren,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EdgeRouting {
    Polyline,
    #[default]
    Orthogonal,
    Splines,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CycleBreakingStrategy {
    #[default]
    Greedy,
    DepthFirst,
    Interactive,
    ModelOrder,
    GreedyModelOrder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayeringStrategy {
    #[default]
    NetworkSimplex,
    LongestPath,
    LongestPathSource,
    CoffmanGraham,
    Interactive,
    StretchWidth,
    MinWidth,
    BreadthFirstModelOrder,
    DepthFirstModelOrder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CrossingMinimizationStrategy {
    #[default]
    LayerSweep,
    Interactive,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NodePlacementStrategy {
    Simple,
    Interactive,
    LinearSegments,
    #[default]
    BrandesKoepf,
    NetworkSimplex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GreedySwitchType {
    Off,
    OneSided,
    #[default]
    TwoSided,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OrderingStrategy {
    #[default]
    None,
    NodesAndEdges,
    PreferEdges,
    PreferNodes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WrappingStrategy {
    #[default]
    Off,
    SingleEdge,
    MultiEdge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelfLoopDistributionStrategy {
    #[default]
    North,
    Equally,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayeredOptions {
    pub direction: ElkDirection,
    pub hierarchy_handling: HierarchyHandling,
    pub edge_routing: EdgeRouting,
    pub spacing: SpacingOptions,
    pub cycle_breaking_strategy: CycleBreakingStrategy,
    pub layering_strategy: LayeringStrategy,
    pub crossing_minimization_strategy: CrossingMinimizationStrategy,
    pub node_placement_strategy: NodePlacementStrategy,
    pub greedy_switch_type: GreedySwitchType,
    pub greedy_switch_hierarchical_type: GreedySwitchType,
    pub greedy_switch_activation_threshold: usize,
    pub consider_model_order_strategy: OrderingStrategy,
    pub force_node_model_order: bool,
    pub merge_edges: bool,
    pub merge_hierarchy_edges: bool,
    pub unnecessary_bendpoints: bool,
    pub self_loop_distribution: SelfLoopDistributionStrategy,
    pub wrapping_strategy: WrappingStrategy,
    pub wrapping_multi_edge_improve_cuts: bool,
    pub wrapping_multi_edge_improve_wrapped_edges: bool,
    pub feedback_edges: bool,
    pub random_seed: i32,
    pub node_placement_favor_straight_edges: Option<bool>,
    pub graph_has_self_loops: bool,
    pub graph_has_center_labels: bool,
    pub graph_has_end_labels: bool,
    pub graph_has_non_free_ports: bool,
    pub graph_has_north_south_ports: bool,
    pub graph_has_hyperedges: bool,
    pub graph_has_external_ports: bool,
    pub graph_has_hypernodes: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpacingOptions {
    pub edge_edge: f64,
}

impl Default for SpacingOptions {
    fn default() -> Self {
        Self { edge_edge: 2.0 }
    }
}

impl Default for LayeredOptions {
    fn default() -> Self {
        Self {
            direction: ElkDirection::Undefined,
            hierarchy_handling: HierarchyHandling::SeparateChildren,
            edge_routing: EdgeRouting::Orthogonal,
            spacing: SpacingOptions::default(),
            cycle_breaking_strategy: CycleBreakingStrategy::Greedy,
            layering_strategy: LayeringStrategy::NetworkSimplex,
            crossing_minimization_strategy: CrossingMinimizationStrategy::LayerSweep,
            node_placement_strategy: NodePlacementStrategy::BrandesKoepf,
            greedy_switch_type: GreedySwitchType::TwoSided,
            greedy_switch_hierarchical_type: GreedySwitchType::Off,
            greedy_switch_activation_threshold: 40,
            consider_model_order_strategy: OrderingStrategy::None,
            force_node_model_order: false,
            merge_edges: false,
            merge_hierarchy_edges: true,
            unnecessary_bendpoints: false,
            self_loop_distribution: SelfLoopDistributionStrategy::North,
            wrapping_strategy: WrappingStrategy::Off,
            wrapping_multi_edge_improve_cuts: true,
            wrapping_multi_edge_improve_wrapped_edges: true,
            feedback_edges: false,
            random_seed: 1,
            node_placement_favor_straight_edges: None,
            graph_has_self_loops: false,
            graph_has_center_labels: false,
            graph_has_end_labels: false,
            graph_has_non_free_ports: false,
            graph_has_north_south_ports: false,
            graph_has_hyperedges: false,
            graph_has_external_ports: false,
            graph_has_hypernodes: false,
        }
    }
}

impl LayeredOptions {
    /// Options set by Mermaid's ELK adapter before calling `elk.layout(...)`.
    ///
    /// Source:
    /// https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid-layout-elk/src/render.ts
    pub fn mermaid_flowchart_defaults(direction: ElkDirection) -> Self {
        Self {
            direction,
            hierarchy_handling: HierarchyHandling::IncludeChildren,
            node_placement_strategy: NodePlacementStrategy::BrandesKoepf,
            consider_model_order_strategy: OrderingStrategy::NodesAndEdges,
            unnecessary_bendpoints: true,
            self_loop_distribution: SelfLoopDistributionStrategy::Equally,
            wrapping_multi_edge_improve_cuts: true,
            wrapping_multi_edge_improve_wrapped_edges: true,
            merge_hierarchy_edges: true,
            ..Self::default()
        }
    }

    pub fn is_hierarchical_layout(&self) -> bool {
        self.hierarchy_handling == HierarchyHandling::IncludeChildren
    }
}
