use merman_model::families::ishikawa::IshikawaDiagramModel;
use merman_render_structured::ishikawa::{
    self as structured, IshikawaTextBaseline, IshikawaTextMeasurer,
};

use crate::Result;
use crate::model::IshikawaDiagramLayout;
use crate::text::{TextMeasurer, TextStyle};

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
        &HostTextMeasurer {
            typography,
            measurer,
        },
    )
}

struct HostTextMeasurer<'a> {
    typography: &'a IshikawaTypographyPlan,
    measurer: &'a dyn TextMeasurer,
}

impl IshikawaTextMeasurer for HostTextMeasurer<'_> {
    fn text_style(&self, class_name: &str) -> &TextStyle {
        self.typography.text_style(class_name)
    }

    fn measure_head_width_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.measurer
            .measure_svg_raw_text_bbox_width_px(text, style)
    }

    fn measure_label_horizontal_px(&self, text: &str, style: &TextStyle) -> (f64, (f64, f64)) {
        let advance = self
            .measurer
            .measure_svg_text_computed_length_px(text, style);
        let bbox = self
            .measurer
            .measure_svg_text_bbox_x_with_ascii_overhang(text, style);
        (advance, bbox)
    }

    fn measure_vertical_bbox_px(
        &self,
        text: &str,
        style: &TextStyle,
        baseline: IshikawaTextBaseline,
    ) -> Option<(f64, f64)> {
        let offset = match baseline {
            IshikawaTextBaseline::Middle => self
                .measurer
                .measure_svg_raw_text_middle_bbox_y_offset_px(text, style),
            IshikawaTextBaseline::Baseline => self
                .measurer
                .measure_svg_raw_text_bbox_y_offset_px(text, style),
            IshikawaTextBaseline::Hanging => self
                .measurer
                .measure_svg_raw_text_hanging_bbox_y_offset_px(text, style),
        }?;
        Some((
            offset,
            self.measurer
                .measure_svg_raw_text_bbox_height_px(text, style),
        ))
    }
}

#[cfg(test)]
mod font_style_tests {
    use super::*;
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
