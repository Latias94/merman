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
    groups: Vec<AsciiGraphGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AsciiGraphNode {
    id: String,
    label: String,
    shape: GraphNodeShape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GraphNodeShape {
    Rect,
    Rounded,
    Diamond,
    Subroutine,
    Cylinder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AsciiGraphEdge {
    from: String,
    to: String,
    label: Option<String>,
    stroke: GraphEdgeStroke,
    arrow: GraphEdgeArrow,
    length: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AsciiGraphGroup {
    title: String,
    nodes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GraphEdgeStroke {
    Normal,
    Dotted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GraphEdgeArrow {
    Open,
    Point,
}

impl AsciiGraph {
    pub(crate) fn new(direction: GraphDirection) -> Self {
        Self {
            direction,
            nodes: Vec::new(),
            edges: Vec::new(),
            groups: Vec::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn add_node(&mut self, id: impl Into<String>, label: impl Into<String>) {
        self.add_node_with_shape(id, label, GraphNodeShape::Rect);
    }

    fn add_node_with_shape(
        &mut self,
        id: impl Into<String>,
        label: impl Into<String>,
        shape: GraphNodeShape,
    ) {
        self.nodes.push(AsciiGraphNode {
            id: id.into(),
            label: label.into(),
            shape,
        });
    }

    #[cfg(test)]
    pub(crate) fn add_edge(&mut self, from: impl Into<String>, to: impl Into<String>) {
        self.add_edge_with_attrs(
            from,
            to,
            None,
            GraphEdgeStroke::Normal,
            GraphEdgeArrow::Point,
            1,
        );
    }

    fn add_edge_with_attrs(
        &mut self,
        from: impl Into<String>,
        to: impl Into<String>,
        label: Option<String>,
        stroke: GraphEdgeStroke,
        arrow: GraphEdgeArrow,
        length: usize,
    ) {
        self.edges.push(AsciiGraphEdge {
            from: from.into(),
            to: to.into(),
            label,
            stroke,
            arrow,
            length: length.max(1),
        });
    }

    fn add_group(&mut self, title: impl Into<String>, nodes: Vec<String>) {
        self.groups.push(AsciiGraphGroup {
            title: title.into(),
            nodes,
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
        graph.add_node_with_shape(
            &node.id,
            node.label.as_deref().unwrap_or(&node.id),
            parse_node_shape(node.layout_shape.as_deref())?,
        );
    }

    for edge in &model.edges {
        graph.add_edge_with_attrs(
            &edge.from,
            &edge.to,
            edge.label
                .as_deref()
                .map(str::trim)
                .filter(|label| !label.is_empty())
                .map(ToOwned::to_owned),
            parse_edge_stroke(edge.stroke.as_deref().unwrap_or("normal"))?,
            parse_edge_arrow(edge.edge_type.as_deref().unwrap_or("arrow_point"))?,
            edge.length,
        );
    }

    for subgraph in &model.subgraphs {
        graph.add_group(&subgraph.title, subgraph.nodes.clone());
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

fn parse_edge_stroke(stroke: &str) -> Result<GraphEdgeStroke> {
    match stroke {
        "normal" => Ok(GraphEdgeStroke::Normal),
        "dotted" => Ok(GraphEdgeStroke::Dotted),
        _ => Err(AsciiError::UnsupportedFeature {
            diagram_type: "flowchart",
            feature: "non-normal edge strokes",
        }),
    }
}

fn parse_edge_arrow(edge_type: &str) -> Result<GraphEdgeArrow> {
    match edge_type {
        "arrow_open" => Ok(GraphEdgeArrow::Open),
        "arrow_point" => Ok(GraphEdgeArrow::Point),
        _ => Err(AsciiError::UnsupportedFeature {
            diagram_type: "flowchart",
            feature: "non-point edge arrows",
        }),
    }
}

fn parse_node_shape(shape: Option<&str>) -> Result<GraphNodeShape> {
    match shape.unwrap_or("squareRect") {
        "rect" | "rectangle" | "square" | "squareRect" => Ok(GraphNodeShape::Rect),
        "roundedRect" | "rounded" | "event" | "stadium" | "terminal" | "pill" | "circle"
        | "circ" | "doublecircle" | "dbl-circ" | "double-circle" => Ok(GraphNodeShape::Rounded),
        "diamond" | "question" | "diam" | "decision" => Ok(GraphNodeShape::Diamond),
        "subroutine" | "fr-rect" | "subproc" | "subprocess" | "framed-rectangle" => {
            Ok(GraphNodeShape::Subroutine)
        }
        "cylinder" | "cyl" | "db" | "database" => Ok(GraphNodeShape::Cylinder),
        _ => Err(AsciiError::UnsupportedFeature {
            diagram_type: "flowchart",
            feature: "non-rectangular node shapes",
        }),
    }
}

fn validate_supported_flowchart_model(model: &FlowchartV2Model) -> Result<()> {
    if model.subgraphs.iter().any(|subgraph| {
        subgraph.title.contains('\n') || subgraph.nodes.iter().any(|node| node.contains('\n'))
    }) {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "flowchart",
            feature: "multiline subgraph labels",
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

    if model.edges.iter().any(|edge| {
        edge.label
            .as_deref()
            .is_some_and(|label| label.contains('\n'))
    }) {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "flowchart",
            feature: "multiline edge labels",
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

    if model
        .subgraphs
        .iter()
        .flat_map(|subgraph| subgraph.nodes.iter())
        .any(|node| !node_ids.contains(node.as_str()))
    {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "flowchart",
            feature: "subgraphs with missing member nodes",
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
    dotted_horizontal: char,
    dotted_vertical: char,
    rounded_top_left: char,
    rounded_top_right: char,
    rounded_bottom_left: char,
    rounded_bottom_right: char,
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
                dotted_horizontal: '.',
                dotted_vertical: ':',
                rounded_top_left: '/',
                rounded_top_right: '\\',
                rounded_bottom_left: '\\',
                rounded_bottom_right: '/',
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
                dotted_horizontal: '┄',
                dotted_vertical: '┆',
                rounded_top_left: '╭',
                rounded_top_right: '╮',
                rounded_bottom_left: '╰',
                rounded_bottom_right: '╯',
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NodeLayout {
    id: String,
    label: String,
    shape: GraphNodeShape,
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct GroupLayout {
    title: String,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

impl GroupLayout {
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
    let group_layouts = layout_groups(graph, &layouts);
    let width = layouts
        .iter()
        .map(|layout| layout.x + layout.width)
        .chain(group_layouts.iter().map(|layout| layout.x + layout.width))
        .max()
        .unwrap_or_default();
    let height = layouts
        .iter()
        .map(|layout| layout.y + layout.height)
        .chain(group_layouts.iter().map(|layout| layout.y + layout.height))
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
    for group in &group_layouts {
        draw_group(&mut canvas, group, &charset);
    }
    for layout in &layouts {
        draw_node(&mut canvas, layout, &charset, options);
    }
    for edge in &graph.edges {
        draw_edge(&mut canvas, &layouts, edge, graph.direction, &charset);
    }

    Ok(canvas.finish())
}

fn layout_nodes(graph: &AsciiGraph, options: &AsciiRenderOptions) -> Vec<NodeLayout> {
    let group_offset_x = usize::from(!graph.groups.is_empty()) * 2;
    let group_offset_y = usize::from(!graph.groups.is_empty()) * 2;
    let measured = graph
        .nodes
        .iter()
        .map(|node| {
            let width = node_width(node, options);
            let height = 1 + options.box_border_padding * 2 + 2;
            (node, width, height)
        })
        .collect::<Vec<_>>();

    match graph.direction {
        GraphDirection::LeftRight => {
            let label_y_offset = usize::from(graph.edges.iter().any(|edge| edge.label.is_some()));
            let mut x = group_offset_x;
            measured
                .into_iter()
                .enumerate()
                .map(|(index, (node, width, height))| {
                    let layout = NodeLayout {
                        id: node.id.clone(),
                        label: node.label.clone(),
                        shape: node.shape,
                        x,
                        y: group_offset_y + label_y_offset,
                        width,
                        height,
                    };
                    x += width + left_right_gap_after(graph, index, options);
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
                        shape: node.shape,
                        x: group_offset_x + (canvas_width - width) / 2,
                        y,
                        width,
                        height,
                    };
                    y += height + options.graph_padding_y;
                    layout
                })
                .map(|mut layout| {
                    layout.y += group_offset_y;
                    layout
                })
                .collect()
        }
    }
}

fn layout_groups(graph: &AsciiGraph, layouts: &[NodeLayout]) -> Vec<GroupLayout> {
    graph
        .groups
        .iter()
        .filter_map(|group| {
            let members = layouts
                .iter()
                .filter(|layout| group.nodes.iter().any(|node| node == &layout.id))
                .collect::<Vec<_>>();
            if members.is_empty() {
                return None;
            }

            let min_x = members.iter().map(|layout| layout.x).min().unwrap_or(0);
            let min_y = members.iter().map(|layout| layout.y).min().unwrap_or(0);
            let max_right = members
                .iter()
                .map(|layout| layout.right())
                .max()
                .unwrap_or(0);
            let max_bottom = members
                .iter()
                .map(|layout| layout.bottom())
                .max()
                .unwrap_or(0);
            let x = min_x.saturating_sub(2);
            let y = min_y.saturating_sub(2);
            let right = max_right + 2;
            let bottom = max_bottom + 1;
            let min_width = display_width(&group.title) + 4;
            let width = (right - x + 1).max(min_width);
            let height = bottom - y + 1;

            Some(GroupLayout {
                title: group.title.clone(),
                x,
                y,
                width,
                height,
            })
        })
        .collect()
}

fn node_width(node: &AsciiGraphNode, options: &AsciiRenderOptions) -> usize {
    let base = display_width(&node.label) + options.box_border_padding * 2 + 2;
    match node.shape {
        GraphNodeShape::Subroutine => base + 2,
        GraphNodeShape::Cylinder => base + 2,
        GraphNodeShape::Rect | GraphNodeShape::Rounded | GraphNodeShape::Diamond => base,
    }
}

fn left_right_gap_after(
    graph: &AsciiGraph,
    node_index: usize,
    options: &AsciiRenderOptions,
) -> usize {
    let Some(from) = graph.nodes.get(node_index) else {
        return options.graph_padding_x;
    };
    let Some(to) = graph.nodes.get(node_index + 1) else {
        return options.graph_padding_x;
    };
    let Some(edge) = graph
        .edges
        .iter()
        .find(|edge| edge.from == from.id && edge.to == to.id)
    else {
        return options.graph_padding_x;
    };

    let length_gap = options
        .graph_padding_x
        .saturating_add(edge.length.saturating_sub(1) * 2);
    let label_gap = edge.label.as_deref().map(display_width).unwrap_or_default();
    length_gap.max(label_gap)
}

fn draw_node(
    canvas: &mut Canvas,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) {
    match layout.shape {
        GraphNodeShape::Rect => draw_rect_node(canvas, layout, charset, options),
        GraphNodeShape::Rounded => draw_rounded_node(canvas, layout, charset, options),
        GraphNodeShape::Diamond => draw_diamond_node(canvas, layout, charset, options),
        GraphNodeShape::Subroutine => draw_subroutine_node(canvas, layout, charset, options),
        GraphNodeShape::Cylinder => draw_cylinder_node(canvas, layout, charset, options),
    }
}

fn draw_group(canvas: &mut Canvas, group: &GroupLayout, charset: &GraphCharset) {
    let right = group.right();
    let bottom = group.bottom();

    canvas.set(group.x, group.y, charset.top_left);
    canvas.set(right, group.y, charset.top_right);
    canvas.set(group.x, bottom, charset.bottom_left);
    canvas.set(right, bottom, charset.bottom_right);

    for x in (group.x + 1)..right {
        canvas.set(x, group.y, charset.horizontal);
        canvas.set(x, bottom, charset.horizontal);
    }

    for y in (group.y + 1)..bottom {
        canvas.set(group.x, y, charset.vertical);
        canvas.set(right, y, charset.vertical);
    }

    let title = format!(" {} ", group.title);
    if display_width(&title) + 2 < group.width {
        canvas.write_text(group.x + 2, group.y, &title);
    }
}

fn draw_rect_node(
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

fn draw_rounded_node(
    canvas: &mut Canvas,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) {
    draw_node_with_corners(
        canvas,
        layout,
        charset,
        options,
        RoundedCorners {
            top_left: charset.rounded_top_left,
            top_right: charset.rounded_top_right,
            bottom_left: charset.rounded_bottom_left,
            bottom_right: charset.rounded_bottom_right,
        },
    );
}

#[derive(Debug, Clone, Copy)]
struct RoundedCorners {
    top_left: char,
    top_right: char,
    bottom_left: char,
    bottom_right: char,
}

fn draw_node_with_corners(
    canvas: &mut Canvas,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
    corners: RoundedCorners,
) {
    let right = layout.right();
    let bottom = layout.bottom();

    canvas.set(layout.x, layout.y, corners.top_left);
    canvas.set(right, layout.y, corners.top_right);
    canvas.set(layout.x, bottom, corners.bottom_left);
    canvas.set(right, bottom, corners.bottom_right);

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

fn draw_diamond_node(
    canvas: &mut Canvas,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) {
    let right = layout.right();
    let bottom = layout.bottom();
    let center_y = layout.center_y();

    canvas.set(layout.x, layout.y, charset.rounded_top_left);
    canvas.set(right, layout.y, charset.rounded_top_right);
    canvas.set(layout.x, layout.y + 1, charset.rounded_top_left);
    canvas.set(right, layout.y + 1, charset.rounded_top_right);
    canvas.set(layout.x, center_y, '<');
    canvas.set(right, center_y, '>');
    canvas.set(layout.x, bottom - 1, charset.rounded_bottom_left);
    canvas.set(right, bottom - 1, charset.rounded_bottom_right);
    canvas.set(layout.x, bottom, charset.rounded_bottom_left);
    canvas.set(right, bottom, charset.rounded_bottom_right);

    for x in (layout.x + 1)..right {
        canvas.set(x, layout.y, charset.horizontal);
        canvas.set(x, bottom, charset.horizontal);
    }

    let text_x = layout.x + 1 + options.box_border_padding;
    canvas.write_text(text_x, center_y, &layout.label);
}

fn draw_subroutine_node(
    canvas: &mut Canvas,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) {
    draw_rect_node(canvas, layout, charset, options);
    if layout.width > 5 {
        let left_inner = layout.x + 2;
        let right_inner = layout.right().saturating_sub(2);
        for y in (layout.y + 1)..layout.bottom() {
            canvas.set(left_inner, y, charset.vertical);
            canvas.set(right_inner, y, charset.vertical);
        }
        let text_y = layout.y + 1 + options.box_border_padding;
        for x in (left_inner + 1)..right_inner {
            canvas.set(x, text_y, ' ');
        }
    }
    write_centered_label(canvas, layout, options);
}

fn draw_cylinder_node(
    canvas: &mut Canvas,
    layout: &NodeLayout,
    charset: &GraphCharset,
    options: &AsciiRenderOptions,
) {
    draw_rounded_node(canvas, layout, charset, options);
    if layout.height > 3 {
        for x in (layout.x + 1)..layout.right() {
            canvas.set(x, layout.y + 1, charset.horizontal);
        }
    }
    let text_y = layout.y + 1 + options.box_border_padding;
    for x in (layout.x + 1)..layout.right() {
        canvas.set(x, text_y, ' ');
    }
    write_centered_label(canvas, layout, options);
}

fn write_centered_label(canvas: &mut Canvas, layout: &NodeLayout, options: &AsciiRenderOptions) {
    let text_width = display_width(&layout.label);
    let text_x = layout.x + layout.width.saturating_sub(text_width) / 2;
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
        GraphDirection::LeftRight => draw_left_right_edge(canvas, from, to, edge, charset),
        GraphDirection::TopDown => draw_top_down_edge(canvas, from, to, edge, charset),
    }

    draw_edge_label(canvas, from, to, edge, direction);
}

fn draw_left_right_edge(
    canvas: &mut Canvas,
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) {
    if to.x <= from.right() + 1 {
        return;
    }

    let y = from.center_y();
    if from.shape != GraphNodeShape::Diamond {
        canvas.set(from.right(), y, charset.right_connector);
    }
    let start = from.right() + 1;
    let end = to.x - 1;
    let line = edge_line_char(edge, charset, GraphDirection::LeftRight);
    for x in start..end {
        canvas.set(x, y, line);
    }
    match edge.arrow {
        GraphEdgeArrow::Open => canvas.set(end, y, line),
        GraphEdgeArrow::Point => canvas.set(end, y, charset.arrow_right),
    }
}

fn draw_top_down_edge(
    canvas: &mut Canvas,
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) {
    if to.y <= from.bottom() + 1 {
        return;
    }

    let x = from.center_x();
    let start = from.bottom() + 1;
    let end = to.y - 1;
    let line = edge_line_char(edge, charset, GraphDirection::TopDown);
    for y in start..end {
        canvas.set(x, y, line);
    }
    match edge.arrow {
        GraphEdgeArrow::Open => canvas.set(x, end, line),
        GraphEdgeArrow::Point => canvas.set(x, end, charset.arrow_down),
    }
}

fn edge_line_char(
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
    direction: GraphDirection,
) -> char {
    match (edge.stroke, direction) {
        (GraphEdgeStroke::Normal, GraphDirection::LeftRight) => charset.horizontal,
        (GraphEdgeStroke::Normal, GraphDirection::TopDown) => charset.vertical,
        (GraphEdgeStroke::Dotted, GraphDirection::LeftRight) => charset.dotted_horizontal,
        (GraphEdgeStroke::Dotted, GraphDirection::TopDown) => charset.dotted_vertical,
    }
}

fn draw_edge_label(
    canvas: &mut Canvas,
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    direction: GraphDirection,
) {
    let Some(label) = edge.label.as_deref() else {
        return;
    };

    match direction {
        GraphDirection::LeftRight => {
            let start = from.right() + 1;
            let end = to.x.saturating_sub(1);
            let available = end.saturating_sub(start).saturating_add(1);
            let width = display_width(label);
            let x = start + available.saturating_sub(width) / 2;
            canvas.write_text(x, from.y.saturating_sub(1), label);
        }
        GraphDirection::TopDown => {
            let start = from.bottom() + 1;
            let end = to.y.saturating_sub(1);
            let available = end.saturating_sub(start).saturating_add(1);
            let y = start + available / 2;
            let width = display_width(label);
            let x = from.center_x().saturating_sub(width / 2);
            canvas.write_text(x, y, label);
        }
    }
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
