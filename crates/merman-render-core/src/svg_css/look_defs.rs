//! Shared Mermaid 12 insertLookDefs resources. Callers preserve their root DOM ordering.

use crate::Result;
use crate::config::{config_string, json_bool, value_at};
use crate::svg_css::SvgDiagramIdValue;
use crate::svg_output::SvgOutput;
use crate::xml::escape_attr_display;

use super::id::DROP_SHADOW_ID;

const SMALL_DROP_SHADOW_ID: &str = "drop-shadow-small";
const GRADIENT_ID: &str = "gradient";

#[derive(Debug, Clone)]
pub struct PreparedLookDefs {
    flood_color: &'static str,
    gradient: Option<(String, String)>,
}

impl PreparedLookDefs {
    pub fn flood_color(&self) -> &str {
        self.flood_color
    }

    pub fn gradient(&self) -> Option<(&str, &str)> {
        self.gradient
            .as_ref()
            .map(|(start, stop)| (start.as_str(), stop.as_str()))
    }

    pub fn uses_gradient(&self) -> bool {
        self.gradient.is_some()
    }

    /// Checks the diagram-local suffixes of the definitions this renderer will emit.
    pub fn defines_local_id(&self, id: &str, emit_shadow_defs: bool) -> bool {
        match id {
            DROP_SHADOW_ID | SMALL_DROP_SHADOW_ID => emit_shadow_defs,
            GRADIENT_ID => self.uses_gradient(),
            _ => false,
        }
    }

    pub fn with_shadow_flood_color(mut self, flood_color: &'static str) -> Self {
        self.flood_color = flood_color;
        self
    }

    pub fn new(config: &(impl crate::config::ConfigLookup + ?Sized)) -> Self {
        let flood_color = config
            .get("theme")
            .and_then(serde_json::Value::as_str)
            .and_then(|name| merman_model::config::MermaidThemeId::parse(name).ok())
            .filter(|theme| theme.is_dark())
            .map_or("#000000", |_| "#FFFFFF");
        let gradient = value_at(config, &["themeVariables", "useGradient"])
            .and_then(json_bool)
            .unwrap_or(false)
            .then(|| {
                let start = config_string(config, &["themeVariables", "gradientStart"])
                    .or_else(|| config_string(config, &["themeVariables", "primaryBorderColor"]))
                    .unwrap_or_else(|| "#9370DB".to_owned());
                let stop = config_string(config, &["themeVariables", "gradientStop"])
                    .or_else(|| config_string(config, &["themeVariables", "secondaryBorderColor"]))
                    .unwrap_or_else(|| start.clone());
                (start, stop)
            });
        Self {
            flood_color,
            gradient,
        }
    }

    pub fn write_shadow_defs(
        &self,
        out: &mut impl SvgOutput,
        diagram_id: impl SvgDiagramIdValue,
    ) -> Result<()> {
        let flood_color = self.flood_color;
        let _ = write!(
            out,
            r#"<defs><filter id="{diagram_id}-{DROP_SHADOW_ID}" height="130%" width="130%"><feDropShadow dx="4" dy="4" stdDeviation="0" flood-opacity="0.06" flood-color="{flood_color}"/></filter></defs><defs><filter id="{diagram_id}-{SMALL_DROP_SHADOW_ID}" height="150%" width="150%"><feDropShadow dx="2" dy="2" stdDeviation="0" flood-opacity="0.06" flood-color="{flood_color}"/></filter></defs>"#,
        );
        out.checkpoint()
    }

    pub fn write_gradient(
        &self,
        out: &mut impl SvgOutput,
        diagram_id: impl SvgDiagramIdValue,
    ) -> Result<()> {
        let Some((start, stop)) = &self.gradient else {
            return out.checkpoint();
        };
        let _ = write!(
            out,
            r#"<linearGradient id="{diagram_id}-{GRADIENT_ID}" gradientUnits="objectBoundingBox" x1="0%" y1="0%" x2="100%" y2="0%"><stop offset="0%" stop-color="{}" stop-opacity="1"/><stop offset="100%" stop-color="{}" stop-opacity="1"/></linearGradient>"#,
            escape_attr_display(start),
            escape_attr_display(stop)
        );
        out.checkpoint()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preparation_keeps_boolean_coercion_raw_stops_and_theme_name_darkness() {
        for (value, expected) in [
            (serde_json::json!(true), true),
            (serde_json::json!(" ON "), true),
            (serde_json::json!(-1), true),
            (serde_json::json!("false"), false),
            (serde_json::json!("unknown"), false),
        ] {
            let prepared = PreparedLookDefs::new(&serde_json::json!({
                "theme":"neo-dark", "darkMode":false,
                "themeVariables":{"useGradient":value,"primaryBorderColor":"var(--start)","gradientStop":"a&b"}
            }));
            assert_eq!(prepared.flood_color, "#FFFFFF");
            assert_eq!(prepared.gradient.is_some(), expected);
            if expected {
                assert_eq!(
                    prepared.gradient,
                    Some(("var(--start)".to_owned(), "a&b".to_owned()))
                );
            }
        }
        let fallback =
            PreparedLookDefs::new(&serde_json::json!({"themeVariables":{"useGradient":true}}));
        assert_eq!(fallback.flood_color, "#000000");
        assert_eq!(
            fallback.gradient,
            Some(("#9370DB".to_owned(), "#9370DB".to_owned()))
        );
    }
}
