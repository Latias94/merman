use merman_model::families::ishikawa::IshikawaDiagramModel;
use merman_render_structured::ishikawa as structured;

use crate::Result;
use crate::model::IshikawaDiagramLayout;
use crate::text::TextMeasurer;

mod theme;

pub(crate) use structured::{IshikawaConfigView, IshikawaLayoutSettings};
pub(crate) use theme::{IshikawaCssBinding, IshikawaTextThemePlan, IshikawaTypographyPlan};

pub(crate) fn layout_ishikawa_diagram_typed(
    model: &IshikawaDiagramModel,
    cfg: &IshikawaLayoutSettings,
    typography: &IshikawaTypographyPlan,
    measurer: &dyn TextMeasurer,
) -> Result<IshikawaDiagramLayout> {
    structured::layout_ishikawa_diagram_typed(
        model,
        cfg,
        typography.text_style("ishikawa-head-label"),
        typography.text_style(""),
        measurer,
    )
}

#[cfg(test)]
mod font_style_tests {
    use super::*;
    use crate::text::TextStyle;
    use merman_model::families::ishikawa::IshikawaNode;
    use serde_json::json;

    #[test]
    fn text_measurement_uses_the_render_theme_family_and_head_weight() {
        let config = json!({
            "fontFamily": "Cousine, sans-serif",
            "themeVariables": {"fontFamily": "Different, sans-serif", "fontSize": "18px"}
        });
        let config = crate::test_support::presentation_config(config);
        let typography = IshikawaTypographyPlan::resolve(&config, 16.0);
        let head = typography.text_style("ishikawa-head-label");
        assert_eq!(head.font_family.as_deref(), Some("Different,sans-serif"));
        assert_eq!(head.font_size, 14.0);
        assert_eq!(head.font_weight.as_deref(), Some("600"));

        let cause = typography.text_style("ishikawa-label cause");
        assert_eq!(cause.font_family.as_deref(), Some("Different,sans-serif"));
        assert_eq!(cause.font_size, 18.0);
        assert_eq!(cause.font_weight, None);
    }

    #[test]
    fn layout_measurement_uses_the_canonical_font_stack() {
        struct FontProbe(std::cell::RefCell<Vec<TextStyle>>);
        impl TextMeasurer for FontProbe {
            fn measure(&self, _text: &str, style: &TextStyle) -> crate::text::TextMetrics {
                self.0.borrow_mut().push(style.clone());
                crate::text::TextMetrics {
                    width: 24.0,
                    height: 14.0,
                    line_count: 1,
                }
            }
            fn measure_svg_raw_text_bbox_width_px(&self, text: &str, style: &TextStyle) -> f64 {
                self.measure(text, style).width
            }
        }
        let config = crate::test_support::presentation_config(json!({
            "fontFamily": "RootFallback",
            "themeVariables": {"fontFamily": "ThemeWinner"}
        }));
        let cfg = IshikawaConfigView::new(&config).layout_settings();
        let typography = IshikawaTypographyPlan::resolve(&config, cfg.font_size);
        let probe = FontProbe(std::cell::RefCell::new(Vec::new()));
        let model = IshikawaDiagramModel {
            root: Some(0),
            nodes: vec![IshikawaNode {
                text: "Result".to_string(),
                children: Vec::new(),
            }],
            ..Default::default()
        };
        layout_ishikawa_diagram_typed(&model, &cfg, &typography, &probe).unwrap();
        let styles = probe.0.borrow();
        assert_eq!(styles.len(), 1);
        assert_eq!(styles[0].font_family.as_deref(), Some("ThemeWinner"));
        assert_eq!(styles[0].font_weight.as_deref(), Some("600"));
    }
}
