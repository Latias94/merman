use super::config::Config;
use super::receipt::{ExpectedRustdocBundle, PreviousRustdocReceipt, read_previous};
use crate::diagnostics::DiagnosticSink;
use crate::error::{CliError, safe_path};
use crate::output::{AcquiredTransaction, PublicationGuards};
use crate::resources::{ByteLedgerKind, CliResourceLimitId, ResolvedResourcePolicy};
use crate::runtime::ExecutionContext;
use crate::transaction::{
    GenerationOwner, RelativeTarget, TargetGeneration, TransactionEntryPlan, TransactionPlan,
    TransactionRole,
};
use sha2::{Digest as _, Sha256};
use std::collections::BTreeSet;
use std::fs::File;
use std::io::Read;
use std::path::Path;

struct ApprovedFragment {
    index: usize,
    target: RelativeTarget,
    generation: TargetGeneration,
    changed: bool,
}

struct ApprovedReceipt {
    target: RelativeTarget,
    generation: TargetGeneration,
    changed: bool,
}

struct ApprovedStale {
    target: RelativeTarget,
    generation: TargetGeneration,
}

struct ManagedTargetObservation {
    generation: TargetGeneration,
    matches_expected: bool,
    sha256: Option<[u8; 32]>,
}

pub(crate) fn build(
    config: &Config,
    resources: &ResolvedResourcePolicy,
    publications: &mut PublicationGuards,
    context: &mut ExecutionContext,
    quiet: bool,
) -> Result<(), CliError> {
    let receipt_path = config.receipt_path();
    let previous_before_generation = read_previous(&receipt_path, resources)?;
    if let Some(previous) = previous_before_generation.as_ref() {
        previous.ensure_owner(config, &receipt_path)?;
    }
    let generated = super::generate(config, resources, &context.stderr)?;
    for input in generated.inputs() {
        publications.protect_rustdoc_input(input.path(), input.identity())?;
    }
    let expected = ExpectedRustdocBundle::new(config, generated, resources)?;

    // Acquiring the publication root is the first filesystem mutation. All inputs have already
    // been acquired, rendered, validated, and registered as protected generations.
    let acquired = context.publication.acquire_transaction(publications)?;
    if acquired.root() != config.output_root() {
        return Err(publication_error(
            acquired.root(),
            format!(
                "approved transaction root does not match Rustdoc output root {}",
                safe_path(config.output_root())
            ),
        ));
    }
    super::document::verify_input_snapshots(config, expected.generated(), resources)?;

    let approved_fragments =
        approve_fragments(&expected, publications, acquired.root(), resources)?;
    let approved_receipt = approve_receipt(&expected, publications, acquired.root(), resources)?;
    let previous = read_previous(expected.receipt_path(), resources)?;
    if let Some(previous) = previous.as_ref() {
        previous.ensure_owner(config, expected.receipt_path())?;
    }
    let approved_stale = approve_stale(
        &expected,
        previous.as_ref(),
        &acquired,
        publications,
        resources,
    )?;
    let changed_fragments = approved_fragments
        .iter()
        .filter(|fragment| fragment.changed)
        .count();
    let changed = changed_fragments > 0 || approved_receipt.changed || !approved_stale.is_empty();

    let owner = GenerationOwner::rustdoc(approved_receipt.target.clone())?;
    let mut entries = Vec::with_capacity(approved_fragments.len() + approved_stale.len() + 1);
    entries.extend(approved_fragments.iter().map(|fragment| {
        TransactionEntryPlan::write(TransactionRole::Artifact, fragment.target.clone())
            .expect_generation(fragment.generation.clone())
    }));
    entries.extend(approved_stale.iter().map(|stale| {
        TransactionEntryPlan::delete_artifact(stale.target.clone())
            .expect_generation(stale.generation.clone())
    }));
    entries.push(
        TransactionEntryPlan::write(TransactionRole::Manifest, approved_receipt.target.clone())
            .expect_generation(approved_receipt.generation.clone()),
    );
    let plan = TransactionPlan::for_generation(owner, entries)?;
    let mut staging = context.publication.begin_transaction(acquired, plan)?;
    let mut staged_bytes = resources.checked_bytes(ByteLedgerKind::StagedOutput);
    let stage_result = (|| {
        for fragment in &approved_fragments {
            let bytes = expected.generated().fragments()[fragment.index].bytes();
            charge(&mut staged_bytes, bytes.len())?;
            staging.stage_slot(&fragment.target)?.write_bytes(bytes)?;
        }
        charge(&mut staged_bytes, expected.receipt_bytes().len())?;
        staging
            .stage_slot(&approved_receipt.target)?
            .write_bytes(expected.receipt_bytes())?;
        super::document::verify_input_snapshots(config, expected.generated(), resources)?;
        Ok::<(), CliError>(())
    })();
    if let Err(error) = stage_result {
        context.publication.abort_transaction(staging)?;
        return Err(error);
    }
    let ready = context.publication.ready_transaction(staging)?;
    context
        .publication
        .commit_transaction_verified(ready, &mut || {
            super::document::verify_input_snapshots(config, expected.generated(), resources)
        })?;

    report(
        quiet,
        &context.stderr,
        &expected,
        changed_fragments,
        approved_stale.len(),
        if changed {
            "Built Rustdoc fragments"
        } else {
            "Rustdoc fragments are already up to date"
        },
    );
    Ok(())
}

fn approve_fragments(
    expected: &ExpectedRustdocBundle,
    publications: &PublicationGuards,
    root: &Path,
    resources: &ResolvedResourcePolicy,
) -> Result<Vec<ApprovedFragment>, CliError> {
    let mut approved = Vec::new();
    for (index, fragment) in expected.generated().fragments().iter().enumerate() {
        let target = publications.approved_transaction_target(fragment.output())?;
        let (path, generation) = target.into_parts();
        if path != fragment.output() {
            return Err(publication_error(
                &path,
                "approved fragment path changed after preflight",
            ));
        }
        let observation =
            observe_managed_target(&path, Some(fragment.bytes()), generation, resources)?;
        approved.push(ApprovedFragment {
            index,
            target: RelativeTarget::from_absolute(root, &path)?,
            generation: observation.generation,
            changed: !observation.matches_expected,
        });
    }
    Ok(approved)
}

fn approve_receipt(
    expected: &ExpectedRustdocBundle,
    publications: &PublicationGuards,
    root: &Path,
    resources: &ResolvedResourcePolicy,
) -> Result<ApprovedReceipt, CliError> {
    let target = publications.approved_transaction_target(expected.receipt_path())?;
    let (path, generation) = target.into_parts();
    if path != expected.receipt_path() {
        return Err(publication_error(
            &path,
            "approved receipt path changed after preflight",
        ));
    }
    let observation =
        observe_managed_target(&path, Some(expected.receipt_bytes()), generation, resources)?;
    Ok(ApprovedReceipt {
        target: RelativeTarget::from_absolute(root, &path)?,
        generation: observation.generation,
        changed: !observation.matches_expected,
    })
}

fn approve_stale(
    expected: &ExpectedRustdocBundle,
    previous: Option<&PreviousRustdocReceipt>,
    acquired: &AcquiredTransaction,
    publications: &PublicationGuards,
    resources: &ResolvedResourcePolicy,
) -> Result<Vec<ApprovedStale>, CliError> {
    let current = expected
        .generated()
        .fragments()
        .iter()
        .map(|fragment| {
            fragment
                .output()
                .file_name()
                .and_then(|name| name.to_str())
                .expect("validated fragment ids produce portable UTF-8 filenames")
        })
        .collect::<BTreeSet<_>>();
    let Some(previous) = previous else {
        return Ok(Vec::new());
    };

    let mut stale = Vec::new();
    for (name, expected_sha256) in previous
        .fragment_outputs()
        .filter(|(name, _)| !current.contains(name))
    {
        let requested = acquired.root().join(name);
        let target = RelativeTarget::from_absolute(acquired.root(), &requested)?;
        let approved = acquired.approve_stale_artifact(publications, &target)?;
        let (path, generation) = approved.into_parts();
        let approved_target = RelativeTarget::from_absolute(acquired.root(), &path)?;
        if approved_target != target {
            return Err(publication_error(
                &requested,
                "stale receipt target changed during approval",
            ));
        }
        let observation = observe_managed_target(&path, None, generation, resources)?;
        let Some(actual_sha256) = observation.sha256 else {
            continue;
        };
        if super::encode_lower_hex(&actual_sha256) != expected_sha256 {
            return Err(publication_error(
                &path,
                "stale managed target no longer matches the output recorded by the prior receipt",
            ));
        }
        stale.push(ApprovedStale {
            target: approved_target,
            generation: observation.generation,
        });
    }
    Ok(stale)
}

fn observe_managed_target(
    path: &Path,
    expected: Option<&[u8]>,
    generation: TargetGeneration,
    resources: &ResolvedResourcePolicy,
) -> Result<ManagedTargetObservation, CliError> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if generation.matches_identity(None) {
                return Ok(ManagedTargetObservation {
                    generation,
                    matches_expected: false,
                    sha256: None,
                });
            }
            return Err(publication_error(
                path,
                "managed target disappeared after preflight",
            ));
        }
        Err(error) => {
            return Err(publication_error(
                path,
                format!("failed to inspect managed target: {error}"),
            ));
        }
    };
    if !metadata.file_type().is_file() {
        return Err(publication_error(
            path,
            "managed target is a symlink or non-regular file",
        ));
    }
    let file = File::open(path).map_err(|error| {
        publication_error(path, format!("failed to open managed target: {error}"))
    })?;
    let opened_metadata = file.metadata().map_err(|error| {
        publication_error(
            path,
            format!("failed to inspect opened managed target: {error}"),
        )
    })?;
    if !opened_metadata.is_file() || opened_metadata.len() != metadata.len() {
        return Err(publication_error(
            path,
            "managed target length changed while it was opened",
        ));
    }
    let opened_identity = same_file::Handle::from_file(file.try_clone().map_err(|error| {
        publication_error(
            path,
            format!("failed to clone managed target for identity inspection: {error}"),
        )
    })?)
    .map_err(|error| {
        publication_error(
            path,
            format!("failed to inspect opened managed target identity: {error}"),
        )
    })?;
    if !generation.matches_identity(Some(&opened_identity)) {
        return Err(publication_error(
            path,
            "managed target identity changed after preflight",
        ));
    }
    if let Some(limit) = resources.value(CliResourceLimitId::MaxStagedBytes)
        && metadata.len() > limit
    {
        return Err(publication_error(
            path,
            format!(
                "managed target has {} bytes, exceeding the {limit}-byte observation limit",
                metadata.len()
            ),
        ));
    }
    let mut file = file;
    let mut hasher = Sha256::new();
    let mut offset = 0usize;
    let mut matches_expected = expected.is_some_and(|expected| {
        metadata.len() == u64::try_from(expected.len()).unwrap_or(u64::MAX)
    });
    let observed_len = metadata.len();
    let mut remaining = observed_len;
    let mut buffer = [0u8; 64 * 1024];
    while remaining > 0 {
        let requested = usize::try_from(remaining.min(buffer.len() as u64))
            .expect("the bounded managed-target buffer length fits usize");
        let read = file.read(&mut buffer[..requested]).map_err(|error| {
            publication_error(path, format!("failed to read managed target: {error}"))
        })?;
        if read == 0 {
            return Err(publication_error(
                path,
                "managed target became shorter while reading",
            ));
        }
        hasher.update(&buffer[..read]);
        if matches_expected {
            let end = offset.saturating_add(read);
            matches_expected = expected.is_some_and(|expected| {
                expected
                    .get(offset..end)
                    .is_some_and(|slice| slice == &buffer[..read])
            });
            offset = end;
        }
        remaining -= read as u64;
    }
    let mut extra = [0u8; 1];
    if file.read(&mut extra).map_err(|error| {
        publication_error(
            path,
            format!("failed to probe managed target length: {error}"),
        )
    })? != 0
    {
        return Err(publication_error(
            path,
            "managed target became longer while reading",
        ));
    }
    let final_metadata = std::fs::symlink_metadata(path).map_err(|error| {
        publication_error(
            path,
            format!("failed to reinspect managed target after reading: {error}"),
        )
    })?;
    if !final_metadata.file_type().is_file() || final_metadata.len() != observed_len {
        return Err(publication_error(
            path,
            "managed target type or length changed while reading",
        ));
    }
    let final_identity = same_file::Handle::from_path(path).map_err(|error| {
        publication_error(
            path,
            format!("failed to reinspect managed target identity: {error}"),
        )
    })?;
    if !generation.matches_identity(Some(&final_identity)) {
        return Err(publication_error(
            path,
            "managed target identity changed while reading",
        ));
    }
    let sha256: [u8; 32] = hasher.finalize().into();
    Ok(ManagedTargetObservation {
        generation: generation.pin_content(observed_len, sha256),
        matches_expected,
        sha256: Some(sha256),
    })
}

fn charge(staged_bytes: &mut crate::resources::CheckedBytes, bytes: usize) -> Result<(), CliError> {
    let bytes = u64::try_from(bytes)
        .map_err(|_| CliError::InvalidOutput("staged output size overflow".to_string()))?;
    staged_bytes.try_add(bytes).map_err(Into::into)
}

fn publication_error(path: impl AsRef<Path>, reason: impl Into<String>) -> CliError {
    super::operational_error(path, reason)
}

fn report(
    quiet: bool,
    stderr: &crate::runtime::SharedWriter,
    expected: &ExpectedRustdocBundle,
    changed: usize,
    removed: usize,
    prefix: &str,
) {
    DiagnosticSink::new(quiet, stderr).info(format!(
        "{prefix} ({} fragments, {} diagrams, {changed} updated, {removed} removed)",
        expected.generated().fragments().len(),
        expected.generated().diagrams()
    ));
}
