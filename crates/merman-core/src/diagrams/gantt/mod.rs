use crate::Error;
#[cfg(feature = "presentation")]
use crate::{Result, SourcePresentationMetadata};
#[cfg(feature = "compat-json")]
use serde_json::{Value, json};

mod parse;

pub(crate) use merman_model::families::gantt::GanttDiagramModel;
#[cfg(all(test, feature = "compat-json"))]
pub(crate) use merman_model::families::gantt::GanttTaskEndConstraint;
#[cfg(test)]
pub(crate) use merman_model::families::gantt::GanttTaskStartConstraint;
#[cfg(feature = "presentation")]
pub(crate) use merman_model::families::gantt::{GanttDiagramRenderModel, GanttOutputData};
#[cfg(all(test, feature = "presentation"))]
pub(crate) use merman_model::families::gantt::{GanttRenderTask, GanttTask};

#[cfg(test)]
pub(crate) use merman_core_structured::gantt::{
    gantt_syntax_construction_count, reset_gantt_syntax_construction_count,
};
#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
pub(crate) use parse::parse_gantt_json_and_editor_facts;
#[cfg(all(test, feature = "compat-json"))]
pub(crate) use parse::parse_gantt_model_for_render;
#[cfg(feature = "presentation")]
pub(crate) use parse::parse_gantt_model_for_render_controlled;
pub(crate) use parse::parse_gantt_semantic_model_controlled;
#[cfg(feature = "compat-json")]
pub(crate) use parse::{parse_gantt, render_model_to_compat_json};

pub(crate) fn sanitize_common_db_fields<Task>(
    model: &mut GanttDiagramModel<Task>,
    config: &crate::MermaidConfig,
) {
    crate::common_db::sanitize_optional_title(&mut model.title, config);
    crate::common_db::sanitize_optional_acc_title(&mut model.acc_title, config);
    crate::common_db::sanitize_optional_acc_descr(&mut model.acc_descr, config);
}

#[cfg(all(test, feature = "compat-json"))]
mod tests;

#[cfg(test)]
mod semantic_tests {
    use super::*;
    #[cfg(feature = "editor-facts")]
    use crate::family::EditorFactsDemand;
    use crate::{OperationControl, SemanticMetadata};
    use merman_model::time::CivilDate;

    fn metadata() -> SemanticMetadata {
        SemanticMetadata {
            diagram_type: "gantt".to_string(),
            config: crate::MermaidConfig::empty_object(),
            semantic_config: crate::MermaidConfig::empty_object(),
            title: None,
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn gantt_canonical_capture_retains_same_pass_facts_on_success_and_failure() {
        for (source, valid) in [
            (
                "gantt\ndateFormat YYYY-MM-DD\nsection Delivery\nTask: task,2026-01-01,1d\n",
                true,
            ),
            ("gantt\nsection Delivery\nweekday someday\n", false),
        ] {
            reset_gantt_syntax_construction_count();
            #[cfg(feature = "editor-facts")]
            crate::editor::test_work::reset();
            let parsed = parse_gantt_semantic_model_controlled(
                source,
                &metadata(),
                EditorFactsDemand::Capture,
                &OperationControl::new(),
            )
            .unwrap();
            assert_eq!(parsed.model.is_ok(), valid);
            let facts = parsed.editor_facts.expect("same-pass Gantt facts");
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "Delivery"));
            assert_eq!(gantt_syntax_construction_count(), 1);
            #[cfg(feature = "editor-facts")]
            assert_eq!(crate::editor::test_work::read().collections, 1);
            assert_eq!(
                facts.completeness,
                if valid {
                    crate::EditorSemanticCompleteness::Complete
                } else {
                    crate::EditorSemanticCompleteness::Recovered
                }
            );
            assert_eq!(facts.diagnostics.is_empty(), valid);
        }
    }

    #[cfg(feature = "editor-facts")]
    #[test]
    fn gantt_capture_keeps_scan_date_and_model_errors_distinct() {
        use crate::{EditorSemanticCompleteness, ParseDiagnosticSpanKind, SourceSpan};

        for (body, message, span_kind, completeness) in [
            (
                "weekday someday\nsection Delivery\nA:a,not-a-date,1d\n",
                "invalid weekday: someday",
                ParseDiagnosticSpanKind::Exact,
                EditorSemanticCompleteness::Recovered,
            ),
            (
                "section Delivery\nA:a,not-a-date,1d\n",
                "Invalid date:not-a-date",
                ParseDiagnosticSpanKind::Fallback,
                EditorSemanticCompleteness::Recovered,
            ),
            (
                "section Delivery\nA:a,after b,1d\nB:b,after a,1d\n",
                "task `a` has unresolved startTime",
                ParseDiagnosticSpanKind::Fallback,
                EditorSemanticCompleteness::Complete,
            ),
        ] {
            let source = format!("gantt\ndateFormat YYYY-MM-DD\n{body}");
            reset_gantt_syntax_construction_count();
            let parsed = parse_gantt_semantic_model_controlled(
                &source,
                &metadata(),
                EditorFactsDemand::Capture,
                &OperationControl::new(),
            )
            .unwrap();
            let Error::DiagramParse { diagnostic, .. } = parsed.model.unwrap_err() else {
                panic!("expected a Gantt parse diagnostic")
            };
            assert_eq!(diagnostic.message(), message);
            assert_eq!(diagnostic.span_kind(), span_kind);
            let expected_span = source
                .find("someday")
                .map(|start| SourceSpan::new(start, start + "someday".len()));
            assert_eq!(diagnostic.span(), expected_span);

            let facts = parsed.editor_facts.unwrap();
            assert_eq!(facts.completeness, completeness);
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "Delivery"));
            if completeness == EditorSemanticCompleteness::Recovered {
                assert_eq!(facts.diagnostics.len(), 1);
                assert_eq!(facts.diagnostics[0].span, expected_span);
            } else {
                assert!(facts.diagnostics.is_empty());
            }
            assert_eq!(gantt_syntax_construction_count(), 1);
        }
    }

    #[test]
    fn gantt_canonical_dates_use_runtime_authority_without_editor_work() {
        let context = merman_model::runtime::RuntimePolicy::deterministic()
            .with_fixed_today(Some(CivilDate::new(2026, 2, 15).unwrap()))
            .with_local_time_zone(crate::time::LocalTimeZone::utc())
            .begin_operation()
            .unwrap();
        crate::runtime::with_operation_context(&context, || {
            let source = "gantt\ndateFormat MM-DD\nsection Delivery\nBase: base,03-01,1d\nShip: ship,after base,1d\n";
            reset_gantt_syntax_construction_count();
            #[cfg(feature = "editor-facts")]
            crate::editor::test_work::reset();
            let model = parse_gantt_semantic_model_controlled(
                source,
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &OperationControl::new(),
            )
            .unwrap()
            .model
            .unwrap();
            assert_eq!(model.sections, ["Delivery"]);
            assert_eq!(model.tasks[0].start_ms, 1_772_323_200_000);
            assert_eq!(model.tasks[1].start_ms, model.tasks[0].end_ms);
            assert_eq!(model.tasks[1].end_ms - model.tasks[1].start_ms, 86_400_000);
            assert_eq!(
                model.tasks[1].start_constraint,
                GanttTaskStartConstraint::After {
                    dependency_ids: vec!["base".to_string()]
                }
            );
            assert_eq!(gantt_syntax_construction_count(), 1);
            #[cfg(feature = "editor-facts")]
            assert_eq!(crate::editor::test_work::read(), Default::default());
        });
    }

    #[test]
    fn gantt_canonical_tasks_retain_excludes_visible_end_in_same_construction() {
        let context = merman_model::runtime::RuntimePolicy::deterministic()
            .with_local_time_zone(crate::time::LocalTimeZone::utc())
            .begin_operation()
            .unwrap();
        crate::runtime::with_operation_context(&context, || {
            reset_gantt_syntax_construction_count();
            let model = parse_gantt_semantic_model_controlled(
                "gantt\ndateFormat YYYY-MM-DD\nexcludes weekends\nTask: task,2019-02-01,1d\n",
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &OperationControl::new(),
            )
            .unwrap()
            .model
            .unwrap();
            let task = &model.tasks[0];
            assert_eq!(task.start_ms, 1_548_979_200_000);
            assert_eq!(task.end_ms, 1_549_238_400_000);
            assert_eq!(task.render_end_ms, Some(1_549_065_600_000));
            assert!(
                serde_json::to_value(task)
                    .unwrap()
                    .get("renderEndTime")
                    .is_none()
            );
            assert_eq!(gantt_syntax_construction_count(), 1);
        });
    }

    #[test]
    fn gantt_canonical_empty_exclusion_scan_accepts_milestones_and_subday_tasks() {
        let context = merman_model::runtime::RuntimePolicy::deterministic()
            .with_local_time_zone(crate::time::LocalTimeZone::utc())
            .begin_operation()
            .unwrap();
        crate::runtime::with_operation_context(&context, || {
            for line_ending in ["\n", "\r\n"] {
                let source = concat!(
                    "gantt\n",
                    "dateFormat YYYY-MM-DD\n",
                    "excludes weekends,monday,tuesday,wednesday,thursday,friday\n",
                    "M:milestone,m,2026-01-01,0d\n",
                    "H:h,2026-01-01,1h\n",
                )
                .replace('\n', line_ending);
                let model = parse_gantt_semantic_model_controlled(
                    &source,
                    &metadata(),
                    #[cfg(feature = "editor-facts")]
                    EditorFactsDemand::Omit,
                    &OperationControl::new(),
                )
                .unwrap()
                .model
                .unwrap();
                assert_eq!(model.tasks.len(), 2);
                assert!(model.tasks[0].milestone);
                for (task, duration) in model.tasks.iter().zip([0, 3_600_000]) {
                    assert!(task.processed);
                    assert_eq!(task.start_ms, 1_767_225_600_000);
                    assert_eq!(task.end_ms - task.start_ms, duration);
                    assert_eq!(task.render_end_ms, None);
                }
            }
        });
    }

    #[test]
    fn gantt_canonical_security_admission_uses_semantic_config() {
        let source = "gantt\ndateFormat YYYY-MM-DD\nTask: task,2026-01-01,1d\nclick task href \"javascript:alert(1)\" callback\n";
        for (level, expected_url, callback_admitted) in [
            ("strict", "about:blank", false),
            ("loose", "javascript:alert(1)", true),
        ] {
            let mut metadata = metadata();
            metadata.semantic_config = crate::MermaidConfig::from_value(serde_json::json!({
                "securityLevel": level, "gantt": {"displayMode": "compact"}
            }));
            #[cfg(feature = "editor-facts")]
            crate::editor::test_work::reset();
            let model = parse_gantt_semantic_model_controlled(
                source,
                &metadata,
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &OperationControl::new(),
            )
            .unwrap()
            .model
            .unwrap();
            assert_eq!(model.links["task"], expected_url);
            assert_eq!(model.click_events.contains_key("task"), callback_admitted);
            let serialized = serde_json::to_value(&model).unwrap();
            assert!(serialized.get("displayMode").is_none());
            assert!(serialized["tasks"][0].get("renderEndTime").is_none());
            #[cfg(feature = "editor-facts")]
            assert_eq!(crate::editor::test_work::read(), Default::default());
        }
    }

    #[test]
    fn gantt_canonical_errors_keep_exact_spans_without_editor_work() {
        let source = "gantt\nweekday someday\n";
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let error = parse_gantt_semantic_model_controlled(
            source,
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected parse diagnostic")
        };
        let offset = source.find("someday").unwrap();
        assert_eq!(
            diagnostic.span(),
            Some(crate::SourceSpan::new(offset, offset + 7))
        );
        assert_eq!(
            diagnostic.span_kind(),
            crate::ParseDiagnosticSpanKind::Exact
        );
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
    }

    #[test]
    fn gantt_canonical_construction_observes_cancellation() {
        let control = OperationControl::new();
        control.cancel();
        assert!(
            parse_gantt_semantic_model_controlled(
                "gantt\n",
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &control,
            )
            .is_err()
        );
    }

    #[cfg(feature = "presentation")]
    #[test]
    fn gantt_full_producer_retains_output_data_independently_of_json_projection() {
        let metadata = SourcePresentationMetadata {
            diagram_type: "gantt".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        };
        for (source, expected) in [
            ("", GanttOutputData::Empty),
            ("gantt", GanttOutputData::Model),
        ] {
            let model = parse_gantt_model_for_render_controlled(
                source,
                &metadata,
                &OperationControl::new(),
            )
            .unwrap()
            .unwrap();
            assert_eq!(model.output_data(), expected);
        }
    }

    #[cfg(feature = "presentation")]
    #[test]
    fn gantt_typed_projection_builds_visual_sidecar_without_editor_work() {
        let source = "gantt\ndateFormat YYYY-MM-DD\nexcludes weekends\nTask: task,2019-02-01,1d\n";
        let metadata = SourcePresentationMetadata {
            diagram_type: "gantt".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::from_value(
                serde_json::json!({"gantt": {"displayMode": "compact"}}),
            ),
            title: None,
        };
        reset_gantt_syntax_construction_count();
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let model =
            parse_gantt_model_for_render_controlled(source, &metadata, &OperationControl::new())
                .unwrap()
                .unwrap();
        assert_eq!(model.display_mode, "compact");
        assert!(model.tasks[0].render_end_ms.is_some());
        assert_eq!(gantt_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
    }
}
