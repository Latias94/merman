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
fn kanban_parser_projection_preserves_repeated_id3_from_documented_example() {
    // Mermaid 12.3.0 docs/syntax/kanban.md ends its full example with these columns.
    let rendered = render_parsed(concat!(
        "kanban\n",
        "  id11[Done]\n",
        "    id3[Update DB function]@{ ticket: MC-2037, assigned: knsv, priority: 'High' }\n",
        "  id12[Can't reproduce]\n",
        "    id3[Weird flickering in Firefox]\n",
    ));

    let (done, cant_reproduce) = rendered
        .split_once("group(bytes=15)=\"Can't reproduce\" [id(bytes=4)=\"id12\"]")
        .expect("the second column must remain after Done");
    assert!(done.starts_with("group(bytes=4)=\"Done\" [id(bytes=4)=\"id11\"]"));
    assert!(done.contains("card(bytes=18)=\"Update DB function\" [id(bytes=3)=\"id3\""));
    assert!(done.contains("ticket(bytes=7)=\"MC-2037\""));
    assert!(done.contains("assigned(bytes=4)=\"knsv\""));
    assert!(done.contains("priority(bytes=4)=\"High\""));
    assert!(cant_reproduce.contains("card(bytes=27)=\"Weird flickering in Firefox\""));
    assert!(cant_reproduce.contains("id(bytes=3)=\"id3\""));
    assert!(!cant_reproduce.contains("ticket("));
    assert!(!cant_reproduce.contains("assigned("));
    assert!(!cant_reproduce.contains("priority("));
    assert_eq!(rendered.matches("id(bytes=3)=\"id3\"").count(), 2);
}

#[test]
fn kanban_render_model_preserves_repeated_cards_in_same_and_distinct_groups() {
    let model = KanbanDiagramRenderModel {
        nodes: vec![
            kanban_node("todo", "Todo", true, KanbanNodeMetadata::default()),
            kanban_node(
                "same",
                "First",
                false,
                KanbanNodeMetadata {
                    parent_id: Some("todo"),
                    ticket: Some("K-1"),
                    priority: Some("high"),
                    assigned: Some("alice"),
                    icon: Some("bug"),
                },
            ),
            kanban_node(
                "same",
                "Second",
                false,
                KanbanNodeMetadata {
                    parent_id: Some("todo"),
                    ticket: Some("K-2"),
                    priority: Some("low"),
                    assigned: Some("bob"),
                    icon: Some("star"),
                },
            ),
            kanban_node("done", "Done", true, KanbanNodeMetadata::default()),
            kanban_node(
                "same",
                "Third",
                false,
                KanbanNodeMetadata {
                    parent_id: Some("done"),
                    ticket: Some("K-3"),
                    priority: Some("medium"),
                    assigned: Some("carol"),
                    icon: Some("check"),
                },
            ),
        ],
    };

    let rendered = render(RenderSemanticModel::Kanban(model));
    let (todo, done) = rendered
        .split_once("group(bytes=4)=\"Done\" [id(bytes=4)=\"done\"]")
        .expect("group order must match the source");
    let (first, second) = todo
        .split_once("  - card(bytes=6)=\"Second\"")
        .expect("the second occurrence must remain in Todo");
    assert!(first.contains("card(bytes=5)=\"First\""));
    for (card, fields) in [
        (
            first,
            [
                "ticket(bytes=3)=\"K-1\"",
                "priority(bytes=4)=\"high\"",
                "assigned(bytes=5)=\"alice\"",
                "icon(bytes=3)=\"bug\"",
            ],
        ),
        (
            second,
            [
                "ticket(bytes=3)=\"K-2\"",
                "priority(bytes=3)=\"low\"",
                "assigned(bytes=3)=\"bob\"",
                "icon(bytes=4)=\"star\"",
            ],
        ),
        (
            done,
            [
                "ticket(bytes=3)=\"K-3\"",
                "priority(bytes=6)=\"medium\"",
                "assigned(bytes=5)=\"carol\"",
                "icon(bytes=5)=\"check\"",
            ],
        ),
    ] {
        assert!(card.contains("id(bytes=4)=\"same\""));
        for field in fields {
            assert!(card.contains(field), "missing {field} in {card}");
        }
    }
    assert!(done.contains("card(bytes=5)=\"Third\""));
    assert_eq!(rendered.matches("id(bytes=4)=\"same\"").count(), 3);
}

#[test]
fn kanban_parser_projection_keeps_card_and_group_id_collisions() {
    let rendered = render_parsed(concat!(
        "kanban\n",
        "  todo[Todo]\n",
        "    todo[Own group ID]\n",
        "    done[Later group ID]\n",
        "  done[Done]\n",
        "    todo[Earlier group ID]\n",
    ));

    let (todo, done) = rendered
        .split_once("group(bytes=4)=\"Done\" [id(bytes=4)=\"done\"]")
        .expect("a card ID must not replace a group");
    assert!(todo.contains("card(bytes=12)=\"Own group ID\" [id(bytes=4)=\"todo\"]"));
    assert!(todo.contains("card(bytes=14)=\"Later group ID\" [id(bytes=4)=\"done\"]"));
    assert!(done.contains("card(bytes=16)=\"Earlier group ID\" [id(bytes=4)=\"todo\"]"));
    assert_eq!(rendered.matches("  - card(").count(), 3);
}

#[test]
fn kanban_repeated_cards_each_consume_output_budget() {
    use merman_ascii::AsciiResourceLimitId;

    let mut model = KanbanDiagramRenderModel {
        nodes: vec![kanban_node(
            "same",
            "Card",
            false,
            KanbanNodeMetadata::default(),
        )],
    };
    let first = render(RenderSemanticModel::Kanban(model.clone()));
    let policy = AsciiResourcePolicy::default()
        .with_limit(AsciiResourceLimitId::MaxOutputBytes, first.len())
        .unwrap();
    assert_eq!(
        support::render_model_with_resources(
            &RenderSemanticModel::Kanban(model.clone()),
            &AsciiRenderOptions::ascii(),
            policy,
        )
        .unwrap(),
        first,
    );
    model.nodes.push(model.nodes[0].clone());
    let error = support::render_model_with_resources(
        &RenderSemanticModel::Kanban(model),
        &AsciiRenderOptions::ascii(),
        policy,
    )
    .expect_err("a second occurrence must consume its own output budget");
    assert!(matches!(
        error,
        AsciiError::ResourceLimitExceeded(resource)
            if resource.limit == AsciiResourceLimitId::MaxOutputBytes
                && resource.max == first.len()
                && resource.actual > resource.max
    ));
}

#[test]
fn kanban_render_model_rejects_duplicate_group_ids() {
    let duplicate = KanbanDiagramRenderModel {
        nodes: vec![
            kanban_node("same", "A", true, KanbanNodeMetadata::default()),
            kanban_node("same", "B", true, KanbanNodeMetadata::default()),
        ],
    };
    let error = render_model(
        &RenderSemanticModel::Kanban(duplicate),
        &AsciiRenderOptions::ascii(),
    )
    .expect_err("parent IDs cannot identify which repeated group owns a card");
    assert!(matches!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "kanban",
            feature: "duplicate group ids",
        }
    ));
}

#[test]
fn kanban_render_model_rejects_empty_ids() {
    for is_group in [false, true] {
        let empty = KanbanDiagramRenderModel {
            nodes: vec![kanban_node(
                "",
                "Empty",
                is_group,
                KanbanNodeMetadata::default(),
            )],
        };
        let error = render_model(
            &RenderSemanticModel::Kanban(empty),
            &AsciiRenderOptions::ascii(),
        )
        .expect_err("authored group and card IDs must remain nonempty");
        assert!(matches!(
            error,
            AsciiError::UnsupportedFeature {
                diagram_type: "kanban",
                feature: "empty node ids",
            }
        ));
    }
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
