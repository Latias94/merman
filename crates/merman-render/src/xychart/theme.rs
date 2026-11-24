use merman_model::presentation::PresentationConfig;

/// Final XY Chart typography shared by layout and SVG emission.
#[derive(Debug)]
pub(crate) struct XyChartTypographyThemePlan {
    common_css: crate::svg::PreparedCommonCss,
    role_fonts: [RoleFont; 6],
    native_title_family: Option<String>,
}

impl XyChartTypographyThemePlan {
    pub(crate) fn resolve(effective_config: &PresentationConfig) -> Self {
        // Bind inherited role sizes and common CSS to the same valid root, including zero.
        let root_size = crate::config::config_theme_font_size_css_or_root_number_px_opt(
            effective_config.as_value(),
        )
        .unwrap_or(16.0);
        let root_size = crate::number_format::canonicalize_number(root_size);
        let role_fonts = XyChartTextRole::ALL.map(|role| RoleFont {
            size: role.prepare_size(effective_config.as_value(), root_size).0,
            weight: None,
        });
        let common_css = crate::svg::PreparedCommonCss::with_resolved_typography(
            effective_config.as_value(),
            crate::config::config_font_family_css(effective_config),
            format!("{}px", crate::number_format::canonical_number(root_size)),
        );
        Self {
            common_css,
            role_fonts,
            native_title_family: None,
        }
    }

    pub(crate) fn native(
        effective_config: &PresentationConfig,
        typography: &merman_theme_contract::v2::Typography,
    ) -> Self {
        let font_family = |role| crate::theme::font_family_css(typography.stack(role));
        let body_family = font_family(typography.body_family);
        let title_family = font_family(typography.title_family);
        let base = typography.base_size_px;
        let role_fonts = XyChartTextRole::ALL.map(|role| {
            let (prepared, configured) = role.prepare_size(effective_config.as_value(), base);
            let authored =
                configured && effective_config.explicit_config_owns_path(role.size_config_path());
            let size = if authored {
                prepared
            } else {
                PreparedFontSize::new(role.default_font_size() * base / 14.0, base)
            };
            let weight = if role == XyChartTextRole::Title {
                typography.title_weight
            } else {
                typography.label_weight
            };
            RoleFont {
                size,
                weight: Some(weight.to_string()),
            }
        });
        let common_css = crate::svg::PreparedCommonCss::with_resolved_typography(
            effective_config.as_value(),
            body_family,
            format!("{}px", crate::number_format::canonical_number(base)),
        );
        Self {
            common_css,
            role_fonts,
            native_title_family: Some(title_family),
        }
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

    /// Preserve the configured SVG font size even when layout uses the inherited size.
    pub(crate) fn svg_font_size(&self, role: XyChartTextRole) -> &str {
        &self.role_fonts[role as usize].size.css
    }

    pub(crate) fn font_weight(&self, role: XyChartTextRole) -> Option<&str> {
        self.role_fonts[role as usize].weight.as_deref()
    }

    pub(crate) fn common_css(&self) -> &crate::svg::PreparedCommonCss {
        &self.common_css
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.common_css.font_family()
    }

    pub(crate) fn title_family_css(&self) -> &str {
        self.native_title_family_css()
            .unwrap_or_else(|| self.font_family_css())
    }

    pub(crate) fn native_title_family_css(&self) -> Option<&str> {
        self.native_title_family.as_deref()
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
                return (PreparedFontSize::css("inherit", root_size), false);
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
                    return (PreparedFontSize::css("inherit", root_size), false);
                }
            }
            let ownership = css_font_size_value_override(css).map(|size| size.ownership());
            // Preserve host-dependent and unrepresentable values before a legacy number
            // coercion can erase their uncertainty, e.g. numeric underflow to zero.
            if ownership == Some(CssFontSizeOwnership::Unverified) {
                return (PreparedFontSize::css(css, root_size), true);
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
                return (PreparedFontSize::css(css, root_size), true);
            }
            return match ownership {
                None => (PreparedFontSize::css("inherit", root_size), false),
                Some(CssFontSizeOwnership::Inherited) => {
                    (PreparedFontSize::css(css, root_size), true)
                }
                Some(CssFontSizeOwnership::SourceOwned) => {
                    let resolved = parse_style_declaration(&format!("font-size:{css}"))
                        .filter(|declaration| !declaration.analysis().depends_on_root_font_size())
                        .and_then(|declaration| {
                            declaration.resolve_font_size_px(CssFontSizeContext::uniform(root_size))
                        });
                    (
                        PreparedFontSize::css(css, resolved.unwrap_or(root_size)),
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
        (PreparedFontSize::css("inherit", root_size), false)
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
}

#[derive(Debug)]
struct PreparedFontSize {
    css: String,
    used: f64,
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
        }
    }

    fn css(css: &str, used: f64) -> Self {
        Self {
            css: css.to_owned(),
            used: crate::number_format::canonicalize_number(used),
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
