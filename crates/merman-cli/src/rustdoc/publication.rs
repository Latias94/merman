use super::config::Config;
use super::receipt::{
    ExpectedRustdocBundle, PreviousRustdocReceipt, decode_previous, read_previous, receipt_limit,
};
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
    previous: Option<PreviousRustdocReceipt>,
}

struct ApprovedStale {
    target: RelativeTarget,
    generation: TargetGeneration,
}

struct ManagedTargetObservation {
    generation: TargetGeneration,
    matches_expected: bool,
    sha256: Option<[u8; 32]>,
    captured_bytes: Option<Vec<u8>>,
}

pub(crate) fn build(
    config: &Config,
    resources: &ResolvedResourcePolicy,
    control: &merman::OperationControl,
    publications: &mut PublicationGuards,
    context: &mut ExecutionContext,
    quiet: bool,
) -> Result<(), CliError> {
    let receipt_path = config.receipt_path();
    let previous_before_generation = read_previous(&receipt_path, resources)?;
    if let Some(previous) = previous_before_generation.as_ref() {
        previous.ensure_owner(config, &receipt_path)?;
    }
    let generated = super::generate(config, resources, control, &context.stderr)?;
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
    super::document::verify_input_snapshots(config, expected.generated(), resources, control)?;

    let mut backup_bytes = resources.checked_bytes(ByteLedgerKind::StagedOutput);
    let approved_fragments =
        approve_fragments(&expected, publications, acquired.root(), &mut backup_bytes)?;
    let approved_receipt = approve_receipt(
        &expected,
        resources,
        publications,
        acquired.root(),
        &mut backup_bytes,
    )?;
    if let Some(previous) = approved_receipt.previous.as_ref() {
        previous.ensure_owner(config, expected.receipt_path())?;
    }
    let approved_stale = approve_stale(
        &expected,
        approved_receipt.previous.as_ref(),
        &acquired,
        publications,
        &mut backup_bytes,
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
    plan.validate_pinned_backup_bytes(resources.value(CliResourceLimitId::MaxStagedBytes))?;
    let mut staging = context.publication.begin_transaction(acquired, plan)?;
    let mut staged_bytes = resources.checked_bytes(ByteLedgerKind::StagedOutput);
    let stage_result = (|| {
        for fragment in &approved_fragments {
            let bytes = expected.generated().fragments()[fragment.index].bytes();
            charge(&mut staged_bytes, bytes.len())?;
            staging
                .stage_slot(&fragment.target)?
                .write_bytes_controlled(bytes, control)?;
        }
        charge(&mut staged_bytes, expected.receipt_bytes().len())?;
        staging
            .stage_slot(&approved_receipt.target)?
            .write_bytes_controlled(expected.receipt_bytes(), control)?;
        super::document::verify_input_snapshots(config, expected.generated(), resources, control)?;
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
            super::document::verify_input_snapshots(
                config,
                expected.generated(),
                resources,
                control,
            )
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
    backup_bytes: &mut crate::resources::CheckedBytes,
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
        let observation = observe_managed_target(
            &path,
            Some(fragment.bytes()),
            generation,
            backup_bytes,
            None,
        )?;
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
    resources: &ResolvedResourcePolicy,
    publications: &PublicationGuards,
    root: &Path,
    backup_bytes: &mut crate::resources::CheckedBytes,
) -> Result<ApprovedReceipt, CliError> {
    let target = publications.approved_transaction_target(expected.receipt_path())?;
    let (path, generation) = target.into_parts();
    if path != expected.receipt_path() {
        return Err(publication_error(
            &path,
            "approved receipt path changed after preflight",
        ));
    }
    let observation = observe_managed_target(
        &path,
        Some(expected.receipt_bytes()),
        generation,
        backup_bytes,
        Some(receipt_limit(resources)),
    )?;
    let previous = observation
        .captured_bytes
        .as_deref()
        .map(|bytes| decode_previous(&path, bytes))
        .transpose()?;
    Ok(ApprovedReceipt {
        target: RelativeTarget::from_absolute(root, &path)?,
        generation: observation.generation,
        changed: !observation.matches_expected,
        previous,
    })
}

fn approve_stale(
    expected: &ExpectedRustdocBundle,
    previous: Option<&PreviousRustdocReceipt>,
    acquired: &AcquiredTransaction,
    publications: &PublicationGuards,
    backup_bytes: &mut crate::resources::CheckedBytes,
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
        let observation = observe_managed_target(&path, None, generation, backup_bytes, None)?;
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
    backup_bytes: &mut crate::resources::CheckedBytes,
    capture_limit: Option<usize>,
) -> Result<ManagedTargetObservation, CliError> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if generation.matches_identity(None) {
                return Ok(ManagedTargetObservation {
                    generation,
                    matches_expected: false,
                    sha256: None,
                    captured_bytes: None,
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
    backup_bytes.try_add(metadata.len()).map_err(|error| {
        publication_error(
            path,
            format!("managed target backup budget exceeded: {error}"),
        )
    })?;
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
    let mut file = file;
    let mut hasher = Sha256::new();
    let mut offset = 0usize;
    let mut matches_expected = expected.is_some_and(|expected| {
        metadata.len() == u64::try_from(expected.len()).unwrap_or(u64::MAX)
    });
    let observed_len = metadata.len();
    let mut captured_bytes = match capture_limit {
        Some(limit) => {
            if observed_len > limit as u64 {
                return Err(publication_error(
                    path,
                    format!("managed receipt exceeds the {limit}-byte limit"),
                ));
            }
            let capacity = usize::try_from(observed_len).map_err(|_| {
                publication_error(path, "managed receipt length does not fit this platform")
            })?;
            Some(Vec::with_capacity(capacity))
        }
        None => None,
    };
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
        if let Some(captured_bytes) = captured_bytes.as_mut() {
            captured_bytes.extend_from_slice(&buffer[..read]);
        }
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
        captured_bytes,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn observed_generation(path: &Path) -> TargetGeneration {
        let identity = same_file::Handle::from_path(path).expect("target identity");
        TargetGeneration::from_preflight_identity(Some(Arc::new(identity)))
    }

    #[test]
    fn managed_target_backup_budget_is_aggregate_across_sparse_outputs() {
        let root = tempfile::tempdir().expect("tempdir");
        let first = root.path().join("first.md");
        let second = root.path().join("second.md");
        File::create(&first)
            .and_then(|file| file.set_len(6))
            .expect("first sparse target");
        File::create(&second)
            .and_then(|file| file.set_len(6))
            .expect("second sparse target");

        let mut resources = ResolvedResourcePolicy::for_profile(
            merman::resources::ResourceProfile::UnboundedForTrustedInput,
        );
        resources
            .apply_override("max_staged_bytes", 10)
            .expect("test override");
        let mut backup_bytes = resources.checked_bytes(ByteLedgerKind::StagedOutput);

        observe_managed_target(
            &first,
            None,
            observed_generation(&first),
            &mut backup_bytes,
            None,
        )
        .expect("first target fits aggregate budget");
        let error = match observe_managed_target(
            &second,
            None,
            observed_generation(&second),
            &mut backup_bytes,
            None,
        ) {
            Ok(_) => panic!("second target must exceed aggregate budget"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("max_staged_bytes"), "{error}");
        assert!(!root.path().join(".merman.transaction").exists());
    }

    #[test]
    fn captured_receipt_bytes_share_the_content_pin_read() {
        let root = tempfile::tempdir().expect("tempdir");
        let receipt = root.path().join("receipt.json");
        std::fs::write(&receipt, b"approved receipt bytes").expect("write receipt");
        let generation = observed_generation(&receipt);
        let mut resources = ResolvedResourcePolicy::for_profile(
            merman::resources::ResourceProfile::UnboundedForTrustedInput,
        );
        resources
            .apply_override("max_staged_bytes", 1024)
            .expect("test override");
        let mut backup_bytes = resources.checked_bytes(ByteLedgerKind::StagedOutput);

        let observation =
            observe_managed_target(&receipt, None, generation, &mut backup_bytes, Some(1024))
                .expect("observe receipt");
        std::fs::write(&receipt, b"temporary alternate bytes").expect("write alternate receipt");
        std::fs::write(&receipt, b"approved receipt bytes").expect("restore approved receipt");

        assert_eq!(
            observation.captured_bytes.as_deref(),
            Some(b"approved receipt bytes".as_slice())
        );
        let expected_sha256: [u8; 32] = Sha256::digest(b"approved receipt bytes").into();
        assert_eq!(observation.sha256, Some(expected_sha256));
    }
}
