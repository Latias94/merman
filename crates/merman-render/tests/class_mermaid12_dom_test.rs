use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn render_class(text: &str) -> String {
    render_class_with_id(text, "merman")
}

fn render_class_with_id(text: &str, diagram_id: &str) -> String {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse Class")
        .expect("Class diagram")
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("layout Class")
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some(diagram_id.to_owned()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render Class")
        .svg()
        .to_owned()
}

#[test]
fn class_namespace_html_breaks_and_literal_markdown_share_the_safe_label_path() {
    let layouts = if cfg!(feature = "layout-elk") {
        &["dagre", "elk"][..]
    } else {
        &["dagre"][..]
    };
    for layout in layouts {
        for (title, expected_text) in [
            ("A<br/>B", "AB"),
            ("**A**<br>_B_", "**A**_B_"),
            ("A &amp; B<br />C", "A & BC"),
        ] {
            for nested in [false, true] {
                let namespace = format!("namespace N[\"{title}\"] {{\nclass C\n}}\n");
                let (body, cluster_id) = if nested {
                    (
                        format!(
                            "namespace Outer {{\n{namespace}}}\nclass Outside\nC --> Outside\n"
                        ),
                        "merman-Outer.N",
                    )
                } else {
                    (namespace, "merman-N")
                };
                let svg = render_class(&format!(
                    "---\nconfig:\n  layout: {layout}\n  htmlLabels: true\n---\nclassDiagram\n{body}"
                ));
                let document = roxmltree::Document::parse(&svg).expect("valid Class SVG");
                let cluster = document
                    .descendants()
                    .find(|node| node.attribute("id") == Some(cluster_id))
                    .expect("named namespace cluster");
                let label = cluster
                    .descendants()
                    .find(|node| node.attribute("class") == Some("cluster-label"))
                    .unwrap();
                let paragraph = label
                    .descendants()
                    .find(|node| node.has_tag_name("p"))
                    .unwrap();
                assert_eq!(
                    paragraph
                        .descendants()
                        .filter(|node| node.has_tag_name("br"))
                        .count(),
                    1
                );
                let text = paragraph
                    .descendants()
                    .filter(|node| node.is_text())
                    .filter_map(|node| node.text())
                    .collect::<String>();
                assert_eq!(text, expected_text, "{layout}: {title}");
                assert!(
                    !paragraph
                        .descendants()
                        .any(|node| { node.has_tag_name("strong") || node.has_tag_name("em") })
                );
            }
        }
    }
}

#[test]
fn class_namespace_html_measures_the_emitted_inline_font_size() {
    let layouts = if cfg!(feature = "layout-elk") {
        &["dagre", "elk"][..]
    } else {
        &["dagre"][..]
    };
    for layout in layouts {
        let render = |label: &str| {
            render_class(&format!(
                "---\nconfig:\n  layout: {layout}\n  htmlLabels: true\n---\nclassDiagram\nnamespace N[\"{label}\"] {{\nclass C\n}}\n"
            ))
        };
        let title_size = |svg: &str| {
            let document = roxmltree::Document::parse(svg).unwrap();
            let label = document
                .descendants()
                .find(|node| node.attribute("class") == Some("cluster-label"))
                .unwrap();
            let foreign_object = label
                .descendants()
                .find(|node| node.has_tag_name("foreignObject"))
                .unwrap();
            (
                foreign_object
                    .attribute("width")
                    .unwrap()
                    .parse::<f64>()
                    .unwrap(),
                foreign_object
                    .attribute("height")
                    .unwrap()
                    .parse::<f64>()
                    .unwrap(),
            )
        };
        let plain = render("A");
        let enlarged = render("<span style='font-size:64px'>A</span>");
        let (plain_width, plain_height) = title_size(&plain);
        let (width, height) = title_size(&enlarged);
        assert!(
            width > plain_width,
            "{layout}: inline size must affect title width"
        );
        assert!(
            height > plain_height && height >= 64.0,
            "{layout}: inline size must affect title height"
        );
        let document = roxmltree::Document::parse(&enlarged).unwrap();
        assert!(document.descendants().any(|node| {
            node.attribute("style")
                .is_some_and(|style| style.contains("64px"))
        }));
    }
}

#[test]
fn class_namespace_html_keeps_safe_markup_without_executable_attributes() {
    let svg = render_class(
        "---\nconfig:\n  layout: dagre\n  htmlLabels: true\n---\nclassDiagram\nnamespace N[\"<b onclick='alert(1)'>A</b><br/><script>alert(2)</script>B\"] {\nclass C\n}\n",
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Class SVG");
    let label = document
        .descendants()
        .find(|node| node.attribute("class") == Some("cluster-label"))
        .unwrap();
    assert!(
        label
            .descendants()
            .any(|node| node.has_tag_name("b") && node.text() == Some("A"))
    );
    assert!(label.descendants().any(|node| node.has_tag_name("br")));
    assert!(!label.descendants().any(|node| {
        node.has_tag_name("script")
            || node
                .attributes()
                .any(|attribute| attribute.name().starts_with("on"))
    }));
}

#[cfg(feature = "layout-shoal")]
#[test]
fn class_html_label_overflow_is_visible_only_in_shoal() {
    for layout in [
        Some("shoal"),
        cfg!(feature = "layout-dagre").then_some("dagre"),
        cfg!(feature = "layout-elk").then_some("elk"),
    ]
    .into_iter()
    .flatten()
    {
        let svg = render_class(&format!(
            r#"---
config:
  layout: {layout}
  htmlLabels: true
---
classDiagram
namespace Domain["<b>Domain</b><br/>Core"] {{
  class OrderAggregate~OrderItem~ {{
    <<service>>
    +UUID customer_account_id
    +applyDiscountCode(String promo) void
  }}
}}
class Audit["<i>Audit</i>"]
OrderAggregate "1" --> "1..*" Audit : settles via
OrderAggregate --() CheckoutPort
note for OrderAggregate "Invariant<br/>second line"
"#,
        ));
        let document = roxmltree::Document::parse(&svg).expect("valid Class SVG");
        let mut labels = Vec::new();
        let mut empty_labels = 0;
        for foreign_object in document
            .descendants()
            .filter(|node| node.has_tag_name("foreignObject"))
        {
            let width = foreign_object.attribute("width").unwrap();
            let height = foreign_object.attribute("height").unwrap();
            let has_area =
                width.parse::<f64>().unwrap() > 0.0 && height.parse::<f64>().unwrap() > 0.0;
            let style = foreign_object.attribute("style").unwrap_or_default();
            let text = foreign_object
                .descendants()
                .filter(|node| node.is_text())
                .filter_map(|node| node.text())
                .collect::<String>();
            assert_eq!(
                style.contains("overflow: visible;"),
                layout == "shoal" && has_area,
                "{layout}: {text:?} ({width} × {height}): {style:?}"
            );
            if !has_area {
                empty_labels += 1;
                continue;
            }
            if foreign_object
                .ancestors()
                .any(|node| node.attribute("class") == Some("edgeTerminals"))
            {
                assert!(style.starts_with("width: "), "{layout}: {style}");
                assert!(style.contains(&format!("height: {height}px;")));
            }
            labels.push(text);
        }
        if layout == "shoal" {
            assert!(empty_labels > 0);
        }
        for expected in [
            "DomainCore",
            "«service»",
            "OrderAggregate<OrderItem>",
            "+UUID customer_account_id",
            "+applyDiscountCode(String promo) : void",
            "Audit",
            "settles via",
            "1",
            "1..*",
            "CheckoutPort",
            "Invariantsecond line",
        ] {
            assert!(
                labels.iter().any(|text| text == expected),
                "{layout}: missing positive-size label {expected:?} in {labels:?}"
            );
        }
        for (tag, text) in [("b", "Domain"), ("i", "Audit")] {
            assert!(
                document
                    .descendants()
                    .any(|node| node.has_tag_name(tag) && node.text() == Some(text)),
                "{layout}: missing {tag} markup for {text:?}"
            );
        }
    }
}

#[test]
fn class_spaced_generics_and_bare_names_preserve_the_rendered_graph() {
    let layouts = if cfg!(feature = "layout-elk") {
        &["dagre", "elk"][..]
    } else {
        &["dagre"][..]
    };
    for layout in layouts {
        let config = format!("---\nconfig:\n  layout: {layout}\n---\nclassDiagram\n");
        let compact = format!("{config}Box~T~ --> Item~U~ : uses\n");
        let spaced =
            format!("{config}Box \t~T~ --> Item ~U~ : uses\nBox\nItem ~Ignored~\nUnused\n");
        assert_eq!(render_class(&spaced), render_class(&compact), "{layout}");
    }
}

#[test]
fn class_neo_dividers_and_marker_references_follow_look_and_gradient() {
    for (look, use_gradient, divider_class, margin) in [
        ("neo", false, "divider neo-line", "-margin"),
        ("neo", true, "divider", "-margin"),
        ("classic", false, "divider", ""),
    ] {
        let svg = render_class(&format!(
            "---\nconfig:\n  layout: dagre\n  look: {look}\n  themeVariables:\n    useGradient: {use_gradient}\n---\nclassDiagram\nA <|-- B\nB ..> C\n"
        ));
        let document = roxmltree::Document::parse(&svg).expect("valid SVG");
        let dividers: Vec<_> = document
            .descendants()
            .filter(|node| {
                node.attribute("class")
                    .is_some_and(|value| value.split_whitespace().any(|class| class == "divider"))
            })
            .collect();
        assert!(!dividers.is_empty());
        for divider in dividers {
            assert_eq!(divider.attribute("class"), Some(divider_class));
        }
        for path in document
            .descendants()
            .filter(|node| node.has_tag_name("path") && node.attribute("data-edge") == Some("true"))
        {
            let style = path.attribute("style").unwrap_or_default();
            if look == "neo" {
                assert!(style.starts_with("stroke-dasharray: 0 0 "), "{style}");
                if path
                    .attribute("class")
                    .is_some_and(|classes| classes.contains("edge-pattern-dashed"))
                {
                    assert!(style.starts_with("stroke-dasharray: 0 0 2 2 "), "{style}");
                }
            } else {
                assert!(!style.contains("stroke-dashoffset:"), "{style}");
            }
        }
        for (attribute, marker) in [
            ("marker-start", "extensionStart"),
            ("marker-end", "dependencyEnd"),
        ] {
            let reference = document
                .descendants()
                .find_map(|node| node.attribute(attribute))
                .expect("relation marker");
            assert!(
                reference.ends_with(&format!("-{marker}{margin})")),
                "{reference}"
            );
            let id = reference
                .strip_prefix("url(#")
                .unwrap()
                .strip_suffix(')')
                .unwrap();
            let definition = document
                .descendants()
                .find(|node| node.has_tag_name("marker") && node.attribute("id") == Some(id))
                .expect("referenced marker definition");
            if look == "neo" && marker == "extensionStart" {
                let polygon = definition.first_element_child().expect("extension polygon");
                assert_eq!(
                    polygon.attribute("style"),
                    Some("stroke-width: 2; stroke-dasharray: 0;")
                );
            }
        }
    }
}

#[cfg(feature = "layout-shoal")]
#[test]
fn class_shoal_native_heading_fill_meets_first_divider() {
    use kurbo::{BezPath, PathEl, Shape};
    use merman_render::theme::{Appearance, BuiltinPreset, NativeTheme, ThemeCompiler};

    let theme = ThemeCompiler::new()
        .compile_preset(BuiltinPreset::Studio, Some(Appearance::Dark))
        .unwrap();
    for html in [false, true] {
        let source = format!(
            "---\nconfig:\n  layout: shoal\n  look: neo\n  htmlLabels: {html}\n  class:\n    htmlLabels: {html}\n---\nclassDiagram\nclass Account {{\n  +String id\n  +balance() Money\n}}\n",
        );
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(&source, ParseOptions::default())
            .unwrap()
            .unwrap()
            .into_presentation();
        let session = RenderEnvironment::deterministic()
            .begin_session_with_native_theme(NativeTheme::new(theme.clone()))
            .unwrap();
        let artifact =
            family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session).unwrap();
        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let heading = document
            .descendants()
            .find(|node| node.attribute("class") == Some("class-native-header"))
            .unwrap();
        let divider = heading
            .parent()
            .unwrap()
            .children()
            .find(|node| node.attribute("class") == Some("divider neo-line"))
            .and_then(|node| node.first_element_child())
            .unwrap();
        let path = |node: roxmltree::Node<'_, '_>| {
            BezPath::from_svg(node.attribute("d").unwrap()).unwrap()
        };
        let PathEl::MoveTo(first) = path(divider).elements()[0] else {
            panic!("divider start")
        };
        let bottom = path(heading).bounding_box().y1;
        assert!(
            (bottom - first.y).abs() < 1e-6,
            "html={html}: heading {bottom}, divider {}",
            first.y
        );
    }
}

#[cfg(feature = "layout-shoal")]
#[test]
fn class_shoal_neo_dividers_keep_solid_guides_with_gradients() {
    for (theme, guide) in [("dark", "#ccc"), ("forest", "#13540c")] {
        for html_labels in [false, true] {
            let svg = render_class(&format!(
                "---\nconfig:\n  layout: shoal\n  look: neo\n  theme: {theme}\n  htmlLabels: {html_labels}\n  themeVariables:\n    useGradient: true\n---\nclassDiagram\nclass Order\nclass Invoice\nOrder --> Invoice\nstyle Invoice stroke:#123456,stroke-width:3px\n"
            ));
            let document = roxmltree::Document::parse(&svg).expect("valid Class SVG");
            let css = document
                .descendants()
                .find(|node| node.has_tag_name("style"))
                .and_then(|node| node.text())
                .unwrap();
            assert!(css.contains(r#"[data-look="neo"].node path{stroke:url(#merman-gradient)"#));
            assert!(css.contains(&format!(
                r#"[data-look="neo"].node .neo-line path{{stroke:{guide};filter:none;}}"#
            )));
            for (id, source_style) in [
                ("merman-classId-Order-0", ""),
                (
                    "merman-classId-Invoice-1",
                    "stroke:#123456;stroke-width:3px",
                ),
            ] {
                let node = document
                    .descendants()
                    .find(|node| node.attribute("id") == Some(id))
                    .expect("class node");
                assert_eq!(node.attribute("data-look"), Some("neo"));
                let dividers = node.children().filter(|child| {
                    child.attribute("class").is_some_and(|classes| {
                        classes.split_whitespace().any(|class| class == "divider")
                    })
                });
                assert_eq!(dividers.clone().count(), 2, "{theme}/{html_labels}: {id}");
                for divider in dividers {
                    assert_eq!(divider.attribute("class"), Some("divider neo-line"));
                    let path = divider.first_element_child().expect("divider path");
                    assert_eq!(path.attribute("style"), Some(source_style));
                }
            }
        }
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn class_elk_omits_unlabelled_edges_but_keeps_terminal_only_labels() {
    let svg = render_class(
        "---\nconfig:\n  layout: elk\n---\nclassDiagram\nA --> B\nB \"1\" --> \"*\" C\n",
    );
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");
    let labels = document
        .descendants()
        .find(|node| node.attribute("class") == Some("edgeLabels"))
        .expect("edge labels container");
    let centers = labels
        .children()
        .filter(|node| node.attribute("class") == Some("edgeLabel"))
        .count();
    let terminals = labels
        .children()
        .filter(|node| node.attribute("class") == Some("edgeTerminals"))
        .count();
    assert_eq!(
        centers, 1,
        "only the cardinality edge receives a center wrapper"
    );
    assert_eq!(terminals, 2);
    assert!(
        document
            .descendants()
            .any(|node| node.attribute("class") == Some("edges edgePaths"))
    );
}

#[cfg(feature = "layout-elk")]
#[test]
fn class_elk_keeps_each_edges_center_and_terminal_labels_together() {
    let svg =
        render_class("classDiagram\nA \"1\" --> \"*\" B : first\nB \"1\" --> \"*\" C : second\n");
    let document = roxmltree::Document::parse(&svg).unwrap();
    let labels = document
        .descendants()
        .find(|node| node.attribute("class") == Some("edgeLabels"))
        .unwrap();
    let classes = labels
        .children()
        .filter_map(|node| node.attribute("class"))
        .collect::<Vec<_>>();
    assert_eq!(
        classes,
        [
            "edgeLabel",
            "edgeTerminals",
            "edgeTerminals",
            "edgeLabel",
            "edgeTerminals",
            "edgeTerminals"
        ]
    );
}

#[cfg(feature = "layout-elk")]
#[test]
fn class_elk_normalizes_breaks_before_markdown_block_parsing() {
    let svg = render_class("classDiagram\nAnimal <|-- Duck : Hello<br/>- l1<br/>- l2\n");
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");
    let label = document
        .descendants()
        .find(|node| node.has_tag_name("span") && node.attribute("class") == Some("edgeLabel"))
        .expect("edge label");
    let elements: Vec<_> = label.children().filter(|node| node.is_element()).collect();
    assert_eq!(elements.len(), 1);
    assert!(elements[0].has_tag_name("p"));
    assert_eq!(elements[0].text(), Some("Hello"));
    assert!(!elements[0].children().any(|node| node.is_element()));
    let trailing: String = label
        .children()
        .filter(|node| node.is_text())
        .filter_map(|node| node.text())
        .collect();
    assert_eq!(
        trailing.split_whitespace().collect::<Vec<_>>(),
        ["-", "l1", "-", "l2"]
    );
}

#[test]
fn class_html_label_metrics_inherit_the_theme_font_size() {
    for (font_size, expected_height) in [(14, 21.0), (22, 33.0)] {
        let svg = render_class(&format!(
            "---\nconfig:\n  layout: dagre\n  themeVariables:\n    fontSize: '{font_size}px'\n---\nclassDiagram\nA --> B : calls\n"
        ));
        let document = roxmltree::Document::parse(&svg).unwrap();
        let label = document
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node.attribute("class") == Some("edgeLabel")
                    && node.attribute("transform").is_some()
            })
            .expect("relation label");
        let foreign_object = label
            .descendants()
            .find(|node| node.has_tag_name("foreignObject"))
            .expect("HTML relation label");
        let height: f64 = foreign_object.attribute("height").unwrap().parse().unwrap();
        assert_eq!(height, expected_height);
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn class_elk_routes_relations_with_rounded_corners() {
    let fixture =
        include_str!("../../../fixtures/class/stress_class_many_relations_labels_020.mmd");
    let svg = render_class(&format!("---\nconfig:\n  layout: elk\n---\n{fixture}"));
    let document = roxmltree::Document::parse(&svg).unwrap();
    let paths: Vec<_> = document
        .descendants()
        .filter(|node| node.attribute("data-edge") == Some("true"))
        .map(|node| node.attribute("d").expect("relation path"))
        .collect();
    assert_eq!(paths.len(), 8);
    assert!(
        paths.iter().any(|path| path.contains('Q')),
        "bent routes need quadratic corners"
    );
    assert!(
        paths.iter().all(|path| !path.contains('C')),
        "ELK must not smooth routes with basis curves"
    );
}

#[test]
fn class_namespace_relations_paint_above_frames_with_scoped_markers() {
    let layouts = if cfg!(feature = "layout-elk") {
        &["dagre", "elk"][..]
    } else {
        &["dagre"][..]
    };
    for layout in layouts {
        for direction in ["LR", "RL", "TB", "BT"] {
            for depth in [0, 1, 3] {
                let namespace = (0..depth)
                    .map(|index| format!("Scope{index}"))
                    .collect::<Vec<_>>()
                    .join(".");
                let classes = if namespace.is_empty() {
                    "class A\nclass B\n".to_owned()
                } else {
                    format!("namespace {namespace} {{\nclass A\nclass B\n}}\n")
                };
                let text = format!(
                    "---\nconfig:\n  layout: {layout}\n---\nclassDiagram\ndirection {direction}\n{classes}A --> B : association\nA <|-- B : inheritance\nA o-- B : aggregation\nA *-- B : composition\nA ..> B : dependency\nA -- B : link\n"
                );
                let diagram_id = format!("class-{layout}-{direction}-{depth}");
                let svg = render_class_with_id(&text, &diagram_id);
                let document = roxmltree::Document::parse(&svg).expect("valid Class SVG");
                let roots = document
                    .descendants()
                    .filter(|node| node.attribute("class") == Some("root"));
                for root in roots {
                    let groups = root
                        .children()
                        .filter(|node| node.is_element())
                        .filter_map(|node| node.attribute("class"))
                        .collect::<Vec<_>>();
                    assert_eq!(
                        groups,
                        [
                            "clusters",
                            if *layout == "elk" {
                                "edges edgePaths"
                            } else {
                                "edgePaths"
                            },
                            "edgeLabels",
                            "nodes",
                        ],
                        "{diagram_id}: namespace fills must not cover relations"
                    );
                }
                let paths = document
                    .descendants()
                    .filter(|node| node.attribute("data-edge") == Some("true"))
                    .collect::<Vec<_>>();
                assert_eq!(paths.len(), 6, "{diagram_id}: every relation must render");
                let mut marker_count = 0;
                for path in paths {
                    for reference in [path.attribute("marker-start"), path.attribute("marker-end")]
                        .into_iter()
                        .flatten()
                    {
                        marker_count += 1;
                        let id = reference
                            .strip_prefix("url(#")
                            .and_then(|value| value.strip_suffix(')'))
                            .expect("local marker reference");
                        assert!(id.starts_with(&format!("{diagram_id}_")), "{id}");
                        assert_eq!(
                            document
                                .descendants()
                                .filter(|node| node.has_tag_name("marker")
                                    && node.attribute("id") == Some(id))
                                .count(),
                            1,
                            "{diagram_id}: each arrow must resolve to its own definition"
                        );
                    }
                }
                assert_eq!(
                    marker_count, 5,
                    "{diagram_id}: all arrow types retain markers"
                );
            }
        }
    }
}

#[test]
fn class_nested_namespace_original_input_retains_visible_relation_layers() {
    let text = include_str!("../../../fixtures/class/upstream_namespaces_and_generics.mmd");
    let first = render_class_with_id(text, "first-class");
    let second = render_class_with_id(text, "second-class");
    let combined = format!("<diagrams>{first}{second}</diagrams>");
    let document = roxmltree::Document::parse(&combined).expect("two valid Class SVGs");
    let mut ids = std::collections::HashSet::new();
    for node in document.descendants() {
        if let Some(id) = node.attribute("id") {
            assert!(
                ids.insert(id),
                "multiple diagrams must not duplicate id {id}"
            );
        }
    }
    for diagram in document
        .root_element()
        .children()
        .filter(|node| node.is_element())
    {
        let relation = diagram
            .descendants()
            .find(|node| node.attribute("data-id") == Some("id_Admin_User_1"))
            .expect("original manages relation");
        assert!(relation.attribute("marker-end").is_some());
        let edge_group = relation.parent_element().expect("edge group");
        let groups = edge_group
            .parent_element()
            .expect("render root")
            .children()
            .filter(|node| node.is_element())
            .collect::<Vec<_>>();
        let clusters = groups
            .iter()
            .position(|node| node.attribute("class") == Some("clusters"))
            .expect("namespace frames");
        let edges = groups
            .iter()
            .position(|node| *node == edge_group)
            .expect("relation layer");
        assert!(
            clusters < edges,
            "namespace backgrounds must paint before manages"
        );
    }
}

#[test]
fn class_margin_marker_stroke_widths_follow_shared_mermaid_definitions() {
    let svg = render_class("classDiagram\nA o-- B\nB *-- C\nC --> D\n");
    let document = roxmltree::Document::parse(&svg).unwrap();
    for (kind, width) in [
        ("aggregation", "2"),
        ("composition", "0"),
        ("dependency", "0"),
    ] {
        for end in ["Start", "End"] {
            let id = format!("merman_classDiagram-{kind}{end}-margin");
            let marker = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("marker") && node.attribute("id") == Some(id.as_str())
                })
                .expect("margin marker");
            let path = marker.first_element_child().expect("marker path");
            assert_eq!(
                path.attribute("style"),
                Some(format!("stroke-width: {width};").as_str()),
                "{id}"
            );
        }
    }
}

#[test]
fn class_list_links_reach_each_declared_svg_node() {
    let layouts = if cfg!(feature = "layout-elk") {
        &["dagre", "elk"][..]
    } else {
        &["dagre"][..]
    };
    for layout in layouts {
        let svg = render_class(&format!(
            "---\nconfig:\n  layout: {layout}\n---\nclassDiagram\nclass A\nclass B\nclick `A,Missing,B` href \"https://example.com\"\n"
        ));
        let document = roxmltree::Document::parse(&svg).expect("valid Class SVG");
        let anchors: Vec<_> = document
            .descendants()
            .filter(|node| node.has_tag_name("a"))
            .collect();
        assert_eq!(anchors.len(), 2, "{layout}: only declared targets link");
        for id in ["A", "B"] {
            let prefix = format!("merman-classId-{id}-");
            let node = document
                .descendants()
                .find(|node| {
                    node.attribute("id")
                        .is_some_and(|id| id.starts_with(&prefix))
                })
                .expect("linked Class node");
            let anchor = node.parent_element().expect("link owns Class node");
            assert!(anchor.has_tag_name("a"));
            assert_eq!(
                anchor.attribute(("http://www.w3.org/1999/xlink", "href")),
                Some("https://example.com/"),
                "{layout}: {id} retains its URL"
            );
        }
    }
}

#[test]
fn class_quoted_namespace_whitespace_preserves_distinct_svg_clusters() {
    let layouts = if cfg!(feature = "layout-elk") {
        &["dagre", "elk"][..]
    } else {
        &["dagre"][..]
    };
    for layout in layouts {
        let svg = render_class(&format!(
            "---\nconfig:\n  layout: {layout}\n---\nclassDiagram\nnamespace `N ` {{\nclass A\n}}\nnamespace N {{\nclass B\n}}\nA --> B\n"
        ));
        let document = roxmltree::Document::parse(&svg).expect("valid Class SVG");
        let clusters = document
            .descendants()
            .filter(|node| {
                node.attribute("class")
                    .is_some_and(|classes| classes.split_whitespace().any(|name| name == "cluster"))
            })
            .collect::<Vec<_>>();
        assert_eq!(clusters.len(), 2, "{layout}: both namespace frames survive");
        assert!(document.descendants().any(|node| {
            node.attribute("data-id") == Some("id_A_B_1") && node.attribute("marker-end").is_some()
        }));
    }
}
