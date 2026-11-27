use merman_model::presentation::PresentationConfig;
use merman_render_core::{
    config::{config_css_number_or_string, config_f64_css_absolute_px},
    mermaid_style::font_family_css,
    route_types::Marker,
};
use merman_theme_contract::v2::{Color, EdgeLabelBackground, EdgeStyle, FamilyTheme, MarkerStyle};

use super::{RequirementConfigView, RequirementCssBinding};

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
    pub(crate) connector_width: f64,
    pub(crate) edge_style: EdgeStyle,
    pub(crate) marker_style: MarkerStyle,
    risk_ink: Option<[String; 4]>,
}

impl RequirementNativeStyle {
    pub(crate) const ARROW_SIZE: f64 = 20.0;

    pub(crate) fn arrow_path(&self) -> Option<&'static str> {
        merman_render_core::theme_svg::marker::arrow_path(self.marker_style)
    }

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

impl RequirementCssBinding {
    pub fn edge_style(&self) -> Option<EdgeStyle> {
        self.native.as_ref().map(|style| style.edge_style)
    }

    pub(crate) fn canonical_stroke_width(&self, config: &RequirementConfigView<'_>) -> f64 {
        self.native.as_ref().map_or_else(
            || config.canonical_stroke_width(),
            |style| style.connector_width,
        )
    }

    pub fn relation_markers(&self, config: &RequirementConfigView<'_>) -> (Marker, Marker) {
        let half_stroke = self.canonical_stroke_width(config) / 2.0;
        let contains_radius = (9.0 + half_stroke).min(10.0);
        let contains = Marker {
            trim: 0.0,
            depth: 10.0 + contains_radius,
            width: 2.0 * contains_radius,
        };
        let custom_arrow = self
            .native
            .as_ref()
            .and_then(RequirementNativeStyle::arrow_path);
        let arrow = if config.look().is_neo() && custom_arrow.is_none() {
            let trim = half_stroke / 5.0_f64.sqrt();
            let half_width = 10.0 + 2.0 * trim;
            let scale = 20.0 / 25.0;
            Marker {
                trim: trim.min(5.0) * scale,
                depth: 20.0 * scale,
                width: 2.0 * half_width.min(12.5) * scale,
            }
        } else {
            Marker {
                trim: 0.0,
                depth: RequirementNativeStyle::ARROW_SIZE,
                width: RequirementNativeStyle::ARROW_SIZE,
            }
        };
        (contains, arrow)
    }

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
        if !owns("themeVariables.textColor") {
            binding.common = binding
                .common
                .clone()
                .with_text_color(&roles.text_primary.to_string());
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
        let connector_width = if owns("themeVariables.strokeWidth") {
            let path = &["themeVariables", "strokeWidth"];
            if let Some(width) = config_css_number_or_string(config.as_value(), path) {
                binding.relationship_line_stroke_width = width;
            }
            config_f64_css_absolute_px(config.as_value(), path)
                .filter(|width| *width >= 0.0)
                .unwrap_or(theme.form.connector_width)
        } else {
            let width = theme.form.connector_width;
            binding.relationship_line_stroke_width = format!("{width}px");
            width
        };
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
            connector_width,
            edge_style: theme.form.edge_style,
            marker_style: theme.form.marker_style,
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
