#[path = "native_memory/allocator.rs"]
mod allocator;

use allocator::CountingSystemAllocator;
use merman_core::runtime::RuntimePolicy;
use merman_core::{
    DiagramParseOutcome, DiagramParseSnapshot, EditorSemanticCompleteness, EditorSemanticRole,
    Engine, ParseOptions, ParsedDiagram, ParsedDiagramRender, ParsedEditorFacts,
    RenderSemanticModel,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::fs::File;
use std::io::{self, Read, Write};

const MEMORY_SCALES: [u32; 6] = [1, 2, 4, 10, 32, 100];
const MAX_REQUEST_BYTES: u64 = 4096;

#[global_allocator]
static ALLOCATOR: CountingSystemAllocator = CountingSystemAllocator::new();

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum Mode {
    Operation,
    Zero,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeRequest {
    schema_version: u32,
    lane_id: String,
    mode: Mode,
    scale: u32,
    seed: u64,
    repeat: u32,
    invocation_id: String,
    nonce: String,
}

#[derive(Clone, Copy)]
enum Family {
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

#[derive(Clone, Copy)]
enum Route {
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

fn selected_lane(id: &str) -> Result<(Family, Route), String> {
    match id {
        "parser-flowchart-typed-memory" => Ok((Family::Flowchart, Route::Typed)),
        "parser-flowchart-json-memory" => Ok((Family::Flowchart, Route::Json)),
        "parser-flowchart-snapshot-memory" => Ok((Family::Flowchart, Route::Snapshot)),
        "parser-sequence-typed-memory" => Ok((Family::Sequence, Route::Typed)),
        "parser-sequence-json-memory" => Ok((Family::Sequence, Route::Json)),
        "parser-sequence-snapshot-memory" => Ok((Family::Sequence, Route::Snapshot)),
        "parser-state-typed-memory" => Ok((Family::State, Route::Typed)),
        "parser-state-json-memory" => Ok((Family::State, Route::Json)),
        "parser-state-snapshot-memory" => Ok((Family::State, Route::Snapshot)),
        _ => Err("unsupported lane_id".into()),
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

#[derive(Serialize)]
struct ProbeResponse {
    schema_version: u32,
    lane_id: String,
    public_operation: &'static str,
    process_lifecycle: &'static str,
    engine_lifecycle: &'static str,
    logical_operations_per_estimate: u32,
    mode: Mode,
    scale: u32,
    seed: u64,
    repeat: u32,
    pid: u32,
    executable_sha256: String,
    invocation_id: String,
    nonce: String,
    output_sha256: String,
    workload_units: u32,
    semantic_output: SemanticOutput,
    snapshot_live_bytes: u64,
    allocation_count: u64,
    allocated_bytes: u64,
    live_bytes_after: u64,
    peak_live_bytes: u64,
    peak_growth_bytes: u64,
    live_bytes_after_drop: u64,
    counter_overflowed: bool,
    counter_underflowed: bool,
}

fn read_request() -> Result<ProbeRequest, String> {
    if std::env::args_os().nth(1).is_some() {
        return Err("parse memory probe accepts no arguments".into());
    }
    let mut bytes = Vec::new();
    io::stdin()
        .lock()
        .take(MAX_REQUEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("failed to read request: {error}"))?;
    if bytes.len() as u64 > MAX_REQUEST_BYTES
        || bytes.last() != Some(&b'\n')
        || bytes.iter().filter(|byte| **byte == b'\n').count() != 1
        || bytes.contains(&b'\r')
    {
        return Err("request must be one LF-terminated JSON line of at most 4096 bytes".into());
    }
    let request: ProbeRequest = serde_json::from_slice(&bytes[..bytes.len() - 1])
        .map_err(|error| format!("invalid request JSON: {error}"))?;
    if request.schema_version != 1 || !MEMORY_SCALES.contains(&request.scale) {
        return Err("unsupported request schema_version or scale".into());
    }
    if request.invocation_id.is_empty()
        || request.invocation_id.trim() != request.invocation_id
        || request.invocation_id.len() > 256
        || request.nonce.len() != 32
        || !request
            .nonce
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("invalid invocation_id or nonce".into());
    }
    Ok(request)
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
    engine: &Engine,
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
                .model()
                .compatibility_json(parsed.metadata())
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

fn characterize_recovery(engine: &Engine, family: Family) -> Result<(), String> {
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

fn sha256_bytes(bytes: &[u8]) -> String {
    lower_hex(&Sha256::digest(bytes))
}

fn lower_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn executable_sha256() -> Result<String, String> {
    let path = std::env::current_exe().map_err(|error| error.to_string())?;
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(lower_hex(&digest.finalize()))
}

fn execute_probe() -> Result<ProbeResponse, String> {
    let request = read_request()?;
    let (family, route) = selected_lane(&request.lane_id)?;
    let executable_sha256 = executable_sha256()?;
    let source = build_source(family, request.scale, request.seed);
    let engine = Engine::new()
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
    Ok(ProbeResponse {
        schema_version: 3,
        lane_id: request.lane_id,
        public_operation: route.public_operation(),
        process_lifecycle: "fresh-process",
        engine_lifecycle: "reused-engine",
        logical_operations_per_estimate: 1,
        mode: request.mode,
        scale: request.scale,
        seed: request.seed,
        repeat: request.repeat,
        pid: std::process::id(),
        executable_sha256,
        invocation_id: request.invocation_id,
        nonce: request.nonce,
        output_sha256,
        workload_units: request.scale,
        semantic_output: SemanticOutput {
            projection,
            generated_units: if operation { request.scale } else { 0 },
        },
        snapshot_live_bytes: metrics.snapshot_live_bytes,
        allocation_count: metrics.allocation_count,
        allocated_bytes: metrics.allocated_bytes,
        live_bytes_after: metrics.live_bytes_after,
        peak_live_bytes: metrics.peak_live_bytes,
        peak_growth_bytes: metrics.peak_growth_bytes,
        live_bytes_after_drop,
        counter_overflowed: metrics.counter_overflowed,
        counter_underflowed: metrics.counter_underflowed,
    })
}

fn write_json_line(value: &impl Serialize) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    serde_json::to_writer(&mut stdout, value)?;
    stdout.write_all(b"\n")?;
    stdout.flush()
}

fn execute_state_shape(seed: u64) -> Result<Value, String> {
    let engine =
        Engine::new().with_runtime_policy(RuntimePolicy::deterministic().with_fixed_seed(seed));
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
        let complexity = merman_core::resources::ModelComplexity::from_parsed_render(&parsed);
        let compatibility = parsed
            .model()
            .compatibility_json(parsed.metadata())
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
        "executable_sha256": executable_sha256()?,
        "rows": rows
    }))
}

fn run_state_shape_mode() {
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
        Ok(sidecar) => {
            if write_json_line(&sidecar).is_err() {
                std::process::exit(2);
            }
        }
        Err(error) => {
            let _ = write_json_line(&serde_json::json!({
                "kind": "state-flat-chain-shape-characterization", "error": error
            }));
            std::process::exit(2);
        }
    }
}

fn main() {
    std::panic::set_hook(Box::new(|_| {}));
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--state-shape")) {
        run_state_shape_mode();
        return;
    }
    let result = match std::panic::catch_unwind(execute_probe) {
        Ok(result) => result,
        Err(_) => Err("parse memory probe panicked".into()),
    };
    match result {
        Ok(response) => {
            if write_json_line(&response).is_err() {
                std::process::exit(2);
            }
        }
        Err(error) => {
            ALLOCATOR.stop_measurement();
            let _ = write_json_line(&serde_json::json!({ "schema_version": 3, "error": error }));
            std::process::exit(2);
        }
    }
}
