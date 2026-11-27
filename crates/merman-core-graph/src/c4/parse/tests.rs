use super::*;
use merman_model::{CancelReason, OperationPhase};

fn recovered_model_and_errors(code: &str) -> (C4DiagramModel, Vec<String>) {
    let outcome = construct_c4_source_controlled(
        code,
        &MermaidConfig::empty_object(),
        #[cfg(feature = "editor-facts")]
        false,
        #[cfg(feature = "editor-facts")]
        None,
        &OperationControl::new(),
    )
    .unwrap();
    let errors = outcome
        .issues
        .into_iter()
        .map(|issue| issue.diagnostic.message)
        .collect();
    (outcome.db.into_model().unwrap(), errors)
}

#[cfg(feature = "editor-facts")]
fn collect_symbol_name(names: &mut Vec<String>, observation: C4Observation<'_>) {
    if let C4Observation::Entity { text, .. } | C4Observation::Payload { text, .. } = observation
        && !text.is_empty()
    {
        names.push(text.to_string());
    }
}

fn boundary_chain(depth: usize) -> String {
    let mut source = String::from("C4Context\n");
    for level in 0..depth {
        source.push_str(&format!("Boundary(b{level}, \"B{level}\") {{\n"));
    }
    source.push_str("System(leaf, \"Leaf\")\n");
    for _ in 0..depth {
        source.push_str("}\n");
    }
    source
}

#[test]
fn leading_whitespace_len_tracks_utf8_width() {
    assert_eq!(leading_whitespace_len(" \t\u{3000}root"), 5);
}

#[test]
fn c4_quoted_trim_matches_javascript_and_keeps_the_authored_body_range() {
    for (body, expected) in [
        (" \t\u{feff}Alice\nSmith\u{feff}\t ", "Alice\nSmith"),
        ("\u{0085}Alice\u{0085}", "\u{0085}Alice\u{0085}"),
        (" \t\n\u{feff}", ""),
    ] {
        for prefix in ["", "$tags="] {
            let input = format!("{prefix}\"{body}\"");
            let arg = parse_arg_spanned(&input, 10).unwrap();
            let value = C4Arg::from_spanned(&arg);
            assert_eq!(value.value(), expected);
            let start = 10 + prefix.len() + 1;
            assert_eq!(arg.span(), SourceSpan::new(start, start + body.len()));
        }
    }
}

#[test]
fn c4_unterminated_quoted_macro_lookahead_keeps_later_safe_statements() {
    let source = concat!(
        "C4Context\r\n",
        "Person(broken, \"Unclosed\r\n",
        "System(after, \"Later\")\r\n",
        "Rel(after, )\r\n",
        "Person(next, \"Next\r\nLine\")\r\n",
    );
    reset_c4_syntax_construction_count();
    #[cfg(feature = "editor-facts")]
    let mut names = Vec::new();
    let outcome = construct_c4_source_controlled(
        source,
        &MermaidConfig::empty_object(),
        #[cfg(feature = "editor-facts")]
        true,
        #[cfg(feature = "editor-facts")]
        Some(&mut |event| collect_symbol_name(&mut names, event)),
        &OperationControl::new(),
    )
    .unwrap();
    assert_eq!(c4_syntax_construction_count(), 1);
    assert_eq!(outcome.issues.len(), 2);
    assert!(
        outcome.issues[0]
            .diagnostic
            .message
            .contains("unterminated macro call")
    );
    assert_eq!(
        outcome.issues[0].recovery_span,
        SourceSpan::new("C4Context\r\n".len(), source.find("\r\nSystem").unwrap())
    );
    assert!(
        outcome.issues[1]
            .diagnostic
            .message
            .contains("missing relation target")
    );
    let model = outcome.db.into_model().unwrap();
    assert_eq!(model.shapes.len(), 2);
    assert_eq!(model.shapes[0].alias, "after");
    assert_eq!(model.shapes[1].label.as_str(), "Next\r\nLine");
    #[cfg(feature = "editor-facts")]
    {
        for name in ["after", "Later", "next", "Next\r\nLine"] {
            assert!(names.iter().any(|symbol| symbol == name));
        }
        assert!(!names.iter().any(|symbol| symbol == "broken"));
    }
}

#[test]
fn c4_macro_scanning_observes_cancellation_inside_long_quoted_tokens() {
    for body in ["row\n".repeat(32), "x".repeat(16 * 1024)] {
        let source = format!("Person(a, \"{body}\")\n");
        let control = OperationControl::new().for_phase(OperationPhase::Parse);
        control.cancel_after_checkpoints(1);
        let cancelled = c4_macro_statement_source(&source, 0, &control).unwrap_err();
        assert_eq!(cancelled.reason, CancelReason::Requested);
        assert_eq!(cancelled.phase, OperationPhase::Parse);
    }
}

#[test]
fn c4_cancellation_during_boundary_closure_releases_partial_fragments() {
    std::thread::Builder::new()
        .name("c4-deep-closure-cancellation".into())
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            const DEPTH: usize = 15_000;
            let source = boundary_chain(DEPTH);
            let control = OperationControl::new().for_phase(OperationPhase::Parse);
            // Construction entry/header use three checkpoints; each macro uses two. Cancel
            // halfway through the closers, after completed inner boundaries belong to an
            // open frame. Nextest isolates this named unit in its own OS subprocess.
            control.cancel_after_checkpoints(3 + 2 * (DEPTH + 1) + DEPTH / 2);

            let cancelled = match parse_c4_model_controlled(
                &source,
                &MermaidConfig::empty_object(),
                &control,
            ) {
                Ok(_) => panic!("closure construction must observe cancellation"),
                Err(cancelled) => cancelled,
            };
            assert_eq!(cancelled.reason, CancelReason::Requested);
            assert_eq!(cancelled.phase, OperationPhase::Parse);
            assert_eq!(control.checkpoint().unwrap_err(), cancelled);
            assert!(
                parse_c4_model_controlled(
                    "C4Context\nSystem(after, \"After\")\n",
                    &MermaidConfig::empty_object(),
                    &OperationControl::new()
                )
                .unwrap()
                .is_ok()
            );
            println!(
                "depth={DEPTH} worker_stack_bytes=2097152 phase=after-small-operation status=done"
            );
        })
        .expect("spawn 2 MiB C4 closure-cancellation worker")
        .join()
        .expect("C4 closure cancellation releases completed inner frames");
}

#[test]
fn c4_cancellation_during_boundary_replay_releases_partial_fragments() {
    std::thread::Builder::new()
        .name("c4-deep-replay-cancellation".into())
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            const DEPTH: usize = 15_000;
            let source = boundary_chain(DEPTH);
            let control = OperationControl::new().for_phase(OperationPhase::Parse);
            // Consume construction and all closers, then enter half the nested replay
            // frames. Nextest isolates this named unit in its own OS subprocess.
            control.cancel_after_checkpoints(3 + 2 * (DEPTH + 1) + DEPTH + DEPTH / 2);

            let cancelled = match parse_c4_model_controlled(
                &source,
                &MermaidConfig::empty_object(),
                &control,
            ) {
                Ok(_) => panic!("boundary replay must observe cancellation"),
                Err(cancelled) => cancelled,
            };
            assert_eq!(cancelled.reason, CancelReason::Requested);
            assert_eq!(cancelled.phase, OperationPhase::Parse);
            assert_eq!(control.checkpoint().unwrap_err(), cancelled);
            assert!(
                parse_c4_model_controlled(
                    "C4Context\nSystem(after, \"After\")\n",
                    &MermaidConfig::empty_object(),
                    &OperationControl::new()
                )
                .unwrap()
                .is_ok()
            );
            println!(
                "depth={DEPTH} worker_stack_bytes=2097152 phase=after-small-operation status=done"
            );
        })
        .expect("spawn 2 MiB C4 replay-cancellation worker")
        .join()
        .expect("C4 replay cancellation releases completed boundary records");
}

#[test]
fn c4_completed_inner_boundary_is_discarded_with_unclosed_outer() {
    let source = r#"C4Context
title Root title
System(before, "Before")
Boundary(open, "Open") {
  Boundary(completed, "Completed") {
    System(inside, "Inside")
  }
  title Unfinished title
"#;
    #[cfg(feature = "editor-facts")]
    let mut names = Vec::new();
    let outcome = construct_c4_source_controlled(
        source,
        &MermaidConfig::empty_object(),
        #[cfg(feature = "editor-facts")]
        true,
        #[cfg(feature = "editor-facts")]
        Some(&mut |event| collect_symbol_name(&mut names, event)),
        &OperationControl::new(),
    )
    .unwrap();
    assert_eq!(outcome.issues.len(), 1);
    assert_eq!(
        outcome.issues[0].recovery_span,
        SourceSpan::new(source.len(), source.len())
    );
    assert!(
        outcome.issues[0]
            .diagnostic
            .message
            .contains("expected '}' before end")
    );
    let model = outcome.db.into_model().expect("safe recovered model");
    assert_eq!(model.boundaries.len(), 1);
    assert_eq!(model.shapes.len(), 1);
    assert_eq!(model.shapes[0].alias, "before");
    assert_eq!(model.title.as_deref(), Some("Root title"));
    #[cfg(feature = "editor-facts")]
    assert!(names.iter().any(|symbol| symbol == "inside"));
}

#[test]
fn c4_empty_nested_boundary_does_not_publish_or_change_sibling_context() {
    let (model, errors) = recovered_model_and_errors(
        r#"C4Context
Boundary(outer, "Outer") {
  System(before, "Before")
  Boundary(empty, "Empty") {
  }
  Boundary(sibling, "Sibling") {
    System(inside, "Inside")
  }
  System(after, "After")
}
System(root_after, "Root After")
"#,
    );
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("boundary must contain"));
    assert_eq!(model.boundaries.len(), 3);
    assert_eq!(model.boundaries[2].alias, "sibling");
    assert_eq!(model.boundaries[2].parent_boundary, "outer");
    for (index, parent) in ["outer", "sibling", "outer", "global"].iter().enumerate() {
        assert_eq!(model.shapes[index].parent_boundary, *parent);
    }
}

#[test]
fn c4_non_boundary_macro_with_brace_is_excluded_from_replay() {
    let source = r#"C4Context
Person(p, "P") {
"#;
    let (model, errors) = recovered_model_and_errors(source);

    assert!(model.shapes.is_empty());
    assert!(
        errors
            .iter()
            .any(|error| error.contains("unexpected '{' after non-boundary"))
    );
}

#[test]
fn c4_boundary_must_start_with_a_diagram_statement() {
    let (model, errors) = recovered_model_and_errors(
        r#"C4Context
Boundary(b, "B") {
  title Invalid inside boundary
  Person(p, "P")
}
"#,
    );

    assert_eq!(model.boundaries.len(), 1);
    assert!(model.shapes.is_empty());
    assert!(model.title.is_none());
    assert!(
        errors
            .iter()
            .any(|error| error.contains("must start with a diagram statement"))
    );
}

#[test]
fn c4_boundary_without_validated_brace_does_not_mutate_db_or_parent() {
    let (model, errors) = recovered_model_and_errors(
        r#"C4Context
Boundary(bank, "Bank")
Person(after, "After")
"#,
    );

    assert!(
        errors
            .iter()
            .any(|error| error.contains("expected '{' after boundary"))
    );
    assert_eq!(model.boundaries.len(), 1);
    assert_eq!(model.shapes[0].alias, "after");
    assert_eq!(model.shapes[0].parent_boundary, "global");
}

#[test]
fn c4_empty_boundary_is_rejected_and_rolled_back() {
    let (model, errors) = recovered_model_and_errors(
        r#"C4Context
Boundary(empty, "Empty") {
}
"#,
    );

    assert!(
        errors
            .iter()
            .any(|error| error.contains("boundary must contain"))
    );
    assert_eq!(model.boundaries.len(), 1);
}

#[test]
fn c4_unclosed_boundary_is_rejected_and_rolled_back_at_eof() {
    let source = r#"C4Context
Boundary(open, "Open") {
  Person(inside, "Inside")
"#;
    let (model, errors) = recovered_model_and_errors(source);

    assert!(
        errors
            .iter()
            .any(|error| error.contains("expected '}' before end"))
    );
    assert_eq!(model.boundaries.len(), 1);
    assert!(model.shapes.is_empty());
}

#[test]
fn c4_unmatched_closing_brace_does_not_change_safe_sibling_parent() {
    let (model, errors) = recovered_model_and_errors(
        r#"C4Context
Boundary(bank, "Bank") {
  Person(inside, "Inside")
}
}
Person(after, "After")
"#,
    );

    assert!(errors.iter().any(|error| error.contains("unexpected '}'")));
    assert_eq!(model.shapes[1].alias, "after");
    assert_eq!(model.shapes[1].parent_boundary, "global");
}
