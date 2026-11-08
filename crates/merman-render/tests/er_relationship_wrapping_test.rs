#![cfg(feature = "diagram-er")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{DeterministicTextMeasurer, TextMeasurer, TextStyle};
use roxmltree::{Document, Node};
use serde_json::Value;

const LONG_LABEL: &str = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu";
const PROVIDERS: &[&str] = &[
    "dagre",
    #[cfg(feature = "layout-elk")]
    "elk",
];

struct Rendered {
    layout: Value,
    svg: String,
}

impl Rendered {
    fn dimensions(&self) -> (f64, f64) {
        let label = &self.layout["layout"]["ErDiagram"]["edges"][0]["label"];
        (
            label["width"].as_f64().expect("relationship label width"),
            label["height"].as_f64().expect("relationship label height"),
        )
    }
}

fn render(
    label: &str,
    provider: &str,
    html_labels: bool,
    wrapping_width: u32,
    markdown_auto_wrap: bool,
) -> Rendered {
    let source = format!(
        "---\nconfig:\n  layout: {provider}\n  look: classic\n  htmlLabels: {html_labels}\n  markdownAutoWrap: {markdown_auto_wrap}\n  flowchart:\n    wrappingWidth: {wrapping_width}\n  themeVariables:\n    fontSize: 16px\n---\nerDiagram\n  A ||--|| B : \"{label}\"\n"
    );
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .expect("parse relationship wrapping fixture")
        .expect("detect ER diagram")
        .into_presentation();
    // This profile requires no platform fonts and makes the row/metric assertions portable.
    let artifact = family::prepare(
        &parsed,
        &LayoutOptions::headless_svg_defaults(),
        RenderEnvironment::deterministic().begin_session().unwrap(),
    )
    .expect("prepare relationship wrapping fixture");
    let layout = serde_json::json!(artifact.layout_json().expect("serialize ER layout"));
    assert_eq!(layout["semantic"]["relationships"][0]["roleA"], label);
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("paint relationship wrapping fixture")
        .svg()
        .to_owned();
    let document = Document::parse(&svg).expect("valid ER SVG");
    for attribute in document
        .descendants()
        .filter(Node::is_element)
        .flat_map(|node| node.attributes())
        .filter(|attribute| {
            matches!(
                attribute.name(),
                "x" | "y" | "width" | "height" | "transform" | "d" | "viewBox"
            )
        })
    {
        assert!(
            !attribute.value().contains("NaN") && !attribute.value().contains("undefined"),
            "invalid ER geometry: {attribute:?}"
        );
    }
    Rendered { layout, svg }
}

fn has_class(node: Node<'_, '_>, class: &str) -> bool {
    node.attribute("class")
        .is_some_and(|value| value.split_whitespace().any(|value| value == class))
}

fn text(node: Node<'_, '_>) -> String {
    node.descendants()
        .filter(|node| node.is_text())
        .filter_map(|node| node.text())
        .collect()
}

fn number(node: Node<'_, '_>, attribute: &str) -> f64 {
    node.attribute(attribute)
        .unwrap_or_else(|| panic!("missing {attribute}"))
        .parse()
        .unwrap_or_else(|_| panic!("invalid {attribute}"))
}

fn translation(node: Node<'_, '_>) -> (f64, f64) {
    let value = node
        .attribute("transform")
        .and_then(|value| value.strip_prefix("translate("))
        .and_then(|value| value.strip_suffix(')'))
        .expect("label translation");
    let (x, y) = value.split_once(',').expect("translation coordinates");
    (x.trim().parse().unwrap(), y.trim().parse().unwrap())
}

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
}

fn edge_labels<'a, 'input>(document: &'a Document<'input>) -> Node<'a, 'input> {
    document
        .descendants()
        .find(|node| node.has_tag_name("g") && has_class(*node, "edgeLabels"))
        .expect("edgeLabels group")
}

fn relationship_label<'a, 'input>(document: &'a Document<'input>) -> Node<'a, 'input> {
    edge_labels(document)
        .children()
        .find(|node| node.has_tag_name("g") && has_class(*node, "edgeLabel"))
        .expect("relationship label")
}

fn svg_rows<'a, 'input>(document: &'a Document<'input>) -> Vec<Node<'a, 'input>> {
    relationship_label(document)
        .descendants()
        .filter(|node| node.has_tag_name("tspan") && has_class(*node, "text-outer-tspan"))
        .collect()
}

fn style_property<'a>(style: &'a str, property: &str) -> Option<&'a str> {
    style.split(';').find_map(|declaration| {
        let (name, value) = declaration.split_once(':')?;
        (name.trim() == property).then(|| value.trim())
    })
}

fn assert_html_geometry(
    document: &Document<'_>,
    rendered: &Rendered,
    rows: usize,
    constrained: bool,
) {
    let label = relationship_label(document);
    let foreign_object = label
        .descendants()
        .find(|node| node.has_tag_name("foreignObject"))
        .expect("HTML relationship label");
    let div = foreign_object
        .children()
        .find(|node| node.has_tag_name("div"))
        .expect("HTML relationship wrapper");
    let (width, height) = rendered.dimensions();
    close(number(foreign_object, "width"), width);
    close(number(foreign_object, "height"), height);
    close(height, rows as f64 * 14.0 * 1.5);
    let style = div.attribute("style").expect("HTML wrapper style");
    assert_eq!(style_property(style, "max-width"), Some("200px"));
    assert_eq!(
        style_property(style, "white-space"),
        Some(if constrained {
            "break-spaces"
        } else {
            "nowrap"
        })
    );
    assert_eq!(
        style_property(style, "display"),
        Some(if constrained { "table" } else { "table-cell" })
    );
    assert_eq!(
        style_property(style, "width"),
        constrained.then_some("200px")
    );
}

fn assert_svg_geometry(document: &Document<'_>, rendered: &Rendered) {
    let label = relationship_label(document);
    assert!(
        !label
            .descendants()
            .any(|node| node.has_tag_name("foreignObject"))
    );
    let rows = svg_rows(document);
    assert!(!rows.is_empty());
    let text_element = rows[0].parent().expect("SVG text terminal");
    assert!(text_element.has_tag_name("text"));
    assert_eq!(text_element.attribute("text-anchor"), Some("middle"));
    close(number(text_element, "y"), -10.1);
    assert!(
        label
            .descendants()
            .filter(|node| node.has_tag_name("g"))
            .all(|node| node.attribute("text-anchor").is_none()),
        "the text and outer tspans must own centering directly"
    );
    let measurer = DeterministicTextMeasurer::default();
    let mut horizontal_ink: Option<(f64, f64)> = None;
    let mut ink: Option<(f64, f64)> = None;
    // Measure the painted word terminals, including their styles and leading spaces.
    // Row positions are read from the SVG, independently of the prepared label's metrics.
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(row.attribute("x"), Some("0"));
        assert_eq!(row.attribute("dy"), Some("1.1em"));
        assert_eq!(row.attribute("text-anchor"), Some("middle"));
        assert_eq!(row.parent(), Some(text_element));
        let y: f64 = row
            .attribute("y")
            .unwrap()
            .trim_end_matches("em")
            .parse()
            .unwrap();
        close(y, index as f64 * 1.1 - 0.1);
        let mut row_width = 0.0_f64;
        let mut row_ink: Option<(f64, f64)> = None;
        for word in row.children().filter(|node| node.has_tag_name("tspan")) {
            let style = TextStyle {
                font_size: 14.0,
                font_weight: word.attribute("font-weight").map(str::to_owned),
                font_style: word.attribute("font-style").map(str::to_owned),
                ..TextStyle::default()
            };
            let visible = text(word);
            let advance = measurer.measure_svg_text_computed_length_px(&visible, &style);
            let (left, right) = measurer.measure_svg_text_bbox_x(&visible, &style);
            let center = row_width + advance / 2.0;
            let (min, max) = (center - left, center + right);
            row_ink = Some(row_ink.map_or((min, max), |(x0, x1)| (x0.min(min), x1.max(max))));
            row_width += advance;
            let top = measurer.measure_svg_create_text_bbox_y_offset_px(&visible, &style)
                + (y + 0.1) * style.font_size;
            let bottom = top + measurer.measure_svg_tspan_text_bbox_height_px(&visible, &style);
            ink = Some(ink.map_or((top, bottom), |(min, max)| (min.min(top), max.max(bottom))));
        }
        assert!(row_width <= 200.0, "over-wide painted row: {}", text(*row));
        let (min, max) = row_ink.expect("painted row ink");
        let (min, max) = (min - row_width / 2.0, max - row_width / 2.0);
        horizontal_ink =
            Some(horizontal_ink.map_or((min, max), |(x0, x1)| (x0.min(min), x1.max(max))));
    }
    let (left, right) = horizontal_ink.expect("painted SVG ink");
    let (top, bottom) = ink.expect("visible SVG words");
    let (layout_width, layout_height) = rendered.dimensions();
    close(layout_width, right - left + 4.0);
    close(layout_height, bottom - top + 4.0);
    let background = label
        .descendants()
        .find(|node| node.has_tag_name("rect") && has_class(*node, "background"))
        .expect("SVG relationship background");
    close(number(background, "width"), layout_width);
    close(number(background, "height"), layout_height);
    let (background_x, background_y) = (number(background, "x"), number(background, "y"));
    close(left - background_x, 2.0);
    close(background_x + number(background, "width") - right, 2.0);
    close(top - background_y, 2.0);
    close(background_y + number(background, "height") - bottom, 2.0);
    let label_group = label
        .descendants()
        .find(|node| node.has_tag_name("g") && has_class(*node, "label"))
        .expect("label transform group");
    let (tx, ty) = translation(label_group);
    close(tx + (left + right) / 2.0, 0.0);
    close(ty + (top + bottom) / 2.0, 0.0);
}

fn assert_zero_size_label(document: &Document<'_>, rendered: &Rendered, html_labels: bool) {
    let geometry = &rendered.layout["layout"]["ErDiagram"]["edges"][0]["label"];
    assert!(geometry.is_null() || (geometry["width"] == 0.0 && geometry["height"] == 0.0));
    assert_eq!(
        edge_labels(document)
            .children()
            .filter(|node| node.has_tag_name("g") && has_class(*node, "edgeLabel"))
            .count(),
        1
    );
    let label = relationship_label(document);
    assert_eq!(text(label), "");
    let label_group = label
        .children()
        .find(|node| node.has_tag_name("g") && has_class(*node, "label"))
        .expect("zero-size label transform group");
    assert_eq!(translation(label_group), (0.0, 0.0));
    if html_labels {
        let terminal = label
            .descendants()
            .find(|node| node.has_tag_name("foreignObject"))
            .expect("zero-size HTML relationship terminal");
        close(number(terminal, "width"), 0.0);
        close(number(terminal, "height"), 0.0);
    } else {
        let rows = svg_rows(document);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].attribute("text-anchor"), Some("middle"));
        assert_eq!(
            rows[0].children().filter(|node| node.is_element()).count(),
            0
        );
        let text_element = rows[0].parent().expect("empty SVG text terminal");
        assert_eq!(text_element.attribute("text-anchor"), Some("middle"));
        assert_eq!(text_element.parent(), Some(label_group));
        assert!(!label.descendants().any(|node| node.has_tag_name("rect")));
    }
}

#[test]
fn ordinary_relationship_width_is_shared_by_layout_and_paint_for_both_providers() {
    for &provider in PROVIDERS {
        for html_labels in [false, true] {
            let mut baseline = None;
            for wrapping_width in [30, 1000] {
                let rendered = render(LONG_LABEL, provider, html_labels, wrapping_width, true);
                let document = Document::parse(&rendered.svg).expect("XHTML-safe ER SVG");
                let dimensions = rendered.dimensions();
                assert!(dimensions.0 > 0.0);
                if html_labels {
                    close(dimensions.0, 200.0);
                    assert_html_geometry(&document, &rendered, 3, true);
                    assert_eq!(text(relationship_label(&document)), LONG_LABEL);
                    assert!(
                        !relationship_label(&document)
                            .descendants()
                            .any(|node| node.has_tag_name("br"))
                    );
                } else {
                    let rows = svg_rows(&document)
                        .into_iter()
                        .map(text)
                        .collect::<Vec<_>>();
                    assert_eq!(rows.len(), 3);
                    assert_eq!(rows.join(" "), LONG_LABEL);
                    assert_svg_geometry(&document, &rendered);
                }
                if let Some(baseline) = baseline {
                    assert_eq!(
                        dimensions, baseline,
                        "flowchart.wrappingWidth must not set ER edge width"
                    );
                }
                baseline = Some(dimensions);
            }
        }
    }
}

#[test]
fn short_labels_and_authored_breaks_keep_their_natural_width() {
    for &provider in PROVIDERS {
        for html_labels in [false, true] {
            for markdown_auto_wrap in [false, true] {
                for (source, expected_rows) in [
                    ("owns", vec!["owns"]),
                    (
                        "alpha beta<br/>gamma delta",
                        vec!["alpha beta", "gamma delta"],
                    ),
                ] {
                    let rendered = render(source, provider, html_labels, 30, markdown_auto_wrap);
                    let document = Document::parse(&rendered.svg).expect("XHTML-safe ER SVG");
                    assert!(rendered.dimensions().0 < 200.0);
                    if html_labels {
                        assert_html_geometry(&document, &rendered, expected_rows.len(), false);
                        assert_eq!(
                            text(relationship_label(&document)).replace('\u{00a0}', " "),
                            expected_rows.concat()
                        );
                        assert_eq!(
                            relationship_label(&document)
                                .descendants()
                                .filter(|node| node.has_tag_name("br"))
                                .count(),
                            expected_rows.len() - 1
                        );
                    } else {
                        assert_eq!(
                            svg_rows(&document)
                                .into_iter()
                                .map(text)
                                .collect::<Vec<_>>(),
                            expected_rows
                        );
                        assert_svg_geometry(&document, &rendered);
                    }
                }
            }
        }
    }
}

#[test]
fn markdown_auto_wrap_false_preserves_html_nbsp_but_still_wraps_svg() {
    for &provider in PROVIDERS {
        for html_labels in [false, true] {
            let rendered = render(LONG_LABEL, provider, html_labels, 30, false);
            let document = Document::parse(&rendered.svg).expect("XHTML-safe ER SVG");
            if html_labels {
                assert!(
                    rendered.dimensions().0 > 200.0,
                    "nonbreaking text keeps its min-content width"
                );
                assert_html_geometry(&document, &rendered, 1, true);
                let visible = text(relationship_label(&document));
                assert_eq!(visible, LONG_LABEL.replace(' ', "\u{00a0}"));
            } else {
                let rows = svg_rows(&document)
                    .into_iter()
                    .map(text)
                    .collect::<Vec<_>>();
                assert_eq!(
                    rows.len(),
                    3,
                    "pinned createText still wraps SVG when markdownAutoWrap is false"
                );
                assert_eq!(rows.join(" "), LONG_LABEL);
                assert_svg_geometry(&document, &rendered);
            }
        }
    }
}

#[test]
fn html_long_words_overflow_while_svg_splits_without_losing_content() {
    let source = "abcdefghij".repeat(12);
    for &provider in PROVIDERS {
        for html_labels in [false, true] {
            let rendered = render(&source, provider, html_labels, 30, true);
            let document = Document::parse(&rendered.svg).expect("XHTML-safe ER SVG");
            if html_labels {
                assert!(rendered.dimensions().0 > 200.0);
                assert_html_geometry(&document, &rendered, 1, true);
                assert_eq!(text(relationship_label(&document)), source);
            } else {
                let rows = svg_rows(&document)
                    .into_iter()
                    .map(text)
                    .collect::<Vec<_>>();
                assert_eq!(rows.len(), 4);
                assert_eq!(rows.concat(), source);
                assert_svg_geometry(&document, &rendered);
            }
        }
    }
}

#[test]
fn wrapped_markdown_keeps_styled_word_terminals() {
    for &provider in PROVIDERS {
        for html_labels in [false, true] {
            for markdown_auto_wrap in [false, true] {
                for source in [
                    format!("**{LONG_LABEL}**"),
                    "**alpha beta gamma delta** *epsilon zeta eta theta* iota kappa lambda mu"
                        .to_owned(),
                ] {
                    let rendered = render(&source, provider, html_labels, 1000, markdown_auto_wrap);
                    let document = Document::parse(&rendered.svg).expect("XHTML-safe ER SVG");
                    let label = relationship_label(&document);
                    let mixed = source.contains("*epsilon");
                    if html_labels {
                        let rows = if markdown_auto_wrap { 3 } else { 1 };
                        assert_html_geometry(&document, &rendered, rows, true);
                        assert_eq!(text(label).replace('\u{00a0}', " "), LONG_LABEL);
                        assert!(label.descendants().any(|node| node.has_tag_name("strong")));
                        assert_eq!(
                            label.descendants().any(|node| node.has_tag_name("em")),
                            mixed
                        );
                    } else {
                        let rows = svg_rows(&document)
                            .into_iter()
                            .map(text)
                            .collect::<Vec<_>>();
                        assert_eq!(rows.len(), 3);
                        assert_eq!(rows.join(" "), LONG_LABEL);
                        assert!(
                            label
                                .descendants()
                                .any(|node| node.attribute("font-weight") == Some("bold"))
                        );
                        assert_eq!(
                            label
                                .descendants()
                                .any(|node| node.attribute("font-style") == Some("italic")),
                            mixed
                        );
                        if !mixed {
                            assert!(
                                label
                                    .descendants()
                                    .filter(|node| has_class(*node, "text-inner-tspan"))
                                    .all(|node| node.attribute("font-weight") == Some("bold"))
                            );
                        }
                        assert_svg_geometry(&document, &rendered);
                    }
                }
            }
        }
    }
}

#[test]
fn blank_roles_reserve_svg_padding_while_html_stays_zero_size() {
    for &provider in PROVIDERS {
        for html_labels in [false, true] {
            let rendered = render("   ", provider, html_labels, 30, true);
            let document = Document::parse(&rendered.svg).expect("XHTML-safe ER SVG");
            if html_labels {
                assert_zero_size_label(&document, &rendered, true);
            } else {
                assert_eq!(rendered.dimensions(), (4.0, 4.0));
                let label = relationship_label(&document);
                assert_eq!(text(label), "");
                let rows = svg_rows(&document);
                assert_eq!(rows.len(), 1);
                assert_eq!(
                    rows[0].children().filter(|node| node.is_element()).count(),
                    0
                );
                assert_eq!(rows[0].attribute("text-anchor"), Some("middle"));
                let text_element = rows[0].parent().expect("blank SVG text terminal");
                assert_eq!(text_element.attribute("text-anchor"), Some("middle"));
                let background = label
                    .descendants()
                    .find(|node| node.has_tag_name("rect") && has_class(*node, "background"))
                    .expect("padded whitespace background");
                close(number(background, "x"), -2.0);
                close(number(background, "y"), -2.0);
                close(number(background, "width"), 4.0);
                close(number(background, "height"), 4.0);
                assert_eq!(text_element.parent(), background.parent());
                let label_group = label
                    .children()
                    .find(|node| node.has_tag_name("g") && has_class(*node, "label"))
                    .expect("blank label transform group");
                assert_eq!(translation(label_group), (0.0, 0.0));
            }
        }
    }
}

#[test]
fn empty_roles_follow_the_selected_provider_without_visible_text_geometry() {
    for &provider in PROVIDERS {
        for html_labels in [false, true] {
            let rendered = render("", provider, html_labels, 30, true);
            let document = Document::parse(&rendered.svg).expect("XHTML-safe ER SVG");
            if provider == "elk" {
                assert!(
                    !edge_labels(&document)
                        .descendants()
                        .any(|node| node.has_tag_name("g") && has_class(node, "edgeLabel"))
                );
            } else {
                assert_zero_size_label(&document, &rendered, html_labels);
            }
        }
    }
}
