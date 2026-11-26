use merman_model::families::railroad::{
    RailroadAstNode, RailroadDiagramModel, RailroadRepeatBound, RailroadRuleModel,
};
use merman_render_core::{Bounds, Result, text::TextStyle};

use super::{
    MeasureRawSvgBBox, RailroadDiagramLayout, RailroadElementLayout, RailroadPathLayout,
    RailroadRuleLayout, RailroadStyle,
};

#[derive(Debug, Clone)]
struct ExprLayout {
    width: f64,
    height: f64,
    up: f64,
    down: f64,
    elements: Vec<RailroadElementLayout>,
    paths: Vec<RailroadPathLayout>,
    render_node: RailroadRenderNode,
}

#[derive(Debug, Clone)]
pub(super) enum RailroadRenderNode {
    Group {
        class: &'static str,
        transform: Option<(f64, f64)>,
        children: Vec<RailroadRenderNode>,
    },
    Element {
        layout: RailroadElementLayout,
        transform: Option<(f64, f64)>,
    },
    Path(RailroadPathLayout),
}

impl RailroadRenderNode {
    fn set_transform(&mut self, x: f64, y: f64) {
        match self {
            Self::Group { transform, .. } | Self::Element { transform, .. } => {
                *transform = Some((x, y));
            }
            Self::Path(path) => {
                path.x = x;
                path.y = y;
            }
        }
    }
}

/// Lays out a typed Railroad diagram using the caller's raw SVG text bbox measurements.
///
/// The callback returns width and height, in that order.
pub fn layout_railroad_diagram_typed(
    model: &RailroadDiagramModel,
    diagram_type: &str,
    style: &RailroadStyle,
    measure_bbox: &dyn Fn(&str, &TextStyle) -> (f64, f64),
) -> Result<RailroadDiagramLayout> {
    let mut y = style.padding;
    let mut max_width: f64 = 0.0;
    let mut rules = Vec::new();

    for rule in &model.rules {
        let mut rule_layout = layout_rule(rule, y, style, measure_bbox);
        y += rule_layout.height + style.vertical_separation;
        max_width = max_width.max(rule_layout.width);
        rule_layout.x = 0.0;
        rules.push(rule_layout);
    }

    let width = if rules.is_empty() {
        200.0
    } else {
        max_width + style.padding * 2.0
    };
    let height = if rules.is_empty() {
        100.0
    } else {
        y + style.padding
    };

    Ok(RailroadDiagramLayout {
        bounds: Some(Bounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: width,
            max_y: height,
        }),
        diagram_type: normalized_railroad_diagram_type(diagram_type).to_string(),
        width,
        height,
        use_max_width: style.use_max_width,
        rules,
    })
}

fn normalized_railroad_diagram_type(diagram_type: &str) -> &'static str {
    match diagram_type {
        "railroadEbnf" => "railroadEbnf",
        "railroadAbnf" => "railroadAbnf",
        "railroadPeg" => "railroadPeg",
        _ => "railroad",
    }
}

fn layout_rule(
    rule: &RailroadRuleModel,
    y: f64,
    style: &RailroadStyle,
    measure_bbox: &MeasureRawSvgBBox<'_>,
) -> RailroadRuleLayout {
    let rule_name = format!("{} =", rule.name);
    let name_width = measure_text(
        &rule_name,
        style.rule_name_font_family(),
        style.font_size,
        style.rule_name_weight(),
        measure_bbox,
    )
    .0 + 20.0;
    let definition_x = name_width + 20.0;
    let definition = layout_expr(&rule.definition, style, measure_bbox);
    let baseline_y = 20.0_f64.max(definition.up);
    let definition_y = baseline_y - definition.up;
    let mut elements = definition.elements;
    let mut paths = definition.paths;
    translate_elements(&mut elements, definition_x, definition_y);
    translate_paths(&mut paths, definition_x, definition_y);

    paths.push(railroad_path(
        PathBuilder::new()
            .move_to(name_width + style.marker_radius, baseline_y)
            .line_to(definition_x, baseline_y)
            .build(),
    ));
    paths.push(railroad_path(
        PathBuilder::new()
            .move_to(definition_x + definition.width, baseline_y)
            .line_to(
                definition_x + definition.width + 10.0 - style.marker_radius,
                baseline_y,
            )
            .build(),
    ));

    RailroadRuleLayout {
        name: rule.name.clone(),
        x: 0.0,
        y,
        width: definition_x + definition.width + 10.0 + style.marker_radius,
        height: 40.0_f64.max(definition_y + definition.height + style.padding * 2.0),
        baseline_y,
        name_width,
        definition_x,
        start_marker_x: name_width,
        end_marker_x: definition_x + definition.width + 10.0,
        marker_radius: style.marker_radius,
        elements,
        paths,
    }
}

fn layout_expr(
    node: &RailroadAstNode,
    style: &RailroadStyle,
    measure_bbox: &MeasureRawSvgBBox<'_>,
) -> ExprLayout {
    match node {
        RailroadAstNode::Terminal { value, .. } => {
            layout_box("terminal", value, style, measure_bbox)
        }
        RailroadAstNode::NonTerminal { name, .. } => {
            layout_box("nonterminal", name, style, measure_bbox)
        }
        RailroadAstNode::Special { text, .. } => {
            layout_box("special", &format!("? {text} ?"), style, measure_bbox)
        }
        RailroadAstNode::Sequence { elements, .. } => {
            layout_sequence(elements, style, measure_bbox)
        }
        RailroadAstNode::Choice { alternatives, .. } => {
            layout_choice(alternatives, style, measure_bbox)
        }
        RailroadAstNode::Optional { element, .. } => layout_optional(element, style, measure_bbox),
        RailroadAstNode::Repetition { element, min, .. } => {
            layout_repetition(element, *min, style, measure_bbox)
        }
    }
}

pub(super) fn railroad_render_node(
    node: &RailroadAstNode,
    style: &RailroadStyle,
    measure_bbox: &MeasureRawSvgBBox<'_>,
) -> (RailroadRenderNode, f64) {
    let layout = layout_expr(node, style, measure_bbox);
    (layout.render_node, layout.up)
}

fn layout_box(
    kind: &str,
    label: &str,
    style: &RailroadStyle,
    measure_bbox: &MeasureRawSvgBBox<'_>,
) -> ExprLayout {
    let font_family = if kind == "terminal" {
        style.terminal_font_family()
    } else {
        &style.font_family
    };
    let (text_width, text_height) =
        measure_text(label, font_family, style.font_size, None, measure_bbox);
    let width = text_width + style.padding * 2.0;
    let height = text_height + style.padding * 2.0;
    let element = RailroadElementLayout {
        kind: kind.to_string(),
        label: label.to_string(),
        x: 0.0,
        y: 0.0,
        width,
        height,
        text_x: width / 2.0,
        text_y: height / 2.0,
    };
    ExprLayout {
        width,
        height,
        up: height / 2.0,
        down: height / 2.0,
        elements: vec![element.clone()],
        paths: Vec::new(),
        render_node: RailroadRenderNode::Element {
            layout: element,
            transform: None,
        },
    }
}

fn layout_sequence(
    elements: &[RailroadAstNode],
    style: &RailroadStyle,
    measure_bbox: &MeasureRawSvgBBox<'_>,
) -> ExprLayout {
    let rendered: Vec<_> = elements
        .iter()
        .map(|element| layout_expr(element, style, measure_bbox))
        .collect();
    if rendered.is_empty() {
        return empty_expr();
    }

    let mut width = rendered.iter().map(|item| item.width).sum::<f64>();
    width += (rendered.len().saturating_sub(1)) as f64 * style.horizontal_separation;
    let up = rendered.iter().map(|item| item.up).fold(0.0, f64::max);
    let down = rendered.iter().map(|item| item.down).fold(0.0, f64::max);
    let mut out = ExprLayout {
        width,
        height: up + down,
        up,
        down,
        elements: Vec::new(),
        paths: Vec::new(),
        render_node: RailroadRenderNode::Group {
            class: "railroad-sequence",
            transform: None,
            children: Vec::new(),
        },
    };
    let mut render_children = Vec::new();
    let mut x = 0.0;
    for (idx, mut child) in rendered.into_iter().enumerate() {
        let y = up - child.up;
        let child_width = child.width;
        translate_elements(&mut child.elements, x, y);
        translate_paths(&mut child.paths, x, y);
        out.elements.extend(child.elements);
        out.paths.extend(child.paths);
        child.render_node.set_transform(x, y);
        render_children.push(child.render_node);
        if idx + 1 < elements.len() {
            let line_x1 = x + child_width;
            let line_x2 = line_x1 + style.horizontal_separation;
            let path = railroad_path(
                PathBuilder::new()
                    .move_to(line_x1, up)
                    .line_to(line_x2, up)
                    .build(),
            );
            out.paths.push(path.clone());
            render_children.push(RailroadRenderNode::Path(path));
        }
        x += child_width + style.horizontal_separation;
    }
    if let RailroadRenderNode::Group { children, .. } = &mut out.render_node {
        *children = render_children;
    }
    out
}

fn layout_choice(
    alternatives: &[RailroadAstNode],
    style: &RailroadStyle,
    measure_bbox: &MeasureRawSvgBBox<'_>,
) -> ExprLayout {
    let rendered: Vec<_> = alternatives
        .iter()
        .map(|alternative| layout_expr(alternative, style, measure_bbox))
        .collect();
    if rendered.is_empty() {
        return empty_expr();
    }

    let max_width = rendered.iter().map(|item| item.width).fold(0.0, f64::max);
    let mut total_height = rendered.iter().map(|item| item.height).sum::<f64>();
    total_height += (rendered.len().saturating_sub(1)) as f64 * style.vertical_separation;
    let arc_radius = style.arc_radius;
    let total_width = max_width + arc_radius * 4.0;
    let center_y = total_height / 2.0;
    let mut out = ExprLayout {
        width: total_width,
        height: total_height,
        up: center_y,
        down: total_height - center_y,
        elements: Vec::new(),
        paths: Vec::new(),
        render_node: RailroadRenderNode::Group {
            class: "railroad-choice",
            transform: None,
            children: Vec::new(),
        },
    };
    let mut render_children = Vec::new();

    let mut y = 0.0;
    for mut child in rendered {
        let elem_y = y;
        let elem_center_y = elem_y + child.up;
        let elem_x = arc_radius * 2.0 + (max_width - child.width) / 2.0;
        let is_center = same_layout_coordinate(elem_center_y, center_y);
        let below_center = !is_center && elem_center_y > center_y;
        let left_path = if is_center {
            PathBuilder::new()
                .move_to(0.0, center_y)
                .line_to(elem_x, elem_center_y)
                .build()
        } else {
            PathBuilder::new()
                .move_to(0.0, center_y)
                .arc_to(
                    arc_radius,
                    arc_radius,
                    0.0,
                    false,
                    below_center,
                    arc_radius,
                    center_y
                        + if below_center {
                            arc_radius
                        } else {
                            -arc_radius
                        },
                )
                .line_to(
                    arc_radius,
                    elem_center_y
                        - if below_center {
                            arc_radius
                        } else {
                            -arc_radius
                        },
                )
                .arc_to(
                    arc_radius,
                    arc_radius,
                    0.0,
                    false,
                    !below_center,
                    arc_radius * 2.0,
                    elem_center_y,
                )
                .line_to(elem_x, elem_center_y)
                .build()
        };
        let left_path = railroad_path(left_path);
        out.paths.push(left_path.clone());

        let right_start = elem_x + child.width;
        let right_lane_x = total_width - arc_radius * 2.0;
        let right_path = if is_center {
            PathBuilder::new()
                .move_to(right_start, elem_center_y)
                .line_to(total_width, center_y)
                .build()
        } else {
            PathBuilder::new()
                .move_to(right_start, elem_center_y)
                .line_to(right_lane_x, elem_center_y)
                .arc_to(
                    arc_radius,
                    arc_radius,
                    0.0,
                    false,
                    !below_center,
                    total_width - arc_radius,
                    elem_center_y
                        + if below_center {
                            -arc_radius
                        } else {
                            arc_radius
                        },
                )
                .line_to(
                    total_width - arc_radius,
                    center_y
                        + if below_center {
                            arc_radius
                        } else {
                            -arc_radius
                        },
                )
                .arc_to(
                    arc_radius,
                    arc_radius,
                    0.0,
                    false,
                    below_center,
                    total_width,
                    center_y,
                )
                .build()
        };
        let right_path = railroad_path(right_path);
        out.paths.push(right_path.clone());
        child.render_node.set_transform(elem_x, elem_y);
        render_children.push(child.render_node);
        render_children.push(RailroadRenderNode::Path(left_path));
        render_children.push(RailroadRenderNode::Path(right_path));
        translate_elements(&mut child.elements, elem_x, elem_y);
        translate_paths(&mut child.paths, elem_x, elem_y);
        out.elements.extend(child.elements);
        out.paths.extend(child.paths);
        y += child.height + style.vertical_separation;
    }

    if let RailroadRenderNode::Group { children, .. } = &mut out.render_node {
        *children = render_children;
    }

    out
}

fn same_layout_coordinate(left: f64, right: f64) -> bool {
    // Distinct addition orders can drift while still producing the same emitted coordinate.
    left.is_finite() && right.is_finite() && fmt_number(left) == fmt_number(right)
}

fn layout_optional(
    element: &RailroadAstNode,
    style: &RailroadStyle,
    measure_bbox: &MeasureRawSvgBBox<'_>,
) -> ExprLayout {
    let mut inner = layout_expr(element, style, measure_bbox);
    let arc_radius = style.arc_radius;
    let arc_height = arc_radius * 2.0;
    let total_width = inner.width + arc_radius * 4.0;
    let total_height = inner.height + arc_height;
    let elem_x = arc_radius * 2.0;
    let elem_y = arc_height;
    let center_y = elem_y + inner.up;
    translate_elements(&mut inner.elements, elem_x, elem_y);
    translate_paths(&mut inner.paths, elem_x, elem_y);
    inner.render_node.set_transform(elem_x, elem_y);

    let mut paths = inner.paths;
    let lower_path = railroad_path(
        PathBuilder::new()
            .move_to(0.0, center_y)
            .line_to(arc_radius * 2.0, center_y)
            .build(),
    );
    paths.push(lower_path.clone());
    let lower_path_2 = railroad_path(
        PathBuilder::new()
            .move_to(elem_x + inner.width, center_y)
            .line_to(total_width, center_y)
            .build(),
    );
    paths.push(lower_path_2.clone());
    let bypass_path = railroad_path(
        PathBuilder::new()
            .move_to(0.0, center_y)
            .arc_to(
                arc_radius,
                arc_radius,
                0.0,
                false,
                false,
                arc_radius,
                center_y - arc_radius,
            )
            .line_to(arc_radius, arc_radius)
            .arc_to(
                arc_radius,
                arc_radius,
                0.0,
                false,
                true,
                arc_radius * 2.0,
                0.0,
            )
            .line_to(total_width - arc_radius * 2.0, 0.0)
            .arc_to(
                arc_radius,
                arc_radius,
                0.0,
                false,
                true,
                total_width - arc_radius,
                arc_radius,
            )
            .line_to(total_width - arc_radius, center_y - arc_radius)
            .arc_to(
                arc_radius,
                arc_radius,
                0.0,
                false,
                false,
                total_width,
                center_y,
            )
            .build(),
    );
    paths.push(bypass_path.clone());

    ExprLayout {
        width: total_width,
        height: total_height,
        up: center_y,
        down: total_height - center_y,
        elements: inner.elements,
        paths,
        render_node: RailroadRenderNode::Group {
            class: "railroad-optional",
            transform: None,
            children: vec![
                inner.render_node,
                RailroadRenderNode::Path(lower_path),
                RailroadRenderNode::Path(lower_path_2),
                RailroadRenderNode::Path(bypass_path),
            ],
        },
    }
}

fn layout_repetition(
    element: &RailroadAstNode,
    min: RailroadRepeatBound,
    style: &RailroadStyle,
    measure_bbox: &MeasureRawSvgBBox<'_>,
) -> ExprLayout {
    let mut inner = layout_expr(element, style, measure_bbox);
    let arc_radius = style.arc_radius;
    let arc_height = arc_radius * 2.0;
    let total_width = inner.width + arc_radius * 4.0;
    let has_bypass = min.is_zero();
    let total_height = inner.height + arc_height + if has_bypass { arc_height } else { 0.0 };
    let elem_x = arc_radius * 2.0;
    let elem_y = if has_bypass { arc_height } else { 0.0 };
    let center_y = elem_y + inner.up;
    translate_elements(&mut inner.elements, elem_x, elem_y);
    translate_paths(&mut inner.paths, elem_x, elem_y);
    inner.render_node.set_transform(elem_x, elem_y);
    let mut paths = inner.paths;
    let forward_path = railroad_path(
        PathBuilder::new()
            .move_to(0.0, center_y)
            .line_to(arc_radius * 2.0, center_y)
            .build(),
    );
    paths.push(forward_path.clone());
    let forward_path_2 = railroad_path(
        PathBuilder::new()
            .move_to(elem_x + inner.width, center_y)
            .line_to(total_width, center_y)
            .build(),
    );
    paths.push(forward_path_2.clone());

    let loop_y = elem_y + inner.height + arc_radius;
    let loop_path = railroad_path(
        PathBuilder::new()
            .move_to(elem_x + inner.width, center_y)
            .arc_to(
                arc_radius,
                arc_radius,
                0.0,
                false,
                true,
                elem_x + inner.width + arc_radius,
                center_y + arc_radius,
            )
            .line_to(elem_x + inner.width + arc_radius, loop_y)
            .arc_to(
                arc_radius,
                arc_radius,
                0.0,
                false,
                true,
                elem_x + inner.width,
                loop_y + arc_radius,
            )
            .line_to(arc_radius * 2.0, loop_y + arc_radius)
            .arc_to(arc_radius, arc_radius, 0.0, false, true, arc_radius, loop_y)
            .line_to(arc_radius, center_y + arc_radius)
            .arc_to(
                arc_radius,
                arc_radius,
                0.0,
                false,
                true,
                arc_radius * 2.0,
                center_y,
            )
            .build(),
    );
    paths.push(loop_path.clone());

    let mut bypass_render_path = None;
    if has_bypass {
        let bypass_path = railroad_path(
            PathBuilder::new()
                .move_to(0.0, center_y)
                .arc_to(
                    arc_radius,
                    arc_radius,
                    0.0,
                    false,
                    false,
                    arc_radius,
                    center_y - arc_radius,
                )
                .line_to(arc_radius, arc_radius)
                .arc_to(
                    arc_radius,
                    arc_radius,
                    0.0,
                    false,
                    true,
                    arc_radius * 2.0,
                    0.0,
                )
                .line_to(total_width - arc_radius * 2.0, 0.0)
                .arc_to(
                    arc_radius,
                    arc_radius,
                    0.0,
                    false,
                    true,
                    total_width - arc_radius,
                    arc_radius,
                )
                .line_to(total_width - arc_radius, center_y - arc_radius)
                .arc_to(
                    arc_radius,
                    arc_radius,
                    0.0,
                    false,
                    false,
                    total_width,
                    center_y,
                )
                .build(),
        );
        paths.push(bypass_path.clone());
        bypass_render_path = Some(bypass_path);
    }

    let mut render_children = vec![
        inner.render_node,
        RailroadRenderNode::Path(forward_path),
        RailroadRenderNode::Path(forward_path_2),
        RailroadRenderNode::Path(loop_path),
    ];
    if let Some(path) = bypass_render_path {
        render_children.push(RailroadRenderNode::Path(path));
    }

    ExprLayout {
        width: total_width,
        height: total_height,
        up: center_y,
        down: total_height - center_y,
        elements: inner.elements,
        paths,
        render_node: RailroadRenderNode::Group {
            class: "railroad-repetition",
            transform: None,
            children: render_children,
        },
    }
}

fn empty_expr() -> ExprLayout {
    ExprLayout {
        width: 0.0,
        height: 0.0,
        up: 0.0,
        down: 0.0,
        elements: Vec::new(),
        paths: Vec::new(),
        render_node: RailroadRenderNode::Group {
            class: "railroad-group",
            transform: None,
            children: Vec::new(),
        },
    }
}

fn measure_text(
    text: &str,
    font_family: &str,
    font_size: f64,
    font_weight: Option<u16>,
    measure_bbox: &MeasureRawSvgBBox<'_>,
) -> (f64, f64) {
    let text_style = TextStyle {
        font_family: Some(font_family.to_owned()),
        font_size,
        font_weight: font_weight.map(|weight| weight.to_string()),
        font_style: None,
    };
    measure_bbox(text, &text_style)
}

fn translate_elements(elements: &mut [RailroadElementLayout], dx: f64, dy: f64) {
    for element in elements {
        element.x += dx;
        element.y += dy;
    }
}

fn translate_paths(paths: &mut [RailroadPathLayout], dx: f64, dy: f64) {
    if dx == 0.0 && dy == 0.0 {
        return;
    }
    for path in paths {
        path.x += dx;
        path.y += dy;
    }
}

fn railroad_path(d: String) -> RailroadPathLayout {
    RailroadPathLayout { x: 0.0, y: 0.0, d }
}

struct PathBuilder {
    d: String,
}

impl PathBuilder {
    fn new() -> Self {
        Self { d: String::new() }
    }

    fn move_to(mut self, x: f64, y: f64) -> Self {
        self.push_cmd("M", &[x, y]);
        self
    }

    fn line_to(mut self, x: f64, y: f64) -> Self {
        self.push_cmd("L", &[x, y]);
        self
    }

    #[allow(clippy::too_many_arguments)]
    fn arc_to(
        mut self,
        rx: f64,
        ry: f64,
        rotation: f64,
        large_arc: bool,
        sweep: bool,
        x: f64,
        y: f64,
    ) -> Self {
        self.push_raw("A");
        self.push_number(rx);
        self.push_number(ry);
        self.push_number(rotation);
        self.push_number(if large_arc { 1.0 } else { 0.0 });
        self.push_number(if sweep { 1.0 } else { 0.0 });
        self.push_number(x);
        self.push_number(y);
        self
    }

    fn build(self) -> String {
        self.d.trim().to_string()
    }

    fn push_cmd(&mut self, cmd: &str, values: &[f64]) {
        self.push_raw(cmd);
        for value in values {
            self.push_number(*value);
        }
    }

    fn push_raw(&mut self, value: &str) {
        if !self.d.is_empty() {
            self.d.push(' ');
        }
        self.d.push_str(value);
    }

    fn push_number(&mut self, value: f64) {
        if !self.d.is_empty() {
            self.d.push(' ');
        }
        self.d.push_str(&fmt_number(value));
    }
}

fn fmt_number(value: f64) -> String {
    if !value.is_finite() || value.abs() < 0.0005 {
        return "0".to_string();
    }
    let mut rounded = if value.abs() <= f64::MAX / 1000.0 {
        (value * 1000.0).round() / 1000.0
    } else {
        value
    };
    if rounded.abs() < 0.0005 {
        rounded = 0.0;
    }
    let mut s = format!("{rounded:.3}");
    while s.contains('.') && s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    if s == "-0" { "0".to_string() } else { s }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::railroad::style::railroad_style;
    use merman_model::SourceSpan;
    use svgtypes::{PathParser, PathSegment};

    const SPAN: SourceSpan = SourceSpan { start: 0, end: 0 };

    fn terminal(value: &str) -> RailroadAstNode {
        RailroadAstNode::Terminal {
            value: value.to_owned(),
            span: SPAN,
            selection: SPAN,
        }
    }

    fn nonterminal(name: &str) -> RailroadAstNode {
        RailroadAstNode::NonTerminal {
            name: name.to_owned(),
            span: SPAN,
            selection: SPAN,
        }
    }

    fn repetition(element: RailroadAstNode, min: RailroadRepeatBound) -> RailroadAstNode {
        RailroadAstNode::Repetition {
            element: Box::new(element),
            min,
            max: RailroadRepeatBound::INFINITY,
            separator: None,
            span: SPAN,
        }
    }

    fn measure_bbox(text: &str, style: &TextStyle) -> (f64, f64) {
        (
            text.len() as f64 * style.font_size / 2.0,
            style.font_size * 1.1,
        )
    }

    fn choice_branch_connector_paths(
        branches: &[&str],
        effective_config: &serde_json::Value,
        branch_label: &str,
    ) -> (String, String) {
        let choice = RailroadAstNode::Choice {
            alternatives: branches.iter().map(|&label| nonterminal(label)).collect(),
            span: SPAN,
        };
        let style = railroad_style(effective_config);
        let (render_node, _) = railroad_render_node(&choice, &style, &measure_bbox);
        let RailroadRenderNode::Group {
            class, children, ..
        } = render_node
        else {
            panic!("expected railroad choice render group");
        };
        assert_eq!(class, "railroad-choice");
        let (branches, remainder) = children.as_chunks::<3>();
        assert!(
            remainder.is_empty(),
            "choice render children must be branch/left-path/right-path triples"
        );
        let mut branches = branches.iter();
        let mut matching_branches = branches.by_ref().filter_map(|branch| {
            let [
                branch,
                RailroadRenderNode::Path(left_path),
                RailroadRenderNode::Path(right_path),
            ] = branch
            else {
                panic!("choice render children must be branch/left-path/right-path triples");
            };
            matches!(
                branch,
                RailroadRenderNode::Element { layout, .. } if layout.label == branch_label
            )
            .then_some((left_path, right_path))
        });
        let (left_path, right_path) = matching_branches
            .next()
            .unwrap_or_else(|| panic!("choice branch render node: {branch_label}"));
        assert!(
            matching_branches.next().is_none(),
            "choice branch label must be unique: {branch_label}"
        );

        (left_path.d.clone(), right_path.d.clone())
    }

    fn connector_segments(path: &str) -> Vec<PathSegment> {
        PathParser::from(path)
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap_or_else(|error| panic!("valid connector path {path:?}: {error}"))
    }

    fn assert_straight_connector(path: &str) {
        assert!(
            matches!(
                connector_segments(path).as_slice(),
                [PathSegment::MoveTo { .. }, PathSegment::LineTo { .. }]
            ),
            "expected a straight connector: {path}"
        );
    }

    fn assert_curved_connector(path: &str) {
        assert!(
            connector_segments(path)
                .iter()
                .any(|segment| matches!(segment, PathSegment::EllipticalArc { .. })),
            "expected an arc connector: {path}"
        );
    }

    fn repetition_render_path_count(min: RailroadRepeatBound) -> usize {
        let node = repetition(terminal("item"), min);
        let (render_node, _) = railroad_render_node(
            &node,
            &railroad_style(&serde_json::json!({})),
            &measure_bbox,
        );
        let RailroadRenderNode::Group {
            class, children, ..
        } = render_node
        else {
            panic!("expected railroad repetition render group");
        };
        assert_eq!(class, "railroad-repetition");
        children
            .iter()
            .filter(|child| matches!(child, RailroadRenderNode::Path(_)))
            .count()
    }

    #[test]
    fn railroad_repetition_bypass_depends_only_on_zero_minimum() {
        assert_eq!(repetition_render_path_count(RailroadRepeatBound::ZERO), 4);
        assert_eq!(repetition_render_path_count(RailroadRepeatBound::ONE), 3);
        assert_eq!(
            repetition_render_path_count(RailroadRepeatBound::INFINITY),
            3
        );
    }

    #[test]
    fn railroad_layout_handles_sequence_choice_and_repetition() {
        let model = RailroadDiagramModel {
            acc_title: None,
            acc_descr: None,
            title: None,
            rules: vec![RailroadRuleModel {
                name: "expr".to_owned(),
                definition: RailroadAstNode::Sequence {
                    elements: vec![
                        nonterminal("term"),
                        repetition(terminal("+"), RailroadRepeatBound::ZERO),
                    ],
                    span: SPAN,
                },
            }],
        };
        let style = railroad_style(&serde_json::json!({}));
        let layout =
            layout_railroad_diagram_typed(&model, "railroad", &style, &measure_bbox).unwrap();

        assert_eq!(layout.rules.len(), 1);
        assert!(layout.rules[0].width > 0.0);
        assert!(
            layout.rules[0]
                .paths
                .iter()
                .any(|path| path.d.contains('A'))
        );
        assert_eq!(layout.rules[0].elements.len(), 2);
        assert_eq!(layout.diagram_type, "railroad");

        let ebnf_layout =
            layout_railroad_diagram_typed(&model, "railroadEbnf", &style, &measure_bbox).unwrap();
        assert_eq!(ebnf_layout.diagram_type, "railroadEbnf");
        assert_eq!(
            serde_json::to_value(&ebnf_layout).unwrap()["diagram_type"],
            "railroadEbnf"
        );
    }

    #[test]
    fn railroad_choice_keeps_center_branch_connectors_straight() {
        let effective_config = serde_json::json!({
            "themeVariables": { "fontSize": 16 }
        });
        let branches = ["a", "b", "c", "number", "e", "f", "g"];
        let (left_path, right_path) =
            choice_branch_connector_paths(&branches, &effective_config, "number");

        assert_straight_connector(&left_path);
        assert_straight_connector(&right_path);

        let (upper_left_path, upper_right_path) =
            choice_branch_connector_paths(&branches, &effective_config, "a");
        assert_curved_connector(&upper_left_path);
        assert_curved_connector(&upper_right_path);
    }

    #[test]
    fn railroad_lane_coordinate_equality_matches_path_serialization() {
        assert!(same_layout_coordinate(56.5004, 56.50049));
        assert!(!same_layout_coordinate(56.5004, 56.5006));
        assert!(!same_layout_coordinate(f64::INFINITY, f64::INFINITY));
        assert!(!same_layout_coordinate(f64::NAN, f64::NAN));
        assert_ne!(fmt_number(f64::MAX), "inf");
    }

    #[test]
    fn railroad_choice_keeps_center_branch_connectors_straight_with_fractional_spacing() {
        let effective_config = serde_json::json!({
            "themeVariables": { "fontSize": 16 },
            "railroad": { "verticalSeparation": 0.1 }
        });
        let (left_path, right_path) =
            choice_branch_connector_paths(&["a", "b", "c"], &effective_config, "b");

        assert_straight_connector(&left_path);
        assert_straight_connector(&right_path);
    }
}
