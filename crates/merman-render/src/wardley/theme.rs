use std::sync::OnceLock;

use crate::config::config_string;

use merman_core::MermaidConfig;

use super::WardleyDiagramLayout;
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, ResolvedDiagramTheme,
    ThemeCapability, ThemeTypographyProperty,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, InheritedFontStackOutcome,
    InheritedFontStackPlan, unsupported_residual_for_facet,
};

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
        }
    }
}

const WARDLEY_TEXT_ROLE_COUNT: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WardleyTextRole {
    Title,
    AxisLabel,
    StageLabel,
    LinkLabel,
    NodeLabel,
    AnnotationPoint,
    AnnotationBoxLine,
    Note,
    Accelerator,
    Deaccelerator,
}

impl WardleyTextRole {
    const fn index(self) -> usize {
        match self {
            Self::Title => 0,
            Self::AxisLabel => 1,
            Self::StageLabel => 2,
            Self::LinkLabel => 3,
            Self::NodeLabel => 4,
            Self::AnnotationPoint => 5,
            Self::AnnotationBoxLine => 6,
            Self::Note => 7,
            Self::Accelerator => 8,
            Self::Deaccelerator => 9,
        }
    }

    const fn class(self) -> Option<&'static str> {
        match self {
            Self::Title => Some("wardley-title"),
            Self::AxisLabel => Some("wardley-axis-label"),
            Self::StageLabel => Some("wardley-stage-label"),
            Self::LinkLabel => Some("wardley-link-label"),
            Self::NodeLabel => Some("wardley-node-label"),
            Self::AnnotationPoint
            | Self::AnnotationBoxLine
            | Self::Note
            | Self::Accelerator
            | Self::Deaccelerator => None,
        }
    }
}

/// Writer-owned proof of Wardley's restored inherited font and every visible text occurrence.
#[derive(Debug)]
pub(crate) struct WardleySurfaceReceipt {
    expected_font_family_css: Box<str>,
    inherited_font_family_css: Option<Box<str>>,
    inherited_font_writer_unique: bool,
    expected_text_counts: [usize; WARDLEY_TEXT_ROLE_COUNT],
    emitted_text_counts: [usize; WARDLEY_TEXT_ROLE_COUNT],
    terminal_matches: bool,
}

impl WardleySurfaceReceipt {
    fn new(
        expected_font_family_css: &str,
        expected_text_counts: [usize; WARDLEY_TEXT_ROLE_COUNT],
    ) -> Self {
        Self {
            expected_font_family_css: expected_font_family_css.into(),
            inherited_font_family_css: None,
            inherited_font_writer_unique: true,
            expected_text_counts,
            emitted_text_counts: [0; WARDLEY_TEXT_ROLE_COUNT],
            terminal_matches: true,
        }
    }

    pub(crate) fn record_inherited_font_writer(
        &mut self,
        emitted_class: &str,
        font_family_css: &str,
    ) {
        if self.inherited_font_family_css.is_some() {
            self.inherited_font_writer_unique = false;
            return;
        }
        self.terminal_matches &= emitted_class
            .split_ascii_whitespace()
            .any(|candidate| candidate == "wardley-map");
        self.terminal_matches &= !self.expected_font_family_css.trim().is_empty();
        self.terminal_matches &= font_family_css == self.expected_font_family_css.as_ref();
        self.inherited_font_family_css = Some(font_family_css.into());
    }

    pub(crate) fn record_text(
        &mut self,
        role: WardleyTextRole,
        emitted_class: Option<&str>,
        text: &str,
    ) {
        if text.trim().is_empty() {
            return;
        }
        self.terminal_matches &= match role.class() {
            Some(expected_class) => emitted_class.is_some_and(|classes| {
                classes
                    .split_ascii_whitespace()
                    .any(|candidate| candidate == expected_class)
            }),
            None => emitted_class.is_none(),
        };
        let count = &mut self.emitted_text_counts[role.index()];
        *count = count.saturating_add(1);
    }

    fn has_visible_text(&self) -> bool {
        self.expected_text_counts.iter().copied().sum::<usize>() != 0
    }

    fn proves_font_stack(&self) -> bool {
        self.inherited_font_writer_unique
            && self.inherited_font_family_css.as_deref()
                == Some(self.expected_font_family_css.as_ref())
            && self.emitted_text_counts == self.expected_text_counts
            && self.terminal_matches
    }
}

/// Final inherited Wardley font shared by annotation measurement, SVG inheritance, and evidence.
#[derive(Debug)]
pub(crate) struct WardleyTypographyThemePlan {
    paint_binding: WardleyPaintBinding,
    inherited_font_stack: InheritedFontStackPlan,
    evidence: FamilyThemeEvidence,
    unsupported_routes: Box<[WardleyUnsupportedRoute]>,
    terminal_receipt: OnceLock<WardleySurfaceReceipt>,
}

#[derive(Debug)]
struct WardleyUnsupportedRoute {
    key: FamilyThemeMechanismKey,
    reason: FamilyThemeResidualReason,
}

impl WardleyTypographyThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
    ) -> Self {
        Self {
            paint_binding: WardleyPaintBinding::from_config(effective_config.as_value()),
            inherited_font_stack: InheritedFontStackPlan::resolve_property_local(
                theme,
                effective_config,
            ),
            evidence: FamilyThemeEvidence::from_theme(theme),
            unsupported_routes: wardley_unsupported_routes(theme),
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) const fn paint_binding(&self) -> &WardleyPaintBinding {
        &self.paint_binding
    }

    pub(crate) fn prepare_root_background(
        &mut self,
        root_theme: Option<&crate::diagram_theme::RootThemePlan>,
        effective_config: &MermaidConfig,
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
        self.inherited_font_stack.font_family_css()
    }

    pub(crate) const fn should_write_inherited_font(&self) -> bool {
        self.inherited_font_stack.typed_font_stack_requested()
    }

    pub(crate) fn begin_terminal_receipt(
        &self,
        layout: &WardleyDiagramLayout,
    ) -> WardleySurfaceReceipt {
        let non_empty = |text: &str| !text.trim().is_empty();
        let expected_text_counts = [
            layout
                .title
                .iter()
                .filter(|text| non_empty(&text.text))
                .count(),
            [&layout.axes.x_label, &layout.axes.y_label]
                .into_iter()
                .filter(|text| non_empty(&text.text))
                .count(),
            layout
                .stages
                .iter()
                .filter(|stage| non_empty(&stage.label.text))
                .count(),
            layout
                .links
                .iter()
                .filter_map(|link| link.label.as_ref())
                .filter(|text| non_empty(&text.text))
                .count(),
            layout
                .nodes
                .iter()
                .filter(|node| non_empty(&node.label_layout.text))
                .count(),
            layout
                .annotations
                .iter()
                .flat_map(|annotation| annotation.points.iter())
                .filter(|point| non_empty(&point.label.text))
                .count(),
            layout
                .annotations_box
                .iter()
                .flat_map(|annotations_box| annotations_box.lines.iter())
                .filter(|text| non_empty(&text.text))
                .count(),
            layout
                .notes
                .iter()
                .filter(|note| non_empty(&note.text.text))
                .count(),
            layout
                .accelerators
                .iter()
                .filter(|arrow| non_empty(&arrow.label.text))
                .count(),
            layout
                .deaccelerators
                .iter()
                .filter(|arrow| non_empty(&arrow.label.text))
                .count(),
        ];
        WardleySurfaceReceipt::new(self.font_family_css(), expected_text_counts)
    }

    pub(crate) fn record_terminal(&self, receipt: WardleySurfaceReceipt) -> bool {
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
            .mark_unsupported_typography_evidence(&mut evidence, receipt.has_visible_text());
        let key = FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontStack);
        if !receipt.has_visible_text() {
            evidence.mark_not_applicable(key);
            return evidence;
        }
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
        evidence
    }
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
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        DiagramThemeCompiler, DiagramThemeSpec, FontStack, ThemeTextStyle, TypographySpec,
    };

    #[test]
    fn prepared_paint_preserves_defaults_and_raw_nested_root_fallbacks() {
        let defaults = WardleyTypographyThemePlan::resolve(None, &MermaidConfig::empty_object());
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

        let config = MermaidConfig::from_value(serde_json::json!({
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
        let prepared = WardleyTypographyThemePlan::resolve(None, &config);
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

    fn typed_plan() -> WardleyTypographyThemePlan {
        let typography = ThemeTextStyle::default().with_font_stack(
            FontStack::single("monospace").expect("valid Wardley receipt font stack"),
        );
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_family_style(DiagramFamilyId::WARDLEY, typography),
            ))
            .expect("compile Wardley receipt theme");
        let resolved = theme.resolve(DiagramFamilyId::WARDLEY);
        WardleyTypographyThemePlan::resolve(
            Some(&resolved),
            &MermaidConfig::from_value(serde_json::json!({})),
        )
    }

    #[test]
    fn wardley_font_stack_receipt_rejects_missing_and_mismatched_writer_events() {
        let mut receipt = WardleySurfaceReceipt::new("monospace", [1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        receipt.record_inherited_font_writer("wardley-map", "monospace");
        receipt.record_text(WardleyTextRole::Title, Some("wrong-class"), "Map");
        assert!(!receipt.proves_font_stack());

        let mut missing_writer =
            WardleySurfaceReceipt::new("monospace", [1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        missing_writer.record_text(WardleyTextRole::Title, Some("wardley-title"), "Map");
        assert!(!missing_writer.proves_font_stack());
    }

    #[test]
    fn wardley_axis_receipt_matches_the_required_class_token() {
        let mut receipt = WardleySurfaceReceipt::new("monospace", [0, 1, 0, 0, 0, 0, 0, 0, 0, 0]);
        receipt.record_inherited_font_writer("wardley-map", "monospace");
        receipt.record_text(
            WardleyTextRole::AxisLabel,
            Some("wardley-axis-label wardley-axis-label-x"),
            "Evolution",
        );

        assert!(receipt.proves_font_stack());
    }

    #[test]
    fn unverified_wardley_writer_receipt_is_a_strict_typography_residual() {
        let plan = typed_plan();
        let mut receipt = WardleySurfaceReceipt::new("monospace", [1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        receipt.record_text(WardleyTextRole::Title, Some("wardley-title"), "Map");
        assert!(plan.record_terminal(receipt));

        let evidence = plan.finish_evidence();
        assert!(evidence.applied().is_empty());
        assert_eq!(evidence.residuals().len(), 1);
        assert_eq!(
            evidence.residuals()[0].reason(),
            FamilyThemeResidualReason::UnsupportedTypography
        );
    }
}
