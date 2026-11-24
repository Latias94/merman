use super::{ALLOCATOR, Mode, Probe, ProbeError, SemanticReport, sha256_bytes};
use merman::model::runtime::RuntimePolicy;
use merman::{
    MermaidEngine, OperationControl, ParseOptions, PresentationArtifact, RenderOutput,
    RenderRequest, Renderer,
};
use serde::Serialize;
use std::collections::HashSet;
use std::fmt::Write as _;

pub(super) const LANE_ID: &str = "flowchart-svg-label-artifact-memory";
const PUBLIC_OPERATION: &str = "prepare-render";
const LABELS_PER_SCALE: u32 = 1;
const SEMANTIC_KIND: &str = "flowchart-prepared-render-v1";
const LABEL_PROFILE: &str = "flowchart-non-markdown-svg-node-v1";
const DIAGRAM_TYPE: &str = "flowchart-v2";

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

fn projected_node_labels(semantic: &PresentationArtifact) -> Result<(u32, bool), ProbeError> {
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

fn prepare_flowchart(
    renderer: &Renderer,
    source: &str,
) -> Result<PresentationArtifact, ProbeError> {
    let output = renderer
        .render(RenderRequest::presentation(source, OperationControl::new()))
        .map_err(|error| ProbeError::new(format!("render preparation failed: {error}")))?;
    let RenderOutput::Presentation(Some(semantic)) = output else {
        return Err(ProbeError::new("render preparation returned no diagram"));
    };
    Ok(semantic)
}

pub(super) fn measure(probe: &Probe) -> Result<(), ProbeError> {
    let request = &probe.request;
    let source = build_flowchart(request.scale, request.seed)?;
    let renderer = Renderer::new()
        .with_presentation_engine(
            MermaidEngine::new()
                .with_runtime_policy(RuntimePolicy::deterministic().with_fixed_seed(request.seed)),
        )
        .with_parse_options(ParseOptions::strict());

    // Materialize parser/engine state before the measurement window. The retained signal must
    // describe the operation-owned semantic artifact, not the renderer's first-use parser caches.
    renderer
        .presentation_engine()
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
                semantic.family_id() == Some(merman::model::family::DiagramFamilyId::FLOWCHART),
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

    probe.report(
        PUBLIC_OPERATION,
        output_sha256,
        SemanticReport {
            workload_units,
            semantic_output: SemanticOutput {
                projection,
                projected_node_label_count,
            },
        },
        metrics,
        Some(live_bytes_after_drop),
    );
    Ok(())
}
