//! Exact Requirement declarations and typed relationships, independent of spatial admission.

use super::record::{RecordWriter, details, write_plain_record};
use super::{AsciiDiagram, AsciiError, RawValue, Result, SemanticFallbackWriter};
use crate::requirement::Identities;
use serde::Deserialize;
use std::collections::BTreeMap;

type Fields<'a> = BTreeMap<String, &'a RawValue>;

pub(super) fn write_if_requirement(
    model: &AsciiDiagram<'_>,
    bytes: &[u8],
    lines: &mut SemanticFallbackWriter<'_>,
) -> Result<bool> {
    let AsciiDiagram::Requirement(model) = model else {
        return Ok(false);
    };
    lines.admit_node(0)?;
    lines.semantic_resources.charge_layout_work(bytes.len())?;
    let mut fields: Fields<'_> = serde_json::from_slice(bytes).map_err(|_| invalid_projection())?;
    let requirements = take_array(&mut fields, "requirements", model.requirements.len(), lines)?;
    let elements = take_array(&mut fields, "elements", model.elements.len(), lines)?;
    let relationships = take_array(
        &mut fields,
        "relationships",
        model.relationships.len(),
        lines,
    )?;
    let identities = Identities::new(model, &lines.semantic_resources)?;
    let mut record = RecordWriter::new(lines)?;
    record.text("direction: ", false)?;
    record.text(&model.direction, true)?;
    record.finish()?;
    fields.remove("direction");
    write_plain_record(
        "Keys: R requirements; N elements; E relationships. Collections keep source order.",
        lines,
    )?;
    write_plain_record(
        "Contains: source circle, no target arrow. Other relations: dotted target arrow.",
        lines,
    )?;
    for (index, (node, raw)) in model.requirements.iter().zip(requirements).enumerate() {
        let key = format!("R{}", index + 1);
        let mut extra: Fields<'_> = decode(raw, lines)?;
        let mut record = RecordWriter::new(lines)?;
        record.text(&format!("{key} ["), false)?;
        record.text(&node.node_type, true)?;
        record.text("] name=", false)?;
        record.text(&node.name, true)?;
        record.text(" id=", false)?;
        record.text(&node.requirement_id, true)?;
        record.finish()?;
        let mut record = RecordWriter::new(lines)?;
        record.text(&format!("{key} text="), false)?;
        record.text(&node.text, true)?;
        record.text(" risk=", false)?;
        record.text(&node.risk, true)?;
        record.text(" verifyMethod=", false)?;
        record.text(&node.verify_method, true)?;
        record.finish()?;
        for field in [
            "name",
            "type",
            "requirementId",
            "text",
            "risk",
            "verifyMethod",
        ] {
            extra.remove(field);
        }
        details(&key, &extra, lines)?;
        disclose_identity(&node.name, &identities, lines)?;
    }
    for (index, (node, raw)) in model.elements.iter().zip(elements).enumerate() {
        let key = format!("N{}", index + 1);
        let mut extra: Fields<'_> = decode(raw, lines)?;
        let mut record = RecordWriter::new(lines)?;
        record.text(&format!("{key} [element] name="), false)?;
        record.text(&node.name, true)?;
        record.text(" type=", false)?;
        record.text(&node.element_type, true)?;
        record.text(" docRef=", false)?;
        record.text(&node.doc_ref, true)?;
        record.finish()?;
        for field in ["name", "type", "docRef"] {
            extra.remove(field);
        }
        details(&key, &extra, lines)?;
        disclose_identity(&node.name, &identities, lines)?;
    }
    for (index, (relation, raw)) in model.relationships.iter().zip(relationships).enumerate() {
        let key = format!("E{}", index + 1);
        let mut extra: Fields<'_> = decode(raw, lines)?;
        let resources = lines.semantic_resources.clone();
        let mut record = RecordWriter::new(lines)?;
        record.text(&format!("{key} ["), false)?;
        record.text(&relation.rel_type, true)?;
        record.text("] source=", false)?;
        reference(&relation.src, &identities, &resources, &mut record)?;
        record.text(" target=", false)?;
        reference(&relation.dst, &identities, &resources, &mut record)?;
        record.finish()?;
        for field in ["type", "src", "dst"] {
            extra.remove(field);
        }
        details(&key, &extra, lines)?;
    }
    // Classes, CSS, accessibility, source title/configuration, and future fields remain raw
    // borrowed JSON. The topology never claims browser styling or resolves a document link.
    details("requirement", &fields, lines)?;
    Ok(true)
}

fn reference(
    name: &str,
    identities: &Identities<'_>,
    resources: &crate::resource::ResourceContext,
    record: &mut RecordWriter<'_, '_>,
) -> Result<()> {
    let keys = identities.keys(name, resources)?;
    record.text("[", false)?;
    for (index, key) in keys.iter().enumerate() {
        if index > 0 {
            record.text(",", false)?;
        }
        record.text(&key.to_string(), false)?;
    }
    record.text("]", false)?;
    if keys.is_empty() {
        record.text(" (unresolved)", false)?;
    }
    if keys.len() > 1 {
        record.text(" (ambiguous)", false)?;
    }
    record.text(" name=", false)?;
    record.text(name, true)
}

fn disclose_identity(
    name: &str,
    identities: &Identities<'_>,
    lines: &mut SemanticFallbackWriter<'_>,
) -> Result<()> {
    let resources = lines.semantic_resources.clone();
    if identities.keys(name, &resources)?.len() < 2 {
        return Ok(());
    }
    let mut record = RecordWriter::new(lines)?;
    record.text("Identity diagnostic: ", false)?;
    reference(name, identities, &resources, &mut record)?;
    record.finish()
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

fn take_array<'a>(
    fields: &mut Fields<'a>,
    name: &str,
    expected: usize,
    lines: &SemanticFallbackWriter<'_>,
) -> Result<Vec<&'a RawValue>> {
    let values: Vec<_> = decode(fields.remove(name).ok_or_else(invalid_projection)?, lines)?;
    if values.len() != expected {
        return Err(invalid_projection());
    }
    Ok(values)
}

fn invalid_projection() -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "requirement",
        feature: "invalid serialized semantic projection",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation::AsciiExecution;
    use crate::{AsciiResourceLimitId, AsciiResourcePolicy, TerminalWidthProfile};
    use merman_model::families::requirement::RequirementDiagramModel;
    use merman_model::{OperationControl, OperationPhase};

    const ENVELOPE: &[u8] = br#"{
        "direction":"TB",
        "requirements":[{"name":"request","type":"Requirement","requirementId":"R-1",
            "text":"  exact \"quote\"\nnext  ","risk":"High","verifyMethod":"Test",
            "futureNode":{"raw":"\ud800","number":1e+09}}],
        "elements":[{"name":"suite","type":"test suite","docRef":"docs/spec",
            "futureElement":false}],
        "relationships":[{"type":"verifies","src":"suite","dst":"request",
            "futureEdge":[1,null,true]}],
        "sourceConfig":{"requirement":{"future":true}},"sourceTitle":"Title",
        "futureRoot":{"opaque":"\ud800","number":1e+09}
    }"#;

    fn render(bytes: &[u8], policy: &AsciiResourcePolicy) -> Result<String> {
        let model: RequirementDiagramModel = serde_json::from_slice(bytes).unwrap();
        let execution = AsciiExecution::for_test(policy);
        let mut lines = SemanticFallbackWriter::new(execution, 80, TerminalWidthProfile::Unicode);
        write_if_requirement(&AsciiDiagram::Requirement(&model), bytes, &mut lines)?;
        Ok(lines.finish().into_text())
    }

    #[test]
    fn complete_requirement_envelope_keeps_unknown_fields_and_raw_json_values() {
        let output = render(ENVELOPE, &AsciiResourcePolicy::default()).unwrap();
        let compact = output.split_whitespace().collect::<String>();
        for value in [
            r#"futureNode:{"raw":"\ud800","number":1e+09}"#,
            "futureElement:false",
            "futureEdge:[1,null,true]",
            r#"futureRoot:{"opaque":"\ud800","number":1e+09}"#,
            "sourceConfig:",
            "sourceTitle:",
        ] {
            // Quoted known fields use the terminal quoted-text boundary; opaque fields retain
            // their raw serialized spelling, including numeric tokens and UTF-16 escapes.
            assert!(compact.contains(value), "missing {value}:\n{output}");
        }
        assert!(
            output.contains(r#"text="  exact \"quote\"\nnext  ""#),
            "{output}"
        );
        assert!(
            compact.contains(r#"E1["verifies"]source=[N1]name="suite"target=[R1]name="request""#)
        );
    }

    #[test]
    fn complete_record_output_budget_and_cancellation_are_terminal() {
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
        let model: RequirementDiagramModel = serde_json::from_slice(ENVELOPE).unwrap();
        let control = OperationControl::new();
        control.cancel_after_checkpoints(0);
        let policy = AsciiResourcePolicy::default();
        let execution = AsciiExecution::new(&control, &policy);
        let mut lines = SemanticFallbackWriter::new(execution, 80, TerminalWidthProfile::Unicode);
        assert!(
            matches!(write_if_requirement(&AsciiDiagram::Requirement(&model), ENVELOPE, &mut lines),
            Err(AsciiError::Cancelled(cancelled)) if cancelled.phase == OperationPhase::Semantic)
        );
        assert!(lines.finish().into_text().is_empty());
    }
}
