use merman_core::presentation::PresentationConfig;
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

#[derive(Debug)]
struct HtmlLabel {
    width: f64,
    height: f64,
    text: String,
    elements: Vec<String>,
}

fn render_labels(label: &str, config: serde_json::Value) -> [HtmlLabel; 2] {
    render_labels_with_environment(label, config, RenderEnvironment::deterministic())
}

fn render_labels_with_environment(
    label: &str,
    config: serde_json::Value,
    environment: RenderEnvironment,
) -> [HtmlLabel; 2] {
    let source = format!(
        "%%{{init: {{\"htmlLabels\": true}}}}%%\nblock\n columns 1\n A[\"{label}\"] -- \"{label}\" --> B[\"Target\"]\n"
    );
    let engine = MermaidEngine::new().with_site_config(PresentationConfig::from_value(config));
    let parsed = engine
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .expect("parse Block HTML labels")
        .expect("detect Block");
    let session = environment.begin_session().unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare Block HTML labels");
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render Block HTML labels");
    let document = roxmltree::Document::parse(rendered.svg()).expect("well-formed XHTML SVG");
    let node = document
        .descendants()
        .find(|node| node.attribute("id") == Some("merman-A"))
        .expect("Block node A");
    let edge = document
        .descendants()
        .find(|node| node.has_tag_name("span") && node.attribute("class") == Some("edgeLabel"))
        .expect("Block edge label");
    [node, edge].map(|label| {
        let foreign_object = label
            .descendants()
            .chain(label.ancestors())
            .find(|node| node.has_tag_name("foreignObject"))
            .expect("HTML label foreignObject");
        let text = foreign_object
            .descendants()
            .filter(|node| node.is_text())
            .filter_map(|node| node.text())
            .collect();
        let elements = foreign_object
            .descendants()
            .filter(|node| node.is_element())
            .map(|node| node.tag_name().name().to_owned())
            .collect();
        assert!(foreign_object.descendants().all(|node| {
            node.attributes()
                .all(|attribute| !attribute.name().to_ascii_lowercase().starts_with("on"))
        }));
        HtmlLabel {
            width: foreign_object.attribute("width").unwrap().parse().unwrap(),
            height: foreign_object.attribute("height").unwrap().parse().unwrap(),
            text,
            elements,
        }
    })
}

fn assert_same_size(actual: &HtmlLabel, expected: &HtmlLabel) {
    assert!(
        (actual.width - expected.width).abs() <= 1.0 / 64.0,
        "label width: {actual:?} versus {expected:?}"
    );
    assert!(
        (actual.height - expected.height).abs() <= 1.0 / 64.0,
        "label height: {actual:?} versus {expected:?}"
    );
}

#[test]
fn block_html_node_and_edge_breaks_measure_the_emitted_lines() {
    let first = render_labels("AAAAAA", serde_json::json!({}));
    let second = render_labels("BBBBBB", serde_json::json!({}));
    for separator in ["<br/>", "<br>", "<br />", "\\n", "\n"] {
        let labels = render_labels(&format!("AAAAAA{separator}BBBBBB"), serde_json::json!({}));
        for (index, label) in labels.iter().enumerate() {
            assert_eq!(label.text, "AAAAAABBBBBB");
            assert_eq!(
                label.elements.iter().filter(|name| *name == "br").count(),
                1
            );
            let expected = HtmlLabel {
                width: first[index].width.max(second[index].width),
                height: first[index].height + second[index].height,
                text: String::new(),
                elements: Vec::new(),
            };
            assert_same_size(label, &expected);
        }
    }
}

#[test]
fn block_html_node_and_edge_plain_text_stays_non_markdown() {
    // A quote immediately followed by a backtick starts Mermaid's Markdown-string lexer state.
    // Keep the backticks inside a regular string to exercise literal text in both labels.
    for source in [
        "Plain text",
        "**literal**",
        "prefix `literal` suffix",
        "1 < 2 & 3 > 2",
    ] {
        for label in render_labels(source, serde_json::json!({})) {
            assert_eq!(label.text, source);
            assert!(
                label
                    .elements
                    .iter()
                    .all(|name| matches!(name.as_str(), "foreignObject" | "div" | "span" | "p"))
            );
        }
    }
}

#[test]
fn block_html_node_and_edge_inline_markup_does_not_contribute_tag_width() {
    let plain = render_labels("AAAAAA", serde_json::json!({}));
    let formatted = render_labels("<span>AAAAAA</span>", serde_json::json!({}));
    for (actual, expected) in formatted.iter().zip(&plain) {
        assert_eq!(actual.text, "AAAAAA");
        assert!(
            actual
                .elements
                .iter()
                .filter(|name| *name == "span")
                .count()
                >= 2
        );
        assert_same_size(actual, expected);
    }
}

#[test]
fn block_html_node_and_edge_entities_stay_text_after_one_html_decode() {
    for (source, expected) in [
        ("Alpha &amp; Beta", "Alpha & Beta"),
        ("A&nbsp;B", "A\u{00a0}B"),
        ("&copy;", "©"),
        ("&lt;br/&gt;", "<br/>"),
        ("&amp;lt;br/&amp;gt;", "&lt;br/&gt;"),
    ] {
        for label in render_labels(source, serde_json::json!({})) {
            assert_eq!(label.text, expected, "{source}");
            assert!(!label.elements.iter().any(|name| name == "br"));
        }
    }
    let encoded = render_labels("Alpha &amp; Beta", serde_json::json!({}));
    let literal = render_labels("Alpha & Beta", serde_json::json!({}));
    for (actual, expected) in encoded.iter().zip(&literal) {
        assert_same_size(actual, expected);
    }
}

#[test]
fn block_html_node_and_edge_sanitization_precedes_measurement() {
    let sanitized = render_labels(
        "A<script>discard</script><span onclick='discard()'>B</span><br/>C",
        serde_json::json!({"securityLevel": "strict"}),
    );
    let reference = render_labels("AB<br/>C", serde_json::json!({}));
    for (actual, expected) in sanitized.iter().zip(&reference) {
        assert_eq!(actual.text, "ABC");
        assert!(!actual.elements.iter().any(|name| name == "script"));
        assert_same_size(actual, expected);
    }

    let configured = render_labels(
        "A<br/>B",
        serde_json::json!({"dompurifyConfig": {"FORBID_TAGS": ["br"]}}),
    );
    let reference = render_labels("AB", serde_json::json!({}));
    for (actual, expected) in configured.iter().zip(&reference) {
        assert_eq!(actual.text, "AB");
        assert!(!actual.elements.iter().any(|name| name == "br"));
        assert_same_size(actual, expected);
    }
}

#[test]
fn block_unwrapped_inline_font_size_sizes_both_foreign_objects() {
    let reference = render_labels("MMMM", serde_json::json!({}));
    let styled = render_labels(
        "<span style='font-size:64px'>MMMM</span>",
        serde_json::json!({}),
    );
    for (actual, reference) in styled.iter().zip(&reference) {
        assert_eq!(actual.text, "MMMM");
        assert!((actual.width - reference.width * 4.0).abs() <= 1.0 / 64.0);
        assert_eq!(actual.height, 96.0);
    }
}

#[test]
fn block_unwrapped_inline_fonts_resolve_parent_relative_sizes_and_restore_siblings() {
    let nested = render_labels(
        "<span style='font-size:2em'><span style='font-size:150%'>MMMM</span></span>",
        serde_json::json!({}),
    );
    let absolute = render_labels(
        "<span style='font-size:48px'>MMMM</span>",
        serde_json::json!({}),
    );
    for (actual, expected) in nested.iter().zip(&absolute) {
        assert_same_size(actual, expected);
    }
    let nested = render_labels(
        "<span style='font-size:2em'><span style='font-size:2rem'>MMMM</span></span>",
        serde_json::json!({}),
    );
    let absolute = render_labels(
        "<span style='font-size:32px'>MMMM</span>",
        serde_json::json!({}),
    );
    for (actual, expected) in nested.iter().zip(&absolute) {
        assert_same_size(actual, expected);
    }

    let mixed = render_labels(
        "<span style='font-size:64px'>MM</span>MM<br/><span style='font-size:32px'>MMMM</span>",
        serde_json::json!({}),
    );
    let large = render_labels(
        "<span style='font-size:64px'>MM</span>",
        serde_json::json!({}),
    );
    let small = render_labels("MM", serde_json::json!({}));
    for ((actual, large), small) in mixed.iter().zip(&large).zip(&small) {
        assert!((actual.width - large.width - small.width).abs() <= 1.0 / 64.0);
        assert_eq!(actual.height, 96.0 + 48.0);
    }
}

#[test]
fn block_unwrapped_inline_fonts_follow_the_sanitized_terminal() {
    let sanitized = render_labels(
        "<span style='font-size:64px;font-family:monospace;font-weight:900;font-style:italic'>MMMM</span>",
        serde_json::json!({"dompurifyConfig": {"FORBID_ATTR": ["style"]}}),
    );
    let reference = render_labels("MMMM", serde_json::json!({}));
    for (actual, expected) in sanitized.iter().zip(&reference) {
        assert_same_size(actual, expected);
    }
}

#[test]
fn block_unwrapped_empty_inline_fonts_do_not_enlarge_text_line_boxes() {
    let reference = render_labels("MMMM", serde_json::json!({}));
    let empty = render_labels(
        "<span style='font-size:64px'></span>MMMM",
        serde_json::json!({}),
    );
    for (actual, expected) in empty.iter().zip(&reference) {
        assert_same_size(actual, expected);
    }
    let small = render_labels(
        "<span style='font-size:8px'>MMMM</span>",
        serde_json::json!({}),
    );
    for (actual, expected) in small.iter().zip(&reference) {
        assert_eq!(
            actual.height, expected.height,
            "inherited 1.5 line-height strut"
        );
    }
}

#[derive(Default)]
struct TerminalFontProbe(Mutex<Vec<(String, TextStyle)>>);

impl TextMeasurer for TerminalFontProbe {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.0
            .lock()
            .unwrap()
            .push((text.to_owned(), style.clone()));
        TextMetrics {
            width: text.chars().count() as f64 * style.font_size,
            height: if text == "Host" && style.font_size == 64.0 {
                137.0
            } else {
                style.font_size * 1.5
            },
            line_count: 1,
        }
    }

    fn measure_html_line_width_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.measure(text, style).width
    }
}

#[test]
fn block_unwrapped_terminal_fonts_reach_the_host_with_element_overrides() {
    let probe = Arc::new(TerminalFontProbe::default());
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("block-unwrapped-terminal-fonts").unwrap(),
        "1",
    )
    .unwrap();
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::uniform(TextMeasurementProfile::new(identity, probe.clone())),
    );
    render_labels_with_environment(
        "<span style='font-size:2em;font-family:serif;font-weight:700;font-style:italic'>A<strong style='font-weight:normal'>B</strong><em style='font-style:normal'>C</em><code style='font-family:inherit'>D</code><span style='font-size:150%;font-weight:lighter;font-style:inherit'>E</span></span>F",
        serde_json::json!({}),
        environment,
    );
    let calls = probe.0.lock().unwrap();
    for (text, size, family, weight, slant) in [
        ("A", 32.0, Some("serif"), Some("700"), Some("italic")),
        ("B", 32.0, Some("serif"), Some("400"), Some("italic")),
        ("C", 32.0, Some("serif"), Some("700"), Some("normal")),
        ("D", 32.0, Some("serif"), Some("700"), Some("italic")),
        ("E", 48.0, Some("serif"), Some("400"), Some("italic")),
    ] {
        assert!(
            calls.iter().any(|(actual, style)| actual == text
                && style.font_size == size
                && style.font_family.as_deref() == family
                && style.font_weight.as_deref() == weight
                && style.font_style.as_deref() == slant),
            "missing terminal font for {text}: {calls:?}"
        );
    }
    assert!(
        calls
            .iter()
            .any(|(text, style)| text == "F" && style.font_size == 16.0)
    );
    let paragraph = calls.iter().position(|(text, _)| text == "ABCDEF").unwrap();
    let styled = calls
        .iter()
        .position(|(text, style)| text == "A" && style.font_size == 32.0)
        .unwrap();
    assert!(
        paragraph < styled,
        "complete host paragraph precedes styled probes"
    );
}

#[test]
fn block_unwrapped_terminal_height_preserves_host_vertical_metrics() {
    let probe = Arc::new(TerminalFontProbe::default());
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("block-unwrapped-terminal-height").unwrap(),
        "1",
    )
    .unwrap();
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::uniform(TextMeasurementProfile::new(identity, probe)),
    );
    for label in render_labels_with_environment(
        "<span style='font-size:64px'>Host</span>",
        serde_json::json!({}),
        environment,
    ) {
        assert_eq!(label.height, 137.0);
    }
}

#[test]
fn block_styled_literal_break_text_stays_on_one_measured_line() {
    for label in render_labels(
        "<span style='font-size:64px'>A&lt;br/&gt;B</span>",
        serde_json::json!({}),
    ) {
        assert_eq!(label.text, "A<br/>B");
        assert_eq!(label.height, 96.0);
        assert!(!label.elements.iter().any(|element| element == "br"));
    }
}

#[test]
fn block_styled_collapsed_line_tails_do_not_add_width() {
    for (actual, expected) in [
        (
            "<span style='font-size:64px'>M </span>",
            "<span style='font-size:64px'>M</span>",
        ),
        (
            "<span style='font-size:64px'>M </span><br/>M",
            "<span style='font-size:64px'>M</span><br/>M",
        ),
    ] {
        let actual = render_labels(actual, serde_json::json!({}));
        let expected = render_labels(expected, serde_json::json!({}));
        for (actual, expected) in actual.iter().zip(&expected) {
            assert_same_size(actual, expected);
        }
    }
    let plain = render_labels(
        "<span style='font-size:64px'>M</span>",
        serde_json::json!({}),
    );
    let nbsp = render_labels(
        "<span style='font-size:64px'>M&nbsp;</span>",
        serde_json::json!({}),
    );
    for (plain, nbsp) in plain.iter().zip(&nbsp) {
        assert!(nbsp.width > plain.width);
    }
}
