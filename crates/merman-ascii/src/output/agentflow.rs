//! Complete Agentflow reading records over the one admitted semantic envelope.
//!
//! Reading keys belong to source-ordered declarations, rather than authored IDs. Shared IDs
//! retain every node, flow, and connector identity; references disclose all matching records.
//! Containment and edges remain authored, including cycles, hidden members, and internal edges
//! of collapsed flows. Opaque details borrow the envelope without rebuilding its JSON values.

use super::record::{RecordPart, RecordWriter, details, write_plain_record, write_record};
use super::{AsciiDiagram, AsciiError, RawValue, Result, SemanticFallbackWriter};
use crate::resource::ResourceContext;
use merman_model::families::agentflow::{
    AgentflowDiagramModel, AgentflowEdgeSemantic, AgentflowVertexKind,
};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};

type Fields<'a> = BTreeMap<String, &'a RawValue>;

#[derive(Clone, Copy, PartialEq, Eq)]
struct ReadingKey {
    prefix: char,
    number: usize,
}

struct Identities<'a> {
    by_id: HashMap<&'a str, Vec<ReadingKey>>,
    resources: ResourceContext,
}

pub(super) fn write_if_agentflow(
    model: &AsciiDiagram<'_>,
    bytes: &[u8],
    lines: &mut SemanticFallbackWriter<'_>,
) -> Result<bool> {
    let AsciiDiagram::Agentflow(model) = model else {
        return Ok(false);
    };
    lines.admit_node(0)?;
    lines.semantic_resources.charge_layout_work(bytes.len())?;
    let mut root: Fields<'_> = serde_json::from_slice(bytes).map_err(|_| invalid_projection())?;
    let nodes = take_array(&mut root, "vertices", model.vertices.len(), lines)?;
    // AgentflowDiagramModel uses serde's camelCase spelling, including the capital G.
    let flows = take_array(&mut root, "subGraphs", model.sub_graphs.len(), lines)?;
    let connectors = take_array(&mut root, "connectors", model.connectors.len(), lines)?;
    let edges = take_array(&mut root, "edges", model.edges.len(), lines)?;
    let diagnostics = take_array(&mut root, "diagnostics", model.diagnostics.len(), lines)?;
    let identities = Identities::new(model, lines)?;

    write_record(
        &[
            RecordPart::Plain("direction: "),
            RecordPart::Quoted(&model.direction),
        ],
        lines,
    )?;
    root.remove("direction");
    write_plain_record(
        "Keys: N nodes; G flows; C connectors; E edges; D diagnostics. Each collection keeps source order; edges name authored source then target. Shared IDs list all matching keys.",
        lines,
    )?;
    for (index, (node, raw)) in model.vertices.iter().zip(nodes).enumerate() {
        let key = format!("N{}", index + 1);
        let role = match node.vertex_kind {
            AgentflowVertexKind::Tool => "tool",
            AgentflowVertexKind::Action => "action",
            AgentflowVertexKind::Input => "input",
            AgentflowVertexKind::Refdoc => "refdoc",
            AgentflowVertexKind::Decision => "decision",
            AgentflowVertexKind::Connector => "connector",
            AgentflowVertexKind::Task => "task",
        };
        let mut fields: Fields<'_> = decode(raw, lines)?;
        let mut record = RecordWriter::new(lines)?;
        record.text(&format!("{key} [{role}] id="), false)?;
        record.text(&node.id, true)?;
        record.text(" label=", false)?;
        optional_text(node.label.as_deref(), &mut record)?;
        record.finish()?;
        for field in ["id", "label", "vertexKind"] {
            fields.remove(field);
        }
        if let Some(parent) = node.parent_id.as_deref() {
            let mut record = RecordWriter::new(lines)?;
            record.text(&format!("{key} parent-flow: "), false)?;
            identities.reference(parent, true, &mut record)?;
            record.finish()?;
        }
        // Keep the exact parentId and shape alongside the readable role and parent keys.
        details(&key, &fields, lines)?;
        identities.disclose_overlap(&node.id, 'N', index + 1, lines)?;
    }
    for (index, (flow, raw)) in model.sub_graphs.iter().zip(flows).enumerate() {
        let key = format!("G{}", index + 1);
        let mut fields: Fields<'_> = decode(raw, lines)?;
        let mut record = RecordWriter::new(lines)?;
        record.text(&format!("{key} [flow] id="), false)?;
        record.text(&flow.id, true)?;
        record.text(" title=", false)?;
        optional_text(flow.title.as_deref(), &mut record)?;
        record.text(" type=", false)?;
        record.text(&flow.sub_graph_type, true)?;
        record.text(" direction=", false)?;
        if let Some(direction) = flow.direction.as_deref() {
            record.text(direction, true)?;
        } else {
            record.text("inherit", false)?;
        }
        record.text(
            if flow
                .metadata
                .get("view")
                .and_then(serde_json::Value::as_str)
                == Some("collapsed")
            {
                " collapsed=true"
            } else {
                " collapsed=false"
            },
            false,
        )?;
        record.finish()?;
        for field in ["id", "title", "type", "direction", "nodes"] {
            fields.remove(field);
        }
        if flow.nodes.is_empty() {
            write_plain_record(&format!("{key} members: []"), lines)?;
        }
        for (position, member) in flow.nodes.iter().enumerate() {
            let mut record = RecordWriter::new(lines)?;
            record.text(&format!("{key} member {}: ", position + 1), false)?;
            identities.reference(member, false, &mut record)?;
            record.finish()?;
        }
        // Metadata includes the authored view, instructions, algorithms, and future values.
        details(&key, &fields, lines)?;
        identities.disclose_overlap(&flow.id, 'G', index + 1, lines)?;
    }
    for (index, (connector, raw)) in model.connectors.iter().zip(connectors).enumerate() {
        let key = format!("C{}", index + 1);
        let mut fields: Fields<'_> = decode(raw, lines)?;
        let mut record = RecordWriter::new(lines)?;
        record.text(&format!("{key} [connector] id="), false)?;
        record.text(&connector.id, true)?;
        record.text(" title=", false)?;
        optional_text(connector.title.as_deref(), &mut record)?;
        record.finish()?;
        for field in ["id", "title"] {
            fields.remove(field);
        }
        details(&key, &fields, lines)?;
        identities.disclose_overlap(&connector.id, 'C', index + 1, lines)?;
    }
    for (index, (edge, raw)) in model.edges.iter().zip(edges).enumerate() {
        let key = format!("E{}", index + 1);
        let mut fields: Fields<'_> = decode(raw, lines)?;
        let semantic = match edge.edge_semantic {
            AgentflowEdgeSemantic::Sequence => "sequence -->",
            AgentflowEdgeSemantic::Reference => "reference -.-",
            AgentflowEdgeSemantic::Failure => "failure --x",
        };
        let mut record = RecordWriter::new(lines)?;
        record.text(&format!("{key} [{semantic}] source="), false)?;
        identities.reference(&edge.start, false, &mut record)?;
        record.text(" target=", false)?;
        identities.reference(&edge.end, false, &mut record)?;
        record.text("; id=", false)?;
        optional_text(edge.id.as_deref(), &mut record)?;
        record.text(" label=", false)?;
        optional_text(edge.label.as_deref(), &mut record)?;
        // The canonical model deliberately skips this provenance bit during serialization.
        record.text(
            if edge.is_user_defined_id {
                " explicit-id=true"
            } else {
                " explicit-id=false"
            },
            false,
        )?;
        record.finish()?;
        for field in ["id", "start", "end", "label"] {
            fields.remove(field);
        }
        // Keep the serialized semantic, type, stroke, length, metadata, and future fields.
        details(&key, &fields, lines)?;
    }
    for (index, (diagnostic, raw)) in model.diagnostics.iter().zip(diagnostics).enumerate() {
        let key = format!("D{}", index + 1);
        let mut fields: Fields<'_> = decode(raw, lines)?;
        let mut record = RecordWriter::new(lines)?;
        record.text(&format!("{key} diagnostic id="), false)?;
        record.text(&diagnostic.id, true)?;
        record.text(" severity=", false)?;
        record.text(&diagnostic.severity, true)?;
        record.text(" node=", false)?;
        identities.reference(&diagnostic.node_id, false, &mut record)?;
        record.text(" message=", false)?;
        record.text(&diagnostic.message, true)?;
        record.finish()?;
        for field in ["id", "severity", "nodeId", "message"] {
            fields.remove(field);
        }
        details(&key, &fields, lines)?;
    }
    if let Some(authored) = root.remove("authored") {
        write_authored(authored, lines)?;
    }
    // Warnings, accessibility, model/source titles, source configuration, and
    // future envelope fields remain complete borrowed JSON, including explicit empty defaults.
    details("agentflow", &root, lines)?;
    Ok(true)
}

fn write_authored(raw: &RawValue, lines: &mut SemanticFallbackWriter<'_>) -> Result<()> {
    let mut fields: Fields<'_> = decode(raw, lines)?;
    // These canonical maps use HashMap storage. Sort borrowed entries so separate parses and
    // canonical/full hosts produce the same records. class_defs keeps its authored IndexMap order.
    for name in [
        "nodes",
        "edges",
        "subgraph_classes",
        "subgraph_label_types",
        "tooltips",
    ] {
        let Some(raw) = fields.remove(name) else {
            continue;
        };
        let entries: Fields<'_> = decode(raw, lines)?;
        if entries.is_empty() {
            continue;
        }
        for (id, value) in entries {
            lines.admit_node(1)?;
            let mut record = RecordWriter::new(lines)?;
            record.text("authored ", false)?;
            record.text(name, false)?;
            record.text(" id=", false)?;
            record.text(&id, true)?;
            record.text(" fields=", false)?;
            record.text(value.get(), false)?;
            record.finish()?;
        }
    }
    details("authored", &fields, lines)
}

impl<'a> Identities<'a> {
    fn new(model: &'a AgentflowDiagramModel, lines: &SemanticFallbackWriter<'_>) -> Result<Self> {
        let resources = lines.semantic_resources.clone();
        let count = resources.checked_work_add(model.vertices.len(), model.sub_graphs.len())?;
        let count = resources.checked_work_add(count, model.connectors.len())?;
        resources.charge_layout_work(count)?;
        let mut by_id: HashMap<&str, Vec<ReadingKey>> = HashMap::new();
        by_id.try_reserve(count).map_err(|_| allocation_failed())?;
        for (prefix, index, id) in model
            .vertices
            .iter()
            .enumerate()
            .map(|(index, node)| ('N', index, node.id.as_str()))
            .chain(
                model
                    .sub_graphs
                    .iter()
                    .enumerate()
                    .map(|(index, flow)| ('G', index, flow.id.as_str())),
            )
            .chain(
                model
                    .connectors
                    .iter()
                    .enumerate()
                    .map(|(index, connector)| ('C', index, connector.id.as_str())),
            )
        {
            resources.charge_layout_work(id.len().max(1))?;
            let keys = by_id.entry(id).or_default();
            keys.try_reserve(1).map_err(|_| allocation_failed())?;
            keys.push(ReadingKey {
                prefix,
                number: index + 1,
            });
        }
        Ok(Self { by_id, resources })
    }

    fn reference(
        &self,
        id: &str,
        only_flows: bool,
        record: &mut RecordWriter<'_, '_>,
    ) -> Result<()> {
        self.resources.charge_layout_work(id.len().max(1))?;
        let keys = self.by_id.get(id).map_or(&[][..], Vec::as_slice);
        self.resources.charge_layout_work(keys.len())?;
        record.text("[", false)?;
        let mut found = false;
        for key in keys {
            if only_flows && key.prefix != 'G' {
                continue;
            }
            if found {
                record.text(",", false)?;
            }
            record.text(&format!("{}{}", key.prefix, key.number), false)?;
            found = true;
        }
        record.text("]", false)?;
        if !found {
            record.text(" (unresolved)", false)?;
        }
        record.text(" id=", false)?;
        record.text(id, true)
    }

    fn disclose_overlap(
        &self,
        id: &str,
        prefix: char,
        number: usize,
        lines: &mut SemanticFallbackWriter<'_>,
    ) -> Result<()> {
        self.resources.charge_layout_work(id.len().max(1))?;
        let Some(keys) = self.by_id.get(id) else {
            return Err(invalid_projection());
        };
        if keys.len() < 2 || keys.first() != Some(&ReadingKey { prefix, number }) {
            return Ok(());
        }
        self.resources.charge_layout_work(keys.len())?;
        // Insertion groups keys by collection. Adjacent equal prefixes expose duplicate
        // declarations without quadratic scans or treating cross-collection overlaps as aliases.
        let duplicates = keys.windows(2).any(|pair| pair[0].prefix == pair[1].prefix);
        let mut record = RecordWriter::new(lines)?;
        record.text(&format!("{prefix}{number}"), false)?;
        record.text(
            if duplicates {
                " identity diagnostic: duplicate ID within a collection; candidates="
            } else {
                " identity records for shared authored ID: "
            },
            false,
        )?;
        self.reference(id, false, &mut record)?;
        record.finish()
    }
}

fn optional_text(value: Option<&str>, record: &mut RecordWriter<'_, '_>) -> Result<()> {
    match value {
        Some(value) => record.text(value, true),
        None => record.text("null", false),
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
        diagram_type: "agentflow",
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
    use crate::resource::AsciiResourceLimitId;
    use crate::{AsciiResourcePolicy, TerminalWidthProfile};
    use merman_model::{OperationControl, OperationPhase};

    const MODEL: &[u8] = br#"{
        "direction":"LR",
        "vertices":[
            {"id":"task\"id","label":"  Step \"one\"\nnext  ","vertexKind":"task",
             "shape":"subroutine","parentId":"worker","metadata":{"instruction":"run"},
             "futureNode":{"raw":"\ud800","number":1e+09}},
            {"id":"choice","label":null,"vertexKind":"decision","shape":"diamond"},
            {"id":"api","label":"Node API","vertexKind":"connector","shape":"connector"},
            {"id":"worker","label":"Flow vertex","vertexKind":"action","shape":"collapsedGroup"}
        ],
        "subGraphs":[
            {"id":"worker","title":"Worker \"flow\"","nodes":["task\"id","api","inner"],
             "type":"flow","direction":"RL","metadata":{"view":"collapsed","algorithm":"elk.layered"},
             "futureFlow":{}},
            {"id":"inner","title":null,"nodes":["choice"],"type":"flow"}
        ],
        "connectors":[{"id":"api","title":"Connector API","metadata":{"instruction":"call"},
                       "futureConnector":[]}],
        "edges":[
            {"start":"task\"id","end":"api","id":"run\"edge","label":"do \"it\"",
             "edgeSemantic":"sequence","type":"arrow_point","stroke":"normal","length":2,
             "metadata":{"retry":3},"futureEdge":{"raw":"\ud800"}},
            {"start":"api","end":"choice","label":"read docs","edgeSemantic":"reference"},
            {"start":"choice","end":"worker","id":"failure","edgeSemantic":"failure"},
            {"start":"task\"id","end":"task\"id","edgeSemantic":"sequence"}
        ],
        "diagnostics":[{"id":"SHAPE_UNSUPPORTED","severity":"warning","nodeId":"task\"id",
                        "message":"Keep \"authored\" shape",
                        "position":{"startLine":1,"startColumn":2,"endLine":3,"endColumn":4,"startIndex":5,"endIndex":6},"futureDiagnostic":false}],
        "authored":{"class_defs":{"hot":["fill:red","stroke:blue"]}},
        "warningFacts":[],"title":"Model title","accTitle":"Accessible title","accDescr":"Details",
        "sourceTitle":"Source title","sourceConfig":{"agentflow":{"defaultRenderer":"elk"}},
        "futureRoot":{"opaque":"\ud800","number":1e+09}
    }"#;

    fn render_with_policy(
        model: &AgentflowDiagramModel,
        bytes: &[u8],
        width: usize,
        policy: &AsciiResourcePolicy,
    ) -> Result<String> {
        let execution = AsciiExecution::for_test(policy);
        let mut lines =
            SemanticFallbackWriter::new(execution, width, TerminalWidthProfile::Unicode);
        assert!(write_if_agentflow(
            &AsciiDiagram::Agentflow(model),
            bytes,
            &mut lines,
        )?);
        Ok(lines.finish().into_text())
    }

    fn render(model: &AgentflowDiagramModel, bytes: &[u8], width: usize) -> Result<String> {
        render_with_policy(model, bytes, width, &AsciiResourcePolicy::unbounded())
    }

    #[test]
    fn complete_document_preserves_semantics_and_raw_details_at_reading_widths() {
        let mut model: AgentflowDiagramModel = serde_json::from_slice(MODEL).unwrap();
        model.edges[0].is_user_defined_id = true;
        for width in [24, 4096] {
            let text = render(&model, MODEL, width).unwrap();
            assert!(text.lines().all(|line| line.len() <= width), "{text}");
            let compact = text.split_whitespace().collect::<String>();
            for expected in [
                r#"direction:"LR""#,
                r#"N1[task]id="task\"id"label="Step\"one\"\nnext""#,
                r#"N2[decision]id="choice"label=null"#,
                r#"N1parent-flow:[G1]id="worker""#,
                r#"parentId:"worker""#,
                r#"shape:"subroutine""#,
                r#"futureNode:{"raw":"\ud800","number":1e+09}"#,
                r#"G1[flow]id="worker"title="Worker\"flow\""type="flow"direction="RL"collapsed=true"#,
                r#"G1member1:[N1]id="task\"id""#,
                r#"G1member2:[N3,C1]id="api""#,
                r#"G1member3:[G2]id="inner""#,
                r#"G2[flow]id="inner"title=nulltype="flow"direction=inheritcollapsed=false"#,
                r#"metadata:{"view":"collapsed","algorithm":"elk.layered"}"#,
                r#"C1[connector]id="api"title="ConnectorAPI""#,
                r#"metadata:{"instruction":"call"}"#,
                r#"E1[sequence-->]source=[N1]id="task\"id"target=[N3,C1]id="api";id="run\"edge"label="do\"it\""explicit-id=true"#,
                r#"E2[reference-.-]source=[N3,C1]id="api"target=[N2]id="choice";id=nulllabel="readdocs""#,
                r#"E3[failure--x]source=[N2]id="choice"target=[N4,G1]id="worker""#,
                r#"E4[sequence-->]source=[N1]id="task\"id"target=[N1]id="task\"id""#,
                r#"futureEdge:{"raw":"\ud800"}"#,
                r#"D1diagnosticid="SHAPE_UNSUPPORTED"severity="warning"node=[N1]id="task\"id"message="Keep\"authored\"shape""#,
                r#"position:{"startLine":1,"startColumn":2,"endLine":3,"endColumn":4,"startIndex":5,"endIndex":6}"#,
                "futureFlow:{}",
                "futureConnector:[]",
                "futureDiagnostic:false",
                r#"class_defs:{"hot":["fill:red","stroke:blue"]}"#,
                r#"sourceConfig:{"agentflow":{"defaultRenderer":"elk"}}"#,
                r#"sourceTitle:"Sourcetitle""#,
                r#"title:"Modeltitle""#,
                r#"accTitle:"Accessibletitle""#,
                r#"accDescr:"Details""#,
                r#"futureRoot:{"opaque":"\ud800","number":1e+09}"#,
                "warningFacts:[]",
            ] {
                assert!(compact.contains(expected), "missing {expected}:\n{text}");
            }
            assert!(
                !text.contains("invalid serialized semantic projection"),
                "{text}"
            );
            if width == 4096 {
                assert!(text.contains(r#"label="  Step \"one\"\nnext  ""#), "{text}");
            }
        }
    }

    #[test]
    fn overlaps_keep_distinct_declarations_and_diagnose_duplicate_or_missing_targets() {
        let mut model: AgentflowDiagramModel = serde_json::from_slice(MODEL).unwrap();
        let text = render(&model, MODEL, 4096).unwrap();
        assert!(text.contains("N3 identity records for shared authored ID: [N3,C1] id=\"api\""));
        assert!(text.contains("N4 identity records for shared authored ID: [N4,G1] id=\"worker\""));
        assert!(!text.contains("duplicate ID within a collection"));

        let mut duplicate = model.vertices[0].clone();
        duplicate.label = Some("Distinct duplicate declaration".into());
        model.vertices.push(duplicate);
        model.vertices[0].parent_id = Some("absent-flow".into());
        model.edges[0].end = "absent-node".into();
        model.sub_graphs[0].nodes.push("absent-member".into());
        let bytes = serde_json::to_vec(&model).unwrap();
        let text = render(&model, &bytes, 4096).unwrap();
        for expected in [
            "N1 identity diagnostic: duplicate ID within a collection; candidates=[N1,N5]",
            r#"N5 [task] id="task\"id" label="Distinct duplicate declaration""#,
            r#"N1 parent-flow: [] (unresolved) id="absent-flow""#,
            r#"target=[] (unresolved) id="absent-node""#,
            r#"G1 member 4: [] (unresolved) id="absent-member""#,
        ] {
            assert!(text.contains(expected), "missing {expected}:\n{text}");
        }
        assert_eq!(text, render(&model, &bytes, 4096).unwrap());
    }

    #[test]
    fn containment_cycles_and_duplicate_members_remain_authored() {
        let bytes =
            br#"{"direction":"TB","vertices":[],"connectors":[],"edges":[],"diagnostics":[],
            "subGraphs":[{"id":"a","nodes":["b","b"],"type":"flow","metadata":{"view":"collapsed"}},
                         {"id":"b","nodes":["a"],"type":"flow"}]}"#;
        let model = serde_json::from_slice(bytes).unwrap();
        let text = render(&model, bytes, 4096).unwrap();
        assert!(text.contains("G1 member 1: [G2] id=\"b\""), "{text}");
        assert!(text.contains("G1 member 2: [G2] id=\"b\""), "{text}");
        assert!(text.contains("G2 member 1: [G1] id=\"a\""), "{text}");
    }

    #[test]
    fn collections_require_exact_lengths_and_camel_case_sub_graphs() {
        let model: AgentflowDiagramModel = serde_json::from_slice(MODEL).unwrap();
        for field in [
            "vertices",
            "subGraphs",
            "connectors",
            "edges",
            "diagnostics",
        ] {
            let mut projection = serde_json::to_value(&model).unwrap();
            projection[field] = serde_json::json!([]);
            let bytes = serde_json::to_vec(&projection).unwrap();
            assert!(
                matches!(
                    render(&model, &bytes, 80),
                    Err(AsciiError::UnsupportedFeature {
                        diagram_type: "agentflow",
                        feature: "invalid serialized semantic projection",
                    })
                ),
                "{field}"
            );
        }
        let mut projection = serde_json::to_value(&model).unwrap();
        let flows = projection
            .as_object_mut()
            .unwrap()
            .remove("subGraphs")
            .unwrap();
        projection["subgraphs"] = flows;
        assert!(render(&model, &serde_json::to_vec(&projection).unwrap(), 80).is_err());
    }

    #[test]
    fn output_budget_is_exact_and_semantic_work_and_cancellation_are_bounded() {
        let model = serde_json::from_slice(MODEL).unwrap();
        let length = render(&model, MODEL, 40).unwrap().len();
        let exact = AsciiResourcePolicy::unbounded()
            .with_limit(AsciiResourceLimitId::MaxOutputBytes, length)
            .unwrap();
        assert_eq!(
            render_with_policy(&model, MODEL, 40, &exact).unwrap().len(),
            length
        );
        let below = AsciiResourcePolicy::unbounded()
            .with_limit(AsciiResourceLimitId::MaxOutputBytes, length - 1)
            .unwrap();
        assert!(matches!(
            render_with_policy(&model, MODEL, 40, &below),
            Err(AsciiError::ResourceLimitExceeded(details))
                if details.limit == AsciiResourceLimitId::MaxOutputBytes
                    && details.actual == length && details.max == length - 1
        ));
        let work = AsciiResourcePolicy::unbounded()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
            .unwrap();
        assert!(matches!(
            render_with_policy(&model, MODEL, 40, &work),
            Err(AsciiError::ResourceLimitExceeded(details))
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
        ));

        let control = OperationControl::new();
        control.cancel_after_checkpoints(0);
        let policy = AsciiResourcePolicy::unbounded();
        let execution = AsciiExecution::new(&control, &policy);
        let mut lines = SemanticFallbackWriter::new(execution, 40, TerminalWidthProfile::Unicode);
        assert!(matches!(
            write_if_agentflow(&AsciiDiagram::Agentflow(&model), MODEL, &mut lines),
            Err(AsciiError::Cancelled(cancelled)) if cancelled.phase == OperationPhase::Semantic
        ));
        assert!(lines.finish().into_text().is_empty());
    }

    #[test]
    fn unrelated_diagrams_are_declined_before_decoding() {
        let policy = AsciiResourcePolicy::unbounded();
        let execution = AsciiExecution::for_test(&policy);
        let mut lines = SemanticFallbackWriter::new(execution, 80, TerminalWidthProfile::Unicode);
        assert!(
            !write_if_agentflow(&AsciiDiagram::Unsupported("other"), b"invalid", &mut lines)
                .unwrap()
        );
        assert!(lines.finish().into_text().is_empty());
    }
}
