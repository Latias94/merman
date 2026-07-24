use merman::svg::{HeadlessRenderer, SvgRenderOptions};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const UPSTREAM_RENDER_REASON: &str =
    "the local corpus gate does not invoke the pinned Mermaid browser renderer";
const PARITY_ADMISSION_REASON: &str =
    "source-corpus membership does not imply normalized fixture and upstream-SVG admission";

#[derive(Debug, Deserialize)]
struct CorpusManifest {
    summary: CorpusSummary,
    entries: BTreeMap<String, CorpusEntry>,
}

#[derive(Debug, Deserialize)]
struct CorpusSummary {
    source_file_count: usize,
    unique_content_count: usize,
    managed_file_count: usize,
}

#[derive(Debug, Deserialize)]
struct CorpusEntry {
    fixture: String,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum CapabilityStage {
    Source,
    Detected,
    Semantic,
    TypedLayout,
    LocalSvg,
    UpstreamRenderable,
    ParityAdmitted,
}

impl CapabilityStage {
    const LOCAL: [Self; 5] = [
        Self::Source,
        Self::Detected,
        Self::Semantic,
        Self::TypedLayout,
        Self::LocalSvg,
    ];

    const ALL: [Self; 7] = [
        Self::Source,
        Self::Detected,
        Self::Semantic,
        Self::TypedLayout,
        Self::LocalSvg,
        Self::UpstreamRenderable,
        Self::ParityAdmitted,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Detected => "detected",
            Self::Semantic => "semantic",
            Self::TypedLayout => "typed_layout",
            Self::LocalSvg => "local_svg",
            Self::UpstreamRenderable => "upstream_renderable",
            Self::ParityAdmitted => "parity_admitted",
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
enum StageStatus {
    Passed,
    Failed(String),
    Blocked { by: CapabilityStage },
    NotEvaluated(&'static str),
}

impl StageStatus {
    fn is_passed(&self) -> bool {
        matches!(self, Self::Passed)
    }
}

#[derive(Debug)]
struct CapabilityReport {
    source_path: String,
    fixture: String,
    diagram_type: Option<String>,
    semantic_kind: Option<String>,
    layout_family: Option<String>,
    stages: BTreeMap<CapabilityStage, StageStatus>,
}

impl CapabilityReport {
    fn new(source_path: &str, fixture: &str) -> Self {
        let mut stages = BTreeMap::new();
        for stage in CapabilityStage::LOCAL {
            stages.insert(stage, StageStatus::NotEvaluated("stage has not started"));
        }
        stages.insert(
            CapabilityStage::UpstreamRenderable,
            StageStatus::NotEvaluated(UPSTREAM_RENDER_REASON),
        );
        stages.insert(
            CapabilityStage::ParityAdmitted,
            StageStatus::NotEvaluated(PARITY_ADMISSION_REASON),
        );
        Self {
            source_path: source_path.to_string(),
            fixture: fixture.to_string(),
            diagram_type: None,
            semantic_kind: None,
            layout_family: None,
            stages,
        }
    }

    fn pass(&mut self, stage: CapabilityStage) {
        self.stages.insert(stage, StageStatus::Passed);
    }

    fn fail(&mut self, stage: CapabilityStage, reason: impl Into<String>) {
        self.stages
            .insert(stage, StageStatus::Failed(reason.into()));
        for later in CapabilityStage::LOCAL
            .into_iter()
            .filter(|candidate| *candidate > stage)
        {
            self.stages
                .insert(later, StageStatus::Blocked { by: stage });
        }
    }

    fn local_pipeline_passed(&self) -> bool {
        CapabilityStage::LOCAL
            .into_iter()
            .all(|stage| self.stages[&stage].is_passed())
    }

    fn describe(&self) -> String {
        let mut out = format!(
            "{}\n  fixture: {}\n  diagram: {}\n  semantic: {}\n  layout: {}",
            self.source_path,
            self.fixture,
            self.diagram_type.as_deref().unwrap_or("<none>"),
            self.semantic_kind.as_deref().unwrap_or("<none>"),
            self.layout_family.as_deref().unwrap_or("<none>")
        );
        for stage in CapabilityStage::ALL {
            let status = match &self.stages[&stage] {
                StageStatus::Passed => "passed".to_string(),
                StageStatus::Failed(reason) => format!("failed: {reason}"),
                StageStatus::Blocked { by } => format!("blocked by {}", by.name()),
                StageStatus::NotEvaluated(reason) => format!("not evaluated: {reason}"),
            };
            let _ = write!(out, "\n  {}: {status}", stage.name());
        }
        out
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn validate_svg(svg: &str) -> Result<(), String> {
    let document = roxmltree::Document::parse(svg)
        .map_err(|error| format!("SVG is not valid XML: {error}"))?;
    let root = document.root_element();
    if !root.has_tag_name("svg") {
        return Err(format!(
            "rendered root is <{}>, not <svg>",
            root.tag_name().name()
        ));
    }

    let view_box = root
        .attribute("viewBox")
        .ok_or_else(|| "SVG root has no viewBox".to_string())?;
    let values = view_box
        .split_ascii_whitespace()
        .map(|value| {
            value
                .parse::<f64>()
                .map_err(|error| format!("invalid viewBox value {value:?}: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if values.len() != 4 || values.iter().any(|value| !value.is_finite()) {
        return Err(format!("invalid finite viewBox: {view_box:?}"));
    }
    if values[2] <= 0.0 || values[3] <= 0.0 {
        return Err(format!("non-positive viewBox extent: {view_box:?}"));
    }
    Ok(())
}

fn evaluate_fixture(
    renderer: &HeadlessRenderer,
    source_path: &str,
    fixture: &str,
    path: &Path,
    diagram_id: &str,
) -> CapabilityReport {
    let mut report = CapabilityReport::new(source_path, fixture);
    let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            report.fail(CapabilityStage::Source, format!("read failed: {error}"));
            return report;
        }
    };
    report.pass(CapabilityStage::Source);

    let metadata = match renderer.parse_metadata_sync(&source) {
        Ok(metadata) => metadata,
        Err(error) => {
            report.fail(CapabilityStage::Detected, error.to_string());
            return report;
        }
    };
    report.diagram_type = Some(metadata.diagram_type.clone());
    report.pass(CapabilityStage::Detected);

    let semantic = match renderer.prepare_semantic_sync(&source) {
        Ok(Some(semantic)) => semantic,
        Ok(None) => {
            report.fail(
                CapabilityStage::Semantic,
                "detector succeeded but semantic preparation returned no diagram",
            );
            return report;
        }
        Err(error) => {
            report.fail(CapabilityStage::Semantic, error.to_string());
            return report;
        }
    };
    if semantic.metadata().diagram_type != metadata.diagram_type {
        report.fail(
            CapabilityStage::Semantic,
            format!(
                "detection changed from {:?} to {:?}",
                metadata.diagram_type,
                semantic.metadata().diagram_type
            ),
        );
        return report;
    }
    report.semantic_kind = Some(semantic.semantic_kind().to_string());
    report.pass(CapabilityStage::Semantic);

    let prepared = match semantic.continue_layout() {
        Ok(prepared) => prepared,
        Err(error) => {
            report.fail(CapabilityStage::TypedLayout, error.to_string());
            return report;
        }
    };
    report.layout_family = Some(format!("{:?}", prepared.family_kind()));
    if let Err(error) = prepared.layout_json() {
        report.fail(
            CapabilityStage::TypedLayout,
            format!("layout compatibility projection failed: {error}"),
        );
        return report;
    }
    report.pass(CapabilityStage::TypedLayout);

    let svg = match prepared.render_svg(&SvgRenderOptions {
        diagram_id: Some(diagram_id.to_string()),
        ..SvgRenderOptions::default()
    }) {
        Ok(svg) => svg,
        Err(error) => {
            report.fail(CapabilityStage::LocalSvg, error.to_string());
            return report;
        }
    };
    match validate_svg(&svg) {
        Ok(()) => report.pass(CapabilityStage::LocalSvg),
        Err(error) => report.fail(CapabilityStage::LocalSvg, error),
    }
    report
}

#[test]
fn all_mermaid_11_16_added_mmds_reach_local_svg_with_explicit_evidence_boundaries() {
    let root = workspace_root();
    let manifest_path = root.join("fixtures/_upstream/mermaid-11.16.0/_manifest.json");
    let manifest: CorpusManifest = serde_json::from_slice(
        &std::fs::read(&manifest_path).expect("read Mermaid 11.16 MMD corpus manifest"),
    )
    .expect("parse Mermaid 11.16 MMD corpus manifest");
    assert_eq!(manifest.summary.source_file_count, 122);
    assert_eq!(manifest.summary.unique_content_count, 121);
    assert_eq!(manifest.summary.managed_file_count, 122);
    assert_eq!(manifest.entries.len(), 122);
    assert_eq!(
        manifest
            .entries
            .values()
            .map(|entry| entry.fixture.as_str())
            .collect::<BTreeSet<_>>()
            .len(),
        122,
        "every upstream path must have its own immutable source copy"
    );

    let renderer = HeadlessRenderer::new().with_strict_parsing();
    let mut failures = Vec::new();
    let mut detected_counts = BTreeMap::<String, usize>::new();
    for (index, (source_path, entry)) in manifest.entries.iter().enumerate() {
        let report = evaluate_fixture(
            &renderer,
            source_path,
            &entry.fixture,
            &root.join("fixtures").join(&entry.fixture),
            &format!("mermaid-11-16-corpus-{index}"),
        );
        if let Some(diagram_type) = &report.diagram_type {
            *detected_counts.entry(diagram_type.clone()).or_default() += 1;
        }
        if !report.local_pipeline_passed() {
            failures.push(report.describe());
        }
        assert_eq!(
            report.stages[&CapabilityStage::UpstreamRenderable],
            StageStatus::NotEvaluated(UPSTREAM_RENDER_REASON)
        );
        assert_eq!(
            report.stages[&CapabilityStage::ParityAdmitted],
            StageStatus::NotEvaluated(PARITY_ADMISSION_REASON)
        );
    }

    assert_eq!(
        detected_counts,
        BTreeMap::from([
            ("class".to_string(), 4),
            ("flowchart-elk".to_string(), 3),
            ("flowchart-v2".to_string(), 76),
            ("kanban".to_string(), 3),
            ("mindmap".to_string(), 1),
            ("radar".to_string(), 1),
            ("requirement".to_string(), 1),
            ("sequence".to_string(), 1),
            ("stateDiagram".to_string(), 1),
            ("swimlane".to_string(), 30),
            ("treemap".to_string(), 1),
        ]),
        "the 11.16 source corpus family inventory changed"
    );
    assert!(
        failures.is_empty(),
        "Mermaid 11.16 added-MMD local capability failures ({}/122):\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

#[test]
fn capability_failures_block_later_local_stages_with_the_exact_owner() {
    let report = evaluate_fixture(
        &HeadlessRenderer::new().with_strict_parsing(),
        "missing/source.mmd",
        "_upstream/missing/source.mmd",
        &workspace_root().join("fixtures/_upstream/definitely-missing.mmd"),
        "missing-source",
    );

    assert!(matches!(
        report.stages[&CapabilityStage::Source],
        StageStatus::Failed(ref reason) if reason.starts_with("read failed:")
    ));
    for stage in [
        CapabilityStage::Detected,
        CapabilityStage::Semantic,
        CapabilityStage::TypedLayout,
        CapabilityStage::LocalSvg,
    ] {
        assert_eq!(
            report.stages[&stage],
            StageStatus::Blocked {
                by: CapabilityStage::Source
            }
        );
    }
    assert_eq!(
        report.stages[&CapabilityStage::UpstreamRenderable],
        StageStatus::NotEvaluated(UPSTREAM_RENDER_REASON)
    );
    assert_eq!(
        report.stages[&CapabilityStage::ParityAdmitted],
        StageStatus::NotEvaluated(PARITY_ADMISSION_REASON)
    );
}
