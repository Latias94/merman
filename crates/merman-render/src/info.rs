use std::sync::OnceLock;

use crate::Result;
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, ResolvedDiagramTheme,
    ThemeCapability,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, InheritedFontStackOutcome,
    InheritedFontStackPlan, unsupported_residual_for_facet,
};
use crate::model::{Bounds, InfoDiagramLayout};
use crate::text::TextMeasurer;
use merman_core::MermaidConfig;
use merman_core::baseline::PINNED_MERMAID_BASELINE_VERSION;
use merman_core::diagrams::info::InfoDiagramRenderModel;

#[derive(Debug)]
pub(crate) struct InfoSurfaceReceipt {
    expected_font_family_css: Box<str>,
    root_font_family_css: Option<Box<str>>,
    inherited_font_family_css: Option<Box<str>>,
    root_variable_font_family_css: Option<Box<str>>,
    css_emission_unique: bool,
    version_text_count: usize,
    terminal_matches: bool,
}

impl InfoSurfaceReceipt {
    fn new(expected_font_family_css: &str) -> Self {
        Self {
            expected_font_family_css: expected_font_family_css.into(),
            root_font_family_css: None,
            inherited_font_family_css: None,
            root_variable_font_family_css: None,
            css_emission_unique: true,
            version_text_count: 0,
            terminal_matches: true,
        }
    }

    pub(crate) fn record_css_emission(
        &mut self,
        root_font_family_css: &str,
        inherited_font_family_css: &str,
        root_variable_font_family_css: &str,
    ) {
        if self.root_font_family_css.is_some()
            || self.inherited_font_family_css.is_some()
            || self.root_variable_font_family_css.is_some()
        {
            self.css_emission_unique = false;
            return;
        }
        let expected = self.expected_font_family_css.as_ref();
        self.terminal_matches &= !expected.trim().is_empty();
        self.terminal_matches &= root_font_family_css == expected;
        self.terminal_matches &= inherited_font_family_css == expected;
        self.terminal_matches &= root_variable_font_family_css == expected;
        self.root_font_family_css = Some(root_font_family_css.into());
        self.inherited_font_family_css = Some(inherited_font_family_css.into());
        self.root_variable_font_family_css = Some(root_variable_font_family_css.into());
    }

    pub(crate) fn record_version_text(&mut self, emitted_class: &str, text: &str) {
        self.terminal_matches &= emitted_class == "version";
        self.terminal_matches &= !text.trim().is_empty();
        if !text.trim().is_empty() {
            self.version_text_count = self.version_text_count.saturating_add(1);
        }
    }

    fn proves_font_stack(&self) -> bool {
        self.css_emission_unique
            && self.root_font_family_css.as_deref() == Some(self.expected_font_family_css.as_ref())
            && self.inherited_font_family_css.as_deref()
                == Some(self.expected_font_family_css.as_ref())
            && self.root_variable_font_family_css.as_deref()
                == Some(self.expected_font_family_css.as_ref())
            && self.version_text_count == 1
            && self.terminal_matches
    }
}

/// Final inherited Info font shared by terminal CSS emission and family evidence.
#[derive(Debug)]
pub(crate) struct InfoTypographyThemePlan {
    inherited_font_stack: InheritedFontStackPlan,
    evidence: FamilyThemeEvidence,
    unsupported_routes: Box<[InfoUnsupportedRoute]>,
    terminal_receipt: OnceLock<InfoSurfaceReceipt>,
}

#[derive(Debug)]
struct InfoUnsupportedRoute {
    key: FamilyThemeMechanismKey,
    reason: FamilyThemeResidualReason,
}

impl InfoTypographyThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
    ) -> Self {
        let unsupported_routes = info_unsupported_routes(theme);
        Self {
            inherited_font_stack: InheritedFontStackPlan::resolve(theme, effective_config),
            evidence: FamilyThemeEvidence::from_theme(theme),
            unsupported_routes,
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.inherited_font_stack.font_family_css()
    }

    pub(crate) fn begin_terminal_receipt(&self) -> InfoSurfaceReceipt {
        InfoSurfaceReceipt::new(self.font_family_css())
    }

    pub(crate) fn record_terminal(&self, receipt: InfoSurfaceReceipt) -> bool {
        self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        for route in &self.unsupported_routes {
            evidence.mark_residual(route.key.clone(), route.reason);
        }
        let Some(receipt) = self.terminal_receipt.get() else {
            return evidence;
        };
        let key = FamilyThemeMechanismKey::Typography;
        match self.inherited_font_stack.outcome() {
            InheritedFontStackOutcome::Typed if receipt.proves_font_stack() => {
                evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
            }
            InheritedFontStackOutcome::ConfigOwned if receipt.proves_font_stack() => {
                evidence.mark_not_applicable(key);
            }
            InheritedFontStackOutcome::Typed
            | InheritedFontStackOutcome::ConfigOwned
            | InheritedFontStackOutcome::Unsupported => {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
            }
            InheritedFontStackOutcome::Inactive => {}
        }
        evidence
    }
}

fn info_unsupported_routes(theme: Option<&ResolvedDiagramTheme>) -> Box<[InfoUnsupportedRoute]> {
    let Some(theme) = theme else {
        return Box::new([]);
    };
    theme
        .family_mechanism_routes()
        .iter()
        .copied()
        .filter_map(|route| {
            if route.disposition() != FamilyThemeDisposition::Unsupported {
                return None;
            }
            let reason = match route.mechanism() {
                FamilyThemeMechanism::RuleFacet { facet, .. } => {
                    unsupported_residual_for_facet(facet)
                }
                FamilyThemeMechanism::OrdinalPalette { .. } => {
                    FamilyThemeResidualReason::UnsupportedOrdinalPalette
                }
                FamilyThemeMechanism::EffectBinding { .. } => {
                    FamilyThemeResidualReason::UnsupportedEffect
                }
                FamilyThemeMechanism::BaseTypography(_) => return None,
            };
            Some(InfoUnsupportedRoute {
                key: theme.family_mechanism_key(route),
                reason,
            })
        })
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

pub(crate) fn layout_info_diagram_typed(
    model: &InfoDiagramRenderModel,
    _effective_config: &serde_json::Value,
    _measurer: &dyn TextMeasurer,
) -> Result<InfoDiagramLayout> {
    let _ = model.show_info;
    Ok(InfoDiagramLayout {
        // Mermaid configures the info renderer with `height = 100`, `width = 400`. Responsive
        // max-width mode omits the height attribute from the emitted SVG, but the layout model
        // still records the configured canvas dimensions.
        bounds: Some(Bounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 400.0,
            max_y: 100.0,
        }),
        version: format!("v{PINNED_MERMAID_BASELINE_VERSION}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::DeterministicTextMeasurer;

    #[test]
    fn info_layout_records_upstream_configured_canvas_size() {
        let measurer = DeterministicTextMeasurer::default();
        let layout = layout_info_diagram_typed(
            &InfoDiagramRenderModel::default(),
            &serde_json::Value::Null,
            &measurer,
        )
        .expect("info layout");

        assert_eq!(
            layout.bounds,
            Some(Bounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 400.0,
                max_y: 100.0,
            })
        );
    }
}
