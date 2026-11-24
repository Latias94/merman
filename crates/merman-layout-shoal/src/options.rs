#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spacing {
    pub node: f64,
    pub rank: f64,
    pub edge: f64,
    pub component: f64,
    pub port_sep: f64,
    pub port_pitch: f64,
    pub min_port_pitch: f64,
    pub corner_clearance: f64,
    pub loop_gap: f64,
    pub end_label_along: f64,
    pub end_label_across: f64,
}

impl Default for Spacing {
    fn default() -> Self {
        Self {
            node: 50.0,
            rank: 50.0,
            edge: 4.0,
            component: 50.0,
            port_sep: 20.0,
            port_pitch: 16.0,
            min_port_pitch: 8.0,
            corner_clearance: 12.0,
            loop_gap: 6.0,
            end_label_along: 4.0,
            end_label_across: 6.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RoutingStyle {
    #[default]
    Curved,
    Orthogonal,
    Polyline,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CyclePolicy {
    #[default]
    SccGreedyFas,
    SccDfs,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RankPolicy {
    #[default]
    TightenedLongestPath,
    BoundedNetworkSimplex {
        max_pivots: u32,
    },
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GroupPolicy {
    pub inherit_crossed_direction: bool,
    pub adaptive_title_clearance: bool,
}

impl Default for GroupPolicy {
    fn default() -> Self {
        Self {
            inherit_crossed_direction: true,
            adaptive_title_clearance: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PackingPolicy {
    AspectTargeted { target_ratio: f64 },
    Strip,
    FitWidth { max_width: f64 },
}

impl Default for PackingPolicy {
    fn default() -> Self {
        Self::AspectTargeted { target_ratio: 1.6 }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum WidthPolicy {
    #[default]
    Natural,
    FitWidth {
        max_width: f64,
        allow_turn: bool,
        turn_breakpoint: f64,
        fold_wide_ranks: bool,
    },
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BudgetCaps {
    pub max_dummies: usize,
    pub max_sweeps: u32,
    pub max_transpose_nodes: usize,
    pub max_weighed_gap_pieces: usize,
    /// Maximum intermediate group-boundary ports. An authored group endpoint remains an endpoint.
    pub max_border_ports: usize,
    pub max_ordered_compound_ports: usize,
    pub max_ordered_compound_edges: usize,
}

impl Default for BudgetCaps {
    fn default() -> Self {
        Self {
            max_dummies: 20_000,
            max_sweeps: 24,
            max_transpose_nodes: 1500,
            max_weighed_gap_pieces: 400,
            max_border_ports: 40_000,
            max_ordered_compound_ports: 400,
            max_ordered_compound_edges: 500,
        }
    }
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Options {
    pub spacing: Spacing,
    pub routing: RoutingStyle,
    pub cycle_policy: CyclePolicy,
    pub rank_policy: RankPolicy,
    pub group_policy: GroupPolicy,
    pub packing: PackingPolicy,
    pub width_policy: WidthPolicy,
    pub caps: BudgetCaps,
    pub grid_quantum: Option<f64>,
    /// Include each node's local layered rank in the result for editor sessions.
    pub capture_node_ranks: bool,
}
