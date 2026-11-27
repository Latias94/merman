use merman_model::presentation::PresentationConfig;

use merman_theme_contract::v2::{Color, EdgeStyle, FamilyTheme};

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
    pub(crate) native_edge_label_fill: Option<Color>,
    pub(crate) sketch_edges: bool,
    pub(crate) rough_backing: bool,
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
            let label_fill = super::native_theme::edge_label_fill(theme, canvas);
            return Self {
                stroke: roles.stroke_connector.first_color().to_string(),
                width: ClassRelationWidthBinding::from_native(theme.form.connector_width),
                note_fill: roles.surface_callout.first_color().to_string(),
                note_stroke: border.clone(),
                cluster_fill: roles.surface_container.first_color().to_string(),
                cluster_stroke: border,
                namespace_title: title.clone(),
                diagram_title: title,
                edge_label_background: label_fill.to_string(),
                native_edge_curve: Some(super::native_theme::edge_curve(theme.form.edge_style)),
                native_edge_label_fill: Some(label_fill),
                sketch_edges: theme.form.edge_style == EdgeStyle::Sketch,
                rough_backing: true,
            };
        }
        let paint = |key, fallback: &str| {
            merman_render_core::config::config_string(config, &["themeVariables", key])
                .unwrap_or_else(|| fallback.to_owned())
        };
        let translucent_canvas = canvas.map_or_else(
            || {
                use merman_markup::theme_color::{ColorChannel, ThemeColor};

                config.explicit_config_owns_path("themeVariables.background")
                    && config
                        .get_str("themeVariables.background")
                        .is_some_and(|value| {
                            ThemeColor::parse(value)
                                .is_ok_and(|color| color.channel(ColorChannel::Alpha) < 1.0)
                        })
            },
            |color| color.channels()[3] < u8::MAX,
        );
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
            native_edge_label_fill: None,
            sketch_edges: false,
            // Keep Mermaid's hachure-only shapes in the compatibility presentation.
            rough_backing: translucent_canvas,
        }
    }

    pub(crate) fn edge_label_background(&self) -> &str {
        &self.edge_label_background
    }

    pub(crate) fn native_edge_label_background(&self) -> Option<&str> {
        self.native_edge_label_fill
            .map(|_| self.edge_label_background())
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
