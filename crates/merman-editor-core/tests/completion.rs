mod support;

use merman_analysis::FenceTextIndexSource;
use merman_editor_core::{
    COMPLETION_TRIGGER_CHARACTERS, CompletionDataKind, CompletionInsertTextFormat,
    CompletionItemKind, DocumentKind, Position, completion_documentation, completion_for_snapshot,
};
use support::SnapshotHarness;

#[test]
fn completion_trigger_characters_are_owned_by_editor_policy() {
    assert_eq!(
        COMPLETION_TRIGGER_CHARACTERS,
        &[' ', '\n', '-', '>', '%', '[', '(', '{', '/', '\\', '@', ':']
    );
}

#[test]
fn completion_offers_known_node_ids_with_text_edits() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            "flowchart TD\nA-->B\nB-->C\nC-->".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let list = completion_for_snapshot(&snapshot, Position::new(3, 4));

    let item = list.items.iter().find(|item| item.label == "B").unwrap();
    let edit = item.text_edit.as_ref().unwrap();

    assert_eq!(edit.new_text, "B");
    assert_eq!(edit.range.start.line, 3);
    assert_eq!(edit.range.start.character, 4);
}

#[test]
fn completion_at_eof_preserves_unicode_ranges_across_line_endings() {
    for (label, line_ending) in [("lf", "\n"), ("crlf", "\r\n"), ("cr", "\r")] {
        let final_line = "target[\"🤓\"] -->";
        let source = ["flowchart TD", "alpha-->beta", final_line].join(line_ending);
        let harness = SnapshotHarness::new();
        let snapshot = harness
            .analyze(
                format!("file:///tmp/completion-{label}.mmd"),
                1,
                source,
                DocumentKind::Diagram,
            )
            .expect("test source should be accepted");
        let cursor = Position::new(2, final_line.encode_utf16().count());
        let completion = completion_for_snapshot(&snapshot, cursor);
        let item = completion
            .items
            .iter()
            .find(|item| item.label == "alpha")
            .unwrap_or_else(|| panic!("missing Unicode node completion for {label}"));
        let edit = item
            .text_edit
            .as_ref()
            .unwrap_or_else(|| panic!("missing Unicode node edit for {label}"));

        assert_eq!(edit.new_text, "alpha", "{label}");
        assert_eq!(edit.range.start, cursor, "{label}");
        assert_eq!(edit.range.end, cursor, "{label}");
    }
}

#[test]
fn completion_offers_node_ids_for_directive_targets() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            "flowchart TD\nA-->B\nstyle \n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let list = completion_for_snapshot(&snapshot, Position::new(2, 6));

    let item = list.items.iter().find(|item| item.label == "A").unwrap();
    let edit = item.text_edit.as_ref().unwrap();

    assert_eq!(edit.new_text, "A");
    assert_eq!(edit.range.start.line, 2);
    assert_eq!(edit.range.start.character, 6);
    assert!(list.items.iter().any(|item| item.label == "B"));
}

#[test]
fn empty_style_targets_use_typed_id_slots_across_line_endings() {
    let cases = [
        ("flowchart", &["flowchart TD", "A", "style "][..], "A"),
        ("class", &["classDiagram", "class A", "style "][..], "A"),
        ("er", &["erDiagram", "A", "style "][..], "A"),
        ("state", &["stateDiagram-v2", "state A", "style "][..], "A"),
        ("block", &["block", "A", "style "][..], "A"),
    ];

    for (line_ending_name, line_ending) in [("lf", "\n"), ("crlf", "\r\n"), ("cr", "\r")] {
        for (name, lines, expected_id) in cases {
            let harness = SnapshotHarness::new();
            let snapshot = harness
                .analyze(
                    format!("file:///tmp/{name}-{line_ending_name}-style-target.mmd"),
                    1,
                    format!("{}{line_ending}", lines.join(line_ending)),
                    DocumentKind::Diagram,
                )
                .unwrap_or_else(|error| panic!("{name} source should be accepted: {error:?}"));
            let list = completion_for_snapshot(&snapshot, Position::new(2, 6));

            assert!(
                list.items.iter().any(|item| item.label == expected_id),
                "{name} must retain its target slot before {line_ending_name}: {:?}",
                list.items
                    .iter()
                    .map(|item| item.label.as_str())
                    .collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn completion_offers_class_names_for_class_references() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            "flowchart TD\nA-->B\nclassDef hot fill:#f00\nclass A h\n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let list = completion_for_snapshot(&snapshot, Position::new(3, 9));

    let item = list.items.iter().find(|item| item.label == "hot").unwrap();
    let edit = item.text_edit.as_ref().unwrap();

    assert_eq!(item.kind, CompletionItemKind::Class);
    assert_eq!(
        item.data.as_ref().unwrap().kind,
        CompletionDataKind::ClassName
    );
    assert_eq!(edit.new_text, "hot");
    assert_eq!(edit.range.start.line, 3);
    assert_eq!(edit.range.start.character, 8);
}

#[test]
fn completion_does_not_offer_class_names_inside_node_payload() {
    let harness = SnapshotHarness::new();
    let line = "A[\"docs :::h\"]";
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            format!("flowchart TD\nclassDef hot fill:#f00\n{line}\n"),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let list = completion_for_snapshot(&snapshot, Position::new(2, line.find('h').unwrap() + 1));

    assert!(
        list.items.iter().all(|item| item
            .data
            .as_ref()
            .is_none_or(|data| data.kind != CompletionDataKind::ClassName)),
        "payload text must not offer class completions: {:?}",
        list.items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn completion_offers_style_snippets_after_style_targets() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            "flowchart TD\nA-->B\nstyle A \n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let list = completion_for_snapshot(&snapshot, Position::new(2, 8));

    let item = list
        .items
        .iter()
        .find(|item| item.label == "fill/stroke style")
        .unwrap();

    assert_eq!(item.insert_text_format, CompletionInsertTextFormat::Snippet);
    assert_eq!(item.data.as_ref().unwrap().kind, CompletionDataKind::Style);
    assert!(item.insert_text.as_ref().unwrap().contains("stroke-width"));
}

#[test]
fn completion_offers_interaction_snippets_after_click_targets() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            "flowchart TD\nA-->B\nclick A \n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let list = completion_for_snapshot(&snapshot, Position::new(2, 8));

    let item = list
        .items
        .iter()
        .find(|item| item.label == "href link action")
        .unwrap();

    assert_eq!(item.insert_text_format, CompletionInsertTextFormat::Snippet);
    assert_eq!(
        item.data.as_ref().unwrap().kind,
        CompletionDataKind::Interaction
    );
}

#[test]
fn completion_uses_typed_style_slots_across_directive_families() {
    let cases = [
        (
            "class",
            "classDiagram\nclass User\nstyle User \n",
            Position::new(2, 11),
        ),
        (
            "er",
            "erDiagram\nCUSTOMER\nstyle CUSTOMER \n",
            Position::new(2, 15),
        ),
        (
            "state",
            "stateDiagram-v2\nstate Running\nstyle Running \n",
            Position::new(2, 14),
        ),
        ("block", "block\nA\nstyle A \n", Position::new(2, 8)),
    ];

    for (name, source, position) in cases {
        let harness = SnapshotHarness::new();
        let snapshot = harness
            .analyze(
                format!("file:///tmp/{name}-style-completion.mmd"),
                1,
                source.to_string(),
                DocumentKind::Diagram,
            )
            .unwrap_or_else(|error| panic!("{name} source should be accepted: {error:?}"));
        let list = completion_for_snapshot(&snapshot, position);

        assert!(
            list.items
                .iter()
                .any(|item| item.label == "fill/stroke style"),
            "{name} must publish a typed style slot: {:?}",
            list.items
                .iter()
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn completion_uses_typed_class_name_slots_across_directive_families() {
    let cases = [
        (
            "class",
            "classDiagram\nclass User\nclassDef hot fill:#f00\ncssClass \"User\" h\n",
            Position::new(3, 17),
        ),
        (
            "er",
            "erDiagram\nCUSTOMER\nclassDef hot fill:#f00\nclass CUSTOMER h\n",
            Position::new(3, 16),
        ),
        (
            "state",
            "stateDiagram-v2\nstate Running\nclassDef hot fill:#f00\nclass Running h\n",
            Position::new(3, 15),
        ),
        (
            "block",
            "block\nA\nclassDef hot fill:#f00\nclass A h\n",
            Position::new(3, 9),
        ),
    ];

    for (name, source, position) in cases {
        let harness = SnapshotHarness::new();
        let snapshot = harness
            .analyze(
                format!("file:///tmp/{name}-class-completion.mmd"),
                1,
                source.to_string(),
                DocumentKind::Diagram,
            )
            .unwrap_or_else(|error| panic!("{name} source should be accepted: {error:?}"));
        let list = completion_for_snapshot(&snapshot, position);

        assert!(
            list.items.iter().any(|item| item.label == "hot"),
            "{name} must publish a typed class-name slot: {:?}",
            list.items
                .iter()
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn completion_uses_typed_interaction_slots_for_class_diagrams() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/class-interaction-completion.mmd",
            1,
            "classDiagram\nclass User\nclick User \n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("class source should be accepted");
    let list = completion_for_snapshot(&snapshot, Position::new(2, 11));

    assert!(
        list.items
            .iter()
            .any(|item| item.label == "href link action"),
        "class parser must publish a typed interaction slot: {:?}",
        list.items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn completion_requires_a_separator_before_post_target_slots() {
    let cases = [
        (
            "flowchart-style",
            "flowchart TD\nA\nstyle A",
            Position::new(2, 7),
            CompletionDataKind::Style,
        ),
        (
            "class-style",
            "classDiagram\nclass User\nstyle User",
            Position::new(2, 10),
            CompletionDataKind::Style,
        ),
        (
            "er-style",
            "erDiagram\nCUSTOMER\nstyle CUSTOMER",
            Position::new(2, 14),
            CompletionDataKind::Style,
        ),
        (
            "state-style",
            "stateDiagram-v2\nstate Running\nstyle Running",
            Position::new(2, 13),
            CompletionDataKind::Style,
        ),
        (
            "block-style",
            "block\nA\nstyle A",
            Position::new(2, 7),
            CompletionDataKind::Style,
        ),
        (
            "flowchart-click",
            "flowchart TD\nA\nclick A",
            Position::new(2, 7),
            CompletionDataKind::Interaction,
        ),
        (
            "class-click",
            "classDiagram\nclass User\nclick User",
            Position::new(2, 10),
            CompletionDataKind::Interaction,
        ),
        (
            "state-click",
            "stateDiagram-v2\nstate Running\nclick Running",
            Position::new(2, 13),
            CompletionDataKind::Interaction,
        ),
        (
            "class-class-name",
            "classDiagram\nclass User\nclassDef hot fill:#f00\ncssClass \"User\"",
            Position::new(3, 15),
            CompletionDataKind::ClassName,
        ),
        (
            "er-class-name",
            "erDiagram\nCUSTOMER\nclassDef hot fill:#f00\nclass CUSTOMER",
            Position::new(3, 14),
            CompletionDataKind::ClassName,
        ),
        (
            "state-class-name",
            "stateDiagram-v2\nstate Running\nclassDef hot fill:#f00\nclass Running",
            Position::new(3, 13),
            CompletionDataKind::ClassName,
        ),
        (
            "block-class-name",
            "block\nA\nclassDef hot fill:#f00\nclass A",
            Position::new(3, 7),
            CompletionDataKind::ClassName,
        ),
    ];

    for (name, source, position, forbidden_kind) in cases {
        let harness = SnapshotHarness::new();
        let snapshot = harness
            .analyze(
                format!("file:///tmp/{name}.mmd"),
                1,
                source.to_string(),
                DocumentKind::Diagram,
            )
            .unwrap_or_else(|error| panic!("{name} source should be accepted: {error:?}"));
        let list = completion_for_snapshot(&snapshot, position);

        assert!(
            list.items.iter().all(|item| item
                .data
                .as_ref()
                .is_none_or(|data| data.kind != forbidden_kind)),
            "{name} must not offer {forbidden_kind:?} without a separator: {:?}",
            list.items
                .iter()
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn completion_uses_class_name_slots_after_real_separators() {
    let cases = [
        (
            "class",
            "classDiagram\nclass User\nclassDef hot fill:#f00\ncssClass \"User\" ",
            Position::new(3, 16),
        ),
        (
            "er",
            "erDiagram\nCUSTOMER\nclassDef hot fill:#f00\nclass CUSTOMER ",
            Position::new(3, 15),
        ),
        (
            "state",
            "stateDiagram-v2\nstate Running\nclassDef hot fill:#f00\nclass Running ",
            Position::new(3, 14),
        ),
        (
            "block",
            "block\nA\nclassDef hot fill:#f00\nclass A ",
            Position::new(3, 8),
        ),
    ];

    for (name, source, position) in cases {
        let harness = SnapshotHarness::new();
        let snapshot = harness
            .analyze(
                format!("file:///tmp/{name}-class-separator.mmd"),
                1,
                source.to_string(),
                DocumentKind::Diagram,
            )
            .unwrap_or_else(|error| panic!("{name} source should be accepted: {error:?}"));
        let list = completion_for_snapshot(&snapshot, position);

        assert!(
            list.items.iter().any(|item| item.label == "hot"),
            "{name} must offer class names after a real separator: {:?}",
            list.items
                .iter()
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn typed_style_slots_precede_line_endings() {
    let cases = [
        (
            "class",
            &["classDiagram", "class User", "style User "][..],
            Position::new(2, 11),
        ),
        (
            "er",
            &["erDiagram", "CUSTOMER", "style CUSTOMER "][..],
            Position::new(2, 15),
        ),
        (
            "state",
            &["stateDiagram-v2", "state Running", "style Running "][..],
            Position::new(2, 14),
        ),
    ];

    for (line_ending_name, line_ending) in [("lf", "\n"), ("crlf", "\r\n"), ("cr", "\r")] {
        for (name, lines, position) in cases {
            let harness = SnapshotHarness::new();
            let snapshot = harness
                .analyze(
                    format!("file:///tmp/{name}-{line_ending_name}-style.mmd"),
                    1,
                    format!("{}{line_ending}", lines.join(line_ending)),
                    DocumentKind::Diagram,
                )
                .unwrap_or_else(|error| panic!("{name} source should be accepted: {error:?}"));
            let list = completion_for_snapshot(&snapshot, position);

            assert!(
                list.items.iter().any(|item| item
                    .data
                    .as_ref()
                    .is_some_and(|data| data.kind == CompletionDataKind::Style)),
                "{name} must publish its typed style slot before {line_ending_name}: {:?}",
                list.items
                    .iter()
                    .map(|item| item.label.as_str())
                    .collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn completion_recognizes_partial_and_complete_interaction_actions() {
    for (name, source, position) in [
        (
            "class-partial",
            "classDiagram\nclass User\nclick User h",
            Position::new(2, 12),
        ),
        (
            "class-complete",
            "classDiagram\nclass User\nclick User href",
            Position::new(2, 15),
        ),
        (
            "class-call-partial",
            "classDiagram\nclass User\nclick User c",
            Position::new(2, 12),
        ),
        (
            "class-call-complete",
            "classDiagram\nclass User\nclick User call",
            Position::new(2, 15),
        ),
        (
            "state-partial",
            "stateDiagram-v2\nstate Running\nclick Running h",
            Position::new(2, 15),
        ),
        (
            "state-complete",
            "stateDiagram-v2\nstate Running\nclick Running href",
            Position::new(2, 18),
        ),
    ] {
        let harness = SnapshotHarness::new();
        let snapshot = harness
            .analyze(
                format!("file:///tmp/{name}.mmd"),
                1,
                source.to_string(),
                DocumentKind::Diagram,
            )
            .unwrap_or_else(|error| panic!("{name} source should be accepted: {error:?}"));
        let list = completion_for_snapshot(&snapshot, position);

        assert!(
            list.items.iter().any(|item| item
                .data
                .as_ref()
                .is_some_and(|data| data.kind == CompletionDataKind::Interaction)),
            "{name} must retain the typed interaction-action span: {:?}",
            list.items
                .iter()
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn class_interaction_actions_replace_only_the_action_token() {
    for (name, action, end_character) in [("partial", "c", 12), ("complete", "call", 15)] {
        let source = format!("classDiagram\nclass User\nclick User {action}");
        let harness = SnapshotHarness::new();
        let snapshot = harness
            .analyze(
                format!("file:///tmp/class-call-{name}.mmd"),
                1,
                source,
                DocumentKind::Diagram,
            )
            .expect("class interaction source should be accepted");
        let list = completion_for_snapshot(&snapshot, Position::new(2, end_character));
        let item = list
            .items
            .iter()
            .find(|item| item.label == "callback action")
            .expect("callback action completion");
        let edit = item.text_edit.as_ref().expect("callback action text edit");

        assert_eq!(edit.range.start, Position::new(2, 11), "{name}");
        assert_eq!(edit.range.end, Position::new(2, end_character), "{name}");
        assert_eq!(edit.new_text, "call ${1:callback}(${2:arg})", "{name}");
    }
}

#[test]
fn class_interaction_payloads_suppress_action_completion() {
    for (name, source) in [
        (
            "href-empty-payload",
            "classDiagram\nclass User\nclick User href ",
        ),
        (
            "call-empty-payload",
            "classDiagram\nclass User\nclick User call ",
        ),
        (
            "callback-unclosed-args",
            "classDiagram\nclass User\nclick User call open(",
        ),
        (
            "callback-name",
            "classDiagram\nclass User\nclick User call open",
        ),
        (
            "callback-args",
            "classDiagram\nclass User\nclick User call open(arg)",
        ),
        (
            "href-url",
            "classDiagram\nclass User\nclick User href \"https://example.com\"",
        ),
        (
            "href-target",
            "classDiagram\nclass User\nclick User href \"https://example.com\" _blank",
        ),
        (
            "link-url",
            "classDiagram\nclass User\nlink User \"https://example.com\"",
        ),
        (
            "callback",
            "classDiagram\nclass User\ncallback User \"open\"",
        ),
    ] {
        let harness = SnapshotHarness::new();
        let snapshot = harness
            .analyze(
                format!("file:///tmp/class-{name}.mmd"),
                1,
                source.to_string(),
                DocumentKind::Diagram,
            )
            .unwrap_or_else(|error| panic!("{name} source should be accepted: {error:?}"));
        let position = snapshot
            .source_map()
            .utf16_position(source.len())
            .map(|position| Position::new(position.line, position.character))
            .unwrap();
        let list = completion_for_snapshot(&snapshot, position);

        assert!(
            list.items
                .iter()
                .all(|item| item.data.as_ref().is_none_or(|data| {
                    !matches!(
                        data.kind,
                        CompletionDataKind::Interaction | CompletionDataKind::Directive
                    )
                })),
            "{name} payload must not offer action or directive snippets: {:?}",
            list.items
                .iter()
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn flowchart_interaction_actions_replace_only_the_action_token() {
    for (name, action, end_character, label, replacement) in [
        (
            "href-partial",
            "h",
            9,
            "href link action",
            "href \"${1:https://example.com}\" \"${2:Tooltip}\" ${3|_blank,_self|}",
        ),
        (
            "href-complete",
            "href",
            12,
            "href link action",
            "href \"${1:https://example.com}\" \"${2:Tooltip}\" ${3|_blank,_self|}",
        ),
        (
            "call-partial",
            "c",
            9,
            "callback action",
            "call ${1:callback}(${2:arg})",
        ),
        (
            "call-complete",
            "call",
            12,
            "callback action",
            "call ${1:callback}(${2:arg})",
        ),
    ] {
        let source = format!("flowchart TD\nA\nclick A {action}");
        let harness = SnapshotHarness::new();
        let snapshot = harness
            .analyze(
                format!("file:///tmp/flowchart-{name}.mmd"),
                1,
                source,
                DocumentKind::Diagram,
            )
            .expect("flowchart interaction source should be accepted");
        let list = completion_for_snapshot(&snapshot, Position::new(2, end_character));
        let item = list
            .items
            .iter()
            .find(|item| item.label == label)
            .unwrap_or_else(|| panic!("missing {label} completion for {name}"));
        let edit = item.text_edit.as_ref().expect("interaction text edit");

        assert_eq!(edit.range.start, Position::new(2, 8), "{name}");
        assert_eq!(edit.range.end, Position::new(2, end_character), "{name}");
        assert_eq!(edit.new_text, replacement, "{name}");
    }
}

#[test]
fn flowchart_interaction_payloads_suppress_action_completion() {
    for (name, source) in [
        ("href-empty-payload", "flowchart TD\nA\nclick A href "),
        (
            "href-url",
            "flowchart TD\nA\nclick A href \"https://example.com\"",
        ),
        (
            "href-tooltip",
            "flowchart TD\nA\nclick A href \"https://example.com\" \"Open\"",
        ),
        (
            "href-target",
            "flowchart TD\nA\nclick A href \"https://example.com\" \"Open\" _blank",
        ),
        ("call-empty-payload", "flowchart TD\nA\nclick A call "),
        ("callback-name", "flowchart TD\nA\nclick A call open"),
        (
            "callback-unclosed-args",
            "flowchart TD\nA\nclick A call open(",
        ),
        (
            "callback-tooltip",
            "flowchart TD\nA\nclick A call open(arg) \"Open\"",
        ),
        (
            "legacy-link",
            "flowchart TD\nA\nclick A \"https://example.com\"",
        ),
        ("legacy-callback", "flowchart TD\nA\nclick A open"),
    ] {
        let harness = SnapshotHarness::new();
        let snapshot = harness
            .analyze(
                format!("file:///tmp/flowchart-{name}.mmd"),
                1,
                source.to_string(),
                DocumentKind::Diagram,
            )
            .unwrap_or_else(|error| panic!("{name} source should be accepted: {error:?}"));
        let position = snapshot
            .source_map()
            .utf16_position(source.len())
            .map(|position| Position::new(position.line, position.character))
            .unwrap();
        let list = completion_for_snapshot(&snapshot, position);

        assert!(
            list.items
                .iter()
                .all(|item| item.data.as_ref().is_none_or(|data| {
                    !matches!(
                        data.kind,
                        CompletionDataKind::Interaction | CompletionDataKind::Directive
                    )
                })),
            "{name} payload must not offer action or directive snippets: {:?}",
            list.items
                .iter()
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn empty_class_definition_names_keep_directive_helpers_across_families() {
    for (name, source) in [
        ("flowchart", "flowchart TD\nclassDef "),
        ("class", "classDiagram\nclassDef "),
        ("er", "erDiagram\nclassDef "),
        ("state", "stateDiagram-v2\nclassDef "),
        ("block", "block\nclassDef "),
    ] {
        let harness = SnapshotHarness::new();
        let snapshot = harness
            .analyze(
                format!("file:///tmp/{name}-empty-classdef.mmd"),
                1,
                source.to_string(),
                DocumentKind::Diagram,
            )
            .unwrap_or_else(|error| panic!("{name} source should be accepted: {error:?}"));
        let position = snapshot
            .source_map()
            .utf16_position(source.len())
            .map(|position| Position::new(position.line, position.character))
            .unwrap();
        let list = completion_for_snapshot(&snapshot, position);

        assert!(
            list.items.iter().any(|item| item
                .data
                .as_ref()
                .is_some_and(|data| data.kind == CompletionDataKind::Directive)),
            "{name} must keep directive helpers for an empty classDef name"
        );
        assert!(list.items.iter().all(|item| {
            item.data
                .as_ref()
                .is_none_or(|data| data.kind != CompletionDataKind::ClassName)
        }));
    }
}

#[test]
fn completion_stays_fence_local_in_markdown_documents() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.markdown",
            1,
            concat!(
                "before\n",
                "```mermaid\n",
                "flowchart TD\n",
                "A-->B\n",
                "C-->\n",
                "```\n",
                "middle\n",
                "```mermaid\n",
                "sequenceDiagram\n",
                "Alice->>Bob: Hi\n",
                "```\n",
                "after\n",
            )
            .to_string(),
            DocumentKind::Markdown,
        )
        .expect("test source should be accepted");

    let flowchart_list = completion_for_snapshot(&snapshot, Position::new(4, 4));
    assert!(flowchart_list.items.iter().any(|item| item.label == "A"));
    assert!(flowchart_list.items.iter().any(|item| item.label == "B"));
    assert!(
        flowchart_list
            .items
            .iter()
            .all(|item| item.label != "Alice" && item.label != "Bob")
    );

    let sequence_list = completion_for_snapshot(&snapshot, Position::new(9, 14));
    assert!(sequence_list.items.is_empty());
}

#[test]
fn completion_ignores_markdown_fence_delimiter_lines() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.markdown",
            1,
            concat!(
                "before\n",
                "```mermaid\n",
                "flowchart TD\n",
                "A-->B\n",
                "```\n",
                "after\n",
            )
            .to_string(),
            DocumentKind::Markdown,
        )
        .expect("test source should be accepted");

    assert!(
        completion_for_snapshot(&snapshot, Position::new(1, 3))
            .items
            .is_empty()
    );
    assert!(
        completion_for_snapshot(&snapshot, Position::new(4, 0))
            .items
            .is_empty()
    );
}

#[test]
fn completion_allows_unclosed_markdown_fence_body() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.markdown",
            1,
            concat!("```mermaid\n", "flowchart TD\n", "A-->\n").to_string(),
            DocumentKind::Markdown,
        )
        .expect("test source should be accepted");

    assert!(
        completion_for_snapshot(&snapshot, Position::new(2, 4))
            .items
            .iter()
            .any(|item| item.label == "A")
    );
}

#[test]
fn completion_uses_parser_identifier_context_after_operator() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            "flowchart TD\nA-->B\nC-->".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let list = completion_for_snapshot(&snapshot, Position::new(2, 4));

    assert!(list.items.iter().any(|item| item.label == "A"));
    assert!(list.items.iter().any(|item| item.label == "B"));
    assert!(
        list.items.iter().all(|item| item.label != "-->"),
        "parser identifier context must not offer operator completions: {:?}",
        list.items
            .iter()
            .map(|item| &item.label)
            .collect::<Vec<_>>()
    );
}

#[test]
fn non_flowchart_parser_facts_do_not_offer_flowchart_body_completions() {
    for (line, forbidden_kind) in [
        ("A--", CompletionDataKind::Operator),
        ("direction", CompletionDataKind::Direction),
        ("A@{ shape: rou", CompletionDataKind::Shape),
    ] {
        let harness = SnapshotHarness::new();
        let snapshot = harness
            .analyze(
                "file:///tmp/history.mmd",
                1,
                format!("gitGraph\n{line}"),
                DocumentKind::Diagram,
            )
            .expect("test source should be accepted");
        let list = completion_for_snapshot(&snapshot, Position::new(1, line.len()));

        assert!(
            list.fact_source
                .is_some_and(FenceTextIndexSource::is_parser_backed),
            "test requires parser-backed gitGraph facts for {line:?}"
        );
        assert!(
            list.items.iter().all(|item| item
                .data
                .as_ref()
                .is_none_or(|data| data.kind != forbidden_kind)),
            "gitGraph must not receive {forbidden_kind:?} completions for {line:?}: {:?}",
            list.items
                .iter()
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn directional_families_project_partial_values_from_recovery_facts() {
    for (source, line, line_text, expected_labels) in [
        (
            "flowchart TD\nsubgraph group\ndirection L\nend\n",
            2,
            "direction L",
            &["TB", "TD", "BT", "LR", "RL"][..],
        ),
        (
            "C4Context\ndirection L",
            1,
            "direction L",
            &["TB", "BT", "LR", "RL"],
        ),
        (
            "requirementDiagram\ndirection L",
            1,
            "direction L",
            &["TB", "BT", "LR", "RL"],
        ),
        (
            "erDiagram\ndirection L",
            1,
            "direction L",
            &["TB", "BT", "LR", "RL"],
        ),
        (
            "classDiagram\ndirection L",
            1,
            "direction L",
            &["TB", "BT", "LR", "RL"],
        ),
        (
            "stateDiagram-v2\ndirection L",
            1,
            "direction L",
            &["TB", "BT", "LR", "RL"],
        ),
    ] {
        let harness = SnapshotHarness::new();
        let snapshot = harness
            .analyze(
                "file:///tmp/example.mmd",
                1,
                source.to_string(),
                DocumentKind::Diagram,
            )
            .expect("test source should be accepted");

        let completion = completion_for_snapshot(&snapshot, Position::new(line, line_text.len()));
        assert_eq!(
            completion.fact_source,
            Some(FenceTextIndexSource::ParserRecovered),
            "partial direction must remain a strict parse failure: {source}"
        );
        let labels = completion
            .items
            .iter()
            .filter(|item| {
                item.data
                    .as_ref()
                    .is_some_and(|data| data.kind == CompletionDataKind::Direction)
            })
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>();

        assert_eq!(labels, expected_labels, "{source}");
        let edit = completion
            .items
            .iter()
            .find(|item| item.label == "LR")
            .and_then(|item| item.text_edit.as_ref())
            .expect("partial direction must produce an LR text edit");
        assert_eq!(edit.range.start.line, line, "{source}");
        assert_eq!(edit.range.start.character, "direction ".len(), "{source}");
        assert_eq!(edit.range.end.line, line, "{source}");
        assert_eq!(edit.range.end.character, line_text.len(), "{source}");
    }
}

#[test]
fn directional_families_reject_prefixed_values_from_recovery_facts() {
    for (source, line) in [
        ("flowchart TD\nsubgraph group\ndirection LRfoo\nend\n", 2),
        ("erDiagram\ndirection LRfoo", 1),
        ("classDiagram\ndirection LRfoo", 1),
    ] {
        let harness = SnapshotHarness::new();
        let snapshot = harness
            .analyze(
                "file:///tmp/example.mmd",
                1,
                source.to_string(),
                DocumentKind::Diagram,
            )
            .expect("test source should be accepted");
        let line_text = source.lines().nth(line).expect("direction line");

        let completion = completion_for_snapshot(&snapshot, Position::new(line, line_text.len()));
        assert_eq!(
            completion.fact_source,
            Some(FenceTextIndexSource::ParserRecovered),
            "{source}"
        );
        let edit = completion
            .items
            .iter()
            .find(|item| item.label == "LR")
            .and_then(|item| item.text_edit.as_ref())
            .expect("invalid direction must produce an LR text edit");

        assert_eq!(edit.range.start.line, line, "{source}");
        assert_eq!(edit.range.start.character, "direction ".len(), "{source}");
        assert_eq!(edit.range.end.line, line, "{source}");
        assert_eq!(edit.range.end.character, line_text.len(), "{source}");
        assert_eq!(edit.new_text, "LR", "{source}");
    }
}

#[test]
fn flowchart_direction_keeps_legacy_trailing_text_compatibility() {
    let source = "flowchart TD\nsubgraph group\ndirection LR trailing\nend\n";
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            source.to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");

    let completion = completion_for_snapshot(&snapshot, Position::new(2, "direction LR".len()));
    assert_eq!(
        completion.fact_source,
        Some(FenceTextIndexSource::ParserComplete)
    );
}

#[test]
fn flowchart_partial_operator_completion_comes_from_recovery_facts() {
    for source in ["flowchart TD\nA ->", "flowchart TD\nA --"] {
        let line = source.lines().last().expect("edge line");
        let harness = SnapshotHarness::new();
        let snapshot = harness
            .analyze(
                "file:///tmp/example.mmd",
                1,
                source.to_string(),
                DocumentKind::Diagram,
            )
            .expect("test source should be accepted");

        let completion = completion_for_snapshot(&snapshot, Position::new(1, line.len()));
        assert_eq!(
            completion.fact_source,
            Some(FenceTextIndexSource::ParserRecovered),
            "{source}"
        );
        let edit = completion
            .items
            .iter()
            .find(|item| item.label == "-->")
            .and_then(|item| item.text_edit.as_ref())
            .expect("partial operator must produce an arrow text edit");

        assert_eq!(edit.range.start.line, 1, "{source}");
        assert_eq!(edit.range.start.character, "A ".len(), "{source}");
        assert_eq!(edit.range.end.line, 1, "{source}");
        assert_eq!(edit.range.end.character, line.len(), "{source}");
        assert_eq!(edit.new_text, "-->", "{source}");
    }
}

#[test]
fn flowchart_partial_shape_completion_comes_from_recovery_facts() {
    for (source, line, expected_start, expected_replacement) in [
        ("flowchart TD\nA((", "A((", 1, "@{ shape: circle }"),
        (
            "flowchart TD\nA@{ shape: rou",
            "A@{ shape: rou",
            "A@{ shape: ".len(),
            "circle }",
        ),
    ] {
        let harness = SnapshotHarness::new();
        let snapshot = harness
            .analyze(
                "file:///tmp/example.mmd",
                1,
                source.to_string(),
                DocumentKind::Diagram,
            )
            .expect("test source should be accepted");

        let completion = completion_for_snapshot(&snapshot, Position::new(1, line.len()));
        assert_eq!(
            completion.fact_source,
            Some(FenceTextIndexSource::ParserRecovered),
            "{source}"
        );
        let edit = completion
            .items
            .iter()
            .find(|item| item.label == "@{ shape: circle }")
            .and_then(|item| item.text_edit.as_ref())
            .expect("partial shape must produce a text edit");

        assert_eq!(edit.range.start.line, 1, "{source}");
        assert_eq!(edit.range.start.character, expected_start, "{source}");
        assert_eq!(edit.range.end.line, 1, "{source}");
        assert_eq!(edit.range.end.character, line.len(), "{source}");
        assert_eq!(edit.new_text, expected_replacement, "{source}");
    }
}

#[test]
fn completion_after_pipe_edge_label_inserts_after_the_label() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            "flowchart TD\nA-->B\nA -->|go|".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let list = completion_for_snapshot(&snapshot, Position::new(2, 9));

    let item = list.items.iter().find(|item| item.label == "B").unwrap();
    let edit = item.text_edit.as_ref().unwrap();

    assert_eq!(edit.new_text, "B");
    assert_eq!(edit.range.start.line, 2);
    assert_eq!(edit.range.start.character, 9);
    assert_eq!(edit.range.end, edit.range.start);
}

#[test]
fn completion_after_pipe_edge_label_replaces_trailing_whitespace_slot() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            "flowchart TD\nA-->B\nA -->|go|   ".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let list = completion_for_snapshot(&snapshot, Position::new(2, 12));

    let item = list.items.iter().find(|item| item.label == "B").unwrap();
    let edit = item.text_edit.as_ref().unwrap();

    assert_eq!(edit.new_text, "B");
    assert_eq!(edit.range.start.line, 2);
    assert_eq!(edit.range.start.character, 9);
    assert_eq!(edit.range.end.line, 2);
    assert_eq!(edit.range.end.character, 12);
}

#[test]
fn completion_keeps_known_node_ids_when_parser_recovers() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            "flowchart TD\nsubgraph group\nA-->B\nC-->".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let list = completion_for_snapshot(&snapshot, Position::new(3, 4));

    assert_eq!(
        list.fact_source,
        Some(FenceTextIndexSource::ParserRecovered)
    );
    assert!(
        list.items.iter().any(|item| item.label == "A"),
        "recovered parser context should still offer existing node ids"
    );
    assert!(
        list.items.iter().any(|item| item.label == "B"),
        "recovered parser context should still offer existing node ids"
    );
}

#[test]
fn completion_payload_contexts_return_no_body_items() {
    for (source, position, label) in [
        (
            concat!("stateDiagram-v2\n", "state \"Small State\" as namedState\n"),
            Position::new(1, 8),
            "state",
        ),
        (
            "sequenceDiagram\nparticipant Alice\nAlice->Bob: Hello",
            Position::new(2, 14),
            "sequence",
        ),
        ("gantt\ntitle Roadmap", Position::new(1, 10), "gantt"),
        ("mindmap\nroot(Root Node)\n", Position::new(1, 8), "mindmap"),
        (
            "flowchart TD\nA[\"Start node\"]-->B",
            Position::new(1, 5),
            "flowchart",
        ),
        (
            "block\nA[\"Start node\"] --> B\n",
            Position::new(1, 5),
            "block",
        ),
        (
            "classDiagram\nclass User\nstyle User fill:#f00\n",
            Position::new(2, 14),
            "class style",
        ),
        (
            "classDiagram\nclassDef hot fill:#f00\n",
            Position::new(1, 16),
            "class classDef",
        ),
        (
            "erDiagram\nCUSTOMER\nstyle CUSTOMER fill:#f00\n",
            Position::new(2, 18),
            "er style",
        ),
        (
            "erDiagram\nclassDef hot fill:#f00\n",
            Position::new(1, 16),
            "er classDef",
        ),
        (
            "stateDiagram-v2\nstate Running\nstyle Running fill:#f00\n",
            Position::new(2, 17),
            "state style",
        ),
        (
            "stateDiagram-v2\nclassDef hot fill:#f00\n",
            Position::new(1, 16),
            "state classDef",
        ),
        (
            "block\nA\nstyle A fill:#f00\n",
            Position::new(2, 11),
            "block style",
        ),
        (
            "block\nclassDef hot fill:#f00\n",
            Position::new(1, 16),
            "block classDef",
        ),
    ] {
        let harness = SnapshotHarness::new();
        let snapshot = harness
            .analyze(
                "file:///tmp/example.mmd",
                1,
                source.to_string(),
                DocumentKind::Diagram,
            )
            .expect("test source should be accepted");
        let list = completion_for_snapshot(&snapshot, position);

        assert!(
            list.items.is_empty(),
            "{label} payload context must not offer generic identifiers or headers: {:?}",
            list.items
                .iter()
                .map(|item| &item.label)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn completion_bounds_unavailable_facts_to_source_start() {
    let harness = SnapshotHarness::new();
    let source_start = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            "flow".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let list = completion_for_snapshot(&source_start, Position::new(0, 4));

    assert_eq!(list.fact_source, Some(FenceTextIndexSource::Unavailable));
    assert!(list.items.iter().any(|item| {
        item.data
            .as_ref()
            .is_some_and(|data| data.kind == CompletionDataKind::DiagramHeader)
    }));
    assert!(list.items.iter().any(|item| {
        item.data
            .as_ref()
            .is_some_and(|data| data.kind == CompletionDataKind::Template)
    }));

    let body = harness
        .analyze(
            "file:///tmp/unknown.mmd",
            1,
            "unknownDiagram\nA-".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let list = completion_for_snapshot(&body, Position::new(1, 2));

    assert_eq!(list.fact_source, Some(FenceTextIndexSource::Unavailable));
    assert!(list.items.is_empty());
}

#[test]
fn completion_uses_parser_expected_syntax_for_shape_values() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            "flowchart TD\nA@{\n  shape: rou\n}\n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let list = completion_for_snapshot(&snapshot, Position::new(2, 11));
    let edit = list
        .items
        .iter()
        .find(|item| item.label == "@{ shape: circle }")
        .and_then(|item| item.text_edit.as_ref())
        .expect("circle shape edit");

    assert_eq!(edit.range.start.line, 2);
    assert_eq!(edit.range.start.character, 9);
    assert_eq!(edit.new_text, "circle");
}

#[test]
fn shape_value_completion_does_not_duplicate_existing_closing_brace() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            "flowchart TD\nA@{ shape: rou }\n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let list = completion_for_snapshot(&snapshot, Position::new(1, 14));
    let edit = list
        .items
        .iter()
        .find(|item| item.label == "@{ shape: circle }")
        .and_then(|item| item.text_edit.as_ref())
        .expect("circle shape edit");

    assert_eq!(edit.range.start.line, 1);
    assert_eq!(edit.range.start.character, 11);
    assert_eq!(edit.range.end.line, 1);
    assert_eq!(edit.range.end.character, 14);
    assert_eq!(edit.new_text, "circle");
}

#[test]
fn shape_value_completion_accepts_mermaid_whitespace_variants() {
    for source in [
        "flowchart TD\nA@{shape: rou }\n",
        "flowchart TD\nA@{       shape: rou }\n",
        "flowchart TD\nA@{ shape : rou }\n",
    ] {
        let harness = SnapshotHarness::new();
        let snapshot = harness
            .analyze(
                "file:///tmp/example.mmd",
                1,
                source.to_string(),
                DocumentKind::Diagram,
            )
            .expect("test source should be accepted");
        let cursor = source.find("rou").unwrap() + "rou".len() - "flowchart TD\n".len();
        let list = completion_for_snapshot(&snapshot, Position::new(1, cursor));
        let edit = list
            .items
            .iter()
            .find(|item| item.label == "@{ shape: circle }")
            .and_then(|item| item.text_edit.as_ref())
            .expect("circle shape edit");

        assert_eq!(edit.range.start.line, 1);
        assert_eq!(
            edit.range.start.character,
            source.find("rou").unwrap() - "flowchart TD\n".len()
        );
        assert_eq!(edit.range.end.line, 1);
        assert_eq!(edit.range.end.character, cursor);
        assert_eq!(edit.new_text, "circle");
    }
}

#[test]
fn shape_value_completion_appends_missing_brace_before_markdown_fence_close() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.markdown",
            1,
            concat!(
                "before\n",
                "```mermaid\n",
                "flowchart TD\n",
                "A@{ shape: rou\n",
                "```\n",
                "after\n",
            )
            .to_string(),
            DocumentKind::Markdown,
        )
        .expect("test source should be accepted");
    let list = completion_for_snapshot(&snapshot, Position::new(3, 14));
    let edit = list
        .items
        .iter()
        .find(|item| item.label == "@{ shape: circle }")
        .and_then(|item| item.text_edit.as_ref())
        .expect("circle shape edit");

    assert_eq!(edit.range.start.line, 3);
    assert_eq!(edit.range.start.character, 11);
    assert_eq!(edit.range.end.line, 3);
    assert_eq!(edit.range.end.character, 14);
    assert_eq!(edit.new_text, "circle }");
}

#[test]
fn shape_value_completion_ignores_host_document_tail_after_markdown_fence() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.markdown",
            1,
            concat!(
                "before\n",
                "```mermaid\n",
                "flowchart TD\n",
                "A@{ shape: rou\n",
                "```\n",
                "host markdown } should not close the active shape\n",
            )
            .to_string(),
            DocumentKind::Markdown,
        )
        .expect("test source should be accepted");
    let list = completion_for_snapshot(&snapshot, Position::new(3, 14));
    let edit = list
        .items
        .iter()
        .find(|item| item.label == "@{ shape: circle }")
        .and_then(|item| item.text_edit.as_ref())
        .expect("circle shape edit");

    assert_eq!(edit.new_text, "circle }");
}

#[test]
fn shape_value_completion_appends_missing_brace_before_next_diagram_statement() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            "flowchart TD\nA@{ shape: rou\nB --> C\n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let list = completion_for_snapshot(&snapshot, Position::new(1, 14));
    let edit = list
        .items
        .iter()
        .find(|item| item.label == "@{ shape: circle }")
        .and_then(|item| item.text_edit.as_ref())
        .expect("circle shape edit");

    assert_eq!(edit.range.start.line, 1);
    assert_eq!(edit.range.start.character, 11);
    assert_eq!(edit.range.end.line, 1);
    assert_eq!(edit.range.end.character, 14);
    assert_eq!(edit.new_text, "circle }");
}

#[test]
fn completion_offers_parser_accepted_flowchart_shapes() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            "flowchart TD\nA@{\n  shape: rou\n}\n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let list = completion_for_snapshot(&snapshot, Position::new(2, 11));
    let labels = list
        .items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(
        labels.contains(&"@{ shape: inv-trapezoid }"),
        "labels: {labels:?}"
    );
    assert!(
        labels.contains(&"@{ shape: notched-rectangle }"),
        "labels: {labels:?}"
    );
    assert!(
        !labels.contains(&"@{ shape: inv_trapezoid }"),
        "labels: {labels:?}"
    );
}

#[test]
fn completion_resolve_documentation_is_protocol_neutral() {
    let documentation = completion_documentation(&merman_editor_core::CompletionResolveData {
        kind: CompletionDataKind::DiagramHeader,
        label: "flowchart TD".to_string(),
    });

    assert!(documentation.contains("Starts a Mermaid"));
    assert!(documentation.contains("flowchart TD"));
}

#[test]
fn completion_offers_snippet_templates_at_diagram_start() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            "flow".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let list = completion_for_snapshot(&snapshot, Position::new(0, 4));

    let item = list
        .items
        .iter()
        .find(|item| item.label == "flowchart template")
        .expect("flowchart template completion");

    assert_eq!(item.insert_text_format, CompletionInsertTextFormat::Snippet);
    assert!(
        item.insert_text
            .as_ref()
            .unwrap()
            .contains("${1|TD,TB,BT,LR,RL|}")
    );
    assert_eq!(
        item.data.as_ref().unwrap().kind,
        CompletionDataKind::Template
    );
}

#[test]
fn completion_offers_icon_template_from_icon_prefix() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            "icon".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let list = completion_for_snapshot(&snapshot, Position::new(0, 4));

    let item = list
        .items
        .iter()
        .find(|item| item.label == "icon node template")
        .expect("icon node template completion");

    assert_eq!(item.insert_text_format, CompletionInsertTextFormat::Snippet);
}

#[test]
fn completion_offers_frontmatter_templates_at_document_start() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            String::new(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let list = completion_for_snapshot(&snapshot, Position::new(0, 0));

    let item = list
        .items
        .iter()
        .find(|item| item.label == "frontmatter config template")
        .expect("frontmatter template completion");

    assert_eq!(item.insert_text_format, CompletionInsertTextFormat::Snippet);
    assert!(item.insert_text.as_ref().unwrap().contains("config:"));
}

#[test]
fn completion_offers_frontmatter_at_the_start_of_a_complete_diagram() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/frontmatter-at-complete-source-start.mmd",
            1,
            "flowchart TD\nA-->B\n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let list = completion_for_snapshot(&snapshot, Position::new(0, 0));

    assert!(list.items.iter().any(|item| {
        item.data
            .as_ref()
            .is_some_and(|data| data.kind == CompletionDataKind::Frontmatter)
    }));
}

#[test]
fn incomplete_frontmatter_opening_keeps_authoring_completion_across_line_endings() {
    for (ending_name, ending) in [("lf", "\n"), ("crlf", "\r\n"), ("cr", "\r")] {
        let source = format!("---{ending}");
        let harness = SnapshotHarness::new();
        let snapshot = harness
            .analyze(
                format!("file:///tmp/frontmatter-opening-{ending_name}.mmd"),
                1,
                source,
                DocumentKind::Diagram,
            )
            .expect("incomplete frontmatter source should remain analyzable");
        let list = completion_for_snapshot(&snapshot, Position::new(1, 0));

        assert!(
            list.items.iter().any(|item| {
                item.data
                    .as_ref()
                    .is_some_and(|data| data.kind == CompletionDataKind::Frontmatter)
            }),
            "{ending_name} opening should retain frontmatter completion"
        );
    }
}

#[test]
fn completion_offers_themecss_inside_frontmatter() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            "---\nconfig:\n  theme\n---\nflowchart TD\nA-->B\n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let list = completion_for_snapshot(&snapshot, Position::new(2, 7));

    let item = list
        .items
        .iter()
        .find(|item| item.label == "themeCSS: |")
        .expect("themeCSS frontmatter completion");

    assert_eq!(item.insert_text_format, CompletionInsertTextFormat::Snippet);
    assert_eq!(
        item.data.as_ref().unwrap().kind,
        CompletionDataKind::Frontmatter
    );
}

#[test]
fn completion_does_not_offer_frontmatter_items_in_diagram_body() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            "flowchart TD\n  theme".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let list = completion_for_snapshot(&snapshot, Position::new(1, 7));

    assert!(!list.items.iter().any(|item| {
        item.data
            .as_ref()
            .is_some_and(|data| data.kind == CompletionDataKind::Frontmatter)
    }));
}

#[test]
fn global_completion_evidence_stops_before_physical_line_endings() {
    for (ending_name, ending) in [("lf", "\n"), ("crlf", "\r\n"), ("cr", "\r")] {
        let frontmatter = [
            "---",
            "config:",
            "  theme: dark",
            "---",
            "flowchart TD",
            "A",
        ]
        .join(ending);
        let comment = ["flowchart TD", "%% comment", "A"].join(ending);

        for (kind, source, position) in [
            ("frontmatter", frontmatter, Position::new(4, 0)),
            ("comment", comment, Position::new(2, 0)),
        ] {
            let harness = SnapshotHarness::new();
            let snapshot = harness
                .analyze(
                    format!("file:///tmp/{kind}-{ending_name}-boundary.mmd"),
                    1,
                    source,
                    DocumentKind::Diagram,
                )
                .unwrap_or_else(|error| {
                    panic!("{kind} source with {ending_name} should be accepted: {error:?}")
                });
            let list = completion_for_snapshot(&snapshot, position);

            assert!(
                list.items
                    .iter()
                    .all(|item| item.data.as_ref().is_none_or(|data| {
                        !matches!(
                            data.kind,
                            CompletionDataKind::Frontmatter | CompletionDataKind::Directive
                        )
                    })),
                "{kind} evidence must stop before the {ending_name} line ending: {:?}",
                list.items
                    .iter()
                    .map(|item| item.label.as_str())
                    .collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn completion_uses_core_frontmatter_semantics_for_indented_frontmatter() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            "  ---\n  config:\n    theme\n  ---\nflowchart TD\nA-->B\n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let list = completion_for_snapshot(&snapshot, Position::new(2, 9));

    let item = list
        .items
        .iter()
        .find(|item| item.label == "themeCSS: |")
        .expect("themeCSS frontmatter completion");

    assert_eq!(item.insert_text_format, CompletionInsertTextFormat::Snippet);
    assert_eq!(
        item.data.as_ref().unwrap().kind,
        CompletionDataKind::Frontmatter
    );
}

#[test]
fn directive_helpers_use_snippet_placeholders() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            "flowchart TD\nclassDef ".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let list = completion_for_snapshot(&snapshot, Position::new(1, 9));

    let item = list
        .items
        .iter()
        .find(|item| item.label == ":::className")
        .expect("class helper completion");

    assert_eq!(item.insert_text_format, CompletionInsertTextFormat::Snippet);
    assert_eq!(item.insert_text.as_deref(), Some(":::${1:className}"));
}
