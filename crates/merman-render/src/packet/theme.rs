use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

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

use terminal::{
    PacketFillProof, PacketTerminalEvidence, PacketTerminalWinnerLedger,
    PacketTerminalWinnerResolver, PacketTextColorOwnership,
};
pub(crate) use terminal::{PacketSurfaceReceipt, PacketTextRole};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PacketTypographyOutcome {
    Inactive,
    TypedFontStack,
    ConfigOwned,
    Unsupported,
}

#[derive(Debug)]
struct PacketUnsupportedRoute {
    key: FamilyThemeMechanismKey,
    target: ThemeTarget,
    reason: FamilyThemeResidualReason,
}

#[derive(Debug)]
struct PacketRuleObservation {
    target: ThemeTarget,
    selector: FamilyThemeSelectorShape,
    pending_fill: Option<ThemeCapability>,
    unsupported: BTreeMap<ResolvedStyleProperty, FamilyThemeResidualReason>,
    incomplete: BTreeSet<ResolvedStyleProperty>,
}

impl PacketRuleObservation {
    const fn new(target: ThemeTarget, selector: FamilyThemeSelectorShape) -> Self {
        Self {
            target,
            selector,
            pending_fill: None,
            unsupported: BTreeMap::new(),
            incomplete: BTreeSet::new(),
        }
    }
}

impl PacketUnsupportedRoute {
    fn is_applicable(&self, receipt: &PacketSurfaceReceipt) -> bool {
        receipt.occurrence_count(self.target) != 0
    }
}

/// Final Packet base font shared by stylesheet emission and terminal evidence.
#[derive(Debug)]
pub(crate) struct PacketTypographyThemePlan {
    font_family_css: Box<str>,
    byte_label_fill: Option<DirectStaticPaint>,
    field_label_fill: Option<DirectStaticPaint>,
    title_fill: Option<DirectStaticPaint>,
    text_color_ownership: PacketTextColorOwnership,
    evidence: FamilyThemeEvidence,
    outcome: PacketTypographyOutcome,
    compatibility_surface_candidate: bool,
    canonical_dark_mode: bool,
    rule_observations: BTreeMap<FamilyThemeMechanismKey, PacketRuleObservation>,
    unsupported_routes: Box<[PacketUnsupportedRoute]>,
    terminal_winner_resolver: Option<PacketTerminalWinnerResolver>,
    terminal_evidence: OnceLock<PacketTerminalEvidence>,
}

impl PacketTypographyThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let configured_font = crate::config::config_font_family_css(effective_config.as_value());
        let text_color_ownership = PacketTextColorOwnership::from_config(effective_config);
        let Some(theme) = theme else {
            return Ok(Self {
                font_family_css: configured_font.into_boxed_str(),
                byte_label_fill: None,
                field_label_fill: None,
                title_fill: None,
                text_color_ownership,
                evidence: FamilyThemeEvidence::default(),
                outcome: PacketTypographyOutcome::Inactive,
                compatibility_surface_candidate: false,
                canonical_dark_mode: packet_dark_mode_is_canonical(effective_config),
                rule_observations: BTreeMap::new(),
                unsupported_routes: Box::new([]),
                terminal_winner_resolver: None,
                terminal_evidence: OnceLock::new(),
            });
        };

        let config_owns_font_stack = packet_config_owns_font_stack(effective_config);
        let mut typed_font_stack = false;
        let mut unsupported_typography = false;
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
        let terminal_winner_resolver = Some(PacketTerminalWinnerResolver::new(
            theme,
            &byte_label_style,
            &field_label_style,
            &title_style,
        ));
        let mut rule_observations = BTreeMap::new();
        let mut unsupported_routes = Vec::new();

        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontStack)
                    if route.disposition() == FamilyThemeDisposition::TypedAdapter =>
                {
                    typed_font_stack = true;
                }
                FamilyThemeMechanism::BaseTypography(_) => unsupported_typography = true,
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target,
                    selector,
                    facet,
                } => {
                    let key = theme.family_mechanism_key(route);
                    let observation = rule_observations
                        .entry(key)
                        .or_insert_with(|| PacketRuleObservation::new(target, selector));
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
                        (FamilyThemeDisposition::TypedAdapter, _, _)
                        | (FamilyThemeDisposition::LegacyCompatibility, _, _) => {
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
            configured_font
        };
        let outcome = if unsupported_typography {
            PacketTypographyOutcome::Unsupported
        } else if typed_font_stack && config_owns_font_stack {
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

        Ok(Self {
            font_family_css: font_family_css.into_boxed_str(),
            byte_label_fill,
            field_label_fill,
            title_fill,
            text_color_ownership,
            evidence: FamilyThemeEvidence::from_theme(Some(theme)),
            outcome,
            compatibility_surface_candidate,
            canonical_dark_mode: packet_dark_mode_is_canonical(effective_config),
            rule_observations,
            unsupported_routes: unsupported_routes.into_boxed_slice(),
            terminal_winner_resolver,
            terminal_evidence: OnceLock::new(),
        })
    }

    pub(crate) fn font_family_css(&self) -> &str {
        &self.font_family_css
    }

    pub(crate) fn fill_css<'a>(
        &'a self,
        role: PacketTextRole,
        configured_fill: &'a str,
    ) -> &'a str {
        let target = role.target();
        if self.text_color_ownership.owns(role) {
            return configured_fill;
        }
        selected_fill_for_target(
            target,
            &self.byte_label_fill,
            &self.field_label_fill,
            &self.title_fill,
        )
        .map_or(configured_fill, DirectStaticPaint::css)
    }

    pub(crate) fn begin_terminal_receipt(
        &self,
        expected_counts: [(PacketTextRole, usize); 4],
    ) -> PacketSurfaceReceipt {
        PacketSurfaceReceipt::new(expected_counts, &self.font_family_css)
    }

    pub(crate) fn record_terminal(
        &self,
        receipt: PacketSurfaceReceipt,
        work_meter: &OperationWorkMeter,
    ) -> Result<bool, OperationWorkError> {
        let winners = match &self.terminal_winner_resolver {
            Some(resolver) => resolver.resolve(&receipt, self.text_color_ownership, work_meter)?,
            None => PacketTerminalWinnerLedger::default(),
        };
        Ok(self
            .terminal_evidence
            .set(PacketTerminalEvidence { receipt, winners })
            .is_ok())
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        let Some(terminal) = self.terminal_evidence.get() else {
            return evidence;
        };
        let receipt = &terminal.receipt;
        let key = FamilyThemeMechanismKey::Typography;
        if receipt.has_visible_text() {
            match self.outcome {
                PacketTypographyOutcome::TypedFontStack
                    if receipt.typography_stylesheet_verified()
                        && receipt.terminal_dom_verified() =>
                {
                    evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
                }
                PacketTypographyOutcome::TypedFontStack => {
                    evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
                }
                PacketTypographyOutcome::ConfigOwned => evidence.mark_not_applicable(key),
                PacketTypographyOutcome::Unsupported => {
                    evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
                }
                PacketTypographyOutcome::Inactive => {}
            }
        } else {
            evidence.mark_not_applicable(key);
        }
        for route in &self.unsupported_routes {
            if route.is_applicable(receipt) {
                evidence.mark_residual(route.key.clone(), route.reason);
            } else {
                evidence.mark_not_applicable(route.key.clone());
            }
        }
        for (key, observation) in &self.rule_observations {
            if !selector_intersects_default_occurrences(
                observation.selector,
                receipt.occurrence_count(observation.target),
            ) {
                evidence.mark_not_applicable(key.clone());
                continue;
            }
            let rule_index = match key {
                FamilyThemeMechanismKey::Rule { index, .. } => *index,
                _ => unreachable!("Packet rule observations must use rule mechanism keys"),
            };
            if let Some(reason) = observation
                .unsupported
                .iter()
                .find_map(|(property, reason)| {
                    terminal
                        .winners
                        .route_won(rule_index, *property)
                        .then_some(*reason)
                })
            {
                evidence.mark_residual(key.clone(), reason);
                continue;
            }
            if observation
                .incomplete
                .iter()
                .any(|property| terminal.winners.route_won(rule_index, *property))
            {
                continue;
            }
            let Some(capability) = observation.pending_fill else {
                evidence.mark_not_applicable(key.clone());
                continue;
            };
            if !terminal
                .winners
                .route_won(rule_index, ResolvedStyleProperty::Fill)
            {
                evidence.mark_not_applicable(key.clone());
                continue;
            }
            let Some(fill) = selected_fill_for_target(
                observation.target,
                &self.byte_label_fill,
                &self.field_label_fill,
                &self.title_fill,
            ) else {
                continue;
            };
            match receipt.prove_typed_fill(
                observation.target,
                fill.css(),
                self.text_color_ownership,
            ) {
                PacketFillProof::NotApplicable => evidence.mark_not_applicable(key.clone()),
                PacketFillProof::Verified => {
                    evidence.mark_applied_with_capabilities(key.clone(), [capability]);
                }
                PacketFillProof::Unverified => {
                    evidence
                        .mark_residual(key.clone(), FamilyThemeResidualReason::UnsupportedPaint);
                }
            }
        }
        evidence
    }

    /// Returns compatibility consumptions only after the terminal writer proves the direct text
    /// surface plan that allowed pre-SVG admission to defer these conceptual fields.
    pub(crate) fn terminal_mermaid_compatibility_consumptions(
        &self,
        parse_evidence: &ThemeParseEvidence,
    ) -> Box<[ThemeCompatibilityFieldConsumption]> {
        if !self.compatibility_surface_candidate {
            return Box::new([]);
        }
        let Some(terminal) = self.terminal_evidence.get() else {
            return Box::new([]);
        };
        let receipt = &terminal.receipt;
        if !receipt.typography_stylesheet_verified() || !receipt.terminal_dom_verified() {
            return Box::new([]);
        }
        for (target, fill) in [
            (ThemeTarget::PacketByteLabel, &self.byte_label_fill),
            (ThemeTarget::PacketFieldLabel, &self.field_label_fill),
            (ThemeTarget::Title, &self.title_fill),
        ] {
            let Some(fill) = fill else {
                return Box::new([]);
            };
            if receipt.prove_typed_fill(target, fill.css(), self.text_color_ownership)
                == PacketFillProof::Unverified
            {
                return Box::new([]);
            }
        }

        let disposition = if receipt.has_visible_text() {
            ThemeCompatibilityConsumptionDisposition::ReplacedByTypedSurface
        } else {
            ThemeCompatibilityConsumptionDisposition::NotReadByFamily
        };
        packet_text_surface_compatibility_consumptions(
            parse_evidence,
            disposition,
            self.canonical_dark_mode,
        )
    }
}

/// Returns provisional path coverage used only to preserve pre-SVG compatibility error order.
/// Final reports replace this deferral with writer-owned terminal consumptions.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontStack,
        MermaidThemeCompatibility, ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTextStyle,
        TypographySpec,
    };

    fn typed_packet_plan() -> PacketTypographyThemePlan {
        let typography = ThemeTextStyle::default().with_font_stack(
            FontStack::single("monospace").expect("valid Packet receipt font stack"),
        );
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_family_style(DiagramFamilyId::PACKET, typography),
            ))
            .expect("compile Packet receipt theme");
        let resolved = theme.resolve(DiagramFamilyId::PACKET);
        let work_meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        PacketTypographyThemePlan::resolve(
            Some(&resolved),
            &MermaidConfig::from_value(serde_json::json!({})),
            &work_meter,
        )
        .expect("resolve Packet receipt theme")
    }

    fn complete_typed_packet_theme() -> DiagramTheme {
        let typography = ThemeTextStyle::default().with_font_stack(
            FontStack::single("monospace").expect("valid Packet receipt font stack"),
        );
        let compatibility = MermaidThemeCompatibility::default()
            .with_theme("base")
            .expect("valid Packet receipt compatibility theme")
            .with_dark_mode(true)
            .expect("valid Packet receipt compatibility dark mode");
        DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new()
                    .with_mermaid_compatibility(compatibility)
                    .with_typography(
                        TypographySpec::default()
                            .with_family_style(DiagramFamilyId::PACKET, typography),
                    )
                    .with_styles(
                        ThemeRuleSet::default()
                            .with_rule(
                                ThemeRule::new(
                                    ThemeTarget::PacketByteLabel,
                                    ThemeStylePatch::default().with_fill(
                                        CanvasPaint::solid("#e5e7eb")
                                            .expect("valid Packet byte fill"),
                                    ),
                                )
                                .for_family(DiagramFamilyId::PACKET),
                            )
                            .with_rule(
                                ThemeRule::new(
                                    ThemeTarget::PacketFieldLabel,
                                    ThemeStylePatch::default().with_fill(
                                        CanvasPaint::solid("#111827")
                                            .expect("valid Packet label fill"),
                                    ),
                                )
                                .for_family(DiagramFamilyId::PACKET),
                            )
                            .with_rule(
                                ThemeRule::new(
                                    ThemeTarget::Title,
                                    ThemeStylePatch::default().with_fill(
                                        CanvasPaint::solid("#fef3c7")
                                            .expect("valid Packet title fill"),
                                    ),
                                )
                                .for_family(DiagramFamilyId::PACKET),
                            ),
                    ),
            )
            .expect("compile complete Packet receipt theme")
    }

    #[test]
    fn unverified_packet_typography_stylesheet_is_a_strict_residual() {
        let plan = typed_packet_plan();
        let mut receipt = plan.begin_terminal_receipt([
            (PacketTextRole::Label, 1),
            (PacketTextRole::ByteStart, 0),
            (PacketTextRole::ByteEnd, 0),
            (PacketTextRole::Title, 0),
        ]);
        receipt.record_successful_css_emission(
            "serif",
            [
                (PacketTextRole::ByteStart, "black"),
                (PacketTextRole::ByteEnd, "black"),
                (PacketTextRole::Label, "black"),
                (PacketTextRole::Title, "black"),
            ],
        );
        receipt.record_text_occurrence(
            PacketTextRole::Label,
            PacketTextRole::Label.class_attribute(),
        );
        let work_meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        assert!(
            plan.record_terminal(receipt, &work_meter)
                .expect("resolve Packet terminal winners")
        );

        let evidence = plan.finish_evidence();
        assert!(evidence.applied().is_empty());
        assert_eq!(evidence.residuals().len(), 1);
        assert_eq!(
            evidence.residuals()[0].reason(),
            FamilyThemeResidualReason::UnsupportedTypography
        );
    }

    #[test]
    fn failed_packet_writer_receipt_cannot_consume_compatibility_fields() {
        let theme = complete_typed_packet_theme();
        let metadata =
            crate::__private::install_parse_compatibility(&theme, merman_core::Engine::new())
                .parse_metadata_sync("packet\ntitle Receipt\n0-7: Header")
                .expect("parse Packet receipt metadata");
        let parse_evidence = merman_core::__private::theme_parse_evidence(&metadata);
        assert_eq!(parse_evidence.mermaid_residual_count(), 2);
        let resolved = theme.resolve(DiagramFamilyId::PACKET);
        let work_meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let plan = PacketTypographyThemePlan::resolve(
            Some(&resolved),
            &metadata.effective_config,
            &work_meter,
        )
        .expect("resolve complete Packet receipt plan");
        let mut receipt = plan.begin_terminal_receipt([
            (PacketTextRole::Label, 1),
            (PacketTextRole::ByteStart, 1),
            (PacketTextRole::ByteEnd, 1),
            (PacketTextRole::Title, 1),
        ]);
        receipt.record_successful_css_emission(
            "serif",
            [
                (PacketTextRole::ByteStart, "#e5e7eb"),
                (PacketTextRole::ByteEnd, "#e5e7eb"),
                (PacketTextRole::Label, "#111827"),
                (PacketTextRole::Title, "#fef3c7"),
            ],
        );
        for role in [
            PacketTextRole::Label,
            PacketTextRole::ByteStart,
            PacketTextRole::ByteEnd,
            PacketTextRole::Title,
        ] {
            receipt.record_text_occurrence(role, role.class_attribute());
        }
        assert!(
            plan.record_terminal(receipt, &work_meter)
                .expect("resolve failed Packet terminal receipt")
        );

        let consumptions = plan.terminal_mermaid_compatibility_consumptions(&parse_evidence);
        assert!(consumptions.is_empty());
        assert_eq!(
            parse_evidence
                .reconcile_mermaid_consumptions(consumptions.iter())
                .remaining_field_count(),
            2
        );
    }
}
