#![cfg(feature = "diagram-agentflow")]

mod support;

use merman_ascii::{
    AsciiColorMode, AsciiDiagram, AsciiError, AsciiFallbackReason, AsciiGraphPresentation,
    AsciiLayoutProfile, AsciiOutput, AsciiOutputEncoding, AsciiOutputOutcome, AsciiProjection,
    AsciiRenderOptions, AsciiRenderer, AsciiResourceLimitId, AsciiResourcePolicy,
    AsciiSourceContext, AsciiTrimPolicy, AsciiViewportPolicy, Lossiness, OverflowPolicy,
    TerminalWidthProfile,
};
use merman_core::SemanticParser;
use merman_model::families::agentflow::{
    AgentflowDiagramModel, AgentflowEdgeSemantic, AgentflowVertexKind,
};
use merman_model::{
    MermaidConfig, OperationControl, ParsedSemanticDiagram, SemanticDiagramModel,
    runtime::RuntimePolicy,
};

const SMALL: &str = include_str!("../../../fixtures/agentflow/basic.mmd");
const COLLAPSED: &str =
    include_str!("../../../fixtures/agentflow/local_nested_collapse_routes.mmd");

const TYPICAL: &str = r#"agentflow-beta TB
flow support["Support"]
  question["Question"]@{shape: input}
  search["Search"]@{shape: tool}
  reply["Reply"]@{shape: task}
  question -->|query| search
  search -->|answer| reply
end
connector api["Service"]
reply -->|send| api
"#;

const DENSE: &str = r#"agentflow-beta TB
A["Intake"]
B["Search"]@{shape: tool}
C["Check"]@{shape: decision}
D["Compose"]@{shape: action}
E["Manual"]@{shape: refdoc}
connector F["Service"]
A -->|ab| B
A -->|ac| C
B -->|bd| D
C -->|cd| D
B -.- |be| E
C -- cf --x F
D -->|de| E
E -->|ef| F
"#;

const ROLES: &str = r#"agentflow-beta TB
task["Queue"]@{shape: task}
tool["Search"]@{shape: tool}
action["Transform"]@{shape: action}
input["Prompt"]@{shape: input}
refdoc["Manual"]@{shape: refdoc}
decision["Approve"]@{shape: decision}
connector api["Service"]
"#;

const RICH: &str = r#"---
title: Agent work
config:
  htmlLabels: false
  agentflow:
    defaultRenderer: elk
---
agentflow-beta LR
accTitle: Accessible work
accDescr: A complete agent workflow
connector api["Search API"]@{options: {modes: [read, write], timeout: 30}, protocol: http}
flow worker["Worker"]
  direction RL
  request["Ask"]@{shape: input, value: {attempts: [1, false, null], question: "为什么"}}
  lookup["Fetch with spaces"]@{shape: tool, instruction: 'Call "search" now', params: {limit: 3, strict: true}, connectorRef: api.search}
  failed["Escalate"]@{shape: action}
  request run@-->|"Run lookup"| lookup
  lookup docs@-.- |"Read docs"| api
  lookup fail@--x failed
  request repeat@--> request
end
run@{instruction: handoff, retry: 2}
"#;

fn parsed(source: &str) -> ParsedSemanticDiagram {
    let parsed = SemanticParser::new()
        .parse_diagram_sync(source)
        .expect("Agentflow source should parse");
    assert_eq!(parsed.metadata.diagram_type, "agentflow");
    assert!(matches!(parsed.model, SemanticDiagramModel::Agentflow(_)));
    parsed
}

fn model(source: &str) -> AgentflowDiagramModel {
    let SemanticDiagramModel::Agentflow(model) = parsed(source).model else {
        panic!("expected canonical Agentflow data");
    };
    model
}

fn controlled_report(
    parsed: &ParsedSemanticDiagram,
    options: AsciiRenderOptions,
    viewport: AsciiViewportPolicy,
    control: &OperationControl,
    resources: AsciiResourcePolicy,
) -> merman_ascii::Result<AsciiOutput> {
    let operation = RuntimePolicy::deterministic().begin_operation().unwrap();
    AsciiRenderer::new(options)?
        .render_parsed_report(parsed, viewport, control, &operation, resources)
}

fn report(
    source: &str,
    options: AsciiRenderOptions,
    viewport: AsciiViewportPolicy,
) -> merman_ascii::Result<AsciiOutput> {
    controlled_report(
        &parsed(source),
        options,
        viewport,
        &OperationControl::new(),
        AsciiResourcePolicy::default(),
    )
}

fn direct_report(model: &AgentflowDiagramModel) -> merman_ascii::Result<AsciiOutput> {
    let operation = RuntimePolicy::deterministic().begin_operation().unwrap();
    let config = MermaidConfig::empty_object();
    AsciiRenderer::new(AsciiRenderOptions::ascii())?.render_model_report(
        &AsciiDiagram::Agentflow(model),
        AsciiSourceContext {
            diagram_type: "agentflow",
            config: &config,
            semantic_config: &config,
            title: None,
        },
        AsciiViewportPolicy::unrestricted(),
        &OperationControl::new(),
        &operation,
        AsciiResourcePolicy::default(),
    )
}

fn assert_measurement(output: &AsciiOutput) {
    let (width, height) = support::terminal_extent_with_profile(&output.text, output.width_profile);
    assert_eq!(output.family, "agentflow");
    assert_eq!(output.encoding, AsciiOutputEncoding::Plain);
    assert_eq!(output.emitted_extent.width, width, "{}", output.text);
    assert_eq!(output.emitted_extent.height, height, "{}", output.text);
    assert_eq!(
        output.requested_layout_profile,
        AsciiLayoutProfile::Canonical
    );
    assert!(!output.compact_attempted);
}

fn assert_primary(output: &AsciiOutput, projection: AsciiProjection) {
    assert_measurement(output);
    assert_eq!(output.projection, projection, "{}", output.text);
    assert_eq!(output.outcome, AsciiOutputOutcome::Primary);
    assert_eq!(output.primary_extent, Some(output.emitted_extent));
    assert_eq!(output.primary_overflowed, Some(false));
    assert!(!output.emitted_overflowed);
    assert!(!output.fallback.attempted);
    assert_eq!(output.lossiness, Lossiness::None);
}

fn assert_contains(text: &str, expected: &[&str]) {
    for expected in expected {
        assert!(text.contains(expected), "missing {expected}:\n{text}");
    }
}

fn assert_records(text: &str, expected: &[&str]) {
    // Reading records wrap at their natural width; test semantic content independently of
    // continuation indentation. The viewport test separately checks every authored space.
    let compact = text.split_whitespace().collect::<String>();
    for expected in expected {
        let expected = expected.split_whitespace().collect::<String>();
        assert!(compact.contains(&expected), "missing {expected}:\n{text}");
    }
}

fn position(text: &str, label: &str) -> (usize, usize) {
    text.lines()
        .enumerate()
        .find_map(|(y, line)| line.find(label).map(|x| (x, y)))
        .unwrap_or_else(|| panic!("missing {label}:\n{text}"))
}

#[test]
fn empty_source_reports_empty_output_without_inventing_visible_work() {
    let output = report(
        "agentflow-beta\n",
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::with_max_width(80).overflow(OverflowPolicy::Fallback),
    )
    .unwrap();
    assert_measurement(&output);
    assert_eq!(output.projection, AsciiProjection::Diagrammatic);
    assert_eq!(output.outcome, AsciiOutputOutcome::Empty);
    assert!(output.text.is_empty());
    assert_eq!(output.emitted_extent.width, 0);
    assert_eq!(output.emitted_extent.height, 0);
    assert!(!output.fallback.attempted);
    assert_eq!(output.lossiness, Lossiness::None);
}

#[test]
fn small_and_typical_are_spatial_at_80_100_120_in_both_charsets() {
    for (source, labels) in [
        (
            SMALL,
            &["Support", "Question", "Resolve", "[input]", "[task]"][..],
        ),
        (
            TYPICAL,
            &[
                "Support", "Question", "Search", "Reply", "Service", "query", "answer", "send",
            ][..],
        ),
    ] {
        for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
            for width in [80, 100, 120] {
                let output = report(
                    source,
                    options,
                    AsciiViewportPolicy::with_max_width(width).overflow(OverflowPolicy::Fallback),
                )
                .unwrap_or_else(|error| panic!("width {width}: {error}"));
                assert_primary(&output, AsciiProjection::Diagrammatic);
                assert!(output.emitted_extent.width <= width);
                assert_contains(&output.text, labels);
                support::assert_rectangular_terminal_grid(&output.text);
            }
        }
    }
}

#[test]
fn cjk_emoji_and_combining_labels_keep_terminal_cell_alignment() {
    let source = "agentflow-beta TB\nflow team[\"支持团队\"]\n A[\"客户👩‍💻\"]@{shape: input}\n B[\"核对金额e\u{301}·\"]@{shape: tool}\n A -->|确认| B\nend\nconnector api[\"接口\"]\nB --> api\n";
    for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
        let output = report(
            source,
            options.with_terminal_width_profile(TerminalWidthProfile::Cjk),
            AsciiViewportPolicy::with_max_width(100).overflow(OverflowPolicy::Fallback),
        )
        .unwrap();
        assert_primary(&output, AsciiProjection::Diagrammatic);
        assert_eq!(output.width_profile, TerminalWidthProfile::Cjk);
        assert_contains(
            &output.text,
            &["支持团队", "客户👩‍💻", "核对金额e\u{301}·", "确认", "接口"],
        );
        support::assert_rectangular_terminal_grid_with_profile(
            &output.text,
            TerminalWidthProfile::Cjk,
        );
    }
}

#[test]
fn seven_roles_and_shape_only_metadata_keep_distinct_visible_meaning() {
    let model = model(ROLES);
    assert_eq!(
        model
            .vertices
            .iter()
            .map(|node| node.vertex_kind)
            .collect::<Vec<_>>(),
        [
            AgentflowVertexKind::Task,
            AgentflowVertexKind::Tool,
            AgentflowVertexKind::Action,
            AgentflowVertexKind::Input,
            AgentflowVertexKind::Refdoc,
            AgentflowVertexKind::Decision
        ]
    );
    assert_eq!(model.connectors[0].id, "api");
    assert!(model.vertices.iter().all(|node| node.id != "api"));
    assert!(
        model
            .vertices
            .iter()
            .all(|node| node.metadata.contains_key("shape"))
    );
    let output = report(
        ROLES,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    )
    .unwrap();
    assert_primary(&output, AsciiProjection::Diagrammatic);
    assert_contains(
        &output.text,
        &[
            "Queue",
            "Search",
            "Transform",
            "Prompt",
            "Manual",
            "Approve",
            "Service",
            "[task]",
            "[tool]",
            "[action]",
            "[input]",
            "[reference]",
            "[decision]",
            "[connector]",
        ],
    );
}

#[test]
fn sequence_reference_and_failure_keep_point_headless_dotted_and_cross_endpoints() {
    for (operator, semantic, required, forbidden) in [
        ("-->", AgentflowEdgeSemantic::Sequence, '>', Some('x')),
        ("-.-", AgentflowEdgeSemantic::Reference, '.', Some('>')),
        ("--x", AgentflowEdgeSemantic::Failure, 'x', Some('>')),
    ] {
        let source = format!("agentflow-beta LR\nA[Alpha]\nB[Beta]\nA {operator} B\n");
        let model = model(&source);
        assert_eq!(model.edges[0].edge_semantic, semantic);
        let output = report(
            &source,
            AsciiRenderOptions::ascii(),
            AsciiViewportPolicy::unrestricted(),
        )
        .unwrap();
        assert_primary(&output, AsciiProjection::Diagrammatic);
        assert!(
            output.text.contains(required),
            "{operator}:\n{}",
            output.text
        );
        if let Some(forbidden) = forbidden {
            assert!(
                !output.text.contains(forbidden),
                "{operator}:\n{}",
                output.text
            );
        }
        if semantic == AgentflowEdgeSemantic::Reference {
            assert!(!output.text.contains(['<', '^', 'v']), "{}", output.text);
        } else {
            let marker = position(&output.text, &required.to_string());
            let alpha = position(&output.text, "Alpha");
            let beta = position(&output.text, "Beta");
            assert!(alpha.0 < beta.0, "{}", output.text);
            assert!(marker.0 > (alpha.0 + beta.0) / 2, "{}", output.text);
        }
    }
}

#[test]
fn nested_flows_honor_local_direction_and_global_declarations_release_membership() {
    let source = "agentflow-beta TB\nglobal\n shared[Shared]@{shape: refdoc}\nend\nflow outer[Outer]\n flow inner[Inner]\n  direction LR\n  A[Alpha] --> B[Beta]\n end\nend\nB --> shared\n";
    let typed = model(source);
    let inner = typed
        .sub_graphs
        .iter()
        .find(|flow| flow.id == "inner")
        .unwrap();
    let outer = typed
        .sub_graphs
        .iter()
        .find(|flow| flow.id == "outer")
        .unwrap();
    assert_eq!(inner.direction.as_deref(), Some("LR"));
    assert_eq!(inner.nodes, ["B", "A"]);
    assert_eq!(outer.nodes, ["inner"]);
    assert_eq!(
        typed
            .vertices
            .iter()
            .find(|node| node.id == "shared")
            .unwrap()
            .parent_id,
        None
    );
    let output = report(
        source,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    )
    .unwrap();
    assert_primary(&output, AsciiProjection::Diagrammatic);
    let alpha = position(&output.text, "Alpha");
    let beta = position(&output.text, "Beta");
    assert!(alpha.0 < beta.0 && alpha.1 == beta.1, "{}", output.text);
    assert!(position(&output.text, "Outer").1 < position(&output.text, "Inner").1);
    assert!(position(&output.text, "Inner").1 < alpha.1);
    assert_contains(&output.text, &["Shared", "[flow]"]);

    let released = "agentflow-beta LR\nflow team[Team]\n direction TB\n A[Alpha] --> B[Beta]\nend\nglobal\n A\nend\n";
    let model = model(released);
    assert_eq!(model.sub_graphs[0].nodes, ["B"]);
    assert_eq!(
        model
            .vertices
            .iter()
            .find(|node| node.id == "A")
            .unwrap()
            .parent_id,
        None
    );
    let output = report(
        released,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    )
    .unwrap();
    assert_primary(&output, AsciiProjection::Diagrammatic);
    assert_contains(&output.text, &["Team", "Alpha", "Beta"]);
}

#[test]
fn rich_metadata_is_a_complete_structured_primary_with_source_ordered_records() {
    let model = model(RICH);
    assert_eq!(
        model.vertices[0].metadata["value"]["attempts"],
        serde_json::json!([1, false, null])
    );
    assert_eq!(
        model.vertices[1].metadata["params"],
        serde_json::json!({"limit": 3, "strict": true})
    );
    assert_eq!(model.connectors[0].metadata["options"]["timeout"], 30);
    let output = report(
        RICH,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    )
    .unwrap();
    assert_primary(&output, AsciiProjection::StructuredText);
    assert_records(
        &output.text,
        &[
            "Keys: N nodes; G flows; C connectors; E edges",
            "direction: \"LR\"",
            "N1 [input] id=\"request\" label=\"Ask\"",
            "N2 [tool] id=\"lookup\" label=\"Fetch with spaces\"",
            "N3 [action] id=\"failed\" label=\"Escalate\"",
            "G1 [flow] id=\"worker\" title=\"Worker\" type=\"flow\" direction=\"RL\" collapsed=false",
            "C1 [connector] id=\"api\" title=\"Search API\"",
            "E1 [sequence -->] source=[N1] id=\"request\" target=[N2] id=\"lookup\"; id=\"run\" label=\"Run lookup\" explicit-id=true",
            "E2 [reference -.-] source=[N2] id=\"lookup\" target=[C1] id=\"api\"; id=\"docs\" label=\"Read docs\"",
            "E3 [failure --x] source=[N2] id=\"lookup\" target=[N3] id=\"failed\"; id=\"fail\"",
            "E4 [sequence -->] source=[N1] id=\"request\" target=[N1] id=\"request\"; id=\"repeat\"",
            r#""attempts":[1,false,null]"#,
            r#""question":"为什么""#,
            r#""params":{"limit":3,"strict":true}"#,
            r#""connectorRef":"api.search""#,
            r#""instruction":"Call \"search\" now""#,
            r#""modes":["read","write"]"#,
            r#""timeout":30"#,
            r#""retry":2"#,
            r#"sourceTitle:"Agent work""#,
            r#"accTitle:"Accessible work""#,
            r#"accDescr:"A complete agent workflow""#,
            "sourceConfig:",
            r#""htmlLabels":false"#,
            r#""defaultRenderer":"elk""#,
        ],
    );
    assert!(output.emitted_extent.width <= 80);
    let compact = output.text.split_whitespace().collect::<String>();
    for (earlier, later) in [
        ("N1[", "N2["),
        ("N2[", "N3["),
        ("E1[", "E2["),
        ("E2[", "E3["),
        ("E3[", "E4["),
    ] {
        assert!(compact.find(earlier).unwrap() < compact.find(later).unwrap());
    }
}

#[test]
fn independently_parsed_rich_documents_are_deterministic_and_keep_class_declaration_order() {
    let source = format!(
        "{RICH}classDef zeta fill:#112233\nclassDef alpha stroke:#445566\n\
         style request stroke:blue\nstyle lookup fill:green\n\
         click request href \"https://example.com/request\" \"Request details\" _blank\n\
         click lookup href \"https://example.com/lookup\" \"Lookup details\" _blank\n"
    );
    let first = report(
        &source,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    )
    .unwrap();
    let second = report(
        &source,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    )
    .unwrap();
    assert_primary(&first, AsciiProjection::StructuredText);
    assert_eq!(
        first.text, second.text,
        "independent canonical parses must not expose randomized authored map order"
    );
    let compact = first.text.split_whitespace().collect::<String>();
    assert_contains(&compact, &["fill:#112233", "stroke:#445566"]);
    assert!(
        compact.find("fill:#112233").unwrap() < compact.find("stroke:#445566").unwrap(),
        "class definitions must retain declaration order, including nonalphabetic names:\n{}",
        first.text
    );
}

#[test]
fn collapsed_children_and_internal_edges_are_records_with_original_endpoints() {
    let model = model(COLLAPSED);
    assert_eq!(
        model
            .sub_graphs
            .iter()
            .find(|flow| flow.id == "inner")
            .unwrap()
            .nodes,
        ["task"]
    );
    let output = report(
        COLLAPSED,
        AsciiRenderOptions::unicode(),
        AsciiViewportPolicy::unrestricted(),
    )
    .unwrap();
    assert_primary(&output, AsciiProjection::StructuredText);
    assert_records(
        &output.text,
        &[
            "N2 [tool] id=\"task\" label=\"Hidden task\"",
            "G1 [flow] id=\"inner\" title=\"Inner\" type=\"flow\" direction=inherit collapsed=true",
            "G1 member 1: [N2] id=\"task\"",
            "E1 [sequence -->] source=[N2] id=\"task\" target=[N2] id=\"task\"; id=\"loop\"",
            "E2 [sequence -->] source=[N1] id=\"start\" target=[N2] id=\"task\"; id=\"enter\"",
            "E3 [sequence -->] source=[N2] id=\"task\" target=[C1] id=\"api\"; id=\"exit\"",
            r#""view":"collapsed""#,
        ],
    );
}

#[test]
fn parser_diagnostics_and_authored_styles_use_honest_structured_primaries() {
    for (source, expected) in [
        (
            "---\ntitle: Standalone title\n---\nagentflow-beta\nA[Alpha]\n",
            &[
                r#"sourceTitle:"Standalone title""#,
                r#"title:"Standalone title""#,
            ][..],
        ),
        (
            "agentflow-beta\nA[\"`**Alpha**`\"]\n",
            &[r#"label="**Alpha**""#, r#""label_type":"markdown""#][..],
        ),
        (
            "agentflow-beta\nA((Circle))\n",
            &[
                "diagnostic id=\"SHAPE_REMOVED\"",
                "severity=\"error\"",
                "position:",
                "Circle",
                "warningFacts:",
            ][..],
        ),
        (
            "agentflow-beta\nA[Alpha]:::hot --> B[Beta]\nclassDef hot fill:red\nstyle A stroke:blue\nclick A href \"https://example.com\" \"Open\" _blank\n",
            &[
                "fill:red",
                r#""classes":["hot","clickable"]"#,
                r#""styles":["stroke:blue"]"#,
                r#""link":"https://example.com/""#,
                r#""link_target":"_blank""#,
            ][..],
        ),
        (
            "agentflow-beta\nflow A\n a --> B\nend\nflow B\n b --> A\nend\n",
            &[
                "CONTAINMENT_VIOLATION",
                "G1 member 1: [G2] id=\"B\"",
                "G2 member 1: [G1] id=\"A\"",
            ][..],
        ),
    ] {
        let output = report(
            source,
            AsciiRenderOptions::ascii(),
            AsciiViewportPolicy::unrestricted(),
        )
        .unwrap();
        assert_primary(&output, AsciiProjection::StructuredText);
        assert_records(&output.text, expected);
    }
}

#[test]
fn overlapping_direct_node_flow_and_connector_ids_keep_each_identity() {
    let mut model = model(
        "agentflow-beta\nA[Alpha]\nflow F[Flow]\n B[Beta]\nend\nconnector api[API]\nA --> api\n",
    );
    model.sub_graphs[0].id = "A".into();
    model.vertices[1].parent_id = Some("A".into());
    model.connectors[0].id = "A".into();
    model.edges[0].end = "A".into();
    let output = direct_report(&model).unwrap();
    assert_primary(&output, AsciiProjection::StructuredText);
    assert_records(
        &output.text,
        &[
            "N1 [task] id=\"A\" label=\"Alpha\"",
            "G1 [flow] id=\"A\" title=\"Flow\"",
            "C1 [connector] id=\"A\" title=\"API\"",
            "identity records for shared authored ID: [N1,G1,C1] id=\"A\"",
            "E1 [sequence -->] source=[N1,G1,C1] id=\"A\" target=[N1,G1,C1] id=\"A\"",
            "G1 member 1: [N2] id=\"B\"",
        ],
    );
}

#[test]
fn dense_adaptive_scenes_are_complete_and_bounded_at_80_100_120() {
    let strict = report(
        DENSE,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    );
    match strict {
        Ok(output) => assert_primary(&output, AsciiProjection::Diagrammatic),
        Err(AsciiError::RoutingSearchExhausted {
            diagram_type,
            edge_index,
        }) => {
            assert_eq!(diagram_type, "agentflow");
            assert!(edge_index < 8);
        }
        Err(AsciiError::PortCapacityExceeded {
            diagram_type,
            required,
            available,
            ..
        }) => {
            assert_eq!(diagram_type, "agentflow");
            assert!(required > available);
        }
        Err(error) => panic!("strict routing must disclose its actual failure: {error:?}"),
    }
    for (width, options) in [
        (80, AsciiRenderOptions::ascii()),
        (
            100,
            AsciiRenderOptions::unicode().with_terminal_width_profile(TerminalWidthProfile::Cjk),
        ),
        (
            120,
            AsciiRenderOptions::ascii().with_terminal_width_profile(TerminalWidthProfile::Cjk),
        ),
    ] {
        let output = report(
            DENSE,
            options.with_graph_presentation(AsciiGraphPresentation::Adaptive),
            AsciiViewportPolicy::with_max_width(width).overflow(OverflowPolicy::Fallback),
        )
        .unwrap();
        assert_measurement(&output);
        assert_eq!(
            output.requested_graph_presentation,
            AsciiGraphPresentation::Adaptive
        );
        assert!(!output.emitted_overflowed);
        assert!(output.emitted_extent.width <= width);
        assert_contains(
            &output.text,
            &["Intake", "Search", "Check", "Compose", "Manual", "Service"],
        );
        if output.projection == AsciiProjection::Diagrammatic {
            assert_primary(&output, AsciiProjection::Diagrammatic);
            assert_contains(
                &output.text,
                &["ab", "ac", "bd", "cd", "be", "cf", "de", "ef"],
            );
            support::assert_rectangular_terminal_grid_with_profile(
                &output.text,
                output.width_profile,
            );
        } else {
            assert_eq!(output.outcome, AsciiOutputOutcome::Fallback);
            assert!(output.fallback.attempted);
            assert!(matches!(
                output.fallback.reason,
                Some(
                    AsciiFallbackReason::PrimaryOverflow
                        | AsciiFallbackReason::RoutingSearchExhausted
                        | AsciiFallbackReason::PortCapacityExceeded
                )
            ));
            assert_eq!(output.lossiness, Lossiness::PresentationOnly);
            let compact = output.text.split_whitespace().collect::<String>();
            for (key, semantic, label) in [
                (1, "sequence-->", "ab"),
                (2, "sequence-->", "ac"),
                (3, "sequence-->", "bd"),
                (4, "sequence-->", "cd"),
                (5, "reference-.-", "be"),
                (6, "failure--x", "cf"),
                (7, "sequence-->", "de"),
                (8, "sequence-->", "ef"),
            ] {
                assert!(
                    compact.contains(&format!("E{key}[{semantic}]")),
                    "{}",
                    output.text
                );
                assert!(
                    compact.contains(&format!("label=\"{label}\"")),
                    "{}",
                    output.text
                );
            }
        }
    }
}

#[test]
fn width_error_and_plain_fallback_preserve_both_primary_types_without_truncation() {
    let long = "agentflow-beta LR\nA[\"A deliberately long authored label with complete words and trailing content\"] --> B[\"Final result\"]\nC[Isolated]\n";
    for (source, projection) in [
        (long, AsciiProjection::Diagrammatic),
        (RICH, AsciiProjection::StructuredText),
    ] {
        let options =
            AsciiRenderOptions::ascii().with_terminal_width_profile(TerminalWidthProfile::Cjk);
        let allow = report(source, options, AsciiViewportPolicy::with_max_width(24)).unwrap();
        assert_measurement(&allow);
        assert_eq!(allow.projection, projection);
        assert_eq!(allow.outcome, AsciiOutputOutcome::WideAllowed);
        assert!(allow.emitted_overflowed);
        assert!(matches!(
            report(source, options, AsciiViewportPolicy::with_max_width(24).overflow(OverflowPolicy::Error)),
            Err(AsciiError::WidthOverflow { max_width: 24, actual_width, .. }) if actual_width > 24
        ));
        let fallback = report(
            source,
            options,
            AsciiViewportPolicy::with_max_width(24).overflow(OverflowPolicy::Fallback),
        )
        .unwrap();
        assert_measurement(&fallback);
        assert_eq!(fallback.projection, AsciiProjection::StructuredText);
        assert_eq!(fallback.outcome, AsciiOutputOutcome::Fallback);
        assert_eq!(fallback.primary_overflowed, Some(true));
        assert!(fallback.fallback.attempted);
        assert_eq!(
            fallback.fallback.reason,
            Some(AsciiFallbackReason::PrimaryOverflow)
        );
        assert!(!fallback.emitted_overflowed);
        assert!(fallback.emitted_extent.width <= 24);
        if projection == AsciiProjection::StructuredText {
            assert_eq!(fallback.lossiness, Lossiness::None);
            assert_eq!(
                fallback.text.replace('\n', ""),
                allow.text.replace('\n', "")
            );
        } else {
            assert_eq!(fallback.lossiness, Lossiness::PresentationOnly);
            let compact = fallback.text.split_whitespace().collect::<String>();
            assert_contains(
                &compact,
                &[
                    r#"label="Adeliberatelylongauthoredlabelwithcompletewordsandtrailingcontent""#,
                    r#"N2[task]id="B"label="Finalresult""#,
                    r#"N3[task]id="C"label="Isolated""#,
                    r#"E1[sequence-->]source=[N1]id="A"target=[N2]id="B""#,
                ],
            );
        }
    }
}

#[test]
fn both_primary_types_support_all_encodings_with_correct_report_metadata() {
    for (source, projection) in [
        (
            "agentflow-beta\nA[\"客户 <&>\"]\n",
            AsciiProjection::Diagrammatic,
        ),
        (
            "agentflow-beta\nA[\"客户 <&>\"]@{instruction: run}\n",
            AsciiProjection::StructuredText,
        ),
    ] {
        for (mode, encoding) in [
            (AsciiColorMode::Plain, AsciiOutputEncoding::Plain),
            (AsciiColorMode::Ansi16, AsciiOutputEncoding::Ansi16),
            (AsciiColorMode::Ansi256, AsciiOutputEncoding::Ansi256),
            (AsciiColorMode::TrueColor, AsciiOutputEncoding::TrueColor),
            (AsciiColorMode::Html, AsciiOutputEncoding::Html),
        ] {
            let output = report(
                source,
                AsciiRenderOptions::unicode()
                    .with_terminal_width_profile(TerminalWidthProfile::Cjk)
                    .with_color_mode(mode),
                AsciiViewportPolicy::unrestricted(),
            )
            .unwrap();
            assert_eq!(output.projection, projection);
            assert_eq!(output.outcome, AsciiOutputOutcome::Primary);
            assert_eq!(output.encoding, encoding);
            assert_eq!(output.metadata().encoding, encoding.as_str());
            assert_eq!(output.width_profile, TerminalWidthProfile::Cjk);
            assert_eq!(output.lossiness, Lossiness::None);
            assert!(!output.fallback.attempted);
            assert!(output.text.contains("客户"), "{}", output.text);
            if mode == AsciiColorMode::Html {
                assert!(output.text.contains("&lt;&amp;&gt;"), "{}", output.text);
            }
        }
    }
}

#[test]
fn feature_selection_keeps_beta_header_canonical_identity_and_shared_models_separate() {
    let parsed = parsed(SMALL);
    assert!(matches!(parsed.model, SemanticDiagramModel::Agentflow(_)));
    let supported = merman_ascii::ascii_supported_diagram_types();
    assert!(supported.contains(&"agentflow"));
    assert_eq!(
        supported.contains(&"flowchart"),
        cfg!(feature = "diagram-flowchart")
    );
    assert_eq!(
        supported.contains(&"swimlane"),
        cfg!(feature = "diagram-swimlane")
    );
    let model = model(SMALL);
    assert_eq!(
        direct_report(&model).unwrap().text,
        report(
            SMALL,
            AsciiRenderOptions::ascii(),
            AsciiViewportPolicy::unrestricted()
        )
        .unwrap()
        .text
    );
    let full = support::parse_model(SMALL);
    assert_eq!(
        support::render_model(&full, &AsciiRenderOptions::ascii()).unwrap(),
        direct_report(&model).unwrap().text
    );

    let config = MermaidConfig::empty_object();
    let operation = RuntimePolicy::deterministic().begin_operation().unwrap();
    let error = AsciiRenderer::default()
        .render_model_report(
            &AsciiDiagram::Agentflow(&model),
            AsciiSourceContext {
                diagram_type: "flowchart",
                config: &config,
                semantic_config: &config,
                title: None,
            },
            AsciiViewportPolicy::unrestricted(),
            &OperationControl::new(),
            &operation,
            AsciiResourcePolicy::default(),
        )
        .unwrap_err();
    if cfg!(feature = "diagram-flowchart") {
        assert!(matches!(
            error,
            AsciiError::InvalidOption {
                field: "diagram_type",
                ..
            }
        ));
    } else {
        assert!(
            matches!(error, AsciiError::UnsupportedDiagram { diagram_type } if diagram_type == "flowchart")
        );
    }
}

#[test]
fn canonical_layout_and_plain_adaptive_are_admitted_without_expanding_other_policies() {
    let source = "agentflow-beta LR\nA[Alpha] --> B[Beta]\n";
    let adaptive = report(
        source,
        AsciiRenderOptions::ascii().with_graph_presentation(AsciiGraphPresentation::Adaptive),
        AsciiViewportPolicy::unrestricted(),
    )
    .unwrap();
    assert_primary(&adaptive, AsciiProjection::Diagrammatic);
    assert_eq!(
        adaptive.requested_graph_presentation,
        AsciiGraphPresentation::Adaptive
    );
    for layout in [AsciiLayoutProfile::Compact, AsciiLayoutProfile::Auto] {
        assert!(matches!(
            report(
                source,
                AsciiRenderOptions::ascii().with_layout_profile(layout),
                AsciiViewportPolicy::with_max_width(80)
            ),
            Err(AsciiError::InvalidOption {
                field: "layout_profile",
                ..
            })
        ));
    }
    assert!(matches!(
        report(
            source,
            AsciiRenderOptions::ascii()
                .with_color_mode(AsciiColorMode::Ansi16)
                .with_graph_presentation(AsciiGraphPresentation::Adaptive),
            AsciiViewportPolicy::unrestricted()
        ),
        Err(AsciiError::InvalidOption {
            field: "graph_presentation",
            ..
        })
    ));
    assert!(matches!(
        report(
            source,
            AsciiRenderOptions::unicode().with_color_mode(AsciiColorMode::Html),
            AsciiViewportPolicy::with_max_width(20).overflow(OverflowPolicy::Fallback)
        ),
        Err(AsciiError::InvalidOption {
            field: "ascii_viewport.overflow",
            ..
        })
    ));
}

#[test]
fn exact_output_bytes_work_limits_and_cancellation_remain_terminal_for_both_primaries() {
    for source in [
        "agentflow-beta LR\nA[Alpha] --> B[Beta]\n",
        "agentflow-beta\nA[Alpha]@{instruction: run}\n",
    ] {
        let parsed = parsed(source);
        let options = AsciiRenderOptions::ascii();
        let baseline = controlled_report(
            &parsed,
            options,
            AsciiViewportPolicy::unrestricted(),
            &OperationControl::new(),
            AsciiResourcePolicy::unbounded(),
        )
        .unwrap();
        let length = baseline.text.len();
        for limit in [length, length - 1] {
            let resources = AsciiResourcePolicy::unbounded()
                .with_limit(AsciiResourceLimitId::MaxOutputBytes, limit)
                .unwrap();
            let result = controlled_report(
                &parsed,
                options,
                AsciiViewportPolicy::unrestricted(),
                &OperationControl::new(),
                resources,
            );
            if limit == length {
                assert_eq!(result.unwrap().text, baseline.text);
            } else {
                assert!(
                    matches!(result, Err(AsciiError::ResourceLimitExceeded(details))
                    if details.limit == AsciiResourceLimitId::MaxOutputBytes && details.actual > details.max)
                );
            }
        }
        let viewport = AsciiViewportPolicy::with_max_width(16).overflow(OverflowPolicy::Fallback);
        let work = AsciiResourcePolicy::unbounded()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
            .unwrap();
        assert!(
            matches!(controlled_report(&parsed, options, viewport, &OperationControl::new(), work),
            Err(AsciiError::ResourceLimitExceeded(details)) if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits)
        );
        for checkpoints in [0, 12] {
            let control = OperationControl::new();
            control.cancel_after_checkpoints(checkpoints);
            assert!(matches!(
                controlled_report(
                    &parsed,
                    options,
                    viewport,
                    &control,
                    AsciiResourcePolicy::default()
                ),
                Err(AsciiError::Cancelled(_))
            ));
        }
    }
}

#[test]
fn nested_semantic_values_enforce_depth_before_complete_document_emission() {
    let parsed = parsed("agentflow-beta\nA[Alpha]@{value: {outer: {inner: [1, null, true]}}}\n");
    let depth =
        merman_model::resources::model_complexity_from_semantic_model(&parsed.model).nesting_depth;
    assert!(depth > 1);
    for limit in [depth, depth - 1] {
        let resources = AsciiResourcePolicy::unbounded()
            .with_limit(AsciiResourceLimitId::MaxNestingDepth, limit)
            .unwrap();
        let result = controlled_report(
            &parsed,
            AsciiRenderOptions::ascii(),
            AsciiViewportPolicy::with_max_width(24).overflow(OverflowPolicy::Fallback),
            &OperationControl::new(),
            resources,
        );
        if limit == depth {
            let output = result.unwrap();
            assert_records(
                &output.text,
                &[r#""value":{"outer":{"inner":[1,null,true]}}"#],
            );
        } else {
            assert!(
                matches!(result, Err(AsciiError::ResourceLimitExceeded(details))
                if details.limit == AsciiResourceLimitId::MaxNestingDepth
                    && details.actual == depth && details.max == limit)
            );
        }
    }
}

#[test]
fn replacing_records_charges_document_cells_once_and_keeps_field_spaces_under_trim() {
    let label = format!("{} tail", "x".repeat(55));
    let source = format!("agentflow-beta\nA[\"{label}\"]@{{instruction: run}}\n");
    let parsed = parsed(&source);
    let options = AsciiRenderOptions::ascii();
    let natural = controlled_report(
        &parsed,
        options,
        AsciiViewportPolicy::unrestricted(),
        &OperationControl::new(),
        AsciiResourcePolicy::unbounded(),
    )
    .unwrap();
    assert!(
        natural.text.lines().any(|line| line.ends_with(' ')),
        "the field must wrap after an authored space"
    );
    let cells = natural.text.lines().map(str::len).sum::<usize>();
    let exact = AsciiResourcePolicy::unbounded()
        .with_limit(AsciiResourceLimitId::MaxDocumentCells, cells)
        .unwrap();
    for viewport in [
        AsciiViewportPolicy::unrestricted(),
        AsciiViewportPolicy::with_max_width(24).overflow(OverflowPolicy::Fallback),
    ] {
        let preserved =
            controlled_report(&parsed, options, viewport, &OperationControl::new(), exact).unwrap();
        let trimmed = controlled_report(
            &parsed,
            options,
            viewport.trim(AsciiTrimPolicy::TrimTrailingSpaces),
            &OperationControl::new(),
            exact,
        )
        .unwrap();
        assert_eq!(
            trimmed.text, preserved.text,
            "record field spaces must never become canvas padding"
        );
        assert!(!trimmed.trimmed);
        assert_eq!(trimmed.lossiness, Lossiness::None);
        assert_eq!(trimmed.text.lines().map(str::len).sum::<usize>(), cells);
        let below = AsciiResourcePolicy::unbounded()
            .with_limit(AsciiResourceLimitId::MaxDocumentCells, cells - 1)
            .unwrap();
        assert!(
            matches!(controlled_report(&parsed, options, viewport, &OperationControl::new(), below), Err(AsciiError::ResourceLimitExceeded(details)) if details.limit == AsciiResourceLimitId::MaxDocumentCells)
        );
    }
}
