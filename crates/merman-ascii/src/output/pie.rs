//! Complete Pie records, preserving section order and exact non-JSON numeric values.

use super::record::{RecordWriter, details, write_plain_record};
use super::{AsciiDiagram, AsciiError, RawValue, Result, SemanticFallbackWriter};
use crate::pie::ExactValue;
use serde::Deserialize;
use std::collections::BTreeMap;

type Fields<'a> = BTreeMap<String, &'a RawValue>;

pub(super) fn write_if_pie(
    model: &AsciiDiagram<'_>,
    bytes: &[u8],
    lines: &mut SemanticFallbackWriter<'_>,
) -> Result<bool> {
    let AsciiDiagram::Pie(model) = model else {
        return Ok(false);
    };
    lines.admit_node(0)?;
    lines.semantic_resources.charge_layout_work(bytes.len())?;
    let mut fields: Fields<'_> = serde_json::from_slice(bytes).map_err(|_| invalid_projection())?;
    let sections: Vec<&RawValue> = decode(
        fields.remove("sections").ok_or_else(invalid_projection)?,
        lines,
    )?;
    if sections.len() != model.sections.len() {
        return Err(invalid_projection());
    }
    write_plain_record(
        if model.show_data {
            "showData: true"
        } else {
            "showData: false"
        },
        lines,
    )?;
    fields.remove("showData");
    for (name, value) in [
        ("title", model.title.as_deref()),
        ("accTitle", model.acc_title.as_deref()),
        ("accDescr", model.acc_descr.as_deref()),
    ] {
        if let Some(value) = value {
            let mut record = RecordWriter::new(lines)?;
            record.text(name, false)?;
            record.text(": ", false)?;
            record.text(value, true)?;
            record.finish()?;
            fields.remove(name);
        }
    }
    for (index, (section, raw)) in model.sections.iter().zip(sections).enumerate() {
        let key = format!("P{}", index + 1);
        let mut extra: Fields<'_> = decode(raw, lines)?;
        let mut record = RecordWriter::new(lines)?;
        record.text(&key, false)?;
        record.text(" label=", false)?;
        record.text(&section.label, true)?;
        record.text(" value=", false)?;
        record.text(&ExactValue(section.value).to_string(), false)?;
        if !section.value.is_finite() {
            record.text(&format!(" bits=0x{:016x}", section.value.to_bits()), false)?;
        }
        record.finish()?;
        extra.remove("label");
        extra.remove("value");
        details(&key, &extra, lines)?;
    }
    // Default metadata and future envelope fields remain complete borrowed JSON, including
    // numeric token spelling and UTF-16 escapes. The Pie heading already identifies type.
    fields.remove("type");
    details("pie", &fields, lines)?;
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
        diagram_type: "pie",
        feature: "invalid serialized semantic projection",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation::AsciiExecution;
    use crate::{AsciiResourceLimitId, AsciiResourcePolicy, TerminalWidthProfile};
    use merman_model::families::pie::PieDiagramModel;
    use merman_model::{OperationControl, OperationPhase};

    const ENVELOPE: &[u8] = br#"{
        "showData":true,"title":"  exact \"quote\"\nnext  ",
        "accTitle":"Accessible","accDescr":"All values",
        "sections":[{"label":"A","value":1,
            "futureSection":{"raw":"\ud800","number":1e+09}},
            {"label":"A","value":-0.0}],
        "sourceConfig":{"pie":{"future":true}},"sourceTitle":"Source",
        "futureRoot":{"opaque":"\ud800","number":1e+09}
    }"#;

    fn render(bytes: &[u8], policy: &AsciiResourcePolicy) -> Result<String> {
        let model: PieDiagramModel = serde_json::from_slice(bytes).unwrap();
        let execution = AsciiExecution::for_test(policy);
        let mut lines = SemanticFallbackWriter::new(execution, 80, TerminalWidthProfile::Unicode);
        write_if_pie(&AsciiDiagram::Pie(&model), bytes, &mut lines)?;
        Ok(lines.finish().into_text())
    }

    #[test]
    fn complete_pie_envelope_keeps_unknown_fields_raw_tokens_and_duplicate_order() {
        let output = render(ENVELOPE, &AsciiResourcePolicy::default()).unwrap();
        let compact = output.split_whitespace().collect::<String>();
        for value in [
            r#"futureSection:{"raw":"\ud800","number":1e+09}"#,
            r#"futureRoot:{"opaque":"\ud800","number":1e+09}"#,
            "sourceConfig:",
            "sourceTitle:",
        ] {
            assert!(compact.contains(value), "missing {value}:\n{output}");
        }
        assert!(output.contains(r#"title: "  exact \"quote\"\nnext  ""#));
        let first = output.find("P1 label=\"A\" value=1").unwrap();
        let second = output.find("P2 label=\"A\" value=-0").unwrap();
        assert!(first < second);
    }

    #[test]
    fn complete_pie_record_budget_and_cancellation_remain_terminal() {
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
                assert!(
                    matches!(result, Err(AsciiError::ResourceLimitExceeded(details)) if details.limit == AsciiResourceLimitId::MaxOutputBytes)
                );
            }
        }
        let model: PieDiagramModel = serde_json::from_slice(ENVELOPE).unwrap();
        let control = OperationControl::new();
        control.cancel_after_checkpoints(0);
        let policy = AsciiResourcePolicy::default();
        let execution = AsciiExecution::new(&control, &policy);
        let mut lines = SemanticFallbackWriter::new(execution, 80, TerminalWidthProfile::Unicode);
        assert!(
            matches!(write_if_pie(&AsciiDiagram::Pie(&model), ENVELOPE, &mut lines),
            Err(AsciiError::Cancelled(cancelled)) if cancelled.phase == OperationPhase::Semantic)
        );
    }
}
