use super::*;

#[test]
fn architecture_failed_quote_lookahead_is_linear_during_recovery() {
    for count in [64, 512] {
        let mut code = String::from("architecture-beta\n");
        for _ in 0..count {
            code.push_str("service broken \\\"\nservice broken \\'\n");
        }
        code.push_str("service later [Later]\n");
        let control = OperationControl::new();
        let mut lines = ArchitectureLineCursor::new(&code);
        lines.next().unwrap();
        let mut statements = 0;
        let mut last = "";
        while let Some(line) = lines.next() {
            last = read_statement(&code, &mut lines, line.start, &control).unwrap();
            statements += 1;
        }
        assert_eq!(statements, count * 2 + 1);
        assert_eq!(last, "service later [Later]");
        assert!(
            lines.lookahead.inspected_characters <= code.len() * 2,
            "failed searches revisited the document: {} characters for {} bytes",
            lines.lookahead.inspected_characters,
            code.len()
        );

        #[cfg(feature = "editor-facts")]
        for capture_rich in [true, false] {
            let mut diagnostics = 0;
            let mut symbols = 0;
            let mut later = None;
            let outcome = parse_architecture_source_observed_controlled(
                &code,
                capture_rich,
                &mut |observation| match observation {
                    ArchitectureObservation::Issue(_) => diagnostics += 1,
                    ArchitectureObservation::Occurrence {
                        text, selection, ..
                    } => {
                        symbols += 1;
                        if text == "later" {
                            later = Some(selection);
                        }
                    }
                    _ => {}
                },
                &control,
            )
            .unwrap();
            assert!(outcome.is_err());
            assert_eq!(diagnostics, count * 2);
            if capture_rich {
                let later =
                    later.expect("capture must retain the declaration after malformed lines");
                assert_eq!(&code[later.start..later.end], "later");
            } else {
                assert_eq!(symbols, 0);
                let trace = parse_trace_controlled(
                    &code,
                    ArchitectureParseMode::Recovering,
                    false,
                    &mut None,
                    &control,
                )
                .unwrap()
                .unwrap();
                let Some(ArchitectureStatement::Service { id, .. }) = trace
                    .entries
                    .last()
                    .and_then(|entry| entry.statement.as_ref())
                else {
                    panic!("diagnostic-only recovery must still parse the later declaration");
                };
                assert_eq!(id.text, "later");
            }
        }
    }
}

#[test]
fn architecture_long_quote_lookahead_observes_cancellation() {
    let code = format!("service a \"{}\"\n", "長".repeat(4096));
    let control = OperationControl::new();
    control.cancel_after_checkpoints(1);
    let mut lines = ArchitectureLineCursor::new(&code);
    assert!(read_statement(&code, &mut lines, 0, &control).is_err());
    assert!(lines.lookahead.inspected_characters <= 256);
    assert_eq!(lines.offset, 0);
}
