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
    pub algorithm: Algorithm,
    pub layered: LayeredOptions,
    pub container: ContainerOptions,
}

/// Resolved ELK provider identity. Root loader names and container metadata have distinct
/// allowlists in Mermaid; callers must resolve those before constructing the graph.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Algorithm {
    #[default]
    Layered,
    Box,
    Rectpacking,
    Force,
    Stress,
    MrTree,
    Radial,
    SporeOverlap,
}

impl Algorithm {
    /// Exact Mermaid 12 container allowlist. Root loaders have a different set of names.
    pub fn from_container_name(name: &str) -> Option<Self> {
        Some(match name {
            "elk.layered" => Self::Layered,
            "elk.box" => Self::Box,
            "elk.rectpacking" => Self::Rectpacking,
            "elk.force" => Self::Force,
            "elk.stress" => Self::Stress,
            "elk.mrtree" => Self::MrTree,
            "elk.radial" => Self::Radial,
            "elk.sporeOverlap" => Self::SporeOverlap,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ContainerNodeOptions {
    /// A valid metadata algorithm; absent or invalid metadata leaves this unset.
    pub algorithm: Option<Algorithm>,
    /// Mermaid's measured node padding, used for the title minimum (not ELK content padding).
    pub padding: f64,
}

/// Mermaid resolves container placement independently from root placement in named presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContainerOptions {
    pub cycle_breaking: CycleBreakingStrategy,
    pub node_placement: NodePlacementStrategy,
    pub node_placement_alignment: NodePlacementAlignment,
}

impl Default for ContainerOptions {
    fn default() -> Self {
        Self {
            cycle_breaking: CycleBreakingStrategy::DepthFirst,
            node_placement: NodePlacementStrategy::BrandesKoepf,
            node_placement_alignment: NodePlacementAlignment::Balanced,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayeredOptions {
    /// ELK's source `randomSeed` option. `0` retains the upstream unseeded sentinel and must be
    /// resolved by an operation-owned seed before processor execution.
    pub random_seed: i32,
    pub hierarchy_handling: HierarchyHandling,
    pub edge_routing: EdgeRouting,
    pub cycle_breaking: CycleBreakingStrategy,
    pub layering: LayeringStrategy,
    pub layering_layer_bound: i32,
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
            cycle_breaking: CycleBreakingStrategy::DepthFirst,
            layering: LayeringStrategy::NetworkSimplex,
            layering_layer_bound: 4,
            node_placement: NodePlacementStrategy::BrandesKoepf,
            node_placement_alignment: NodePlacementAlignment::Balanced,
            model_order: ModelOrderStrategy::NodesAndEdges,
            consider_model_order: true,
            force_node_model_order: false,
            merge_edges: false,
            merge_hierarchy_edges: true,
            unnecessary_bendpoints: true,
            inside_self_loops_activate: false,
            self_loop_distribution: SelfLoopDistributionStrategy::North,
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
pub enum LayeringStrategy {
    #[default]
    NetworkSimplex,
    LongestPath,
    LongestPathSource,
    CoffmanGraham,
    MinWidth,
    StretchWidth,
    Interactive,
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
    /// ELK container title text. Leaf text is already measured and is not an ELK node label.
    pub label_text: Option<String>,
    pub container: ContainerNodeOptions,
    pub width: f64,
    pub height: f64,
    pub parent: Option<String>,
    pub direction: Option<Direction>,
    pub hierarchy_handling: Option<HierarchyHandling>,
    pub layer_constraint: Option<LayerConstraint>,
    /// Alignment of this node's implicit ports, overriding the provider default.
    pub port_alignment: Option<PortAlignment>,
    pub label: Option<Label>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortAlignment {
    Distributed,
    Justified,
    Begin,
    Center,
    End,
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
    /// Node center in the containing result coordinate system.
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EdgeLayout {
    pub id: String,
    /// Provider route. Empty means no section was emitted; the renderer must apply Mermaid's
    /// missing-section handling after final node placement and shape intersection are known.
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
