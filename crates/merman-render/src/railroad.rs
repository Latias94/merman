use crate::Result;
use crate::config::{config_bool, config_f64, config_string};
use crate::model::{
    Bounds, RailroadDiagramLayout, RailroadElementLayout, RailroadPathLayout, RailroadRuleLayout,
};
use crate::text::{TextMeasurer, TextStyle};
use merman_core::diagrams::railroad::{
    RailroadAstNode, RailroadDiagramRenderModel, RailroadRuleModel,
};

#[derive(Debug, Clone)]
pub(crate) struct RailroadStyle {
    pub padding: f64,
    pub vertical_separation: f64,
    pub horizontal_separation: f64,
    pub arc_radius: f64,
    pub font_size: f64,
    pub font_family: String,
    pub terminal_fill: String,
    pub terminal_stroke: String,
    pub terminal_text_color: String,
    pub non_terminal_fill: String,
    pub non_terminal_stroke: String,
    pub non_terminal_text_color: String,
    pub line_color: String,
    pub stroke_width: f64,
    pub marker_fill: String,
    pub comment_fill: String,
    pub comment_stroke: String,
    pub comment_text_color: String,
    pub special_fill: String,
    pub special_stroke: String,
    pub rule_name_color: String,
    pub marker_radius: f64,
    pub use_max_width: bool,
}

#[derive(Debug, Clone)]
struct ExprLayout {
    width: f64,
    height: f64,
    up: f64,
    down: f64,
    elements: Vec<RailroadElementLayout>,
    paths: Vec<RailroadPathLayout>,
}

pub fn layout_railroad_diagram(
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    measurer: &dyn TextMeasurer,
) -> Result<RailroadDiagramLayout> {
    let model: RailroadDiagramRenderModel = crate::json::from_value_ref(semantic)?;
    layout_railroad_diagram_typed(&model, effective_config, measurer)
}

pub fn layout_railroad_diagram_typed(
    model: &RailroadDiagramRenderModel,
    effective_config: &serde_json::Value,
    measurer: &dyn TextMeasurer,
) -> Result<RailroadDiagramLayout> {
    let style = railroad_style(effective_config);
    let mut y = style.padding;
    let mut max_width: f64 = 0.0;
    let mut rules = Vec::new();

    for rule in &model.rules {
        let mut rule_layout = layout_rule(rule, y, &style, measurer);
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
        width,
        height,
        use_max_width: style.use_max_width,
        rules,
    })
}

pub(crate) fn railroad_style(effective_config: &serde_json::Value) -> RailroadStyle {
    fn theme_string(cfg: &serde_json::Value, key: &str) -> Option<String> {
        config_string(cfg, &["themeVariables", key])
    }
    fn railroad_string(cfg: &serde_json::Value, key: &str, fallback: String) -> String {
        config_string(cfg, &["railroad", key]).unwrap_or(fallback)
    }
    fn railroad_f64(cfg: &serde_json::Value, key: &str, fallback: f64) -> f64 {
        config_f64(cfg, &["railroad", key])
            .filter(|value| value.is_finite() && *value >= 0.0)
            .unwrap_or(fallback)
    }

    let font_family = railroad_string(
        effective_config,
        "fontFamily",
        theme_string(effective_config, "fontFamily").unwrap_or_else(|| "monospace".to_string()),
    );
    let font_size = railroad_f64(
        effective_config,
        "fontSize",
        config_f64(effective_config, &["themeVariables", "fontSize"]).unwrap_or(14.0),
    )
    .max(1.0);
    let line_color =
        theme_string(effective_config, "lineColor").unwrap_or_else(|| "#000000".into());
    let text_color =
        theme_string(effective_config, "textColor").unwrap_or_else(|| "#000000".into());

    RailroadStyle {
        padding: railroad_f64(effective_config, "padding", 10.0),
        vertical_separation: railroad_f64(effective_config, "verticalSeparation", 8.0),
        horizontal_separation: railroad_f64(effective_config, "horizontalSeparation", 10.0),
        arc_radius: railroad_f64(effective_config, "arcRadius", 10.0),
        font_size,
        font_family,
        terminal_fill: railroad_string(
            effective_config,
            "terminalFill",
            theme_string(effective_config, "secondBkg")
                .or_else(|| theme_string(effective_config, "secondaryColor"))
                .unwrap_or_else(|| "#FFFFC0".to_string()),
        ),
        terminal_stroke: railroad_string(
            effective_config,
            "terminalStroke",
            theme_string(effective_config, "secondaryBorderColor")
                .or_else(|| Some(line_color.clone()))
                .unwrap_or_else(|| "#000000".to_string()),
        ),
        terminal_text_color: railroad_string(
            effective_config,
            "terminalTextColor",
            theme_string(effective_config, "secondaryTextColor")
                .or_else(|| Some(text_color.clone()))
                .unwrap_or_else(|| "#000000".to_string()),
        ),
        non_terminal_fill: railroad_string(
            effective_config,
            "nonTerminalFill",
            theme_string(effective_config, "mainBkg")
                .or_else(|| theme_string(effective_config, "background"))
                .unwrap_or_else(|| "#FFFFFF".to_string()),
        ),
        non_terminal_stroke: railroad_string(
            effective_config,
            "nonTerminalStroke",
            theme_string(effective_config, "primaryBorderColor")
                .or_else(|| Some(line_color.clone()))
                .unwrap_or_else(|| "#000000".to_string()),
        ),
        non_terminal_text_color: railroad_string(
            effective_config,
            "nonTerminalTextColor",
            theme_string(effective_config, "primaryTextColor")
                .or_else(|| Some(text_color.clone()))
                .unwrap_or_else(|| "#000000".to_string()),
        ),
        line_color: railroad_string(effective_config, "lineColor", line_color.clone()),
        stroke_width: railroad_f64(effective_config, "strokeWidth", 2.0),
        marker_fill: railroad_string(effective_config, "markerFill", line_color),
        comment_fill: railroad_string(
            effective_config,
            "commentFill",
            theme_string(effective_config, "labelBackground")
                .or_else(|| theme_string(effective_config, "tertiaryColor"))
                .unwrap_or_else(|| "#E8E8E8".to_string()),
        ),
        comment_stroke: railroad_string(
            effective_config,
            "commentStroke",
            theme_string(effective_config, "tertiaryBorderColor")
                .unwrap_or_else(|| "#888888".to_string()),
        ),
        comment_text_color: railroad_string(
            effective_config,
            "commentTextColor",
            theme_string(effective_config, "tertiaryTextColor")
                .or_else(|| Some(text_color.clone()))
                .unwrap_or_else(|| "#666666".to_string()),
        ),
        special_fill: railroad_string(
            effective_config,
            "specialFill",
            theme_string(effective_config, "tertiaryColor")
                .or_else(|| theme_string(effective_config, "secondaryColor"))
                .unwrap_or_else(|| "#F0E0FF".to_string()),
        ),
        special_stroke: railroad_string(
            effective_config,
            "specialStroke",
            theme_string(effective_config, "tertiaryBorderColor")
                .or_else(|| theme_string(effective_config, "secondaryBorderColor"))
                .unwrap_or_else(|| "#8800CC".to_string()),
        ),
        rule_name_color: railroad_string(
            effective_config,
            "ruleNameColor",
            theme_string(effective_config, "titleColor")
                .or_else(|| Some(text_color))
                .unwrap_or_else(|| "#000066".to_string()),
        ),
        marker_radius: railroad_f64(effective_config, "markerRadius", 5.0),
        use_max_width: config_bool(effective_config, &["railroad", "useMaxWidth"]).unwrap_or(true),
    }
}

fn layout_rule(
    rule: &RailroadRuleModel,
    y: f64,
    style: &RailroadStyle,
    measurer: &dyn TextMeasurer,
) -> RailroadRuleLayout {
    let rule_name = format!("{} =", rule.name);
    let name_width = measure_text(rule_name.as_str(), style, measurer).0 + 20.0;
    let definition_x = name_width + 20.0;
    let definition = layout_expr(&rule.definition, style, measurer);
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
    measurer: &dyn TextMeasurer,
) -> ExprLayout {
    match node {
        RailroadAstNode::Terminal { value, .. } => layout_box("terminal", value, style, measurer),
        RailroadAstNode::NonTerminal { name, .. } => {
            layout_box("nonterminal", name, style, measurer)
        }
        RailroadAstNode::Special { text, .. } => {
            layout_box("special", &format!("? {text} ?"), style, measurer)
        }
        RailroadAstNode::Sequence { elements, .. } => layout_sequence(elements, style, measurer),
        RailroadAstNode::Choice { alternatives, .. } => {
            layout_choice(alternatives, style, measurer)
        }
        RailroadAstNode::Optional { element, .. } => layout_optional(element, style, measurer),
        RailroadAstNode::Repetition { element, min, .. } => {
            layout_repetition(element, *min, style, measurer)
        }
    }
}

fn layout_box(
    kind: &str,
    label: &str,
    style: &RailroadStyle,
    measurer: &dyn TextMeasurer,
) -> ExprLayout {
    let (text_width, text_height) = measure_text(label, style, measurer);
    let width = text_width + style.padding * 2.0;
    let height = text_height + style.padding * 2.0;
    ExprLayout {
        width,
        height,
        up: height / 2.0,
        down: height / 2.0,
        elements: vec![RailroadElementLayout {
            kind: kind.to_string(),
            label: label.to_string(),
            x: 0.0,
            y: 0.0,
            width,
            height,
            text_x: width / 2.0,
            text_y: height / 2.0,
        }],
        paths: Vec::new(),
    }
}

fn layout_sequence(
    elements: &[RailroadAstNode],
    style: &RailroadStyle,
    measurer: &dyn TextMeasurer,
) -> ExprLayout {
    let rendered: Vec<_> = elements
        .iter()
        .map(|element| layout_expr(element, style, measurer))
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
    };
    let mut x = 0.0;
    for (idx, mut child) in rendered.into_iter().enumerate() {
        let y = up - child.up;
        translate_elements(&mut child.elements, x, y);
        translate_paths(&mut child.paths, x, y);
        out.elements.extend(child.elements);
        out.paths.extend(child.paths);
        if idx + 1 < elements.len() {
            let line_x1 = x + child.width;
            let line_x2 = line_x1 + style.horizontal_separation;
            out.paths.push(railroad_path(
                PathBuilder::new()
                    .move_to(line_x1, up)
                    .line_to(line_x2, up)
                    .build(),
            ));
        }
        x += child.width + style.horizontal_separation;
    }
    out
}

fn layout_choice(
    alternatives: &[RailroadAstNode],
    style: &RailroadStyle,
    measurer: &dyn TextMeasurer,
) -> ExprLayout {
    let rendered: Vec<_> = alternatives
        .iter()
        .map(|alternative| layout_expr(alternative, style, measurer))
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
    };

    let mut y = 0.0;
    for mut child in rendered {
        let elem_y = y;
        let elem_center_y = elem_y + child.up;
        let elem_x = arc_radius * 2.0 + (max_width - child.width) / 2.0;
        let below_center = elem_center_y > center_y;
        let left_path = if (elem_center_y - center_y).abs() < f64::EPSILON {
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
        out.paths.push(railroad_path(left_path));

        let right_start = elem_x + child.width;
        let right_lane_x = total_width - arc_radius * 2.0;
        let right_path = if (elem_center_y - center_y).abs() < f64::EPSILON {
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
        out.paths.push(railroad_path(right_path));
        translate_elements(&mut child.elements, elem_x, elem_y);
        translate_paths(&mut child.paths, elem_x, elem_y);
        out.elements.extend(child.elements);
        out.paths.extend(child.paths);
        y += child.height + style.vertical_separation;
    }

    out
}

fn layout_optional(
    element: &RailroadAstNode,
    style: &RailroadStyle,
    measurer: &dyn TextMeasurer,
) -> ExprLayout {
    let mut inner = layout_expr(element, style, measurer);
    let arc_radius = style.arc_radius;
    let arc_height = arc_radius * 2.0;
    let total_width = inner.width + arc_radius * 4.0;
    let total_height = inner.height + arc_height;
    let elem_x = arc_radius * 2.0;
    let elem_y = arc_height;
    let center_y = elem_y + inner.up;
    translate_elements(&mut inner.elements, elem_x, elem_y);
    translate_paths(&mut inner.paths, elem_x, elem_y);

    let mut paths = inner.paths;
    paths.push(railroad_path(
        PathBuilder::new()
            .move_to(0.0, center_y)
            .line_to(arc_radius * 2.0, center_y)
            .build(),
    ));
    paths.push(railroad_path(
        PathBuilder::new()
            .move_to(elem_x + inner.width, center_y)
            .line_to(total_width, center_y)
            .build(),
    ));
    paths.push(railroad_path(
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
    ));

    ExprLayout {
        width: total_width,
        height: total_height,
        up: center_y,
        down: total_height - center_y,
        elements: inner.elements,
        paths,
    }
}

fn layout_repetition(
    element: &RailroadAstNode,
    min: u64,
    style: &RailroadStyle,
    measurer: &dyn TextMeasurer,
) -> ExprLayout {
    let mut inner = layout_expr(element, style, measurer);
    let arc_radius = style.arc_radius;
    let arc_height = arc_radius * 2.0;
    let total_width = inner.width + arc_radius * 4.0;
    let has_bypass = min == 0;
    let total_height = inner.height + arc_height + if has_bypass { arc_height } else { 0.0 };
    let elem_x = arc_radius * 2.0;
    let elem_y = if has_bypass { arc_height } else { 0.0 };
    let center_y = elem_y + inner.up;
    translate_elements(&mut inner.elements, elem_x, elem_y);
    translate_paths(&mut inner.paths, elem_x, elem_y);
    let mut paths = inner.paths;
    paths.push(railroad_path(
        PathBuilder::new()
            .move_to(0.0, center_y)
            .line_to(arc_radius * 2.0, center_y)
            .build(),
    ));
    paths.push(railroad_path(
        PathBuilder::new()
            .move_to(elem_x + inner.width, center_y)
            .line_to(total_width, center_y)
            .build(),
    ));

    let loop_y = elem_y + inner.height + arc_radius;
    paths.push(railroad_path(
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
    ));

    if has_bypass {
        paths.push(railroad_path(
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
        ));
    }

    ExprLayout {
        width: total_width,
        height: total_height,
        up: center_y,
        down: total_height - center_y,
        elements: inner.elements,
        paths,
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
    }
}

fn measure_text(text: &str, style: &RailroadStyle, measurer: &dyn TextMeasurer) -> (f64, f64) {
    let text_style = TextStyle {
        font_family: Some(style.font_family.clone()),
        font_size: style.font_size,
        font_weight: None,
    };
    let width = measurer
        .measure_svg_raw_text_bbox_width_px(text, &text_style)
        .max(text.chars().count() as f64 * style.font_size * 0.55);
    let height = measurer
        .measure_svg_simple_text_bbox_height_px(text, &text_style)
        .max(style.font_size);
    (width, height)
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
    let mut rounded = (value * 1000.0).round() / 1000.0;
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
    use crate::text::DeterministicTextMeasurer;

    #[test]
    fn railroad_layout_handles_sequence_choice_and_repetition() {
        let parsed = merman_core::Engine::new()
            .parse_diagram_for_render_model_sync(
                "railroad-beta\nexpr = sequence(nonterminal(\"term\"), zeroOrMore(terminal(\"+\"))) ;\n",
                merman_core::ParseOptions::strict(),
            )
            .unwrap()
            .expect("railroad render model parses");
        let merman_core::RenderSemanticModel::Railroad(model) = parsed.model else {
            panic!("expected railroad render model");
        };

        let layout = layout_railroad_diagram_typed(
            &model,
            &serde_json::json!({}),
            &DeterministicTextMeasurer::default(),
        )
        .unwrap();

        assert_eq!(layout.rules.len(), 1);
        assert!(layout.rules[0].width > 0.0);
        assert!(
            layout.rules[0]
                .paths
                .iter()
                .any(|path| path.d.contains('A'))
        );
        assert_eq!(layout.rules[0].elements.len(), 2);
    }
}
