#[path = "native_memory/allocator.rs"]
mod allocator;

use allocator::CountingSystemAllocator;
use merman::runtime::RuntimePolicy;
use merman::{
    Engine, OperationControl, ParseOptions, RenderOutput, RenderRequest, Renderer, SemanticArtifact,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fmt::Write as _;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

const RESPONSE_SCHEMA_VERSION: u32 = 3;
const REQUEST_SCHEMA_VERSION: u32 = 1;
const LANE_ID: &str = "flowchart-svg-label-artifact-memory";
const PUBLIC_OPERATION: &str = "prepare-render";
const PROCESS_LIFECYCLE: &str = "fresh-process";
const ENGINE_LIFECYCLE: &str = "reused-engine";
const LOGICAL_OPERATIONS_PER_ESTIMATE: u32 = 1;
const MEMORY_SCALES: [u32; 6] = [1, 2, 4, 10, 32, 100];
const MAX_REQUEST_BYTES: u64 = 4 * 1024;
const LABELS_PER_SCALE: u32 = 1;
const SEMANTIC_KIND: &str = "flowchart-prepared-render-v1";
const LABEL_PROFILE: &str = "flowchart-non-markdown-svg-node-v1";
const DIAGRAM_TYPE: &str = "flowchart-v2";

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

#[derive(Debug, Serialize)]
struct SemanticProjection {
    kind: &'static str,
    label_profile: &'static str,
    metadata_diagram_type_matches: bool,
    prepared_family_flowchart: bool,
    html_labels_disabled: bool,
    prepared_render_alive_at_checkpoint: bool,
    projected_node_labels_match_workload: bool,
    unique_projected_node_labels: bool,
}

impl SemanticProjection {
    const fn zero() -> Self {
        Self {
            kind: SEMANTIC_KIND,
            label_profile: LABEL_PROFILE,
            metadata_diagram_type_matches: false,
            prepared_family_flowchart: false,
            html_labels_disabled: false,
            prepared_render_alive_at_checkpoint: false,
            projected_node_labels_match_workload: false,
            unique_projected_node_labels: false,
        }
    }
}

#[derive(Debug, Serialize)]
struct SemanticOutput {
    #[serde(flatten)]
    projection: SemanticProjection,
    projected_node_label_count: u32,
}

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
struct ErrorResponse<'a> {
    schema_version: u32,
    error: &'a str,
}

#[derive(Debug)]
struct ProbeError(String);

impl ProbeError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

fn read_request() -> Result<ProbeRequest, ProbeError> {
    let mut bytes = Vec::new();
    io::stdin()
        .lock()
        .take(MAX_REQUEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| ProbeError::new(format!("failed to read request: {error}")))?;
    if bytes.len() as u64 > MAX_REQUEST_BYTES {
        return Err(ProbeError::new("request exceeds 4096 bytes"));
    }
    if bytes.last() != Some(&b'\n')
        || bytes.iter().filter(|byte| **byte == b'\n').count() != 1
        || bytes.contains(&b'\r')
    {
        return Err(ProbeError::new(
            "request must contain exactly one newline-terminated JSON line",
        ));
    }

    serde_json::from_slice(&bytes[..bytes.len() - 1])
        .map_err(|error| ProbeError::new(format!("invalid request JSON: {error}")))
}

fn validate_request(request: &ProbeRequest) -> Result<(), ProbeError> {
    if request.schema_version != REQUEST_SCHEMA_VERSION {
        return Err(ProbeError::new("unsupported schema_version"));
    }
    if request.lane_id != LANE_ID {
        return Err(ProbeError::new("unsupported lane_id"));
    }
    if !MEMORY_SCALES.contains(&request.scale) {
        return Err(ProbeError::new("unsupported memory scale"));
    }
    if request.invocation_id.is_empty()
        || request.invocation_id.trim() != request.invocation_id
        || request.invocation_id.len() > 256
    {
        return Err(ProbeError::new(
            "invocation_id must be trimmed, non-empty, and at most 256 bytes",
        ));
    }
    if request.nonce.len() != 32
        || !request
            .nonce
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ProbeError::new(
            "nonce must be 32-character lowercase hexadecimal",
        ));
    }
    Ok(())
}

fn build_flowchart(scale: u32, seed: u64) -> Result<String, ProbeError> {
    let mut source = String::with_capacity(scale as usize * 1_200);
    source.push_str(
        "---\nconfig:\n  htmlLabels: false\n  flowchart:\n    htmlLabels: false\n    wrappingWidth: 120\n---\nflowchart LR\n",
    );

    for group in 0..scale {
        let token = format!("{seed:016x}-{group:06}");
        writeln!(
            &mut source,
            "  N{group}[\"alpha beta gamma delta epsilon zeta eta theta unique node label {token}\"]"
        )
        .map_err(|_| ProbeError::new("failed to construct flowchart"))?;
    }

    Ok(source)
}

fn sha256_file(path: &Path) -> Result<String, ProbeError> {
    let mut file = File::open(path)
        .map_err(|error| ProbeError::new(format!("failed to open executable: {error}")))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let bytes_read = file
            .read(&mut buffer)
            .map_err(|error| ProbeError::new(format!("failed to hash executable: {error}")))?;
        if bytes_read == 0 {
            break;
        }
        digest.update(&buffer[..bytes_read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn executable_sha256() -> Result<String, ProbeError> {
    let path = std::env::current_exe()
        .map_err(|error| ProbeError::new(format!("failed to locate executable: {error}")))?;
    sha256_file(&path)
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn projected_node_labels(semantic: &SemanticArtifact) -> Result<(u32, bool), ProbeError> {
    let projection = semantic
        .compatibility_json()
        .map_err(|error| ProbeError::new(format!("semantic projection failed: {error}")))?;
    let nodes = projection
        .get("nodes")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| ProbeError::new("prepared Flowchart projection has no semantic nodes"))?;
    let count = u32::try_from(nodes.len())
        .map_err(|_| ProbeError::new("prepared Flowchart node count exceeds u32"))?;
    let mut labels = HashSet::with_capacity(nodes.len());
    for node in nodes {
        let label = node
            .get("label")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| ProbeError::new("prepared Flowchart node has no string label"))?;
        labels.insert(label);
    }
    Ok((count, labels.len() == nodes.len()))
}

fn prepare_flowchart(renderer: &Renderer, source: &str) -> Result<SemanticArtifact, ProbeError> {
    let output = renderer
        .render(RenderRequest::semantic(source, OperationControl::new()))
        .map_err(|error| ProbeError::new(format!("render preparation failed: {error}")))?;
    let RenderOutput::Semantic(Some(semantic)) = output else {
        return Err(ProbeError::new("render preparation returned no diagram"));
    };
    Ok(semantic)
}

fn execute_probe() -> Result<ProbeResponse, ProbeError> {
    if std::env::args_os().nth(1).is_some() {
        return Err(ProbeError::new(
            "flowchart SVG label memory probe accepts no arguments",
        ));
    }

    let request = read_request()?;
    validate_request(&request)?;
    let executable_sha256 = executable_sha256()?;
    let source = build_flowchart(request.scale, request.seed)?;
    let renderer = Renderer::new()
        .with_engine(
            Engine::new()
                .with_runtime_policy(RuntimePolicy::deterministic().with_fixed_seed(request.seed)),
        )
        .with_parse_options(ParseOptions::strict());

    // Materialize parser/engine state before the measurement window. The retained signal must
    // describe the operation-owned semantic artifact, not the renderer's first-use parser caches.
    renderer
        .engine()
        .parse_metadata_sync(&source)
        .map_err(|error| ProbeError::new(format!("metadata preparation failed: {error}")))?;

    let input_label_count = request.scale * LABELS_PER_SCALE;
    let workload_units = input_label_count;
    let snapshot_live_bytes = ALLOCATOR.begin_measurement();
    let semantic = match request.mode {
        Mode::Operation => Some(prepare_flowchart(&renderer, &source)?),
        Mode::Zero => None,
    };

    // Keep the format-neutral artifact live until the allocator records the checkpoint. The
    // response field retains its historical name so existing evidence readers remain compatible.
    let prepared_render_alive_at_checkpoint = std::hint::black_box(semantic.as_ref()).is_some();
    let metrics = ALLOCATOR.finish_measurement(snapshot_live_bytes);

    // Inspect the public compatibility projection after the allocator checkpoint so the receipt
    // proves the measured artifact contains the registered label workload without charging JSON
    // materialization to the candidate's retained-memory result.
    let (
        metadata_diagram_type_matches,
        prepared_family_flowchart,
        html_labels_disabled,
        projected_node_label_count,
        unique_projected_node_labels,
    ) = match semantic.as_ref() {
        Some(semantic) => {
            let (count, unique) = projected_node_labels(semantic)?;
            (
                semantic.diagram_type() == DIAGRAM_TYPE,
                semantic.semantic_kind() == "flowchart",
                semantic.metadata().effective_config.get_bool("htmlLabels") == Some(false),
                count,
                unique,
            )
        }
        None => (false, false, false, 0, false),
    };
    let projected_node_labels_match_workload =
        projected_node_label_count == input_label_count && semantic.is_some();
    drop(semantic);
    let live_bytes_after_drop = ALLOCATOR.begin_measurement();
    ALLOCATOR.stop_measurement();

    let operation = matches!(request.mode, Mode::Operation);
    let projection = if operation {
        SemanticProjection {
            kind: SEMANTIC_KIND,
            label_profile: LABEL_PROFILE,
            metadata_diagram_type_matches,
            prepared_family_flowchart,
            html_labels_disabled,
            prepared_render_alive_at_checkpoint,
            projected_node_labels_match_workload,
            unique_projected_node_labels,
        }
    } else {
        SemanticProjection::zero()
    };
    let projection_bytes = serde_json::to_vec(&projection).map_err(|error| {
        ProbeError::new(format!("failed to serialize semantic projection: {error}"))
    })?;
    let output_sha256 = if operation {
        sha256_bytes(&projection_bytes)
    } else {
        sha256_bytes(&[])
    };

    Ok(ProbeResponse {
        schema_version: RESPONSE_SCHEMA_VERSION,
        lane_id: request.lane_id,
        public_operation: PUBLIC_OPERATION,
        process_lifecycle: PROCESS_LIFECYCLE,
        engine_lifecycle: ENGINE_LIFECYCLE,
        logical_operations_per_estimate: LOGICAL_OPERATIONS_PER_ESTIMATE,
        mode: request.mode,
        scale: request.scale,
        seed: request.seed,
        repeat: request.repeat,
        pid: std::process::id(),
        executable_sha256,
        invocation_id: request.invocation_id,
        nonce: request.nonce,
        output_sha256,
        workload_units,
        semantic_output: SemanticOutput {
            projection,
            projected_node_label_count,
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

fn write_json_line<T: Serialize>(value: &T) -> io::Result<()> {
    let stdout = io::stdout();
    let mut lock = stdout.lock();
    serde_json::to_writer(&mut lock, value)?;
    lock.write_all(b"\n")?;
    lock.flush()
}

fn fail(message: &str) -> ! {
    ALLOCATOR.stop_measurement();
    let response = ErrorResponse {
        schema_version: RESPONSE_SCHEMA_VERSION,
        error: message,
    };
    let _ = write_json_line(&response);
    std::process::exit(2);
}

fn main() {
    std::panic::set_hook(Box::new(|_| {}));
    match std::panic::catch_unwind(execute_probe) {
        Ok(Ok(response)) => {
            if write_json_line(&response).is_err() {
                std::process::exit(2);
            }
        }
        Ok(Err(error)) => fail(&error.0),
        Err(_) => fail("flowchart SVG label memory probe panicked"),
    }
}
