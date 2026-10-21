use std::fmt::Write as _;
use std::sync::OnceLock;

use merman_core::presentation::PresentationConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ThemeCapability, ThemeTarget,
    ThemeTypographyProperty, ThemeVariant,
};
use crate::family::{
    DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeOutcomes,
    FamilyThemeResidualReason, InheritedFontStackOutcome, InheritedFontStackPlan,
    SingleTitleFillPlan, resolve_direct_static_fill,
};
use crate::model::VennDiagramLayout;
use crate::resources::OperationWorkMeter;

mod css_binding;
pub(crate) use css_binding::{VennAreaPaintBinding, VennCssBinding};

/// Final Venn inherited font stack shared by its four local stylesheet selectors and evidence.
#[derive(Debug)]
pub(crate) struct VennTypographyThemePlan {
    css_binding: VennCssBinding,
    inherited_font_stack: InheritedFontStackPlan,
    evidence: FamilyThemeOutcomes,
    text_fill: Option<crate::family::DirectStaticPaint>,
    text_fill_routes: Box<[(FamilyThemeMechanismKey, usize)]>,
    config_owns_text_fill: bool,
    terminal_receipt: OnceLock<VennTypographyThemeReceipt>,
}

impl VennTypographyThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &PresentationConfig,
        _title: Option<&str>,
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
            && merman_core::__private::config_paint_overrides_typed_default(
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
            evidence: FamilyThemeOutcomes::default(),
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
        VennTypographyThemeReceipt {
            requested: self.inherited_font_stack.typography_requested()
                || !self.text_fill_routes.is_empty(),
            ..VennTypographyThemeReceipt::default()
        }
    }

    pub(crate) fn stylesheet(
        &self,
        diagram_id: &str,
        title_fill: &str,
        set_text_color: &str,
    ) -> String {
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
            font_family = self.font_family_css(),
        );
        css
    }

    pub(crate) fn record_terminal(&self, receipt: VennTypographyThemeReceipt) -> bool {
        self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        evidence.merge_borrowed(&self.evidence);
        let key = FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontStack);
        let Some(receipt) = self.terminal_receipt.get() else {
            self.inherited_font_stack
                .record_unsupported_typography_evidence(evidence, true);
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
            return;
        };
        self.inherited_font_stack
            .record_unsupported_typography_evidence(evidence, receipt.visible_text);
        if !receipt.visible_text {
            evidence.mark_not_applicable(key);
            for (key, _) in &self.text_fill_routes {
                evidence.mark_not_applicable(key.clone());
            }
            return;
        }
        if self.inherited_font_stack.typed_font_stack_active() {
            evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
        } else if self.inherited_font_stack.outcome() == InheritedFontStackOutcome::ConfigOwned {
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
            if receipt.text_fill_terminal {
                evidence.mark_applied_with_capabilities(key.clone(), [fill.capability()]);
            } else {
                evidence.mark_not_applicable(key.clone());
            }
        }
    }
}

/// Visible text outcomes collected during normal Venn emission.
#[derive(Debug, Default)]
pub(crate) struct VennTypographyThemeReceipt {
    requested: bool,
    visible_text: bool,
    text_fill_terminal: bool,
}

impl VennTypographyThemeReceipt {
    pub(crate) fn record_title_text(&mut self, _class: &str, text: &str) {
        if self.requested {
            self.visible_text |= !text.trim().is_empty();
        }
    }

    pub(crate) fn record_circle_label(&mut self, _parent: &str, _class: &str, text: &str) {
        self.record_title_text("", text);
    }

    pub(crate) fn record_intersection_label(
        &mut self,
        _parent: &str,
        _class: &str,
        text: &str,
        source_owns_fill: bool,
        _fill: &str,
    ) {
        self.record_text_node("", text, source_owns_fill, "");
    }

    pub(crate) fn record_text_node(
        &mut self,
        _class: &str,
        text: &str,
        source_owns_fill: bool,
        _fill: &str,
    ) {
        if !self.requested || text.trim().is_empty() {
            return;
        }
        self.visible_text = true;
        self.text_fill_terminal |= !source_owns_fill;
    }
}

/// Final Venn title fill shared by stylesheet emission, the title terminal, and evidence.
#[derive(Debug)]
pub(crate) struct VennTitleThemePlan {
    fill: SingleTitleFillPlan,
}

impl VennTitleThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &PresentationConfig,
        title: Option<&str>,
        work_meter: &OperationWorkMeter,
    ) -> crate::Result<Self> {
        let Some(theme) = theme else {
            return Ok(Self {
                fill: SingleTitleFillPlan::default(),
            });
        };
        let title_present = title.is_some_and(|title| !title.trim().is_empty());
        let config_owns_fill = merman_core::__private::config_paint_overrides_typed_default(
            effective_config,
            "themeVariables.vennTitleTextColor",
        );
        let fill = SingleTitleFillPlan::resolve(
            theme,
            title_present,
            config_owns_fill,
            FamilyThemeOutcomes::default(),
            work_meter,
        )?;
        Ok(Self { fill })
    }

    pub(crate) fn fill_css(&self) -> Option<&str> {
        self.fill.fill_css()
    }

    pub(crate) fn record_title(&self) {
        if self.fill.fill_css().is_some() {
            self.fill.record_proven_terminal();
        }
    }

    pub(crate) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        self.fill.record_evidence(evidence);
    }
}

#[cfg(test)]
mod tests {
    fn collect_evidence(
        theme: &crate::diagram_theme::ResolvedDiagramTheme,
        record: impl FnOnce(&mut crate::family::FamilyThemeEvidence),
    ) -> crate::family::FamilyThemeEvidence {
        let mut evidence = crate::family::FamilyThemeEvidence::from_theme(Some(theme));
        record(&mut evidence);
        evidence
    }

    use super::VennTitleThemePlan;
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, OrdinalPalette, ThemeColorValue,
        ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
    };
    use crate::resources::RenderResourcePolicy;
    use merman_core::presentation::PresentationConfig;
    use serde_json::json;

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
        let config = PresentationConfig::from_value(json!({}));
        let plan = VennTitleThemePlan::resolve(Some(&resolved), &config, Some("Title"), &meter)
            .expect("resolve Venn title theme");
        plan.record_title();

        let evidence = collect_evidence(&resolved, |evidence| plan.record_evidence(evidence));
        assert_eq!(evidence.required_mechanisms().len(), 2);
        assert_eq!(evidence.applied().len(), 1);
        assert_eq!(evidence.not_applicable_mechanisms().len(), 1);
        assert!(evidence.residuals().is_empty());
    }
}
