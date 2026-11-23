//! Temporary flat-DAG backend behind the public measured-graph interface.

mod kernel;
mod layers;
mod routing;

use crate::scratch::Buffers;
use crate::work::{LayoutResult, Work, reserved};
use crate::{
    Direction, EdgeSpec, Endpoint, Graph, LayoutError, LayoutOutput, NodeSpec, Options,
    PackingPolicy, PortSideHint, RankPolicy, Side, Size, WidthPolicy, WorkError,
};

impl Direction {
    fn ports(self) -> (Side, Side) {
        match self {
            Self::Down => (Side::Bottom, Side::Top),
            Self::Up => (Side::Top, Side::Bottom),
            Self::Right => (Side::Right, Side::Left),
            Self::Left => (Side::Left, Side::Right),
        }
    }
}

fn length(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}
fn measured(size: Size) -> bool {
    length(size.width) && length(size.height)
}
fn positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

fn validate_options(options: &Options) -> LayoutResult<()> {
    let s = &options.spacing;
    if ![
        s.node,
        s.rank,
        s.edge,
        s.component,
        s.port_sep,
        s.port_pitch,
        s.min_port_pitch,
        s.corner_clearance,
        s.loop_gap,
        s.end_label_along,
        s.end_label_across,
    ]
    .into_iter()
    .all(length)
    {
        return Err(LayoutError::InvalidInput(
            "spacing must be finite and nonnegative",
        ));
    }
    if let Some(quantum) = options.grid_quantum {
        if !positive(quantum) {
            return Err(LayoutError::InvalidInput(
                "grid quantum must be positive and finite",
            ));
        }
        return Err(LayoutError::Unsupported("grid layout"));
    }
    if let WidthPolicy::FitWidth {
        max_width,
        turn_breakpoint,
        ..
    } = options.width_policy
    {
        if !positive(max_width) || !length(turn_breakpoint) {
            return Err(LayoutError::InvalidInput("invalid width policy dimensions"));
        }
        return Err(LayoutError::Unsupported("fit-width layout"));
    }
    match options.packing {
        PackingPolicy::Strip => {}
        PackingPolicy::FitWidth { max_width } if !positive(max_width) => {
            return Err(LayoutError::InvalidInput(
                "packing width must be positive and finite",
            ));
        }
        PackingPolicy::AspectTargeted { target_ratio } if !positive(target_ratio) => {
            return Err(LayoutError::InvalidInput(
                "packing aspect ratio must be positive and finite",
            ));
        }
        _ => {
            return Err(LayoutError::Unsupported(
                "two-dimensional component packing",
            ));
        }
    }
    if matches!(
        options.rank_policy,
        RankPolicy::BoundedNetworkSimplex { .. }
    ) {
        return Err(LayoutError::Unsupported("network simplex ranking"));
    }
    Ok(())
}

struct FlatEdge<'a> {
    source: usize,
    target: usize,
    spec: &'a EdgeSpec,
}

struct FlatGraph<'a> {
    direction: Direction,
    nodes: &'a [NodeSpec],
    edges: Vec<FlatEdge<'a>>,
    options: &'a Options,
}

fn node_index(endpoint: Endpoint, graph: &Graph) -> LayoutResult<usize> {
    match endpoint {
        Endpoint::Node(id) if (id.0 as usize) < graph.nodes.len() => Ok(id.0 as usize),
        _ => Err(LayoutError::InvalidInput(
            "edge endpoint is outside its input array",
        )),
    }
}

fn validate_port(hint: PortSideHint, natural: Side, size: Size) -> LayoutResult<()> {
    let side = match hint {
        PortSideHint::Auto => return Ok(()),
        PortSideHint::Side(side) => side,
        PortSideHint::FixedOffset { side, offset } => {
            let face = match side {
                Side::Top | Side::Bottom => size.width,
                Side::Left | Side::Right => size.height,
            };
            if !offset.is_finite() || offset.abs() > face * 0.5 {
                return Err(LayoutError::InvalidInput(
                    "fixed port offset is outside the node face",
                ));
            }
            side
        }
    };
    if side != natural {
        return Err(LayoutError::Unsupported("ports on a non-flow-facing side"));
    }
    Ok(())
}

pub(super) fn run(
    graph: &Graph,
    options: &Options,
    buffers: &mut Buffers,
    work: &mut Work,
) -> LayoutResult<LayoutOutput> {
    validate_options(options)?;
    for count in [graph.nodes.len(), graph.groups.len(), graph.edges.len()] {
        u32::try_from(count).map_err(|_| WorkError::ArithmeticOverflow)?;
    }
    if !graph.groups.is_empty() {
        return Err(LayoutError::Unsupported("subgraphs"));
    }
    for node in &graph.nodes {
        work.visit(1)?;
        if node.parent.is_some() {
            return Err(LayoutError::InvalidInput(
                "node parent is outside the group array",
            ));
        }
        if !measured(node.size)
            || node.port_span.is_some_and(|v| !length(v))
            || node
                .anchor_y
                .is_some_and(|v| !length(v) || v > node.size.height)
            || node.order_hint.is_some_and(|h| !h.cross_key.is_finite())
        {
            return Err(LayoutError::InvalidInput(
                "invalid node measurement or order hint",
            ));
        }
    }
    work.charge(graph.edges.len())?;
    let mut edges = reserved(graph.edges.len())?;
    let (source_side, target_side) = graph.direction.ports();
    for spec in &graph.edges {
        work.visit(1)?;
        let source = node_index(spec.source, graph)?;
        let target = node_index(spec.target, graph)?;
        if ![spec.label, spec.start_label, spec.end_label]
            .into_iter()
            .flatten()
            .all(measured)
            || ![spec.start_marker, spec.end_marker]
                .into_iter()
                .all(|m| length(m.length) && length(m.width))
            || !positive(spec.weight.into())
        {
            return Err(LayoutError::InvalidInput(
                "invalid edge weight, label, or marker measurement",
            ));
        }
        validate_port(spec.source_port, source_side, graph.nodes[source].size)?;
        validate_port(spec.target_port, target_side, graph.nodes[target].size)?;
        if spec.start_label.is_some() || spec.end_label.is_some() {
            return Err(LayoutError::Unsupported("terminal labels"));
        }
        if source == target {
            return Err(LayoutError::Unsupported("self-loops"));
        }
        edges.push(FlatEdge {
            source,
            target,
            spec,
        });
    }
    kernel::run(
        &FlatGraph {
            direction: graph.direction,
            nodes: &graph.nodes,
            edges,
            options,
        },
        buffers,
        work,
    )
}
