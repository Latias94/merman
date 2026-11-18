//! Complete Architecture declarations without inferring coordinates, routing, or icon rendering.
//!
//! Reading keys follow each declaration-ordered collection. IDs retain every matching node and
//! group; duplicates never silently choose an endpoint or boundary. Opaque fields borrow the
//! admitted envelope, including edge indices, optional endpoint flags, and alignment members.

use super::record::{RecordWriter, details, write_plain_record};
use super::{AsciiDiagram, AsciiError, RawValue, Result, SemanticFallbackWriter};
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use merman_model::families::architecture::{ArchitectureDiagramModel, ArchitectureNodeKind};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};

type Fields<'a> = BTreeMap<String, &'a RawValue>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum IdentityKind {
    Node,
    Group,
}

impl IdentityKind {
    fn prefix(self) -> char {
        match self {
            Self::Node => 'N',
            Self::Group => 'G',
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Node => "node",
            Self::Group => "group",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct ReadingKey {
    kind: IdentityKind,
    index: usize,
}

struct Identities<'a> {
    by_id: HashMap<&'a str, Vec<ReadingKey>>,
    resources: ResourceContext,
}

pub(super) fn write_if_architecture(
    model: &AsciiDiagram<'_>,
    bytes: &[u8],
    lines: &mut SemanticFallbackWriter<'_>,
) -> Result<bool> {
    let AsciiDiagram::Architecture(model) = model else {
        return Ok(false);
    };
    lines.admit_node(0)?;
    lines.semantic_resources.charge_layout_work(bytes.len())?;
    lines
        .semantic_resources
        .check(AsciiResourceLimitId::MaxOutputBytes, bytes.len())?;
    let mut root: Fields<'_> = serde_json::from_slice(bytes).map_err(|_| invalid_projection())?;
    let nodes = take_array(&mut root, "nodes", model.nodes.len(), lines)?;
    let groups = take_array(&mut root, "groups", model.groups.len(), lines)?;
    let edges = take_array(&mut root, "edges", model.edges.len(), lines)?;
    let alignments = take_array(&mut root, "layoutHints", model.layout_hints.len(), lines)?;
    let identities = Identities::new(model, lines)?;

    for (name, text) in [
        ("title", model.title.as_deref()),
        ("accTitle", model.acc_title.as_deref()),
        ("accDescr", model.acc_descr.as_deref()),
    ] {
        let mut record = RecordWriter::new(lines)?;
        record.text(&format!("Architecture {name}="), false)?;
        optional_text(text, &mut record)?;
        record.finish()?;
        root.remove(name);
    }
    write_plain_record(
        "Keys: N nodes; G groups; E edges; A alignment hints. Each collection keeps declaration order. Duplicate IDs list all matching keys and are ambiguous. edgeIndices are zero-based model indices.",
        lines,
    )?;
    write_plain_record(
        "Edges keep authored lhs and rhs independently. L/R/T/B name endpoint sides. Into=true requests an arrowhead at that end; null/false do not. Group=true requests the node's immediate containing-group boundary. Alignment hints are declarations, not rendered geometry.",
        lines,
    )?;

    for (index, (node, raw)) in model.nodes.iter().zip(nodes).enumerate() {
        let key = format!("N{}", index + 1);
        let kind = match node.node_type {
            ArchitectureNodeKind::Service => "service",
            ArchitectureNodeKind::Junction => "junction",
        };
        let mut fields: Fields<'_> = decode(raw, lines)?;
        let mut record = RecordWriter::new(lines)?;
        record.text(&format!("{key} [{kind}] id="), false)?;
        record.text(&node.id, true)?;
        record.text(" title=", false)?;
        optional_text(node.title.as_deref(), &mut record)?;
        record.text(" icon=", false)?;
        optional_text(node.icon.as_deref(), &mut record)?;
        record.text(" iconText=", false)?;
        optional_text(node.icon_text.as_deref(), &mut record)?;
        write_parent(node.in_group.as_deref(), &identities, &mut record)?;
        record.finish()?;
        for name in ["id", "type", "title", "icon", "iconText"] {
            fields.remove(name);
        }
        let mut record = RecordWriter::new(lines)?;
        record.text(&format!("{key} edgeIndices=["), false)?;
        for (position, &edge_index) in node.edge_indices.iter().enumerate() {
            identities.resources.charge_layout_work(1)?;
            if position > 0 {
                record.text(",", false)?;
            }
            if edge_index < model.edges.len() {
                record.text(&format!("E{}", edge_index + 1), false)?;
            } else {
                // Invalid external models can contain usize::MAX; never increment it.
                record.text(&format!("index={edge_index} (unresolved)"), false)?;
            }
        }
        record.text("]", false)?;
        record.finish()?;
        // Keep exact indices and containment alongside their readable references.
        details(&key, &fields, lines)?;
        identities.disclose_duplicate(&node.id, IdentityKind::Node, index, lines)?;
    }
    for (index, (group, raw)) in model.groups.iter().zip(groups).enumerate() {
        let key = format!("G{}", index + 1);
        let mut fields: Fields<'_> = decode(raw, lines)?;
        let mut record = RecordWriter::new(lines)?;
        record.text(&format!("{key} [group] id="), false)?;
        record.text(&group.id, true)?;
        record.text(" title=", false)?;
        optional_text(group.title.as_deref(), &mut record)?;
        record.text(" icon=", false)?;
        optional_text(group.icon.as_deref(), &mut record)?;
        write_parent(group.in_group.as_deref(), &identities, &mut record)?;
        record.finish()?;
        for name in ["id", "title", "icon"] {
            fields.remove(name);
        }
        details(&key, &fields, lines)?;
        identities.disclose_duplicate(&group.id, IdentityKind::Group, index, lines)?;
    }
    for (index, (edge, raw)) in model.edges.iter().zip(edges).enumerate() {
        let key = format!("E{}", index + 1);
        let mut fields: Fields<'_> = decode(raw, lines)?;
        let mut record = RecordWriter::new(lines)?;
        record.text(&format!("{key} [edge] title="), false)?;
        optional_text(edge.title.as_deref(), &mut record)?;
        record.finish()?;
        fields.remove("title");
        for (side, id, direction, into, group) in [
            (
                "lhs",
                edge.lhs_id.as_str(),
                edge.lhs_dir,
                edge.lhs_into,
                edge.lhs_group,
            ),
            (
                "rhs",
                edge.rhs_id.as_str(),
                edge.rhs_dir,
                edge.rhs_into,
                edge.rhs_group,
            ),
        ] {
            let mut record = RecordWriter::new(lines)?;
            record.text(&format!("{key} {side}="), false)?;
            let keys = identities.reference(id, IdentityKind::Node, &mut record)?;
            record.text(&format!(" {side}Dir="), false)?;
            record.text(direction.encode_utf8(&mut [0; 4]), true)?;
            record.text(&format!(" {side}Into="), false)?;
            optional_bool(into, &mut record)?;
            record.text(&format!(" {side}Group="), false)?;
            optional_bool(group, &mut record)?;
            record.text(&format!(" {side}Anchor="), false)?;
            identities.write_anchor(id, keys, group, model, &mut record)?;
            record.finish()?;
        }
        // Serialized endpoints and all optional flags stay raw as well as readable. In
        // particular, false, null, and an omitted field never get rewritten in these details.
        details(&key, &fields, lines)?;
    }
    for (index, (alignment, raw)) in model.layout_hints.iter().zip(alignments).enumerate() {
        let key = format!("A{}", index + 1);
        let mut fields: Fields<'_> = decode(raw, lines)?;
        let mut record = RecordWriter::new(lines)?;
        record.text(&format!("{key} [alignment] direction="), false)?;
        record.text(alignment.direction.as_str(), true)?;
        record.text(" members=[", false)?;
        for (position, member) in alignment.members.iter().enumerate() {
            if position > 0 {
                record.text("; ", false)?;
            }
            identities.reference(member, IdentityKind::Node, &mut record)?;
        }
        record.text("]", false)?;
        record.finish()?;
        fields.remove("direction");
        // Preserve the exact member order, repeats, empty arrays, and future hint fields.
        details(&key, &fields, lines)?;
    }
    // Source configuration/title, the diagram tag, and future envelope fields stay complete.
    details("architecture", &root, lines)?;
    Ok(true)
}

fn optional_text(text: Option<&str>, record: &mut RecordWriter<'_, '_>) -> Result<()> {
    match text {
        Some(text) => record.text(text, true),
        None => record.text("null", false),
    }
}

fn optional_bool(value: Option<bool>, record: &mut RecordWriter<'_, '_>) -> Result<()> {
    record.text(
        match value {
            Some(true) => "true",
            Some(false) => "false",
            None => "null",
        },
        false,
    )
}

fn write_parent(
    parent: Option<&str>,
    identities: &Identities<'_>,
    record: &mut RecordWriter<'_, '_>,
) -> Result<()> {
    record.text(" in=", false)?;
    if let Some(id) = parent {
        identities.reference(id, IdentityKind::Group, record)?;
        Ok(())
    } else {
        record.text("null (root)", false)
    }
}

impl<'a> Identities<'a> {
    fn new(
        model: &'a ArchitectureDiagramModel,
        lines: &SemanticFallbackWriter<'_>,
    ) -> Result<Self> {
        let resources = lines.semantic_resources.clone();
        let count = resources.checked_work_add(model.nodes.len(), model.groups.len())?;
        resources.charge_layout_work(count)?;
        resources.check(AsciiResourceLimitId::MaxDocumentCells, count)?;
        resources.check(AsciiResourceLimitId::MaxOutputBytes, count)?;
        let mut by_id: HashMap<&str, Vec<ReadingKey>> = HashMap::new();
        by_id.try_reserve(count).map_err(|_| allocation_failed())?;
        for (kind, index, id) in model
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (IdentityKind::Node, index, node.id.as_str()))
            .chain(
                model
                    .groups
                    .iter()
                    .enumerate()
                    .map(|(index, group)| (IdentityKind::Group, index, group.id.as_str())),
            )
        {
            resources.charge_layout_work(id.len().max(1))?;
            let keys = by_id.entry(id).or_default();
            keys.try_reserve(1).map_err(|_| allocation_failed())?;
            keys.push(ReadingKey { kind, index });
        }
        Ok(Self { by_id, resources })
    }

    fn keys(&self, id: &str) -> Result<&[ReadingKey]> {
        // Hashing authored strings is work too, even when a reference cannot be resolved.
        self.resources.charge_layout_work(id.len().max(1))?;
        Ok(self.by_id.get(id).map_or(&[], Vec::as_slice))
    }

    fn reference(
        &self,
        id: &str,
        expected: IdentityKind,
        record: &mut RecordWriter<'_, '_>,
    ) -> Result<&[ReadingKey]> {
        let keys = self.keys(id)?;
        self.write_reference(id, keys, expected, record)?;
        Ok(keys)
    }

    fn write_reference(
        &self,
        id: &str,
        keys: &[ReadingKey],
        expected: IdentityKind,
        record: &mut RecordWriter<'_, '_>,
    ) -> Result<()> {
        self.write_keys(keys, record)?;
        match keys {
            [] => record.text(" (unresolved)", false)?,
            [key] if key.kind != expected => {
                record.text(" (unresolved: expected ", false)?;
                record.text(expected.name(), false)?;
                record.text(")", false)?;
            }
            [_] => {}
            _ => record.text(" (ambiguous)", false)?,
        }
        record.text(" id=", false)?;
        record.text(id, true)
    }

    fn write_keys(&self, keys: &[ReadingKey], record: &mut RecordWriter<'_, '_>) -> Result<()> {
        self.resources.charge_layout_work(keys.len())?;
        record.text("[", false)?;
        for (position, key) in keys.iter().enumerate() {
            if position > 0 {
                record.text(",", false)?;
            }
            record.text(&format!("{}{}", key.kind.prefix(), key.index + 1), false)?;
        }
        record.text("]", false)
    }

    fn write_anchor(
        &self,
        id: &str,
        endpoint_keys: &[ReadingKey],
        group: Option<bool>,
        model: &ArchitectureDiagramModel,
        record: &mut RecordWriter<'_, '_>,
    ) -> Result<()> {
        if group != Some(true) {
            return self.write_reference(id, endpoint_keys, IdentityKind::Node, record);
        }
        // Never scan all nodes per endpoint or choose one of several duplicate declarations.
        let [
            ReadingKey {
                kind: IdentityKind::Node,
                index,
            },
        ] = endpoint_keys
        else {
            return record.text("[] (unresolved: no unique node endpoint)", false);
        };
        let node = model.nodes.get(*index).ok_or_else(invalid_projection)?;
        if let Some(parent) = node.in_group.as_deref() {
            self.reference(parent, IdentityKind::Group, record)?;
            Ok(())
        } else {
            record.text("[] (unresolved: endpoint has no containing group)", false)
        }
    }

    fn disclose_duplicate(
        &self,
        id: &str,
        kind: IdentityKind,
        index: usize,
        lines: &mut SemanticFallbackWriter<'_>,
    ) -> Result<()> {
        let keys = self.keys(id)?;
        // One diagnostic per ID avoids quadratic repetition for duplicate declarations.
        if keys.len() < 2 || keys.first() != Some(&ReadingKey { kind, index }) {
            return Ok(());
        }
        let mut record = RecordWriter::new(lines)?;
        record.text(
            &format!(
                "{}{} identity diagnostic: duplicate id ",
                kind.prefix(),
                index + 1
            ),
            false,
        )?;
        self.write_keys(keys, &mut record)?;
        record.text(" (ambiguous) id=", false)?;
        record.text(id, true)?;
        record.finish()
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
        diagram_type: "architecture",
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
    use merman_model::{OperationControl, OperationPhase};

    const ENVELOPE: &[u8] = br#"{
        "title":"  Architecture title  ","accTitle":"Accessible title",
        "accDescr":"Description\nsecond line",
        "nodes":[
            {"id":"left","type":"service","edgeIndices":[0,0,9],
             "icon":"logos:database","iconText":"  \"DB\"\n  ",
             "title":"  Left \"title\"\nnext  ","in":"inner",
             "futureNode":{"raw":"\ud800","number":1e+09}},
            {"id":"right","type":"junction","edgeIndices":[0],"in":"outer"}
        ],
        "groups":[
            {"id":"outer","icon":"cloud","title":"Outer"},
            {"id":"inner","title":"Inner","in":"outer","futureGroup":false}
        ],
        "edges":[{"lhsId":"left","lhsDir":"B","lhsInto":true,"lhsGroup":true,
                  "rhsId":"right","rhsDir":"L","rhsInto":false,"rhsGroup":null,
                  "title":"  edge label  ","futureEdge":[1,null,true]}],
        "layoutHints":[
            {"direction":"row","members":["right","left","missing","left"],
             "futureHint":{"number":1e+09}},
            {"direction":"column","members":[]}
        ],
        "type":"architecture","sourceConfig":{"architecture":{"future":true}},
        "sourceTitle":"Source title","futureRoot":{"opaque":"\ud800","number":1e+09}
    }"#;

    fn render(bytes: &[u8], policy: &AsciiResourcePolicy) -> Result<String> {
        let model: ArchitectureDiagramModel = serde_json::from_slice(bytes).unwrap();
        let execution = AsciiExecution::for_test(policy);
        let mut lines =
            SemanticFallbackWriter::new(execution, usize::MAX, TerminalWidthProfile::Unicode);
        assert!(write_if_architecture(
            &AsciiDiagram::Architecture(&model),
            bytes,
            &mut lines,
        )?);
        Ok(lines.finish().into_text())
    }

    #[test]
    fn complete_envelope_preserves_declarations_raw_details_and_independent_endpoints() {
        let output = render(ENVELOPE, &AsciiResourcePolicy::default()).unwrap();
        for text in [
            r#"Architecture title="  Architecture title  ""#,
            r#"Architecture accTitle="Accessible title""#,
            r#"Architecture accDescr="Description\nsecond line""#,
            r#"N1 [service] id="left" title="  Left \"title\"\nnext  " icon="logos:database" iconText="  \"DB\"\n  " in=[G2] id="inner""#,
            "N1 edgeIndices=[E1,E1,index=9 (unresolved)]",
            r#"G2 [group] id="inner" title="Inner" icon=null in=[G1] id="outer""#,
            r#"E1 [edge] title="  edge label  ""#,
            r#"E1 lhs=[N1] id="left" lhsDir="B" lhsInto=true lhsGroup=true lhsAnchor=[G2] id="inner""#,
            r#"E1 rhs=[N2] id="right" rhsDir="L" rhsInto=false rhsGroup=null rhsAnchor=[N2] id="right""#,
            r#"A1 [alignment] direction="row" members=[[N2] id="right"; [N1] id="left"; [] (unresolved) id="missing"; [N1] id="left"]"#,
            r#"A2 [alignment] direction="column" members=[]"#,
        ] {
            assert!(output.contains(text), "missing {text}:\n{output}");
        }
        let compact = output.split_whitespace().collect::<String>();
        for text in [
            "edgeIndices:[0,0,9]",
            r#"futureNode:{"raw":"\ud800","number":1e+09}"#,
            "futureGroup:false",
            "futureEdge:[1,null,true]",
            r#"members:["right","left","missing","left"]"#,
            r#"futureHint:{"number":1e+09}"#,
            r#"futureRoot:{"opaque":"\ud800","number":1e+09}"#,
            r#"sourceConfig:{"architecture":{"future":true}}"#,
            r#"sourceTitle:"Sourcetitle""#,
            "lhsInto:true",
            "rhsInto:false",
            "lhsGroup:true",
            "rhsGroup:null",
        ] {
            assert!(compact.contains(text), "missing {text}:\n{output}");
        }
        let mut previous = 0;
        for key in [
            "N1 [service]",
            "N2 [junction]",
            "G1 [group]",
            "G2 [group]",
            "E1 [edge]",
            "A1 [alignment]",
            "A2 [alignment]",
        ] {
            let position = output.find(key).unwrap();
            assert!(position >= previous, "out-of-order {key}:\n{output}");
            previous = position;
        }
    }

    #[test]
    fn duplicate_ids_wrong_kinds_and_missing_boundaries_never_choose_an_anchor() {
        let bytes = br#"{
            "nodes":[
                {"id":"dup","type":"service","in":"g"},
                {"id":"dup","type":"junction"},
                {"id":"orphan","type":"service","in":"missing-group"},
                {"id":"root","type":"junction"}
            ],
            "groups":[{"id":"dup"},{"id":"g"},{"id":"g"},{"id":"group-only"}],
            "edges":[
                {"lhsId":"dup","lhsDir":"R","lhsGroup":true,
                 "rhsId":"absent","rhsDir":"T","rhsInto":true,"rhsGroup":true},
                {"lhsId":"orphan","lhsDir":"L","lhsGroup":true,
                 "rhsId":"root","rhsDir":"B","rhsGroup":true},
                {"lhsId":"group-only","lhsDir":"L","lhsInto":false,"lhsGroup":false,
                 "rhsId":"root","rhsDir":"R","rhsInto":true,"rhsGroup":false}
            ],
            "layoutHints":[{"direction":"column","members":["dup","group-only","absent"]}]
        }"#;
        let output = render(bytes, &AsciiResourcePolicy::default()).unwrap();
        for text in [
            r#"in=[G2,G3] (ambiguous) id="g""#,
            r#"E1 lhs=[N1,N2,G1] (ambiguous) id="dup""#,
            "lhsGroup=true lhsAnchor=[] (unresolved: no unique node endpoint)",
            r#"E1 rhs=[] (unresolved) id="absent""#,
            r#"E2 lhs=[N3] id="orphan" lhsDir="L" lhsInto=null lhsGroup=true lhsAnchor=[] (unresolved) id="missing-group""#,
            "rhsGroup=true rhsAnchor=[] (unresolved: endpoint has no containing group)",
            r#"E3 lhs=[G4] (unresolved: expected node) id="group-only""#,
            r#"E3 rhs=[N4] id="root" rhsDir="R" rhsInto=true rhsGroup=false rhsAnchor=[N4] id="root""#,
        ] {
            assert!(output.contains(text), "missing {text}:\n{output}");
        }
        assert_eq!(
            output.matches("identity diagnostic: duplicate id").count(),
            2
        );
    }

    #[test]
    fn invalid_edge_index_does_not_overflow_or_disappear() {
        let mut model: ArchitectureDiagramModel = serde_json::from_slice(ENVELOPE).unwrap();
        model.nodes[0].edge_indices = vec![usize::MAX];
        let bytes = serde_json::to_vec(&model).unwrap();
        let output = render(&bytes, &AsciiResourcePolicy::default()).unwrap();
        assert!(output.contains(&format!(
            "N1 edgeIndices=[index={} (unresolved)]",
            usize::MAX
        )));
        assert!(output.contains(&format!("edgeIndices:[{}]", usize::MAX)));
    }

    #[test]
    fn output_budget_and_cancellation_remain_terminal() {
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
                    matches!(result, Err(AsciiError::ResourceLimitExceeded(details))
                    if details.limit == AsciiResourceLimitId::MaxOutputBytes)
                );
            }
        }
        let model: ArchitectureDiagramModel = serde_json::from_slice(ENVELOPE).unwrap();
        let control = OperationControl::new();
        control.cancel_after_checkpoints(0);
        let policy = AsciiResourcePolicy::default();
        let execution = AsciiExecution::new(&control, &policy);
        let mut lines = SemanticFallbackWriter::new(execution, 80, TerminalWidthProfile::Unicode);
        assert!(matches!(
            write_if_architecture(&AsciiDiagram::Architecture(&model), ENVELOPE, &mut lines),
            Err(AsciiError::Cancelled(cancelled)) if cancelled.phase == OperationPhase::Semantic
        ));
        assert!(lines.finish().into_text().is_empty());
    }
}
