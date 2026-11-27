use merman_model::presentation::PresentationConfig;
use merman_theme_contract::v2::{EdgeLabelBackground, EdgeStyle};

use merman_render_core::mermaid_style::font_family_css;
use merman_theme_contract::v2::{Color, FamilyTheme};

#[derive(Debug)]
pub(crate) struct RequirementNativeStyle {
    pub(crate) emphasis: Option<String>,
    pub(crate) emphasis_ink: Option<String>,
    pub(crate) title_family: String,
    pub(crate) title_weight: String,
    pub(crate) title_ink: String,
    pub(crate) guide: String,
    pub(crate) radius: f64,
    pub(crate) border_width: f64,
    #[cfg(feature = "layout-shoal")]
    pub(crate) connector_width: Option<f64>,
    pub(crate) edge_style: EdgeStyle,
    risk_ink: Option<[String; 4]>,
}

impl RequirementNativeStyle {
    pub(crate) fn risk_ink(&self, risk: &str) -> Option<&str> {
        let risk = risk.trim();
        let [low, medium, high, other] = self.risk_ink.as_ref()?;
        Some(if risk.eq_ignore_ascii_case("low") {
            low
        } else if risk.eq_ignore_ascii_case("medium") {
            medium
        } else if risk.eq_ignore_ascii_case("high") {
            high
        } else {
            other
        })
    }
}

impl super::RequirementCssBinding {
    pub(super) fn resolve_native(
        theme: &FamilyTheme,
        config: &PresentationConfig,
        canvas: Option<Color>,
    ) -> Self {
        let roles = &theme.roles;
        let typography = &theme.typography;
        let owns = |key: &str| config.explicit_config_owns_path(key);
        let native_font = (!owns("fontFamily") && !owns("themeVariables.fontFamily"))
            .then(|| font_family_css(typography.stack(typography.body_family)));
        let native_size = (!owns("fontSize") && !owns("themeVariables.fontSize"))
            .then_some(typography.base_size_px);
        let size_css = native_size.map(|size| format!("{size}px"));
        let mut binding = Self::resolve(
            config.as_value(),
            native_font.as_deref(),
            size_css.as_deref(),
        );
        if let Some(size) = native_size {
            binding.layout.calculation_font_size = size;
        }
        let authored_fill =
            owns("themeVariables.mainBkg") || owns("themeVariables.requirementBackground");
        if owns("themeVariables.requirementBackground") && !owns("themeVariables.mainBkg") {
            binding
                .default_fill
                .clone_from(&binding.requirement_background);
        } else if !authored_fill {
            binding.default_fill = roles.surface_node.first_color().to_string();
            binding
                .requirement_background
                .clone_from(&binding.default_fill);
        }
        if !owns("themeVariables.nodeBorder") && !owns("themeVariables.requirementBorderColor") {
            binding.default_stroke = roles.stroke_border.first_color().to_string();
            binding
                .requirement_border_color
                .clone_from(&binding.default_stroke);
            binding.node_border.clone_from(&binding.default_stroke);
        }
        let authored_ink = owns("themeVariables.nodeTextColor")
            || owns("themeVariables.textColor")
            || owns("themeVariables.requirementTextColor");
        if !authored_ink {
            binding.node_text_color = roles.text_primary.to_string();
            binding
                .requirement_text_color
                .clone_from(&binding.node_text_color);
        }
        if !owns("themeVariables.relationLabelColor") {
            binding.relation_label_color = roles.text_primary.to_string();
        }
        if !owns("themeVariables.lineColor") {
            binding.line_color = roles.stroke_connector.first_color().to_string();
        }
        if !owns("themeVariables.relationColor") {
            binding.relation_color.clone_from(&binding.line_color);
        }
        let connector_width =
            (!owns("themeVariables.strokeWidth")).then_some(theme.form.connector_width);
        if let Some(width) = connector_width {
            binding.relationship_line_stroke_width = format!("{width}px");
        } else if let Some(width) = merman_render_core::config::config_css_number_or_string(
            config.as_value(),
            &["themeVariables", "strokeWidth"],
        ) {
            binding.relationship_line_stroke_width = width;
        }
        if !owns("themeVariables.edgeLabelBackground")
            && !owns("themeVariables.requirementEdgeLabelBackground")
        {
            let background = match theme.form.edge_label_bg {
                EdgeLabelBackground::Auto | EdgeLabelBackground::Canvas => {
                    canvas.unwrap_or_else(|| roles.canvas.first_color())
                }
                EdgeLabelBackground::Pill => roles.surface_node.first_color(),
                EdgeLabelBackground::Transparent => Color::TRANSPARENT,
            };
            binding.edge_label_background = background.to_string();
            binding
                .requirement_edge_label_background
                .clone_from(&binding.edge_label_background);
        }
        let title_family = if native_font.is_some() {
            font_family_css(typography.stack(typography.title_family))
        } else {
            binding.layout.font_family.clone()
        };
        let title_ink = if owns("themeVariables.textColor") {
            binding.common.text_color().to_owned()
        } else {
            roles.text_title.to_string()
        };
        let border_width = if owns("themeVariables.requirementBorderSize") {
            merman_render_core::config::config_f64_css_px(
                config.as_value(),
                &["themeVariables", "requirementBorderSize"],
            )
            .unwrap_or(theme.form.stroke_width)
        } else {
            theme.form.stroke_width
        };
        binding.native = Some(RequirementNativeStyle {
            emphasis: (!authored_fill).then(|| roles.surface_emphasis.first_color().to_string()),
            emphasis_ink: (!authored_ink).then(|| roles.text_on_emphasis.to_string()),
            title_family,
            title_weight: typography.title_weight.to_string(),
            title_ink,
            guide: roles.stroke_guide.first_color().to_string(),
            radius: theme.form.radius,
            border_width,
            #[cfg(feature = "layout-shoal")]
            connector_width,
            edge_style: theme.form.edge_style,
            risk_ink: (!authored_ink).then(|| {
                [
                    roles.status_success,
                    roles.status_warn,
                    roles.status_danger,
                    roles.status_info,
                ]
                .map(|color| color.to_string())
            }),
        });
        binding
    }
}
