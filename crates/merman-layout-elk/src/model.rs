const DEFAULT_NODE_SPACING: f64 = 50.0;
const DEFAULT_LAYER_SPACING: f64 = 70.0;
const DEFAULT_GROUP_PADDING_X: f64 = 40.0;
const DEFAULT_GROUP_PADDING_Y: f64 = 48.0;
const DEFAULT_GROUP_LABEL_GAP: f64 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    Left,
    Right,
    Up,
    #[default]
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NodeKind {
    #[default]
    Leaf,
    Group,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Graph {
    pub id: String,
    pub direction: Direction,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub spacing: Spacing,
    pub options: LayoutOptions,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spacing {
    pub node_node: f64,
    pub layer_layer: f64,
    pub group_padding_x: f64,
    pub group_padding_y: f64,
    pub group_label_gap: f64,
}

impl Default for Spacing {
    fn default() -> Self {
        Self {
            node_node: DEFAULT_NODE_SPACING,
            layer_layer: DEFAULT_LAYER_SPACING,
            group_padding_x: DEFAULT_GROUP_PADDING_X,
            group_padding_y: DEFAULT_GROUP_PADDING_Y,
            group_label_gap: DEFAULT_GROUP_LABEL_GAP,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LayoutOptions {
    pub layered: LayeredOptions,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayeredOptions {
    /// ELK's source `randomSeed` option. `0` retains the upstream unseeded sentinel and must be
    /// resolved by an operation-owned seed before processor execution.
    pub random_seed: i32,
    pub hierarchy_handling: HierarchyHandling,
    pub edge_routing: EdgeRouting,
    pub cycle_breaking: CycleBreakingStrategy,
    pub node_placement: NodePlacementStrategy,
    pub node_placement_alignment: NodePlacementAlignment,
    pub model_order: ModelOrderStrategy,
    pub consider_model_order: bool,
    pub force_node_model_order: bool,
    pub merge_edges: bool,
    pub merge_hierarchy_edges: bool,
    pub unnecessary_bendpoints: bool,
    pub inside_self_loops_activate: bool,
    pub self_loop_distribution: SelfLoopDistributionStrategy,
    pub self_loop_ordering: SelfLoopOrderingStrategy,
}

impl Default for LayeredOptions {
    fn default() -> Self {
        Self {
            random_seed: 1,
            hierarchy_handling: HierarchyHandling::IncludeChildren,
            edge_routing: EdgeRouting::Orthogonal,
            cycle_breaking: CycleBreakingStrategy::Greedy,
            node_placement: NodePlacementStrategy::BrandesKoepf,
            node_placement_alignment: NodePlacementAlignment::None,
            model_order: ModelOrderStrategy::NodesAndEdges,
            consider_model_order: true,
            force_node_model_order: false,
            merge_edges: false,
            merge_hierarchy_edges: true,
            unnecessary_bendpoints: true,
            inside_self_loops_activate: false,
            self_loop_distribution: SelfLoopDistributionStrategy::Equally,
            self_loop_ordering: SelfLoopOrderingStrategy::Stacked,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HierarchyHandling {
    #[default]
    IncludeChildren,
    SeparateChildren,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EdgeRouting {
    #[default]
    Orthogonal,
    Polyline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CycleBreakingStrategy {
    ModelOrder,
    #[default]
    Greedy,
    DepthFirst,
    Interactive,
    GreedyModelOrder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NodePlacementStrategy {
    Simple,
    NetworkSimplex,
    LinearSegments,
    #[default]
    BrandesKoepf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NodePlacementAlignment {
    #[default]
    None,
    LeftUp,
    RightUp,
    LeftDown,
    RightDown,
    Balanced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModelOrderStrategy {
    None,
    #[default]
    NodesAndEdges,
    PreferEdges,
    PreferNodes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelfLoopDistributionStrategy {
    North,
    #[default]
    Equally,
    NorthSouth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelfLoopOrderingStrategy {
    #[default]
    Stacked,
    ReverseStacked,
    Sequenced,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: String,
    pub kind: NodeKind,
    pub width: f64,
    pub height: f64,
    pub parent: Option<String>,
    pub direction: Option<Direction>,
    pub hierarchy_handling: Option<HierarchyHandling>,
    pub layer_constraint: Option<LayerConstraint>,
    pub label: Option<Label>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerConstraint {
    First,
    FirstSeparate,
    Last,
    LastSeparate,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub label: Option<Label>,
    pub minlen: usize,
    pub inside_self_loops_yo: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Label {
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LayoutResult {
    pub nodes: Vec<NodeLayout>,
    pub edges: Vec<EdgeLayout>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeLayout {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EdgeLayout {
    pub id: String,
    pub points: Vec<Point>,
    pub labels: Vec<EdgeLabelLayout>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EdgeLabelLayout {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}
