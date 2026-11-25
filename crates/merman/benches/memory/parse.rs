use super::{
    ALLOCATOR, MEMORY_SCALES, Mode, Probe, SemanticReport, executable_sha256, sha256_bytes,
    write_json_line, write_response,
};
use merman::model::presentation::RenderSemanticModel;
use merman::model::runtime::RuntimePolicy;
use merman_core::{
    DiagramParseOutcome, DiagramParseSnapshot, EditorSemanticCompleteness, EditorSemanticRole,
    MermaidEngine, ParseOptions, ParsedDiagram, ParsedDiagramRender, ParsedEditorFacts,
};
use serde::Serialize;
use serde_json::Value;
use std::fmt::Write as _;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Family {
    Flowchart,
    Sequence,
    State,
}

impl Family {
    const fn name(self) -> &'static str {
        match self {
            Self::Flowchart => "flowchart",
            Self::Sequence => "sequence",
            Self::State => "state",
        }
    }

    const fn diagram_type(self) -> &'static str {
        match self {
            Self::Flowchart => "flowchart-v2",
            Self::Sequence => "sequence",
            Self::State => "stateDiagram",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Route {
    Typed,
    Json,
    Snapshot,
}

impl Route {
    const fn name(self) -> &'static str {
        match self {
            Self::Typed => "typed",
            Self::Json => "json",
            Self::Snapshot => "snapshot",
        }
    }

    const fn public_operation(self) -> &'static str {
        match self {
            Self::Typed => "parse-render-model",
            Self::Json => "parse-known-type-json",
            Self::Snapshot => "parse-editor-snapshot",
        }
    }
}

pub(super) fn selected_lane(id: &str) -> Option<(Family, Route)> {
    match id {
        "parser-flowchart-typed-memory" => Some((Family::Flowchart, Route::Typed)),
        "parser-flowchart-json-memory" => Some((Family::Flowchart, Route::Json)),
        "parser-flowchart-snapshot-memory" => Some((Family::Flowchart, Route::Snapshot)),
        "parser-sequence-typed-memory" => Some((Family::Sequence, Route::Typed)),
        "parser-sequence-json-memory" => Some((Family::Sequence, Route::Json)),
        "parser-sequence-snapshot-memory" => Some((Family::Sequence, Route::Snapshot)),
        "parser-state-typed-memory" => Some((Family::State, Route::Typed)),
        "parser-state-json-memory" => Some((Family::State, Route::Json)),
        "parser-state-snapshot-memory" => Some((Family::State, Route::Snapshot)),
        _ => None,
    }
}

#[derive(Serialize)]
struct SemanticProjection {
    kind: &'static str,
    family: &'static str,
    route: &'static str,
    metadata_matches: bool,
    model_matches: bool,
    editor_facts_requested: bool,
    editor_facts_complete: bool,
    editor_symbols_match: bool,
    editor_spans_match_source: bool,
    result_alive_at_checkpoint: bool,
    recovery_characterized: bool,
}

#[derive(Serialize)]
struct SemanticOutput {
    #[serde(flatten)]
    projection: SemanticProjection,
    generated_units: u32,
}

fn label(kind: &str, seed: u64, index: u32) -> String {
    format!("{kind}-{seed:016x}-{index:08x}")
}

fn build_source(family: Family, scale: u32, seed: u64) -> String {
    let mut source = String::with_capacity(scale as usize * 200 + 256);
    match family {
        Family::Flowchart => {
            source.push_str("flowchart LR\n");
            for index in 0..=scale {
                writeln!(source, "  n{index}[\"{}\"]", label("node", seed, index)).unwrap();
            }
            for index in 0..scale {
                writeln!(
                    source,
                    "  n{index} -->|{}| n{}",
                    label("edge", seed, index),
                    index + 1
                )
                .unwrap();
            }
        }
        Family::Sequence => {
            source.push_str("sequenceDiagram\n  participant A\n  participant B\n");
            for index in 0..scale {
                writeln!(source, "  A->>B: {}", label("message", seed, index)).unwrap();
            }
        }
        Family::State => {
            source.push_str("stateDiagram-v2\n");
            for index in 0..=scale {
                writeln!(
                    source,
                    "  state \"{}\" as S{index}",
                    label("state", seed, index)
                )
                .unwrap();
            }
            for index in 0..scale {
                writeln!(
                    source,
                    "  S{index} --> S{}: {}",
                    index + 1,
                    label("transition", seed, index)
                )
                .unwrap();
            }
        }
    }
    source
}

#[allow(
    clippy::large_enum_variant,
    reason = "Keep parse results inline; boxing would add probe-owned allocations to the measurement."
)]
enum ParseResult {
    Typed(ParsedDiagramRender),
    Json(ParsedDiagram),
    Snapshot(DiagramParseSnapshot),
}

fn parse(
    engine: &MermaidEngine,
    route: Route,
    diagram_type: &str,
    source: &str,
) -> Result<ParseResult, String> {
    let options = ParseOptions::strict();
    match route {
        Route::Typed => engine
            .parse_diagram_for_render_model_sync(source, options)
            .map(|parsed| parsed.map(ParseResult::Typed)),
        Route::Json => engine
            .parse_diagram_with_type_sync(diagram_type, source, options)
            .map(|parsed| parsed.map(ParseResult::Json)),
        Route::Snapshot => engine
            .parse_diagram_snapshot_sync(source)
            .map(|parsed| parsed.map(ParseResult::Snapshot)),
    }
    .map_err(|error| format!("parse operation failed: {error}"))?
    .ok_or_else(|| "parse operation returned no diagram".into())
}

fn verify_model(model: &Value, family: Family, scale: u32, seed: u64) {
    assert_eq!(model["type"], family.diagram_type());
    match family {
        Family::Flowchart => {
            let nodes = model["nodes"].as_array().expect("Flowchart nodes");
            let edges = model["edges"].as_array().expect("Flowchart edges");
            assert_eq!(nodes.len(), scale as usize + 1);
            assert_eq!(edges.len(), scale as usize);
            for index in 0..=scale {
                let id = format!("n{index}");
                let node = nodes
                    .iter()
                    .find(|node| node["id"] == id)
                    .expect("generated node");
                assert_eq!(node["label"], label("node", seed, index));
            }
            for (index, edge) in edges.iter().enumerate() {
                assert_eq!(edge["from"], format!("n{index}"));
                assert_eq!(edge["to"], format!("n{}", index + 1));
                assert_eq!(edge["label"], label("edge", seed, index as u32));
            }
        }
        Family::Sequence => {
            assert_eq!(model["actorOrder"], serde_json::json!(["A", "B"]));
            assert_eq!(
                model["actors"].as_object().expect("Sequence actors").len(),
                2
            );
            let messages = model["messages"].as_array().expect("Sequence messages");
            assert_eq!(messages.len(), scale as usize);
            for (index, message) in messages.iter().enumerate() {
                assert_eq!(message["from"], "A");
                assert_eq!(message["to"], "B");
                assert_eq!(message["message"], label("message", seed, index as u32));
            }
        }
        Family::State => {
            let states = model["states"].as_object().expect("State records");
            let relations = model["relations"].as_array().expect("State relations");
            assert_eq!(states.len(), scale as usize + 1);
            assert_eq!(relations.len(), scale as usize);
            for index in 0..=scale {
                let state = &states[&format!("S{index}")];
                assert_eq!(state["id"], format!("S{index}"));
                assert_eq!(state["descriptions"][0], label("state", seed, index));
            }
            for (index, relation) in relations.iter().enumerate() {
                assert_eq!(relation["id1"], format!("S{index}"));
                assert_eq!(relation["id2"], format!("S{}", index + 1));
                assert_eq!(
                    relation["relationTitle"],
                    label("transition", seed, index as u32)
                );
            }
        }
    }
}

fn verify_editor_facts(snapshot: &DiagramParseSnapshot, family: Family, scale: u32, source: &str) {
    let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
        panic!("requested snapshot must retain editor facts");
    };
    assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
    assert!(facts.diagnostics.is_empty());
    assert!(!snapshot.recovered_incomplete_directive());
    let expected_ids: Vec<String> = match family {
        Family::Flowchart => (0..=scale).map(|index| format!("n{index}")).collect(),
        Family::Sequence => vec!["A".into(), "B".into()],
        Family::State => (0..=scale).map(|index| format!("S{index}")).collect(),
    };
    for id in expected_ids {
        assert!(
            facts
                .symbols
                .iter()
                .any(|symbol| symbol.name == id && symbol.role == EditorSemanticRole::Entity),
            "missing generated entity {id}"
        );
    }
    for symbol in &facts.symbols {
        assert!(symbol.span.start <= symbol.selection.start);
        assert!(symbol.selection.end <= symbol.span.end);
        assert!(source.get(symbol.span.start..symbol.span.end).is_some());
        assert!(
            source
                .get(symbol.selection.start..symbol.selection.end)
                .is_some()
        );
        if matches!(
            symbol.role,
            EditorSemanticRole::Entity | EditorSemanticRole::Reference
        ) {
            assert_eq!(
                source.get(symbol.selection.start..symbol.selection.end),
                Some(symbol.name.as_str())
            );
        }
    }
}

fn verify_result(
    result: &ParseResult,
    family: Family,
    scale: u32,
    seed: u64,
    source: &str,
) -> Result<(), String> {
    match result {
        ParseResult::Typed(parsed) => {
            assert_eq!(parsed.metadata().diagram_type, family.diagram_type());
            assert!(matches!(
                (family, parsed.model()),
                (Family::Flowchart, RenderSemanticModel::Flowchart(_))
                    | (Family::Sequence, RenderSemanticModel::Sequence(_))
                    | (Family::State, RenderSemanticModel::State(_))
            ));
            // This temporary compatibility projection is dropped before the result-drop sample.
            let json = parsed
                .compatibility_json()
                .map_err(|error| error.to_string())?;
            verify_model(&json, family, scale, seed);
        }
        ParseResult::Json(parsed) => {
            assert_eq!(parsed.meta.diagram_type, family.diagram_type());
            verify_model(&parsed.model, family, scale, seed);
        }
        ParseResult::Snapshot(snapshot) => {
            assert_eq!(snapshot.metadata().diagram_type, family.diagram_type());
            let DiagramParseOutcome::Parsed {
                model,
                warning_facts,
            } = snapshot.outcome()
            else {
                panic!("measured snapshot must be a full successful parse");
            };
            assert!(warning_facts.is_empty());
            verify_model(model, family, scale, seed);
            verify_editor_facts(snapshot, family, scale, source);
        }
    }
    Ok(())
}

fn characterize_recovery(engine: &MermaidEngine, family: Family) -> Result<(), String> {
    let source = match family {
        Family::Flowchart => "flowchart TD\naccTitle: One pass\nA((\n",
        Family::Sequence => "sequenceDiagram\nAlice->>Bob: Hello\nBob->>",
        Family::State => "stateDiagram-v2\nIdle --> Running\nRunning -->",
    };
    let snapshot = engine
        .parse_diagram_snapshot_sync(source)
        .map_err(|error| error.to_string())?
        .expect("recovery snapshot");
    assert!(matches!(snapshot.outcome(), DiagramParseOutcome::Failed(_)));
    let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
        panic!("failed strict parsing must retain requested recovery facts");
    };
    assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
    assert!(!facts.symbols.is_empty());
    Ok(())
}

pub(super) fn measure(probe: &Probe, family: Family, route: Route) -> Result<(), String> {
    let request = &probe.request;
    let source = build_source(family, request.scale, request.seed);
    let engine = MermaidEngine::new()
        .with_runtime_policy(RuntimePolicy::deterministic().with_fixed_seed(request.seed));
    let operation = matches!(request.mode, Mode::Operation);

    // Zero mode constructs inputs and an engine but never enters detection or any parser.
    // The real route is warmed outside the allocation window for reused-engine evidence.
    let diagram_type = if operation {
        let metadata = engine
            .parse_metadata_sync(&source)
            .map_err(|error| error.to_string())?;
        assert_eq!(metadata.diagram_type, family.diagram_type());
        let warm_source = build_source(family, 1, request.seed);
        drop(parse(&engine, route, &metadata.diagram_type, &warm_source)?);
        metadata.diagram_type
    } else {
        family.diagram_type().to_string()
    };

    let snapshot_live_bytes = ALLOCATOR.begin_measurement();
    let result = if operation {
        Some(parse(&engine, route, &diagram_type, &source)?)
    } else {
        None
    };
    let metrics = ALLOCATOR.finish_measurement(snapshot_live_bytes);

    // Semantic validation and compatibility materialization are outside the allocation window.
    // All validation temporaries must drop before sampling retained bytes after result drop.
    if let Some(result) = &result {
        verify_result(result, family, request.scale, request.seed, &source)?;
    }
    drop(result);
    let live_bytes_after_drop = ALLOCATOR.begin_measurement();
    ALLOCATOR.stop_measurement();

    let snapshot_requested = operation && matches!(route, Route::Snapshot);
    // Malformed input is characterized separately, after both allocation checkpoints.
    if snapshot_requested {
        characterize_recovery(&engine, family)?;
    }
    let projection = SemanticProjection {
        kind: "parse-allocation-observation-v1",
        family: family.name(),
        route: route.name(),
        metadata_matches: operation,
        model_matches: operation,
        editor_facts_requested: snapshot_requested,
        editor_facts_complete: snapshot_requested,
        editor_symbols_match: snapshot_requested,
        editor_spans_match_source: snapshot_requested,
        result_alive_at_checkpoint: operation,
        recovery_characterized: snapshot_requested,
    };
    // Protocol v3 hashes the fixed, verified semantic projection. Exact full-output identities
    // belong to the separate parser latency preflight; seeds and scale change this input's text.
    let output_sha256 = if operation {
        sha256_bytes(&serde_json::to_vec(&projection).map_err(|error| error.to_string())?)
    } else {
        sha256_bytes(&[])
    };
    probe.report(
        route.public_operation(),
        output_sha256,
        SemanticReport {
            workload_units: request.scale,
            semantic_output: SemanticOutput {
                projection,
                generated_units: if operation { request.scale } else { 0 },
            },
        },
        metrics,
        Some(live_bytes_after_drop),
    );
    Ok(())
}

fn execute_state_shape(seed: u64) -> Result<Value, String> {
    let engine = MermaidEngine::new()
        .with_runtime_policy(RuntimePolicy::deterministic().with_fixed_seed(seed));
    let mut rows = Vec::with_capacity(MEMORY_SCALES.len());
    for scale in MEMORY_SCALES {
        let source = build_source(Family::State, scale, seed);
        let result = parse(&engine, Route::Typed, Family::State.diagram_type(), &source)?;
        let ParseResult::Typed(parsed) = result else {
            return Err("State shape mode requires the typed route".into());
        };
        let RenderSemanticModel::State(model) = parsed.model() else {
            return Err("State shape mode returned another diagram family".into());
        };
        if model.document.len() != 1 {
            return Err("State shape mode supports only the existing single-document chain".into());
        }
        let statements = model
            .document
            .root()
            .and_then(|root| model.document.get(root))
            .ok_or("State shape mode requires its canonical root document")?;
        let complexity =
            merman::model::resources::model_complexity_from_presentation(parsed.presentation());
        let compatibility = parsed
            .compatibility_json()
            .map_err(|error| error.to_string())?;
        verify_model(&compatibility, Family::State, scale, seed);
        let mut compatibility_bytes = Vec::new();
        compatibility
            .write_json(&mut compatibility_bytes)
            .map_err(|error| error.to_string())?;
        rows.push(serde_json::json!({
            "scale": scale,
            "source_bytes": source.len(),
            "source_sha256": sha256_bytes(source.as_bytes()),
            "canonical": {
                "statement_count": statements.len(),
                "state_count": model.states.len(),
                "relation_count": model.relations.len(),
                "flat_document_count": model.document.len(),
                "node_count": model.nodes.len(),
                "edge_count": model.edges.len()
            },
            "admitted_model_complexity": {
                "items": complexity.items,
                "text_bytes": complexity.text_bytes,
                "nesting_depth": complexity.nesting_depth
            },
            "compatibility_projection": {
                "required_by_typed_operation": false,
                "legacy_json_output_bytes": compatibility_bytes.len()
            }
        }));
    }
    Ok(serde_json::json!({
        "kind": "state-flat-chain-shape-characterization",
        "allocation_evidence": false,
        "family": "state",
        "route": "typed",
        "seed": seed,
        "executable_sha256": executable_sha256().map_err(|error| error.0)?,
        "rows": rows
    }))
}

pub(super) fn run_state_shape_mode() {
    let result = std::panic::catch_unwind(|| {
        let mut args = std::env::args().skip(2);
        let seed = args
            .next()
            .ok_or("--state-shape requires one decimal u64 seed")?;
        if seed.is_empty() || !seed.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err("--state-shape seed must be a decimal u64".into());
        }
        let seed = seed
            .parse::<u64>()
            .map_err(|_| "--state-shape seed must be a decimal u64")?;
        if args.next().is_some() {
            return Err("--state-shape accepts only one decimal u64 seed".into());
        }
        execute_state_shape(seed)
    })
    .unwrap_or_else(|_| Err("State shape characterization panicked".into()));
    match result {
        Ok(sidecar) => write_response(&sidecar),
        Err(error) => {
            let _ = write_json_line(&serde_json::json!({
                "kind": "state-flat-chain-shape-characterization", "error": error
            }));
            std::process::exit(2);
        }
    }
}
