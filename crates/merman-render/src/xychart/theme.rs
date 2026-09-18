use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    FamilyThemeMechanismKey, ResolvedDiagramTheme, ThemeCapability, ThemeTypographyProperty,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, InheritedFontStackOutcome,
    InheritedFontStackPlan,
};

/// Final XY Chart base typography shared by layout, stylesheet emission, and evidence.
#[derive(Debug)]
pub(crate) struct XyChartTypographyThemePlan {
    inherited_font_stack: InheritedFontStackPlan,
    evidence: FamilyThemeEvidence,
    terminal_receipt: OnceLock<XyChartTypographyTerminalSeal>,
}

#[derive(Debug, Clone, Copy)]
struct XyChartTypographyTerminalSeal {
    emitted_visible_text_count: usize,
    css_seen: bool,
    font_family_matches: bool,
}

/// Writer-owned proof for the final XY Chart font-family CSS and visible text stream.
#[derive(Debug)]
pub(crate) struct XyChartTypographyTerminalReceipt {
    expected_font_family: Box<str>,
    emitted_visible_text_count: usize,
    css_seen: bool,
    font_family_matches: bool,
}

impl XyChartTypographyThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
    ) -> Self {
        Self {
            inherited_font_stack: InheritedFontStackPlan::resolve_property_local(
                theme,
                effective_config,
            ),
            evidence: FamilyThemeEvidence::from_theme(theme),
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.inherited_font_stack.font_family_css()
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<XyChartTypographyTerminalReceipt> {
        self.inherited_font_stack
            .typography_requested()
            .then(|| XyChartTypographyTerminalReceipt {
                expected_font_family: self.font_family_css().into(),
                emitted_visible_text_count: 0,
                css_seen: false,
                font_family_matches: true,
            })
    }

    pub(crate) fn record_terminal(&self, receipt: XyChartTypographyTerminalReceipt) -> bool {
        self.terminal_receipt
            .set(XyChartTypographyTerminalSeal {
                emitted_visible_text_count: receipt.emitted_visible_text_count,
                css_seen: receipt.css_seen,
                font_family_matches: receipt.font_family_matches,
            })
            .is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        if !self.inherited_font_stack.typography_requested() {
            return evidence;
        }

        let Some(receipt) = self.terminal_receipt.get() else {
            self.inherited_font_stack
                .mark_unsupported_typography_evidence(&mut evidence, true);
            if self.inherited_font_stack.typed_font_stack_requested() {
                evidence.mark_residual(
                    FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontStack),
                    FamilyThemeResidualReason::UnsupportedTypography,
                );
            }
            return evidence;
        };

        let has_visible_text = receipt.emitted_visible_text_count != 0;
        self.inherited_font_stack
            .mark_unsupported_typography_evidence(&mut evidence, has_visible_text);
        if !has_visible_text {
            if self.inherited_font_stack.typed_font_stack_requested() {
                evidence.mark_not_applicable(FamilyThemeMechanismKey::Typography(
                    ThemeTypographyProperty::FontStack,
                ));
            }
            return evidence;
        }

        if self.inherited_font_stack.typed_font_stack_requested() {
            let key = FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontStack);
            match self.inherited_font_stack.outcome() {
                InheritedFontStackOutcome::Typed
                    if self.inherited_font_stack.typed_font_stack_active()
                        && receipt.css_seen
                        && receipt.font_family_matches =>
                {
                    evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
                }
                InheritedFontStackOutcome::ConfigOwned => evidence.mark_not_applicable(key),
                InheritedFontStackOutcome::Typed | InheritedFontStackOutcome::Unsupported => {
                    evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography)
                }
                InheritedFontStackOutcome::Inactive => {}
            }
        }
        evidence
    }
}

impl XyChartTypographyTerminalReceipt {
    pub(crate) fn record_css(&mut self, font_family: &str) {
        if self.css_seen {
            self.font_family_matches = false;
            return;
        }
        self.css_seen = true;
        self.font_family_matches = font_family == self.expected_font_family.as_ref();
    }

    pub(crate) fn record_visible_text(&mut self) {
        self.emitted_visible_text_count = self.emitted_visible_text_count.saturating_add(1);
    }
}
