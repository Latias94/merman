//! Operation-owned Usecase visual values from Mermaid 12 styles.ts.

use crate::config::{config_f64, config_string};
use crate::text::TextStyle;
use merman_theme_contract::v2::{EdgeStyle, MarkerStyle};
use serde_json::Value;

#[derive(Debug, Clone)]
pub(crate) struct UsecaseCssPaint {
    color: String,
    variable: Option<&'static str>,
}

impl UsecaseCssPaint {
    fn resolve(config: &Value, keys: &[&'static str], fallback: &str) -> Self {
        keys.iter()
            .find_map(|&key| {
                config_string(config, &["themeVariables", key]).map(|color| Self {
                    color,
                    variable: Some(key),
                })
            })
            .unwrap_or_else(|| Self::literal(fallback))
    }

    fn literal(color: &str) -> Self {
        Self {
            color: color.to_owned(),
            variable: None,
        }
    }

    pub(crate) fn css(&self) -> &str {
        &self.color
    }
}

impl std::fmt::Display for UsecaseCssPaint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.css())
    }
}

#[derive(Debug)]
pub(crate) struct UsecaseMermaidLook {
    pub(crate) css: crate::svg::PreparedCommonNeoCss,
    pub(crate) defs: crate::svg::PreparedLookDefs,
}

#[derive(Debug)]
pub(crate) struct UsecaseCssBinding {
    pub(crate) generic_font: TextStyle,
    pub(crate) root_font: TextStyle,
    pub(crate) actor_font: TextStyle,
    pub(crate) usecase_font: TextStyle,
    pub(crate) main: UsecaseCssPaint,
    pub(crate) node_border: UsecaseCssPaint,
    pub(crate) body: UsecaseCssPaint,
    pub(crate) border: UsecaseCssPaint,
    pub(crate) actor: UsecaseCssPaint,
    pub(crate) actor_border: UsecaseCssPaint,
    pub(crate) boundary: UsecaseCssPaint,
    pub(crate) boundary_border: UsecaseCssPaint,
    pub(crate) line: UsecaseCssPaint,
    pub(crate) include: UsecaseCssPaint,
    pub(crate) extend: UsecaseCssPaint,
    pub(crate) text: UsecaseCssPaint,
    pub(crate) root_text: UsecaseCssPaint,
    pub(crate) actor_text: UsecaseCssPaint,
    pub(crate) title: UsecaseCssPaint,
    pub(crate) note: UsecaseCssPaint,
    pub(crate) note_border: UsecaseCssPaint,
    pub(crate) note_text: UsecaseCssPaint,
    pub(crate) edge_background: UsecaseCssPaint,
    pub(crate) palette: Vec<String>,
    pub(crate) backgrounds: Vec<String>,
    pub(crate) look: String,
    pub(crate) edge_look: String,
    pub(crate) rotate: bool,
    pub(crate) hand_drawn_seed: f64,
    native_edge_style: Option<EdgeStyle>,
    pub(crate) marker_style: MarkerStyle,
    pub(crate) connector_width: f64,
    pub(crate) mermaid_look: Option<UsecaseMermaidLook>,
    pub(crate) native_title_font: Option<TextStyle>,
}

impl UsecaseCssBinding {
    pub(crate) fn new(config: &Value) -> Self {
        let token = |keys: &[&'static str], fallback: &str| {
            UsecaseCssPaint::resolve(config, keys, fallback)
        };
        let main = token(&["mainBkg", "primaryColor"], "#ECECFF");
        let node_border = token(&["nodeBorder", "primaryColor"], "#9370DB");
        let text = token(&["primaryTextColor"], "#333");
        let line = token(&["lineColor"], "#333");
        let generic_font = super::measure::text_style(config, None);
        let root_font = TextStyle {
            font_family: config_string(config, &["themeVariables", "fontFamily"])
                .or_else(|| config_string(config, &["fontFamily"])),
            font_size: generic_font.font_size,
            ..Default::default()
        };
        let redux_palette = matches!(
            config.get("theme").and_then(Value::as_str),
            Some("redux-color" | "redux-dark-color")
        );
        let palette = if redux_palette {
            raw_palette(config, "borderColorArray")
        } else {
            Vec::new()
        };
        let backgrounds = if palette.is_empty() {
            Vec::new()
        } else {
            raw_palette(config, "bkgColorArray")
        };
        let look = match config.get("look") {
            Some(Value::String(value)) => value.clone(),
            Some(Value::Number(value)) => value.to_string(),
            _ => String::new(),
        };
        let look = if !look.is_empty()
            && look
                .bytes()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == b'_' || ch == b'-')
        {
            look
        } else {
            "classic".into()
        };
        Self {
            generic_font,
            root_font,
            actor_font: super::measure::text_style(config, Some(true)),
            usecase_font: super::measure::text_style(config, Some(false)),
            body: token(&["usecaseBkg", "mainBkg", "primaryColor"], main.css()),
            border: token(
                &["usecaseBorder", "nodeBorder", "primaryColor"],
                node_border.css(),
            ),
            actor: token(
                &["usecaseActorBkg", "actorBkg", "mainBkg", "primaryColor"],
                main.css(),
            ),
            actor_border: token(
                &["usecaseActorBorder", "actorBorder", "primaryColor"],
                node_border.css(),
            ),
            boundary: token(&["usecaseBoundaryBkg", "clusterBkg"], "#ffffde"),
            boundary_border: token(&["usecaseBoundaryBorder", "clusterBorder"], "#aaaa33"),
            include: token(&["usecaseIncludeLine", "lineColor"], line.css()),
            extend: token(&["usecaseExtendLine", "lineColor"], line.css()),
            root_text: token(&["textColor"], "#333"),
            actor_text: token(&["actorTextColor", "primaryTextColor"], text.css()),
            title: token(&["titleColor", "primaryTextColor"], text.css()),
            note: token(&["noteBkgColor"], "#fff5ad"),
            note_border: token(&["noteBorderColor"], "#aaaa33"),
            note_text: token(&["noteTextColor", "primaryTextColor"], text.css()),
            edge_background: token(
                &["edgeLabelBackground", "mainBkg", "primaryColor"],
                main.css(),
            ),
            main,
            node_border,
            text,
            line,
            palette,
            backgrounds,
            look,
            edge_look: config_string(config, &["look"]).unwrap_or_else(|| "neo".into()),
            rotate: config
                .pointer("/usecase/colorScheme")
                .and_then(Value::as_str)
                == Some("rotate"),
            hand_drawn_seed: config_f64(config, &["handDrawnSeed"]).unwrap_or(0.0),
            native_edge_style: None,
            marker_style: MarkerStyle::Classic,
            connector_width: 1.0,
            mermaid_look: Some(UsecaseMermaidLook {
                css: crate::svg::PreparedCommonNeoCss::new(config),
                defs: crate::svg::PreparedLookDefs::new(config),
            }),
            native_title_font: None,
        }
    }

    pub(super) fn apply_native(
        &mut self,
        config: &merman_model::presentation::PresentationConfig,
        theme: &crate::theme::FamilyTheme,
        background: Option<crate::theme::Color>,
    ) {
        use merman_theme_contract::v2::EdgeLabelBackground;

        let roles = &theme.roles;
        if !config.explicit_config_owns_path("flowchart.curve") {
            self.native_edge_style = Some(theme.form.edge_style);
        }
        self.marker_style = theme.form.marker_style;
        self.connector_width = theme.form.connector_width;
        let paint = |binding: &mut UsecaseCssPaint, color: crate::theme::Color| {
            if !binding.variable.is_some_and(|key| {
                config.explicit_config_owns_path(&format!("themeVariables.{key}"))
            }) {
                *binding = UsecaseCssPaint::literal(&color.to_string());
            }
        };
        let node = roles.surface_node.first_color();
        let outline = roles.stroke_border.first_color();
        let connector = roles.stroke_connector.first_color();
        for binding in [&mut self.main, &mut self.body, &mut self.actor] {
            paint(binding, node);
        }
        for binding in [
            &mut self.node_border,
            &mut self.border,
            &mut self.boundary_border,
            &mut self.note_border,
        ] {
            paint(binding, outline);
        }
        for binding in [
            &mut self.actor_border,
            &mut self.line,
            &mut self.include,
            &mut self.extend,
        ] {
            paint(binding, connector);
        }
        for binding in [&mut self.text, &mut self.root_text, &mut self.actor_text] {
            paint(binding, roles.text_primary);
        }
        paint(&mut self.title, roles.text_title);
        paint(&mut self.boundary, roles.surface_container.first_color());
        paint(&mut self.note, roles.surface_callout.first_color());
        paint(&mut self.note_text, roles.text_callout);
        let canvas = background.unwrap_or_else(|| roles.canvas.first_color());
        let edge_label = match theme.form.edge_label_bg {
            EdgeLabelBackground::Auto | EdgeLabelBackground::Canvas => canvas,
            EdgeLabelBackground::Pill => node,
            EdgeLabelBackground::Transparent => crate::theme::Color::TRANSPARENT,
        };
        paint(&mut self.edge_background, edge_label);
        self.palette.clear();
        self.backgrounds.clear();
        self.mermaid_look = None;

        let typography = &theme.typography;
        let family = crate::theme::font_family_css(typography.stack(typography.body_family));
        for style in [
            &mut self.generic_font,
            &mut self.root_font,
            &mut self.actor_font,
            &mut self.usecase_font,
        ] {
            style.font_family = Some(family.clone());
            style.font_size = typography.base_size_px;
            style.font_weight = Some(typography.label_weight.to_string());
        }
        let title = crate::theme::font_family_css(typography.stack(typography.title_family));
        self.native_title_font = Some(TextStyle {
            font_family: Some(title),
            font_size: typography.base_size_px,
            font_weight: Some(typography.title_weight.to_string()),
            ..TextStyle::default()
        });
    }

    pub(crate) fn native_curve(&self) -> Option<&'static str> {
        self.native_edge_style.map(|style| match style {
            EdgeStyle::Curved => "basis",
            EdgeStyle::Orthogonal => "step",
            EdgeStyle::Straight | EdgeStyle::Sketch => "linear",
        })
    }

    pub(crate) fn sketches_edges(&self) -> bool {
        self.native_edge_style == Some(EdgeStyle::Sketch)
    }
}

fn raw_palette(config: &Value, key: &str) -> Vec<String> {
    config
        .get("themeVariables")
        .and_then(|values| values.get(key))
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| value.to_string())
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn raw_appearance_retains_distinct_node_and_edge_look_contracts() {
        for (look, node_look, edge_look) in [
            (json!(7), "7", "neo"),
            (json!("not a token"), "classic", "not a token"),
        ] {
            let binding = UsecaseCssBinding::new(&json!({"look": look}));
            assert_eq!(binding.look, node_look);
            assert_eq!(binding.edge_look, edge_look);
        }
    }
}
