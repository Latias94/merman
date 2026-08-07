#[path = "request_overlay_memory/allocator.rs"]
mod allocator;

use allocator::CountingSystemAllocator;
use merman_bindings_core::{BindingEngine, BindingOperationRequest, BindingOperationResult};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

const SCHEMA_VERSION: u32 = 2;
const LANE_ID: &str = "binding-request-version-only-memory";
const PUBLIC_OPERATION: &str = "binding-execute-operation-semantic-json";
const PROCESS_LIFECYCLE: &str = "fresh-process";
const ENGINE_LIFECYCLE: &str = "reused-engine";
const LOGICAL_OPERATIONS_PER_ESTIMATE: u32 = 1;
const MEMORY_SCALES: [u32; 6] = [1, 2, 4, 10, 32, 100];
const MAX_REQUEST_BYTES: u64 = 4 * 1024;
const OPERATION_ID: &str = "semantic-json";
const MEDIA_TYPE: &str = "application/json";
const SEMANTIC_KIND: &str = "binding-operation-result-v1";
const SOURCE: &[u8] = include_bytes!("fixtures/info_fixed_cost.mmd");
const BASE_OPTIONS: &[u8] = br#"{
  "version": 2,
  "runtime_policy": "deterministic",
  "resources": {"profile": "trusted-native"}
}"#;
const VERSION_ONLY_OPTIONS: &[u8] = br#"{"version":2}"#;
const EXPECTED_DATA: &[u8] = br#"{"type":"info","showInfo":true}"#;
const EXPECTED_METADATA: &[u8] = br#"{"version":1,"operation_id":"semantic-json","media_type":"application/json","runtime_policy":"deterministic","byte_length":31}"#;

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
struct SemanticOutput {
    kind: &'static str,
    operation_id: &'static str,
    media_type: &'static str,
    result_data_bytes: u64,
    result_metadata_bytes: u64,
    result_data_sha256: String,
    result_metadata_sha256: String,
    operation_calls: u32,
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
    if request.schema_version != 1 {
        return Err(ProbeError::new("unsupported request schema_version"));
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

fn result_sha256(result: &BindingOperationResult) -> String {
    let mut digest = Sha256::new();
    digest.update(result.data());
    digest.update([0]);
    digest.update(result.metadata_json());
    format!("{:x}", digest.finalize())
}

fn validate_result(result: &BindingOperationResult) -> Result<(), ProbeError> {
    if result.operation().operation_id() != OPERATION_ID
        || result.media_type() != MEDIA_TYPE
        || result.data() != EXPECTED_DATA
        || result.metadata_json() != EXPECTED_METADATA
    {
        return Err(ProbeError::new(
            "binding operation result differs from the fixed semantic contract",
        ));
    }
    Ok(())
}

fn execute_version_only(
    engine: &BindingEngine,
    calls: u32,
) -> Result<BindingOperationResult, ProbeError> {
    let request =
        BindingOperationRequest::new(OPERATION_ID, SOURCE).with_options_json(VERSION_ONLY_OPTIONS);
    for call in 0..calls {
        let result = engine.execute(request).map_err(|error| {
            ProbeError::new(format!("binding operation failed: {}", error.message()))
        })?;
        validate_result(&result)?;
        if call + 1 == calls {
            return Ok(result);
        }
    }
    Err(ProbeError::new("operation scale produced no result"))
}

fn execute_probe() -> Result<ProbeResponse, ProbeError> {
    if std::env::args_os().nth(1).is_some() {
        return Err(ProbeError::new(
            "binding request memory probe accepts no arguments",
        ));
    }

    let request = read_request()?;
    validate_request(&request)?;
    let executable_sha256 = executable_sha256()?;
    let engine = BindingEngine::from_options(BASE_OPTIONS)
        .map_err(|error| ProbeError::new(format!("binding engine failed: {}", error.message())))?;

    let snapshot_live_bytes = ALLOCATOR.begin_measurement();
    let operation_result = match request.mode {
        Mode::Operation => Some(execute_version_only(&engine, request.scale)?),
        Mode::Zero => None,
    };
    let metrics = ALLOCATOR.finish_measurement(snapshot_live_bytes);

    let empty_sha256 = sha256_bytes(&[]);
    let (output_sha256, semantic_output) = match operation_result.as_ref() {
        Some(result) => (
            result_sha256(result),
            SemanticOutput {
                kind: SEMANTIC_KIND,
                operation_id: OPERATION_ID,
                media_type: MEDIA_TYPE,
                result_data_bytes: result.data().len() as u64,
                result_metadata_bytes: result.metadata_json().len() as u64,
                result_data_sha256: sha256_bytes(result.data()),
                result_metadata_sha256: sha256_bytes(result.metadata_json()),
                operation_calls: request.scale,
            },
        ),
        None => (
            empty_sha256.clone(),
            SemanticOutput {
                kind: SEMANTIC_KIND,
                operation_id: OPERATION_ID,
                media_type: MEDIA_TYPE,
                result_data_bytes: 0,
                result_metadata_bytes: 0,
                result_data_sha256: empty_sha256.clone(),
                result_metadata_sha256: empty_sha256,
                operation_calls: 0,
            },
        ),
    };

    Ok(ProbeResponse {
        schema_version: SCHEMA_VERSION,
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
        workload_units: request.scale,
        semantic_output,
        snapshot_live_bytes: metrics.snapshot_live_bytes,
        allocation_count: metrics.allocation_count,
        allocated_bytes: metrics.allocated_bytes,
        live_bytes_after: metrics.live_bytes_after,
        peak_live_bytes: metrics.peak_live_bytes,
        peak_growth_bytes: metrics.peak_growth_bytes,
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
        schema_version: SCHEMA_VERSION,
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
        Err(_) => fail("binding request memory probe panicked"),
    }
}
