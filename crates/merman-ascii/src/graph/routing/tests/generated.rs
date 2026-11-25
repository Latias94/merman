//! Bounded, authored-input coverage. Neither the corpus nor its denominator depends on routing.
use super::*;
use crate::graph::layout::layout_graph_with_resources;
use crate::graph::routing::label::RoutedLabelDescriptor;
use crate::graph::routing::plan::NodeAttachment;
use std::collections::{BTreeMap, BTreeSet};

// These seeds, sizes, options, and permutations are deliberately fixed. Do not retry with
// different inputs, discard a rejected scene, or turn a newly rejected case into an exception.
const SAMPLES: [(u64, usize); 3] = [(0x19a4_7c21, 2), (0x73b9_025d, 5), (0xd061_e8af, 8)];
const DIRECTIONS: [GraphDirection; 2] = [GraphDirection::LeftRight, GraphDirection::TopDown];
const MODES: [&str; 3] = ["ascii", "unicode", "unicode-zero"];
const ORDERS: usize = 3;
const SCENE_COUNT: usize = 9 * 3 * 2 * 3 * ORDERS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Family {
    Chain,
    Dag,
    FanOut,
    FanIn,
    Feedback,
    Reciprocal,
    Parallel,
    Loop,
    Mixed,
}

const FAMILIES: [Family; 9] = [
    Family::Chain,
    Family::Dag,
    Family::FanOut,
    Family::FanIn,
    Family::Feedback,
    Family::Reciprocal,
    Family::Parallel,
    Family::Loop,
    Family::Mixed,
];

struct Generator(u64);

impl Generator {
    fn choose(&mut self, bound: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        ((self.0 >> 32) as usize) % bound
    }

    fn shuffle<T>(&mut self, values: &mut [T]) {
        for end in (1..values.len()).rev() {
            values.swap(end, self.choose(end + 1));
        }
    }
}

fn generated_graph(
    family: Family,
    seed: u64,
    count: usize,
    direction: GraphDirection,
) -> AsciiGraph {
    let mut rng = Generator(seed);
    let mut graph = AsciiGraph::new(direction);
    for index in 0..count {
        let shape = if family == Family::Chain || index == 0 || index + 1 == count {
            GraphNodeShape::Rect
        } else {
            match index % 4 {
                0 => GraphNodeShape::Rect,
                1 => GraphNodeShape::Diamond,
                2 => GraphNodeShape::LeanRight,
                _ => GraphNodeShape::LeanLeft,
            }
        };
        graph.add_node_with_shape_and_style(
            format!("n{index}"),
            format!("N{index}"),
            shape,
            GraphNodeStyle::default(),
        );
    }
    let mut pairs: Vec<_> = match family {
        Family::FanOut => (1..count).map(|index| (0, index)).collect(),
        Family::FanIn => (0..count - 1).map(|index| (index, count - 1)).collect(),
        _ => (1..count).map(|index| (index - 1, index)).collect(),
    };
    match family {
        Family::Dag => {
            let mut shortcuts: Vec<_> = (0..count)
                .flat_map(|from| (from + 2..count).map(move |to| (from, to)))
                .collect();
            rng.shuffle(&mut shortcuts);
            pairs.extend(shortcuts.into_iter().take(12 - pairs.len()));
        }
        Family::Feedback => {
            pairs.push((count - 1, 0));
            if count > 2 {
                pairs.extend([(count - 1, 1), (count - 2, 0)]);
            }
        }
        Family::Reciprocal => pairs.extend((1..count).take(12 - pairs.len()).map(|i| (i, i - 1))),
        Family::Parallel => pairs.extend([(0, 1), (0, 1), (count - 2, count - 1)]),
        Family::Loop => pairs.extend([(0, 0), (count / 2, count / 2), (count - 1, count - 1)]),
        Family::Mixed => {
            pairs.extend([(count - 1, 0), (1, 0), (0, 1), (count / 2, count / 2)]);
            while pairs.len() < (count + 4).min(12) {
                pairs.push((rng.choose(count), rng.choose(count)));
            }
        }
        Family::Chain | Family::FanOut | Family::FanIn => {}
    }
    let markers = [
        GraphEdgeMarker::Open,
        GraphEdgeMarker::Point,
        GraphEdgeMarker::Circle,
        GraphEdgeMarker::Cross,
    ];
    for (index, (from, to)) in pairs.into_iter().enumerate() {
        let mut attrs = GraphEdgeAttrs::default();
        if family != Family::Chain && index % 3 != 0 {
            attrs.label = Some(if index % 3 == 1 {
                format!("e{index}")
            } else {
                format!("e{index}<br>ok")
            });
        }
        match family {
            Family::FanOut => {
                attrs.start_marker = GraphEdgeMarker::Circle;
                attrs.end_marker = markers[index % 4];
            }
            Family::FanIn => attrs.start_marker = markers[index % 4],
            Family::Mixed => {
                attrs.stroke = [
                    GraphEdgeStroke::Normal,
                    GraphEdgeStroke::Dotted,
                    GraphEdgeStroke::Thick,
                ][index % 3];
                attrs.start_marker = markers[index % 4];
                attrs.end_marker = markers[(index + 1) % 4];
            }
            _ => {}
        }
        graph.add_edge_with_attrs(format!("n{from}"), format!("n{to}"), attrs);
    }
    if family == Family::Parallel {
        // Keep a genuinely identical pair, including its absent label, to test multiplicity.
        let duplicate = graph.edges[0].clone();
        let index = graph
            .edges
            .iter()
            .rposition(|edge| edge.from == "n0" && edge.to == "n1")
            .unwrap();
        graph.edges[index] = duplicate;
    }
    if family == Family::Mixed {
        graph.add_edge_with_attrs(
            "n0",
            format!("n{}", count - 1),
            GraphEdgeAttrs {
                label: Some("hidden".into()),
                stroke: GraphEdgeStroke::Invisible,
                start_marker: GraphEdgeMarker::Cross,
                ..GraphEdgeAttrs::default()
            },
        );
    }
    // Keep contour probes on paths/leaves. Independent, potentially incompatible
    // incident demand uses expandable rectangular frames, rather than fixed diamond tips.
    for node in &mut graph.nodes {
        let degree: usize = graph
            .edges
            .iter()
            .filter(|edge| edge.stroke != GraphEdgeStroke::Invisible)
            .map(|edge| usize::from(edge.from == node.id) + usize::from(edge.to == node.id))
            .sum();
        if degree > 2 {
            node.shape = GraphNodeShape::Rect;
        }
    }
    assert!(graph.nodes.len() <= 8);
    assert!(
        graph
            .edges
            .iter()
            .filter(|edge| edge.stroke != GraphEdgeStroke::Invisible)
            .count()
            <= 12
    );
    graph
}

fn options(mode: &str) -> AsciiRenderOptions {
    match mode {
        "ascii" => AsciiRenderOptions::ascii(),
        "unicode" => AsciiRenderOptions::unicode(),
        "unicode-zero" => AsciiRenderOptions::unicode()
            .with_graph_padding_x(0)
            .with_graph_padding_y(0),
        _ => unreachable!("fixed test mode"),
    }
}

macro_rules! require {
    ($condition:expr, $($message:tt)+) => {
        if !$condition {
            return Err(format!($($message)+));
        }
    };
}

type OracleResult = std::result::Result<(), String>;
type CellIndex<'a> = BTreeMap<(usize, usize), &'a PlannedRouteCell>;

fn marker_key(marker: GraphEdgeMarker) -> u8 {
    match marker {
        GraphEdgeMarker::Open => 0,
        GraphEdgeMarker::Point => 1,
        GraphEdgeMarker::Circle => 2,
        GraphEdgeMarker::Cross => 3,
    }
}

// This tuple is authored information, not a call into canonicalize_edges/compare_edges.
// Generated edges have no explicit IDs, colors, or non-default lengths.
fn edge_key(edge: &AsciiGraphEdge) -> (&str, &str, Option<&str>, u8, u8, u8) {
    let stroke = match edge.stroke {
        GraphEdgeStroke::Normal => 0,
        GraphEdgeStroke::Dotted => 1,
        GraphEdgeStroke::Thick => 2,
        GraphEdgeStroke::Invisible => 3,
    };
    (
        &edge.from,
        &edge.to,
        edge.label.as_deref(),
        stroke,
        marker_key(edge.start_marker),
        marker_key(edge.end_marker),
    )
}

fn step(coord: CanvasCoord, direction: StepDirection) -> Option<CanvasCoord> {
    let (x, y) = (coord.x, coord.y);
    match direction {
        StepDirection::Up => y.checked_sub(1).map(|y| CanvasCoord { x, y }),
        StepDirection::Right => x.checked_add(1).map(|x| CanvasCoord { x, y }),
        StepDirection::Down => y.checked_add(1).map(|y| CanvasCoord { x, y }),
        StepDirection::Left => x.checked_sub(1).map(|x| CanvasCoord { x, y }),
    }
}

fn inward(outward: StepDirection) -> StepDirection {
    match outward {
        StepDirection::Up => StepDirection::Down,
        StepDirection::Right => StepDirection::Left,
        StepDirection::Down => StepDirection::Up,
        StepDirection::Left => StepDirection::Right,
    }
}

fn on_ray(port: NodeAttachment, coord: CanvasCoord) -> bool {
    match port.outward {
        StepDirection::Up => coord.x == port.contact.x && coord.y <= port.contact.y,
        StepDirection::Right => coord.y == port.contact.y && coord.x >= port.contact.x,
        StepDirection::Down => coord.x == port.contact.x && coord.y >= port.contact.y,
        StepDirection::Left => coord.y == port.contact.y && coord.x <= port.contact.x,
    }
}

// Independently rasterize the contours used by the fixtures. Slanted nodes occupy
// different column ranges on each row.
fn shape_contains(node: &NodeLayout, coord: CanvasCoord) -> bool {
    if coord.y < node.y || coord.y >= node.y + node.height {
        return false;
    }
    let right = node.x + node.width - 1;
    let slant = (node.height - 1).min(node.width.saturating_sub(2));
    let shift = (coord.y - node.y) * slant / (node.height - 1).max(1);
    let (left, right) = match node.shape {
        GraphNodeShape::LeanRight => (node.x + shift, right - slant + shift),
        GraphNodeShape::LeanLeft => (node.x + slant - shift, right - shift),
        GraphNodeShape::Rect
        | GraphNodeShape::Rounded
        | GraphNodeShape::Diamond
        | GraphNodeShape::Cylinder => (node.x, right),
        _ => panic!("generator introduced a shape without an independent contour oracle"),
    };
    (left..=right).contains(&coord.x)
}

fn valid_contact(node: &NodeLayout, port: NodeAttachment) -> bool {
    let contact = port.contact;
    let is_tip = node.shape != GraphNodeShape::Diamond
        || [
            (node.x, node.y + node.height / 2),
            (node.x + node.width - 1, node.y + node.height / 2),
            (node.x + node.width / 2, node.y),
            (node.x + node.width / 2, node.y + node.height - 1),
        ]
        .contains(&(contact.x, contact.y));
    is_tip
        && shape_contains(node, contact)
        && step(contact, port.outward).is_none_or(|next| !shape_contains(node, next))
}

fn expected_marker(marker: GraphEdgeMarker, direction: StepDirection, unicode: bool) -> char {
    match (marker, unicode) {
        (GraphEdgeMarker::Circle, false) => 'o',
        (GraphEdgeMarker::Circle, true) => '○',
        (GraphEdgeMarker::Cross, false) => 'x',
        (GraphEdgeMarker::Cross, true) => '×',
        (GraphEdgeMarker::Point, false) => match direction {
            StepDirection::Up => '^',
            StepDirection::Right => '>',
            StepDirection::Down => 'v',
            StepDirection::Left => '<',
        },
        (GraphEdgeMarker::Point, true) => match direction {
            StepDirection::Up => '▲',
            StepDirection::Right => '►',
            StepDirection::Down => '▼',
            StepDirection::Left => '◄',
        },
        (GraphEdgeMarker::Open, _) => unreachable!("open endpoints have no materialized glyph"),
    }
}

fn straight_stroke(stroke: GraphEdgeStroke, axis: u8, unicode: bool) -> Option<char> {
    match (stroke, axis, unicode) {
        (GraphEdgeStroke::Normal, 1, false) => Some('-'),
        (GraphEdgeStroke::Normal, 2, false) => Some('|'),
        (GraphEdgeStroke::Dotted, 1, false) => Some('.'),
        (GraphEdgeStroke::Dotted, 2, false) => Some(':'),
        (GraphEdgeStroke::Thick, 1, false) => Some('='),
        (GraphEdgeStroke::Thick, 2, false) => Some('#'),
        (GraphEdgeStroke::Normal, 1, true) => Some('─'),
        (GraphEdgeStroke::Normal, 2, true) => Some('│'),
        (GraphEdgeStroke::Dotted, 1, true) => Some('┄'),
        (GraphEdgeStroke::Dotted, 2, true) => Some('┆'),
        (GraphEdgeStroke::Thick, 1, true) => Some('━'),
        (GraphEdgeStroke::Thick, 2, true) => Some('┃'),
        _ => None,
    }
}

// Axis bits come from glyphs/neighboring coordinates, never the planner's direction mask.
fn axes(cell: &PlannedRouteCell, cells: &CellIndex<'_>) -> u8 {
    match cell.ch {
        '-' | '.' | '=' | '─' | '┄' | '━' | '<' | '>' | '◄' | '►' => 1,
        '|' | '│' | '┆' | '┃' | '^' | 'v' | '▲' | '▼' => 2,
        '┌' | '┐' | '└' | '┘' | '╭' | '╮' | '╰' | '╯' | '┏' | '┓' | '┗' | '┛' | '├' | '┤' | '┬'
        | '┴' | '┣' | '┫' | '┳' | '┻' | '┝' | '┥' | '┰' | '┸' | '┼' | '╋' => {
            3
        }
        // ASCII dotted/thick bends share their vertical glyph; infer their axes
        // from this route's coordinates instead of treating every ':'/'#' as straight.
        '+' | ':' | '#' | '·' | 'o' | 'x' | '○' | '×' => {
            let mut axes = 0;
            for direction in [
                StepDirection::Up,
                StepDirection::Right,
                StepDirection::Down,
                StepDirection::Left,
            ] {
                if step(cell.coord, direction)
                    .is_some_and(|next| cells.contains_key(&(next.x, next.y)))
                {
                    axes |= match direction {
                        StepDirection::Left | StepDirection::Right => 1,
                        StepDirection::Up | StepDirection::Down => 2,
                    };
                }
            }
            axes
        }
        _ => 0,
    }
}

fn endpoint_marker(edge: &AsciiGraphEdge, endpoint: MarkerEndpoint) -> GraphEdgeMarker {
    match endpoint {
        MarkerEndpoint::Start => edge.start_marker,
        MarkerEndpoint::End => edge.end_marker,
    }
}

fn justified_sharing(
    a: &PreparedRoute,
    b: &PreparedRoute,
    ea: &AsciiGraphEdge,
    eb: &AsciiGraphEdge,
    coord: CanvasCoord,
) -> bool {
    let (Some(pa), Some(pb)) = (a.plan.attachments, b.plan.attachments) else {
        return false;
    };
    let mut common_ray = false;
    for (end_a, id_a, port_a) in [
        (MarkerEndpoint::Start, &ea.from, pa.start),
        (MarkerEndpoint::End, &ea.to, pa.end),
    ] {
        for (end_b, id_b, port_b) in [
            (MarkerEndpoint::Start, &eb.from, pb.start),
            (MarkerEndpoint::End, &eb.to, pb.end),
        ] {
            if id_a != id_b {
                continue;
            }
            if coord == port_a.contact && coord == port_b.contact {
                return true;
            }
            if end_a == end_b
                && port_a.contact == port_b.contact
                && port_a.outward == port_b.outward
                && on_ray(port_a, coord)
                && ea.stroke == eb.stroke
                && ea.style == eb.style
            {
                common_ray = true;
            }
        }
    }
    if a.owner.allocated_branch.is_none() && b.owner.allocated_branch.is_none() {
        return common_ray;
    }
    let (Some(ca), Some(cb)) = (a.owner.allocated_branch, b.owner.allocated_branch) else {
        return false;
    };
    let in_channel = |points: [CanvasCoord; 4]| {
        points.windows(2).any(|ends| {
            let (p, q) = (ends[0], ends[1]);
            (p.x == q.x && coord.x == p.x && (p.y.min(q.y)..=p.y.max(q.y)).contains(&coord.y))
                || (p.y == q.y
                    && coord.y == p.y
                    && (p.x.min(q.x)..=p.x.max(q.x)).contains(&coord.x))
        })
    };
    ca.group == cb.group
        && ca.shared_endpoint == cb.shared_endpoint
        && ca.attachments == pa
        && cb.attachments == pb
        && a.owner.endpoint_id(ca.shared_endpoint) == b.owner.endpoint_id(cb.shared_endpoint)
        && ea.stroke == eb.stroke
        && ea.style == eb.style
        && endpoint_marker(ea, ca.shared_endpoint) == endpoint_marker(eb, cb.shared_endpoint)
        && match ca.shared_endpoint {
            MarkerEndpoint::Start => pa.start == pb.start,
            MarkerEndpoint::End => pa.end == pb.end,
        }
        && in_channel(ca.points)
        && in_channel(cb.points)
}

fn information_oracle(
    graph: &AsciiGraph,
    layout: &GraphLayout,
    scene: &RouteScene,
    options: &AsciiRenderOptions,
) -> OracleResult {
    require!(
        graph.groups.is_empty() && layout.groups.is_empty(),
        "shape: containers require a separate oracle"
    );
    require!(
        layout.nodes.len() == graph.nodes.len(),
        "shape: node multiplicity"
    );
    for node in &layout.nodes {
        require!(
            node.width > 0
                && node.height > 0
                && graph
                    .nodes
                    .iter()
                    .filter(|expected| expected.id == node.id && expected.shape == node.shape)
                    .count()
                    == 1,
            "shape: node identity/shape at {}",
            node.id
        );
    }
    let mut expected: Vec<_> = graph.edges.iter().collect();
    expected.sort_by_key(|&edge| edge_key(edge));
    let visible = expected
        .iter()
        .filter(|edge| edge.stroke != GraphEdgeStroke::Invisible)
        .count();
    require!(
        scene.routes.len() == visible,
        "edge: expected {visible} visible contributions, got {}",
        scene.routes.len()
    );
    let mut seen = vec![false; expected.len()];
    let indices: Vec<CellIndex<'_>> = scene
        .routes
        .iter()
        .map(|route| {
            route
                .plan
                .active_cells()
                .map(|(_, cell)| ((cell.coord.x, cell.coord.y), cell))
                .collect()
        })
        .collect();
    let mut markers = Vec::new();
    let mut labels = BTreeMap::new();
    let unicode = options.charset == crate::AsciiCharset::Unicode;
    for (route_index, route) in scene.routes.iter().enumerate() {
        let canonical = route.owner.canonical_edge_index;
        require!(
            canonical < expected.len() && !seen[canonical],
            "edge: repeated/invalid canonical index {canonical}"
        );
        seen[canonical] = true;
        let edge = expected[canonical];
        require!(
            edge.stroke != GraphEdgeStroke::Invisible
                && route.owner.from == edge.from
                && route.owner.to == edge.to
                && route.owner.stroke == edge.stroke,
            "edge: endpoint/stroke identity at {canonical}"
        );
        let Some(ports) = route.plan.attachments else {
            return Err(format!("shape: missing contacts at {canonical}"));
        };
        let cells = &indices[route_index];
        require!(
            !cells.is_empty(),
            "edge: empty visible contribution at {canonical}"
        );
        let mut own_markers = BTreeSet::new();
        for (endpoint, id, port) in [
            (MarkerEndpoint::Start, &edge.from, ports.start),
            (MarkerEndpoint::End, &edge.to, ports.end),
        ] {
            let node = layout
                .nodes
                .iter()
                .find(|node| &node.id == id)
                .ok_or_else(|| format!("shape: missing node {id}"))?;
            require!(
                valid_contact(node, port),
                "shape: invalid actual contour contact {id}: {port:?}"
            );
            let terminal = route
                .plan
                .terminal_candidate(endpoint, "flowchart")
                .map_err(|error| format!("marker: {error}"))?;
            require!(
                terminal.point_direction == inward(port.outward) && on_ray(port, terminal.coord),
                "marker: direction/contact ray for {id} at {canonical}"
            );
            require!(
                cells.contains_key(&(terminal.coord.x, terminal.coord.y)),
                "edge: terminal is absent at {canonical}"
            );
            // Relocated heads may suppress their original terminal tail. Every position
            // between the actual contour and the new berth must still belong to that tail.
            require!(
                port.contact.x.abs_diff(terminal.coord.x)
                    + port.contact.y.abs_diff(terminal.coord.y)
                    <= route.plan.cells.len(),
                "shape: unbounded terminal ray at {canonical}"
            );
            let mut coord = port.contact;
            while coord != terminal.coord {
                coord = step(coord, port.outward).unwrap();
                require!(
                    route.plan.cells.iter().any(|cell| cell.coord == coord),
                    "shape: detached terminal ray for {id} at {coord:?}"
                );
            }
            let marker = endpoint_marker(edge, endpoint);
            require!(
                route.plan.requests_marker(endpoint) == (marker != GraphEdgeMarker::Open),
                "marker: request kind at {canonical}"
            );
            if marker != GraphEdgeMarker::Open {
                let cell = cells
                    .get(&(terminal.coord.x, terminal.coord.y))
                    .ok_or_else(|| format!("marker: suppressed head at {canonical}"))?;
                let glyph = expected_marker(marker, inward(port.outward), unicode);
                require!(
                    cell.kind == PlannedRouteCellKind::EdgeArrow && cell.ch == glyph,
                    "marker: expected {marker:?} {glyph:?}, got {:?} at {canonical}",
                    cell.ch
                );
                require!(
                    own_markers.insert((terminal.coord.x, terminal.coord.y)),
                    "marker: two endpoints collapsed at {canonical}"
                );
                markers.push((terminal.coord, glyph));
            }
        }
        for cell in cells.values() {
            require!(
                cell.stroke == edge.stroke,
                "stroke: cell identity at {canonical}"
            );
            if cell.kind == PlannedRouteCellKind::RouteCell
                && matches!(
                    cell.ch,
                    '-' | '.' | '=' | '|' | ':' | '#' | '─' | '┄' | '━' | '│' | '┆' | '┃'
                )
                && let Some(glyph) = straight_stroke(edge.stroke, axes(cell, cells), unicode)
            {
                require!(
                    cell.ch == glyph,
                    "stroke: materialized glyph at {canonical}"
                );
            }
            require!(
                cell.kind != PlannedRouteCellKind::EdgeArrow
                    || own_markers.contains(&(cell.coord.x, cell.coord.y)),
                "marker: unsolicited head at {canonical}"
            );
            for node in &layout.nodes {
                require!(
                    !shape_contains(node, cell.coord)
                        || (node.id == edge.from && cell.coord == ports.start.contact)
                        || (node.id == edge.to && cell.coord == ports.end.contact),
                    "shape: route {canonical} entered {} at {:?}",
                    node.id,
                    cell.coord
                );
            }
        }
        // Border connectors belong to the shape; a relocated terminal can leave one
        // behind. The visible transit body itself must be one connected contribution.
        let body: BTreeSet<_> = cells
            .keys()
            .copied()
            .filter(|&(x, y)| {
                let coord = CanvasCoord { x, y };
                coord != ports.start.contact && coord != ports.end.contact
            })
            .collect();
        let first = *body
            .first()
            .ok_or_else(|| format!("edge: no transit body at {canonical}"))?;
        let mut connected = BTreeSet::from([first]);
        let mut pending = vec![CanvasCoord {
            x: first.0,
            y: first.1,
        }];
        while let Some(coord) = pending.pop() {
            for direction in [
                StepDirection::Up,
                StepDirection::Right,
                StepDirection::Down,
                StepDirection::Left,
            ] {
                if let Some(next) = step(coord, direction) {
                    let key = (next.x, next.y);
                    if body.contains(&key) && connected.insert(key) {
                        pending.push(next);
                    }
                }
            }
        }
        require!(
            connected.len() == body.len(),
            "edge: disconnected route {canonical}"
        );
        let lines: Vec<_> = edge
            .label
            .as_deref()
            .map(|raw| raw.split("<br>").collect())
            .unwrap_or_default();
        require!(
            route.plan.labels.len() == usize::from(!lines.is_empty()),
            "label: multiplicity at {canonical}"
        );
        for label in &route.plan.labels {
            let width = lines.iter().map(|line| line.len()).max().unwrap();
            // for_test exposes the otherwise private catalog index. Text and metrics below
            // are checked separately using this generator's ASCII-only label grammar.
            let descriptor = RoutedLabelDescriptor::for_test(
                canonical,
                edge.label.as_deref().unwrap(),
                options.terminal_width_profile,
            )
            .unwrap();
            require!(
                label.descriptor == descriptor
                    && label.descriptor.width() == width
                    && label.descriptor.line_count() == lines.len()
                    && label.placement.width() == width,
                "label: canonical descriptor/metrics at {canonical}"
            );
            let text = scene
                .labels
                .get(label.descriptor)
                .map_err(|error| format!("label: {error}"))?;
            require!(
                text.lines()
                    .iter()
                    .map(String::as_str)
                    .eq(lines.iter().copied()),
                "label: materialized text at {canonical}"
            );
            for y in label.placement.y()..label.placement.y() + lines.len() {
                for x in label.placement.x()..label.placement.x() + width {
                    let coord = CanvasCoord { x, y };
                    require!(
                        labels.insert((x, y), route_index).is_none(),
                        "label: overlapping label boxes at {coord:?}"
                    );
                    require!(
                        layout.nodes.iter().all(|node| !shape_contains(node, coord)),
                        "label: node intersection at {coord:?}"
                    );
                    require!(
                        indices.iter().enumerate().all(
                            |(other, body)| other == route_index || !body.contains_key(&(x, y))
                        ),
                        "label: overwrote another edge at {coord:?}"
                    );
                }
            }
        }
    }
    require!(
        expected
            .iter()
            .enumerate()
            .all(|(index, edge)| seen[index] == (edge.stroke != GraphEdgeStroke::Invisible)),
        "edge: missing canonical multiplicity"
    );
    let mut crossings = BTreeSet::new();
    for (a_index, a) in scene.routes.iter().enumerate() {
        for (b_index, b) in scene.routes.iter().enumerate().skip(a_index + 1) {
            let (ea, eb) = (
                expected[a.owner.canonical_edge_index],
                expected[b.owner.canonical_edge_index],
            );
            for (coord, cell_a) in &indices[a_index] {
                if let Some(cell_b) = indices[b_index].get(coord) {
                    let transverse = matches!(
                        (
                            axes(cell_a, &indices[a_index]),
                            axes(cell_b, &indices[b_index])
                        ),
                        (1, 2) | (2, 1)
                    );
                    if transverse {
                        crossings.insert(*coord);
                    }
                    require!(
                        (transverse
                            && cell_a.kind != PlannedRouteCellKind::EdgeArrow
                            && cell_b.kind != PlannedRouteCellKind::EdgeArrow)
                            || justified_sharing(a, b, ea, eb, cell_a.coord),
                        "sharing: unrelated collinear cells between {} and {} at {coord:?}",
                        a.owner.canonical_edge_index,
                        b.owner.canonical_edge_index
                    );
                }
            }
            if ea.from == eb.from && ea.to == eb.to {
                require!(
                    indices[a_index]
                        .keys()
                        .any(|coord| !indices[b_index].contains_key(coord))
                        && indices[b_index]
                            .keys()
                            .any(|coord| !indices[a_index].contains_key(coord)),
                    "sharing: parallel multiplicity erased"
                );
            }
        }
    }
    let (width, height) = scene.canvas_extent();
    let mut canvas = RawCanvas::with_width_profile(width, height, options.terminal_width_profile);
    let mut route_cells = RouteCells::default();
    let policy = AsciiResourcePolicy::unbounded();
    scene
        .paint_routes_with_execution(
            &mut RouteDrawing::new(&mut canvas, &mut route_cells),
            AsciiExecution::for_test(&policy),
        )
        .map_err(|error| format!("paint: {error}"))?;
    scene
        .draw_labels(&mut canvas, RouteLabelTransform::Identity)
        .map_err(|error| format!("paint: {error}"))?;
    for (coord, glyph) in markers {
        require!(
            canvas.get(coord.x, coord.y) == Some(glyph),
            "marker: final painting erased {glyph:?} at {coord:?}"
        );
    }
    for (x, y) in crossings {
        let glyph = canvas.get(x, y);
        require!(
            if unicode {
                matches!(glyph, Some('╪' | '┼' | '╋' | '┿' | '╂'))
            } else {
                glyph == Some('+')
            },
            "paint: crossing erased a continuing axis at ({x}, {y}): {glyph:?}"
        );
    }
    for route in &scene.routes {
        for label in &route.plan.labels {
            let text = scene
                .labels
                .get(label.descriptor)
                .map_err(|error| format!("label: {error}"))?;
            for y in label.placement.y()..label.placement.y() + text.line_count() {
                for x in [
                    label.placement.x().checked_sub(1),
                    Some(label.placement.x() + label.placement.width()),
                ]
                .into_iter()
                .flatten()
                {
                    for (other_index, other) in scene.routes.iter().enumerate() {
                        if other.owner.canonical_edge_index == route.owner.canonical_edge_index {
                            continue;
                        }
                        let cells = &indices[other_index];
                        if let Some(cell) = cells.get(&(x, y)) {
                            require!(
                                axes(cell, cells) & 2 == 0,
                                "label: an unrelated vertical stem touches the caption"
                            );
                        }
                    }
                }
            }
            for (row, line) in text.lines().iter().enumerate() {
                // A hard-break row retains its reserved geometry and line count,
                // but has no painted text centre to observe.
                if line.is_empty() {
                    continue;
                }
                let painted: String = (label.placement.x()
                    ..label.placement.x() + label.placement.width())
                    .map(|x| canvas.get(x, label.placement.y() + row).unwrap_or(' '))
                    .collect();
                let Some(byte_offset) = painted.find(line) else {
                    return Err("label: final painting truncated text".into());
                };
                let offset = painted[..byte_offset].chars().count();
                let centre_twice = 2 * offset + line.len().saturating_sub(1);
                let host_twice = 2 * (label.placement.width() / 2);
                require!(
                    centre_twice.abs_diff(host_twice) <= 1,
                    "label: painted row left its shared host column"
                );
            }
        }
    }
    Ok(())
}

fn routing_failure(error: AsciiError) -> String {
    let class = match &error {
        AsciiError::RoutingSearchExhausted { .. } => "search-exhausted",
        AsciiError::UnsupportedFeature { .. } => "routing-unsupported",
        AsciiError::ResourceLimitExceeded(_) => "routing-resource",
        _ => "routing-unexpected",
    };
    format!("{class}: {error}")
}

#[test]
fn generated_supported_small_graphs_preserve_information() {
    let mut attempted = 0;
    let mut visible_edges = 0;
    let mut succeeded = 0;
    let mut chain_successes = 0;
    let mut failures = Vec::new();
    let mut classes = BTreeMap::new();
    for family in FAMILIES {
        for (seed, count) in SAMPLES {
            for direction in DIRECTIONS {
                let original = generated_graph(family, seed, count, direction);
                for mode in MODES {
                    for order in 0..ORDERS {
                        attempted += 1;
                        let options = options(mode);
                        let mut graph = original.clone();
                        visible_edges += graph
                            .edges
                            .iter()
                            .filter(|edge| edge.stroke != GraphEdgeStroke::Invisible)
                            .count();
                        match order {
                            0 => {}
                            1 => graph.edges.reverse(),
                            _ => Generator(seed ^ 0xa81b_2047).shuffle(&mut graph.edges),
                        }
                        // One Interactive ledger covers layout, search/repair, and materialization.
                        let mut resources = ResourceContext::new(AsciiResourcePolicy::for_profile(
                            ResourceProfile::Interactive,
                        ));
                        let result = layout_graph_with_resources(&graph, &options, &mut resources)
                            .map_err(|error| format!("layout: {error}"))
                            .and_then(|layout| {
                                let policy = resources.policy();
                                prepare_route_scene_with_execution(
                                    &graph,
                                    &layout,
                                    &graph.edges,
                                    &GraphCharset::for_options(&options),
                                    &options.flowchart_layout().graph_policy(),
                                    &mut resources,
                                    AsciiExecution::for_test(&policy),
                                )
                                .and_then(|plan| plan.materialize(&resources))
                                .map_err(routing_failure)
                                .and_then(|scene| {
                                    information_oracle(&graph, &layout, &scene, &options)
                                })
                            });
                        match result {
                            Ok(()) => {
                                succeeded += 1;
                                chain_successes += usize::from(family == Family::Chain);
                            }
                            Err(error) => {
                                let class = error.split_once(':').unwrap().0.to_owned();
                                *classes.entry(class).or_insert(0usize) += 1;
                                failures.push(format!("{family:?}/seed={seed:08x}/n={count}/{direction:?}/{mode}/order={order}: {error}"));
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(attempted, SCENE_COUNT, "failures stay in the denominator");
    assert_eq!(
        visible_edges, 3_042,
        "visible edge demand stays in the denominator"
    );
    // Baseline f09aa799d, binary SHA-256 923b76328b4c889bcdf52c5fab364a4d6c2a9a4cb42c444f10ec59a4a4d51fea:
    // all 18 Chain size/direction/mode combinations rendered diagrammatically, with each
    // node label once, n-1 directed heads, and straight continuity between adjacent nodes.
    // Three authored edge orders make 54 cases.
    // Evidence: target/ascii-routing-audit/generated-routing-baseline/results.json (local).
    // This narrow historical denominator never grants an exception to the other families.
    let report = format!(
        "{succeeded}/{attempted} valid scenes; classes={classes:?}\n{}",
        failures.join("\n")
    );
    assert_eq!(
        chain_successes, 54,
        "fixed valid-success denominator regressed: {report}"
    );
    assert!(
        failures.is_empty(),
        "supported small graphs lost information or were rejected: {report}"
    );
}

#[test]
fn generated_information_oracle_rejects_corrupted_successes() {
    let options = AsciiRenderOptions::unicode();
    let mut graph = generated_graph(Family::Chain, SAMPLES[0].0, 3, GraphDirection::LeftRight);
    graph.edges[0].label = Some("first".into());
    graph.edges[1].label = Some("next<br>ok".into());
    let layout = layout_graph(&graph, &options);
    type Mutation = (&'static str, fn(&mut RouteScene));
    let mutations: [Mutation; 6] = [
        ("edge", |scene| {
            scene.routes.pop();
        }),
        ("edge", |scene| {
            scene.routes[1].owner.canonical_edge_index = scene.routes[0].owner.canonical_edge_index;
        }),
        ("marker", |scene| {
            scene.routes[0]
                .plan
                .cells
                .iter_mut()
                .find(|cell| cell.kind == PlannedRouteCellKind::EdgeArrow)
                .unwrap()
                .ch = '◄';
        }),
        ("shape", |scene| {
            scene.routes[0]
                .plan
                .attachments
                .as_mut()
                .unwrap()
                .start
                .contact
                .x += 1;
        }),
        ("label", |scene| {
            scene.routes[0].plan.labels[0].descriptor = scene.routes[1].plan.labels[0].descriptor;
        }),
        ("stroke", |scene| {
            scene.routes[0].plan.cells[0].stroke = GraphEdgeStroke::Dotted;
        }),
    ];
    for (class, mutate) in mutations {
        let mut scene = prepare_route_scene(
            &graph,
            &layout,
            &graph.edges,
            &GraphCharset::for_options(&options),
        )
        .unwrap();
        information_oracle(&graph, &layout, &scene, &options).expect("uncorrupted control scene");
        mutate(&mut scene);
        let error = information_oracle(&graph, &layout, &scene, &options)
            .expect_err("independent oracle must reject corrupted information");
        assert!(error.starts_with(class), "expected {class}, got {error}");
    }
}

#[cfg(feature = "diagram-flowchart")]
fn assert_planned_flowchart_preserves_information(
    fixture: &str,
    source: &str,
    node_count: usize,
    edge_count: usize,
    directions: &[GraphDirection],
    options: AsciiRenderOptions,
) {
    let parsed = merman_core::SemanticParser::new()
        .parse_diagram_sync(source)
        .unwrap_or_else(|error| panic!("{fixture}: typed parse failed: {error}"));
    let merman_model::SemanticDiagramModel::Flowchart(model) = &parsed.model else {
        panic!("{fixture}: expected a typed flowchart model");
    };
    assert_eq!(
        (model.nodes.len(), model.edges.len()),
        (node_count, edge_count),
        "{fixture}: frozen authored node and edge counts"
    );
    for &direction in directions {
        for charset in [crate::AsciiCharset::Ascii, crate::AsciiCharset::Unicode] {
            let case = format!("{fixture}/{direction:?}/{charset:?}");
            let options = AsciiRenderOptions { charset, ..options };
            let policy = options.flowchart_layout().graph_policy();
            let charset = GraphCharset::for_options(&options);
            let limits = AsciiResourcePolicy::unbounded();
            let mut resources = ResourceContext::new(limits);
            let mut graph =
                crate::graph::adapter::from_flowchart_model(model, &options, &mut resources)
                    .unwrap_or_else(|error| panic!("{case}: projection failed: {error}"));
            graph.direction = direction;
            assert_eq!(
                (graph.nodes.len(), graph.edges.len()),
                (node_count, edge_count),
                "{case}: projection must retain every authored node and edge"
            );
            // Use the production planner and default internal allowances.
            let planned = crate::graph::planning::plan_graph(
                &graph,
                &policy,
                &charset,
                &mut resources,
                AsciiExecution::for_test(&limits),
            )
            .unwrap_or_else(|error| panic!("{case}: graph planning failed: {error}"));
            let scene = planned
                .routes
                .materialize(&resources)
                .unwrap_or_else(|error| panic!("{case}: route materialization failed: {error}"));
            assert_eq!(
                (planned.layout.nodes.len(), scene.routes.len()),
                (node_count, edge_count),
                "{case}: the scene must contain every authored node and edge without summaries"
            );
            information_oracle(&graph, &planned.layout, &scene, &options)
                .unwrap_or_else(|error| panic!("{case}: {error}"));
        }
    }
}

#[test]
#[cfg(feature = "diagram-flowchart")]
fn multiline_labels_crossed_branches_and_parallel_self_loops_keep_complete_painted_routes() {
    use std::fmt::Write;
    let mut loops = String::from("flowchart TD\nStart --> Queue\nQueue --> Done\n");
    for index in 1..=8 {
        writeln!(loops, "Queue -->|retry {index}| Queue").unwrap();
    }
    for (name, source, nodes, edges, directions) in [
        (
            "mixed-parity-label",
            "graph TD\nA -->|First<br>Second| B\n",
            2,
            1,
            &[GraphDirection::TopDown, GraphDirection::LeftRight][..],
        ),
        (
            "mixed-parity-with-blank-row",
            "graph TD\nA -->|First<br><br>Second| B\n",
            2,
            1,
            &[GraphDirection::TopDown, GraphDirection::LeftRight][..],
        ),
        // The authored TD case is admitted. This source's additional LR variant
        // already has a bounded planning miss at the baseline, before paint changes.
        (
            "crossed-branches",
            "graph TD\nA & B --> C & D\nC --> E\nD --> E\n",
            5,
            6,
            &[GraphDirection::TopDown][..],
        ),
        (
            "independent-caption-clearance",
            "flowchart TD\nMermaid[Mermaid.js] -->|derivative| Beautiful[Beautiful Mermaid]\nASCII[Mermaid ASCII] -.->|ASCII component port| Beautiful\n",
            3,
            2,
            &[GraphDirection::TopDown][..],
        ),
        (
            "parallel-self-loops",
            loops.as_str(),
            3,
            10,
            &[GraphDirection::TopDown, GraphDirection::LeftRight][..],
        ),
    ] {
        assert_planned_flowchart_preserves_information(
            name,
            source,
            nodes,
            edges,
            directions,
            AsciiRenderOptions::default(),
        );
    }
}

#[test]
#[cfg(feature = "diagram-flowchart")]
fn dense_minimized_planning_preserves_information() {
    assert_planned_flowchart_preserves_information(
        "dense-minimized",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/testdata/local-semantic/flowchart/scene_planning/dense-minimized.mmd"
        )),
        20,
        33,
        &[GraphDirection::TopDown, GraphDirection::LeftRight],
        AsciiRenderOptions::default(),
    );
}

#[test]
#[cfg(feature = "diagram-flowchart")]
fn dense_work_budget_component_rank_reflow_preserves_information() {
    assert_planned_flowchart_preserves_information(
        "dense-work-budget",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/testdata/local-semantic/flowchart/scene_planning/dense-work-budget.mmd"
        )),
        100,
        195,
        &[GraphDirection::TopDown, GraphDirection::LeftRight],
        AsciiRenderOptions::default(),
    );
}

#[test]
#[cfg(feature = "diagram-flowchart")]
fn capacity_recovery_keeps_every_edge_of_previously_readable_cycles() {
    for (fixture, source, edges) in [
        (
            "labelled-cycle-capacity",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/testdata/local-semantic/flowchart/scene_planning/labelled-cycle-capacity.mmd"
            )),
            12,
        ),
        (
            "narrow-cycle-capacity",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/testdata/local-semantic/flowchart/scene_planning/narrow-cycle-capacity.mmd"
            )),
            15,
        ),
    ] {
        assert_planned_flowchart_preserves_information(
            fixture,
            source,
            8,
            edges,
            &[GraphDirection::TopDown],
            AsciiRenderOptions::default(),
        );
    }
}

#[test]
#[cfg(feature = "diagram-flowchart")]
fn issue_370_fixed_routing_preserves_every_edge_and_label() {
    assert_planned_flowchart_preserves_information(
        "issue_370_terminal_height",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/testdata/local-semantic/flowchart/scene_planning/issue_370_terminal_height.mmd"
        )),
        10,
        10,
        &[GraphDirection::TopDown],
        AsciiRenderOptions::unicode()
            .with_node_padding_y(0)
            .with_graph_padding_y(2)
            .with_graph_routing_space(crate::AsciiGraphRoutingSpace::Fixed),
    );
}

#[test]
#[cfg(feature = "diagram-agentflow")]
fn agentflow_operators_preserve_independent_contacts_markers_and_paint() {
    let parsed = merman_core::SemanticParser::new().parse_diagram_sync(
            "agentflow-beta\nstart[Start] --> next[Next]\nnext -.- docs[Guide]\nnext --x fail[Failure]\n",
    ).unwrap();
    let merman_model::SemanticDiagramModel::Agentflow(model) = &parsed.model else {
        panic!("expected canonical Agentflow");
    };
    for direction in [GraphDirection::TopDown, GraphDirection::LeftRight] {
        for mode in ["ascii", "unicode"] {
            let options = options(mode);
            let limits = AsciiResourcePolicy::unbounded();
            let mut resources = ResourceContext::new(limits);
            let mut graph =
                crate::agentflow::project(model, options.terminal_width_profile, &resources)
                    .unwrap();
            graph.direction = direction;
            assert_eq!((graph.nodes.len(), graph.edges.len()), (4, 3));
            assert_eq!(graph.edges[0].end_marker, GraphEdgeMarker::Point);
            assert_eq!(
                (graph.edges[1].stroke, graph.edges[1].end_marker),
                (GraphEdgeStroke::Dotted, GraphEdgeMarker::Open)
            );
            assert_eq!(graph.edges[2].end_marker, GraphEdgeMarker::Cross);
            let planned = crate::graph::planning::plan_graph(
                &graph,
                &options.flowchart_layout().graph_policy(),
                &GraphCharset::for_options(&options),
                &mut resources,
                AsciiExecution::for_test(&limits),
            )
            .unwrap();
            let scene = planned.routes.materialize(&resources).unwrap();
            information_oracle(&graph, &planned.layout, &scene, &options).unwrap();
        }
    }
}

#[test]
#[cfg(feature = "diagram-requirement")]
fn requirement_shared_endpoints_and_typed_markers_pass_the_independent_oracle() {
    let parsed = merman_core::SemanticParser::new()
        .parse_diagram_sync(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/requirement/upstream_docs_requirementdiagram_larger_example_010.mmd"
        )))
        .unwrap();
    let merman_model::SemanticDiagramModel::Requirement(model) = &parsed.model else {
        panic!("expected Requirement");
    };
    for direction in [GraphDirection::TopDown, GraphDirection::LeftRight] {
        for mode in ["ascii", "unicode"] {
            let options = options(mode);
            let limits = AsciiResourcePolicy::default();
            let mut resources = ResourceContext::new(limits);
            let mut graph =
                crate::requirement::project(model, options.terminal_width_profile, &resources)
                    .unwrap();
            graph.direction = direction;
            assert_eq!((graph.nodes.len(), graph.edges.len()), (9, 8));
            assert_eq!((&*graph.edges[7].from, &*graph.edges[7].to), ("N2", "R1"));
            assert_eq!(graph.edges.iter().filter(|edge| edge.to == "R2").count(), 2);
            assert_eq!(graph.edges.iter().filter(|edge| edge.to == "R5").count(), 2);
            for (edge, authored) in graph.edges.iter().zip(&model.relationships) {
                assert_eq!(edge.label.as_deref(), Some(authored.rel_type.as_str()));
                if authored.rel_type == "contains" {
                    assert_eq!(
                        (edge.stroke, edge.start_marker, edge.end_marker),
                        (
                            GraphEdgeStroke::Normal,
                            GraphEdgeMarker::Circle,
                            GraphEdgeMarker::Open
                        )
                    );
                } else {
                    assert_eq!(
                        (edge.stroke, edge.start_marker, edge.end_marker),
                        (
                            GraphEdgeStroke::Dotted,
                            GraphEdgeMarker::Open,
                            GraphEdgeMarker::Point
                        )
                    );
                }
            }
            let planned = crate::graph::planning::plan_graph(
                &graph,
                &options.flowchart_layout().graph_policy(),
                &GraphCharset::for_options(&options),
                &mut resources,
                AsciiExecution::for_test(&limits),
            )
            .unwrap();
            let scene = planned.routes.materialize(&resources).unwrap();
            information_oracle(&graph, &planned.layout, &scene, &options).unwrap();
        }
    }
}
