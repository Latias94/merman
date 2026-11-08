#![cfg(feature = "diagram-er")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::diagram_theme::{
    DiagramThemeCompiler, DiagramThemeSpec, FontStack, ThemeTextStyle, TypographySpec,
};
use merman_render::environment::{
    MeasurementProfileId, RenderEnvironment, TextMeasurementPolicy, TextMeasurementProfile,
    TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{
    DeterministicTextMeasurer, TextMeasurer, TextMetrics, TextStyle, WrapMode,
};
use merman_render::{DiagramFamilyId, LayoutOptions};
use serde_json::json;
use std::sync::{Arc, Mutex};

#[allow(dead_code)]
mod common;

#[derive(Clone, Debug)]
struct FontCall {
    operation: &'static str,
    text: String,
    style: TextStyle,
}

#[derive(Default)]
struct Fonts(Mutex<Vec<FontCall>>);

impl Fonts {
    fn record(&self, operation: &'static str, text: &str, style: &TextStyle) {
        self.0.lock().unwrap().push(FontCall {
            operation,
            text: text.to_owned(),
            style: style.clone(),
        });
    }

    fn width(text: &str, style: &TextStyle) -> f64 {
        text.chars().count() as f64 * style.font_size / 2.0
    }
}

impl TextMeasurer for Fonts {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.measure_wrapped(text, style, None, WrapMode::HtmlLike)
    }

    fn measure_wrapped(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        mode: WrapMode,
    ) -> TextMetrics {
        self.record("html", text, style);
        DeterministicTextMeasurer::default()
            .with_width_callback(Self::width)
            .measure_wrapped(text, style, max_width, mode)
    }

    fn measure_html_line_width_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.record("html-leaf-width", text, style);
        Self::width(text, style)
    }

    fn measure_html_line_height_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.record("html-leaf-height", text, style);
        style.font_size * 1.5
    }

    fn measure_mermaid_calculate_text_dimensions(
        &self,
        text: &str,
        style: &TextStyle,
    ) -> TextMetrics {
        self.record("budget", text, style);
        TextMetrics {
            width: Self::width(text, style),
            height: style.font_size,
            line_count: 1,
        }
    }

    fn measure_svg_text_computed_length_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.record("svg-width", text, style);
        Self::width(text, style)
    }

    fn measure_svg_tspan_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.record("svg-height", text, style);
        style.font_size
    }

    fn measure_svg_tspan_text_bbox_width_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.record("svg-box-width", text, style);
        Self::width(text, style) + 0.75
    }

    fn measure_svg_create_text_bbox_y_offset_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.record("svg-y", text, style);
        -style.font_size * 0.75
    }
}

fn render(source: &str, html: bool, layout: &str) -> (String, Vec<FontCall>) {
    render_with_root_size(source, html, layout, 10)
}

fn render_with_root_size(
    source: &str,
    html: bool,
    layout: &str,
    root_size: i64,
) -> (String, Vec<FontCall>) {
    let fonts = Arc::new(Fonts::default());
    let profile = TextMeasurementProfile::new(
        TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.er-source-typography").unwrap(),
            "test",
        )
        .unwrap(),
        fonts.clone(),
    );
    let environment = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile));
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_family_style(
                    DiagramFamilyId::ER,
                    ThemeTextStyle::default()
                        .with_font_stack(FontStack::single("TypedFace").unwrap())
                        .with_font_size_px(24.0)
                        .unwrap(),
                ),
            ),
        )
        .unwrap();
    let engine = common::install_parse_compatibility(
        &theme,
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(json!({
                "look": "classic", "layout": layout, "htmlLabels": html,
                "fontFamily": "BudgetFace", "fontSize": root_size,
                "flowchart": {"wrappingWidth": 120}
            })),
        ),
    );
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let artifact = family::prepare(
        &parsed,
        &LayoutOptions::default(),
        environment.begin_session_with_theme(&theme).unwrap(),
    )
    .unwrap();
    let prepared_calls = fonts.0.lock().unwrap().len();
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap()
        .svg()
        .to_owned();
    let calls = fonts.0.lock().unwrap().clone();
    assert_eq!(
        calls.len(),
        prepared_calls,
        "ER emission must reuse retained entity metrics and SVG rows"
    );
    (svg, calls)
}

fn layouts() -> impl Iterator<Item = &'static str> {
    ["dagre", "elk"]
        .into_iter()
        .filter(|layout| *layout != "elk" || cfg!(feature = "layout-elk"))
}

fn label<'a, 'input>(
    doc: &'a roxmltree::Document<'input>,
    class: &str,
) -> roxmltree::Node<'a, 'input> {
    doc.descendants()
        .find(|node| node.attribute("class") == Some(class))
        .unwrap()
}

fn row_count(svg: &str, class: &str) -> usize {
    let doc = roxmltree::Document::parse(svg).unwrap();
    label(&doc, class)
        .descendants()
        .filter(|node| {
            node.has_tag_name("tspan")
                && node
                    .parent()
                    .is_some_and(|parent| parent.has_tag_name("text"))
        })
        .count()
}

#[test]
fn er_source_and_classdef_fonts_win_before_entity_and_attribute_measurement() {
    for layout in layouts() {
        for html in [false, true] {
            for source_style in [
                "style E font-family:SourceFace,font-size:32px,font-weight:500,font-style:italic",
                "classDef local font-family:SourceFace,font-size:32px,font-weight:500,font-style:italic\nclass E local",
                "style E font-family:SourceFace,font-size:32px,font-weight:500,font-style:italic\nclassDef later font-family:WrongFace,font-size:48px,font-weight:900,font-style:normal\nclass E later",
                "classDef earlier font-family:WrongFace,font-size:48px,font-weight:900,font-style:normal\nclass E earlier\nstyle E font-family:SourceFace,font-size:32px,font-weight:900!important,font-weight:500,font-style:italic",
            ] {
                let source = format!("erDiagram\nE {{\n string account PK\n}}\n{source_style}\n");
                let (svg, calls) = render(&source, html, layout);
                let terminal_calls: Vec<_> = calls
                    .iter()
                    .filter(|call| call.operation != "budget")
                    .collect();
                assert!(!terminal_calls.is_empty());
                for call in terminal_calls {
                    assert_eq!(
                        call.style.font_family.as_deref(),
                        Some("SourceFace"),
                        "{call:?}"
                    );
                    assert_eq!(call.style.font_size, 32.0, "{call:?}");
                    if html {
                        assert_eq!(call.style.font_weight.as_deref(), Some("500"), "{call:?}");
                        assert_eq!(call.style.font_style.as_deref(), Some("italic"), "{call:?}");
                    } else {
                        assert_eq!(
                            call.style.font_weight.as_deref(),
                            Some("normal"),
                            "{call:?}"
                        );
                        assert_eq!(call.style.font_style.as_deref(), Some("normal"), "{call:?}");
                    }
                }
                assert!(
                    calls
                        .iter()
                        .filter(|call| call.operation == "budget")
                        .all(|call| {
                            call.style.font_size == 10.0
                                && matches!(
                                    call.style.font_family.as_deref(),
                                    Some("BudgetFace" | "sans-serif")
                                )
                        })
                );
                let doc = roxmltree::Document::parse(&svg).unwrap();
                let name = label(&doc, "label name");
                assert!(
                    name.attribute("style")
                        .unwrap()
                        .contains("font-style:italic !important")
                );
                if html {
                    let fo = name
                        .descendants()
                        .find(|node| node.has_tag_name("foreignObject"))
                        .unwrap();
                    assert_eq!(fo.attribute("height"), Some("48"));
                    for node in name
                        .descendants()
                        .filter(|node| node.has_tag_name("div") || node.has_tag_name("span"))
                    {
                        assert!(
                            node.attribute("style")
                                .unwrap()
                                .contains("font-size:32px !important")
                        );
                    }
                } else {
                    let text = name
                        .descendants()
                        .find(|node| node.has_tag_name("text"))
                        .unwrap();
                    assert!(
                        text.attribute("style")
                            .unwrap()
                            .contains("font-size:32px !important")
                    );
                    assert!(
                        text.descendants()
                            .filter(|node| node.attribute("class") == Some("text-inner-tspan"))
                            .all(|node| node.attribute("font-weight") == Some("normal")
                                && node.attribute("font-style") == Some("normal"))
                    );
                }
            }
        }
    }
}

#[test]
fn er_attribute_free_source_fonts_keep_the_draw_rect_minimum_as_a_floor() {
    for html in [false, true] {
        let (svg, calls) = render(
            "erDiagram\nE[\"MMMM\"]\nstyle E font-family:SourceFace,font-size:32px,font-weight:500,font-style:italic\n",
            html,
            "dagre",
        );
        assert!(
            calls
                .iter()
                .filter(|call| call.operation != "budget")
                .all(|call| {
                    call.style.font_family.as_deref() == Some("SourceFace")
                        && call.style.font_size == 32.0
                })
        );
        let doc = roxmltree::Document::parse(&svg).unwrap();
        let rect = doc
            .descendants()
            .find(|node| node.attribute("class") == Some("basic label-container"))
            .unwrap();
        assert_eq!(
            rect.attribute("width"),
            Some("104"),
            "drawRect must fit the actual label plus padding, even when its root probe selects minEntityWidth=100"
        );
        let terminal = label(&doc, "label");
        if html {
            let span = terminal
                .descendants()
                .find(|node| node.has_tag_name("span"))
                .unwrap();
            assert!(
                span.attribute("style")
                    .unwrap()
                    .contains("font-style:italic !important")
            );
        } else {
            assert!(
                terminal
                    .descendants()
                    .filter(|node| node.attribute("class") == Some("text-inner-tspan"))
                    .all(|node| node.attribute("font-style") == Some("normal"))
            );
        }
    }
}

#[test]
fn er_svg_retains_branch_specific_constrained_rows() {
    let long = "M".repeat(40);
    for layout in layouts() {
        let (simple, _) = render(&format!("erDiagram\nE[\"{long}\"]\n"), false, layout);
        let (table, calls) = render(
            &format!("erDiagram\nE[\"{long}\"] {{\n string {long}\n}}\n"),
            false,
            layout,
        );
        assert_eq!(
            row_count(&simple, "label"),
            4,
            "drawRect follows the 120px wrapping width"
        );
        assert_eq!(
            row_count(&table, "label name"),
            2,
            "table labels use the root 200px probe plus 100px"
        );
        assert_eq!(row_count(&table, "label attribute-name"), 2);
        assert!(calls.iter().any(|call| call.operation == "budget"
            && call.text == long
            && call.style.font_size == 10.0));
    }
}

#[test]
fn er_svg_markdown_faces_override_parent_source_faces_in_retained_rows() {
    let (svg, calls) = render(
        "erDiagram\nE[\"**Strong** *Italic* Normal\"] {\n string id\n}\nstyle E font-weight:500,font-style:oblique\n",
        false,
        "dagre",
    );
    assert!(calls.iter().any(|call| call.operation == "svg-height"
        && call.text.trim() == "Strong"
        && call.style.font_weight.as_deref() == Some("700")));
    assert!(calls.iter().any(|call| call.operation == "svg-height"
        && call.text.trim() == "Italic"
        && call.style.font_style.as_deref() == Some("italic")));
    assert!(calls.iter().any(|call| call.operation == "svg-height"
        && call.text.trim() == "Normal"
        && call.style.font_weight.as_deref() == Some("normal")
        && call.style.font_style.as_deref() == Some("normal")));
    let doc = roxmltree::Document::parse(&svg).unwrap();
    let name = label(&doc, "label name");
    assert!(
        name.descendants()
            .any(|node| node.attribute("font-weight") == Some("bold")
                && node.text() == Some("Strong"))
    );
    assert!(
        name.descendants()
            .any(|node| node.attribute("font-style") == Some("italic")
                && node.text().is_some_and(|text| text.trim() == "Italic"))
    );
}

#[test]
fn er_relative_source_sizes_follow_html_layers_and_svg_probe_order() {
    for html in [false, true] {
        let (svg, calls) = render(
            "erDiagram\nE {\n string account\n}\nstyle E font-size:2em\n",
            html,
            "dagre",
        );
        if html {
            assert!(
                calls
                    .iter()
                    .filter(|call| call.operation != "budget")
                    .all(|call| call.style.font_size == 192.0)
            );
            let doc = roxmltree::Document::parse(&svg).unwrap();
            let name = label(&doc, "label name");
            for node in name
                .descendants()
                .filter(|node| node.has_tag_name("div") || node.has_tag_name("span"))
            {
                assert!(
                    node.attribute("style")
                        .unwrap()
                        .contains("font-size:2em !important")
                );
            }
        } else {
            assert!(
                calls
                    .iter()
                    .any(|call| call.operation == "svg-width" && call.style.font_size == 48.0),
                "wrapping precedes createText's text-level style"
            );
            assert!(
                calls
                    .iter()
                    .filter(|call| call.operation == "svg-height")
                    .all(|call| call.style.font_size == 96.0)
            );
        }
    }
}

#[test]
fn er_html_inline_fonts_use_the_shared_terminal_font_consumer() {
    let (svg, calls) = render(
        "erDiagram\nE[\"<big>Account</big>\"] {\n string id \"<span style='font-size:40px'>Wide</span>\"\n}\n",
        true,
        "dagre",
    );
    assert!(calls.iter().any(|call| call.operation != "budget"
        && call.text.contains("Account")
        && call.style.font_size > 24.0));
    assert!(calls.iter().any(|call| call.operation != "budget"
        && call.text.contains("Wide")
        && call.style.font_size == 40.0));
    assert!(svg.contains("<big>Account</big>"));
    assert!(svg.contains("font-size:40px"));
    let doc = roxmltree::Document::parse(&svg).unwrap();
    let comment = label(&doc, "label attribute-comment");
    let fo = comment
        .descendants()
        .find(|node| node.has_tag_name("foreignObject"))
        .unwrap();
    assert_eq!(
        fo.attribute("height"),
        Some("60"),
        "a large comment must retain its own terminal height"
    );
}

#[test]
fn er_generic_html_text_content_removes_the_span_and_its_relative_font_layer() {
    for source_label in ["A~T~B", "A&lt;T&gt;B"] {
        let (svg, calls) = render(
            &format!(
                "erDiagram\nE[\"{source_label}\"] {{\n string id\n}}\nstyle E font-size:2em\n"
            ),
            true,
            "dagre",
        );
        let doc = roxmltree::Document::parse(&svg).unwrap();
        let name = label(&doc, "label name");
        let div = name
            .descendants()
            .find(|node| node.has_tag_name("div"))
            .unwrap();
        assert_eq!(div.text(), Some("A<T>B"));
        assert!(!div.descendants().any(|node| node.has_tag_name("span")));
        assert_eq!(
            name.descendants()
                .find(|node| node.has_tag_name("foreignObject"))
                .unwrap()
                .attribute("height"),
            Some("144")
        );
        assert!(calls.iter().any(|call| call.operation == "html-leaf-height"
            && call.text == "A<T>B"
            && call.style.font_size == 96.0));
        assert!(
            calls
                .iter()
                .filter(|call| call.operation == "html-leaf-height" && call.text == "A<T>B")
                .all(|call| call.style.font_size == 96.0)
        );
    }
}

#[test]
fn er_unbreakable_html_keeps_the_initial_width_cap_after_min_content_expansion() {
    let word = "W".repeat(50);
    for attributes in [false, true] {
        let body = if attributes { " {\n string id\n}" } else { "" };
        let (svg, calls) = render(
            &format!("erDiagram\nE[\"{word}\"]{body}\nstyle E font-size:32px\n"),
            true,
            "dagre",
        );
        let document = roxmltree::Document::parse(&svg).unwrap();
        let terminal = document
            .descendants()
            .find(|node| {
                node.has_tag_name("foreignObject")
                    && node
                        .descendants()
                        .any(|text| text.is_text() && text.text() == Some(word.as_str()))
            })
            .unwrap();
        assert_eq!(terminal.attribute("width"), Some("800"));
        assert_eq!(terminal.attribute("height"), Some("48"));
        let div = terminal
            .children()
            .find(|node| node.has_tag_name("div"))
            .unwrap();
        let style = div.attribute("style").unwrap();
        let budget = if attributes { 350 } else { 120 };
        for (property, value) in [
            ("display", "table".to_string()),
            ("white-space", "break-spaces".to_string()),
            ("max-width", format!("{budget}px")),
            ("width", format!("{budget}px")),
        ] {
            assert!(
                style
                    .split(';')
                    .filter_map(|item| item.split_once(':'))
                    .any(|(key, actual)| { key.trim() == property && actual.trim() == value }),
                "{property} must be {value}: {style}"
            );
        }
        assert!(calls.iter().any(|call| {
            call.operation == "budget" && call.text == word && call.style.font_size == 10.0
        }));
    }
}

#[test]
fn er_generic_svg_flattens_only_the_first_row_and_inherits_its_source_face() {
    for (source_label, root_size, expected_rows) in [
        ("A~T~B", 10, &["A<T>B"][..]),
        ("A&lt;T&gt;B", 10, &["A<T>B"][..]),
        (
            "A&lt;T&gt;B<br/>**Strong** plain",
            10,
            &["A<T>B", "Strong", "plain"][..],
        ),
        (
            "A&lt;T&gt;B<br/>**Strong** plain",
            16,
            &["A<T>B", "Strong plain"][..],
        ),
    ] {
        let (svg, calls) = render_with_root_size(
            &format!(
                "erDiagram\nE[\"{source_label}\"] {{\n string id\n}}\nstyle E font-size:32px,font-weight:500,font-style:italic\n"
            ),
            false,
            "dagre",
            root_size,
        );
        let doc = roxmltree::Document::parse(&svg).unwrap();
        let name = label(&doc, "label name");
        let text = name
            .descendants()
            .find(|node| node.has_tag_name("text"))
            .unwrap();
        let rows: Vec<_> = text
            .children()
            .filter(|node| node.has_tag_name("tspan"))
            .collect();
        let visible_rows: Vec<String> = rows
            .iter()
            .map(|row| {
                row.descendants()
                    .filter(|node| node.is_text())
                    .filter_map(|node| node.text())
                    .collect()
            })
            .collect();
        assert_eq!(visible_rows, expected_rows, "{source_label}/{root_size}");
        assert_eq!(rows[0].text(), Some("A<T>B"));
        assert!(!rows[0].children().any(|node| node.is_element()));
        assert!(calls.iter().any(|call| call.operation == "svg-height"
            && call.text == "A<T>B"
            && call.style.font_weight.as_deref() == Some("500")
            && call.style.font_style.as_deref() == Some("italic")));
        assert!(calls.iter().any(|call| call.operation == "svg-box-width"
            && call.text == "A<T>B"
            && call.style.font_weight.as_deref() == Some("500")
            && call.style.font_style.as_deref() == Some("italic")));
        if !source_label.contains("<br/>") {
            let (x, _) = name
                .attribute("transform")
                .unwrap()
                .strip_prefix("translate(")
                .unwrap()
                .split_once(',')
                .unwrap();
            assert_eq!(
                x.trim().parse::<f64>().unwrap(),
                -40.375,
                "the flattened row centers its 80.75px ink bbox instead of its 80px advance"
            );
        }
        if source_label.contains("<br/>") {
            let budget = Fonts::width(
                "**Strong** plain",
                &TextStyle {
                    font_size: root_size as f64,
                    ..TextStyle::default()
                },
            ) + 100.0;
            let terminal_width = Fonts::width(
                "Strong plain",
                &TextStyle {
                    font_size: 32.0,
                    ..TextStyle::default()
                },
            );
            assert_eq!(budget, if root_size == 10 { 180.0 } else { 228.0 });
            assert_eq!(terminal_width, 192.0);
            assert_eq!(rows.len() == 3, terminal_width > budget);
            assert!(calls.iter().any(|call| {
                call.operation == "budget"
                    && call.text == "**Strong** plain"
                    && call.style.font_size == root_size as f64
            }));
            assert!(
                rows[1]
                    .descendants()
                    .any(|node| node.text() == Some("Strong")
                        && node.attribute("font-weight") == Some("bold"))
            );
            assert!(rows.last().unwrap().descendants().any(|node| {
                node.text().is_some_and(|text| text.trim() == "plain")
                    && node.attribute("font-style") == Some("normal")
                    && node.attribute("font-weight") == Some("normal")
            }));
            assert!(calls.iter().any(|call| call.operation == "svg-height"
                && call.text.trim() == "plain"
                && call.style.font_weight.as_deref() == Some("normal")
                && call.style.font_style.as_deref() == Some("normal")));
        }
    }
}

#[test]
fn er_html_projection_consumes_table_and_draw_rect_width_budgets() {
    let phrase = "Customer Account Identifier";
    for layout in layouts() {
        let (svg, calls) = render_with_root_size(
            &format!(
                "erDiagram\nE[\"{phrase}\"] {{\n string id \"{phrase}\"\n}}\nstyle E font-size:32px\n"
            ),
            true,
            layout,
            16,
        );
        let doc = roxmltree::Document::parse(&svg).unwrap();
        for class in ["label name", "label attribute-comment"] {
            let terminal = label(&doc, class);
            let fo = terminal
                .descendants()
                .find(|node| node.has_tag_name("foreignObject"))
                .unwrap();
            assert_eq!(fo.attribute("width"), Some("316"));
            assert_eq!(fo.attribute("height"), Some("96"));
            let div = terminal
                .descendants()
                .find(|node| node.has_tag_name("div"))
                .unwrap();
            let style = div.attribute("style").unwrap();
            assert!(style.contains("display: table;"));
            assert!(style.contains("white-space: break-spaces;"));
            assert!(style.contains("max-width: 316px;") && style.contains(" width: 316px;"));
        }
        assert!(
            calls
                .iter()
                .filter(|call| call.operation == "budget" && call.text == phrase)
                .all(|call| call.style.font_size == 16.0)
        );
        let keys = label(&doc, "label attribute-keys");
        assert_eq!(
            keys.descendants()
                .find(|node| node.has_tag_name("foreignObject"))
                .unwrap()
                .attribute("height"),
            Some("0")
        );

        let (simple, _) = render_with_root_size(
            "erDiagram\nE[\"One Two Three Four Five\"]\nstyle E font-size:32px\n",
            true,
            layout,
            16,
        );
        let doc = roxmltree::Document::parse(&simple).unwrap();
        let terminal = label(&doc, "label");
        let div = terminal
            .descendants()
            .find(|node| node.has_tag_name("div"))
            .unwrap();
        assert!(
            div.attribute("style")
                .unwrap()
                .contains("max-width: 120px;")
        );
        assert!(
            div.attribute("style")
                .unwrap()
                .contains("white-space: break-spaces;")
        );
    }
}

#[test]
fn er_generic_html_keeps_the_create_text_width_decision_after_span_removal() {
    let (svg, calls) = render_with_root_size(
        "erDiagram\nE[\"&lt; &gt; &lt; &gt;\"] {\n string id\n}\nstyle E font-size:1.5em\n",
        true,
        "dagre",
        16,
    );
    let doc = roxmltree::Document::parse(&svg).unwrap();
    let name = label(&doc, "label name");
    let div = name
        .descendants()
        .find(|node| node.has_tag_name("div"))
        .unwrap();
    assert_eq!(div.text(), Some("< > < >"));
    assert!(
        div.attribute("style")
            .unwrap()
            .contains("white-space: break-spaces;")
    );
    let fo = name
        .descendants()
        .find(|node| node.has_tag_name("foreignObject"))
        .unwrap();
    assert_eq!(fo.attribute("width"), Some("252"));
    assert_eq!(fo.attribute("height"), Some("81"));
    assert!(
        calls
            .iter()
            .any(|call| call.operation == "html-leaf-height" && call.style.font_size == 54.0)
    );
}
