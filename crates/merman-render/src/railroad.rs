use crate::Result;
use crate::model::RailroadDiagramLayout;
use crate::text::{TextMeasurer, TextStyle};
use merman_model::families::railroad::RailroadDiagramModel;

pub(crate) use merman_render_structured::railroad::{
    RailroadStyle, resolve_railroad_style, write_svg_contents,
};

pub(crate) fn layout_railroad_diagram_typed_for_type_with_style(
    model: &RailroadDiagramModel,
    diagram_type: &str,
    style: &RailroadStyle,
    measurer: &dyn TextMeasurer,
) -> Result<RailroadDiagramLayout> {
    merman_render_structured::railroad::layout_railroad_diagram_typed(
        model,
        diagram_type,
        style,
        &raw_svg_bbox(measurer),
    )
}

pub(crate) fn raw_svg_bbox(
    measurer: &dyn TextMeasurer,
) -> impl Fn(&str, &TextStyle) -> (f64, f64) + '_ {
    move |text, style| {
        let width = measurer.measure_svg_raw_text_bbox_width_px(text, style);
        let height = measurer.measure_svg_raw_text_bbox_height_px(text, style);
        (width, height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::TextMetrics;
    use merman_model::presentation::PresentationConfig;
    use std::cell::RefCell;

    #[derive(Default)]
    struct RailroadBBoxMeasurer {
        calls: RefCell<Vec<(&'static str, String)>>,
    }

    impl TextMeasurer for RailroadBBoxMeasurer {
        fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
            panic!("Railroad must request the raw SVG bbox probes");
        }

        fn measure_svg_raw_text_bbox_width_px(&self, text: &str, _style: &TextStyle) -> f64 {
            self.calls.borrow_mut().push(("width", text.to_owned()));
            match text {
                "other" => 31.25,
                "? anything ?" => 62.75,
                _ => 40.0,
            }
        }

        fn measure_svg_raw_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
            self.calls.borrow_mut().push(("height", text.to_owned()));
            style.font_size * 1.1
        }
    }

    #[test]
    fn railroad_variants_use_raw_svg_bbox_width_without_character_floor() {
        let parsed = merman_core::MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "railroad-beta\nexpr = choice(nonterminal(\"other\"), special(\"anything\")) ;\n",
                merman_core::ParseOptions::strict(),
            )
            .unwrap()
            .expect("railroad render model parses")
            .into_presentation();
        let merman_model::presentation::RenderSemanticModel::Railroad(model) = parsed.model()
        else {
            panic!("expected railroad render model");
        };

        let style = resolve_railroad_style(&PresentationConfig::empty_object(), None);
        for diagram_type in ["railroad", "railroadEbnf", "railroadAbnf", "railroadPeg"] {
            let measurer = RailroadBBoxMeasurer::default();
            let layout = layout_railroad_diagram_typed_for_type_with_style(
                model,
                diagram_type,
                &style,
                &measurer,
            )
            .unwrap();
            let elements = &layout.rules[0].elements;
            let other = elements
                .iter()
                .find(|element| element.label == "other")
                .expect("nonterminal element");
            let anything = elements
                .iter()
                .find(|element| element.label == "? anything ?")
                .expect("special element");

            assert_eq!(layout.diagram_type, diagram_type);
            assert_eq!(other.width, 31.25 + 20.0, "{diagram_type}");
            assert_eq!(anything.width, 62.75 + 20.0, "{diagram_type}");
            assert_eq!(other.height, 14.0 * 1.1 + 20.0, "{diagram_type}");
            assert_eq!(anything.height, other.height, "{diagram_type}");
            let calls = measurer.calls.into_inner();
            assert_eq!(
                calls
                    .iter()
                    .map(|(probe, text)| (*probe, text.as_str()))
                    .collect::<Vec<_>>(),
                [
                    ("width", "expr ="),
                    ("height", "expr ="),
                    ("width", "other"),
                    ("height", "other"),
                    ("width", "? anything ?"),
                    ("height", "? anything ?"),
                ],
            );
        }
    }
}
