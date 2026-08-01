use crate::error::CliError;
use crate::invocation::{ResolvedOutput, ResolvedRenderCommon};
use crate::markdown::{
    self, MarkdownChart, MarkdownChartLimitExceeded, MarkdownImage, NumberedOutputNamespace,
};
use crate::output::{AcquiredTransaction, PublicationGuards};
use crate::render::render_markdown_charts;
use crate::resources::{ByteLedgerKind, CheckedBytes, CountLedgerKind, ResolvedResourcePolicy};
use crate::runtime::{ExecutionContext, SharedWriter};
use crate::transaction::{
    ArtifactNamespace, GenerationDialect, GenerationManifest, GenerationOwner, RelativeTarget,
    StageSlot, StagingTransaction, TargetGeneration, TransactionEntryPlan, TransactionPlan,
    TransactionRole,
};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

// This historical allowlist is part of native stale-file deletion authorization.
const NATIVE_MANAGED_ARTIFACT_EXTENSIONS: &[&str] = &["svg", "png", "jpg", "pdf"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BatchDialect {
    NativeBatchV1,
    Mmdc11_16_0,
}

impl BatchDialect {
    fn scanner(self) -> MarkdownScanner {
        match self {
            Self::NativeBatchV1 => markdown::scan_native_limited,
            Self::Mmdc11_16_0 => markdown::scan_mmdc_11_16_0_limited,
        }
    }

    fn writes_document(self, output_path: &Path) -> bool {
        match self {
            Self::NativeBatchV1 => true,
            Self::Mmdc11_16_0 => markdown::is_markdown_path(output_path),
        }
    }

    fn is_native(self) -> bool {
        matches!(self, Self::NativeBatchV1)
    }
}

type MarkdownScanner =
    for<'source> fn(
        &'source str,
        Option<u64>,
    ) -> Result<Vec<MarkdownChart<'source>>, MarkdownChartLimitExceeded>;

pub(crate) struct PreparedBatch {
    pub(crate) dialect: BatchDialect,
    pub(crate) source: String,
    pub(crate) output: ResolvedOutput,
    pub(crate) common: ResolvedRenderCommon,
    pub(crate) output_path: PathBuf,
    pub(crate) transaction_root: PathBuf,
    pub(crate) artefacts: Option<PathBuf>,
    pub(crate) publications: PublicationGuards,
    #[cfg(feature = "parallel-markdown")]
    pub(crate) jobs: usize,
    pub(crate) quiet: bool,
}

struct RequestedLayout {
    dialect: BatchDialect,
    root: PathBuf,
    output: PathBuf,
    manifest: PathBuf,
    namespace: NumberedOutputNamespace,
    artifact_directory: PathBuf,
    artifacts: Vec<PathBuf>,
    urls: Vec<String>,
    writes_document: bool,
}

struct ApprovedLayout {
    root: PathBuf,
    owner: GenerationOwner,
    artifact_directory: PathBuf,
    manifest_path: PathBuf,
    manifest: ApprovedTarget,
    document: Option<ApprovedTarget>,
    artifacts: Vec<ApprovedTarget>,
}

struct ApprovedTarget {
    target: RelativeTarget,
    generation: TargetGeneration,
}

struct StageSlots {
    artifacts: Vec<StageSlot>,
    manifest: StageSlot,
    document: Option<StageSlot>,
}

enum ValidatedPreviousGeneration {
    Native { stale: Vec<RelativeTarget> },
    Strict,
}

impl ValidatedPreviousGeneration {
    fn stale_artifacts(&self) -> &[RelativeTarget] {
        match self {
            Self::Native { stale } => stale,
            Self::Strict => &[],
        }
    }
}

pub(crate) fn execute(
    prepared: PreparedBatch,
    context: &mut ExecutionContext,
) -> Result<(), CliError> {
    let PreparedBatch {
        dialect,
        source,
        output,
        common,
        output_path,
        transaction_root,
        artefacts,
        publications,
        #[cfg(feature = "parallel-markdown")]
        jobs,
        quiet,
    } = prepared;
    let resources = common.resources;
    let charts = scan_charts(dialect.scanner(), &source, &resources)?;
    let mut requested = RequestedLayout::resolve(
        dialect,
        &common.cwd,
        &transaction_root,
        &output_path,
        output.format(),
        artefacts.as_deref(),
        charts.len(),
    )?;

    let renderer = if charts.is_empty() {
        None
    } else {
        let (_, renderer) = crate::render::prepare_graphical_output(
            output,
            common,
            false,
            matches!(dialect, BatchDialect::Mmdc11_16_0),
            #[cfg(feature = "network-icons")]
            context.network.as_mut(),
        )?;
        Some(renderer)
    };

    // This is the first filesystem mutation in the batch path. Source
    // acquisition, scanning, renderer preparation, target expansion, and
    // strict containment have already completed.
    let acquired = context.publication.acquire_transaction(&publications)?;
    let approved_root_path = acquired.root().to_path_buf();

    if charts.is_empty() && !requested.writes_document && !dialect.is_native() {
        report_chart_count(quiet, 0, &context.stderr);
        return Ok(());
    }

    let approved = requested.approve(&publications, &approved_root_path)?;
    let previous = read_previous_manifest(&approved.manifest_path, &approved.root)?;
    let previous = previous
        .as_ref()
        .map(|previous| validate_previous_generation(previous, &approved))
        .transpose()?;
    if !charts.is_empty() {
        materialize_artifact_directory(
            &publications,
            requested.namespace.directory(),
            &approved.artifact_directory,
            &approved_root_path,
        )?;
    }

    let stale_targets = previous
        .as_ref()
        .map_or(&[][..], ValidatedPreviousGeneration::stale_artifacts);
    let stale = approved.approve_stale_targets(&acquired, &publications, stale_targets)?;

    let plan = transaction_plan(&approved, &stale)?;
    let mut staging = context.publication.begin_transaction(acquired, plan)?;
    let setup = (|| {
        let manifest = GenerationManifest::new(
            staging.transaction_id().to_owned(),
            approved.owner.clone(),
            approved
                .document
                .as_ref()
                .map(|document| document.target.clone()),
            approved
                .artifacts
                .iter()
                .map(|artifact| artifact.target.clone())
                .collect(),
        )?;
        let manifest_bytes = manifest.encode()?;
        let slots = issue_stage_slots(&mut staging, &approved)?;
        Ok::<_, CliError>((manifest_bytes, slots))
    })();
    let (manifest_bytes, slots) = match setup {
        Ok(setup) => setup,
        Err(error) => return abort_staging(context, staging, error),
    };
    let StageSlots {
        artifacts: artifact_slots,
        manifest: manifest_slot,
        document: document_slot,
    } = slots;
    let staged_bytes = Mutex::new(resources.checked_bytes(ByteLedgerKind::StagedOutput));

    let urls = std::mem::take(&mut requested.urls);
    let images = match renderer.as_ref() {
        Some(renderer) => render_markdown_charts(
            renderer,
            &charts,
            artifact_slots,
            urls,
            &staged_bytes,
            &context.stderr,
            #[cfg(feature = "parallel-markdown")]
            jobs,
        ),
        None => Ok(Vec::new()),
    };
    let images = match images {
        Ok(images) => images,
        Err(error) => {
            drop(manifest_slot);
            drop(document_slot);
            return abort_staging(context, staging, error);
        }
    };

    if let Err(error) = stage_generation_metadata(
        &source,
        &charts,
        &images,
        manifest_slot,
        document_slot,
        &manifest_bytes,
        &staged_bytes,
    ) {
        return abort_staging(context, staging, error);
    }

    let ready = context.publication.ready_transaction(staging)?;
    context.publication.commit_transaction(ready)?;
    report_chart_count(quiet, charts.len(), &context.stderr);
    report_publication(quiet, &requested, &images, &context.stderr);
    Ok(())
}

impl RequestedLayout {
    fn resolve(
        dialect: BatchDialect,
        cwd: &Path,
        transaction_root: &Path,
        output_path: &Path,
        format: crate::cli::RenderFormat,
        artefacts: Option<&Path>,
        chart_count: usize,
    ) -> Result<Self, CliError> {
        let root = markdown::absolute_path(transaction_root, cwd);
        let output = markdown::absolute_path(output_path, cwd);
        let namespace = NumberedOutputNamespace::new(output_path, format, artefacts);
        let artifact_directory = markdown::absolute_path(
            if namespace.directory().as_os_str().is_empty() {
                Path::new(".")
            } else {
                namespace.directory()
            },
            cwd,
        );
        let artifacts = (1..=chart_count)
            .map(|index| markdown::absolute_path(&namespace.path(index), cwd))
            .collect::<Vec<_>>();
        let writes_document = dialect.writes_document(output_path);
        let manifest = match dialect {
            BatchDialect::NativeBatchV1 => {
                markdown::absolute_path(&markdown::native_manifest_path(transaction_root), cwd)
            }
            BatchDialect::Mmdc11_16_0 => {
                markdown::absolute_path(&markdown::strict_manifest_path(output_path)?, cwd)
            }
        };

        ensure_direct_child(&root, &output, "Markdown output")?;
        ensure_direct_child(&root, &manifest, "generation manifest")?;
        if matches!(dialect, BatchDialect::Mmdc11_16_0) {
            ensure_directory_within_root(
                &root,
                &artifact_directory,
                "strict mmdc artefacts directory",
            )?;
            for artifact in &artifacts {
                ensure_descendant(&root, artifact, "strict mmdc artefact")?;
            }
        }
        let urls = artifacts
            .iter()
            .map(|artifact| markdown::relative_markdown_url(&output, artifact, cwd))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            dialect,
            root,
            output,
            manifest,
            namespace,
            artifact_directory,
            artifacts,
            urls,
            writes_document,
        })
    }

    fn approve(
        &self,
        publications: &PublicationGuards,
        canonical_root: &Path,
    ) -> Result<ApprovedLayout, CliError> {
        let (output_path, output_generation) = if self.writes_document {
            let (path, generation) = publications
                .approved_transaction_target(&self.output)?
                .into_parts();
            (path, Some(generation))
        } else {
            (
                rebase_target(&self.root, canonical_root, &self.output)?,
                None,
            )
        };
        let (manifest_path, manifest_generation) = publications
            .approved_transaction_target(&self.manifest)?
            .into_parts();
        let artifact_directory = publications.approved_directory_path(&self.artifact_directory)?;
        let artifacts = self
            .artifacts
            .iter()
            .map(|artifact| {
                let (path, generation) = publications
                    .approved_transaction_target(artifact)?
                    .into_parts();
                Ok::<_, CliError>(ApprovedTarget {
                    target: RelativeTarget::from_absolute(canonical_root, path)?,
                    generation,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let owner_target = RelativeTarget::from_absolute(canonical_root, &output_path)?;
        let namespace = ArtifactNamespace::from_absolute(
            canonical_root,
            &artifact_directory,
            self.namespace.stem(),
            self.namespace.extension(),
        )?;
        let dialect = match self.dialect {
            BatchDialect::NativeBatchV1 => GenerationDialect::NativeBatchV1,
            BatchDialect::Mmdc11_16_0 => GenerationDialect::Mmdc11_16_0,
        };
        let owner = GenerationOwner::new(dialect, owner_target, namespace)?;
        let manifest = ApprovedTarget {
            target: RelativeTarget::from_absolute(canonical_root, &manifest_path)?,
            generation: manifest_generation,
        };
        let document = output_generation
            .map(|generation| {
                Ok::<_, CliError>(ApprovedTarget {
                    target: RelativeTarget::from_absolute(canonical_root, &output_path)?,
                    generation,
                })
            })
            .transpose()?;
        Ok(ApprovedLayout {
            root: canonical_root.to_path_buf(),
            owner,
            artifact_directory,
            manifest_path,
            manifest,
            document,
            artifacts,
        })
    }
}

impl ApprovedLayout {
    fn approve_stale_targets(
        &self,
        acquired: &AcquiredTransaction,
        publications: &PublicationGuards,
        stale: &[RelativeTarget],
    ) -> Result<Vec<ApprovedTarget>, CliError> {
        let mut approved_stale = Vec::with_capacity(stale.len());
        for target in stale {
            let requested = target.to_path(&self.root)?;
            let (approved, generation) = acquired
                .approve_native_stale_artifact(publications, target)?
                .into_parts();
            let approved = RelativeTarget::from_absolute(&self.root, approved)?;
            if &approved != target {
                return Err(CliError::InvalidOutput(format!(
                    "stale artifact {} no longer matches its approved transaction target",
                    crate::error::safe_path(requested)
                )));
            }
            approved_stale.push(ApprovedTarget {
                target: approved,
                generation,
            });
        }
        Ok(approved_stale)
    }
}

impl ApprovedTarget {
    fn write_entry(&self, role: TransactionRole) -> TransactionEntryPlan {
        TransactionEntryPlan::write(role, self.target.clone())
            .expect_generation(self.generation.clone())
    }

    fn delete_artifact_entry(&self) -> TransactionEntryPlan {
        TransactionEntryPlan::delete_artifact(self.target.clone())
            .expect_generation(self.generation.clone())
    }
}

fn scan_charts<'source>(
    scan: MarkdownScanner,
    source: &'source str,
    resources: &ResolvedResourcePolicy,
) -> Result<Vec<MarkdownChart<'source>>, CliError> {
    let mut chart_counter = resources.checked_count(CountLedgerKind::MarkdownCharts);
    let charts = match scan(source, chart_counter.max()) {
        Ok(charts) => charts,
        Err(error) => {
            debug_assert_eq!(chart_counter.max(), Some(error.max));
            let limit_error = chart_counter
                .try_add(error.observed)
                .expect_err("scanner reported a count above the same policy limit");
            return Err(CliError::markdown_chart(
                error.observed,
                error.location,
                limit_error.into(),
            ));
        }
    };
    let count = u64::try_from(charts.len())
        .map_err(|_| CliError::InvalidInput("Markdown chart count overflow".to_string()))?;
    chart_counter.try_add(count)?;
    Ok(charts)
}

fn ensure_direct_child(root: &Path, target: &Path, role: &str) -> Result<(), CliError> {
    if target.parent() == Some(root) {
        return Ok(());
    }
    Err(CliError::InvalidOutput(format!(
        "{role} {} must be a direct child of transaction root {}",
        crate::error::safe_path(target),
        crate::error::safe_path(root)
    )))
}

fn ensure_descendant(root: &Path, target: &Path, role: &str) -> Result<(), CliError> {
    if target != root && target.strip_prefix(root).is_ok() {
        return Ok(());
    }
    Err(CliError::InvalidOutput(format!(
        "{role} {} falls outside the single transaction root {}; choose an artefacts directory beneath the rewritten Markdown target's parent",
        crate::error::safe_path(target),
        crate::error::safe_path(root)
    )))
}

fn ensure_directory_within_root(root: &Path, directory: &Path, role: &str) -> Result<(), CliError> {
    if directory == root || directory.strip_prefix(root).is_ok() {
        return Ok(());
    }
    Err(CliError::InvalidOutput(format!(
        "{role} {} falls outside the single transaction root {}; choose an artefacts directory beneath the rewritten Markdown target's parent",
        crate::error::safe_path(directory),
        crate::error::safe_path(root)
    )))
}

fn rebase_target(
    requested_root: &Path,
    canonical_root: &Path,
    requested_target: &Path,
) -> Result<PathBuf, CliError> {
    let relative = requested_target.strip_prefix(requested_root).map_err(|_| {
        CliError::InvalidOutput(format!(
            "publication target {} falls outside transaction root {}",
            crate::error::safe_path(requested_target),
            crate::error::safe_path(requested_root)
        ))
    })?;
    if relative.as_os_str().is_empty() {
        return Err(CliError::InvalidOutput(format!(
            "publication target {} must name a file beneath transaction root {}",
            crate::error::safe_path(requested_target),
            crate::error::safe_path(requested_root)
        )));
    }
    Ok(canonical_root.join(relative))
}

fn materialize_artifact_directory(
    publications: &PublicationGuards,
    requested_directory: &Path,
    approved_directory: &Path,
    canonical_root: &Path,
) -> Result<(), CliError> {
    if approved_directory == canonical_root {
        return Ok(());
    }
    let child = publications.prepare_directory(if requested_directory.as_os_str().is_empty() {
        Path::new(".")
    } else {
        requested_directory
    })?;
    child.verify()?;
    if child.path() != approved_directory || child.path().strip_prefix(canonical_root).is_err() {
        return Err(CliError::InvalidOutput(format!(
            "artefacts directory {} no longer matches its approved path beneath transaction root {}",
            crate::error::safe_path(child.path()),
            crate::error::safe_path(canonical_root)
        )));
    }
    Ok(())
}

fn read_previous_manifest(
    path: &Path,
    root: &Path,
) -> Result<Option<GenerationManifest>, CliError> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => GenerationManifest::read_bounded(path, root)
            .map(Some)
            .map_err(Into::into),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(crate::transaction::TransactionError::Operational {
            operation: "inspect generation manifest",
            path: path.to_path_buf(),
            source,
        }
        .into()),
    }
}

fn validate_previous_generation(
    manifest: &GenerationManifest,
    approved: &ApprovedLayout,
) -> Result<ValidatedPreviousGeneration, CliError> {
    if !manifest.owner().has_same_subject_as(&approved.owner) {
        return Err(crate::transaction::TransactionError::InvalidState {
            evidence: approved.manifest_path.clone(),
            reason:
                "generation manifest belongs to a different Markdown dialect, owner, or managed output"
                    .to_string(),
        }
        .into());
    }
    if manifest.document() != approved.document.as_ref().map(|document| &document.target) {
        return Err(crate::transaction::TransactionError::InvalidState {
            evidence: approved.manifest_path.clone(),
            reason: "generation manifest belongs to a different Markdown document".to_string(),
        }
        .into());
    }

    match approved.owner.dialect() {
        GenerationDialect::NativeBatchV1 => {
            let previous_namespace = manifest.owner().namespace();
            let current_namespace = approved.owner.namespace();
            let extension = previous_namespace.extension().to_str();
            if !previous_namespace.has_same_series_as(current_namespace)
                || !extension.is_some_and(|extension| {
                    NATIVE_MANAGED_ARTIFACT_EXTENSIONS.contains(&extension)
                })
            {
                return Err(crate::transaction::TransactionError::InvalidState {
                    evidence: approved.manifest_path.clone(),
                    reason: "generation manifest contains an unsupported native artifact namespace"
                        .to_string(),
                }
                .into());
            }
            Ok(ValidatedPreviousGeneration::Native {
                stale: native_stale_artifacts(manifest, &approved.artifacts),
            })
        }
        GenerationDialect::Mmdc11_16_0 => Ok(ValidatedPreviousGeneration::Strict),
    }
}

fn native_stale_artifacts(
    previous: &GenerationManifest,
    current: &[ApprovedTarget],
) -> Vec<RelativeTarget> {
    let current = current
        .iter()
        .map(|artifact| &artifact.target)
        .collect::<HashSet<_>>();
    previous
        .artifacts()
        .iter()
        .filter(|artifact| !current.contains(artifact))
        .cloned()
        .collect()
}

fn transaction_plan(
    approved: &ApprovedLayout,
    stale: &[ApprovedTarget],
) -> Result<TransactionPlan, CliError> {
    let mut entries = Vec::with_capacity(
        approved.artifacts.len() + stale.len() + 1 + usize::from(approved.document.is_some()),
    );
    for artifact in &approved.artifacts {
        entries.push(artifact.write_entry(TransactionRole::Artifact));
    }
    for artifact in stale {
        entries.push(artifact.delete_artifact_entry());
    }
    entries.push(approved.manifest.write_entry(TransactionRole::Manifest));
    if let Some(document) = &approved.document {
        entries.push(document.write_entry(TransactionRole::Document));
    }
    TransactionPlan::for_generation(approved.owner.clone(), entries).map_err(Into::into)
}

fn issue_stage_slots(
    staging: &mut StagingTransaction,
    approved: &ApprovedLayout,
) -> Result<StageSlots, CliError> {
    let artifacts = approved
        .artifacts
        .iter()
        .map(|artifact| staging.stage_slot(&artifact.target).map_err(CliError::from))
        .collect::<Result<Vec<_>, _>>()?;
    let manifest = staging.stage_slot(&approved.manifest.target)?;
    let document = approved
        .document
        .as_ref()
        .map(|document| staging.stage_slot(&document.target))
        .transpose()?;
    Ok(StageSlots {
        artifacts,
        manifest,
        document,
    })
}

fn stage_generation_metadata(
    source: &str,
    charts: &[MarkdownChart<'_>],
    images: &[MarkdownImage],
    manifest_slot: StageSlot,
    document_slot: Option<StageSlot>,
    manifest_bytes: &[u8],
    staged_bytes: &Mutex<CheckedBytes>,
) -> Result<(), CliError> {
    charge_and_stage(manifest_slot, manifest_bytes, staged_bytes)?;
    if let Some(document_slot) = document_slot {
        let rewritten_len = markdown::rewritten_markdown_len(source, charts, images)?;
        charge_staged_bytes(staged_bytes, rewritten_len)?;
        let rewritten =
            markdown::replace_known_charts_with_images(source, charts, images, rewritten_len);
        document_slot.write_bytes(rewritten.as_bytes())?;
    }
    Ok(())
}

fn charge_and_stage(
    slot: StageSlot,
    bytes: &[u8],
    staged_bytes: &Mutex<CheckedBytes>,
) -> Result<(), CliError> {
    charge_staged_bytes(staged_bytes, bytes.len())?;
    slot.write_bytes(bytes)?;
    Ok(())
}

fn charge_staged_bytes(staged_bytes: &Mutex<CheckedBytes>, bytes: usize) -> Result<(), CliError> {
    let bytes = u64::try_from(bytes)
        .map_err(|_| CliError::InvalidOutput("staged output size overflow".to_string()))?;
    staged_bytes
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .try_add(bytes)?;
    Ok(())
}

fn abort_staging(
    context: &mut ExecutionContext,
    staging: StagingTransaction,
    error: CliError,
) -> Result<(), CliError> {
    context.publication.abort_transaction(staging)?;
    Err(error)
}

fn report_chart_count(quiet: bool, count: usize, stderr: &SharedWriter) {
    let sink = crate::diagnostics::DiagnosticSink::new(quiet, stderr);
    if count == 0 {
        sink.info("No mermaid charts found in Markdown input");
    } else {
        sink.info(format!("Found {count} mermaid charts in Markdown input"));
    }
}

fn report_publication(
    quiet: bool,
    requested: &RequestedLayout,
    images: &[MarkdownImage],
    stderr: &SharedWriter,
) {
    let sink = crate::diagnostics::DiagnosticSink::new(quiet, stderr);
    for image in images {
        sink.info(format!("Wrote {}", image.url));
    }
    if requested.writes_document {
        sink.info(format!(
            "Wrote {}",
            crate::error::safe_path(&requested.output)
        ));
    }
}
