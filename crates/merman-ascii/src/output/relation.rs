//! Class/ER reading documents over the one admitted semantic serialization.
//!
//! Short keys identify declarations, never authored labels. Each relationship keeps both endpoint
//! semantics independently. Fields not used by the reading records remain complete borrowed JSON
//! details with their owner, including opaque metadata. Known defaults, duplicate labels, native
//! generated ER IDs, and Class's derived topology cache are omitted; custom vocabulary tables,
//! nondefault properties, and authored source configuration remain disclosed.

use super::record::{RecordPart, RecordWriter, details, write_plain_record, write_record};
use super::{AsciiDiagram, AsciiError, RawValue, Result, SemanticFallbackWriter};
use serde::Deserialize;
use std::collections::BTreeMap;

type Fields<'a> = BTreeMap<String, &'a RawValue>;
type Identities<'a> = BTreeMap<&'a str, &'a str>;

struct Entity<'a> {
    id: String,
    model_id: String,
    key: String,
    raw: &'a RawValue,
}

pub(super) fn write_if_relation(
    model: &AsciiDiagram<'_>,
    bytes: &[u8],
    lines: &mut SemanticFallbackWriter<'_>,
) -> Result<bool> {
    match model {
        #[cfg(feature = "diagram-class")]
        AsciiDiagram::Class(class) => write_class(class, bytes, lines)?,
        #[cfg(feature = "diagram-er")]
        AsciiDiagram::Er(_) => write_er(bytes, lines)?,
        _ => return Ok(false),
    }
    Ok(true)
}

#[cfg(feature = "diagram-class")]
fn write_class(
    model: &merman_model::families::class::ClassTopologyView<'_>,
    bytes: &[u8],
    lines: &mut SemanticFallbackWriter<'_>,
) -> Result<()> {
    use merman_model::families::class::{ClassConstants, ClassStylePrecedenceFacts};

    let mut root = root_fields(bytes, lines)?;
    let constants: ClassConstants = decode(
        root.get("constants")
            .copied()
            .ok_or_else(invalid_projection)?,
        lines,
    )?;
    if constants == default_class_constants() {
        root.remove("constants");
    }
    // ClassTopologyView serializes the authored model plus derived routing/DOM identities,
    // effective namespace containers, and ownership. The reading document uses authored
    // namespaces and members directly; reproducing that layout cache would duplicate them.
    root.remove("topology");
    if let Some(raw) = root.get("stylePrecedenceFacts")
        && decode::<ClassStylePrecedenceFacts>(raw, lines)? == ClassStylePrecedenceFacts::default()
    {
        root.remove("stylePrecedenceFacts");
    }
    let classes = map_entities(&mut root, "classes", "N", lines)?;
    let namespaces = map_entities(&mut root, "namespaces", "G", lines)?;
    let interfaces = list_entities(&mut root, "interfaces", "I", lines)?;
    let notes = list_entities(&mut root, "notes", "T", lines)?;
    let aliases = &model.model().namespace_facade_aliases;
    // Namespace members name declaration map keys; primary endpoints name ClassNode.id.
    // Keep aliases out of the routing table: primary resolution follows exactly one alias.
    let class_keys = identities(classes.iter());
    let node_ids = model_identities(classes.iter());
    let group_ids = identities(namespaces.iter());
    // Reuse the admitted topology's actual routing identities. Reconstructing its remap policy
    // from authored IDs would disagree for duplicates or collisions with generated IDs.
    let interface_ids: Identities<'_> = model
        .interfaces()
        .zip(&interfaces)
        .map(|(projected, interface)| (projected.id, interface.key.as_str()))
        .collect();
    let note_ids = identities(notes.iter());
    let mut endpoints = node_ids.clone();
    for (&id, &key) in &interface_ids {
        endpoints.entry(id).or_insert(key);
    }
    for namespace in &namespaces {
        endpoints
            .entry(namespace.id.as_str())
            .or_insert(&namespace.key);
    }

    write_header(&mut root, lines)?;
    let mut keys = String::from("Keys: N classes");
    for (present, legend) in [
        (!namespaces.is_empty(), "; G namespaces"),
        (!interfaces.is_empty(), "; I interfaces"),
        (!notes.is_empty(), "; T notes"),
    ] {
        if present {
            keys.push_str(legend);
        }
    }
    keys.push_str("; E relations.");
    write_plain_record(&keys, lines)?;
    for namespace in &namespaces {
        let mut fields = declaration(namespace, "namespace", lines)?;
        omit_same_text(&mut fields, "label", &namespace.id, lines)?;
        quoted_field(&namespace.key, "label", &mut fields, "label", lines)?;
        reference_field(
            &namespace.key,
            "parent",
            &mut fields,
            "parent",
            &group_ids,
            lines,
        )?;
        reference_list(
            &namespace.key,
            "classes",
            &mut fields,
            "classIds",
            &class_keys,
            lines,
        )?;
        reference_list(
            &namespace.key,
            "notes",
            &mut fields,
            "noteIds",
            &note_ids,
            lines,
        )?;
        omit_values(&mut fields, "parent", &["null"]);
        // Namespace's explicit bit is authored provenance and remains disclosed even when false.
        details(&namespace.key, &fields, lines)?;
    }
    for class in &classes {
        let alias = aliases.contains_key(&class.model_id);
        let mut fields = declaration(class, if alias { "class alias" } else { "class" }, lines)?;
        if alias {
            let mut record = RecordWriter::new(lines)?;
            record.text(&format!("{} alias-of: ", class.key), false)?;
            class_reference(&class.model_id, &node_ids, aliases, &mut record)?;
            record.finish()?;
        }
        let display = text(&fields, "text", lines)?;
        if let Some(display) = &display {
            omit_same_text(&mut fields, "label", display, lines)?;
        }
        omit_same_text(&mut fields, "label", &class.id, lines)?;
        omit_same_text(&mut fields, "text", &class.id, lines)?;
        quoted_field(&class.key, "label", &mut fields, "text", lines)?;
        reference_field(
            &class.key,
            "namespace",
            &mut fields,
            "parent",
            &group_ids,
            lines,
        )?;
        for (field, kind) in [("members", "member"), ("methods", "method")] {
            for (index, raw) in take_array(&mut fields, field, lines)?
                .into_iter()
                .enumerate()
            {
                let mut member: Fields<'_> = decode(raw, lines)?;
                let scope = format!("{} {kind} {}", class.key, index + 1);
                quoted_field(&scope, "", &mut member, "displayText", lines)?;
                omit_empty(
                    &mut member,
                    &[
                        "classifier",
                        "cssStyle",
                        "parameters",
                        "returnType",
                        "visibility",
                    ],
                );
                omit_same_text(
                    &mut member,
                    "memberType",
                    if kind == "member" {
                        "attribute"
                    } else {
                        "method"
                    },
                    lines,
                )?;
                details(&scope, &member, lines)?;
            }
        }
        for (index, raw) in take_array(&mut fields, "annotations", lines)?
            .into_iter()
            .enumerate()
        {
            let annotation: String = decode(raw, lines)?;
            quoted(
                &format!("{} annotation {}", class.key, index + 1),
                &annotation,
                lines,
            )?;
        }
        omit_empty(&mut fields, &["type", "styles"]);
        for field in ["parent", "link", "linkTarget", "tooltip"] {
            omit_values(&mut fields, field, &["null"]);
        }
        omit_values(&mut fields, "callback", &["null"]);
        omit_values(&mut fields, "haveCallback", &["false"]);
        omit_values(&mut fields, "callbackEffective", &["false"]);
        omit_values(&mut fields, "cssClasses", &["\"\"", "\"default\""]);
        details(&class.key, &fields, lines)?;
    }
    for interface in &interfaces {
        let mut fields = declaration(interface, "interface", lines)?;
        quoted_field(&interface.key, "label", &mut fields, "label", lines)?;
        class_target(
            &interface.key,
            &mut fields,
            "classId",
            &node_ids,
            aliases,
            lines,
        )?;
        details(&interface.key, &fields, lines)?;
    }
    for note in &notes {
        let mut fields = declaration(note, "note", lines)?;
        quoted_field(&note.key, "text", &mut fields, "text", lines)?;
        class_target(&note.key, &mut fields, "class", &endpoints, aliases, lines)?;
        reference_field(
            &note.key,
            "namespace",
            &mut fields,
            "parent",
            &group_ids,
            lines,
        )?;
        omit_values(&mut fields, "class", &["null"]);
        omit_values(&mut fields, "parent", &["null"]);
        details(&note.key, &fields, lines)?;
    }
    let relations = take_array(&mut root, "relations", lines)?;
    if !relations.is_empty() {
        write_plain_record("Relationships (first endpoint, then second):", lines)?;
    }
    for (index, (raw, projected)) in relations.into_iter().zip(model.relations()).enumerate() {
        let mut fields: Fields<'_> = decode(raw, lines)?;
        let key = format!("E{}", index + 1);
        let source = required_text(&mut fields, "id1", lines)?;
        let target = required_text(&mut fields, "id2", lines)?;
        let mut shape: Fields<'_> = take_object(&mut fields, "relation", lines)?;
        let type1: i32 = take_required(&mut shape, "type1", lines)?;
        let type2: i32 = take_required(&mut shape, "type2", lines)?;
        let line_type: i32 = take_required(&mut shape, "lineType", lines)?;
        let marker1 = class_marker(type1, &constants);
        let marker2 = class_marker(type2, &constants);
        let stroke = match line_type {
            value if value == constants.line_type.line => "solid".into(),
            value if value == constants.line_type.dotted_line => "dotted".into(),
            value => format!("lineType={value}"),
        };
        // ClassTopologyView has already remapped lollipop endpoints into the interface routing
        // domain. Resolve the projected ID, including direct interface references, as primary does.
        let mut record = RecordWriter::new(lines)?;
        record.text(&format!("{key}: "), false)?;
        class_reference(projected.id1, &endpoints, aliases, &mut record)?;
        record.text(&format!(" [{marker1}] --{stroke}-- "), false)?;
        class_reference(projected.id2, &endpoints, aliases, &mut record)?;
        record.text(&format!(" [{marker2}]"), false)?;
        record.finish()?;
        for (side, id, marker) in [("source", &source, type1), ("target", &target, type2)] {
            // Interface routing IDs may also be referenced directly. Keep their authored
            // spelling independently of which declaration the admitted topology selected.
            if marker == constants.relation_type.lollipop || aliases.contains_key(id) {
                quoted(&format!("{key} {side}-ref"), id, lines)?;
            }
        }
        omit_values(&mut fields, "relationTitle1", &["null"]);
        omit_values(&mut fields, "relationTitle2", &["null"]);
        omit_values(&mut fields, "title", &["\"\""]);
        omit_same_text(&mut fields, "id", &index.to_string(), lines)?;
        quoted_field(&key, "source-label", &mut fields, "relationTitle1", lines)?;
        quoted_field(&key, "target-label", &mut fields, "relationTitle2", lines)?;
        quoted_field(&key, "label", &mut fields, "title", lines)?;
        details(&format!("{key} shape"), &shape, lines)?;
        details(&key, &fields, lines)?;
    }
    omit_empty(&mut root, &["namespaceFacadeAliases", "styleClasses"]);
    for field in ["accTitle", "accDescr"] {
        omit_values(&mut root, field, &["null"]);
    }
    omit_values(
        &mut root,
        "type",
        &["\"class\"", "\"classDiagram\"", "\"classDiagram-v2\""],
    );
    details("graph", &root, lines)
}

#[cfg(feature = "diagram-class")]
fn default_class_constants() -> merman_model::families::class::ClassConstants {
    use merman_model::families::class::{
        ClassConstants, ClassLineTypeConstants, ClassRelationTypeConstants, LINE_DOTTED,
        LINE_SOLID, REL_AGGREGATION, REL_COMPOSITION, REL_DEPENDENCY, REL_EXTENSION, REL_LOLLIPOP,
        REL_NONE,
    };
    ClassConstants {
        line_type: ClassLineTypeConstants {
            line: LINE_SOLID,
            dotted_line: LINE_DOTTED,
        },
        relation_type: ClassRelationTypeConstants {
            none: REL_NONE,
            aggregation: REL_AGGREGATION,
            extension: REL_EXTENSION,
            composition: REL_COMPOSITION,
            dependency: REL_DEPENDENCY,
            lollipop: REL_LOLLIPOP,
        },
    }
}

#[cfg(feature = "diagram-class")]
fn class_marker(marker: i32, constants: &merman_model::families::class::ClassConstants) -> String {
    let constants = &constants.relation_type;
    match marker {
        value if value == constants.none => "none".into(),
        value if value == constants.extension => "inheritance".into(),
        value if value == constants.dependency => "arrow".into(),
        value if value == constants.aggregation => "aggregation".into(),
        value if value == constants.composition => "composition".into(),
        value if value == constants.lollipop => "lollipop".into(),
        value => format!("type={value}"),
    }
}

#[cfg(feature = "diagram-class")]
fn class_target(
    scope: &str,
    fields: &mut Fields<'_>,
    field: &str,
    identities: &Identities<'_>,
    aliases: &BTreeMap<String, String>,
    lines: &mut SemanticFallbackWriter<'_>,
) -> Result<()> {
    if let Some(id) = take_text(fields, field, lines)? {
        let mut record = RecordWriter::new(lines)?;
        record.text(&format!("{scope} class: "), false)?;
        class_reference(&id, identities, aliases, &mut record)?;
        record.finish()?;
        if aliases.contains_key(&id) {
            quoted(&format!("{scope} class-ref"), &id, lines)?;
        }
    }
    Ok(())
}

#[cfg(feature = "diagram-class")]
fn class_reference(
    id: &str,
    identities: &Identities<'_>,
    aliases: &BTreeMap<String, String>,
    record: &mut RecordWriter<'_, '_>,
) -> Result<()> {
    let resolved = aliases.get(id).map(String::as_str).unwrap_or(id);
    reference(resolved, identities, record)
}

#[cfg(feature = "diagram-er")]
fn write_er(bytes: &[u8], lines: &mut SemanticFallbackWriter<'_>) -> Result<()> {
    let mut root = root_fields(bytes, lines)?;
    let entities = map_entities(&mut root, "entities", "N", lines)?;
    let groups = list_entities(&mut root, "subgraphs", "G", lines)?;
    let mut members = Identities::new();
    for group in &groups {
        lines
            .semantic_resources
            .charge_layout_work(group.id.len().max(1))?;
        members.insert(group.id.as_str(), group.key.as_str());
    }
    let group_ids = members.clone();
    // Membership names authored declarations; relationships name routing IDs. A group spelling
    // can also have an entity shadow created by Jison, which is never a visible entity endpoint.
    let mut endpoints = if groups.is_empty() {
        model_identities(entities.iter())
    } else {
        model_identities(groups.iter())
    };
    if !groups.is_empty() {
        for entity in &entities {
            lines
                .semantic_resources
                .charge_layout_work(entity.id.len().max(1))?;
            if !group_ids.contains_key(entity.id.as_str()) {
                members.insert(entity.id.as_str(), entity.key.as_str());
                endpoints.insert(entity.model_id.as_str(), entity.key.as_str());
            }
        }
    }
    write_header(&mut root, lines)?;
    write_plain_record(
        if groups.is_empty() {
            "Keys: N entities; E relations."
        } else {
            "Keys: N entities; G groups; E relations."
        },
        lines,
    )?;
    for group in &groups {
        let mut fields = declaration(group, "group", lines)?;
        quoted_field(&group.key, "title", &mut fields, "title", lines)?;
        reference_list(&group.key, "members", &mut fields, "nodes", &members, lines)?;
        details(&group.key, &fields, lines)?;
    }
    for entity in &entities {
        if let Some(group_key) = group_ids.get(entity.id.as_str()) {
            let fields = declaration(entity, "shadow entity", lines)?;
            write_record(
                &[RecordPart::Plain(&format!(
                    "{} shadow-of: {group_key}",
                    entity.key
                ))],
                lines,
            )?;
            details(&entity.key, &fields, lines)?;
            continue;
        }
        let mut fields = declaration(entity, "entity", lines)?;
        omit_same_text(&mut fields, "label", &entity.id, lines)?;
        omit_values(&mut fields, "alias", &["\"\""]);
        quoted_field(&entity.key, "label", &mut fields, "label", lines)?;
        quoted_field(&entity.key, "alias", &mut fields, "alias", lines)?;
        for (index, raw) in take_array(&mut fields, "attributes", lines)?
            .into_iter()
            .enumerate()
        {
            let mut attribute: Fields<'_> = decode(raw, lines)?;
            let scope = format!("{} attribute {}", entity.key, index + 1);
            let name = required_text(&mut attribute, "name", lines)?;
            let kind = required_text(&mut attribute, "type", lines)?;
            write_record(
                &[
                    RecordPart::Plain(&format!("{scope}: ")),
                    RecordPart::Quoted(&kind),
                    RecordPart::Plain(" "),
                    RecordPart::Quoted(&name),
                ],
                lines,
            )?;
            omit_empty(&mut attribute, &["keys", "comment"]);
            details(&scope, &attribute, lines)?;
        }
        omit_values(&mut fields, "cssClasses", &["\"\"", "\"default\""]);
        omit_values(&mut fields, "shape", &["\"\"", "\"erBox\""]);
        omit_empty(&mut fields, &["cssStyles"]);
        // Native ER IDs are generated routing identities, not entity labels. Custom wire IDs
        // remain details, while every edge uses the stable authored entity declaration key.
        if entity
            .model_id
            .strip_prefix(&format!("entity-{}-", entity.id))
            .is_some_and(|ordinal| {
                !ordinal.is_empty() && ordinal.bytes().all(|byte| byte.is_ascii_digit())
            })
        {
            fields.remove("id");
        }
        details(&entity.key, &fields, lines)?;
    }
    let relations = take_array(&mut root, "relationships", lines)?;
    if !relations.is_empty() {
        write_plain_record("Relationships (first endpoint, then second):", lines)?;
        write_plain_record(
            "Stroke: solid = identifying; dotted = non-identifying.",
            lines,
        )?;
    }
    for (index, raw) in relations.into_iter().enumerate() {
        let mut fields: Fields<'_> = decode(raw, lines)?;
        let key = format!("E{}", index + 1);
        let source = required_text(&mut fields, "entityA", lines)?;
        let target = required_text(&mut fields, "entityB", lines)?;
        let mut shape = take_object(&mut fields, "relSpec", lines)?;
        // Mermaid's grammar stores the *left* cardinality in cardB, not cardA.
        let source_cardinality = required_text(&mut shape, "cardB", lines)?;
        let target_cardinality = required_text(&mut shape, "cardA", lines)?;
        let stroke = required_text(&mut shape, "relType", lines)?;
        let mut record = RecordWriter::new(lines)?;
        record.text(&format!("{key}: "), false)?;
        reference(&source, &endpoints, &mut record)?;
        record.text(" [", false)?;
        let (source_marker, source_quoted) = er_cardinality(&source_cardinality);
        record.text(source_marker, source_quoted)?;
        record.text("] --", false)?;
        match stroke.as_str() {
            "IDENTIFYING" => record.text("solid", false)?,
            "NON_IDENTIFYING" => record.text("dotted", false)?,
            _ => record.text(&stroke, true)?,
        }
        record.text("-- ", false)?;
        reference(&target, &endpoints, &mut record)?;
        record.text(" [", false)?;
        let (target_marker, target_quoted) = er_cardinality(&target_cardinality);
        record.text(target_marker, target_quoted)?;
        record.text("]", false)?;
        record.finish()?;
        omit_values(&mut fields, "roleA", &["\"\""]);
        quoted_field(&key, "label", &mut fields, "roleA", lines)?;
        details(&format!("{key} shape"), &shape, lines)?;
        details(&key, &fields, lines)?;
    }
    omit_empty(&mut root, &["classes"]);
    for field in ["accTitle", "accDescr"] {
        omit_values(&mut root, field, &["null"]);
    }
    omit_values(&mut root, "type", &["\"er\""]);
    details("graph", &root, lines)
}

#[cfg(feature = "diagram-er")]
fn er_cardinality(value: &str) -> (&str, bool) {
    match value {
        "ONLY_ONE" => ("1", false),
        "ZERO_OR_ONE" => ("0..1", false),
        "ZERO_OR_MORE" => ("0..*", false),
        "ONE_OR_MORE" => ("1..*", false),
        "MD_PARENT" => ("parent", false),
        _ => (value, true),
    }
}

fn write_header(root: &mut Fields<'_>, lines: &mut SemanticFallbackWriter<'_>) -> Result<()> {
    if let Some(direction) = take_text(root, "direction", lines)? {
        quoted("direction", &direction, lines)?;
    }
    omit_values(root, "sourceTitle", &["null"]);
    quoted_field("title", "", root, "sourceTitle", lines)
}

fn root_fields<'a>(bytes: &'a [u8], lines: &SemanticFallbackWriter<'_>) -> Result<Fields<'a>> {
    let raw: &RawValue = serde_json::from_slice(bytes).map_err(|_| invalid_projection())?;
    decode(raw, lines)
}

#[cfg(feature = "diagram-class")]
fn identities<'a>(entities: impl Iterator<Item = &'a Entity<'a>>) -> Identities<'a> {
    let mut identities = Identities::new();
    for entity in entities {
        // Keep the first declaration as endpoint owner when detached IDs collide.
        identities.entry(entity.id.as_str()).or_insert(&entity.key);
    }
    identities
}

fn model_identities<'a>(entities: impl Iterator<Item = &'a Entity<'a>>) -> Identities<'a> {
    let mut identities = Identities::new();
    for entity in entities {
        identities
            .entry(entity.model_id.as_str())
            .or_insert(&entity.key);
    }
    identities
}

fn map_entities<'a>(
    root: &mut Fields<'a>,
    field: &str,
    prefix: &str,
    lines: &SemanticFallbackWriter<'_>,
) -> Result<Vec<Entity<'a>>> {
    let values = take_object(root, field, lines)?;
    let mut entities = Vec::new();
    for (index, (id, raw)) in values.into_iter().enumerate() {
        let mut fields: Fields<'_> = decode(raw, lines)?;
        let model_id = required_text(&mut fields, "id", lines)?;
        entities.push(Entity {
            id,
            model_id,
            key: format!("{prefix}{}", index + 1),
            raw,
        });
    }
    Ok(entities)
}

fn list_entities<'a>(
    root: &mut Fields<'a>,
    field: &str,
    prefix: &str,
    lines: &SemanticFallbackWriter<'_>,
) -> Result<Vec<Entity<'a>>> {
    let mut entities = Vec::new();
    for (index, raw) in take_array(root, field, lines)?.into_iter().enumerate() {
        let mut fields: Fields<'_> = decode(raw, lines)?;
        let id = required_text(&mut fields, "id", lines)?;
        entities.push(Entity {
            model_id: id.clone(),
            id,
            key: format!("{prefix}{}", index + 1),
            raw,
        });
    }
    Ok(entities)
}

fn declaration<'a>(
    entity: &Entity<'a>,
    kind: &str,
    lines: &mut SemanticFallbackWriter<'_>,
) -> Result<Fields<'a>> {
    let mut fields: Fields<'_> = decode(entity.raw, lines)?;
    quoted(&format!("{} [{kind}]", entity.key), &entity.id, lines)?;
    // Map identity and the model's internal id can differ (notably ER generated IDs).
    // Only elide the id when the declaration already carries its exact value.
    if entity.model_id == entity.id {
        fields.remove("id");
    }
    Ok(fields)
}

fn quoted(scope: &str, text: &str, lines: &mut SemanticFallbackWriter<'_>) -> Result<()> {
    write_record(
        &[
            RecordPart::Plain(&format!("{scope}: ")),
            RecordPart::Quoted(text),
        ],
        lines,
    )
}

fn quoted_field(
    scope: &str,
    label: &str,
    fields: &mut Fields<'_>,
    field: &str,
    lines: &mut SemanticFallbackWriter<'_>,
) -> Result<()> {
    if let Some(value) = take_text(fields, field, lines)? {
        let prefix = if label.is_empty() {
            scope.to_owned()
        } else {
            format!("{scope} {label}")
        };
        quoted(&prefix, &value, lines)?;
    }
    Ok(())
}

fn reference(
    id: &str,
    identities: &Identities<'_>,
    record: &mut RecordWriter<'_, '_>,
) -> Result<()> {
    if let Some(key) = identities.get(id) {
        record.text(key, false)
    } else {
        // Preserve an unresolved authored endpoint rather than truncating or inventing a node.
        record.text(id, true)
    }
}

#[cfg(feature = "diagram-class")]
fn reference_field(
    scope: &str,
    label: &str,
    fields: &mut Fields<'_>,
    field: &str,
    identities: &Identities<'_>,
    lines: &mut SemanticFallbackWriter<'_>,
) -> Result<()> {
    if let Some(id) = take_text(fields, field, lines)? {
        let mut record = RecordWriter::new(lines)?;
        record.text(&format!("{scope} {label}: "), false)?;
        reference(&id, identities, &mut record)?;
        record.finish()?;
    }
    Ok(())
}

fn reference_list(
    scope: &str,
    label: &str,
    fields: &mut Fields<'_>,
    field: &str,
    identities: &Identities<'_>,
    lines: &mut SemanticFallbackWriter<'_>,
) -> Result<()> {
    if !fields.contains_key(field) {
        return Ok(());
    }
    let values = take_array(fields, field, lines)?
        .into_iter()
        .map(|raw| decode::<String>(raw, lines))
        .collect::<Result<Vec<_>>>()?;
    if values.is_empty() {
        return Ok(());
    }
    let mut record = RecordWriter::new(lines)?;
    record.text(&format!("{scope} {label}: ["), false)?;
    for (index, id) in values.into_iter().enumerate() {
        if index > 0 {
            record.text(", ", false)?;
        }
        reference(&id, identities, &mut record)?;
    }
    record.text("]", false)?;
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

#[cfg(feature = "diagram-class")]
fn take_required<'a, T: Deserialize<'a>>(
    fields: &mut Fields<'a>,
    field: &str,
    lines: &SemanticFallbackWriter<'_>,
) -> Result<T> {
    decode(fields.remove(field).ok_or_else(invalid_projection)?, lines)
}

fn take_object<'a>(
    fields: &mut Fields<'a>,
    field: &str,
    lines: &SemanticFallbackWriter<'_>,
) -> Result<Fields<'a>> {
    fields
        .remove(field)
        .map_or_else(|| Ok(Fields::new()), |raw| decode(raw, lines))
}

fn take_array<'a>(
    fields: &mut Fields<'a>,
    field: &str,
    lines: &SemanticFallbackWriter<'_>,
) -> Result<Vec<&'a RawValue>> {
    fields
        .remove(field)
        .map_or_else(|| Ok(Vec::new()), |raw| decode(raw, lines))
}

fn text(
    fields: &Fields<'_>,
    field: &str,
    lines: &SemanticFallbackWriter<'_>,
) -> Result<Option<String>> {
    fields
        .get(field)
        .filter(|raw| raw.get() != "null")
        .map(|raw| decode(raw, lines))
        .transpose()
}

fn take_text(
    fields: &mut Fields<'_>,
    field: &str,
    lines: &SemanticFallbackWriter<'_>,
) -> Result<Option<String>> {
    // A retained null is not a string record; keep it with the owner's remaining details.
    let text = text(fields, field, lines)?;
    if text.is_some() {
        fields.remove(field);
    }
    Ok(text)
}

fn required_text(
    fields: &mut Fields<'_>,
    field: &str,
    lines: &SemanticFallbackWriter<'_>,
) -> Result<String> {
    take_text(fields, field, lines)?.ok_or_else(invalid_projection)
}

fn omit_values(fields: &mut Fields<'_>, field: &str, defaults: &[&str]) {
    if fields
        .get(field)
        .is_some_and(|raw| defaults.contains(&raw.get()))
    {
        fields.remove(field);
    }
}

fn omit_empty(fields: &mut Fields<'_>, keys: &[&str]) {
    for key in keys {
        omit_values(fields, key, &["null", "\"\"", "[]", "{}"]);
    }
}

fn omit_same_text(
    fields: &mut Fields<'_>,
    field: &str,
    equivalent: &str,
    lines: &SemanticFallbackWriter<'_>,
) -> Result<()> {
    if text(fields, field, lines)?.as_deref() == Some(equivalent) {
        fields.remove(field);
    }
    Ok(())
}

fn invalid_projection() -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "ascii",
        feature: "invalid serialized relation projection",
    }
}

#[cfg(all(test, feature = "diagram-class"))]
mod tests {
    use super::*;
    use crate::operation::AsciiExecution;
    use crate::{AsciiResourcePolicy, TerminalWidthProfile};
    use merman_model::families::class::{ClassDiagramModel, ClassInterface, ClassNote};
    use merman_model::{MermaidConfig, OperationControl, SemanticDiagramModel};

    fn class_model(source: &str) -> ClassDiagramModel {
        let parsed = merman_core::SemanticParser::new()
            .parse_diagram_sync(source)
            .expect("the class identity fixture should parse");
        let SemanticDiagramModel::Class(model) = parsed.model else {
            panic!("expected a Class model");
        };
        model
    }

    fn reading_records(model: &ClassDiagramModel) -> Vec<String> {
        let topology = model
            .topology(&MermaidConfig::default(), &OperationControl::new())
            .expect("the class identity topology should build");
        let bytes = serde_json::to_vec(&topology).expect("the admitted topology should serialize");
        let policy = AsciiResourcePolicy::unbounded();
        let mut writer = SemanticFallbackWriter::new(
            AsciiExecution::for_test(&policy),
            256,
            TerminalWidthProfile::Unicode,
        );
        // Exercise the reading writer directly: primary namespace validation currently requires
        // map keys to equal ClassNode.id, independently of the reading document's routing policy.
        write_class(&topology, &bytes, &mut writer)
            .expect("the reading document should retain ids");
        writer.finish().text().lines().map(str::to_owned).collect()
    }

    #[test]
    fn class_reading_routes_swapped_model_ids_to_their_declaration_owners() {
        let mut model = class_model("classDiagram\ndirection LR\nA --> B\n");
        model.classes.get_mut("A").unwrap().id = "B".into();
        model.classes.get_mut("B").unwrap().id = "A".into();
        model.interfaces.push(ClassInterface {
            id: "Api".into(),
            label: "A API".into(),
            class_id: "A".into(),
        });
        model.notes.push(ClassNote {
            id: "fact".into(),
            text: "A fact".into(),
            class_id: Some("A".into()),
            parent: None,
        });

        let rows = reading_records(&model);
        // The boxes keep their authored A/B text. The unchanged relation A --> B must connect
        // the declaration under map B (model id A) to the declaration under map A (model id B).
        for expected in [
            "N1 [class]: \"A\"",
            "N1 details: {id:\"B\"}",
            "N2 [class]: \"B\"",
            "N2 details: {id:\"A\"}",
            "I1 class: N2",
            "T1 class: N2",
            "E1: N2 [none] --solid-- N1 [arrow]",
        ] {
            assert!(
                rows.iter().any(|row| row == expected),
                "{expected}: {rows:#?}"
            );
        }
    }

    #[test]
    fn class_reading_keeps_namespace_map_keys_separate_from_alias_route_owners() {
        let mut model = class_model(concat!(
            "classDiagram\ndirection LR\n",
            "namespace Domain {\n class A\n note for A \"owned note\"\n}\n",
            "class B\nB --> Domain.A\n",
        ));
        assert_eq!(model.namespace_facade_aliases["Domain.A"], "A");
        // Route B belongs to map A, the facade route Domain.A belongs to map B, and route A
        // belongs to map Domain.A. Namespace membership still refers to the map key A.
        model.classes.get_mut("A").unwrap().id = "B".into();
        model.classes.get_mut("B").unwrap().id = "Domain.A".into();
        model.classes.get_mut("Domain.A").unwrap().id = "A".into();
        model.notes[0].class_id = Some("Domain.A".into());
        model.interfaces.push(ClassInterface {
            id: "Api".into(),
            label: "Owned API".into(),
            class_id: "Domain.A".into(),
        });

        let rows = reading_records(&model);
        for expected in [
            "G1 classes: [N1]",
            "G1 notes: [T1]",
            "N1 [class]: \"A\"",
            "N1 namespace: G1",
            "N1 details: {id:\"B\"}",
            "N2 [class alias]: \"B\"",
            "N2 alias-of: N3",
            "N2 details: {id:\"Domain.A\"}",
            "N3 [class]: \"Domain.A\"",
            "N3 details: {id:\"A\"}",
            "I1 class: N3",
            "I1 class-ref: \"Domain.A\"",
            "T1 class: N3",
            "T1 class-ref: \"Domain.A\"",
            "E1: N1 [none] --solid-- N3 [arrow]",
            "E1 target-ref: \"Domain.A\"",
        ] {
            assert!(
                rows.iter().any(|row| row == expected),
                "{expected}: {rows:#?}"
            );
        }
    }
}
