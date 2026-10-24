use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use crate::config::config_string;

use merman_core::presentation::PresentationConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ThemeCapability, ThemeTarget,
    ThemeTypographyProperty, ThemeVariant,
};
use crate::family::{
    DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeOutcomes,
    FamilyThemeResidualReason, InheritedFontStackOutcome, InheritedFontStackPlan,
    resolve_direct_static_fill, unsupported_residual_for_facet,
};
use crate::resources::OperationWorkMeter;

#[derive(Debug)]
pub(crate) struct WardleyPaintBinding {
    pub(crate) background_color: String,
    pub(crate) axis_color: String,
    pub(crate) axis_text_color: String,
    pub(crate) grid_color: String,
    pub(crate) component_fill: String,
    pub(crate) component_stroke: String,
    pub(crate) component_label_color: String,
    pub(crate) link_stroke: String,
    pub(crate) evolution_stroke: String,
    pub(crate) anchor_label_color: String,
    pub(crate) stage_divider_color: String,
    pub(crate) build_fill: String,
    pub(crate) build_stroke: String,
    pub(crate) buy_fill: String,
    pub(crate) outsource_fill: String,
    pub(crate) market_fill: String,
    pub(crate) annotation_fill: String,
    pub(crate) annotation_text: String,
    pub(crate) arrow_fill: String,
    pub(crate) arrow_text: String,
}

impl WardleyPaintBinding {
    fn from_config(config: &serde_json::Value) -> Self {
        let nested = |key, fallback: &str| {
            config_string(config, &["themeVariables", "wardley", key])
                .unwrap_or_else(|| fallback.to_string())
        };
        let nested_or_root = |key, root_key, fallback: &str| {
            config_string(config, &["themeVariables", "wardley", key])
                .or_else(|| config_string(config, &["themeVariables", root_key]))
                .unwrap_or_else(|| fallback.to_string())
        };

        Self {
            background_color: nested_or_root("backgroundColor", "background", "#fff"),
            axis_color: nested("axisColor", "#000"),
            axis_text_color: nested_or_root("axisTextColor", "primaryTextColor", "#222"),
            grid_color: nested("gridColor", "rgba(100, 100, 100, 0.2)"),
            component_fill: nested("componentFill", "#fff"),
            component_stroke: nested("componentStroke", "#000"),
            component_label_color: nested_or_root(
                "componentLabelColor",
                "primaryTextColor",
                "#222",
            ),
            link_stroke: nested("linkStroke", "#000"),
            evolution_stroke: nested("evolutionStroke", "#dc3545"),
            anchor_label_color: nested("anchorLabelColor", "#000"),
            stage_divider_color: nested("stageDividerColor", "#000"),
            build_fill: nested("buildFill", "#eee"),
            build_stroke: nested("buildStroke", "#000"),
            buy_fill: nested("buyFill", "#ccc"),
            outsource_fill: nested("outsourceFill", "#666"),
            market_fill: nested("marketFill", "white"),
            annotation_fill: nested("annotationBackgroundColor", "white"),
            annotation_text: config_string(
                config,
                &["themeVariables", "wardley", "annotationLabelColor"],
            )
            .unwrap_or_else(|| nested_or_root("axisTextColor", "primaryTextColor", "#222")),
            arrow_fill: nested("arrowFill", "white"),
            arrow_text: nested_or_root("axisTextColor", "primaryTextColor", "#222"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum WardleyTextRole {
    Title,
    AxisLabel,
    StageLabel,
    LinkLabel,
    NodeLabel,
    AnchorLabel,
    EvolvedLabel,
    AnnotationPoint,
    AnnotationBoxLine,
    Note,
    Accelerator,
    Deaccelerator,
}

/// Optional application outcomes collected at the existing text writer events.
#[derive(Debug, Default)]
pub(crate) struct WardleySurfaceReceipt {
    requested: bool,
    text_fills: BTreeMap<WardleyTextRole, BTreeSet<Box<str>>>,
}

impl WardleySurfaceReceipt {
    pub(crate) fn record_text(
        &mut self,
        role: WardleyTextRole,
        _emitted_class: Option<&str>,
        text: &str,
        fill: &str,
    ) {
        if self.requested && !text.trim().is_empty() {
            self.text_fills.entry(role).or_default().insert(fill.into());
        }
    }

    fn has_visible_text(&self) -> bool {
        !self.text_fills.is_empty()
    }
}

/// Final inherited Wardley font shared by annotation measurement, SVG inheritance, and evidence.
#[derive(Debug)]
pub(crate) struct WardleyTypographyThemePlan {
    paint_binding: WardleyPaintBinding,
    source_font_family_css: Option<Box<str>>,
    inherited_font_stack: InheritedFontStackPlan,
    evidence: FamilyThemeOutcomes,
    unsupported_routes: Box<[WardleyUnsupportedRoute]>,
    direct_text_fill_routes: Box<[WardleyDirectTextFillRoute]>,
    typed_text_fills: BTreeMap<WardleyTextRole, DirectStaticPaint>,
    terminal_receipt: OnceLock<WardleySurfaceReceipt>,
}

#[derive(Debug)]
struct WardleyUnsupportedRoute {
    key: FamilyThemeMechanismKey,
    reason: FamilyThemeResidualReason,
}

#[derive(Debug)]
struct WardleyDirectTextFillRoute {
    key: FamilyThemeMechanismKey,
    rule_index: usize,
}

impl WardleyTypographyThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &PresentationConfig,
        work_meter: &OperationWorkMeter,
    ) -> crate::Result<Self> {
        let mut plan = Self {
            paint_binding: WardleyPaintBinding::from_config(effective_config.as_value()),
            source_font_family_css: config_string(
                effective_config.as_value(),
                &["themeVariables", "wardley", "fontFamily"],
            )
            .map(|font| crate::config::normalize_css_font_family(&font))
            .filter(|font| !font.is_empty())
            .map(String::into_boxed_str),
            inherited_font_stack: InheritedFontStackPlan::resolve_property_local(
                theme,
                effective_config,
            ),
            evidence: FamilyThemeOutcomes::default(),
            unsupported_routes: wardley_unsupported_routes(theme),
            direct_text_fill_routes: wardley_direct_text_fill_routes(theme),
            typed_text_fills: BTreeMap::new(),
            terminal_receipt: OnceLock::new(),
        };
        let Some(theme) = theme else {
            return Ok(plan);
        };
        let text_style = theme.text_style_with_work_meter(
            ThemeTarget::Text,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let node_style = theme.text_style_with_work_meter(
            ThemeTarget::NodeLabel,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let text_fill = resolve_direct_static_fill(
            theme,
            &text_style,
            &[ThemeTarget::Text],
            DirectStaticSelectorDomain::Default,
        );
        let node_fill = resolve_direct_static_fill(
            theme,
            &node_style,
            &[ThemeTarget::Text, ThemeTarget::NodeLabel],
            DirectStaticSelectorDomain::Default,
        );
        if text_fill.is_some()
            && ![
                "themeVariables.wardley.axisTextColor",
                "themeVariables.primaryTextColor",
            ]
            .iter()
            .any(|path| {
                merman_core::__private::config_paint_overrides_typed_default(effective_config, path)
            })
        {
            plan.paint_binding.arrow_text = "#333".to_owned();
        }

        // Upstream anchors hard-code black. Only an active host label-paint request
        // introduces this adapter channel, whose foreground an authored palette owns.
        if node_fill.is_some()
            && !merman_core::__private::config_paint_overrides_typed_default(
                effective_config,
                "themeVariables.wardley.anchorLabelColor",
            )
            && merman_core::__private::config_paint_overrides_typed_default(
                effective_config,
                "themeVariables.primaryTextColor",
            )
            && let Some(color) = config_string(
                effective_config.as_value(),
                &["themeVariables", "primaryTextColor"],
            )
        {
            plan.paint_binding.anchor_label_color = color;
        }
        bind_text_fill(
            &mut plan.paint_binding.axis_text_color,
            &mut plan.typed_text_fills,
            effective_config,
            &[
                "themeVariables.wardley.axisTextColor",
                "themeVariables.primaryTextColor",
            ],
            &[
                WardleyTextRole::Title,
                WardleyTextRole::AxisLabel,
                WardleyTextRole::StageLabel,
                WardleyTextRole::LinkLabel,
                WardleyTextRole::Note,
            ],
            text_fill.as_ref(),
        );
        bind_text_fill(
            &mut plan.paint_binding.component_label_color,
            &mut plan.typed_text_fills,
            effective_config,
            &[
                "themeVariables.wardley.componentLabelColor",
                "themeVariables.primaryTextColor",
            ],
            &[WardleyTextRole::NodeLabel],
            node_fill.as_ref(),
        );
        bind_text_fill(
            &mut plan.paint_binding.anchor_label_color,
            &mut plan.typed_text_fills,
            effective_config,
            &[
                "themeVariables.wardley.anchorLabelColor",
                "themeVariables.primaryTextColor",
            ],
            &[WardleyTextRole::AnchorLabel],
            node_fill.as_ref(),
        );
        // Annotation labels own an independent foreground/background pair. Generic
        // host text applies only to the shared map surface, not those white boxes.
        Ok(plan)
    }

    pub(crate) const fn paint_binding(&self) -> &WardleyPaintBinding {
        &self.paint_binding
    }

    pub(crate) fn prepare_root_background(
        &mut self,
        root_theme: Option<&crate::diagram_theme::RootThemePlan>,
        effective_config: &PresentationConfig,
    ) {
        if root_theme.is_some_and(|plan| {
            plan.replaces_default_family_background(
                effective_config,
                "themeVariables.wardley.backgroundColor",
            )
        }) {
            self.paint_binding.background_color = "none".to_owned();
        }
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.source_font_family_css
            .as_deref()
            .unwrap_or_else(|| self.inherited_font_stack.font_family_css())
    }

    pub(crate) const fn should_write_inherited_font(&self) -> bool {
        self.source_font_family_css.is_some()
            || self.inherited_font_stack.typed_font_stack_requested()
    }

    pub(crate) fn begin_terminal_receipt(&self) -> WardleySurfaceReceipt {
        WardleySurfaceReceipt {
            requested: self.inherited_font_stack.typography_requested()
                || !self.unsupported_routes.is_empty()
                || !self.direct_text_fill_routes.is_empty(),
            text_fills: BTreeMap::new(),
        }
    }

    pub(crate) fn record_terminal(&self, receipt: WardleySurfaceReceipt) -> bool {
        self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        evidence.merge_borrowed(&self.evidence);
        for route in &self.unsupported_routes {
            evidence.mark_residual(route.key.clone(), route.reason);
        }
        let Some(receipt) = self.terminal_receipt.get() else {
            return;
        };
        self.finish_text_fill_evidence(evidence, receipt);
        self.inherited_font_stack
            .record_unsupported_typography_evidence(evidence, receipt.has_visible_text());
        let key = FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontStack);
        if !receipt.has_visible_text() {
            evidence.mark_not_applicable(key);
            return;
        }
        if self.source_font_family_css.is_some()
            && self.inherited_font_stack.typed_font_stack_requested()
        {
            evidence.mark_not_applicable(key);
            return;
        }
        if self.inherited_font_stack.typed_font_stack_active() {
            evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
        } else {
            match self.inherited_font_stack.outcome() {
                InheritedFontStackOutcome::ConfigOwned => {
                    evidence.mark_not_applicable(key);
                }
                InheritedFontStackOutcome::Typed | InheritedFontStackOutcome::Unsupported => {
                    evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
                }
                InheritedFontStackOutcome::Inactive => {}
            }
        }
    }

    fn finish_text_fill_evidence(
        &self,
        evidence: &mut FamilyThemeEvidence,
        receipt: &WardleySurfaceReceipt,
    ) {
        for route in &self.direct_text_fill_routes {
            // Mechanism keys aggregate a rule's facets. Applied fill must not erase an
            // unsupported sibling such as opacity, typography, or a gradient channel.
            if self
                .unsupported_routes
                .iter()
                .any(|unsupported| unsupported.key == route.key)
            {
                continue;
            }
            let mut visible = false;
            let mut matches = true;
            let mut capabilities = BTreeSet::new();
            for (role, paint) in &self.typed_text_fills {
                if paint.rule_index() != route.rule_index {
                    continue;
                }
                let Some(fills) = receipt.text_fills.get(role) else {
                    continue;
                };
                visible = true;
                matches &= fills.len() == 1 && fills.contains(paint.css());
                capabilities.insert(paint.capability());
            }
            if !visible {
                evidence.mark_not_applicable(route.key.clone());
            } else if matches {
                evidence.mark_applied_with_capabilities(route.key.clone(), capabilities);
            } else {
                evidence.mark_residual(
                    route.key.clone(),
                    FamilyThemeResidualReason::UnsupportedPaint,
                );
            }
        }
    }
}

fn bind_text_fill(
    color: &mut String,
    typed_text_fills: &mut BTreeMap<WardleyTextRole, DirectStaticPaint>,
    config: &PresentationConfig,
    source_paths: &[&str],
    roles: &[WardleyTextRole],
    paint: Option<&DirectStaticPaint>,
) {
    if source_paths
        .iter()
        .any(|path| merman_core::__private::config_paint_overrides_typed_default(config, path))
    {
        return;
    }
    let Some(paint) = paint else {
        return;
    };
    paint.css().clone_into(color);
    for role in roles {
        typed_text_fills.insert(*role, paint.clone());
    }
}

fn wardley_direct_text_fill_routes(
    theme: Option<&ResolvedDiagramTheme>,
) -> Box<[WardleyDirectTextFillRoute]> {
    let Some(theme) = theme else {
        return Box::new([]);
    };
    theme
        .family_mechanism_routes()
        .iter()
        .copied()
        .filter_map(|route| match route.mechanism() {
            FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::Text | ThemeTarget::NodeLabel,
                selector:
                    FamilyThemeSelectorShape::Static {
                        variant: None | Some(ThemeVariant::Default),
                    },
                facet: FamilyThemeRuleFacet::Fill(_),
            } if route.disposition() == FamilyThemeDisposition::TypedAdapter => {
                Some(WardleyDirectTextFillRoute {
                    key: theme.family_mechanism_key(route),
                    rule_index,
                })
            }
            _ => None,
        })
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

fn wardley_unsupported_routes(
    theme: Option<&ResolvedDiagramTheme>,
) -> Box<[WardleyUnsupportedRoute]> {
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
            Some(WardleyUnsupportedRoute {
                key: theme.family_mechanism_key(route),
                reason,
            })
        })
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepared_paint_preserves_defaults_and_raw_nested_root_fallbacks() {
        let work_meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let defaults = WardleyTypographyThemePlan::resolve(
            None,
            &PresentationConfig::empty_object(),
            &work_meter,
        )
        .unwrap();
        let paint = defaults.paint_binding();
        assert_eq!(paint.background_color, "#fff");
        assert_eq!(paint.axis_color, "#000");
        assert_eq!(paint.axis_text_color, "#222");
        assert_eq!(paint.grid_color, "rgba(100, 100, 100, 0.2)");
        assert_eq!(paint.component_fill, "#fff");
        assert_eq!(paint.component_stroke, "#000");
        assert_eq!(paint.component_label_color, "#222");
        assert_eq!(paint.link_stroke, "#000");
        assert_eq!(paint.evolution_stroke, "#dc3545");

        let config = PresentationConfig::from_value(serde_json::json!({
            "themeVariables": {
                "background": "var(--background)",
                "primaryTextColor": "currentColor",
                "wardley": {
                    "backgroundColor": null,
                    "axisColor": "var(--axis)",
                    "axisTextColor": "",
                    "gridColor": "transparent",
                    "componentFill": "none",
                    "componentStroke": "invalid-color",
                    "componentLabelColor": 7,
                    "linkStroke": "var(--link)",
                    "evolutionStroke": "currentColor"
                }
            }
        }));
        let prepared = WardleyTypographyThemePlan::resolve(None, &config, &work_meter).unwrap();
        let paint = prepared.paint_binding();
        assert_eq!(paint.background_color, "var(--background)");
        assert_eq!(paint.axis_color, "var(--axis)");
        assert_eq!(paint.axis_text_color, "");
        assert_eq!(paint.grid_color, "transparent");
        assert_eq!(paint.component_fill, "none");
        assert_eq!(paint.component_stroke, "invalid-color");
        assert_eq!(paint.component_label_color, "currentColor");
        assert_eq!(paint.link_stroke, "var(--link)");
        assert_eq!(paint.evolution_stroke, "currentColor");
    }
}
