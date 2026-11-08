use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_model::OperationPhase;
use merman_model::presentation::PresentationConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    ResolvedDiagramTheme, ResolvedStyleProperty, ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectPaintExpectation, DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence,
    FamilyThemeOutcomes, FamilyThemeResidualReason, InheritedFontStackOutcome,
    InheritedFontStackPlan, SingleTitleFillPlan, TerminalVariantDomain, UnsupportedTerminalDomain,
    reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
    resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::model::TreemapDiagramLayout;
use crate::resources::{OperationWorkMeter, PreparedTextRetainedReservation};

mod node_visual;

#[derive(Debug, Clone, Default)]
enum TreemapSourceTextOverride<T> {
    #[default]
    Generated,
    Inherited,
    Value(T),
    Unverified,
}

#[derive(Debug, Clone, Default)]
struct TreemapSourceTextStyle {
    visual: node_visual::TreemapNodeVisual,
    font_family: TreemapSourceTextOverride<Box<str>>,
    font_size_px: TreemapSourceTextOverride<f64>,
    font_weight: TreemapSourceTextOverride<Box<str>>,
    font_style: TreemapSourceTextOverride<Box<str>>,
    has_unverified_typography: bool,
}

impl TreemapSourceTextStyle {
    fn resolve(
        &self,
        root_font_family_css: &str,
        root_font_size_px: f64,
        generated_font_size_px: f64,
        generated_font_weight: Option<&str>,
        generated_font_style: Option<&str>,
    ) -> TreemapResolvedTextStyle {
        let mut verified = true;
        let (font_family, ownership) = match &self.font_family {
            TreemapSourceTextOverride::Generated | TreemapSourceTextOverride::Inherited => (
                Some(root_font_family_css.to_string()),
                crate::mermaid_style::CssFontFamilyOwnership::Inherited,
            ),
            TreemapSourceTextOverride::Value(font_family) => (
                Some(font_family.to_string()),
                crate::mermaid_style::CssFontFamilyOwnership::SourceOwned,
            ),
            TreemapSourceTextOverride::Unverified => {
                verified = false;
                (
                    Some(root_font_family_css.to_string()),
                    crate::mermaid_style::CssFontFamilyOwnership::Unverified,
                )
            }
        };
        let font_size = match &self.font_size_px {
            TreemapSourceTextOverride::Generated => generated_font_size_px,
            TreemapSourceTextOverride::Inherited => root_font_size_px,
            TreemapSourceTextOverride::Value(font_size_px) => *font_size_px,
            TreemapSourceTextOverride::Unverified => {
                verified = false;
                generated_font_size_px
            }
        };
        let font_weight = match &self.font_weight {
            TreemapSourceTextOverride::Generated => generated_font_weight.map(str::to_owned),
            TreemapSourceTextOverride::Inherited => None,
            TreemapSourceTextOverride::Value(font_weight) => Some(font_weight.to_string()),
            TreemapSourceTextOverride::Unverified => {
                verified = false;
                generated_font_weight.map(str::to_owned)
            }
        };
        let font_style = match &self.font_style {
            TreemapSourceTextOverride::Generated => generated_font_style.map(str::to_owned),
            TreemapSourceTextOverride::Inherited => None,
            TreemapSourceTextOverride::Value(font_style) => Some(font_style.to_string()),
            TreemapSourceTextOverride::Unverified => {
                verified = false;
                generated_font_style.map(str::to_owned)
            }
        };
        verified &= !self.has_unverified_typography;

        TreemapResolvedTextStyle {
            style: crate::text::TextStyle {
                font_family,
                font_size,
                font_weight,
                font_style,
            },
            font_family_ownership: ownership,
            verified,
        }
    }

    fn retained_bytes(&self) -> Option<usize> {
        fn override_bytes(value: &TreemapSourceTextOverride<Box<str>>) -> usize {
            match value {
                TreemapSourceTextOverride::Value(value) => value.len(),
                TreemapSourceTextOverride::Generated
                | TreemapSourceTextOverride::Inherited
                | TreemapSourceTextOverride::Unverified => 0,
            }
        }

        std::mem::size_of::<Self>()
            .checked_add(override_bytes(&self.font_family))?
            .checked_add(override_bytes(&self.font_weight))?
            .checked_add(override_bytes(&self.font_style))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TreemapResolvedTextStyle {
    style: crate::text::TextStyle,
    font_family_ownership: crate::mermaid_style::CssFontFamilyOwnership,
    verified: bool,
}

impl TreemapResolvedTextStyle {
    pub(crate) const fn style(&self) -> &crate::text::TextStyle {
        &self.style
    }

    pub(crate) const fn font_family_ownership(
        &self,
    ) -> crate::mermaid_style::CssFontFamilyOwnership {
        self.font_family_ownership
    }

    pub(crate) const fn verified(&self) -> bool {
        self.verified
    }

    pub(crate) fn with_font_size_px(&self, font_size_px: f64) -> Self {
        let mut resolved = self.clone();
        resolved.style.font_size = font_size_px;
        resolved
    }

    pub(crate) fn matches_measurement(&self, actual: &crate::text::TextStyle) -> bool {
        actual.font_family == self.style.font_family
            && actual.font_size == self.style.font_size
            && actual.font_weight == self.style.font_weight
            && actual.font_style == self.style.font_style
    }
}

/// Final inherited Treemap font stack shared by CSS emission, text fitting, and evidence.
///
/// Treemap's other text sizes are role-local values from Mermaid (`14px`, `12px`, `10px`, and
/// the adaptive leaf sizes), so base `FontSize` remains intentionally unsupported. The inherited
/// stack is nevertheless a real consumer: Mermaid uses it for the title and all section/leaf text
/// while measuring those same terminals through `getComputedTextLength()`.
#[derive(Debug)]
pub(crate) struct TreemapTypographyThemePlan {
    css: super::css_binding::TreemapCssBinding,
    inherited_font_stack: InheritedFontStackPlan,
    text_fill: Option<DirectStaticPaint>,
    root_font_size_px: f64,
    section_source_styles: Box<[TreemapSourceTextStyle]>,
    leaf_source_styles: Box<[TreemapSourceTextStyle]>,
    possible_participating_text_count: usize,
    node_paint_requested: bool,
    node_label_requested: bool,
    node_pending: BTreeMap<FamilyThemeMechanismKey, BTreeSet<ThemeCapability>>,
    evidence: FamilyThemeOutcomes,
    terminal_receipt: OnceLock<TreemapTypographyTerminalSeal>,
    _retained_reservation: PreparedTextRetainedReservation,
    _visual_retained_reservation: Option<PreparedTextRetainedReservation>,
}

impl TreemapTypographyThemePlan {
    #[cfg(test)]
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &PresentationConfig,
        layout: &TreemapDiagramLayout,
        work_meter: std::sync::Arc<OperationWorkMeter>,
    ) -> crate::Result<Self> {
        Self::resolve_with_title_fill(theme, effective_config, layout, work_meter, None)
    }

    pub(crate) fn resolve_with_title_fill(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &PresentationConfig,
        layout: &TreemapDiagramLayout,
        work_meter: std::sync::Arc<OperationWorkMeter>,
        title_fill: Option<&str>,
    ) -> crate::Result<Self> {
        let inherited_font_stack =
            InheritedFontStackPlan::resolve_property_local(theme, effective_config);
        let text_style = theme
            .map(|theme| {
                theme.style_with_work_meter(
                    ThemeTarget::Text,
                    ThemeVariant::Default,
                    None,
                    &work_meter,
                )
            })
            .transpose()?;
        let text_fill = theme.zip(text_style.as_ref()).and_then(|(theme, style)| {
            resolve_direct_static_fill(
                theme,
                style,
                &[ThemeTarget::Text],
                DirectStaticSelectorDomain::Default,
            )
        });
        let root_font_size_px = crate::config::config_theme_font_size_css_or_root_number_px(
            effective_config.as_value(),
            16.0,
        );
        let retained_upper_bound = treemap_source_style_retained_upper_bound(
            layout,
            inherited_font_stack.font_family_css().len(),
            work_meter.as_ref(),
        )?;
        let mut retained_reservation = work_meter
            .reserve_prepared_text_retained_bytes(retained_upper_bound)
            .map_err(crate::Error::from)?;
        // This count only feeds unsupported typography evidence. Source styles still need to be
        // resolved below for ordinary measurement and rendering, but an unthemed plan has no
        // typography evidence to reconcile and must not scan every visible label just to compute
        // an unused count.
        let tracks_typography_evidence =
            theme.is_some() && inherited_font_stack.typography_requested();
        let mut possible_participating_text_count = if tracks_typography_evidence {
            usize::from(
                layout
                    .title
                    .as_deref()
                    .is_some_and(|title| !title.trim().is_empty()),
            )
        } else {
            0
        };
        let mut section_source_styles = Vec::with_capacity(layout.sections.len());
        for section in &layout.sections {
            let style = resolve_treemap_source_text_style(
                section.css_compiled_styles.as_deref().unwrap_or_default(),
                root_font_size_px,
                work_meter.as_ref(),
            )?;
            if tracks_typography_evidence {
                possible_participating_text_count = possible_participating_text_count
                    .saturating_add(
                        usize::from(section.depth != 0 && !section.name.trim().is_empty())
                            + usize::from(
                                layout.show_values && section.depth != 0 && section.value != 0.0,
                            ),
                    );
            }
            section_source_styles.push(style);
        }
        let mut leaf_source_styles = Vec::with_capacity(layout.leaves.len());
        for leaf in &layout.leaves {
            let style = resolve_treemap_source_text_style(
                leaf.css_compiled_styles.as_deref().unwrap_or_default(),
                root_font_size_px,
                work_meter.as_ref(),
            )?;
            if tracks_typography_evidence {
                possible_participating_text_count = possible_participating_text_count
                    .saturating_add(
                        usize::from(!leaf.name.trim().is_empty())
                            + usize::from(layout.show_values && leaf.value != 0.0),
                    );
            }
            leaf_source_styles.push(style);
        }
        let retained_bytes = section_source_styles
            .iter()
            .chain(&leaf_source_styles)
            .try_fold(
                inherited_font_stack.font_family_css().len(),
                |retained, style| retained.checked_add(style.retained_bytes()?),
            )
            .ok_or_else(|| crate::Error::from(work_meter.arithmetic_overflow()))?;
        retained_reservation.reconcile_downward(retained_bytes);

        let mut css = super::css_binding::TreemapCssBinding::new(
            effective_config.as_value(),
            inherited_font_stack.font_family_css(),
        )?;
        if let Some(fill) = title_fill {
            css.title_color = fill.to_owned();
        }
        if let Some(fill) = text_fill.as_ref() {
            css.label_color = fill.css().to_owned();
            css.value_color = fill.css().to_owned();
        }
        Self {
            css,
            inherited_font_stack,
            text_fill,
            root_font_size_px,
            section_source_styles: section_source_styles.into_boxed_slice(),
            leaf_source_styles: leaf_source_styles.into_boxed_slice(),
            possible_participating_text_count,
            node_paint_requested: false,
            node_label_requested: false,
            node_pending: BTreeMap::new(),
            evidence: FamilyThemeOutcomes::default(),
            terminal_receipt: OnceLock::new(),
            _retained_reservation: retained_reservation,
            _visual_retained_reservation: None,
        }
        .prepare_node_visuals(layout, &work_meter, theme, effective_config)
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.css.common.font_family()
    }

    pub(crate) fn css_binding(&self) -> &super::css_binding::TreemapCssBinding {
        &self.css
    }

    pub(crate) fn section_visual(&self, index: usize) -> Option<&node_visual::TreemapNodeVisual> {
        self.section_source_styles
            .get(index)
            .map(|source| &source.visual)
    }

    pub(crate) fn leaf_visual(&self, index: usize) -> Option<&node_visual::TreemapNodeVisual> {
        self.leaf_source_styles
            .get(index)
            .map(|source| &source.visual)
    }

    pub(crate) fn label_text_fill_css(&self) -> Option<&str> {
        self.text_fill.as_ref().map(DirectStaticPaint::css)
    }

    pub(crate) fn value_text_fill_css(&self) -> Option<&str> {
        self.text_fill.as_ref().map(DirectStaticPaint::css)
    }

    pub(crate) fn title_text_style(&self, title_font_size_css: &str) -> TreemapResolvedTextStyle {
        let title_font_size_css = crate::mermaid_style::strip_css_important(title_font_size_css);
        let title_font_size_px = if title_font_size_css.eq_ignore_ascii_case("inherit") {
            Some(self.root_font_size_px)
        } else {
            crate::mermaid_style::resolve_mermaid_font_size_px(
                title_font_size_css,
                crate::mermaid_style::CssFontSizeContext::new(
                    self.root_font_size_px,
                    self.root_font_size_px,
                ),
            )
        };
        let mut resolved = TreemapSourceTextStyle::default().resolve(
            self.font_family_css(),
            self.root_font_size_px,
            title_font_size_px.unwrap_or(14.0),
            None,
            None,
        );
        resolved.verified &= title_font_size_px.is_some();
        resolved
    }

    pub(crate) fn section_text_style(
        &self,
        index: usize,
        generated_font_size_px: f64,
        generated_font_weight: Option<&str>,
        generated_font_style: Option<&str>,
    ) -> TreemapResolvedTextStyle {
        self.section_source_styles
            .get(index)
            .map(|style| {
                style.resolve(
                    self.font_family_css(),
                    self.root_font_size_px,
                    generated_font_size_px,
                    generated_font_weight,
                    generated_font_style,
                )
            })
            .unwrap_or_else(|| TreemapResolvedTextStyle {
                style: crate::text::TextStyle {
                    font_family: Some(self.font_family_css().to_string()),
                    font_size: generated_font_size_px,
                    font_weight: generated_font_weight.map(str::to_owned),
                    font_style: generated_font_style.map(str::to_owned),
                },
                font_family_ownership: crate::mermaid_style::CssFontFamilyOwnership::Unverified,
                verified: false,
            })
    }

    pub(crate) fn leaf_text_style(
        &self,
        index: usize,
        generated_font_size_px: f64,
    ) -> TreemapResolvedTextStyle {
        self.leaf_source_styles
            .get(index)
            .map(|style| {
                style.resolve(
                    self.font_family_css(),
                    self.root_font_size_px,
                    generated_font_size_px,
                    None,
                    None,
                )
            })
            .unwrap_or_else(|| TreemapResolvedTextStyle {
                style: crate::text::TextStyle {
                    font_family: Some(self.font_family_css().to_string()),
                    font_size: generated_font_size_px,
                    font_weight: None,
                    font_style: None,
                },
                font_family_ownership: crate::mermaid_style::CssFontFamilyOwnership::Unverified,
                verified: false,
            })
    }

    pub(crate) fn begin_terminal_receipt(
        &self,
        layout: &TreemapDiagramLayout,
    ) -> Option<TreemapTypographyThemeReceipt<'_>> {
        (self.inherited_font_stack.typography_requested()
            || self.node_paint_requested
            || self.node_label_requested)
            .then(|| TreemapTypographyThemeReceipt::new(self, layout))
    }

    pub(crate) fn record_terminal(
        &self,
        mut receipt: TreemapTypographyThemeReceipt<'_>,
        theme: Option<&ResolvedDiagramTheme>,
        work_meter: &OperationWorkMeter,
    ) -> crate::Result<bool> {
        let node_label_terminals = receipt.node_label_terminals.take();
        let Some(mut seal) = receipt.seal() else {
            return Ok(false);
        };
        let mut fallback_evidence = FamilyThemeOutcomes::default();
        if let Some(terminals) = node_label_terminals {
            let Some(theme) = theme else { return Ok(false) };
            node_visual::record_text_paint_evidence(
                theme,
                &terminals,
                &mut fallback_evidence,
                work_meter,
            )?;
        }
        seal.fallback_evidence = Some(fallback_evidence);
        Ok(self.terminal_receipt.set(seal).is_ok())
    }

    pub(crate) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        evidence.merge_borrowed(&self.evidence);
        for (key, capabilities) in &self.node_pending {
            if self.terminal_receipt.get().is_some() {
                evidence.mark_applied_with_capabilities(key.clone(), capabilities.iter().copied());
            } else {
                evidence.mark_residual(key.clone(), FamilyThemeResidualReason::UnsupportedPaint);
            }
        }
        let Some(receipt) = self.terminal_receipt.get() else {
            self.inherited_font_stack
                .record_unsupported_typography_evidence(
                    evidence,
                    self.possible_participating_text_count != 0,
                );
            if self.inherited_font_stack.typed_font_stack_requested()
                && self.possible_participating_text_count != 0
            {
                evidence.mark_residual(
                    FamilyThemeMechanismKey::Typography(
                        crate::diagram_theme::ThemeTypographyProperty::FontStack,
                    ),
                    FamilyThemeResidualReason::UnsupportedTypography,
                );
            }
            return;
        };

        if let Some(fallback) = &receipt.fallback_evidence {
            evidence.merge_borrowed(fallback);
        }
        if self.inherited_font_stack.typography_requested() {
            self.inherited_font_stack
                .record_unsupported_typography_evidence(evidence, receipt.participating_count != 0);
            if self.inherited_font_stack.typed_font_stack_requested() {
                let key = FamilyThemeMechanismKey::Typography(
                    crate::diagram_theme::ThemeTypographyProperty::FontStack,
                );
                match self.inherited_font_stack.outcome() {
                    InheritedFontStackOutcome::ConfigOwned => evidence.mark_not_applicable(key),
                    InheritedFontStackOutcome::Typed
                        if receipt.participating_count == 0
                            && receipt.source_owned_count == 0
                            && receipt.unverified_count == 0 =>
                    {
                        evidence.mark_not_applicable(key)
                    }
                    InheritedFontStackOutcome::Typed
                        if receipt.source_owned_count == receipt.participating_count
                            && receipt.unverified_count == 0 =>
                    {
                        evidence.mark_not_applicable(key)
                    }
                    InheritedFontStackOutcome::Typed
                        if receipt.participating_count != 0 && receipt.unverified_count == 0 =>
                    {
                        evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography])
                    }
                    InheritedFontStackOutcome::Typed | InheritedFontStackOutcome::Unsupported => {
                        evidence
                            .mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography)
                    }
                    InheritedFontStackOutcome::Inactive => {}
                }
            }
        }
    }
}

fn treemap_source_style_retained_upper_bound(
    layout: &TreemapDiagramLayout,
    root_font_family_bytes: usize,
    work_meter: &OperationWorkMeter,
) -> crate::Result<usize> {
    let item_count = layout
        .sections
        .len()
        .checked_add(layout.leaves.len())
        .ok_or_else(|| crate::Error::from(work_meter.arithmetic_overflow()))?;
    let mut retained_bytes = item_count
        .checked_mul(std::mem::size_of::<TreemapSourceTextStyle>())
        .and_then(|bytes| bytes.checked_add(root_font_family_bytes))
        .ok_or_else(|| crate::Error::from(work_meter.arithmetic_overflow()))?;

    for declaration_lists in layout
        .sections
        .iter()
        .map(|section| section.css_compiled_styles.as_deref().unwrap_or_default())
        .chain(
            layout
                .leaves
                .iter()
                .map(|leaf| leaf.css_compiled_styles.as_deref().unwrap_or_default()),
        )
    {
        work_meter.charge(1)?;
        for declaration_list in declaration_lists {
            work_meter.charge(1)?;
            retained_bytes = retained_bytes
                .checked_add(declaration_list.len())
                .ok_or_else(|| crate::Error::from(work_meter.arithmetic_overflow()))?;
        }
    }

    Ok(retained_bytes)
}

/// CSS facts returned after successful writes, borrowing the prepared values. The typography
/// receipt never reparses the final stylesheet.
#[derive(Debug, Clone)]
pub(crate) struct TreemapTypographyCssEmission<'a> {
    font_family_css: &'a str,
    base_typography_emitted: bool,
    root_typography_emitted: bool,
    label_text_fill_css: &'a str,
    value_text_fill_css: &'a str,
}

impl<'a> TreemapTypographyCssEmission<'a> {
    pub(crate) fn new(
        font_family_css: &'a str,
        base_typography_emitted: bool,
        root_typography_emitted: bool,
        label_text_fill_css: &'a str,
        value_text_fill_css: &'a str,
    ) -> Self {
        Self {
            font_family_css,
            base_typography_emitted,
            root_typography_emitted,
            label_text_fill_css,
            value_text_fill_css,
        }
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.font_family_css
    }

    pub(crate) const fn base_typography_emitted(&self) -> bool {
        self.base_typography_emitted
    }

    pub(crate) const fn root_typography_emitted(&self) -> bool {
        self.root_typography_emitted
    }

    pub(crate) fn label_text_fill_css(&self) -> &str {
        self.label_text_fill_css
    }

    pub(crate) fn value_text_fill_css(&self) -> &str {
        self.value_text_fill_css
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TreemapTextRole {
    Title,
    SectionLabel,
    SectionValue,
    LeafLabel,
    LeafValue,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum TreemapTextFillOwnership {
    #[default]
    Generated,
    SourceOwned,
    Unverified,
}

#[derive(Debug)]
pub(crate) struct TreemapTypographyThemeReceipt<'a> {
    plan: &'a TreemapTypographyThemePlan,
    expected_font_family_css: &'a str,
    expected_label_text_fill_css: Option<&'a str>,
    expected_value_text_fill_css: Option<&'a str>,
    title_expected: bool,
    section_count: usize,
    leaf_count: usize,
    show_values: bool,
    next_role: usize,
    css_emitted: bool,
    css_matches: bool,
    label_text_fill_matches: bool,
    value_text_fill_matches: bool,
    terminal_matches: bool,
    participating_count: usize,
    source_owned_count: usize,
    unverified_count: usize,
    next_node: usize,
    node_label_terminals: Option<Vec<node_visual::TreemapLabelTerminal>>,
}

#[derive(Debug)]
struct TreemapTypographyTerminalSeal {
    fallback_evidence: Option<FamilyThemeOutcomes>,
    participating_count: usize,
    source_owned_count: usize,
    unverified_count: usize,
}

impl<'a> TreemapTypographyThemeReceipt<'a> {
    fn new(plan: &'a TreemapTypographyThemePlan, layout: &TreemapDiagramLayout) -> Self {
        Self {
            plan,
            expected_font_family_css: plan.font_family_css(),
            expected_label_text_fill_css: plan.label_text_fill_css(),
            expected_value_text_fill_css: plan.value_text_fill_css(),
            title_expected: layout
                .title
                .as_deref()
                .is_some_and(|title| !title.trim().is_empty()),
            section_count: layout.sections.len(),
            leaf_count: layout.leaves.len(),
            show_values: layout.show_values,
            next_role: 0,
            css_emitted: false,
            css_matches: false,
            label_text_fill_matches: plan.label_text_fill_css().is_none(),
            value_text_fill_matches: plan.value_text_fill_css().is_none(),
            terminal_matches: true,
            participating_count: 0,
            source_owned_count: 0,
            unverified_count: 0,
            next_node: 0,
            node_label_terminals: plan.node_label_requested.then(|| {
                Vec::with_capacity(
                    layout
                        .sections
                        .len()
                        .saturating_add(layout.leaves.len())
                        .saturating_mul(1 + usize::from(layout.show_values)),
                )
            }),
        }
    }

    pub(crate) fn record_css_emission(&mut self, emission: TreemapTypographyCssEmission<'_>) {
        self.terminal_matches &= !self.css_emitted;
        self.css_matches = emission.base_typography_emitted()
            && emission.root_typography_emitted()
            && emission.font_family_css() == self.expected_font_family_css;
        if let Some(expected) = self.expected_label_text_fill_css {
            self.label_text_fill_matches = emission.label_text_fill_css() == expected;
        }
        if let Some(expected) = self.expected_value_text_fill_css {
            self.value_text_fill_matches = emission.value_text_fill_css() == expected;
        }
        self.css_emitted = true;
    }

    fn expected_role(&self, terminal_index: usize) -> Option<TreemapTextRole> {
        let mut index = terminal_index;
        if self.title_expected {
            if index == 0 {
                return Some(TreemapTextRole::Title);
            }
            index = index.saturating_sub(1);
        }

        let stride = usize::from(self.show_values).saturating_add(1);
        let section_terminals = self.section_count.saturating_mul(stride);
        if index < section_terminals {
            return Some(if self.show_values && index % stride == 1 {
                TreemapTextRole::SectionValue
            } else {
                TreemapTextRole::SectionLabel
            });
        }
        index = index.saturating_sub(section_terminals);
        let leaf_terminals = self.leaf_count.saturating_mul(stride);
        if index < leaf_terminals {
            return Some(if self.show_values && index % stride == 1 {
                TreemapTextRole::LeafValue
            } else {
                TreemapTextRole::LeafLabel
            });
        }
        None
    }

    fn expected_terminal_count(&self) -> usize {
        let stride = usize::from(self.show_values).saturating_add(1);
        usize::from(self.title_expected)
            .saturating_add(self.section_count.saturating_mul(stride))
            .saturating_add(self.leaf_count.saturating_mul(stride))
    }

    pub(crate) fn record_node(
        &mut self,
        section: bool,
        index: usize,
        fill: &str,
        stroke: &str,
        style: &str,
    ) {
        let ordinal = if section {
            index
        } else {
            self.section_count + index
        };
        self.terminal_matches &= self.css_emitted && ordinal == self.next_node;
        let visual = if section {
            self.plan.section_visual(index)
        } else {
            self.plan.leaf_visual(index)
        };
        self.terminal_matches &= visual.is_some_and(|visual| {
            visual.fill == fill && visual.stroke == stroke && visual.rect_style == style
        });
        self.next_node = self.next_node.saturating_add(1);
    }

    pub(crate) fn record_text(
        &mut self,
        role: TreemapTextRole,
        participates: bool,
        fill_ownership: TreemapTextFillOwnership,
        resolved: &TreemapResolvedTextStyle,
        measurement_matches: bool,
        emitted_fill: &str,
    ) {
        let participates = participates && resolved.style().font_size > 0.0;
        let stride = 1 + usize::from(self.show_values);
        let index = self
            .next_role
            .saturating_sub(usize::from(self.title_expected))
            / stride;
        let visual = if role == TreemapTextRole::Title {
            None
        } else if index < self.section_count {
            self.plan.section_visual(index)
        } else {
            self.plan
                .leaf_visual(index.saturating_sub(self.section_count))
        };
        let value_role = matches!(
            role,
            TreemapTextRole::SectionValue | TreemapTextRole::LeafValue
        );
        let role_owns_fill = visual.is_some_and(|visual| visual.node_label_winner);
        let expected_paint = visual.and_then(|visual| {
            if value_role {
                visual.value_paint.as_ref()
            } else {
                visual.label_paint.as_ref()
            }
        });
        if participates {
            if let Some(visual) = visual {
                let (plain, fitted) = if value_role {
                    (&visual.value_fill, &visual.fitted_value_fill)
                } else {
                    (&visual.label_fill, &visual.fitted_label_fill)
                };
                self.terminal_matches &= emitted_fill == plain || emitted_fill == fitted;
            }
            if role != TreemapTextRole::Title
                && let Some(terminals) = &mut self.node_label_terminals
            {
                terminals.push(node_visual::TreemapLabelTerminal {
                    ordinal: visual.map_or(0, |visual| visual.ordinal),
                    source_owned: fill_ownership == TreemapTextFillOwnership::SourceOwned
                        || visual.is_some_and(|visual| visual.label_config_owns_fill),
                    unverified: fill_ownership == TreemapTextFillOwnership::Unverified,
                    paint: expected_paint
                        .map(|paint| (paint.rule_index(), paint.capability()))
                        .or_else(|| {
                            (!role_owns_fill)
                                .then_some(self.plan.text_fill.as_ref())
                                .flatten()
                                .map(|paint| (paint.rule_index(), paint.capability()))
                        }),
                });
            }
        }
        self.terminal_matches &= self.expected_role(self.next_role) == Some(role);
        self.terminal_matches &= self.css_emitted;
        self.next_role = self.next_role.saturating_add(1);
        if !participates {
            return;
        }
        self.participating_count = self.participating_count.saturating_add(1);
        let inherited_matches = resolved.font_family_ownership()
            != crate::mermaid_style::CssFontFamilyOwnership::Inherited
            || resolved.style().font_family.as_deref() == Some(self.expected_font_family_css);
        if !resolved.verified() || !measurement_matches || !inherited_matches {
            self.unverified_count = self.unverified_count.saturating_add(1);
            return;
        }
        match resolved.font_family_ownership() {
            crate::mermaid_style::CssFontFamilyOwnership::Inherited => {}
            crate::mermaid_style::CssFontFamilyOwnership::SourceOwned => {
                self.source_owned_count = self.source_owned_count.saturating_add(1)
            }
            crate::mermaid_style::CssFontFamilyOwnership::Unverified => {
                self.unverified_count = self.unverified_count.saturating_add(1)
            }
        }
    }

    fn seal(self) -> Option<TreemapTypographyTerminalSeal> {
        (self.css_emitted
            && self.css_matches
            && self.label_text_fill_matches
            && self.value_text_fill_matches
            && self.terminal_matches
            && self.next_role == self.expected_terminal_count()
            && (!self.plan.node_paint_requested
                || self.next_node == self.section_count + self.leaf_count))
            .then_some(TreemapTypographyTerminalSeal {
                fallback_evidence: None,
                participating_count: self.participating_count,
                source_owned_count: self.source_owned_count,
                unverified_count: self.unverified_count,
            })
    }
}

fn resolve_treemap_source_text_style(
    declaration_lists: &[String],
    root_font_size_px: f64,
    work_meter: &OperationWorkMeter,
) -> crate::Result<TreemapSourceTextStyle> {
    let mut style = TreemapSourceTextStyle::default();
    let mut text_transform_unverified = false;
    let mut letter_spacing_unverified = false;
    let mut word_spacing_unverified = false;
    let mut white_space_unverified = false;
    let font_size_context =
        crate::mermaid_style::CssFontSizeContext::new(root_font_size_px, root_font_size_px);

    for declaration_list in declaration_lists {
        work_meter.charge(1usize.saturating_add(declaration_list.len().div_ceil(64)))?;
        let mut checkpoint = || work_meter.checkpoint(OperationPhase::Layout);
        crate::mermaid_style::visit_style_declaration_boundaries_with_checkpoints(
            declaration_list,
            &mut checkpoint,
            |boundary| {
                work_meter.charge(1)?;
                let raw = boundary.raw();
                let Some(declaration) = crate::mermaid_style::parse_style_declaration(raw) else {
                    let property = raw
                        .split_once(':')
                        .map(|(property, _)| property.trim())
                        .unwrap_or_else(|| raw.trim());
                    match property {
                        "font-family" => style.font_family = TreemapSourceTextOverride::Unverified,
                        "font-size" => style.font_size_px = TreemapSourceTextOverride::Unverified,
                        "font-weight" => style.font_weight = TreemapSourceTextOverride::Unverified,
                        "font-style" => style.font_style = TreemapSourceTextOverride::Unverified,
                        "text-transform" => text_transform_unverified = true,
                        "letter-spacing" => letter_spacing_unverified = true,
                        "word-spacing" => word_spacing_unverified = true,
                        "white-space" => white_space_unverified = true,
                        _ => {}
                    }
                    return Ok(true);
                };

                // Mermaid's Treemap style splitter uses exact, case-sensitive keys before the
                // declarations reach the browser. Keep that boundary instead of treating this as
                // a general CSS cascade.
                match declaration.property_source().trim() {
                    "font-family" => {
                        style.font_family = if declaration.important() {
                            // Upstream appends its own `!important`; accepting an authored one
                            // would produce an invalid doubled priority token.
                            TreemapSourceTextOverride::Unverified
                        } else if declaration.inherits_property_value() {
                            TreemapSourceTextOverride::Inherited
                        } else if crate::mermaid_style::is_static_css_font_family_list(
                            declaration.value(),
                        ) {
                            TreemapSourceTextOverride::Value(declaration.value().into())
                        } else {
                            TreemapSourceTextOverride::Unverified
                        };
                    }
                    "font-size" => {
                        style.font_size_px = if declaration.important() {
                            TreemapSourceTextOverride::Unverified
                        } else if declaration.inherits_property_value() {
                            TreemapSourceTextOverride::Inherited
                        } else if crate::mermaid_style::css_font_size_is_zero(declaration.value()) {
                            // The shared positive-size resolver intentionally excludes zero.
                            // Treemap's fitting pass must retain it so the label stays hidden.
                            TreemapSourceTextOverride::Value(0.0)
                        } else if let Some(font_size_px) =
                            declaration.resolve_font_size_px(font_size_context)
                        {
                            if font_size_px.is_finite() && font_size_px >= 0.0 {
                                TreemapSourceTextOverride::Value(font_size_px)
                            } else {
                                TreemapSourceTextOverride::Unverified
                            }
                        } else {
                            TreemapSourceTextOverride::Unverified
                        };
                    }
                    "font-weight" => {
                        style.font_weight = if declaration.important() {
                            TreemapSourceTextOverride::Unverified
                        } else if declaration.inherits_property_value() {
                            TreemapSourceTextOverride::Inherited
                        } else if crate::mermaid_style::is_supported_css_font_weight_value(
                            declaration.value(),
                        ) {
                            TreemapSourceTextOverride::Value(declaration.value().into())
                        } else {
                            TreemapSourceTextOverride::Unverified
                        };
                    }
                    "font-style" => {
                        style.font_style = if declaration.important() {
                            TreemapSourceTextOverride::Unverified
                        } else if declaration.inherits_property_value() {
                            TreemapSourceTextOverride::Inherited
                        } else if crate::mermaid_style::is_supported_css_font_style_value(
                            declaration.value(),
                        ) {
                            TreemapSourceTextOverride::Value(declaration.value().into())
                        } else {
                            TreemapSourceTextOverride::Unverified
                        };
                    }
                    "text-transform" => {
                        text_transform_unverified = declaration.important()
                            || !(declaration.inherits_property_value()
                                || declaration.value().eq_ignore_ascii_case("none"));
                    }
                    "letter-spacing" => {
                        letter_spacing_unverified = declaration.important()
                            || !(declaration.inherits_property_value()
                                || declaration.value().eq_ignore_ascii_case("normal")
                                || declaration.svg_number_or_px() == Some(0.0));
                    }
                    "word-spacing" => {
                        word_spacing_unverified = declaration.important()
                            || !(declaration.inherits_property_value()
                                || declaration.value().eq_ignore_ascii_case("normal")
                                || declaration.svg_number_or_px() == Some(0.0));
                    }
                    "white-space" => {
                        white_space_unverified = declaration.important()
                            || !(declaration.inherits_property_value()
                                || declaration.value().eq_ignore_ascii_case("normal"));
                    }
                    _ => {}
                }
                Ok(true)
            },
        )?;
    }
    // Mermaid folds repeated class declarations by exact property before emitting CSS.
    // Only each property's final value can invalidate the measured typography.
    style.has_unverified_typography = text_transform_unverified
        || letter_spacing_unverified
        || word_spacing_unverified
        || white_space_unverified;
    Ok(style)
}

/// Final Treemap title fill shared by stylesheet emission and terminal evidence.
#[derive(Debug)]
pub(crate) struct TreemapTitleThemePlan {
    fill: SingleTitleFillPlan,
}

impl TreemapTitleThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &PresentationConfig,
        title: Option<&str>,
        work_meter: &OperationWorkMeter,
    ) -> crate::Result<Self> {
        let Some(theme) = theme else {
            return Ok(Self {
                fill: SingleTitleFillPlan::default(),
            });
        };
        let title_present = title.is_some_and(|title| !title.trim().is_empty());
        let config_owns_fill = effective_config
            .config_paint_overrides_typed_default("treemap.titleColor")
            || effective_config.config_paint_overrides_typed_default("themeVariables.titleColor");
        let fill = SingleTitleFillPlan::resolve(
            theme,
            title_present,
            config_owns_fill,
            FamilyThemeOutcomes::default(),
            work_meter,
        )?;
        Ok(Self { fill })
    }

    pub(crate) fn fill_css(&self) -> Option<&str> {
        self.fill.fill_css()
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<TreemapTitleThemeReceipt<'_>> {
        self.fill.fill_css().map(TreemapTitleThemeReceipt::new)
    }

    pub(crate) fn record_terminal(&self, receipt: TreemapTitleThemeReceipt<'_>) -> bool {
        self.fill
            .fill_css()
            .is_some_and(|fill_css| receipt.proves(fill_css) && self.fill.record_proven_terminal())
    }

    pub(crate) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        self.fill.record_evidence(evidence);
    }
}

/// Writer-owned proof that the selected title fill reached both the stylesheet and title node.
#[derive(Debug)]
pub(crate) struct TreemapTitleThemeReceipt<'a> {
    expected_fill: &'a str,
    css_emitted: bool,
    title_text_count: usize,
    terminal_matches: bool,
}

impl<'a> TreemapTitleThemeReceipt<'a> {
    fn new(expected_fill: &'a str) -> Self {
        Self {
            expected_fill,
            css_emitted: false,
            title_text_count: 0,
            terminal_matches: true,
        }
    }

    pub(crate) fn record_stylesheet(&mut self, emitted_class: &str, emitted_fill: &str) {
        if self.css_emitted {
            self.terminal_matches = false;
            return;
        }
        self.terminal_matches &= emitted_class == super::TREEMAP_TITLE_CLASS;
        self.terminal_matches &= emitted_fill == self.expected_fill;
        self.css_emitted = true;
    }

    pub(crate) fn record_title_text(&mut self, emitted_class: &str) {
        self.title_text_count = self.title_text_count.saturating_add(1);
        self.terminal_matches &= emitted_class == super::TREEMAP_TITLE_CLASS;
    }

    fn proves(&self, expected_fill: &str) -> bool {
        self.expected_fill == expected_fill
            && self.css_emitted
            && self.title_text_count == 1
            && self.terminal_matches
    }
}

#[cfg(test)]
mod tests {
    fn collect_evidence(
        theme: &crate::diagram_theme::ResolvedDiagramTheme,
        record: impl FnOnce(&mut crate::family::FamilyThemeEvidence),
    ) -> crate::family::FamilyThemeEvidence {
        let mut evidence = crate::family::FamilyThemeEvidence::from_theme(Some(theme));
        record(&mut evidence);
        evidence
    }

    use super::{TreemapTitleThemePlan, TreemapTitleThemeReceipt, TreemapTypographyThemePlan};
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, OrdinalPalette, ThemeColorValue,
        ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
    };
    use crate::model::{TreemapDiagramLayout, TreemapLeafLayout, TreemapSectionLayout};
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy};
    use serde_json::json;

    #[test]
    fn unthemed_typography_plan_skips_evidence_only_participating_text_scan() {
        let layout = TreemapDiagramLayout {
            title_height: 30.0,
            width: 960.0,
            height: 500.0,
            use_max_width: true,
            diagram_padding: 8.0,
            show_values: true,
            value_format: ",".to_string(),
            acc_title: None,
            acc_descr: None,
            title: Some("A title that still needs ordinary rendering".to_string()),
            sections: vec![TreemapSectionLayout {
                name: "A section".to_string(),
                depth: 1,
                value: 3.0,
                x0: 0.0,
                y0: 0.0,
                x1: 100.0,
                y1: 100.0,
                class_selector: None,
                css_compiled_styles: None,
            }],
            leaves: vec![TreemapLeafLayout {
                name: "A leaf".to_string(),
                value: 2.0,
                parent_name: Some("A section".to_string()),
                x0: 0.0,
                y0: 0.0,
                x1: 50.0,
                y1: 50.0,
                class_selector: None,
                css_compiled_styles: None,
            }],
        };
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let config = crate::test_support::presentation_config(json!({}));
        let plan = TreemapTypographyThemePlan::resolve(None, &config, &layout, meter.into())
            .expect("resolve unthemed Treemap typography plan");

        assert_eq!(plan.possible_participating_text_count, 0);
        assert!(plan.begin_terminal_receipt(&layout).is_none());
    }

    #[test]
    fn title_receipt_requires_one_matching_stylesheet_and_title_checkpoint() {
        let mut complete = TreemapTitleThemeReceipt::new("#123456");
        complete.record_stylesheet(super::super::TREEMAP_TITLE_CLASS, "#123456");
        complete.record_title_text("treemapTitle");
        assert!(complete.proves("#123456"));
        assert!(!complete.proves("#abcdef"));
        complete.record_title_text("treemapTitle");
        assert!(!complete.proves("#123456"));

        let mut missing_stylesheet = TreemapTitleThemeReceipt::new("#123456");
        missing_stylesheet.record_title_text("treemapTitle");
        assert!(!missing_stylesheet.proves("#123456"));

        let mut duplicate_stylesheet = TreemapTitleThemeReceipt::new("#123456");
        duplicate_stylesheet.record_stylesheet(super::super::TREEMAP_TITLE_CLASS, "#123456");
        duplicate_stylesheet.record_title_text("treemapTitle");
        duplicate_stylesheet.record_stylesheet(super::super::TREEMAP_TITLE_CLASS, "#123456");
        assert!(!duplicate_stylesheet.proves("#123456"));

        let mut empty_fill = TreemapTitleThemeReceipt::new("");
        empty_fill.record_stylesheet(super::super::TREEMAP_TITLE_CLASS, "");
        empty_fill.record_title_text("treemapTitle");
        assert!(empty_fill.proves(""));

        let mut missing_title = TreemapTitleThemeReceipt::new("#123456");
        missing_title.record_stylesheet(super::super::TREEMAP_TITLE_CLASS, "#123456");
        assert!(!missing_title.proves("#123456"));

        let mut wrong_fill = TreemapTitleThemeReceipt::new("#123456");
        wrong_fill.record_stylesheet(super::super::TREEMAP_TITLE_CLASS, "#abcdef");
        wrong_fill.record_title_text("treemapTitle");
        assert!(!wrong_fill.proves("#123456"));
        wrong_fill.record_stylesheet(super::super::TREEMAP_TITLE_CLASS, "#123456");
        assert!(!wrong_fill.proves("#123456"));

        let mut wrong_class = TreemapTitleThemeReceipt::new("#123456");
        wrong_class.record_stylesheet("otherTitle", "#123456");
        wrong_class.record_title_text("treemapTitle");
        assert!(!wrong_class.proves("#123456"));
        wrong_class.record_stylesheet(super::super::TREEMAP_TITLE_CLASS, "#123456");
        assert!(!wrong_class.proves("#123456"));
    }

    #[test]
    fn typed_title_fill_shadows_unsupported_ordinal_palette() {
        let palette = OrdinalPalette::new([
            ThemeColorValue::parse("#abcdef").expect("valid Treemap ordinal palette color")
        ])
        .expect("non-empty Treemap ordinal palette");
        let resolved = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default()
                        .with_rule(ThemeRule::new(
                            ThemeTarget::Title,
                            ThemeStylePatch::default().with_fill(
                                CanvasPaint::solid("#123456").expect("valid Treemap title fill"),
                            ),
                        ))
                        .with_ordinal_palette(ThemeTarget::Title, palette),
                ),
            )
            .expect("compile Treemap title fill and palette theme")
            .resolve(DiagramFamilyId::TREEMAP);
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let config = crate::test_support::presentation_config(json!({}));
        let plan = TreemapTitleThemePlan::resolve(Some(&resolved), &config, Some("Title"), &meter)
            .expect("resolve Treemap title theme");
        let mut receipt = plan.begin_terminal_receipt().expect("typed title receipt");
        receipt.record_stylesheet(super::super::TREEMAP_TITLE_CLASS, "#123456");
        receipt.record_title_text(super::super::TREEMAP_TITLE_CLASS);
        assert!(plan.record_terminal(receipt));

        let evidence = collect_evidence(&resolved, |evidence| plan.record_evidence(evidence));
        assert_eq!(evidence.required_mechanisms().len(), 2);
        assert_eq!(evidence.applied().len(), 1);
        assert_eq!(evidence.not_applicable_mechanisms().len(), 1);
        assert!(evidence.residuals().is_empty());
    }
}
