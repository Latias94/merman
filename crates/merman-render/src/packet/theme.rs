use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ThemeCapability, ThemeTarget,
    ThemeTypographyProperty,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, unsupported_residual_for_facet,
};

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
    selector: Option<FamilyThemeSelectorShape>,
    reason: FamilyThemeResidualReason,
}

impl PacketUnsupportedRoute {
    fn is_applicable(&self, receipt: &PacketSurfaceReceipt) -> bool {
        let occurrence_count = receipt.occurrence_count(self.target);
        self.selector.is_none_or(|selector| {
            selector.ordinal_domain_intersects_occurrence_count(occurrence_count)
        }) && occurrence_count != 0
    }
}

/// Writer-owned count of the independently styled Packet text surfaces emitted to terminal SVG.
#[derive(Debug, Default)]
pub(crate) struct PacketSurfaceReceipt {
    text_count: usize,
    title_count: usize,
}

impl PacketSurfaceReceipt {
    pub(crate) fn record_non_empty_text(&mut self, text: &str) {
        if !text.trim().is_empty() {
            self.record_text_occurrence();
        }
    }

    pub(crate) fn record_text_occurrence(&mut self) {
        self.text_count = self.text_count.saturating_add(1);
    }

    pub(crate) fn record_title_occurrence(&mut self) {
        self.title_count = self.title_count.saturating_add(1);
    }

    fn occurrence_count(&self, target: ThemeTarget) -> usize {
        match target {
            ThemeTarget::Text => self.text_count,
            ThemeTarget::Title => self.title_count,
            _ => 0,
        }
    }

    fn has_visible_text(&self) -> bool {
        self.text_count != 0 || self.title_count != 0
    }
}

/// Final Packet base font shared by stylesheet emission and terminal evidence.
#[derive(Debug)]
pub(crate) struct PacketTypographyThemePlan {
    font_family_css: Box<str>,
    evidence: FamilyThemeEvidence,
    outcome: PacketTypographyOutcome,
    unsupported_routes: Box<[PacketUnsupportedRoute]>,
    terminal_receipt: OnceLock<PacketSurfaceReceipt>,
}

impl PacketTypographyThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
    ) -> Self {
        let configured_font = crate::config::config_font_family_css(effective_config.as_value());
        let Some(theme) = theme else {
            return Self {
                font_family_css: configured_font.into_boxed_str(),
                evidence: FamilyThemeEvidence::default(),
                outcome: PacketTypographyOutcome::Inactive,
                unsupported_routes: Box::new([]),
                terminal_receipt: OnceLock::new(),
            };
        };

        let config_owns_font_stack = packet_config_owns_font_stack(effective_config);
        let mut typed_font_stack = false;
        let mut unsupported_typography = false;
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
                    target,
                    selector,
                    facet,
                    ..
                } => {
                    debug_assert_eq!(route.disposition(), FamilyThemeDisposition::Unsupported);
                    unsupported_routes.push(PacketUnsupportedRoute {
                        key: theme.family_mechanism_key(route),
                        target,
                        selector: Some(selector),
                        reason: unsupported_residual_for_facet(facet),
                    });
                }
                FamilyThemeMechanism::OrdinalPalette { target } => {
                    debug_assert_eq!(route.disposition(), FamilyThemeDisposition::Unsupported);
                    unsupported_routes.push(PacketUnsupportedRoute {
                        key: theme.family_mechanism_key(route),
                        target,
                        selector: None,
                        reason: FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                    });
                }
                FamilyThemeMechanism::EffectBinding { target, .. } => {
                    debug_assert_eq!(route.disposition(), FamilyThemeDisposition::Unsupported);
                    unsupported_routes.push(PacketUnsupportedRoute {
                        key: theme.family_mechanism_key(route),
                        target,
                        selector: None,
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

        Self {
            font_family_css: font_family_css.into_boxed_str(),
            evidence: FamilyThemeEvidence::from_theme(Some(theme)),
            outcome,
            unsupported_routes: unsupported_routes.into_boxed_slice(),
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn font_family_css(&self) -> &str {
        &self.font_family_css
    }

    pub(crate) fn begin_terminal_receipt(&self) -> PacketSurfaceReceipt {
        PacketSurfaceReceipt::default()
    }

    pub(crate) fn record_terminal(&self, receipt: PacketSurfaceReceipt) -> bool {
        self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        let Some(receipt) = self.terminal_receipt.get() else {
            return evidence;
        };
        let key = FamilyThemeMechanismKey::Typography;
        if !receipt.has_visible_text() {
            evidence.mark_not_applicable(key);
            for route in &self.unsupported_routes {
                evidence.mark_not_applicable(route.key.clone());
            }
            return evidence;
        }
        match self.outcome {
            PacketTypographyOutcome::TypedFontStack => {
                evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
            }
            PacketTypographyOutcome::ConfigOwned => evidence.mark_not_applicable(key),
            PacketTypographyOutcome::Unsupported => {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
            }
            PacketTypographyOutcome::Inactive => {}
        }
        for route in &self.unsupported_routes {
            if route.is_applicable(receipt) {
                evidence.mark_residual(route.key.clone(), route.reason);
            } else {
                evidence.mark_not_applicable(route.key.clone());
            }
        }
        evidence
    }
}

fn packet_config_owns_font_stack(config: &MermaidConfig) -> bool {
    merman_core::__private::config_path_overrides_typed_default(config, "themeVariables.fontFamily")
        || merman_core::__private::config_path_overrides_typed_default(config, "fontFamily")
}
