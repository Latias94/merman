#![cfg(feature = "diagram-kanban")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn prepare_and_render(body: &str) -> (serde_json::Value, String) {
    let source = format!(
        "---\nconfig:\n  securityLevel: loose\n  kanban:\n    ticketBaseUrl: https://example.invalid/#TICKET#\n---\n{body}"
    );
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .expect("parse Kanban regression")
        .expect("detect Kanban")
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare Kanban regression");
    let projection = artifact.layout_json().unwrap();
    let svg = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("kanban-123".to_owned()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render Kanban regression")
        .svg()
        .to_owned();
    (projection["layout"]["KanbanDiagram"].clone(), svg)
}

fn card<'a>(document: &'a roxmltree::Document<'a>) -> roxmltree::Node<'a, 'a> {
    document
        .descendants()
        .find(|node| node.has_tag_name("g") && node.attribute("id") == Some("kanban-123-card"))
        .expect("rendered card with diagram-prefixed ID")
}

fn visible_text(node: roxmltree::Node<'_, '_>) -> String {
    node.descendants()
        .filter(|child| child.is_text())
        .filter_map(|child| child.text())
        .collect()
}

#[test]
fn kanban_ticket_truthiness_reaches_prepared_items_and_actual_links() {
    for (raw, expected) in [
        ("0", None),
        ("false", None),
        ("null", None),
        ("''", None),
        ("'0'", Some("0")),
        ("1", Some("1")),
    ] {
        let (layout, svg) = prepare_and_render(&format!(
            "kanban\n  todo[Todo]\n    card[Task]@{{ ticket: {raw} }}\n"
        ));
        let items = layout["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["ticket"].as_str(), expected, "ticket: {raw}");

        let document = roxmltree::Document::parse(&svg).expect("valid Kanban SVG");
        let card = card(&document);
        let links: Vec<_> = card
            .descendants()
            .filter(|node| {
                node.has_tag_name("a") && node.attribute("class") == Some("kanban-ticket-link")
            })
            .collect();
        assert_eq!(
            links.len(),
            usize::from(expected.is_some()),
            "ticket: {raw}"
        );
        if let Some(ticket) = expected {
            let href = links[0]
                .attributes()
                .find(|attribute| attribute.name() == "href")
                .expect("ticket link href");
            assert_eq!(href.value(), format!("https://example.invalid/{ticket}"));
            assert_eq!(visible_text(links[0]).trim(), ticket);
        } else {
            assert_eq!(visible_text(card).trim(), "Task");
        }
    }
}

#[test]
fn kanban_physical_label_lines_render_with_following_metadata_and_card() {
    for (label, expected) in [
        ("[\"First\nSecond\"]", "First\nSecond"),
        ("[\"First Second\"]", "First Second"),
    ] {
        let (layout, svg) = prepare_and_render(&format!(
            "kanban\n  todo[Todo]\n    card{label}@{{\n      assigned: 'Alice'\n    }}\n    next[Next]\n"
        ));
        let items = layout["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["id"], "card");
        assert_eq!(items[0]["label"], expected);
        assert_eq!(items[0]["assigned"], "Alice");
        assert_eq!(items[0]["parent_id"], "todo");
        assert_eq!(items[1]["id"], "next");
        assert_eq!(items[1]["parent_id"], "todo");

        let document = roxmltree::Document::parse(&svg).unwrap();
        let text = visible_text(card(&document));
        for word in ["First", "Second", "Alice"] {
            assert!(text.contains(word), "missing {word:?}: {svg}");
        }
        assert!(document.descendants().any(|node| {
            node.has_tag_name("g") && node.attribute("id") == Some("kanban-123-next")
        }));
    }
}

#[test]
fn kanban_literal_percent_labels_and_metadata_reach_the_svg() {
    for label in ["[Progress %% complete]", "[\"Progress %% complete\"]"] {
        let (layout, svg) = prepare_and_render(&format!(
            "kanban\n  todo[Todo]\n    card{label}@{{ assigned: 'Alice %% Bob' }} %% hidden\n    next[Next]\n"
        ));
        let items = layout["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["label"], "Progress %% complete");
        assert_eq!(items[0]["assigned"], "Alice %% Bob");
        assert_eq!(items[1]["id"], "next");

        let document = roxmltree::Document::parse(&svg).unwrap();
        let text = visible_text(card(&document));
        assert!(text.contains("Progress %% complete"));
        assert!(text.contains("Alice %% Bob"));
        assert!(!text.contains("hidden"));
    }
}
