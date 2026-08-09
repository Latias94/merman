use std::collections::{BTreeMap, BTreeSet, HashMap};

use merman_core::RenderSemanticModel;
use merman_core::diagrams::ishikawa::{IshikawaDiagramRenderModel, IshikawaNodeRenderModel};
use merman_core::diagrams::quadrant_chart::QuadrantChartRenderModel;
use merman_core::diagrams::railroad::{
    RailroadAstNode, RailroadDiagramRenderModel, RailroadRepeatBound,
};
use merman_core::diagrams::requirement::RequirementDiagramRenderModel;
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateKind {
    Railroad,
    Requirement,
    Ishikawa,
    Quadrant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scenario {
    Small,
    Typical,
    Dense,
}

impl Scenario {
    pub const ALL: [Self; 3] = [Self::Small, Self::Typical, Self::Dense];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Small => "Small",
            Self::Typical => "Typical",
            Self::Dense => "Dense",
        }
    }
}

#[derive(Debug)]
pub struct PrototypeObservation {
    pub output: String,
    pub expected_spatial_facts: usize,
    pub recovered_spatial_facts: usize,
    pub structured_text_facts: usize,
    pub topology_recoverable: bool,
    pub information_gain: bool,
    pub clipped_lines: usize,
    pub diagnostic: String,
}

impl PrototypeObservation {
    pub fn max_line_width(&self) -> usize {
        self.output
            .lines()
            .map(UnicodeWidthStr::width)
            .max()
            .unwrap_or(0)
    }
}

pub fn evaluate(
    kind: CandidateKind,
    model: &RenderSemanticModel,
    width: usize,
) -> PrototypeObservation {
    match (kind, model) {
        (CandidateKind::Railroad, RenderSemanticModel::Railroad(model)) => {
            evaluate_railroad(model, width)
        }
        (CandidateKind::Requirement, RenderSemanticModel::Requirement(model)) => {
            evaluate_requirement(model, width)
        }
        (CandidateKind::Ishikawa, RenderSemanticModel::Ishikawa(model)) => {
            evaluate_ishikawa(model, width)
        }
        (CandidateKind::Quadrant, RenderSemanticModel::QuadrantChart(model)) => {
            evaluate_quadrant(model, width)
        }
        _ => panic!("candidate kind does not match typed render model"),
    }
}

fn evaluate_railroad(model: &RailroadDiagramRenderModel, width: usize) -> PrototypeObservation {
    let mut raw_lines = Vec::new();
    let mut route_count = 0;
    let mut expected_operators = 0;
    let mut recovered_operators = 0;

    for rule in &model.rules {
        raw_lines.push(format!("{}:", rule.name));
        let routes = railroad_routes(&rule.definition);
        route_count += routes.len();
        let (operators, spatial_operators) = railroad_operator_counts(&rule.definition);
        expected_operators += operators;
        recovered_operators += spatial_operators;

        for (index, route) in routes.iter().enumerate() {
            let branch = if index + 1 == routes.len() {
                "`--"
            } else {
                "+--"
            };
            raw_lines.push(format!("  {branch}o--{route}--o"));
        }
    }

    let (output, clipped_lines) = fit_lines(raw_lines, width);
    let route_expansion_scannable = route_count <= 12;
    let topology_recoverable = expected_operators == recovered_operators && clipped_lines == 0;
    let information_gain = topology_recoverable && route_count > 1 && route_expansion_scannable;
    let diagnostic = if expected_operators != recovered_operators {
        let mut diagnostic = format!(
            "{recovered_operators}/{expected_operators} choice/optional/repetition operators are spatial; repetition remains a text token"
        );
        if !route_expansion_scannable {
            diagnostic.push_str(&format!(
                "; {route_count} route rows also duplicate shared prefixes"
            ));
        }
        diagnostic
    } else if !route_expansion_scannable {
        format!(
            "{route_count} expanded route rows duplicate shared prefixes; the field-complete grammar report is shorter"
        )
    } else if route_count <= 1 {
        "one linear route adds no scan advantage over the grammar record".to_string()
    } else if clipped_lines > 0 {
        format!("{clipped_lines} route rows were clipped")
    } else {
        format!("{route_count} connected route alternatives remain scannable")
    };

    PrototypeObservation {
        output,
        expected_spatial_facts: expected_operators,
        recovered_spatial_facts: recovered_operators,
        structured_text_facts: expected_operators,
        topology_recoverable,
        information_gain,
        clipped_lines,
        diagnostic,
    }
}

fn railroad_routes(node: &RailroadAstNode) -> Vec<String> {
    match node {
        RailroadAstNode::Terminal { value, .. } => vec![format!("[{value}]")],
        RailroadAstNode::NonTerminal { name, .. } => vec![format!("<{name}>")],
        RailroadAstNode::Special { text, .. } => vec![format!("{{{text}}}")],
        RailroadAstNode::Sequence { elements, .. } => {
            let mut paths = vec![String::new()];
            for element in elements {
                let element_paths = railroad_routes(element);
                let mut combined = Vec::new();
                for prefix in &paths {
                    for suffix in &element_paths {
                        combined.push(join_rail_segments(prefix, suffix));
                    }
                }
                paths = combined;
            }
            paths
        }
        RailroadAstNode::Choice { alternatives, .. } => alternatives
            .iter()
            .flat_map(railroad_routes)
            .collect::<Vec<_>>(),
        RailroadAstNode::Optional { element, .. } => {
            let mut paths = vec!["bypass".to_string()];
            paths.extend(railroad_routes(element));
            paths
        }
        RailroadAstNode::Repetition {
            element,
            min,
            max,
            separator,
            ..
        } => {
            let separator = separator
                .as_deref()
                .map(railroad_routes)
                .and_then(|routes| routes.into_iter().next())
                .map(|route| format!(";sep={route}"))
                .unwrap_or_default();
            railroad_routes(element)
                .into_iter()
                .map(|route| {
                    format!(
                        "loop[{}..{}]({route}{separator})",
                        repeat_bound(*min),
                        repeat_bound(*max)
                    )
                })
                .collect()
        }
    }
}

fn join_rail_segments(left: &str, right: &str) -> String {
    if left.is_empty() {
        right.to_string()
    } else if right.is_empty() {
        left.to_string()
    } else {
        format!("{left}--{right}")
    }
}

fn repeat_bound(bound: RailroadRepeatBound) -> String {
    if bound.is_infinite() {
        "*".to_string()
    } else {
        bound.as_f64().to_string()
    }
}

fn railroad_operator_counts(node: &RailroadAstNode) -> (usize, usize) {
    match node {
        RailroadAstNode::Terminal { .. }
        | RailroadAstNode::NonTerminal { .. }
        | RailroadAstNode::Special { .. } => (0, 0),
        RailroadAstNode::Sequence { elements, .. } => elements
            .iter()
            .map(railroad_operator_counts)
            .fold((0, 0), add_counts),
        RailroadAstNode::Choice { alternatives, .. } => alternatives
            .iter()
            .map(railroad_operator_counts)
            .fold((1, 1), add_counts),
        RailroadAstNode::Optional { element, .. } => {
            add_counts((1, 1), railroad_operator_counts(element))
        }
        RailroadAstNode::Repetition {
            element, separator, ..
        } => {
            let element_counts = railroad_operator_counts(element);
            let separator_counts = separator
                .as_deref()
                .map(railroad_operator_counts)
                .unwrap_or((0, 0));
            let nested = add_counts(element_counts, separator_counts);
            (nested.0 + 1, nested.1)
        }
    }
}

fn add_counts(left: (usize, usize), right: (usize, usize)) -> (usize, usize) {
    (left.0 + right.0, left.1 + right.1)
}

fn evaluate_requirement(
    model: &RequirementDiagramRenderModel,
    width: usize,
) -> PrototypeObservation {
    let node_names = model
        .requirements
        .iter()
        .map(|node| node.name.as_str())
        .chain(model.elements.iter().map(|node| node.name.as_str()))
        .collect::<Vec<_>>();
    let mut indegree = node_names
        .iter()
        .map(|name| (*name, 0usize))
        .collect::<HashMap<_, _>>();
    for relationship in &model.relationships {
        *indegree.entry(relationship.dst.as_str()).or_default() += 1;
    }

    let mut roots = node_names
        .iter()
        .copied()
        .filter(|name| indegree.get(name).copied().unwrap_or(0) == 0)
        .collect::<Vec<_>>();
    if roots.is_empty() {
        roots.extend(node_names.iter().copied());
    }

    let mut raw_lines = Vec::new();
    let mut placements = HashMap::<String, usize>::new();
    let mut rendered_edges = 0;
    let mut placed_roots = BTreeSet::new();
    for root in roots {
        if !placed_roots.insert(root) {
            continue;
        }
        raw_lines.push(format!("[{root}]"));
        *placements.entry(root.to_string()).or_default() += 1;
        let mut ancestry = vec![root.to_string()];
        render_requirement_children(
            model,
            root,
            "",
            &mut ancestry,
            &mut raw_lines,
            &mut placements,
            &mut rendered_edges,
        );
    }

    let placed = placements.keys().cloned().collect::<BTreeSet<_>>();
    for node in node_names {
        if !placed.contains(node) {
            raw_lines.push(format!("[{node}]"));
            *placements.entry(node.to_string()).or_default() += 1;
        }
    }

    let duplicate_names = placements
        .iter()
        .filter(|(_, count)| **count > 1)
        .map(|(name, _)| name.clone())
        .collect::<BTreeSet<_>>();
    let duplicate_placements = placements
        .values()
        .map(|count| count.saturating_sub(1))
        .sum::<usize>();
    let (output, clipped_lines) = fit_lines(raw_lines, width);
    let topology_recoverable = rendered_edges == model.relationships.len()
        && duplicate_placements == 0
        && clipped_lines == 0;
    let information_gain = topology_recoverable && rendered_edges >= 2;
    let diagnostic = if duplicate_placements > 0 {
        format!(
            "{rendered_edges}/{} edges are present, but {duplicate_placements} repeated placements ({}) hide global node identity",
            model.relationships.len(),
            duplicate_names.into_iter().collect::<Vec<_>>().join(", ")
        )
    } else if rendered_edges <= 1 {
        format!("{rendered_edges} relation adds no scan advantage over the typed edge record")
    } else if clipped_lines > 0 {
        format!("{clipped_lines} relation rows were clipped")
    } else {
        format!("{rendered_edges} relations form one recoverable node-owned projection")
    };

    PrototypeObservation {
        output,
        expected_spatial_facts: model.relationships.len(),
        recovered_spatial_facts: rendered_edges,
        structured_text_facts: model.relationships.len(),
        topology_recoverable,
        information_gain,
        clipped_lines,
        diagnostic,
    }
}

#[allow(clippy::too_many_arguments)]
fn render_requirement_children(
    model: &RequirementDiagramRenderModel,
    source: &str,
    prefix: &str,
    ancestry: &mut Vec<String>,
    lines: &mut Vec<String>,
    placements: &mut HashMap<String, usize>,
    rendered_edges: &mut usize,
) {
    let outgoing = model
        .relationships
        .iter()
        .filter(|relationship| relationship.src == source)
        .collect::<Vec<_>>();
    for (index, relationship) in outgoing.iter().enumerate() {
        let last = index + 1 == outgoing.len();
        let branch = if last { "`--" } else { "+--" };
        let repeated = placements
            .get(&relationship.dst)
            .copied()
            .unwrap_or_default()
            > 0;
        let repeat_suffix = if repeated { " (repeat)" } else { "" };
        lines.push(format!(
            "{prefix}{branch} {} --> [{}]{repeat_suffix}",
            relationship.rel_type, relationship.dst,
        ));
        *rendered_edges += 1;
        *placements.entry(relationship.dst.clone()).or_default() += 1;

        if ancestry.contains(&relationship.dst) {
            lines.push(format!("{prefix}    [cycle to {}]", relationship.dst));
            continue;
        }
        if repeated {
            continue;
        }
        ancestry.push(relationship.dst.clone());
        let child_prefix = format!("{prefix}{}", if last { "    " } else { "|   " });
        render_requirement_children(
            model,
            &relationship.dst,
            &child_prefix,
            ancestry,
            lines,
            placements,
            rendered_edges,
        );
        ancestry.pop();
    }
}

fn evaluate_ishikawa(model: &IshikawaDiagramRenderModel, width: usize) -> PrototypeObservation {
    let Some(root) = &model.root else {
        return PrototypeObservation {
            output: "[empty effect]".to_string(),
            expected_spatial_facts: 0,
            recovered_spatial_facts: 0,
            structured_text_facts: 0,
            topology_recoverable: false,
            information_gain: false,
            clipped_lines: 0,
            diagnostic: "the typed model has no effect root".to_string(),
        };
    };

    let effect = format!("[{}]", root.text);
    let effect_width = UnicodeWidthStr::width(effect.as_str());
    let spine_width = width.saturating_sub(effect_width + 4).clamp(20, 64);
    let mut raw_lines = Vec::new();
    let top = root.children.iter().step_by(2).collect::<Vec<_>>();
    let bottom = root.children.iter().skip(1).step_by(2).collect::<Vec<_>>();

    for branch in top {
        raw_lines.push(right_align(
            &ishikawa_branch_label(branch, '/'),
            spine_width,
        ));
        raw_lines.push(right_align("\\", spine_width));
    }
    raw_lines.push(format!("{}=> {effect}", "=".repeat(spine_width)));
    for branch in bottom {
        raw_lines.push(right_align("/", spine_width));
        raw_lines.push(right_align(
            &ishikawa_branch_label(branch, '\\'),
            spine_width,
        ));
    }

    let expected_edges = ishikawa_edge_count(root);
    let recovered_edges = root.children.len()
        + root
            .children
            .iter()
            .map(|child| child.children.len())
            .sum::<usize>();
    if recovered_edges < expected_edges {
        raw_lines.push(format!(
            "[prototype omits {} deeper edge(s)]",
            expected_edges - recovered_edges
        ));
    }

    let (output, clipped_lines) = fit_lines(raw_lines, width);
    let topology_recoverable = recovered_edges == expected_edges && clipped_lines == 0;
    let information_gain = topology_recoverable && expected_edges >= 3;
    let diagnostic = if recovered_edges < expected_edges {
        format!(
            "{recovered_edges}/{expected_edges} parent-child edges are connected; descendants below depth two lose ownership"
        )
    } else if expected_edges <= 1 {
        "one cause/effect edge adds no scan advantage over the two-line outline".to_string()
    } else if clipped_lines > 0 {
        format!("{clipped_lines} fishbone rows were clipped")
    } else {
        format!("all {expected_edges} cause edges are connected around the effect spine")
    };

    PrototypeObservation {
        output,
        expected_spatial_facts: expected_edges,
        recovered_spatial_facts: recovered_edges,
        structured_text_facts: expected_edges,
        topology_recoverable,
        information_gain,
        clipped_lines,
        diagnostic,
    }
}

fn ishikawa_branch_label(node: &IshikawaNodeRenderModel, slash: char) -> String {
    if node.children.is_empty() {
        return node.text.clone();
    }
    let children = node
        .children
        .iter()
        .map(|child| child.text.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    format!("{children} {slash} {}", node.text)
}

fn ishikawa_edge_count(root: &IshikawaNodeRenderModel) -> usize {
    let mut count = 0;
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        count += node.children.len();
        stack.extend(node.children.iter());
    }
    count
}

fn evaluate_quadrant(model: &QuadrantChartRenderModel, width: usize) -> PrototypeObservation {
    let plot_width = match width {
        0..=89 => 28,
        90..=109 => 38,
        _ => 48,
    };
    let plot_height = 9;
    let mut cells = BTreeMap::<(usize, usize), Vec<usize>>::new();
    for (index, point) in model.points.iter().enumerate() {
        let x = quantize(point.x, plot_width);
        let y = plot_height - 1 - quantize(point.y, plot_height);
        cells.entry((x, y)).or_default().push(index);
    }

    let mut grid = vec![vec![' '; plot_width]; plot_height];
    let center_x = (plot_width - 1) / 2;
    let center_y = (plot_height - 1) / 2;
    for row in &mut grid {
        row[center_x] = '|';
    }
    for cell in &mut grid[center_y] {
        *cell = '-';
    }
    grid[center_y][center_x] = '+';
    for (&(x, y), point_indexes) in &cells {
        grid[y][x] = if point_indexes.len() == 1 {
            point_marker(point_indexes[0])
        } else {
            '*'
        };
    }

    let mut raw_lines = Vec::new();
    if let Some(title) = &model.title {
        raw_lines.push(title.clone());
    }
    raw_lines.push(format!(
        "Q2 {} | Q1 {}",
        model.quadrants.quadrant2_text, model.quadrants.quadrant1_text
    ));
    raw_lines.push(format!(
        "y: {} -> {}",
        model.axes.y_axis_bottom_text, model.axes.y_axis_top_text
    ));
    raw_lines.push(format!("+{}+", "-".repeat(plot_width)));
    for row in grid {
        raw_lines.push(format!("|{}|", row.into_iter().collect::<String>()));
    }
    raw_lines.push(format!("+{}+", "-".repeat(plot_width)));
    raw_lines.push(format!(
        "x: {} -> {}",
        model.axes.x_axis_left_text, model.axes.x_axis_right_text
    ));
    raw_lines.push(format!(
        "Q3 {} | Q4 {}",
        model.quadrants.quadrant3_text, model.quadrants.quadrant4_text
    ));
    raw_lines.push("points (exact):".to_string());
    for (index, point) in model.points.iter().enumerate() {
        raw_lines.push(format!(
            "{} {} x={:.3} y={:.3} {}",
            point_marker(index),
            point.text,
            point.x,
            point.y,
            quadrant_for(point.x, point.y)
        ));
    }

    let occupied_cells = cells.len();
    let collided_points = cells
        .values()
        .filter(|indexes| indexes.len() > 1)
        .map(Vec::len)
        .sum::<usize>();
    let (output, clipped_lines) = fit_lines(raw_lines, width);
    let topology_recoverable = occupied_cells == model.points.len() && clipped_lines == 0;
    let information_gain = topology_recoverable && model.points.len() >= 3;
    let diagnostic = if collided_points > 0 {
        format!(
            "{occupied_cells}/{} distinct grid positions remain; {collided_points} points collide and their relative order is recoverable only from the exact table",
            model.points.len()
        )
    } else if model.points.len() < 3 {
        "fewer than three points add no useful positional pattern".to_string()
    } else if clipped_lines > 0 {
        format!("{clipped_lines} plot or disclosure rows were clipped")
    } else {
        format!(
            "all {} point positions and quadrants remain spatially distinct",
            model.points.len()
        )
    };

    PrototypeObservation {
        output,
        expected_spatial_facts: model.points.len(),
        recovered_spatial_facts: occupied_cells,
        structured_text_facts: model.points.len(),
        topology_recoverable,
        information_gain,
        clipped_lines,
        diagnostic,
    }
}

fn quantize(value: f64, cells: usize) -> usize {
    let last = cells.saturating_sub(1);
    (value.clamp(0.0, 1.0) * last as f64).round() as usize
}

fn point_marker(index: usize) -> char {
    const MARKERS: &[u8] = b"123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    MARKERS.get(index).copied().map(char::from).unwrap_or('?')
}

fn quadrant_for(x: f64, y: f64) -> &'static str {
    match (x >= 0.5, y >= 0.5) {
        (true, true) => "Q1",
        (false, true) => "Q2",
        (false, false) => "Q3",
        (true, false) => "Q4",
    }
}

fn right_align(text: &str, width: usize) -> String {
    let text_width = UnicodeWidthStr::width(text);
    format!("{}{text}", " ".repeat(width.saturating_sub(text_width)))
}

fn fit_lines(lines: Vec<String>, width: usize) -> (String, usize) {
    let mut clipped = 0;
    let lines = lines
        .into_iter()
        .map(|line| {
            if UnicodeWidthStr::width(line.as_str()) <= width {
                line
            } else {
                clipped += 1;
                clip_ascii(&line, width)
            }
        })
        .collect::<Vec<_>>();
    (lines.join("\n"), clipped)
}

fn clip_ascii(line: &str, width: usize) -> String {
    const SUFFIX: &str = "...[cut]";
    if width <= SUFFIX.len() {
        return SUFFIX[..width].to_string();
    }
    let keep = width - SUFFIX.len();
    let mut output = line.chars().take(keep).collect::<String>();
    output.push_str(SUFFIX);
    output
}
