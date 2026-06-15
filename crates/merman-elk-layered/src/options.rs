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

impl ElkDirection {
    pub fn is_vertical(self) -> bool {
        matches!(self, Self::Up | Self::Down)
    }
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
pub enum FixedAlignment {
    #[default]
    None,
    LeftUp,
    RightUp,
    LeftDown,
    RightDown,
    Balanced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Alignment {
    #[default]
    Automatic,
    Left,
    Right,
    Top,
    Bottom,
    Center,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EdgeStraighteningStrategy {
    None,
    #[default]
    ImproveStraightness,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EdgeLabelSideSelection {
    AlwaysUp,
    AlwaysDown,
    DirectionUp,
    DirectionDown,
    SmartUp,
    #[default]
    SmartDown,
}

impl EdgeLabelSideSelection {
    pub fn transpose(self) -> Self {
        match self {
            Self::AlwaysUp => Self::AlwaysDown,
            Self::AlwaysDown => Self::AlwaysUp,
            Self::DirectionUp => Self::DirectionDown,
            Self::DirectionDown => Self::DirectionUp,
            Self::SmartUp => Self::SmartDown,
            Self::SmartDown => Self::SmartUp,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayeredOptions {
    pub direction: ElkDirection,
    pub direction_congruency: DirectionCongruency,
    pub hierarchy_handling: HierarchyHandling,
    pub edge_routing: EdgeRouting,
    pub padding: ElkPadding,
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
    pub port_constraints: PortConstraints,
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
    pub node_placement_bk_fixed_alignment: FixedAlignment,
    pub node_placement_bk_edge_straightening: EdgeStraighteningStrategy,
    pub graph_has_self_loops: bool,
    pub graph_has_center_labels: bool,
    pub graph_has_end_labels: bool,
    pub edge_label_side_selection: EdgeLabelSideSelection,
    pub graph_has_non_free_ports: bool,
    pub graph_has_north_south_ports: bool,
    pub graph_has_hyperedges: bool,
    pub graph_has_external_ports: bool,
    pub graph_has_hypernodes: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ElkPadding {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl ElkPadding {
    pub fn uniform(value: f64) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }
}

impl Default for ElkPadding {
    fn default() -> Self {
        Self::uniform(12.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpacingOptions {
    pub node_node: f64,
    pub edge_edge: f64,
    pub edge_node: f64,
    pub edge_label: f64,
    pub label_label: f64,
    pub label_node: f64,
    pub label_port_horizontal: f64,
    pub label_port_vertical: f64,
    pub port_port: f64,
    pub edge_edge_between_layers: f64,
    pub edge_node_between_layers: f64,
    pub node_node_between_layers: f64,
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
            node_node: 20.0,
            edge_edge: 10.0,
            edge_node: 10.0,
            edge_label: 2.0,
            label_label: 0.0,
            label_node: 5.0,
            label_port_horizontal: 1.0,
            label_port_vertical: 1.0,
            port_port: 10.0,
            edge_edge_between_layers: 10.0,
            edge_node_between_layers: 10.0,
            node_node_between_layers: 20.0,
            ports_surrounding: SpacingMargin::default(),
        }
    }
}

impl SpacingOptions {
    /// Applies Eclipse ELK's `LayeredSpacings.withBaseValue(...)` default factors.
    ///
    /// Source:
    /// https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/options/LayeredSpacings.java
    pub fn layered_base_value(base_value: f64) -> Self {
        let defaults = Self::default();
        let scale = base_value / defaults.node_node;
        Self {
            node_node: base_value,
            edge_edge: defaults.edge_edge * scale,
            edge_node: defaults.edge_node * scale,
            edge_label: defaults.edge_label * scale,
            label_label: defaults.label_label * scale,
            label_node: defaults.label_node * scale,
            label_port_horizontal: defaults.label_port_horizontal * scale,
            label_port_vertical: defaults.label_port_vertical * scale,
            port_port: defaults.port_port * scale,
            edge_edge_between_layers: defaults.edge_edge_between_layers * scale,
            edge_node_between_layers: defaults.edge_node_between_layers * scale,
            node_node_between_layers: defaults.node_node_between_layers * scale,
            ports_surrounding: defaults.ports_surrounding,
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
            padding: ElkPadding::default(),
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
            port_constraints: PortConstraints::Undefined,
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
            node_placement_bk_fixed_alignment: FixedAlignment::None,
            node_placement_bk_edge_straightening: EdgeStraighteningStrategy::ImproveStraightness,
            graph_has_self_loops: false,
            graph_has_center_labels: false,
            graph_has_end_labels: false,
            edge_label_side_selection: EdgeLabelSideSelection::SmartDown,
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
            spacing: SpacingOptions::layered_base_value(40.0),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mermaid_flowchart_defaults_apply_layered_padding_default() {
        let options = LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down);

        assert_eq!(options.padding, ElkPadding::uniform(12.0));
    }

    #[test]
    fn mermaid_flowchart_defaults_apply_layered_spacing_base_value() {
        let options = LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down);

        assert_eq!(options.spacing.node_node, 40.0);
        assert_eq!(options.spacing.edge_edge, 20.0);
        assert_eq!(options.spacing.edge_node, 20.0);
        assert_eq!(options.spacing.edge_label, 4.0);
        assert_eq!(options.spacing.label_label, 0.0);
        assert_eq!(options.spacing.label_node, 10.0);
        assert_eq!(options.spacing.label_port_horizontal, 2.0);
        assert_eq!(options.spacing.label_port_vertical, 2.0);
        assert_eq!(options.spacing.port_port, 20.0);
        assert_eq!(options.spacing.edge_edge_between_layers, 20.0);
        assert_eq!(options.spacing.edge_node_between_layers, 20.0);
        assert_eq!(options.spacing.node_node_between_layers, 40.0);
    }

    #[test]
    fn edge_label_side_selection_transposes_symmetrically() {
        use EdgeLabelSideSelection::*;

        assert_eq!(AlwaysUp.transpose(), AlwaysDown);
        assert_eq!(AlwaysDown.transpose(), AlwaysUp);
        assert_eq!(DirectionUp.transpose(), DirectionDown);
        assert_eq!(DirectionDown.transpose(), DirectionUp);
        assert_eq!(SmartUp.transpose(), SmartDown);
        assert_eq!(SmartDown.transpose(), SmartUp);
    }
}
