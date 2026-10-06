use crate::LayoutOptions;
use crate::environment::RenderEnvironment;
use crate::family;
use crate::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_core::{Engine, MermaidConfig, ParseOptions};
use serde_json::json;

#[test]
fn usecase_math_measurement_and_output_share_the_operation_backend() {
    use crate::math::MathRenderer;
    use crate::text::{TextMetrics, TextStyle, WrapMode};
    use std::sync::Arc;

    #[derive(Debug)]
    struct FixedMath;
    impl MathRenderer for FixedMath {
        fn render_html_label(&self, text: &str, _: &MermaidConfig) -> Option<String> {
            assert_eq!(text, "$$x&lt;br/&gt;y$$");
            Some("<span>rendered formula</span>".to_owned())
        }
        fn measure_html_label(
            &self,
            text: &str,
            _: &MermaidConfig,
            _: &TextStyle,
            _: Option<f64>,
            _: WrapMode,
        ) -> Option<TextMetrics> {
            assert_eq!(text, "$$x&lt;br/&gt;y$$");
            Some(TextMetrics {
                width: 222.0,
                height: 37.0,
                line_count: 1,
            })
        }
    }
    let parsed = Engine::new()
        .with_site_config(MermaidConfig::from_value(
            json!({"layout":"dagre", "htmlLabels":true}),
        ))
        .parse_diagram_for_render_model_sync(
            "usecase-beta\nU(\"$$x<br/>y$$\")\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .with_math_renderer(Arc::new(FixedMath))
        .begin_session()
        .unwrap();
    let artifact =
        family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session).unwrap();
    let layout = artifact.layout_json().unwrap();
    let node = &layout["layout"]["UsecaseDiagram"]["nodes"][0];
    assert_eq!(node["width"], json!(262.0));
    assert_eq!(node["height"], json!(77.0));
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    assert!(rendered.svg().contains("<span>rendered formula</span>"));

    // Markdown <br/> splits math delimiters; unlike a plain label it must not
    // acquire a math requirement or invoke the backend on a literal fragment.
    let parsed = Engine::new()
        .with_site_config(MermaidConfig::from_value(
            json!({"layout":"dagre", "htmlLabels":true}),
        ))
        .parse_diagram_for_render_model_sync(
            "usecase-beta\nU(\"`$$x<br/>y$$`\")\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .with_math_renderer(Arc::new(FixedMath))
        .begin_session()
        .unwrap();
    let plan = family::plan_render(&parsed, &session).unwrap();
    assert!(!plan.required_capability_ids().any(|id| id == "math"));
    let artifact =
        family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session).unwrap();
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    assert!(!rendered.svg().contains("<span>rendered formula</span>"));
}
