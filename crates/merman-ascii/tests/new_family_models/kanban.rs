use super::*;

#[test]
fn kanban_render_model_renders_groups_and_child_metadata() {
    let model = KanbanDiagramRenderModel {
        nodes: vec![
            kanban_node("backlog", "Backlog", true, KanbanNodeMetadata::default()),
            kanban_node(
                "card-a",
                "Ticket A",
                false,
                KanbanNodeMetadata {
                    parent_id: Some("backlog"),
                    ticket: Some("K-1"),
                    priority: Some("high"),
                    assigned: Some("alice"),
                    icon: Some("bug"),
                },
            ),
            kanban_node(
                "card-b",
                "Ticket B",
                false,
                KanbanNodeMetadata {
                    parent_id: Some("backlog"),
                    ticket: Some("K-2"),
                    ..Default::default()
                },
            ),
            kanban_node("doing", "Doing", true, KanbanNodeMetadata::default()),
            kanban_node(
                "card-c",
                "Ticket C",
                false,
                KanbanNodeMetadata {
                    parent_id: Some("doing"),
                    ticket: Some("K-3"),
                    ..Default::default()
                },
            ),
        ],
    };

    let rendered = render(RenderSemanticModel::Kanban(model));

    assert_eq!(
        rendered,
        concat!(
            "group(bytes=7)=\"Backlog\" [id(bytes=7)=\"backlog\"]\n",
            "  - card(bytes=8)=\"Ticket A\" [id(bytes=6)=\"card-a\", ticket(bytes=3)=\"K-1\",\n",
            "    priority(bytes=4)=\"high\", assigned(bytes=5)=\"alice\", icon(bytes=3)=\"bug\"]\n",
            "  - card(bytes=8)=\"Ticket B\" [id(bytes=6)=\"card-b\", ticket(bytes=3)=\"K-2\"]\n",
            "group(bytes=5)=\"Doing\" [id(bytes=5)=\"doing\"]\n",
            "  - card(bytes=8)=\"Ticket C\" [id(bytes=6)=\"card-c\", ticket(bytes=3)=\"K-3\"]",
        )
    );
}
#[test]
fn kanban_structured_text_framing_distinguishes_ticket_from_priority() {
    let embedded_priority = KanbanDiagramRenderModel {
        nodes: vec![kanban_node(
            "card",
            "Card",
            false,
            KanbanNodeMetadata {
                ticket: Some("K-1, priority=high"),
                ..Default::default()
            },
        )],
    };
    let explicit_priority = KanbanDiagramRenderModel {
        nodes: vec![kanban_node(
            "card",
            "Card",
            false,
            KanbanNodeMetadata {
                ticket: Some("K-1"),
                priority: Some("high"),
                ..Default::default()
            },
        )],
    };

    assert_ne!(
        render(RenderSemanticModel::Kanban(embedded_priority)),
        render(RenderSemanticModel::Kanban(explicit_priority)),
        "authored ticket text must not be able to forge a priority field",
    );
}

#[test]
fn kanban_group_parent_ownership_is_disclosed_without_nested_geometry() {
    let model = KanbanDiagramRenderModel {
        nodes: vec![
            kanban_node("root", "Root", true, KanbanNodeMetadata::default()),
            kanban_node(
                "child",
                "Child",
                true,
                KanbanNodeMetadata {
                    parent_id: Some("root, priority=high"),
                    ..Default::default()
                },
            ),
        ],
    };

    let rendered = render(RenderSemanticModel::Kanban(model));

    assert_eq!(
        rendered,
        concat!(
            "group(bytes=4)=\"Root\" [id(bytes=4)=\"root\"]\n",
            "group(bytes=5)=\"Child\" [id(bytes=5)=\"child\",\n",
            "parent(bytes=19)=\"root, priority=high\"]",
        )
    );
}

#[test]
fn kanban_render_model_keeps_unassigned_and_unknown_parent_cards() {
    let model = KanbanDiagramRenderModel {
        nodes: vec![
            kanban_node("backlog", "Backlog", true, KanbanNodeMetadata::default()),
            kanban_node(
                "known",
                "Known",
                false,
                KanbanNodeMetadata {
                    parent_id: Some("backlog"),
                    ..Default::default()
                },
            ),
            kanban_node("loose", "Loose", false, KanbanNodeMetadata::default()),
            kanban_node(
                "unknown",
                "Unknown",
                false,
                KanbanNodeMetadata {
                    parent_id: Some("missing"),
                    ticket: Some("K-404"),
                    ..Default::default()
                },
            ),
        ],
    };

    let rendered = render(RenderSemanticModel::Kanban(model));

    assert_eq!(
        rendered,
        concat!(
            "group(bytes=7)=\"Backlog\" [id(bytes=7)=\"backlog\"]\n",
            "  - card(bytes=5)=\"Known\" [id(bytes=5)=\"known\"]\n",
            "Unassigned\n",
            "  - card(bytes=5)=\"Loose\" [id(bytes=5)=\"loose\"]\n",
            "  - card(bytes=7)=\"Unknown\" [id(bytes=7)=\"unknown\", parent(bytes=7)=\"missing\",\n",
            "    ticket(bytes=5)=\"K-404\"]",
        )
    );
}

#[test]
fn kanban_render_model_rejects_duplicate_or_empty_ids() {
    let duplicate = KanbanDiagramRenderModel {
        nodes: vec![
            kanban_node("same", "A", true, KanbanNodeMetadata::default()),
            kanban_node("same", "B", false, KanbanNodeMetadata::default()),
        ],
    };
    let error = render_model(
        &RenderSemanticModel::Kanban(duplicate),
        &AsciiRenderOptions::ascii(),
    )
    .expect_err("group/card ids share one namespace");
    assert!(matches!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "kanban",
            feature: "duplicate node ids",
        }
    ));

    let empty = KanbanDiagramRenderModel {
        nodes: vec![kanban_node(
            "",
            "Empty",
            false,
            KanbanNodeMetadata::default(),
        )],
    };
    let error = render_model(
        &RenderSemanticModel::Kanban(empty),
        &AsciiRenderOptions::ascii(),
    )
    .expect_err("empty ids cannot provide stable card identity");
    assert!(matches!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "kanban",
            feature: "empty node ids",
        }
    ));
}

#[test]
fn kanban_parser_projection_keeps_group_metadata() {
    let rendered =
        render_parsed("kanban\n  root@{ priority: high, assigned: alice, icon: star }\n");

    assert_eq!(
        rendered,
        concat!(
            "group(bytes=4)=\"root\" [id(bytes=4)=\"root\", priority(bytes=4)=\"high\",\n",
            "assigned(bytes=5)=\"alice\", icon(bytes=4)=\"star\"]",
        )
    );
}
