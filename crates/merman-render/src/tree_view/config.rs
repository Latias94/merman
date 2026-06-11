use crate::config::{config_bool, config_f64};
use crate::theme::PresentationTheme;
use serde_json::Value;

const DEFAULT_ROW_INDENT: f64 = 10.0;
const DEFAULT_PADDING_X: f64 = 5.0;
const DEFAULT_PADDING_Y: f64 = 5.0;
const DEFAULT_LINE_THICKNESS: f64 = 1.0;
const DEFAULT_USE_MAX_WIDTH: bool = true;

pub(crate) struct TreeViewConfigView<'a> {
    effective_config: &'a Value,
    tree_view_config: &'a Value,
}

impl<'a> TreeViewConfigView<'a> {
    pub(crate) fn new(effective_config: &'a Value) -> Self {
        Self {
            effective_config,
            tree_view_config: effective_config.get("treeView").unwrap_or(&Value::Null),
        }
    }

    pub(crate) fn layout_settings(&self) -> TreeViewLayoutSettings {
        let theme = PresentationTheme::new(self.effective_config).tree_view();
        TreeViewLayoutSettings {
            row_indent: self
                .tree_view_f64("rowIndent")
                .unwrap_or(DEFAULT_ROW_INDENT)
                .max(0.0),
            padding_x: self
                .tree_view_f64("paddingX")
                .unwrap_or(DEFAULT_PADDING_X)
                .max(0.0),
            padding_y: self
                .tree_view_f64("paddingY")
                .unwrap_or(DEFAULT_PADDING_Y)
                .max(0.0),
            line_thickness: self
                .tree_view_f64("lineThickness")
                .unwrap_or(DEFAULT_LINE_THICKNESS)
                .max(0.0),
            use_max_width: self
                .tree_view_bool("useMaxWidth")
                .unwrap_or(DEFAULT_USE_MAX_WIDTH),
            label_font_size: theme.label_font_size,
        }
    }

    fn tree_view_bool(&self, key: &str) -> Option<bool> {
        config_bool(self.tree_view_config, &[key])
    }

    fn tree_view_f64(&self, key: &str) -> Option<f64> {
        config_f64(self.tree_view_config, &[key])
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TreeViewLayoutSettings {
    pub(crate) row_indent: f64,
    pub(crate) padding_x: f64,
    pub(crate) padding_y: f64,
    pub(crate) line_thickness: f64,
    pub(crate) use_max_width: bool,
    pub(crate) label_font_size: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tree_view_layout_settings_preserve_defaults_and_theme_font_size() {
        let cfg = json!({});
        let settings = TreeViewConfigView::new(&cfg).layout_settings();

        assert_eq!(settings.row_indent, DEFAULT_ROW_INDENT);
        assert_eq!(settings.padding_x, DEFAULT_PADDING_X);
        assert_eq!(settings.padding_y, DEFAULT_PADDING_Y);
        assert_eq!(settings.line_thickness, DEFAULT_LINE_THICKNESS);
        assert!(settings.use_max_width);
        assert_eq!(settings.label_font_size, 16.0);
    }

    #[test]
    fn tree_view_layout_settings_project_configured_values() {
        let cfg = json!({
            "treeView": {
                "rowIndent": "12",
                "paddingX": 7,
                "paddingY": 8,
                "lineThickness": 2,
                "useMaxWidth": false
            },
            "themeVariables": {
                "treeView": {
                    "labelFontSize": "21px"
                }
            }
        });
        let settings = TreeViewConfigView::new(&cfg).layout_settings();

        assert_eq!(settings.row_indent, 12.0);
        assert_eq!(settings.padding_x, 7.0);
        assert_eq!(settings.padding_y, 8.0);
        assert_eq!(settings.line_thickness, 2.0);
        assert!(!settings.use_max_width);
        assert_eq!(settings.label_font_size, 21.0);
    }

    #[test]
    fn tree_view_layout_settings_clamp_negative_geometry() {
        let cfg = json!({
            "treeView": {
                "rowIndent": -12,
                "paddingX": -7,
                "paddingY": -8,
                "lineThickness": -2
            }
        });
        let settings = TreeViewConfigView::new(&cfg).layout_settings();

        assert_eq!(settings.row_indent, 0.0);
        assert_eq!(settings.padding_x, 0.0);
        assert_eq!(settings.padding_y, 0.0);
        assert_eq!(settings.line_thickness, 0.0);
    }
}
