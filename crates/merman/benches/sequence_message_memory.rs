#[path = "native_memory/allocator.rs"]
mod allocator;

use allocator::CountingSystemAllocator;
use merman::svg::RuntimePolicy;
use merman::{
    Engine, OperationControl, ParseOptions, RenderOutput, RenderRequest, Renderer, SemanticArtifact,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

const RESPONSE_SCHEMA_VERSION: u32 = 3;
const REQUEST_SCHEMA_VERSION: u32 = 1;
const REPEATED_LANE_ID: &str = "sequence-message-repeated-memory";
const UNIQUE_LANE_ID: &str = "sequence-message-unique-memory";
const PUBLIC_OPERATION: &str = "prepare-render";
const PROCESS_LIFECYCLE: &str = "fresh-process";
const ENGINE_LIFECYCLE: &str = "reused-engine";
const LOGICAL_OPERATIONS_PER_ESTIMATE: u32 = 1;
const MEMORY_SCALES: [u32; 6] = [1, 2, 4, 10, 32, 100];
const MAX_REQUEST_BYTES: u64 = 4 * 1024;
const SEMANTIC_KIND: &str = "sequence-message-prepare-render-v1";
const LABEL_PROFILE: &str = "sequence-outer-loop-equal-length-v1";
const DIAGRAM_TYPE: &str = "sequence";

#[global_allocator]
static ALLOCATOR: CountingSystemAllocator = CountingSystemAllocator::new();

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum Mode {
    Operation,
    Zero,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum MessageProfile {
    Repeated,
    Unique,
}

impl MessageProfile {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Repeated => "repeated",
            Self::Unique => "unique",
        }
    }

    fn from_lane_id(lane_id: &str) -> Option<Self> {
        match lane_id {
            REPEATED_LANE_ID => Some(Self::Repeated),
            UNIQUE_LANE_ID => Some(Self::Unique),
            _ => None,
        }
    }
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
    message_profile: &'static str,
    metadata_diagram_type_matches: bool,
    prepared_family_sequence: bool,
    prepared_render_alive_at_checkpoint: bool,
    projected_message_count_matches: bool,
    equal_length_message_labels: bool,
}

impl SemanticProjection {
    const fn zero(message_profile: MessageProfile) -> Self {
        Self {
            kind: SEMANTIC_KIND,
            label_profile: LABEL_PROFILE,
            message_profile: message_profile.as_str(),
            metadata_diagram_type_matches: false,
            prepared_family_sequence: false,
            prepared_render_alive_at_checkpoint: false,
            projected_message_count_matches: false,
            equal_length_message_labels: false,
        }
    }
}

#[derive(Debug, Serialize)]
struct SemanticOutput {
    #[serde(flatten)]
    projection: SemanticProjection,
    message_count: u32,
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
    if MessageProfile::from_lane_id(&request.lane_id).is_none() {
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

fn message_label(seed: u64, label_index: u32) -> String {
    format!("message-{seed:016x}-{label_index:08x}")
}

fn build_sequence(
    message_count: u32,
    seed: u64,
    profile: MessageProfile,
) -> Result<(String, bool), ProbeError> {
    let capacity = usize::try_from(message_count)
        .ok()
        .and_then(|count| count.checked_mul(64))
        .and_then(|bytes| bytes.checked_add(128))
        .ok_or_else(|| ProbeError::new("sequence source capacity overflow"))?;
    let mut source = String::with_capacity(capacity);
    source.push_str(
        "sequenceDiagram\n  participant A\n  participant B\n  loop operation-scoped-block\n",
    );

    let mut expected_label_len = None;
    let mut equal_length_message_labels = true;
    for message_index in 0..message_count {
        let label_index = match profile {
            MessageProfile::Repeated => 0,
            MessageProfile::Unique => message_index,
        };
        let label = message_label(seed, label_index);
        equal_length_message_labels &= expected_label_len
            .map(|expected| expected == label.len())
            .unwrap_or(true);
        expected_label_len = Some(label.len());
        writeln!(&mut source, "    A->>B: {label}")
            .map_err(|_| ProbeError::new("failed to construct Sequence diagram"))?;
    }
    source.push_str("  end\n");
    Ok((source, equal_length_message_labels))
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
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

fn projected_message_count(semantic: &SemanticArtifact) -> Result<u32, ProbeError> {
    let projection = semantic.compatibility_json().map_err(|error| {
        ProbeError::new(format!("Sequence semantic projection failed: {error}"))
    })?;
    let messages = projection
        .get("messages")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| ProbeError::new("prepared Sequence projection has no semantic messages"))?;
    let ordinary_messages = messages
        .iter()
        .filter(|message| {
            message
                .get("from")
                .is_some_and(serde_json::Value::is_string)
                && message.get("to").is_some_and(serde_json::Value::is_string)
        })
        .count();
    u32::try_from(ordinary_messages)
        .map_err(|_| ProbeError::new("prepared Sequence message count exceeds u32"))
}

fn prepare_sequence(
    renderer: &Renderer,
    source: &str,
    operation: &str,
) -> Result<SemanticArtifact, ProbeError> {
    let output = renderer
        .render(RenderRequest::semantic(source, OperationControl::new()))
        .map_err(|error| ProbeError::new(format!("Sequence {operation} failed: {error}")))?;
    let RenderOutput::Semantic(Some(semantic)) = output else {
        return Err(ProbeError::new(format!(
            "Sequence {operation} returned no diagram"
        )));
    };
    Ok(semantic)
}

fn execute_probe() -> Result<ProbeResponse, ProbeError> {
    if std::env::args_os().nth(1).is_some() {
        return Err(ProbeError::new(
            "Sequence message memory probe accepts no arguments",
        ));
    }

    let request = read_request()?;
    validate_request(&request)?;
    let message_profile = MessageProfile::from_lane_id(&request.lane_id)
        .ok_or_else(|| ProbeError::new("unsupported lane_id"))?;
    let executable_sha256 = executable_sha256()?;
    let message_count = request.scale;
    let (source, equal_length_message_labels) =
        build_sequence(message_count, request.seed, message_profile)?;
    let renderer = Renderer::new()
        .with_engine(
            Engine::new()
                .with_runtime_policy(RuntimePolicy::deterministic().with_fixed_seed(request.seed)),
        )
        .with_parse_options(ParseOptions::strict());

    // Warm the exact public route so the measured reused-engine operation excludes lazy parser,
    // registry, and semantic-model initialization retained by the process.
    let warmup = prepare_sequence(&renderer, "sequenceDiagram\n  A->>B: warmup\n", "warmup")?;
    drop(warmup);

    let snapshot_live_bytes = ALLOCATOR.begin_measurement();
    let semantic = match request.mode {
        Mode::Operation => Some(prepare_sequence(&renderer, &source, "preparation")?),
        Mode::Zero => None,
    };
    let prepared_render_alive_at_checkpoint = semantic.is_some();
    let metrics = ALLOCATOR.finish_measurement(snapshot_live_bytes);

    // Inspect the compatibility projection after the allocator checkpoint so JSON materialization
    // is not attributed to the operation-owned semantic artifact's retained-memory result.
    let (metadata_diagram_type_matches, prepared_family_sequence, projected_message_count_matches) =
        match semantic.as_ref() {
            Some(semantic) => (
                semantic.diagram_type() == DIAGRAM_TYPE,
                semantic.semantic_kind() == "sequence",
                projected_message_count(semantic)? == message_count,
            ),
            None => (false, false, false),
        };
    drop(semantic);
    let live_bytes_after_drop = ALLOCATOR.begin_measurement();
    ALLOCATOR.stop_measurement();

    let projection = if matches!(request.mode, Mode::Operation) {
        SemanticProjection {
            kind: SEMANTIC_KIND,
            label_profile: LABEL_PROFILE,
            message_profile: message_profile.as_str(),
            metadata_diagram_type_matches,
            prepared_family_sequence,
            prepared_render_alive_at_checkpoint,
            projected_message_count_matches,
            equal_length_message_labels,
        }
    } else {
        SemanticProjection::zero(message_profile)
    };
    let output_sha256 = if matches!(request.mode, Mode::Operation) {
        let projection_bytes = serde_json::to_vec(&projection).map_err(|error| {
            ProbeError::new(format!("failed to serialize semantic projection: {error}"))
        })?;
        sha256_bytes(&projection_bytes)
    } else {
        sha256_bytes(&[])
    };
    let semantic_output = SemanticOutput {
        projection,
        message_count: if matches!(request.mode, Mode::Operation) {
            message_count
        } else {
            0
        },
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
        workload_units: message_count,
        semantic_output,
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
        Err(_) => fail("Sequence message memory probe panicked"),
    }
}
