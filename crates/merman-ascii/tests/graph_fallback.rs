#![cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-state",
    feature = "diagram-swimlane"
))]

use merman_ascii::{
    ASCII_OUTPUT_SCHEMA_VERSION, AsciiOutput, AsciiOutputOutcome, AsciiProjection,
    AsciiRenderOptions, AsciiRenderer, AsciiResourcePolicy, AsciiViewportPolicy, OverflowPolicy,
    TerminalWidthProfile,
};
#[cfg(feature = "diagram-flowchart")]
use merman_ascii::{AsciiError, AsciiResourceLimitId};
use merman_core::SemanticParser;
use merman_model::{OperationControl, runtime::RuntimePolicy};
use unicode_width::UnicodeWidthStr;

fn render(
    source: &str,
    width: usize,
    options: AsciiRenderOptions,
    resources: AsciiResourcePolicy,
    control: &OperationControl,
) -> merman_ascii::Result<AsciiOutput> {
    let parsed = SemanticParser::new()
        .parse_diagram_sync(source)
        .expect("the native graph fixture should parse");
    let operation = RuntimePolicy::deterministic().begin_operation().unwrap();
    AsciiRenderer::new(options)?.render_parsed_report(
        &parsed,
        AsciiViewportPolicy::with_max_width(width).overflow(OverflowPolicy::Fallback),
        control,
        &operation,
        resources,
    )
}

fn fallback(source: &str, width: usize) -> AsciiOutput {
    render(
        source,
        width,
        AsciiRenderOptions::ascii(),
        AsciiResourcePolicy::unbounded(),
        &OperationControl::new(),
    )
    .expect("the complete graph document should fit the viewport")
}

fn assert_bounded(report: &AsciiOutput, width: usize) {
    assert_eq!(report.schema_version, ASCII_OUTPUT_SCHEMA_VERSION);
    assert_eq!(report.outcome, AsciiOutputOutcome::Fallback);
    assert_eq!(report.projection, AsciiProjection::StructuredText);
    assert_eq!(report.lossiness.as_str(), "presentation_only");
    assert!(report.fallback.attempted);
    assert!(report.overflowed);
    assert!(report.primary_extent.width > width);
    assert_eq!(report.emitted_extent.height, report.text.lines().count());
    for line in report.text.lines() {
        let measured = match report.width_profile {
            TerminalWidthProfile::Cjk => UnicodeWidthStr::width_cjk(line),
            _ => UnicodeWidthStr::width(line),
        };
        assert!(measured <= width, "{measured} > {width}: {line:?}");
    }
}

fn reader_key(input: &str) -> Option<(&str, &str)> {
    if !input.starts_with(['N', 'G', 'E']) {
        return None;
    }
    let digits = input[1..].bytes().take_while(u8::is_ascii_digit).count();
    (digits > 0).then(|| input.split_at(1 + digits))
}

// Quoted labels and structured values can contain convincing record/endpoint syntax. Only
// renderer-owned tokens outside those fields decide whether a wrapped record is complete.
fn record_is_open(record: &str) -> bool {
    let mut quoted = false;
    let mut escaped = false;
    let mut depth = 0usize;
    let mut tokens = String::new();
    for ch in record.chars() {
        if quoted {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                quoted = false;
            }
        } else {
            match ch {
                '"' => quoted = true,
                '[' | '{' => depth += 1,
                ']' | '}' => depth = depth.checked_sub(1).expect("balanced record framing"),
                _ if depth == 0 => tokens.push(ch),
                _ => {}
            }
        }
    }
    if quoted || depth > 0 {
        return true;
    }
    if reader_key(record).is_some_and(|(key, rest)| key.starts_with('E') && rest.starts_with(": "))
    {
        return tokens
            .split_whitespace()
            .filter(|token| {
                reader_key(token.trim_end_matches(':'))
                    .is_some_and(|(key, rest)| key.starts_with(['N', 'G']) && rest.is_empty())
            })
            .count()
            < 2;
    }
    false
}

// Rejoin physical wraps without interpreting an inline declaration as a new top-level node.
fn records(text: &str) -> Vec<String> {
    let mut records: Vec<String> = Vec::new();
    let state = text.starts_with("family: stateDiagram");
    for line in text.lines() {
        let formatted_record = records.last().is_some_and(|record| {
            reader_key(record).is_some_and(|(key, rest)| {
                (key.starts_with('E') && rest.starts_with(": "))
                    || (key.starts_with('N') && rest.starts_with(" ["))
                    || (key.starts_with('G')
                        && ((state && rest.starts_with(" [")) || rest.starts_with(" vertex [")))
            })
        });
        if let Some(continuation) = line.strip_prefix("  ").filter(|_| formatted_record) {
            records
                .last_mut()
                .expect("a preceding record")
                .push_str(continuation);
            continue;
        }
        let keyed = reader_key(line).is_some_and(|(_, rest)| rest.starts_with([' ', ':']));
        let boundary = keyed
            || matches!(line, "Nodes:" | "Groups:" | "Relationships:")
            || [
                "family: ",
                "direction: ",
                "Keys: ",
                "Markers ",
                "Stroke: ",
                "members ",
                "membership: ",
                "authored ",
                "edge defaults ",
                "graph details:",
            ]
            .iter()
            .any(|prefix| line.starts_with(prefix));
        if records.is_empty() || (boundary && !record_is_open(records.last().unwrap())) {
            records.push(line.to_owned());
        } else {
            records.last_mut().unwrap().push_str(line);
        }
    }
    records
}

#[derive(Debug)]
struct ReadableNode {
    key: String,
    kind: String,
    label: serde_json::Value,
}

#[derive(Debug)]
struct ReadableRelationship {
    key: String,
    from: String,
    connector: String,
    to: String,
    label: String,
    invisible: bool,
    declarations: [bool; 2],
}

#[derive(Debug, Default)]
struct ReadableGraph {
    nodes: Vec<ReadableNode>,
    relationships: Vec<ReadableRelationship>,
    memberships: Vec<(String, Vec<String>)>,
}

fn quoted_text(input: &mut &str) -> String {
    *input = input.strip_prefix('"').expect("a quoted authored field");
    let mut value = String::new();
    loop {
        let ch = input.chars().next().expect("a closing quote");
        *input = &input[ch.len_utf8()..];
        match ch {
            '"' => return value,
            '\\' => {
                let escape = input.chars().next().expect("a complete escape");
                *input = &input[escape.len_utf8()..];
                value.push(match escape {
                    '"' => '"',
                    '\\' => '\\',
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    'u' => {
                        let (hex, rest) = input
                            .strip_prefix('{')
                            .and_then(|rest| rest.split_once('}'))
                            .expect("a visible Unicode escape");
                        *input = rest;
                        char::from_u32(u32::from_str_radix(hex, 16).unwrap()).unwrap()
                    }
                    _ => panic!("unknown quoted escape {escape:?}"),
                });
            }
            _ => value.push(ch),
        }
    }
}

fn endpoint(input: &mut &str, graph: &mut ReadableGraph) -> (String, bool) {
    let (key, rest) = reader_key(input).expect("a reader endpoint key");
    assert!(key.starts_with(['N', 'G']), "endpoint {key:?}");
    let key = key.to_owned();
    *input = rest;
    let vertex = input.starts_with(" vertex [");
    if vertex {
        *input = input.strip_prefix(" vertex").unwrap();
    }
    if let Some(rest) = input.strip_prefix(" [") {
        let (kind, rest) = rest.split_once("]: ").expect("a typed declaration");
        *input = rest;
        let label = if input.starts_with('"') {
            serde_json::Value::String(quoted_text(input))
        } else {
            let mut values =
                serde_json::Deserializer::from_str(input).into_iter::<serde_json::Value>();
            let label = values.next().expect("a structured state label").unwrap();
            *input = &input[values.byte_offset()..];
            label
        };
        assert!(
            vertex || !graph.nodes.iter().any(|node| node.key == key),
            "duplicate declaration of {key}"
        );
        graph.nodes.push(ReadableNode {
            key: key.clone(),
            kind: kind.to_owned(),
            label,
        });
        (key, true)
    } else {
        assert!(
            graph.nodes.iter().any(|node| node.key == key),
            "reference to undeclared endpoint {key}"
        );
        (key, false)
    }
}

// Independently recover graph facts from the reader syntax. Labels are parsed as framed fields,
// never searched for endpoint/arrow substrings; every reference must already have a declaration.
fn readable_graph(text: &str) -> ReadableGraph {
    let mut graph = ReadableGraph::default();
    for record in records(text) {
        if let Some((group, members)) = record
            .strip_prefix("members ")
            .and_then(|rest| rest.split_once(": "))
        {
            let members = members
                .split(", ")
                .filter(|member| !member.is_empty())
                .map(str::to_owned)
                .collect();
            graph.memberships.push((group.to_owned(), members));
            continue;
        }
        let Some((key, rest)) = reader_key(&record) else {
            continue;
        };
        if key.starts_with(['N', 'G']) && (rest.starts_with(" [") || rest.starts_with(" vertex ["))
        {
            let mut input = record.as_str();
            endpoint(&mut input, &mut graph);
            assert!(input.is_empty(), "unread declaration: {input:?}");
        } else if key.starts_with('E') && rest.starts_with(": ") {
            let mut input = rest.strip_prefix(": ").unwrap();
            let (from, source_declaration) = endpoint(&mut input, &mut graph);
            let (connector, rest) = input
                .strip_prefix(' ')
                .and_then(|rest| rest.split_once(' '))
                .expect("a relationship connector");
            let invisible = rest.starts_with("(invisible constraint) ");
            input = rest.strip_prefix("(invisible constraint) ").unwrap_or(rest);
            let (to, target_declaration) = endpoint(&mut input, &mut graph);
            let label = if let Some(rest) = input.strip_prefix(": ") {
                input = rest;
                quoted_text(&mut input)
            } else {
                String::new()
            };
            assert!(input.is_empty(), "unread relationship: {input:?}");
            assert!(
                !graph.relationships.iter().any(|edge| edge.key == key),
                "duplicate relationship key {key}"
            );
            graph.relationships.push(ReadableRelationship {
                key: key.to_owned(),
                from,
                connector: connector.to_owned(),
                to,
                label,
                invisible,
                declarations: [source_declaration, target_declaration],
            });
        }
    }
    for (group, members) in &graph.memberships {
        assert!(group.starts_with('G') && graph.nodes.iter().any(|node| node.key == *group));
        for member in members {
            assert!(
                graph.nodes.iter().any(|node| node.key == *member),
                "unknown member {member}"
            );
        }
    }
    graph
}

fn key_for_label(text: &str, label: &str) -> String {
    readable_graph(text)
        .nodes
        .into_iter()
        .find(|node| node.label.as_str() == Some(label))
        .unwrap_or_else(|| panic!("missing keyed label {label:?}:\n{text}"))
        .key
}

fn assert_relationship(text: &str, from: &str, connector: &str, to: &str, label: &str) {
    assert!(
        readable_graph(text).relationships.iter().any(|edge| {
            edge.from == from && edge.connector == connector && edge.to == to && edge.label == label
        }),
        "missing relationship {from} {connector} {to}, label {label:?}:\n{text}"
    );
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn sixteen_way_td_is_a_readable_document_with_all_sixteen_real_relationships() {
    let source = include_str!("testdata/local-semantic/flowchart/presentation/fan-out-16-td.mmd");
    for width in [60, 80, 100, 120] {
        let report = fallback(source, width);
        assert_bounded(&report, width);
        assert!(report.emitted_extent.height <= 20, "{}", report.text);
        assert!(report.text.contains("direction: TB"), "{}", report.text);
        assert!(!report.text.contains("model.nodes["));
        assert!(!report.text.contains("Markers "), "{}", report.text);
        assert!(!report.text.contains("Stroke"), "{}", report.text);
        assert!(!report.text.contains("Nodes:"), "{}", report.text);
        let graph = readable_graph(&report.text);
        assert_eq!(graph.nodes.len(), 17, "{graph:?}");
        assert_eq!(graph.relationships.len(), 16, "{graph:?}");
        assert!(graph.nodes.iter().all(|node| node.kind == "squareRect"));
        let root = key_for_label(&report.text, "Dispatcher");
        for job in 1..=16 {
            let target = key_for_label(&report.text, &format!("Job {job}"));
            assert_relationship(&report.text, &root, "-->", &target, "");
        }
        assert!(
            graph.relationships.iter().all(|edge| edge.declarations[1]),
            "every private target must carry its declaration at the relationship: {graph:?}"
        );
        assert!(graph.relationships[0].declarations[0]);
        assert!(
            graph.relationships[1..]
                .iter()
                .all(|edge| !edge.declarations[0])
        );
    }
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn long_labels_keep_their_complete_text_and_private_branch_associations() {
    let source = include_str!("testdata/local-semantic/flowchart/presentation/long-labels.mmd");
    let report = fallback(source, 80);
    assert_bounded(&report, 80);
    assert!(report.emitted_extent.height <= 17, "{}", report.text);
    assert!(
        report.text.contains("[diamond]"),
        "split type cue:\n{}",
        report.text
    );
    assert!(
        report.text.contains("required"),
        "split ordinary word:\n{}",
        report.text
    );
    assert!(
        report.text.lines().any(|line| line.starts_with("  ")),
        "wrapped records need a visible continuation indent:\n{}",
        report.text,
    );
    let graph = readable_graph(&report.text);
    assert_eq!(graph.nodes.len(), 5, "{graph:?}");
    assert_eq!(graph.relationships.len(), 5, "{graph:?}");
    let decision = key_for_label(&report.text, "Is the production environment approved?");
    let wait = key_for_label(&report.text, "Wait for the authorized operator");
    let validate = key_for_label(&report.text, "Run validation and collect detailed results");
    let read = key_for_label(&report.text, "Read request and identify the environment");
    let result = key_for_label(&report.text, "Report the final outcome to the user");
    assert_relationship(&report.text, &read, "-->", &decision, "");
    assert_relationship(
        &report.text,
        &decision,
        "-->",
        &wait,
        "Approval is required before production writes",
    );
    assert_relationship(
        &report.text,
        &decision,
        "-->",
        &validate,
        "The operation is read only",
    );
    assert_relationship(&report.text, &wait, "-->", &result, "");
    assert_relationship(&report.text, &validate, "-->", &result, "");
    assert!(
        graph
            .nodes
            .iter()
            .any(|node| node.key == decision && node.kind == "diamond")
    );
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn first_use_declarations_keep_merges_cycles_duplicate_labels_and_isolated_nodes() {
    let source = concat!(
        "flowchart LR\n",
        "A[Source with its complete authored description] --> B[Repeated label]\n",
        "A --> C[Repeated label]\n",
        "B --> D{Approval gate}\n",
        "C --> D\n",
        "D --> A\n",
        "A --> A\n",
        "L[Local loop] --> L\n",
        "Z[Isolated node with its complete authored description]\n",
    );
    let report = fallback(source, 60);
    assert_bounded(&report, 60);
    let graph = readable_graph(&report.text);
    assert_eq!(graph.nodes.len(), 6, "{graph:?}");
    assert_eq!(graph.relationships.len(), 7, "{graph:?}");
    assert_eq!(
        graph
            .nodes
            .iter()
            .filter(|node| node.label == "Repeated label")
            .count(),
        2
    );
    let source = key_for_label(
        &report.text,
        "Source with its complete authored description",
    );
    let gate = key_for_label(&report.text, "Approval gate");
    let isolated = key_for_label(
        &report.text,
        "Isolated node with its complete authored description",
    );
    let left = &graph.relationships[0].to;
    let right = &graph.relationships[1].to;
    assert_ne!(left, right, "same-label nodes need separate identities");
    assert_relationship(&report.text, &source, "-->", left, "");
    assert_relationship(&report.text, &source, "-->", right, "");
    assert_relationship(&report.text, left, "-->", &gate, "");
    assert_relationship(&report.text, right, "-->", &gate, "");
    assert_relationship(&report.text, &gate, "-->", &source, "");
    assert_relationship(&report.text, &source, "-->", &source, "");
    let local_loop = key_for_label(&report.text, "Local loop");
    assert_relationship(&report.text, &local_loop, "-->", &local_loop, "");
    assert_eq!(graph.relationships[0].declarations, [true, true]);
    assert_eq!(graph.relationships[1].declarations, [false, true]);
    assert_eq!(graph.relationships[2].declarations, [false, true]);
    assert!(
        graph.relationships[3..6]
            .iter()
            .all(|edge| edge.declarations == [false, false])
    );
    assert_eq!(graph.relationships[6].declarations, [true, false]);
    assert!(
        graph
            .relationships
            .iter()
            .all(|edge| edge.from != isolated && edge.to != isolated)
    );
    assert!(
        records(&report.text)
            .iter()
            .any(|record| record.starts_with(&format!("{isolated} [")))
    );
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn inline_quoted_fields_keep_escapes_and_cannot_fabricate_graph_records() {
    use merman_ascii::{AsciiDiagram, AsciiSourceContext};
    use merman_model::{MermaidConfig, families::flowchart::FlowchartModel};
    use serde_json::json;

    let from_label = "Owner \"private\" \\ path\nN999 [diamond]: \"spoof\" --> N888\u{202e} e\u{301} 👩‍💻  \u{301} \u{600} ";
    let to_label = "Destination\nE999: N999 --> N888";
    let condition = "Private \"condition\" \\n E123: N999 --> N888\t保留";
    let model: FlowchartModel = serde_json::from_value(json!({
        "direction": "LR",
        "nodes": [
            {"id": "a", "label": from_label, "shape": "square", "layoutShape": "squareRect"},
            {"id": "b", "label": to_label, "shape": "square", "layoutShape": "squareRect"}
        ],
        "edges": [
            {"id": "generated", "from": "a", "to": "b", "length": 1,
             "endMarker": "point", "label": condition}
        ]
    }))
    .unwrap();
    let config = MermaidConfig::default();
    for width in [19, 60] {
        let operation = RuntimePolicy::deterministic().begin_operation().unwrap();
        let report = AsciiRenderer::new(
            AsciiRenderOptions::ascii().with_flowchart_node_label_wrap_width(500),
        )
        .unwrap()
        .render_model_report(
            &AsciiDiagram::Flowchart(&model),
            AsciiSourceContext {
                diagram_type: "flowchart-v2",
                config: &config,
                semantic_config: &config,
                title: None,
            },
            AsciiViewportPolicy::with_max_width(width).overflow(OverflowPolicy::Fallback),
            &OperationControl::new(),
            &operation,
            AsciiResourcePolicy::unbounded(),
        )
        .unwrap();
        assert_bounded(&report, width);
        let graph = readable_graph(&report.text);
        assert_eq!(graph.nodes.len(), 2, "{graph:?}");
        assert_eq!(graph.relationships.len(), 1, "{graph:?}");
        let from = key_for_label(&report.text, from_label);
        let to = key_for_label(&report.text, to_label);
        assert_relationship(&report.text, &from, "-->", &to, condition);
        assert!(
            graph.relationships[0]
                .declarations
                .iter()
                .all(|declared| *declared)
        );
        assert!(!report.text.contains('\u{202e}'));
        assert!(records(&report.text).concat().contains("\\u{202E}"));
    }
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn wrapped_cjk_and_distinguishing_suffixes_keep_separate_identities() {
    let prefix = "请求处理审批状态与客户环境标识".repeat(8);
    let left = format!("{prefix}-production-甲");
    let right = format!("{prefix}-staging-乙");
    let condition = "只允许审核后的请求进入生产环境".repeat(7);
    let source = format!("flowchart LR\nA[\"{left}\"] -->|{condition}| B[\"{right}\"]\n");
    for width in [60, 80, 100, 120] {
        let report = render(
            &source,
            width,
            AsciiRenderOptions::ascii()
                .with_terminal_width_profile(TerminalWidthProfile::Cjk)
                .with_flowchart_node_label_wrap_width(500),
            AsciiResourcePolicy::unbounded(),
            &OperationControl::new(),
        )
        .unwrap();
        assert_bounded(&report, width);
        let from = key_for_label(&report.text, &left);
        let to = key_for_label(&report.text, &right);
        assert_ne!(from, to);
        assert_relationship(&report.text, &from, "-->", &to, &condition);
    }
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn wrapped_group_titles_keep_authored_leading_spaces() {
    let source = "flowchart LR\nsubgraph G[xxxxxx  after]\nA[One]\nend\nB[External consumer with complete descriptive text]\nA --> B";
    let report = fallback(source, 19);
    assert_bounded(&report, 19);
    let group = key_for_label(&report.text, "xxxxxx  after");
    let member = key_for_label(&report.text, "One");
    assert!(
        readable_graph(&report.text)
            .memberships
            .iter()
            .any(|(owner, members)| { owner == &group && members.contains(&member) })
    );
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn group_endpoints_are_distinct_from_their_members_and_local_directions_survive() {
    let source = concat!(
        "flowchart LR\n",
        "subgraph Outer[Outer team]\n",
        "direction TB\n",
        "subgraph Inner[Inner team]\n",
        "A[Ingress] --> B[Worker]\n",
        "end\nend\n",
        "C[External consumer with a deliberately wide descriptive label]\n",
        "Outer -->|whole team| C\n",
        "B -->|private work| C\n",
    );
    let report = fallback(source, 60);
    assert_bounded(&report, 60);
    let group = key_for_label(&report.text, "Outer team");
    let inner = key_for_label(&report.text, "Inner team");
    let worker = key_for_label(&report.text, "Worker");
    let consumer = key_for_label(
        &report.text,
        "External consumer with a deliberately wide descriptive label",
    );
    assert!(group.starts_with('G'));
    assert!(worker.starts_with('N'));
    let graph = readable_graph(&report.text);
    assert!(
        graph
            .memberships
            .iter()
            .any(|(owner, members)| owner == &group && members.contains(&inner))
    );
    assert!(
        graph
            .memberships
            .iter()
            .any(|(owner, members)| owner == &inner && members.contains(&worker))
    );
    assert!(
        graph
            .nodes
            .iter()
            .any(|node| node.key == group && node.kind.contains("direction=TB (explicit)"))
    );
    let joined = records(&report.text).concat();
    assert!(
        joined.contains(&format!("members {group}: {inner}")),
        "{joined}"
    );
    assert!(joined.contains(&format!("members {inner}:")), "{joined}");
    assert!(joined.contains("direction=TB"), "{joined}");
    assert_relationship(&report.text, &group, "-->", &consumer, "whole team");
    assert_relationship(&report.text, &worker, "-->", &consumer, "private work");
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn marker_stroke_parallel_edges_and_authored_details_are_not_collapsed() {
    use merman_ascii::{AsciiDiagram, AsciiSourceContext};
    use merman_model::{MermaidConfig, families::flowchart::FlowchartModel};
    use serde_json::json;

    let label = "A full authored label with a distinguishing suffix ".repeat(4);
    let model: FlowchartModel = serde_json::from_value(json!({
        "direction": "RL",
        "accTitle": "Accessible graph title",
        "nodes": [
            {"id": "private-source", "label": label, "shape": "round",
             "layoutShape": "diamond", "link": "https://example.org/authored",
             "classes": ["review"], "styles": ["fill:#123456"]},
            {"id": "private-target", "label": "Destination", "shape": "circle",
             "layoutShape": "circle", "haveCallback": true}
        ],
        "edges": [
            {"id": "named-edge", "isUserDefinedId": true,
             "from": "private-source", "to": "private-target", "length": 2,
             "startMarker": "circle", "endMarker": "cross", "strokeKind": "dotted",
             "label": "private branch", "step": 3, "animate": true},
            {"id": "generated-one", "from": "private-source", "to": "private-target",
             "length": 1, "endMarker": "point", "strokeKind": "thick",
             "label": "same branch", "animate": false},
            {"id": "generated-two", "from": "private-source", "to": "private-target",
             "length": 1, "endMarker": "point", "strokeKind": "thick",
             "label": "same branch"},
            {"id": "rank-only", "from": "private-target", "to": "private-source",
             "length": 1, "visibility": "invisible"},
            {"id": "generated-return", "from": "private-target", "to": "private-source",
             "length": 1, "startMarker": "point", "endMarker": "circle",
             "label": "return only"}
        ],
        "subgraphs": [{"id": "team", "title": "Team", "nodes": ["private-source"],
                       "dir": null, "metadata": {"payload": {}}}]
    }))
    .unwrap();
    let config = MermaidConfig::from_value(json!({"flowchart": {"curve": "linear"}}));
    let operation = RuntimePolicy::deterministic().begin_operation().unwrap();
    let report =
        AsciiRenderer::new(AsciiRenderOptions::ascii().with_flowchart_node_label_wrap_width(500))
            .unwrap()
            .render_model_report(
                &AsciiDiagram::Flowchart(&model),
                AsciiSourceContext {
                    diagram_type: "flowchart-v2",
                    config: &config,
                    semantic_config: &config,
                    title: Some("Authored source title"),
                },
                AsciiViewportPolicy::with_max_width(80).overflow(OverflowPolicy::Fallback),
                &OperationControl::new(),
                &operation,
                AsciiResourcePolicy::unbounded(),
            )
            .unwrap();
    assert_bounded(&report, 80);
    let source = key_for_label(&report.text, &label);
    let target = key_for_label(&report.text, "Destination");
    assert!(
        records(&report.text).iter().any(|record| {
            record.starts_with(&format!("{source} details:")) && record.contains("shape:\"round\"")
        }),
        "the source shape must stay with its owner alongside the resolved diamond: {}",
        report.text
    );
    assert_relationship(&report.text, &source, "o..x", &target, "private branch");
    assert_relationship(&report.text, &target, "<--o", &source, "return only");
    let graph = readable_graph(&report.text);
    assert!(
        graph
            .nodes
            .iter()
            .any(|node| node.key == source && node.kind == "diamond")
    );
    assert!(
        graph
            .nodes
            .iter()
            .any(|node| node.key == target && node.kind == "circle")
    );
    let edge_keys: std::collections::BTreeSet<_> = graph
        .relationships
        .iter()
        .map(|edge| edge.key.as_str())
        .collect();
    assert_eq!(edge_keys.len(), 5, "{}", report.text);
    assert_eq!(
        graph
            .relationships
            .iter()
            .filter(|edge| {
                edge.from == source
                    && edge.connector == "==>"
                    && edge.to == target
                    && edge.label == "same branch"
            })
            .count(),
        2,
        "parallel relationships need independent keys: {}",
        report.text
    );
    assert!(
        graph.relationships.iter().any(|edge| {
            edge.from == target && edge.to == source && edge.connector == "--" && edge.invisible
        }),
        "the invisible constraint needs its own real endpoints: {}",
        report.text
    );
    let joined = records(&report.text).concat();
    for value in [
        "direction: RL",
        "diamond",
        "circle",
        "startMarker",
        "endMarker",
        "invisible",
        "Accessible graph title",
        "Authored source title",
        "linear",
        "https://example.org/authored",
        "review",
        "fill:#123456",
        "haveCallback",
        "named-edge",
        "step",
        "animate",
        "payload",
    ] {
        assert!(joined.contains(value), "missing {value:?}: {}", report.text);
    }
    assert!(!joined.contains("generated-one"));
    assert!(!joined.contains("generated-two"));
    assert!(!joined.contains("generated-return"));
    assert!(
        joined.contains("metadata:{\"payload\":{}}"),
        "{}",
        report.text
    );
    assert!(joined.contains("animate:false"), "{}", report.text);
}

#[cfg(feature = "diagram-state")]
#[test]
fn state_document_keeps_composites_pseudostates_notes_and_transition_conditions() {
    let source = concat!(
        "stateDiagram-v2\n",
        "direction LR\n",
        "state Session {\n",
        "direction TB\n",
        "[*] --> Ready\n",
        "Ready --> Running: permission granted for the complete production operation\n",
        "Running --> [*]: finished\n",
        "note right of Ready: Only authorized operators may start the operation\n",
        "}\n",
        "Session --> Closed: close the complete session\n",
    );
    let report = fallback(source, 60);
    assert_bounded(&report, 60);
    let ready = key_for_label(&report.text, "Ready");
    let running = key_for_label(&report.text, "Running");
    let session = key_for_label(&report.text, "Session");
    let closed = key_for_label(&report.text, "Closed");
    let start = key_for_label(&report.text, "start");
    let end = key_for_label(&report.text, "end");
    assert!(session.starts_with('G'));
    assert_relationship(
        &report.text,
        &ready,
        "-->",
        &running,
        "permission granted for the complete production operation",
    );
    assert_relationship(
        &report.text,
        &session,
        "-->",
        &closed,
        "close the complete session",
    );
    assert_relationship(&report.text, &start, "-->", &ready, "");
    assert_relationship(&report.text, &running, "-->", &end, "finished");
    let joined = records(&report.text).concat();
    for required in [
        "direction: LR",
        "direction=TB",
        "start",
        "end",
        "note",
        "Only authorized operators may start the operation",
        "members G",
    ] {
        assert!(
            joined.contains(required),
            "missing {required:?}: {}",
            report.text
        );
    }
}

#[cfg(feature = "diagram-state")]
#[test]
fn state_authored_details_keep_descriptions_styles_classes_and_click_ownership() {
    let source = concat!(
        "stateDiagram-v2\n",
        "direction LR\n",
        "state Outer {\n",
        "state \"Human state label before authorization\" as A\n",
        "A : A second authored description that remains complete\n",
        "A --> B: transition with an authored condition\n",
        "note right of A : Remember the original authorization context\n",
        "}\n",
        "classDef active fill:red,color:blue\n",
        "class A active\n",
        "style A color:green\n",
        "click A href \"https://example.org/authorize\"\n",
    );
    let report = fallback(source, 20);
    assert_bounded(&report, 20);
    let joined = records(&report.text).concat();
    for value in [
        "Human state label before authorization",
        "A second authored description that remains complete",
        "transition with an authored condition",
        "Remember the original authorization context",
        "fill:red",
        "color:blue",
        "color:green",
        "active",
        "https://example.org/authorize",
        "authored D",
        "applyClass",
        "click",
    ] {
        assert!(joined.contains(value), "missing {value:?}: {}", report.text);
    }
    let node = records(&report.text)
        .into_iter()
        .find(|record| {
            record.starts_with('N') && record.contains("Human state label before authorization")
        })
        .expect("the authored state should have a reader identity");
    let key = node.split_whitespace().next().unwrap();
    assert!(joined.contains(&format!("{key} links:")), "{}", report.text);
    assert!(joined.contains(&format!("click {key}")), "{}", report.text);
    assert!(joined.contains("direction=inherited"), "{}", report.text);
    assert!(!joined.contains("direction=TB"), "{}", report.text);
}

#[cfg(feature = "diagram-swimlane")]
#[test]
fn swimlane_uses_the_shared_graph_document_with_its_own_family_identity() {
    let source = concat!(
        "swimlane-beta LR\n",
        "subgraph Team[Operations team]\n",
        "A[Review the complete request and its production environment] --> B[Approve]\n",
        "end\n",
        "B -->|authorized| C[Execute the approved operation and report results]\n",
    );
    let report = fallback(source, 60);
    assert_bounded(&report, 60);
    assert_eq!(report.family, "swimlane");
    let approve = key_for_label(&report.text, "Approve");
    let execute = key_for_label(
        &report.text,
        "Execute the approved operation and report results",
    );
    assert_relationship(&report.text, &approve, "-->", &execute, "authorized");
    assert!(records(&report.text).concat().contains("members G"));
}

#[cfg(feature = "diagram-sequence")]
#[test]
fn sequence_retains_its_existing_complete_projection_fallback() {
    let report = fallback(
        concat!(
            "sequenceDiagram\n",
            "participant A\n",
            "properties A: {\"payload\": {}}\n",
            "participant B\n",
            "A->>B: Preserve the complete authored acknowledgment message\n",
        ),
        12,
    );
    assert_bounded(&report, 12);
    let joined = report.text.replace('\n', "");
    assert!(
        joined.contains("model.actors.A.properties.payload: {}"),
        "{}",
        report.text
    );
    assert!(joined.contains("Preserve the complete authored acknowledgment message"));
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn graph_document_admits_exact_cells_and_honors_cumulative_work_and_cancellation() {
    let source = "flowchart LR\nA[Alpha] --> B[Beta]";
    let baseline = fallback(source, 5);
    let cells: usize = baseline.text.lines().map(UnicodeWidthStr::width).sum();
    let exact = render(
        source,
        5,
        AsciiRenderOptions::ascii(),
        AsciiResourcePolicy::unbounded()
            .with_limit(AsciiResourceLimitId::MaxDocumentCells, cells)
            .unwrap(),
        &OperationControl::new(),
    )
    .unwrap();
    assert_eq!(exact.text, baseline.text);
    let error = render(
        source,
        5,
        AsciiRenderOptions::ascii(),
        AsciiResourcePolicy::unbounded()
            .with_limit(AsciiResourceLimitId::MaxDocumentCells, cells - 1)
            .unwrap(),
        &OperationControl::new(),
    )
    .unwrap_err();
    assert!(matches!(error, AsciiError::ResourceLimitExceeded(details)
        if details.limit == AsciiResourceLimitId::MaxDocumentCells));

    let error = render(
        source,
        5,
        AsciiRenderOptions::ascii(),
        AsciiResourcePolicy::unbounded()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 5_000)
            .unwrap(),
        &OperationControl::new(),
    )
    .unwrap_err();
    assert!(matches!(error, AsciiError::ResourceLimitExceeded(details)
        if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits));

    // Find a late cooperative checkpoint through the public API, without assuming how many
    // checkpoints primary layout or bounded semantic serialization currently consume.
    let mut rejected = 0usize;
    let mut accepted = 1usize;
    let cancelled_render = |checkpoints| {
        let control = OperationControl::new();
        control.cancel_after_checkpoints(checkpoints);
        render(
            source,
            5,
            AsciiRenderOptions::ascii(),
            AsciiResourcePolicy::unbounded(),
            &control,
        )
    };
    loop {
        match cancelled_render(accepted) {
            Ok(_) => break,
            Err(AsciiError::Cancelled(_)) => {
                rejected = accepted;
                accepted = accepted.checked_mul(2).unwrap();
            }
            Err(error) => panic!("unexpected cancellation probe error: {error}"),
        }
    }
    while accepted - rejected > 1 {
        let middle = rejected + (accepted - rejected) / 2;
        match cancelled_render(middle) {
            Ok(_) => accepted = middle,
            Err(AsciiError::Cancelled(_)) => rejected = middle,
            Err(error) => panic!("unexpected cancellation probe error: {error}"),
        }
    }
    assert!(
        matches!(cancelled_render(accepted - 1), Err(AsciiError::Cancelled(cancelled))
        if cancelled.phase == merman_model::OperationPhase::Emit)
    );
    assert_eq!(cancelled_render(accepted).unwrap().text, baseline.text);
}
