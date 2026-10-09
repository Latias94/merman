use std::sync::OnceLock;

use crate::Result;
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    ResolvedDiagramTheme, ThemeCapability, ThemeTarget, ThemeTypographyProperty, ThemeVariant,
};
use crate::family::{
    DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    InheritedFontStackOutcome, InheritedFontStackPlan, InheritedTextRunFacts, InheritedTextRunSpec,
    InheritedTextViewportFacts, UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains,
    resolve_direct_static_fill, unsupported_residual_for_facet,
};
use crate::model::{Bounds, InfoDiagramLayout};
use crate::resources::{OperationWorkError, OperationWorkMeter};
use crate::text::TextMeasurer;
use merman_core::MermaidConfig;
use merman_core::baseline::PINNED_MERMAID_BASELINE_VERSION;
use merman_core::diagrams::info::InfoDiagramRenderModel;

const INFO_BASELINE_WIDTH_PX: f64 = 400.0;
const INFO_BASELINE_HEIGHT_PX: f64 = 100.0;
const INFO_VERSION_FONT_SIZE_PX: f64 = 32.0;
const INFO_VERSION_BASELINE_X_PX: f64 = 100.0;
const INFO_VERSION_BASELINE_Y_PX: f64 = 40.0;

#[derive(Debug)]
pub(crate) struct InfoSurfaceReceipt {
    expected_font_family_css: Box<str>,
    expected_version: InheritedTextRunFacts,
    root_font_family_css: Option<Box<str>>,
    inherited_font_family_css: Option<Box<str>>,
    root_variable_font_family_css: Option<Box<str>>,
    css_emission_unique: bool,
    version_text_count: usize,
    version_fill_css: Option<Box<str>>,
    terminal_matches: bool,
}

impl InfoSurfaceReceipt {
    fn new(expected_font_family_css: &str, expected_version: &InheritedTextRunFacts) -> Self {
        Self {
            expected_font_family_css: expected_font_family_css.into(),
            expected_version: expected_version.clone(),
            root_font_family_css: None,
            inherited_font_family_css: None,
            root_variable_font_family_css: None,
            css_emission_unique: true,
            version_text_count: 0,
            version_fill_css: None,
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

    pub(crate) fn record_version_text(
        &mut self,
        emitted_class: &str,
        text: &str,
        font_size_px: f64,
        x: f64,
        y: f64,
        fill_css: Option<&str>,
    ) {
        self.terminal_matches &= emitted_class == "version";
        self.terminal_matches &= self
            .expected_version
            .matches_terminal(text, font_size_px, x, y);
        if !text.trim().is_empty() {
            self.version_text_count = self.version_text_count.saturating_add(1);
        }
        self.version_fill_css = fill_css.map(Into::into);
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

    fn proves_text_fill(&self, expected_fill_css: &str) -> bool {
        self.version_text_count == 1
            && self.terminal_matches
            && self.version_fill_css.as_deref() == Some(expected_fill_css)
    }
}

#[derive(Debug)]
struct InfoDirectTextFillRoute {
    key: FamilyThemeMechanismKey,
    rule_index: usize,
}

#[derive(Debug, Default)]
struct InfoThemeRoutes {
    unsupported: Box<[InfoUnsupportedRoute]>,
    direct_text_fill: Box<[InfoDirectTextFillRoute]>,
}

/// Final inherited Info font shared by terminal CSS emission and family evidence.
#[derive(Debug)]
pub(crate) struct InfoTypographyThemePlan {
    css_binding: crate::svg::PreparedCommonCss,
    inherited_font_stack: InheritedFontStackPlan,
    terminal_geometry: InheritedTextViewportFacts,
    evidence: FamilyThemeEvidence,
    unsupported_routes: Box<[InfoUnsupportedRoute]>,
    text_fill: Option<DirectStaticPaint>,
    direct_text_fill_routes: Box<[InfoDirectTextFillRoute]>,
    config_owns_text_fill: bool,
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
        measurer: &dyn TextMeasurer,
        work_meter: &OperationWorkMeter,
    ) -> std::result::Result<Self, OperationWorkError> {
        let inherited_font_stack =
            InheritedFontStackPlan::resolve_property_local(theme, effective_config);
        let css_binding = crate::svg::PreparedCommonCss::new(
            effective_config.as_value(),
            Some(inherited_font_stack.font_family_css()),
        );
        let version = format!("v{PINNED_MERMAID_BASELINE_VERSION}");
        let terminal_geometry = InheritedTextViewportFacts::prepare(
            INFO_BASELINE_WIDTH_PX,
            INFO_BASELINE_HEIGHT_PX,
            [InheritedTextRunSpec::new(
                &version,
                INFO_VERSION_FONT_SIZE_PX,
                INFO_VERSION_BASELINE_X_PX,
                INFO_VERSION_BASELINE_Y_PX,
            )],
            inherited_font_stack.font_family_css(),
            measurer,
        );
        let routes = info_theme_routes(theme);
        let text_fill = theme.and_then(|theme| {
            let style = theme.style(ThemeTarget::Text, ThemeVariant::Default, None);
            resolve_direct_static_fill(
                theme,
                &style,
                &[ThemeTarget::Text],
                DirectStaticSelectorDomain::Default,
            )
        });
        let config_owns_text_fill = theme.is_some()
            && merman_core::__private::config_path_overrides_typed_default(
                effective_config,
                "themeVariables.textColor",
            );
        let mut evidence = FamilyThemeEvidence::from_theme(theme);
        if let Some(theme) = theme
            && theme.ordinal_palette_disposition(ThemeTarget::Text)
                == Some(FamilyThemeDisposition::Unsupported)
        {
            // The version is one shared text-fill terminal. Keep unsupported rule and
            // effect accounting local while reconciling the palette's fallback winner.
            let mut fallbacks = FamilyThemeEvidence::from_theme(Some(theme));
            reconcile_unsupported_terminal_domains(
                theme,
                &mut fallbacks,
                &[UnsupportedTerminalDomain::static_fallbacks_only(
                    ThemeTarget::Text,
                    ThemeVariant::Default,
                )
                .with_source_owned_fill(&[config_owns_text_fill])],
                work_meter,
            )?;
            let key = FamilyThemeMechanismKey::OrdinalPalette {
                target: ThemeTarget::Text,
            };
            if fallbacks.not_applicable_mechanisms().contains(&key) {
                evidence.mark_not_applicable(key);
            } else {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedOrdinalPalette);
            }
        }
        Ok(Self {
            css_binding,
            inherited_font_stack,
            terminal_geometry,
            evidence,
            unsupported_routes: routes.unsupported,
            text_fill,
            direct_text_fill_routes: routes.direct_text_fill,
            config_owns_text_fill,
            terminal_receipt: OnceLock::new(),
        })
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.inherited_font_stack.font_family_css()
    }

    pub(crate) fn css_binding(&self) -> &crate::svg::PreparedCommonCss {
        &self.css_binding
    }

    pub(crate) fn begin_terminal_receipt(&self) -> InfoSurfaceReceipt {
        InfoSurfaceReceipt::new(self.font_family_css(), self.version_geometry())
    }

    pub(crate) const fn viewport_width_px(&self) -> f64 {
        self.terminal_geometry.width_px()
    }

    pub(crate) const fn viewport_height_px(&self) -> f64 {
        self.terminal_geometry.height_px()
    }

    pub(crate) fn requires_explicit_viewport_height(&self) -> bool {
        self.viewport_height_px() > INFO_BASELINE_HEIGHT_PX
    }

    pub(crate) fn version_geometry(&self) -> &InheritedTextRunFacts {
        self.terminal_geometry.run(0)
    }

    pub(crate) fn version_fill_css(&self) -> Option<&str> {
        (!self.config_owns_text_fill)
            .then_some(self.text_fill.as_ref())
            .flatten()
            .map(DirectStaticPaint::css)
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
        self.inherited_font_stack
            .mark_unsupported_typography_evidence(&mut evidence, receipt.version_text_count != 0);
        let key = FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontStack);
        if self.inherited_font_stack.typed_font_stack_active() {
            if receipt.proves_font_stack() {
                evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
            } else {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
            }
        } else {
            match self.inherited_font_stack.outcome() {
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
        }
        for route in &self.direct_text_fill_routes {
            if self.config_owns_text_fill {
                evidence.mark_not_applicable(route.key.clone());
                continue;
            }
            let Some(fill) = self
                .text_fill
                .as_ref()
                .filter(|fill| fill.rule_index() == route.rule_index)
            else {
                evidence.mark_not_applicable(route.key.clone());
                continue;
            };
            if receipt.proves_text_fill(fill.css()) {
                evidence.mark_applied_with_capabilities(route.key.clone(), [fill.capability()]);
            } else {
                evidence.mark_residual(
                    route.key.clone(),
                    FamilyThemeResidualReason::UnsupportedPaint,
                );
            }
        }
        evidence
    }
}

fn info_theme_routes(theme: Option<&ResolvedDiagramTheme>) -> InfoThemeRoutes {
    let Some(theme) = theme else {
        return InfoThemeRoutes::default();
    };
    let mut unsupported = Vec::new();
    let mut direct_text_fill = Vec::new();
    for route in theme.family_mechanism_routes().iter().copied() {
        match route.mechanism() {
            FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::Text,
                facet: FamilyThemeRuleFacet::Fill(_),
                selector,
                ..
            } if route.disposition() == FamilyThemeDisposition::TypedAdapter
                && matches!(
                    selector,
                    crate::diagram_theme::FamilyThemeSelectorShape::Static {
                        variant: None | Some(ThemeVariant::Default)
                    }
                ) =>
            {
                direct_text_fill.push(InfoDirectTextFillRoute {
                    key: theme.family_mechanism_key(route),
                    rule_index,
                });
            }
            mechanism if route.disposition() == FamilyThemeDisposition::Unsupported => {
                let reason = match mechanism {
                    FamilyThemeMechanism::RuleFacet { facet, .. } => {
                        unsupported_residual_for_facet(facet)
                    }
                    FamilyThemeMechanism::OrdinalPalette {
                        target: ThemeTarget::Text,
                    } => continue,
                    FamilyThemeMechanism::OrdinalPalette { .. } => {
                        FamilyThemeResidualReason::UnsupportedOrdinalPalette
                    }
                    FamilyThemeMechanism::EffectBinding { .. } => {
                        FamilyThemeResidualReason::UnsupportedEffect
                    }
                    FamilyThemeMechanism::BaseTypography(_) => continue,
                };
                unsupported.push(InfoUnsupportedRoute {
                    key: theme.family_mechanism_key(route),
                    reason,
                });
            }
            FamilyThemeMechanism::BaseTypography(_)
            | FamilyThemeMechanism::RuleFacet { .. }
            | FamilyThemeMechanism::OrdinalPalette { .. }
            | FamilyThemeMechanism::EffectBinding { .. } => {}
        }
    }
    InfoThemeRoutes {
        unsupported: unsupported.into_boxed_slice(),
        direct_text_fill: direct_text_fill.into_boxed_slice(),
    }
}

pub(crate) fn layout_info_diagram_typed(
    model: &InfoDiagramRenderModel,
    typography_theme: &InfoTypographyThemePlan,
) -> Result<InfoDiagramLayout> {
    let _ = model.show_info;
    Ok(InfoDiagramLayout {
        // Mermaid configures the info renderer with `height = 100`, `width = 400`. Responsive
        // max-width mode omits the height attribute from the emitted SVG, but the layout model
        // still records the configured canvas dimensions.
        bounds: Some(Bounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: typography_theme.viewport_width_px(),
            max_y: typography_theme.viewport_height_px(),
        }),
        version: typography_theme.version_geometry().text().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::DeterministicTextMeasurer;

    #[test]
    fn info_layout_records_upstream_configured_canvas_size() {
        let measurer = DeterministicTextMeasurer::default();
        let config = MermaidConfig::default();
        let typography_theme = InfoTypographyThemePlan::resolve(
            None,
            &config,
            &measurer,
            &OperationWorkMeter::new(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            ),
        )
        .expect("resolve Info theme");
        let layout =
            layout_info_diagram_typed(&InfoDiagramRenderModel::default(), &typography_theme)
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
        assert_eq!(
            typography_theme.version_geometry().x(),
            INFO_VERSION_BASELINE_X_PX
        );
        assert_eq!(
            typography_theme.version_geometry().y(),
            INFO_VERSION_BASELINE_Y_PX
        );
    }
}
