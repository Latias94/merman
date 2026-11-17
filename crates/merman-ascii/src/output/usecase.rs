//! Complete, keyed Usecase reading records over the one admitted semantic serialization.
//!
//! Authored labels are borrowed from the typed model. Additional fields borrow RawValue slices
//! from `bytes`, including defaults, accessibility, classes, styles, animation, and source config.

use super::record::{RecordPart, RecordWriter, details, write_plain_record, write_record};
use super::{AsciiDiagram, AsciiError, RawValue, Result, SemanticFallbackWriter};
use merman_model::families::usecase::{UsecaseArrowType, UsecaseNodeKind, UsecaseRelationshipType};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};

type Fields<'a> = BTreeMap<String, &'a RawValue>;

pub(super) fn write_if_usecase(
    model: &AsciiDiagram<'_>,
    bytes: &[u8],
    lines: &mut SemanticFallbackWriter<'_>,
) -> Result<bool> {
    let AsciiDiagram::Usecase(model) = model else {
        return Ok(false);
    };
    if !model.json_nodes.is_empty() {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "usecase",
            feature: "JSON table nodes",
        });
    }
    lines.semantic_resources.charge_layout_work(bytes.len())?;
    let raw: &RawValue = serde_json::from_slice(bytes).map_err(|_| invalid_projection())?;
    let mut root: Fields<'_> = decode(raw, lines)?;
    let nodes = take_array(&mut root, "nodes", model.nodes.len(), lines)?;
    let boundaries = take_array(&mut root, "boundaries", model.boundaries.len(), lines)?;
    let relationships = take_array(&mut root, "relationships", model.relationships.len(), lines)?;
    let notes = take_array(&mut root, "notes", model.notes.len(), lines)?;

    let mut node_keys = HashMap::new();
    let mut boundary_keys = HashMap::new();
    lines.semantic_resources.charge_layout_work(
        lines
            .semantic_resources
            .checked_work_add(model.nodes.len(), model.boundaries.len())?,
    )?;
    node_keys
        .try_reserve(model.nodes.len())
        .map_err(|_| allocation_failed())?;
    boundary_keys
        .try_reserve(model.boundaries.len())
        .map_err(|_| allocation_failed())?;
    for (index, node) in model.nodes.iter().enumerate() {
        lines
            .semantic_resources
            .charge_layout_work(node.id.len().max(1))?;
        if node_keys.insert(node.id.as_str(), index + 1).is_some() {
            return Err(invalid_projection());
        }
    }
    for (index, boundary) in model.boundaries.iter().enumerate() {
        lines
            .semantic_resources
            .charge_layout_work(boundary.id.len().max(1))?;
        if boundary_keys
            .insert(boundary.id.as_str(), index + 1)
            .is_some()
        {
            return Err(invalid_projection());
        }
    }
    write_record(
        &[
            RecordPart::Plain("direction: "),
            RecordPart::Quoted(&model.direction),
        ],
        lines,
    )?;
    root.remove("direction");
    write_plain_record(
        "Keys: N nodes; G boundaries; E relationships (written source, then target); T notes.",
        lines,
    )?;
    for (index, (node, raw)) in model.nodes.iter().zip(nodes).enumerate() {
        let key = format!("N{}", index + 1);
        let role = match node.kind {
            UsecaseNodeKind::Actor => "actor",
            UsecaseNodeKind::UseCaseEllipse => "use-case ellipse",
            UsecaseNodeKind::UseCaseRect => "use-case rectangle",
        };
        let mut fields: Fields<'_> = decode(raw, lines)?;
        write_record(
            &[
                RecordPart::Plain(&format!("{key} [{role}] id=")),
                RecordPart::Quoted(&node.id),
                RecordPart::Plain(" label="),
                RecordPart::Quoted(&node.label),
            ],
            lines,
        )?;
        for field in ["id", "label", "kind"] {
            fields.remove(field);
        }
        if let Some(parent) = node.parent_id.as_deref() {
            let parent = boundary_keys.get(parent).ok_or_else(invalid_projection)?;
            write_plain_record(&format!("{key} parent: G{parent}"), lines)?;
        }
        // Keep parentId alongside its readable key, so authored spellings remain explicit.
        details(&key, &fields, lines)?;
    }
    for (index, (boundary, raw)) in model.boundaries.iter().zip(boundaries).enumerate() {
        let key = format!("G{}", index + 1);
        let mut fields: Fields<'_> = decode(raw, lines)?;
        write_record(
            &[
                RecordPart::Plain(&format!(
                    "{key} [{}] id=",
                    if boundary.package {
                        "package"
                    } else {
                        "boundary"
                    }
                )),
                RecordPart::Quoted(&boundary.id),
                RecordPart::Plain(" label="),
                RecordPart::Quoted(&boundary.label),
            ],
            lines,
        )?;
        for field in ["id", "label"] {
            fields.remove(field);
        }
        let mut record = RecordWriter::new(lines)?;
        record.text(&format!("{key} members: ["), false)?;
        for (member_index, member) in boundary.members.iter().enumerate() {
            if member_index > 0 {
                record.text(", ", false)?;
            }
            let member = node_keys
                .get(member.as_str())
                .ok_or_else(invalid_projection)?;
            record.text(&format!("N{member}"), false)?;
        }
        record.text("]", false)?;
        record.finish()?;
        details(&key, &fields, lines)?;
    }
    for (index, (relationship, raw)) in model.relationships.iter().zip(relationships).enumerate() {
        let key = format!("E{}", index + 1);
        let mut fields: Fields<'_> = decode(raw, lines)?;
        let source = node_keys
            .get(relationship.source.as_str())
            .ok_or_else(invalid_projection)?;
        let target = node_keys
            .get(relationship.target.as_str())
            .ok_or_else(invalid_projection)?;
        let kind = match relationship.relationship_type {
            UsecaseRelationshipType::Association => "association",
            UsecaseRelationshipType::Include => "include",
            UsecaseRelationshipType::Extend => "extend",
            UsecaseRelationshipType::Generalization => "generalization",
        };
        let (start, end) = marker_names(relationship.arrow_type);
        write_record(
            &[
                RecordPart::Plain(&format!(
                    "{key}: N{source} [{start}] --{} {kind}-- N{target} [{end}]; id=",
                    if relationship.dotted {
                        "dotted"
                    } else {
                        "solid"
                    }
                )),
                RecordPart::Quoted(&relationship.id),
            ],
            lines,
        )?;
        for field in ["id", "source", "target"] {
            fields.remove(field);
        }
        if let Some(label) = relationship.label.as_deref() {
            write_record(
                &[
                    RecordPart::Plain(&format!("{key} label: ")),
                    RecordPart::Quoted(label),
                ],
                lines,
            )?;
            fields.remove("label");
        }
        // Enum values and every non-label relationship property remain complete details.
        details(&key, &fields, lines)?;
    }
    for (index, (note, raw)) in model.notes.iter().zip(notes).enumerate() {
        let key = format!("T{}", index + 1);
        let target = node_keys
            .get(note.target.as_str())
            .ok_or_else(invalid_projection)?;
        let mut fields: Fields<'_> = decode(raw, lines)?;
        write_record(
            &[
                RecordPart::Plain(&format!("{key} note for N{target}; id=")),
                RecordPart::Quoted(&note.id),
                RecordPart::Plain(" label="),
                RecordPart::Quoted(&note.label),
            ],
            lines,
        )?;
        for field in ["id", "target", "label"] {
            fields.remove(field);
        }
        details(&key, &fields, lines)?;
    }
    // Root fields include title/accessibility, class_defs, sourceConfig/sourceTitle and the
    // explicit empty JSON table collection. No authored field is omitted as an assumed default.
    details("usecase", &root, lines)?;
    Ok(true)
}

fn marker_names(arrow: UsecaseArrowType) -> (&'static str, &'static str) {
    match arrow {
        UsecaseArrowType::Point => ("open", "point"),
        UsecaseArrowType::ReversedPoint => ("point", "open"),
        UsecaseArrowType::Line => ("open", "open"),
        UsecaseArrowType::Circle => ("open", "circle"),
        UsecaseArrowType::Cross => ("open", "cross"),
        UsecaseArrowType::ReversedCircle => ("circle", "open"),
        UsecaseArrowType::ReversedCross => ("cross", "open"),
    }
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
    key: &str,
    expected: usize,
    lines: &SemanticFallbackWriter<'_>,
) -> Result<Vec<&'a RawValue>> {
    let values: Vec<_> = decode(fields.remove(key).ok_or_else(invalid_projection)?, lines)?;
    if values.len() != expected {
        return Err(invalid_projection());
    }
    Ok(values)
}

fn invalid_projection() -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "usecase",
        feature: "invalid serialized semantic projection",
    }
}

fn allocation_failed() -> AsciiError {
    AsciiError::allocation_failed("ascii_semantic_fallback")
}
