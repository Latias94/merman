use std::sync::OnceLock;

use merman_model::presentation::PresentationConfig;

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
    common_css: crate::svg::PreparedCommonCss,
    role_fonts: [RoleFont; 6],
    terminal_receipt: OnceLock<XyChartTypographyTerminalSeal>,
}

#[derive(Debug, Clone, Copy)]
struct XyChartTypographyTerminalSeal {
    emitted_visible_text_count: usize,
    emitted_uncertain_text_count: usize,
    css_seen: bool,
    font_family_matches: bool,
}

/// Writer-owned proof for the final XY Chart font-family CSS and visible text stream.
#[derive(Debug)]
pub(crate) struct XyChartTypographyTerminalReceipt {
    expected_font_family: Box<str>,
    emitted_visible_text_count: usize,
    emitted_uncertain_text_count: usize,
    css_seen: bool,
    font_family_matches: bool,
}

impl XyChartTypographyThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &PresentationConfig,
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> crate::Result<Self> {
        // Bind inherited role sizes and common CSS to the same valid root, including zero.
        let root_size = crate::config::config_theme_font_size_css_or_root_number_px_opt(
            effective_config.as_value(),
        )
        .unwrap_or(16.0);
        let root_size = crate::number_format::canonicalize_number(root_size);
        let mut role_fonts = XyChartTextRole::ALL.map(|role| {
            let (size, admissible) = role.prepare_size(effective_config.as_value(), root_size);
            let source_owns_size = admissible
                && effective_config.config_path_overrides_typed_default(role.size_config_path());
            RoleFont {
                size,
                weight: None,
                source_owns_size,
                size_rule: None,
                weight_rule: None,
            }
        });
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
                let font = &mut role_fonts[role as usize];
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
                let size_rule = (!font.source_owns_size)
                    .then(|| admitted(ThemeTypographyProperty::FontSize))
                    .flatten();
                let weight_rule = admitted(ThemeTypographyProperty::FontWeight);
                if size_rule.is_some()
                    && let Specified::Value(size) = typography.patch().font_size_px
                {
                    font.size = PreparedFontSize::new(f64::from(*size), root_size);
                }
                font.weight = weight_rule.and_then(|_| match typography.patch().font_weight {
                    Specified::Value(weight) => Some(weight.to_string()),
                    Specified::Clear | Specified::Unspecified => None,
                });
                font.size_rule = size_rule;
                font.weight_rule = weight_rule;
            }
        }
        let inherited_font_stack =
            InheritedFontStackPlan::resolve_property_local(theme, effective_config);
        let common_css = crate::svg::PreparedCommonCss::with_resolved_typography(
            effective_config.as_value(),
            inherited_font_stack.font_family_css().to_owned(),
            format!("{}px", crate::number_format::canonical_number(root_size)),
        );
        Ok(Self {
            inherited_font_stack,
            common_css,
            role_fonts,
            terminal_receipt: OnceLock::new(),
        })
    }

    pub(crate) fn apply_font(
        &self,
        role: XyChartTextRole,
        size: &mut f64,
        weight: &mut Option<String>,
    ) {
        let font = &self.role_fonts[role as usize];
        *size = font.size.used;
        weight.clone_from(&font.weight);
    }

    /// Known numeric sizes stay canonical; unresolved CSS stays attached to its actual role.
    pub(crate) fn svg_font_size(&self, role: XyChartTextRole) -> &str {
        &self.role_fonts[role as usize].size.css
    }

    pub(crate) fn font_size_known(&self, role: XyChartTextRole) -> bool {
        self.role_fonts[role as usize].size.known
    }

    pub(crate) fn font_weight(&self, role: XyChartTextRole) -> Option<&str> {
        self.role_fonts[role as usize].weight.as_deref()
    }

    pub(crate) fn source_owns_size(&self, role: XyChartTextRole) -> bool {
        self.role_fonts[role as usize].source_owns_size
    }

    pub(crate) fn consumes(
        &self,
        role: XyChartTextRole,
        property: ThemeTypographyProperty,
        rule: usize,
    ) -> bool {
        let font = &self.role_fonts[role as usize];
        match property {
            ThemeTypographyProperty::FontSize => font.size_rule == Some(rule),
            ThemeTypographyProperty::FontWeight => font.weight_rule == Some(rule),
            _ => false,
        }
    }

    pub(crate) fn common_css(&self) -> &crate::svg::PreparedCommonCss {
        &self.common_css
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
                emitted_uncertain_text_count: 0,
                css_seen: false,
                font_family_matches: true,
            })
    }

    pub(crate) fn record_terminal(&self, receipt: XyChartTypographyTerminalReceipt) -> bool {
        self.terminal_receipt
            .set(XyChartTypographyTerminalSeal {
                emitted_visible_text_count: receipt.emitted_visible_text_count,
                emitted_uncertain_text_count: receipt.emitted_uncertain_text_count,
                css_seen: receipt.css_seen,
                font_family_matches: receipt.font_family_matches,
            })
            .is_ok()
    }

    pub(crate) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        if !self.inherited_font_stack.typography_requested() {
            return;
        }

        let Some(receipt) = self.terminal_receipt.get() else {
            self.inherited_font_stack
                .record_unsupported_typography_evidence(evidence, true);
            if self.inherited_font_stack.typed_font_stack_requested() {
                evidence.mark_residual(
                    FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontStack),
                    FamilyThemeResidualReason::UnsupportedTypography,
                );
            }
            return;
        };

        let has_uncertain_text = receipt.emitted_uncertain_text_count != 0;
        let has_visible_text = receipt.emitted_visible_text_count != 0 || has_uncertain_text;
        self.inherited_font_stack
            .record_unsupported_typography_evidence(evidence, has_visible_text);
        if !has_visible_text {
            if self.inherited_font_stack.typed_font_stack_requested() {
                evidence.mark_not_applicable(FamilyThemeMechanismKey::Typography(
                    ThemeTypographyProperty::FontStack,
                ));
            }
            return;
        }

        if self.inherited_font_stack.typed_font_stack_requested() {
            let key = FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontStack);
            match self.inherited_font_stack.outcome() {
                InheritedFontStackOutcome::Typed
                    if self.inherited_font_stack.typed_font_stack_active()
                        && !has_uncertain_text
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

    pub(crate) fn record_uncertain_text(&mut self) {
        self.emitted_uncertain_text_count = self.emitted_uncertain_text_count.saturating_add(1);
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

    fn configured_value(self, config: &serde_json::Value) -> Option<&serde_json::Value> {
        self.size_config_path()
            .split('.')
            .try_fold(config, |value, key| value.get(key))
    }

    fn configured_size(self, config: &serde_json::Value) -> Option<f64> {
        self.configured_value(config)
            .and_then(|value| {
                crate::config::json_f64_css_px(value).or_else(|| {
                    // Title and axis text emit SVG sizes directly. Legend dimensions also
                    // multiply their numeric option, so its existing numeric contract is separate.
                    if self == Self::Legend {
                        return None;
                    }
                    value
                        .as_str()
                        .and_then(crate::mermaid_style::resolve_mermaid_absolute_font_size_px)
                })
            })
            .map(|size| {
                if self == Self::Legend {
                    crate::number_format::canonicalize_number(size)
                } else {
                    size
                }
            })
    }

    fn prepare_size(self, config: &serde_json::Value, root_size: f64) -> (PreparedFontSize, bool) {
        use crate::mermaid_style::{
            CssFontSizeContext, CssFontSizeOwnership, css_font_size_value_override,
            parse_style_declaration,
        };

        // Legend layout performs numeric arithmetic, including its existing 1px floor.
        if self == Self::Legend {
            let configured = self.configured_size(config);
            return (
                PreparedFontSize::new(self.size_or_default(configured), root_size),
                configured.is_some(),
            );
        }
        let Some(value) = self.configured_value(config) else {
            return (
                PreparedFontSize::new(self.default_font_size(), root_size),
                false,
            );
        };
        // The shallow title option's configuration merge retains its default for null and
        // container values. Nested axis nulls instead reach the SVG attribute and inherit.
        if self == Self::Title && (value.is_null() || value.is_array() || value.is_object()) {
            return (
                PreparedFontSize::new(self.default_font_size(), root_size),
                false,
            );
        }
        let mut value = value;
        while let serde_json::Value::Array(items) = value {
            let [item] = items.as_slice() else {
                return (PreparedFontSize::css("inherit", root_size, true), false);
            };
            value = item;
        }
        if let Some(css) = value.as_str() {
            // Presentation attributes accept a value, not declaration priority or terminators.
            // Inspect the existing CSS token stream before any legacy numeric normalization.
            let mut parser = cssparser::Parser::new(css);
            while let Ok(token) = parser.next() {
                if matches!(
                    token,
                    cssparser::Token::Semicolon | cssparser::Token::Delim('!')
                ) {
                    return (PreparedFontSize::css("inherit", root_size, true), false);
                }
            }
            let ownership = css_font_size_value_override(css).map(|size| size.ownership());
            // Preserve host-dependent and unrepresentable values before a legacy number
            // coercion can erase their uncertainty, e.g. numeric underflow to zero.
            if ownership == Some(CssFontSizeOwnership::Unverified) {
                return (PreparedFontSize::css(css, root_size, false), true);
            }
            if let Some(size) = crate::config::json_f64_css_px(value)
                .or_else(|| crate::mermaid_style::resolve_mermaid_absolute_font_size_px(css))
            {
                return (PreparedFontSize::new(size, root_size), size >= 0.0);
            }
            // Unlike stylesheet font-size, an SVG attribute accepts unitless numbers.
            // A numeric token outside the measurable range is unknown, not invalid CSS.
            let mut parser = cssparser::Parser::new(css);
            if matches!(parser.next(), Ok(cssparser::Token::Number { .. })) && parser.is_exhausted()
            {
                return (PreparedFontSize::css(css, root_size, false), true);
            }
            return match ownership {
                None => (PreparedFontSize::css("inherit", root_size, true), false),
                Some(CssFontSizeOwnership::Inherited) => {
                    (PreparedFontSize::css(css, root_size, true), true)
                }
                Some(CssFontSizeOwnership::SourceOwned) => {
                    let resolved = parse_style_declaration(&format!("font-size:{css}"))
                        .filter(|declaration| !declaration.analysis().depends_on_root_font_size())
                        .and_then(|declaration| {
                            declaration.resolve_font_size_px(CssFontSizeContext::uniform(root_size))
                        });
                    (
                        PreparedFontSize::css(
                            css,
                            resolved.unwrap_or(root_size),
                            resolved.is_some(),
                        ),
                        true,
                    )
                }
                Some(CssFontSizeOwnership::Unverified) => unreachable!("handled above"),
            };
        }
        if let Some(size) = crate::config::json_f64(value) {
            return (PreparedFontSize::new(size, root_size), size >= 0.0);
        }
        // Unsupported JSON types cannot supply an SVG font-size declaration. An ignored
        // declaration inherits; it must not create a new role default or own a typed size.
        (PreparedFontSize::css("inherit", root_size, true), false)
    }

    pub(super) const fn default_font_size(self) -> f64 {
        match self {
            Self::Title => 20.0,
            Self::XAxisTitle | Self::YAxisTitle => 16.0,
            Self::XAxisLabel | Self::YAxisLabel | Self::Legend => 14.0,
        }
    }

    fn size_or_default(self, configured: Option<f64>) -> f64 {
        let size = configured.unwrap_or(self.default_font_size());
        if self == Self::Legend {
            size.max(1.0)
        } else {
            size
        }
    }

    pub(super) fn configured_font_size(self, config: &serde_json::Value) -> f64 {
        self.size_or_default(self.configured_size(config))
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
    size: PreparedFontSize,
    weight: Option<String>,
    source_owns_size: bool,
    size_rule: Option<usize>,
    weight_rule: Option<usize>,
}

#[derive(Debug)]
struct PreparedFontSize {
    css: String,
    used: f64,
    known: bool,
}

impl PreparedFontSize {
    fn new(raw: f64, root_size: f64) -> Self {
        let invalid_negative = raw < 0.0;
        let raw = crate::number_format::canonicalize_number(raw);
        Self {
            css: if invalid_negative && raw == 0.0 {
                "inherit".to_owned()
            } else {
                crate::number_format::canonical_number(raw).to_string()
            },
            used: if invalid_negative { root_size } else { raw },
            known: true,
        }
    }

    fn css(css: &str, used: f64, known: bool) -> Self {
        Self {
            css: css.to_owned(),
            used: crate::number_format::canonicalize_number(used),
            known,
        }
    }
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
