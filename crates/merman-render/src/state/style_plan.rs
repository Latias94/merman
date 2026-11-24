use indexmap::IndexMap;
use merman_model::families::state::{
    StateDiagramRenderEdge, StateDiagramRenderModel, StateDiagramRenderNode,
};
use merman_model::presentation::PresentationConfig;
use std::collections::{BTreeMap, BTreeSet};

#[cfg(any(feature = "layout-dagre", test))]
use crate::layout_backend;
use crate::layout_backend::GraphLayoutBackend;
use crate::mermaid_style::{
    CssFontSizeContext, ParsedStyleDeclaration, is_label_style_key, is_safe_css_font_family_value,
    is_supported_css_font_style_value, is_supported_css_font_weight_value, parse_style_declaration,
};
use crate::text::{FontStyle, TextMeasurer, TextMetrics, TextStyle, WrapMode};
use crate::theme::{Color, FamilyTheme};

use super::{StateCompatibilityPlan, StateNodeKind};

/// One source-resolved font decision shared by State measurement and SVG emission.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedLabelTypography {
    text_style: TextStyle,
    svg_inline_weight: bool,
    svg_inline_style: bool,
}

impl ResolvedLabelTypography {
    pub(crate) fn new(text_style: TextStyle) -> Self {
        Self {
            text_style,
            svg_inline_weight: false,
            svg_inline_style: false,
        }
    }

    pub(crate) const fn text_style(&self) -> &TextStyle {
        &self.text_style
    }

    fn apply_font_family(&mut self, stack: crate::text::ParsedCssFontStack) {
        self.text_style.font_family = Some(stack.as_css());
    }

    fn apply_font_weight(&mut self, value: u16) {
        self.text_style.font_weight = Some(value.to_string());
        self.svg_inline_weight = true;
    }

    fn apply_font_style(&mut self, value: FontStyle) {
        self.text_style.font_style = Some(value.id().to_string());
        self.svg_inline_style = true;
    }

    fn canonicalize_emission(&mut self, out: &mut IndexMap<String, EmittedDeclaration>) {
        self.svg_inline_weight |= out.contains_key("font-weight");
        self.svg_inline_style |= out.contains_key("font-style");
        if out.contains_key("font-family")
            && let Some(family) = self.text_style.font_family.as_deref()
        {
            insert_emitted(out, "font-family", family);
        }
        if out.contains_key("font-size") {
            insert_emitted(out, "font-size", format!("{}px", self.text_style.font_size));
        }
        if out.contains_key("font-weight")
            && let Some(weight) = self.text_style.font_weight.as_deref()
        {
            insert_emitted(out, "font-weight", weight);
        }
        if out.contains_key("font-style")
            && let Some(font_style) = self.text_style.font_style.as_deref()
        {
            insert_emitted(out, "font-style", font_style);
        }
        out.shift_remove("letter-spacing");
        out.shift_remove("word-spacing");
    }

    pub(super) fn svg_measurer<'a>(
        &'a self,
        inner: &'a dyn TextMeasurer,
        wrap_mode: WrapMode,
    ) -> StateLabelMeasurer<'a> {
        StateLabelMeasurer {
            inner,
            typography: (wrap_mode != WrapMode::HtmlLike
                && (self.svg_inline_weight || self.svg_inline_style))
                .then_some(self),
        }
    }
}

/// Inner inline declarations override the SVG Markdown word's presentation attributes.
pub(super) struct StateLabelMeasurer<'a> {
    inner: &'a dyn TextMeasurer,
    typography: Option<&'a ResolvedLabelTypography>,
}

impl StateLabelMeasurer<'_> {
    pub(super) fn as_measurer(&self) -> &dyn TextMeasurer {
        if self.typography.is_some() {
            self
        } else {
            self.inner
        }
    }

    fn final_style(&self, style: &TextStyle) -> TextStyle {
        let mut style = style.clone();
        if let Some(typography) = self.typography {
            if typography.svg_inline_weight {
                style.font_weight = typography.text_style.font_weight.clone();
            }
            if typography.svg_inline_style {
                style.font_style = typography.text_style.font_style.clone();
            }
        }
        style
    }
}

impl TextMeasurer for StateLabelMeasurer<'_> {
    fn cancellation_requested(&self) -> bool {
        self.inner.cancellation_requested()
    }

    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.inner.measure(text, &self.final_style(style))
    }

    fn measure_wrapped(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> TextMetrics {
        self.inner
            .measure_wrapped(text, &self.final_style(style), max_width, wrap_mode)
    }

    fn measure_svg_text_computed_length_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.inner
            .measure_svg_text_computed_length_px(text, &self.final_style(style))
    }

    fn measure_svg_text_bbox_x(&self, text: &str, style: &TextStyle) -> (f64, f64) {
        self.inner
            .measure_svg_text_bbox_x(text, &self.final_style(style))
    }

    fn measure_svg_tspan_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.inner
            .measure_svg_tspan_text_bbox_height_px(text, &self.final_style(style))
    }

    fn measure_svg_create_text_bbox_y_offset_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.inner
            .measure_svg_create_text_bbox_y_offset_px(text, &self.final_style(style))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct StateCssDeclaration {
    pub(crate) property: String,
    pub(crate) source_name: String,
    pub(crate) value: String,
}

impl StateCssDeclaration {
    fn from_source(declaration: &ParsedStyleDeclaration<'_>) -> Self {
        Self {
            property: declaration.property().to_owned(),
            source_name: declaration.property_css().to_owned(),
            value: declaration.value().to_owned(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct StateClassStylePlan {
    id: String,
    styles: Vec<StateCssDeclaration>,
    text_styles: Vec<StateCssDeclaration>,
}

impl StateClassStylePlan {
    pub(crate) fn id(&self) -> &str {
        &self.id
    }
    pub(crate) fn styles(&self) -> &[StateCssDeclaration] {
        &self.styles
    }
    pub(crate) fn text_styles(&self) -> &[StateCssDeclaration] {
        &self.text_styles
    }
}

#[derive(Debug, Clone)]
pub(crate) struct StateNodeStylePlan {
    shape_style_attr: String,
    fill_path_style_attr: String,
    stroke_path_style_attr: String,
    special_state_inner_fill_path_style_attr: String,
    special_state_inner_stroke_path_style_attr: String,
    label_style_attr: String,
    div_style_prefix: String,
    fill_override: Option<String>,
    composite_fill_opacity: Option<f64>,
    stroke_override: Option<String>,
    stroke_width_override: Option<f64>,
    radius_override: Option<f64>,
    composite_outer_radius_override: Option<f64>,
    padding_override: Option<f64>,
    cluster_label_typography: Option<ResolvedLabelTypography>,
    label_typography: ResolvedLabelTypography,
}

impl StateNodeStylePlan {
    pub(crate) fn shape_style_attr(&self) -> &str {
        &self.shape_style_attr
    }

    pub(crate) fn fill_path_style_attr(&self) -> &str {
        &self.fill_path_style_attr
    }

    pub(crate) fn stroke_path_style_attr(&self) -> &str {
        &self.stroke_path_style_attr
    }

    pub(crate) fn special_state_inner_fill_path_style_attr(&self) -> &str {
        &self.special_state_inner_fill_path_style_attr
    }

    pub(crate) fn special_state_inner_stroke_path_style_attr(&self) -> &str {
        &self.special_state_inner_stroke_path_style_attr
    }

    pub(crate) fn label_style_attr(&self) -> &str {
        &self.label_style_attr
    }

    pub(crate) fn div_style_prefix(&self) -> &str {
        &self.div_style_prefix
    }

    pub(crate) fn fill_override(&self) -> Option<&str> {
        self.fill_override.as_deref()
    }

    pub(crate) const fn composite_fill_opacity(&self) -> Option<f64> {
        self.composite_fill_opacity
    }

    pub(crate) fn stroke_override(&self) -> Option<&str> {
        self.stroke_override.as_deref()
    }

    pub(crate) const fn stroke_width_override(&self) -> Option<f64> {
        self.stroke_width_override
    }

    pub(crate) const fn radius_override(&self) -> Option<f64> {
        self.radius_override
    }

    pub(crate) const fn composite_outer_radius_override(&self) -> Option<f64> {
        self.composite_outer_radius_override
    }

    pub(crate) const fn padding_override(&self) -> Option<f64> {
        self.padding_override
    }

    pub(crate) const fn text_style(&self) -> &TextStyle {
        self.label_typography.text_style()
    }

    pub(crate) const fn resolved_label_typography(&self) -> &ResolvedLabelTypography {
        &self.label_typography
    }

    pub(crate) const fn resolved_cluster_label_typography(&self) -> &ResolvedLabelTypography {
        match &self.cluster_label_typography {
            Some(typography) => typography,
            None => &self.label_typography,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct StateStylePlan {
    compatibility: StateCompatibilityPlan,
    base_label_typography: ResolvedLabelTypography,
    title_text_style: TextStyle,
    title_style_attr: String,
    classes: IndexMap<String, StateClassStylePlan>,
    nodes: BTreeMap<String, StateNodeStylePlan>,
    shadowed_self_loop_edges: BTreeSet<String>,
}

impl StateStylePlan {
    #[cfg(any(feature = "layout-dagre", test))]
    pub(crate) fn resolve_unthemed(
        model: &StateDiagramRenderModel,
        config: &serde_json::Value,
    ) -> Self {
        Self::prepare(
            model,
            StateCompatibilityPlan::from_value(config),
            layout_backend::resolve(crate::DiagramFamilyId::STATE, "stateDiagram", config)
                .unwrap_or(GraphLayoutBackend::Dagre),
            None,
        )
    }

    pub(crate) fn resolve(
        model: &StateDiagramRenderModel,
        config: &PresentationConfig,
        backend: GraphLayoutBackend,
        native: Option<&FamilyTheme>,
        background: Option<Color>,
    ) -> Self {
        let compatibility = StateCompatibilityPlan::from_config(config);
        let compatibility = match native {
            Some(native) => compatibility.with_native(native, config, background),
            None => compatibility,
        };
        Self::prepare(model, compatibility, backend, native)
    }

    fn prepare(
        model: &StateDiagramRenderModel,
        compatibility: StateCompatibilityPlan,
        backend: GraphLayoutBackend,
        native: Option<&FamilyTheme>,
    ) -> Self {
        let base = TextStyle {
            font_family: Some(compatibility.font_family_css.clone()),
            font_size: crate::number_format::canonicalize_number(compatibility.font_size_px),
            font_weight: compatibility.native_label_weight.clone(),
            font_style: None,
        };
        let mut title_text_style =
            compatibility
                .native_title
                .clone()
                .unwrap_or_else(|| TextStyle {
                    font_size: 18.0,
                    ..base.clone()
                });
        title_text_style.font_size =
            crate::number_format::canonicalize_number(title_text_style.font_size);
        let title_style_attr = native.map_or_else(String::new, |theme| {
            let size = crate::number_format::canonical_number(title_text_style.font_size);
            let family = title_text_style
                .font_family
                .as_deref()
                .unwrap_or("sans-serif");
            let weight = title_text_style.font_weight.as_deref().unwrap_or("normal");
            format!(
                "font-family:{family};font-size:{size}px;font-weight:{weight};fill:{}",
                theme.roles.text_title
            )
        });
        let parse_declarations = |raw: &[String]| {
            raw.iter()
                .filter_map(|css| parse_style_declaration(css))
                .map(|declaration| StateCssDeclaration::from_source(&declaration))
                .collect()
        };
        let classes = model
            .style_classes
            .iter()
            .map(|(key, class)| {
                (
                    key.clone(),
                    StateClassStylePlan {
                        id: class.id.clone(),
                        styles: parse_declarations(&class.styles),
                        text_styles: parse_declarations(&class.text_styles),
                    },
                )
            })
            .collect();
        let nodes = model
            .nodes
            .iter()
            .map(|node| {
                let plan = prepare_node(node, model, &base, &compatibility, native);
                (node.id.clone(), plan)
            })
            .collect();
        let shadowed_self_loop_edges = if backend == GraphLayoutBackend::Dagre {
            let hidden_prefixes = hidden_state_prefixes(model);
            shadowed_self_loop_edge_indices(model, &hidden_prefixes)
                .into_iter()
                .map(|index| model.edges[index].id.clone())
                .collect()
        } else {
            BTreeSet::new()
        };
        Self {
            compatibility,
            base_label_typography: ResolvedLabelTypography::new(base),
            title_text_style,
            title_style_attr,
            classes,
            nodes,
            shadowed_self_loop_edges,
        }
    }

    pub(crate) const fn compatibility(&self) -> &StateCompatibilityPlan {
        &self.compatibility
    }
    pub(crate) const fn uses_structured_typography(&self) -> bool {
        self.compatibility.native_title.is_some()
    }
    pub(crate) const fn base_text_style(&self) -> &TextStyle {
        self.base_label_typography.text_style()
    }
    pub(crate) const fn base_label_typography(&self) -> &ResolvedLabelTypography {
        &self.base_label_typography
    }
    pub(crate) const fn transition_label_typography(&self) -> &ResolvedLabelTypography {
        &self.base_label_typography
    }
    pub(crate) const fn composite_label_typography(&self) -> &ResolvedLabelTypography {
        &self.base_label_typography
    }
    pub(crate) const fn title_text_style(&self) -> &TextStyle {
        &self.title_text_style
    }
    pub(crate) fn title_style_attr(&self) -> &str {
        &self.title_style_attr
    }
    pub(crate) fn classes(&self) -> impl Iterator<Item = &StateClassStylePlan> {
        self.classes.values()
    }
    pub(crate) fn node(&self, id: &str) -> Option<&StateNodeStylePlan> {
        self.nodes.get(id)
    }
    pub(crate) fn is_shadowed_self_loop_edge(&self, id: &str) -> bool {
        self.shadowed_self_loop_edges.contains(id)
    }
}

fn source_compositing_property(property: &str) -> &str {
    // Normalize known browser aliases for ownership and winner selection only;
    // EmittedDeclaration retains the authored CSS property spelling.
    match property {
        "-webkit-opacity" => "opacity",
        "-webkit-filter" => "filter",
        _ => property,
    }
}

fn prepare_node(
    node: &StateDiagramRenderNode,
    model: &StateDiagramRenderModel,
    base: &TextStyle,
    compatibility: &StateCompatibilityPlan,
    native: Option<&FamilyTheme>,
) -> StateNodeStylePlan {
    let kind = StateNodeKind::from_node(node);
    let surface = compatibility.node_shape_surface(node, kind);
    let composite = kind == StateNodeKind::Composite;
    let classic_body =
        kind == StateNodeKind::Ordinary && node.shape == "rect" && compatibility.is_classic();
    let mut shape = IndexMap::new();
    let mut label = IndexMap::new();
    let mut typography = ResolvedLabelTypography::new(base.clone());
    let mut group_typography = (super::config::state_node_is_effective_group(node) && !composite)
        .then(|| typography.clone());
    let mut fill_override = None;
    let mut stroke_override = None;
    let mut stroke_width_override = None;
    let mut radius_override = None;
    let mut padding_override = None;

    let structural_node = (node.shape == "rect"
        && !super::config::state_node_is_effective_group(node))
    .then(|| model.style_classes.get_full("node"))
    .flatten();
    let mut classes = node
        .css_classes
        .split_whitespace()
        .filter_map(|id| {
            let (index, _, class) = model.style_classes.get_full(id)?;
            Some((index, id == "node" && structural_node.is_some(), class))
        })
        .collect::<Vec<_>>();
    if let Some((index, _, class)) = structural_node {
        if !classes.iter().any(|(_, structural, _)| *structural) {
            classes.push((index, true, class));
        }
        classes.sort_by_key(|(index, _, _)| *index);
    }

    let mut apply = |declaration: ParsedStyleDeclaration<'_>, origin: SourceStyle| {
        if composite
            && !matches!(
                origin,
                SourceStyle::AssignedClass | SourceStyle::AssignedText
            )
        {
            return;
        }
        let property = declaration.property();
        if origin.is_label(property, compatibility) {
            if let Some(group) = &mut group_typography
                && matches!(
                    origin,
                    SourceStyle::AssignedClass | SourceStyle::AssignedText
                )
            {
                apply_source_text_property(&declaration, base, group, false);
            }
            if apply_source_text_property(
                &declaration,
                base,
                &mut typography,
                origin == SourceStyle::StructuralText,
            ) {
                if matches!(property, "color" | "fill") {
                    label.shift_remove(if property == "fill" { "color" } else { "fill" });
                }
                label.insert(
                    property.to_owned(),
                    EmittedDeclaration::from_source(&declaration),
                );
            }
        } else if origin.can_style_shape(property, compatibility)
            && surface.is_none_or(|surface| {
                compatibility.source_shape_property_reaches(surface, property)
            })
            && ((classic_body && source_compositing_property(property) == "filter")
                || apply_source_shape_property(
                    &declaration,
                    &mut fill_override,
                    &mut stroke_override,
                    &mut stroke_width_override,
                    &mut radius_override,
                    &mut padding_override,
                ))
        {
            shape.insert(
                source_compositing_property(property).to_owned(),
                EmittedDeclaration::from_source(&declaration),
            );
        }
    };
    for (_, structural, class) in classes {
        for declaration in class
            .styles
            .iter()
            .filter_map(|css| parse_style_declaration(css))
        {
            apply(
                declaration,
                if structural {
                    SourceStyle::StructuralShape
                } else {
                    SourceStyle::AssignedClass
                },
            );
        }
        for declaration in class
            .text_styles
            .iter()
            .filter_map(|css| parse_style_declaration(css))
        {
            apply(
                declaration,
                if structural {
                    SourceStyle::StructuralText
                } else {
                    SourceStyle::AssignedText
                },
            );
        }
    }
    for declaration in node
        .css_styles
        .iter()
        .map(String::as_str)
        .chain(node.label_style.split(';'))
        .filter_map(parse_style_declaration)
    {
        apply(declaration, SourceStyle::Inline);
    }
    typography.canonicalize_emission(&mut label);
    let composite_outer_radius_override =
        radius_override.or_else(|| native.map(|theme| theme.form.container_radius));
    let radius_override = radius_override.or_else(|| native.map(|theme| theme.form.radius));
    let label_style_attr = compact_style_attr(&label);
    let div_style_prefix = div_style_prefix(&label);
    StateNodeStylePlan {
        shape_style_attr: compact_style_attr(&shape),
        fill_path_style_attr: compact_split_fill_path_style_attr(&shape),
        stroke_path_style_attr: compact_split_stroke_path_style_attr(&shape),
        special_state_inner_fill_path_style_attr: compact_split_fill_path_style_attr(&shape),
        special_state_inner_stroke_path_style_attr: compact_split_stroke_path_style_attr(&shape),
        label_style_attr,
        div_style_prefix,
        fill_override,
        composite_fill_opacity: if composite && native.is_some() {
            composite_fill_opacity(&shape)
        } else {
            None
        },
        stroke_override,
        stroke_width_override,
        radius_override,
        composite_outer_radius_override,
        padding_override,
        cluster_label_typography: group_typography,
        label_typography: typography,
    }
}

fn composite_fill_opacity(declarations: &IndexMap<String, EmittedDeclaration>) -> Option<f64> {
    let opacity = |property| {
        let Some(declaration) = declarations.get(property) else {
            return Some(1.0);
        };
        crate::theme::edge_label_bg::literal_opacity(&declaration.value)
    };
    Some(opacity("opacity")? * opacity("fill-opacity")?)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SourceStyle {
    AssignedClass,
    AssignedText,
    StructuralShape,
    StructuralText,
    Inline,
}

impl SourceStyle {
    fn is_label(self, property: &str, compatibility: &StateCompatibilityPlan) -> bool {
        match self {
            Self::AssignedText => true,
            Self::StructuralText => !compatibility.html_labels() && property == "fill",
            Self::StructuralShape => {
                compatibility.html_labels()
                    && matches!(
                        property,
                        "font-family" | "font-size" | "font-weight" | "font-style"
                    )
            }
            Self::Inline | Self::AssignedClass => is_label_style_key(property),
        }
    }

    fn can_style_shape(self, property: &str, compatibility: &StateCompatibilityPlan) -> bool {
        match self {
            Self::StructuralText | Self::AssignedText => false,
            Self::StructuralShape => match property {
                "fill" | "stroke" | "stroke-width" | "stroke-dasharray" | "stroke-linecap"
                | "stroke-linejoin" | "opacity" | "fill-opacity" | "stroke-opacity" => {
                    !compatibility.html_labels() || !compatibility.is_hand_drawn()
                }
                property => {
                    compatibility.is_classic() && source_compositing_property(property) == "filter"
                }
            },
            Self::Inline | Self::AssignedClass => true,
        }
    }
}

fn hidden_state_prefixes(model: &StateDiagramRenderModel) -> Vec<String> {
    model
        .states
        .iter()
        .filter_map(|(id, state)| {
            state
                .note
                .as_ref()
                .filter(|note| !note.text.trim().is_empty() && note.position.is_none())
                .map(|_| id.clone())
        })
        .collect()
}

fn shadowed_self_loop_edge_indices(
    model: &StateDiagramRenderModel,
    hidden_prefixes: &[String],
) -> BTreeSet<usize> {
    let mut retained_endpoints = BTreeSet::new();
    let mut shadowed = BTreeSet::new();
    for (index, edge) in model.edges.iter().enumerate().rev() {
        if edge.start != edge.end || state_edge_is_hidden(edge, hidden_prefixes) {
            continue;
        }
        if !retained_endpoints.insert(edge.start.as_str()) {
            shadowed.insert(index);
        }
    }
    shadowed
}

fn state_edge_is_hidden(edge: &StateDiagramRenderEdge, hidden_prefixes: &[String]) -> bool {
    edge.classes
        .split_whitespace()
        .any(|class| class == "note-edge")
        || state_is_hidden_id(hidden_prefixes, edge.start.as_str())
        || state_is_hidden_id(hidden_prefixes, edge.end.as_str())
        || state_is_hidden_id(hidden_prefixes, edge.id.as_str())
}

fn state_is_hidden_id(prefixes: &[String], id: &str) -> bool {
    prefixes.iter().any(|prefix| {
        id == prefix
            || id
                .strip_prefix(prefix)
                .is_some_and(|suffix| suffix.starts_with("----"))
    })
}

fn apply_source_shape_property(
    declaration: &ParsedStyleDeclaration<'_>,
    fill: &mut Option<String>,
    stroke: &mut Option<String>,
    stroke_width: &mut Option<f64>,
    radius: &mut Option<f64>,
    padding: &mut Option<f64>,
) -> bool {
    let value = declaration.value().trim();
    match declaration.property() {
        "fill" | "stroke" => {
            if declaration.is_single_component_value()
                && (value.eq_ignore_ascii_case("none")
                    || merman_markup::theme_color::ThemeColor::parse(value).is_ok())
            {
                if declaration.property() == "fill" {
                    *fill = Some(value.to_string());
                } else {
                    *stroke = Some(value.to_string());
                }
                true
            } else {
                false
            }
        }
        "stroke-width" => apply_numeric_source_property(declaration, stroke_width),
        "border-radius" | "rx" | "ry" => apply_numeric_source_property(declaration, radius),
        "padding" => apply_numeric_source_property(declaration, padding),
        "opacity" | "fill-opacity" | "stroke-opacity" => {
            if source_opacity_value_is_admissible(declaration) {
                true
            } else {
                false
            }
        }
        "stroke-dasharray" | "stroke-linecap" | "stroke-linejoin" => true,
        _ => false,
    }
}

fn source_opacity_value_is_admissible(declaration: &ParsedStyleDeclaration<'_>) -> bool {
    // Variable substitution can produce one scalar from several source components.
    // Preserve that host-owned declaration without attempting to evaluate it.
    if declaration.analysis().has_dynamic_reference_function() {
        return true;
    }
    if !declaration.is_single_component_value() {
        return false;
    }
    let value = declaration.value();
    let mut parser = cssparser::Parser::new(value);
    match parser.next() {
        Ok(cssparser::Token::Number { .. } | cssparser::Token::Percentage { .. }) => {
            // Preserve the source token and CSS clamping, including negatives. Read
            // its finite f64 magnitude rather than cssparser's narrower f32 value.
            value
                .strip_suffix('%')
                .unwrap_or(value)
                .parse::<f64>()
                .is_ok_and(f64::is_finite)
        }
        Ok(cssparser::Token::Ident(keyword)) => {
            ["inherit", "initial", "unset", "revert", "revert-layer"]
                .iter()
                .any(|candidate| keyword.eq_ignore_ascii_case(candidate))
        }
        // Prepared declarations already passed structural CSS admission. Host
        // expressions retain their source ownership without being evaluated here.
        Ok(cssparser::Token::Function(_)) => true,
        _ => false,
    }
}

fn apply_numeric_source_property(
    declaration: &ParsedStyleDeclaration<'_>,
    slot: &mut Option<f64>,
) -> bool {
    match declaration.svg_number_or_px() {
        Some(value) => {
            *slot = Some(value);
            true
        }
        None => false,
    }
}

fn apply_source_text_property(
    declaration: &ParsedStyleDeclaration<'_>,
    inherited: &TextStyle,
    typography: &mut ResolvedLabelTypography,
    native_class_fill: bool,
) -> bool {
    let value = declaration.value().trim();
    match declaration.property() {
        "font-family" if is_safe_css_font_family_value(value) && !value.is_empty() => {
            if let Some(stack) = crate::text::parse_css_font_stack(value) {
                typography.apply_font_family(stack);
                true
            } else {
                false
            }
        }
        "font-size" => match declaration
            .resolve_font_size_px(CssFontSizeContext::uniform(inherited.font_size))
        {
            Some(value) => {
                typography.text_style.font_size = value;
                true
            }
            None => false,
        },
        "font-weight" if is_supported_css_font_weight_value(value) => {
            typography.apply_font_weight(resolve_source_font_weight(
                value,
                inherited.font_weight.as_deref(),
            ));
            true
        }
        "font-style" if is_supported_css_font_style_value(value) => {
            let style = match value.to_ascii_lowercase().as_str() {
                "italic" => FontStyle::Italic,
                "oblique" => FontStyle::Oblique,
                _ => FontStyle::Normal,
            };
            typography.apply_font_style(style);
            true
        }
        "color" | "fill"
            if (declaration.property() == "color" || native_class_fill)
                && declaration.is_single_component_value()
                && (merman_markup::theme_color::ThemeColor::parse(value).is_ok()
                    || value.eq_ignore_ascii_case("currentcolor")
                    || (native_class_fill && value.eq_ignore_ascii_case("none"))) =>
        {
            true
        }
        "fill" if native_class_fill => false,
        "font-family" | "font-weight" | "font-style" | "color" => false,
        "line-height" | "letter-spacing" | "word-spacing" | "text-transform" | "white-space"
        | "word-wrap" | "word-break" | "overflow-wrap" | "text-align" | "text-decoration"
        | "text-shadow" | "text-overflow" | "hyphens" => false,
        _ => false,
    }
}

fn resolve_source_font_weight(value: &str, inherited: Option<&str>) -> u16 {
    crate::text::resolve_css_font_weight(value, inherited_font_weight(inherited))
        .expect("font-weight was validated before resolution")
}

fn inherited_font_weight(value: Option<&str>) -> u16 {
    match value.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        Some("bold" | "bolder") => 700,
        Some("lighter") => 300,
        Some("normal") | None => 400,
        Some(value) => value.parse::<u16>().unwrap_or(400).clamp(1, 1000),
    }
}

#[derive(Debug, Clone)]
struct EmittedDeclaration {
    property: String,
    value: String,
}

impl EmittedDeclaration {
    fn from_source(declaration: &ParsedStyleDeclaration<'_>) -> Self {
        Self {
            property: declaration.property_css().to_string(),
            value: declaration.value().to_string(),
        }
    }

    fn new(property: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            property: property.into(),
            value: value.into(),
        }
    }
}

fn insert_emitted(
    out: &mut IndexMap<String, EmittedDeclaration>,
    property: impl Into<String>,
    value: impl Into<String>,
) {
    let property = property.into();
    out.insert(property.clone(), EmittedDeclaration::new(property, value));
}

fn compact_style_attr(declarations: &IndexMap<String, EmittedDeclaration>) -> String {
    compact_filtered_style_attr(declarations, |_| true)
}

fn compact_split_fill_path_style_attr(
    declarations: &IndexMap<String, EmittedDeclaration>,
) -> String {
    compact_filtered_style_attr(declarations, |property| {
        !matches!(
            property,
            "stroke"
                | "stroke-width"
                | "stroke-dasharray"
                | "stroke-linecap"
                | "stroke-linejoin"
                | "stroke-opacity"
        )
    })
}

fn compact_split_stroke_path_style_attr(
    declarations: &IndexMap<String, EmittedDeclaration>,
) -> String {
    compact_filtered_style_attr(declarations, |property| {
        !matches!(property, "fill" | "fill-opacity")
    })
}

fn compact_filtered_style_attr(
    declarations: &IndexMap<String, EmittedDeclaration>,
    include: impl Fn(&str) -> bool,
) -> String {
    declarations
        .values()
        .filter(|declaration| include(declaration.property.trim()))
        .map(|declaration| {
            format!(
                "{}:{} !important",
                declaration.property.trim(),
                declaration.value.trim()
            )
        })
        .collect::<Vec<_>>()
        .join(";")
}

fn div_style_prefix(declarations: &IndexMap<String, EmittedDeclaration>) -> String {
    declarations
        .values()
        .map(|declaration| {
            format!(
                "{}: {} !important; ",
                declaration.property.trim(),
                declaration.value.trim()
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_model::families::state::StateStyleClass;
    use serde_json::json;

    fn rect_node() -> StateDiagramRenderNode {
        StateDiagramRenderNode {
            id: "Ready".to_string(),
            label_style: String::new(),
            label: Some(json!("Ready")),
            description: None,
            dom_id: "state-Ready-0".to_string(),
            is_group: false,
            node_type: None,
            parent_id: None,
            css_classes: "first second statediagram-state".to_string(),
            css_compiled_styles: vec![
                "font-size:18px".to_string(),
                "fill:#111".to_string(),
                "font-size:20px".to_string(),
            ],
            css_styles: vec!["font-size:not-a-size".to_string(), "fill:#333".to_string()],
            dir: None,
            explicit_dir: None,
            padding: Some(8.0),
            rx: Some(10.0),
            ry: Some(10.0),
            shape: "rect".to_string(),
            position: None,
            color_index: None,
            wrapping_width: None,
            min_width: None,
        }
    }

    fn semantic_node(id: &str, shape: &str) -> StateDiagramRenderNode {
        let mut node = rect_node();
        node.id = id.to_string();
        node.dom_id = format!("state-{id}-0");
        node.label = Some(json!(id));
        node.shape = shape.to_string();
        node.css_classes.clear();
        node.css_compiled_styles.clear();
        node.css_styles.clear();
        node
    }

    #[test]
    fn source_typography_is_one_decision_for_measurement_and_svg_emission() {
        let mut model = StateDiagramRenderModel::default();
        model.style_classes.insert(
            "source-type".to_string(),
            StateStyleClass {
                id: "source-type".to_string(),
                styles: vec![
                    "font-family:\"Class Sans\", serif".to_string(),
                    "font-size:18px".to_string(),
                    "font-weight:bolder".to_string(),
                    "font-style:italic".to_string(),
                ],
                text_styles: Vec::new(),
            },
        );
        let mut class_only = semantic_node("ClassOnly", "rect");
        class_only.css_classes = "source-type".to_string();
        let mut overridden = semantic_node("Overridden", "rect");
        overridden.css_classes = "source-type".to_string();
        overridden.css_styles = vec![
            "font-size:21px".to_string(),
            "font-weight:lighter".to_string(),
        ];
        overridden.label_style =
            "font-family:\"Final Sans\", monospace;font-style:oblique".to_string();
        model.nodes.extend([class_only, overridden]);

        let plan = StateStylePlan::resolve_unthemed(&model, &json!({}));
        let class_only = plan.node("ClassOnly").expect("class-only node style");
        let class_request = class_only.resolved_label_typography().text_style();
        assert_eq!(
            class_request.font_family.as_deref(),
            Some("\"Class Sans\", serif")
        );
        assert_eq!(class_request.font_size, 18.0);
        assert_eq!(class_request.font_weight.as_deref(), Some("700"));
        assert_eq!(class_request.font_style.as_deref(), Some("italic"));
        assert!(
            class_only
                .label_style_attr()
                .contains("font-family:\"Class Sans\", serif !important")
        );
        assert!(
            class_only
                .label_style_attr()
                .contains("font-size:18px !important")
        );
        assert!(
            class_only
                .label_style_attr()
                .contains("font-weight:700 !important")
        );
        assert!(
            class_only
                .label_style_attr()
                .contains("font-style:italic !important")
        );

        let overridden = plan.node("Overridden").expect("overridden node style");
        let overridden_request = overridden.resolved_label_typography().text_style();
        assert_eq!(
            overridden_request.font_family.as_deref(),
            Some("\"Final Sans\", monospace")
        );
        assert_eq!(overridden_request.font_size, 21.0);
        assert_eq!(overridden_request.font_weight.as_deref(), Some("100"));
        assert_eq!(overridden_request.font_style.as_deref(), Some("oblique"));
        assert!(
            overridden
                .label_style_attr()
                .contains("font-family:\"Final Sans\", monospace !important")
        );
        assert!(
            overridden
                .label_style_attr()
                .contains("font-size:21px !important")
        );
        assert!(
            overridden
                .label_style_attr()
                .contains("font-weight:100 !important")
        );
        assert!(
            overridden
                .label_style_attr()
                .contains("font-style:oblique !important")
        );
    }

    #[test]
    fn group_titles_keep_class_typography_when_inline_styles_change_leaf_labels() {
        let mut model = StateDiagramRenderModel::default();
        model.style_classes.insert(
            "title".to_string(),
            StateStyleClass {
                id: "title".to_string(),
                styles: vec!["font-size:18px".to_string()],
                text_styles: vec!["font-family:\"Class Sans\", serif".to_string()],
            },
        );
        for (id, shape, is_group) in [
            ("Composite", "roundedWithTitle", true),
            ("CompositeLeaf", "roundedWithTitle", false),
            ("NoteGroup", "noteGroup", true),
            ("Leaf", "rect", false),
        ] {
            let mut node = semantic_node(id, shape);
            node.is_group = is_group;
            node.css_classes = "title".to_string();
            node.css_styles = vec!["font-size:32px".to_string()];
            node.label_style = "font-family:\"Inline Sans\", monospace".to_string();
            model.nodes.push(node);
        }

        let plan = StateStylePlan::resolve_unthemed(&model, &json!({}));
        for id in ["Composite", "CompositeLeaf"] {
            let node = plan.node(id).expect("composite style");
            assert_eq!(
                node.resolved_label_typography().text_style().font_size,
                18.0
            );
            assert!(
                node.label_style_attr()
                    .contains("font-size:18px !important")
            );
            assert!(
                node.label_style_attr()
                    .contains("font-family:\"Class Sans\", serif !important")
            );
        }
        for id in ["Composite", "NoteGroup"] {
            let typography = plan
                .node(id)
                .expect("group style")
                .resolved_cluster_label_typography()
                .text_style();
            assert_eq!(typography.font_size, 18.0);
            assert_eq!(
                typography.font_family.as_deref(),
                Some("\"Class Sans\", serif")
            );
        }
        for id in ["NoteGroup", "Leaf"] {
            let typography = plan
                .node(id)
                .expect("node style")
                .resolved_label_typography()
                .text_style();
            assert_eq!(typography.font_size, 32.0);
            assert_eq!(
                typography.font_family.as_deref(),
                Some("\"Inline Sans\", monospace")
            );
        }
    }

    #[test]
    fn source_font_stack_rejects_unquoted_css_wide_keywords_in_any_position() {
        assert!(crate::text::parse_css_font_stack("Excalifont, inherit").is_none());
        assert!(crate::text::parse_css_font_stack("initial, Excalifont").is_none());
        assert!(crate::text::parse_css_font_stack("Family unset, serif").is_none());

        let quoted = crate::text::parse_css_font_stack("\"inherit\", Excalifont")
            .expect("quoted CSS-wide text remains a valid family name");
        assert_eq!(quoted.as_css(), "\"inherit\", \"Excalifont\"");
    }

    #[test]
    fn hand_drawn_source_paints_reach_only_the_upstream_shape_surfaces() {
        let source_styles = vec![
            "fill:#111827".to_string(),
            "stroke:#2563eb".to_string(),
            "stroke-width:6px".to_string(),
        ];
        let mut model = StateDiagramRenderModel::default();
        model.style_classes.insert(
            "source-paint".to_string(),
            StateStyleClass {
                id: "source-paint".to_string(),
                styles: source_styles,
                text_styles: Vec::new(),
            },
        );

        for (id, shape) in [
            ("Ordinary", "rect"),
            ("Note", "note"),
            ("Choice", "choice"),
            ("Fork", "fork"),
            ("Join", "join"),
            ("End", "stateEnd"),
            ("Start", "stateStart"),
        ] {
            let mut node = semantic_node(id, shape);
            node.css_classes = "source-paint".to_string();
            model.nodes.push(node);
        }
        for (id, shape) in [("Composite", "roundedWithTitle"), ("Divider", "divider")] {
            let mut node = semantic_node(id, shape);
            node.is_group = true;
            node.node_type = Some("group".to_string());
            node.css_classes = "source-paint".to_string();
            model.nodes.push(node);
        }

        let plan = StateStylePlan::resolve_unthemed(&model, &json!({ "look": "handDrawn" }));

        for id in ["Ordinary", "Choice", "End"] {
            let node = plan.node(id).unwrap_or_else(|| panic!("prepared {id}"));
            assert_eq!(node.fill_override(), Some("#111827"), "{id}");
            assert_eq!(node.stroke_override(), Some("#2563eb"), "{id}");
            assert_eq!(node.stroke_width_override(), Some(6.0), "{id}");
        }
        let end = plan.node("End").expect("prepared End");
        assert!(
            end.special_state_inner_fill_path_style_attr()
                .contains("fill:#111827 !important")
        );
        assert!(
            end.special_state_inner_stroke_path_style_attr()
                .contains("stroke:#2563eb !important")
        );

        for id in ["Note", "Fork", "Join"] {
            let node = plan.node(id).unwrap_or_else(|| panic!("prepared {id}"));
            assert_eq!(node.fill_override(), None, "{id}");
            assert_eq!(node.stroke_override(), None, "{id}");
            assert_eq!(node.stroke_width_override(), Some(6.0), "{id}");
        }

        for id in ["Start", "Composite", "Divider"] {
            let node = plan.node(id).unwrap_or_else(|| panic!("prepared {id}"));
            assert_eq!(node.fill_override(), None, "{id}");
            assert_eq!(node.stroke_override(), None, "{id}");
            assert_eq!(node.stroke_width_override(), None, "{id}");
        }
    }
}
