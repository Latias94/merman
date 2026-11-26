use crate::config::config_bool;
use serde_json::Value;

const DEFAULT_USE_MAX_WIDTH: bool = true;

pub(crate) struct RadarConfigView<'a> {
    radar_config: &'a Value,
}

impl<'a> RadarConfigView<'a> {
    pub(crate) fn new(effective_config: &'a Value) -> Self {
        Self {
            radar_config: effective_config.get("radar").unwrap_or(&Value::Null),
        }
    }

    pub(crate) fn render_settings(&self) -> RadarRenderSettings {
        RadarRenderSettings {
            use_max_width: config_bool(self.radar_config, &["useMaxWidth"])
                .unwrap_or(DEFAULT_USE_MAX_WIDTH),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RadarRenderSettings {
    pub(crate) use_max_width: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn radar_render_settings_project_use_max_width() {
        let cfg = json!({
            "radar": {
                "useMaxWidth": false
            }
        });
        let settings = RadarConfigView::new(&cfg).render_settings();

        assert!(!settings.use_max_width);
    }
}
