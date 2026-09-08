use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_core::{MermaidConfig, OperationPhase};

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    InheritedFontStackOutcome, InheritedFontStackPlan, TerminalVariantDomain,
    UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
    resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::model::TreemapDiagramLayout;
use crate::resources::{OperationWorkMeter, PreparedTextRetainedReservation};

#[derive(Debug, Clone)]
enum TreemapSourceTextOverride<T> {
    Generated,
    Inherited,
    Value(T),
    Unverified,
}

impl<T> Default for TreemapSourceTextOverride<T> {
    fn default() -> Self {
        Self::Generated
    }
}

#[derive(Debug, Clone, Default)]
struct TreemapSourceTextStyle {
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
    inherited_font_stack: InheritedFontStackPlan,
    text_fill: Option<DirectStaticPaint>,
    text_fill_routes: Box<[(FamilyThemeMechanismKey, usize)]>,
    label_config_owns_text_fill: bool,
    value_config_owns_text_fill: bool,
    root_font_size_px: f64,
    section_source_styles: Box<[TreemapSourceTextStyle]>,
    leaf_source_styles: Box<[TreemapSourceTextStyle]>,
    possible_participating_text_count: usize,
    evidence: FamilyThemeEvidence,
    terminal_receipt: OnceLock<TreemapTypographyTerminalSeal>,
    _retained_reservation: PreparedTextRetainedReservation,
}

impl TreemapTypographyThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        layout: &TreemapDiagramLayout,
        work_meter: std::sync::Arc<OperationWorkMeter>,
    ) -> crate::Result<Self> {
        let inherited_font_stack =
            InheritedFontStackPlan::resolve_property_local(theme, effective_config);
        let text_fill = theme.and_then(|theme| {
            let style = theme.style(ThemeTarget::Text, ThemeVariant::Default, None);
            resolve_direct_static_fill(
                theme,
                &style,
                &[ThemeTarget::Text],
                DirectStaticSelectorDomain::Default,
            )
        });
        let text_fill_routes = theme
            .map(|theme| {
                theme
                    .family_mechanism_routes()
                    .iter()
                    .copied()
                    .filter_map(|route| match route.mechanism() {
                        FamilyThemeMechanism::RuleFacet {
                            rule_index,
                            target: ThemeTarget::Text,
                            selector:
                                FamilyThemeSelectorShape::Static {
                                    variant: None | Some(ThemeVariant::Default),
                                },
                            facet: FamilyThemeRuleFacet::Fill(_),
                        } if route.disposition() == FamilyThemeDisposition::TypedAdapter => {
                            Some((theme.family_mechanism_key(route), rule_index))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice()
            })
            .unwrap_or_default();
        let theme_text_color_is_source_owned =
            merman_core::__private::config_path_overrides_typed_default(
                effective_config,
                "themeVariables.textColor",
            );
        let label_config_owns_text_fill = theme_text_color_is_source_owned
            || merman_core::__private::config_path_overrides_typed_default(
                effective_config,
                "treemap.labelColor",
            );
        let value_config_owns_text_fill = theme_text_color_is_source_owned
            || merman_core::__private::config_path_overrides_typed_default(
                effective_config,
                "treemap.valueColor",
            );
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
        let mut possible_participating_text_count = usize::from(
            layout
                .title
                .as_deref()
                .is_some_and(|title| !title.trim().is_empty()),
        );
        let mut section_source_styles = Vec::with_capacity(layout.sections.len());
        for section in &layout.sections {
            let style = resolve_treemap_source_text_style(
                section.css_compiled_styles.as_deref().unwrap_or_default(),
                root_font_size_px,
                work_meter.as_ref(),
            )?;
            possible_participating_text_count = possible_participating_text_count.saturating_add(
                usize::from(section.depth != 0 && !section.name.trim().is_empty())
                    + usize::from(layout.show_values && section.depth != 0 && section.value != 0.0),
            );
            section_source_styles.push(style);
        }
        let mut leaf_source_styles = Vec::with_capacity(layout.leaves.len());
        for leaf in &layout.leaves {
            let style = resolve_treemap_source_text_style(
                leaf.css_compiled_styles.as_deref().unwrap_or_default(),
                root_font_size_px,
                work_meter.as_ref(),
            )?;
            possible_participating_text_count = possible_participating_text_count.saturating_add(
                usize::from(!leaf.name.trim().is_empty())
                    + usize::from(layout.show_values && leaf.value != 0.0),
            );
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

        Ok(Self {
            inherited_font_stack,
            text_fill,
            text_fill_routes,
            label_config_owns_text_fill,
            value_config_owns_text_fill,
            root_font_size_px,
            section_source_styles: section_source_styles.into_boxed_slice(),
            leaf_source_styles: leaf_source_styles.into_boxed_slice(),
            possible_participating_text_count,
            evidence: FamilyThemeEvidence::from_theme(theme),
            terminal_receipt: OnceLock::new(),
            _retained_reservation: retained_reservation,
        })
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.inherited_font_stack.font_family_css()
    }

    pub(crate) fn label_text_fill_css(&self) -> Option<&str> {
        (!self.label_config_owns_text_fill)
            .then_some(self.text_fill.as_ref())
            .flatten()
            .map(DirectStaticPaint::css)
    }

    pub(crate) fn value_text_fill_css(&self) -> Option<&str> {
        (!self.value_config_owns_text_fill)
            .then_some(self.text_fill.as_ref())
            .flatten()
            .map(DirectStaticPaint::css)
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
        (self.inherited_font_stack.typography_requested() || !self.text_fill_routes.is_empty())
            .then(|| TreemapTypographyThemeReceipt::new(self, layout))
    }

    pub(crate) fn record_terminal(&self, receipt: TreemapTypographyThemeReceipt<'_>) -> bool {
        receipt
            .seal()
            .is_some_and(|seal| self.terminal_receipt.set(seal).is_ok())
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        let Some(receipt) = self.terminal_receipt.get() else {
            self.inherited_font_stack
                .mark_unsupported_typography_evidence(
                    &mut evidence,
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
            for (key, _) in &self.text_fill_routes {
                if self.label_config_owns_text_fill && self.value_config_owns_text_fill {
                    evidence.mark_not_applicable(key.clone());
                } else {
                    evidence
                        .mark_residual(key.clone(), FamilyThemeResidualReason::UnsupportedPaint);
                }
            }
            return evidence;
        };

        if self.inherited_font_stack.typography_requested() {
            self.inherited_font_stack
                .mark_unsupported_typography_evidence(
                    &mut evidence,
                    receipt.participating_count != 0,
                );
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
        for (key, rule_index) in &self.text_fill_routes {
            if self.label_config_owns_text_fill && self.value_config_owns_text_fill {
                evidence.mark_not_applicable(key.clone());
            } else if self
                .text_fill
                .as_ref()
                .is_some_and(|fill| fill.rule_index() == *rule_index && receipt.text_fill_proven)
            {
                let fill = self.text_fill.as_ref().expect("matching Treemap text fill");
                evidence.mark_applied_with_capabilities(key.clone(), [fill.capability()]);
            } else if receipt.typed_text_fill_terminal_count == 0 {
                evidence.mark_not_applicable(key.clone());
            } else if !receipt.text_fill_proven {
                evidence.mark_residual(key.clone(), FamilyThemeResidualReason::UnsupportedPaint);
            } else {
                evidence.mark_not_applicable(key.clone());
            }
        }
        evidence
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

/// CSS facts returned by the Treemap stylesheet writer. These are writer-owned values; the
/// typography receipt never reparses the final stylesheet.
#[derive(Debug, Clone)]
pub(crate) struct TreemapTypographyCssEmission {
    font_family_css: Box<str>,
    base_typography_emitted: bool,
    root_typography_emitted: bool,
    label_text_fill_css: Box<str>,
    value_text_fill_css: Box<str>,
}

impl TreemapTypographyCssEmission {
    pub(crate) fn new(
        font_family_css: &str,
        base_typography_emitted: bool,
        root_typography_emitted: bool,
        label_text_fill_css: &str,
        value_text_fill_css: &str,
    ) -> Self {
        Self {
            font_family_css: font_family_css.into(),
            base_typography_emitted,
            root_typography_emitted,
            label_text_fill_css: label_text_fill_css.into(),
            value_text_fill_css: value_text_fill_css.into(),
        }
    }

    pub(crate) fn font_family_css(&self) -> &str {
        &self.font_family_css
    }

    pub(crate) const fn base_typography_emitted(&self) -> bool {
        self.base_typography_emitted
    }

    pub(crate) const fn root_typography_emitted(&self) -> bool {
        self.root_typography_emitted
    }

    pub(crate) fn label_text_fill_css(&self) -> &str {
        &self.label_text_fill_css
    }

    pub(crate) fn value_text_fill_css(&self) -> &str {
        &self.value_text_fill_css
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

#[derive(Debug)]
pub(crate) struct TreemapTypographyThemeReceipt<'a> {
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
    label_text_fill_terminal_count: usize,
    value_text_fill_terminal_count: usize,
    terminal_matches: bool,
    participating_count: usize,
    source_owned_count: usize,
    unverified_count: usize,
}

#[derive(Debug)]
struct TreemapTypographyTerminalSeal {
    participating_count: usize,
    source_owned_count: usize,
    unverified_count: usize,
    typed_text_fill_terminal_count: usize,
    text_fill_proven: bool,
}

impl<'a> TreemapTypographyThemeReceipt<'a> {
    fn new(plan: &'a TreemapTypographyThemePlan, layout: &TreemapDiagramLayout) -> Self {
        Self {
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
            label_text_fill_terminal_count: 0,
            value_text_fill_terminal_count: 0,
            terminal_matches: true,
            participating_count: 0,
            source_owned_count: 0,
            unverified_count: 0,
        }
    }

    pub(crate) fn record_css_emission(&mut self, emission: TreemapTypographyCssEmission) {
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

    pub(crate) fn record_text(
        &mut self,
        role: TreemapTextRole,
        participates: bool,
        resolved: &TreemapResolvedTextStyle,
        measurement_matches: bool,
    ) {
        self.terminal_matches &= self.expected_role(self.next_role) == Some(role);
        self.terminal_matches &= self.css_emitted;
        self.next_role = self.next_role.saturating_add(1);
        if !participates {
            return;
        }
        self.participating_count = self.participating_count.saturating_add(1);
        match role {
            TreemapTextRole::SectionLabel | TreemapTextRole::LeafLabel
                if self.expected_label_text_fill_css.is_some() =>
            {
                self.label_text_fill_terminal_count =
                    self.label_text_fill_terminal_count.saturating_add(1);
            }
            TreemapTextRole::SectionValue | TreemapTextRole::LeafValue
                if self.expected_value_text_fill_css.is_some() =>
            {
                self.value_text_fill_terminal_count =
                    self.value_text_fill_terminal_count.saturating_add(1);
            }
            TreemapTextRole::Title
            | TreemapTextRole::SectionLabel
            | TreemapTextRole::SectionValue
            | TreemapTextRole::LeafLabel
            | TreemapTextRole::LeafValue => {}
        }
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
            && self.next_role == self.expected_terminal_count())
        .then_some(TreemapTypographyTerminalSeal {
            participating_count: self.participating_count,
            source_owned_count: self.source_owned_count,
            unverified_count: self.unverified_count,
            typed_text_fill_terminal_count: self
                .label_text_fill_terminal_count
                .saturating_add(self.value_text_fill_terminal_count),
            text_fill_proven: (self.label_text_fill_matches
                && self.label_text_fill_terminal_count != 0)
                || (self.value_text_fill_matches && self.value_text_fill_terminal_count != 0),
        })
    }
}

fn resolve_treemap_source_text_style(
    declaration_lists: &[String],
    root_font_size_px: f64,
    work_meter: &OperationWorkMeter,
) -> crate::Result<TreemapSourceTextStyle> {
    let mut style = TreemapSourceTextStyle::default();
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
                        "text-transform" | "letter-spacing" | "word-spacing" | "white-space" => {
                            style.has_unverified_typography = true
                        }
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
                        style.has_unverified_typography |= declaration.important()
                            || !(declaration.inherits_property_value()
                                || declaration.value().eq_ignore_ascii_case("none"));
                    }
                    "letter-spacing" => {
                        style.has_unverified_typography |= declaration.important()
                            || !(declaration.inherits_property_value()
                                || declaration.value().eq_ignore_ascii_case("normal")
                                || declaration.svg_number_or_px() == Some(0.0));
                    }
                    "word-spacing" => {
                        style.has_unverified_typography |= declaration.important()
                            || !(declaration.inherits_property_value()
                                || declaration.value().eq_ignore_ascii_case("normal")
                                || declaration.svg_number_or_px() == Some(0.0));
                    }
                    "white-space" => {
                        style.has_unverified_typography |= declaration.important()
                            || !(declaration.inherits_property_value()
                                || declaration.value().eq_ignore_ascii_case("normal"));
                    }
                    _ => {}
                }
                Ok(true)
            },
        )?;
    }
    Ok(style)
}

/// Final Treemap title fill shared by stylesheet emission and terminal evidence.
#[derive(Debug)]
pub(crate) struct TreemapTitleThemePlan {
    fill_css: Option<Box<str>>,
    typed_fill_capability: Option<ThemeCapability>,
    evidence: FamilyThemeEvidence,
    pending_fill_key: Option<FamilyThemeMechanismKey>,
    terminal_receipt: OnceLock<()>,
}

impl TreemapTitleThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        title: Option<&str>,
        work_meter: &OperationWorkMeter,
    ) -> crate::Result<Self> {
        let title_present = title.is_some_and(|title| !title.trim().is_empty());
        let Some(theme) = theme else {
            return Ok(Self {
                fill_css: None,
                typed_fill_capability: None,
                evidence: FamilyThemeEvidence::default(),
                pending_fill_key: None,
                terminal_receipt: OnceLock::new(),
            });
        };

        let title_count = usize::from(title_present);
        let config_owns_fill = merman_core::__private::config_path_overrides_typed_default(
            effective_config,
            "treemap.titleColor",
        ) || merman_core::__private::config_path_overrides_typed_default(
            effective_config,
            "themeVariables.titleColor",
        );
        let style = title_present.then(|| {
            theme.style_with_work_meter(
                ThemeTarget::Title,
                ThemeVariant::Default,
                Some(1),
                work_meter,
            )
        });
        let style = style.transpose()?;
        let winner_properties = style
            .as_ref()
            .into_iter()
            .flat_map(|style| style.winner_rule_properties())
            .map(|(property, origin)| (origin.rule_index(), property))
            .collect::<BTreeSet<_>>();

        let typed_fill = (!config_owns_fill)
            .then(|| {
                style.as_ref().and_then(|style| {
                    resolve_direct_static_fill(
                        theme,
                        style,
                        &[ThemeTarget::Title],
                        DirectStaticSelectorDomain::Unqualified,
                    )
                })
            })
            .flatten();
        let (fill_css, typed_fill_rule, typed_fill_capability) =
            typed_fill.map_or((None, None, None), |fill| {
                let (css, rule_index, capability) = fill.into_parts();
                (Some(css), Some(rule_index), Some(capability))
            });
        let source_owned_fill = [config_owns_fill];

        let mut evidence = FamilyThemeEvidence::from_theme(Some(theme));
        let mut observations = BTreeMap::<usize, TreemapTitleRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Title,
                    selector,
                    facet,
                } => {
                    let observation = observations.entry(rule_index).or_default();
                    if !selector.ordinal_domain_intersects_occurrence_count(title_count) {
                        continue;
                    }
                    let route_won = winner_properties
                        .contains(&(rule_index, resolved_style_property_for_facet(facet)));
                    let qualified_variant = matches!(
                        selector,
                        FamilyThemeSelectorShape::Static { variant: Some(_) }
                            | FamilyThemeSelectorShape::Ordinal {
                                variant: Some(_),
                                ..
                            }
                    );
                    if !route_won && !qualified_variant {
                        continue;
                    }

                    observation.applicable = true;
                    if config_owns_fill && matches!(facet, FamilyThemeRuleFacet::Fill(_)) {
                        observation.fill_config_owned = true;
                        continue;
                    }
                    match (route.disposition(), selector, facet) {
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeSelectorShape::Static { variant: None },
                            FamilyThemeRuleFacet::Fill(_),
                        ) if typed_fill_rule == Some(rule_index) => {
                            observation.fill_pending = true;
                        }
                        (FamilyThemeDisposition::Unsupported, _, facet) => {
                            observation
                                .residual
                                .get_or_insert(unsupported_residual_for_facet(facet));
                        }
                        (FamilyThemeDisposition::TypedAdapter, _, _)
                        | (FamilyThemeDisposition::LegacyCompatibility, _, _) => {
                            observation.incomplete = true;
                        }
                    }
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Title,
                } => {
                    // Reconciled below from the final title fill winner.
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Title,
                    ..
                } => {
                    // Reconciled below from the final title style.
                }
                FamilyThemeMechanism::BaseTypography(_)
                | FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {}
            }
        }

        reconcile_unsupported_terminal_domains(
            theme,
            &mut evidence,
            &[UnsupportedTerminalDomain::fallbacks_only(
                ThemeTarget::Title,
                TerminalVariantDomain::uniform(title_count, ThemeVariant::Default),
            )
            .with_source_owned_fill(&source_owned_fill)],
            work_meter,
        )?;

        let mut pending_fill_key = None;
        for (rule_index, observation) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target: ThemeTarget::Title,
            };
            if !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // Mixed rules stay unaccounted until every winning facet has a terminal owner.
            } else if observation.fill_pending {
                debug_assert!(pending_fill_key.is_none());
                pending_fill_key = Some(key);
            } else if observation.fill_config_owned {
                evidence.mark_not_applicable(key);
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        Ok(Self {
            fill_css,
            typed_fill_capability,
            evidence,
            pending_fill_key,
            terminal_receipt: OnceLock::new(),
        })
    }

    pub(crate) fn fill_css(&self) -> Option<&str> {
        self.fill_css.as_deref()
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<TreemapTitleThemeReceipt> {
        self.fill_css
            .as_ref()
            .map(|fill_css| TreemapTitleThemeReceipt::new(fill_css.clone()))
    }

    pub(crate) fn record_terminal(&self, receipt: TreemapTitleThemeReceipt) -> bool {
        self.fill_css.as_deref().is_some_and(|fill_css| {
            receipt.proves(fill_css) && self.terminal_receipt.set(()).is_ok()
        })
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        if let Some(key) = self.pending_fill_key.clone()
            && self.terminal_receipt.get().is_some()
            && let Some(capability) = self.typed_fill_capability
        {
            evidence.mark_applied_with_capabilities(key, [capability]);
        }
        evidence
    }
}

/// Writer-owned proof that the selected title fill reached both the stylesheet and title node.
#[derive(Debug)]
pub(crate) struct TreemapTitleThemeReceipt {
    expected_fill: Box<str>,
    stylesheet_fill: Option<Box<str>>,
    stylesheet_class: Option<Box<str>>,
    title_text_count: usize,
    terminal_matches: bool,
}

impl TreemapTitleThemeReceipt {
    fn new(expected_fill: Box<str>) -> Self {
        Self {
            expected_fill,
            stylesheet_fill: None,
            stylesheet_class: None,
            title_text_count: 0,
            terminal_matches: true,
        }
    }

    pub(crate) fn record_stylesheet(&mut self, emitted_class: &str, emitted_fill: &str) {
        if self.stylesheet_fill.is_some() || self.stylesheet_class.is_some() {
            self.terminal_matches = false;
            return;
        }
        self.terminal_matches &= emitted_class == super::TREEMAP_TITLE_CLASS;
        self.terminal_matches &= emitted_fill == self.expected_fill.as_ref();
        self.stylesheet_class = Some(emitted_class.into());
        self.stylesheet_fill = Some(emitted_fill.into());
    }

    pub(crate) fn record_title_text(&mut self, emitted_class: &str) {
        self.title_text_count = self.title_text_count.saturating_add(1);
        self.terminal_matches &= emitted_class == super::TREEMAP_TITLE_CLASS;
    }

    fn proves(&self, expected_fill: &str) -> bool {
        self.expected_fill.as_ref() == expected_fill
            && self.stylesheet_fill.as_deref() == Some(expected_fill)
            && self.stylesheet_class.as_deref() == Some(super::TREEMAP_TITLE_CLASS)
            && self.title_text_count == 1
            && self.terminal_matches
    }
}

#[derive(Debug, Default)]
struct TreemapTitleRuleObservation {
    applicable: bool,
    fill_config_owned: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    fill_pending: bool,
}

#[cfg(test)]
mod tests {
    use super::{TreemapTitleThemePlan, TreemapTitleThemeReceipt};
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, OrdinalPalette, ThemeColorValue,
        ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
    };
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy};
    use merman_core::MermaidConfig;
    use serde_json::json;

    #[test]
    fn title_receipt_requires_one_matching_stylesheet_and_title_checkpoint() {
        let mut complete = TreemapTitleThemeReceipt::new("#123456".into());
        complete.record_stylesheet(super::super::TREEMAP_TITLE_CLASS, "#123456");
        complete.record_title_text("treemapTitle");
        assert!(complete.proves("#123456"));

        let mut missing_title = TreemapTitleThemeReceipt::new("#123456".into());
        missing_title.record_stylesheet(super::super::TREEMAP_TITLE_CLASS, "#123456");
        assert!(!missing_title.proves("#123456"));

        let mut wrong_fill = TreemapTitleThemeReceipt::new("#123456".into());
        wrong_fill.record_stylesheet(super::super::TREEMAP_TITLE_CLASS, "#abcdef");
        wrong_fill.record_title_text("treemapTitle");
        assert!(!wrong_fill.proves("#123456"));

        let mut wrong_class = TreemapTitleThemeReceipt::new("#123456".into());
        wrong_class.record_stylesheet("otherTitle", "#123456");
        wrong_class.record_title_text("treemapTitle");
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
        let config = MermaidConfig::from_value(json!({}));
        let plan = TreemapTitleThemePlan::resolve(Some(&resolved), &config, Some("Title"), &meter)
            .expect("resolve Treemap title theme");
        let mut receipt = plan.begin_terminal_receipt().expect("typed title receipt");
        receipt.record_stylesheet(super::super::TREEMAP_TITLE_CLASS, "#123456");
        receipt.record_title_text(super::super::TREEMAP_TITLE_CLASS);
        assert!(plan.record_terminal(receipt));

        let evidence = plan.finish_evidence();
        assert_eq!(resolved.family_evidence_mechanism_keys().len(), 2);
        assert_eq!(evidence.applied().len(), 1);
        assert_eq!(evidence.not_applicable_mechanisms().len(), 1);
        assert!(evidence.residuals().is_empty());
    }
}
