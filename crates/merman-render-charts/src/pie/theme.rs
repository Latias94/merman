use std::collections::HashMap;
use std::sync::OnceLock;

use merman_model::families::pie::PieDiagramModel;
use merman_render_core::{Error, Result, config::ConfigLookup, config::selected_config_string};

mod css_binding;
pub use css_binding::{PieMermaidCss, PieTextRole};

const MERMAID_PIE_PALETTE: [(&str, &str); 12] = [
    ("themeVariables.pie1", "#ECECFF"),
    ("themeVariables.pie2", "#ffffde"),
    ("themeVariables.pie3", "hsl(80, 100%, 56.2745098039%)"),
    ("themeVariables.pie4", "hsl(240, 100%, 86.2745098039%)"),
    ("themeVariables.pie5", "hsl(60, 100%, 63.5294117647%)"),
    ("themeVariables.pie6", "hsl(80, 100%, 76.2745098039%)"),
    ("themeVariables.pie7", "hsl(300, 100%, 76.2745098039%)"),
    ("themeVariables.pie8", "hsl(180, 100%, 56.2745098039%)"),
    ("themeVariables.pie9", "hsl(0, 100%, 56.2745098039%)"),
    ("themeVariables.pie10", "hsl(300, 100%, 56.2745098039%)"),
    ("themeVariables.pie11", "hsl(150, 100%, 56.2745098039%)"),
    ("themeVariables.pie12", "hsl(0, 100%, 66.2745098039%)"),
];

/// Classic palette, CSS values, and prepared geometry shared by layout and SVG emission.
#[derive(Debug)]
pub struct PieMermaidStyle {
    css: PieMermaidCss,
    fills: HashMap<String, String>,
    content_width: OnceLock<f64>,
}

impl PieMermaidStyle {
    pub fn baseline(model: &PieDiagramModel, effective_config: &impl ConfigLookup) -> Self {
        let mut fills = HashMap::new();
        for section in &model.sections {
            let (path, fallback) = MERMAID_PIE_PALETTE[fills.len() % MERMAID_PIE_PALETTE.len()];
            fills.entry(section.label.clone()).or_insert_with(|| {
                selected_config_string(effective_config, &[path])
                    .map_or(fallback, |(_, fill)| fill)
                    .to_owned()
            });
        }
        Self {
            css: PieMermaidCss::new(effective_config),
            fills,
            content_width: OnceLock::new(),
        }
    }

    pub fn css_binding(&self) -> &PieMermaidCss {
        &self.css
    }

    pub(super) fn prepare_content_width(&self, width: f64) -> Result<()> {
        self.content_width
            .set(width)
            .map_err(|_| Error::InvalidModel {
                message: "Pie content geometry was already prepared".to_string(),
            })
    }

    pub fn content_width(&self) -> Result<f64> {
        self.content_width
            .get()
            .copied()
            .ok_or_else(|| Error::InvalidModel {
                message: "Pie content geometry has not been prepared".to_string(),
            })
    }

    pub(super) fn fill_for(&self, label: &str) -> Option<&str> {
        self.fills.get(label).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn bound_css_preserves_raw_values_and_color_fallbacks() {
        let plan = PieMermaidStyle::baseline(
            &PieDiagramModel::default(),
            &json!({"themeVariables": {
                "textColor": "var(--section-color)",
                "taskTextDarkColor": "currentColor",
                "pieStrokeWidth": 3,
                "pieOuterStrokeWidth": "calc(1px + 1em)",
                "pieOpacity": 0.4,
                "pieTitleTextSize": "1.5em"
            }}),
        );
        let css = plan.css_binding();
        assert_eq!(css.text_fill(PieTextRole::Title), "currentColor");
        assert_eq!(css.text_fill(PieTextRole::Legend), "currentColor");
        assert_eq!(css.text_fill(PieTextRole::Section), "var(--section-color)");
        assert_eq!(css.slice_stroke_width, "3");
        assert_eq!(css.outer_stroke_width, "calc(1px + 1em)");
        assert_eq!(css.outer_stroke_width_number, 2.0);
        assert_eq!(css.slice_opacity, "0.4");
        assert_eq!(css.text_size(PieTextRole::Title), "1.5em");
        assert_eq!(css.title_measurement_style("serif").font_size, 24.0);
        assert_eq!(css.legend_measurement_style("serif").font_size, 17.0);
    }
}
