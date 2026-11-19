//! Complete Gantt reading records over the borrowed canonical and presentation task view.
//!
//! Task keys match source model indices, independently of section grouping or task order.
//! Dependencies stay literal typed rules in details; compatibility raw fields are never parsed.

use super::record::{RecordWriter, details, write_plain_record};
use super::{AsciiDiagram, AsciiError, RawValue, Result, SemanticFallbackWriter};
use merman_model::time::LocalTimeZone;
use serde::Deserialize;
use std::collections::BTreeMap;

type Fields<'a> = BTreeMap<String, &'a RawValue>;

pub(super) fn write_if_gantt(
    model: &AsciiDiagram<'_>,
    bytes: &[u8],
    lines: &mut SemanticFallbackWriter<'_>,
) -> Result<bool> {
    let AsciiDiagram::Gantt(model) = model else {
        return Ok(false);
    };
    lines.admit_node(0)?;
    lines.semantic_resources.charge_layout_work(bytes.len())?;
    let mut root: Fields<'_> = serde_json::from_slice(bytes).map_err(|_| invalid_projection())?;
    let tasks: Vec<&RawValue> =
        decode(root.remove("tasks").ok_or_else(invalid_projection)?, lines)?;
    if tasks.len() != model.tasks().len() {
        return Err(invalid_projection());
    }

    write_plain_record(
        "Keys: T1..Tn follow source task order. Dependencies retain literal IDs in typed constraints; raw fields are compatibility data.",
        lines,
    )?;
    let utc = LocalTimeZone::utc();
    for (index, (task, raw)) in model.tasks().zip(tasks).enumerate() {
        let mut fields: Fields<'_> = decode(raw, lines)?;
        let key = format!("T{}", index + 1);
        let mut record = RecordWriter::new(lines)?;
        record.text(&key, false)?;
        record.text(" [task] id=", false)?;
        record.text(&task.id, true)?;
        record.text(" label=", false)?;
        record.text(&task.task, true)?;
        record.text(" section=", false)?;
        record.text(&task.section, true)?;
        record.text(" sectionIndex=", false)?;
        match task.section_index {
            Some(index) => record.text(&index.to_string(), false)?,
            None => record.text("null", false)?,
        }
        record.text(&format!(" order={}", task.order), false)?;
        record.finish()?;
        for name in ["id", "task", "section", "sectionIndex", "order"] {
            fields.remove(name);
        }

        let mut record = RecordWriter::new(lines)?;
        record.text(&key, false)?;
        record.text(" interval (UTC): ", false)?;
        record.text(&crate::gantt::format_date(task.start_ms, &utc), false)?;
        record.text(" -> ", false)?;
        record.text(&crate::gantt::format_date(task.end_ms, &utc), false)?;
        if let Some(render_end) = task.render_end_ms {
            record.text(" renderEnd=", false)?;
            record.text(&crate::gantt::format_date(render_end, &utc), false)?;
        }
        record.finish()?;
        // Exact endpoints, typed rules, flags, classes, processing state, previous ID, raw
        // compatibility data, and future task fields remain complete borrowed JSON.
        details(&key, &fields, lines)?;
    }
    // These typed maps use randomized HashMap iteration. Sort their borrowed keys so fresh
    // canonical and full parses retain identical records, including arbitrary authored IDs.
    for name in ["links", "clickEvents"] {
        let raw = root.remove(name).ok_or_else(invalid_projection)?;
        let fields: Fields<'_> = decode(raw, lines)?;
        let mut record = RecordWriter::new(lines)?;
        record.text("gantt ", false)?;
        record.text(name, false)?;
        record.text(": {", false)?;
        for (index, (key, value)) in fields.iter().enumerate() {
            if index > 0 {
                record.text(",", false)?;
            }
            record.text(key, true)?;
            record.text(":", false)?;
            record.text(value.get(), false)?;
        }
        record.text("}", false)?;
        record.finish()?;
    }
    // Keep every remaining metadata field, including null/default values, repeated occurrences,
    // interaction data, source configuration/title, and any future envelope fields.
    details("gantt", &root, lines)?;
    Ok(true)
}

fn decode<'a, T: Deserialize<'a>>(
    raw: &'a RawValue,
    lines: &SemanticFallbackWriter<'_>,
) -> Result<T> {
    lines.admit_node(1)?;
    lines
        .semantic_resources
        .charge_layout_work(raw.get().len())?;
    serde_json::from_str(raw.get()).map_err(|_| invalid_projection())
}

fn invalid_projection() -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "gantt",
        feature: "invalid serialized semantic projection",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation::AsciiExecution;
    use crate::{AsciiRenderOptions, AsciiSourceContext};
    use crate::{AsciiResourceLimitId, AsciiResourcePolicy, TerminalWidthProfile};
    use merman_model::families::gantt::{GanttDiagramModel, GanttTerminalView};
    use merman_model::{MermaidConfig, OperationControl, OperationPhase};

    const ENVELOPE: &[u8] = br#"{
        "title":"Schedule","accTitle":"Accessible","accDescr":"Plan\nDetails",
        "dateFormat":"YYYY-MM-DD","axisFormat":"%d","tickInterval":"1day",
        "todayMarker":"off","includes":["2026-01-03"],"excludes":["weekends"],
        "inclusiveEndDates":true,"displayMode":"compact","topAxis":true,
        "weekday":"monday","weekend":"friday","sections":["A","B","A"],
        "tasks":[{
            "id":"T2","task":"  exact \"label\"\nnext  ","section":"A",
            "sectionIndex":2,"type":"A","order":7,"startTime":1,"endTime":2,
            "renderEndTime":null,
            "startConstraint":{"kind":"after","dependencyIds":["a b","a-b","a_b"]},
            "endConstraint":{"kind":"until","dependencyIds":["missing"]},
            "raw":{"data":"T2,after raw-start,until raw-end",
                "startTime":{"type":"prevTaskEnd","id":"compat"},
                "endTime":{"data":"until raw-end"}},
            "futureTask":{"opaque":"\ud800","number":1e+09}
        }],
        "links":{"T2":"https://example.com/task"},
        "clickEvents":{"T2":{"function_name":"onTask","function_args":["T2"],
            "raw_function_args":"'T2'"}},
        "type":"gantt","sourceConfig":{"gantt":{"displayMode":"compact"}},
        "sourceTitle":"Source title","futureRoot":{"opaque":"\ud800","number":1e+09}
    }"#;

    fn render(bytes: &[u8], policy: &AsciiResourcePolicy) -> Result<String> {
        let model: GanttDiagramModel = serde_json::from_slice(bytes).unwrap();
        let config =
            MermaidConfig::from_value(serde_json::json!({"gantt": {"displayMode": "compact"}}));
        let execution = AsciiExecution::for_test(policy);
        let mut lines =
            SemanticFallbackWriter::new(execution, usize::MAX, TerminalWidthProfile::Unicode);
        assert!(write_if_gantt(
            &AsciiDiagram::Gantt(GanttTerminalView::from_model(&model, &config)),
            bytes,
            &mut lines,
        )?);
        Ok(lines.finish().into_text())
    }

    #[test]
    fn complete_envelope_preserves_literal_rules_metadata_and_unknown_raw_tokens() {
        let output = render(ENVELOPE, &AsciiResourcePolicy::default()).unwrap();
        assert!(output.contains(
            r#"T1 [task] id="T2" label="  exact \"label\"\nnext  " section="A" sectionIndex=2 order=7"#,
        ));
        for text in [
            r#"startConstraint:{"kind":"after","dependencyIds":["a b","a-b","a_b"]}"#,
            r#"endConstraint:{"kind":"until","dependencyIds":["missing"]}"#,
            r#"futureTask:{"opaque":"\ud800","number":1e+09}"#,
            r#"futureRoot:{"opaque":"\ud800","number":1e+09}"#,
            r#"sections:["A","B","A"]"#,
            r#"gantt links: {"T2":"https://example.com/task"}"#,
            r#"sourceConfig:{"gantt":{"displayMode":"compact"}}"#,
            r#"sourceTitle:"Source title""#,
            r#"accDescr:"Plan\nDetails""#,
            r#"includes:["2026-01-03"]"#,
            r#"excludes:["weekends"]"#,
            r#"axisFormat:"%d""#,
            r#"dateFormat:"YYYY-MM-DD""#,
            r#"tickInterval:"1day""#,
            r#"todayMarker:"off""#,
            r#"displayMode:"compact""#,
            r#"weekday:"monday""#,
            r#"weekend:"friday""#,
            "inclusiveEndDates:true",
            "topAxis:true",
        ] {
            assert!(output.contains(text), "missing {text}:\n{output}");
        }
        for token in [
            r#"gantt clickEvents: {"T2":{"function_name":"onTask","function_args":["T2"],"#,
            r#""raw_function_args":"'T2'"}}"#,
        ] {
            assert!(output.contains(token), "missing {token}:\n{output}");
        }
        for field in ["T1 [task]", "T1 details:", "gantt details:"] {
            assert_eq!(output.matches(field).count(), 1);
        }
        let task_details = output
            .lines()
            .find(|line| line.starts_with("T1 details:"))
            .unwrap();
        for identity in ["id:", "task:", "section:", "sectionIndex:", "order:"] {
            assert!(!task_details.contains(identity));
        }
    }

    #[test]
    fn projection_work_is_admitted_before_deserialization() {
        let model = GanttDiagramModel::default();
        let config = MermaidConfig::empty_object();
        let policy = AsciiResourcePolicy::unbounded()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
            .unwrap();
        let execution = AsciiExecution::for_test(&policy);
        let mut lines = SemanticFallbackWriter::new(execution, 80, TerminalWidthProfile::Unicode);
        let error = write_if_gantt(
            &AsciiDiagram::Gantt(GanttTerminalView::from_model(&model, &config)),
            b"invalid JSON",
            &mut lines,
        )
        .expect_err("work admission must reject the input before attempting to decode it");
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
        ));
    }

    #[test]
    fn record_output_limit_and_cancellation_remain_terminal() {
        let length = render(ENVELOPE, &AsciiResourcePolicy::unbounded())
            .unwrap()
            .len();
        for limit in [length, length - 1] {
            let policy = AsciiResourcePolicy::unbounded()
                .with_limit(AsciiResourceLimitId::MaxOutputBytes, limit)
                .unwrap();
            let result = render(ENVELOPE, &policy);
            if limit == length {
                assert_eq!(result.unwrap().len(), length);
            } else {
                assert!(matches!(
                    result,
                    Err(AsciiError::ResourceLimitExceeded(details))
                        if details.limit == AsciiResourceLimitId::MaxOutputBytes
                ));
            }
        }
        let model: GanttDiagramModel = serde_json::from_slice(ENVELOPE).unwrap();
        let config = MermaidConfig::empty_object();
        let control = OperationControl::new();
        control.cancel_after_checkpoints(0);
        let policy = AsciiResourcePolicy::default();
        let execution = AsciiExecution::new(&control, &policy);
        let mut lines = SemanticFallbackWriter::new(execution, 80, TerminalWidthProfile::Unicode);
        assert!(matches!(
            write_if_gantt(
                &AsciiDiagram::Gantt(GanttTerminalView::from_model(&model, &config)),
                ENVELOPE,
                &mut lines,
            ),
            Err(AsciiError::Cancelled(cancelled)) if cancelled.phase == OperationPhase::Semantic
        ));
    }

    #[test]
    fn primary_peak_output_admits_topology_projection_and_records_together() {
        use merman_model::families::gantt::GanttTask;

        let data = GanttDiagramModel {
            tasks: vec![GanttTask {
                id: "long".into(),
                task: "x".repeat(65_536),
                start_ms: 0,
                end_ms: 1,
                processed: true,
                ..Default::default()
            }],
            ..Default::default()
        };
        let config = MermaidConfig::empty_object();
        let model = AsciiDiagram::Gantt(GanttTerminalView::from_model(&data, &config));
        let source = AsciiSourceContext {
            diagram_type: "gantt",
            config: &config,
            semantic_config: &config,
            title: None,
        };
        let render = |policy: AsciiResourcePolicy| {
            crate::gantt::render_gantt_diagram(
                GanttTerminalView::from_model(&data, &config),
                &AsciiRenderOptions::ascii(),
                &LocalTimeZone::utc(),
                source,
                AsciiExecution::for_test(&policy),
            )
            .map(|primary| primary.text)
        };
        let text = render(AsciiResourcePolicy::unbounded()).unwrap();
        let topology_bytes = text.split_once("\n\nGantt records").unwrap().0.len();
        let policy = AsciiResourcePolicy::unbounded();
        let projection = super::super::semantic_fallback_projection(
            &model,
            source,
            AsciiExecution::for_test(&policy),
        )
        .unwrap_or_else(|_| panic!("valid Gantt projection"));
        let peak = topology_bytes + projection.len() + text.len();
        for limit in [text.len(), peak - 1, peak] {
            let policy = policy
                .with_limit(AsciiResourceLimitId::MaxOutputBytes, limit)
                .unwrap();
            let result = render(policy);
            if limit == peak {
                assert_eq!(result.unwrap(), text);
            } else {
                assert!(
                    matches!(result, Err(AsciiError::ResourceLimitExceeded(details)) if details.limit == AsciiResourceLimitId::MaxOutputBytes),
                    "limit {limit}, peak {peak}"
                );
            }
        }
    }
}
