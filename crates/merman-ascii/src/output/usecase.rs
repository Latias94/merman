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
    lines.semantic_resources.charge_layout_work(bytes.len())?;
    let raw: &RawValue = serde_json::from_slice(bytes).map_err(|_| invalid_projection())?;
    let mut root: Fields<'_> = decode(raw, lines)?;
    let nodes = take_array(&mut root, "nodes", model.nodes.len(), lines)?;
    let boundaries = take_array(&mut root, "boundaries", model.boundaries.len(), lines)?;
    let relationships = take_array(&mut root, "relationships", model.relationships.len(), lines)?;
    let notes = take_array(&mut root, "notes", model.notes.len(), lines)?;
    let json_nodes: Vec<&RawValue> = decode(
        root.get("json_nodes")
            .copied()
            .ok_or_else(invalid_projection)?,
        lines,
    )?;
    if json_nodes.len() != model.json_nodes.len() {
        return Err(invalid_projection());
    }
    // Retain the explicit empty root collection for existing no-JSON reading records.
    if !json_nodes.is_empty() {
        root.remove("json_nodes");
    }

    let mut entity_keys = HashMap::new();
    let mut boundary_keys = HashMap::new();
    let entity_count = lines
        .semantic_resources
        .checked_work_add(model.nodes.len(), model.json_nodes.len())?;
    lines.semantic_resources.charge_layout_work(
        lines
            .semantic_resources
            .checked_work_add(entity_count, model.boundaries.len())?,
    )?;
    entity_keys
        .try_reserve(entity_count)
        .map_err(|_| allocation_failed())?;
    boundary_keys
        .try_reserve(model.boundaries.len())
        .map_err(|_| allocation_failed())?;
    for (index, id) in model
        .nodes
        .iter()
        .map(|node| node.id.as_str())
        .chain(model.json_nodes.iter().map(|node| node.id.as_str()))
        .enumerate()
    {
        lines
            .semantic_resources
            .charge_layout_work(id.len().max(1))?;
        if entity_keys.insert(id, index + 1).is_some() {
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
    for (index, (node, raw)) in model.json_nodes.iter().zip(json_nodes).enumerate() {
        let key = format!("N{}", model.nodes.len() + index + 1);
        let mut fields: Fields<'_> = decode(raw, lines)?;
        write_record(
            &[
                RecordPart::Plain(&format!("{key} [JSON table] id=")),
                RecordPart::Quoted(&node.id),
            ],
            lines,
        )?;
        fields.remove("id");
        // Keep values, property order, UTF-16 encoding and non-finite metadata as raw JSON.
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
            let member = entity_keys
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
        let source = entity_keys
            .get(relationship.source.as_str())
            .ok_or_else(invalid_projection)?;
        let target = entity_keys
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
        let target = entity_keys
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation::AsciiExecution;
    use crate::{AsciiResourcePolicy, TerminalWidthProfile};
    use merman_model::families::usecase::UsecaseDiagramModel;

    const MODEL: &[u8] = br#"{
        "direction":"LR",
        "nodes":[{
            "id":"Owner","label":"Owner","labelType":"text","kind":"actor",
            "actorType":"normal","icon":null,"parentId":null,"business":false,
            "stereotype":null,"classes":[],"styles":[]
        }],
        "boundaries":[{
            "id":"System","label":"System","labelType":"text",
            "members":["Data\"quoted","Owner","Other"],"package":false,
            "classes":[],"styles":[]
        }],
        "relationships":[{
            "id":"edge","explicitId":false,"source":"Owner","target":"Data\"quoted",
            "relationshipType":"association","arrowType":0,"label":null,"labelType":null,
            "dotted":false,"minlen":1,"classes":[],"styles":[],"animate":false,"animation":null
        },{
            "id":"back","explicitId":false,"source":"Other","target":"Owner",
            "relationshipType":"association","arrowType":0,"label":null,"labelType":null,
            "dotted":false,"minlen":1,"classes":[],"styles":[],"animate":false,"animation":null
        }],
        "notes":[{"id":"note","target":"Other","label":"Details","labelType":"text"}],
        "json_nodes":[{
            "id":"Data\"quoted",
            "value":{"\"z\"":"\"\\ud800\"","\"a\"":null},
            "stringEncoding":"json-utf16",
            "propertyOrder":{"":["\"z\"","\"a\""]},
            "nonFiniteNumbers":{"/\"a\"":"positive"},
            "classes":["payload"],"styles":["fill:red"]
        },{
            "id":"Other","value":[],"propertyOrder":{},"classes":[],"styles":[]
        }],
        "class_defs":[],"title":null,"acc_title":null,"acc_description":null
    }"#;

    fn render(model: &UsecaseDiagramModel, bytes: &[u8], width: usize) -> Result<String> {
        let policy = AsciiResourcePolicy::unbounded();
        let execution = AsciiExecution::for_test(&policy);
        let mut lines =
            SemanticFallbackWriter::new(execution, width, TerminalWidthProfile::Unicode);
        assert!(write_if_usecase(
            &AsciiDiagram::Usecase(model),
            bytes,
            &mut lines
        )?);
        Ok(lines.finish().into_text())
    }

    #[test]
    fn json_table_keys_resolve_references_and_preserve_lossless_details_at_any_width() {
        let model = serde_json::from_slice(MODEL).unwrap();
        for width in [24, 4096] {
            let text = render(&model, MODEL, width).unwrap();
            assert!(text.lines().all(|line| line.len() <= width), "{text}");
            let collapsed = text.split_whitespace().collect::<String>();
            for expected in [
                r#"N1[actor]id="Owner""#,
                r#"N2[JSONtable]id="Data\"quoted""#,
                r#"N3[JSONtable]id="Other""#,
                "G1members:[N2,N1,N3]",
                "E1:N1[open]--solidassociation--N2[point]",
                "E2:N3[open]--solidassociation--N1[point]",
                "T1noteforN3",
                r#"value:{"\"z\"":"\"\\ud800\"","\"a\"":null}"#,
                r#"stringEncoding:"json-utf16""#,
                r#"propertyOrder:{"":["\"z\"","\"a\""]}"#,
                r#"nonFiniteNumbers:{"/\"a\"":"positive"}"#,
                r#"classes:["payload"]"#,
                r#"styles:["fill:red"]"#,
                "N3details:{classes:[],propertyOrder:{},styles:[],value:[]}",
            ] {
                assert!(collapsed.contains(expected), "missing {expected}:\n{text}");
            }
            assert!(!collapsed.contains("json_nodes:"), "{text}");
        }
    }

    #[test]
    fn json_table_projection_requires_an_exact_array_length() {
        let model = serde_json::from_slice(MODEL).unwrap();
        for length in [0, 1, 3] {
            let mut projection: serde_json::Value = serde_json::from_slice(MODEL).unwrap();
            let nodes = projection["json_nodes"].as_array_mut().unwrap();
            if length > nodes.len() {
                let extra = nodes[0].clone();
                nodes.push(extra);
            } else {
                nodes.truncate(length);
            }
            let bytes = serde_json::to_vec(&projection).unwrap();
            assert!(matches!(
                render(&model, &bytes, 80),
                Err(AsciiError::UnsupportedFeature {
                    diagram_type: "usecase",
                    feature: "invalid serialized semantic projection",
                })
            ));
        }
    }

    #[test]
    fn json_table_ids_cannot_shadow_other_entities() {
        for id in ["Owner", "Data\"quoted"] {
            let mut model: UsecaseDiagramModel = serde_json::from_slice(MODEL).unwrap();
            model.json_nodes[1].id = id.into();
            let bytes = serde_json::to_vec(&model).unwrap();
            assert!(matches!(
                render(&model, &bytes, 80),
                Err(AsciiError::UnsupportedFeature {
                    diagram_type: "usecase",
                    feature: "invalid serialized semantic projection",
                })
            ));
        }
    }

    #[test]
    fn empty_json_tables_keep_the_existing_root_record() {
        let bytes = br#"{
            "direction":"LR","nodes":[],"boundaries":[],"relationships":[],"notes":[],
            "json_nodes":[],"class_defs":[],"title":null,"acc_title":null,"acc_description":null
        }"#;
        let model = serde_json::from_slice(bytes).unwrap();
        assert_eq!(
            render(&model, bytes, 4096).unwrap(),
            concat!(
                "direction: \"LR\"\n",
                "Keys: N nodes; G boundaries; E relationships (written source, then target); T notes.\n",
                "usecase details: {acc_description:null,acc_title:null,class_defs:[],json_nodes:[],title:null}"
            )
        );
    }
}
