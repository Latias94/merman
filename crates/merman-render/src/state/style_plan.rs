use indexmap::IndexMap;
use merman_core::diagrams::state::{
    StateDiagramRenderEdge, StateDiagramRenderModel, StateDiagramRenderNode,
    StateDiagramRenderStyleClass,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::diagram_theme::{
    CanvasPaint, FamilyThemeMechanismKey, FontStyle, PreparedSourceStyleDeclaration,
    ResolvedDiagramTheme, ResolvedProperty, ResolvedStyleProperty, ResolvedThemeEffect,
    ResolvedThemeStyle, SourceStyleChannel, SourceStyleDeclaration, SourceStyleOrigin,
    SourceStyleProvenance, SourceStyleResidual, SourceStyleResidualReason, Specified,
    TextStylePatch, ThemeCapability, ThemeRule, ThemeTarget, ThemeTextStyle,
    ThemeTypographyProperty, ThemeVariant, collect_effect_graph_capabilities, paint_capability,
};
use crate::family::{FamilyThemeEvidence, FamilyThemeResidualReason};
use crate::mermaid_style::{
    CssFontSizeContext, is_label_style_key, is_safe_css_font_family_value,
    is_supported_css_font_style_value, is_supported_css_font_weight_value,
};
use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitExceeded};
use crate::text::TextStyle;
use crate::theme::MermaidThemeAdapter;

use super::{StateEffectOutsets, StateEffectPlan, StateNodeEffectPlan, StateSvgEffect};

/// One immutable typography decision shared by State layout, prepared-text measurement and SVG
/// emission.
///
/// `TextStyle` is the compatibility projection consumed by the ordinary measurer and CSS
/// emitter. When structured typography is active, `prepared_typography` is the same decision in
/// the typed representation retained by the prepared-text session. Source declarations may update
/// both views, but callers never need to merge them independently.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedLabelTypography {
    text_style: TextStyle,
    prepared_typography: Option<ThemeTextStyle>,
    source_font_stack: Option<crate::text::ParsedCssFontStack>,
}

impl ResolvedLabelTypography {
    pub(crate) fn new(text_style: TextStyle, prepared_typography: Option<ThemeTextStyle>) -> Self {
        Self {
            text_style,
            prepared_typography,
            source_font_stack: None,
        }
    }

    pub(crate) const fn text_style(&self) -> &TextStyle {
        &self.text_style
    }

    pub(crate) const fn prepared_typography(&self) -> Option<&ThemeTextStyle> {
        self.prepared_typography.as_ref()
    }

    pub(crate) const fn source_font_stack(&self) -> Option<&crate::text::ParsedCssFontStack> {
        self.source_font_stack.as_ref()
    }

    fn apply_font_family(&mut self, stack: crate::text::ParsedCssFontStack) {
        let css = stack.as_css();
        self.text_style.font_family = Some(css);
        if let Some(typography) = self.prepared_typography.as_mut() {
            *typography = typography
                .clone()
                .with_font_stack(stack.font_stack().clone());
        }
        self.source_font_stack = Some(stack);
    }

    fn apply_font_size(&mut self, value: f64) -> bool {
        if let Some(typography) = self.prepared_typography.as_mut() {
            let Ok(updated) = typography.clone().with_font_size_px(value as f32) else {
                return false;
            };
            *typography = updated;
        }
        self.text_style.font_size = value;
        true
    }

    fn apply_font_weight(&mut self, value: u16) {
        self.text_style.font_weight = Some(value.to_string());
        if let Some(typography) = self.prepared_typography.as_mut() {
            *typography = typography
                .clone()
                .with_font_weight(value)
                .expect("admitted CSS font weights are within the typed range");
        }
    }

    fn apply_font_style(&mut self, value: FontStyle) {
        self.text_style.font_style = Some(value.id().to_string());
        if let Some(typography) = self.prepared_typography.as_mut() {
            *typography = typography.clone().with_font_style(value);
        }
    }

    fn canonicalize_emission(&self, out: &mut IndexMap<String, EmittedDeclaration>) {
        if out.contains_key("font-family") {
            if let Some(family) = self.text_style.font_family.as_deref() {
                insert_emitted(out, "font-family", family);
            }
        }
        if out.contains_key("font-size") {
            insert_emitted(out, "font-size", format!("{}px", self.text_style.font_size));
        }
        if out.contains_key("font-weight") {
            if let Some(weight) = self.text_style.font_weight.as_deref() {
                insert_emitted(out, "font-weight", weight);
            }
        }
        if out.contains_key("font-style") {
            if let Some(font_style) = self.text_style.font_style.as_deref() {
                insert_emitted(out, "font-style", font_style);
            }
        }
        if self.prepared_typography.is_none() {
            out.shift_remove("letter-spacing");
            out.shift_remove("word-spacing");
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct StateCompatibilityStyle {
    pub(crate) dark_mode: bool,
    pub(crate) neo: bool,
    pub(crate) font_family_css: String,
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
    effect: Option<StateNodeEffectPlan>,
    cluster_label_typography: ResolvedLabelTypography,
    label_typography: ResolvedLabelTypography,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy)]
struct StateNodeThemeBinding {
    target: ThemeTarget,
    label_target: ThemeTarget,
    variant: ThemeVariant,
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
    label_typography: ResolvedLabelTypography,
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
        self.label_typography.text_style()
    }

    pub(crate) const fn resolved_label_typography(&self) -> &ResolvedLabelTypography {
        &self.label_typography
    }
}

impl StateNodeStylePlan {
    #[cfg(test)]
    pub(crate) const fn target(&self) -> ThemeTarget {
        self.binding.target
    }

    #[cfg(test)]
    pub(crate) const fn label_target(&self) -> ThemeTarget {
        self.binding.label_target
    }

    #[cfg(test)]
    pub(crate) const fn variant(&self) -> ThemeVariant {
        self.binding.variant
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
        self.label_typography.text_style()
    }

    pub(crate) const fn resolved_label_typography(&self) -> &ResolvedLabelTypography {
        &self.label_typography
    }

    pub(crate) const fn resolved_cluster_label_typography(&self) -> &ResolvedLabelTypography {
        &self.cluster_label_typography
    }

    pub(crate) const fn effect(&self) -> Option<&StateNodeEffectPlan> {
        self.effect.as_ref()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct StateStylePlan {
    compatibility: StateCompatibilityStyle,
    structured_typography: bool,
    base_label_typography: ResolvedLabelTypography,
    transition_label_typography: ResolvedLabelTypography,
    composite_label_typography: ResolvedLabelTypography,
    title_label_typography: ResolvedLabelTypography,
    title_style_attr: String,
    transition_marker_style_attr: String,
    transition_label_background_style_attr: String,
    transition_label_background_div_style_prefix: String,
    classes: IndexMap<String, StateClassStylePlan>,
    nodes: BTreeMap<String, StateNodeStylePlan>,
    edges: BTreeMap<String, StateEdgeStylePlan>,
    effects: StateEffectPlan,
    residuals: Vec<SourceStyleResidual>,
}

#[derive(Debug)]
struct StateThemeEvidenceBuilder<'a> {
    theme: Option<&'a ResolvedDiagramTheme>,
    uses: Vec<StateThemeUse>,
    observed_targets: BTreeSet<ThemeTarget>,
    available_palettes: BTreeSet<ThemeTarget>,
    consumed_palettes: BTreeSet<ThemeTarget>,
    applied_effect_bindings: BTreeSet<StateEffectBindingKey>,
    residual_effect_bindings: BTreeSet<StateEffectBindingKey>,
    suppressed_effect_bindings: BTreeSet<StateEffectBindingKey>,
    has_visible_text: bool,
    base_typography_properties: BTreeSet<ThemeTypographyProperty>,
    prepared_text_available: bool,
    base_typography_applied: bool,
    base_typography_residual: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StateThemeUseId(usize);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct StateEffectBindingKey {
    target: ThemeTarget,
    effect_id: String,
}

impl StateEffectBindingKey {
    fn from_binding(binding: &crate::diagram_theme::EffectBinding) -> Self {
        Self {
            target: binding.target(),
            effect_id: binding.effect_id().to_string(),
        }
    }

    fn family_key(&self) -> FamilyThemeMechanismKey {
        FamilyThemeMechanismKey::EffectBinding {
            target: self.target,
            effect_id: self.effect_id.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StateThemePropertyOutcome {
    Applied,
    Residual(FamilyThemeResidualReason),
    SupersededBySource,
}

#[derive(Debug)]
struct StateThemePropertyUse {
    rule_index: usize,
    property: ResolvedStyleProperty,
    outcome: StateThemePropertyOutcome,
}

#[derive(Debug)]
struct StateThemeUse {
    properties: Vec<StateThemePropertyUse>,
}

impl<'a> StateThemeEvidenceBuilder<'a> {
    fn new(theme: Option<&'a ResolvedDiagramTheme>, prepared_text_available: bool) -> Self {
        let base_typography_properties = theme
            .map(|theme| configured_base_typography_properties(theme.typography()))
            .unwrap_or_default();
        Self {
            theme,
            uses: Vec::new(),
            observed_targets: BTreeSet::new(),
            available_palettes: BTreeSet::new(),
            consumed_palettes: BTreeSet::new(),
            applied_effect_bindings: BTreeSet::new(),
            residual_effect_bindings: BTreeSet::new(),
            suppressed_effect_bindings: BTreeSet::new(),
            has_visible_text: false,
            base_typography_properties,
            prepared_text_available,
            base_typography_applied: false,
            base_typography_residual: false,
        }
    }

    fn observe_target(&mut self, target: ThemeTarget) {
        self.observed_targets.insert(target);
    }

    fn observe_text(&mut self, target: ThemeTarget) {
        self.has_visible_text = true;
        self.observe_target(ThemeTarget::Text);
        self.observe_target(target);
    }

    fn observe_base_typography_use(
        &mut self,
        semantic: Option<&ResolvedThemeStyle>,
        shadowed_source_properties: &BTreeSet<ResolvedStyleProperty>,
    ) {
        if self.base_typography_properties.is_empty() {
            return;
        }
        let patch = semantic.map(|style| style.typography_resolution().patch());
        for property in &self.base_typography_properties {
            let resolved_property = ResolvedStyleProperty::Typography(*property);
            if shadowed_source_properties.contains(&resolved_property)
                || patch.is_some_and(|patch| {
                    semantic_typography_property_overridden(patch, resolved_property)
                })
            {
                continue;
            }
            match property {
                ThemeTypographyProperty::FontStack
                | ThemeTypographyProperty::FontSize
                | ThemeTypographyProperty::FontWeight
                | ThemeTypographyProperty::FontStyle => self.base_typography_applied = true,
                ThemeTypographyProperty::LetterSpacing | ThemeTypographyProperty::WordSpacing => {
                    if self.prepared_text_available {
                        self.base_typography_applied = true;
                    } else {
                        self.base_typography_residual = true;
                    }
                }
                ThemeTypographyProperty::LineHeight
                | ThemeTypographyProperty::Transform
                | ThemeTypographyProperty::Decoration
                | ThemeTypographyProperty::TextAlign
                | ThemeTypographyProperty::WhiteSpace
                | ThemeTypographyProperty::Wrap => self.base_typography_residual = true,
            }
        }
    }
}

fn configured_base_typography_properties(
    typography: &ThemeTextStyle,
) -> BTreeSet<ThemeTypographyProperty> {
    let default = ThemeTextStyle::default();
    let mut properties = BTreeSet::new();
    if typography.font_stack() != default.font_stack() {
        properties.insert(ThemeTypographyProperty::FontStack);
    }
    if (typography.font_size_px() - default.font_size_px()).abs() > f32::EPSILON {
        properties.insert(ThemeTypographyProperty::FontSize);
    }
    if typography.font_weight() != default.font_weight() {
        properties.insert(ThemeTypographyProperty::FontWeight);
    }
    if typography.font_style() != default.font_style() {
        properties.insert(ThemeTypographyProperty::FontStyle);
    }
    if typography.line_height() != crate::diagram_theme::LineHeight::Normal {
        properties.insert(ThemeTypographyProperty::LineHeight);
    }
    if typography.letter_spacing_px().abs() > f32::EPSILON {
        properties.insert(ThemeTypographyProperty::LetterSpacing);
    }
    if typography.word_spacing_px().abs() > f32::EPSILON {
        properties.insert(ThemeTypographyProperty::WordSpacing);
    }
    if typography.transform() != crate::diagram_theme::TextTransform::None {
        properties.insert(ThemeTypographyProperty::Transform);
    }
    if typography.decoration() != crate::diagram_theme::TextDecoration::None {
        properties.insert(ThemeTypographyProperty::Decoration);
    }
    if typography.text_align() != crate::diagram_theme::TextAlign::Start {
        properties.insert(ThemeTypographyProperty::TextAlign);
    }
    if typography.white_space() != crate::diagram_theme::WhiteSpace::Normal {
        properties.insert(ThemeTypographyProperty::WhiteSpace);
    }
    if typography.wrap() != crate::diagram_theme::ThemeWrapMode::Normal {
        properties.insert(ThemeTypographyProperty::Wrap);
    }
    properties
}

fn semantic_typography_property_overridden(
    patch: &TextStylePatch,
    property: ResolvedStyleProperty,
) -> bool {
    let ResolvedStyleProperty::Typography(property) = property else {
        return false;
    };
    match property {
        ThemeTypographyProperty::FontStack => !patch.font_stack.is_unspecified(),
        ThemeTypographyProperty::FontSize => !patch.font_size_px.is_unspecified(),
        ThemeTypographyProperty::FontWeight => !patch.font_weight.is_unspecified(),
        ThemeTypographyProperty::FontStyle => !patch.font_style.is_unspecified(),
        ThemeTypographyProperty::LineHeight => !patch.line_height.is_unspecified(),
        ThemeTypographyProperty::LetterSpacing => !patch.letter_spacing_px.is_unspecified(),
        ThemeTypographyProperty::WordSpacing => !patch.word_spacing_px.is_unspecified(),
        ThemeTypographyProperty::Transform => !patch.transform.is_unspecified(),
        ThemeTypographyProperty::Decoration => !patch.decoration.is_unspecified(),
        ThemeTypographyProperty::TextAlign => !patch.text_align.is_unspecified(),
        ThemeTypographyProperty::WhiteSpace => !patch.white_space.is_unspecified(),
        ThemeTypographyProperty::Wrap => !patch.wrap.is_unspecified(),
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct StateGeometrySupport {
    radius: bool,
    padding: bool,
}

impl StateGeometrySupport {
    const NONE: Self = Self {
        radius: false,
        padding: false,
    };
    const BOTH: Self = Self {
        radius: true,
        padding: true,
    };
    const PADDING: Self = Self {
        radius: false,
        padding: true,
    };
}

impl StateStylePlan {
    /// Resolves the compatibility-only plan used by parity helpers that do not install a theme.
    ///
    /// The themed production path must use [`Self::resolve_with_evidence`] so dynamic selector
    /// work is charged to the render operation's cumulative meter.
    pub(crate) fn resolve_unthemed(
        model: &StateDiagramRenderModel,
        effective_config: &serde_json::Value,
    ) -> Self {
        let work_meter =
            OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        Self::resolve_internal(model, effective_config, None, None, false, &work_meter)
            .expect("an unthemed State style plan cannot exhaust theme selector work")
            .0
    }

    pub(crate) fn resolve_with_evidence(
        model: &StateDiagramRenderModel,
        effective_config: &serde_json::Value,
        resolved_theme: Option<&ResolvedDiagramTheme>,
        title: Option<&str>,
        prepared_text_available: bool,
        work_meter: &OperationWorkMeter,
    ) -> Result<(Self, FamilyThemeEvidence), ResourceLimitExceeded> {
        Self::resolve_internal(
            model,
            effective_config,
            resolved_theme,
            title,
            prepared_text_available,
            work_meter,
        )
    }

    fn resolve_internal(
        model: &StateDiagramRenderModel,
        effective_config: &serde_json::Value,
        resolved_theme: Option<&ResolvedDiagramTheme>,
        title: Option<&str>,
        prepared_text_available: bool,
        work_meter: &OperationWorkMeter,
    ) -> Result<(Self, FamilyThemeEvidence), ResourceLimitExceeded> {
        let compatibility = MermaidThemeAdapter::new(effective_config).state_diagram();
        let config_view = super::StateConfigView::new(effective_config);
        let config_text_style = config_view.text_style();
        let render_settings = config_view.render_settings();
        let html_labels = render_settings.html_labels;
        let diagram_look = render_settings.diagram_look;
        let structured_typography =
            resolved_theme.is_some_and(|theme| theme.typography() != &ThemeTextStyle::default());
        let base_text_typography = prepared_text_available
            .then(|| resolved_theme.map(|theme| theme.typography().clone()))
            .flatten();
        let mut base_text_style = config_text_style.clone();
        if let Some(theme) = resolved_theme {
            apply_theme_base_typography(&mut base_text_style, theme.typography());
        }
        let mut title_base_text_style = config_text_style;
        title_base_text_style.font_size = 18.0;
        if let Some(theme) = resolved_theme {
            apply_theme_base_typography(&mut title_base_text_style, theme.typography());
        }
        let mut residuals = Vec::new();
        let mut theme_evidence =
            StateThemeEvidenceBuilder::new(resolved_theme, prepared_text_available);
        let has_title = title.is_some_and(|value| !value.trim().is_empty());
        if has_title {
            theme_evidence.observe_text(ThemeTarget::Title);
        }
        let classes = prepare_classes(model, &mut residuals);
        let mut effects = StateEffectPlan::default();

        let semantic_transition_text = resolve_theme_text_style(
            resolved_theme,
            ThemeTarget::TransitionLabel,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let transition_text_style =
            resolve_semantic_text_style(&base_text_style, semantic_transition_text.as_ref());
        let transition_text_typography = prepared_text_available
            .then(|| {
                semantic_transition_text
                    .as_ref()
                    .map(|style| style.typography().clone())
            })
            .flatten();
        let semantic_composite_text = resolve_theme_text_style(
            resolved_theme,
            ThemeTarget::CompositeLabel,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let composite_text_style =
            resolve_semantic_text_style(&base_text_style, semantic_composite_text.as_ref());
        let composite_text_typography = prepared_text_available
            .then(|| {
                semantic_composite_text
                    .as_ref()
                    .map(|style| style.typography().clone())
            })
            .flatten();
        let semantic_title_text = resolve_theme_text_style(
            resolved_theme,
            ThemeTarget::Title,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let title_text_style =
            resolve_semantic_text_style(&title_base_text_style, semantic_title_text.as_ref());
        let base_label_typography =
            ResolvedLabelTypography::new(base_text_style.clone(), base_text_typography);
        let transition_label_typography =
            ResolvedLabelTypography::new(transition_text_style, transition_text_typography);
        let composite_label_typography =
            ResolvedLabelTypography::new(composite_text_style, composite_text_typography);
        let title_label_typography = ResolvedLabelTypography::new(
            title_text_style,
            prepared_text_available
                .then(|| {
                    semantic_title_text
                        .as_ref()
                        .map(|style| style.typography().clone())
                })
                .flatten(),
        );
        if has_title {
            theme_evidence
                .observe_base_typography_use(semantic_title_text.as_ref(), &BTreeSet::new());
        }
        let title_style_attr = resolved_theme
            .zip(semantic_title_text.as_ref())
            .map(|(theme, style)| {
                if has_title {
                    theme_evidence.consume_text_style(style);
                }
                let mut emission = IndexMap::new();
                append_theme_base_text_emission(theme, &mut emission);
                append_semantic_text_emission(style, &mut emission);
                title_label_typography.canonicalize_emission(&mut emission);
                if let Some(color) = text_paint_value(style.fill_resolution()) {
                    insert_emitted(&mut emission, "color", color);
                }
                compact_style_attr(&emission)
            })
            .unwrap_or_default();

        let semantic_transition_marker = resolve_theme_style(
            resolved_theme,
            ThemeTarget::TransitionMarker,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let transition_marker_style_attr =
            semantic_shape_style_attr(semantic_transition_marker.as_ref());
        let semantic_transition_label_background = resolve_theme_style(
            resolved_theme,
            ThemeTarget::TransitionLabelBackground,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let transition_label_background_style_attr =
            semantic_shape_style_attr(semantic_transition_label_background.as_ref());
        let transition_label_background_div_style_prefix =
            semantic_html_background_style(semantic_transition_label_background.as_ref());
        let hidden_prefixes = hidden_state_prefixes(model);
        let shadowed_self_loops = shadowed_self_loop_edge_indices(model, &hidden_prefixes);
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
                && node.shape == "stateEnd")
                .then(|| next_target_ordinal(&mut target_ordinals, ThemeTarget::SpecialStateInner));
            if participates {
                theme_evidence.observe_target(target);
                if label_ordinal.is_some() {
                    theme_evidence.observe_text(label_target);
                }
                if composite_header_ordinal.is_some() {
                    theme_evidence.observe_target(ThemeTarget::CompositeHeader);
                }
                if special_state_inner_ordinal.is_some() {
                    theme_evidence.observe_target(ThemeTarget::SpecialStateInner);
                }
            }
            let node_plan = prepare_node(
                node,
                &classes,
                &base_text_style,
                resolved_theme,
                target,
                label_target,
                variant,
                diagram_look.as_str(),
                one_based_ordinal,
                label_ordinal,
                composite_header_ordinal,
                special_state_inner_ordinal,
                &mut residuals,
                &mut theme_evidence,
                &mut effects,
                work_meter,
            )?;
            nodes.insert(node.id.clone(), node_plan);
        }

        let mut transition_ordinal = 0usize;
        let mut edges = BTreeMap::new();
        for (edge_index, edge) in model.edges.iter().enumerate() {
            let hidden = state_edge_is_hidden(edge, &hidden_prefixes)
                || shadowed_self_loops.contains(&edge_index);
            let ordinal = (!hidden).then(|| {
                transition_ordinal += 1;
                transition_ordinal
            });
            let label_ordinal = (ordinal.is_some() && !edge.label.trim().is_empty())
                .then(|| next_target_ordinal(&mut target_ordinals, ThemeTarget::TransitionLabel));
            let marker_ordinal = (ordinal.is_some() && !edge.arrow_type_end.trim().is_empty())
                .then(|| next_target_ordinal(&mut target_ordinals, ThemeTarget::TransitionMarker));
            if ordinal.is_some() {
                theme_evidence.observe_target(ThemeTarget::Transition);
                if label_ordinal.is_some() {
                    theme_evidence.observe_text(ThemeTarget::TransitionLabel);
                    theme_evidence.observe_target(ThemeTarget::TransitionLabelBackground);
                }
                if marker_ordinal.is_some() {
                    theme_evidence.observe_target(ThemeTarget::TransitionMarker);
                }
            }
            edges.insert(
                edge.id.clone(),
                prepare_edge(
                    &base_text_style,
                    resolved_theme,
                    ordinal,
                    label_ordinal,
                    marker_ordinal,
                    html_labels,
                    &mut theme_evidence,
                    work_meter,
                )?,
            );
        }

        let plan = Self {
            compatibility,
            structured_typography,
            base_label_typography,
            transition_label_typography,
            composite_label_typography,
            title_label_typography,
            title_style_attr,
            transition_marker_style_attr,
            transition_label_background_style_attr,
            transition_label_background_div_style_prefix,
            classes,
            nodes,
            edges,
            effects,
            residuals,
        };
        Ok((plan, theme_evidence.finish()))
    }

    pub(crate) const fn compatibility(&self) -> &StateCompatibilityStyle {
        &self.compatibility
    }

    pub(crate) const fn uses_structured_typography(&self) -> bool {
        self.structured_typography
    }

    pub(crate) const fn base_text_style(&self) -> &TextStyle {
        self.base_label_typography.text_style()
    }

    pub(crate) const fn base_label_typography(&self) -> &ResolvedLabelTypography {
        &self.base_label_typography
    }

    pub(crate) const fn transition_label_typography(&self) -> &ResolvedLabelTypography {
        &self.transition_label_typography
    }

    pub(crate) const fn composite_label_typography(&self) -> &ResolvedLabelTypography {
        &self.composite_label_typography
    }

    pub(crate) const fn title_text_style(&self) -> &TextStyle {
        self.title_label_typography.text_style()
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

    pub(crate) fn effects(&self) -> impl ExactSizeIterator<Item = &StateSvgEffect> {
        self.effects.effects()
    }

    pub(crate) fn effect(&self, id: &str) -> Option<&StateSvgEffect> {
        self.effects.effect(id)
    }

    pub(crate) const fn effect_outsets(&self) -> StateEffectOutsets {
        self.effects.outsets()
    }

    pub(crate) fn expected_native_filter_application_count(&self) -> usize {
        self.nodes
            .values()
            .filter_map(StateNodeStylePlan::effect)
            .count()
    }

    pub(crate) fn residuals(&self) -> &[SourceStyleResidual] {
        &self.residuals
    }
}

fn apply_theme_base_typography(base: &mut TextStyle, style: &ThemeTextStyle) {
    if style == &ThemeTextStyle::default() {
        return;
    }
    // Keep structured theme typography on Mermaid's canonical CSS spelling at the compatibility
    // boundary. The typed stack retains family identity; only separator whitespace is normalized.
    base.font_family = Some(crate::config::normalize_css_font_family(
        &style.font_stack().as_css(),
    ));
    base.font_size = f64::from(style.font_size_px()).max(1.0);
    base.font_weight = Some(style.font_weight().to_string());
    base.font_style = Some(style.font_style().id().to_string());
}

impl StateThemeEvidenceBuilder<'_> {
    fn consume_shape_style(
        &mut self,
        style: &ResolvedThemeStyle,
        geometry: StateGeometrySupport,
    ) -> StateThemeUseId {
        let use_id = self.begin_use(style);
        self.consume_paint(
            use_id,
            style.fill_resolution(),
            ResolvedStyleProperty::Fill,
            true,
        );
        self.consume_paint(
            use_id,
            style.stroke_resolution(),
            ResolvedStyleProperty::Stroke,
            true,
        );
        self.consume_shape_property(
            use_id,
            style.stroke_width_resolution(),
            ResolvedStyleProperty::StrokeWidth,
        );
        self.consume_shape_property(
            use_id,
            style.stroke_dasharray_resolution(),
            ResolvedStyleProperty::StrokeDasharray,
        );
        self.consume_shape_property(
            use_id,
            style.stroke_linecap_resolution(),
            ResolvedStyleProperty::StrokeLinecap,
        );
        self.consume_shape_property(
            use_id,
            style.stroke_linejoin_resolution(),
            ResolvedStyleProperty::StrokeLinejoin,
        );
        self.consume_shape_property(
            use_id,
            style.opacity_resolution(),
            ResolvedStyleProperty::Opacity,
        );
        self.consume_shape_property(
            use_id,
            style.fill_opacity_resolution(),
            ResolvedStyleProperty::FillOpacity,
        );
        self.consume_shape_property(
            use_id,
            style.stroke_opacity_resolution(),
            ResolvedStyleProperty::StrokeOpacity,
        );
        self.consume_geometry(use_id, style, geometry);
        self.reject_all_typography(use_id, style);
        self.reject_property(
            use_id,
            style.effect_resolution(),
            ResolvedStyleProperty::Effect,
            FamilyThemeResidualReason::UnsupportedEffect,
        );
        use_id
    }

    fn consume_text_style(&mut self, style: &ResolvedThemeStyle) -> StateThemeUseId {
        let use_id = self.begin_use(style);
        self.consume_paint(
            use_id,
            style.fill_resolution(),
            ResolvedStyleProperty::Fill,
            true,
        );
        self.consume_paint(
            use_id,
            style.stroke_resolution(),
            ResolvedStyleProperty::Stroke,
            false,
        );
        self.reject_property(
            use_id,
            style.stroke_width_resolution(),
            ResolvedStyleProperty::StrokeWidth,
            FamilyThemeResidualReason::UnsupportedGeometry,
        );
        self.reject_property(
            use_id,
            style.stroke_dasharray_resolution(),
            ResolvedStyleProperty::StrokeDasharray,
            FamilyThemeResidualReason::UnsupportedGeometry,
        );
        self.reject_property(
            use_id,
            style.stroke_linecap_resolution(),
            ResolvedStyleProperty::StrokeLinecap,
            FamilyThemeResidualReason::UnsupportedGeometry,
        );
        self.reject_property(
            use_id,
            style.stroke_linejoin_resolution(),
            ResolvedStyleProperty::StrokeLinejoin,
            FamilyThemeResidualReason::UnsupportedGeometry,
        );
        self.reject_property(
            use_id,
            style.opacity_resolution(),
            ResolvedStyleProperty::Opacity,
            FamilyThemeResidualReason::UnsupportedGeometry,
        );
        self.reject_property(
            use_id,
            style.fill_opacity_resolution(),
            ResolvedStyleProperty::FillOpacity,
            FamilyThemeResidualReason::UnsupportedGeometry,
        );
        self.reject_property(
            use_id,
            style.stroke_opacity_resolution(),
            ResolvedStyleProperty::StrokeOpacity,
            FamilyThemeResidualReason::UnsupportedGeometry,
        );
        self.reject_property(
            use_id,
            style.radius_resolution(),
            ResolvedStyleProperty::Radius,
            FamilyThemeResidualReason::UnsupportedGeometry,
        );
        self.reject_property(
            use_id,
            style.padding_resolution(),
            ResolvedStyleProperty::Padding,
            FamilyThemeResidualReason::UnsupportedGeometry,
        );
        self.consume_simple_typography(use_id, style);
        self.reject_advanced_typography(use_id, style);
        self.reject_property(
            use_id,
            style.effect_resolution(),
            ResolvedStyleProperty::Effect,
            FamilyThemeResidualReason::UnsupportedEffect,
        );
        use_id
    }

    fn consume_html_background_style(&mut self, style: &ResolvedThemeStyle) -> StateThemeUseId {
        let use_id = self.begin_use(style);
        self.consume_paint(
            use_id,
            style.fill_resolution(),
            ResolvedStyleProperty::Fill,
            true,
        );
        self.consume_paint(
            use_id,
            style.stroke_resolution(),
            ResolvedStyleProperty::Stroke,
            false,
        );
        self.reject_property(
            use_id,
            style.stroke_width_resolution(),
            ResolvedStyleProperty::StrokeWidth,
            FamilyThemeResidualReason::UnsupportedGeometry,
        );
        self.reject_property(
            use_id,
            style.stroke_dasharray_resolution(),
            ResolvedStyleProperty::StrokeDasharray,
            FamilyThemeResidualReason::UnsupportedGeometry,
        );
        self.reject_property(
            use_id,
            style.stroke_linecap_resolution(),
            ResolvedStyleProperty::StrokeLinecap,
            FamilyThemeResidualReason::UnsupportedGeometry,
        );
        self.reject_property(
            use_id,
            style.stroke_linejoin_resolution(),
            ResolvedStyleProperty::StrokeLinejoin,
            FamilyThemeResidualReason::UnsupportedGeometry,
        );
        self.reject_property(
            use_id,
            style.opacity_resolution(),
            ResolvedStyleProperty::Opacity,
            FamilyThemeResidualReason::UnsupportedGeometry,
        );
        self.reject_property(
            use_id,
            style.fill_opacity_resolution(),
            ResolvedStyleProperty::FillOpacity,
            FamilyThemeResidualReason::UnsupportedGeometry,
        );
        self.reject_property(
            use_id,
            style.stroke_opacity_resolution(),
            ResolvedStyleProperty::StrokeOpacity,
            FamilyThemeResidualReason::UnsupportedGeometry,
        );
        self.reject_property(
            use_id,
            style.radius_resolution(),
            ResolvedStyleProperty::Radius,
            FamilyThemeResidualReason::UnsupportedGeometry,
        );
        self.reject_property(
            use_id,
            style.padding_resolution(),
            ResolvedStyleProperty::Padding,
            FamilyThemeResidualReason::UnsupportedGeometry,
        );
        self.reject_all_typography(use_id, style);
        self.reject_property(
            use_id,
            style.effect_resolution(),
            ResolvedStyleProperty::Effect,
            FamilyThemeResidualReason::UnsupportedEffect,
        );
        use_id
    }

    fn series_color(&mut self, target: ThemeTarget, one_based_ordinal: usize) -> Option<String> {
        let color = self
            .theme
            .and_then(|theme| theme.series_color(target, one_based_ordinal))
            .map(|color| color.as_css());
        if color.is_some() {
            self.available_palettes.insert(target);
        }
        color
    }

    fn record_series_color_emission(&mut self, target: ThemeTarget) {
        self.consumed_palettes.insert(target);
    }

    fn consume_effect(
        &mut self,
        use_id: StateThemeUseId,
        target: ThemeTarget,
        property: &ResolvedProperty<String>,
        resolved: Option<ResolvedThemeEffect<'_>>,
        surface_supported: bool,
        effects: &mut StateEffectPlan,
    ) -> Option<StateNodeEffectPlan> {
        match resolved {
            None => None,
            Some(ResolvedThemeEffect::ClearedByRule) => {
                self.suppress_effect_binding(target);
                self.accept_property(use_id, property, ResolvedStyleProperty::Effect);
                None
            }
            Some(ResolvedThemeEffect::Rule { graph }) => {
                self.suppress_effect_binding(target);
                let effect = surface_supported
                    .then_some(graph)
                    .flatten()
                    .and_then(|graph| effects.admit(graph));
                if effect.is_some() {
                    self.accept_property(use_id, property, ResolvedStyleProperty::Effect);
                } else {
                    self.reject_property(
                        use_id,
                        property,
                        ResolvedStyleProperty::Effect,
                        FamilyThemeResidualReason::UnsupportedEffect,
                    );
                }
                effect
            }
            Some(ResolvedThemeEffect::Binding { binding, graph }) => {
                let key = StateEffectBindingKey::from_binding(binding);
                let effect = surface_supported
                    .then_some(graph)
                    .flatten()
                    .and_then(|graph| effects.admit(graph));
                if effect.is_some() {
                    self.applied_effect_bindings.insert(key);
                } else {
                    self.residual_effect_bindings.insert(key);
                }
                effect
            }
        }
    }

    fn suppress_effect_binding(&mut self, target: ThemeTarget) {
        let Some(theme) = self.theme else {
            return;
        };
        self.suppressed_effect_bindings.extend(
            theme
                .effect_bindings()
                .filter(|binding| binding.target() == target)
                .map(StateEffectBindingKey::from_binding),
        );
    }

    fn accept_property<T>(
        &mut self,
        use_id: StateThemeUseId,
        property: &ResolvedProperty<T>,
        kind: ResolvedStyleProperty,
    ) {
        if let Some(origin) = property.winner() {
            self.set_property_outcome(
                use_id,
                origin.rule_index(),
                kind,
                StateThemePropertyOutcome::Applied,
            );
        }
    }

    fn finish(self) -> FamilyThemeEvidence {
        let mut evidence = FamilyThemeEvidence::from_theme(self.theme);
        let Some(theme) = self.theme else {
            return evidence;
        };

        if theme.typography() != &ThemeTextStyle::default() {
            if !self.has_visible_text {
                evidence.mark_not_applicable(FamilyThemeMechanismKey::Typography);
            } else if self.base_typography_residual {
                evidence.mark_residual(
                    FamilyThemeMechanismKey::Typography,
                    FamilyThemeResidualReason::UnsupportedTypography,
                );
            } else if self.base_typography_applied {
                evidence.mark_applied_with_capabilities(
                    FamilyThemeMechanismKey::Typography,
                    typography_capabilities(&self.base_typography_properties),
                );
            } else {
                evidence.mark_not_applicable(FamilyThemeMechanismKey::Typography);
            }
        }

        let mut consumed_rules = BTreeMap::<usize, BTreeSet<ResolvedStyleProperty>>::new();
        let mut residual_rules = BTreeMap::new();
        for use_record in &self.uses {
            for property in &use_record.properties {
                match property.outcome {
                    StateThemePropertyOutcome::Applied => {
                        consumed_rules
                            .entry(property.rule_index)
                            .or_default()
                            .insert(property.property);
                    }
                    StateThemePropertyOutcome::SupersededBySource => {}
                    StateThemePropertyOutcome::Residual(reason) => {
                        residual_rules.entry(property.rule_index).or_insert(reason);
                    }
                }
            }
        }

        for (index, rule) in theme.family_rules() {
            let key = FamilyThemeMechanismKey::Rule {
                index,
                target: rule.target(),
            };
            if let Some(reason) = residual_rules.get(&index).copied() {
                evidence.mark_residual(key, reason);
            } else if let Some(properties) = consumed_rules.get(&index) {
                evidence.mark_applied_with_capabilities(
                    key,
                    rule_capabilities(theme, rule, properties),
                );
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        for target in theme.family_ordinal_palette_targets() {
            let key = FamilyThemeMechanismKey::OrdinalPalette { target };
            if self.consumed_palettes.contains(&target) {
                evidence.mark_applied_with_capabilities(key, [ThemeCapability::SolidPaint]);
            } else if self.available_palettes.contains(&target) {
                evidence.mark_not_applicable(key);
            } else if self.observed_targets.contains(&target) {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedOrdinalPalette);
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        for binding in theme.effect_bindings() {
            let binding_key = StateEffectBindingKey::from_binding(binding);
            let key = binding_key.family_key();
            if self.residual_effect_bindings.contains(&binding_key) {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedEffect);
            } else if self.applied_effect_bindings.contains(&binding_key) {
                let mut capabilities = BTreeSet::new();
                if let Some(graph) = theme.effect_graph(binding.effect_id()) {
                    collect_effect_graph_capabilities(graph, &mut capabilities);
                }
                evidence.mark_applied_with_capabilities(key, capabilities);
            } else if self.suppressed_effect_bindings.contains(&binding_key) {
                evidence.mark_not_applicable(key);
            } else if self.observed_targets.contains(&binding.target()) {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedEffect);
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        evidence
    }

    fn begin_use(&mut self, style: &ResolvedThemeStyle) -> StateThemeUseId {
        let use_id = StateThemeUseId(self.uses.len());
        let properties = style
            .winner_rule_properties()
            .into_iter()
            .map(|(property, origin)| StateThemePropertyUse {
                rule_index: origin.rule_index(),
                property,
                outcome: StateThemePropertyOutcome::Applied,
            })
            .collect();
        self.uses.push(StateThemeUse { properties });
        use_id
    }

    fn shadow_source_property(
        &mut self,
        use_id: Option<StateThemeUseId>,
        kind: ResolvedStyleProperty,
    ) {
        let Some(use_id) = use_id else {
            return;
        };
        if let Some(property) = self.uses[use_id.0]
            .properties
            .iter_mut()
            .find(|property| property.property == kind)
        {
            property.outcome = StateThemePropertyOutcome::SupersededBySource;
        }
    }

    fn consume_paint(
        &mut self,
        use_id: StateThemeUseId,
        property: &ResolvedProperty<CanvasPaint>,
        kind: ResolvedStyleProperty,
        supported: bool,
    ) {
        match property.specified() {
            Specified::Unspecified => {}
            Specified::Value(CanvasPaint::Transparent | CanvasPaint::Solid(_)) if supported => {}
            Specified::Clear
            | Specified::Value(
                CanvasPaint::Transparent
                | CanvasPaint::Solid(_)
                | CanvasPaint::LinearGradient(_)
                | CanvasPaint::RadialGradient(_)
                | CanvasPaint::Pattern(_),
            ) => self.reject_property(
                use_id,
                property,
                kind,
                FamilyThemeResidualReason::UnsupportedPaint,
            ),
        }
    }

    fn consume_shape_property<T>(
        &mut self,
        use_id: StateThemeUseId,
        property: &ResolvedProperty<T>,
        kind: ResolvedStyleProperty,
    ) {
        if matches!(property.specified(), Specified::Clear) {
            self.reject_property(
                use_id,
                property,
                kind,
                FamilyThemeResidualReason::UnsupportedGeometry,
            );
        }
    }

    fn consume_geometry(
        &mut self,
        use_id: StateThemeUseId,
        style: &ResolvedThemeStyle,
        support: StateGeometrySupport,
    ) {
        match style.radius_resolution().specified() {
            Specified::Unspecified | Specified::Value(_) if support.radius => {}
            Specified::Unspecified => {}
            Specified::Clear | Specified::Value(_) => self.reject_property(
                use_id,
                style.radius_resolution(),
                ResolvedStyleProperty::Radius,
                FamilyThemeResidualReason::UnsupportedGeometry,
            ),
        }
        match style.padding_resolution().specified() {
            Specified::Unspecified => {}
            Specified::Value(padding)
                if support.padding
                    && [padding.top, padding.right, padding.bottom, padding.left]
                        .iter()
                        .all(|value| (*value - padding.top).abs() <= f32::EPSILON) => {}
            Specified::Clear | Specified::Value(_) => self.reject_property(
                use_id,
                style.padding_resolution(),
                ResolvedStyleProperty::Padding,
                FamilyThemeResidualReason::UnsupportedGeometry,
            ),
        }
    }

    fn consume_simple_typography(&mut self, use_id: StateThemeUseId, style: &ResolvedThemeStyle) {
        let patch = style.typography_resolution().patch();
        if matches!(patch.font_stack, Specified::Clear) {
            self.reject_typography(use_id, style, ThemeTypographyProperty::FontStack);
        }
        if matches!(patch.font_size_px, Specified::Clear) {
            self.reject_typography(use_id, style, ThemeTypographyProperty::FontSize);
        }
        if matches!(patch.font_weight, Specified::Clear) {
            self.reject_typography(use_id, style, ThemeTypographyProperty::FontWeight);
        }
        if matches!(patch.font_style, Specified::Clear) {
            self.reject_typography(use_id, style, ThemeTypographyProperty::FontStyle);
        }
        if matches!(patch.letter_spacing_px, Specified::Clear)
            || (!self.prepared_text_available
                && matches!(patch.letter_spacing_px, Specified::Value(_)))
        {
            self.reject_typography(use_id, style, ThemeTypographyProperty::LetterSpacing);
        }
        if matches!(patch.word_spacing_px, Specified::Clear)
            || (!self.prepared_text_available
                && matches!(patch.word_spacing_px, Specified::Value(_)))
        {
            self.reject_typography(use_id, style, ThemeTypographyProperty::WordSpacing);
        }
    }

    fn reject_advanced_typography(&mut self, use_id: StateThemeUseId, style: &ResolvedThemeStyle) {
        let patch = style.typography_resolution().patch();
        let properties = [
            (
                ThemeTypographyProperty::LineHeight,
                !patch.line_height.is_unspecified(),
            ),
            (
                ThemeTypographyProperty::Transform,
                !patch.transform.is_unspecified(),
            ),
            (
                ThemeTypographyProperty::Decoration,
                !patch.decoration.is_unspecified(),
            ),
            (
                ThemeTypographyProperty::TextAlign,
                !patch.text_align.is_unspecified(),
            ),
            (
                ThemeTypographyProperty::WhiteSpace,
                !patch.white_space.is_unspecified(),
            ),
            (ThemeTypographyProperty::Wrap, !patch.wrap.is_unspecified()),
        ];
        for (property, present) in properties {
            if present {
                self.reject_typography(use_id, style, property);
            }
        }
    }

    fn reject_all_typography(&mut self, use_id: StateThemeUseId, style: &ResolvedThemeStyle) {
        for property in [
            ThemeTypographyProperty::FontStack,
            ThemeTypographyProperty::FontSize,
            ThemeTypographyProperty::FontWeight,
            ThemeTypographyProperty::FontStyle,
            ThemeTypographyProperty::LineHeight,
            ThemeTypographyProperty::LetterSpacing,
            ThemeTypographyProperty::WordSpacing,
            ThemeTypographyProperty::Transform,
            ThemeTypographyProperty::Decoration,
            ThemeTypographyProperty::TextAlign,
            ThemeTypographyProperty::WhiteSpace,
            ThemeTypographyProperty::Wrap,
        ] {
            self.reject_typography(use_id, style, property);
        }
    }

    fn reject_typography(
        &mut self,
        use_id: StateThemeUseId,
        style: &ResolvedThemeStyle,
        property: ThemeTypographyProperty,
    ) {
        if let Some(origin) = style.typography_resolution().winner(property) {
            self.set_property_outcome(
                use_id,
                origin.rule_index(),
                ResolvedStyleProperty::Typography(property),
                StateThemePropertyOutcome::Residual(
                    FamilyThemeResidualReason::UnsupportedTypography,
                ),
            );
        }
    }

    fn reject_property<T>(
        &mut self,
        use_id: StateThemeUseId,
        property: &ResolvedProperty<T>,
        kind: ResolvedStyleProperty,
        reason: FamilyThemeResidualReason,
    ) {
        if let Some(origin) = property.winner() {
            self.set_property_outcome(
                use_id,
                origin.rule_index(),
                kind,
                StateThemePropertyOutcome::Residual(reason),
            );
        }
    }

    fn set_property_outcome(
        &mut self,
        use_id: StateThemeUseId,
        rule_index: usize,
        kind: ResolvedStyleProperty,
        outcome: StateThemePropertyOutcome,
    ) {
        if let Some(property) = self.uses[use_id.0]
            .properties
            .iter_mut()
            .find(|property| property.rule_index == rule_index && property.property == kind)
        {
            property.outcome = outcome;
        }
    }
}

fn rule_capabilities(
    theme: &ResolvedDiagramTheme,
    rule: &ThemeRule,
    properties: &BTreeSet<ResolvedStyleProperty>,
) -> BTreeSet<ThemeCapability> {
    let mut capabilities = BTreeSet::new();
    let style = rule.style();
    for property in properties {
        match property {
            ResolvedStyleProperty::Fill => {
                if let Specified::Value(paint) = &style.paint.fill
                    && let Some(capability) = paint_capability(paint)
                {
                    capabilities.insert(capability);
                }
            }
            ResolvedStyleProperty::Stroke => {
                if let Specified::Value(paint) = &style.stroke.paint
                    && let Some(capability) = paint_capability(paint)
                {
                    capabilities.insert(capability);
                }
            }
            ResolvedStyleProperty::StrokeWidth
            | ResolvedStyleProperty::StrokeLinecap
            | ResolvedStyleProperty::StrokeLinejoin => {
                capabilities.insert(ThemeCapability::BorderStyling);
            }
            ResolvedStyleProperty::StrokeDasharray => {
                capabilities.insert(ThemeCapability::DashStyling);
            }
            ResolvedStyleProperty::Opacity
            | ResolvedStyleProperty::FillOpacity
            | ResolvedStyleProperty::StrokeOpacity => {
                capabilities.insert(ThemeCapability::Opacity);
            }
            ResolvedStyleProperty::Radius => {
                capabilities.insert(ThemeCapability::RoundedGeometry);
            }
            ResolvedStyleProperty::Padding => {
                capabilities.insert(ThemeCapability::ContentPadding);
            }
            ResolvedStyleProperty::Typography(property) => {
                capabilities.extend(typography_capabilities(&BTreeSet::from([*property])));
            }
            ResolvedStyleProperty::Effect => {
                if let Specified::Value(effect_id) = &style.effects.effect
                    && let Some(graph) = theme.effect_graph(effect_id)
                {
                    collect_effect_graph_capabilities(graph, &mut capabilities);
                }
            }
        }
    }
    capabilities
}

fn typography_capabilities(
    properties: &BTreeSet<ThemeTypographyProperty>,
) -> BTreeSet<ThemeCapability> {
    if properties.is_empty() {
        return BTreeSet::new();
    }
    let mut capabilities = BTreeSet::from([ThemeCapability::Typography]);
    for property in properties {
        match property {
            ThemeTypographyProperty::LetterSpacing => {
                capabilities.insert(ThemeCapability::LetterSpacing);
            }
            ThemeTypographyProperty::WordSpacing => {
                capabilities.insert(ThemeCapability::WordSpacing);
            }
            ThemeTypographyProperty::Transform => {
                capabilities.insert(ThemeCapability::TextTransform);
            }
            ThemeTypographyProperty::Decoration => {
                capabilities.insert(ThemeCapability::TextDecoration);
            }
            ThemeTypographyProperty::WhiteSpace | ThemeTypographyProperty::Wrap => {
                capabilities.insert(ThemeCapability::WhiteSpaceWrapping);
            }
            ThemeTypographyProperty::FontStack
            | ThemeTypographyProperty::FontSize
            | ThemeTypographyProperty::FontWeight
            | ThemeTypographyProperty::FontStyle
            | ThemeTypographyProperty::LineHeight
            | ThemeTypographyProperty::TextAlign => {}
        }
    }
    capabilities
}

fn source_style_theme_property(property: &str) -> Option<ResolvedStyleProperty> {
    Some(match property {
        "fill" | "color" => ResolvedStyleProperty::Fill,
        "stroke" => ResolvedStyleProperty::Stroke,
        "stroke-width" => ResolvedStyleProperty::StrokeWidth,
        "stroke-dasharray" => ResolvedStyleProperty::StrokeDasharray,
        "stroke-linecap" => ResolvedStyleProperty::StrokeLinecap,
        "stroke-linejoin" => ResolvedStyleProperty::StrokeLinejoin,
        "opacity" => ResolvedStyleProperty::Opacity,
        "fill-opacity" => ResolvedStyleProperty::FillOpacity,
        "stroke-opacity" => ResolvedStyleProperty::StrokeOpacity,
        "border-radius" | "rx" | "ry" => ResolvedStyleProperty::Radius,
        "padding" => ResolvedStyleProperty::Padding,
        "font-family" => ResolvedStyleProperty::Typography(ThemeTypographyProperty::FontStack),
        "font-size" => ResolvedStyleProperty::Typography(ThemeTypographyProperty::FontSize),
        "font-weight" => ResolvedStyleProperty::Typography(ThemeTypographyProperty::FontWeight),
        "font-style" => ResolvedStyleProperty::Typography(ThemeTypographyProperty::FontStyle),
        _ => return None,
    })
}

fn source_shape_property_reaches_primary_surface(
    node: &StateDiagramRenderNode,
    geometry: StateGeometrySupport,
    property: ResolvedStyleProperty,
) -> bool {
    if node.shape == "stateStart" {
        return false;
    }
    match property {
        ResolvedStyleProperty::Radius => geometry.radius,
        ResolvedStyleProperty::Padding => geometry.padding,
        ResolvedStyleProperty::Typography(_) | ResolvedStyleProperty::Effect => false,
        ResolvedStyleProperty::Fill
        | ResolvedStyleProperty::Stroke
        | ResolvedStyleProperty::StrokeWidth
        | ResolvedStyleProperty::StrokeDasharray
        | ResolvedStyleProperty::StrokeLinecap
        | ResolvedStyleProperty::StrokeLinejoin
        | ResolvedStyleProperty::Opacity
        | ResolvedStyleProperty::FillOpacity
        | ResolvedStyleProperty::StrokeOpacity => true,
    }
}

fn state_geometry_support(node: &StateDiagramRenderNode) -> StateGeometrySupport {
    match node.shape.as_str() {
        "note" => StateGeometrySupport::PADDING,
        "stateStart" | "stateEnd" | "choice" | "fork" | "join" | "noteGroup" => {
            StateGeometrySupport::NONE
        }
        // Rectangles, title rectangles, composite clusters, and divider clusters all consume
        // uniform padding during layout and emit a bounded corner radius when requested.
        _ => StateGeometrySupport::BOTH,
    }
}

fn source_shape_property_reaches_inner_surface(property: ResolvedStyleProperty) -> bool {
    matches!(
        property,
        ResolvedStyleProperty::Fill
            | ResolvedStyleProperty::Stroke
            | ResolvedStyleProperty::StrokeWidth
            | ResolvedStyleProperty::StrokeDasharray
            | ResolvedStyleProperty::StrokeLinecap
            | ResolvedStyleProperty::StrokeLinejoin
            | ResolvedStyleProperty::Opacity
            | ResolvedStyleProperty::FillOpacity
            | ResolvedStyleProperty::StrokeOpacity
    )
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
    diagram_look: &str,
    ordinal: Option<usize>,
    label_ordinal: Option<usize>,
    composite_header_ordinal: Option<usize>,
    special_state_inner_ordinal: Option<usize>,
    residuals: &mut Vec<SourceStyleResidual>,
    theme_evidence: &mut StateThemeEvidenceBuilder<'_>,
    effects: &mut StateEffectPlan,
    work_meter: &OperationWorkMeter,
) -> Result<StateNodeStylePlan, ResourceLimitExceeded> {
    let prepared_text_available = theme_evidence.prepared_text_available;
    let semantic_shape = resolve_theme_style(resolved_theme, target, variant, ordinal, work_meter)?;
    let semantic_label = resolve_theme_text_style(
        resolved_theme,
        label_target,
        variant,
        label_ordinal,
        work_meter,
    )?;
    let semantic_composite_header = (target == ThemeTarget::Composite)
        .then(|| {
            resolve_theme_style(
                resolved_theme,
                ThemeTarget::CompositeHeader,
                ThemeVariant::Default,
                composite_header_ordinal,
                work_meter,
            )
        })
        .transpose()?
        .flatten();
    let semantic_special_state_inner = (node.shape == "stateEnd")
        .then(|| {
            resolve_theme_style(
                resolved_theme,
                ThemeTarget::SpecialStateInner,
                variant,
                special_state_inner_ordinal,
                work_meter,
            )
        })
        .transpose()?
        .flatten();
    let geometry_support = state_geometry_support(node);
    let shape_use = ordinal
        .zip(semantic_shape.as_ref())
        .map(|(_, style)| theme_evidence.consume_shape_style(style, geometry_support));
    let effect = shape_use.and_then(|use_id| {
        let theme = resolved_theme?;
        let style = semantic_shape.as_ref()?;
        let resolved = theme.resolve_effect(target, style.effect_resolution());
        let surface_supported =
            target == ThemeTarget::State && node.shape == "rect" && diagram_look == "classic";
        theme_evidence.consume_effect(
            use_id,
            target,
            style.effect_resolution(),
            resolved,
            surface_supported,
            effects,
        )
    });
    let label_use = label_ordinal
        .zip(semantic_label.as_ref())
        .map(|(_, style)| theme_evidence.consume_text_style(style));
    let _composite_header_use = composite_header_ordinal
        .zip(semantic_composite_header.as_ref())
        .map(|(_, style)| theme_evidence.consume_shape_style(style, StateGeometrySupport::NONE));
    let special_state_inner_use = special_state_inner_ordinal
        .zip(semantic_special_state_inner.as_ref())
        .map(|(_, style)| theme_evidence.consume_shape_style(style, StateGeometrySupport::NONE));
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
    let mut shadowed_shape_properties = BTreeSet::new();
    let mut shadowed_label_properties = BTreeSet::new();
    let mut label_uses_series_color = false;
    let semantic_fill = semantic_shape
        .as_ref()
        .and_then(|style| shape_paint_value(style.fill_resolution()));
    let series_fill = ordinal.and_then(|ordinal| theme_evidence.series_color(target, ordinal));
    let mut fill_uses_series_color = semantic_fill.is_none() && series_fill.is_some();
    let mut fill_override = semantic_fill.or(series_fill);
    let mut stroke_override = semantic_shape
        .as_ref()
        .and_then(|style| shape_paint_value(style.stroke_resolution()));
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
        if let Some(theme) = resolved_theme {
            append_theme_base_text_emission(theme, &mut label_emission);
        }
        append_semantic_text_emission(style, &mut label_emission);
        let series_color =
            label_ordinal.and_then(|ordinal| theme_evidence.series_color(label_target, ordinal));
        let semantic_color = text_paint_value(style.fill_resolution());
        label_uses_series_color = semantic_color.is_none() && series_color.is_some();
        let color = semantic_color.or(series_color);
        if let Some(color) = color {
            insert_emitted(&mut label_emission, "color", color);
        }
    }

    let semantic_shape_style_attr = compact_style_attr(&shape_emission);
    let mut composite_header_emission = IndexMap::new();
    let mut composite_header_text_emission = IndexMap::new();
    if let Some(style) = semantic_composite_header.as_ref() {
        append_semantic_shape_emission(style, &mut composite_header_emission);
        if let Some(fill) = shape_paint_value(style.fill_resolution()) {
            insert_emitted(&mut composite_header_emission, "fill", fill);
        }
        if let Some(stroke) = shape_paint_value(style.stroke_resolution()) {
            insert_emitted(&mut composite_header_emission, "stroke", stroke);
        }
    }
    let special_state_inner_style_attr = semantic_special_state_inner
        .as_ref()
        .map(semantic_shape_style_attr_for_style)
        .unwrap_or_default();
    for declaration in &shape_declarations {
        if node.shape == "stateStart" {
            residuals.push(SourceStyleResidual::from_declaration(
                declaration,
                SourceStyleResidualReason::UnsupportedSurface,
            ));
            continue;
        }
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
            if let Some(kind) = source_style_theme_property(declaration.property())
                && shadowed_shape_properties.insert(kind)
            {
                if source_shape_property_reaches_primary_surface(node, geometry_support, kind) {
                    theme_evidence.shadow_source_property(shape_use, kind);
                }
                if node.shape == "stateEnd" && source_shape_property_reaches_inner_surface(kind) {
                    theme_evidence.shadow_source_property(special_state_inner_use, kind);
                }
            }
            if declaration.property() == "fill" {
                fill_uses_series_color = false;
            }
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

    let semantic_text_style = resolve_semantic_text_style(base_text_style, semantic_label.as_ref());
    let semantic_prepared_typography = prepared_text_available
        .then(|| {
            semantic_label
                .as_ref()
                .map(|style| style.typography().clone())
        })
        .flatten();
    if target == ThemeTarget::Composite {
        if let Some(style) = semantic_label.as_ref() {
            if let Some(theme) = resolved_theme {
                append_theme_base_text_emission(theme, &mut composite_header_text_emission);
            }
            append_semantic_text_emission(style, &mut composite_header_text_emission);
            let color = text_paint_value(style.fill_resolution());
            if let Some(color) = color {
                insert_emitted(&mut composite_header_text_emission, "color", color);
            }
        }
    }
    let mut cluster_label_typography = ResolvedLabelTypography::new(
        semantic_text_style.clone(),
        semantic_prepared_typography.clone(),
    );
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
                &mut cluster_label_typography,
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
    cluster_label_typography.canonicalize_emission(&mut composite_header_text_emission);
    let mut label_typography =
        ResolvedLabelTypography::new(semantic_text_style, semantic_prepared_typography);
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
        let accepted = apply_source_text_property(
            declaration,
            base_text_style,
            &mut label_typography,
            residuals,
        );
        if accepted {
            if let Some(kind) = source_style_theme_property(declaration.property())
                && shadowed_label_properties.insert(kind)
            {
                theme_evidence.shadow_source_property(label_use, kind);
            }
            if declaration.property() == "color" {
                label_uses_series_color = false;
            }
            label_emission.insert(
                declaration.property().to_string(),
                EmittedDeclaration::from_source(declaration),
            );
        }
    }
    label_typography.canonicalize_emission(&mut label_emission);

    if label_ordinal.is_some() {
        theme_evidence
            .observe_base_typography_use(semantic_label.as_ref(), &shadowed_label_properties);
    }

    if fill_uses_series_color {
        theme_evidence.record_series_color_emission(target);
    }
    if label_uses_series_color {
        theme_evidence.record_series_color_emission(label_target);
    }

    Ok(StateNodeStylePlan {
        #[cfg(test)]
        binding: StateNodeThemeBinding {
            target,
            label_target,
            variant,
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
        effect,
        cluster_label_typography,
        label_typography,
    })
}

fn prepare_edge(
    base_text_style: &TextStyle,
    resolved_theme: Option<&ResolvedDiagramTheme>,
    ordinal: Option<usize>,
    label_ordinal: Option<usize>,
    marker_ordinal: Option<usize>,
    html_labels: bool,
    theme_evidence: &mut StateThemeEvidenceBuilder<'_>,
    work_meter: &OperationWorkMeter,
) -> Result<StateEdgeStylePlan, ResourceLimitExceeded> {
    let prepared_text_available = theme_evidence.prepared_text_available;
    let semantic_path = resolve_theme_style(
        resolved_theme,
        ThemeTarget::Transition,
        ThemeVariant::Default,
        ordinal,
        work_meter,
    )?;
    let mut path_style_attr = semantic_shape_style_attr(semantic_path.as_ref());
    if ordinal.is_some()
        && let Some(style) = semantic_path.as_ref()
    {
        theme_evidence.consume_shape_style(style, StateGeometrySupport::NONE);
    }
    let series_stroke =
        ordinal.and_then(|ordinal| theme_evidence.series_color(ThemeTarget::Transition, ordinal));
    if !path_style_attr.contains("stroke:")
        && let Some(color) = series_stroke
    {
        append_style_declaration(&mut path_style_attr, "stroke", color);
        theme_evidence.record_series_color_emission(ThemeTarget::Transition);
    }
    let semantic_marker = resolve_theme_style(
        resolved_theme,
        ThemeTarget::TransitionMarker,
        ThemeVariant::Default,
        marker_ordinal,
        work_meter,
    )?;
    let marker_style_attr = semantic_shape_style_attr(semantic_marker.as_ref());
    if marker_ordinal.is_some()
        && let Some(style) = semantic_marker.as_ref()
    {
        theme_evidence.consume_shape_style(style, StateGeometrySupport::NONE);
    }
    let semantic_label_background = resolve_theme_style(
        resolved_theme,
        ThemeTarget::TransitionLabelBackground,
        ThemeVariant::Default,
        label_ordinal,
        work_meter,
    )?;
    let label_background_style_attr = semantic_shape_style_attr(semantic_label_background.as_ref());
    if label_ordinal.is_some()
        && let Some(style) = semantic_label_background.as_ref()
    {
        if html_labels {
            theme_evidence.consume_html_background_style(style);
        } else {
            theme_evidence.consume_shape_style(style, StateGeometrySupport::NONE);
        }
    }
    let label_background_div_style_prefix =
        semantic_html_background_style(semantic_label_background.as_ref());
    let semantic_label = resolve_theme_text_style(
        resolved_theme,
        ThemeTarget::TransitionLabel,
        ThemeVariant::Default,
        label_ordinal,
        work_meter,
    )?;
    let mut label_emission = IndexMap::<String, EmittedDeclaration>::new();
    if let Some((theme, style)) = resolved_theme.zip(semantic_label.as_ref()) {
        if label_ordinal.is_some() {
            theme_evidence.consume_text_style(style);
        }
        append_theme_base_text_emission(theme, &mut label_emission);
        append_semantic_text_emission(style, &mut label_emission);
        let series_color = label_ordinal
            .and_then(|ordinal| theme_evidence.series_color(ThemeTarget::TransitionLabel, ordinal));
        let semantic_color = text_paint_value(style.fill_resolution());
        let color_uses_series = semantic_color.is_none() && series_color.is_some();
        let color = semantic_color.or(series_color);
        if let Some(color) = color {
            insert_emitted(&mut label_emission, "color", color);
        }
        if color_uses_series {
            theme_evidence.record_series_color_emission(ThemeTarget::TransitionLabel);
        }
    }
    if label_ordinal.is_some() {
        theme_evidence.observe_base_typography_use(semantic_label.as_ref(), &BTreeSet::new());
    }
    let label_typography = ResolvedLabelTypography::new(
        resolve_semantic_text_style(base_text_style, semantic_label.as_ref()),
        prepared_text_available
            .then(|| {
                semantic_label
                    .as_ref()
                    .map(|style| style.typography().clone())
            })
            .flatten(),
    );
    label_typography.canonicalize_emission(&mut label_emission);

    Ok(StateEdgeStylePlan {
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
        label_typography,
    })
}

fn semantic_shape_style_attr(style: Option<&ResolvedThemeStyle>) -> String {
    let Some(style) = style else {
        return String::new();
    };
    semantic_shape_style_attr_for_style(style)
}

fn semantic_shape_style_attr_for_style(style: &ResolvedThemeStyle) -> String {
    let mut emission = IndexMap::<String, EmittedDeclaration>::new();
    append_semantic_shape_emission(style, &mut emission);

    if let Some(fill) = shape_paint_value(style.fill_resolution()) {
        insert_emitted(&mut emission, "fill", fill);
    }
    if let Some(stroke) = shape_paint_value(style.stroke_resolution()) {
        insert_emitted(&mut emission, "stroke", stroke);
    }
    compact_style_attr(&emission)
}

fn semantic_html_background_style(style: Option<&ResolvedThemeStyle>) -> String {
    let Some(style) = style else {
        return String::new();
    };
    match shape_paint_value(style.fill_resolution()).as_deref() {
        Some("none") => "background-color: transparent !important; ".to_string(),
        Some(value) => format!("background-color: {value} !important; "),
        None => String::new(),
    }
}

fn resolve_theme_style(
    theme: Option<&ResolvedDiagramTheme>,
    target: ThemeTarget,
    variant: ThemeVariant,
    ordinal: Option<usize>,
    work_meter: &OperationWorkMeter,
) -> Result<Option<ResolvedThemeStyle>, ResourceLimitExceeded> {
    theme
        .map(|theme| theme.style_with_work_meter(target, variant, ordinal, work_meter))
        .transpose()
}

fn resolve_theme_text_style(
    theme: Option<&ResolvedDiagramTheme>,
    target: ThemeTarget,
    variant: ThemeVariant,
    ordinal: Option<usize>,
    work_meter: &OperationWorkMeter,
) -> Result<Option<ResolvedThemeStyle>, ResourceLimitExceeded> {
    theme
        .map(|theme| theme.text_style_with_work_meter(target, variant, ordinal, work_meter))
        .transpose()
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
    resolved: Option<&ResolvedThemeStyle>,
) -> TextStyle {
    let mut style = base.clone();
    let Some(resolved) = resolved else {
        return style;
    };
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
    typography: &mut ResolvedLabelTypography,
    residuals: &mut Vec<SourceStyleResidual>,
) -> bool {
    let value = declaration.value().trim();
    match declaration.property() {
        "font-family" if is_safe_css_font_family_value(value) && !value.is_empty() => {
            if let Some(stack) = crate::text::parse_css_font_stack(value) {
                typography.apply_font_family(stack);
                true
            } else {
                residuals.push(SourceStyleResidual::from_declaration(
                    declaration,
                    SourceStyleResidualReason::InvalidValue,
                ));
                false
            }
        }
        "font-size" => match declaration
            .resolve_font_size_px(CssFontSizeContext::uniform(inherited.font_size))
        {
            Some(value) if typography.apply_font_size(value) => true,
            None => {
                residuals.push(SourceStyleResidual::from_declaration(
                    declaration,
                    SourceStyleResidualReason::InvalidValue,
                ));
                false
            }
            Some(_) => {
                residuals.push(SourceStyleResidual::from_declaration(
                    declaration,
                    SourceStyleResidualReason::InvalidValue,
                ));
                false
            }
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

fn shape_paint_value(property: &ResolvedProperty<CanvasPaint>) -> Option<String> {
    match property.value()? {
        CanvasPaint::Transparent => Some("none".to_string()),
        CanvasPaint::Solid(color) => Some(color.as_css()),
        CanvasPaint::LinearGradient(_)
        | CanvasPaint::RadialGradient(_)
        | CanvasPaint::Pattern(_) => None,
    }
}

fn text_paint_value(property: &ResolvedProperty<CanvasPaint>) -> Option<String> {
    match property.value()? {
        CanvasPaint::Transparent => Some("transparent".to_string()),
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
    if let Specified::Value(letter_spacing_px) = patch.letter_spacing_px {
        insert_emitted(
            out,
            "letter-spacing",
            format!("{}px", f64::from(letter_spacing_px)),
        );
    }
    if let Specified::Value(word_spacing_px) = patch.word_spacing_px {
        insert_emitted(
            out,
            "word-spacing",
            format!("{}px", f64::from(word_spacing_px)),
        );
    }
}

fn append_theme_base_text_emission(
    theme: &ResolvedDiagramTheme,
    out: &mut IndexMap<String, EmittedDeclaration>,
) {
    let style = theme.typography();
    if style == &ThemeTextStyle::default() {
        return;
    }
    insert_emitted(out, "font-family", style.font_stack().as_css());
    insert_emitted(
        out,
        "font-size",
        format!("{}px", f64::from(style.font_size_px())),
    );
    insert_emitted(out, "font-weight", style.font_weight().to_string());
    insert_emitted(out, "font-style", style.font_style().id());
    if style.letter_spacing_px().abs() > f32::EPSILON {
        insert_emitted(
            out,
            "letter-spacing",
            format!("{}px", f64::from(style.letter_spacing_px())),
        );
    }
    if style.word_spacing_px().abs() > f32::EPSILON {
        insert_emitted(
            out,
            "word-spacing",
            format!("{}px", f64::from(style.word_spacing_px())),
        );
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
    use crate::diagram_theme::{
        DiagramEffectSet, DiagramThemeCompiler, DiagramThemeSpec, EffectBinding, EffectGraph,
        EffectInput, EffectPrimitive, FilterRegion, GradientStop, InsetsPx, LinearGradient,
        OrdinalPalette, OrdinalSelector, TextStylePatch, ThemeColorValue, ThemeEffectPatch,
        ThemeGeometryPatch, ThemeRule, ThemeRuleSet, ThemeStylePatch, TypographySpec,
    };
    use merman_core::diagrams::state::{
        StateDiagramRenderEdge, StateDiagramRenderNode, StateDiagramRenderStyleClass,
    };
    use serde_json::json;

    fn test_work_meter() -> OperationWorkMeter {
        OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input())
    }

    fn resolve_theme_plan(
        model: &StateDiagramRenderModel,
        config: &serde_json::Value,
        theme: &ResolvedDiagramTheme,
    ) -> StateStylePlan {
        resolve_theme_plan_with_evidence(model, config, theme, None).0
    }

    fn resolve_theme_plan_with_evidence(
        model: &StateDiagramRenderModel,
        config: &serde_json::Value,
        theme: &ResolvedDiagramTheme,
        title: Option<&str>,
    ) -> (StateStylePlan, FamilyThemeEvidence) {
        StateStylePlan::resolve_with_evidence(
            model,
            config,
            Some(theme),
            title,
            true,
            &test_work_meter(),
        )
        .expect("resolve State theme plan")
    }

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

    fn hard_shadow_graph(id: &str, blur_radius: f32) -> EffectGraph {
        EffectGraph::new(
            id,
            FilterRegion::bounded(-0.2, -0.2, 1.4, 1.4),
            [EffectPrimitive::DropShadow {
                input: EffectInput::SourceGraphic,
                offset_x: 5.0,
                offset_y: 6.0,
                blur_radius,
                spread: 0.0,
                color: ThemeColorValue::parse("#111827").expect("valid shadow color"),
            }],
        )
        .expect("valid shadow graph")
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

        let plan = StateStylePlan::resolve_unthemed(&model, &json!({}));
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
    fn source_typography_is_one_decision_for_prepared_text_and_svg_emission() {
        let typography = ThemeTextStyle::default()
            .with_font_weight(500)
            .expect("fixture font weight should be valid");
        let resolved = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new()
                    .with_typography(TypographySpec::default().with_default(typography)),
            )
            .expect("fixture theme should compile")
            .resolve(crate::render_family::RenderFamilyKind::State);
        let mut model = StateDiagramRenderModel::default();
        model.style_classes.insert(
            "source-type".to_string(),
            StateDiagramRenderStyleClass {
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

        let plan = resolve_theme_plan(&model, &json!({}), &resolved);
        let class_only = plan.node("ClassOnly").expect("class-only node style");
        let class_request = crate::text::PrepareTextRequest::new(
            "ClassOnly",
            class_only
                .resolved_label_typography()
                .prepared_typography()
                .expect("structured typography")
                .clone(),
        );
        assert_eq!(
            class_request.typography().font_stack().families(),
            &["Class Sans".to_string(), "serif".to_string()]
        );
        assert_eq!(class_request.typography().font_size_px(), 18.0);
        assert_eq!(class_request.typography().font_weight(), 700);
        assert_eq!(class_request.typography().font_style(), FontStyle::Italic);
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
        let overridden_request = crate::text::PrepareTextRequest::new(
            "Overridden",
            overridden
                .resolved_label_typography()
                .prepared_typography()
                .expect("structured typography")
                .clone(),
        );
        assert_eq!(
            overridden_request.typography().font_stack().families(),
            &["Final Sans".to_string(), "monospace".to_string()]
        );
        assert_eq!(overridden_request.typography().font_size_px(), 21.0);
        assert_eq!(overridden_request.typography().font_weight(), 100);
        assert_eq!(
            overridden_request.typography().font_style(),
            FontStyle::Oblique
        );
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
    fn source_font_stack_rejects_unquoted_css_wide_keywords_in_any_position() {
        assert!(crate::text::parse_css_font_stack("Excalifont, inherit").is_none());
        assert!(crate::text::parse_css_font_stack("initial, Excalifont").is_none());
        assert!(crate::text::parse_css_font_stack("Family unset, serif").is_none());

        let quoted = crate::text::parse_css_font_stack("\"inherit\", Excalifont")
            .expect("quoted CSS-wide text remains a valid family name");
        assert_eq!(
            quoted.families(),
            &["inherit".to_string(), "Excalifont".to_string()]
        );
        assert_eq!(quoted.as_css(), "\"inherit\", \"Excalifont\"");
    }

    #[test]
    fn source_font_size_that_cannot_enter_typed_typography_is_a_residual() {
        let resolved =
            DiagramThemeCompiler::new()
                .compile(DiagramThemeSpec::new().with_typography(
                    TypographySpec::default().with_default(ThemeTextStyle::default()),
                ))
                .expect("fixture theme should compile")
                .resolve(crate::render_family::RenderFamilyKind::State);
        let mut model = StateDiagramRenderModel::default();
        let mut node = semantic_node("Overflow", "rect");
        node.css_styles = vec!["font-size:1e100px".to_string()];
        model.nodes.push(node);

        let plan = resolve_theme_plan(&model, &json!({}), &resolved);
        let node = plan.node("Overflow").expect("prepared node style");

        assert_ne!(node.text_style().font_size, 1e100);
        assert!(!node.label_style_attr().contains("1e100"));
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

        let plan = StateStylePlan::resolve_unthemed(&model, &json!({}));
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
        let plan = resolve_theme_plan(&model, &json!({}), &resolved);

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

    #[test]
    fn note_group_does_not_prove_a_note_rule() {
        let mut model = StateDiagramRenderModel::default();
        let mut note_group = semantic_node("note-group", "noteGroup");
        note_group.is_group = true;
        note_group.node_type = Some("group".to_string());
        model.nodes.push(note_group);
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(
                        ThemeTarget::Note,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#fef3c7").unwrap()),
                    )),
                ),
            )
            .unwrap();
        let resolved = theme.resolve(crate::render_family::RenderFamilyKind::State);
        let (_, evidence) = resolve_theme_plan_with_evidence(&model, &json!({}), &resolved, None);
        let key = FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Note,
        };

        assert!(evidence.not_applicable_mechanisms().contains(&key));
        assert!(!evidence.applied().contains(&key));
        assert!(evidence.residuals().is_empty());
    }

    #[test]
    fn special_state_inner_is_proved_only_by_an_end_state() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::SpecialStateInner,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#ffffff").unwrap()),
                        )
                        .with_variant(ThemeVariant::End),
                    ),
                ),
            )
            .unwrap();
        let resolved = theme.resolve(crate::render_family::RenderFamilyKind::State);
        let key = FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::SpecialStateInner,
        };

        let mut choice_model = StateDiagramRenderModel::default();
        choice_model.nodes.push(semantic_node("choice", "choice"));
        let (_, choice_evidence) =
            resolve_theme_plan_with_evidence(&choice_model, &json!({}), &resolved, None);
        assert!(choice_evidence.not_applicable_mechanisms().contains(&key));

        let mut end_model = StateDiagramRenderModel::default();
        end_model.nodes.push(semantic_node("end", "stateEnd"));
        let (end_plan, end_evidence) =
            resolve_theme_plan_with_evidence(&end_model, &json!({}), &resolved, None);
        assert!(end_evidence.applied().contains(&key));
        assert!(
            end_plan
                .node("end")
                .unwrap()
                .special_state_inner_style_attr()
                .contains("fill:#ffffff !important")
        );
    }

    #[test]
    fn later_typed_winner_does_not_credit_the_shadowed_rule() {
        let gradient = LinearGradient::new(
            90.0,
            [
                GradientStop::new(0.0, ThemeColorValue::parse("#0f172a").unwrap()).unwrap(),
                GradientStop::new(1.0, ThemeColorValue::parse("#22d3ee").unwrap()).unwrap(),
            ],
        )
        .unwrap();
        let rules = ThemeRuleSet::default()
            .with_rule(ThemeRule::new(
                ThemeTarget::State,
                ThemeStylePatch::default().with_fill(CanvasPaint::LinearGradient(gradient)),
            ))
            .with_rule(ThemeRule::new(
                ThemeTarget::State,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#22c55e").unwrap()),
            ));
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(rules))
            .unwrap();
        let resolved = theme.resolve(crate::render_family::RenderFamilyKind::State);
        let mut model = StateDiagramRenderModel::default();
        model.nodes.push(semantic_node("Ready", "rect"));
        let (plan, evidence) =
            resolve_theme_plan_with_evidence(&model, &json!({}), &resolved, None);

        assert_eq!(plan.node("Ready").unwrap().fill_override(), Some("#22c55e"));
        assert!(evidence.residuals().is_empty());
        assert!(evidence.applied().contains(&FamilyThemeMechanismKey::Rule {
            index: 1,
            target: ThemeTarget::State,
        }));
        let shadowed = FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::State,
        };
        assert!(!evidence.applied().contains(&shadowed));
        assert!(evidence.not_applicable_mechanisms().contains(&shadowed));
    }

    #[test]
    fn generic_text_rule_is_consumed_by_state_labels() {
        let mut text_patch = TextStylePatch::default();
        text_patch.font_size_px = Specified::Value(23.0);
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Text,
                        ThemeStylePatch {
                            paint: crate::diagram_theme::ThemePaintPatch {
                                fill: Specified::Value(CanvasPaint::solid("#e11d48").unwrap()),
                                ..crate::diagram_theme::ThemePaintPatch::default()
                            },
                            typography: text_patch,
                            ..ThemeStylePatch::default()
                        },
                    ),
                )),
            )
            .unwrap();
        let resolved = theme.resolve(crate::render_family::RenderFamilyKind::State);
        let mut model = StateDiagramRenderModel::default();
        model.nodes.push(semantic_node("Ready", "rect"));
        let (plan, evidence) =
            resolve_theme_plan_with_evidence(&model, &json!({}), &resolved, None);
        let node = plan.node("Ready").unwrap();

        assert_eq!(node.text_style().font_size, 23.0);
        assert!(node.label_style_attr().contains("color:#e11d48 !important"));
        assert!(evidence.applied().contains(&FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Text,
        }));
        assert!(evidence.residuals().is_empty());
    }

    #[test]
    fn state_node_charges_each_ordinal_semantic_role_once() {
        let mut generic_text = TextStylePatch::default();
        generic_text.font_size_px = Specified::Value(19.0);
        let mut state_label = TextStylePatch::default();
        state_label.font_size_px = Specified::Value(23.0);
        let ordinal = OrdinalSelector::exact(1).unwrap();
        let rules = ThemeRuleSet::default()
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::State,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                )
                .with_ordinal(ordinal.clone()),
            )
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::State,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#22c55e").unwrap()),
                )
                .with_ordinal(ordinal.clone()),
            )
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::Text,
                    ThemeStylePatch {
                        typography: generic_text,
                        ..ThemeStylePatch::default()
                    },
                )
                .with_ordinal(ordinal.clone()),
            )
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::StateLabel,
                    ThemeStylePatch {
                        typography: state_label,
                        ..ThemeStylePatch::default()
                    },
                )
                .with_ordinal(ordinal),
            );
        let resolved = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(rules))
            .unwrap()
            .resolve(crate::render_family::RenderFamilyKind::State);
        let mut model = StateDiagramRenderModel::default();
        model.nodes.push(semantic_node("Ready", "rect"));
        let work_meter = test_work_meter();

        let (plan, evidence) = StateStylePlan::resolve_with_evidence(
            &model,
            &json!({}),
            Some(&resolved),
            None,
            true,
            &work_meter,
        )
        .expect("resolve metered State theme plan");

        let node = plan.node("Ready").unwrap();
        assert_eq!(node.fill_override(), Some("#22c55e"));
        assert_eq!(node.text_style().font_size, 23.0);
        assert_eq!(work_meter.used(), 4);
        assert!(evidence.applied().contains(&FamilyThemeMechanismKey::Rule {
            index: 1,
            target: ThemeTarget::State,
        }));
        assert!(evidence.applied().contains(&FamilyThemeMechanismKey::Rule {
            index: 3,
            target: ThemeTarget::StateLabel,
        }));
        let shadowed_state_rule = FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::State,
        };
        assert!(!evidence.applied().contains(&shadowed_state_rule));
        assert!(
            evidence
                .not_applicable_mechanisms()
                .contains(&shadowed_state_rule)
        );
        let shadowed_text_rule = FamilyThemeMechanismKey::Rule {
            index: 2,
            target: ThemeTarget::Text,
        };
        assert!(!evidence.applied().contains(&shadowed_text_rule));
        assert!(
            evidence
                .not_applicable_mechanisms()
                .contains(&shadowed_text_rule)
        );
    }

    #[test]
    fn state_edge_charges_each_ordinal_semantic_role_once() {
        let mut label_text = TextStylePatch::default();
        label_text.font_size_px = Specified::Value(27.0);
        let ordinal = OrdinalSelector::exact(1).unwrap();
        let rules = ThemeRuleSet::default()
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::Transition,
                    ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#22d3ee").unwrap()),
                )
                .with_ordinal(ordinal.clone()),
            )
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::TransitionMarker,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#f472b6").unwrap()),
                )
                .with_ordinal(ordinal.clone()),
            )
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::TransitionLabelBackground,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#0f172a").unwrap()),
                )
                .with_ordinal(ordinal.clone()),
            )
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::TransitionLabel,
                    ThemeStylePatch {
                        typography: label_text,
                        ..ThemeStylePatch::default()
                    },
                )
                .with_ordinal(ordinal),
            );
        let resolved = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(rules))
            .unwrap()
            .resolve(crate::render_family::RenderFamilyKind::State);
        let mut model = StateDiagramRenderModel::default();
        model.edges.push(StateDiagramRenderEdge {
            id: "transition-1".to_string(),
            start: "A".to_string(),
            end: "B".to_string(),
            classes: "transition".to_string(),
            arrow_type_end: "arrow_barb".to_string(),
            label: "go".to_string(),
        });
        let work_meter = test_work_meter();

        let (plan, evidence) = StateStylePlan::resolve_with_evidence(
            &model,
            &json!({}),
            Some(&resolved),
            None,
            true,
            &work_meter,
        )
        .expect("resolve metered State edge theme plan");

        let edge = plan.edge("transition-1").unwrap();
        assert!(edge.path_style_attr().contains("stroke:#22d3ee"));
        assert!(edge.marker_style_attr().contains("fill:#f472b6"));
        assert!(edge.label_background_style_attr().contains("fill:#0f172a"));
        assert_eq!(edge.text_style().font_size, 27.0);
        assert_eq!(work_meter.used(), 4);
        for (index, target) in [
            (0, ThemeTarget::Transition),
            (1, ThemeTarget::TransitionMarker),
            (2, ThemeTarget::TransitionLabelBackground),
            (3, ThemeTarget::TransitionLabel),
        ] {
            assert!(
                evidence
                    .applied()
                    .contains(&FamilyThemeMechanismKey::Rule { index, target })
            );
        }
    }

    #[test]
    fn explicit_state_fill_does_not_credit_an_unemitted_ordinal_palette() {
        let palette = OrdinalPalette::new([ThemeColorValue::parse("#22d3ee").unwrap()]).unwrap();
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default()
                        .with_ordinal_palette(ThemeTarget::State, palette)
                        .with_rule(ThemeRule::new(
                            ThemeTarget::State,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                        )),
                ),
            )
            .unwrap();
        let resolved = theme.resolve(crate::render_family::RenderFamilyKind::State);
        let mut model = StateDiagramRenderModel::default();
        model.nodes.push(semantic_node("Ready", "rect"));
        let (plan, evidence) =
            resolve_theme_plan_with_evidence(&model, &json!({}), &resolved, None);
        let key = FamilyThemeMechanismKey::OrdinalPalette {
            target: ThemeTarget::State,
        };

        assert_eq!(plan.node("Ready").unwrap().fill_override(), Some("#ef4444"));
        assert!(!evidence.applied().contains(&key));
        assert!(evidence.not_applicable_mechanisms().contains(&key));
        assert!(evidence.residuals().iter().all(|item| item.key() != &key));
    }

    #[test]
    fn note_padding_is_applied_but_note_radius_is_residual() {
        let radius_patch = ThemeStylePatch {
            geometry: ThemeGeometryPatch {
                radius: Specified::Value(12.0),
            },
            ..ThemeStylePatch::default()
        };
        let rules = ThemeRuleSet::default()
            .with_rule(ThemeRule::new(ThemeTarget::Note, radius_patch))
            .with_rule(ThemeRule::new(
                ThemeTarget::Note,
                ThemeStylePatch::default().with_padding(InsetsPx::all(14.0)),
            ));
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(rules))
            .unwrap();
        let resolved = theme.resolve(crate::render_family::RenderFamilyKind::State);
        let mut model = StateDiagramRenderModel::default();
        model.nodes.push(semantic_node("N", "note"));
        let (plan, evidence) =
            resolve_theme_plan_with_evidence(&model, &json!({}), &resolved, None);
        let radius_key = FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Note,
        };
        let padding_key = FamilyThemeMechanismKey::Rule {
            index: 1,
            target: ThemeTarget::Note,
        };

        assert_eq!(plan.node("N").unwrap().padding_override(), Some(14.0));
        assert!(evidence.applied().contains(&padding_key));
        assert!(evidence.residuals().iter().any(|residual| {
            residual.key() == &radius_key
                && residual.reason() == FamilyThemeResidualReason::UnsupportedGeometry
        }));
    }

    #[test]
    fn transparent_state_label_emits_valid_css_color() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::StateLabel,
                        ThemeStylePatch::default().with_fill(CanvasPaint::Transparent),
                    ),
                )),
            )
            .unwrap();
        let resolved = theme.resolve(crate::render_family::RenderFamilyKind::State);
        let mut model = StateDiagramRenderModel::default();
        model.nodes.push(semantic_node("Ready", "rect"));
        let (plan, evidence) =
            resolve_theme_plan_with_evidence(&model, &json!({}), &resolved, None);

        let style = plan.node("Ready").unwrap().label_style_attr();
        assert!(style.contains("color:transparent !important"));
        assert!(!style.contains("color:none"));
        assert!(evidence.applied().contains(&FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::StateLabel,
        }));
    }

    #[test]
    fn accepted_source_fill_shadows_only_the_typed_fill_winner() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(
                        ThemeTarget::State,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                    )),
                ),
            )
            .unwrap();
        let resolved = theme.resolve(crate::render_family::RenderFamilyKind::State);
        let mut model = StateDiagramRenderModel::default();
        let mut node = semantic_node("Ready", "rect");
        node.css_styles = vec!["fill:#111827".to_string()];
        model.nodes.push(node);

        let (plan, evidence) =
            resolve_theme_plan_with_evidence(&model, &json!({}), &resolved, None);
        let key = FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::State,
        };

        assert_eq!(plan.node("Ready").unwrap().fill_override(), Some("#111827"));
        assert!(!evidence.applied().contains(&key));
        assert!(evidence.not_applicable_mechanisms().contains(&key));
        assert!(evidence.residuals().iter().all(|item| item.key() != &key));
    }

    #[test]
    fn accepted_source_fill_preserves_another_typed_property_from_the_same_rule() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::State,
                        ThemeStylePatch {
                            paint: crate::diagram_theme::ThemePaintPatch {
                                fill: Specified::Value(CanvasPaint::solid("#ef4444").unwrap()),
                                ..Default::default()
                            },
                            geometry: ThemeGeometryPatch {
                                radius: Specified::Value(12.0),
                            },
                            ..ThemeStylePatch::default()
                        },
                    ),
                )),
            )
            .unwrap();
        let resolved = theme.resolve(crate::render_family::RenderFamilyKind::State);
        let mut model = StateDiagramRenderModel::default();
        let mut node = semantic_node("Ready", "rect");
        node.css_styles = vec!["fill:#111827".to_string()];
        model.nodes.push(node);

        let (plan, evidence) =
            resolve_theme_plan_with_evidence(&model, &json!({}), &resolved, None);
        let key = FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::State,
        };

        assert_eq!(plan.node("Ready").unwrap().fill_override(), Some("#111827"));
        assert_eq!(plan.node("Ready").unwrap().radius_override(), Some(12.0));
        assert!(evidence.applied().contains(&key));
        assert!(!evidence.not_applicable_mechanisms().contains(&key));
        assert!(evidence.residuals().iter().all(|item| item.key() != &key));
    }

    #[test]
    fn source_shadow_is_scoped_to_one_rendered_node() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(
                        ThemeTarget::State,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                    )),
                ),
            )
            .unwrap();
        let resolved = theme.resolve(crate::render_family::RenderFamilyKind::State);
        let mut model = StateDiagramRenderModel::default();
        let mut overridden = semantic_node("Overridden", "rect");
        overridden.css_styles = vec!["fill:#111827".to_string(), "fill:#0f172a".to_string()];
        model.nodes.push(overridden);
        model.nodes.push(semantic_node("Untouched", "rect"));

        let (plan, evidence) =
            resolve_theme_plan_with_evidence(&model, &json!({}), &resolved, None);
        let key = FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::State,
        };

        assert_eq!(
            plan.node("Overridden").unwrap().fill_override(),
            Some("#0f172a")
        );
        assert_eq!(
            plan.node("Untouched").unwrap().fill_override(),
            Some("#ef4444")
        );
        assert!(evidence.applied().contains(&key));
        assert!(!evidence.not_applicable_mechanisms().contains(&key));
    }

    #[test]
    fn source_shadow_does_not_erase_a_residual_from_another_rendered_node() {
        let gradient = LinearGradient::new(
            90.0,
            [
                GradientStop::new(0.0, ThemeColorValue::parse("#0f172a").unwrap()).unwrap(),
                GradientStop::new(1.0, ThemeColorValue::parse("#22d3ee").unwrap()).unwrap(),
            ],
        )
        .unwrap();
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::State,
                        ThemeStylePatch::default().with_fill(CanvasPaint::LinearGradient(gradient)),
                    ),
                )),
            )
            .unwrap();
        let resolved = theme.resolve(crate::render_family::RenderFamilyKind::State);
        let mut model = StateDiagramRenderModel::default();
        let mut overridden = semantic_node("Overridden", "rect");
        overridden.css_styles = vec!["fill:#111827".to_string()];
        model.nodes.push(overridden);
        model.nodes.push(semantic_node("Unsupported", "rect"));

        let (_, evidence) = resolve_theme_plan_with_evidence(&model, &json!({}), &resolved, None);
        let key = FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::State,
        };

        assert!(evidence.residuals().iter().any(|residual| {
            residual.key() == &key
                && residual.reason() == FamilyThemeResidualReason::UnsupportedPaint
        }));
        assert!(!evidence.applied().contains(&key));
        assert!(!evidence.not_applicable_mechanisms().contains(&key));
    }

    #[test]
    fn end_state_source_fill_supersedes_the_inner_surface_use() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(
                        ThemeTarget::SpecialStateInner,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#ffffff").unwrap()),
                    )),
                ),
            )
            .unwrap();
        let resolved = theme.resolve(crate::render_family::RenderFamilyKind::State);
        let mut model = StateDiagramRenderModel::default();
        let mut end = semantic_node("end", "stateEnd");
        end.css_styles = vec!["fill:#111827".to_string()];
        model.nodes.push(end);

        let (plan, evidence) =
            resolve_theme_plan_with_evidence(&model, &json!({}), &resolved, None);
        let key = FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::SpecialStateInner,
        };

        assert!(
            plan.node("end")
                .unwrap()
                .special_state_inner_style_attr()
                .contains("fill:#ffffff !important")
        );
        assert!(
            plan.node("end")
                .unwrap()
                .source_shape_style_attr()
                .contains("fill:#111827 !important")
        );
        assert!(!evidence.applied().contains(&key));
        assert!(evidence.not_applicable_mechanisms().contains(&key));
        assert!(
            evidence
                .residuals()
                .iter()
                .all(|residual| residual.key() != &key)
        );
    }

    #[test]
    fn start_state_source_fill_cannot_shadow_the_emitted_typed_style() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(
                        ThemeTarget::SpecialState,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#22c55e").unwrap()),
                    )),
                ),
            )
            .unwrap();
        let resolved = theme.resolve(crate::render_family::RenderFamilyKind::State);
        let mut model = StateDiagramRenderModel::default();
        let mut start = semantic_node("start", "stateStart");
        start.css_styles = vec!["fill:#111827".to_string()];
        model.nodes.push(start);

        let (plan, evidence) =
            resolve_theme_plan_with_evidence(&model, &json!({}), &resolved, None);
        let key = FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::SpecialState,
        };
        let node = plan.node("start").unwrap();

        assert!(
            node.semantic_shape_style_attr()
                .contains("fill:#22c55e !important")
        );
        assert!(node.source_shape_style_attr().is_empty());
        assert!(evidence.applied().contains(&key));
        assert!(plan.residuals().iter().any(|residual| {
            residual.property() == Some("fill")
                && residual.reason() == SourceStyleResidualReason::UnsupportedSurface
        }));
    }

    #[test]
    fn composite_source_fill_shadows_only_the_body_surface() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default()
                        .with_rule(ThemeRule::new(
                            ThemeTarget::Composite,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#22c55e").unwrap()),
                        ))
                        .with_rule(ThemeRule::new(
                            ThemeTarget::CompositeHeader,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#ffffff").unwrap()),
                        )),
                ),
            )
            .unwrap();
        let resolved = theme.resolve(crate::render_family::RenderFamilyKind::State);
        let mut model = StateDiagramRenderModel::default();
        model.style_classes.insert(
            "surface".to_string(),
            StateDiagramRenderStyleClass {
                id: "surface".to_string(),
                styles: vec!["fill:#111827".to_string()],
                text_styles: Vec::new(),
            },
        );
        let mut composite = semantic_node("cluster", "roundedWithTitle");
        composite.is_group = true;
        composite.node_type = Some("group".to_string());
        composite.css_classes = "surface".to_string();
        model.nodes.push(composite);

        let (plan, evidence) =
            resolve_theme_plan_with_evidence(&model, &json!({}), &resolved, None);
        let body_key = FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Composite,
        };
        let header_key = FamilyThemeMechanismKey::Rule {
            index: 1,
            target: ThemeTarget::CompositeHeader,
        };
        let node = plan.node("cluster").unwrap();

        assert_eq!(node.fill_override(), Some("#111827"));
        assert!(
            node.composite_header_style_attr()
                .contains("fill:#ffffff !important")
        );
        assert!(!evidence.applied().contains(&body_key));
        assert!(evidence.not_applicable_mechanisms().contains(&body_key));
        assert!(evidence.applied().contains(&header_key));
    }

    #[test]
    fn invalid_source_fill_does_not_supersede_the_typed_winner() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(
                        ThemeTarget::State,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#22c55e").unwrap()),
                    )),
                ),
            )
            .unwrap();
        let resolved = theme.resolve(crate::render_family::RenderFamilyKind::State);
        let mut model = StateDiagramRenderModel::default();
        let mut node = semantic_node("Ready", "rect");
        node.css_styles = vec!["fill:not-a-color".to_string()];
        model.nodes.push(node);

        let (plan, evidence) =
            resolve_theme_plan_with_evidence(&model, &json!({}), &resolved, None);
        let key = FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::State,
        };

        assert_eq!(plan.node("Ready").unwrap().fill_override(), Some("#22c55e"));
        assert!(evidence.applied().contains(&key));
        assert!(plan.residuals().iter().any(|residual| {
            residual.property() == Some("fill")
                && residual.reason() == SourceStyleResidualReason::InvalidValue
        }));
    }

    #[test]
    fn html_transition_background_rejects_unemitted_stroke() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(
                        ThemeTarget::TransitionLabelBackground,
                        ThemeStylePatch::default()
                            .with_stroke(CanvasPaint::solid("#ef4444").unwrap()),
                    )),
                ),
            )
            .unwrap();
        let resolved = theme.resolve(crate::render_family::RenderFamilyKind::State);
        let mut model = StateDiagramRenderModel::default();
        model.edges.push(StateDiagramRenderEdge {
            id: "transition-1".to_string(),
            start: "A".to_string(),
            end: "B".to_string(),
            classes: "transition".to_string(),
            arrow_type_end: "arrow_barb".to_string(),
            label: "go".to_string(),
        });
        let (_, evidence) = resolve_theme_plan_with_evidence(
            &model,
            &json!({ "htmlLabels": true }),
            &resolved,
            None,
        );
        let key = FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::TransitionLabelBackground,
        };

        assert!(evidence.residuals().iter().any(|residual| {
            residual.key() == &key
                && residual.reason() == FamilyThemeResidualReason::UnsupportedPaint
        }));
        assert!(!evidence.applied().contains(&key));
    }

    #[test]
    fn shadowed_self_loop_is_removed_before_transition_ordinals_are_assigned() {
        let rule = ThemeRule::new(
            ThemeTarget::Transition,
            ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#22d3ee").unwrap()),
        )
        .with_ordinal(OrdinalSelector::exact(1).unwrap());
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule)))
            .unwrap();
        let resolved = theme.resolve(crate::render_family::RenderFamilyKind::State);
        let mut model = StateDiagramRenderModel::default();
        model.edges.extend([
            StateDiagramRenderEdge {
                id: "loop-early".to_string(),
                start: "Ready".to_string(),
                end: "Ready".to_string(),
                classes: "transition".to_string(),
                arrow_type_end: "arrow_barb".to_string(),
                label: String::new(),
            },
            StateDiagramRenderEdge {
                id: "loop-late".to_string(),
                start: "Ready".to_string(),
                end: "Ready".to_string(),
                classes: "transition".to_string(),
                arrow_type_end: "arrow_barb".to_string(),
                label: String::new(),
            },
        ]);
        let (plan, evidence) =
            resolve_theme_plan_with_evidence(&model, &json!({}), &resolved, None);
        let key = FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Transition,
        };

        assert_eq!(plan.edge("loop-early").unwrap().ordinal(), None);
        assert_eq!(plan.edge("loop-late").unwrap().ordinal(), Some(1));
        assert!(evidence.applied().contains(&key));
        assert!(evidence.residuals().is_empty());
    }

    #[test]
    fn base_font_size_is_not_credited_when_every_visible_label_overrides_it() {
        let typography = ThemeTextStyle::default().with_font_size_px(26.0).unwrap();
        let resolved = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new()
                    .with_typography(TypographySpec::default().with_default(typography)),
            )
            .unwrap()
            .resolve(crate::render_family::RenderFamilyKind::State);
        let mut first = semantic_node("First", "rect");
        first.label_style = "font-size:18px".to_string();
        let mut second = semantic_node("Second", "rect");
        second.label_style = "font-size:20px".to_string();
        let mut model = StateDiagramRenderModel::default();
        model.nodes.extend([first, second]);

        let (plan, evidence) =
            resolve_theme_plan_with_evidence(&model, &json!({}), &resolved, None);
        let key = FamilyThemeMechanismKey::Typography;

        assert_eq!(plan.node("First").unwrap().text_style().font_size, 18.0);
        assert_eq!(plan.node("Second").unwrap().text_style().font_size, 20.0);
        assert!(!evidence.applied().contains(&key));
        assert!(evidence.not_applicable_mechanisms().contains(&key));
        assert!(
            evidence
                .residuals()
                .iter()
                .all(|residual| residual.key() != &key)
        );
    }

    #[test]
    fn base_font_size_is_applied_when_any_visible_label_keeps_it() {
        let typography = ThemeTextStyle::default().with_font_size_px(26.0).unwrap();
        let resolved = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new()
                    .with_typography(TypographySpec::default().with_default(typography)),
            )
            .unwrap()
            .resolve(crate::render_family::RenderFamilyKind::State);
        let mut overridden = semantic_node("Overridden", "rect");
        overridden.label_style = "font-size:18px".to_string();
        let surviving = semantic_node("Surviving", "rect");
        let mut model = StateDiagramRenderModel::default();
        model.nodes.extend([overridden, surviving]);

        let (plan, evidence) =
            resolve_theme_plan_with_evidence(&model, &json!({}), &resolved, None);
        let key = FamilyThemeMechanismKey::Typography;

        assert_eq!(
            plan.node("Overridden").unwrap().text_style().font_size,
            18.0
        );
        assert_eq!(plan.node("Surviving").unwrap().text_style().font_size, 26.0);
        assert!(evidence.applied().contains(&key));
        assert!(!evidence.not_applicable_mechanisms().contains(&key));
        assert!(evidence.residuals().iter().all(|item| item.key() != &key));
    }

    #[test]
    fn surviving_advanced_base_typography_remains_residual() {
        let typography = ThemeTextStyle::default()
            .with_line_height(crate::diagram_theme::LineHeight::Multiplier(1.4))
            .unwrap();
        let resolved = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new()
                    .with_typography(TypographySpec::default().with_default(typography)),
            )
            .unwrap()
            .resolve(crate::render_family::RenderFamilyKind::State);
        let mut model = StateDiagramRenderModel::default();
        model.nodes.push(semantic_node("Ready", "rect"));

        let (_, evidence) = resolve_theme_plan_with_evidence(&model, &json!({}), &resolved, None);
        let key = FamilyThemeMechanismKey::Typography;

        assert!(!evidence.applied().contains(&key));
        assert!(evidence.residuals().iter().any(|residual| {
            residual.key() == &key
                && residual.reason() == FamilyThemeResidualReason::UnsupportedTypography
        }));
    }

    #[test]
    fn base_spacing_is_shared_by_measurement_emission_and_evidence() {
        let typography = ThemeTextStyle::default()
            .with_letter_spacing_px(1.5)
            .unwrap()
            .with_word_spacing_px(2.5)
            .unwrap();
        let resolved = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new()
                    .with_typography(TypographySpec::default().with_default(typography)),
            )
            .unwrap()
            .resolve(crate::render_family::RenderFamilyKind::State);
        let mut model = StateDiagramRenderModel::default();
        model.nodes.push(semantic_node("Ready", "rect"));

        let (plan, evidence) =
            resolve_theme_plan_with_evidence(&model, &json!({}), &resolved, None);
        let node = plan.node("Ready").expect("prepared node style");
        let prepared = node
            .resolved_label_typography()
            .prepared_typography()
            .expect("structured typography");
        let key = FamilyThemeMechanismKey::Typography;

        assert_eq!(prepared.letter_spacing_px(), 1.5);
        assert_eq!(prepared.word_spacing_px(), 2.5);
        assert!(
            node.label_style_attr()
                .contains("letter-spacing:1.5px !important")
        );
        assert!(
            node.label_style_attr()
                .contains("word-spacing:2.5px !important")
        );
        assert!(evidence.applied().contains(&key));
        assert!(evidence.residuals().iter().all(|item| item.key() != &key));
    }

    #[test]
    fn semantic_spacing_is_shared_by_measurement_emission_and_evidence() {
        let mut typography = TextStylePatch::default();
        typography.letter_spacing_px = Specified::Value(3.0);
        typography.word_spacing_px = Specified::Value(4.0);
        let resolved = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::StateLabel,
                        ThemeStylePatch {
                            typography,
                            ..ThemeStylePatch::default()
                        },
                    ),
                )),
            )
            .unwrap()
            .resolve(crate::render_family::RenderFamilyKind::State);
        let mut model = StateDiagramRenderModel::default();
        model.nodes.push(semantic_node("Ready", "rect"));

        let (plan, evidence) =
            resolve_theme_plan_with_evidence(&model, &json!({}), &resolved, None);
        let node = plan.node("Ready").expect("prepared node style");
        let prepared = node
            .resolved_label_typography()
            .prepared_typography()
            .expect("structured typography");
        let key = FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::StateLabel,
        };

        assert_eq!(prepared.letter_spacing_px(), 3.0);
        assert_eq!(prepared.word_spacing_px(), 4.0);
        assert!(
            node.label_style_attr()
                .contains("letter-spacing:3px !important")
        );
        assert!(
            node.label_style_attr()
                .contains("word-spacing:4px !important")
        );
        assert!(evidence.applied().contains(&key));
        assert!(evidence.residuals().iter().all(|item| item.key() != &key));
    }

    #[test]
    fn title_only_document_uses_family_typography_after_legacy_title_defaults() {
        let typography = ThemeTextStyle::default().with_font_size_px(26.0).unwrap();
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new()
                    .with_typography(TypographySpec::default().with_default(typography)),
            )
            .unwrap();
        let resolved = theme.resolve(crate::render_family::RenderFamilyKind::State);
        let (plan, evidence) = resolve_theme_plan_with_evidence(
            &StateDiagramRenderModel::default(),
            &json!({}),
            &resolved,
            Some("Architecture"),
        );

        assert_eq!(plan.title_text_style().font_size, 26.0);
        assert!(
            evidence
                .applied()
                .contains(&FamilyThemeMechanismKey::Typography)
        );
        assert!(evidence.residuals().is_empty());
    }

    #[test]
    fn state_rule_effect_overrides_and_clear_suppresses_target_binding() {
        let effects = DiagramEffectSet::default()
            .with_graph(hard_shadow_graph("binding-shadow", 0.0))
            .unwrap()
            .with_graph(hard_shadow_graph("rule-shadow", 0.0))
            .unwrap()
            .with_binding(EffectBinding::new(ThemeTarget::State, "binding-shadow").unwrap())
            .unwrap();
        let binding_key = FamilyThemeMechanismKey::EffectBinding {
            target: ThemeTarget::State,
            effect_id: "binding-shadow".to_string(),
        };
        let rule_key = FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::State,
        };
        let mut model = StateDiagramRenderModel::default();
        model.nodes.push(semantic_node("Ready", "rect"));

        let rule_theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new()
                    .with_effects(effects.clone())
                    .with_styles(
                        ThemeRuleSet::default().with_rule(ThemeRule::new(
                            ThemeTarget::State,
                            ThemeStylePatch::default()
                                .with_effect("rule-shadow")
                                .unwrap(),
                        )),
                    ),
            )
            .unwrap();
        let resolved = rule_theme.resolve(crate::render_family::RenderFamilyKind::State);
        let (plan, evidence) =
            resolve_theme_plan_with_evidence(&model, &json!({}), &resolved, None);

        assert_eq!(
            plan.node("Ready")
                .and_then(StateNodeStylePlan::effect)
                .map(StateNodeEffectPlan::effect_id),
            Some("rule-shadow")
        );
        assert!(evidence.applied().contains(&rule_key));
        assert!(evidence.not_applicable_mechanisms().contains(&binding_key));
        assert!(evidence.residuals().is_empty());
        assert_eq!(
            evidence.applied_capabilities(),
            BTreeSet::from([ThemeCapability::Shadow, ThemeCapability::SvgFilter])
        );

        let clear_theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_effects(effects).with_styles(
                ThemeRuleSet::default().with_rule(ThemeRule::new(
                    ThemeTarget::State,
                    ThemeStylePatch {
                        effects: ThemeEffectPatch {
                            effect: Specified::Clear,
                        },
                        ..ThemeStylePatch::default()
                    },
                )),
            ))
            .unwrap();
        let resolved = clear_theme.resolve(crate::render_family::RenderFamilyKind::State);
        let (plan, evidence) =
            resolve_theme_plan_with_evidence(&model, &json!({}), &resolved, None);

        assert!(plan.node("Ready").unwrap().effect().is_none());
        assert!(evidence.applied().contains(&rule_key));
        assert!(evidence.not_applicable_mechanisms().contains(&binding_key));
        assert!(evidence.residuals().is_empty());
        assert!(evidence.applied_capabilities().is_empty());
    }

    #[test]
    fn state_effect_binding_is_residual_when_graph_or_surface_is_not_supported() {
        for (config, graph) in [
            (json!({}), hard_shadow_graph("soft-shadow", 1.0)),
            (
                json!({ "look": "handDrawn" }),
                hard_shadow_graph("hand-drawn-shadow", 0.0),
            ),
        ] {
            let effect_id = graph.id().to_string();
            let theme = DiagramThemeCompiler::new()
                .compile(
                    DiagramThemeSpec::new().with_effects(
                        DiagramEffectSet::default()
                            .with_graph(graph)
                            .unwrap()
                            .with_binding(
                                EffectBinding::new(ThemeTarget::State, effect_id.clone()).unwrap(),
                            )
                            .unwrap(),
                    ),
                )
                .unwrap();
            let resolved = theme.resolve(crate::render_family::RenderFamilyKind::State);
            let mut model = StateDiagramRenderModel::default();
            model.nodes.push(semantic_node("Ready", "rect"));
            let (plan, evidence) =
                resolve_theme_plan_with_evidence(&model, &config, &resolved, None);
            let key = FamilyThemeMechanismKey::EffectBinding {
                target: ThemeTarget::State,
                effect_id,
            };

            assert!(plan.node("Ready").unwrap().effect().is_none());
            assert!(evidence.residuals().iter().any(|residual| {
                residual.key() == &key
                    && residual.reason() == FamilyThemeResidualReason::UnsupportedEffect
            }));
            assert!(evidence.applied_capabilities().is_empty());
        }
    }
}
