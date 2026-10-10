use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_core::diagrams::zenuml::ZenumlDiagramRenderModel;

use super::{
    ZENUML_FRAME_TITLE_CLASS, ZENUML_FRAME_TITLE_SELECTOR, ZenumlDiagramLayout,
    resolve_zenuml_title,
};
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemePaintKind,
    FamilyThemeRuleFacet, FamilyThemeSelectorShape, ResolvedDiagramTheme, ThemeCapability,
    ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    TerminalVariantDomain, UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains,
    resolve_direct_static_fill, resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::resources::OperationWorkMeter;

/// Final ZenUML frame-title fill shared by stylesheet emission and terminal evidence.
#[derive(Debug)]
pub(crate) struct ZenumlTitleThemePlan {
    fill_css: Option<Box<str>>,
    typed_fill_capability: Option<ThemeCapability>,
    evidence: FamilyThemeEvidence,
    pending_fill_key: Option<FamilyThemeMechanismKey>,
    terminal_receipt: OnceLock<()>,
}

impl ZenumlTitleThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        model: &ZenumlDiagramRenderModel,
        layout: &ZenumlDiagramLayout,
        diagram_title: Option<&str>,
        work_meter: &OperationWorkMeter,
    ) -> crate::Result<Self> {
        let title_present = resolve_zenuml_title(model, diagram_title).is_some();
        let non_title_text_present = has_visible_non_title_text(layout);
        let Some(theme) = theme else {
            return Ok(Self::baseline());
        };

        let title_count = usize::from(title_present);
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

        let typed_fill = style.as_ref().and_then(|style| {
            resolve_direct_static_fill(
                theme,
                style,
                &[ThemeTarget::Title],
                DirectStaticSelectorDomain::Unqualified,
            )
        });
        let (fill_css, typed_fill_rule, typed_fill_capability) =
            typed_fill.map_or((None, None, None), |fill| {
                let (css, rule_index, capability) = fill.into_parts();
                (Some(css), Some(rule_index), Some(capability))
            });

        let mut evidence = FamilyThemeEvidence::from_theme(Some(theme));
        let mut observations = BTreeMap::<usize, ZenumlTitleRuleObservation>::new();
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
                    match (route.disposition(), selector, facet) {
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeSelectorShape::Static { variant: None },
                            FamilyThemeRuleFacet::Fill(
                                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                            ),
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
                    // Reconciled below against the final Title style winner.  A qualified or
                    // explicit fill can suppress the fallback palette even when a title exists.
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Title,
                    ..
                } => {
                    // Reconciled below against the final Title effect winner.
                }
                FamilyThemeMechanism::BaseTypography(_) => {
                    let key = theme.family_mechanism_key(route);
                    if !title_present && !non_title_text_present {
                        evidence.mark_not_applicable(key);
                    } else if route.disposition() == FamilyThemeDisposition::Unsupported {
                        evidence
                            .mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
                    }
                }
                FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::Text,
                    facet,
                    ..
                } => {
                    let key = theme.family_mechanism_key(route);
                    if !non_title_text_present {
                        evidence.mark_not_applicable(key);
                    } else if route.disposition() == FamilyThemeDisposition::Unsupported {
                        evidence.mark_residual(key, unsupported_residual_for_facet(facet));
                    }
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Text,
                } => {
                    let key = theme.family_mechanism_key(route);
                    if !non_title_text_present {
                        evidence.mark_not_applicable(key);
                    } else if route.disposition() == FamilyThemeDisposition::Unsupported {
                        evidence.mark_residual(
                            key,
                            FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                        );
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Text,
                    ..
                } => {
                    let key = theme.family_mechanism_key(route);
                    if !non_title_text_present {
                        evidence.mark_not_applicable(key);
                    } else if route.disposition() == FamilyThemeDisposition::Unsupported {
                        evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedEffect);
                    }
                }
                FamilyThemeMechanism::RuleFacet { .. }
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
            )],
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
                // Mixed rules remain unaccounted until every winning facet has a terminal owner.
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

    fn baseline() -> Self {
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

    pub(crate) fn begin_terminal_receipt(&self) -> Option<ZenumlTitleThemeReceipt> {
        self.fill_css
            .as_ref()
            .map(|fill_css| ZenumlTitleThemeReceipt::new(fill_css.clone()))
    }

    pub(crate) fn record_terminal(&self, receipt: ZenumlTitleThemeReceipt) -> bool {
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

fn has_visible_non_title_text(layout: &ZenumlDiagramLayout) -> bool {
    layout
        .groups
        .iter()
        .any(|group| !group.name.trim().is_empty())
        || layout.participants.iter().any(|participant| {
            !participant.is_starter
                && (!participant.label.trim().is_empty()
                    || participant
                        .stereotype
                        .as_deref()
                        .is_some_and(|text| !text.trim().is_empty())
                    || participant
                        .emoji
                        .as_deref()
                        .is_some_and(|text| !text.trim().is_empty()))
        })
        || layout
            .messages
            .iter()
            .any(|message| !message.label.trim().is_empty() || !message.number.trim().is_empty())
        || layout
            .self_calls
            .iter()
            .any(|message| !message.label.trim().is_empty() || !message.number.trim().is_empty())
        || layout.creations.iter().any(|creation| {
            !creation.message.label.trim().is_empty() || !creation.message.number.trim().is_empty()
        })
        || layout.returns.iter().any(|returned| {
            !returned.label.trim().is_empty()
                || (!returned.is_self && !returned.number.trim().is_empty())
        })
        || !layout.fragments.is_empty()
        || layout
            .dividers
            .iter()
            .any(|divider| !divider.label.trim_matches('=').trim().is_empty())
        || layout
            .comments
            .iter()
            .any(|comment| !comment.text.trim().is_empty())
}

/// Writer-owned proof that Title.fill reached the stylesheet and exactly one `text.frame-title`.
#[derive(Debug)]
pub(crate) struct ZenumlTitleThemeReceipt {
    expected_fill: Box<str>,
    stylesheet_fill: Option<Box<str>>,
    stylesheet_selector: Option<Box<str>>,
    frame_title_text_count: usize,
    terminal_matches: bool,
}

impl ZenumlTitleThemeReceipt {
    fn new(expected_fill: Box<str>) -> Self {
        Self {
            expected_fill,
            stylesheet_fill: None,
            stylesheet_selector: None,
            frame_title_text_count: 0,
            terminal_matches: true,
        }
    }

    pub(crate) fn record_stylesheet_writer(&mut self, emitted_selector: &str, emitted_fill: &str) {
        if self.stylesheet_fill.is_some() || self.stylesheet_selector.is_some() {
            self.terminal_matches = false;
            return;
        }
        self.terminal_matches &= emitted_selector == ZENUML_FRAME_TITLE_SELECTOR;
        self.terminal_matches &= emitted_fill == self.expected_fill.as_ref();
        self.stylesheet_selector = Some(emitted_selector.into());
        self.stylesheet_fill = Some(emitted_fill.into());
    }

    pub(crate) fn record_frame_title_text_writer(
        &mut self,
        emitted_element: &str,
        emitted_class: &str,
        text: &str,
    ) {
        self.terminal_matches &= emitted_element == "text";
        self.terminal_matches &= emitted_class == ZENUML_FRAME_TITLE_CLASS;
        self.terminal_matches &= !text.is_empty();
        if !text.is_empty() {
            self.frame_title_text_count = self.frame_title_text_count.saturating_add(1);
        }
    }

    fn proves(&self, expected_fill: &str) -> bool {
        self.expected_fill.as_ref() == expected_fill
            && self.stylesheet_fill.as_deref() == Some(expected_fill)
            && self.stylesheet_selector.as_deref() == Some(ZENUML_FRAME_TITLE_SELECTOR)
            && self.frame_title_text_count == 1
            && self.terminal_matches
    }
}

#[derive(Debug, Default)]
struct ZenumlTitleRuleObservation {
    applicable: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    fill_pending: bool,
}

#[cfg(test)]
mod tests {
    use super::ZenumlTitleThemeReceipt;

    #[test]
    fn title_receipt_requires_matching_css_and_one_text_frame_title_writer_event() {
        let mut complete = ZenumlTitleThemeReceipt::new("#123456".into());
        complete.record_stylesheet_writer(super::ZENUML_FRAME_TITLE_SELECTOR, "#123456");
        complete.record_frame_title_text_writer(
            "text",
            super::ZENUML_FRAME_TITLE_CLASS,
            "Themed ZenUML",
        );
        assert!(complete.proves("#123456"));

        let mut missing_text = ZenumlTitleThemeReceipt::new("#123456".into());
        missing_text.record_stylesheet_writer(super::ZENUML_FRAME_TITLE_SELECTOR, "#123456");
        assert!(!missing_text.proves("#123456"));

        let mut wrong_stylesheet = ZenumlTitleThemeReceipt::new("#123456".into());
        wrong_stylesheet.record_stylesheet_writer(super::ZENUML_FRAME_TITLE_SELECTOR, "#abcdef");
        wrong_stylesheet.record_frame_title_text_writer(
            "text",
            super::ZENUML_FRAME_TITLE_CLASS,
            "Themed ZenUML",
        );
        assert!(!wrong_stylesheet.proves("#123456"));

        let mut wrong_terminal = ZenumlTitleThemeReceipt::new("#123456".into());
        wrong_terminal.record_stylesheet_writer(super::ZENUML_FRAME_TITLE_SELECTOR, "#123456");
        wrong_terminal.record_frame_title_text_writer("text", "participant-label", "Not a title");
        assert!(!wrong_terminal.proves("#123456"));

        let mut duplicate_text = ZenumlTitleThemeReceipt::new("#123456".into());
        duplicate_text.record_stylesheet_writer(super::ZENUML_FRAME_TITLE_SELECTOR, "#123456");
        duplicate_text.record_frame_title_text_writer(
            "text",
            super::ZENUML_FRAME_TITLE_CLASS,
            "First",
        );
        duplicate_text.record_frame_title_text_writer(
            "text",
            super::ZENUML_FRAME_TITLE_CLASS,
            "Second",
        );
        assert!(!duplicate_text.proves("#123456"));
    }
}
