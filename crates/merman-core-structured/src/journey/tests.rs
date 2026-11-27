use super::*;

#[cfg(feature = "editor-facts")]
#[test]
fn journey_recovery_retains_first_issue_and_observes_the_unclosed_block() {
    let source = "broken\r\njourney\r\n  Pay:4:Actor\r\naccDescr{";
    let opening = source.find('{').unwrap();
    for capture_facts in [false, true] {
        let mut observations = Vec::new();
        let result = parse_journey_source_with_observer_controlled(
            source,
            capture_facts,
            &mut |observation| observations.push(observation),
            &OperationControl::new(),
        )
        .unwrap();
        assert_eq!(
            result.unwrap_err(),
            JourneySourceIssue::ExpectedHeader(SourceSpan::new(0, "broken".len()))
        );
        assert_eq!(
            observations.pop(),
            Some(JourneyObservation::UnterminatedAccDescr(SourceSpan::new(
                opening,
                source.len(),
            )))
        );
        let observed_text: Vec<_> = observations
            .into_iter()
            .map(|observation| {
                let JourneyObservation::Text {
                    kind,
                    text,
                    selection,
                    ..
                } = observation
                else {
                    panic!("expected one block recovery observation");
                };
                assert_eq!(&source[selection.start..selection.end], text);
                (kind, text)
            })
            .collect();
        if capture_facts {
            assert_eq!(
                observed_text,
                [
                    (JourneyTextKind::Task, "Pay"),
                    (JourneyTextKind::Score, "4"),
                    (JourneyTextKind::People, "Actor"),
                    (JourneyTextKind::AccDescrBlock, ""),
                ]
            );
        } else {
            assert!(observed_text.is_empty());
        }
    }
}

#[test]
fn journey_scores_retain_fractions_and_existing_non_finite_state() {
    let mut db = JourneyDb::default();
    for (literal, expected, expected_nan) in [
        ("3.5", 3.5, false),
        ("3.01", 3.01, false),
        ("2.75", 2.75, false),
        ("3", 3.0, false),
        ("4", 4.0, false),
        ("0x4", 4.0, false),
        ("0B11", 3.0, false),
        ("0o5", 5.0, false),
        ("0x000000000000000000000000000000004", 4.0, false),
        (
            "0o2000000000000000000001",
            18_446_744_073_709_551_616.0,
            false,
        ),
        (
            "0b10000000000000000000000000000000000000000000000000000101",
            36_028_797_018_963_976.0,
            false,
        ),
        ("\u{feff}3.5\u{feff}", 3.5, false),
        ("\u{0085}3.5\u{0085}", 0.0, true),
        ("0x", 0.0, true),
        ("0b2", 0.0, true),
        ("0o8", 0.0, true),
        ("+0x4", 0.0, true),
        ("-0x4", 0.0, true),
        ("0x10000000000000800", 18_446_744_073_709_551_616.0, false),
        ("0x10000000000000801", 18_446_744_073_709_555_712.0, false),
        ("0x10000000000001800", 18_446_744_073_709_559_808.0, false),
        (" ", 0.0, false),
        ("bad", 0.0, true),
        ("NaN", 0.0, true),
        ("Infinity", f64::INFINITY, false),
        ("-Infinity", f64::NEG_INFINITY, false),
        ("1e999", f64::INFINITY, false),
    ] {
        db.add_task("Pay", &format!(":{literal}: Customer"));
        let task = db.tasks.last().unwrap();
        assert_eq!(task.score, expected, "{literal}");
        assert_eq!(task.score_is_nan, expected_nan, "{literal}");
        assert_eq!(task.people, ["Customer"]);
    }
}

#[test]
fn journey_actor_deduplication_uses_ecmascript_utf16_sort_order() {
    let mut unicode = JourneyDb::default();
    unicode.add_task("Pay", ":4: 😀, \u{e000}, 😀");
    assert_eq!(unicode.actors_sorted(), ["😀", "\u{e000}"]);
    assert_eq!(unicode.tasks[0].people, ["😀", "\u{e000}", "😀"]);

    let mut ascii = JourneyDb::default();
    ascii.add_task("Pay", ":4: A, Z");
    assert_eq!(ascii.actors_sorted(), ["A", "Z"]);
    assert_eq!(ascii.tasks[0].people, ["A", "Z"]);
}

#[test]
fn journey_actor_trimming_uses_ecmascript_whitespace() {
    let mut db = JourneyDb::default();
    db.add_task(
        "Pay",
        ":4: \u{feff}Alice\u{feff}, \u{0085}Bob\u{0085}, Alice",
    );
    assert_eq!(
        db.tasks[0].people,
        ["Alice", "\u{0085}Bob\u{0085}", "Alice"]
    );
    assert_eq!(db.actors_sorted(), ["Alice", "\u{0085}Bob\u{0085}"]);
}

#[test]
fn journey_raw_keyword_scan_distinguishes_crlf_and_preserves_token_priority() {
    let control = OperationControl::new();
    let title = parse_keyword_arg_one_ws("title\nValue:4:Actor", 0, "title", &control)
        .unwrap()
        .unwrap();
    assert_eq!(title.text, "Value:4:Actor");
    assert_eq!(
        title.statement,
        SourceSpan::new(0, "title\nValue:4:Actor".len())
    );
    for source in ["title\r\nValue", "title \nValue", "title\n\nValue"] {
        assert!(
            parse_keyword_arg_one_ws(source, 0, "title", &control)
                .unwrap()
                .is_none()
        );
    }
    assert!(
        parse_keyword_arg_one_ws("section :4", 0, "section", &control)
            .unwrap()
            .is_none()
    );
    let section = parse_keyword_arg_one_ws("section\nValue:4", 0, "section", &control)
        .unwrap()
        .unwrap();
    assert_eq!(section.text, "Value");
    assert_eq!(section.statement.end, "section\nValue".len());
}

#[test]
fn journey_multiline_scans_observe_cancellation_within_large_tokens() {
    let control = OperationControl::new();
    control.cancel_after_checkpoints(2);
    let scalar = format!("accTitle{}:Value", "\u{feff}\n".repeat(2048));
    assert!(parse_key_colon_value(&scalar, 0, "accTitle", &control).is_err());

    let control = OperationControl::new();
    control.cancel_after_checkpoints(3);
    let block = format!("accDescr{{{}}}", "\u{0085}".repeat(4096));
    assert!(parse_acc_descr_block_spanned(&mut LineCursor::new(&block), 0, &control).is_err());
}

#[test]
fn journey_db_tasks_and_actors_should_be_added_matches_upstream_spec() {
    let mut db = JourneyDb {
        acc_title: "Shopping".to_string(),
        acc_descr: "A user journey for family shopping".to_string(),
        ..JourneyDb::default()
    };
    db.add_section("Journey to the shops");
    db.add_task("Get car keys", ":5:Dad");
    db.add_task("Go to car", ":3:Dad, Mum, Child#1, Child#2");
    db.add_task("Drive to supermarket", ":4:Dad");
    db.add_section("Do shopping");
    db.add_task("Go shopping", ":5:Mum");

    let actors = db.actors_sorted();
    let tasks: Vec<_> = db
        .tasks
        .iter()
        .map(|task| {
            let people: Vec<_> = task.people.iter().map(String::as_str).collect();
            (
                task.score,
                people,
                task.section.as_str(),
                task.task.as_str(),
                task.task_type.as_str(),
            )
        })
        .collect();
    assert_eq!(
        tasks,
        [
            (
                5.0,
                vec!["Dad"],
                "Journey to the shops",
                "Get car keys",
                "Journey to the shops"
            ),
            (
                3.0,
                vec!["Dad", "Mum", "Child#1", "Child#2"],
                "Journey to the shops",
                "Go to car",
                "Journey to the shops"
            ),
            (
                4.0,
                vec!["Dad"],
                "Journey to the shops",
                "Drive to supermarket",
                "Journey to the shops"
            ),
            (
                5.0,
                vec!["Mum"],
                "Do shopping",
                "Go shopping",
                "Do shopping"
            ),
        ]
    );

    assert_eq!(
        actors,
        vec![
            "Child#1".to_string(),
            "Child#2".to_string(),
            "Dad".to_string(),
            "Mum".to_string()
        ]
    );
    assert_eq!(
        db.sections,
        vec![
            "Journey to the shops".to_string(),
            "Do shopping".to_string()
        ]
    );
}
