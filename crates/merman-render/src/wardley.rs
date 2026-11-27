//! Wardley renderer integration and layout type exports.

pub(crate) use merman_render_charts::wardley::{WardleyStyle, prepare_wardley_diagram};

pub use merman_render_charts::wardley::{
    WardleyAnnotationLayout, WardleyAnnotationPointLayout, WardleyAnnotationsBoxLayout,
    WardleyArrowDirection, WardleyArrowLayout, WardleyAxesLayout, WardleyCircleLayout,
    WardleyDiagramLayout, WardleyDominantBaseline, WardleyFlowMarkersLayout, WardleyFontWeight,
    WardleyGridLayout, WardleyLineLayout, WardleyLinkLayout, WardleyNodeLayout,
    WardleyNodeShapeLayout, WardleyNoteLayout, WardleyPipelineBoxLayout, WardleyPipelineLinkLayout,
    WardleyPointLayout, WardleyRectLayout, WardleyRotationLayout, WardleySourceOverlayLayout,
    WardleyStageLayout, WardleyTextAnchor, WardleyTextLayout, WardleyTrendLayout,
};

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use serde_json::json;

    use super::prepare_wardley_diagram;
    use crate::{
        test_support::presentation_config,
        text::{TextMeasurer, TextMetrics, TextStyle},
    };

    #[test]
    fn annotations_measure_the_family_font_override_used_by_svg() {
        struct FontProbe(RefCell<Vec<String>>);
        impl TextMeasurer for FontProbe {
            fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
                assert_eq!(style.font_family.as_deref(), Some("Georgia,serif"));
                self.0.borrow_mut().push(text.to_owned());
                crate::text::DeterministicTextMeasurer::default().measure(text, style)
            }
        }
        let parsed = merman_core::MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "wardley-beta\nannotations [0.1, 0.2]\nannotation 1,[0.78, 0.82] \"Font probe\"",
                merman_core::ParseOptions::strict(),
            )
            .unwrap()
            .unwrap()
            .into_presentation();
        let merman_model::presentation::RenderSemanticModel::Wardley(model) = parsed.model() else {
            panic!("expected Wardley render model");
        };
        let measurer = FontProbe(RefCell::new(Vec::new()));
        let config = presentation_config(json!({
            "themeVariables":{"fontFamily":"Inter, sans-serif", "wardley":{"fontFamily":"Georgia, serif"}}
        }));
        let (layout, _) = prepare_wardley_diagram(model, None, &config, None, &measurer).unwrap();
        assert!(layout.annotations_box.unwrap().rect.is_some());
        assert!(
            measurer
                .0
                .borrow()
                .iter()
                .any(|text| text == "1. Font probe")
        );
    }
}
