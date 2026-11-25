use merman_model::presentation::PresentationConfig;
use merman_theme_contract::v2::{EdgeLabelBackground, EdgeStyle};

use crate::text::TextStyle;
use crate::theme::{Color, FamilyTheme, font_family_css};

#[derive(Debug)]
pub(crate) struct ClassNativeStyle {
    pub(crate) code_style: TextStyle,
    pub(crate) title_style: TextStyle,
    pub(crate) emphasis: String,
    pub(crate) on_emphasis: String,
    pub(crate) guide: String,
    pub(crate) width: String,
    pub(crate) radius: f64,
}

impl ClassNativeStyle {
    pub(crate) fn resolve(
        theme: &FamilyTheme,
        config: &PresentationConfig,
        body_style: &TextStyle,
    ) -> Self {
        let typography = &theme.typography;
        let roles = &theme.roles;
        let has_configured_family = ["fontFamily", "themeVariables.fontFamily"]
            .into_iter()
            .any(|path| config.explicit_config_owns_path(path));
        Self {
            code_style: TextStyle {
                font_family: Some(font_family_css(typography.stack(typography.code_family))),
                ..body_style.clone()
            },
            title_style: TextStyle {
                font_family: if has_configured_family {
                    body_style.font_family.clone()
                } else {
                    Some(font_family_css(typography.stack(typography.title_family)))
                },
                font_weight: Some(typography.title_weight.to_string()),
                ..body_style.clone()
            },
            emphasis: roles.surface_emphasis.first_color().to_string(),
            on_emphasis: roles.text_on_emphasis.to_string(),
            guide: roles.stroke_guide.first_color().to_string(),
            width: theme.form.stroke_width.to_string(),
            radius: theme.form.radius,
        }
    }
}

pub(super) fn edge_background(theme: &FamilyTheme, background: Option<Color>) -> String {
    match theme.form.edge_label_bg {
        EdgeLabelBackground::Transparent => Color::TRANSPARENT,
        EdgeLabelBackground::Pill => theme.roles.surface_node.first_color(),
        EdgeLabelBackground::Auto | EdgeLabelBackground::Canvas => {
            background.unwrap_or_else(|| theme.roles.canvas.first_color())
        }
    }
    .to_string()
}

pub(super) fn edge_curve(style: EdgeStyle) -> &'static str {
    match style {
        EdgeStyle::Curved => "basis",
        EdgeStyle::Orthogonal => "step",
        EdgeStyle::Straight | EdgeStyle::Sketch => "linear",
    }
}

#[cfg(all(test, feature = "layout-dagre"))]
mod tests {
    use std::sync::{Arc, Mutex};

    use merman_core::{MermaidEngine, ParseOptions};

    use crate::environment::{
        MeasurementProfileId, RenderEnvironment, TextMeasurementPolicy, TextMeasurementProfile,
        TextMeasurementProfileIdentity,
    };
    use crate::svg::{SvgDebugOptions, SvgRenderOptions};
    use crate::text::{DeterministicTextMeasurer, TextMeasurer, TextMetrics};
    use crate::theme::{NativeTheme, ThemeCompiler};
    use crate::{LayoutOptions, family};

    use super::*;

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
        for html in [false, true] {
            let source = format!(
                "---\nconfig:\n  htmlLabels: {html}\n---\nclassDiagram\nclass Report {{\n+renderSummary()\n}}\nclass Draft\nstyle Draft fill:#aabbcc\nReport <|-- Draft\n"
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
                    .any(|font| font.contains("Signature Mono")),
                "layout must measure the method signature with the code font"
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
