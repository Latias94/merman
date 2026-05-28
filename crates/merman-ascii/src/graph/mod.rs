use crate::canvas::Canvas;
use crate::error::{AsciiError, Result};
use crate::options::{AsciiCharset, AsciiRenderOptions};
use crate::text::display_width;
use merman_core::diagrams::flowchart::FlowchartV2Model;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GraphDirection {
    LeftRight,
    TopDown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AsciiGraph {
    direction: GraphDirection,
    nodes: Vec<AsciiGraphNode>,
    edges: Vec<AsciiGraphEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AsciiGraphNode {
    id: String,
    label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AsciiGraphEdge {
    from: String,
    to: String,
}

impl AsciiGraph {
    pub(crate) fn new(direction: GraphDirection) -> Self {
        Self {
            direction,
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    pub(crate) fn add_node(&mut self, id: impl Into<String>, label: impl Into<String>) {
        self.nodes.push(AsciiGraphNode {
            id: id.into(),
            label: label.into(),
        });
    }

    pub(crate) fn add_edge(&mut self, from: impl Into<String>, to: impl Into<String>) {
        self.edges.push(AsciiGraphEdge {
            from: from.into(),
            to: to.into(),
        });
    }
}

pub(crate) fn from_flowchart_model(
    model: &FlowchartV2Model,
    options: &AsciiRenderOptions,
) -> Result<AsciiGraph> {
    validate_supported_flowchart_model(model)?;

    let direction = if let Some(direction) = model.direction.as_deref() {
        parse_direction(direction)?
    } else {
        match options.fallback_direction {
            crate::AsciiDirection::LeftRight => GraphDirection::LeftRight,
            crate::AsciiDirection::TopDown => GraphDirection::TopDown,
        }
    };
    let mut graph = AsciiGraph::new(direction);

    for node in &model.nodes {
        graph.add_node(&node.id, node.label.as_deref().unwrap_or(&node.id));
    }

    for edge in &model.edges {
        graph.add_edge(&edge.from, &edge.to);
    }

    Ok(graph)
}

fn parse_direction(direction: &str) -> Result<GraphDirection> {
    match direction {
        "LR" => Ok(GraphDirection::LeftRight),
        "TB" | "TD" => Ok(GraphDirection::TopDown),
        _ => Err(AsciiError::UnsupportedFeature {
            diagram_type: "flowchart",
            feature: "non-LR/TD graph directions",
        }),
    }
}

fn validate_supported_flowchart_model(model: &FlowchartV2Model) -> Result<()> {
    if !model.subgraphs.is_empty() {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "flowchart",
            feature: "subgraphs",
        });
    }

    if model.nodes.iter().any(|node| {
        node.label
            .as_deref()
            .is_some_and(|label| label.contains('\n'))
    }) {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "flowchart",
            feature: "multiline node labels",
        });
    }

    if model.nodes.iter().any(|node| {
        node.layout_shape
            .as_deref()
            .is_some_and(|shape| !matches!(shape, "rect" | "rectangle" | "square" | "squareRect"))
    }) {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "flowchart",
            feature: "non-rectangular node shapes",
        });
    }

    if model.edges.iter().any(|edge| {
        edge.label
            .as_deref()
            .is_some_and(|label| !label.trim().is_empty())
    }) {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "flowchart",
            feature: "edge labels",
        });
    }

    if model.edges.iter().any(|edge| edge.length != 1) {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "flowchart",
            feature: "edge length modifiers",
        });
    }

    if model.edges.iter().any(|edge| {
        edge.stroke
            .as_deref()
            .is_some_and(|stroke| stroke != "normal")
    }) {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "flowchart",
            feature: "non-normal edge strokes",
        });
    }

    if model.edges.iter().any(|edge| {
        edge.edge_type
            .as_deref()
            .is_some_and(|edge_type| edge_type != "arrow_point")
    }) {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "flowchart",
            feature: "non-point edge arrows",
        });
    }

    let node_ids = model
        .nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<HashSet<_>>();
    if model
        .edges
        .iter()
        .any(|edge| !node_ids.contains(edge.from.as_str()) || !node_ids.contains(edge.to.as_str()))
    {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "flowchart",
            feature: "edges with missing endpoint nodes",
        });
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GraphCharset {
    top_left: char,
    top_right: char,
    bottom_left: char,
    bottom_right: char,
    horizontal: char,
    vertical: char,
    right_connector: char,
    arrow_right: char,
    arrow_down: char,
}

impl GraphCharset {
    fn for_options(options: &AsciiRenderOptions) -> Self {
        match options.charset {
            AsciiCharset::Ascii => Self {
                top_left: '+',
                top_right: '+',
                bottom_left: '+',
                bottom_right: '+',
                horizontal: '-',
                vertical: '|',
                right_connector: '|',
                arrow_right: '>',
                arrow_down: 'v',
            },
            AsciiCharset::Unicode => Self {
                top_left: '┌',
                top_right: '┐',
                bottom_left: '└',
                bottom_right: '┘',
                horizontal: '─',
                vertical: '│',
                right_connector: '├',
                arrow_right: '►',
                arrow_down: '▼',
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NodeLayout {
    id: String,
    label: String,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

impl NodeLayout {
    fn center_x(&self) -> usize {
        self.x + self.width / 2
    }

    fn center_y(&self) -> usize {
        self.y + self.height / 2
    }

    fn right(&self) -> usize {
        self.x + self.width - 1
    }

    fn bottom(&self) -> usize {
        self.y + self.height - 1
    }
}

pub(crate) fn render_graph(graph: &AsciiGraph, options: &AsciiRenderOptions) -> Result<String> {
    options.validate()?;
    if graph.nodes.is_empty() {
        return Ok(String::new());
    }

    let charset = GraphCharset::for_options(options);
    let layouts = layout_nodes(graph, options);
    let width = layouts
        .iter()
        .map(|layout| layout.x + layout.width)
        .max()
        .unwrap_or_default();
    let height = layouts
        .iter()
        .map(|layout| layout.y + layout.height)
        .max()
        .unwrap_or_default();
    let actual_cells = width.saturating_mul(height);
    if actual_cells > options.max_grid_cells {
        return Err(AsciiError::RenderLimitExceeded {
            actual: actual_cells,
            limit: options.max_grid_cells,
        });
    }

    let mut canvas = Canvas::new(width, height);
    for layout in &layouts {
        draw_node(&mut canvas, layout, &charset, options);
    }
    for edge in &graph.edges {
        draw_edge(&mut canvas, &layouts, edge, graph.direction, &charset);
    }

    Ok(canvas.finish())
}

fn layout_nodes(graph: &AsciiGraph, options: &AsciiRenderOptions) -> Vec<NodeLayout> {
    let measured = graph
        .nodes
        .iter()
        .map(|node| {
            let width = display_width(&node.label) + options.box_border_padding * 2 + 2;
            let height = 1 + options.box_border_padding * 2 + 2;
            (node, width, height)
        })
        .collect::<Vec<_>>();

    match graph.direction {
        GraphDirection::LeftRight => {
            let mut x = 0;
            measured
                .into_iter()
                .map(|(node, width, height)| {
                    let layout = NodeLayout {
                        id: node.id.clone(),
                        label: node.label.clone(),
                        x,
                        y: 0,
                        width,
                        height,
                    };
                    x += width + options.graph_padding_x;
                    layout
                })
                .collect()
        }
        GraphDirection::TopDown => {
            let canvas_width = measured
                .iter()
                .map(|(_, width, _)| *width)
                .max()
                .unwrap_or_default();
            let mut y = 0;
            measured
                .into_iter()
                .map(|(node, width, height)| {
                    let layout = NodeLayout {
                        id: node.id.clone(),
                        label: node.label.clone(),
                        x: (canvas_width - width) / 2,
                        y,
                        width,
                        height,
                    };
                    y += height + options.graph_padding_y;
                    layout
                })
                .collect()
        }
    }
}

fn draw_node(
    canvas: &mut Canvas,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) {
    let right = layout.right();
    let bottom = layout.bottom();

    canvas.set(layout.x, layout.y, charset.top_left);
    canvas.set(right, layout.y, charset.top_right);
    canvas.set(layout.x, bottom, charset.bottom_left);
    canvas.set(right, bottom, charset.bottom_right);

    for x in (layout.x + 1)..right {
        canvas.set(x, layout.y, charset.horizontal);
        canvas.set(x, bottom, charset.horizontal);
    }

    for y in (layout.y + 1)..bottom {
        canvas.set(layout.x, y, charset.vertical);
        canvas.set(right, y, charset.vertical);
    }

    let text_x = layout.x + 1 + options.box_border_padding;
    let text_y = layout.y + 1 + options.box_border_padding;
    canvas.write_text(text_x, text_y, &layout.label);
}

fn draw_edge(
    canvas: &mut Canvas,
    layouts: &[NodeLayout],
    edge: &AsciiGraphEdge,
    direction: GraphDirection,
    charset: &GraphCharset,
) {
    let Some(from) = layouts.iter().find(|layout| layout.id == edge.from) else {
        return;
    };
    let Some(to) = layouts.iter().find(|layout| layout.id == edge.to) else {
        return;
    };

    match direction {
        GraphDirection::LeftRight => draw_left_right_edge(canvas, from, to, charset),
        GraphDirection::TopDown => draw_top_down_edge(canvas, from, to, charset),
    }
}

fn draw_left_right_edge(
    canvas: &mut Canvas,
    from: &NodeLayout,
    to: &NodeLayout,
    charset: &GraphCharset,
) {
    if to.x <= from.right() + 1 {
        return;
    }

    let y = from.center_y();
    canvas.set(from.right(), y, charset.right_connector);
    let start = from.right() + 1;
    let end = to.x - 1;
    for x in start..end {
        canvas.set(x, y, charset.horizontal);
    }
    canvas.set(end, y, charset.arrow_right);
}

fn draw_top_down_edge(
    canvas: &mut Canvas,
    from: &NodeLayout,
    to: &NodeLayout,
    charset: &GraphCharset,
) {
    if to.y <= from.bottom() + 1 {
        return;
    }

    let x = from.center_x();
    let start = from.bottom() + 1;
    let end = to.y - 1;
    for y in start..end {
        canvas.set(x, y, charset.vertical);
    }
    canvas.set(x, end, charset.arrow_down);
}

#[cfg(test)]
mod graph_golden {
    use super::*;
    use crate::AsciiRenderOptions;
    use std::path::Path;

    fn fixture_expected(directory: &str, name: &str) -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/testdata/mermaid-ascii")
            .join(directory)
            .join(name);
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
            .replace("\r\n", "\n");
        let (_, expected) = content
            .split_once("\n---\n")
            .unwrap_or_else(|| panic!("fixture missing separator: {}", path.display()));
        expected.to_string()
    }

    #[test]
    fn single_node_ascii_matches_upstream_golden() {
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        graph.add_node("A", "A");

        let actual = render_graph(&graph, &AsciiRenderOptions::ascii()).unwrap();

        assert_eq!(actual, fixture_expected("ascii", "single_node.txt"));
    }

    #[test]
    fn single_node_unicode_matches_upstream_golden() {
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        graph.add_node("A", "A");

        let actual = render_graph(&graph, &AsciiRenderOptions::unicode()).unwrap();

        assert_eq!(
            actual,
            fixture_expected("extended-chars", "single_node.txt")
        );
    }

    #[test]
    fn two_nodes_linked_ascii_matches_upstream_golden() {
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        graph.add_node("A", "A");
        graph.add_node("B", "B");
        graph.add_edge("A", "B");

        let actual = render_graph(&graph, &AsciiRenderOptions::ascii()).unwrap();

        assert_eq!(actual, fixture_expected("ascii", "two_nodes_linked.txt"));
    }

    #[test]
    fn two_nodes_linked_unicode_matches_upstream_golden() {
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        graph.add_node("A", "A");
        graph.add_node("B", "B");
        graph.add_edge("A", "B");

        let actual = render_graph(&graph, &AsciiRenderOptions::unicode()).unwrap();

        assert_eq!(
            actual,
            fixture_expected("extended-chars", "two_nodes_linked.txt")
        );
    }

    #[test]
    fn long_node_labels_ascii_match_upstream_golden() {
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        graph.add_node("LongerName1", "LongerName1");
        graph.add_node("LongerName2", "LongerName2");
        graph.add_edge("LongerName1", "LongerName2");

        let actual = render_graph(&graph, &AsciiRenderOptions::ascii()).unwrap();

        assert_eq!(
            actual,
            fixture_expected("ascii", "two_nodes_longer_names.txt")
        );
    }

    #[test]
    fn top_down_chain_ascii_matches_upstream_golden() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("A", "A");
        graph.add_node("B", "B");
        graph.add_node("C", "C");
        graph.add_edge("A", "B");
        graph.add_edge("B", "C");

        let actual = render_graph(&graph, &AsciiRenderOptions::ascii()).unwrap();

        assert_eq!(actual, fixture_expected("ascii", "flowchart_tb_simple.txt"));
    }
}
