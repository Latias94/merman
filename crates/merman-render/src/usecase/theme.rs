//! Operation-owned Usecase visual values from Mermaid 12 styles.ts.

use crate::config::{config_f64, config_string};
use crate::text::TextStyle;
use serde_json::Value;

mod compiled;
pub(crate) use compiled::{UsecaseThemePlan, UsecaseThemeReceipt, UsecaseThemeStyle};

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

    pub(super) fn literal(color: &str) -> Self {
        Self {
            color: color.to_owned(),
            variable: None,
        }
    }

    pub(crate) fn css(&self) -> &str {
        &self.color
    }

    pub(super) fn source_owned(
        &self,
        config: &merman_model::presentation::PresentationConfig,
    ) -> bool {
        self.variable.is_some_and(|key| {
            config.config_paint_overrides_typed_default(&format!("themeVariables.{key}"))
        })
    }
}

impl std::fmt::Display for UsecaseCssPaint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.css())
    }
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
    pub(crate) neo: crate::svg::PreparedCommonNeoCss,
    pub(crate) look_defs: crate::svg::PreparedLookDefs,
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
            neo: crate::svg::PreparedCommonNeoCss::new(config),
            look_defs: crate::svg::PreparedLookDefs::new(config),
        }
    }

    pub(super) fn palette_fill(&self, index: usize) -> Option<UsecaseCssPaint> {
        (!self.palette.is_empty() && !self.backgrounds.is_empty()).then(|| UsecaseCssPaint {
            color: self.backgrounds[index % self.palette.len() % self.backgrounds.len()].clone(),
            variable: Some("bkgColorArray"),
        })
    }

    pub(super) fn palette_stroke(&self, index: usize) -> Option<UsecaseCssPaint> {
        (!self.palette.is_empty()).then(|| UsecaseCssPaint {
            color: self.palette[index % self.palette.len()].clone(),
            variable: Some("borderColorArray"),
        })
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
