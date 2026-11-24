use merman_model::presentation::PresentationConfig;

use crate::theme::{Color, FamilyTheme};

mod css_binding;
mod paint;
mod relation_width;
mod text;

pub(crate) use css_binding::ClassCssThemeBinding;
use paint::ClassPaintBinding;
use relation_width::ClassRelationWidthBinding;
pub(crate) use text::ClassTextThemePlan;

#[derive(Debug)]
pub(crate) struct ClassRelationThemePlan {
    stroke: ClassPaintBinding,
    width: ClassRelationWidthBinding,
    note_fill: ClassPaintBinding,
    note_stroke: ClassPaintBinding,
    cluster_fill: ClassPaintBinding,
    cluster_stroke: ClassPaintBinding,
    namespace_title: ClassPaintBinding,
    diagram_title: ClassPaintBinding,
    native_edge_label_background: Option<String>,
    native_edge_curve: Option<&'static str>,
}

impl ClassRelationThemePlan {
    pub(crate) fn resolve(
        theme: Option<&FamilyTheme>,
        config: &PresentationConfig,
        canvas: Option<Color>,
    ) -> Self {
        let paint = |key, fallback| ClassPaintBinding::from_config(config, key, fallback);
        let mut plan = Self {
            stroke: paint("lineColor", "#333333"),
            width: ClassRelationWidthBinding::from_config(config),
            note_fill: paint("noteBkgColor", "#fff5ad"),
            note_stroke: paint("noteBorderColor", "#aaaa33"),
            cluster_fill: paint("clusterBkg", "#ffffde"),
            cluster_stroke: paint("clusterBorder", "#aaaa33"),
            namespace_title: paint("titleColor", "#333"),
            diagram_title: paint("textColor", "#333"),
            native_edge_label_background: None,
            native_edge_curve: None,
        };
        if let Some(theme) = theme {
            let roles = &theme.roles;
            let border = roles.stroke_border.first_color().to_string();
            plan.stroke
                .apply_native(roles.stroke_connector.first_color().to_string());
            plan.width.apply_native(theme.form.connector_width);
            plan.note_fill
                .apply_native(roles.surface_callout.first_color().to_string());
            plan.note_stroke.apply_native(border.clone());
            plan.cluster_fill
                .apply_native(roles.surface_container.first_color().to_string());
            plan.cluster_stroke.apply_native(border);
            plan.namespace_title
                .apply_native(roles.text_title.to_string());
            plan.diagram_title
                .apply_native(roles.text_title.to_string());
            let background = config
                .explicit_config_owns_path("themeVariables.edgeLabelBackground")
                .then(|| {
                    crate::config::config_string(
                        config.as_value(),
                        &["themeVariables", "edgeLabelBackground"],
                    )
                })
                .flatten()
                .unwrap_or_else(|| super::native_theme::edge_background(theme, canvas));
            plan.native_edge_label_background = Some(background);
            plan.native_edge_curve = Some(super::native_theme::edge_curve(theme.form.edge_style));
        }
        plan
    }

    pub(crate) fn native_edge_label_background(&self) -> Option<&str> {
        self.native_edge_label_background.as_deref()
    }

    pub(crate) const fn native_edge_curve(&self) -> Option<&'static str> {
        self.native_edge_curve
    }

    pub(crate) fn diagram_title_css_default(&self) -> &str {
        self.diagram_title.css()
    }

    pub(crate) fn cluster_css_defaults(&self) -> (&str, &str) {
        (self.cluster_fill.css(), self.cluster_stroke.css())
    }

    pub(crate) fn relation_css_default(&self) -> &str {
        self.stroke.css()
    }

    pub(crate) fn namespace_title_css_default(&self) -> &str {
        self.namespace_title.css()
    }

    pub(crate) fn relation_width(&self) -> &ClassRelationWidthBinding {
        &self.width
    }

    pub(crate) fn note_paint(&self) -> (&str, &str) {
        (self.note_fill.css(), self.note_stroke.css())
    }
}
