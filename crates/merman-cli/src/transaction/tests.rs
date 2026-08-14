use super::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering as AtomicOrdering};

#[derive(Clone)]
struct Targets {
    artifact_a: RelativeTarget,
    artifact_z: RelativeTarget,
    manifest: RelativeTarget,
    document: RelativeTarget,
}

impl Targets {
    fn under(root: &Path) -> Self {
        Self {
            artifact_a: relative(root, "a.svg"),
            artifact_z: relative(root, "z.svg"),
            manifest: relative(root, ".merman-manifest.json"),
            document: relative(root, "document.md"),
        }
    }

    fn standard_plan(&self) -> TransactionPlan {
        TransactionPlan::new([
            TransactionEntryPlan::write(TransactionRole::Document, self.document.clone()),
            TransactionEntryPlan::write(TransactionRole::Artifact, self.artifact_z.clone()),
            TransactionEntryPlan::write(TransactionRole::Manifest, self.manifest.clone()),
            TransactionEntryPlan::write(TransactionRole::Artifact, self.artifact_a.clone()),
        ])
        .unwrap()
    }
}

fn generation_owner(
    root: &Path,
    owner: &RelativeTarget,
    directory: impl AsRef<Path>,
    stem: impl AsRef<std::ffi::OsStr>,
    extension: impl AsRef<std::ffi::OsStr>,
) -> GenerationOwner {
    GenerationOwner::new(
        GenerationDialect::NativeBatchV1,
        owner.clone(),
        ArtifactNamespace::from_absolute(root, directory, stem, extension).unwrap(),
    )
    .unwrap()
}

fn existing_generation(path: &Path) -> TargetGeneration {
    TargetGeneration::Existing {
        identity: Arc::new(same_file::Handle::from_path(path).unwrap()),
        content: None,
    }
}

#[cfg(feature = "rustdoc")]
fn content_pinned_generation(path: &Path) -> TargetGeneration {
    use sha2::{Digest as _, Sha256};

    let digest: [u8; 32] = Sha256::digest(std::fs::read(path).unwrap()).into();
    existing_generation(path).pin_content(std::fs::metadata(path).unwrap().len(), digest)
}

#[cfg(unix)]
#[test]
fn approved_root_replacement_is_rejected_before_lock_creation() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("output");
    let displaced = temp.path().join("displaced");
    std::fs::create_dir(&root).unwrap();
    let approved = crate::output::ApprovedTransactionRoot::for_test(&root).unwrap();

    std::fs::rename(&root, &displaced).unwrap();
    std::fs::create_dir(&root).unwrap();

    let error = LockedRecoveredRoot::acquire_approved(approved).unwrap_err();
    assert!(matches!(error, TransactionError::InvalidState { .. }));
    assert!(!root.join(LOCK_FILE_NAME).exists());
    assert!(!displaced.join(LOCK_FILE_NAME).exists());
}

#[cfg(windows)]
#[test]
fn approved_root_handle_prevents_replacement_before_lock_creation() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("output");
    let displaced = temp.path().join("displaced");
    std::fs::create_dir(&root).unwrap();
    let approved = crate::output::ApprovedTransactionRoot::for_test(&root).unwrap();

    std::fs::rename(&root, &displaced).unwrap_err();

    assert!(root.is_dir());
    assert!(!displaced.exists());
    assert!(!root.join(LOCK_FILE_NAME).exists());
    drop(approved);
}

#[test]
fn relative_targets_and_generation_manifests_round_trip_losslessly() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let document = relative(&root, "document.md");
    let artifact = relative(&root, "nested/output-1.svg");
    std::fs::create_dir(root.join("nested")).unwrap();

    assert_eq!(
        artifact.to_path(&root).unwrap(),
        root.join("nested/output-1.svg")
    );
    assert_eq!(artifact.components().count(), 2);
    assert_eq!(artifact.order().len(), 2);

    let manifest = GenerationManifest::new(
        "0123456789abcdef0123456789abcdef",
        generation_owner(&root, &document, root.join("nested"), "output", "svg"),
        Some(document.clone()),
        vec![artifact.clone()],
    )
    .unwrap();
    let encoded = manifest.encode().unwrap();
    let decoded = GenerationManifest::decode_bounded(&encoded, &root).unwrap();
    assert_eq!(decoded.generation_id(), manifest.generation_id());
    assert_eq!(decoded.document(), Some(&document));
    assert_eq!(decoded.artifacts(), &[artifact]);
    let path = root.join(".merman-manifest.json");
    std::fs::write(&path, &encoded).unwrap();
    assert_eq!(
        GenerationManifest::read_bounded(&path, &root).unwrap(),
        manifest
    );
}

#[cfg(unix)]
#[test]
fn relative_targets_preserve_non_utf8_unix_components() {
    use std::os::unix::ffi::OsStringExt;

    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let component = std::ffi::OsString::from_vec(vec![b'o', b'u', b't', 0x80]);
    let mut file_name = component.clone();
    file_name.push("-1.svg");
    let target = RelativeTarget::from_absolute(&root, root.join(&file_name)).unwrap();
    let owner_target = relative(&root, "document.md");
    let manifest = GenerationManifest::new(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        GenerationOwner::new(
            GenerationDialect::NativeBatchV1,
            owner_target,
            ArtifactNamespace::from_absolute(&root, &root, &component, "svg").unwrap(),
        )
        .unwrap(),
        None,
        vec![target.clone()],
    )
    .unwrap();
    let decoded = GenerationManifest::decode_bounded(&manifest.encode().unwrap(), &root).unwrap();
    assert_eq!(decoded.artifacts(), &[target]);
}

#[test]
fn transaction_plan_sorts_artifacts_and_keeps_document_last() {
    let temp = tempfile::tempdir().unwrap();
    let targets = Targets::under(temp.path());
    let plan = targets.standard_plan();
    assert_eq!(
        plan.entries()
            .iter()
            .map(|entry| entry.role)
            .collect::<Vec<_>>(),
        vec![
            TransactionRole::Artifact,
            TransactionRole::Artifact,
            TransactionRole::Manifest,
            TransactionRole::Document,
        ]
    );
    assert_eq!(plan.entries()[0].target(), &targets.artifact_a);
    assert_eq!(plan.entries()[1].target(), &targets.artifact_z);
    assert_eq!(plan.target_indices.len(), plan.entries().len());
    assert_eq!(plan.target_indices.get(&targets.artifact_a), Some(&0));
    assert_eq!(plan.target_indices.get(&targets.artifact_z), Some(&1));
    assert_eq!(plan.target_indices.get(&targets.manifest), Some(&2));
    assert_eq!(plan.target_indices.get(&targets.document), Some(&3));
}

#[test]
fn transaction_plan_indexes_every_target_across_representative_cardinalities() {
    let temp = tempfile::tempdir().unwrap();
    for artifact_count in [0, 1, 16, 64, 256] {
        let mut entries = (0..artifact_count)
            .map(|index| {
                let target = relative(temp.path(), format!("artifact-{index:03}.svg"));
                if index % 2 == 0 {
                    TransactionEntryPlan::write(TransactionRole::Artifact, target)
                } else {
                    TransactionEntryPlan::delete_artifact(target)
                }
            })
            .collect::<Vec<_>>();
        entries.push(TransactionEntryPlan::write(
            TransactionRole::Manifest,
            relative(temp.path(), ".merman-manifest.json"),
        ));

        let plan = TransactionPlan::new(entries).unwrap();
        assert_eq!(plan.entries().len(), artifact_count + 1);
        assert_eq!(plan.target_indices.len(), plan.entries().len());
        for (index, entry) in plan.entries().iter().enumerate() {
            assert_eq!(plan.target_indices.get(entry.target()), Some(&index));
        }
    }
}

#[test]
fn oversized_transaction_plan_fails_before_creating_transaction_evidence() {
    let temp = tempfile::tempdir().unwrap();
    #[cfg(windows)]
    let component_bytes = 1_800;
    #[cfg(not(windows))]
    let component_bytes = 3_800;
    let mut entries = (0..200)
        .map(|index| {
            let name = format!("{index:03}-{}", "x".repeat(component_bytes));
            let target = RelativeTarget::from_components(
                vec![std::ffi::OsString::from(name)],
                Path::new(TRANSACTION_DIR_NAME),
            )
            .unwrap();
            TransactionEntryPlan::write(TransactionRole::Artifact, target)
        })
        .collect::<Vec<_>>();
    entries.push(TransactionEntryPlan::write(
        TransactionRole::Manifest,
        relative(temp.path(), ".merman-manifest.json"),
    ));
    let plan = TransactionPlan::new(entries).unwrap();

    let error = LockedRecoveredRoot::acquire(temp.path())
        .unwrap()
        .begin(plan)
        .err()
        .expect("oversized journal must fail");

    assert!(
        error.to_string().contains("hard state-size limit"),
        "{error}"
    );
    assert!(
        !temp.path().join(TRANSACTION_DIR_NAME).exists(),
        "journal capacity must be checked before transaction evidence is created"
    );
}

#[test]
fn journal_capacity_models_the_largest_platform_prior_encoding() {
    let target = RelativeTarget::from_components(
        vec![std::ffi::OsString::from("artifact.svg")],
        Path::new(TRANSACTION_DIR_NAME),
    )
    .unwrap();
    let owner = GenerationOwner::test_fixture();
    let state = |prior, prior_mode| JournalState {
        id: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        owner: owner.clone(),
        sequence: u64::MAX,
        phase: JournalPhase::RollingBack,
        next_index: 1,
        entries: vec![JournalEntry {
            role: TransactionRole::Manifest,
            operation: TransactionOperation::Write,
            target: target.clone(),
            prior,
            prior_mode,
        }],
    };
    let mut maximum = state(PriorState::Unknown, None);
    maximize_prior_encoding(&mut maximum.entries[0]);
    let maximum = maximum.encode(Path::new(TRANSACTION_DIR_NAME)).unwrap();
    #[cfg(unix)]
    let present_mode = Some(0o777);
    #[cfg(not(unix))]
    let present_mode = None;
    let present = state(PriorState::Present, present_mode)
        .encode(Path::new(TRANSACTION_DIR_NAME))
        .unwrap();

    assert!(
        maximum.len() >= present.len(),
        "capacity preflight must use an encoding at least as large as a present prior file: maximum={}, present={}",
        maximum.len(),
        present.len()
    );
}

#[test]
fn stage_target_lookup_count_is_one_per_request_at_scale() {
    for target_count in [1_usize, 16, 64, 256] {
        let temp = tempfile::tempdir().unwrap();
        let mut targets = (0..target_count.saturating_sub(1))
            .map(|index| relative(temp.path(), format!("artifact-{index:03}.svg")))
            .collect::<Vec<_>>();
        targets.push(relative(temp.path(), ".merman-manifest.json"));

        let entries = targets
            .iter()
            .enumerate()
            .map(|(index, target)| {
                let role = if index + 1 == targets.len() {
                    TransactionRole::Manifest
                } else {
                    TransactionRole::Artifact
                };
                TransactionEntryPlan::write(role, target.clone())
            })
            .collect::<Vec<_>>();
        let mut staging = LockedRecoveredRoot::acquire(temp.path())
            .unwrap()
            .begin(TransactionPlan::new(entries).unwrap())
            .unwrap();

        for target in &targets {
            staging.stage_bytes(target, b"x").unwrap();
        }

        assert_eq!(staging.target_lookup_count(), target_count);
    }
}

#[test]
fn transaction_plan_rejects_duplicate_targets_before_building_the_index() {
    let temp = tempfile::tempdir().unwrap();
    let artifact = relative(temp.path(), "duplicate.svg");
    let error = TransactionPlan::new([
        TransactionEntryPlan::write(TransactionRole::Artifact, artifact.clone()),
        TransactionEntryPlan::write(TransactionRole::Artifact, artifact),
    ])
    .unwrap_err();

    assert!(matches!(&error, TransactionError::InvalidState { .. }));
    assert!(error.to_string().contains("duplicate targets"));
}

#[test]
fn live_transaction_plan_requires_every_preflight_generation() {
    let temp = tempfile::tempdir().unwrap();
    let manifest = relative(temp.path(), ".merman-manifest.json");

    let error = TransactionPlan::for_generation(
        GenerationOwner::test_fixture(),
        [TransactionEntryPlan::write(
            TransactionRole::Manifest,
            manifest,
        )],
    )
    .unwrap_err();

    assert!(matches!(error, TransactionError::InvalidState { .. }));
}

#[test]
fn reserved_names_and_ascii_case_collisions_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    for name in [".MERMAN.LOCK", ".Merman.Transaction"] {
        let error = RelativeTarget::from_absolute(&root, root.join(name)).unwrap_err();
        assert!(matches!(error, TransactionError::InvalidState { .. }));
    }

    let upper = relative(&root, "Diagram.svg");
    let lower = relative(&root, "diagram.svg");
    let manifest = relative(&root, ".merman-manifest.json");
    let error = TransactionPlan::new([
        TransactionEntryPlan::write(TransactionRole::Artifact, upper),
        TransactionEntryPlan::write(TransactionRole::Artifact, lower),
        TransactionEntryPlan::write(TransactionRole::Manifest, manifest),
    ])
    .unwrap_err();
    assert!(matches!(error, TransactionError::InvalidState { .. }));

    let composed = relative(&root, "\u{e9}.svg");
    let decomposed = relative(&root, "e\u{301}.svg");
    let manifest = relative(&root, ".merman-manifest.json");
    let error = TransactionPlan::new([
        TransactionEntryPlan::write(TransactionRole::Artifact, composed),
        TransactionEntryPlan::write(TransactionRole::Artifact, decomposed),
        TransactionEntryPlan::write(TransactionRole::Manifest, manifest),
    ])
    .unwrap_err();
    assert!(matches!(error, TransactionError::InvalidState { .. }));
}

#[test]
fn generation_manifest_limits_apply_before_allocation_and_after_encoding() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let artifact = relative(&root, "a.svg");
    let error = GenerationManifest::new(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        generation_owner(&root, &relative(&root, "document.md"), &root, "a", "svg"),
        None,
        vec![artifact; 65_536],
    )
    .unwrap_err();
    assert!(matches!(error, TransactionError::InvalidState { .. }));

    let owner = generation_owner(
        &root,
        &relative(&root, "document.md"),
        &root,
        "x".repeat(600),
        "svg",
    );
    let artifacts = (1..=1_000)
        .map(|index| owner.namespace().unwrap().target(index).unwrap())
        .collect::<Vec<_>>();
    let manifest = GenerationManifest::new(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        owner.clone(),
        None,
        artifacts,
    )
    .unwrap();
    let error = manifest.encode().unwrap_err();
    assert!(matches!(error, TransactionError::InvalidState { .. }));

    let mut oversized_count: serde_json::Value = serde_json::from_slice(
        &GenerationManifest::new("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", owner, None, Vec::new())
            .unwrap()
            .encode()
            .unwrap(),
    )
    .unwrap();
    oversized_count["artifacts"] = serde_json::json!(vec![vec!["61"]; 65_536]);
    let bytes = serde_json::to_vec(&oversized_count).unwrap();
    assert!(bytes.len() as u64 <= MAX_STATE_BYTES);
    let error = GenerationManifest::decode_bounded(&bytes, &root).unwrap_err();
    assert!(matches!(error, TransactionError::InvalidState { .. }));
}

#[test]
fn generation_manifest_accepts_only_its_exact_contiguous_namespace() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let document = relative(&root, "document.md");
    let owner = generation_owner(&root, &document, &root, "diagram", "svg");
    let cases = [
        vec![owner.namespace().unwrap().target(2).unwrap()],
        vec![
            owner.namespace().unwrap().target(1).unwrap(),
            owner.namespace().unwrap().target(3).unwrap(),
        ],
        vec![relative(&root, "diagram-01.svg")],
        vec![relative(&root, "other-1.svg")],
    ];

    for artifacts in cases {
        let error = GenerationManifest::new(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            owner.clone(),
            Some(document.clone()),
            artifacts,
        )
        .unwrap_err();
        assert!(matches!(error, TransactionError::InvalidState { .. }));
    }
}

#[test]
fn successful_commit_is_deterministic_and_publishes_document_last() {
    let temp = tempfile::tempdir().unwrap();
    seed_standard_old_generation(temp.path());
    let events = Arc::new(std::sync::Mutex::new(Vec::new()));
    let captured = Arc::clone(&events);
    let hook = CheckpointHook::new(move |checkpoint| {
        if let Checkpoint::CommitBefore { index } = checkpoint {
            captured.lock().unwrap().push(index);
        }
        Ok(())
    });
    let targets = Targets::under(temp.path());
    let mut staging = LockedRecoveredRoot::acquire_with_checkpoint(temp.path(), hook)
        .unwrap()
        .begin(targets.standard_plan())
        .unwrap();
    stage_standard_new_generation(&mut staging, &targets);
    staging.ready().unwrap().commit().unwrap();

    assert_eq!(*events.lock().unwrap(), vec![0, 1, 2, 3]);
    assert_eq!(read(temp.path().join("a.svg")), b"new-a");
    assert_eq!(read(temp.path().join("z.svg")), b"new-z");
    assert_eq!(
        read(temp.path().join(".merman-manifest.json")),
        b"new-manifest"
    );
    assert_eq!(read(temp.path().join("document.md")), b"new-document");
    assert!(!temp.path().join(TRANSACTION_DIR_NAME).exists());
    assert!(temp.path().join(LOCK_FILE_NAME).is_file());
}

#[test]
fn target_created_after_missing_generation_approval_is_rejected_at_begin() {
    let temp = tempfile::tempdir().unwrap();
    let manifest_path = temp.path().join(".merman-manifest.json");
    let manifest = relative(temp.path(), ".merman-manifest.json");
    let plan =
        TransactionPlan::new([
            TransactionEntryPlan::write(TransactionRole::Manifest, manifest)
                .expect_generation(TargetGeneration::Missing),
        ])
        .unwrap();

    std::fs::write(&manifest_path, b"concurrent generation").unwrap();
    let error = match LockedRecoveredRoot::acquire(temp.path())
        .unwrap()
        .begin(plan)
    {
        Ok(_) => panic!("a target created after approval must be rejected at begin"),
        Err(error) => error,
    };

    assert!(matches!(error, TransactionError::InvalidState { .. }));
    assert_eq!(read(&manifest_path), b"concurrent generation");
    drop(LockedRecoveredRoot::acquire(temp.path()).unwrap());
    assert_eq!(read(&manifest_path), b"concurrent generation");
}

#[test]
fn target_created_after_missing_generation_approval_is_rejected_before_ready() {
    let temp = tempfile::tempdir().unwrap();
    let manifest_path = temp.path().join(".merman-manifest.json");
    let manifest = relative(temp.path(), ".merman-manifest.json");
    let mut staging = LockedRecoveredRoot::acquire(temp.path())
        .unwrap()
        .begin(
            TransactionPlan::new([TransactionEntryPlan::write(
                TransactionRole::Manifest,
                manifest.clone(),
            )
            .expect_generation(TargetGeneration::Missing)])
            .unwrap(),
        )
        .unwrap();
    staging.stage_bytes(&manifest, b"our generation").unwrap();

    std::fs::write(&manifest_path, b"concurrent generation").unwrap();
    let error = match staging.ready() {
        Ok(_) => panic!("a target created after approval must not become the prior generation"),
        Err(error) => error,
    };

    assert!(matches!(error, TransactionError::InvalidState { .. }));
    assert_eq!(read(&manifest_path), b"concurrent generation");
    drop(LockedRecoveredRoot::acquire(temp.path()).unwrap());
    assert_eq!(read(&manifest_path), b"concurrent generation");
}

#[test]
fn target_replaced_after_existing_generation_approval_is_rejected_before_ready() {
    let temp = tempfile::tempdir().unwrap();
    let manifest_path = temp.path().join(".merman-manifest.json");
    std::fs::write(&manifest_path, b"approved generation").unwrap();
    let approved_generation = existing_generation(&manifest_path);
    let manifest = relative(temp.path(), ".merman-manifest.json");
    let mut staging = LockedRecoveredRoot::acquire(temp.path())
        .unwrap()
        .begin(
            TransactionPlan::new([TransactionEntryPlan::write(
                TransactionRole::Manifest,
                manifest.clone(),
            )
            .expect_generation(approved_generation)])
            .unwrap(),
        )
        .unwrap();
    staging.stage_bytes(&manifest, b"our generation").unwrap();

    std::fs::remove_file(&manifest_path).unwrap();
    std::fs::write(&manifest_path, b"approved generation").unwrap();
    let error = match staging.ready() {
        Ok(_) => panic!("a replacement target must not become the prior generation"),
        Err(error) => error,
    };

    assert!(matches!(error, TransactionError::InvalidState { .. }));
    assert_eq!(read(&manifest_path), b"approved generation");
    drop(LockedRecoveredRoot::acquire(temp.path()).unwrap());
    assert_eq!(read(&manifest_path), b"approved generation");
}

#[cfg(feature = "rustdoc")]
#[test]
fn same_inode_content_change_after_approval_is_rejected_before_backup() {
    let temp = tempfile::tempdir().unwrap();
    let manifest_path = temp.path().join(".merman-manifest.json");
    std::fs::write(&manifest_path, b"approved generation").unwrap();
    let approved_generation = content_pinned_generation(&manifest_path);
    let manifest = relative(temp.path(), ".merman-manifest.json");
    let mut staging = LockedRecoveredRoot::acquire(temp.path())
        .unwrap()
        .begin(
            TransactionPlan::new([TransactionEntryPlan::write(
                TransactionRole::Manifest,
                manifest.clone(),
            )
            .expect_generation(approved_generation)])
            .unwrap(),
        )
        .unwrap();
    staging.stage_bytes(&manifest, b"our generation").unwrap();

    std::fs::write(&manifest_path, b"concurrent generation").unwrap();
    let error = match staging.ready() {
        Ok(_) => panic!("same-inode content replacement must be rejected before backup"),
        Err(error) => error,
    };

    assert!(matches!(error, TransactionError::InvalidState { .. }));
    assert_eq!(read(&manifest_path), b"concurrent generation");
    drop(LockedRecoveredRoot::acquire(temp.path()).unwrap());
    assert_eq!(read(&manifest_path), b"concurrent generation");
}

#[cfg(feature = "rustdoc")]
#[test]
fn content_pinned_backup_never_copies_beyond_the_approved_length() {
    use std::io::Write as _;

    let temp = tempfile::tempdir().unwrap();
    let manifest_path = temp.path().join(".merman-manifest.json");
    let approved = b"approved generation";
    std::fs::write(&manifest_path, approved).unwrap();
    let approved_generation = content_pinned_generation(&manifest_path);
    let manifest = relative(temp.path(), ".merman-manifest.json");
    let mut staging = LockedRecoveredRoot::acquire(temp.path())
        .unwrap()
        .begin(
            TransactionPlan::new([TransactionEntryPlan::write(
                TransactionRole::Manifest,
                manifest.clone(),
            )
            .expect_generation(approved_generation)])
            .unwrap(),
        )
        .unwrap();
    staging.stage_bytes(&manifest, b"our generation").unwrap();

    let mut target = std::fs::OpenOptions::new()
        .append(true)
        .open(&manifest_path)
        .unwrap();
    target.write_all(&vec![b'x'; 1024 * 1024]).unwrap();
    target.sync_all().unwrap();

    let error = match staging.ready() {
        Ok(_) => panic!("same-inode append must invalidate approved content"),
        Err(error) => error,
    };

    assert!(matches!(error, TransactionError::InvalidState { .. }));
    let backup = temp.path().join(TRANSACTION_DIR_NAME).join("old-00000000");
    assert!(
        std::fs::metadata(&backup).unwrap().len() <= approved.len() as u64,
        "content-pinned backup copied bytes beyond the approved generation"
    );
    drop(LockedRecoveredRoot::acquire(temp.path()).unwrap());
    assert_eq!(
        std::fs::metadata(&manifest_path).unwrap().len(),
        approved.len() as u64 + 1024 * 1024
    );
}

#[cfg(unix)]
#[test]
fn rename_replacement_at_commit_boundary_is_not_published_over() {
    let temp = tempfile::tempdir().unwrap();
    let manifest_path = temp.path().join(".merman-manifest.json");
    let replacement = temp.path().join("replacement");
    std::fs::write(&manifest_path, b"approved generation").unwrap();
    std::fs::write(&replacement, b"approved generation").unwrap();
    let approved_generation = existing_generation(&manifest_path);
    let replacement_identity = same_file::Handle::from_path(&replacement).unwrap();
    let hook_target = manifest_path.clone();
    let hook_replacement = replacement.clone();
    let replaced = Arc::new(AtomicBool::new(false));
    let hook_replaced = Arc::clone(&replaced);
    let hook = CheckpointHook::new(move |checkpoint| {
        if checkpoint == (Checkpoint::ReplaceBefore { index: 0 })
            && !hook_replaced.swap(true, AtomicOrdering::SeqCst)
        {
            std::fs::rename(&hook_replacement, &hook_target)?;
        }
        Ok(())
    });
    let manifest = relative(temp.path(), ".merman-manifest.json");
    let mut staging = LockedRecoveredRoot::acquire_with_checkpoint(temp.path(), hook)
        .unwrap()
        .begin(
            TransactionPlan::new([TransactionEntryPlan::write(
                TransactionRole::Manifest,
                manifest.clone(),
            )
            .expect_generation(approved_generation)])
            .unwrap(),
        )
        .unwrap();
    staging.stage_bytes(&manifest, b"our generation").unwrap();

    let error = staging.ready().unwrap().commit().unwrap_err();

    assert!(matches!(error, TransactionError::CommitRolledBack { .. }));
    assert_eq!(read(&manifest_path), b"approved generation");
    assert_eq!(
        same_file::Handle::from_path(&manifest_path).unwrap(),
        replacement_identity
    );
}

#[cfg(unix)]
#[test]
fn unlink_then_replace_at_delete_boundary_does_not_delete_the_replacement() {
    let temp = tempfile::tempdir().unwrap();
    let artifact_path = temp.path().join("stale.svg");
    let replacement = temp.path().join("replacement");
    std::fs::write(&artifact_path, b"approved generation").unwrap();
    std::fs::write(&replacement, b"approved generation").unwrap();
    std::fs::write(temp.path().join(".merman-manifest.json"), b"old manifest").unwrap();
    let approved_generation = existing_generation(&artifact_path);
    let replacement_identity = same_file::Handle::from_path(&replacement).unwrap();
    let hook_target = artifact_path.clone();
    let hook_replacement = replacement.clone();
    let replaced = Arc::new(AtomicBool::new(false));
    let hook_replaced = Arc::clone(&replaced);
    let hook = CheckpointHook::new(move |checkpoint| {
        if checkpoint == (Checkpoint::DeleteBefore { index: 0 })
            && !hook_replaced.swap(true, AtomicOrdering::SeqCst)
        {
            std::fs::remove_file(&hook_target)?;
            std::fs::rename(&hook_replacement, &hook_target)?;
        }
        Ok(())
    });
    let artifact = relative(temp.path(), "stale.svg");
    let manifest = relative(temp.path(), ".merman-manifest.json");
    let mut staging = LockedRecoveredRoot::acquire_with_checkpoint(temp.path(), hook)
        .unwrap()
        .begin(
            TransactionPlan::new([
                TransactionEntryPlan::delete_artifact(artifact)
                    .expect_generation(approved_generation),
                TransactionEntryPlan::write(TransactionRole::Manifest, manifest.clone()),
            ])
            .unwrap(),
        )
        .unwrap();
    staging.stage_bytes(&manifest, b"new manifest").unwrap();

    let error = staging.ready().unwrap().commit().unwrap_err();

    assert!(matches!(error, TransactionError::CommitRolledBack { .. }));
    assert_eq!(read(&artifact_path), b"approved generation");
    assert_eq!(
        same_file::Handle::from_path(&artifact_path).unwrap(),
        replacement_identity
    );
}

#[cfg(unix)]
#[test]
fn publication_preserves_existing_modes_and_uses_the_process_umask_for_new_targets() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();
    let manifest_path = temp.path().join(".merman-manifest.json");
    std::fs::write(&manifest_path, b"old").unwrap();
    std::fs::set_permissions(&manifest_path, std::fs::Permissions::from_mode(0o640)).unwrap();
    let manifest = relative(temp.path(), ".merman-manifest.json");
    let mut staging = LockedRecoveredRoot::acquire(temp.path())
        .unwrap()
        .begin(
            TransactionPlan::new([TransactionEntryPlan::write(
                TransactionRole::Manifest,
                manifest.clone(),
            )])
            .unwrap(),
        )
        .unwrap();
    staging.stage_bytes(&manifest, b"new").unwrap();
    staging.ready().unwrap().commit().unwrap();
    assert_eq!(ordinary_mode(&manifest_path), 0o640);

    let temp = tempfile::tempdir().unwrap();
    let probe = temp.path().join("umask-probe");
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .unwrap();
    let expected_mode = ordinary_mode(&probe);
    std::fs::remove_file(&probe).unwrap();
    let manifest_path = temp.path().join(".merman-manifest.json");
    let manifest = relative(temp.path(), ".merman-manifest.json");
    let mut staging = LockedRecoveredRoot::acquire(temp.path())
        .unwrap()
        .begin(
            TransactionPlan::new([TransactionEntryPlan::write(
                TransactionRole::Manifest,
                manifest.clone(),
            )])
            .unwrap(),
        )
        .unwrap();
    staging.stage_bytes(&manifest, b"new").unwrap();
    staging.ready().unwrap().commit().unwrap();
    assert_eq!(ordinary_mode(&manifest_path), expected_mode);
}

#[cfg(unix)]
#[test]
fn recovery_restores_the_mode_of_a_deleted_artifact() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();
    let artifact_path = temp.path().join("stale.svg");
    std::fs::write(&artifact_path, b"owned-stale").unwrap();
    std::fs::set_permissions(&artifact_path, std::fs::Permissions::from_mode(0o640)).unwrap();
    std::fs::write(temp.path().join(".merman-manifest.json"), b"old-manifest").unwrap();
    let failures = Arc::new(AtomicU8::new(0));
    let hook_failures = Arc::clone(&failures);
    let hook = CheckpointHook::new(move |checkpoint| {
        let bit = match checkpoint {
            Checkpoint::CommitAfter { index: 0 } => 1,
            Checkpoint::RollbackBefore { index: 0 } => 2,
            _ => 0,
        };
        if bit != 0 && hook_failures.fetch_or(bit, AtomicOrdering::SeqCst) & bit == 0 {
            return Err(std::io::Error::other("interrupt deletion rollback"));
        }
        Ok(())
    });
    let artifact = relative(temp.path(), "stale.svg");
    let manifest = relative(temp.path(), ".merman-manifest.json");
    let mut staging = LockedRecoveredRoot::acquire_with_checkpoint(temp.path(), hook)
        .unwrap()
        .begin(
            TransactionPlan::new([
                TransactionEntryPlan::delete_artifact(artifact),
                TransactionEntryPlan::write(TransactionRole::Manifest, manifest.clone()),
            ])
            .unwrap(),
        )
        .unwrap();
    staging.stage_bytes(&manifest, b"new-manifest").unwrap();
    assert!(
        staging
            .ready()
            .unwrap()
            .commit()
            .unwrap_err()
            .is_partial_publication()
    );
    assert!(!artifact_path.exists());

    drop(LockedRecoveredRoot::acquire(temp.path()).unwrap());
    assert_eq!(read(&artifact_path), b"owned-stale");
    assert_eq!(ordinary_mode(&artifact_path), 0o640);
}

#[cfg(unix)]
#[test]
fn recovery_restores_mode_when_old_bytes_survive_with_changed_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();
    let manifest_path = temp.path().join(".merman-manifest.json");
    std::fs::write(&manifest_path, b"old-manifest").unwrap();
    std::fs::set_permissions(&manifest_path, std::fs::Permissions::from_mode(0o640)).unwrap();
    let manifest = relative(temp.path(), ".merman-manifest.json");
    let mut staging = LockedRecoveredRoot::acquire(temp.path())
        .unwrap()
        .begin(
            TransactionPlan::new([TransactionEntryPlan::write(
                TransactionRole::Manifest,
                manifest.clone(),
            )])
            .unwrap(),
        )
        .unwrap();
    staging.stage_bytes(&manifest, b"new-manifest").unwrap();
    let ready = staging.ready().unwrap();
    std::fs::set_permissions(&manifest_path, std::fs::Permissions::from_mode(0o000)).unwrap();
    drop(ready);

    drop(LockedRecoveredRoot::acquire(temp.path()).unwrap());
    assert_eq!(read(&manifest_path), b"old-manifest");
    assert_eq!(ordinary_mode(&manifest_path), 0o640);
}

#[test]
fn zero_artifact_generations_commit_manifest_and_optional_document() {
    for with_document in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let manifest = relative(temp.path(), ".merman-manifest.json");
        let document = relative(temp.path(), "document.md");
        std::fs::write(temp.path().join(".merman-manifest.json"), b"old-manifest").unwrap();
        if with_document {
            std::fs::write(temp.path().join("document.md"), b"old-document").unwrap();
        }
        let mut entries = vec![TransactionEntryPlan::write(
            TransactionRole::Manifest,
            manifest.clone(),
        )];
        if with_document {
            entries.push(TransactionEntryPlan::write(
                TransactionRole::Document,
                document.clone(),
            ));
        }
        let mut staging = LockedRecoveredRoot::acquire(temp.path())
            .unwrap()
            .begin(TransactionPlan::new(entries).unwrap())
            .unwrap();
        staging.stage_bytes(&manifest, b"new-manifest").unwrap();
        if with_document {
            staging.stage_bytes(&document, b"new-document").unwrap();
        }
        staging.ready().unwrap().commit().unwrap();
        assert_eq!(
            read(temp.path().join(".merman-manifest.json")),
            b"new-manifest"
        );
        if with_document {
            assert_eq!(read(temp.path().join("document.md")), b"new-document");
        }
    }
}

#[test]
fn independent_stage_slots_are_send_and_write_without_a_shared_mutex() {
    fn assert_send<T: Send>() {}
    assert_send::<StageSlot>();

    let temp = tempfile::tempdir().unwrap();
    seed_standard_old_generation(temp.path());
    let targets = Targets::under(temp.path());
    let mut staging = LockedRecoveredRoot::acquire(temp.path())
        .unwrap()
        .begin(targets.standard_plan())
        .unwrap();
    let slots = [
        (
            staging.stage_slot(&targets.artifact_a).unwrap(),
            b"new-a".as_slice(),
        ),
        (
            staging.stage_slot(&targets.artifact_z).unwrap(),
            b"new-z".as_slice(),
        ),
        (
            staging.stage_slot(&targets.manifest).unwrap(),
            b"new-manifest".as_slice(),
        ),
        (
            staging.stage_slot(&targets.document).unwrap(),
            b"new-document".as_slice(),
        ),
    ];
    std::thread::scope(|scope| {
        for (slot, bytes) in slots {
            scope.spawn(move || slot.write_bytes(bytes).unwrap());
        }
    });
    staging.ready().unwrap().commit().unwrap();
    assert_eq!(read(temp.path().join("a.svg")), b"new-a");
    assert_eq!(read(temp.path().join("document.md")), b"new-document");
}

#[test]
fn stage_slot_rejects_targets_outside_the_plan() {
    let temp = tempfile::tempdir().unwrap();
    let manifest = relative(temp.path(), ".merman-manifest.json");
    let unknown = relative(temp.path(), "unknown.svg");
    let mut staging = LockedRecoveredRoot::acquire(temp.path())
        .unwrap()
        .begin(
            TransactionPlan::new([TransactionEntryPlan::write(
                TransactionRole::Manifest,
                manifest.clone(),
            )])
            .unwrap(),
        )
        .unwrap();

    let error = staging.stage_slot(&unknown).err().unwrap();
    assert!(matches!(&error, TransactionError::InvalidState { .. }));
    assert!(error.to_string().contains("outside the transaction plan"));

    staging.stage_bytes(&manifest, b"new").unwrap();
    staging.ready().unwrap().commit().unwrap();
}

#[test]
fn a_stage_slot_can_only_be_issued_once() {
    let temp = tempfile::tempdir().unwrap();
    let manifest = relative(temp.path(), ".merman-manifest.json");
    let mut staging = LockedRecoveredRoot::acquire(temp.path())
        .unwrap()
        .begin(
            TransactionPlan::new([TransactionEntryPlan::write(
                TransactionRole::Manifest,
                manifest.clone(),
            )])
            .unwrap(),
        )
        .unwrap();
    let slot = staging.stage_slot(&manifest).unwrap();
    let error = staging.stage_slot(&manifest).err().unwrap();
    assert!(matches!(error, TransactionError::InvalidState { .. }));
    slot.write_bytes(b"new").unwrap();
    staging.ready().unwrap().commit().unwrap();
}

#[test]
fn an_unfinished_stage_slot_fails_ready_and_keeps_the_lock_until_dropped() {
    let temp = tempfile::tempdir().unwrap();
    let manifest = relative(temp.path(), ".merman-manifest.json");
    let mut staging = LockedRecoveredRoot::acquire(temp.path())
        .unwrap()
        .begin(
            TransactionPlan::new([TransactionEntryPlan::write(
                TransactionRole::Manifest,
                manifest.clone(),
            )])
            .unwrap(),
        )
        .unwrap();
    let slot = staging.stage_slot(&manifest).unwrap();
    assert!(staging.ready().is_err());
    assert!(
        LockedRecoveredRoot::acquire(temp.path())
            .unwrap_err()
            .is_contention()
    );
    drop(slot);
    drop(LockedRecoveredRoot::acquire(temp.path()).unwrap());
    assert!(!temp.path().join(TRANSACTION_DIR_NAME).exists());
}

#[test]
fn artifact_delete_is_backed_up_and_committed_as_an_owned_stale_entry() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("stale.svg"), b"owned-stale").unwrap();
    std::fs::write(temp.path().join(".merman-manifest.json"), b"old-manifest").unwrap();
    std::fs::write(temp.path().join("document.md"), b"old-document").unwrap();
    let stale = relative(temp.path(), "stale.svg");
    let manifest = relative(temp.path(), ".merman-manifest.json");
    let document = relative(temp.path(), "document.md");
    let plan = TransactionPlan::new([
        TransactionEntryPlan::delete_artifact(stale),
        TransactionEntryPlan::write(TransactionRole::Manifest, manifest.clone()),
        TransactionEntryPlan::write(TransactionRole::Document, document.clone()),
    ])
    .unwrap();
    let mut staging = LockedRecoveredRoot::acquire(temp.path())
        .unwrap()
        .begin(plan)
        .unwrap();
    staging.stage_bytes(&manifest, b"new-manifest").unwrap();
    staging.stage_bytes(&document, b"new-document").unwrap();
    staging.ready().unwrap().commit().unwrap();
    assert!(!temp.path().join("stale.svg").exists());
    assert_eq!(read(temp.path().join("document.md")), b"new-document");
}

#[test]
fn lock_is_nonblocking_persistent_and_keeps_one_identity() {
    let temp = tempfile::tempdir().unwrap();
    let first = LockedRecoveredRoot::acquire(temp.path()).unwrap();
    let lock = temp.path().join(LOCK_FILE_NAME);
    let first_identity = same_file::Handle::from_path(&lock).unwrap();
    let error = LockedRecoveredRoot::acquire(temp.path()).unwrap_err();
    assert!(error.is_contention());
    let canonical_lock = lock.canonicalize().unwrap();
    assert_eq!(error.evidence_path(), Some(canonical_lock.as_path()));
    drop(first);

    let second = LockedRecoveredRoot::acquire(temp.path()).unwrap();
    assert_eq!(same_file::Handle::from_path(&lock).unwrap(), first_identity);
    drop(second);
    assert!(lock.is_file());
    assert!(!temp.path().join(TRANSACTION_DIR_NAME).exists());
}

#[cfg(unix)]
#[test]
fn lock_rejects_static_symlinks() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let external = tempfile::NamedTempFile::new().unwrap();
    symlink(external.path(), temp.path().join(LOCK_FILE_NAME)).unwrap();
    let error = LockedRecoveredRoot::acquire(temp.path()).unwrap_err();
    assert!(matches!(error, TransactionError::InvalidState { .. }));
}

#[test]
fn lock_rejects_non_regular_objects() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir(temp.path().join(LOCK_FILE_NAME)).unwrap();
    let error = LockedRecoveredRoot::acquire(temp.path()).unwrap_err();
    assert!(matches!(error, TransactionError::InvalidState { .. }));
}

#[cfg(unix)]
#[test]
fn targets_cannot_alias_the_lock_or_each_other_through_hard_links() {
    let temp = tempfile::tempdir().unwrap();
    let locked = LockedRecoveredRoot::acquire(temp.path()).unwrap();
    std::fs::hard_link(
        temp.path().join(LOCK_FILE_NAME),
        temp.path().join("lock-alias.svg"),
    )
    .unwrap();
    let lock_alias = relative(temp.path(), "lock-alias.svg");
    let manifest = relative(temp.path(), ".merman-manifest.json");
    let error = locked
        .begin(
            TransactionPlan::new([
                TransactionEntryPlan::write(TransactionRole::Artifact, lock_alias),
                TransactionEntryPlan::write(TransactionRole::Manifest, manifest),
            ])
            .unwrap(),
        )
        .err()
        .unwrap();
    assert!(matches!(error, TransactionError::InvalidState { .. }));

    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("a.svg"), b"same inode").unwrap();
    std::fs::hard_link(temp.path().join("a.svg"), temp.path().join("b.svg")).unwrap();
    let a = relative(temp.path(), "a.svg");
    let b = relative(temp.path(), "b.svg");
    let manifest = relative(temp.path(), ".merman-manifest.json");
    let error = LockedRecoveredRoot::acquire(temp.path())
        .unwrap()
        .begin(
            TransactionPlan::new([
                TransactionEntryPlan::write(TransactionRole::Artifact, a),
                TransactionEntryPlan::write(TransactionRole::Artifact, b),
                TransactionEntryPlan::write(TransactionRole::Manifest, manifest),
            ])
            .unwrap(),
        )
        .err()
        .unwrap();
    assert!(matches!(error, TransactionError::InvalidState { .. }));
}

#[test]
fn staging_write_failure_leaves_no_final_mutation_and_is_recoverable() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join(".merman-manifest.json"), b"old").unwrap();
    let failed = Arc::new(AtomicBool::new(false));
    let hook_failed = Arc::clone(&failed);
    let hook = CheckpointHook::new(move |checkpoint| {
        if matches!(checkpoint, Checkpoint::StageBefore { .. })
            && !hook_failed.swap(true, AtomicOrdering::SeqCst)
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::StorageFull,
                "injected ENOSPC",
            ));
        }
        Ok(())
    });
    let manifest = relative(temp.path(), ".merman-manifest.json");
    let mut staging = LockedRecoveredRoot::acquire_with_checkpoint(temp.path(), hook)
        .unwrap()
        .begin(
            TransactionPlan::new([TransactionEntryPlan::write(
                TransactionRole::Manifest,
                manifest.clone(),
            )])
            .unwrap(),
        )
        .unwrap();
    assert!(staging.stage_bytes(&manifest, b"new").is_err());
    drop(staging);
    assert_eq!(read(temp.path().join(".merman-manifest.json")), b"old");
    drop(LockedRecoveredRoot::acquire(temp.path()).unwrap());
    assert!(!temp.path().join(TRANSACTION_DIR_NAME).exists());
}

#[test]
fn failures_before_and_after_every_commit_action_restore_the_old_generation() {
    for index in 0..4 {
        for after in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            seed_standard_old_generation(temp.path());
            let failed = Arc::new(AtomicBool::new(false));
            let hook_failed = Arc::clone(&failed);
            let hook = CheckpointHook::new(move |checkpoint| {
                let selected = if after {
                    checkpoint == (Checkpoint::CommitAfter { index })
                } else {
                    checkpoint == (Checkpoint::CommitBefore { index })
                };
                if selected && !hook_failed.swap(true, AtomicOrdering::SeqCst) {
                    return Err(std::io::Error::other("injected commit failure"));
                }
                Ok(())
            });
            let targets = Targets::under(temp.path());
            let mut staging = LockedRecoveredRoot::acquire_with_checkpoint(temp.path(), hook)
                .unwrap()
                .begin(targets.standard_plan())
                .unwrap();
            stage_standard_new_generation(&mut staging, &targets);
            let error = staging.ready().unwrap().commit().unwrap_err();
            assert!(
                matches!(error, TransactionError::CommitRolledBack { .. }),
                "index={index} after={after}: {error}"
            );
            assert_standard_old_generation(temp.path());
            assert!(!temp.path().join(TRANSACTION_DIR_NAME).exists());
        }
    }
}

#[test]
fn commit_point_precondition_failure_restores_every_published_target() {
    let temp = tempfile::tempdir().unwrap();
    seed_standard_old_generation(temp.path());
    let source = temp.path().join("source.md");
    std::fs::write(&source, b"approved source").unwrap();
    let changed_source = source.clone();
    let hook = CheckpointHook::new(move |checkpoint| {
        if checkpoint == (Checkpoint::CommitBefore { index: 3 }) {
            std::fs::write(&changed_source, b"concurrent source").unwrap();
        }
        Ok(())
    });
    let targets = Targets::under(temp.path());
    let mut staging = LockedRecoveredRoot::acquire_with_checkpoint(temp.path(), hook)
        .unwrap()
        .begin(targets.standard_plan())
        .unwrap();
    stage_standard_new_generation(&mut staging, &targets);
    let mut validate = || {
        (read(&source) == b"approved source")
            .then_some(())
            .ok_or_else(|| "declared input changed before the commit point".to_string())
    };

    let error = staging
        .ready()
        .unwrap()
        .commit_with_precommit_validation(&mut validate)
        .unwrap_err();

    assert!(matches!(error, TransactionError::CommitRolledBack { .. }));
    assert_standard_old_generation(temp.path());
    assert_eq!(read(&source), b"concurrent source");
    assert!(!temp.path().join(TRANSACTION_DIR_NAME).exists());
}

#[test]
fn abrupt_termination_at_every_commit_boundary_recovers_by_observed_generation() {
    for index in 0..4 {
        for after in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            seed_standard_old_generation(temp.path());
            let armed = Arc::new(AtomicBool::new(false));
            let hook_armed = Arc::clone(&armed);
            let hook = CheckpointHook::new(move |checkpoint| {
                let selected = if after {
                    checkpoint == (Checkpoint::CommitAfter { index })
                } else {
                    checkpoint == (Checkpoint::CommitBefore { index })
                };
                assert!(
                    !(selected && hook_armed.load(AtomicOrdering::SeqCst)),
                    "injected abrupt termination"
                );
                Ok(())
            });
            let targets = Targets::under(temp.path());
            let mut staging = LockedRecoveredRoot::acquire_with_checkpoint(temp.path(), hook)
                .unwrap()
                .begin(targets.standard_plan())
                .unwrap();
            stage_standard_new_generation(&mut staging, &targets);
            let ready = staging.ready().unwrap();
            armed.store(true, AtomicOrdering::SeqCst);
            let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ready.commit();
            }));
            assert!(unwind.is_err(), "index={index} after={after}");
            assert!(temp.path().join(TRANSACTION_DIR_NAME).is_dir());

            drop(LockedRecoveredRoot::acquire(temp.path()).unwrap());
            if index == 3 && after {
                assert_eq!(read(temp.path().join("a.svg")), b"new-a");
                assert_eq!(read(temp.path().join("z.svg")), b"new-z");
                assert_eq!(
                    read(temp.path().join(".merman-manifest.json")),
                    b"new-manifest"
                );
                assert_eq!(read(temp.path().join("document.md")), b"new-document");
            } else {
                assert_standard_old_generation(temp.path());
            }
            assert!(!temp.path().join(TRANSACTION_DIR_NAME).exists());
        }
    }
}

#[test]
fn an_ambiguous_commit_point_rolls_back_an_incomplete_generation() {
    let temp = tempfile::tempdir().unwrap();
    seed_standard_old_generation(temp.path());
    let armed = Arc::new(AtomicBool::new(false));
    let hook_armed = Arc::clone(&armed);
    let hook = CheckpointHook::new(move |checkpoint| {
        assert!(
            !(checkpoint == (Checkpoint::CommitAfter { index: 0 })
                && hook_armed.load(AtomicOrdering::SeqCst)),
            "leave an incomplete generation with an old-and-new commit point"
        );
        Ok(())
    });
    let targets = Targets::under(temp.path());
    let mut staging = LockedRecoveredRoot::acquire_with_checkpoint(temp.path(), hook)
        .unwrap()
        .begin(targets.standard_plan())
        .unwrap();
    staging.stage_bytes(&targets.artifact_a, b"new-a").unwrap();
    staging.stage_bytes(&targets.artifact_z, b"new-z").unwrap();
    staging
        .stage_bytes(&targets.manifest, b"new-manifest")
        .unwrap();
    staging
        .stage_bytes(&targets.document, b"old-document")
        .unwrap();
    let ready = staging.ready().unwrap();
    armed.store(true, AtomicOrdering::SeqCst);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ready.commit())).is_err());

    drop(LockedRecoveredRoot::acquire(temp.path()).unwrap());
    assert_standard_old_generation(temp.path());
}

#[test]
fn a_generation_whose_old_and_new_bytes_are_identical_can_commit() {
    let temp = tempfile::tempdir().unwrap();
    let manifest_path = temp.path().join(".merman-manifest.json");
    std::fs::write(&manifest_path, b"same").unwrap();
    let identity = same_file::Handle::from_path(&manifest_path).unwrap();
    let modified = std::fs::metadata(&manifest_path)
        .unwrap()
        .modified()
        .unwrap();
    let manifest = relative(temp.path(), ".merman-manifest.json");
    let mut staging = LockedRecoveredRoot::acquire(temp.path())
        .unwrap()
        .begin(
            TransactionPlan::new([TransactionEntryPlan::write(
                TransactionRole::Manifest,
                manifest.clone(),
            )])
            .unwrap(),
        )
        .unwrap();
    staging.stage_bytes(&manifest, b"same").unwrap();
    staging.ready().unwrap().commit().unwrap();
    assert_eq!(read(&manifest_path), b"same");
    assert_eq!(
        same_file::Handle::from_path(&manifest_path).unwrap(),
        identity,
        "an identical transaction write must preserve the target identity"
    );
    assert_eq!(
        std::fs::metadata(&manifest_path)
            .unwrap()
            .modified()
            .unwrap(),
        modified,
        "an identical transaction write must preserve the target mtime"
    );
    assert!(!temp.path().join(TRANSACTION_DIR_NAME).exists());
}

#[test]
fn final_generation_is_rechecked_before_the_committed_marker() {
    let temp = tempfile::tempdir().unwrap();
    seed_standard_old_generation(temp.path());
    let artifact = temp.path().join("a.svg");
    let tampered = Arc::new(AtomicBool::new(false));
    let hook_tampered = Arc::clone(&tampered);
    let hook = CheckpointHook::new(move |checkpoint| {
        if checkpoint == (Checkpoint::CommitAfter { index: 3 })
            && !hook_tampered.swap(true, AtomicOrdering::SeqCst)
        {
            std::fs::write(&artifact, b"old-a")?;
        }
        Ok(())
    });
    let targets = Targets::under(temp.path());
    let mut staging = LockedRecoveredRoot::acquire_with_checkpoint(temp.path(), hook)
        .unwrap()
        .begin(targets.standard_plan())
        .unwrap();
    stage_standard_new_generation(&mut staging, &targets);
    let error = staging.ready().unwrap().commit().unwrap_err();
    assert!(matches!(error, TransactionError::CommitRolledBack { .. }));
    assert_standard_old_generation(temp.path());
    assert!(!temp.path().join(TRANSACTION_DIR_NAME).exists());
}

#[test]
fn rollback_failure_retains_evidence_and_next_acquire_recovers() {
    let temp = tempfile::tempdir().unwrap();
    seed_standard_old_generation(temp.path());
    let failures = Arc::new(AtomicU8::new(0));
    let hook_failures = Arc::clone(&failures);
    let hook = CheckpointHook::new(move |checkpoint| {
        let bit = match checkpoint {
            Checkpoint::CommitAfter { index: 0 } => 1,
            Checkpoint::RollbackBefore { index: 0 } => 2,
            _ => 0,
        };
        if bit != 0 {
            let previous = hook_failures.fetch_or(bit, AtomicOrdering::SeqCst);
            if previous & bit == 0 {
                return Err(std::io::Error::other("injected publication failure"));
            }
        }
        Ok(())
    });
    let targets = Targets::under(temp.path());
    let mut staging = LockedRecoveredRoot::acquire_with_checkpoint(temp.path(), hook)
        .unwrap()
        .begin(targets.standard_plan())
        .unwrap();
    stage_standard_new_generation(&mut staging, &targets);
    let error = staging.ready().unwrap().commit().unwrap_err();
    assert!(error.is_partial_publication(), "{error}");
    assert!(temp.path().join(TRANSACTION_DIR_NAME).is_dir());
    assert_eq!(read(temp.path().join("a.svg")), b"new-a");

    drop(LockedRecoveredRoot::acquire(temp.path()).unwrap());
    assert_standard_old_generation(temp.path());
    assert!(!temp.path().join(TRANSACTION_DIR_NAME).exists());
}

#[test]
fn acquire_recovers_staging_backing_up_publishing_and_committed_states() {
    recover_staging_state();
    recover_staging_from_a_truncated_pending_write();
    recover_publishing_state();
    recover_committed_state();
}

#[test]
fn committed_cleanup_is_idempotent_across_every_file_boundary() {
    for ordinal in 0..6 {
        for after in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            std::fs::write(temp.path().join(".merman-manifest.json"), b"old").unwrap();
            let failed = Arc::new(AtomicBool::new(false));
            let hook_failed = Arc::clone(&failed);
            let hook = CheckpointHook::new(move |checkpoint| {
                let selected = if after {
                    checkpoint == (Checkpoint::CleanupFileAfter { ordinal })
                } else {
                    checkpoint == (Checkpoint::CleanupFileBefore { ordinal })
                };
                if selected && !hook_failed.swap(true, AtomicOrdering::SeqCst) {
                    return Err(std::io::Error::other("interrupt committed cleanup"));
                }
                Ok(())
            });
            let manifest = relative(temp.path(), ".merman-manifest.json");
            let mut staging = LockedRecoveredRoot::acquire_with_checkpoint(temp.path(), hook)
                .unwrap()
                .begin(
                    TransactionPlan::new([TransactionEntryPlan::write(
                        TransactionRole::Manifest,
                        manifest.clone(),
                    )])
                    .unwrap(),
                )
                .unwrap();
            staging.stage_bytes(&manifest, b"new").unwrap();
            let error = staging.ready().unwrap().commit().unwrap_err();
            assert!(
                matches!(error, TransactionError::Recovery { .. }),
                "ordinal={ordinal} after={after}: {error}"
            );
            assert!(!error.is_partial_publication());

            drop(LockedRecoveredRoot::acquire(temp.path()).unwrap());
            assert_eq!(read(temp.path().join(".merman-manifest.json")), b"new");
            assert!(!temp.path().join(TRANSACTION_DIR_NAME).exists());
        }
    }
}

#[test]
fn rolled_back_cleanup_is_idempotent_across_every_file_boundary() {
    for ordinal in 0..6 {
        for after in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            std::fs::write(temp.path().join(".merman-manifest.json"), b"old").unwrap();
            let failures = Arc::new(AtomicU8::new(0));
            let hook_failures = Arc::clone(&failures);
            let hook = CheckpointHook::new(move |checkpoint| {
                if checkpoint == (Checkpoint::CommitAfter { index: 0 })
                    && hook_failures.fetch_or(1, AtomicOrdering::SeqCst) & 1 == 0
                {
                    return Err(std::io::Error::other("force rollback"));
                }
                let selected = if after {
                    checkpoint == (Checkpoint::CleanupFileAfter { ordinal })
                } else {
                    checkpoint == (Checkpoint::CleanupFileBefore { ordinal })
                };
                if selected && hook_failures.fetch_or(2, AtomicOrdering::SeqCst) & 2 == 0 {
                    return Err(std::io::Error::other("interrupt rolled-back cleanup"));
                }
                Ok(())
            });
            let manifest = relative(temp.path(), ".merman-manifest.json");
            let mut staging = LockedRecoveredRoot::acquire_with_checkpoint(temp.path(), hook)
                .unwrap()
                .begin(
                    TransactionPlan::new([TransactionEntryPlan::write(
                        TransactionRole::Manifest,
                        manifest.clone(),
                    )])
                    .unwrap(),
                )
                .unwrap();
            staging.stage_bytes(&manifest, b"new").unwrap();
            let error = staging.ready().unwrap().commit().unwrap_err();
            assert!(
                matches!(error, TransactionError::Recovery { .. }),
                "ordinal={ordinal} after={after}: {error}"
            );
            assert!(!error.is_partial_publication());

            drop(LockedRecoveredRoot::acquire(temp.path()).unwrap());
            assert_eq!(read(temp.path().join(".merman-manifest.json")), b"old");
            assert!(!temp.path().join(TRANSACTION_DIR_NAME).exists());
        }
    }
}

#[test]
fn malformed_unsupported_oversized_and_traversal_states_fail_without_mutation() {
    let cases = [
        ForgedState::Malformed,
        ForgedState::Unsupported,
        ForgedState::Oversized,
        ForgedState::Traversal,
        ForgedState::UnknownField,
        ForgedState::ModeWithoutPrior,
    ];
    for case in cases {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("sentinel"), b"unchanged").unwrap();
        let transaction = make_private_transaction_dir(temp.path());
        write_forged_state(&transaction.join(STATE_A_NAME), case);
        let error = LockedRecoveredRoot::acquire(temp.path()).unwrap_err();
        assert!(
            matches!(error, TransactionError::InvalidState { .. }),
            "{case:?}: {error}"
        );
        assert_eq!(read(temp.path().join("sentinel")), b"unchanged");
        assert!(transaction.exists());
    }
}

#[test]
fn semantically_invalid_second_slot_fails_closed_even_with_one_valid_slot() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join(".merman-manifest.json"), b"old").unwrap();
    let manifest = relative(temp.path(), ".merman-manifest.json");
    let staging = LockedRecoveredRoot::acquire(temp.path())
        .unwrap()
        .begin(
            TransactionPlan::new([TransactionEntryPlan::write(
                TransactionRole::Manifest,
                manifest,
            )])
            .unwrap(),
        )
        .unwrap();
    drop(staging);
    let transaction = temp.path().join(TRANSACTION_DIR_NAME);
    write_forged_state(&transaction.join(STATE_B_NAME), ForgedState::Unsupported);
    let error = LockedRecoveredRoot::acquire(temp.path()).unwrap_err();
    assert!(matches!(error, TransactionError::InvalidState { .. }));
    assert_eq!(read(temp.path().join(".merman-manifest.json")), b"old");
    assert!(transaction.exists());
}

#[test]
fn complete_pending_successor_is_promoted_before_recovery() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join(".merman-manifest.json"), b"old").unwrap();
    let armed = Arc::new(AtomicBool::new(false));
    let failed = Arc::new(AtomicBool::new(false));
    let hook_armed = Arc::clone(&armed);
    let hook_failed = Arc::clone(&failed);
    let hook = CheckpointHook::new(move |checkpoint| {
        if matches!(checkpoint, Checkpoint::PersistPrepared { .. })
            && hook_armed.load(AtomicOrdering::SeqCst)
            && !hook_failed.swap(true, AtomicOrdering::SeqCst)
        {
            return Err(std::io::Error::other("leave complete pending journal"));
        }
        Ok(())
    });
    let manifest = relative(temp.path(), ".merman-manifest.json");
    let mut staging = LockedRecoveredRoot::acquire_with_checkpoint(temp.path(), hook)
        .unwrap()
        .begin(
            TransactionPlan::new([TransactionEntryPlan::write(
                TransactionRole::Manifest,
                manifest.clone(),
            )])
            .unwrap(),
        )
        .unwrap();
    staging.stage_bytes(&manifest, b"new").unwrap();
    armed.store(true, AtomicOrdering::SeqCst);
    assert!(staging.ready().is_err());
    let transaction = temp.path().join(TRANSACTION_DIR_NAME);
    assert!(transaction.join(STATE_PENDING_NAME).is_file());

    drop(LockedRecoveredRoot::acquire(temp.path()).unwrap());
    assert_eq!(read(temp.path().join(".merman-manifest.json")), b"old");
    assert!(!transaction.exists());
}

#[test]
fn bootstrap_pending_is_recovered_but_only_eof_truncation_is_discarded() {
    for bytes in [
        serde_json::to_vec(&valid_staging_value()).unwrap(),
        b"{\"schema\":".to_vec(),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let transaction = make_private_transaction_dir(temp.path());
        write_private_bytes(&transaction.join(STATE_PENDING_NAME), &bytes);
        drop(LockedRecoveredRoot::acquire(temp.path()).unwrap());
        assert!(!transaction.exists());
    }

    for bytes in [b"{]".to_vec(), vec![b'x'; MAX_STATE_BYTES as usize + 1], {
        let mut value = valid_staging_value();
        value["version"] = serde_json::json!(999);
        serde_json::to_vec(&value).unwrap()
    }] {
        let temp = tempfile::tempdir().unwrap();
        let transaction = make_private_transaction_dir(temp.path());
        let pending = transaction.join(STATE_PENDING_NAME);
        write_private_bytes(&pending, &bytes);
        let error = LockedRecoveredRoot::acquire(temp.path()).unwrap_err();
        assert!(matches!(error, TransactionError::InvalidState { .. }));
        assert_eq!(read(&pending), bytes);
    }
}

#[test]
fn truncated_journal_slots_are_not_treated_as_recoverable_pending_writes() {
    let temp = tempfile::tempdir().unwrap();
    let transaction = make_private_transaction_dir(temp.path());
    write_private_bytes(&transaction.join(STATE_A_NAME), b"{\"schema\":");
    let error = LockedRecoveredRoot::acquire(temp.path()).unwrap_err();
    assert!(matches!(error, TransactionError::InvalidState { .. }));
    assert!(transaction.exists());
}

#[test]
fn nonconsecutive_or_regressive_valid_slots_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let transaction = make_private_transaction_dir(temp.path());
    let mut first = valid_staging_value();
    first["sequence"] = serde_json::json!(1);
    let mut second = first.clone();
    second["sequence"] = serde_json::json!(100);
    write_private_bytes(
        &transaction.join(STATE_A_NAME),
        &serde_json::to_vec(&first).unwrap(),
    );
    write_private_bytes(
        &transaction.join(STATE_B_NAME),
        &serde_json::to_vec(&second).unwrap(),
    );
    let error = LockedRecoveredRoot::acquire(temp.path()).unwrap_err();
    assert!(matches!(error, TransactionError::InvalidState { .. }));
    assert!(transaction.exists());
}

#[cfg(unix)]
#[test]
fn hard_linked_journal_slots_are_rejected_without_truncating_the_other_link() {
    let temp = tempfile::tempdir().unwrap();
    let transaction = make_private_transaction_dir(temp.path());
    let slot = transaction.join(STATE_A_NAME);
    let bytes = serde_json::to_vec(&valid_staging_value()).unwrap();
    write_private_bytes(&slot, &bytes);
    let external = temp.path().join("external-journal-link");
    std::fs::hard_link(&slot, &external).unwrap();

    let error = LockedRecoveredRoot::acquire(temp.path()).unwrap_err();
    assert!(matches!(error, TransactionError::InvalidState { .. }));
    assert_eq!(read(&external), bytes);
    assert_eq!(read(&slot), bytes);
}

#[test]
fn unknown_transaction_file_is_preserved_and_blocks_recovery() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join(".merman-manifest.json"), b"old").unwrap();
    let manifest = relative(temp.path(), ".merman-manifest.json");
    let staging = LockedRecoveredRoot::acquire(temp.path())
        .unwrap()
        .begin(
            TransactionPlan::new([TransactionEntryPlan::write(
                TransactionRole::Manifest,
                manifest,
            )])
            .unwrap(),
        )
        .unwrap();
    drop(staging);
    let unknown = temp
        .path()
        .join(TRANSACTION_DIR_NAME)
        .join("operator-notes");
    write_private_bytes(&unknown, b"preserve me");
    let error = LockedRecoveredRoot::acquire(temp.path()).unwrap_err();
    assert!(matches!(error, TransactionError::InvalidState { .. }));
    assert_eq!(read(&unknown), b"preserve me");
    assert_eq!(read(temp.path().join(".merman-manifest.json")), b"old");
}

#[test]
fn deterministic_put_files_are_known_recovery_evidence() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join(".merman-manifest.json"), b"old").unwrap();
    let manifest = relative(temp.path(), ".merman-manifest.json");
    let mut staging = LockedRecoveredRoot::acquire(temp.path())
        .unwrap()
        .begin(
            TransactionPlan::new([TransactionEntryPlan::write(
                TransactionRole::Manifest,
                manifest.clone(),
            )])
            .unwrap(),
        )
        .unwrap();
    staging.stage_bytes(&manifest, b"new").unwrap();
    let ready = staging.ready().unwrap();
    let transaction = temp.path().join(TRANSACTION_DIR_NAME);
    write_private_bytes(&transaction.join(put_name(0)), b"interrupted copy");
    drop(ready);

    drop(LockedRecoveredRoot::acquire(temp.path()).unwrap());
    assert_eq!(read(temp.path().join(".merman-manifest.json")), b"old");
    assert!(!transaction.exists());
}

#[cfg(unix)]
#[test]
fn transaction_stage_symlink_substitution_is_rejected_without_following_it() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let external = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(temp.path().join(".merman-manifest.json"), b"old").unwrap();
    let manifest = relative(temp.path(), ".merman-manifest.json");
    let mut staging = LockedRecoveredRoot::acquire(temp.path())
        .unwrap()
        .begin(
            TransactionPlan::new([TransactionEntryPlan::write(
                TransactionRole::Manifest,
                manifest.clone(),
            )])
            .unwrap(),
        )
        .unwrap();
    staging.stage_bytes(&manifest, b"new").unwrap();
    let stage = temp.path().join(TRANSACTION_DIR_NAME).join(stage_name(0));
    drop(staging);
    std::fs::remove_file(&stage).unwrap();
    symlink(external.path(), &stage).unwrap();
    let before = read(external.path());
    let error = LockedRecoveredRoot::acquire(temp.path()).unwrap_err();
    assert!(matches!(error, TransactionError::InvalidState { .. }));
    assert_eq!(read(external.path()), before);
    assert_eq!(read(temp.path().join(".merman-manifest.json")), b"old");
}

fn recover_staging_state() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join(".merman-manifest.json"), b"old").unwrap();
    let manifest = relative(temp.path(), ".merman-manifest.json");
    let mut staging = LockedRecoveredRoot::acquire(temp.path())
        .unwrap()
        .begin(
            TransactionPlan::new([TransactionEntryPlan::write(
                TransactionRole::Manifest,
                manifest.clone(),
            )])
            .unwrap(),
        )
        .unwrap();
    staging.stage_bytes(&manifest, b"new").unwrap();
    drop(staging);
    drop(LockedRecoveredRoot::acquire(temp.path()).unwrap());
    assert_eq!(read(temp.path().join(".merman-manifest.json")), b"old");
    assert!(!temp.path().join(TRANSACTION_DIR_NAME).exists());
}

fn recover_staging_from_a_truncated_pending_write() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join(".merman-manifest.json"), b"old").unwrap();
    let manifest = relative(temp.path(), ".merman-manifest.json");
    let mut staging = LockedRecoveredRoot::acquire(temp.path())
        .unwrap()
        .begin(
            TransactionPlan::new([TransactionEntryPlan::write(
                TransactionRole::Manifest,
                manifest.clone(),
            )])
            .unwrap(),
        )
        .unwrap();
    staging.stage_bytes(&manifest, b"new").unwrap();
    drop(staging);
    let pending = temp
        .path()
        .join(TRANSACTION_DIR_NAME)
        .join(STATE_PENDING_NAME);
    write_private_bytes(&pending, b"{\"schema\":");

    drop(LockedRecoveredRoot::acquire(temp.path()).unwrap());
    assert_eq!(read(temp.path().join(".merman-manifest.json")), b"old");
    assert!(!temp.path().join(TRANSACTION_DIR_NAME).exists());
}

fn recover_publishing_state() {
    let temp = tempfile::tempdir().unwrap();
    seed_standard_old_generation(temp.path());
    let failures = Arc::new(AtomicU8::new(0));
    let hook_failures = Arc::clone(&failures);
    let hook = CheckpointHook::new(move |checkpoint| {
        let bit = match checkpoint {
            Checkpoint::CommitAfter { index: 0 } => 1,
            Checkpoint::RollbackBefore { index: 0 } => 2,
            _ => 0,
        };
        if bit != 0 && hook_failures.fetch_or(bit, AtomicOrdering::SeqCst) & bit == 0 {
            return Err(std::io::Error::other("leave publishing evidence"));
        }
        Ok(())
    });
    let targets = Targets::under(temp.path());
    let mut staging = LockedRecoveredRoot::acquire_with_checkpoint(temp.path(), hook)
        .unwrap()
        .begin(targets.standard_plan())
        .unwrap();
    stage_standard_new_generation(&mut staging, &targets);
    assert!(
        staging
            .ready()
            .unwrap()
            .commit()
            .unwrap_err()
            .is_partial_publication()
    );
    drop(LockedRecoveredRoot::acquire(temp.path()).unwrap());
    assert_standard_old_generation(temp.path());
}

fn recover_committed_state() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join(".merman-manifest.json"), b"old").unwrap();
    let failed = Arc::new(AtomicBool::new(false));
    let hook_failed = Arc::clone(&failed);
    let hook = CheckpointHook::new(move |checkpoint| {
        if checkpoint == Checkpoint::CleanupBefore
            && !hook_failed.swap(true, AtomicOrdering::SeqCst)
        {
            return Err(std::io::Error::other("leave committed evidence"));
        }
        Ok(())
    });
    let manifest = relative(temp.path(), ".merman-manifest.json");
    let mut staging = LockedRecoveredRoot::acquire_with_checkpoint(temp.path(), hook)
        .unwrap()
        .begin(
            TransactionPlan::new([TransactionEntryPlan::write(
                TransactionRole::Manifest,
                manifest.clone(),
            )])
            .unwrap(),
        )
        .unwrap();
    staging.stage_bytes(&manifest, b"new").unwrap();
    let error = staging.ready().unwrap().commit().unwrap_err();
    assert!(matches!(error, TransactionError::Recovery { .. }));
    assert!(!error.is_partial_publication());
    assert_eq!(read(temp.path().join(".merman-manifest.json")), b"new");
    std::fs::write(
        temp.path().join(".merman-manifest.json"),
        b"changed-after-commit",
    )
    .unwrap();
    drop(LockedRecoveredRoot::acquire(temp.path()).unwrap());
    assert_eq!(
        read(temp.path().join(".merman-manifest.json")),
        b"changed-after-commit"
    );
    assert!(!temp.path().join(TRANSACTION_DIR_NAME).exists());
}

#[derive(Debug, Clone, Copy)]
enum ForgedState {
    Malformed,
    Unsupported,
    Oversized,
    Traversal,
    UnknownField,
    ModeWithoutPrior,
}

fn write_forged_state(path: &Path, state: ForgedState) {
    let bytes = match state {
        ForgedState::Malformed => b"{\"schema\":".to_vec(),
        ForgedState::Oversized => vec![b'x'; MAX_STATE_BYTES as usize + 1],
        ForgedState::Unsupported => {
            let mut value = valid_staging_value();
            value["version"] = serde_json::json!(999);
            serde_json::to_vec(&value).unwrap()
        }
        ForgedState::Traversal => {
            let mut value = valid_staging_value();
            value["entries"][0]["target"] = serde_json::json!([
                format::encode_component(std::ffi::OsStr::new("..")).unwrap(),
                format::encode_component(std::ffi::OsStr::new("escaped.txt")).unwrap(),
            ]);
            serde_json::to_vec(&value).unwrap()
        }
        ForgedState::UnknownField => {
            let mut value = valid_staging_value();
            value["late_malicious_field"] = serde_json::json!("ignored by weak parsers");
            serde_json::to_vec(&value).unwrap()
        }
        ForgedState::ModeWithoutPrior => {
            let mut value = valid_staging_value();
            value["entries"][0]["prior_mode"] = serde_json::json!(0o644);
            serde_json::to_vec(&value).unwrap()
        }
    };
    write_private_bytes(path, &bytes);
}

fn valid_staging_value() -> serde_json::Value {
    let state = JournalState {
        id: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        owner: GenerationOwner::test_fixture(),
        sequence: 1,
        phase: JournalPhase::Staging,
        next_index: 0,
        entries: vec![JournalEntry {
            role: TransactionRole::Manifest,
            operation: TransactionOperation::Write,
            target: RelativeTarget::from_components(
                vec![std::ffi::OsString::from(".merman-manifest.json")],
                Path::new(TRANSACTION_DIR_NAME),
            )
            .unwrap(),
            prior: PriorState::Unknown,
            prior_mode: None,
        }],
    };
    serde_json::from_slice(&state.encode(Path::new(TRANSACTION_DIR_NAME)).unwrap()).unwrap()
}

fn make_private_transaction_dir(root: &Path) -> PathBuf {
    let transaction = root.join(TRANSACTION_DIR_NAME);
    create_private_directory(&transaction).unwrap();
    transaction
}

fn write_private_bytes(path: &Path, bytes: &[u8]) {
    let mut file = create_private_file(path).unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}

fn relative(root: &Path, name: impl AsRef<Path>) -> RelativeTarget {
    let root = root.canonicalize().unwrap();
    RelativeTarget::from_absolute(&root, root.join(name)).unwrap()
}

fn stage_standard_new_generation(staging: &mut StagingTransaction, targets: &Targets) {
    staging.stage_bytes(&targets.artifact_z, b"new-z").unwrap();
    staging
        .stage_bytes(&targets.document, b"new-document")
        .unwrap();
    staging
        .stage_bytes(&targets.manifest, b"new-manifest")
        .unwrap();
    staging.stage_bytes(&targets.artifact_a, b"new-a").unwrap();
}

fn seed_standard_old_generation(root: &Path) {
    std::fs::write(root.join("a.svg"), b"old-a").unwrap();
    std::fs::write(root.join("z.svg"), b"old-z").unwrap();
    std::fs::write(root.join(".merman-manifest.json"), b"old-manifest").unwrap();
    std::fs::write(root.join("document.md"), b"old-document").unwrap();
}

fn assert_standard_old_generation(root: &Path) {
    assert_eq!(read(root.join("a.svg")), b"old-a");
    assert_eq!(read(root.join("z.svg")), b"old-z");
    assert_eq!(read(root.join(".merman-manifest.json")), b"old-manifest");
    assert_eq!(read(root.join("document.md")), b"old-document");
}

fn read(path: impl AsRef<Path>) -> Vec<u8> {
    std::fs::read(path).unwrap()
}

#[cfg(unix)]
fn ordinary_mode(path: impl AsRef<Path>) -> u32 {
    use std::os::unix::fs::PermissionsExt;

    std::fs::symlink_metadata(path)
        .unwrap()
        .permissions()
        .mode()
        & 0o777
}
