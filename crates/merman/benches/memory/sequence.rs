use super::{ALLOCATOR, Mode, Probe, ProbeError, SemanticReport, sha256_bytes};
use merman::model::runtime::RuntimePolicy;
use merman::{
    MermaidEngine, OperationControl, ParseOptions, PresentationArtifact, RenderOutput,
    RenderRequest, Renderer,
};
use serde::Serialize;
use std::fmt::Write as _;

const REPEATED_LANE_ID: &str = "sequence-message-repeated-memory";
const UNIQUE_LANE_ID: &str = "sequence-message-unique-memory";
const PUBLIC_OPERATION: &str = "prepare-render";
const SEMANTIC_KIND: &str = "sequence-message-prepare-render-v1";
const LABEL_PROFILE: &str = "sequence-outer-loop-equal-length-v1";
const DIAGRAM_TYPE: &str = "sequence";

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum MessageProfile {
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

    pub(super) fn from_lane_id(lane_id: &str) -> Option<Self> {
        match lane_id {
            REPEATED_LANE_ID => Some(Self::Repeated),
            UNIQUE_LANE_ID => Some(Self::Unique),
            _ => None,
        }
    }
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

fn projected_message_count(semantic: &PresentationArtifact) -> Result<u32, ProbeError> {
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
) -> Result<PresentationArtifact, ProbeError> {
    let output = renderer
        .render(RenderRequest::presentation(source, OperationControl::new()))
        .map_err(|error| ProbeError::new(format!("Sequence {operation} failed: {error}")))?;
    let RenderOutput::Presentation(Some(semantic)) = output else {
        return Err(ProbeError::new(format!(
            "Sequence {operation} returned no diagram"
        )));
    };
    Ok(semantic)
}

pub(super) fn measure(probe: &Probe, message_profile: MessageProfile) -> Result<(), ProbeError> {
    let request = &probe.request;
    let message_count = request.scale;
    let (source, equal_length_message_labels) =
        build_sequence(message_count, request.seed, message_profile)?;
    let renderer = Renderer::new()
        .with_presentation_engine(
            MermaidEngine::new()
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
                semantic.family_id() == Some(merman::model::family::DiagramFamilyId::SEQUENCE),
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

    probe.report(
        PUBLIC_OPERATION,
        output_sha256,
        SemanticReport {
            workload_units: message_count,
            semantic_output,
        },
        metrics,
        Some(live_bytes_after_drop),
    );
    Ok(())
}
