use crate::config::config_bool;
pub(super) use merman_render_charts::quadrantchart::QuadrantChartLayoutSettings;
use serde_json::Value;

const DEFAULT_USE_MAX_WIDTH: bool = true;

pub(super) fn inherited_font_size_px(effective_config: &Value) -> f64 {
    crate::number_format::canonicalize_number(
        crate::config::config_theme_font_size_css_or_root_number_px(effective_config, 16.0),
    )
}

pub(crate) struct QuadrantChartConfigView<'a> {
    quadrant_config: &'a Value,
    has_render_config: bool,
}

impl<'a> QuadrantChartConfigView<'a> {
    pub(crate) fn new(effective_config: &'a Value) -> Self {
        let quadrant_config = effective_config
            .get("quadrantChart")
            .unwrap_or(&Value::Null);
        Self {
            quadrant_config,
            has_render_config: effective_config
                .get("quadrantChart")
                .is_some_and(|cfg| !has_ref_object(cfg)),
        }
    }

    pub(crate) fn render_settings(&self) -> QuadrantChartRenderSettings {
        QuadrantChartRenderSettings {
            use_max_width: self
                .render_bool("useMaxWidth")
                .unwrap_or(DEFAULT_USE_MAX_WIDTH),
        }
    }

    fn quadrant_bool(&self, key: &str) -> Option<bool> {
        config_bool(self.quadrant_config, &[key])
    }

    fn render_bool(&self, key: &str) -> Option<bool> {
        self.has_render_config
            .then(|| self.quadrant_bool(key))
            .flatten()
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct QuadrantChartRenderSettings {
    pub(crate) use_max_width: bool,
}

fn has_ref_object(v: &Value) -> bool {
    v.as_object().is_some_and(|m| m.contains_key("$ref"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn quadrantchart_render_settings_project_use_max_width() {
        let cfg = json!({
            "quadrantChart": {
                "useMaxWidth": false
            }
        });
        let settings = QuadrantChartConfigView::new(&cfg).render_settings();

        assert!(!settings.use_max_width);
    }

    #[test]
    fn quadrantchart_ref_config_uses_render_default() {
        let cfg = json!({
            "quadrantChart": {
                "$ref": "#/defs/quadrantChart",
                "useMaxWidth": false
            }
        });
        assert!(
            QuadrantChartConfigView::new(&cfg)
                .render_settings()
                .use_max_width
        );
    }
}
