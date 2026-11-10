#![cfg(feature = "diagram-state")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::{
    MeasurementProfileId, RenderEnvironment, TextMeasurementPolicy, TextMeasurementProfile,
    TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{TextMeasurer, TextMetrics, TextStyle};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct TerminalFonts(Mutex<Vec<(String, TextStyle)>>);

impl TextMeasurer for TerminalFonts {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.0
            .lock()
            .unwrap()
            .push((text.to_owned(), style.clone()));
        let weight = if style.font_weight.as_deref() == Some("700") {
            1.25
        } else {
            1.0
        };
        TextMetrics {
            width: text.chars().count() as f64 * style.font_size * weight,
            height: style.font_size * 1.5,
            line_count: 1,
        }
    }

    fn measure_html_line_width_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.measure(text, style).width
    }
}

fn render_state_labels(layout: &str, font_size: u32, label: &str) -> (String, Arc<TerminalFonts>) {
    let fonts = Arc::new(TerminalFonts::default());
    let profile = TextMeasurementProfile::new(
        TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("state-html-document-root").unwrap(),
            "1",
        )
        .unwrap(),
        fonts.clone(),
    );
    let environment = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile));
    // Transition statements use semicolons as separators; retain HTML style
    // declaration separators through the supported entity spelling.
    let transition_label = label.replace(';', "&#59;");
    let parsed = MermaidEngine::new()
        .with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "layout": layout,
                "htmlLabels": true,
                "look": "classic",
                "fontFamily": "Arial",
                "themeVariables": { "fontFamily": "Arial", "fontSize": format!("{font_size}px") },
                "state": { "minNodeWidth": 0, "wrappingWidth": 1000 }
            })),
        )
        .parse_diagram_for_render_model_sync(
            &format!("stateDiagram-v2\nstate \"{label}\" as A\nA --> B : {transition_label}\n"),
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap()
        .into_presentation();
    let prepared = family::prepare(
        &parsed,
        &LayoutOptions::headless_svg_defaults(),
        environment.begin_session().unwrap(),
    )
    .unwrap();
    let rendered = prepared
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    (rendered.svg().to_owned(), fonts)
}

fn label_sizes(svg: &str) -> Vec<(f64, f64)> {
    let document = roxmltree::Document::parse(svg).unwrap();
    let sizes = document
        .descendants()
        .filter(|node| node.has_tag_name("foreignObject"))
        .filter(|node| {
            node.descendants()
                .filter(|child| child.is_text())
                .filter_map(|child| child.text())
                .collect::<String>()
                == "ABCDEFGHIJKLMN"
        })
        .map(|node| {
            (
                node.attribute("width").unwrap().parse().unwrap(),
                node.attribute("height").unwrap().parse().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(sizes.len(), 2, "node and transition HTML labels: {svg}");
    sizes
}

#[test]
fn state_html_node_and_transition_rem_sizes_use_the_document_root() {
    for layout in ["dagre", "elk"]
        .into_iter()
        .filter(|layout| *layout != "elk" || cfg!(feature = "layout-elk"))
    {
        for font_size in [14, 24] {
            let (relative, fonts) = render_state_labels(
                layout,
                font_size,
                "<span style='font-size:2rem'>ABCDEFGHIJKLMN</span>",
            );
            let (pixels, _) = render_state_labels(
                layout,
                font_size,
                "<span style='font-size:32px'>ABCDEFGHIJKLMN</span>",
            );
            assert_eq!(label_sizes(&relative), label_sizes(&pixels));
            for (width, height) in label_sizes(&relative) {
                assert_eq!(width, 14.0 * 32.0, "{layout}/{font_size}");
                assert_eq!(height, 48.0, "{layout}/{font_size}");
            }
            let calls = fonts.0.lock().unwrap();
            assert!(
                calls
                    .iter()
                    .any(|(text, style)| { text == "ABCDEFGHIJKLMN" && style.font_size == 32.0 })
            );
        }
    }
}

#[test]
fn state_html_rem_sizes_preserve_inline_emphasis_and_declaration_order() {
    for font_size in [14, 24] {
        for (label, expected_size, weight, slant) in [
            (
                "<strong><span style='font-size:2rem;font-weight:normal'>ABCDEFGHIJKLMN</span></strong>",
                32.0,
                Some("400"),
                None,
            ),
            (
                "<span style='font-size:24px;font-size:2rem'><em>ABCDEFGHIJKLMN</em></span>",
                32.0,
                None,
                Some("italic"),
            ),
            (
                "<span style='font-size:2rem!important;font-size:24px'><strong>ABCDEFGHIJKLMN</strong></span>",
                32.0,
                Some("700"),
                None,
            ),
            (
                "<span style='font-size:2rem;font-size:24px'>ABCDEFGHIJKLMN</span>",
                24.0,
                None,
                None,
            ),
        ] {
            let (relative, fonts) = render_state_labels("dagre", font_size, label);
            let (pixels, _) =
                render_state_labels("dagre", font_size, &label.replace("2rem", "32px"));
            assert_eq!(label_sizes(&relative), label_sizes(&pixels), "{label}");
            let calls = fonts.0.lock().unwrap();
            assert!(
                calls.iter().any(|(text, style)| {
                    text == "ABCDEFGHIJKLMN"
                        && style.font_size == expected_size
                        && style.font_weight.as_deref() == weight
                        && style.font_style.as_deref() == slant
                }),
                "{font_size}/{label}: {calls:?}"
            );
        }
    }
}

#[allow(dead_code)]
mod common;
