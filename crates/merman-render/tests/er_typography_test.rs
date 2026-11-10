#![cfg(feature = "diagram-er")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_model::OperationControl;
use merman_model::families::er::ErDiagramModel;
use merman_model::presentation::{PresentationDiagram, RenderSemanticModel};
use merman_model::resources::InputResourcePolicy;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, FontStack, OrdinalSelector, ThemeRule,
    ThemeRuleSet, ThemeStylePatch, ThemeTarget, ThemeTextStyle, TypographySpec,
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
use serde_json::{Value, json};
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
    render_with_root_size_and_model(source, html, layout, root_size, |_| {})
}

fn amend_er_model(
    parsed: PresentationDiagram,
    amend: impl FnOnce(&mut ErDiagramModel),
) -> PresentationDiagram {
    let (metadata, mut model, context) = parsed.into_parts();
    let RenderSemanticModel::Er(er) = &mut model else {
        panic!("expected ER model");
    };
    amend(er);
    PresentationDiagram::try_from_frozen(
        metadata,
        model,
        context,
        &InputResourcePolicy::default(),
        &OperationControl::new(),
    )
    .expect("validate caller-authored ER presentation")
    .expect("admit caller-authored ER presentation")
}

fn render_with_root_size_and_model(
    source: &str,
    html: bool,
    layout: &str,
    root_size: i64,
    amend: impl FnOnce(&mut ErDiagramModel),
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
    let parsed = amend_er_model(parsed, amend);
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
                "classDef earlier font-family:WrongFace,font-size:48px,font-weight:900,font-style:normal\nclass E earlier\nstyle E font-family:SourceFace,font-size:32px,font-weight:900,font-weight:500,font-style:italic",
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
fn er_authored_duplicate_font_weights_keep_the_last_declaration() {
    for layout in layouts() {
        for html in [false, true] {
            let (svg, calls) = render_with_root_size_and_model(
                "erDiagram\nE {\n string account PK\n}\nclassDef earlier font-family:WrongFace,font-size:48px,font-weight:900,font-style:normal\nclass E earlier\n",
                html,
                layout,
                10,
                |model| {
                    // `!important` is valid caller-authored CSS, outside ER's source grammar.
                    model.entities.get_mut("E").unwrap().css_styles = [
                        "font-family:SourceFace",
                        "font-size:32px",
                        "font-weight:900!important",
                        "font-weight:500",
                        "font-style:italic",
                    ]
                    .map(str::to_owned)
                    .into();
                },
            );
            let terminal_calls: Vec<_> = calls
                .iter()
                .filter(|call| call.operation != "budget")
                .collect();
            assert!(!terminal_calls.is_empty());
            for call in terminal_calls {
                assert_eq!(call.style.font_family.as_deref(), Some("SourceFace"));
                assert_eq!(call.style.font_size, 32.0);
                assert_eq!(
                    call.style.font_weight.as_deref(),
                    Some(if html { "500" } else { "normal" })
                );
                assert_eq!(
                    call.style.font_style.as_deref(),
                    Some(if html { "italic" } else { "normal" })
                );
            }
            let document = roxmltree::Document::parse(&svg).unwrap();
            assert!(
                label(&document, "label name")
                    .attribute("style")
                    .unwrap()
                    .contains("font-weight:500 !important")
            );
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
fn er_authored_generic_html_keeps_the_create_text_width_decision_after_span_removal() {
    let (svg, calls) = render_with_root_size_and_model(
        "erDiagram\nE[\"&lt; &gt; &lt; &gt;\"] {\n string id\n}\n",
        true,
        "dagre",
        16,
        |model| {
            // Decimal relative sizes belong to the typed CSS path, not ER source syntax.
            model.entities.get_mut("E").unwrap().css_styles = vec!["font-size:1.5em".into()];
        },
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

struct TerminalRender {
    svg: String,
    calls: Vec<FontCall>,
    // Applied, not applicable, and residual family mechanisms, independently of SVG portability.
    evidence: [usize; 3],
}

fn font_spec(style: ThemeTextStyle) -> DiagramThemeSpec {
    DiagramThemeSpec::new()
        .with_typography(TypographySpec::default().with_family_style(DiagramFamilyId::ER, style))
}

fn typed_stack() -> ThemeTextStyle {
    ThemeTextStyle::default().with_font_stack(FontStack::single("TypedFace").unwrap())
}

fn render_terminal(source: &str, site: Value, spec: DiagramThemeSpec) -> TerminalRender {
    render_terminal_with_model(source, site, spec, |_| {})
}

fn render_terminal_with_model(
    source: &str,
    site: Value,
    spec: DiagramThemeSpec,
    amend: impl FnOnce(&mut ErDiagramModel),
) -> TerminalRender {
    let fonts = Arc::new(Fonts::default());
    let profile = TextMeasurementProfile::new(
        TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.er-terminal-typography").unwrap(),
            "test",
        )
        .unwrap(),
        fonts.clone(),
    );
    let theme = DiagramThemeCompiler::new().compile(spec).unwrap();
    let engine = common::install_parse_compatibility(
        &theme,
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(site),
        ),
    );
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let parsed = amend_er_model(parsed, amend);
    let environment = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile));
    let artifact = family::prepare(
        &parsed,
        &LayoutOptions::default(),
        environment.begin_session_with_theme(&theme).unwrap(),
    )
    .unwrap();
    let prepared_calls = fonts.0.lock().unwrap().len();
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let svg = rendered.svg().to_owned();
    let calls = fonts.0.lock().unwrap().clone();
    if parsed.metadata().title.is_none() {
        assert_eq!(
            calls.len(),
            prepared_calls,
            "prepared entity/relationship text is reused"
        );
    }
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    TerminalRender {
        svg,
        calls,
        evidence: [
            evidence.applied_count(),
            evidence.not_applicable_count(),
            evidence.theme_residual_count(),
        ],
    }
}

fn terminal_providers() -> impl Iterator<Item = &'static str> {
    layouts().chain(std::iter::once("swimlane"))
}

fn assert_measured_size(rendered: &TerminalRender, text: &str, size: f64) {
    let calls: Vec<_> = rendered
        .calls
        .iter()
        .filter(|call| call.operation != "budget" && call.text.trim() == text)
        .collect();
    assert!(!calls.is_empty(), "missing terminal measurement for {text}");
    assert!(
        calls.iter().all(|call| call.style.font_size == size),
        "{text}: {calls:?}"
    );
}

#[test]
fn er_configured_zero_and_subpixel_sizes_reach_measurement_css_and_title() {
    for provider in terminal_providers() {
        for html in [false, true] {
            for size in [0.0, 0.5] {
                for source_owned in [false, true] {
                    let config = json!({"themeVariables": {"fontSize": format!("{size}px")}});
                    let mut site = if source_owned {
                        json!({"themeVariables": {"fontSize": "48px"}})
                    } else {
                        config.clone()
                    };
                    site.as_object_mut().unwrap().extend(
                        json!({"secure": [], "look": "classic", "layout": provider,
                            "htmlLabels": html})
                        .as_object()
                        .unwrap()
                        .clone(),
                    );
                    let source_config = if source_owned { config } else { json!({}) };
                    let source = format!(
                        "---\ntitle: AuditTitle\nconfig: {source_config}\n---\nerDiagram\nE[EntityTitle] {{\n string FieldName PK \"CommentLabel\"\n}}\nF\nE ||--|| F : RelationLabel\n"
                    );
                    let rendered = render_terminal(
                        &source,
                        site,
                        font_spec(typed_stack().with_font_size_px(23.0).unwrap()),
                    );
                    for text in [
                        "EntityTitle",
                        "string",
                        "FieldName",
                        "PK",
                        "CommentLabel",
                        "AuditTitle",
                    ] {
                        assert_measured_size(&rendered, text, size);
                    }
                    assert_measured_size(&rendered, "RelationLabel", 14.0);
                    assert!(rendered.svg.contains(&format!("font-size:{size}px;")));
                    assert!(rendered.svg.contains("font-size:14px;"));
                    assert_eq!(
                        rendered.evidence,
                        [1, 1, 0],
                        "{provider}, {html}, {source_owned}, {size}"
                    );
                }
            }
        }
    }
}

#[test]
fn er_typed_subpixel_size_is_not_rounded_up_before_its_receipt() {
    for html in [false, true] {
        let rendered = render_terminal(
            "erDiagram\nE {\n string id\n}\n",
            json!({"look": "classic", "htmlLabels": html}),
            font_spec(typed_stack().with_font_size_px(0.5).unwrap()),
        );
        assert_measured_size(&rendered, "string", 0.5);
        assert!(rendered.svg.contains("font-size:0.5px;"));
        assert_eq!(rendered.evidence, [2, 0, 0]);
    }
}

#[test]
fn er_entity_and_cell_receipts_use_the_emitted_html_or_svg_projection() {
    for provider in terminal_providers() {
        for html in [false, true] {
            for declaration in ["font-family:Other", "font-size:40px"] {
                for attributes in [false, true] {
                    let literal = format!("<span style='{declaration}'>MMMM</span>");
                    let source = if attributes {
                        format!("erDiagram\nE {{\n string id \"{literal}\"\n}}\n")
                    } else {
                        format!("erDiagram\nE[\"{literal}\"]\n")
                    };
                    let rendered = render_terminal(
                        &source,
                        json!({"layout": provider, "look": "classic", "htmlLabels": html}),
                        font_spec(typed_stack().with_font_size_px(24.0).unwrap()),
                    );
                    assert_eq!(rendered.evidence, if html { [1, 0, 1] } else { [2, 0, 0] });
                    let document = roxmltree::Document::parse(&rendered.svg).unwrap();
                    if html {
                        assert!(document.descendants().any(|node| node.has_tag_name("span")
                            && node.attribute("style") == Some(declaration)));
                    } else {
                        let text = document
                            .descendants()
                            .filter(|node| node.is_text())
                            .filter(|node| {
                                node.ancestors().any(|parent| parent.has_tag_name("text"))
                            })
                            .filter_map(|node| node.text())
                            .collect::<String>();
                        assert!(text.contains("<span style=") && text.contains("MMMM</span>"));
                        assert!(
                            rendered
                                .calls
                                .iter()
                                .filter(|call| call.operation != "budget")
                                .all(
                                    |call| call.style.font_family.as_deref() == Some("TypedFace")
                                        && call.style.font_size == 24.0
                                )
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn er_zero_source_runs_are_not_font_consumers_but_positive_children_and_relations_are() {
    for html in [false, true] {
        let rendered = render_terminal(
            "erDiagram\nE {\n string id\n}\nstyle E font-size:0px\n",
            json!({"look": "classic", "htmlLabels": html}),
            font_spec(typed_stack().with_font_size_px(24.0).unwrap()),
        );
        assert_eq!(
            rendered.evidence,
            [0, 2, 0],
            "hidden source sizes do not own a visible request"
        );
    }
    for provider in terminal_providers() {
        for html in [false, true] {
            for (source, root_size, visible, entity_count) in [
                (
                    "erDiagram\nE {\n string id\n}\nstyle E font-size:0px\n",
                    16,
                    false,
                    1,
                ),
                (
                    "---\ntitle: HiddenTitle\n---\nerDiagram\nE {\n string id\n}\n",
                    0,
                    false,
                    1,
                ),
                (
                    "erDiagram\nE {\n string id\n}\nstyle E font-size:20px\n",
                    0,
                    true,
                    1,
                ),
                ("erDiagram\nE ||--|| F : VisibleRelation\n", 0, true, 2),
            ] {
                let rendered = render_terminal(
                    source,
                    json!({"layout": provider, "look": "classic", "htmlLabels": html,
                        "themeVariables": {"fontSize": format!("{root_size}px")}}),
                    font_spec(typed_stack()),
                );
                assert_eq!(
                    rendered.evidence,
                    if visible { [1, 0, 0] } else { [0, 1, 0] }
                );
                let document = roxmltree::Document::parse(&rendered.svg).unwrap();
                let nodes: Vec<_> = document
                    .descendants()
                    .filter(|node| {
                        node.has_tag_name("g")
                            && node.attribute("class").is_some_and(|class| {
                                class.split_whitespace().any(|class| class == "node")
                            })
                    })
                    .collect();
                assert_eq!(nodes.len(), entity_count, "{provider}, html={html}");
                for node in nodes {
                    // Tables paint an outer-path group; attribute-free entities use a rect.
                    let has_body = node.descendants().any(|shape| {
                        (shape.has_tag_name("rect")
                            && shape.attribute("class") == Some("basic label-container")
                            && ["width", "height"].into_iter().all(|dimension| {
                                shape
                                    .attribute(dimension)
                                    .and_then(|value| value.parse::<f64>().ok())
                                    .is_some_and(|value| value.is_finite() && value > 0.0)
                            }))
                            || (shape.has_tag_name("g")
                                && shape.attribute("class") == Some("outer-path")
                                && shape.children().any(|path| {
                                    path.has_tag_name("path")
                                        && path.attribute("d").is_some_and(|d| !d.is_empty())
                                }))
                    });
                    assert!(
                        has_body,
                        "missing entity body: {provider}, html={html}, {:?}",
                        node.attribute("id")
                    );
                }
            }
        }
    }
    for (root_size, local_style) in [(0, ""), (16, "style E font-size:0px")] {
        for (comment, visible, measured_size) in [
            ("<span style='font-size:0px'>Hidden</span>", false, 0.0),
            (
                "<span style='font-size:0px'>Hidden<span style='font-size:40px'>Restored</span></span>",
                true,
                40.0,
            ),
            (
                "<span style='font-size:40px'><span style='font-size:0px'>Hidden</span></span>",
                false,
                0.0,
            ),
            (
                "<span style='font-size:0cap'>Hidden<span style='font-size:24pt'>Restored</span></span>",
                true,
                32.0,
            ),
        ] {
            let source = format!("erDiagram\nE {{\n string id \"{comment}\"\n}}\n{local_style}\n");
            let rendered = render_terminal(
                &source,
                json!({"look": "classic", "htmlLabels": true,
                    "themeVariables": {"fontSize": format!("{root_size}px")}}),
                font_spec(typed_stack()),
            );
            assert_eq!(
                rendered.evidence,
                if visible { [1, 0, 0] } else { [0, 1, 0] }
            );
            assert_measured_size(
                &rendered,
                if visible { "Restored" } else { "Hidden" },
                measured_size,
            );
            assert!(
                rendered.svg.contains("Hidden"),
                "evidence filtering must not edit emitted text"
            );
            if visible {
                assert!(rendered.svg.contains("Restored"));
            }
        }
    }
}

#[test]
fn er_unknown_html_sizes_remain_possible_consumers_above_a_zero_root() {
    for html in [false, true] {
        for markup in [
            "<span style='font-size:var(--host-size)'>Possible</span>",
            "<span class='host'>Possible</span>",
            "<span style='font-size:2rem'>Possible</span>",
        ] {
            let source = format!("erDiagram\nE[\"{markup}\"]\n");
            let rendered = render_terminal(
                &source,
                json!({"look": "classic", "htmlLabels": html,
                    "themeVariables": {"fontSize": if html { "0px" } else { "16px" }}}),
                font_spec(typed_stack().with_font_weight(600).unwrap()),
            );
            assert_eq!(rendered.evidence[1], 0, "unknown CSS cannot prove absence");
            assert_eq!(
                rendered.evidence[0],
                if html { 0 } else { 1 },
                "unknown visibility cannot certify typography; literal SVG remains verified"
            );
            assert!(
                rendered.evidence[2] > 0,
                "possible text retains unsupported weight"
            );
        }
    }
}

#[test]
fn er_authored_unknown_sizes_remain_possible_consumers_above_a_zero_root() {
    for html in [false, true] {
        let rendered = render_terminal_with_model(
            "erDiagram\nE {\n string id\n}\n",
            json!({"look": "classic", "htmlLabels": html,
                "themeVariables": {"fontSize": "0px"}}),
            font_spec(typed_stack()),
            |model| {
                model.entities.get_mut("E").unwrap().css_styles =
                    vec!["font-size:var(--host-size)".into()];
            },
        );
        assert_eq!(
            rendered.evidence,
            [0, 0, 1],
            "an unresolved authored size must not certify fallback-zero visibility"
        );
    }
}

fn assert_font_stack_inheritance(rendered: &TerminalRender, value: &str, inherits: bool) {
    assert_eq!(
        rendered.evidence,
        if inherits { [1, 0, 0] } else { [0, 0, 1] }
    );
    let document = roxmltree::Document::parse(&rendered.svg).unwrap();
    assert!(document.descendants().any(|node| {
        node.attribute("style")
            .is_some_and(|style| style.contains(&format!("font-family:{value} !important")))
    }));
    if inherits {
        assert!(
            rendered
                .calls
                .iter()
                .filter(|call| call.operation != "budget")
                .all(|call| call.style.font_family.as_deref() == Some("TypedFace"))
        );
    }
}

#[test]
fn er_source_and_class_font_inheritance_preserves_the_typed_stack_receipt() {
    for html in [false, true] {
        for (value, inherits) in [("inherit", true), ("unset", true), ("Other", false)] {
            for class_def in [false, true] {
                let style = if class_def {
                    format!("classDef local font-family:{value}\nclass E local")
                } else {
                    format!("style E font-family:{value}")
                };
                let source = format!("erDiagram\nE {{\n string id\n}}\n{style}\n");
                let rendered = render_terminal(
                    &source,
                    json!({"look": "classic", "htmlLabels": html}),
                    font_spec(typed_stack()),
                );
                assert_font_stack_inheritance(&rendered, value, inherits);
            }
        }
    }
}

#[test]
fn er_authored_escaped_and_quoted_fonts_preserve_the_typed_stack_receipt() {
    for html in [false, true] {
        for (value, inherits) in [(r"i\6eherit", true), ("'inherit'", false)] {
            for class_def in [false, true] {
                let source = if class_def {
                    "erDiagram\nE {\n string id\n}\nclassDef local font-family:inherit\nclass E local\n"
                } else {
                    "erDiagram\nE {\n string id\n}\n"
                };
                let rendered = render_terminal_with_model(
                    source,
                    json!({"look": "classic", "htmlLabels": html}),
                    font_spec(typed_stack()),
                    |model| {
                        let styles = if class_def {
                            &mut model.classes.get_mut("local").unwrap().styles
                        } else {
                            &mut model.entities.get_mut("E").unwrap().css_styles
                        };
                        *styles = vec![format!("font-family:{value}")];
                    },
                );
                assert_font_stack_inheritance(&rendered, value, inherits);
            }
        }
    }
}

#[test]
fn er_local_font_inheritance_keeps_an_explicit_site_stack_as_the_owner() {
    for html in [false, true] {
        let rendered = render_terminal(
            "erDiagram\nE {\n string id\n}\nstyle E font-family:inherit\n",
            json!({"look": "classic", "htmlLabels": html,
                "themeVariables": {"fontFamily": "SiteFace"}}),
            font_spec(typed_stack()),
        );
        assert_eq!(rendered.evidence, [0, 1, 0]);
        assert!(rendered.svg.contains("font-family:SiteFace;"));
        assert!(
            rendered
                .calls
                .iter()
                .filter(|call| call.operation != "budget")
                .all(|call| call.style.font_family.as_deref() == Some("SiteFace"))
        );
    }
}

#[test]
fn er_zero_text_does_not_renumber_later_text_rules() {
    for html in [false, true] {
        for (ordinal, expected) in [(1, [2, 1, 0]), (2, [1, 1, 1])] {
            let spec = font_spec(typed_stack()).with_styles(
                ThemeRuleSet::default()
                    .with_rule(ThemeRule::new(
                        ThemeTarget::Text,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#00aa00").unwrap()),
                    ))
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::Text,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#aa0000").unwrap()),
                        )
                        .with_ordinal(OrdinalSelector::exact(ordinal).unwrap()),
                    ),
            );
            let rendered = render_terminal(
                "erDiagram\nA[Hidden]\nB[Visible]\nstyle A font-size:0px\n",
                json!({"look": "classic", "htmlLabels": html}),
                spec,
            );
            // An unsupported rule on hidden ordinal 1 is not applicable. On visible ordinal 2
            // it must stay residual and shadow the supported static fill without being emitted.
            assert_eq!(
                rendered.evidence, expected,
                "html={html}, ordinal={ordinal}"
            );
            let document = roxmltree::Document::parse(&rendered.svg).unwrap();
            let visible_label = document
                .descendants()
                .find(|node| {
                    node.attribute("class") == Some("label")
                        && node
                            .descendants()
                            .any(|text| text.is_text() && text.text() == Some("Visible"))
                })
                .expect("the second entity retains its text label");
            assert_eq!(
                visible_label
                    .attribute("style")
                    .unwrap_or_default()
                    .contains("fill:#00aa00"),
                ordinal == 1
            );
            assert!(
                !document.descendants().any(|node| node
                    .attribute("style")
                    .is_some_and(|style| style.contains("#aa0000"))),
                "unsupported ordinal fill must not be emitted"
            );
        }
    }
}
