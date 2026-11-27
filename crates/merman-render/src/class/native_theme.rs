#[cfg(all(test, feature = "layout-dagre"))]
mod tests {
    use std::sync::{Arc, Mutex};

    use merman_core::{MermaidEngine, ParseOptions};

    use crate::environment::{
        MeasurementProfileId, RenderEnvironment, TextMeasurementPolicy, TextMeasurementProfile,
        TextMeasurementProfileIdentity,
    };
    use crate::svg::{SvgDebugOptions, SvgRenderOptions};
    use crate::text::{DeterministicTextMeasurer, TextMeasurer, TextMetrics, TextStyle};
    use crate::theme::{NativeTheme, ThemeCompiler};
    use crate::{LayoutOptions, family};

    #[derive(Default)]
    struct SignatureMeasurer {
        default: DeterministicTextMeasurer,
        fonts: Mutex<Vec<String>>,
    }

    impl TextMeasurer for SignatureMeasurer {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            if text.contains("renderSummary") {
                self.fonts
                    .lock()
                    .unwrap()
                    .push(style.font_family.clone().unwrap_or_default());
            }
            self.default.measure(text, style)
        }
    }

    #[test]
    fn native_theme_v2_class_measures_signatures_and_keeps_authored_fill() {
        let theme = ThemeCompiler::new().compile_json(br##"{
            "schema_version":2,"extends":"paper",
            "roles":{"surface.node":"#EEEEEE","surface.emphasis":"#345678","stroke.connector":"#123456"},
            "typography":{"mono":["Signature Mono","monospace"]},
            "form":{"radius":7,"edge_style":"straight"}
        }"##, None).unwrap();
        let measurer = Arc::new(SignatureMeasurer::default());
        let profile = TextMeasurementProfile::new(
            TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("class-native-signatures").unwrap(),
                "test",
            )
            .unwrap(),
            measurer.clone(),
        );
        let environment = RenderEnvironment::deterministic()
            .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile));
        for (html, source_font) in [(false, None), (true, None), (false, Some("monospace"))] {
            let font_style = source_font.map_or_else(String::new, |font| {
                format!("style Report font-family:{font}\n")
            });
            let source = format!(
                "---\nconfig:\n  htmlLabels: {html}\n---\nclassDiagram\nclass Report {{\n+renderSummary()\n}}\n{font_style}class Draft\nstyle Draft fill:#aabbcc\nReport <|-- Draft\n"
            );
            let parsed = MermaidEngine::new()
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap();
            measurer.fonts.lock().unwrap().clear();
            let prepared = family::prepare(
                parsed.presentation(),
                &LayoutOptions::default(),
                environment
                    .begin_session_with_native_theme(NativeTheme::new(theme.clone()))
                    .unwrap(),
            )
            .unwrap();
            assert!(
                measurer
                    .fonts
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|font| match source_font {
                        Some(expected) => font == expected,
                        None => font.contains("Signature Mono"),
                    }),
                "layout must measure the method signature with the selected font"
            );
            let render = prepared
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap();
            let svg = render.svg();
            assert!(
                svg.contains(".methods-group{font-family:&quot;Signature Mono&quot;, monospace;")
                    || svg.contains(".methods-group{font-family:\"Signature Mono\", monospace;")
            );
            let dom = roxmltree::Document::parse(svg).unwrap();
            if let Some(font) = source_font {
                let declaration = format!("font-family:{font}");
                for label in ["Report", "+renderSummary()"] {
                    let text = dom
                        .descendants()
                        .find(|node| {
                            node.has_tag_name("text")
                                && node.descendants().any(|child| child.text() == Some(label))
                        })
                        .unwrap();
                    assert!(text.attribute("style").unwrap().contains(&declaration));
                }
            }
            let headers = dom
                .descendants()
                .filter(|node| node.attribute("class") == Some("class-native-header"))
                .collect::<Vec<_>>();
            assert_eq!(headers.len(), 1);
            assert_eq!(
                headers[0].attribute("style"),
                Some("fill:#345678;stroke:none")
            );
            assert!(dom.descendants().any(|node| {
                node.attribute("class") == Some("class-native-box")
                    && node.attribute("rx") == Some("7")
                    && node
                        .attribute("style")
                        .is_some_and(|style| style.contains("#aabbcc"))
            }));
            assert!(dom.descendants().any(|node| {
                node.attribute("marker-start")
                    .is_some_and(|marker| marker.contains("extension"))
                    || node
                        .attribute("marker-end")
                        .is_some_and(|marker| marker.contains("extension"))
            }));
        }
    }
}
