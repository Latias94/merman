use std::collections::BTreeMap;
use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet, ResolvedDiagramTheme,
    ResolvedThemeStyle, Specified, ThemeCapability, ThemeTarget, ThemeTypographyProperty,
    ThemeVariant,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, InheritedFontStackOutcome,
    InheritedFontStackPlan,
};

/// Final XY Chart typography shared by layout, SVG emission, and evidence.
#[derive(Debug)]
pub(crate) struct XyChartTypographyThemePlan {
    inherited_font_stack: InheritedFontStackPlan,
    role_fonts: BTreeMap<XyChartTextRole, RoleFont>,
    source_sizes: [bool; 6],
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
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> crate::Result<Self> {
        let mut role_fonts = BTreeMap::new();
        // Source ownership is independent of which role-local font rules are present.
        let source_sizes = if theme.is_some_and(|theme| {
            theme.family_mechanism_routes().iter().any(|route| {
                matches!(
                    route.mechanism(),
                    FamilyThemeMechanism::RuleFacet {
                        facet: FamilyThemeRuleFacet::Typography(ThemeTypographyProperty::FontSize),
                        ..
                    }
                )
            })
        }) {
            XyChartTextRole::ALL.map(|role| {
                merman_core::__private::config_path_overrides_typed_default(
                    effective_config,
                    role.size_config_path(),
                )
            })
        } else {
            [false; 6]
        };
        if let Some(theme) = theme
            && theme.family_mechanism_routes().iter().any(|route| {
                matches!(
                    route.mechanism(),
                    FamilyThemeMechanism::RuleFacet {
                        target: ThemeTarget::Title
                            | ThemeTarget::AxisTitle
                            | ThemeTarget::AxisLabel
                            | ThemeTarget::Legend,
                        facet: FamilyThemeRuleFacet::Typography(
                            ThemeTypographyProperty::FontSize | ThemeTypographyProperty::FontWeight
                        ),
                        ..
                    }
                )
            })
        {
            for role in XyChartTextRole::ALL {
                let style = resolve_text_style(theme, role.target(), work_meter)?;
                let typography = style.typography_resolution();
                let source_size = source_sizes[role as usize];
                let admitted = |property| {
                    typography
                        .winner(property)
                        .filter(|origin| {
                            matches!(
                                origin.target(),
                                ThemeTarget::Title
                                    | ThemeTarget::AxisTitle
                                    | ThemeTarget::AxisLabel
                                    | ThemeTarget::Legend
                            ) && origin.ordinal().is_none()
                                && matches!(origin.variant(), None | Some(ThemeVariant::Default))
                        })
                        .map(|origin| origin.rule_index())
                };
                let size_rule = (!source_size)
                    .then(|| admitted(ThemeTypographyProperty::FontSize))
                    .flatten();
                let weight_rule = admitted(ThemeTypographyProperty::FontWeight);
                let font = RoleFont {
                    size: size_rule.and_then(|_| match typography.patch().font_size_px {
                        Specified::Value(size) => Some(f64::from(size)),
                        Specified::Clear | Specified::Unspecified => None,
                    }),
                    weight: weight_rule.and_then(|_| match typography.patch().font_weight {
                        Specified::Value(weight) => Some(weight.to_string()),
                        Specified::Clear | Specified::Unspecified => None,
                    }),
                    size_rule,
                    weight_rule,
                };
                role_fonts.insert(role, font);
            }
        }
        Ok(Self {
            inherited_font_stack: InheritedFontStackPlan::resolve_property_local(
                theme,
                effective_config,
            ),
            role_fonts,
            source_sizes,
            evidence: FamilyThemeEvidence::from_theme(theme),
            terminal_receipt: OnceLock::new(),
        })
    }

    pub(crate) fn apply_font(
        &self,
        role: XyChartTextRole,
        size: &mut f64,
        weight: &mut Option<String>,
    ) {
        if let Some(font) = self.role_fonts.get(&role) {
            if let Some(value) = font.size {
                *size = value;
            }
            weight.clone_from(&font.weight);
        }
    }

    pub(crate) fn font_weight(&self, role: XyChartTextRole) -> Option<&str> {
        self.role_fonts
            .get(&role)
            .and_then(|font| font.weight.as_deref())
    }

    pub(crate) fn source_owns_size(&self, role: XyChartTextRole) -> bool {
        self.source_sizes[role as usize]
    }

    pub(crate) fn consumes(
        &self,
        role: XyChartTextRole,
        property: ThemeTypographyProperty,
        rule: usize,
    ) -> bool {
        self.role_fonts
            .get(&role)
            .is_some_and(|font| match property {
                ThemeTypographyProperty::FontSize => font.size_rule == Some(rule),
                ThemeTypographyProperty::FontWeight => font.weight_rule == Some(rule),
                _ => false,
            })
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

/// Logical roles stay stable when XY axes change physical orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum XyChartTextRole {
    Title,
    XAxisTitle,
    YAxisTitle,
    XAxisLabel,
    YAxisLabel,
    Legend,
}

impl XyChartTextRole {
    const ALL: [Self; 6] = [
        Self::Title,
        Self::XAxisTitle,
        Self::YAxisTitle,
        Self::XAxisLabel,
        Self::YAxisLabel,
        Self::Legend,
    ];

    pub(crate) const fn target(self) -> ThemeTarget {
        match self {
            Self::Title => ThemeTarget::Title,
            Self::XAxisTitle | Self::YAxisTitle => ThemeTarget::AxisTitle,
            Self::XAxisLabel | Self::YAxisLabel => ThemeTarget::AxisLabel,
            Self::Legend => ThemeTarget::Legend,
        }
    }

    const fn size_config_path(self) -> &'static str {
        match self {
            Self::Title => "xyChart.titleFontSize",
            Self::XAxisTitle => "xyChart.xAxis.titleFontSize",
            Self::YAxisTitle => "xyChart.yAxis.titleFontSize",
            Self::XAxisLabel => "xyChart.xAxis.labelFontSize",
            Self::YAxisLabel => "xyChart.yAxis.labelFontSize",
            Self::Legend => "xyChart.legendFontSize",
        }
    }

    pub(crate) fn from_groups(groups: &[String], orientation: &str) -> Option<Self> {
        if groups.len() == 1 && groups[0] == "chart-title" {
            return Some(Self::Title);
        }
        if groups.len() != 2 {
            return None;
        }
        if groups[0] == "legend" && groups[1] == "label" {
            return Some(Self::Legend);
        }
        let x = logical_axis(&groups[0], orientation)?;
        match (x, groups[1].as_str()) {
            (true, "title") => Some(Self::XAxisTitle),
            (false, "title") => Some(Self::YAxisTitle),
            (true, "label") => Some(Self::XAxisLabel),
            (false, "label") => Some(Self::YAxisLabel),
            _ => None,
        }
    }
}

#[derive(Debug)]
struct RoleFont {
    size: Option<f64>,
    weight: Option<String>,
    size_rule: Option<usize>,
    weight_rule: Option<usize>,
}

pub(super) fn logical_axis(group: &str, orientation: &str) -> Option<bool> {
    match (orientation == "horizontal", group) {
        (true, "left-axis") | (false, "bottom-axis") => Some(true),
        (true, "top-axis") | (false, "left-axis") => Some(false),
        _ => None,
    }
}

pub(super) fn resolve_text_style(
    theme: &ResolvedDiagramTheme,
    target: ThemeTarget,
    meter: &crate::resources::OperationWorkMeter,
) -> Result<ResolvedThemeStyle, crate::resources::OperationWorkError> {
    let mut style = theme.text_style_with_work_meter(target, ThemeVariant::Default, None, meter)?;
    if matches!(target, ThemeTarget::AxisTitle | ThemeTarget::AxisLabel) {
        style.merge_from(&theme.style_with_work_meter(
            ThemeTarget::Axis,
            ThemeVariant::Default,
            None,
            meter,
        )?);
    }
    Ok(style)
}
