#![cfg(any(
    feature = "diagram-er",
    feature = "diagram-sequence",
    feature = "diagram-state"
))]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::Value;

fn prepare_and_render(source: &str) -> (Value, String) {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse source-backed syntax")
        .expect("detect selected family")
        .into_presentation();
    let artifact = family::prepare(
        &parsed,
        &LayoutOptions::default(),
        RenderEnvironment::deterministic().begin_session().unwrap(),
    )
    .expect("prepare parsed semantics");
    let projection = artifact.layout_json().unwrap().into_unmanaged_value();
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render prepared semantics")
        .svg()
        .to_owned();
    (projection["semantic"].clone(), svg)
}

#[cfg(feature = "diagram-sequence")]
#[test]
fn sequence_continuation_identity_and_wrapped_menu_reach_svg() {
    let (model, svg) = prepare_and_render(concat!(
        "sequenceDiagram\nparticipant api/v2 as Service\n",
        "links api/v2: wrap: {\"Docs\":\"https://example.test/docs\"}\n",
        "link api/v2: nowrap: Help @ https://example.test/help\n",
        "api/v2->>Other: request\n",
    ));
    assert_eq!(model["actors"]["api/v2"]["description"], "Service");
    assert_eq!(model["messages"][0]["from"], "api/v2");
    let document = roxmltree::Document::parse(&svg).unwrap();
    for (label, url) in [
        ("Docs", "https://example.test/docs"),
        ("Help", "https://example.test/help"),
    ] {
        assert!(
            document.descendants().any(|node| {
                node.has_tag_name("a")
                    && node
                        .attributes()
                        .any(|attr| attr.name() == "href" && attr.value() == url)
                    && node
                        .descendants()
                        .any(|child| child.is_text() && child.text() == Some(label))
            }),
            "missing menu link {label}"
        );
    }
}

#[cfg(feature = "diagram-er")]
#[test]
fn er_empty_backticks_and_style_token_whitespace_reach_svg() {
    let (model, svg) = prepare_and_render(concat!(
        "erDiagram\nE {``int ``id``PK}\n",
        "style E fill:\u{feff}#abcdef\n",
    ));
    assert_eq!(model["entities"]["E"]["attributes"][0]["name"], "id");
    assert_eq!(model["entities"]["E"]["attributes"][0]["type"], "int");
    assert_eq!(model["entities"]["E"]["cssStyles"][0], "fill:#abcdef");
    let document = roxmltree::Document::parse(&svg).unwrap();
    assert!(document.descendants().any(|node| {
        node.attribute("style")
            .is_some_and(|style| style.contains("#abcdef"))
    }));
    assert!(
        document
            .descendants()
            .any(|node| node.is_text() && node.text() == Some("id"))
    );
}

#[cfg(feature = "diagram-state")]
#[test]
fn state_next_line_alias_block_and_complete_note_reach_prepared_output() {
    let (model, svg) = prepare_and_render(
        "stateDiagram-v2\nstate \"Container\" as Outer\n{\nA\n}\nOuter --> Done\n",
    );
    let nodes = model["nodes"].as_array().unwrap();
    assert!(
        nodes
            .iter()
            .any(|node| node["id"] == "Outer" && node["isGroup"] == true)
    );
    assert!(
        nodes
            .iter()
            .any(|node| node["id"] == "A" && node["parentId"] == "Outer")
    );
    assert!(svg.contains("Container"));

    let expected = "first  \n    indented\n\n      deepest\n  last";
    let (model, svg) = prepare_and_render(concat!(
        "stateDiagram-v2\nA\nnote right of A\n",
        "  first  \n    indented\n\n      deepest\n  last  \nend note\nA --> B\n",
    ));
    assert!(
        model["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| { node["shape"] == "note" && node["label"] == expected })
    );
    let document = roxmltree::Document::parse(&svg).unwrap();
    for word in ["first", "indented", "deepest", "last"] {
        assert!(
            document.descendants().any(|node| {
                node.is_text() && node.text().is_some_and(|text| text.contains(word))
            })
        );
    }
}
