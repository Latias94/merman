use super::model::{GraphEdgeStyle, GraphGroupStyle, GraphNodeStyle};
use crate::color::AsciiRgb;
use crate::error::{AsciiError, Result};
use crate::operation::AsciiExecution;
use crate::resource::{AsciiResourceLimitPhase, ResourceContext};
use crate::style_color::{parse_border_color, parse_css_color};
use merman_core::OperationPhase;
use merman_core::diagrams::flowchart::FlowchartModel;

#[derive(Clone, Copy)]
struct StyleTargets {
    node: bool,
    edge: bool,
    group: bool,
}

impl StyleTargets {
    const ALL: Self = Self {
        node: true,
        edge: true,
        group: true,
    };
    const NODE: Self = Self {
        node: true,
        edge: false,
        group: false,
    };
    const EDGE: Self = Self {
        node: false,
        edge: true,
        group: false,
    };
    const GROUP: Self = Self {
        node: false,
        edge: false,
        group: true,
    };
    const NODE_AND_GROUP: Self = Self {
        node: true,
        edge: false,
        group: true,
    };
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PreparedGraphStyle {
    node_text: Option<Option<AsciiRgb>>,
    node_border: Option<Option<AsciiRgb>>,
    node_background: Option<Option<AsciiRgb>>,
    edge_line: Option<Option<AsciiRgb>>,
    edge_arrow: Option<Option<AsciiRgb>>,
    edge_label: Option<Option<AsciiRgb>>,
    group_title: Option<Option<AsciiRgb>>,
    group_border: Option<Option<AsciiRgb>>,
    group_background: Option<Option<AsciiRgb>>,
}

impl PreparedGraphStyle {
    pub(crate) fn apply_node(self, style: &mut GraphNodeStyle) {
        if let Some(value) = self.node_text {
            style.text = value;
        }
        if let Some(value) = self.node_border {
            style.border = value;
        }
        if let Some(value) = self.node_background {
            style.background = value;
        }
    }

    fn apply_edge(self, style: &mut GraphEdgeStyle) {
        if let Some(value) = self.edge_line {
            style.line = value;
        }
        if let Some(value) = self.edge_arrow {
            style.arrow = value;
        }
        if let Some(value) = self.edge_label {
            style.label = value;
        }
    }

    pub(crate) fn apply_group(self, style: &mut GraphGroupStyle) {
        if let Some(value) = self.group_title {
            style.title = value;
        }
        if let Some(value) = self.group_border {
            style.border = value;
        }
        if let Some(value) = self.group_background {
            style.background = value;
        }
    }
}

pub(super) struct FlowchartStylePlan {
    pub(super) nodes: Vec<GraphNodeStyle>,
    pub(super) edges: Vec<GraphEdgeStyle>,
    pub(super) groups: Vec<GraphGroupStyle>,
}

impl FlowchartStylePlan {
    pub(super) fn try_new(
        model: &FlowchartModel,
        is_group_id: impl Fn(&str) -> bool,
        resources: &ResourceContext,
        execution: AsciiExecution<'_>,
    ) -> Result<Self> {
        let mut class_styles = Vec::new();
        class_styles
            .try_reserve_exact(model.class_defs.len())
            .map_err(|_| style_allocation_failed())?;
        for (index, declarations) in model.class_defs.values().enumerate() {
            checkpoint_style(execution, index)?;
            class_styles.push(prepare_style_declarations(
                declarations.iter().map(String::as_str),
                StyleTargets::ALL,
                resources,
                execution,
            )?);
        }

        let edge_defaults = model
            .edge_defaults
            .as_ref()
            .map(|defaults| {
                prepare_style_declarations(
                    defaults.style.iter().map(String::as_str),
                    StyleTargets::EDGE,
                    resources,
                    execution,
                )
            })
            .transpose()?
            .unwrap_or_default();

        let mut nodes = Vec::new();
        nodes
            .try_reserve_exact(model.nodes.len())
            .map_err(|_| style_allocation_failed())?;
        for (index, node) in model.nodes.iter().enumerate() {
            checkpoint_style(execution, index)?;
            let mut style = GraphNodeStyle::default();
            if is_group_id(&node.id) {
                nodes.push(style);
                continue;
            }
            for (class_index, class_name) in node.classes.iter().enumerate() {
                checkpoint_style(execution, class_index)?;
                if let Some(class_index) = model.class_defs.get_index_of(class_name) {
                    class_styles[class_index].apply_node(&mut style);
                }
            }
            prepare_style_declarations(
                node.styles.iter().map(String::as_str),
                StyleTargets::NODE,
                resources,
                execution,
            )?
            .apply_node(&mut style);
            nodes.push(style);
        }

        let mut edges = Vec::new();
        edges
            .try_reserve_exact(model.edges.len())
            .map_err(|_| style_allocation_failed())?;
        for (index, edge) in model.edges.iter().enumerate() {
            checkpoint_style(execution, index)?;
            let mut style = GraphEdgeStyle::default();
            edge_defaults.apply_edge(&mut style);
            for (class_index, class_name) in edge.classes.iter().enumerate() {
                checkpoint_style(execution, class_index)?;
                if let Some(class_index) = model.class_defs.get_index_of(class_name) {
                    class_styles[class_index].apply_edge(&mut style);
                }
            }
            prepare_style_declarations(
                edge.style.iter().map(String::as_str),
                StyleTargets::EDGE,
                resources,
                execution,
            )?
            .apply_edge(&mut style);
            edges.push(style);
        }

        let mut groups = Vec::new();
        groups
            .try_reserve_exact(model.subgraphs.len())
            .map_err(|_| style_allocation_failed())?;
        for (index, group) in model.subgraphs.iter().enumerate() {
            checkpoint_style(execution, index)?;
            let mut style = GraphGroupStyle::default();
            let (classes, declarations) = match &group.same_id_vertex_style {
                Some(vertex) => (vertex.classes.as_slice(), vertex.styles.as_slice()),
                None => (group.classes.as_slice(), group.styles.as_slice()),
            };
            for (class_index, class_name) in classes.iter().enumerate() {
                checkpoint_style(execution, class_index)?;
                if let Some(class_index) = model.class_defs.get_index_of(class_name) {
                    class_styles[class_index].apply_group(&mut style);
                }
            }
            prepare_style_declarations(
                declarations.iter().map(String::as_str),
                StyleTargets::GROUP,
                resources,
                execution,
            )?
            .apply_group(&mut style);
            groups.push(style);
        }

        Ok(Self {
            nodes,
            edges,
            groups,
        })
    }
}

pub(crate) fn prepare_state_style<'a>(
    declarations: impl IntoIterator<Item = &'a str>,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<PreparedGraphStyle> {
    prepare_style_declarations(
        declarations,
        StyleTargets::NODE_AND_GROUP,
        resources,
        execution,
    )
}

fn prepare_style_declarations<'a>(
    declarations: impl IntoIterator<Item = &'a str>,
    targets: StyleTargets,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<PreparedGraphStyle> {
    let mut prepared = PreparedGraphStyle::default();
    for (index, declaration) in declarations.into_iter().enumerate() {
        checkpoint_style(execution, index)?;
        let scan_work = resources.checked_work_add(declaration.len(), 1)?;
        resources.charge_layout_work(scan_work)?;
        parse_style_declaration(&mut prepared, declaration, targets, resources, execution)?;
    }
    Ok(prepared)
}

fn parse_style_declaration(
    prepared: &mut PreparedGraphStyle,
    declaration: &str,
    targets: StyleTargets,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    let mut colon_seen = false;
    let mut name_start = None;
    let mut name_end = 0usize;
    let mut value_start = None;
    let mut value_end = 0usize;

    for (iteration, (byte_index, ch)) in declaration.char_indices().enumerate() {
        checkpoint_style(execution, iteration)?;
        if matches!(ch, ',' | ';') {
            apply_style_property(
                prepared,
                declaration,
                name_start.zip(Some(name_end)),
                value_start.zip(Some(value_end)),
                targets,
                resources,
            )?;
            colon_seen = false;
            name_start = None;
            name_end = 0;
            value_start = None;
            value_end = 0;
            continue;
        }
        if !colon_seen && ch == ':' {
            colon_seen = true;
            continue;
        }
        if ch.is_whitespace() {
            continue;
        }
        let end = byte_index + ch.len_utf8();
        if colon_seen {
            value_start.get_or_insert(byte_index);
            value_end = end;
        } else {
            name_start.get_or_insert(byte_index);
            name_end = end;
        }
    }

    apply_style_property(
        prepared,
        declaration,
        name_start.zip(Some(name_end)),
        value_start.zip(Some(value_end)),
        targets,
        resources,
    )
}

fn apply_style_property(
    prepared: &mut PreparedGraphStyle,
    declaration: &str,
    name: Option<(usize, usize)>,
    value: Option<(usize, usize)>,
    targets: StyleTargets,
    resources: &ResourceContext,
) -> Result<()> {
    let (Some((name_start, name_end)), Some((value_start, value_end))) = (name, value) else {
        return Ok(());
    };
    let name = &declaration[name_start..name_end];
    let value = &declaration[value_start..value_end];

    match style_property(name) {
        Some(StyleProperty::Color) => {
            charge_style_value_parse(resources, value, 1)?;
            let color = parse_css_color(value);
            if targets.node {
                prepared.node_text = Some(color);
            }
            if targets.edge {
                prepared.edge_label = Some(color);
            }
            if targets.group {
                prepared.group_title = Some(color);
            }
        }
        Some(StyleProperty::Stroke) => {
            if targets.node || targets.group {
                charge_style_value_parse(resources, value, 2)?;
                let color = parse_border_color(value);
                if targets.node {
                    prepared.node_border = Some(color);
                }
                if targets.group {
                    prepared.group_border = Some(color);
                }
            }
            if targets.edge {
                charge_style_value_parse(resources, value, 1)?;
                let color = parse_css_color(value);
                prepared.edge_line = Some(color);
                prepared.edge_arrow = Some(color);
            }
        }
        Some(StyleProperty::Border) if targets.node || targets.group => {
            charge_style_value_parse(resources, value, 2)?;
            let color = parse_border_color(value);
            if targets.node {
                prepared.node_border = Some(color);
            }
            if targets.group {
                prepared.group_border = Some(color);
            }
        }
        Some(StyleProperty::Fill | StyleProperty::Background) if targets.node || targets.group => {
            charge_style_value_parse(resources, value, 1)?;
            let color = parse_css_color(value);
            if targets.node {
                prepared.node_background = Some(color);
            }
            if targets.group {
                prepared.group_background = Some(color);
            }
        }
        Some(StyleProperty::Border | StyleProperty::Fill | StyleProperty::Background) | None => {}
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum StyleProperty {
    Color,
    Stroke,
    Border,
    Fill,
    Background,
}

fn style_property(name: &str) -> Option<StyleProperty> {
    match name.len() {
        4 if name.eq_ignore_ascii_case("fill") => Some(StyleProperty::Fill),
        5 if name.eq_ignore_ascii_case("color") => Some(StyleProperty::Color),
        6 if name.eq_ignore_ascii_case("stroke") => Some(StyleProperty::Stroke),
        6 if name.eq_ignore_ascii_case("border") => Some(StyleProperty::Border),
        10 if name.eq_ignore_ascii_case("background") => Some(StyleProperty::Background),
        _ => None,
    }
}

fn charge_style_value_parse(
    resources: &ResourceContext,
    value: &str,
    parser_passes: usize,
) -> Result<()> {
    resources.charge_layout_work_product(value.len().max(1), parser_passes)
}

fn checkpoint_style(execution: AsciiExecution<'_>, iteration: usize) -> Result<()> {
    execution.checkpoint_loop(OperationPhase::Semantic, iteration)
}

fn style_allocation_failed() -> AsciiError {
    AsciiError::AllocationFailed {
        phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
    }
}
