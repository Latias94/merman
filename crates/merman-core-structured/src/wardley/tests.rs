use super::*;

#[test]
fn wardley_model_construction_observes_cancellation_inside_large_ast_collections() {
    let span = SourceSpan::new(0, 1);
    let ast = WardleyAst {
        components: (0..512)
            .map(|index| WardleyComponentAst {
                positioned: WardleyPositionedNodeAst {
                    name: SpannedText {
                        text: format!("component-{index}"),
                        span,
                        #[cfg(feature = "editor-facts")]
                        selection: span,
                        #[cfg(feature = "editor-facts")]
                        quoted: false,
                    },
                    visibility: SpannedNumber { value: 0.5, span },
                    evolution: SpannedNumber { value: 0.5, span },
                },
                label: None,
                source_strategy: None,
                #[cfg(feature = "editor-facts")]
                source_strategy_span: None,
                inertia: false,
                #[cfg(feature = "editor-facts")]
                inertia_span: None,
            })
            .collect(),
        ..WardleyAst::default()
    };
    let control = OperationControl::new();
    control.cancel_after_checkpoints(2);

    assert!(build_wardley_model_controlled(&ast, &control).is_err());
}

#[test]
fn wardley_source_keeps_syntax_error_priority_and_checkpoint_order() {
    let valid = "wardley-beta\ncomponent API [0.5, 0.25]\n";
    let range_error = "wardley-beta\ncomponent API [0.5, 101.0]\n";
    let syntax_error = "wardley-beta\ncomponent API [0.5, 101.0]\ncomponent Broken [1, 1]\n";
    // Coordinate validation follows the component checkpoint. Success also checks node
    // materialization, builder completion, and the model return.
    for (source, checkpoints, bad_coordinate) in [
        (valid, 10, None),
        (range_error, 7, Some(("101.0", 5))),
        (syntax_error, 5, Some(("1, 1", 1))),
    ] {
        let control = OperationControl::new();
        control.cancel_after_checkpoints(checkpoints);
        let ordinary = parse_wardley_source_controlled(source, &control)
            .expect("the exact checkpoint budget is sufficient");
        if let Some((token, length)) = bad_coordinate {
            let start = source.find(token).unwrap();
            assert_eq!(
                ordinary.unwrap_err().span,
                SourceSpan::new(start, start + length)
            );
        } else {
            assert_eq!(ordinary.unwrap().nodes[0].id, "API");
        }

        let control = OperationControl::new();
        control.cancel_after_checkpoints(checkpoints - 1);
        assert!(parse_wardley_source_controlled(source, &control).is_err());
    }
}

#[cfg(feature = "editor-facts")]
#[test]
fn wardley_observer_retains_confirmed_pipeline_suffix_after_first_issue() {
    let source = concat!(
        "wardley-beta\r\n",
        "pipeline Parent {\r\n",
        "  broken pipeline line\r\n",
        "  component \"Child λ\" [0.25]\r\n",
        "}\r\n",
        "title Recovered\r\n",
    );
    let mut directives = Vec::new();
    let mut fields = Vec::new();
    let mut common = Vec::new();
    let mut observe = |observation: WardleyObservation<'_>| match observation {
        WardleyObservation::Started => assert!(directives.is_empty()),
        WardleyObservation::Directive(prefix) => directives.push(prefix),
        WardleyObservation::Common(fact) => common.push(fact.value.clone()),
        WardleyObservation::Field {
            kind,
            text,
            detail,
            span,
            selection,
        } => fields.push((kind, text.to_owned(), detail, span, selection)),
    };
    let control = OperationControl::new();
    let problem = parse_wardley_source_with_observer_controlled(source, &mut observe, &control)
        .unwrap()
        .unwrap_err();
    let plain = parse_wardley_source_controlled(source, &control)
        .unwrap()
        .unwrap_err();
    assert_eq!((problem.message, problem.span), (plain.message, plain.span));
    assert_eq!(
        &source[problem.span.start..problem.span.end],
        "broken pipeline line"
    );
    assert_eq!(directives, ["wardley-beta", "pipeline", "component"]);
    assert_eq!(common, ["Recovered"]);
    assert_eq!(fields.len(), 3);
    let (kind, text, detail, span, selection) = &fields[1];
    assert_eq!(*kind, WardleyFieldKind::Definition { quoted: true });
    assert_eq!(text, "Child λ");
    assert_eq!(*detail, "wardley pipeline component");
    assert_eq!(&source[span.start..span.end], "\"Child λ\"");
    assert_eq!(&source[selection.start..selection.end], "Child λ");
}
