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
pub enum DirectionCongruency {
    #[default]
    ReadingDirection,
    Rotation,
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
pub enum PortSortingStrategy {
    #[default]
    InputOrder,
    PortDegree,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LongEdgeOrderingStrategy {
    #[default]
    DummyNodeOver,
    DummyNodeUnder,
    Equal,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PortConstraints {
    #[default]
    Undefined,
    Free,
    FixedSide,
    FixedOrder,
    FixedRatio,
    FixedPos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PortAlignment {
    #[default]
    Distributed,
    Justified,
    Begin,
    Center,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayerConstraint {
    #[default]
    None,
    First,
    FirstSeparate,
    Last,
    LastSeparate,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayeredOptions {
    pub direction: ElkDirection,
    pub direction_congruency: DirectionCongruency,
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
    pub consider_model_order_long_edge_strategy: LongEdgeOrderingStrategy,
    pub consider_model_order_port_model_order: bool,
    pub force_node_model_order: bool,
    pub port_sorting_strategy: PortSortingStrategy,
    pub port_alignment_default: PortAlignment,
    pub merge_edges: bool,
    pub merge_hierarchy_edges: bool,
    pub unnecessary_bendpoints: bool,
    pub self_loop_distribution: SelfLoopDistributionStrategy,
    pub wrapping_strategy: WrappingStrategy,
    pub wrapping_multi_edge_improve_cuts: bool,
    pub wrapping_multi_edge_improve_wrapped_edges: bool,
    pub feedback_edges: bool,
    pub random_seed: i32,
    pub thoroughness: usize,
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
    pub label_label: f64,
    pub label_node: f64,
    pub label_port_horizontal: f64,
    pub label_port_vertical: f64,
    pub port_port: f64,
    pub ports_surrounding: SpacingMargin,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SpacingMargin {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl Default for SpacingOptions {
    fn default() -> Self {
        Self {
            edge_edge: 10.0,
            label_label: 0.0,
            label_node: 5.0,
            label_port_horizontal: 1.0,
            label_port_vertical: 1.0,
            port_port: 10.0,
            ports_surrounding: SpacingMargin::default(),
        }
    }
}

impl Default for LayeredOptions {
    fn default() -> Self {
        Self {
            direction: ElkDirection::Undefined,
            direction_congruency: DirectionCongruency::ReadingDirection,
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
            consider_model_order_long_edge_strategy: LongEdgeOrderingStrategy::DummyNodeOver,
            consider_model_order_port_model_order: false,
            force_node_model_order: false,
            port_sorting_strategy: PortSortingStrategy::InputOrder,
            port_alignment_default: PortAlignment::Distributed,
            merge_edges: false,
            merge_hierarchy_edges: true,
            unnecessary_bendpoints: false,
            self_loop_distribution: SelfLoopDistributionStrategy::North,
            wrapping_strategy: WrappingStrategy::Off,
            wrapping_multi_edge_improve_cuts: true,
            wrapping_multi_edge_improve_wrapped_edges: true,
            feedback_edges: false,
            random_seed: 1,
            thoroughness: 7,
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

impl LongEdgeOrderingStrategy {
    pub fn return_value(self) -> i64 {
        match self {
            Self::DummyNodeOver => i64::MAX,
            Self::DummyNodeUnder => -1,
            Self::Equal => 0,
        }
    }
}

impl PortConstraints {
    pub fn is_pos_fixed(self) -> bool {
        self == Self::FixedPos
    }

    pub fn is_ratio_fixed(self) -> bool {
        self == Self::FixedRatio
    }

    pub fn is_order_fixed(self) -> bool {
        matches!(self, Self::FixedOrder | Self::FixedRatio | Self::FixedPos)
    }

    pub fn is_side_fixed(self) -> bool {
        !matches!(self, Self::Free | Self::Undefined)
    }
}
