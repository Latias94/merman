use std::collections::{BTreeMap, BTreeSet};

use merman_model::families::pie::PieDiagramModel;
use merman_model::presentation::PresentationConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    ResolvedDiagramTheme, ResolvedStyleProperty, Specified, ThemeCapability, ThemeTarget,
    ThemeTypographyProperty, ThemeVariant,
};
use crate::family::{
    DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeOutcomes,
    FamilyThemeResidualReason, InheritedFontStackPlan, TerminalVariantDomain,
    UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
    resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

use super::PieCssBinding;

/// Match the writer's entity decoding and XML scalar filtering before checking visibility.
fn has_visible_pie_text(text: &str) -> bool {
    crate::entities::decode_mermaid_entities_for_render_text(text)
        .chars()
        .any(|ch| crate::xml::is_xml_1_0_char(ch) && !ch.is_whitespace())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PieTextRole {
    Title,
    Section,
    Legend,
}

impl PieTextRole {
    pub(crate) const ALL: [Self; 3] = [Self::Title, Self::Section, Self::Legend];

    pub(crate) const fn index(self) -> usize {
        self as usize
    }

    pub(crate) const fn selector(self) -> &'static str {
        match self {
            Self::Title => super::PIE_TITLE_CLASS,
            Self::Section => "slice",
            Self::Legend => "legend text",
        }
    }

    const fn target(self) -> ThemeTarget {
        match self {
            Self::Title => ThemeTarget::Title,
            Self::Section => ThemeTarget::Text,
            Self::Legend => ThemeTarget::Legend,
        }
    }

    const fn fill_path(self) -> &'static str {
        match self {
            Self::Title => "themeVariables.pieTitleTextColor",
            Self::Section => "themeVariables.pieSectionTextColor",
            Self::Legend => "themeVariables.pieLegendTextColor",
        }
    }

    const fn size_path(self) -> &'static str {
        match self {
            Self::Title => "themeVariables.pieTitleTextSize",
            Self::Section => "themeVariables.pieSectionTextSize",
            Self::Legend => "themeVariables.pieLegendTextSize",
        }
    }
}

/// Values returned only after the role's actual stylesheet write passes its sink checkpoint.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PieTextCssEmission<'a> {
    pub(crate) selector: &'a str,
    pub(crate) font_family: &'a str,
    pub(crate) font_size: &'a str,
    pub(crate) base_font_size: &'a str,
    pub(crate) nested_font_size: &'a str,
    pub(crate) fill: &'a str,
    pub(crate) font_weight: Option<&'a str>,
    pub(crate) font_style: Option<&'a str>,
}

/// Pie's three independent text surfaces share resolution, measurement inputs, and receipts.
/// A Text rule selects percentage labels; it does not implicitly style titles or legends.
#[derive(Debug)]
pub(super) struct PiePreparedText {
    expected: [Vec<String>; 3],
    inherited_font_stack: InheritedFontStackPlan,
    base_size_requested: bool,
    base_size_active: bool,
    requested: bool,
    evidence: FamilyThemeOutcomes,
    pending: [BTreeMap<usize, BTreeSet<ThemeCapability>>; 3],
}

impl PiePreparedText {
    pub(super) fn new(
        model: &PieDiagramModel,
        title: Option<&str>,
        config: &PresentationConfig,
        theme: Option<&ResolvedDiagramTheme>,
    ) -> (Self, PieCssBinding) {
        let inherited_font_stack = InheritedFontStackPlan::resolve_property_local(theme, config);
        let base_size_requested = theme.is_some_and(|theme| {
            theme.family_mechanism_routes().iter().any(|route| {
                route.mechanism()
                    == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
                    && route.disposition() == FamilyThemeDisposition::TypedAdapter
            })
        });
        let base_size_active = base_size_requested
            && !["themeVariables.fontSize", "fontSize"]
                .into_iter()
                .any(|path| config.config_path_overrides_typed_default(path));
        let mut css = PieCssBinding::new(
            config.as_value(),
            Some(inherited_font_stack.font_family_css()),
        );
        if let Some(theme) = theme.filter(|_| base_size_active) {
            css.bind_base_font_size(config.as_value(), theme.typography().font_size_px());
        }
        if !inherited_font_stack.typed_font_stack_requested() {
            css.common = css
                .common
                .with_configured_root_font_family(config.as_value());
        }

        let requested = inherited_font_stack.typography_requested()
            || base_size_requested
            || theme.is_some_and(|theme| {
                theme
                    .family_mechanism_routes()
                    .iter()
                    .any(|route| match route.mechanism() {
                        FamilyThemeMechanism::RuleFacet { target, .. }
                        | FamilyThemeMechanism::EffectBinding { target, .. }
                        | FamilyThemeMechanism::OrdinalPalette { target } => {
                            PieTextRole::ALL.iter().any(|role| role.target() == target)
                        }
                        FamilyThemeMechanism::BaseTypography(_) => false,
                    })
            });
        let expected = if requested {
            let (total, sections) = crate::pie::visible_pie_sections(model);
            [
                vec![title.unwrap_or_default().to_owned()],
                sections
                    .iter()
                    .map(|section| {
                        format!(
                            "{}%",
                            ((100.0 * (section.value / total)).max(0.0)).round() as i64
                        )
                    })
                    .collect(),
                model
                    .sections
                    .iter()
                    .map(|section| {
                        crate::pie::pie_legend_text(&section.label, section.value, model.show_data)
                    })
                    .collect(),
            ]
        } else {
            std::array::from_fn(|_| Vec::new())
        };
        (
            Self {
                expected,
                inherited_font_stack,
                base_size_requested,
                base_size_active,
                requested,
                evidence: FamilyThemeOutcomes::default(),
                pending: std::array::from_fn(|_| BTreeMap::new()),
            },
            css,
        )
    }

    pub(super) fn resolve_rules(
        &mut self,
        css: &mut PieCssBinding,
        config: &PresentationConfig,
        theme: &ResolvedDiagramTheme,
        meter: &OperationWorkMeter,
    ) -> Result<(), OperationWorkError> {
        if !self.requested {
            return Ok(());
        }
        for role in PieTextRole::ALL {
            let target = role.target();
            // Shared CSS uses static winners, while evidence observes the actual visible ordinals.
            let style = theme.style_with_work_meter(target, ThemeVariant::Default, None, meter)?;
            let occurrences = self.expected[role.index()]
                .iter()
                .enumerate()
                .filter(|(_, text)| has_visible_pie_text(text))
                .map(|(index, _)| (index + 1, ThemeVariant::Default))
                .collect::<Vec<_>>();
            meter.charge(theme.family_mechanism_routes().len())?;
            let has_ordinals = theme.family_rules().any(|(_, rule)| {
                rule.target() == target
                    && rule.ordinal().is_some()
                    && matches!(rule.variant(), None | Some(ThemeVariant::Default))
            });
            let mut winners = BTreeSet::new();
            if has_ordinals {
                for &(ordinal, variant) in &occurrences {
                    meter.charge(1)?;
                    winners.extend(
                        theme
                            .style_with_work_meter(target, variant, Some(ordinal), meter)?
                            .winner_rule_properties()
                            .map(|(property, origin)| (origin.rule_index(), property)),
                    );
                }
            } else if !occurrences.is_empty() {
                winners.extend(
                    style
                        .winner_rule_properties()
                        .map(|(property, origin)| (origin.rule_index(), property)),
                );
            }

            let fill_owned = config.config_paint_overrides_typed_default(role.fill_path());
            let size_owned = config.config_path_overrides_typed_default(role.size_path());
            let mut owners = BTreeMap::new();
            if !fill_owned
                && let Some(fill) = resolve_direct_static_fill(
                    theme,
                    &style,
                    &[target],
                    DirectStaticSelectorDomain::Default,
                )
            {
                css.bind_text_fill(role, fill.css());
                owners.insert(
                    ResolvedStyleProperty::Fill,
                    (fill.rule_index(), fill.capability()),
                );
            }
            let typography = style.typography_resolution();
            for property in [
                ThemeTypographyProperty::FontSize,
                ThemeTypographyProperty::FontWeight,
                ThemeTypographyProperty::FontStyle,
            ] {
                if property == ThemeTypographyProperty::FontSize && size_owned {
                    continue;
                }
                let Some(origin) = typography.winner(property) else {
                    continue;
                };
                if origin.target() != target
                    || origin.ordinal().is_some()
                    || !matches!(origin.variant(), None | Some(ThemeVariant::Default))
                    || theme.rule_facet_disposition(
                        origin.rule_index(),
                        FamilyThemeRuleFacet::Typography(property),
                    ) != Some(FamilyThemeDisposition::TypedAdapter)
                {
                    continue;
                }
                let applied = match property {
                    ThemeTypographyProperty::FontSize => {
                        if let Specified::Value(size) = typography.patch().font_size_px {
                            css.bind_text_size(role, &format!("{size}px"));
                            true
                        } else {
                            false
                        }
                    }
                    ThemeTypographyProperty::FontWeight => {
                        if let Specified::Value(weight) = typography.patch().font_weight {
                            css.bind_text_weight(role, weight.to_string());
                            true
                        } else {
                            false
                        }
                    }
                    ThemeTypographyProperty::FontStyle => {
                        if let Specified::Value(style) = typography.patch().font_style {
                            css.bind_text_style(role, style.id().to_owned());
                            true
                        } else {
                            false
                        }
                    }
                    _ => unreachable!("bounded Pie role typography"),
                };
                if applied {
                    owners.insert(
                        ResolvedStyleProperty::Typography(property),
                        (origin.rule_index(), ThemeCapability::Typography),
                    );
                }
            }

            let mut observations = BTreeMap::<usize, TextRuleObservation>::new();
            for route in theme.family_mechanism_routes().iter().copied() {
                let FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: rule_target,
                    facet,
                    ..
                } = route.mechanism()
                else {
                    continue;
                };
                if rule_target != target {
                    continue;
                }
                let observation = observations.entry(rule_index).or_default();
                let property = resolved_style_property_for_facet(facet);
                if !winners.contains(&(rule_index, property)) {
                    continue;
                }
                if (fill_owned && property == ResolvedStyleProperty::Fill)
                    || (size_owned
                        && property
                            == ResolvedStyleProperty::Typography(ThemeTypographyProperty::FontSize))
                {
                    continue;
                }
                if route.disposition() == FamilyThemeDisposition::TypedAdapter
                    && let Some(&(owner, capability)) = owners.get(&property)
                    && owner == rule_index
                {
                    observation.capabilities.insert(capability);
                } else {
                    observation
                        .residual
                        .get_or_insert(unsupported_residual_for_facet(facet));
                }
            }
            for (index, observation) in observations {
                let key = FamilyThemeMechanismKey::Rule { index, target };
                if let Some(reason) = observation.residual {
                    self.evidence.mark_residual(key, reason);
                } else if observation.capabilities.is_empty() {
                    self.evidence.mark_not_applicable(key);
                } else {
                    self.pending[role.index()].insert(index, observation.capabilities);
                }
            }
            let source_owned_fill = vec![fill_owned; self.expected[role.index()].len()];
            reconcile_unsupported_terminal_domains(
                theme,
                &mut self.evidence,
                &[UnsupportedTerminalDomain::fallbacks_only(
                    target,
                    TerminalVariantDomain::indexed(&occurrences),
                )
                .with_source_owned_fill(&source_owned_fill)],
                meter,
            )?;
        }
        Ok(())
    }

    pub(super) fn requested(&self) -> bool {
        self.requested
    }

    pub(super) fn record_evidence(
        &self,
        css: &PieCssBinding,
        terminal: Option<&PieTextReceipt>,
        evidence: &mut FamilyThemeEvidence,
    ) {
        evidence.merge_borrowed(&self.evidence);
        let visible =
            |role: PieTextRole| terminal.is_some_and(|terminal| terminal.visible[role.index()] > 0);
        let any_visible = PieTextRole::ALL.into_iter().any(visible);
        self.inherited_font_stack
            .record_unsupported_typography_evidence(evidence, terminal.is_none() || any_visible);
        for (property, requested, active) in [
            (
                ThemeTypographyProperty::FontStack,
                self.inherited_font_stack.typed_font_stack_requested(),
                self.inherited_font_stack.typed_font_stack_active(),
            ),
            (
                ThemeTypographyProperty::FontSize,
                self.base_size_requested,
                self.base_size_active,
            ),
        ] {
            if !requested {
                continue;
            }
            let key = FamilyThemeMechanismKey::Typography(property);
            let Some(_) = terminal else {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
                continue;
            };
            let unmeasurable = active
                && PieTextRole::ALL
                    .into_iter()
                    .any(|role| visible(role) && css.text_size_resolution(role).is_none());
            let consumed = active
                && PieTextRole::ALL.into_iter().any(|role| {
                    visible(role)
                        && (property == ThemeTypographyProperty::FontStack
                            || css
                                .text_size_resolution(role)
                                .is_some_and(|(_, inherited)| inherited))
                });
            if unmeasurable {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
            } else if consumed {
                evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
            } else {
                evidence.mark_not_applicable(key);
            }
        }
        if terminal.is_none() {
            return;
        }
        for role in PieTextRole::ALL {
            for (&index, capabilities) in &self.pending[role.index()] {
                let key = FamilyThemeMechanismKey::Rule {
                    index,
                    target: role.target(),
                };
                if !visible(role) {
                    evidence.mark_not_applicable(key);
                } else if capabilities.contains(&ThemeCapability::Typography)
                    && css.text_size_resolution(role).is_none()
                {
                    evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
                } else {
                    evidence.mark_applied_with_capabilities(key, capabilities.iter().copied());
                }
            }
        }
    }
}

#[derive(Debug, Default)]
struct TextRuleObservation {
    residual: Option<FamilyThemeResidualReason>,
    capabilities: BTreeSet<ThemeCapability>,
}

/// The writer must prove all final role styles and every ordered text occurrence, even empty ones.
#[derive(Debug, Default)]
pub(super) struct PieTextReceipt {
    css_recorded: [bool; 3],
    next: [usize; 3],
    visible: [usize; 3],
    invalid: bool,
}

impl PieTextReceipt {
    pub(super) fn record_css(
        &mut self,
        css: &PieCssBinding,
        role: PieTextRole,
        emitted: PieTextCssEmission<'_>,
    ) {
        let index = role.index();
        self.invalid |= self.css_recorded[index]
            || emitted.selector != role.selector()
            || emitted.font_family != css.common.font_family()
            || emitted.font_size != css.text_size(role)
            || emitted.base_font_size != css.common.font_size_css()
            || emitted.nested_font_size != css.common.font_size_css()
            || emitted.fill != css.text_fill(role)
            || emitted.font_weight != css.text_weight(role)
            || emitted.font_style != css.text_style(role);
        self.css_recorded[index] = true;
    }

    pub(super) fn record_text(
        &mut self,
        plan: &PiePreparedText,
        role: PieTextRole,
        index: usize,
        selector: Option<&str>,
        text: Option<&str>,
    ) {
        let role_index = role.index();
        self.invalid |= index != self.next[role_index]
            || selector != Some(role.selector())
            || plan.expected[role_index].get(index).map(String::as_str)
                != Some(text.unwrap_or_default());
        self.next[role_index] = self.next[role_index].saturating_add(1);
        self.visible[role_index] += usize::from(text.is_some_and(has_visible_pie_text));
    }

    pub(super) fn proves_complete(&self, plan: &PiePreparedText) -> bool {
        !self.invalid
            && PieTextRole::ALL.into_iter().all(|role| {
                self.css_recorded[role.index()]
                    && self.next[role.index()] == plan.expected[role.index()].len()
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        DiagramThemeCompiler, DiagramThemeSpec, FontStack, ThemeTextStyle, TypographySpec,
    };
    use merman_model::families::pie::PieSection;

    #[test]
    fn pie_text_visibility_decodes_entities_and_filters_forbidden_xml_scalars() {
        for text in [
            "",
            "#32;",
            "#nbsp;",
            "#9;",
            "#31;",
            "\u{1}",
            "\u{fffe}",
            "#32;\u{1}#nbsp;",
        ] {
            assert!(!has_visible_pie_text(text), "{text:?}");
        }
        for text in ["Oak", "#32;Oak\u{1}", "#65;", " [0]"] {
            assert!(has_visible_pie_text(text), "{text:?}");
        }
    }

    #[test]
    fn text_receipts_reject_missing_duplicate_reordered_and_mismatched_terminal_values() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_typography(
                    TypographySpec::default().with_default(
                        ThemeTextStyle::default()
                            .with_font_stack(FontStack::single("Pie Receipt").unwrap()),
                    ),
                ),
            )
            .unwrap()
            .resolve(DiagramFamilyId::PIE);
        let mut model = PieDiagramModel::default();
        model.title = Some("Title".to_owned());
        model.sections = ["#32;\u{1}", "Beta"]
            .map(|label| PieSection {
                label: label.to_owned(),
                value: 1.0,
            })
            .to_vec();
        let (plan, css) = PiePreparedText::new(
            &model,
            model.title.as_deref(),
            &PresentationConfig::empty_object(),
            Some(&theme),
        );
        for case in [
            "complete",
            "missing-css",
            "duplicate-css",
            "selector",
            "size",
            "base-size",
            "family",
            "fill",
            "weight",
            "style",
            "missing-text",
            "wrong-text",
            "reordered-text",
            "extra-text",
        ] {
            let mut receipt = PieTextReceipt::default();
            for role in PieTextRole::ALL {
                let mut emitted = PieTextCssEmission {
                    selector: role.selector(),
                    font_family: css.common.font_family(),
                    font_size: css.text_size(role),
                    base_font_size: css.common.font_size_css(),
                    nested_font_size: css.common.font_size_css(),
                    fill: css.text_fill(role),
                    font_weight: css.text_weight(role),
                    font_style: css.text_style(role),
                };
                if role == PieTextRole::Legend {
                    match case {
                        "missing-css" => continue,
                        "selector" => emitted.selector = "wrong text",
                        "size" => emitted.font_size = "99px",
                        "base-size" => emitted.base_font_size = "99px",
                        "family" => emitted.font_family = "Wrong Family",
                        "fill" => emitted.fill = "#abcdef",
                        "weight" => emitted.font_weight = Some("900"),
                        "style" => emitted.font_style = Some("italic"),
                        "duplicate-css" => receipt.record_css(&css, role, emitted),
                        _ => {}
                    }
                }
                receipt.record_css(&css, role, emitted);
            }
            for role in PieTextRole::ALL {
                for (index, text) in plan.expected[role.index()].iter().enumerate() {
                    if role == PieTextRole::Legend && index == 0 && case == "missing-text" {
                        continue;
                    }
                    let emitted_text = if role == PieTextRole::Legend && case == "wrong-text" {
                        "Wrong"
                    } else {
                        text
                    };
                    let emitted_index = if role == PieTextRole::Legend && case == "reordered-text" {
                        1 - index
                    } else {
                        index
                    };
                    receipt.record_text(
                        &plan,
                        role,
                        emitted_index,
                        Some(role.selector()),
                        Some(emitted_text),
                    );
                }
            }
            if case == "extra-text" {
                receipt.record_text(
                    &plan,
                    PieTextRole::Title,
                    0,
                    Some(PieTextRole::Title.selector()),
                    Some("Title"),
                );
            }
            assert_eq!(receipt.proves_complete(&plan), case == "complete", "{case}");
            if case == "complete" {
                assert_eq!(receipt.visible[PieTextRole::Legend.index()], 1);
            }
        }
    }
}
