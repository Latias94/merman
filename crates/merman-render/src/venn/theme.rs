use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ThemeCapability, ThemeTarget,
    ThemeTypographyProperty, ThemeVariant,
};
use crate::family::{
    DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    InheritedFontStackOutcome, InheritedFontStackPlan, TerminalVariantDomain,
    UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
    resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::model::VennDiagramLayout;
use crate::resources::OperationWorkMeter;

mod css_binding;
pub(crate) use css_binding::{VennAreaPaintBinding, VennCssBinding};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct VennTypographyOccurrences {
    titles: usize,
    circle_labels: usize,
    intersection_labels: usize,
    text_nodes: usize,
}

impl VennTypographyOccurrences {
    fn from_layout(title: Option<&str>, layout: &VennDiagramLayout) -> Self {
        let titles = usize::from(title.is_some_and(|title| !title.trim().is_empty()));
        let circle_labels = layout
            .areas
            .iter()
            .filter(|area| {
                area.sets.len() == 1 && !super::rendered_area_label(area).trim().is_empty()
            })
            .count();
        let intersection_labels = layout
            .areas
            .iter()
            .filter(|area| {
                area.sets.len() > 1 && !super::rendered_area_label(area).trim().is_empty()
            })
            .count();
        let text_nodes = layout
            .text_nodes
            .iter()
            .filter(|node| !super::rendered_text_node_label(node).trim().is_empty())
            .count();
        Self {
            titles,
            circle_labels,
            intersection_labels,
            text_nodes,
        }
    }

    const fn total(self) -> usize {
        self.titles
            .saturating_add(self.circle_labels)
            .saturating_add(self.intersection_labels)
            .saturating_add(self.text_nodes)
    }
}

/// Final Venn inherited font stack shared by its four local stylesheet selectors and evidence.
#[derive(Debug)]
pub(crate) struct VennTypographyThemePlan {
    css_binding: VennCssBinding,
    inherited_font_stack: InheritedFontStackPlan,
    occurrences: VennTypographyOccurrences,
    evidence: FamilyThemeEvidence,
    text_fill: Option<crate::family::DirectStaticPaint>,
    text_fill_routes: Box<[(FamilyThemeMechanismKey, usize)]>,
    config_owns_text_fill: bool,
    terminal_receipt: OnceLock<VennTypographyThemeReceipt>,
}

impl VennTypographyThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        title: Option<&str>,
        layout: &VennDiagramLayout,
        model: &merman_core::diagrams::venn::VennDiagramRenderModel,
        title_theme: &VennTitleThemePlan,
        work_meter: &OperationWorkMeter,
    ) -> crate::Result<Self> {
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
        // `vennSetTextColor` is the renderer's final Venn text-color role.  A caller may set
        // `textColor` without owning that role: Default retains its constructor snapshot, while
        // non-Default themes can derive `vennSetTextColor` and carry the ownership edge there.
        // Check the resolved role rather than its possible source variables.
        let config_owns_text_fill = theme.is_some()
            && merman_core::__private::config_path_overrides_typed_default(
                effective_config,
                "themeVariables.vennSetTextColor",
            );
        let css_binding = VennCssBinding::resolve(
            effective_config.as_value(),
            model,
            layout,
            title_theme.fill_css(),
            (!config_owns_text_fill)
                .then_some(text_fill.as_ref())
                .flatten()
                .map(crate::family::DirectStaticPaint::css),
            work_meter,
        )?;
        Ok(Self {
            css_binding,
            inherited_font_stack: InheritedFontStackPlan::resolve_property_local(
                theme,
                effective_config,
            ),
            occurrences: VennTypographyOccurrences::from_layout(title, layout),
            evidence: theme.map_or_else(FamilyThemeEvidence::default, |theme| {
                FamilyThemeEvidence::from_theme(Some(theme))
            }),
            text_fill,
            text_fill_routes,
            config_owns_text_fill,
            terminal_receipt: OnceLock::new(),
        })
    }

    pub(crate) fn css_binding(&self) -> &VennCssBinding {
        &self.css_binding
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.inherited_font_stack.font_family_css()
    }

    pub(crate) fn begin_terminal_receipt(&self) -> VennTypographyThemeReceipt {
        VennTypographyThemeReceipt::new(
            self.occurrences,
            self.font_family_css(),
            self.text_fill_css(),
        )
    }

    pub(crate) fn text_fill_css(&self) -> Option<&str> {
        (!self.config_owns_text_fill)
            .then_some(self.text_fill.as_ref())
            .flatten()
            .map(crate::family::DirectStaticPaint::css)
    }

    pub(crate) fn record_terminal(&self, receipt: VennTypographyThemeReceipt) -> bool {
        receipt.proves_font_stack() && self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        let key = FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontStack);
        let Some(receipt) = self.terminal_receipt.get() else {
            self.inherited_font_stack
                .mark_unsupported_typography_evidence(&mut evidence, true);
            if self.inherited_font_stack.typed_font_stack_requested() {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
            }
            for (key, _) in &self.text_fill_routes {
                if self.config_owns_text_fill {
                    evidence.mark_not_applicable(key.clone());
                } else {
                    evidence
                        .mark_residual(key.clone(), FamilyThemeResidualReason::UnsupportedPaint);
                }
            }
            return evidence;
        };
        self.inherited_font_stack
            .mark_unsupported_typography_evidence(&mut evidence, self.occurrences.total() != 0);
        if self.occurrences.total() == 0 {
            evidence.mark_not_applicable(key);
            for (key, _) in &self.text_fill_routes {
                evidence.mark_not_applicable(key.clone());
            }
            return evidence;
        }
        if self.inherited_font_stack.typed_font_stack_active() && receipt.proves_font_stack() {
            evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
        } else if self.inherited_font_stack.outcome() == InheritedFontStackOutcome::ConfigOwned
            && receipt.proves_font_stack()
        {
            evidence.mark_not_applicable(key);
        } else if self.inherited_font_stack.typed_font_stack_requested() {
            evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
        }
        for (key, rule_index) in &self.text_fill_routes {
            if self.config_owns_text_fill {
                evidence.mark_not_applicable(key.clone());
                continue;
            }
            let Some(fill) = self
                .text_fill
                .as_ref()
                .filter(|fill| fill.rule_index() == *rule_index)
            else {
                evidence.mark_not_applicable(key.clone());
                continue;
            };
            if receipt.proves_text_fill(fill.css()) {
                evidence.mark_applied_with_capabilities(key.clone(), [fill.capability()]);
            } else if receipt.has_text_fill_terminal() {
                evidence.mark_residual(key.clone(), FamilyThemeResidualReason::UnsupportedPaint);
            } else {
                evidence.mark_not_applicable(key.clone());
            }
        }
        evidence
    }
}

/// Writer-owned proof for the Venn font stack and every visible local text role.
#[derive(Debug)]
pub(crate) struct VennTypographyThemeReceipt {
    expected: VennTypographyOccurrences,
    emitted: VennTypographyOccurrences,
    expected_font_family_css: Box<str>,
    emitted_font_family_css: Option<Box<str>>,
    expected_text_fill_css: Option<Box<str>>,
    stylesheet_text_fill_css: Option<Box<str>>,
    text_fill_terminal_count: usize,
    text_fill_terminal_matches: bool,
    stylesheet_selector_mask: u8,
    terminal_matches: bool,
}

impl VennTypographyThemeReceipt {
    fn new(
        expected: VennTypographyOccurrences,
        expected_font_family_css: &str,
        expected_text_fill_css: Option<&str>,
    ) -> Self {
        Self {
            expected,
            emitted: VennTypographyOccurrences::default(),
            expected_font_family_css: expected_font_family_css.into(),
            emitted_font_family_css: None,
            expected_text_fill_css: expected_text_fill_css.map(Into::into),
            stylesheet_text_fill_css: None,
            text_fill_terminal_count: 0,
            text_fill_terminal_matches: true,
            stylesheet_selector_mask: 0,
            terminal_matches: true,
        }
    }

    /// Build the exact Venn stylesheet and seal the single local font-stack writer event.
    pub(crate) fn stylesheet(
        &mut self,
        diagram_id: &str,
        title_fill: &str,
        set_text_color: &str,
    ) -> String {
        if self.emitted_font_family_css.is_some() {
            self.terminal_matches = false;
        }
        self.emitted_font_family_css = Some(self.expected_font_family_css.clone());
        self.stylesheet_text_fill_css = Some(set_text_color.into());
        self.stylesheet_selector_mask = 0b1111;
        let id = crate::svg::escape_css_identifier(diagram_id);
        let mut css = String::new();
        let _ = write!(
            css,
            "#{id} .{title_class}{{font-size:32px;fill:{title_fill};font-family:{font_family};}}\
#{id} .{circle_class} text{{font-size:48px;font-family:{font_family};}}\
#{id} .{intersection_class} text{{font-size:48px;fill:{set_text_color};font-family:{font_family};}}\
#{id} .{text_node_class}{{font-family:{font_family};color:{set_text_color};}}",
            title_class = super::VENN_TITLE_CLASS,
            circle_class = super::VENN_CIRCLE_CLASS,
            intersection_class = super::VENN_INTERSECTION_CLASS,
            text_node_class = super::VENN_TEXT_NODE_CLASS,
            font_family = self.expected_font_family_css,
        );
        css
    }

    pub(crate) fn record_title_text(&mut self, emitted_class: &str, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        self.emitted.titles = self.emitted.titles.saturating_add(1);
        self.terminal_matches &= emitted_class == super::VENN_TITLE_CLASS;
    }

    pub(crate) fn record_circle_label(
        &mut self,
        emitted_parent_class: &str,
        emitted_text_class: &str,
        text: &str,
    ) {
        if text.trim().is_empty() {
            return;
        }
        self.emitted.circle_labels = self.emitted.circle_labels.saturating_add(1);
        self.terminal_matches &= emitted_parent_class == super::VENN_CIRCLE_CLASS;
        self.terminal_matches &= emitted_text_class == super::VENN_AREA_LABEL_CLASS;
    }

    pub(crate) fn record_intersection_label(
        &mut self,
        emitted_parent_class: &str,
        emitted_text_class: &str,
        text: &str,
        source_owns_fill: bool,
        emitted_fill: &str,
    ) {
        if text.trim().is_empty() {
            return;
        }
        self.emitted.intersection_labels = self.emitted.intersection_labels.saturating_add(1);
        self.terminal_matches &= emitted_parent_class == super::VENN_INTERSECTION_CLASS;
        self.terminal_matches &= emitted_text_class == super::VENN_AREA_LABEL_CLASS;
        self.record_text_fill_terminal(source_owns_fill, emitted_fill);
    }

    pub(crate) fn record_text_node(
        &mut self,
        emitted_class: &str,
        text: &str,
        source_owns_fill: bool,
        emitted_fill: &str,
    ) {
        if text.trim().is_empty() {
            return;
        }
        self.emitted.text_nodes = self.emitted.text_nodes.saturating_add(1);
        self.terminal_matches &= emitted_class == super::VENN_TEXT_NODE_CLASS;
        self.record_text_fill_terminal(source_owns_fill, emitted_fill);
    }

    fn record_text_fill_terminal(&mut self, source_owns_fill: bool, emitted_fill: &str) {
        if source_owns_fill || self.expected_text_fill_css.is_none() {
            return;
        }
        self.text_fill_terminal_count = self.text_fill_terminal_count.saturating_add(1);
        self.text_fill_terminal_matches &=
            self.expected_text_fill_css.as_deref() == Some(emitted_fill);
    }

    fn proves_font_stack(&self) -> bool {
        self.emitted == self.expected
            && self.emitted_font_family_css.as_deref()
                == Some(self.expected_font_family_css.as_ref())
            && self.stylesheet_selector_mask == 0b1111
            && self.terminal_matches
    }

    fn has_text_fill_terminal(&self) -> bool {
        self.text_fill_terminal_count != 0
    }

    fn proves_text_fill(&self, expected_fill: &str) -> bool {
        self.has_text_fill_terminal()
            && self.expected_text_fill_css.as_deref() == Some(expected_fill)
            && self.stylesheet_text_fill_css.as_deref() == Some(expected_fill)
            && self.text_fill_terminal_matches
    }
}

/// Final Venn title fill shared by stylesheet emission, the title terminal, and evidence.
#[derive(Debug)]
pub(crate) struct VennTitleThemePlan {
    fill_css: Option<Box<str>>,
    typed_fill_capability: Option<ThemeCapability>,
    evidence: FamilyThemeEvidence,
    pending_fill_key: Option<FamilyThemeMechanismKey>,
    terminal_receipt: OnceLock<()>,
}

impl VennTitleThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        title: Option<&str>,
        work_meter: &OperationWorkMeter,
    ) -> crate::Result<Self> {
        let title_present = title.is_some_and(|title| !title.trim().is_empty());
        let Some(theme) = theme else {
            return Ok(Self::baseline());
        };

        let title_count = usize::from(title_present);
        let config_owns_fill = merman_core::__private::config_path_overrides_typed_default(
            effective_config,
            "themeVariables.vennTitleTextColor",
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
        let mut observations = BTreeMap::<usize, VennTitleRuleObservation>::new();
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

    pub(crate) fn baseline() -> Self {
        Self {
            fill_css: None,
            typed_fill_capability: None,
            evidence: FamilyThemeEvidence::default(),
            pending_fill_key: None,
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn fill_css(&self) -> Option<&str> {
        self.fill_css.as_deref()
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<VennTitleThemeReceipt> {
        self.fill_css
            .as_ref()
            .map(|fill_css| VennTitleThemeReceipt::new(fill_css.clone()))
    }

    pub(crate) fn record_terminal(&self, receipt: VennTitleThemeReceipt) -> bool {
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

/// Writer-owned proof that the selected title fill reached CSS and the real title terminal.
#[derive(Debug)]
pub(crate) struct VennTitleThemeReceipt {
    expected_fill: Box<str>,
    stylesheet_fill: Option<Box<str>>,
    stylesheet_class: Option<Box<str>>,
    inline_fill: Option<Box<str>>,
    inline_class: Option<Box<str>>,
    title_text_count: usize,
    terminal_matches: bool,
}

impl VennTitleThemeReceipt {
    fn new(expected_fill: Box<str>) -> Self {
        Self {
            expected_fill,
            stylesheet_fill: None,
            stylesheet_class: None,
            inline_fill: None,
            inline_class: None,
            title_text_count: 0,
            terminal_matches: true,
        }
    }

    pub(crate) fn record_stylesheet(&mut self, emitted_class: &str, emitted_fill: &str) {
        if self.stylesheet_fill.is_some() || self.stylesheet_class.is_some() {
            self.terminal_matches = false;
            return;
        }
        self.terminal_matches &= emitted_class == super::VENN_TITLE_CLASS;
        self.terminal_matches &= emitted_fill == self.expected_fill.as_ref();
        self.stylesheet_class = Some(emitted_class.into());
        self.stylesheet_fill = Some(emitted_fill.into());
    }

    pub(crate) fn record_title_text(&mut self, emitted_class: &str, emitted_fill: &str) {
        self.title_text_count = self.title_text_count.saturating_add(1);
        self.terminal_matches &= emitted_class == super::VENN_TITLE_CLASS;
        self.terminal_matches &= emitted_fill == self.expected_fill.as_ref();
        if self.inline_fill.is_some() || self.inline_class.is_some() {
            self.terminal_matches = false;
            return;
        }
        self.inline_class = Some(emitted_class.into());
        self.inline_fill = Some(emitted_fill.into());
    }

    fn proves(&self, expected_fill: &str) -> bool {
        self.expected_fill.as_ref() == expected_fill
            && self.stylesheet_fill.as_deref() == Some(expected_fill)
            && self.stylesheet_class.as_deref() == Some(super::VENN_TITLE_CLASS)
            && self.inline_fill.as_deref() == Some(expected_fill)
            && self.inline_class.as_deref() == Some(super::VENN_TITLE_CLASS)
            && self.title_text_count == 1
            && self.terminal_matches
    }
}

#[derive(Debug, Default)]
struct VennTitleRuleObservation {
    applicable: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    fill_pending: bool,
}

#[cfg(test)]
mod tests {
    use super::{
        VennTitleThemePlan, VennTitleThemeReceipt, VennTypographyOccurrences,
        VennTypographyThemeReceipt,
    };
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, OrdinalPalette, ThemeColorValue,
        ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
    };
    use crate::resources::RenderResourcePolicy;
    use merman_core::MermaidConfig;
    use serde_json::json;

    #[test]
    fn title_receipt_requires_matching_stylesheet_and_inline_terminal() {
        let mut complete = VennTitleThemeReceipt::new("#123456".into());
        complete.record_stylesheet(super::super::VENN_TITLE_CLASS, "#123456");
        complete.record_title_text(super::super::VENN_TITLE_CLASS, "#123456");
        assert!(complete.proves("#123456"));

        let mut missing_inline = VennTitleThemeReceipt::new("#123456".into());
        missing_inline.record_stylesheet(super::super::VENN_TITLE_CLASS, "#123456");
        assert!(!missing_inline.proves("#123456"));

        let mut wrong_stylesheet = VennTitleThemeReceipt::new("#123456".into());
        wrong_stylesheet.record_stylesheet(super::super::VENN_TITLE_CLASS, "#abcdef");
        wrong_stylesheet.record_title_text(super::super::VENN_TITLE_CLASS, "#123456");
        assert!(!wrong_stylesheet.proves("#123456"));

        let mut wrong_inline = VennTitleThemeReceipt::new("#123456".into());
        wrong_inline.record_stylesheet(super::super::VENN_TITLE_CLASS, "#123456");
        wrong_inline.record_title_text(super::super::VENN_TITLE_CLASS, "transparent");
        assert!(!wrong_inline.proves("#123456"));

        let mut duplicate_inline = VennTitleThemeReceipt::new("#123456".into());
        duplicate_inline.record_stylesheet(super::super::VENN_TITLE_CLASS, "#123456");
        duplicate_inline.record_title_text(super::super::VENN_TITLE_CLASS, "#123456");
        duplicate_inline.record_title_text(super::super::VENN_TITLE_CLASS, "#123456");
        assert!(!duplicate_inline.proves("#123456"));
    }

    #[test]
    fn typography_receipt_requires_all_local_selectors_and_visible_roles() {
        let expected = VennTypographyOccurrences {
            titles: 1,
            circle_labels: 2,
            intersection_labels: 1,
            text_nodes: 2,
        };
        let mut complete = VennTypographyThemeReceipt::new(expected, "VennSans", None);
        let stylesheet = complete.stylesheet("venn-test", "#123456", "#abcdef");
        for selector in [
            "#venn-test .venn-title{",
            "#venn-test .venn-circle text{",
            "#venn-test .venn-intersection text{",
            "#venn-test .venn-text-node{",
        ] {
            assert!(
                stylesheet.contains(selector),
                "missing Venn selector `{selector}`"
            );
        }
        complete.record_title_text(super::super::VENN_TITLE_CLASS, "Title");
        for _ in 0..2 {
            complete.record_circle_label(
                super::super::VENN_CIRCLE_CLASS,
                super::super::VENN_AREA_LABEL_CLASS,
                "Set",
            );
        }
        complete.record_intersection_label(
            super::super::VENN_INTERSECTION_CLASS,
            super::super::VENN_AREA_LABEL_CLASS,
            "Shared",
            false,
            "#abcdef",
        );
        for _ in 0..2 {
            complete.record_text_node(
                super::super::VENN_TEXT_NODE_CLASS,
                "Nested",
                false,
                "#abcdef",
            );
        }
        assert!(complete.proves_font_stack());

        let mut missing_role = VennTypographyThemeReceipt::new(expected, "VennSans", None);
        missing_role.stylesheet("venn-test", "#123456", "#abcdef");
        missing_role.record_title_text(super::super::VENN_TITLE_CLASS, "Title");
        missing_role.record_circle_label(
            super::super::VENN_CIRCLE_CLASS,
            super::super::VENN_AREA_LABEL_CLASS,
            "Set",
        );
        missing_role.record_intersection_label(
            super::super::VENN_INTERSECTION_CLASS,
            super::super::VENN_AREA_LABEL_CLASS,
            "Shared",
            false,
            "#abcdef",
        );
        for _ in 0..2 {
            missing_role.record_text_node(
                super::super::VENN_TEXT_NODE_CLASS,
                "Nested",
                false,
                "#abcdef",
            );
        }
        assert!(!missing_role.proves_font_stack());

        let mut duplicate_stylesheet = VennTypographyThemeReceipt::new(expected, "VennSans", None);
        duplicate_stylesheet.stylesheet("venn-test", "#123456", "#abcdef");
        duplicate_stylesheet.stylesheet("venn-test", "#123456", "#abcdef");
        assert!(!duplicate_stylesheet.proves_font_stack());
    }

    #[test]
    fn typed_title_fill_shadows_unsupported_ordinal_palette() {
        let palette = OrdinalPalette::new([
            ThemeColorValue::parse("#abcdef").expect("valid Venn ordinal palette color")
        ])
        .expect("non-empty Venn ordinal palette");
        let resolved = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default()
                        .with_rule(ThemeRule::new(
                            ThemeTarget::Title,
                            ThemeStylePatch::default().with_fill(
                                CanvasPaint::solid("#123456").expect("valid Venn title fill"),
                            ),
                        ))
                        .with_ordinal_palette(ThemeTarget::Title, palette),
                ),
            )
            .expect("compile Venn title fill and palette theme")
            .resolve(DiagramFamilyId::VENN);
        let meter = crate::resources::OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let config = MermaidConfig::from_value(json!({}));
        let plan = VennTitleThemePlan::resolve(Some(&resolved), &config, Some("Title"), &meter)
            .expect("resolve Venn title theme");
        let mut receipt = plan.begin_terminal_receipt().expect("typed title receipt");
        receipt.record_stylesheet(super::super::VENN_TITLE_CLASS, "#123456");
        receipt.record_title_text(super::super::VENN_TITLE_CLASS, "#123456");
        assert!(plan.record_terminal(receipt));

        let evidence = plan.finish_evidence();
        assert_eq!(evidence.required_mechanisms().len(), 2);
        assert_eq!(evidence.applied().len(), 1);
        assert_eq!(evidence.not_applicable_mechanisms().len(), 1);
        assert!(evidence.residuals().is_empty());
    }
}
