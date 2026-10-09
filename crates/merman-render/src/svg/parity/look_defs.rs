//! Shared Mermaid 12 insertLookDefs resources. Callers preserve their root DOM ordering.

use super::*;

/// Mermaid Neo emits a zero-blur shadow translated by this distance on each axis.
pub(super) const NEO_SHADOW_OFFSET_PX: f64 = 4.0;

#[derive(Debug, Clone)]
pub(crate) struct PreparedLookDefs {
    flood_color: &'static str,
    gradient: Option<(String, String)>,
}

impl PreparedLookDefs {
    #[cfg(feature = "diagram-class")]
    pub(crate) fn uses_gradient(&self) -> bool {
        self.gradient.is_some()
    }
    pub(crate) fn new(config: &serde_json::Value) -> Self {
        let flood_color = config
            .get("theme")
            .and_then(serde_json::Value::as_str)
            .filter(|theme| theme.contains("dark"))
            .map(|_| "#FFFFFF")
            .unwrap_or("#000000");
        let gradient = config_bool(config, &["themeVariables", "useGradient"])
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

    pub(super) fn write_shadow_defs(
        &self,
        out: &mut impl SvgOutput,
        diagram_id: impl SvgDiagramIdValue,
    ) -> Result<()> {
        let flood_color = self.flood_color;
        let _ = write!(
            out,
            r#"<defs><filter id="{}-drop-shadow" height="130%" width="130%"><feDropShadow dx="4" dy="4" stdDeviation="0" flood-opacity="0.06" flood-color="{}"/></filter></defs><defs><filter id="{}-drop-shadow-small" height="150%" width="150%"><feDropShadow dx="2" dy="2" stdDeviation="0" flood-opacity="0.06" flood-color="{}"/></filter></defs>"#,
            diagram_id, flood_color, diagram_id, flood_color
        );
        out.checkpoint()
    }

    pub(super) fn write_gradient(
        &self,
        out: &mut impl SvgOutput,
        diagram_id: impl SvgDiagramIdValue,
    ) -> Result<()> {
        let Some((start, stop)) = &self.gradient else {
            return out.checkpoint();
        };
        let start = escape_xml(start);
        let stop = escape_xml(stop);
        let _ = write!(
            out,
            r#"<linearGradient id="{}-gradient" gradientUnits="objectBoundingBox" x1="0%" y1="0%" x2="100%" y2="0%"><stop offset="0%" stop-color="{}" stop-opacity="1"/><stop offset="100%" stop-color="{}" stop-opacity="1"/></linearGradient>"#,
            diagram_id, start, stop
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
                "theme":"custom-dark-name", "darkMode":false,
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
        assert_eq!(
            fallback.gradient,
            Some(("#9370DB".to_owned(), "#9370DB".to_owned()))
        );
    }
}
