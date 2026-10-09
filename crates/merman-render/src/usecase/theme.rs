//! Operation-owned Usecase visual values from Mermaid 12 styles.ts.

use crate::config::{config_f64, config_string};
use crate::text::TextStyle;
use serde_json::Value;

#[derive(Debug)]
pub(crate) struct UsecaseCssBinding {
    pub(crate) generic_font: TextStyle,
    pub(crate) root_font: TextStyle,
    pub(crate) actor_font: TextStyle,
    pub(crate) usecase_font: TextStyle,
    pub(crate) main: String,
    pub(crate) node_border: String,
    pub(crate) body: String,
    pub(crate) border: String,
    pub(crate) actor: String,
    pub(crate) actor_border: String,
    pub(crate) boundary: String,
    pub(crate) boundary_border: String,
    pub(crate) line: String,
    pub(crate) include: String,
    pub(crate) extend: String,
    pub(crate) text: String,
    pub(crate) root_text: String,
    pub(crate) actor_text: String,
    pub(crate) title: String,
    pub(crate) note: String,
    pub(crate) note_border: String,
    pub(crate) note_text: String,
    pub(crate) edge_background: String,
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
        let token = |keys: &[&str], fallback: &str| {
            keys.iter()
                .find_map(|key| config_string(config, &["themeVariables", key]))
                .unwrap_or_else(|| fallback.into())
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
            body: token(&["usecaseBkg", "mainBkg"], &main),
            border: token(
                &["usecaseBorder", "nodeBorder", "primaryColor"],
                &node_border,
            ),
            actor: token(&["usecaseActorBkg", "actorBkg", "mainBkg"], &main),
            actor_border: token(
                &["usecaseActorBorder", "actorBorder", "primaryColor"],
                &node_border,
            ),
            boundary: token(&["usecaseBoundaryBkg", "clusterBkg"], "#ffffde"),
            boundary_border: token(&["usecaseBoundaryBorder", "clusterBorder"], "#aaaa33"),
            include: token(&["usecaseIncludeLine", "lineColor"], &line),
            extend: token(&["usecaseExtendLine", "lineColor"], &line),
            root_text: token(&["textColor"], "#333"),
            actor_text: token(&["actorTextColor", "primaryTextColor"], &text),
            title: token(&["titleColor", "primaryTextColor"], &text),
            note: token(&["noteBkgColor"], "#fff5ad"),
            note_border: token(&["noteBorderColor"], "#aaaa33"),
            note_text: token(&["noteTextColor"], &text),
            edge_background: token(&["edgeLabelBackground"], &main),
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
