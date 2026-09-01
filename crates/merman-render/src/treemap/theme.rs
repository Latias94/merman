use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    InheritedFontStackOutcome, InheritedFontStackPlan, TerminalVariantDomain,
    UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
    resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::model::TreemapDiagramLayout;
use crate::resources::OperationWorkMeter;

/// Final inherited Treemap font stack shared by CSS emission, text fitting, and evidence.
///
/// Treemap's other text sizes are role-local values from Mermaid (`14px`, `12px`, `10px`, and
/// the adaptive leaf sizes), so base `FontSize` remains intentionally unsupported. The inherited
/// stack is nevertheless a real consumer: Mermaid uses it for the title and all section/leaf text
/// while measuring those same terminals through `getComputedTextLength()`.
#[derive(Debug)]
pub(crate) struct TreemapTypographyThemePlan {
    inherited_font_stack: InheritedFontStackPlan,
    section_font_ownership: Box<[crate::mermaid_style::CssFontFamilyOwnership]>,
    leaf_font_ownership: Box<[crate::mermaid_style::CssFontFamilyOwnership]>,
    possible_visible_text_count: usize,
    evidence: FamilyThemeEvidence,
    terminal_receipt: OnceLock<TreemapTypographyTerminalSeal>,
}

impl TreemapTypographyThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        layout: &TreemapDiagramLayout,
    ) -> Self {
        let inherited_font_stack =
            InheritedFontStackPlan::resolve_property_local(theme, effective_config);
        let section_font_ownership = layout
            .sections
            .iter()
            .map(|section| {
                crate::mermaid_style::css_font_family_ownership(
                    section
                        .css_compiled_styles
                        .as_deref()
                        .unwrap_or_default()
                        .iter()
                        .map(String::as_str),
                )
            })
            .collect();
        let leaf_font_ownership = layout
            .leaves
            .iter()
            .map(|leaf| {
                crate::mermaid_style::css_font_family_ownership(
                    leaf.css_compiled_styles
                        .as_deref()
                        .unwrap_or_default()
                        .iter()
                        .map(String::as_str),
                )
            })
            .collect();
        let possible_visible_text_count = possible_visible_text_count(layout);

        Self {
            inherited_font_stack,
            section_font_ownership,
            leaf_font_ownership,
            possible_visible_text_count,
            evidence: FamilyThemeEvidence::from_theme(theme),
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.inherited_font_stack.font_family_css()
    }

    pub(crate) fn section_font_ownership(
        &self,
        index: usize,
    ) -> crate::mermaid_style::CssFontFamilyOwnership {
        self.section_font_ownership
            .get(index)
            .copied()
            .unwrap_or(crate::mermaid_style::CssFontFamilyOwnership::Unverified)
    }

    pub(crate) fn leaf_font_ownership(
        &self,
        index: usize,
    ) -> crate::mermaid_style::CssFontFamilyOwnership {
        self.leaf_font_ownership
            .get(index)
            .copied()
            .unwrap_or(crate::mermaid_style::CssFontFamilyOwnership::Unverified)
    }

    pub(crate) fn begin_terminal_receipt(
        &self,
        layout: &TreemapDiagramLayout,
    ) -> Option<TreemapTypographyThemeReceipt<'_>> {
        self.inherited_font_stack
            .typography_requested()
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
                    self.possible_visible_text_count != 0,
                );
            if self.inherited_font_stack.typed_font_stack_requested()
                && self.possible_visible_text_count != 0
            {
                evidence.mark_residual(
                    FamilyThemeMechanismKey::Typography(
                        crate::diagram_theme::ThemeTypographyProperty::FontStack,
                    ),
                    FamilyThemeResidualReason::UnsupportedTypography,
                );
            }
            return evidence;
        };

        self.inherited_font_stack
            .mark_unsupported_typography_evidence(&mut evidence, receipt.visible_count != 0);
        if !self.inherited_font_stack.typed_font_stack_requested() {
            return evidence;
        }

        let key = FamilyThemeMechanismKey::Typography(
            crate::diagram_theme::ThemeTypographyProperty::FontStack,
        );
        match self.inherited_font_stack.outcome() {
            InheritedFontStackOutcome::ConfigOwned => evidence.mark_not_applicable(key),
            InheritedFontStackOutcome::Typed
                if receipt.visible_count == 0
                    && receipt.source_owned_count == 0
                    && receipt.unverified_count == 0 =>
            {
                evidence.mark_not_applicable(key)
            }
            InheritedFontStackOutcome::Typed
                if receipt.source_owned_count == receipt.visible_count
                    && receipt.unverified_count == 0 =>
            {
                evidence.mark_not_applicable(key)
            }
            InheritedFontStackOutcome::Typed
                if receipt.visible_count != 0
                    && receipt.source_owned_count == 0
                    && receipt.unverified_count == 0 =>
            {
                evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography])
            }
            InheritedFontStackOutcome::Typed | InheritedFontStackOutcome::Unsupported => {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography)
            }
            InheritedFontStackOutcome::Inactive => {}
        }
        evidence
    }
}

/// CSS facts returned by the Treemap stylesheet writer. These are writer-owned values; the
/// typography receipt never reparses the final stylesheet.
#[derive(Debug, Clone)]
pub(crate) struct TreemapTypographyCssEmission {
    font_family_css: Box<str>,
    base_typography_emitted: bool,
    root_typography_emitted: bool,
}

impl TreemapTypographyCssEmission {
    pub(crate) fn new(
        font_family_css: &str,
        base_typography_emitted: bool,
        root_typography_emitted: bool,
    ) -> Self {
        Self {
            font_family_css: font_family_css.into(),
            base_typography_emitted,
            root_typography_emitted,
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
    expected_roles: Box<[TreemapTextRole]>,
    next_role: usize,
    css_emitted: bool,
    css_matches: bool,
    terminal_matches: bool,
    measurement_matches: bool,
    visible_count: usize,
    source_owned_count: usize,
    unverified_count: usize,
}

#[derive(Debug)]
struct TreemapTypographyTerminalSeal {
    visible_count: usize,
    source_owned_count: usize,
    unverified_count: usize,
}

impl<'a> TreemapTypographyThemeReceipt<'a> {
    fn new(plan: &'a TreemapTypographyThemePlan, layout: &TreemapDiagramLayout) -> Self {
        let mut expected_roles = Vec::with_capacity(
            usize::from(layout.title.is_some())
                .saturating_add(
                    layout
                        .sections
                        .len()
                        .saturating_mul(usize::from(layout.show_values).saturating_add(1)),
                )
                .saturating_add(
                    layout
                        .leaves
                        .len()
                        .saturating_mul(usize::from(layout.show_values).saturating_add(1)),
                ),
        );
        if layout
            .title
            .as_deref()
            .is_some_and(|title| !title.trim().is_empty())
        {
            expected_roles.push(TreemapTextRole::Title);
        }
        for _ in &layout.sections {
            expected_roles.push(TreemapTextRole::SectionLabel);
            if layout.show_values {
                expected_roles.push(TreemapTextRole::SectionValue);
            }
        }
        for _ in &layout.leaves {
            expected_roles.push(TreemapTextRole::LeafLabel);
            if layout.show_values {
                expected_roles.push(TreemapTextRole::LeafValue);
            }
        }
        Self {
            expected_font_family_css: plan.font_family_css(),
            expected_roles: expected_roles.into_boxed_slice(),
            next_role: 0,
            css_emitted: false,
            css_matches: false,
            terminal_matches: true,
            measurement_matches: true,
            visible_count: 0,
            source_owned_count: 0,
            unverified_count: 0,
        }
    }

    pub(crate) fn record_css_emission(&mut self, emission: TreemapTypographyCssEmission) {
        self.terminal_matches &= !self.css_emitted;
        self.css_matches = emission.base_typography_emitted()
            && emission.root_typography_emitted()
            && emission.font_family_css() == self.expected_font_family_css;
        self.css_emitted = true;
    }

    pub(crate) fn record_measurement(&mut self, font_family_css: Option<&str>) {
        self.measurement_matches &= font_family_css == Some(self.expected_font_family_css);
    }

    pub(crate) fn record_text(
        &mut self,
        role: TreemapTextRole,
        visible: bool,
        ownership: crate::mermaid_style::CssFontFamilyOwnership,
    ) {
        self.terminal_matches &= self
            .expected_roles
            .get(self.next_role)
            .is_some_and(|expected| *expected == role);
        self.terminal_matches &= self.css_emitted;
        self.next_role = self.next_role.saturating_add(1);
        if !visible {
            return;
        }
        self.visible_count = self.visible_count.saturating_add(1);
        match ownership {
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
            && self.terminal_matches
            && self.measurement_matches
            && self.next_role == self.expected_roles.len())
        .then_some(TreemapTypographyTerminalSeal {
            visible_count: self.visible_count,
            source_owned_count: self.source_owned_count,
            unverified_count: self.unverified_count,
        })
    }
}

fn possible_visible_text_count(layout: &TreemapDiagramLayout) -> usize {
    let title = usize::from(
        layout
            .title
            .as_deref()
            .is_some_and(|title| !title.trim().is_empty()),
    );
    let sections = layout
        .sections
        .iter()
        .map(|section| {
            usize::from(section.depth != 0 && !section.name.trim().is_empty())
                + usize::from(layout.show_values && section.depth != 0 && section.value != 0.0)
        })
        .sum::<usize>();
    let leaves = layout
        .leaves
        .iter()
        .map(|leaf| {
            usize::from(!leaf.name.trim().is_empty())
                + usize::from(layout.show_values && leaf.value != 0.0)
        })
        .sum::<usize>();
    title.saturating_add(sections).saturating_add(leaves)
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
