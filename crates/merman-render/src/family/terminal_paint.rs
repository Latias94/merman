use std::collections::{BTreeMap, BTreeSet};

use merman_model::presentation::PresentationConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    ResolvedDiagramTheme, ResolvedStyleProperty, ResolvedThemeStyle, Specified, ThemeCapability,
    ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectStaticSelectorDomain, FamilyThemeOutcomes, FamilyThemeResidualReason,
    resolve_direct_static_fill, resolve_direct_static_stroke, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::resources::OperationWorkMeter;

#[derive(Debug)]
pub(crate) struct TerminalPaint {
    css: Box<str>,
    key: FamilyThemeMechanismKey,
}

impl TerminalPaint {
    pub(crate) fn css(&self) -> &str {
        &self.css
    }
}

#[derive(Debug, Default)]
struct Observation {
    expected: usize,
    emitted: usize,
    capabilities: BTreeSet<ThemeCapability>,
    residual: Option<FamilyThemeResidualReason>,
}

/// Resolve colors at the existing writer events, using the actual semantic occurrence.
/// The family supplies its closed target set; paints are acknowledged only after emission.
pub(crate) struct TerminalPaintPlan<'a> {
    theme: Option<&'a ResolvedDiagramTheme>,
    config: &'a PresentationConfig,
    owned_targets: &'a [ThemeTarget],
    work_meter: &'a OperationWorkMeter,
    observations: BTreeMap<FamilyThemeMechanismKey, Observation>,
}

impl<'a> TerminalPaintPlan<'a> {
    pub(crate) fn charge(&self, units: usize) -> crate::Result<()> {
        if self.theme.is_some() {
            self.work_meter.charge(units)?;
        }
        Ok(())
    }

    pub(crate) fn new(
        theme: Option<&'a ResolvedDiagramTheme>,
        config: &'a PresentationConfig,
        owned_targets: &'a [ThemeTarget],
        work_meter: &'a OperationWorkMeter,
    ) -> crate::Result<Self> {
        let mut observations = BTreeMap::new();
        if let Some(theme) = theme {
            work_meter.charge(theme.family_mechanism_routes().len())?;
            for route in theme.family_mechanism_routes().iter().copied() {
                let target = match route.mechanism() {
                    FamilyThemeMechanism::RuleFacet { target, .. }
                    | FamilyThemeMechanism::OrdinalPalette { target }
                    | FamilyThemeMechanism::EffectBinding { target, .. } => target,
                    FamilyThemeMechanism::BaseTypography(_) => continue,
                };
                if owned_targets.contains(&target) {
                    observations
                        .entry(theme.family_mechanism_key(route))
                        .or_default();
                }
            }
        }
        Ok(Self {
            theme,
            config,
            owned_targets,
            work_meter,
            observations,
        })
    }

    pub(crate) fn style(
        &mut self,
        target: ThemeTarget,
        ordinal: usize,
        inherit_text: bool,
    ) -> crate::Result<Option<ResolvedThemeStyle>> {
        let Some(theme) = self.theme else {
            return Ok(None);
        };
        let style = if inherit_text {
            theme.text_style_with_work_meter(
                target,
                ThemeVariant::Default,
                Some(ordinal),
                self.work_meter,
            )?
        } else {
            theme.style_with_work_meter(
                target,
                ThemeVariant::Default,
                Some(ordinal),
                self.work_meter,
            )?
        };
        self.work_meter
            .charge(theme.family_mechanism_routes().len())?;
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target,
                    facet,
                    ..
                } if self.owned_targets.contains(&target)
                    && !matches!(
                        facet,
                        FamilyThemeRuleFacet::Fill(_) | FamilyThemeRuleFacet::Stroke(_)
                    )
                    && style.winner_rule_properties().any(|(property, origin)| {
                        origin.rule_index() == rule_index
                            && property == resolved_style_property_for_facet(facet)
                    }) =>
                {
                    self.residual(
                        theme.family_mechanism_key(route),
                        unsupported_residual_for_facet(facet),
                    );
                }
                FamilyThemeMechanism::EffectBinding {
                    target: binding_target,
                    ..
                } if binding_target == target
                    && style.effect_resolution().specified().is_unspecified() =>
                {
                    self.residual(
                        theme.family_mechanism_key(route),
                        FamilyThemeResidualReason::UnsupportedEffect,
                    );
                }
                _ => {}
            }
        }
        Ok(Some(style))
    }

    pub(crate) fn paint(
        &mut self,
        target: ThemeTarget,
        ordinal: usize,
        style: Option<&ResolvedThemeStyle>,
        property: ResolvedStyleProperty,
        source_paths: &[&str],
        base: &str,
    ) -> Option<TerminalPaint> {
        let theme = self.theme?;
        let style = style?;
        if source_paths
            .iter()
            .any(|path| self.config.config_paint_overrides_typed_default(path))
        {
            return None;
        }
        let resolution = match property {
            ResolvedStyleProperty::Fill => style.fill_resolution(),
            ResolvedStyleProperty::Stroke => style.stroke_resolution(),
            _ => return None,
        };
        if let Some(origin) = resolution.winner() {
            if !self.owned_targets.contains(&origin.target()) {
                return None;
            }
            let facet = if property == ResolvedStyleProperty::Fill {
                FamilyThemeRuleFacet::fill(resolution.specified())?
            } else {
                FamilyThemeRuleFacet::stroke(resolution.specified())?
            };
            let key = FamilyThemeMechanismKey::Rule {
                index: origin.rule_index(),
                target: origin.target(),
            };
            let direct = if property == ResolvedStyleProperty::Fill {
                resolve_direct_static_fill(
                    theme,
                    style,
                    &[origin.target()],
                    DirectStaticSelectorDomain::Default,
                )
            } else {
                resolve_direct_static_stroke(
                    theme,
                    style,
                    &[origin.target()],
                    DirectStaticSelectorDomain::Default,
                )
            };
            if let Some(paint) = direct {
                return Some(self.expect(key, paint.css().into(), paint.capability()));
            }
            if matches!(resolution.specified(), Specified::Clear)
                && origin.ordinal().is_none()
                && matches!(origin.variant(), None | Some(ThemeVariant::Default))
                && theme.rule_facet_disposition(origin.rule_index(), facet)
                    == Some(FamilyThemeDisposition::TypedAdapter)
            {
                if base.eq_ignore_ascii_case("none")
                    || crate::mermaid_style::is_safe_browser_css_color_value(base)
                {
                    return Some(self.expect(key, base.into(), ThemeCapability::SemanticRules));
                }
                self.residual(key, FamilyThemeResidualReason::UnsupportedPaint);
                return None;
            }
            self.residual(key, unsupported_residual_for_facet(facet));
        } else if property == ResolvedStyleProperty::Fill
            && let Some(disposition) = theme.ordinal_palette_disposition(target)
        {
            let key = FamilyThemeMechanismKey::OrdinalPalette { target };
            if target == ThemeTarget::Node && disposition == FamilyThemeDisposition::TypedAdapter {
                if let Some(color) = theme.series_color(target, ordinal) {
                    self.observations
                        .entry(key.clone())
                        .or_default()
                        .capabilities
                        .insert(if color.is_transparent() {
                            ThemeCapability::TransparentPaint
                        } else {
                            ThemeCapability::SolidPaint
                        });
                    return Some(self.expect(
                        key,
                        color.as_css().into_boxed_str(),
                        ThemeCapability::OrdinalPalette,
                    ));
                }
            } else {
                self.residual(key, FamilyThemeResidualReason::UnsupportedOrdinalPalette);
            }
        }
        None
    }

    /// Unsupported text strokes and line fills still need an applicability observation.
    pub(crate) fn unsupported_property(
        &mut self,
        target: ThemeTarget,
        style: Option<&ResolvedThemeStyle>,
        property: ResolvedStyleProperty,
    ) {
        let Some(style) = style else { return };
        let resolution = if property == ResolvedStyleProperty::Fill {
            style.fill_resolution()
        } else {
            style.stroke_resolution()
        };
        if let Some(origin) = resolution
            .winner()
            .filter(|origin| self.owned_targets.contains(&origin.target()))
        {
            self.residual(
                FamilyThemeMechanismKey::Rule {
                    index: origin.rule_index(),
                    target: origin.target(),
                },
                FamilyThemeResidualReason::UnsupportedPaint,
            );
        } else if property == ResolvedStyleProperty::Fill
            && resolution.specified().is_unspecified()
            && self
                .theme
                .is_some_and(|theme| theme.ordinal_palette_disposition(target).is_some())
        {
            self.residual(
                FamilyThemeMechanismKey::OrdinalPalette { target },
                FamilyThemeResidualReason::UnsupportedOrdinalPalette,
            );
        }
    }

    fn expect(
        &mut self,
        key: FamilyThemeMechanismKey,
        css: Box<str>,
        capability: ThemeCapability,
    ) -> TerminalPaint {
        let observation = self.observations.entry(key.clone()).or_default();
        observation.expected += 1;
        observation.capabilities.insert(capability);
        TerminalPaint { css, key }
    }

    fn residual(&mut self, key: FamilyThemeMechanismKey, reason: FamilyThemeResidualReason) {
        self.observations
            .entry(key)
            .or_default()
            .residual
            .get_or_insert(reason);
    }

    pub(crate) fn record(&mut self, paint: Option<&TerminalPaint>, emitted: &str) {
        if let Some(paint) = paint
            && paint.css() == emitted
        {
            self.observations
                .entry(paint.key.clone())
                .or_default()
                .emitted += 1;
        }
    }

    pub(crate) fn finish(self) -> FamilyThemeOutcomes {
        let mut outcomes = FamilyThemeOutcomes::default();
        for (key, observation) in self.observations {
            if let Some(reason) = observation.residual {
                outcomes.mark_residual(key, reason);
            } else if observation.expected == 0 {
                outcomes.mark_not_applicable(key);
            } else if observation.expected == observation.emitted {
                outcomes.mark_applied_with_capabilities(key, observation.capabilities);
            } else {
                outcomes.mark_residual(key, FamilyThemeResidualReason::UnsupportedPaint);
            }
        }
        outcomes
    }
}
