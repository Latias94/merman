use std::collections::{BTreeMap, BTreeSet};

use merman_core::{
    __private::{
        ThemeCompatibilityConsumptionDisposition, ThemeCompatibilityFieldConsumption,
        ThemeCompatibilityFieldKind, ThemeParseEvidence,
    },
    MermaidConfig,
};

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty, ThemeCapability,
    ThemeTarget, ThemeTypographyProperty, ThemeVariant,
};
use crate::family::{
    DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    resolve_direct_static_fill, resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

mod terminal;

use terminal::{PacketTextColorOwnership, PacketTextWinners};
pub(crate) use terminal::{PacketTextOutput, PacketTextRole};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PacketTypographyOutcome {
    Inactive,
    TypedFontStack,
    ConfigOwned,
}

#[derive(Debug)]
struct PacketUnsupportedRoute {
    key: FamilyThemeMechanismKey,
    target: ThemeTarget,
    reason: FamilyThemeResidualReason,
}

#[derive(Debug)]
struct PacketRuleObservation {
    selector: FamilyThemeSelectorShape,
    pending_fill: Option<ThemeCapability>,
    unsupported: BTreeMap<ResolvedStyleProperty, FamilyThemeResidualReason>,
    incomplete: BTreeSet<ResolvedStyleProperty>,
}

impl PacketRuleObservation {
    const fn new(selector: FamilyThemeSelectorShape) -> Self {
        Self {
            selector,
            pending_fill: None,
            unsupported: BTreeMap::new(),
            incomplete: BTreeSet::new(),
        }
    }
}

/// Final Packet stylesheet values used by the SVG writer.
#[derive(Debug)]
pub(crate) struct PacketTypographyThemePlan {
    css_binding: super::config::PacketStyleSettings,
    font_family_css: Box<str>,
    selection: Option<PacketThemeSelection>,
}

#[derive(Debug)]
struct PacketThemeSelection {
    text_color_ownership: PacketTextColorOwnership,
    typography: PacketTypographyOutcome,
    unsupported_properties: BTreeSet<ThemeTypographyProperty>,
    compatibility_surface_candidate: bool,
    canonical_dark_mode: bool,
    rule_observations: BTreeMap<FamilyThemeMechanismKey, PacketRuleObservation>,
    unsupported_routes: Box<[PacketUnsupportedRoute]>,
    text_winners: PacketTextWinners,
}

/// Requested theme results produced only by successful visible SVG writes.
pub(crate) struct PacketThemeOutcome {
    pub(crate) evidence: FamilyThemeEvidence,
    compatibility: Option<(ThemeCompatibilityConsumptionDisposition, bool)>,
}

impl PacketThemeOutcome {
    pub(crate) fn compatibility_consumptions(
        &self,
        parse_evidence: &ThemeParseEvidence,
    ) -> Box<[ThemeCompatibilityFieldConsumption]> {
        let Some((disposition, canonical_dark_mode)) = self.compatibility else {
            return Box::new([]);
        };
        packet_text_surface_compatibility_consumptions(
            parse_evidence,
            disposition,
            canonical_dark_mode,
        )
    }
}

impl PacketTypographyThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let mut css_binding =
            super::PacketConfigView::new(effective_config.as_value()).style_settings();
        let Some(theme) = theme.filter(|theme| !theme.family_mechanism_routes().is_empty()) else {
            return Ok(Self {
                css_binding,
                font_family_css: crate::config::config_font_family_css(effective_config.as_value())
                    .into_boxed_str(),
                selection: None,
            });
        };
        let text_color_ownership = PacketTextColorOwnership::from_config(effective_config);

        let config_owns_font_stack = packet_config_owns_font_stack(effective_config);
        let mut typed_font_stack = false;
        let mut unsupported_properties = BTreeSet::new();
        let byte_label_style = theme.style_with_work_meter(
            ThemeTarget::PacketByteLabel,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let field_label_style = theme.text_style_with_work_meter(
            ThemeTarget::PacketFieldLabel,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let title_style = theme.style_with_work_meter(
            ThemeTarget::Title,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let byte_label_fill =
            packet_direct_static_fill(theme, &byte_label_style, &[ThemeTarget::PacketByteLabel]);
        let field_label_fill = packet_direct_static_fill(
            theme,
            &field_label_style,
            &[ThemeTarget::Text, ThemeTarget::PacketFieldLabel],
        );
        let title_fill = packet_direct_static_fill(theme, &title_style, &[ThemeTarget::Title]);
        let text_winners =
            PacketTextWinners::new(theme, &byte_label_style, &field_label_style, &title_style);
        let mut rule_observations = BTreeMap::new();
        let mut unsupported_routes = Vec::new();

        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontStack)
                    if route.disposition() == FamilyThemeDisposition::TypedAdapter =>
                {
                    typed_font_stack = true;
                }
                FamilyThemeMechanism::BaseTypography(property)
                    if route.disposition() == FamilyThemeDisposition::Unsupported =>
                {
                    unsupported_properties.insert(property);
                }
                FamilyThemeMechanism::BaseTypography(_) => {}
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target,
                    selector,
                    facet,
                } => {
                    let key = theme.family_mechanism_key(route);
                    let observation = rule_observations
                        .entry(key)
                        .or_insert_with(|| PacketRuleObservation::new(selector));
                    let property = resolved_style_property_for_facet(facet);
                    match (route.disposition(), selector, facet) {
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeSelectorShape::Static {
                                variant: None | Some(ThemeVariant::Default),
                            },
                            FamilyThemeRuleFacet::Fill(_),
                        ) => {
                            observation.pending_fill = selected_fill_for_target(
                                target,
                                &byte_label_fill,
                                &field_label_fill,
                                &title_fill,
                            )
                            .filter(|fill| fill.rule_index() == rule_index)
                            .map(DirectStaticPaint::capability);
                        }
                        (FamilyThemeDisposition::Unsupported, _, facet) => {
                            observation
                                .unsupported
                                .entry(property)
                                .or_insert_with(|| unsupported_residual_for_facet(facet));
                        }
                        (FamilyThemeDisposition::TypedAdapter, _, _) => {
                            observation.incomplete.insert(property);
                        }
                    }
                }
                FamilyThemeMechanism::OrdinalPalette { target } => {
                    debug_assert_eq!(route.disposition(), FamilyThemeDisposition::Unsupported);
                    unsupported_routes.push(PacketUnsupportedRoute {
                        key: theme.family_mechanism_key(route),
                        target,
                        reason: FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                    });
                }
                FamilyThemeMechanism::EffectBinding { target, .. } => {
                    debug_assert_eq!(route.disposition(), FamilyThemeDisposition::Unsupported);
                    unsupported_routes.push(PacketUnsupportedRoute {
                        key: theme.family_mechanism_key(route),
                        target,
                        reason: FamilyThemeResidualReason::UnsupportedEffect,
                    });
                }
            }
        }

        let font_family_css = if typed_font_stack && !config_owns_font_stack {
            theme.typography().font_stack().as_css()
        } else {
            crate::config::config_font_family_css(effective_config.as_value())
        };
        let outcome = if typed_font_stack && config_owns_font_stack {
            PacketTypographyOutcome::ConfigOwned
        } else if typed_font_stack {
            PacketTypographyOutcome::TypedFontStack
        } else {
            PacketTypographyOutcome::Inactive
        };
        let compatibility_surface_candidate = outcome == PacketTypographyOutcome::TypedFontStack
            && byte_label_fill.is_some()
            && field_label_fill.is_some()
            && title_fill.is_some();

        for (role, configured) in [
            (PacketTextRole::ByteStart, &mut css_binding.start_byte_color),
            (PacketTextRole::ByteEnd, &mut css_binding.end_byte_color),
            (PacketTextRole::Label, &mut css_binding.label_color),
            (PacketTextRole::Title, &mut css_binding.title_color),
        ] {
            if !text_color_ownership.owns(role)
                && let Some(fill) = selected_fill_for_target(
                    role.target(),
                    &byte_label_fill,
                    &field_label_fill,
                    &title_fill,
                )
            {
                *configured = fill.css().to_owned();
            }
        }

        Ok(Self {
            css_binding,
            font_family_css: font_family_css.into_boxed_str(),
            selection: Some(PacketThemeSelection {
                text_color_ownership,
                typography: outcome,
                unsupported_properties,
                compatibility_surface_candidate,
                canonical_dark_mode: packet_dark_mode_is_canonical(effective_config),
                rule_observations,
                unsupported_routes: unsupported_routes.into_boxed_slice(),
                text_winners,
            }),
        })
    }

    pub(crate) fn font_family_css(&self) -> &str {
        &self.font_family_css
    }

    pub(crate) fn css_binding(&self) -> &super::config::PacketStyleSettings {
        &self.css_binding
    }

    pub(crate) fn begin_text_output(&self) -> Option<PacketTextOutput<'_>> {
        self.selection.as_ref().map(PacketTextOutput::new)
    }
}

impl PacketThemeSelection {
    fn finish(&self, output: &PacketTextOutput<'_>) -> PacketThemeOutcome {
        let mut evidence = FamilyThemeEvidence::outcomes();
        let key = FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontStack);
        if output.has_visible_text() {
            match self.typography {
                PacketTypographyOutcome::TypedFontStack => {
                    evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
                }
                PacketTypographyOutcome::ConfigOwned => evidence.mark_not_applicable(key),
                PacketTypographyOutcome::Inactive => {}
            }
        } else {
            evidence.mark_not_applicable(key);
        }
        for property in &self.unsupported_properties {
            let key = FamilyThemeMechanismKey::Typography(*property);
            if output.has_visible_text() {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
            } else {
                evidence.mark_not_applicable(key);
            }
        }
        for route in &self.unsupported_routes {
            if output.occurrence_count(route.target) != 0 {
                evidence.mark_residual(route.key.clone(), route.reason);
            } else {
                evidence.mark_not_applicable(route.key.clone());
            }
        }
        for (key, observation) in &self.rule_observations {
            let FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target,
            } = *key
            else {
                unreachable!("Packet rule observations must use rule mechanism keys");
            };
            if !selector_intersects_default_occurrences(
                observation.selector,
                output.occurrence_count(target),
            ) {
                evidence.mark_not_applicable(key.clone());
                continue;
            }
            if let Some(reason) = observation
                .unsupported
                .iter()
                .find_map(|(property, reason)| {
                    output.route_won(rule_index, *property).then_some(*reason)
                })
            {
                evidence.mark_residual(key.clone(), reason);
                continue;
            }
            if observation
                .incomplete
                .iter()
                .any(|property| output.route_won(rule_index, *property))
            {
                continue;
            }
            match observation.pending_fill {
                Some(capability)
                    if output.route_won(rule_index, ResolvedStyleProperty::Fill)
                        && output.has_typed_fill(target) =>
                {
                    evidence.mark_applied_with_capabilities(key.clone(), [capability]);
                }
                _ => evidence.mark_not_applicable(key.clone()),
            }
        }
        let compatibility = self.compatibility_surface_candidate.then(|| {
            let disposition = if output.has_visible_text() {
                ThemeCompatibilityConsumptionDisposition::ReplacedByTypedSurface
            } else {
                ThemeCompatibilityConsumptionDisposition::NotReadByFamily
            };
            (disposition, self.canonical_dark_mode)
        });
        PacketThemeOutcome {
            evidence,
            compatibility,
        }
    }
}

/// Returns provisional path coverage used only to preserve pre-SVG compatibility error order.
/// Final reports replace this deferral with actual writer outcomes.
pub(crate) fn deferred_mermaid_compatibility_consumptions(
    theme: Option<&ResolvedDiagramTheme>,
    effective_config: &MermaidConfig,
    parse_evidence: &ThemeParseEvidence,
) -> Box<[ThemeCompatibilityFieldConsumption]> {
    let Some(theme) = theme else {
        return Box::new([]);
    };
    if !has_complete_direct_text_surface_plan(theme) {
        return Box::new([]);
    }
    packet_text_surface_compatibility_consumptions(
        parse_evidence,
        ThemeCompatibilityConsumptionDisposition::ReplacedByTypedSurface,
        packet_dark_mode_is_canonical(effective_config),
    )
}

fn has_complete_direct_text_surface_plan(theme: &ResolvedDiagramTheme) -> bool {
    let byte_label_style = theme.style(ThemeTarget::PacketByteLabel, ThemeVariant::Default, None);
    let field_label_style = theme.style(ThemeTarget::PacketFieldLabel, ThemeVariant::Default, None);
    let text_style = theme.style(ThemeTarget::Text, ThemeVariant::Default, None);
    let title_style = theme.style(ThemeTarget::Title, ThemeVariant::Default, None);
    packet_direct_static_fill(theme, &byte_label_style, &[ThemeTarget::PacketByteLabel]).is_some()
        && (packet_direct_static_fill(theme, &field_label_style, &[ThemeTarget::PacketFieldLabel])
            .is_some()
            || packet_direct_static_fill(theme, &text_style, &[ThemeTarget::Text]).is_some())
        && packet_direct_static_fill(theme, &title_style, &[ThemeTarget::Title]).is_some()
        && theme.family_mechanism_routes().iter().any(|route| {
            route.disposition() == FamilyThemeDisposition::TypedAdapter
                && route.mechanism()
                    == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontStack)
        })
}

fn packet_text_surface_compatibility_consumptions(
    parse_evidence: &ThemeParseEvidence,
    disposition: ThemeCompatibilityConsumptionDisposition,
    consume_dark_mode: bool,
) -> Box<[ThemeCompatibilityFieldConsumption]> {
    parse_evidence
        .mermaid_fields()
        .filter(|field| {
            field.kind() == ThemeCompatibilityFieldKind::Theme
                || (consume_dark_mode && field.kind() == ThemeCompatibilityFieldKind::DarkMode)
        })
        .flat_map(|field| field.consume_all(disposition))
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

fn packet_dark_mode_is_canonical(config: &MermaidConfig) -> bool {
    match (
        config.get_bool("darkMode"),
        config.get_bool("themeVariables.darkMode"),
    ) {
        (Some(root), Some(variable)) => root == variable,
        (None, None) => true,
        (Some(_), None) | (None, Some(_)) => false,
    }
}

fn selected_fill_for_target<'a>(
    target: ThemeTarget,
    byte_label_fill: &'a Option<DirectStaticPaint>,
    field_label_fill: &'a Option<DirectStaticPaint>,
    title_fill: &'a Option<DirectStaticPaint>,
) -> Option<&'a DirectStaticPaint> {
    match target {
        ThemeTarget::PacketByteLabel => byte_label_fill.as_ref(),
        ThemeTarget::Text | ThemeTarget::PacketFieldLabel => field_label_fill.as_ref(),
        ThemeTarget::Title => title_fill.as_ref(),
        _ => None,
    }
}

fn packet_direct_static_fill(
    theme: &ResolvedDiagramTheme,
    style: &crate::diagram_theme::ResolvedThemeStyle,
    accepted_targets: &[ThemeTarget],
) -> Option<DirectStaticPaint> {
    resolve_direct_static_fill(
        theme,
        style,
        accepted_targets,
        DirectStaticSelectorDomain::Default,
    )
}

const fn selector_intersects_default_variant(selector: FamilyThemeSelectorShape) -> bool {
    let variant = match selector {
        FamilyThemeSelectorShape::Static { variant }
        | FamilyThemeSelectorShape::Ordinal { variant, .. } => variant,
    };
    matches!(variant, None | Some(ThemeVariant::Default))
}

const fn selector_intersects_default_occurrences(
    selector: FamilyThemeSelectorShape,
    occurrence_count: usize,
) -> bool {
    selector_intersects_default_variant(selector)
        && selector.ordinal_domain_intersects_occurrence_count(occurrence_count)
}

fn packet_config_owns_font_stack(config: &MermaidConfig) -> bool {
    merman_core::__private::config_path_overrides_typed_default(config, "themeVariables.fontFamily")
        || merman_core::__private::config_path_overrides_typed_default(config, "fontFamily")
}

fn packet_config_path_owns(config: &MermaidConfig, path: &str) -> bool {
    merman_core::__private::config_path_overrides_typed_default(config, path)
}
