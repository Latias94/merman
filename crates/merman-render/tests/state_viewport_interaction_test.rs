#![cfg(feature = "diagram-state")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use roxmltree::{Document, Node};
use serde_json::{Value, json};

const XLINK: &str = "http://www.w3.org/1999/xlink";

fn providers() -> impl Iterator<Item = &'static str> {
    ["dagre", "elk"]
        .into_iter()
        .filter(|provider| *provider != "elk" || cfg!(feature = "layout-elk"))
}

fn render(source: &str, config: Value) -> (Value, String) {
    let parsed = MermaidEngine::new()
        .with_site_config(merman_core::presentation::SourcePresentationConfig::from_value(config))
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse State interaction fixture")
        .expect("detect State")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("State render session");
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare State interaction fixture");
    let layout = artifact.layout_json().expect("State layout")["layout"]["StateDiagramV2"].clone();
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render State interaction fixture")
        .svg()
        .to_owned();
    (layout, svg)
}

fn state_node<'a, 'input>(document: &'a Document<'input>, state_id: &str) -> Node<'a, 'input> {
    let diagram_id = document.root_element().attribute("id").expect("diagram id");
    let prefix = format!("{diagram_id}-state-{state_id}-");
    document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node
                    .attribute("id")
                    .is_some_and(|id| id.starts_with(&prefix))
        })
        .unwrap_or_else(|| panic!("missing State node {state_id}"))
}

#[test]
fn state_viewport_policy_preserves_graph_for_html_svg_and_both_providers() {
    let body = "stateDiagram-v2\naccTitle: State access\naccDescr: State metadata\nA --> B: next\n";
    for provider in providers() {
        for html_labels in [false, true] {
            let base_config = json!({
                "layout": provider, "theme": "default", "look": "classic",
                "htmlLabels": html_labels,
            });
            let (expected_layout, expected_svg) = render(body, base_config.clone());
            let expected_document = Document::parse(&expected_svg).expect("baseline State SVG");
            let expected_root = expected_document.root_element();
            let expected_body = expected_svg.split_once('>').unwrap().1;
            for source_owned in [false, true] {
                for use_max_width in [None, Some(true), Some(false)] {
                    let mut config = base_config.clone();
                    if let Some(value) = use_max_width {
                        config["state"] = json!({"useMaxWidth": value});
                    }
                    let (layout, svg) = if source_owned {
                        render(&format!("---\nconfig: {config}\n---\n{body}"), json!({}))
                    } else {
                        render(body, config)
                    };
                    assert_eq!(layout, expected_layout);
                    assert_eq!(svg.split_once('>').unwrap().1, expected_body);
                    let document = Document::parse(&svg).expect("valid State SVG");
                    let root = document.root_element();
                    assert_eq!(
                        root.attribute("viewBox"),
                        expected_root.attribute("viewBox")
                    );
                    for (attribute, tag, text) in [
                        ("aria-labelledby", "title", "State access"),
                        ("aria-describedby", "desc", "State metadata"),
                    ] {
                        let id = root.attribute(attribute).expect("State ARIA reference");
                        assert!(root.children().any(|node| {
                            node.has_tag_name(tag)
                                && node.attribute("id") == Some(id)
                                && node.text() == Some(text)
                        }));
                    }
                    assert_eq!(
                        document
                            .descendants()
                            .any(|node| node.has_tag_name("foreignObject")),
                        html_labels
                    );
                    if use_max_width == Some(false) {
                        let view_box: Vec<f64> = root
                            .attribute("viewBox")
                            .unwrap()
                            .split_whitespace()
                            .map(|value| value.parse().unwrap())
                            .collect();
                        for (attribute, expected) in
                            [("width", view_box[2]), ("height", view_box[3])]
                        {
                            let actual = root.attribute(attribute).unwrap().parse::<f64>().unwrap();
                            assert!(actual.is_finite() && actual > 0.0);
                            assert_eq!(actual, expected);
                        }
                        assert!(!root.attribute("style").unwrap_or("").contains("max-width"));
                    } else {
                        assert_eq!(root.attribute("width"), Some("100%"));
                        assert_eq!(root.attribute("height"), None);
                        assert!(root.attribute("style").unwrap().contains("max-width:"));
                    }
                }
            }
        }
    }
}

#[test]
fn state_leaf_clicks_wrap_each_shape_once_without_changing_the_graph() {
    let body = concat!(
        "stateDiagram-v2\nstate Gate <<choice>>\nstate Split <<fork>>\n",
        "state Merge <<join>>\nPlain\nTitled: Header\nTitled: Detail\n",
        "[*] --> Plain\nPlain --> Gate\nGate --> Split\nSplit --> Merge\n",
        "Merge --> Titled\nTitled --> [*]\n",
    );
    let targets = [
        "Gate",
        "Split",
        "Merge",
        "Plain",
        "Titled",
        "root_start",
        "root_end",
    ];
    for provider in providers() {
        for html_labels in [false, true] {
            for look in ["classic", "neo", "handDrawn"] {
                let config = json!({
                    "layout": provider, "htmlLabels": html_labels, "look": look,
                    "theme": "default", "handDrawnSeed": 42,
                });
                let (expected_layout, expected_svg) = render(body, config.clone());
                for with_tooltip in [false, true] {
                    let mut source = body.to_owned();
                    for target in targets {
                        if with_tooltip {
                            source.push_str(&format!(
                                "click {target} \"https://example.test/{target}\" \"State docs\"\n"
                            ));
                        } else {
                            source.push_str(&format!(
                                "click {target} href \"https://example.test/{target}\"\n"
                            ));
                        }
                    }
                    let (layout, svg) = render(&source, config.clone());
                    assert_eq!(layout, expected_layout);
                    let document = Document::parse(&svg).expect("linked State SVG");
                    let baseline = Document::parse(&expected_svg).unwrap();
                    assert_eq!(
                        document.root_element().attribute("viewBox"),
                        baseline.root_element().attribute("viewBox")
                    );
                    assert_eq!(
                        document
                            .descendants()
                            .filter(|node| node.has_tag_name("a"))
                            .count(),
                        targets.len()
                    );
                    for target in targets {
                        let node = state_node(&document, target);
                        let anchor = node.parent().expect("State link wrapper");
                        assert!(anchor.has_tag_name("a"));
                        assert_eq!(
                            node.ancestors()
                                .filter(|node| node.has_tag_name("a"))
                                .count(),
                            1
                        );
                        let href = format!("https://example.test/{target}");
                        assert_eq!(anchor.attribute((XLINK, "href")), Some(href.as_str()));
                        let title = with_tooltip.then_some("State docs");
                        assert_eq!(anchor.attribute("title"), title);
                        assert_eq!(node.attribute("title"), title);
                        assert_eq!(anchor.attribute("target"), None);
                    }
                }
            }
        }
    }
}

#[test]
fn state_special_leaf_links_preserve_security_escaping_and_declaration_order() {
    let targets = ["Gate", "Split", "Merge", "Plain"];
    let tooltip = "  Last & <docs> 'quote'  ";
    for provider in providers() {
        for loose in [false, true] {
            let config = json!({
                "layout": provider, "htmlLabels": false, "look": "classic", "theme": "default",
                "securityLevel": if loose { "loose" } else { "strict" },
            });
            let mut source = concat!(
                "stateDiagram-v2\nstate Gate <<choice>>\nstate Split <<fork>>\n",
                "state Merge <<join>>\nPlain\n",
            )
            .to_owned();
            for target in targets {
                source.push_str(&format!(
                    "click {target} \"javascript:alert(1)\" \"First\"\n\
                     click {target} \"https://example.test/state?a=1&b=2\" \"{tooltip}\"\n\
                     click {target} href \"https://example.test/last\"\n"
                ));
            }
            let (_, svg) = render(&source, config);
            let document = Document::parse(&svg).expect("escaped State SVG");
            assert_eq!(
                document
                    .descendants()
                    .filter(|node| node.has_tag_name("a"))
                    .count(),
                targets.len() * 3
            );
            for target in targets {
                let node = state_node(&document, target);
                let wrappers: Vec<_> = node
                    .ancestors()
                    .filter(|node| node.has_tag_name("a"))
                    .collect();
                assert_eq!(wrappers.len(), 3);
                let [last, middle, first] = wrappers.as_slice() else {
                    panic!("three ordered State links");
                };
                assert_eq!(
                    last.attribute((XLINK, "href")),
                    Some("https://example.test/last")
                );
                assert_eq!(last.attribute("title"), None);
                assert_eq!(
                    middle.attribute((XLINK, "href")),
                    Some("https://example.test/state?a=1&b=2")
                );
                let expected_tooltip = if loose { tooltip } else { tooltip.trim() };
                assert_eq!(middle.attribute("title"), Some(expected_tooltip));
                assert_eq!(node.attribute("title"), Some(expected_tooltip));
                assert_eq!(first.attribute("title"), Some("First"));
                assert_eq!(
                    first.attribute((XLINK, "href")),
                    loose.then_some("javascript:alert(1)")
                );
                for anchor in wrappers {
                    assert_eq!(anchor.attribute("target"), loose.then_some("_blank"));
                    assert_eq!(anchor.attribute("onclick"), None);
                }
            }
            assert!(!document.descendants().any(|node| node.has_tag_name("docs")));
        }
    }
}

#[test]
fn state_href_with_tooltip_remains_an_invalid_probe() {
    for declaration in ["Gate", "state Gate <<choice>>"] {
        let source = format!(
            "stateDiagram-v2\n{declaration}\nclick Gate href \"https://example.test/state\" \"State docs\"\n"
        );
        assert!(
            MermaidEngine::new()
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .is_err()
        );
    }
}
