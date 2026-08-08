#[path = "native_memory/allocator.rs"]
mod allocator;

use allocator::CountingSystemAllocator;
use merman::svg::{HeadlessRenderer, RenderFamilyKind, RuntimePolicy};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fmt::Write as _;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

const RESPONSE_SCHEMA_VERSION: u32 = 2;
const REQUEST_SCHEMA_VERSION: u32 = 1;
const LANE_ID: &str = "flowchart-elk-separate-children-memory";
const PUBLIC_OPERATION: &str = "render-flowchart-elk-hierarchy-svg";
const PROCESS_LIFECYCLE: &str = "fresh-process";
const ENGINE_LIFECYCLE: &str = "reused-engine";
const LOGICAL_OPERATIONS_PER_ESTIMATE: u32 = 1;
const MEMORY_SCALES: [u32; 6] = [1, 2, 4, 10, 32, 100];
const MAX_REQUEST_BYTES: u64 = 4 * 1024;
const DEPTH_UNITS_PER_SCALE: u32 = 2;
const SEMANTIC_KIND: &str = "flowchart-elk-hierarchy-render-v2";
const DIAGRAM_TYPE: &str = "flowchart-elk";
const TERMINAL_NODE_ID: &str = "elk-hierarchy-memory-flowchart-terminal-0";

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
    metadata_diagram_type_matches: bool,
    metadata_layout_elk: bool,
    semantic_kind_flowchart: bool,
    prepared_family_flowchart: bool,
    svg_role_matches: bool,
    svg_contains_no_nan: bool,
    terminal_node_count: u32,
    scope_cluster_ids_unique: bool,
    scope_cluster_ids_complete: bool,
    view_box_finite_positive: bool,
    strict_nested_geometry: bool,
    rendered_edge_count: u32,
}

impl SemanticProjection {
    const fn zero() -> Self {
        Self {
            kind: SEMANTIC_KIND,
            metadata_diagram_type_matches: false,
            metadata_layout_elk: false,
            semantic_kind_flowchart: false,
            prepared_family_flowchart: false,
            svg_role_matches: false,
            svg_contains_no_nan: false,
            terminal_node_count: 0,
            scope_cluster_ids_unique: false,
            scope_cluster_ids_complete: false,
            view_box_finite_positive: false,
            strict_nested_geometry: false,
            rendered_edge_count: 0,
        }
    }
}

#[derive(Debug, Serialize)]
struct SemanticOutput {
    #[serde(flatten)]
    projection: SemanticProjection,
    scope_cluster_count: u32,
}

#[derive(Debug)]
struct StagedRenderObservation {
    metadata_diagram_type_matches: bool,
    metadata_layout_elk: bool,
    semantic_kind_flowchart: bool,
    prepared_family_flowchart: bool,
    svg: String,
}

#[derive(Debug)]
struct SvgObservation {
    svg_role_matches: bool,
    svg_contains_no_nan: bool,
    terminal_node_count: u32,
    scope_cluster_count: u32,
    scope_cluster_ids_unique: bool,
    scope_cluster_ids_complete: bool,
    view_box_finite_positive: bool,
    strict_nested_geometry: bool,
    rendered_edge_count: u32,
}

#[derive(Clone, Copy, Debug)]
struct Rect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl Rect {
    fn is_finite_positive(self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.width.is_finite()
            && self.width > 0.0
            && self.height.is_finite()
            && self.height > 0.0
    }

    fn strictly_contains(self, nested: Self) -> bool {
        self.is_finite_positive()
            && nested.is_finite_positive()
            && self.x < nested.x
            && self.y < nested.y
            && self.x + self.width > nested.x + nested.width
            && self.y + self.height > nested.y + nested.height
    }
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
    if request.schema_version != REQUEST_SCHEMA_VERSION {
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

fn hierarchy_depth(scale: u32) -> Result<u32, ProbeError> {
    scale
        .checked_mul(DEPTH_UNITS_PER_SCALE)
        .ok_or_else(|| ProbeError::new("hierarchy depth overflow"))
}

fn build_flowchart(depth: u32) -> Result<String, ProbeError> {
    let mut source = String::with_capacity(depth as usize * 64);
    source.push_str("flowchart-elk TD\n");
    for level in 0..depth {
        writeln!(&mut source, "subgraph scope{level:03}[\"scope{level:03}\"]")
            .map_err(|_| ProbeError::new("failed to construct flowchart"))?;
        source.push_str("direction TD\n");
    }
    source.push_str("terminal[\"Leaf\"]\n");
    for _ in 0..depth {
        source.push_str("end\n");
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

fn has_class(node: roxmltree::Node<'_, '_>, expected: &str) -> bool {
    node.attribute("class").is_some_and(|classes| {
        classes
            .split_ascii_whitespace()
            .any(|class| class == expected)
    })
}

fn parse_finite(value: Option<&str>) -> Option<f64> {
    value?.parse::<f64>().ok().filter(|value| value.is_finite())
}

fn direct_rect(node: roxmltree::Node<'_, '_>) -> Option<Rect> {
    let rect = node.children().find(|child| child.has_tag_name("rect"))?;
    Some(Rect {
        x: parse_finite(rect.attribute("x"))?,
        y: parse_finite(rect.attribute("y"))?,
        width: parse_finite(rect.attribute("width"))?,
        height: parse_finite(rect.attribute("height"))?,
    })
}

fn translate(node: roxmltree::Node<'_, '_>) -> Option<(f64, f64)> {
    let transform = node.attribute("transform")?;
    let values = transform
        .strip_prefix("translate(")?
        .strip_suffix(')')?
        .split(|character: char| character.is_ascii_whitespace() || character == ',')
        .filter(|value| !value.is_empty())
        .map(str::parse::<f64>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    match values.as_slice() {
        [x] if x.is_finite() => Some((*x, 0.0)),
        [x, y] if x.is_finite() && y.is_finite() => Some((*x, *y)),
        _ => None,
    }
}

fn translated_rect(node: roxmltree::Node<'_, '_>) -> Option<Rect> {
    let rect = direct_rect(node)?;
    let (translate_x, translate_y) = translate(node)?;
    Some(Rect {
        x: rect.x + translate_x,
        y: rect.y + translate_y,
        ..rect
    })
}

fn rendered_scope_index(node: roxmltree::Node<'_, '_>) -> Option<usize> {
    // Mermaid's ELK adapter intentionally emits `[object Object]` as the cluster DOM id. The
    // unique source scope id is therefore carried in the rendered cluster label.
    let label_node = node
        .children()
        .find(|child| child.has_tag_name("g") && has_class(*child, "cluster-label"))?;
    let mut label = String::new();
    for text in label_node
        .descendants()
        .filter(|descendant| descendant.is_text())
        .filter_map(|descendant| descendant.text())
    {
        label.push_str(text);
    }
    label.trim().strip_prefix("scope")?.parse::<usize>().ok()
}

fn view_box_is_finite_positive(root: roxmltree::Node<'_, '_>) -> bool {
    let Some(view_box) = root.attribute("viewBox") else {
        return false;
    };
    let Ok(values) = view_box
        .split(|character: char| character.is_ascii_whitespace() || character == ',')
        .filter(|value| !value.is_empty())
        .map(str::parse::<f64>)
        .collect::<Result<Vec<_>, _>>()
    else {
        return false;
    };
    matches!(values.as_slice(), [x, y, width, height]
        if x.is_finite()
            && y.is_finite()
            && width.is_finite()
            && *width > 0.0
            && height.is_finite()
            && *height > 0.0)
}

fn observe_svg(svg: &str, expected_depth: u32) -> Result<SvgObservation, ProbeError> {
    let document = roxmltree::Document::parse(svg)
        .map_err(|error| ProbeError::new(format!("invalid rendered SVG: {error}")))?;
    let root = document.root_element();
    let svg_role_matches = root.tag_name().name() == "svg"
        && root.attribute("aria-roledescription") == Some(DIAGRAM_TYPE);
    let svg_contains_no_nan = !svg.contains("NaN");
    let clusters = document
        .descendants()
        .filter(|node| node.has_tag_name("g") && has_class(*node, "cluster"))
        .collect::<Vec<_>>();
    let scope_cluster_count = u32::try_from(clusters.len()).unwrap_or(u32::MAX);
    let mut scope_ids = HashSet::with_capacity(clusters.len());
    let mut scope_rects = vec![None; expected_depth as usize];
    let mut every_scope_id_valid = true;
    for cluster in clusters {
        let Some(scope_index) = rendered_scope_index(cluster) else {
            every_scope_id_valid = false;
            continue;
        };
        if !scope_ids.insert(scope_index) {
            every_scope_id_valid = false;
        }
        let Some(slot) = scope_rects.get_mut(scope_index) else {
            every_scope_id_valid = false;
            continue;
        };
        let Some(rect) = direct_rect(cluster) else {
            every_scope_id_valid = false;
            continue;
        };
        if slot.replace(rect).is_some() {
            every_scope_id_valid = false;
        }
    }
    let scope_cluster_ids_unique =
        every_scope_id_valid && scope_ids.len() == scope_cluster_count as usize;
    let ordered_scope_rects = scope_rects.into_iter().collect::<Option<Vec<_>>>();
    let scope_cluster_ids_complete = scope_cluster_count == expected_depth
        && scope_cluster_ids_unique
        && ordered_scope_rects.is_some();

    let terminal_nodes = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("g")
                && has_class(*node, "node")
                && node.attribute("id") == Some(TERMINAL_NODE_ID)
        })
        .collect::<Vec<_>>();
    let terminal_node_count = u32::try_from(terminal_nodes.len()).unwrap_or(u32::MAX);
    let terminal_rect = match terminal_nodes.as_slice() {
        [terminal] => translated_rect(*terminal),
        _ => None,
    };
    let strict_nested_geometry = ordered_scope_rects
        .as_ref()
        .filter(|rects| !rects.is_empty())
        .is_some_and(|rects| {
            rects
                .windows(2)
                .all(|pair| pair[0].strictly_contains(pair[1]))
                && terminal_rect.is_some_and(|terminal| {
                    rects
                        .last()
                        .is_some_and(|deepest| deepest.strictly_contains(terminal))
                })
        });
    let rendered_edge_count = u32::try_from(
        document
            .descendants()
            .filter(|node| node.attribute("data-edge") == Some("true"))
            .count(),
    )
    .unwrap_or(u32::MAX);

    Ok(SvgObservation {
        svg_role_matches,
        svg_contains_no_nan,
        terminal_node_count,
        scope_cluster_count,
        scope_cluster_ids_unique,
        scope_cluster_ids_complete,
        view_box_finite_positive: view_box_is_finite_positive(root),
        strict_nested_geometry,
        rendered_edge_count,
    })
}

fn render_hierarchy(
    renderer: &HeadlessRenderer,
    source: &str,
) -> Result<StagedRenderObservation, ProbeError> {
    // The public staged API keeps metadata, capability admission, layout, and SVG tied to one
    // consuming parse artifact. `continue_layout` performs capability planning; calling
    // `render_plan` here would plan twice and pollute the measured allocation window.
    let semantic = renderer
        .prepare_semantic_sync(source)
        .map_err(|error| {
            ProbeError::new(format!("flowchart semantic preparation failed: {error}"))
        })?
        .ok_or_else(|| ProbeError::new("flowchart semantic preparation returned no diagram"))?;
    let metadata_diagram_type_matches = semantic.metadata().diagram_type == DIAGRAM_TYPE;
    let metadata_layout_elk = semantic.metadata().effective_config.get_str("layout") == Some("elk");
    let semantic_kind_flowchart = semantic.semantic_kind() == "flowchart";
    let prepared = semantic
        .continue_layout()
        .map_err(|error| ProbeError::new(format!("flowchart layout failed: {error}")))?;
    let prepared_family_flowchart = prepared.family_kind() == RenderFamilyKind::Flowchart;
    let svg = prepared
        .render_svg(renderer.svg_options())
        .map_err(|error| ProbeError::new(format!("flowchart SVG render failed: {error}")))?;

    Ok(StagedRenderObservation {
        metadata_diagram_type_matches,
        metadata_layout_elk,
        semantic_kind_flowchart,
        prepared_family_flowchart,
        svg,
    })
}

fn execute_probe() -> Result<ProbeResponse, ProbeError> {
    if std::env::args_os().nth(1).is_some() {
        return Err(ProbeError::new(
            "ELK hierarchy memory probe accepts no arguments",
        ));
    }

    let request = read_request()?;
    validate_request(&request)?;
    let executable_sha256 = executable_sha256()?;
    let depth = hierarchy_depth(request.scale)?;
    let source = build_flowchart(depth)?;
    let renderer = HeadlessRenderer::new()
        .with_strict_parsing()
        .with_vendored_text_measurer()
        .with_runtime_policy(RuntimePolicy::deterministic().with_fixed_seed(request.seed))
        .with_diagram_id("elk-hierarchy-memory");

    let snapshot_live_bytes = ALLOCATOR.begin_measurement();
    let render_result = match request.mode {
        Mode::Operation => Some(render_hierarchy(&renderer, &source)),
        Mode::Zero => None,
    };
    let metrics = ALLOCATOR.finish_measurement(snapshot_live_bytes);

    let empty_sha256 = sha256_bytes(&[]);
    let (output_sha256, semantic_output) = match render_result {
        Some(Ok(staged)) => {
            let svg = observe_svg(&staged.svg, depth)?;
            let projection = SemanticProjection {
                kind: SEMANTIC_KIND,
                metadata_diagram_type_matches: staged.metadata_diagram_type_matches,
                metadata_layout_elk: staged.metadata_layout_elk,
                semantic_kind_flowchart: staged.semantic_kind_flowchart,
                prepared_family_flowchart: staged.prepared_family_flowchart,
                svg_role_matches: svg.svg_role_matches,
                svg_contains_no_nan: svg.svg_contains_no_nan,
                terminal_node_count: svg.terminal_node_count,
                scope_cluster_ids_unique: svg.scope_cluster_ids_unique,
                scope_cluster_ids_complete: svg.scope_cluster_ids_complete,
                view_box_finite_positive: svg.view_box_finite_positive,
                strict_nested_geometry: svg.strict_nested_geometry,
                rendered_edge_count: svg.rendered_edge_count,
            };
            let projection_bytes = serde_json::to_vec(&projection).map_err(|error| {
                ProbeError::new(format!("failed to serialize semantic projection: {error}"))
            })?;
            (
                sha256_bytes(&projection_bytes),
                SemanticOutput {
                    projection,
                    scope_cluster_count: svg.scope_cluster_count,
                },
            )
        }
        Some(Err(error)) => return Err(error),
        None => {
            let projection = SemanticProjection::zero();
            (
                empty_sha256,
                SemanticOutput {
                    projection,
                    scope_cluster_count: 0,
                },
            )
        }
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
        workload_units: depth,
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
        Err(_) => fail("ELK hierarchy memory probe panicked"),
    }
}
