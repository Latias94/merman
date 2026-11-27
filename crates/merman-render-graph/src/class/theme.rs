use merman_model::presentation::PresentationConfig;

use merman_theme_contract::v2::{Color, FamilyTheme};

mod css_binding;
mod relation_width;
mod text;

pub(crate) use css_binding::ClassCssThemeBinding;
use relation_width::ClassRelationWidthBinding;
pub use text::ClassTextThemePlan;

#[derive(Debug)]
pub struct ClassRelationThemePlan {
    stroke: String,
    width: ClassRelationWidthBinding,
    note_fill: String,
    note_stroke: String,
    cluster_fill: String,
    cluster_stroke: String,
    namespace_title: String,
    diagram_title: String,
    edge_label_background: String,
    native_edge_curve: Option<&'static str>,
}

impl ClassRelationThemePlan {
    pub fn resolve(
        theme: Option<&FamilyTheme>,
        config: &PresentationConfig,
        canvas: Option<Color>,
    ) -> Self {
        if let Some(theme) = theme {
            let roles = &theme.roles;
            let border = roles.stroke_border.first_color().to_string();
            let title = roles.text_title.to_string();
            return Self {
                stroke: roles.stroke_connector.first_color().to_string(),
                width: ClassRelationWidthBinding::from_native(theme.form.connector_width),
                note_fill: roles.surface_callout.first_color().to_string(),
                note_stroke: border.clone(),
                cluster_fill: roles.surface_container.first_color().to_string(),
                cluster_stroke: border,
                namespace_title: title.clone(),
                diagram_title: title,
                edge_label_background: super::native_theme::edge_background(theme, canvas),
                native_edge_curve: Some(super::native_theme::edge_curve(theme.form.edge_style)),
            };
        }
        let paint = |key, fallback: &str| {
            merman_render_core::config::config_string(config, &["themeVariables", key])
                .unwrap_or_else(|| fallback.to_owned())
        };
        Self {
            stroke: paint("lineColor", "#333333"),
            width: ClassRelationWidthBinding::from_config(config),
            note_fill: paint("noteBkgColor", "#fff5ad"),
            note_stroke: paint("noteBorderColor", "#aaaa33"),
            cluster_fill: paint("clusterBkg", "#ffffde"),
            cluster_stroke: paint("clusterBorder", "#aaaa33"),
            namespace_title: paint("titleColor", "#333"),
            diagram_title: paint("textColor", "#333"),
            edge_label_background: paint("edgeLabelBackground", "rgba(232,232,232, 0.8)"),
            native_edge_curve: None,
        }
    }

    pub(crate) fn edge_label_background(&self) -> &str {
        &self.edge_label_background
    }

    pub(crate) fn native_edge_label_background(&self) -> Option<&str> {
        self.native_edge_curve.map(|_| self.edge_label_background())
    }

    pub(crate) const fn native_edge_curve(&self) -> Option<&'static str> {
        self.native_edge_curve
    }

    pub(crate) fn diagram_title_css_default(&self) -> &str {
        &self.diagram_title
    }

    pub(crate) fn cluster_css_defaults(&self) -> (&str, &str) {
        (&self.cluster_fill, &self.cluster_stroke)
    }

    pub(crate) fn relation_css_default(&self) -> &str {
        &self.stroke
    }

    pub(crate) fn namespace_title_css_default(&self) -> &str {
        &self.namespace_title
    }

    pub(crate) fn relation_width(&self) -> &ClassRelationWidthBinding {
        &self.width
    }

    pub(crate) fn note_paint(&self) -> (&str, &str) {
        (&self.note_fill, &self.note_stroke)
    }
}
