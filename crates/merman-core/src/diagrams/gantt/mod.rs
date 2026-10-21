#[cfg(feature = "presentation")]
use crate::ParseMetadata;
use crate::{
    Error, Result,
    time::{CivilDate, CivilDateTime, OffsetDateTime, UtcOffset},
    utils,
};
#[cfg(feature = "compat-json")]
use serde_json::{Value, json};
use std::collections::HashMap;

mod date;
mod datetime;
mod model;
mod parse;

use date::*;
use datetime::*;
use model::*;

pub use model::{
    GanttClickEvent, GanttDiagramModel, GanttTask, GanttTaskEnd, GanttTaskEndConstraint,
    GanttTaskRaw, GanttTaskStart, GanttTaskStartConstraint,
};
#[cfg(feature = "presentation")]
pub use model::{
    GanttDiagramRenderModel, GanttRenderClickEvent, GanttRenderTask, GanttRenderTaskEnd,
    GanttRenderTaskRaw, GanttRenderTaskStart,
};
#[cfg(all(test, feature = "presentation"))]
pub(crate) use parse::parse_gantt_model_for_render;
#[cfg(feature = "presentation")]
pub(crate) use parse::parse_gantt_model_for_render_controlled;
pub(crate) use parse::parse_gantt_semantic_model_controlled;
#[cfg(test)]
pub(crate) use parse::{gantt_syntax_construction_count, reset_gantt_syntax_construction_count};
#[cfg(feature = "compat-json")]
pub(crate) use parse::{
    parse_gantt, render_model_to_compat_json, render_model_to_compat_json_controlled,
};

#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
pub(crate) use parse::parse_gantt_json_and_editor_facts;

const ALL_WEEKDAYS_MASK: u8 = 0b0111_1111;
const MAX_CONSECUTIVE_EXCLUDED_DAYS: usize = 366;

fn weekday_mask_monday_based(iso_weekday: u32) -> u8 {
    1u8 << (iso_weekday.saturating_sub(1) as u8)
}

fn excluded_weekdays_mask(excludes: &[String], weekend: &str) -> u8 {
    let mut mask = 0u8;

    if excludes.iter().any(|v| v == "weekends") {
        let weekend_start = match weekend {
            "friday" => 5u32,
            _ => 6u32,
        };
        mask |= weekday_mask_monday_based(weekend_start);
        mask |= weekday_mask_monday_based(weekend_start + 1);
    }

    for value in excludes {
        let weekday = match value.as_str() {
            "monday" => Some(1),
            "tuesday" => Some(2),
            "wednesday" => Some(3),
            "thursday" => Some(4),
            "friday" => Some(5),
            "saturday" => Some(6),
            "sunday" => Some(7),
            _ => None,
        };
        if let Some(weekday) = weekday {
            mask |= weekday_mask_monday_based(weekday);
        }
    }

    mask
}

fn validate_excludes_leave_a_working_weekday(db: &GanttDb) -> Result<()> {
    if db.includes.is_empty()
        && excluded_weekdays_mask(&db.excludes, &db.weekend) == ALL_WEEKDAYS_MASK
    {
        return Err(Error::diagram_parse_fallback(
            "gantt".to_string(),
            "invalid excludes: excludes every weekday".to_string(),
        ));
    }
    Ok(())
}

fn is_invalid_date(db: &GanttDb, date: OffsetDateTime, date_format: &str) -> bool {
    let formatted = format_dayjs_like(date, date_format);
    let date_only = format_dayjs_like(date, "YYYY-MM-DD");

    if db
        .includes
        .iter()
        .any(|v| v == &formatted || v == &date_only)
    {
        return false;
    }

    if db.excludes.iter().any(|v| v == "weekends") {
        let weekend_start = match db.weekend.as_str() {
            "friday" => 5u32,
            _ => 6u32,
        };
        let local = crate::runtime::datetime_to_local_civil(date);
        let iso = local.weekday().number_from_monday(); // 1..=7
        if iso == weekend_start || iso == weekend_start + 1 {
            return true;
        }
    }

    let weekday = crate::runtime::datetime_to_local_civil(date)
        .weekday()
        .full_name()
        .to_lowercase();
    if db.excludes.iter().any(|v| v == &weekday) {
        return true;
    }

    db.excludes
        .iter()
        .any(|v| v == &formatted || v == &date_only)
}

fn fix_task_dates(
    db: &GanttDb,
    mut start_time: OffsetDateTime,
    mut end_time: OffsetDateTime,
    date_format: &str,
) -> Result<(OffsetDateTime, Option<OffsetDateTime>)> {
    validate_excludes_leave_a_working_weekday(db)?;

    let mut invalid = false;
    #[cfg(feature = "presentation")]
    let mut render_end_time: Option<OffsetDateTime> = None;
    #[cfg(not(feature = "presentation"))]
    let render_end_time: Option<OffsetDateTime> = None;
    let mut consecutive_invalid_days = 0usize;
    while start_time <= end_time {
        #[cfg(feature = "presentation")]
        if db.capture_presentation && !invalid {
            render_end_time = Some(end_time);
        }
        invalid = is_invalid_date(db, start_time, date_format);
        if invalid {
            consecutive_invalid_days += 1;
            if consecutive_invalid_days > MAX_CONSECUTIVE_EXCLUDED_DAYS {
                return Err(Error::diagram_parse_fallback(
                    "gantt".to_string(),
                    "invalid excludes: no includable date found within one year".to_string(),
                ));
            }
            end_time = add_days_local(end_time, 1).ok_or_else(|| {
                Error::diagram_parse_fallback(
                    "gantt".to_string(),
                    "invalid excludes: adjusted task end date is out of range".to_string(),
                )
            })?;
        } else {
            consecutive_invalid_days = 0;
        }
        start_time = add_days_local(start_time, 1).ok_or_else(|| {
            Error::diagram_parse_fallback(
                "gantt".to_string(),
                "invalid excludes: adjusted task start date is out of range".to_string(),
            )
        })?;
    }
    Ok((end_time, render_end_time))
}

#[cfg(all(test, feature = "compat-json"))]
mod tests;

#[cfg(test)]
mod semantic_tests {
    use super::*;
    #[cfg(feature = "editor-facts")]
    use crate::family::EditorFactsDemand;
    use crate::{OperationControl, SemanticMetadata};

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

    #[test]
    fn gantt_canonical_dates_use_runtime_authority_without_editor_work() {
        let context = crate::runtime::RuntimePolicy::deterministic()
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
    fn gantt_typed_projection_builds_visual_sidecar_without_editor_work() {
        let source = "gantt\ndateFormat YYYY-MM-DD\nexcludes weekends\nTask: task,2019-02-01,1d\n";
        let metadata = ParseMetadata {
            diagram_type: "gantt".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::PresentationConfig::from_value(
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
