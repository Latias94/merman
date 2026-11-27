//! Ishikawa layout, presentation binding, and SVG emission.

use merman_model::{families::ishikawa::IshikawaDiagramModel, presentation::PresentationConfig};
use merman_render_core::{Result, text::TextMeasurer};
use merman_theme_contract::v2::FamilyTheme;

mod config;
mod layout;
mod model;
mod render;
mod svg;
mod theme;

pub use config::{IshikawaConfigView, IshikawaLayoutSettings};
pub use layout::{IshikawaTextBaseline, layout_ishikawa_diagram_typed};
pub use model::{
    IshikawaBranchLayout, IshikawaCauseLabelGroupLayout, IshikawaDiagramLayout, IshikawaHeadLayout,
    IshikawaLabelBoxLayout, IshikawaLineLayout, IshikawaPairLayout, IshikawaSubGroupLayout,
    IshikawaTextLayout,
};
pub use render::render_ishikawa_diagram_svg;
pub use svg::{IshikawaRoughPaint, write_classic_svg_body, write_hand_drawn_svg_body};
pub use theme::IshikawaTextThemePlan;

/// Binds typography before layout and prepares the terminal paint from the same configuration.
pub fn prepare_ishikawa_diagram(
    model: &IshikawaDiagramModel,
    config: &PresentationConfig,
    native: Option<&FamilyTheme>,
    measurer: &dyn TextMeasurer,
) -> Result<(IshikawaDiagramLayout, IshikawaTextThemePlan)> {
    let mut cfg = IshikawaConfigView::new(config).layout_settings();
    let mut typography = theme::IshikawaTypographyPlan::resolve(config, cfg.font_size);
    if let Some(theme) = native {
        typography.apply_native(theme, config, &mut cfg);
    }
    let layout = layout_ishikawa_diagram_typed(
        model,
        &cfg,
        typography.text_style("ishikawa-head-label"),
        typography.text_style(""),
        measurer,
    )?;
    let text_theme = IshikawaTextThemePlan::resolve(native, config, typography);
    Ok((layout, text_theme))
}

#[cfg(test)]
mod test_support {
    use merman_model::{
        MermaidConfig, OperationControl,
        presentation::PresentationConfig,
        resources::{InputResourcePolicy, ResourceProfile},
    };

    pub(super) fn presentation_config(value: serde_json::Value) -> PresentationConfig {
        PresentationConfig::from_config_controlled(
            MermaidConfig::from_value(value),
            &InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap()
    }
}

#[cfg(test)]
mod font_style_tests {
    use std::cell::RefCell;

    use merman_model::families::ishikawa::IshikawaNode;
    use merman_render_core::text::{TextMetrics, TextStyle};
    use serde_json::json;

    use super::test_support::presentation_config;
    use super::{IshikawaDiagramModel, TextMeasurer, prepare_ishikawa_diagram};

    #[test]
    fn layout_measurement_uses_the_canonical_font_stack() {
        struct FontProbe(RefCell<Vec<TextStyle>>);
        impl TextMeasurer for FontProbe {
            fn measure(&self, _text: &str, style: &TextStyle) -> TextMetrics {
                self.0.borrow_mut().push(style.clone());
                TextMetrics {
                    width: 24.0,
                    height: 14.0,
                    line_count: 1,
                }
            }
            fn measure_svg_raw_text_bbox_width_px(&self, text: &str, style: &TextStyle) -> f64 {
                self.measure(text, style).width
            }
        }
        let config = presentation_config(json!({
            "fontFamily": "RootFallback",
            "themeVariables": {"fontFamily": "ThemeWinner"}
        }));
        let probe = FontProbe(RefCell::new(Vec::new()));
        let model = IshikawaDiagramModel {
            root: Some(0),
            nodes: vec![IshikawaNode {
                text: "Result".to_string(),
                children: Vec::new(),
            }],
            ..Default::default()
        };
        prepare_ishikawa_diagram(&model, &config, None, &probe).unwrap();
        let styles = probe.0.borrow();
        assert_eq!(styles.len(), 1);
        assert_eq!(styles[0].font_family.as_deref(), Some("ThemeWinner"));
        assert_eq!(styles[0].font_weight.as_deref(), Some("600"));
    }
}
