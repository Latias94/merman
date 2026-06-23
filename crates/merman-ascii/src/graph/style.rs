use super::model::{GraphEdgeStyle, GraphGroupStyle, GraphNodeStyle};
use crate::color::AsciiRgb;
use merman_core::diagrams::flowchart::{FlowEdge, FlowNode, FlowSubgraph, FlowchartV2Model};

pub(super) fn resolve_node_style(model: &FlowchartV2Model, node: &FlowNode) -> GraphNodeStyle {
    let mut style = GraphNodeStyle::default();
    for class_name in &node.classes {
        if let Some(class_styles) = model.class_defs.get(class_name) {
            apply_node_declarations(&mut style, class_styles);
        }
    }
    apply_node_declarations(&mut style, &node.styles);
    style
}

pub(super) fn resolve_edge_style(model: &FlowchartV2Model, edge: &FlowEdge) -> GraphEdgeStyle {
    let mut style = GraphEdgeStyle::default();
    if let Some(defaults) = &model.edge_defaults {
        apply_edge_declarations(&mut style, &defaults.style);
    }
    for class_name in &edge.classes {
        if let Some(class_styles) = model.class_defs.get(class_name) {
            apply_edge_declarations(&mut style, class_styles);
        }
    }
    apply_edge_declarations(&mut style, &edge.style);
    style
}

pub(super) fn resolve_group_style(
    model: &FlowchartV2Model,
    group: &FlowSubgraph,
) -> GraphGroupStyle {
    let mut style = GraphGroupStyle::default();
    for class_name in &group.classes {
        if let Some(class_styles) = model.class_defs.get(class_name) {
            apply_group_declarations(&mut style, class_styles);
        }
    }
    apply_group_declarations(&mut style, &group.styles);
    style
}

pub(crate) fn apply_node_declarations(style: &mut GraphNodeStyle, declarations: &[String]) {
    for declaration in declarations {
        apply_node_declaration(style, declaration);
    }
}

pub(crate) fn apply_node_declaration(style: &mut GraphNodeStyle, declaration: &str) {
    for (name, value) in style_declaration(declaration) {
        if name.eq_ignore_ascii_case("color") {
            style.text = parse_css_color(value);
        } else if name.eq_ignore_ascii_case("stroke") || name.eq_ignore_ascii_case("border") {
            style.border = parse_border_color(value);
        }
    }
}

fn apply_edge_declarations(style: &mut GraphEdgeStyle, declarations: &[String]) {
    for (name, value) in style_declarations(declarations) {
        if name.eq_ignore_ascii_case("stroke") {
            let color = parse_css_color(value);
            style.line = color;
            style.arrow = color;
        } else if name.eq_ignore_ascii_case("color") {
            style.label = parse_css_color(value);
        }
    }
}

pub(crate) fn apply_group_declarations(style: &mut GraphGroupStyle, declarations: &[String]) {
    for declaration in declarations {
        apply_group_declaration(style, declaration);
    }
}

pub(crate) fn apply_group_declaration(style: &mut GraphGroupStyle, declaration: &str) {
    for (name, value) in style_declaration(declaration) {
        if name.eq_ignore_ascii_case("color") {
            style.title = parse_css_color(value);
        } else if name.eq_ignore_ascii_case("stroke") || name.eq_ignore_ascii_case("border") {
            style.border = parse_border_color(value);
        }
    }
}

fn style_declarations(declarations: &[String]) -> impl Iterator<Item = (&str, &str)> {
    declarations
        .iter()
        .flat_map(|declaration| style_declaration(declaration))
}

fn style_declaration(declaration: &str) -> impl Iterator<Item = (&str, &str)> {
    declaration.split([',', ';']).filter_map(|declaration| {
        let (name, value) = declaration.split_once(':')?;
        let name = name.trim();
        let value = value.trim();
        (!name.is_empty() && !value.is_empty()).then_some((name, value))
    })
}

fn parse_css_color(value: &str) -> Option<AsciiRgb> {
    let value = value.trim().trim_end_matches(';').trim();
    if value.eq_ignore_ascii_case("transparent") || value.eq_ignore_ascii_case("none") {
        return None;
    }
    if let Some(hex) = value.strip_prefix('#') {
        return parse_hex_color(hex);
    }
    parse_named_color(value)
}

fn parse_border_color(value: &str) -> Option<AsciiRgb> {
    parse_css_color(value).or_else(|| value.split_whitespace().rev().find_map(parse_css_color))
}

fn parse_hex_color(hex: &str) -> Option<AsciiRgb> {
    match hex.len() {
        3 => {
            let r = parse_hex_digit(hex.as_bytes()[0])?;
            let g = parse_hex_digit(hex.as_bytes()[1])?;
            let b = parse_hex_digit(hex.as_bytes()[2])?;
            Some(AsciiRgb::new(r * 17, g * 17, b * 17))
        }
        6 => {
            let rgb = u32::from_str_radix(hex, 16).ok()?;
            Some(AsciiRgb::from_hex24(rgb))
        }
        _ => None,
    }
}

fn parse_hex_digit(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        b'A'..=b'F' => Some(digit - b'A' + 10),
        _ => None,
    }
}

fn parse_named_color(value: &str) -> Option<AsciiRgb> {
    match value.to_ascii_lowercase().as_str() {
        "black" => Some(AsciiRgb::from_hex24(0x000000)),
        "white" => Some(AsciiRgb::from_hex24(0xffffff)),
        "red" => Some(AsciiRgb::from_hex24(0xff0000)),
        "green" => Some(AsciiRgb::from_hex24(0x008000)),
        "blue" => Some(AsciiRgb::from_hex24(0x0000ff)),
        "yellow" => Some(AsciiRgb::from_hex24(0xffff00)),
        "cyan" | "aqua" => Some(AsciiRgb::from_hex24(0x00ffff)),
        "magenta" | "fuchsia" => Some(AsciiRgb::from_hex24(0xff00ff)),
        "gray" | "grey" => Some(AsciiRgb::from_hex24(0x808080)),
        "orange" => Some(AsciiRgb::from_hex24(0xffa500)),
        "purple" => Some(AsciiRgb::from_hex24(0x800080)),
        _ => None,
    }
}
