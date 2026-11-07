#![cfg(feature = "diagram-c4")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::C4DiagramLayout;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn render(source: &str) -> (C4DiagramLayout, String) {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse C4")
        .expect("C4 diagram")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("render session");
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare C4");
    let projection = artifact.layout_json().expect("layout projection");
    let layout =
        serde_json::from_value(projection["layout"]["C4Diagram"].clone()).expect("typed C4 layout");
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render C4 SVG")
        .svg()
        .to_owned();
    (layout, svg)
}

fn text_content(node: roxmltree::Node<'_, '_>) -> String {
    node.descendants()
        .filter(|descendant| descendant.is_text())
        .filter_map(|descendant| descendant.text())
        .collect()
}

fn section_rows(document: &roxmltree::Document<'_>, class: &str) -> Vec<Vec<String>> {
    document
        .descendants()
        .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some(class))
        .map(|section| {
            section
                .descendants()
                .filter(|node| {
                    node.has_tag_name("tspan")
                        && node.attribute("class") == Some("text-outer-tspan row")
                })
                .map(text_content)
                .collect()
        })
        .collect()
}

#[test]
fn c4_trimmed_quoted_aliases_resolve_relationship_endpoints_and_style_targets() {
    let source = concat!(
        "C4Context\n",
        "Person(\" a \", \" Alice \", \" Description \")\n",
        "System(\" b \", \" Backend \")\n",
        "Rel(a, b, \" Uses \")\n",
        "UpdateElementStyle(\" b \", $bgColor=\" #123456 \")\n",
        "UpdateRelStyle(\" a \", \" b \", $lineColor=\" #654321 \")\n",
    );
    let (layout, svg) = render(source);
    assert_eq!(layout.shapes.len(), 2);
    assert_eq!(layout.shapes[0].alias, "a");
    assert_eq!(layout.shapes[1].alias, "b");
    assert_eq!(layout.rels[0].from, "a");
    assert_eq!(layout.rels[0].to, "b");
    assert_eq!(layout.rels[0].label.text, "Uses");
    let document = roxmltree::Document::parse(&svg).unwrap();
    assert_eq!(
        section_rows(&document, "c4-name"),
        [vec!["Alice"], vec!["Backend"]]
    );
    assert_eq!(section_rows(&document, "c4-descr"), [vec!["Description"]]);
    assert!(
        document
            .descendants()
            .any(|node| node.has_tag_name("text") && text_content(node) == "Uses")
    );
    assert!(document.descendants().any(|node| {
        node.attribute("style")
            .is_some_and(|style| style.contains("fill:#123456 !important"))
    }));
    assert!(document.descendants().any(|node| {
        node.attribute("marker-end").is_some() && node.attribute("stroke") == Some("#654321")
    }));
}

#[test]
fn c4_explicit_name_and_description_rows_reach_layout_and_svg_with_or_without_wrap() {
    for wrap in [false, true] {
        for line_break in ["\\n", "\n", "<br/>", "<BR>"] {
            let source = format!(
                "---\nconfig:\n  c4:\n    wrap: {wrap}\n---\nC4Context\n\
                 Person(a, \"Alice{line_break}Smith\", \"First{line_break}Second\")\n"
            );
            let (layout, svg) = render(&source);
            let shape = &layout.shapes[0];
            assert_eq!(
                shape.label.line_count, 2,
                "wrap={wrap} break={line_break:?}"
            );
            assert_eq!(shape.descr.as_ref().unwrap().line_count, 2);
            let document = roxmltree::Document::parse(&svg).unwrap();
            assert_eq!(section_rows(&document, "c4-name"), [vec!["Alice", "Smith"]]);
            assert_eq!(
                section_rows(&document, "c4-descr"),
                [vec!["First", "Second"]]
            );
        }
    }
}
#[test]
fn c4_decoded_literal_breaks_and_ordinary_spaces_remain_final_single_rows() {
    for wrap in [false, true] {
        let source = format!(
            "---\nconfig:\n  c4:\n    wrap: {wrap}\n---\nC4Context\n\
             System(encoded, \"A&lt;br/&gt;B\")\nSystem(space, \"Alice Smith\")\n"
        );
        let (layout, svg) = render(&source);
        for shape in &layout.shapes {
            assert_eq!(shape.label.line_count, 1);
        }
        let document = roxmltree::Document::parse(&svg).unwrap();
        assert_eq!(
            section_rows(&document, "c4-name"),
            [vec!["A<br/>B"], vec!["Alice Smith"]]
        );
    }
}

#[test]
fn c4_explicit_rows_increase_geometry_without_changing_unwrapped_source_text() {
    let source_for = |text: &str| {
        format!("---\nconfig:\n  c4:\n    wrap: false\n---\nC4Context\nSystem(a, \"{text}\")\n")
    };
    let (single, _) = render(&source_for("Alice Smith"));
    let (escaped, escaped_svg) = render(&source_for("Alice\\nSmith"));
    let (physical, physical_svg) = render(&source_for("Alice\nSmith"));
    let single = &single.shapes[0];
    let escaped = &escaped.shapes[0];
    let physical = &physical.shapes[0];
    assert!(escaped.label.height > single.label.height);
    assert!(escaped.height > single.height);
    assert_eq!(escaped.label.height, physical.label.height);
    assert_eq!(escaped.height, physical.height);
    assert_eq!(escaped.label.text, "Alice\\nSmith");
    assert_eq!(physical.label.text, "Alice\nSmith");
    assert_eq!(
        section_rows(
            &roxmltree::Document::parse(&escaped_svg).unwrap(),
            "c4-name"
        ),
        section_rows(
            &roxmltree::Document::parse(&physical_svg).unwrap(),
            "c4-name"
        )
    );
}
