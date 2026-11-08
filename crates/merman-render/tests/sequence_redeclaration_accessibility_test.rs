#![cfg(feature = "diagram-sequence")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::{Value, json};

fn prepare_and_render(source: &str) -> (Value, String) {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
    let projection = artifact.layout_json().unwrap().into_unmanaged_value();
    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("sequence-redeclaration-accessibility".to_owned()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .unwrap();
    (projection, rendered.svg().to_owned())
}

#[test]
fn sequence_redeclaration_preserves_prepared_and_mirrored_participant_captions() {
    for declaration in ["participant A", "participant A as Alice"] {
        let source =
            format!("sequenceDiagram\nparticipant A as Alice\n{declaration}\nA->>B: ping\n");
        let (projection, svg) = prepare_and_render(&source);
        assert_eq!(
            projection["semantic"]["actors"]["A"]["description"],
            "Alice"
        );
        assert_eq!(projection["semantic"]["actorOrder"], json!(["A", "B"]));
        let nodes = projection["layout"]["SequenceDiagram"]["nodes"]
            .as_array()
            .unwrap();
        assert_eq!(nodes.len(), 4);
        for id in ["actor-top-A", "actor-bottom-A"] {
            assert_eq!(nodes.iter().filter(|node| node["id"] == id).count(), 1);
        }

        let document = roxmltree::Document::parse(&svg).unwrap();
        let captions: Vec<_> = document
            .descendants()
            .filter(|node| {
                node.has_tag_name("text")
                    && node.attribute("class").is_some_and(|class| {
                        class.split_whitespace().any(|class| class == "actor-box")
                    })
            })
            .map(|node| {
                node.descendants()
                    .filter(|node| node.is_text())
                    .filter_map(|node| node.text())
                    .collect::<String>()
            })
            .collect();
        assert_eq!(captions.len(), 4);
        assert_eq!(
            captions
                .iter()
                .filter(|caption| *caption == "Alice")
                .count(),
            2
        );
        assert_eq!(captions.iter().filter(|caption| *caption == "B").count(), 2);
    }
}

#[test]
fn sequence_accessibility_preserves_prepared_metadata_and_referenced_svg_text() {
    for (title, description) in [
        (
            "Request # draft; ready",
            "Response; A->>Ghost: literal # details",
        ),
        ("Request draft", "Response details"),
    ] {
        let source =
            format!("sequenceDiagram\naccTitle: {title}\naccDescr: {description}\nA->>B: ping\n");
        let (projection, svg) = prepare_and_render(&source);
        assert_eq!(projection["semantic"]["accTitle"], title);
        assert_eq!(projection["semantic"]["accDescr"], description);
        assert_eq!(projection["semantic"]["actorOrder"], json!(["A", "B"]));
        assert_eq!(
            projection["semantic"]["messages"].as_array().unwrap().len(),
            1
        );

        let document = roxmltree::Document::parse(&svg).unwrap();
        let root = document.root_element();
        for (attribute, tag, expected) in [
            ("aria-labelledby", "title", title),
            ("aria-describedby", "desc", description),
        ] {
            let id = root.attribute(attribute).expect("accessibility reference");
            let element = root
                .children()
                .find(|node| node.has_tag_name(tag) && node.attribute("id") == Some(id))
                .expect("referenced accessibility element");
            assert_eq!(element.text(), Some(expected));
        }
    }
}
