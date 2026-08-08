use indexmap::IndexMap;
use merman_core::diagrams::state::{
    StateDiagramRenderModel, StateDiagramRenderNode, StateDiagramRenderStyleClass,
};
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::diagram_theme::{
    CanvasPaint, PreparedSourceStyleDeclaration, ResolvedDiagramTheme, ResolvedProperty,
    ResolvedThemeStyle, SourceStyleChannel, SourceStyleDeclaration, SourceStyleOrigin,
    SourceStyleProvenance, SourceStyleResidual, SourceStyleResidualReason, Specified, ThemeTarget,
    ThemeVariant,
};
use crate::mermaid_style::{CssFontSizeContext, is_label_style_key, is_safe_css_font_family_value};
use crate::text::TextStyle;
use crate::theme::MermaidThemeAdapter;

#[derive(Debug, Clone)]
pub(crate) struct StateCompatibilityStyle {
    pub(crate) dark_mode: bool,
    pub(crate) neo: bool,
    pub(crate) font_family_css: String,
    pub(crate) font_size_px: f64,
    pub(crate) text_color: String,
    pub(crate) title_color: String,
    pub(crate) line_color: String,
    pub(crate) error_bkg: String,
    pub(crate) error_text: String,
    pub(crate) transition_color: String,
    pub(crate) node_border: String,
    pub(crate) background: String,
    pub(crate) main_bkg: String,
    pub(crate) alt_background: String,
    pub(crate) stroke_width: String,
    pub(crate) stroke_width_px: String,
    pub(crate) rough_stroke_width_value: f64,
    pub(crate) note_border: String,
    pub(crate) note_bkg: String,
    pub(crate) note_text: String,
    pub(crate) label_background: String,
    pub(crate) edge_label_background: String,
    pub(crate) transition_label_color: String,
    pub(crate) special_state_color: String,
    pub(crate) inner_end_background: String,
    pub(crate) end_outer_fill: String,
    pub(crate) end_outer_stroke: String,
    pub(crate) end_inner_stroke: String,
    pub(crate) composite_background: String,
    pub(crate) state_bkg: String,
    pub(crate) state_border: String,
    pub(crate) composite_title_background: String,
    pub(crate) state_label_color: String,
    pub(crate) drop_shadow: String,
}

#[derive(Debug, Clone)]
pub(crate) struct StateClassStylePlan {
    id: String,
    styles: Vec<(usize, Arc<PreparedSourceStyleDeclaration>)>,
    text_styles: Vec<(usize, Arc<PreparedSourceStyleDeclaration>)>,
}

impl StateClassStylePlan {
    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn styles(&self) -> &[(usize, Arc<PreparedSourceStyleDeclaration>)] {
        &self.styles
    }

    pub(crate) fn text_styles(&self) -> &[(usize, Arc<PreparedSourceStyleDeclaration>)] {
        &self.text_styles
    }
}

#[derive(Debug, Clone)]
pub(crate) struct StateNodeStylePlan {
    #[cfg(test)]
    binding: StateNodeThemeBinding,
    semantic_shape_style_attr: String,
    composite_header_style_attr: String,
    composite_header_text_style_attr: String,
    special_state_inner_style_attr: String,
    source_shape_style_attr: String,
    shape_style_attr: String,
    label_style_attr: String,
    div_style_prefix: String,
    composite_header_text_div_style_prefix: String,
    fill_override: Option<String>,
    stroke_override: Option<String>,
    stroke_width_override: Option<f64>,
    radius_override: Option<f64>,
    padding_override: Option<f64>,
    cluster_text_style: TextStyle,
    text_style: TextStyle,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy)]
struct StateNodeThemeBinding {
    target: ThemeTarget,
    ordinal: Option<usize>,
    label_ordinal: Option<usize>,
    composite_header_ordinal: Option<usize>,
    special_state_inner_ordinal: Option<usize>,
}

#[derive(Debug, Clone)]
pub(crate) struct StateEdgeStylePlan {
    #[cfg(test)]
    ordinal: Option<usize>,
    #[cfg(test)]
    label_ordinal: Option<usize>,
    marker_ordinal: Option<usize>,
    path_style_attr: String,
    marker_style_attr: String,
    label_style_attr: String,
    label_div_style_prefix: String,
    label_background_style_attr: String,
    label_background_div_style_prefix: String,
    text_style: TextStyle,
}

impl StateEdgeStylePlan {
    #[cfg(test)]
    pub(crate) const fn ordinal(&self) -> Option<usize> {
        self.ordinal
    }

    #[cfg(test)]
    pub(crate) const fn label_ordinal(&self) -> Option<usize> {
        self.label_ordinal
    }

    pub(crate) const fn marker_ordinal(&self) -> Option<usize> {
        self.marker_ordinal
    }

    pub(crate) fn path_style_attr(&self) -> &str {
        &self.path_style_attr
    }

    pub(crate) fn marker_style_attr(&self) -> &str {
        &self.marker_style_attr
    }

    pub(crate) fn label_style_attr(&self) -> &str {
        &self.label_style_attr
    }

    pub(crate) fn label_div_style_prefix(&self) -> &str {
        &self.label_div_style_prefix
    }

    pub(crate) fn label_background_style_attr(&self) -> &str {
        &self.label_background_style_attr
    }

    pub(crate) fn label_background_div_style_prefix(&self) -> &str {
        &self.label_background_div_style_prefix
    }

    pub(crate) const fn text_style(&self) -> &TextStyle {
        &self.text_style
    }
}

impl StateNodeStylePlan {
    #[cfg(test)]
    pub(crate) const fn target(&self) -> ThemeTarget {
        self.binding.target
    }

    #[cfg(test)]
    pub(crate) const fn ordinal(&self) -> Option<usize> {
        self.binding.ordinal
    }

    #[cfg(test)]
    pub(crate) const fn label_ordinal(&self) -> Option<usize> {
        self.binding.label_ordinal
    }

    #[cfg(test)]
    pub(crate) const fn composite_header_ordinal(&self) -> Option<usize> {
        self.binding.composite_header_ordinal
    }

    #[cfg(test)]
    pub(crate) const fn special_state_inner_ordinal(&self) -> Option<usize> {
        self.binding.special_state_inner_ordinal
    }

    pub(crate) fn shape_style_attr(&self) -> &str {
        &self.shape_style_attr
    }

    pub(crate) fn semantic_shape_style_attr(&self) -> &str {
        &self.semantic_shape_style_attr
    }

    pub(crate) fn composite_header_style_attr(&self) -> &str {
        &self.composite_header_style_attr
    }

    pub(crate) fn composite_header_text_style_attr(&self) -> &str {
        &self.composite_header_text_style_attr
    }

    pub(crate) fn composite_header_text_div_style_prefix(&self) -> &str {
        &self.composite_header_text_div_style_prefix
    }

    pub(crate) fn special_state_inner_style_attr(&self) -> &str {
        &self.special_state_inner_style_attr
    }

    pub(crate) fn source_shape_style_attr(&self) -> &str {
        &self.source_shape_style_attr
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

    pub(crate) fn stroke_override(&self) -> Option<&str> {
        self.stroke_override.as_deref()
    }

    pub(crate) const fn stroke_width_override(&self) -> Option<f64> {
        self.stroke_width_override
    }

    pub(crate) const fn radius_override(&self) -> Option<f64> {
        self.radius_override
    }

    pub(crate) const fn padding_override(&self) -> Option<f64> {
        self.padding_override
    }

    pub(crate) const fn text_style(&self) -> &TextStyle {
        &self.text_style
    }

    pub(crate) const fn cluster_text_style(&self) -> &TextStyle {
        &self.cluster_text_style
    }
}

#[derive(Debug, Clone)]
pub(crate) struct StateStylePlan {
    compatibility: StateCompatibilityStyle,
    base_text_style: TextStyle,
    transition_text_style: TextStyle,
    composite_text_style: TextStyle,
    title_text_style: TextStyle,
    title_style_attr: String,
    transition_marker_style_attr: String,
    transition_label_background_style_attr: String,
    transition_label_background_div_style_prefix: String,
    classes: IndexMap<String, StateClassStylePlan>,
    nodes: BTreeMap<String, StateNodeStylePlan>,
    edges: BTreeMap<String, StateEdgeStylePlan>,
    residuals: Vec<SourceStyleResidual>,
}

impl StateStylePlan {
    pub(crate) fn resolve(
        model: &StateDiagramRenderModel,
        effective_config: &serde_json::Value,
        resolved_theme: Option<&ResolvedDiagramTheme>,
    ) -> Self {
        let compatibility = MermaidThemeAdapter::new(effective_config).state_diagram();
        let base_text_style = super::StateConfigView::new(effective_config).text_style();
        let mut residuals = Vec::new();
        let classes = prepare_classes(model, &mut residuals);

        let transition_text_style = resolve_semantic_text_style(
            &base_text_style,
            resolved_theme,
            ThemeTarget::TransitionLabel,
            ThemeVariant::Default,
            None,
        );
        let composite_text_style = resolve_semantic_text_style(
            &base_text_style,
            resolved_theme,
            ThemeTarget::CompositeLabel,
            ThemeVariant::Default,
            None,
        );
        let mut title_base_text_style = base_text_style.clone();
        title_base_text_style.font_size = 18.0;
        let title_text_style = resolve_semantic_text_style(
            &title_base_text_style,
            resolved_theme,
            ThemeTarget::Title,
            ThemeVariant::Default,
            None,
        );
        let title_style_attr = resolved_theme
            .map(|theme| {
                let style = theme.style(ThemeTarget::Title, ThemeVariant::Default, None);
                let mut emission = IndexMap::new();
                append_semantic_text_emission(&style, &mut emission);
                if let Some(color) = direct_paint(
                    ThemeTarget::Title,
                    PaintSlot::Fill,
                    style.fill_resolution(),
                    ThemeVariant::Default,
                    None,
                ) {
                    insert_emitted(&mut emission, "color", color);
                }
                compact_style_attr(&emission)
            })
            .unwrap_or_default();

        let transition_marker_style_attr = semantic_shape_style_attr(
            resolved_theme,
            ThemeTarget::TransitionMarker,
            ThemeVariant::Default,
            None,
            true,
        );
        let transition_label_background_style_attr = semantic_shape_style_attr(
            resolved_theme,
            ThemeTarget::TransitionLabelBackground,
            ThemeVariant::Default,
            None,
            true,
        );
        let transition_label_background_div_style_prefix = semantic_html_background_style(
            resolved_theme,
            ThemeTarget::TransitionLabelBackground,
            ThemeVariant::Default,
            None,
        );
        let hidden_prefixes = hidden_state_prefixes(model);
        let mut target_ordinals = BTreeMap::<ThemeTarget, usize>::new();
        let mut nodes = BTreeMap::new();
        for node in &model.nodes {
            let (target, label_target, variant) = semantic_binding(node);
            let visible = !state_is_hidden_id(&hidden_prefixes, node.id.as_str());
            let participates = visible && participates_in_ordinal(node);
            let one_based_ordinal =
                participates.then(|| next_target_ordinal(&mut target_ordinals, target));
            let label_ordinal = (participates && has_visible_label(node))
                .then(|| next_target_ordinal(&mut target_ordinals, label_target));
            let composite_header_ordinal = (participates
                && matches!(target, ThemeTarget::Composite)
                && has_visible_label(node))
            .then(|| next_target_ordinal(&mut target_ordinals, ThemeTarget::CompositeHeader));
            let special_state_inner_ordinal = (participates
                && matches!(target, ThemeTarget::SpecialState)
                && matches!(node.shape.as_str(), "stateEnd" | "choice" | "fork" | "join"))
            .then(|| next_target_ordinal(&mut target_ordinals, ThemeTarget::SpecialStateInner));
            let node_plan = prepare_node(
                node,
                &classes,
                &base_text_style,
                resolved_theme,
                target,
                label_target,
                variant,
                one_based_ordinal,
                label_ordinal,
                composite_header_ordinal,
                special_state_inner_ordinal,
                &mut residuals,
            );
            nodes.insert(node.id.clone(), node_plan);
        }

        let mut transition_ordinal = 0usize;
        let mut edges = BTreeMap::new();
        for edge in &model.edges {
            let hidden = edge
                .classes
                .split_whitespace()
                .any(|class| class == "note-edge")
                || state_is_hidden_id(&hidden_prefixes, edge.start.as_str())
                || state_is_hidden_id(&hidden_prefixes, edge.end.as_str());
            let ordinal = (!hidden).then(|| {
                transition_ordinal += 1;
                transition_ordinal
            });
            let label_ordinal = (ordinal.is_some() && !edge.label.trim().is_empty())
                .then(|| next_target_ordinal(&mut target_ordinals, ThemeTarget::TransitionLabel));
            let marker_ordinal = (ordinal.is_some() && !edge.arrow_type_end.trim().is_empty())
                .then(|| next_target_ordinal(&mut target_ordinals, ThemeTarget::TransitionMarker));
            edges.insert(
                edge.id.clone(),
                prepare_edge(
                    &base_text_style,
                    resolved_theme,
                    ordinal,
                    label_ordinal,
                    marker_ordinal,
                ),
            );
        }

        Self {
            compatibility,
            base_text_style,
            transition_text_style,
            composite_text_style,
            title_text_style,
            title_style_attr,
            transition_marker_style_attr,
            transition_label_background_style_attr,
            transition_label_background_div_style_prefix,
            classes,
            nodes,
            edges,
            residuals,
        }
    }

    pub(crate) const fn compatibility(&self) -> &StateCompatibilityStyle {
        &self.compatibility
    }

    pub(crate) const fn base_text_style(&self) -> &TextStyle {
        &self.base_text_style
    }

    pub(crate) const fn transition_text_style(&self) -> &TextStyle {
        &self.transition_text_style
    }

    pub(crate) const fn composite_text_style(&self) -> &TextStyle {
        &self.composite_text_style
    }

    pub(crate) const fn title_text_style(&self) -> &TextStyle {
        &self.title_text_style
    }

    pub(crate) fn title_style_attr(&self) -> &str {
        &self.title_style_attr
    }

    pub(crate) fn transition_marker_style_attr(&self) -> &str {
        &self.transition_marker_style_attr
    }

    pub(crate) fn transition_label_background_style_attr(&self) -> &str {
        &self.transition_label_background_style_attr
    }

    pub(crate) fn transition_label_background_div_style_prefix(&self) -> &str {
        &self.transition_label_background_div_style_prefix
    }

    pub(crate) fn classes(&self) -> impl Iterator<Item = &StateClassStylePlan> {
        self.classes.values()
    }

    pub(crate) fn node(&self, id: &str) -> Option<&StateNodeStylePlan> {
        self.nodes.get(id)
    }

    pub(crate) fn edge(&self, id: &str) -> Option<&StateEdgeStylePlan> {
        self.edges.get(id)
    }

    pub(crate) fn edges(&self) -> impl Iterator<Item = &StateEdgeStylePlan> {
        self.edges.values()
    }

    pub(crate) fn residuals(&self) -> &[SourceStyleResidual] {
        &self.residuals
    }
}

fn prepare_classes(
    model: &StateDiagramRenderModel,
    residuals: &mut Vec<SourceStyleResidual>,
) -> IndexMap<String, StateClassStylePlan> {
    model
        .style_classes
        .iter()
        .map(|(key, class)| {
            let styles = prepare_class_declarations(
                class,
                &class.styles,
                SourceStyleChannel::Stylesheet,
                residuals,
            );
            let text_styles = prepare_class_declarations(
                class,
                &class.text_styles,
                SourceStyleChannel::Label,
                residuals,
            );
            (
                key.clone(),
                StateClassStylePlan {
                    id: class.id.clone(),
                    styles,
                    text_styles,
                },
            )
        })
        .collect()
}

fn prepare_class_declarations(
    class: &StateDiagramRenderStyleClass,
    raw_declarations: &[String],
    channel: SourceStyleChannel,
    residuals: &mut Vec<SourceStyleResidual>,
) -> Vec<(usize, Arc<PreparedSourceStyleDeclaration>)> {
    raw_declarations
        .iter()
        .enumerate()
        .filter_map(|(declaration_ordinal, raw)| {
            let provenance = SourceStyleProvenance::generated_class_css(
                class.id.clone(),
                channel,
                declaration_ordinal,
            );
            match PreparedSourceStyleDeclaration::parse(raw) {
                Some(prepared) => Some((declaration_ordinal, Arc::new(prepared))),
                None => {
                    residuals.push(SourceStyleResidual::invalid(raw, provenance));
                    None
                }
            }
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn prepare_node(
    node: &StateDiagramRenderNode,
    classes: &IndexMap<String, StateClassStylePlan>,
    base_text_style: &TextStyle,
    resolved_theme: Option<&ResolvedDiagramTheme>,
    target: ThemeTarget,
    label_target: ThemeTarget,
    variant: ThemeVariant,
    ordinal: Option<usize>,
    label_ordinal: Option<usize>,
    composite_header_ordinal: Option<usize>,
    special_state_inner_ordinal: Option<usize>,
    residuals: &mut Vec<SourceStyleResidual>,
) -> StateNodeStylePlan {
    let semantic_shape = resolved_theme.map(|theme| theme.style(target, variant, ordinal));
    let semantic_label =
        resolved_theme.map(|theme| theme.style(label_target, variant, label_ordinal));
    let semantic_composite_header = (target == ThemeTarget::Composite)
        .then(|| {
            resolved_theme.map(|theme| {
                theme.style(
                    ThemeTarget::CompositeHeader,
                    ThemeVariant::Default,
                    composite_header_ordinal,
                )
            })
        })
        .flatten();
    let semantic_special_state_inner = (target == ThemeTarget::SpecialState)
        .then(|| {
            resolved_theme.map(|theme| {
                theme.style(
                    ThemeTarget::SpecialStateInner,
                    ThemeVariant::End,
                    special_state_inner_ordinal,
                )
            })
        })
        .flatten();
    let mut shape_declarations = Vec::new();
    let mut label_declarations = Vec::new();

    for (assignment_ordinal, class_id) in node.css_classes.split_whitespace().enumerate() {
        let Some(class) = classes.get(class_id) else {
            continue;
        };
        for (declaration_ordinal, prepared) in &class.styles {
            let channel = declaration_channel(prepared.property());
            let declaration = prepared.bind(SourceStyleProvenance::assigned_class(
                node.id.clone(),
                class_id.to_string(),
                channel,
                assignment_ordinal,
                *declaration_ordinal,
            ));
            push_declaration(
                declaration,
                &mut shape_declarations,
                &mut label_declarations,
            );
        }
        for (declaration_ordinal, prepared) in &class.text_styles {
            let declaration = prepared.bind(SourceStyleProvenance::assigned_class(
                node.id.clone(),
                class_id.to_string(),
                SourceStyleChannel::Label,
                assignment_ordinal,
                *declaration_ordinal,
            ));
            label_declarations.push(declaration);
        }
    }

    for (declaration_ordinal, raw) in node.css_styles.iter().enumerate() {
        let parsed = PreparedSourceStyleDeclaration::parse(raw);
        let channel = parsed
            .as_ref()
            .map_or(SourceStyleChannel::Shape, |declaration| {
                declaration_channel(declaration.property())
            });
        let provenance =
            SourceStyleProvenance::inline(node.id.clone(), channel, declaration_ordinal);
        let Some(prepared) = parsed else {
            residuals.push(SourceStyleResidual::invalid(raw, provenance));
            continue;
        };
        let declaration = Arc::new(prepared).bind(provenance);
        push_declaration(
            declaration,
            &mut shape_declarations,
            &mut label_declarations,
        );
    }

    for (declaration_ordinal, raw) in node
        .label_style
        .split(';')
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
        .enumerate()
    {
        let parsed = PreparedSourceStyleDeclaration::parse(raw);
        let channel = parsed
            .as_ref()
            .map_or(SourceStyleChannel::Label, |declaration| {
                declaration_channel(declaration.property())
            });
        let provenance =
            SourceStyleProvenance::label_style(node.id.clone(), channel, declaration_ordinal);
        let Some(prepared) = parsed else {
            residuals.push(SourceStyleResidual::invalid(raw, provenance));
            continue;
        };
        let declaration = Arc::new(prepared).bind(provenance);
        push_declaration(
            declaration,
            &mut shape_declarations,
            &mut label_declarations,
        );
    }

    let mut shape_emission = IndexMap::<String, EmittedDeclaration>::new();
    let mut source_shape_emission = IndexMap::<String, EmittedDeclaration>::new();
    let mut label_emission = IndexMap::<String, EmittedDeclaration>::new();
    let mut fill_override = semantic_shape
        .as_ref()
        .and_then(|style| {
            direct_paint(
                target,
                PaintSlot::Fill,
                style.fill_resolution(),
                variant,
                ordinal,
            )
        })
        .or_else(|| {
            resolved_theme
                .zip(ordinal)
                .and_then(|(theme, ordinal)| theme.series_color(target, ordinal))
                .map(|color| color.as_css())
        });
    let mut stroke_override = semantic_shape.as_ref().and_then(|style| {
        direct_paint(
            target,
            PaintSlot::Stroke,
            style.stroke_resolution(),
            variant,
            ordinal,
        )
    });
    let mut stroke_width_override = semantic_shape
        .as_ref()
        .and_then(ResolvedThemeStyle::stroke_width)
        .map(f64::from);
    let mut radius_override = semantic_shape
        .as_ref()
        .and_then(ResolvedThemeStyle::radius)
        .map(f64::from);
    let mut padding_override = semantic_shape
        .as_ref()
        .and_then(ResolvedThemeStyle::padding)
        .and_then(uniform_padding_px);

    if let Some(style) = semantic_shape.as_ref() {
        append_semantic_shape_emission(style, &mut shape_emission);
    }
    if let Some(fill) = fill_override.as_ref() {
        insert_emitted(&mut shape_emission, "fill", fill.clone());
    }
    if let Some(stroke) = stroke_override.as_ref() {
        insert_emitted(&mut shape_emission, "stroke", stroke.clone());
    }
    if let Some(style) = semantic_label.as_ref() {
        append_semantic_text_emission(style, &mut label_emission);
        let color = direct_paint(
            label_target,
            PaintSlot::Fill,
            style.fill_resolution(),
            variant,
            label_ordinal,
        )
        .or_else(|| {
            resolved_theme
                .zip(label_ordinal)
                .and_then(|(theme, ordinal)| theme.series_color(label_target, ordinal))
                .map(|color| color.as_css())
        });
        if let Some(color) = color {
            insert_emitted(&mut label_emission, "color", color);
        }
    }

    let semantic_shape_style_attr = compact_style_attr(&shape_emission);
    let mut composite_header_emission = IndexMap::new();
    let mut composite_header_text_emission = IndexMap::new();
    if let Some(style) = semantic_composite_header.as_ref() {
        append_semantic_shape_emission(style, &mut composite_header_emission);
        if let Some(fill) = paint_value(style.fill_resolution()) {
            insert_emitted(&mut composite_header_emission, "fill", fill);
        }
        if let Some(stroke) = paint_value(style.stroke_resolution()) {
            insert_emitted(&mut composite_header_emission, "stroke", stroke);
        }
    }
    let special_state_inner_style_attr = semantic_special_state_inner
        .as_ref()
        .map(|style| {
            semantic_shape_style_attr_for_style(
                style,
                ThemeTarget::SpecialStateInner,
                ThemeVariant::End,
                special_state_inner_ordinal,
                true,
            )
        })
        .unwrap_or_default();
    for declaration in &shape_declarations {
        if target == ThemeTarget::Composite
            && declaration.provenance().origin() != SourceStyleOrigin::AssignedClass
        {
            residuals.push(SourceStyleResidual::from_declaration(
                declaration,
                SourceStyleResidualReason::UnsupportedSurface,
            ));
            continue;
        }
        let accepted = apply_source_shape_property(
            declaration,
            &mut fill_override,
            &mut stroke_override,
            &mut stroke_width_override,
            &mut radius_override,
            &mut padding_override,
            residuals,
        );
        if accepted {
            let emitted = EmittedDeclaration::from_source(declaration);
            shape_emission.insert(declaration.property().to_string(), emitted.clone());
            source_shape_emission.insert(declaration.property().to_string(), emitted);
        }
        /*
         * Validation and emission intentionally share the same admission decision.  A malformed
         * later declaration must not erase a valid earlier winner in the SVG attribute.
         */
        if !accepted {
            continue;
        }
    }

    let semantic_text_style = resolve_semantic_text_style(
        base_text_style,
        resolved_theme,
        label_target,
        variant,
        label_ordinal,
    );
    if target == ThemeTarget::Composite {
        if let Some(style) = semantic_label.as_ref() {
            append_semantic_text_emission(style, &mut composite_header_text_emission);
            let color = direct_paint(
                label_target,
                PaintSlot::Fill,
                style.fill_resolution(),
                variant,
                label_ordinal,
            );
            if let Some(color) = color {
                insert_emitted(&mut composite_header_text_emission, "color", color);
            }
        }
    }
    let mut cluster_text_style = semantic_text_style.clone();
    let mut ignored_cluster_residuals = Vec::new();
    for declaration in &label_declarations {
        if target == ThemeTarget::Composite
            && declaration.provenance().origin() != SourceStyleOrigin::AssignedClass
        {
            continue;
        }
        if declaration.provenance().origin() == SourceStyleOrigin::AssignedClass {
            let accepted = apply_source_text_property(
                declaration,
                base_text_style,
                &mut cluster_text_style,
                &mut ignored_cluster_residuals,
            );
            if target == ThemeTarget::Composite && accepted {
                composite_header_text_emission.insert(
                    declaration.property().to_string(),
                    EmittedDeclaration::from_source(declaration),
                );
            }
        }
    }
    let mut text_style = semantic_text_style;
    for declaration in &label_declarations {
        if target == ThemeTarget::Composite
            && declaration.provenance().origin() != SourceStyleOrigin::AssignedClass
        {
            residuals.push(SourceStyleResidual::from_declaration(
                declaration,
                SourceStyleResidualReason::UnsupportedSurface,
            ));
            continue;
        }
        let accepted =
            apply_source_text_property(declaration, base_text_style, &mut text_style, residuals);
        if accepted {
            label_emission.insert(
                declaration.property().to_string(),
                EmittedDeclaration::from_source(declaration),
            );
        }
    }

    StateNodeStylePlan {
        #[cfg(test)]
        binding: StateNodeThemeBinding {
            target,
            ordinal,
            label_ordinal,
            composite_header_ordinal,
            special_state_inner_ordinal,
        },
        semantic_shape_style_attr,
        composite_header_style_attr: compact_style_attr(&composite_header_emission),
        composite_header_text_style_attr: compact_style_attr(&composite_header_text_emission),
        special_state_inner_style_attr,
        source_shape_style_attr: compact_style_attr(&source_shape_emission),
        shape_style_attr: compact_style_attr(&shape_emission),
        label_style_attr: compact_style_attr(&label_emission),
        div_style_prefix: div_style_prefix(&label_emission),
        composite_header_text_div_style_prefix: div_style_prefix(&composite_header_text_emission),
        fill_override,
        stroke_override,
        stroke_width_override,
        radius_override,
        padding_override,
        cluster_text_style,
        text_style,
    }
}

fn prepare_edge(
    base_text_style: &TextStyle,
    resolved_theme: Option<&ResolvedDiagramTheme>,
    ordinal: Option<usize>,
    label_ordinal: Option<usize>,
    marker_ordinal: Option<usize>,
) -> StateEdgeStylePlan {
    let mut path_style_attr = semantic_shape_style_attr(
        resolved_theme,
        ThemeTarget::Transition,
        ThemeVariant::Default,
        ordinal,
        false,
    );
    if !path_style_attr.contains("stroke:")
        && let Some(color) = resolved_theme
            .zip(ordinal)
            .and_then(|(theme, ordinal)| theme.series_color(ThemeTarget::Transition, ordinal))
    {
        append_style_declaration(&mut path_style_attr, "stroke", color.as_css());
    }
    let marker_style_attr = semantic_shape_style_attr(
        resolved_theme,
        ThemeTarget::TransitionMarker,
        ThemeVariant::Default,
        marker_ordinal,
        false,
    );
    let label_background_style_attr = semantic_shape_style_attr(
        resolved_theme,
        ThemeTarget::TransitionLabelBackground,
        ThemeVariant::Default,
        label_ordinal,
        true,
    );
    let label_background_div_style_prefix = semantic_html_background_style(
        resolved_theme,
        ThemeTarget::TransitionLabelBackground,
        ThemeVariant::Default,
        label_ordinal,
    );
    let mut label_emission = IndexMap::<String, EmittedDeclaration>::new();
    if let Some(theme) = resolved_theme {
        let style = theme.style(
            ThemeTarget::TransitionLabel,
            ThemeVariant::Default,
            label_ordinal,
        );
        append_semantic_text_emission(&style, &mut label_emission);
        let color = direct_paint(
            ThemeTarget::TransitionLabel,
            PaintSlot::Fill,
            style.fill_resolution(),
            ThemeVariant::Default,
            label_ordinal,
        )
        .or_else(|| {
            label_ordinal
                .and_then(|ordinal| theme.series_color(ThemeTarget::TransitionLabel, ordinal))
                .map(|color| color.as_css())
        });
        if let Some(color) = color {
            insert_emitted(&mut label_emission, "color", color);
        }
    }

    StateEdgeStylePlan {
        #[cfg(test)]
        ordinal,
        #[cfg(test)]
        label_ordinal,
        marker_ordinal,
        path_style_attr,
        marker_style_attr,
        label_style_attr: compact_style_attr(&label_emission),
        label_div_style_prefix: div_style_prefix(&label_emission),
        label_background_style_attr,
        label_background_div_style_prefix,
        text_style: resolve_semantic_text_style(
            base_text_style,
            resolved_theme,
            ThemeTarget::TransitionLabel,
            ThemeVariant::Default,
            label_ordinal,
        ),
    }
}

fn semantic_shape_style_attr(
    resolved_theme: Option<&ResolvedDiagramTheme>,
    target: ThemeTarget,
    variant: ThemeVariant,
    ordinal: Option<usize>,
    include_default_paint: bool,
) -> String {
    let Some(theme) = resolved_theme else {
        return String::new();
    };
    let style = theme.style(target, variant, ordinal);
    semantic_shape_style_attr_for_style(&style, target, variant, ordinal, include_default_paint)
}

fn semantic_shape_style_attr_for_style(
    style: &ResolvedThemeStyle,
    target: ThemeTarget,
    variant: ThemeVariant,
    ordinal: Option<usize>,
    include_default_paint: bool,
) -> String {
    let mut emission = IndexMap::<String, EmittedDeclaration>::new();
    append_semantic_shape_emission(style, &mut emission);

    let resolve_paint = |slot, property: &ResolvedProperty<CanvasPaint>| {
        if include_default_paint {
            paint_value(property)
        } else {
            direct_paint(target, slot, property, variant, ordinal)
        }
    };
    if let Some(fill) = resolve_paint(PaintSlot::Fill, style.fill_resolution()) {
        insert_emitted(&mut emission, "fill", fill);
    }
    if let Some(stroke) = resolve_paint(PaintSlot::Stroke, style.stroke_resolution()) {
        insert_emitted(&mut emission, "stroke", stroke);
    }
    compact_style_attr(&emission)
}

fn semantic_html_background_style(
    resolved_theme: Option<&ResolvedDiagramTheme>,
    target: ThemeTarget,
    variant: ThemeVariant,
    ordinal: Option<usize>,
) -> String {
    let Some(theme) = resolved_theme else {
        return String::new();
    };
    let style = theme.style(target, variant, ordinal);
    match paint_value(style.fill_resolution()).as_deref() {
        Some("none") => "background-color: transparent !important; ".to_string(),
        Some(value) => format!("background-color: {value} !important; "),
        None => String::new(),
    }
}

fn push_declaration(
    declaration: SourceStyleDeclaration,
    shape: &mut Vec<SourceStyleDeclaration>,
    label: &mut Vec<SourceStyleDeclaration>,
) {
    if declaration.provenance().channel() == SourceStyleChannel::Label {
        label.push(declaration);
    } else {
        shape.push(declaration);
    }
}

fn declaration_channel(property: &str) -> SourceStyleChannel {
    if is_label_style_key(property) {
        SourceStyleChannel::Label
    } else {
        SourceStyleChannel::Shape
    }
}

fn semantic_binding(node: &StateDiagramRenderNode) -> (ThemeTarget, ThemeTarget, ThemeVariant) {
    match node.shape.as_str() {
        "note" | "noteGroup" => (
            ThemeTarget::Note,
            ThemeTarget::NoteLabel,
            ThemeVariant::Default,
        ),
        "roundedWithTitle" => (
            ThemeTarget::Composite,
            ThemeTarget::CompositeLabel,
            ThemeVariant::Default,
        ),
        "stateStart" => (
            ThemeTarget::SpecialState,
            ThemeTarget::StateLabel,
            ThemeVariant::Start,
        ),
        "stateEnd" => (
            ThemeTarget::SpecialState,
            ThemeTarget::StateLabel,
            ThemeVariant::End,
        ),
        "choice" | "fork" | "join" => (
            ThemeTarget::SpecialState,
            ThemeTarget::StateLabel,
            ThemeVariant::Special,
        ),
        _ if node.is_group || node.node_type.as_deref() == Some("group") => (
            ThemeTarget::Composite,
            ThemeTarget::CompositeLabel,
            ThemeVariant::Default,
        ),
        _ => (
            ThemeTarget::State,
            ThemeTarget::StateLabel,
            ThemeVariant::Default,
        ),
    }
}

fn participates_in_ordinal(node: &StateDiagramRenderNode) -> bool {
    node.shape != "noteGroup"
}

fn next_target_ordinal(ordinals: &mut BTreeMap<ThemeTarget, usize>, target: ThemeTarget) -> usize {
    let ordinal = ordinals.entry(target).or_default();
    *ordinal += 1;
    *ordinal
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

fn state_is_hidden_id(prefixes: &[String], id: &str) -> bool {
    prefixes.iter().any(|prefix| {
        id == prefix
            || id
                .strip_prefix(prefix)
                .is_some_and(|suffix| suffix.starts_with("----"))
    })
}

fn has_visible_label(node: &StateDiagramRenderNode) -> bool {
    if matches!(
        node.shape.as_str(),
        "stateStart" | "stateEnd" | "choice" | "fork" | "join" | "divider" | "noteGroup"
    ) {
        return false;
    }
    if node
        .label
        .as_ref()
        .is_some_and(|label| !label.to_string().trim_matches('"').trim().is_empty())
    {
        return true;
    }
    node.description
        .as_ref()
        .is_some_and(|lines| lines.iter().any(|line| !line.trim().is_empty()))
        || !node.id.trim().is_empty()
}

fn resolve_semantic_text_style(
    base: &TextStyle,
    theme: Option<&ResolvedDiagramTheme>,
    target: ThemeTarget,
    variant: ThemeVariant,
    ordinal: Option<usize>,
) -> TextStyle {
    let mut style = base.clone();
    let Some(theme) = theme else {
        return style;
    };
    let resolved = theme.style(target, variant, ordinal);
    let patch = resolved.typography_resolution().patch();
    if let Specified::Value(stack) = &patch.font_stack {
        style.font_family = Some(stack.as_css());
    }
    if let Specified::Value(size) = patch.font_size_px {
        style.font_size = f64::from(size).max(1.0);
    }
    if let Specified::Value(weight) = patch.font_weight {
        style.font_weight = Some(weight.to_string());
    }
    if let Specified::Value(font_style) = patch.font_style {
        style.font_style = Some(font_style.id().to_string());
    }
    style
}

fn apply_source_shape_property(
    declaration: &SourceStyleDeclaration,
    fill: &mut Option<String>,
    stroke: &mut Option<String>,
    stroke_width: &mut Option<f64>,
    radius: &mut Option<f64>,
    padding: &mut Option<f64>,
    residuals: &mut Vec<SourceStyleResidual>,
) -> bool {
    let value = declaration.value().trim();
    match declaration.property() {
        "fill" | "stroke" => {
            if declaration.is_single_component_value()
                && (value.eq_ignore_ascii_case("none")
                    || merman_core::theme_color::ThemeColor::parse(value).is_ok())
            {
                if declaration.property() == "fill" {
                    *fill = Some(value.to_string());
                } else {
                    *stroke = Some(value.to_string());
                }
                true
            } else {
                residuals.push(SourceStyleResidual::from_declaration(
                    declaration,
                    SourceStyleResidualReason::InvalidValue,
                ));
                false
            }
        }
        "stroke-width" => apply_numeric_source_property(declaration, stroke_width, residuals),
        "border-radius" | "rx" | "ry" => {
            apply_numeric_source_property(declaration, radius, residuals)
        }
        "padding" => apply_numeric_source_property(declaration, padding, residuals),
        "opacity" | "fill-opacity" | "stroke-opacity" | "stroke-dasharray" | "stroke-linecap"
        | "stroke-linejoin" => true,
        _ => {
            residuals.push(SourceStyleResidual::from_declaration(
                declaration,
                SourceStyleResidualReason::UnsupportedSurface,
            ));
            false
        }
    }
}

fn apply_numeric_source_property(
    declaration: &SourceStyleDeclaration,
    slot: &mut Option<f64>,
    residuals: &mut Vec<SourceStyleResidual>,
) -> bool {
    match declaration.svg_number_or_px() {
        Some(value) => {
            *slot = Some(value);
            true
        }
        None => {
            residuals.push(SourceStyleResidual::from_declaration(
                declaration,
                SourceStyleResidualReason::InvalidValue,
            ));
            false
        }
    }
}

fn apply_source_text_property(
    declaration: &SourceStyleDeclaration,
    inherited: &TextStyle,
    style: &mut TextStyle,
    residuals: &mut Vec<SourceStyleResidual>,
) -> bool {
    let value = declaration.value().trim();
    match declaration.property() {
        "font-family" if is_safe_css_font_family_value(value) && !value.is_empty() => {
            style.font_family = Some(value.to_string());
            true
        }
        "font-size" => match declaration
            .resolve_font_size_px(CssFontSizeContext::uniform(inherited.font_size))
        {
            Some(value) => {
                style.font_size = value;
                true
            }
            None => {
                residuals.push(SourceStyleResidual::from_declaration(
                    declaration,
                    SourceStyleResidualReason::InvalidValue,
                ));
                false
            }
        },
        "font-weight" if valid_font_weight(value) => {
            style.font_weight = Some(value.to_string());
            true
        }
        "font-style" if valid_font_style(value) => {
            style.font_style = Some(value.to_ascii_lowercase());
            true
        }
        "color"
            if declaration.is_single_component_value()
                && (merman_core::theme_color::ThemeColor::parse(value).is_ok()
                    || value.eq_ignore_ascii_case("currentcolor")) =>
        {
            true
        }
        "font-family" | "font-weight" | "font-style" | "color" => {
            residuals.push(SourceStyleResidual::from_declaration(
                declaration,
                SourceStyleResidualReason::InvalidValue,
            ));
            false
        }
        "line-height" | "letter-spacing" | "word-spacing" | "text-transform" | "white-space"
        | "word-wrap" | "word-break" | "overflow-wrap" | "text-align" | "text-decoration"
        | "text-shadow" | "text-overflow" | "hyphens" => {
            residuals.push(SourceStyleResidual::from_declaration(
                declaration,
                SourceStyleResidualReason::UnsupportedSurface,
            ));
            false
        }
        _ => {
            residuals.push(SourceStyleResidual::from_declaration(
                declaration,
                SourceStyleResidualReason::UnsupportedProperty,
            ));
            false
        }
    }
}

fn valid_font_weight(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "normal" | "bold" | "bolder" | "lighter"
    ) || value
        .parse::<u16>()
        .is_ok_and(|weight| (1..=1000).contains(&weight))
}

fn valid_font_style(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "normal" | "italic" | "oblique"
    )
}

#[derive(Debug, Clone)]
struct EmittedDeclaration {
    property: String,
    value: String,
}

impl EmittedDeclaration {
    fn from_source(declaration: &SourceStyleDeclaration) -> Self {
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

fn append_semantic_shape_emission(
    style: &ResolvedThemeStyle,
    out: &mut IndexMap<String, EmittedDeclaration>,
) {
    if let Some(width) = style.stroke_width() {
        insert_emitted(out, "stroke-width", format!("{}px", f64::from(width)));
    }
    if let Some(values) = style.stroke_dasharray() {
        insert_emitted(
            out,
            "stroke-dasharray",
            values
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" "),
        );
    }
    if let Some(linecap) = style.stroke_linecap() {
        insert_emitted(out, "stroke-linecap", linecap.id());
    }
    if let Some(linejoin) = style.stroke_linejoin() {
        insert_emitted(out, "stroke-linejoin", linejoin.id());
    }
    if let Some(opacity) = style.opacity() {
        insert_emitted(out, "opacity", opacity.to_string());
    }
    if let Some(opacity) = style.fill_opacity() {
        insert_emitted(out, "fill-opacity", opacity.to_string());
    }
    if let Some(opacity) = style.stroke_opacity() {
        insert_emitted(out, "stroke-opacity", opacity.to_string());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaintSlot {
    Fill,
    Stroke,
}

fn direct_paint(
    target: ThemeTarget,
    slot: PaintSlot,
    property: &ResolvedProperty<CanvasPaint>,
    requested_variant: ThemeVariant,
    requested_ordinal: Option<usize>,
) -> Option<String> {
    let winner = property.winner()?;
    let variant_specific = winner
        .variant()
        .is_some_and(|variant| variant != ThemeVariant::Default && variant == requested_variant);
    let ordinal_specific = winner.ordinal().is_some() && requested_ordinal.is_some();
    if !variant_specific && !ordinal_specific && paint_is_projected(target, slot) {
        return None;
    }
    paint_value(property)
}

fn paint_is_projected(target: ThemeTarget, slot: PaintSlot) -> bool {
    match (target, slot) {
        (ThemeTarget::State, PaintSlot::Fill | PaintSlot::Stroke)
        | (ThemeTarget::StateLabel, PaintSlot::Fill)
        | (ThemeTarget::Transition, PaintSlot::Stroke)
        | (ThemeTarget::TransitionMarker, PaintSlot::Fill | PaintSlot::Stroke)
        | (ThemeTarget::TransitionLabel, PaintSlot::Fill)
        | (ThemeTarget::Composite, PaintSlot::Fill)
        | (ThemeTarget::SpecialState, PaintSlot::Fill | PaintSlot::Stroke)
        | (ThemeTarget::Note, PaintSlot::Fill | PaintSlot::Stroke)
        | (ThemeTarget::NoteLabel, PaintSlot::Fill)
        | (ThemeTarget::Title, PaintSlot::Fill) => true,
        _ => false,
    }
}

fn paint_value(property: &ResolvedProperty<CanvasPaint>) -> Option<String> {
    match property.value()? {
        CanvasPaint::Transparent => Some("none".to_string()),
        CanvasPaint::Solid(color) => Some(color.as_css()),
        CanvasPaint::LinearGradient(_)
        | CanvasPaint::RadialGradient(_)
        | CanvasPaint::Pattern(_) => None,
    }
}

fn uniform_padding_px(padding: crate::diagram_theme::InsetsPx) -> Option<f64> {
    let values = [padding.top, padding.right, padding.bottom, padding.left];
    values
        .iter()
        .all(|value| (*value - values[0]).abs() <= f32::EPSILON)
        .then(|| f64::from(values[0]))
}

fn append_semantic_text_emission(
    style: &ResolvedThemeStyle,
    out: &mut IndexMap<String, EmittedDeclaration>,
) {
    let patch = style.typography_resolution().patch();
    if let Specified::Value(stack) = &patch.font_stack {
        insert_emitted(out, "font-family", stack.as_css());
    }
    if let Specified::Value(size) = patch.font_size_px {
        insert_emitted(out, "font-size", format!("{}px", f64::from(size)));
    }
    if let Specified::Value(weight) = patch.font_weight {
        insert_emitted(out, "font-weight", weight.to_string());
    }
    if let Specified::Value(font_style) = patch.font_style {
        insert_emitted(out, "font-style", font_style.id());
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

fn append_style_declaration(style: &mut String, property: &str, value: impl AsRef<str>) {
    if !style.is_empty() {
        style.push(';');
    }
    let _ = std::fmt::Write::write_fmt(
        style,
        format_args!("{property}:{} !important", value.as_ref()),
    );
}

fn compact_style_attr(declarations: &IndexMap<String, EmittedDeclaration>) -> String {
    declarations
        .values()
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
    use crate::diagram_theme::{TextStylePatch, ThemeRule, ThemeStylePatch};
    use merman_core::diagrams::state::{
        StateDiagramRenderEdge, StateDiagramRenderNode, StateDiagramRenderStyleClass,
    };
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
        }
    }

    #[test]
    fn class_declarations_parse_once_and_inline_invalid_values_do_not_erase_typed_winners() {
        let mut model = StateDiagramRenderModel::default();
        model.style_classes.insert(
            "first".to_string(),
            StateDiagramRenderStyleClass {
                id: "first".to_string(),
                styles: vec!["font-size:18px".to_string(), "fill:#111".to_string()],
                text_styles: Vec::new(),
            },
        );
        model.style_classes.insert(
            "second".to_string(),
            StateDiagramRenderStyleClass {
                id: "second".to_string(),
                styles: vec!["font-size:20px".to_string()],
                text_styles: Vec::new(),
            },
        );
        model.nodes.push(rect_node());

        let plan = StateStylePlan::resolve(&model, &json!({}), None);
        let node = plan.node("Ready").expect("prepared node");

        assert_eq!(node.text_style().font_size, 20.0);
        assert_eq!(node.fill_override(), Some("#333"));
        assert!(
            node.label_style_attr()
                .contains("font-size:20px !important")
        );
        assert!(!node.label_style_attr().contains("not-a-size"));
        assert!(plan.residuals().iter().any(|residual| {
            residual.property() == Some("font-size")
                && residual.reason() == SourceStyleResidualReason::InvalidValue
        }));
    }

    #[test]
    fn state_semantic_binding_uses_stable_target_local_ordinals() {
        let mut model = StateDiagramRenderModel::default();
        let mut first = rect_node();
        first.id = "A".to_string();
        first.css_classes.clear();
        first.css_compiled_styles.clear();
        first.css_styles.clear();
        let mut note = rect_node();
        note.id = "N".to_string();
        note.shape = "note".to_string();
        note.css_classes.clear();
        note.css_compiled_styles.clear();
        note.css_styles.clear();
        let mut second = first.clone();
        second.id = "B".to_string();
        let mut note_group = rect_node();
        note_group.id = "NG".to_string();
        note_group.shape = "noteGroup".to_string();
        note_group.is_group = true;
        note_group.node_type = Some("group".to_string());
        note_group.css_classes.clear();
        note_group.css_compiled_styles.clear();
        note_group.css_styles.clear();
        model.nodes.extend([first, note, note_group, second]);

        let plan = StateStylePlan::resolve(&model, &json!({}), None);
        assert_eq!(plan.node("A").unwrap().ordinal(), Some(1));
        assert_eq!(plan.node("N").unwrap().ordinal(), Some(1));
        assert_eq!(plan.node("NG").unwrap().ordinal(), None);
        assert_eq!(plan.node("B").unwrap().ordinal(), Some(2));
        assert_eq!(plan.node("N").unwrap().target(), ThemeTarget::Note);
    }

    #[test]
    fn state_edge_and_structural_roles_compile_into_one_layout_render_plan() {
        let mut model = StateDiagramRenderModel::default();
        let mut end = rect_node();
        end.id = "end".to_string();
        end.shape = "stateEnd".to_string();
        end.css_classes.clear();
        end.css_compiled_styles.clear();
        end.css_styles.clear();
        model.nodes.push(end);
        let mut composite = rect_node();
        composite.id = "cluster".to_string();
        composite.shape = "roundedWithTitle".to_string();
        composite.is_group = true;
        composite.node_type = Some("group".to_string());
        composite.label = Some(json!("Group"));
        composite.css_classes.clear();
        composite.css_compiled_styles.clear();
        composite.css_styles.clear();
        model.nodes.push(composite);
        model.edges.extend([
            StateDiagramRenderEdge {
                id: "transition-1".to_string(),
                start: "A".to_string(),
                end: "B".to_string(),
                classes: "transition".to_string(),
                arrow_type_end: "arrow_barb".to_string(),
                label: "go".to_string(),
            },
            StateDiagramRenderEdge {
                id: "note-edge".to_string(),
                start: "B".to_string(),
                end: "N".to_string(),
                classes: "transition note-edge".to_string(),
                arrow_type_end: String::new(),
                label: String::new(),
            },
        ]);

        let mut label_patch = TextStylePatch::default();
        label_patch.font_size_px = Specified::Value(24.0);
        let styles = crate::diagram_theme::ThemeRuleSet::default()
            .with_rule(ThemeRule::new(
                ThemeTarget::TransitionMarker,
                ThemeStylePatch::default()
                    .with_fill(CanvasPaint::solid("#00f2ff").unwrap())
                    .with_stroke(CanvasPaint::solid("#00f2ff").unwrap()),
            ))
            .with_rule(ThemeRule::new(
                ThemeTarget::TransitionLabelBackground,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#051423").unwrap()),
            ))
            .with_rule(ThemeRule::new(
                ThemeTarget::TransitionLabel,
                ThemeStylePatch {
                    typography: label_patch,
                    ..ThemeStylePatch::default()
                },
            ))
            .with_rule(ThemeRule::new(
                ThemeTarget::CompositeHeader,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ff00aa").unwrap()),
            ))
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::SpecialStateInner,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ffffff").unwrap()),
                )
                .with_variant(ThemeVariant::End),
            );
        let theme = crate::diagram_theme::DiagramThemeCompiler::new()
            .compile(crate::diagram_theme::DiagramThemeSpec::new().with_styles(styles))
            .expect("compile State structural theme");
        let resolved = theme.resolve(crate::render_family::RenderFamilyKind::State);
        let plan = StateStylePlan::resolve(&model, &json!({}), Some(&resolved));

        assert!(
            plan.transition_marker_style_attr()
                .contains("fill:#00f2ff !important")
        );
        assert!(
            plan.transition_label_background_style_attr()
                .contains("fill:#051423 !important")
        );
        assert!(
            plan.transition_label_background_div_style_prefix()
                .contains("background-color: #051423 !important")
        );
        assert!(
            plan.node("cluster")
                .unwrap()
                .composite_header_style_attr()
                .contains("fill:#ff00aa !important")
        );
        assert!(
            plan.node("end")
                .unwrap()
                .special_state_inner_style_attr()
                .contains("fill:#ffffff !important")
        );
        assert_eq!(
            plan.edge("transition-1").unwrap().text_style().font_size,
            24.0
        );
        assert_eq!(plan.edge("transition-1").unwrap().ordinal(), Some(1));
        assert_eq!(plan.edge("transition-1").unwrap().label_ordinal(), Some(1));
        assert_eq!(plan.edge("transition-1").unwrap().marker_ordinal(), Some(1));
        assert_eq!(plan.edge("note-edge").unwrap().ordinal(), None);
        assert_eq!(plan.node("cluster").unwrap().label_ordinal(), Some(1));
        assert_eq!(
            plan.node("cluster").unwrap().composite_header_ordinal(),
            Some(1)
        );
        assert_eq!(
            plan.node("end").unwrap().special_state_inner_ordinal(),
            Some(1)
        );
    }
}
