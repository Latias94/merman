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
            node: 44.0,
            rank: 48.0,
            edge: 14.0,
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
    /// Anchor small cycles at their authored entry and use greedy FAS for larger cycles.
    #[default]
    HybridAdaptive,
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
    /// Treat top-left titles as tabs attached to the group corner.
    pub flush_top_left_titles: bool,
}

impl Default for GroupPolicy {
    fn default() -> Self {
        Self {
            inherit_crossed_direction: true,
            adaptive_title_clearance: true,
            flush_top_left_titles: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PackingPolicy {
    AspectTargeted {
        target_ratio: f64,
    },
    /// Pack complete components against a two-dimensional skyline in drawing coordinates.
    BalancedAspect {
        target_ratio: f64,
    },
    Strip,
    FitWidth {
        max_width: f64,
    },
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
    /// Maximum vertices in the optional orthogonal visibility graph.
    pub max_ortho_grid_nodes: usize,
}

impl Default for BudgetCaps {
    fn default() -> Self {
        Self {
            max_dummies: 20_000,
            max_sweeps: 24,
            max_transpose_nodes: 600,
            max_weighed_gap_pieces: 400,
            max_border_ports: 40_000,
            max_ordered_compound_ports: 400,
            max_ordered_compound_edges: 500,
            max_ortho_grid_nodes: crate::ortho::DEFAULT_MAX_ORTHO_GRID_NODES,
        }
    }
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub spacing: Spacing,
    pub routing: RoutingStyle,
    /// Allow unlabelled orthogonal fans of consistently styled edges to share a trunk.
    pub bus_routing: bool,
    /// Use obstacle-aware global routing for orthogonal paths when a route can be improved.
    pub global_orthogonal: bool,
    /// Try centering short middle labels on their route within the original rank gap.
    pub inline_labels: bool,
    /// Allow adjacent ranks to use free space below or above their shorter members.
    pub compact_ranks: bool,
    /// Reuse proven unchanged flat components from the last accepted editor solve in the scratch.
    pub reuse_unchanged_components: bool,
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

impl Default for Options {
    fn default() -> Self {
        Self {
            spacing: Spacing::default(),
            routing: RoutingStyle::default(),
            bus_routing: true,
            global_orthogonal: false,
            inline_labels: true,
            compact_ranks: true,
            reuse_unchanged_components: false,
            cycle_policy: CyclePolicy::default(),
            rank_policy: RankPolicy::default(),
            group_policy: GroupPolicy::default(),
            packing: PackingPolicy::default(),
            width_policy: WidthPolicy::default(),
            caps: BudgetCaps::default(),
            grid_quantum: None,
            capture_node_ranks: false,
        }
    }
}
