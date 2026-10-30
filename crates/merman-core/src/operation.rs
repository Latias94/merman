//! Source integration tests for shared operation control.
// Temporary forwarding during M1; removed when all consumers use merman-model at M7.
pub use merman_model::operation::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, Instant};

    #[cfg(all(
        feature = "diagram-sequence",
        feature = "diagram-state",
        feature = "compat-json",
        feature = "editor-facts"
    ))]
    #[test]
    fn parse_projection_terminals_precede_facts_and_allow_fresh_operations() {
        use crate::parse_pipeline::{ParsePipeline, ParseTiming};
        use crate::{MermaidEngine, ParseOptions, ParsedEditorFacts};

        let engine = MermaidEngine::new();
        for (diagram_type, source) in [
            ("sequence", "sequenceDiagram\nAlice->>Bob: Hello\n"),
            (
                "stateDiagram",
                "stateDiagram-v2\nIdle --> Running: starts\n",
            ),
        ] {
            for known_type in [false, true] {
                for snapshot in [false, true] {
                    for reason in [CancelReason::Requested, CancelReason::DeadlineExceeded] {
                        crate::editor::test_work::reset();
                        let base = Instant::now();
                        let control = OperationControl::with_test_clock(Arc::new(move || base));
                        match reason {
                            CancelReason::Requested => control.cancel(),
                            CancelReason::DeadlineExceeded => {
                                control.set_deadline(Duration::ZERO);
                            }
                        }
                        let terminal = if snapshot {
                            let pipeline = if known_type {
                                ParsePipeline::known_type(
                                    &engine,
                                    diagram_type,
                                    source,
                                    ParseOptions::strict(),
                                )
                            } else {
                                ParsePipeline::detect(&engine, source, ParseOptions::strict())
                            };
                            pipeline
                                .parse_editor_snapshot_controlled(ParseTiming::None, &control)
                                .expect_err("an early terminal cannot return a partial snapshot")
                        } else if known_type {
                            engine
                                .parse_diagram_for_render_model_with_type_controlled_sync(
                                    diagram_type,
                                    source,
                                    ParseOptions::strict(),
                                    &control,
                                )
                                .expect_err("an early terminal cannot return a typed model")
                        } else {
                            engine
                                .parse_diagram_for_render_model_controlled_sync(
                                    source,
                                    ParseOptions::strict(),
                                    &control,
                                )
                                .expect_err("an early terminal cannot return a typed model")
                        };
                        assert_eq!(terminal.reason, reason);
                        assert_eq!(crate::editor::test_work::read(), Default::default());
                        assert_eq!(control.checkpoint(), Err(terminal));
                        assert_eq!(crate::editor::test_work::read(), Default::default());

                        let fresh = OperationControl::new();
                        if snapshot {
                            let pipeline = if known_type {
                                ParsePipeline::known_type(
                                    &engine,
                                    diagram_type,
                                    source,
                                    ParseOptions::strict(),
                                )
                            } else {
                                ParsePipeline::detect(&engine, source, ParseOptions::strict())
                            };
                            let parsed = pipeline
                                .parse_editor_snapshot_controlled(ParseTiming::None, &fresh)
                                .unwrap()
                                .unwrap()
                                .expect("a fresh operation recovers the snapshot");
                            assert!(matches!(
                                parsed.outcome(),
                                crate::DiagramParseOutcome::Parsed { .. }
                            ));
                            assert!(matches!(
                                parsed.editor_facts(),
                                ParsedEditorFacts::Available(_)
                            ));
                            assert_eq!(crate::editor::test_work::read().collections, 1);
                            assert!(crate::editor::test_work::read().symbols > 0);
                        } else {
                            let parsed = if known_type {
                                engine.parse_diagram_for_render_model_with_type_controlled_sync(
                                    diagram_type,
                                    source,
                                    ParseOptions::strict(),
                                    &fresh,
                                )
                            } else {
                                engine.parse_diagram_for_render_model_controlled_sync(
                                    source,
                                    ParseOptions::strict(),
                                    &fresh,
                                )
                            }
                            .unwrap()
                            .unwrap()
                            .expect("a fresh operation recovers the typed model");
                            assert_eq!(parsed.metadata().diagram_type, diagram_type);
                            assert_eq!(crate::editor::test_work::read(), Default::default());
                        }
                    }
                }
            }
        }
    }

    #[cfg(all(
        feature = "diagram-sequence",
        feature = "compat-json",
        feature = "editor-facts"
    ))]
    #[test]
    fn sequence_editor_capture_deadline_interrupts_facts_and_recovers() {
        use crate::{
            DiagramParseOutcome, EditorSemanticCompleteness, MermaidEngine, ParsedEditorFacts,
        };

        let engine = MermaidEngine::new();
        let source = format!("sequenceDiagram\n{}", "Alice->>Bob: Hello\n".repeat(1_024));
        crate::editor::test_work::reset();
        crate::diagrams::sequence::reset_sequence_syntax_construction_count();
        let clock_calls = Arc::new(AtomicU64::new(0));
        let observed_calls = Arc::clone(&clock_calls);
        let base = Instant::now();
        let clock = Arc::new(move || {
            observed_calls.fetch_add(1, Ordering::Relaxed);
            if crate::editor::test_work::read().symbols > 0 {
                base + Duration::from_millis(2)
            } else {
                base
            }
        });
        let control =
            OperationControl::with_test_clock(clock).with_deadline(Duration::from_millis(1));

        let terminal = engine
            .capture_diagram_snapshot_controlled_sync(&source, &control)
            .expect_err("a deadline during editor collection cannot return a partial snapshot");

        assert_eq!(terminal.reason, CancelReason::DeadlineExceeded);
        assert_eq!(
            crate::diagrams::sequence::sequence_syntax_construction_count(),
            1
        );
        let partial = crate::editor::test_work::read();
        assert_eq!(partial.collections, 1);
        assert!(partial.symbols > 0 && partial.symbols <= 128 * 3);
        assert!(partial.expected_syntax > 0 && partial.expected_syntax <= 128);
        let calls_at_terminal = clock_calls.load(Ordering::Relaxed);
        assert!(calls_at_terminal > 1);
        assert_eq!(control.checkpoint(), Err(terminal));
        assert_eq!(
            engine
                .capture_diagram_snapshot_controlled_sync(&source, &control)
                .unwrap_err(),
            terminal,
        );
        assert_eq!(clock_calls.load(Ordering::Relaxed), calls_at_terminal);
        assert_eq!(crate::editor::test_work::read(), partial);

        crate::editor::test_work::reset();
        crate::diagrams::sequence::reset_sequence_syntax_construction_count();
        let snapshot = engine
            .capture_diagram_snapshot_controlled_sync(&source, &OperationControl::new())
            .unwrap()
            .into_result()
            .unwrap()
            .expect("a fresh operation recovers the full snapshot");
        let DiagramParseOutcome::Parsed { model, .. } = snapshot.outcome() else {
            panic!("the recovered Sequence snapshot must contain its canonical model");
        };
        assert_eq!(model["actorOrder"], serde_json::json!(["Alice", "Bob"]));
        assert_eq!(model["messages"].as_array().unwrap().len(), 1_024);
        let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
            panic!("the recovered Sequence snapshot must retain editor facts");
        };
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
        assert!(facts.symbols.len() > partial.symbols);
        assert_eq!(
            crate::diagrams::sequence::sequence_syntax_construction_count(),
            1
        );
        assert_eq!(crate::editor::test_work::read().collections, 1);
    }

    #[cfg(all(
        feature = "diagram-state",
        feature = "compat-json",
        feature = "editor-facts"
    ))]
    #[test]
    fn state_editor_capture_deadline_bounds_event_drain_and_recovers() {
        use crate::{
            DiagramParseOutcome, EditorExpectedSyntaxKind, EditorSemanticCompleteness,
            EditorSemanticRole, MermaidEngine, ParsedEditorFacts, SourceSpan,
        };

        let engine = MermaidEngine::new();
        let source = format!(
            "stateDiagram-v2\n{}",
            "Idle --> Running: starts\n".repeat(1_024)
        );
        crate::editor::test_work::reset();
        crate::diagrams::state::reset_state_syntax_construction_count();
        let clock_calls = Arc::new(AtomicU64::new(0));
        let observed_calls = Arc::clone(&clock_calls);
        let base = Instant::now();
        let clock = Arc::new(move || {
            observed_calls.fetch_add(1, Ordering::Relaxed);
            if crate::editor::test_work::read().symbols > 0 {
                base + Duration::from_millis(2)
            } else {
                base
            }
        });
        let control =
            OperationControl::with_test_clock(clock).with_deadline(Duration::from_millis(1));

        let terminal = engine
            .capture_diagram_snapshot_controlled_sync(&source, &control)
            .expect_err("a deadline during State event drain cannot return a partial snapshot");

        assert_eq!(terminal.reason, CancelReason::DeadlineExceeded);
        assert_eq!(crate::diagrams::state::state_syntax_construction_count(), 1);
        let partial = crate::editor::test_work::read();
        assert_eq!(partial.collections, 1);
        assert!(partial.symbols > 0 && partial.symbols <= 128, "{partial:?}");
        assert!(
            partial.expected_syntax > 0 && partial.expected_syntax <= 128,
            "{partial:?}"
        );
        let calls_at_terminal = clock_calls.load(Ordering::Relaxed);
        assert!(calls_at_terminal > 1);
        assert_eq!(control.checkpoint(), Err(terminal));
        assert_eq!(
            engine
                .capture_diagram_snapshot_controlled_sync(&source, &control)
                .unwrap_err(),
            terminal,
        );
        assert_eq!(clock_calls.load(Ordering::Relaxed), calls_at_terminal);
        assert_eq!(crate::editor::test_work::read(), partial);

        crate::editor::test_work::reset();
        crate::diagrams::state::reset_state_syntax_construction_count();
        let snapshot = engine
            .capture_diagram_snapshot_controlled_sync(&source, &OperationControl::new())
            .unwrap()
            .into_result()
            .unwrap()
            .expect("a fresh operation recovers the full State snapshot");
        let DiagramParseOutcome::Parsed { model, .. } = snapshot.outcome() else {
            panic!("the recovered State snapshot must contain its canonical model");
        };
        assert_eq!(model["states"]["Idle"]["id"], "Idle");
        assert_eq!(model["states"]["Running"]["id"], "Running");
        assert_eq!(model["relations"].as_array().unwrap().len(), 1_024);
        assert_eq!(model["edges"][0]["label"], "starts");
        let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
            panic!("the recovered State snapshot must retain editor facts");
        };
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
        assert!(facts.symbols.len() > partial.symbols);
        let running_start = source.find("Running").unwrap();
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "Running"
                && symbol.selection
                    == SourceSpan::new(running_start, running_start + "Running".len())
        }));
        let label_start = source.find("starts").unwrap();
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "starts"
                && symbol.role == EditorSemanticRole::Payload
                && symbol.selection == SourceSpan::new(label_start, label_start + "starts".len())
        }));
        assert!(facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::Payload
                && expected.span.start <= label_start
                && expected.span.end >= label_start + "starts".len()
        }));
        assert_eq!(crate::diagrams::state::state_syntax_construction_count(), 1);
        assert_eq!(crate::editor::test_work::read().collections, 1);
    }

    #[cfg(all(
        feature = "diagram-state",
        feature = "compat-json",
        feature = "editor-facts"
    ))]
    #[test]
    fn state_editor_capture_deadline_bounds_single_token_id_list() {
        use crate::{
            DiagramParseOutcome, EditorSemanticCompleteness, EditorSemanticRole, MermaidEngine,
            ParsedEditorFacts, SourceSpan,
        };

        let engine = MermaidEngine::new();
        let ids = (0..1_024)
            .map(|index| format!("S{index}"))
            .collect::<Vec<_>>();
        let source = format!("stateDiagram-v2\nstyle {} fill:red\n", ids.join(","));
        crate::editor::test_work::reset();
        crate::diagrams::state::reset_state_syntax_construction_count();
        let base = Instant::now();
        let clock = Arc::new(move || {
            if crate::editor::test_work::read().symbols > 0 {
                base + Duration::from_millis(2)
            } else {
                base
            }
        });
        let control =
            OperationControl::with_test_clock(clock).with_deadline(Duration::from_millis(1));

        let terminal = engine
            .capture_diagram_snapshot_controlled_sync(&source, &control)
            .expect_err("a deadline inside one ID-list token cannot return a partial snapshot");

        assert_eq!(terminal.reason, CancelReason::DeadlineExceeded);
        assert_eq!(crate::diagrams::state::state_syntax_construction_count(), 1);
        let partial = crate::editor::test_work::read();
        assert_eq!(partial.collections, 1);
        assert!(partial.symbols > 0 && partial.symbols <= 128, "{partial:?}");
        assert!(
            partial.expected_syntax > 0 && partial.expected_syntax <= 129,
            "one aggregate ID-list slot plus at most 128 reference slots: {partial:?}"
        );

        crate::editor::test_work::reset();
        crate::diagrams::state::reset_state_syntax_construction_count();
        let snapshot = engine
            .parse_diagram_snapshot_sync(&source)
            .unwrap()
            .unwrap();
        let DiagramParseOutcome::Parsed { model, .. } = snapshot.outcome() else {
            panic!("a fresh operation must recover the complete State style assignment");
        };
        assert_eq!(model["states"].as_object().unwrap().len(), ids.len());
        assert_eq!(
            model["states"]["S0"]["styles"],
            serde_json::json!(["fill:red"])
        );
        assert_eq!(
            model["states"]["S1023"]["styles"],
            serde_json::json!(["fill:red"])
        );
        let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
            panic!("the complete State ID list must retain requested facts");
        };
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
        let references = facts
            .symbols
            .iter()
            .filter(|symbol| symbol.role == EditorSemanticRole::Reference)
            .collect::<Vec<_>>();
        assert_eq!(references.len(), ids.len());
        let mut start = source.find("S0").unwrap();
        for (symbol, id) in references.into_iter().zip(&ids) {
            assert_eq!(&symbol.name, id);
            assert_eq!(symbol.selection, SourceSpan::new(start, start + id.len()));
            start += id.len() + 1;
        }
        assert_eq!(crate::diagrams::state::state_syntax_construction_count(), 1);
        assert_eq!(crate::editor::test_work::read().collections, 1);
    }
}
