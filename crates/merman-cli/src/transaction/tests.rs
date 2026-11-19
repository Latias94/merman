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

    let error = LockedRecoveredRoot::acquire_approved(approved, &mut PublicationGuards::new(None))
        .unwrap_err();
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

#[cfg(feature = "rustdoc")]
#[test]
fn transaction_plan_rejects_aggregate_pinned_backups_over_budget() {
    let temp = tempfile::tempdir().unwrap();
    let targets = Targets::under(temp.path());
    std::fs::write(temp.path().join("a.svg"), [0u8; 6]).unwrap();
    std::fs::write(temp.path().join("z.svg"), [0u8; 6]).unwrap();
    let plan = TransactionPlan::new([
        TransactionEntryPlan::write(TransactionRole::Artifact, targets.artifact_a)
            .expect_generation(content_pinned_generation(&temp.path().join("a.svg"))),
        TransactionEntryPlan::write(TransactionRole::Artifact, targets.artifact_z)
            .expect_generation(content_pinned_generation(&temp.path().join("z.svg"))),
        TransactionEntryPlan::write(TransactionRole::Manifest, targets.manifest)
            .expect_generation(TargetGeneration::Missing),
    ])
    .unwrap();

    let error = plan
        .validate_pinned_backup_bytes(Some(10))
        .expect_err("aggregate backups must respect the transaction budget");
    assert!(error.to_string().contains("12 bytes"), "{error}");
    assert!(error.to_string().contains("10-byte limit"), "{error}");
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
fn root_swap_at_replace_syscall_cannot_redirect_publication_outside_root() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("output");
    let displaced = temp.path().join("displaced-output");
    let outside = temp.path().join("outside");
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(&outside).unwrap();
    let manifest_path = root.join(".merman-manifest.json");
    let outside_manifest = outside.join(".merman-manifest.json");
    std::fs::write(&manifest_path, b"old manifest").unwrap();
    std::fs::write(&outside_manifest, b"outside sentinel").unwrap();
    let hook_root = root.clone();
    let hook_displaced = displaced.clone();
    let hook_outside = outside.clone();
    let swapped = Arc::new(AtomicBool::new(false));
    let hook_swapped = Arc::clone(&swapped);
    let hook = CheckpointHook::new(move |checkpoint| {
        if checkpoint == (Checkpoint::ReplaceBoundary { index: 0 })
            && !hook_swapped.swap(true, AtomicOrdering::SeqCst)
        {
            std::fs::rename(&hook_root, &hook_displaced)?;
            symlink(&hook_outside, &hook_root)?;
        }
        Ok(())
    });
    let manifest = relative(&root, ".merman-manifest.json");
    let mut staging = LockedRecoveredRoot::acquire_with_checkpoint(&root, hook)
        .unwrap()
        .begin(
            TransactionPlan::new([TransactionEntryPlan::write(
                TransactionRole::Manifest,
                manifest.clone(),
            )])
            .unwrap(),
        )
        .unwrap();
    staging.stage_bytes(&manifest, b"new manifest").unwrap();

    let error = staging.ready().unwrap().commit().unwrap_err();

    assert!(
        matches!(
            error,
            TransactionError::PartialPublication { .. } | TransactionError::Recovery { .. }
        ),
        "{error}"
    );
    assert_eq!(read(&outside_manifest), b"outside sentinel");
    assert_eq!(
        read(displaced.join(".merman-manifest.json")),
        b"new manifest"
    );
}

#[cfg(unix)]
#[test]
fn root_swap_at_unlink_syscall_cannot_delete_outside_root() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("output");
    let displaced = temp.path().join("displaced-output");
    let outside = temp.path().join("outside");
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(&outside).unwrap();
    let stale_path = root.join("stale.svg");
    let outside_stale = outside.join("stale.svg");
    std::fs::write(&stale_path, b"old artifact").unwrap();
    std::fs::write(&outside_stale, b"outside sentinel").unwrap();
    let hook_root = root.clone();
    let hook_displaced = displaced.clone();
    let hook_outside = outside.clone();
    let swapped = Arc::new(AtomicBool::new(false));
    let hook_swapped = Arc::clone(&swapped);
    let hook = CheckpointHook::new(move |checkpoint| {
        if checkpoint == (Checkpoint::DeleteBoundary { index: 0 })
            && !hook_swapped.swap(true, AtomicOrdering::SeqCst)
        {
            std::fs::rename(&hook_root, &hook_displaced)?;
            symlink(&hook_outside, &hook_root)?;
        }
        Ok(())
    });
    let stale = relative(&root, "stale.svg");
    let manifest = relative(&root, ".merman-manifest.json");
    let mut staging = LockedRecoveredRoot::acquire_with_checkpoint(&root, hook)
        .unwrap()
        .begin(
            TransactionPlan::new([
                TransactionEntryPlan::delete_artifact(stale),
                TransactionEntryPlan::write(TransactionRole::Manifest, manifest.clone()),
            ])
            .unwrap(),
        )
        .unwrap();
    staging.stage_bytes(&manifest, b"new manifest").unwrap();

    let error = staging.ready().unwrap().commit().unwrap_err();

    assert!(
        matches!(
            error,
            TransactionError::PartialPublication { .. } | TransactionError::Recovery { .. }
        ),
        "{error}"
    );
    assert_eq!(read(&outside_stale), b"outside sentinel");
    assert!(!displaced.join("stale.svg").exists());
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
fn recovery_restores_readable_mode_changes_and_retains_unreadable_evidence() {
    use std::os::unix::fs::PermissionsExt;

    for changed_mode in [0o600, 0o000] {
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
        std::fs::set_permissions(
            &manifest_path,
            std::fs::Permissions::from_mode(changed_mode),
        )
        .unwrap();
        let readable = match File::open(&manifest_path) {
            Ok(_) => true,
            Err(error) => {
                assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
                false
            }
        };
        drop(ready);

        let recovered = LockedRecoveredRoot::acquire(temp.path());
        if readable {
            drop(recovered.unwrap());
        } else {
            assert!(recovered.is_err());
            assert_eq!(ordinary_mode(&manifest_path), changed_mode);
            assert!(temp.path().join(TRANSACTION_DIR_NAME).is_dir());
            std::fs::set_permissions(&manifest_path, std::fs::Permissions::from_mode(0o640))
                .unwrap();
            assert_eq!(read(&manifest_path), b"old-manifest");
            drop(LockedRecoveredRoot::acquire(temp.path()).unwrap());
        }
        assert_eq!(read(&manifest_path), b"old-manifest");
        assert_eq!(ordinary_mode(&manifest_path), 0o640);
        assert!(!temp.path().join(TRANSACTION_DIR_NAME).exists());
    }
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
fn targets_cannot_alias_the_lock_through_hard_links() {
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
}

#[test]
fn hard_linked_targets_report_the_first_alias_in_publication_order() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("a.svg"),
        b"same bytes, distinct identities",
    )
    .unwrap();
    std::fs::write(
        temp.path().join("b.svg"),
        b"same bytes, distinct identities",
    )
    .unwrap();
    std::fs::hard_link(temp.path().join("a.svg"), temp.path().join("c.svg")).unwrap();
    std::fs::hard_link(temp.path().join("b.svg"), temp.path().join("d.svg")).unwrap();
    std::fs::create_dir(temp.path().join("z.svg")).unwrap();
    let a = relative(temp.path(), "a.svg");
    let b = relative(temp.path(), "b.svg");
    let c = relative(temp.path(), "c.svg");
    let d = relative(temp.path(), "d.svg");
    let z = relative(temp.path(), "z.svg");
    let manifest = relative(temp.path(), ".merman-manifest.json");
    let root = temp.path().canonicalize().unwrap();
    let first_alias = c.to_path(&root).unwrap();
    let error = LockedRecoveredRoot::acquire(temp.path())
        .unwrap()
        .begin(
            TransactionPlan::new([
                TransactionEntryPlan::write(TransactionRole::Artifact, z),
                TransactionEntryPlan::write(TransactionRole::Artifact, d),
                TransactionEntryPlan::write(TransactionRole::Artifact, c),
                TransactionEntryPlan::write(TransactionRole::Artifact, b),
                TransactionEntryPlan::write(TransactionRole::Artifact, a),
                TransactionEntryPlan::write(TransactionRole::Manifest, manifest),
            ])
            .unwrap(),
        )
        .err()
        .unwrap();
    match error {
        TransactionError::InvalidState { evidence, reason } => {
            assert_eq!(evidence, root.join(TRANSACTION_DIR_NAME));
            assert_eq!(
                reason,
                format!("transaction targets resolve to the same file: {first_alias:?}")
            );
        }
        other => panic!("expected the first target alias, got {other}"),
    }
    for name in ["a.svg", "b.svg", "c.svg", "d.svg"] {
        assert_eq!(read(root.join(name)), b"same bytes, distinct identities");
    }
    assert!(!root.join(TRANSACTION_DIR_NAME).join(STATE_A_NAME).exists());
}

#[test]
fn replacement_rejects_changed_owner_before_missing_source() {
    let temp = tempfile::tempdir().unwrap();
    let manifest_path = temp.path().join(".merman-manifest.json");
    std::fs::write(&manifest_path, b"old manifest").unwrap();
    let manifest = relative(temp.path(), ".merman-manifest.json");
    let mut staging = LockedRecoveredRoot::acquire(temp.path())
        .unwrap()
        .begin(
            TransactionPlan::new([TransactionEntryPlan::write(
                TransactionRole::Manifest,
                manifest,
            )])
            .unwrap(),
        )
        .unwrap();
    let source = staging.working.stage_path(0);
    let target = staging.working.target_path(0).unwrap();
    assert!(matches!(
        staging.working.verify_internal_regular(&source),
        Err(TransactionError::Operational {
            operation: "inspect transaction-owned file",
            ..
        })
    ));
    let original_identity = Arc::clone(&staging.working.transaction_identity);
    staging.working.transaction_identity =
        Arc::new(same_file::Handle::from_path(temp.path()).unwrap());
    let expected = staging.working.verify_owned_state().unwrap_err();
    let actual = deterministic_replace_from(
        &staging.working,
        0,
        &source,
        &target,
        None,
        "publish replacement",
    )
    .unwrap_err();
    assert!(matches!(actual, TransactionError::InvalidState { .. }));
    assert_eq!(actual.to_string(), expected.to_string());
    assert_eq!(read(&manifest_path), b"old manifest");
    assert!(!source.exists());
    staging.working.transaction_identity = original_identity;
    staging.abort().unwrap();
}

#[cfg(unix)]
#[test]
fn unknown_bytes_with_changed_permissions_are_not_restored_over() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();
    let manifest_path = temp.path().join(".merman-manifest.json");
    std::fs::write(&manifest_path, b"old manifest").unwrap();
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
    staging.stage_bytes(&manifest, b"new manifest").unwrap();
    let ready = staging.ready().unwrap();
    std::fs::write(&manifest_path, b"external contents").unwrap();
    std::fs::set_permissions(&manifest_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(
        ready.working.classify_entry(0).unwrap(),
        GenerationMatch::Unknown
    );

    let error = ready.commit().unwrap_err();
    assert!(matches!(error, TransactionError::InvalidState { .. }));
    assert_eq!(read(&manifest_path), b"external contents");
    assert_eq!(ordinary_mode(&manifest_path), 0o600);
    assert!(temp.path().join(TRANSACTION_DIR_NAME).is_dir());
    assert!(LockedRecoveredRoot::acquire(temp.path()).is_err());
    assert_eq!(read(&manifest_path), b"external contents");
    assert_eq!(ordinary_mode(&manifest_path), 0o600);
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
fn stable_publication_checks_the_full_frontier_once_before_the_final_entry() {
    for artifact_count in [0, 3] {
        for with_document in [false, true] {
            for identical in [false, true] {
                let temp = tempfile::tempdir().unwrap();
                let mut entries = Vec::new();
                for index in 0..artifact_count {
                    entries.push(TransactionEntryPlan::write(
                        TransactionRole::Artifact,
                        relative(temp.path(), format!("{index}.svg")),
                    ));
                }
                entries.push(TransactionEntryPlan::write(
                    TransactionRole::Manifest,
                    relative(temp.path(), ".merman-manifest.json"),
                ));
                if with_document {
                    entries.push(TransactionEntryPlan::write(
                        TransactionRole::Document,
                        relative(temp.path(), "document.md"),
                    ));
                }
                for entry in &entries {
                    std::fs::write(entry.target.to_path(temp.path()).unwrap(), b"old").unwrap();
                }
                let final_index = entries.len() - 1;
                let frontiers = Arc::new(std::sync::Mutex::new(Vec::new()));
                let captured = Arc::clone(&frontiers);
                let hook = CheckpointHook::new(move |checkpoint| {
                    if let Checkpoint::PublicationFrontier { next_index } = checkpoint {
                        captured.lock().unwrap().push(next_index);
                    }
                    Ok(())
                });
                let mut staging = LockedRecoveredRoot::acquire_with_checkpoint(temp.path(), hook)
                    .unwrap()
                    .begin(TransactionPlan::new(entries.clone()).unwrap())
                    .unwrap();
                let bytes = if identical { b"old" } else { b"new" };
                for entry in &entries {
                    staging.stage_bytes(&entry.target, bytes).unwrap();
                }
                staging.ready().unwrap().commit().unwrap();

                assert_eq!(*frontiers.lock().unwrap(), vec![final_index]);
                for entry in &entries {
                    assert_eq!(read(entry.target.to_path(temp.path()).unwrap()), bytes);
                }
                assert!(!temp.path().join(TRANSACTION_DIR_NAME).exists());
            }
        }
    }
}

#[test]
fn persistent_prefix_conflicts_withhold_the_document_and_preserve_unknown_targets() {
    for unknown in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        seed_standard_old_generation(temp.path());
        let artifact = temp.path().join("a.svg");
        let document = temp.path().join("document.md");
        let document_identity = same_file::Handle::from_path(&document).unwrap();
        let entered = Arc::new(AtomicU8::new(0));
        let hook_entered = Arc::clone(&entered);
        let document_attempted = Arc::new(AtomicBool::new(false));
        let hook_document_attempted = Arc::clone(&document_attempted);
        let hook_artifact = artifact.clone();
        let hook = CheckpointHook::new(move |checkpoint| {
            if let Checkpoint::CommitBefore { index } = checkpoint {
                hook_entered.fetch_or(1 << index, AtomicOrdering::SeqCst);
            }
            if checkpoint == (Checkpoint::CommitAfter { index: 0 }) {
                std::fs::write(&hook_artifact, if unknown { b"alien" } else { b"old-a" })?;
            }
            if checkpoint == (Checkpoint::ReplaceBefore { index: 3 }) {
                hook_document_attempted.store(true, AtomicOrdering::SeqCst);
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

        // Detection is deliberately deferred; the final document must still never be attempted.
        assert_eq!(entered.load(AtomicOrdering::SeqCst), 0b1111);
        assert!(!document_attempted.load(AtomicOrdering::SeqCst));
        assert_eq!(read(&document), b"old-document");
        assert_eq!(
            same_file::Handle::from_path(&document).unwrap(),
            document_identity
        );
        let transaction = temp.path().join(TRANSACTION_DIR_NAME);
        if unknown {
            assert!(error.is_partial_publication(), "{error}");
            assert_eq!(read(&artifact), b"alien");
            assert_eq!(read(temp.path().join("z.svg")), b"new-z");
            assert_eq!(read(transaction.join(backup_name(0))), b"old-a");
            assert_eq!(read(transaction.join(stage_name(0))), b"new-a");
            assert!(LockedRecoveredRoot::acquire(temp.path()).is_err());
            assert_eq!(read(&artifact), b"alien");
            assert!(transaction.is_dir());
        } else {
            assert!(matches!(error, TransactionError::CommitRolledBack { .. }));
            assert_standard_old_generation(temp.path());
            assert!(!transaction.exists());
        }
    }
}

#[test]
fn final_callback_prefix_changes_are_checked_before_document_or_manifest_publication() {
    for with_document in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        seed_standard_old_generation(temp.path());
        let targets = Targets::under(temp.path());
        let mut entries = vec![
            TransactionEntryPlan::write(TransactionRole::Artifact, targets.artifact_a.clone()),
            TransactionEntryPlan::write(TransactionRole::Artifact, targets.artifact_z.clone()),
            TransactionEntryPlan::write(TransactionRole::Manifest, targets.manifest.clone()),
        ];
        if with_document {
            entries.push(TransactionEntryPlan::write(
                TransactionRole::Document,
                targets.document.clone(),
            ));
        }
        let final_index = entries.len() - 1;
        let final_target = entries[final_index].target.to_path(temp.path()).unwrap();
        let final_identity = same_file::Handle::from_path(&final_target).unwrap();
        let final_attempted = Arc::new(AtomicBool::new(false));
        let hook_final_attempted = Arc::clone(&final_attempted);
        let hook = CheckpointHook::new(move |checkpoint| {
            if checkpoint == (Checkpoint::ReplaceBefore { index: final_index }) {
                hook_final_attempted.store(true, AtomicOrdering::SeqCst);
            }
            Ok(())
        });
        let mut staging = LockedRecoveredRoot::acquire_with_checkpoint(temp.path(), hook)
            .unwrap()
            .begin(TransactionPlan::new(entries).unwrap())
            .unwrap();
        staging.stage_bytes(&targets.artifact_a, b"new-a").unwrap();
        staging.stage_bytes(&targets.artifact_z, b"new-z").unwrap();
        staging
            .stage_bytes(&targets.manifest, b"new-manifest")
            .unwrap();
        if with_document {
            staging
                .stage_bytes(&targets.document, b"new-document")
                .unwrap();
        }
        let mut callback_count = 0;
        let mut validate = || {
            callback_count += 1;
            assert_eq!(read(temp.path().join("a.svg")), b"new-a");
            std::fs::write(temp.path().join("a.svg"), b"old-a").map_err(|error| error.to_string())
        };
        let error = staging
            .ready()
            .unwrap()
            .commit_with_precommit_validation(&mut validate)
            .unwrap_err();

        assert_eq!(callback_count, 1);
        assert!(matches!(error, TransactionError::CommitRolledBack { .. }));
        assert!(!final_attempted.load(AtomicOrdering::SeqCst));
        assert_eq!(
            same_file::Handle::from_path(&final_target).unwrap(),
            final_identity
        );
        assert_standard_old_generation(temp.path());
        assert!(!temp.path().join(TRANSACTION_DIR_NAME).exists());
    }
}

#[test]
fn a_transient_prefix_conflict_repaired_before_the_final_observation_can_commit() {
    let temp = tempfile::tempdir().unwrap();
    seed_standard_old_generation(temp.path());
    let artifact = temp.path().join("a.svg");
    let passed_intermediate = Arc::new(AtomicBool::new(false));
    let hook_passed_intermediate = Arc::clone(&passed_intermediate);
    let hook = CheckpointHook::new(move |checkpoint| {
        match checkpoint {
            Checkpoint::CommitAfter { index: 0 } => std::fs::write(&artifact, b"alien")?,
            Checkpoint::CommitAfter { index: 1 } => {
                assert_eq!(read(&artifact), b"alien");
                hook_passed_intermediate.store(true, AtomicOrdering::SeqCst);
            }
            Checkpoint::CommitBefore { index: 2 } => std::fs::write(&artifact, b"new-a")?,
            _ => {}
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

    // This is the bounded observation policy, not arbitrary-writer equivalence.
    assert!(passed_intermediate.load(AtomicOrdering::SeqCst));
    assert_eq!(read(temp.path().join("document.md")), b"new-document");
    assert_eq!(read(temp.path().join("a.svg")), b"new-a");
    assert!(!temp.path().join(TRANSACTION_DIR_NAME).exists());
}

#[test]
fn future_and_current_same_inode_writes_fail_when_the_entry_is_processed() {
    for mutation_index in [0, 1] {
        for unknown in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            seed_standard_old_generation(temp.path());
            let target = temp.path().join("z.svg");
            let identity = same_file::Handle::from_path(&target).unwrap();
            let entered = Arc::new(AtomicU8::new(0));
            let hook_entered = Arc::clone(&entered);
            let hook_target = target.clone();
            let hook = CheckpointHook::new(move |checkpoint| {
                if let Checkpoint::CommitBefore { index } = checkpoint {
                    hook_entered.fetch_or(1 << index, AtomicOrdering::SeqCst);
                    if index == mutation_index {
                        std::fs::write(&hook_target, if unknown { b"alien" } else { b"new-z" })?;
                        assert_eq!(
                            same_file::Handle::from_path(&hook_target).unwrap(),
                            identity
                        );
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

            assert_eq!(entered.load(AtomicOrdering::SeqCst), 0b0011);
            assert_eq!(read(temp.path().join("document.md")), b"old-document");
            if unknown {
                assert!(error.is_partial_publication(), "{error}");
                assert_eq!(read(&target), b"alien");
                assert_eq!(
                    read(temp.path().join(TRANSACTION_DIR_NAME).join(backup_name(1))),
                    b"old-z"
                );
            } else {
                // Matching only the new generation is insufficient permission to publish.
                assert!(matches!(error, TransactionError::CommitRolledBack { .. }));
                assert_standard_old_generation(temp.path());
            }
        }
    }
}

#[test]
fn forward_replace_and_delete_recheck_old_bytes_after_the_boundary_hook() {
    for (deleting, conflict) in [
        (false, &b"alien"[..]),
        (false, &b"new-a"[..]),
        (true, &b"alien"[..]),
    ] {
        let temp = tempfile::tempdir().unwrap();
        seed_standard_old_generation(temp.path());
        let artifact = temp.path().join("a.svg");
        let identity = same_file::Handle::from_path(&artifact).unwrap();
        let changed = Arc::new(AtomicBool::new(false));
        let hook_changed = Arc::clone(&changed);
        let hook_artifact = artifact.clone();
        let hook = CheckpointHook::new(move |checkpoint| {
            let boundary = if deleting {
                Checkpoint::DeleteBefore { index: 0 }
            } else {
                Checkpoint::ReplaceBefore { index: 0 }
            };
            if checkpoint == boundary && !hook_changed.swap(true, AtomicOrdering::SeqCst) {
                std::fs::write(&hook_artifact, conflict)?;
            }
            Ok(())
        });
        let targets = Targets::under(temp.path());
        let artifact_entry = if deleting {
            TransactionEntryPlan::delete_artifact(targets.artifact_a.clone())
        } else {
            TransactionEntryPlan::write(TransactionRole::Artifact, targets.artifact_a.clone())
        };
        let mut staging = LockedRecoveredRoot::acquire_with_checkpoint(temp.path(), hook)
            .unwrap()
            .begin(
                TransactionPlan::new([
                    artifact_entry,
                    TransactionEntryPlan::write(
                        TransactionRole::Manifest,
                        targets.manifest.clone(),
                    ),
                    TransactionEntryPlan::write(
                        TransactionRole::Document,
                        targets.document.clone(),
                    ),
                ])
                .unwrap(),
            )
            .unwrap();
        if !deleting {
            staging.stage_bytes(&targets.artifact_a, b"new-a").unwrap();
        }
        staging
            .stage_bytes(&targets.manifest, b"new-manifest")
            .unwrap();
        staging
            .stage_bytes(&targets.document, b"new-document")
            .unwrap();
        let error = staging.ready().unwrap().commit().unwrap_err();

        assert!(changed.load(AtomicOrdering::SeqCst));
        assert_eq!(read(temp.path().join("document.md")), b"old-document");
        if conflict == b"alien" {
            assert!(error.is_partial_publication(), "{error}");
            assert_eq!(read(&artifact), conflict);
            assert_eq!(same_file::Handle::from_path(&artifact).unwrap(), identity);
            assert_eq!(
                read(temp.path().join(TRANSACTION_DIR_NAME).join(backup_name(0))),
                b"old-a"
            );
            assert!(LockedRecoveredRoot::acquire(temp.path()).is_err());
            assert_eq!(read(&artifact), conflict);
        } else {
            assert!(matches!(error, TransactionError::CommitRolledBack { .. }));
            assert_standard_old_generation(temp.path());
            assert!(!temp.path().join(TRANSACTION_DIR_NAME).exists());
        }
    }
}

#[test]
fn byte_identical_inode_replacement_is_rejected_before_old_or_both_publication() {
    for identical in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        seed_standard_old_generation(temp.path());
        let artifact = temp.path().join("a.svg");
        let replacement = temp.path().join("replacement");
        std::fs::write(&replacement, b"old-a").unwrap();
        let original_identity = same_file::Handle::from_path(&artifact).unwrap();
        let replacement_identity = same_file::Handle::from_path(&replacement).unwrap();
        assert_ne!(original_identity, replacement_identity);
        let hook_artifact = artifact.clone();
        let hook = CheckpointHook::new(move |checkpoint| {
            if checkpoint == (Checkpoint::CommitBefore { index: 0 }) {
                std::fs::rename(&replacement, &hook_artifact)?;
            }
            Ok(())
        });
        let targets = Targets::under(temp.path());
        let mut staging = LockedRecoveredRoot::acquire_with_checkpoint(temp.path(), hook)
            .unwrap()
            .begin(targets.standard_plan())
            .unwrap();
        staging
            .stage_bytes(
                &targets.artifact_a,
                if identical { b"old-a" } else { b"new-a" },
            )
            .unwrap();
        staging.stage_bytes(&targets.artifact_z, b"new-z").unwrap();
        staging
            .stage_bytes(&targets.manifest, b"new-manifest")
            .unwrap();
        staging
            .stage_bytes(&targets.document, b"new-document")
            .unwrap();
        let error = staging.ready().unwrap().commit().unwrap_err();

        assert!(matches!(error, TransactionError::CommitRolledBack { .. }));
        assert_standard_old_generation(temp.path());
        assert_eq!(
            same_file::Handle::from_path(&artifact).unwrap(),
            replacement_identity
        );
        assert!(!temp.path().join(TRANSACTION_DIR_NAME).exists());
    }
}

#[test]
fn a_both_commit_point_rechecks_identity_after_the_final_callback() {
    let temp = tempfile::tempdir().unwrap();
    let manifest = relative(temp.path(), ".merman-manifest.json");
    let manifest_path = manifest.to_path(temp.path()).unwrap();
    let replacement = temp.path().join("replacement");
    std::fs::write(&manifest_path, b"same").unwrap();
    std::fs::write(&replacement, b"same").unwrap();
    let old_identity = same_file::Handle::from_path(&manifest_path).unwrap();
    let replacement_identity = same_file::Handle::from_path(&replacement).unwrap();
    assert_ne!(old_identity, replacement_identity);
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
    let mut callback_count = 0;
    let mut validate = || {
        callback_count += 1;
        std::fs::rename(&replacement, &manifest_path).map_err(|error| error.to_string())
    };
    let error = staging
        .ready()
        .unwrap()
        .commit_with_precommit_validation(&mut validate)
        .unwrap_err();

    assert_eq!(callback_count, 1);
    assert!(matches!(error, TransactionError::CommitRolledBack { .. }));
    assert_eq!(read(&manifest_path), b"same");
    assert_eq!(
        same_file::Handle::from_path(&manifest_path).unwrap(),
        replacement_identity
    );
    assert!(!temp.path().join(TRANSACTION_DIR_NAME).exists());
}

#[test]
fn current_stage_and_backup_are_validated_before_a_both_noop() {
    for substitute_stage in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        seed_standard_old_generation(temp.path());
        let transaction = temp.path().join(TRANSACTION_DIR_NAME);
        let evidence = transaction.join(if substitute_stage {
            stage_name(0)
        } else {
            backup_name(0)
        });
        let hook_evidence = evidence.clone();
        let completed_first = Arc::new(AtomicBool::new(false));
        let hook_completed_first = Arc::clone(&completed_first);
        let hook = CheckpointHook::new(move |checkpoint| {
            if checkpoint == (Checkpoint::CommitBefore { index: 0 }) {
                std::fs::remove_file(&hook_evidence)?;
                std::fs::create_dir(&hook_evidence)?;
            }
            if checkpoint == (Checkpoint::CommitAfter { index: 0 }) {
                hook_completed_first.store(true, AtomicOrdering::SeqCst);
            }
            Ok(())
        });
        let targets = Targets::under(temp.path());
        let mut staging = LockedRecoveredRoot::acquire_with_checkpoint(temp.path(), hook)
            .unwrap()
            .begin(targets.standard_plan())
            .unwrap();
        staging.stage_bytes(&targets.artifact_a, b"old-a").unwrap();
        staging.stage_bytes(&targets.artifact_z, b"new-z").unwrap();
        staging
            .stage_bytes(&targets.manifest, b"new-manifest")
            .unwrap();
        staging
            .stage_bytes(&targets.document, b"new-document")
            .unwrap();
        let error = staging.ready().unwrap().commit().unwrap_err();

        assert!(error.is_partial_publication(), "{error}");
        assert!(!completed_first.load(AtomicOrdering::SeqCst));
        assert_standard_old_generation(temp.path());
        assert!(evidence.is_dir());
        assert!(transaction.is_dir());
    }
}

#[cfg(unix)]
#[test]
fn private_source_modes_and_link_counts_are_checked_before_a_both_noop() {
    use std::os::unix::fs::PermissionsExt;

    for substitute_stage in [false, true] {
        for hard_link in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            seed_standard_old_generation(temp.path());
            let evidence = temp
                .path()
                .join(TRANSACTION_DIR_NAME)
                .join(if substitute_stage {
                    stage_name(0)
                } else {
                    backup_name(0)
                });
            let neighbor = temp.path().join("external-link");
            let hook_evidence = evidence.clone();
            let hook_neighbor = neighbor.clone();
            let hook = CheckpointHook::new(move |checkpoint| {
                if checkpoint == (Checkpoint::CommitBefore { index: 0 }) {
                    if hard_link {
                        std::fs::hard_link(&hook_evidence, &hook_neighbor)?;
                    } else {
                        std::fs::set_permissions(
                            &hook_evidence,
                            std::fs::Permissions::from_mode(0o644),
                        )?;
                    }
                }
                Ok(())
            });
            let targets = Targets::under(temp.path());
            let mut staging = LockedRecoveredRoot::acquire_with_checkpoint(temp.path(), hook)
                .unwrap()
                .begin(targets.standard_plan())
                .unwrap();
            staging.stage_bytes(&targets.artifact_a, b"old-a").unwrap();
            staging.stage_bytes(&targets.artifact_z, b"new-z").unwrap();
            staging
                .stage_bytes(&targets.manifest, b"new-manifest")
                .unwrap();
            staging
                .stage_bytes(&targets.document, b"new-document")
                .unwrap();
            let error = staging.ready().unwrap().commit().unwrap_err();

            assert!(error.is_partial_publication(), "{error}");
            assert_standard_old_generation(temp.path());
            assert_eq!(read(&evidence), b"old-a");
            if hard_link {
                assert_eq!(read(&neighbor), b"old-a");
            }
            assert!(temp.path().join(TRANSACTION_DIR_NAME).is_dir());
        }
    }
}

#[cfg(unix)]
#[test]
fn forward_boundaries_reject_changed_modes_and_rollback_keeps_its_original_acceptance() {
    use std::os::unix::fs::PermissionsExt;

    for deleting in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let artifact = temp.path().join("a.svg");
        std::fs::write(&artifact, b"old-a").unwrap();
        std::fs::set_permissions(&artifact, std::fs::Permissions::from_mode(0o640)).unwrap();
        let changed = Arc::new(AtomicBool::new(false));
        let hook_changed = Arc::clone(&changed);
        let hook_artifact = artifact.clone();
        let hook = CheckpointHook::new(move |checkpoint| {
            let boundary = if deleting {
                Checkpoint::DeleteBefore { index: 0 }
            } else {
                Checkpoint::ReplaceBefore { index: 0 }
            };
            if checkpoint == boundary && !hook_changed.swap(true, AtomicOrdering::SeqCst) {
                std::fs::set_permissions(&hook_artifact, std::fs::Permissions::from_mode(0o600))?;
            }
            Ok(())
        });
        let artifact_target = relative(temp.path(), "a.svg");
        let manifest = relative(temp.path(), ".merman-manifest.json");
        let artifact_entry = if deleting {
            TransactionEntryPlan::delete_artifact(artifact_target.clone())
        } else {
            TransactionEntryPlan::write(TransactionRole::Artifact, artifact_target.clone())
        };
        let mut staging = LockedRecoveredRoot::acquire_with_checkpoint(temp.path(), hook)
            .unwrap()
            .begin(
                TransactionPlan::new([
                    artifact_entry,
                    TransactionEntryPlan::write(TransactionRole::Manifest, manifest.clone()),
                ])
                .unwrap(),
            )
            .unwrap();
        if !deleting {
            staging.stage_bytes(&artifact_target, b"new-a").unwrap();
        }
        staging.stage_bytes(&manifest, b"new-manifest").unwrap();
        let error = staging.ready().unwrap().commit().unwrap_err();

        assert!(matches!(error, TransactionError::CommitRolledBack { .. }));
        assert!(changed.load(AtomicOrdering::SeqCst));
        assert_eq!(read(&artifact), b"old-a");
        assert_eq!(ordinary_mode(&artifact), 0o640);
        assert!(!temp.path().join(".merman-manifest.json").exists());
        assert!(!temp.path().join(TRANSACTION_DIR_NAME).exists());
    }
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

#[cfg(windows)]
#[test]
fn rollback_retries_after_windows_read_handles_are_released_separately() {
    use std::os::windows::fs::OpenOptionsExt as _;
    use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_READ, FILE_SHARE_WRITE};

    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let artifacts = (0..4)
        .map(|index| relative(root, format!("artifact-{index}.svg")))
        .collect::<Vec<_>>();
    let manifest = relative(root, ".merman-manifest.json");
    let plan = TransactionPlan::new(
        artifacts
            .iter()
            .cloned()
            .map(|target| TransactionEntryPlan::write(TransactionRole::Artifact, target))
            .chain([TransactionEntryPlan::write(
                TransactionRole::Manifest,
                manifest.clone(),
            )]),
    )
    .unwrap();
    let mut staging = LockedRecoveredRoot::acquire(root)
        .unwrap()
        .begin(plan)
        .unwrap();
    for target in artifacts.iter().chain([&manifest]) {
        staging.stage_bytes(target, b"new").unwrap();
    }
    staging.ready().unwrap().interrupt_after_publish(3);

    let open_without_delete_sharing = |index: usize| {
        OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .open(artifacts[index].to_path(root).unwrap())
            .unwrap()
    };
    let held_lower = open_without_delete_sharing(1);
    let held_upper = open_without_delete_sharing(3);
    let transaction = root.join(TRANSACTION_DIR_NAME);
    let assert_incomplete = || {
        let error = LockedRecoveredRoot::acquire(root).unwrap_err();
        assert!(
            matches!(error, TransactionError::Recovery { .. }),
            "{error}"
        );
        let (state, _) = load_journal_slots(&transaction).unwrap();
        assert_eq!(state.phase, JournalPhase::RollingBack);
        assert_eq!(state.next_index, 0);
    };
    assert_incomplete();
    assert!(!artifacts[0].to_path(root).unwrap().exists());
    assert!(!artifacts[2].to_path(root).unwrap().exists());
    drop(held_upper);
    assert_incomplete();
    assert!(!artifacts[3].to_path(root).unwrap().exists());
    drop(held_lower);

    let recovered = LockedRecoveredRoot::acquire(root).unwrap();
    assert!(!transaction.exists());
    assert!(
        artifacts
            .iter()
            .all(|target| !target.to_path(root).unwrap().exists())
    );
    let mut staging = recovered
        .begin(
            TransactionPlan::new([TransactionEntryPlan::write(
                TransactionRole::Manifest,
                manifest.clone(),
            )])
            .unwrap(),
        )
        .unwrap();
    staging.stage_bytes(&manifest, b"retry").unwrap();
    staging.ready().unwrap().commit().unwrap();
    assert_eq!(read(manifest.to_path(root).unwrap()), b"retry");
}

#[test]
fn recovery_accepts_a_legacy_rollback_cursor_in_a_pending_or_promoted_slot() {
    for pending in [true, false] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        seed_standard_old_generation(root);
        let targets = Targets::under(root);
        let mut staging = LockedRecoveredRoot::acquire(root)
            .unwrap()
            .begin(targets.standard_plan())
            .unwrap();
        stage_standard_new_generation(&mut staging, &targets);
        staging.ready().unwrap().interrupt_after_publish(1);

        let hook = CheckpointHook::new(|checkpoint| {
            if checkpoint == (Checkpoint::RollbackBefore { index: 1 }) {
                return Err(std::io::Error::other("leave a higher index pending"));
            }
            Ok(())
        });
        assert!(LockedRecoveredRoot::acquire_with_checkpoint(root, hook).is_err());
        let transaction = root.join(TRANSACTION_DIR_NAME);
        let (mut state, slot) = load_journal_slots(&transaction).unwrap();
        assert_eq!(
            (state.phase, state.next_index),
            (JournalPhase::RollingBack, 0)
        );
        state.sequence += 1;
        state.next_index = 1;
        write_private_bytes(
            &transaction.join(STATE_PENDING_NAME),
            &state.encode(&transaction).unwrap(),
        );
        if !pending {
            promote_pending_journal(&transaction, slot.other()).unwrap();
        }

        drop(LockedRecoveredRoot::acquire(root).unwrap());
        assert_standard_old_generation(root);
        assert!(!transaction.exists());
    }
}

#[test]
fn rollback_rejects_same_byte_file_replacements_at_its_mutation_boundary() {
    for replacing in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let targets = Targets::under(root);
        let artifact = targets.artifact_a.to_path(root).unwrap();
        if replacing {
            std::fs::write(&artifact, b"old-a").unwrap();
        }
        let mut staging = LockedRecoveredRoot::acquire(root)
            .unwrap()
            .begin(targets.standard_plan())
            .unwrap();
        stage_standard_new_generation(&mut staging, &targets);
        staging.ready().unwrap().interrupt_after_publish(0);

        let replacement = root.join("external.svg");
        std::fs::write(&replacement, b"new-a").unwrap();
        let replacement_identity = same_file::Handle::from_path(&replacement).unwrap();
        let hook_target = artifact.clone();
        let hook = CheckpointHook::new(move |checkpoint| {
            let boundary = if replacing {
                Checkpoint::ReplaceBefore { index: 0 }
            } else {
                Checkpoint::DeleteBefore { index: 0 }
            };
            if checkpoint == boundary {
                std::fs::rename(&replacement, &hook_target)?;
            }
            Ok(())
        });
        let error = LockedRecoveredRoot::acquire_with_checkpoint(root, hook).unwrap_err();
        assert!(error.to_string().contains("rollback boundary"), "{error}");
        assert!(root.join(TRANSACTION_DIR_NAME).is_dir());
        assert_eq!(read(&artifact), b"new-a");
        assert_eq!(
            same_file::Handle::from_path(&artifact).unwrap(),
            replacement_identity
        );
    }
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

#[cfg(unix)]
#[test]
fn root_swap_during_cleanup_cannot_remove_outside_transaction_files() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("output");
    let displaced = temp.path().join("displaced-output");
    let outside = temp.path().join("outside");
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(root.join(".merman-manifest.json"), b"old").unwrap();
    let outside_transaction = make_private_transaction_dir(&outside);
    let outside_stage = outside_transaction.join(stage_name(0));
    write_private_bytes(&outside_stage, b"outside sentinel");
    let hook_root = root.clone();
    let hook_displaced = displaced.clone();
    let hook_outside = outside.clone();
    let swapped = Arc::new(AtomicBool::new(false));
    let hook_swapped = Arc::clone(&swapped);
    let hook = CheckpointHook::new(move |checkpoint| {
        if checkpoint == (Checkpoint::CleanupFileBefore { ordinal: 0 })
            && !hook_swapped.swap(true, AtomicOrdering::SeqCst)
        {
            std::fs::rename(&hook_root, &hook_displaced)?;
            symlink(&hook_outside, &hook_root)?;
        }
        Ok(())
    });
    let manifest = relative(&root, ".merman-manifest.json");
    let mut staging = LockedRecoveredRoot::acquire_with_checkpoint(&root, hook)
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
        "{error}"
    );
    assert_eq!(read(&outside_stage), b"outside sentinel");
    assert_eq!(read(displaced.join(".merman-manifest.json")), b"new");
    assert!(!displaced.join(TRANSACTION_DIR_NAME).exists());
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
fn prepared_publishing_cursors_and_committed_marker_recover_mixed_operations() {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    for with_document in [false, true] {
        let entry_count = if with_document { 4 } else { 3 };
        for pending_ordinal in 1..=entry_count + 1 {
            let temp = tempfile::tempdir().unwrap();
            seed_standard_old_generation(temp.path());
            let targets = Targets::under(temp.path());
            let mut entries = vec![
                TransactionEntryPlan::write(TransactionRole::Artifact, targets.artifact_a.clone()),
                TransactionEntryPlan::delete_artifact(targets.artifact_z.clone()),
                TransactionEntryPlan::write(TransactionRole::Manifest, targets.manifest.clone()),
            ];
            if with_document {
                entries.push(TransactionEntryPlan::write(
                    TransactionRole::Document,
                    targets.document.clone(),
                ));
            }

            let armed = Arc::new(AtomicBool::new(false));
            let prepared_count = Arc::new(AtomicU8::new(0));
            let hook_armed = Arc::clone(&armed);
            let hook_count = Arc::clone(&prepared_count);
            let hook = CheckpointHook::new(move |checkpoint| {
                if matches!(checkpoint, Checkpoint::PersistPrepared { .. })
                    && hook_armed.load(AtomicOrdering::SeqCst)
                    && usize::from(hook_count.fetch_add(1, AtomicOrdering::SeqCst)) + 1
                        == pending_ordinal
                {
                    panic!("abrupt publishing pending interruption");
                }
                Ok(())
            });
            let mut staging = LockedRecoveredRoot::acquire_with_checkpoint(temp.path(), hook)
                .unwrap()
                .begin(TransactionPlan::new(entries).unwrap())
                .unwrap();
            staging.stage_bytes(&targets.artifact_a, b"new-a").unwrap();
            staging
                .stage_bytes(&targets.manifest, b"new-manifest")
                .unwrap();
            if with_document {
                staging
                    .stage_bytes(&targets.document, b"new-document")
                    .unwrap();
            }
            let ready = staging.ready().unwrap();
            armed.store(true, AtomicOrdering::SeqCst);
            let abrupt = catch_unwind(AssertUnwindSafe(|| ready.commit()))
                .expect_err("commit must unwind without entering its error rollback path");
            assert_eq!(
                abrupt.downcast_ref::<&str>().copied(),
                Some("abrupt publishing pending interruption")
            );
            assert_eq!(
                usize::from(prepared_count.load(AtomicOrdering::SeqCst)),
                pending_ordinal
            );

            let transaction = temp.path().join(TRANSACTION_DIR_NAME);
            let pending_path = transaction.join(STATE_PENDING_NAME);
            let pending = match load_journal_file(&pending_path, &transaction, false).unwrap() {
                JournalRead::Valid(state) => state,
                _ => panic!("expected a complete pending publication journal"),
            };
            let marker_pending = pending_ordinal == entry_count + 1;
            let published_count = pending_ordinal.min(entry_count);
            assert_eq!(
                pending.phase,
                if marker_pending {
                    JournalPhase::Committed
                } else {
                    JournalPhase::Publishing
                }
            );
            assert_eq!(pending.next_index, published_count);
            assert_eq!(pending.entries.len(), entry_count);
            let (current, active_slot) = load_journal_slots(&transaction).unwrap();
            assert_eq!(current.phase, JournalPhase::Publishing);
            assert_eq!(current.next_index, pending_ordinal - 1);
            current.validate_successor(&pending, &transaction).unwrap();

            assert_eq!(read(temp.path().join("a.svg")), b"new-a");
            if published_count >= 2 {
                assert!(!temp.path().join("z.svg").exists());
            } else {
                assert_eq!(read(temp.path().join("z.svg")), b"old-z");
            }
            assert_eq!(
                read(temp.path().join(".merman-manifest.json")),
                if published_count >= 3 {
                    b"new-manifest"
                } else {
                    b"old-manifest"
                }
            );
            assert_eq!(
                read(temp.path().join("document.md")),
                if with_document && published_count == 4 {
                    b"new-document"
                } else {
                    b"old-document"
                }
            );

            let expect_new = published_count == entry_count;
            let commit_point = if with_document {
                temp.path().join("document.md")
            } else {
                temp.path().join(".merman-manifest.json")
            };
            let commit_identity = same_file::Handle::from_path(&commit_point).unwrap();
            let new_artifact_identity = expect_new
                .then(|| same_file::Handle::from_path(temp.path().join("a.svg")).unwrap());
            let pending_bytes = read(&pending_path);
            let promoted_slot = active_slot.other();
            let promotion_seen = Arc::new(AtomicBool::new(false));
            let hook_seen = Arc::clone(&promotion_seen);
            let recovery_directory = transaction.clone();
            let recovery_hook = CheckpointHook::new(move |checkpoint| {
                if matches!(
                    checkpoint,
                    Checkpoint::PersistBefore { .. } | Checkpoint::CleanupBefore
                ) && !hook_seen.swap(true, AtomicOrdering::SeqCst)
                {
                    assert!(!recovery_directory.join(STATE_PENDING_NAME).exists());
                    let (_, recovered_slot) = load_journal_slots(&recovery_directory).unwrap();
                    assert_eq!(recovered_slot, promoted_slot);
                    assert_eq!(
                        read(recovery_directory.join(promoted_slot.file_name())),
                        pending_bytes,
                        "promote the complete pending successor before recovery writes or cleanup"
                    );
                }
                Ok(())
            });
            drop(LockedRecoveredRoot::acquire_with_checkpoint(temp.path(), recovery_hook).unwrap());
            assert!(promotion_seen.load(AtomicOrdering::SeqCst));

            if expect_new {
                assert_eq!(read(temp.path().join("a.svg")), b"new-a");
                assert!(!temp.path().join("z.svg").exists());
                assert_eq!(
                    read(temp.path().join(".merman-manifest.json")),
                    b"new-manifest"
                );
                assert_eq!(
                    read(temp.path().join("document.md")),
                    if with_document {
                        b"new-document"
                    } else {
                        b"old-document"
                    }
                );
            } else {
                assert_standard_old_generation(temp.path());
            }
            assert_eq!(
                same_file::Handle::from_path(&commit_point).unwrap(),
                commit_identity
            );
            if let Some(identity) = new_artifact_identity {
                assert_eq!(
                    same_file::Handle::from_path(temp.path().join("a.svg")).unwrap(),
                    identity
                );
            }
            assert!(!transaction.exists());
        }
    }
}

#[test]
fn prepared_publishing_cursor_with_a_both_commit_point_uses_actual_generations() {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    // Both cannot prove that the commit point ran. Existing recovery keeps all-new
    // generations first, and rolls back an incomplete generation with a Both commit point.
    for with_document in [false, true] {
        let entry_count = if with_document { 4 } else { 3 };
        for pending_ordinal in [1, entry_count - 1, entry_count] {
            let temp = tempfile::tempdir().unwrap();
            seed_standard_old_generation(temp.path());
            let targets = Targets::under(temp.path());
            let mut entries = vec![
                TransactionEntryPlan::write(TransactionRole::Artifact, targets.artifact_a.clone()),
                TransactionEntryPlan::delete_artifact(targets.artifact_z.clone()),
                TransactionEntryPlan::write(TransactionRole::Manifest, targets.manifest.clone()),
            ];
            if with_document {
                entries.push(TransactionEntryPlan::write(
                    TransactionRole::Document,
                    targets.document.clone(),
                ));
            }
            let commit_point = if with_document {
                temp.path().join("document.md")
            } else {
                temp.path().join(".merman-manifest.json")
            };
            let commit_identity = same_file::Handle::from_path(&commit_point).unwrap();
            let commit_modified = std::fs::metadata(&commit_point)
                .unwrap()
                .modified()
                .unwrap();
            let armed = Arc::new(AtomicBool::new(false));
            let prepared_count = Arc::new(AtomicU8::new(0));
            let hook_armed = Arc::clone(&armed);
            let hook_count = Arc::clone(&prepared_count);
            let hook = CheckpointHook::new(move |checkpoint| {
                if matches!(checkpoint, Checkpoint::PersistPrepared { .. })
                    && hook_armed.load(AtomicOrdering::SeqCst)
                    && usize::from(hook_count.fetch_add(1, AtomicOrdering::SeqCst)) + 1
                        == pending_ordinal
                {
                    panic!("abrupt Both commit-point pending interruption");
                }
                Ok(())
            });
            let mut staging = LockedRecoveredRoot::acquire_with_checkpoint(temp.path(), hook)
                .unwrap()
                .begin(TransactionPlan::new(entries).unwrap())
                .unwrap();
            staging.stage_bytes(&targets.artifact_a, b"new-a").unwrap();
            staging
                .stage_bytes(
                    &targets.manifest,
                    if with_document {
                        b"new-manifest"
                    } else {
                        b"old-manifest"
                    },
                )
                .unwrap();
            if with_document {
                staging
                    .stage_bytes(&targets.document, b"old-document")
                    .unwrap();
            }
            let ready = staging.ready().unwrap();
            assert_eq!(
                ready.working.classify_entry(entry_count - 1).unwrap(),
                GenerationMatch::Both
            );
            armed.store(true, AtomicOrdering::SeqCst);
            let abrupt = catch_unwind(AssertUnwindSafe(|| ready.commit()))
                .expect_err("commit must unwind without automatically rolling back");
            assert_eq!(
                abrupt.downcast_ref::<&str>().copied(),
                Some("abrupt Both commit-point pending interruption")
            );
            assert_eq!(
                usize::from(prepared_count.load(AtomicOrdering::SeqCst)),
                pending_ordinal
            );

            let transaction = temp.path().join(TRANSACTION_DIR_NAME);
            let pending_path = transaction.join(STATE_PENDING_NAME);
            let pending = match load_journal_file(&pending_path, &transaction, false).unwrap() {
                JournalRead::Valid(state) => state,
                _ => panic!("expected a complete pending publication cursor"),
            };
            assert_eq!(pending.phase, JournalPhase::Publishing);
            assert_eq!(pending.next_index, pending_ordinal);
            assert_eq!(pending.entries.len(), entry_count);
            let (current, _) = load_journal_slots(&transaction).unwrap();
            assert_eq!(current.phase, JournalPhase::Publishing);
            assert_eq!(current.next_index, pending_ordinal - 1);
            current.validate_successor(&pending, &transaction).unwrap();

            let expect_new = pending_ordinal >= entry_count - 1;
            assert_eq!(read(temp.path().join("a.svg")), b"new-a");
            if expect_new {
                assert!(!temp.path().join("z.svg").exists());
            } else {
                assert_eq!(read(temp.path().join("z.svg")), b"old-z");
            }
            assert_eq!(
                read(temp.path().join(".merman-manifest.json")),
                if with_document && expect_new {
                    b"new-manifest"
                } else {
                    b"old-manifest"
                }
            );
            assert_eq!(read(temp.path().join("document.md")), b"old-document");

            drop(LockedRecoveredRoot::acquire(temp.path()).unwrap());
            if expect_new {
                assert_eq!(read(temp.path().join("a.svg")), b"new-a");
                assert!(!temp.path().join("z.svg").exists());
                assert_eq!(
                    read(temp.path().join(".merman-manifest.json")),
                    if with_document {
                        b"new-manifest"
                    } else {
                        b"old-manifest"
                    }
                );
                assert_eq!(read(temp.path().join("document.md")), b"old-document");
            } else {
                assert_standard_old_generation(temp.path());
            }
            assert_eq!(
                same_file::Handle::from_path(&commit_point).unwrap(),
                commit_identity,
                "recovery must not rewrite the Both commit point"
            );
            assert_eq!(
                std::fs::metadata(&commit_point)
                    .unwrap()
                    .modified()
                    .unwrap(),
                commit_modified
            );
            assert!(!transaction.exists());
        }
    }
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
