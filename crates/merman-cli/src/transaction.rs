mod format;

pub(crate) use format::{
    ArtifactNamespace, GenerationDialect, GenerationManifest, GenerationOwner, RelativeTarget,
    TransactionOperation, TransactionRole,
};

use format::{
    JournalEntry, JournalPhase, JournalState, MAX_STATE_BYTES, PriorState, read_at_most,
    validate_entries,
};
use std::collections::HashSet;
use std::fs::{File, OpenOptions, TryLockError};
use std::hash::{Hash, Hasher};
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

const LOCK_FILE_NAME: &str = ".merman.lock";
const TRANSACTION_DIR_NAME: &str = ".merman.transaction";
const STATE_A_NAME: &str = "state-a.json";
const STATE_B_NAME: &str = "state-b.json";
const STATE_PENDING_NAME: &str = "state-pending.json";

#[derive(Debug, thiserror::Error)]
pub(crate) enum TransactionError {
    #[error("another Merman publication holds transaction lock {lock_path:?}")]
    Contended { lock_path: PathBuf },
    #[error("invalid transaction state retained at {evidence:?}: {reason}")]
    InvalidState { evidence: PathBuf, reason: String },
    #[error("failed to recover transaction evidence at {evidence:?}: {reason}")]
    Recovery { evidence: PathBuf, reason: String },
    #[error("failed to {operation} {path:?}: {source}")]
    Operational {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("publication failed but the previous generation was restored under {root:?}: {reason}")]
    CommitRolledBack { root: PathBuf, reason: String },
    #[error(
        "publication or rollback is incomplete; recovery evidence remains at {evidence:?}: {reason}"
    )]
    PartialPublication { evidence: PathBuf, reason: String },
}

impl TransactionError {
    #[cfg(test)]
    pub(crate) fn evidence_path(&self) -> Option<&Path> {
        match self {
            Self::Contended { lock_path } => Some(lock_path),
            Self::InvalidState { evidence, .. }
            | Self::Recovery { evidence, .. }
            | Self::PartialPublication { evidence, .. } => Some(evidence),
            Self::Operational { path, .. } => Some(path),
            Self::CommitRolledBack { root, .. } => Some(root),
        }
    }

    #[cfg(test)]
    pub(crate) fn is_partial_publication(&self) -> bool {
        matches!(self, Self::PartialPublication { .. })
    }

    #[cfg(test)]
    pub(crate) fn is_contention(&self) -> bool {
        matches!(self, Self::Contended { .. })
    }

    fn invalid_state(evidence: impl AsRef<Path>, reason: impl Into<String>) -> Self {
        Self::InvalidState {
            evidence: evidence.as_ref().to_path_buf(),
            reason: reason.into(),
        }
    }

    fn recovery(evidence: impl AsRef<Path>, reason: impl Into<String>) -> Self {
        Self::Recovery {
            evidence: evidence.as_ref().to_path_buf(),
            reason: reason.into(),
        }
    }

    fn operational(
        operation: &'static str,
        path: impl AsRef<Path>,
        source: std::io::Error,
    ) -> Self {
        Self::Operational {
            operation,
            path: path.as_ref().to_path_buf(),
            source,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TargetGeneration {
    Missing,
    Existing(Arc<same_file::Handle>),
}

impl TargetGeneration {
    pub(crate) fn from_preflight_identity(identity: Option<Arc<same_file::Handle>>) -> Self {
        identity.map_or(Self::Missing, Self::Existing)
    }

    fn from_observed_identity(identity: Option<same_file::Handle>) -> Self {
        identity.map_or(Self::Missing, |identity| Self::Existing(Arc::new(identity)))
    }

    fn matches_identity(&self, current: Option<&same_file::Handle>) -> bool {
        match (self, current) {
            (Self::Missing, None) => true,
            (Self::Existing(expected), Some(current)) => expected.as_ref() == current,
            (Self::Missing, Some(_)) | (Self::Existing(_), None) => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TransactionEntryPlan {
    role: TransactionRole,
    operation: TransactionOperation,
    target: RelativeTarget,
    expected_generation: Option<TargetGeneration>,
}

impl TransactionEntryPlan {
    pub(crate) fn write(role: TransactionRole, target: RelativeTarget) -> Self {
        Self {
            role,
            operation: TransactionOperation::Write,
            target,
            expected_generation: None,
        }
    }

    pub(crate) fn delete_artifact(target: RelativeTarget) -> Self {
        Self {
            role: TransactionRole::Artifact,
            operation: TransactionOperation::Delete,
            target,
            expected_generation: None,
        }
    }

    pub(crate) fn expect_generation(mut self, generation: TargetGeneration) -> Self {
        self.expected_generation = Some(generation);
        self
    }

    #[cfg(test)]
    pub(crate) fn target(&self) -> &RelativeTarget {
        &self.target
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TransactionPlan {
    owner: GenerationOwner,
    entries: Vec<TransactionEntryPlan>,
}

impl TransactionPlan {
    #[cfg(test)]
    pub(crate) fn new(
        entries: impl IntoIterator<Item = TransactionEntryPlan>,
    ) -> Result<Self, TransactionError> {
        let owner = GenerationOwner::test_fixture();
        owner.validate(Path::new(TRANSACTION_DIR_NAME))?;
        Self::normalize(owner, entries.into_iter().collect())
    }

    pub(crate) fn for_generation(
        owner: GenerationOwner,
        entries: impl IntoIterator<Item = TransactionEntryPlan>,
    ) -> Result<Self, TransactionError> {
        owner.validate(Path::new(TRANSACTION_DIR_NAME))?;
        let entries = entries.into_iter().collect::<Vec<_>>();
        if entries
            .iter()
            .any(|entry| entry.expected_generation.is_none())
        {
            return Err(TransactionError::invalid_state(
                Path::new(TRANSACTION_DIR_NAME),
                "every live transaction entry must carry its preflight target generation",
            ));
        }
        Self::normalize(owner, entries)
    }

    fn normalize(
        owner: GenerationOwner,
        entries: Vec<TransactionEntryPlan>,
    ) -> Result<Self, TransactionError> {
        let mut artifacts = entries
            .iter()
            .filter(|entry| entry.role == TransactionRole::Artifact)
            .cloned()
            .collect::<Vec<_>>();
        artifacts.sort_by(|left, right| left.target.cmp(&right.target));
        let manifests = entries
            .iter()
            .filter(|entry| entry.role == TransactionRole::Manifest)
            .cloned()
            .collect::<Vec<_>>();
        let documents = entries
            .iter()
            .filter(|entry| entry.role == TransactionRole::Document)
            .cloned()
            .collect::<Vec<_>>();
        let normalized = artifacts
            .into_iter()
            .chain(manifests)
            .chain(documents)
            .collect::<Vec<_>>();
        let journal_entries = normalized
            .iter()
            .map(|entry| JournalEntry {
                role: entry.role,
                operation: entry.operation,
                target: entry.target.clone(),
                prior: PriorState::Unknown,
                prior_mode: None,
            })
            .collect::<Vec<_>>();
        validate_entries(&journal_entries, Path::new(TRANSACTION_DIR_NAME))?;
        Ok(Self {
            owner,
            entries: normalized,
        })
    }

    #[cfg(test)]
    pub(crate) fn entries(&self) -> &[TransactionEntryPlan] {
        &self.entries
    }
}

pub(crate) struct LockedRecoveredRoot {
    context: RootContext,
}

impl std::fmt::Debug for LockedRecoveredRoot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LockedRecoveredRoot")
            .field("root", &self.context.root)
            .finish_non_exhaustive()
    }
}

impl LockedRecoveredRoot {
    pub(crate) fn acquire_approved(
        approved: crate::output::ApprovedTransactionRoot,
    ) -> Result<Self, TransactionError> {
        approved.verify().map_err(|error| {
            TransactionError::invalid_state(
                approved.path(),
                format!("approved transaction root failed verification: {error}"),
            )
        })?;
        let root = approved.path().to_path_buf();
        let context = RootContext::acquire_approved(&root, approved, CheckpointHook::inactive())?;
        let context = recover_existing_transaction(context)?;
        Ok(Self { context })
    }

    #[cfg(test)]
    pub(crate) fn acquire(root: impl AsRef<Path>) -> Result<Self, TransactionError> {
        Self::acquire_with_checkpoint(root, CheckpointHook::inactive())
    }

    #[cfg(test)]
    fn acquire_with_checkpoint(
        root: impl AsRef<Path>,
        checkpoint: CheckpointHook,
    ) -> Result<Self, TransactionError> {
        let context = RootContext::acquire(root.as_ref(), checkpoint)?;
        let context = recover_existing_transaction(context)?;
        Ok(Self { context })
    }

    pub(crate) fn begin(
        self,
        plan: TransactionPlan,
    ) -> Result<StagingTransaction, TransactionError> {
        self.context.verify_root_and_lock()?;
        let transaction_dir = self.context.root.join(TRANSACTION_DIR_NAME);
        match std::fs::symlink_metadata(&transaction_dir) {
            Ok(_) => {
                return Err(TransactionError::invalid_state(
                    &transaction_dir,
                    "a transaction directory exists after recovery",
                ));
            }
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(TransactionError::operational(
                    "inspect transaction directory",
                    &transaction_dir,
                    source,
                ));
            }
        }
        create_private_directory(&transaction_dir)?;
        sync_directory(&self.context.root)?;
        let transaction_identity = Arc::new(
            same_file::Handle::from_path(&transaction_dir).map_err(|source| {
                TransactionError::operational(
                    "inspect transaction directory identity",
                    &transaction_dir,
                    source,
                )
            })?,
        );
        let transaction_metadata =
            verify_private_directory(&transaction_dir, &transaction_identity)?;
        self.context
            .verify_transaction_filesystem(&transaction_dir, &transaction_metadata)?;
        let TransactionPlan {
            owner,
            entries: planned_entries,
        } = plan;
        let mut entries = Vec::with_capacity(planned_entries.len());
        let mut expected_generations = Vec::with_capacity(planned_entries.len());
        for entry in planned_entries {
            let TransactionEntryPlan {
                role,
                operation,
                target,
                expected_generation,
            } = entry;
            entries.push(JournalEntry {
                role,
                operation,
                target,
                prior: PriorState::Unknown,
                prior_mode: None,
            });
            expected_generations.push(expected_generation);
        }
        let state = JournalState {
            id: transaction_id(&self.context.root),
            owner,
            sequence: 0,
            phase: JournalPhase::Staging,
            next_index: 0,
            entries,
        };
        let mut working = WorkingTransaction {
            context: self.context,
            transaction_dir,
            transaction_identity,
            state,
            active_slot: None,
            expected_generations,
        };
        working.validate_target_set()?;
        working.persist()?;
        let issued = vec![false; working.state.entries.len()];
        Ok(StagingTransaction {
            working,
            issued,
            outstanding_slots: Arc::new(AtomicUsize::new(0)),
        })
    }
}

pub(crate) struct StagingTransaction {
    working: WorkingTransaction,
    issued: Vec<bool>,
    outstanding_slots: Arc<AtomicUsize>,
}

impl StagingTransaction {
    pub(crate) fn transaction_id(&self) -> &str {
        &self.working.state.id
    }

    #[cfg(test)]
    pub(crate) fn stage_bytes(
        &mut self,
        target: &RelativeTarget,
        bytes: &[u8],
    ) -> Result<(), TransactionError> {
        self.stage_slot(target)?.write_bytes(bytes)
    }

    pub(crate) fn stage_slot(
        &mut self,
        target: &RelativeTarget,
    ) -> Result<StageSlot, TransactionError> {
        let index = self
            .working
            .state
            .entries
            .iter()
            .position(|entry| &entry.target == target)
            .ok_or_else(|| {
                TransactionError::invalid_state(
                    &self.working.transaction_dir,
                    "attempted to stage a target outside the transaction plan",
                )
            })?;
        if self.working.state.entries[index].operation != TransactionOperation::Write {
            return Err(TransactionError::invalid_state(
                &self.working.transaction_dir,
                "attempted to stage a delete entry",
            ));
        }
        if self.issued[index] {
            return Err(TransactionError::invalid_state(
                &self.working.transaction_dir,
                "attempted to issue a transaction stage slot more than once",
            ));
        }
        self.working.verify_owned_state()?;
        self.working.checkpoint(Checkpoint::StageBefore { index })?;
        self.working
            .verify_internal_missing(&self.working.stage_path(index))?;
        self.issued[index] = true;
        self.outstanding_slots.fetch_add(1, Ordering::Release);
        Ok(StageSlot {
            index,
            path: self.working.stage_path(index),
            transaction_dir: self.working.transaction_dir.clone(),
            transaction_identity: Arc::clone(&self.working.transaction_identity),
            _lock_file: Arc::clone(&self.working.context._lock_file),
            checkpoint: self.working.context.checkpoint.clone(),
            outstanding_slots: Arc::clone(&self.outstanding_slots),
        })
    }

    pub(crate) fn ready(mut self) -> Result<ReadyTransaction, TransactionError> {
        self.verify_no_outstanding_slots()?;
        self.working.verify_owned_state()?;
        for (index, entry) in self.working.state.entries.iter().enumerate() {
            let stage = self.working.stage_path(index);
            match entry.operation {
                TransactionOperation::Write => {
                    self.working.verify_internal_regular(&stage)?;
                }
                TransactionOperation::Delete => {
                    self.working.verify_internal_missing(&stage)?;
                }
            }
        }
        self.working.state.phase = JournalPhase::BackingUp;
        self.working.persist()?;
        let mut prior = Vec::with_capacity(self.working.state.entries.len());
        let mut prior_generations = Vec::with_capacity(self.working.state.entries.len());
        for index in 0..self.working.state.entries.len() {
            let target = self.working.target_path(index)?;
            let observed_identity = self.working.inspect_target_identity(&target)?;
            if let Some(expected) = &self.working.expected_generations[index]
                && !expected.matches_identity(observed_identity.as_ref())
            {
                return Err(TransactionError::invalid_state(
                    &self.working.transaction_dir,
                    format!(
                        "publication target changed after local preflight approval: {target:?}"
                    ),
                ));
            }
            let retained_generation = self.working.expected_generations[index]
                .take()
                .unwrap_or_else(|| TargetGeneration::from_observed_identity(observed_identity));
            match &retained_generation {
                TargetGeneration::Missing => {
                    prior.push((PriorState::Missing, None));
                    self.working
                        .verify_internal_missing(&self.working.backup_path(index))?;
                }
                TargetGeneration::Existing(_) => {
                    let backup = self.working.backup_path(index);
                    let prior_mode = copy_regular_to_private(
                        &self.working,
                        &target,
                        &backup,
                        "back up publication target",
                    )?;
                    prior.push((PriorState::Present, prior_mode));
                    self.working.verify_target_generation(
                        &target,
                        &retained_generation,
                        "publication target changed while its prior generation was retained",
                    )?;
                }
            }
            prior_generations.push(retained_generation);
        }
        for (entry, (prior, prior_mode)) in self.working.state.entries.iter_mut().zip(prior) {
            entry.prior = prior;
            entry.prior_mode = prior_mode;
        }
        let generations = self.working.classify_all()?;
        if generations.iter().any(|generation| !generation.is_old()) {
            return Err(TransactionError::invalid_state(
                &self.working.transaction_dir,
                "a publication target changed while backups were prepared",
            ));
        }
        self.working.state.phase = JournalPhase::Publishing;
        self.working.state.next_index = 0;
        self.working.persist()?;
        Ok(ReadyTransaction {
            working: self.working,
            prior_generations,
        })
    }

    pub(crate) fn abort(self) -> Result<LockedRecoveredRoot, TransactionError> {
        self.verify_no_outstanding_slots()?;
        let mut working = self.working;
        working.validate_recovery_files(false)?;
        working.cleanup()?;
        Ok(LockedRecoveredRoot {
            context: working.context,
        })
    }

    fn verify_no_outstanding_slots(&self) -> Result<(), TransactionError> {
        let outstanding = self.outstanding_slots.load(Ordering::Acquire);
        if outstanding != 0 {
            return Err(TransactionError::invalid_state(
                &self.working.transaction_dir,
                format!(
                    "cannot finalize transaction while {outstanding} stage slot(s) are still active"
                ),
            ));
        }
        Ok(())
    }
}

pub(crate) struct StageSlot {
    index: usize,
    path: PathBuf,
    transaction_dir: PathBuf,
    transaction_identity: Arc<same_file::Handle>,
    _lock_file: Arc<File>,
    checkpoint: CheckpointHook,
    outstanding_slots: Arc<AtomicUsize>,
}

impl StageSlot {
    pub(crate) fn write_bytes(self, bytes: &[u8]) -> Result<(), TransactionError> {
        verify_private_directory(&self.transaction_dir, &self.transaction_identity)?;
        let mut file = create_private_file(&self.path)?;
        file.write_all(bytes).map_err(|source| {
            TransactionError::operational("write transaction stage", &self.path, source)
        })?;
        file.sync_all().map_err(|source| {
            TransactionError::operational("sync transaction stage", &self.path, source)
        })?;
        sync_directory(&self.transaction_dir)?;
        self.checkpoint
            .run(Checkpoint::StageAfter { index: self.index })
            .map_err(|source| {
                TransactionError::operational(
                    "run transaction checkpoint",
                    &self.transaction_dir,
                    source,
                )
            })?;
        Ok(())
    }
}

impl Drop for StageSlot {
    fn drop(&mut self) {
        let previous = self.outstanding_slots.fetch_sub(1, Ordering::Release);
        debug_assert!(previous > 0, "stage slot accounting underflow");
    }
}

pub(crate) struct ReadyTransaction {
    working: WorkingTransaction,
    prior_generations: Vec<TargetGeneration>,
}

impl ReadyTransaction {
    pub(crate) fn commit(mut self) -> Result<(), TransactionError> {
        self.verify_unpublished_generations(0)?;
        let generations = self.working.classify_all()?;
        if generations.iter().any(|generation| !generation.is_old()) {
            return Err(TransactionError::invalid_state(
                &self.working.transaction_dir,
                "publication targets no longer match the backed-up generation",
            ));
        }

        let publication_result = self.publish_all();
        if let Err(commit_error) = publication_result {
            return self.rollback_after_commit_failure(commit_error);
        }

        let final_generation = self.working.classify_all();
        match final_generation {
            Ok(generations) if generations.iter().all(|generation| generation.is_new()) => {}
            Ok(_) => {
                let source = TransactionError::recovery(
                    &self.working.transaction_dir,
                    "publication targets changed before the committed marker was written",
                );
                return self.rollback_after_commit_failure(source);
            }
            Err(source) => return self.rollback_after_commit_failure(source),
        }

        self.working.state.phase = JournalPhase::Committed;
        self.working.state.next_index = self.working.state.entries.len();
        if let Err(commit_marker_error) = self.working.persist() {
            return Err(TransactionError::Recovery {
                evidence: self.working.transaction_dir.clone(),
                reason: format!(
                    "all targets, including the commit point, contain the new generation, but the committed marker could not be persisted: {commit_marker_error}"
                ),
            });
        }
        if let Err(cleanup_error) = self.working.cleanup() {
            return Err(TransactionError::Recovery {
                evidence: self.working.transaction_dir.clone(),
                reason: format!(
                    "new generation is committed, but recovery cleanup failed: {cleanup_error}"
                ),
            });
        }
        Ok(())
    }

    fn publish_all(&mut self) -> Result<(), TransactionError> {
        for index in self.working.state.next_index..self.working.state.entries.len() {
            self.working.verify_publication_frontier(index)?;
            self.working
                .checkpoint(Checkpoint::CommitBefore { index })?;
            self.verify_unpublished_generations(index)?;
            let prior_generation = self.prior_generations[index].clone();
            self.working.publish_entry(index, &prior_generation)?;
            self.working.checkpoint(Checkpoint::CommitAfter { index })?;
            self.working.state.next_index = index + 1;
            self.working.persist()?;
        }
        Ok(())
    }

    fn verify_unpublished_generations(&self, start: usize) -> Result<(), TransactionError> {
        for index in start..self.working.state.entries.len() {
            let target = self.working.target_path(index)?;
            self.working.verify_target_generation(
                &target,
                &self.prior_generations[index],
                "publication target generation changed before commit",
            )?;
        }
        Ok(())
    }

    fn rollback_after_commit_failure(
        mut self,
        commit_error: TransactionError,
    ) -> Result<(), TransactionError> {
        self.working.state.phase = JournalPhase::RollingBack;
        if let Err(source) = self.working.persist() {
            return Err(self.working.publication_failure(
                format!(
                    "commit failed ({commit_error}); rollback state could not be persisted: {source}"
                ),
                true,
            ));
        }
        let rollback_errors = self.working.rollback_all_best_effort();
        let rollback_reason = if rollback_errors.is_empty() {
            format!("commit failed ({commit_error})")
        } else {
            format!(
                "commit failed ({commit_error}); rollback failures: {}",
                rollback_errors.join("; ")
            )
        };
        match self.working.generation_disposition() {
            Ok(GenerationDisposition::Old) => {}
            Ok(GenerationDisposition::New) => {
                return Err(TransactionError::recovery(
                    &self.working.transaction_dir,
                    format!("{rollback_reason}; all final targets still match the new generation"),
                ));
            }
            Ok(GenerationDisposition::MixedOrUnknown) | Err(_) => {
                return Err(TransactionError::PartialPublication {
                    evidence: self.working.transaction_dir.clone(),
                    reason: rollback_reason,
                });
            }
        }
        self.working.state.phase = JournalPhase::RolledBack;
        self.working.state.next_index = 0;
        if let Err(source) = self.working.persist() {
            return Err(TransactionError::Recovery {
                evidence: self.working.transaction_dir.clone(),
                reason: format!(
                    "{rollback_reason}; the old generation was restored but the rolled-back marker could not be persisted: {source}"
                ),
            });
        }
        if let Err(source) = self.working.cleanup() {
            return Err(TransactionError::Recovery {
                evidence: self.working.transaction_dir.clone(),
                reason: format!(
                    "{rollback_reason}; the old generation was restored but cleanup failed: {source}"
                ),
            });
        }
        Err(TransactionError::CommitRolledBack {
            root: self.working.context.root.clone(),
            reason: commit_error.to_string(),
        })
    }
}

struct RootContext {
    root: PathBuf,
    root_identity: same_file::Handle,
    lock_path: PathBuf,
    lock_identity: same_file::Handle,
    _lock_file: Arc<File>,
    #[cfg(unix)]
    root_device: u64,
    approved_root: Option<crate::output::ApprovedTransactionRoot>,
    checkpoint: CheckpointHook,
}

impl RootContext {
    #[cfg(test)]
    fn acquire(root: &Path, checkpoint: CheckpointHook) -> Result<Self, TransactionError> {
        Self::acquire_inner(root, None, checkpoint)
    }

    fn acquire_approved(
        root: &Path,
        approved_root: crate::output::ApprovedTransactionRoot,
        checkpoint: CheckpointHook,
    ) -> Result<Self, TransactionError> {
        Self::acquire_inner(root, Some(approved_root), checkpoint)
    }

    fn acquire_inner(
        root: &Path,
        approved_root: Option<crate::output::ApprovedTransactionRoot>,
        checkpoint: CheckpointHook,
    ) -> Result<Self, TransactionError> {
        let root = std::fs::canonicalize(root).map_err(|source| {
            TransactionError::operational("canonicalize transaction root", root, source)
        })?;
        let root_metadata = std::fs::symlink_metadata(&root).map_err(|source| {
            TransactionError::operational("inspect transaction root", &root, source)
        })?;
        if !root_metadata.file_type().is_dir() {
            return Err(TransactionError::invalid_state(
                &root,
                "canonical transaction root is not a directory",
            ));
        }
        let root_identity = same_file::Handle::from_path(&root).map_err(|source| {
            TransactionError::operational("inspect transaction root identity", &root, source)
        })?;
        let lock_path = root.join(LOCK_FILE_NAME);
        let (lock_file, created) = open_stable_lock_file(&lock_path)?;
        let lock_identity =
            same_file::Handle::from_file(lock_file.try_clone().map_err(|source| {
                TransactionError::operational("clone transaction lock handle", &lock_path, source)
            })?)
            .map_err(|source| {
                TransactionError::operational("inspect transaction lock handle", &lock_path, source)
            })?;
        let path_identity = same_file::Handle::from_path(&lock_path).map_err(|source| {
            TransactionError::operational("inspect transaction lock identity", &lock_path, source)
        })?;
        if lock_identity != path_identity {
            return Err(TransactionError::invalid_state(
                &lock_path,
                "transaction lock identity changed while it was opened",
            ));
        }
        match lock_file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                return Err(TransactionError::Contended { lock_path });
            }
            Err(TryLockError::Error(source)) => {
                return Err(TransactionError::operational(
                    "acquire transaction lock",
                    &lock_path,
                    source,
                ));
            }
        }
        if created {
            lock_file.sync_all().map_err(|source| {
                TransactionError::operational("sync transaction lock", &lock_path, source)
            })?;
            sync_directory(&root)?;
        }
        let context = Self {
            root,
            root_identity,
            lock_path,
            lock_identity,
            _lock_file: Arc::new(lock_file),
            #[cfg(unix)]
            root_device: {
                use std::os::unix::fs::MetadataExt;
                root_metadata.dev()
            },
            approved_root,
            checkpoint,
        };
        context.verify_root_and_lock()?;
        Ok(context)
    }

    fn verify_root_and_lock(&self) -> Result<(), TransactionError> {
        if let Some(approved) = &self.approved_root {
            approved.verify().map_err(|error| {
                TransactionError::invalid_state(
                    approved.path(),
                    format!("approved transaction root identity changed: {error}"),
                )
            })?;
        }
        let root_metadata = std::fs::symlink_metadata(&self.root).map_err(|source| {
            TransactionError::operational("reinspect transaction root", &self.root, source)
        })?;
        if !root_metadata.file_type().is_dir() {
            return Err(TransactionError::invalid_state(
                &self.root,
                "transaction root became a symlink or non-directory",
            ));
        }
        let current_root = same_file::Handle::from_path(&self.root).map_err(|source| {
            TransactionError::operational("reinspect transaction root identity", &self.root, source)
        })?;
        if current_root != self.root_identity {
            return Err(TransactionError::invalid_state(
                &self.root,
                "transaction root identity changed",
            ));
        }
        let lock_metadata = std::fs::symlink_metadata(&self.lock_path).map_err(|source| {
            TransactionError::operational("reinspect transaction lock", &self.lock_path, source)
        })?;
        if !lock_metadata.file_type().is_file() {
            return Err(TransactionError::invalid_state(
                &self.lock_path,
                "transaction lock became a symlink or non-regular file",
            ));
        }
        verify_private_file_mode(&self.lock_path, &lock_metadata)?;
        let current_lock = same_file::Handle::from_path(&self.lock_path).map_err(|source| {
            TransactionError::operational(
                "reinspect transaction lock identity",
                &self.lock_path,
                source,
            )
        })?;
        if current_lock != self.lock_identity {
            return Err(TransactionError::invalid_state(
                &self.lock_path,
                "transaction lock identity changed",
            ));
        }
        Ok(())
    }

    fn verify_transaction_filesystem(
        &self,
        path: &Path,
        metadata: &std::fs::Metadata,
    ) -> Result<(), TransactionError> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if metadata.dev() != self.root_device {
                return Err(TransactionError::invalid_state(
                    path,
                    "transaction-owned directory crosses a nested filesystem",
                ));
            }
        }
        if let Some(approved) = &self.approved_root {
            approved.verify_same_filesystem(path).map_err(|error| {
                TransactionError::invalid_state(
                    path,
                    format!("transaction-owned directory filesystem changed: {error}"),
                )
            })?;
        }
        Ok(())
    }

    fn checkpoint(&self, checkpoint: Checkpoint) -> Result<(), TransactionError> {
        self.checkpoint.run(checkpoint).map_err(|source| {
            TransactionError::operational("run transaction checkpoint", &self.root, source)
        })
    }
}

struct WorkingTransaction {
    context: RootContext,
    transaction_dir: PathBuf,
    transaction_identity: Arc<same_file::Handle>,
    state: JournalState,
    active_slot: Option<StateSlot>,
    expected_generations: Vec<Option<TargetGeneration>>,
}

impl WorkingTransaction {
    fn checkpoint(&self, checkpoint: Checkpoint) -> Result<(), TransactionError> {
        self.context.checkpoint(checkpoint)
    }

    fn verify_owned_state(&self) -> Result<(), TransactionError> {
        self.context.verify_root_and_lock()?;
        let metadata = verify_private_directory(&self.transaction_dir, &self.transaction_identity)?;
        self.context
            .verify_transaction_filesystem(&self.transaction_dir, &metadata)
    }

    fn verify_target_generation(
        &self,
        target: &Path,
        expected: &TargetGeneration,
        reason: &'static str,
    ) -> Result<(), TransactionError> {
        let current = self.inspect_target_identity(target)?;
        if expected.matches_identity(current.as_ref()) {
            return Ok(());
        }
        Err(TransactionError::invalid_state(
            &self.transaction_dir,
            format!("{reason}: {target:?}"),
        ))
    }

    fn inspect_target_identity(
        &self,
        target: &Path,
    ) -> Result<Option<same_file::Handle>, TransactionError> {
        match self.inspect_target(target)? {
            TargetPresence::Missing => Ok(None),
            TargetPresence::Regular => {
                let identity = same_file::Handle::from_path(target).map_err(|source| {
                    TransactionError::operational(
                        "inspect publication target generation",
                        target,
                        source,
                    )
                })?;
                Ok(Some(identity))
            }
        }
    }

    fn validate_target_set(&self) -> Result<(), TransactionError> {
        validate_entries(&self.state.entries, &self.transaction_dir)?;
        self.verify_owned_state()?;
        let mut existing_identities = Vec::new();
        for index in 0..self.state.entries.len() {
            let target = self.target_path(index)?;
            self.verify_target_ancestors(&target)?;
            if let Some(expected) = &self.expected_generations[index] {
                self.verify_target_generation(
                    &target,
                    expected,
                    "publication target changed before transaction staging",
                )?;
            }
            if self.inspect_target(&target)? == TargetPresence::Regular {
                let identity = publication_target_identity(&target)?;
                if existing_identities.contains(&identity) {
                    return Err(TransactionError::invalid_state(
                        &self.transaction_dir,
                        format!("transaction targets resolve to the same file: {target:?}"),
                    ));
                }
                existing_identities.push(identity);
            }
        }
        Ok(())
    }

    fn target_path(&self, index: usize) -> Result<PathBuf, TransactionError> {
        self.state.entries[index].target.to_path(&self.context.root)
    }

    fn stage_path(&self, index: usize) -> PathBuf {
        self.transaction_dir.join(stage_name(index))
    }

    fn backup_path(&self, index: usize) -> PathBuf {
        self.transaction_dir.join(backup_name(index))
    }

    fn put_path(&self, index: usize) -> PathBuf {
        self.transaction_dir.join(put_name(index))
    }

    fn persist(&mut self) -> Result<(), TransactionError> {
        self.verify_owned_state()?;
        let target_slot = self.active_slot.map_or(StateSlot::A, StateSlot::other);
        let mut next_state = self.state.clone();
        next_state.sequence = next_state.sequence.checked_add(1).ok_or_else(|| {
            TransactionError::invalid_state(
                &self.transaction_dir,
                "transaction journal sequence overflowed",
            )
        })?;
        let bytes = next_state.encode(&self.transaction_dir)?;
        if bytes.len() as u64 > MAX_STATE_BYTES {
            return Err(TransactionError::invalid_state(
                &self.transaction_dir,
                "transaction journal exceeds the hard state-size limit",
            ));
        }
        self.checkpoint(Checkpoint::PersistBefore { slot: target_slot })?;
        let pending = self.transaction_dir.join(STATE_PENDING_NAME);
        self.verify_internal_missing(&pending)?;
        write_new_private_file(&pending, &bytes, "write pending transaction journal")?;
        self.checkpoint(Checkpoint::PersistPrepared { slot: target_slot })?;
        promote_pending_journal(&self.transaction_dir, target_slot)?;
        self.state = next_state;
        self.active_slot = Some(target_slot);
        self.checkpoint(Checkpoint::PersistAfter { slot: target_slot })?;
        Ok(())
    }

    fn verify_internal_regular(&self, path: &Path) -> Result<(), TransactionError> {
        self.verify_owned_state()?;
        let metadata = std::fs::symlink_metadata(path).map_err(|source| {
            TransactionError::operational("inspect transaction-owned file", path, source)
        })?;
        if !metadata.file_type().is_file() {
            return Err(TransactionError::invalid_state(
                &self.transaction_dir,
                format!("transaction-owned path is a symlink or non-regular file: {path:?}"),
            ));
        }
        verify_private_file_mode(path, &metadata)?;
        let opened = File::open(path).map_err(|source| {
            TransactionError::operational("open transaction-owned file", path, source)
        })?;
        let opened_identity = same_file::Handle::from_file(opened).map_err(|source| {
            TransactionError::operational("inspect transaction-owned file handle", path, source)
        })?;
        let path_identity = same_file::Handle::from_path(path).map_err(|source| {
            TransactionError::operational("inspect transaction-owned file identity", path, source)
        })?;
        if opened_identity != path_identity {
            return Err(TransactionError::invalid_state(
                &self.transaction_dir,
                format!("transaction-owned file identity changed: {path:?}"),
            ));
        }
        Ok(())
    }

    fn verify_internal_missing(&self, path: &Path) -> Result<(), TransactionError> {
        match std::fs::symlink_metadata(path) {
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Ok(_) => Err(TransactionError::invalid_state(
                &self.transaction_dir,
                format!("unexpected transaction-owned file exists: {path:?}"),
            )),
            Err(source) => Err(TransactionError::operational(
                "inspect transaction-owned path",
                path,
                source,
            )),
        }
    }

    fn verify_target_ancestors(&self, target: &Path) -> Result<(), TransactionError> {
        self.context.verify_root_and_lock()?;
        let relative = target.strip_prefix(&self.context.root).map_err(|_| {
            TransactionError::invalid_state(
                &self.transaction_dir,
                format!("transaction target escaped the canonical root: {target:?}"),
            )
        })?;
        let mut current = self.context.root.clone();
        let component_count = relative.components().count();
        for (position, component) in relative.components().enumerate() {
            if position + 1 == component_count {
                break;
            }
            current.push(component.as_os_str());
            let metadata = std::fs::symlink_metadata(&current).map_err(|source| {
                TransactionError::operational(
                    "inspect transaction target ancestor",
                    &current,
                    source,
                )
            })?;
            if !metadata.file_type().is_dir() {
                return Err(TransactionError::invalid_state(
                    &self.transaction_dir,
                    format!(
                        "transaction target ancestor is a symlink or non-directory: {current:?}"
                    ),
                ));
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if metadata.dev() != self.context.root_device {
                    return Err(TransactionError::invalid_state(
                        &self.transaction_dir,
                        format!("transaction target crosses a nested filesystem: {current:?}"),
                    ));
                }
            }
        }
        Ok(())
    }

    fn inspect_target(&self, target: &Path) -> Result<TargetPresence, TransactionError> {
        self.verify_target_ancestors(target)?;
        match std::fs::symlink_metadata(target) {
            Ok(metadata) if metadata.file_type().is_file() => {
                #[cfg(unix)]
                let aliases_lock = {
                    use std::os::unix::fs::MetadataExt;

                    let lock_metadata = self.context._lock_file.metadata().map_err(|source| {
                        TransactionError::operational(
                            "inspect transaction lock identity",
                            &self.context.lock_path,
                            source,
                        )
                    })?;
                    metadata.dev() == lock_metadata.dev() && metadata.ino() == lock_metadata.ino()
                };
                #[cfg(not(unix))]
                let aliases_lock = {
                    let identity = same_file::Handle::from_path(target).map_err(|source| {
                        TransactionError::operational(
                            "inspect publication target identity",
                            target,
                            source,
                        )
                    })?;
                    identity == self.context.lock_identity
                };
                if aliases_lock {
                    return Err(TransactionError::invalid_state(
                        &self.transaction_dir,
                        format!("transaction target aliases the stable lock file: {target:?}"),
                    ));
                }
                Ok(TargetPresence::Regular)
            }
            Ok(_) => Err(TransactionError::invalid_state(
                &self.transaction_dir,
                format!("transaction target is a symlink or non-regular file: {target:?}"),
            )),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                Ok(TargetPresence::Missing)
            }
            Err(source) => Err(TransactionError::operational(
                "inspect transaction target",
                target,
                source,
            )),
        }
    }

    fn classify_all(&self) -> Result<Vec<GenerationMatch>, TransactionError> {
        self.verify_owned_state()?;
        self.validate_recovery_files(true)?;
        (0..self.state.entries.len())
            .map(|index| self.classify_entry(index))
            .collect()
    }

    fn generation_disposition(&self) -> Result<GenerationDisposition, TransactionError> {
        self.classify_all()
            .map(|generations| GenerationDisposition::from_matches(&generations))
    }

    fn publication_failure(&self, reason: String, uncertain_on_error: bool) -> TransactionError {
        match self.generation_disposition() {
            Ok(GenerationDisposition::MixedOrUnknown) => TransactionError::PartialPublication {
                evidence: self.transaction_dir.clone(),
                reason,
            },
            Ok(GenerationDisposition::Old | GenerationDisposition::New) => {
                TransactionError::recovery(&self.transaction_dir, reason)
            }
            Err(source) if uncertain_on_error => TransactionError::PartialPublication {
                evidence: self.transaction_dir.clone(),
                reason: format!("{reason}; final generation could not be classified: {source}"),
            },
            Err(source) => TransactionError::recovery(
                &self.transaction_dir,
                format!("{reason}; final generation could not be classified: {source}"),
            ),
        }
    }

    fn verify_publication_frontier(&self, next_index: usize) -> Result<(), TransactionError> {
        let generations = self.classify_all()?;
        let valid = generations.iter().enumerate().all(|(index, generation)| {
            if index < next_index {
                generation.is_new()
            } else {
                generation.is_old()
            }
        });
        if valid {
            Ok(())
        } else {
            Err(TransactionError::recovery(
                &self.transaction_dir,
                format!(
                    "publication targets no longer match the journal frontier at entry {next_index}"
                ),
            ))
        }
    }

    fn classify_entry(&self, index: usize) -> Result<GenerationMatch, TransactionError> {
        let entry = &self.state.entries[index];
        let target = self.target_path(index)?;
        let presence = self.inspect_target(&target)?;
        if entry.prior == PriorState::Present && presence == TargetPresence::Regular {
            let metadata = std::fs::symlink_metadata(&target).map_err(|source| {
                TransactionError::operational(
                    "inspect publication target permissions",
                    &target,
                    source,
                )
            })?;
            if ordinary_file_mode(&target, &metadata)? != entry.prior_mode {
                return Ok(GenerationMatch::ModeMismatch);
            }
        }
        let matches_old_bytes = match entry.prior {
            PriorState::Unknown => {
                return Err(TransactionError::invalid_state(
                    &self.transaction_dir,
                    "cannot classify a target without its prior state",
                ));
            }
            PriorState::Missing => presence == TargetPresence::Missing,
            PriorState::Present => {
                presence == TargetPresence::Regular
                    && files_equal(&target, &self.backup_path(index), &self.transaction_dir)?
            }
        };
        let matches_new_bytes = match entry.operation {
            TransactionOperation::Write => {
                presence == TargetPresence::Regular
                    && files_equal(&target, &self.stage_path(index), &self.transaction_dir)?
            }
            TransactionOperation::Delete => presence == TargetPresence::Missing,
        };
        Ok(match (matches_old_bytes, matches_new_bytes) {
            (true, true) => GenerationMatch::Both,
            (true, false) => GenerationMatch::Old,
            (false, true) => GenerationMatch::New,
            (false, false) => GenerationMatch::Unknown,
        })
    }

    fn publish_entry(
        &mut self,
        index: usize,
        prior_generation: &TargetGeneration,
    ) -> Result<(), TransactionError> {
        let target = self.target_path(index)?;
        match self.state.entries[index].operation {
            TransactionOperation::Write => {
                deterministic_replace_from(
                    self,
                    index,
                    &self.stage_path(index),
                    &target,
                    Some(prior_generation),
                    "publish transaction entry",
                )?;
            }
            TransactionOperation::Delete => {
                remove_regular_if_present(
                    self,
                    index,
                    &target,
                    Some(prior_generation),
                    "publish artifact deletion",
                )?;
            }
        }
        let generation = self.classify_entry(index)?;
        if !generation.is_new() {
            return Err(TransactionError::recovery(
                &self.transaction_dir,
                format!("published target does not match retained staged bytes: {target:?}"),
            ));
        }
        Ok(())
    }

    fn rollback_entry(&mut self, index: usize) -> Result<(), TransactionError> {
        let target = self.target_path(index)?;
        match self.state.entries[index].prior {
            PriorState::Present => {
                deterministic_replace_from(
                    self,
                    index,
                    &self.backup_path(index),
                    &target,
                    None,
                    "restore transaction backup",
                )?;
            }
            PriorState::Missing => {
                remove_regular_if_present(
                    self,
                    index,
                    &target,
                    None,
                    "remove newly published target",
                )?;
            }
            PriorState::Unknown => {
                return Err(TransactionError::invalid_state(
                    &self.transaction_dir,
                    "cannot roll back an entry without its prior state",
                ));
            }
        }
        let generation = self.classify_entry(index)?;
        if !generation.is_old() {
            return Err(TransactionError::recovery(
                &self.transaction_dir,
                format!("rolled-back target does not match retained prior bytes: {target:?}"),
            ));
        }
        Ok(())
    }

    fn rollback_all_best_effort(&mut self) -> Vec<String> {
        let generations = match self.classify_all() {
            Ok(generations) => generations,
            Err(source) => return vec![source.to_string()],
        };
        if generations.contains(&GenerationMatch::Unknown) {
            return vec![
                "at least one final target matches neither retained generation".to_owned(),
            ];
        }
        let mut errors = Vec::new();
        for index in (0..self.state.entries.len()).rev() {
            if generations[index].is_old() {
                continue;
            }
            if let Err(source) = self.checkpoint(Checkpoint::RollbackBefore { index }) {
                errors.push(source.to_string());
                continue;
            }
            if let Err(source) = self.rollback_entry(index) {
                errors.push(source.to_string());
                continue;
            }
            if let Err(source) = self.checkpoint(Checkpoint::RollbackAfter { index }) {
                errors.push(source.to_string());
            }
            self.state.next_index = index;
            if let Err(source) = self.persist() {
                errors.push(source.to_string());
            }
        }
        match self.classify_all() {
            Ok(current) if current.iter().all(|generation| generation.is_old()) => {}
            Ok(_) => errors.push("rollback left a mixed target generation".to_owned()),
            Err(source) => errors.push(source.to_string()),
        }
        errors
    }

    fn validate_recovery_files(&self, require_complete: bool) -> Result<(), TransactionError> {
        self.verify_owned_state()?;
        let expected = self.expected_internal_names();
        let put_names = (0..self.state.entries.len())
            .map(put_name)
            .collect::<HashSet<_>>();
        for entry in std::fs::read_dir(&self.transaction_dir).map_err(|source| {
            TransactionError::operational(
                "read transaction directory",
                &self.transaction_dir,
                source,
            )
        })? {
            let entry = entry.map_err(|source| {
                TransactionError::operational(
                    "read transaction directory entry",
                    &self.transaction_dir,
                    source,
                )
            })?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                return Err(TransactionError::invalid_state(
                    &self.transaction_dir,
                    "transaction directory contains a non-UTF-8 entry name",
                ));
            };
            if !expected.contains(name) {
                return Err(TransactionError::invalid_state(
                    &self.transaction_dir,
                    format!("transaction directory contains unknown entry {name:?}"),
                ));
            }
            let metadata = std::fs::symlink_metadata(entry.path()).map_err(|source| {
                TransactionError::operational(
                    "inspect transaction directory entry",
                    entry.path(),
                    source,
                )
            })?;
            if !metadata.file_type().is_file() {
                return Err(TransactionError::invalid_state(
                    &self.transaction_dir,
                    format!(
                        "transaction directory entry is a symlink or non-regular file: {:?}",
                        entry.path()
                    ),
                ));
            }
            if put_names.contains(name) {
                verify_private_file_link_count(&entry.path(), &metadata)?;
            } else {
                verify_private_file_mode(&entry.path(), &metadata)?;
            }
        }
        for (index, entry) in self.state.entries.iter().enumerate() {
            let stage = self.stage_path(index);
            let backup = self.backup_path(index);
            if require_complete && entry.operation == TransactionOperation::Write {
                self.verify_internal_regular(&stage)?;
            }
            if require_complete && entry.operation == TransactionOperation::Delete {
                self.verify_internal_missing(&stage)?;
            }
            match entry.prior {
                PriorState::Present if require_complete => self.verify_internal_regular(&backup)?,
                PriorState::Missing if require_complete => self.verify_internal_missing(&backup)?,
                _ => {}
            }
        }
        Ok(())
    }

    fn expected_internal_names(&self) -> HashSet<String> {
        let mut expected = HashSet::from([
            STATE_A_NAME.to_owned(),
            STATE_B_NAME.to_owned(),
            STATE_PENDING_NAME.to_owned(),
        ]);
        for index in 0..self.state.entries.len() {
            expected.insert(stage_name(index));
            expected.insert(backup_name(index));
            expected.insert(put_name(index));
        }
        expected
    }

    fn cleanup(&mut self) -> Result<(), TransactionError> {
        if matches!(
            self.state.phase,
            JournalPhase::Publishing | JournalPhase::RollingBack
        ) {
            return Err(TransactionError::invalid_state(
                &self.transaction_dir,
                "cannot clean transaction evidence before a terminal generation is recorded",
            ));
        }
        self.validate_recovery_files(false)?;
        self.checkpoint(Checkpoint::CleanupBefore)?;
        let mut paths = Vec::new();
        for index in 0..self.state.entries.len() {
            paths.push(self.stage_path(index));
            paths.push(self.backup_path(index));
            paths.push(self.put_path(index));
        }
        paths.push(self.transaction_dir.join(STATE_PENDING_NAME));
        let inactive = self.active_slot.map(StateSlot::other);
        if let Some(slot) = inactive {
            paths.push(self.transaction_dir.join(slot.file_name()));
        }
        if let Some(slot) = self.active_slot {
            paths.push(self.transaction_dir.join(slot.file_name()));
        } else {
            paths.push(self.transaction_dir.join(STATE_A_NAME));
            paths.push(self.transaction_dir.join(STATE_B_NAME));
        }
        for (ordinal, path) in paths.into_iter().enumerate() {
            self.checkpoint(Checkpoint::CleanupFileBefore { ordinal })?;
            remove_owned_regular_if_present(&self.transaction_dir, &path)?;
            sync_directory(&self.transaction_dir)?;
            self.checkpoint(Checkpoint::CleanupFileAfter { ordinal })?;
        }
        std::fs::remove_dir(&self.transaction_dir).map_err(|source| {
            TransactionError::operational(
                "remove empty transaction directory",
                &self.transaction_dir,
                source,
            )
        })?;
        sync_directory(&self.context.root)?;
        self.checkpoint(Checkpoint::CleanupAfter)?;
        Ok(())
    }
}

fn recover_existing_transaction(context: RootContext) -> Result<RootContext, TransactionError> {
    context.verify_root_and_lock()?;
    let transaction_dir = context.root.join(TRANSACTION_DIR_NAME);
    match std::fs::symlink_metadata(&transaction_dir) {
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(context),
        Err(source) => {
            return Err(TransactionError::operational(
                "inspect recovery transaction directory",
                &transaction_dir,
                source,
            ));
        }
        Ok(metadata) if !metadata.file_type().is_dir() => {
            return Err(TransactionError::invalid_state(
                &transaction_dir,
                "reserved transaction path is a symlink or non-directory",
            ));
        }
        Ok(_) => {}
    }
    let transaction_identity = Arc::new(same_file::Handle::from_path(&transaction_dir).map_err(
        |source| {
            TransactionError::operational(
                "inspect recovery transaction directory identity",
                &transaction_dir,
                source,
            )
        },
    )?);
    let transaction_metadata = verify_private_directory(&transaction_dir, &transaction_identity)?;
    context.verify_transaction_filesystem(&transaction_dir, &transaction_metadata)?;
    context.checkpoint(Checkpoint::RecoveryBefore)?;
    if directory_is_empty(&transaction_dir)? {
        std::fs::remove_dir(&transaction_dir).map_err(|source| {
            TransactionError::operational(
                "remove empty recovered transaction directory",
                &transaction_dir,
                source,
            )
        })?;
        sync_directory(&context.root)?;
        context.checkpoint(Checkpoint::RecoveryAfter)?;
        return Ok(context);
    }
    resolve_pending_journal(&transaction_dir)?;
    if directory_is_empty(&transaction_dir)? {
        std::fs::remove_dir(&transaction_dir).map_err(|source| {
            TransactionError::operational(
                "remove recovered bootstrap transaction directory",
                &transaction_dir,
                source,
            )
        })?;
        sync_directory(&context.root)?;
        context.checkpoint(Checkpoint::RecoveryAfter)?;
        return Ok(context);
    }
    let (state, active_slot) = load_journal_slots(&transaction_dir)?;
    let mut working = WorkingTransaction {
        context,
        transaction_dir,
        transaction_identity,
        expected_generations: vec![None; state.entries.len()],
        state,
        active_slot: Some(active_slot),
    };
    if matches!(
        working.state.phase,
        JournalPhase::Publishing | JournalPhase::RollingBack
    ) {
        working.validate_target_set()?;
    } else {
        validate_entries(&working.state.entries, &working.transaction_dir)?;
        working.verify_owned_state()?;
    }
    let require_complete = matches!(
        working.state.phase,
        JournalPhase::Publishing | JournalPhase::RollingBack
    );
    working.validate_recovery_files(require_complete)?;

    let recovery_result = match working.state.phase {
        JournalPhase::Staging | JournalPhase::BackingUp => working.cleanup(),
        JournalPhase::Publishing => recover_publishing(&mut working),
        JournalPhase::RollingBack => recover_rolling_back(&mut working),
        JournalPhase::RolledBack => recover_rolled_back(&mut working),
        JournalPhase::Committed => recover_committed(&mut working),
    };
    if let Err(source) = recovery_result {
        return Err(match source {
            TransactionError::InvalidState { .. } => source,
            other => TransactionError::recovery(
                &working.transaction_dir,
                format!("interrupted transaction recovery failed: {other}"),
            ),
        });
    }
    working.context.checkpoint(Checkpoint::RecoveryAfter)?;
    Ok(working.context)
}

fn recover_publishing(working: &mut WorkingTransaction) -> Result<(), TransactionError> {
    let generations = working.classify_all()?;
    if generations.contains(&GenerationMatch::Unknown) {
        return Err(TransactionError::PartialPublication {
            evidence: working.transaction_dir.clone(),
            reason: "a final target matches neither retained generation".to_owned(),
        });
    }
    let commit_point = generations
        .last()
        .copied()
        .expect("validated transactions always contain a manifest");
    let all_new = generations.iter().all(|generation| generation.is_new());
    if all_new {
        working.state.phase = JournalPhase::Committed;
        working.state.next_index = working.state.entries.len();
        working.persist()?;
        working.cleanup()
    } else if commit_point == GenerationMatch::New {
        Err(TransactionError::PartialPublication {
            evidence: working.transaction_dir.clone(),
            reason: "commit point is new while an earlier transaction target is not".to_owned(),
        })
    } else {
        working.state.phase = JournalPhase::RollingBack;
        working.persist()?;
        recover_rolling_back(working)
    }
}

fn recover_rolling_back(working: &mut WorkingTransaction) -> Result<(), TransactionError> {
    let errors = working.rollback_all_best_effort();
    let reason = if errors.is_empty() {
        "interrupted rollback did not restore a complete generation".to_owned()
    } else {
        errors.join("; ")
    };
    match working.generation_disposition() {
        Ok(GenerationDisposition::Old) => {}
        Ok(GenerationDisposition::New) => {
            return Err(TransactionError::Recovery {
                evidence: working.transaction_dir.clone(),
                reason: format!("{reason}; all final targets still match the new generation"),
            });
        }
        Ok(GenerationDisposition::MixedOrUnknown) | Err(_) => {
            return Err(TransactionError::PartialPublication {
                evidence: working.transaction_dir.clone(),
                reason,
            });
        }
    }
    working.state.phase = JournalPhase::RolledBack;
    working.state.next_index = 0;
    working
        .persist()
        .map_err(|source| TransactionError::Recovery {
            evidence: working.transaction_dir.clone(),
            reason: format!("old generation was restored but terminal marker failed: {source}"),
        })?;
    working.cleanup()
}

fn recover_rolled_back(working: &mut WorkingTransaction) -> Result<(), TransactionError> {
    working.cleanup()
}

fn recover_committed(working: &mut WorkingTransaction) -> Result<(), TransactionError> {
    working.cleanup()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum StateSlot {
    A,
    B,
}

impl StateSlot {
    const fn other(self) -> Self {
        match self {
            Self::A => Self::B,
            Self::B => Self::A,
        }
    }

    const fn file_name(self) -> &'static str {
        match self {
            Self::A => STATE_A_NAME,
            Self::B => STATE_B_NAME,
        }
    }
}

fn load_journal_slots(
    transaction_dir: &Path,
) -> Result<(JournalState, StateSlot), TransactionError> {
    load_journal_slots_optional(transaction_dir)?.ok_or_else(|| {
        TransactionError::invalid_state(
            transaction_dir,
            "non-empty transaction directory has no journal slot",
        )
    })
}

fn load_journal_slots_optional(
    transaction_dir: &Path,
) -> Result<Option<(JournalState, StateSlot)>, TransactionError> {
    let a = load_journal_slot(transaction_dir, StateSlot::A)?;
    let b = load_journal_slot(transaction_dir, StateSlot::B)?;
    match (a, b) {
        (JournalRead::Valid(a), JournalRead::Valid(b)) => match a.sequence.cmp(&b.sequence) {
            std::cmp::Ordering::Greater => {
                b.validate_successor(&a, transaction_dir)?;
                Ok(Some((a, StateSlot::A)))
            }
            std::cmp::Ordering::Less => {
                a.validate_successor(&b, transaction_dir)?;
                Ok(Some((b, StateSlot::B)))
            }
            std::cmp::Ordering::Equal => Err(TransactionError::invalid_state(
                transaction_dir,
                "journal slots have the same sequence",
            )),
        },
        (JournalRead::Valid(state), JournalRead::Missing) => Ok(Some((state, StateSlot::A))),
        (JournalRead::Missing, JournalRead::Valid(state)) => Ok(Some((state, StateSlot::B))),
        (JournalRead::Missing, JournalRead::Missing) => Ok(None),
        (JournalRead::EofTruncated, _) | (_, JournalRead::EofTruncated) => {
            unreachable!("journal slots never permit truncated input")
        }
    }
}

enum JournalRead {
    Missing,
    EofTruncated,
    Valid(JournalState),
}

fn load_journal_slot(
    transaction_dir: &Path,
    slot: StateSlot,
) -> Result<JournalRead, TransactionError> {
    let path = transaction_dir.join(slot.file_name());
    load_journal_file(&path, transaction_dir, false)
}

fn load_journal_file(
    path: &Path,
    transaction_dir: &Path,
    permit_eof_truncation: bool,
) -> Result<JournalRead, TransactionError> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            return Ok(JournalRead::Missing);
        }
        Err(source) => {
            return Err(TransactionError::operational(
                "inspect transaction journal slot",
                path,
                source,
            ));
        }
    };
    if !metadata.file_type().is_file() {
        return Err(TransactionError::invalid_state(
            transaction_dir,
            format!("journal slot is a symlink or non-regular file: {path:?}"),
        ));
    }
    verify_private_file_mode(path, &metadata)?;
    let mut file = File::open(path).map_err(|source| {
        TransactionError::operational("open transaction journal slot", path, source)
    })?;
    verify_open_regular_identity(path, &file, "transaction journal slot")?;
    let bytes = read_at_most(&mut file, MAX_STATE_BYTES, path)?;
    let value = match serde_json::from_slice::<serde_json::Value>(&bytes) {
        Ok(value) => value,
        Err(source)
            if permit_eof_truncation && source.classify() == serde_json::error::Category::Eof =>
        {
            return Ok(JournalRead::EofTruncated);
        }
        Err(source) => {
            return Err(TransactionError::invalid_state(
                transaction_dir,
                format!("transaction journal JSON is malformed at {path:?}: {source}"),
            ));
        }
    };
    Ok(JournalRead::Valid(JournalState::decode_json_value(
        value,
        transaction_dir,
    )?))
}

fn resolve_pending_journal(transaction_dir: &Path) -> Result<(), TransactionError> {
    let pending_path = transaction_dir.join(STATE_PENDING_NAME);
    let pending = load_journal_file(&pending_path, transaction_dir, true)?;
    match pending {
        JournalRead::Missing => Ok(()),
        JournalRead::EofTruncated => {
            if load_journal_slots_optional(transaction_dir)?.is_none() {
                verify_bootstrap_contains_only_pending(transaction_dir)?;
            }
            remove_owned_regular_if_present(transaction_dir, &pending_path)?;
            sync_directory(transaction_dir)
        }
        JournalRead::Valid(pending_state) => {
            let target_slot = if let Some((current, active_slot)) =
                load_journal_slots_optional(transaction_dir)?
            {
                current.validate_successor(&pending_state, transaction_dir)?;
                active_slot.other()
            } else {
                verify_bootstrap_contains_only_pending(transaction_dir)?;
                if pending_state.sequence != 1 || pending_state.phase != JournalPhase::Staging {
                    return Err(TransactionError::invalid_state(
                        transaction_dir,
                        "bootstrap pending journal is not the initial staging state",
                    ));
                }
                StateSlot::A
            };
            promote_pending_journal(transaction_dir, target_slot)
        }
    }
}

fn verify_bootstrap_contains_only_pending(transaction_dir: &Path) -> Result<(), TransactionError> {
    for entry in std::fs::read_dir(transaction_dir).map_err(|source| {
        TransactionError::operational(
            "read bootstrap transaction directory",
            transaction_dir,
            source,
        )
    })? {
        let entry = entry.map_err(|source| {
            TransactionError::operational(
                "read bootstrap transaction directory entry",
                transaction_dir,
                source,
            )
        })?;
        if entry.file_name() != std::ffi::OsStr::new(STATE_PENDING_NAME) {
            return Err(TransactionError::invalid_state(
                transaction_dir,
                format!(
                    "journal bootstrap contains unexpected entry {:?}",
                    entry.file_name()
                ),
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GenerationMatch {
    Old,
    New,
    Both,
    ModeMismatch,
    Unknown,
}

impl GenerationMatch {
    const fn is_old(self) -> bool {
        matches!(self, Self::Old | Self::Both)
    }

    const fn is_new(self) -> bool {
        matches!(self, Self::New | Self::Both)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GenerationDisposition {
    Old,
    New,
    MixedOrUnknown,
}

impl GenerationDisposition {
    fn from_matches(generations: &[GenerationMatch]) -> Self {
        if generations.iter().all(|generation| generation.is_old()) {
            Self::Old
        } else if generations.iter().all(|generation| generation.is_new()) {
            Self::New
        } else {
            Self::MixedOrUnknown
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TargetPresence {
    Missing,
    Regular,
}

#[cfg(unix)]
fn publication_target_identity(path: &Path) -> Result<(u64, u64), TransactionError> {
    use std::os::unix::fs::MetadataExt;

    let metadata = std::fs::symlink_metadata(path).map_err(|source| {
        TransactionError::operational("inspect transaction target identity", path, source)
    })?;
    Ok((metadata.dev(), metadata.ino()))
}

#[cfg(not(unix))]
fn publication_target_identity(path: &Path) -> Result<same_file::Handle, TransactionError> {
    same_file::Handle::from_path(path).map_err(|source| {
        TransactionError::operational("inspect transaction target identity", path, source)
    })
}

fn open_stable_lock_file(path: &Path) -> Result<(File, bool), TransactionError> {
    let mut create = OpenOptions::new();
    create.read(true).write(true).create_new(true);
    set_private_create_mode(&mut create);
    match create.open(path) {
        Ok(file) => {
            verify_open_regular_identity(path, &file, "transaction lock")?;
            verify_private_file_mode(
                path,
                &file.metadata().map_err(|source| {
                    TransactionError::operational("inspect transaction lock", path, source)
                })?,
            )?;
            Ok((file, true))
        }
        Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {
            let metadata = std::fs::symlink_metadata(path).map_err(|source| {
                TransactionError::operational("inspect transaction lock", path, source)
            })?;
            if !metadata.file_type().is_file() {
                return Err(TransactionError::invalid_state(
                    path,
                    "transaction lock is a symlink or non-regular file",
                ));
            }
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(path)
                .map_err(|source| {
                    TransactionError::operational("open transaction lock", path, source)
                })?;
            verify_open_regular_identity(path, &file, "transaction lock")?;
            verify_private_file_mode(
                path,
                &file.metadata().map_err(|source| {
                    TransactionError::operational("inspect transaction lock", path, source)
                })?,
            )?;
            Ok((file, false))
        }
        Err(source) => Err(TransactionError::operational(
            "create transaction lock",
            path,
            source,
        )),
    }
}

fn verify_open_regular_identity(
    path: &Path,
    file: &File,
    role: &str,
) -> Result<(), TransactionError> {
    let path_metadata = std::fs::symlink_metadata(path)
        .map_err(|source| TransactionError::operational("reinspect opened path", path, source))?;
    let file_metadata = file
        .metadata()
        .map_err(|source| TransactionError::operational("inspect opened file", path, source))?;
    if !path_metadata.file_type().is_file() || !file_metadata.is_file() {
        return Err(TransactionError::invalid_state(
            path,
            format!("{role} is a symlink or non-regular file"),
        ));
    }
    let opened = same_file::Handle::from_file(
        file.try_clone()
            .map_err(|source| TransactionError::operational("clone opened file", path, source))?,
    )
    .map_err(|source| {
        TransactionError::operational("inspect opened file identity", path, source)
    })?;
    let current = same_file::Handle::from_path(path).map_err(|source| {
        TransactionError::operational("inspect opened path identity", path, source)
    })?;
    if opened != current {
        return Err(TransactionError::invalid_state(
            path,
            format!("{role} identity changed while it was opened"),
        ));
    }
    Ok(())
}

fn create_private_directory(path: &Path) -> Result<(), TransactionError> {
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path).map_err(|source| {
        TransactionError::operational("create transaction directory", path, source)
    })?;
    Ok(())
}

fn verify_private_directory(
    path: &Path,
    expected: &same_file::Handle,
) -> Result<std::fs::Metadata, TransactionError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|source| {
        TransactionError::operational("inspect transaction directory", path, source)
    })?;
    if !metadata.file_type().is_dir() {
        return Err(TransactionError::invalid_state(
            path,
            "transaction directory is a symlink or non-directory",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(TransactionError::invalid_state(
                path,
                "transaction directory permissions are not owner-only",
            ));
        }
    }
    let current = same_file::Handle::from_path(path).map_err(|source| {
        TransactionError::operational("inspect transaction directory identity", path, source)
    })?;
    if &current != expected {
        return Err(TransactionError::invalid_state(
            path,
            "transaction directory identity changed",
        ));
    }
    Ok(metadata)
}

fn create_private_file(path: &Path) -> Result<File, TransactionError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    set_private_create_mode(&mut options);
    let file = options.open(path).map_err(|source| {
        TransactionError::operational("create transaction-owned file", path, source)
    })?;
    verify_open_regular_identity(path, &file, "transaction-owned file")?;
    verify_private_file_mode(
        path,
        &file.metadata().map_err(|source| {
            TransactionError::operational("inspect transaction-owned file", path, source)
        })?,
    )?;
    Ok(file)
}

fn create_put_file(path: &Path) -> Result<File, TransactionError> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| {
            TransactionError::operational("create deterministic publication file", path, source)
        })?;
    verify_open_put_identity(path, &file, None)?;
    Ok(file)
}

fn apply_ordinary_file_mode(
    file: &File,
    path: &Path,
    mode: Option<u32>,
) -> Result<(), TransactionError> {
    #[cfg(unix)]
    if let Some(mode) = mode {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(mode))
            .map_err(|source| {
                TransactionError::operational(
                    "apply ordinary publication permissions",
                    path,
                    source,
                )
            })?;
    }
    #[cfg(not(unix))]
    {
        let _ = (file, path, mode);
    }
    Ok(())
}

fn write_new_private_file(
    path: &Path,
    bytes: &[u8],
    operation: &'static str,
) -> Result<(), TransactionError> {
    let mut file = create_private_file(path)?;
    file.write_all(bytes)
        .map_err(|source| TransactionError::operational(operation, path, source))?;
    file.sync_all().map_err(|source| {
        TransactionError::operational("sync transaction-owned file", path, source)
    })?;
    if let Some(parent) = path.parent() {
        sync_directory(parent)?;
    }
    Ok(())
}

fn promote_pending_journal(
    transaction_dir: &Path,
    target_slot: StateSlot,
) -> Result<(), TransactionError> {
    let pending = transaction_dir.join(STATE_PENDING_NAME);
    verify_private_regular(&pending, transaction_dir)?;
    let target = transaction_dir.join(target_slot.file_name());
    match std::fs::symlink_metadata(&target) {
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(TransactionError::operational(
                "inspect transaction journal destination",
                &target,
                source,
            ));
        }
        Ok(metadata) if metadata.file_type().is_file() => {
            verify_private_file_mode(&target, &metadata)?;
        }
        Ok(_) => {
            return Err(TransactionError::invalid_state(
                transaction_dir,
                format!("journal destination is a symlink or non-regular file: {target:?}"),
            ));
        }
    }
    std::fs::rename(&pending, &target).map_err(|source| {
        TransactionError::operational("promote pending transaction journal", &target, source)
    })?;
    sync_directory(transaction_dir)
}

fn copy_regular_to_private(
    working: &WorkingTransaction,
    source: &Path,
    destination: &Path,
    operation: &'static str,
) -> Result<Option<u32>, TransactionError> {
    let mut source_file = open_verified_regular(source, &working.transaction_dir)?;
    let prior_mode = ordinary_file_mode(
        source,
        &source_file.metadata().map_err(|source_error| {
            TransactionError::operational("inspect backup source permissions", source, source_error)
        })?,
    )?;
    let source_identity =
        same_file::Handle::from_file(source_file.try_clone().map_err(|source_error| {
            TransactionError::operational("clone source for backup", source, source_error)
        })?)
        .map_err(|source_error| {
            TransactionError::operational("inspect backup source identity", source, source_error)
        })?;
    let mut destination_file = create_private_file(destination)?;
    std::io::copy(&mut source_file, &mut destination_file).map_err(|source_error| {
        TransactionError::operational(operation, destination, source_error)
    })?;
    destination_file.sync_all().map_err(|source_error| {
        TransactionError::operational("sync transaction backup", destination, source_error)
    })?;
    if let Some(parent) = destination.parent() {
        sync_directory(parent)?;
    }
    let current_source = same_file::Handle::from_path(source).map_err(|source_error| {
        TransactionError::operational("reinspect backup source identity", source, source_error)
    })?;
    let current_mode = ordinary_file_mode(
        source,
        &std::fs::symlink_metadata(source).map_err(|source_error| {
            TransactionError::operational(
                "reinspect backup source permissions",
                source,
                source_error,
            )
        })?,
    )?;
    if current_source != source_identity
        || current_mode != prior_mode
        || !files_equal(source, destination, &working.transaction_dir)?
    {
        return Err(TransactionError::invalid_state(
            &working.transaction_dir,
            format!("publication target changed while it was backed up: {source:?}"),
        ));
    }
    Ok(prior_mode)
}

fn deterministic_replace_from(
    working: &WorkingTransaction,
    index: usize,
    source: &Path,
    target: &Path,
    expected_target: Option<&TargetGeneration>,
    operation: &'static str,
) -> Result<(), TransactionError> {
    working.verify_owned_state()?;
    working.verify_internal_regular(source)?;
    let _ = working.inspect_target(target)?;
    let mut source_file = open_verified_regular(source, &working.transaction_dir)?;
    let source_identity =
        same_file::Handle::from_file(source_file.try_clone().map_err(|source_error| {
            TransactionError::operational("clone retained publication source", source, source_error)
        })?)
        .map_err(|source_error| {
            TransactionError::operational(
                "inspect retained publication source",
                source,
                source_error,
            )
        })?;
    let put = working.put_path(index);
    remove_owned_regular_if_present(&working.transaction_dir, &put)?;
    sync_directory(&working.transaction_dir)?;
    let mut destination = create_put_file(&put)?;
    std::io::copy(&mut source_file, &mut destination)
        .map_err(|source_error| TransactionError::operational(operation, &put, source_error))?;
    apply_ordinary_file_mode(&destination, &put, working.state.entries[index].prior_mode)?;
    destination.sync_all().map_err(|source_error| {
        TransactionError::operational("sync deterministic publication file", &put, source_error)
    })?;
    sync_directory(&working.transaction_dir)?;
    working.verify_internal_regular(source)?;
    verify_open_put_identity(&put, &destination, working.state.entries[index].prior_mode)?;
    let current_source = same_file::Handle::from_path(source).map_err(|source_error| {
        TransactionError::operational(
            "reinspect retained publication source",
            source,
            source_error,
        )
    })?;
    if current_source != source_identity
        || !opened_files_equal(&mut source_file, &mut destination, &working.transaction_dir)?
    {
        return Err(TransactionError::invalid_state(
            &working.transaction_dir,
            format!("retained publication source changed while it was copied: {source:?}"),
        ));
    }
    working.context.verify_root_and_lock()?;
    working.verify_target_ancestors(target)?;
    working.checkpoint(Checkpoint::ReplaceBefore { index })?;
    if let Some(expected) = expected_target {
        working.verify_target_generation(
            target,
            expected,
            "publication target generation changed at replace boundary",
        )?;
    } else {
        let _ = working.inspect_target(target)?;
    }
    verify_open_put_identity(&put, &destination, working.state.entries[index].prior_mode)?;
    std::fs::rename(&put, target)
        .map_err(|source_error| TransactionError::operational(operation, target, source_error))?;
    if let Some(parent) = target.parent() {
        sync_directory(parent)?;
    }
    sync_directory(&working.transaction_dir)
}

fn remove_regular_if_present(
    working: &WorkingTransaction,
    index: usize,
    path: &Path,
    expected_target: Option<&TargetGeneration>,
    operation: &'static str,
) -> Result<(), TransactionError> {
    working.context.verify_root_and_lock()?;
    working.checkpoint(Checkpoint::DeleteBefore { index })?;
    if let Some(expected) = expected_target {
        working.verify_target_generation(
            path,
            expected,
            "publication target generation changed at delete boundary",
        )?;
    }
    match working.inspect_target(path)? {
        TargetPresence::Missing => Ok(()),
        TargetPresence::Regular => {
            std::fs::remove_file(path)
                .map_err(|source| TransactionError::operational(operation, path, source))?;
            if let Some(parent) = path.parent() {
                sync_directory(parent)?;
            }
            Ok(())
        }
    }
}

fn remove_owned_regular_if_present(
    transaction_dir: &Path,
    path: &Path,
) -> Result<(), TransactionError> {
    match std::fs::symlink_metadata(path) {
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(TransactionError::operational(
            "inspect transaction cleanup file",
            path,
            source,
        )),
        Ok(metadata) if metadata.file_type().is_file() => {
            verify_private_file_link_count(path, &metadata)?;
            std::fs::remove_file(path).map_err(|source| {
                TransactionError::operational("remove transaction cleanup file", path, source)
            })
        }
        Ok(_) => Err(TransactionError::invalid_state(
            transaction_dir,
            format!("cleanup path is a symlink or non-regular file: {path:?}"),
        )),
    }
}

fn open_verified_regular(path: &Path, evidence: &Path) -> Result<File, TransactionError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|source| {
        TransactionError::operational("inspect regular transaction file", path, source)
    })?;
    if !metadata.file_type().is_file() {
        return Err(TransactionError::invalid_state(
            evidence,
            format!("expected a regular file without symlink traversal: {path:?}"),
        ));
    }
    let file = File::open(path)
        .map_err(|source| TransactionError::operational("open regular file", path, source))?;
    verify_open_regular_identity(path, &file, "regular transaction file")?;
    Ok(file)
}

fn verify_private_regular(path: &Path, evidence: &Path) -> Result<(), TransactionError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|source| {
        TransactionError::operational("inspect private transaction file", path, source)
    })?;
    if !metadata.file_type().is_file() {
        return Err(TransactionError::invalid_state(
            evidence,
            format!("private transaction path is a symlink or non-regular file: {path:?}"),
        ));
    }
    verify_private_file_mode(path, &metadata)?;
    let file = File::open(path).map_err(|source| {
        TransactionError::operational("open private transaction file", path, source)
    })?;
    verify_open_regular_identity(path, &file, "private transaction file")
}

fn verify_open_put_identity(
    path: &Path,
    file: &File,
    expected_mode: Option<u32>,
) -> Result<(), TransactionError> {
    let path_metadata = std::fs::symlink_metadata(path).map_err(|source| {
        TransactionError::operational("inspect deterministic publication file", path, source)
    })?;
    let file_metadata = file.metadata().map_err(|source| {
        TransactionError::operational("inspect open deterministic publication file", path, source)
    })?;
    if !path_metadata.file_type().is_file() || !file_metadata.is_file() {
        return Err(TransactionError::invalid_state(
            path,
            format!("deterministic publication path is a symlink or non-regular file: {path:?}"),
        ));
    }
    verify_private_file_link_count(path, &path_metadata)?;
    if expected_mode.is_some() && ordinary_file_mode(path, &path_metadata)? != expected_mode {
        return Err(TransactionError::invalid_state(
            path,
            format!("deterministic publication file has the wrong Unix mode: {path:?}"),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if path_metadata.dev() != file_metadata.dev() || path_metadata.ino() != file_metadata.ino()
        {
            return Err(TransactionError::invalid_state(
                path,
                "deterministic publication file identity changed",
            ));
        }
    }
    #[cfg(not(unix))]
    verify_open_regular_identity(path, file, "deterministic publication file")?;
    Ok(())
}

fn files_equal(left: &Path, right: &Path, evidence: &Path) -> Result<bool, TransactionError> {
    let left_path = left;
    let right_path = right;
    let mut left = open_verified_regular(left_path, evidence)?;
    let mut right = open_verified_regular(right_path, evidence)?;
    opened_files_equal(&mut left, &mut right, evidence)
}

fn opened_files_equal(
    left: &mut File,
    right: &mut File,
    evidence: &Path,
) -> Result<bool, TransactionError> {
    let left_len = left
        .metadata()
        .map_err(|source| {
            TransactionError::operational("inspect comparison file", evidence, source)
        })?
        .len();
    let right_len = right
        .metadata()
        .map_err(|source| {
            TransactionError::operational("inspect comparison file", evidence, source)
        })?
        .len();
    if left_len != right_len {
        return Ok(false);
    }
    left.rewind().map_err(|source| {
        TransactionError::operational("rewind comparison file", evidence, source)
    })?;
    right.rewind().map_err(|source| {
        TransactionError::operational("rewind comparison file", evidence, source)
    })?;
    let mut left_buffer = [0u8; 64 * 1024];
    let mut right_buffer = [0u8; 64 * 1024];
    loop {
        let left_read = left.read(&mut left_buffer).map_err(|source| {
            TransactionError::operational("read comparison file", evidence, source)
        })?;
        let right_read = right.read(&mut right_buffer).map_err(|source| {
            TransactionError::operational("read comparison file", evidence, source)
        })?;
        if left_read != right_read || left_buffer[..left_read] != right_buffer[..right_read] {
            return Ok(false);
        }
        if left_read == 0 {
            return Ok(true);
        }
    }
}

fn stage_name(index: usize) -> String {
    format!("new-{index:08}")
}

fn backup_name(index: usize) -> String {
    format!("old-{index:08}")
}

fn put_name(index: usize) -> String {
    format!("put-{index:08}")
}

fn directory_is_empty(path: &Path) -> Result<bool, TransactionError> {
    Ok(std::fs::read_dir(path)
        .map_err(|source| {
            TransactionError::operational("read transaction directory", path, source)
        })?
        .next()
        .transpose()
        .map_err(|source| {
            TransactionError::operational("read transaction directory entry", path, source)
        })?
        .is_none())
}

fn transaction_id(root: &Path) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let process = u64::from(std::process::id());
    let mut first = std::collections::hash_map::DefaultHasher::new();
    (timestamp, process, counter, root).hash(&mut first);
    let first = first.finish();
    let mut second = std::collections::hash_map::DefaultHasher::new();
    (
        root,
        counter.rotate_left(17),
        timestamp.rotate_left(31),
        first,
    )
        .hash(&mut second);
    format!("{first:016x}{:016x}", second.finish())
}

fn set_private_create_mode(options: &mut OpenOptions) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
}

fn verify_private_file_mode(
    path: &Path,
    metadata: &std::fs::Metadata,
) -> Result<(), TransactionError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(TransactionError::invalid_state(
                path,
                "transaction-owned file permissions are not owner-only",
            ));
        }
    }
    verify_private_file_link_count(path, metadata)
}

fn verify_private_file_link_count(
    path: &Path,
    metadata: &std::fs::Metadata,
) -> Result<(), TransactionError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() != 1 {
            return Err(TransactionError::invalid_state(
                path,
                "transaction-owned file has an unexpected hard-link count",
            ));
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (path, metadata);
    }
    Ok(())
}

fn ordinary_file_mode(
    path: &Path,
    metadata: &std::fs::Metadata,
) -> Result<Option<u32>, TransactionError> {
    if !metadata.file_type().is_file() {
        return Err(TransactionError::invalid_state(
            path,
            "cannot capture permissions from a non-regular file",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        Ok(Some(metadata.permissions().mode() & 0o777))
    }
    #[cfg(not(unix))]
    {
        Ok(None)
    }
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), TransactionError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| TransactionError::operational("sync directory", path, source))
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), TransactionError> {
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Checkpoint {
    StageBefore { index: usize },
    StageAfter { index: usize },
    PersistBefore { slot: StateSlot },
    PersistPrepared { slot: StateSlot },
    PersistAfter { slot: StateSlot },
    CommitBefore { index: usize },
    ReplaceBefore { index: usize },
    DeleteBefore { index: usize },
    CommitAfter { index: usize },
    RollbackBefore { index: usize },
    RollbackAfter { index: usize },
    RecoveryBefore,
    RecoveryAfter,
    CleanupBefore,
    CleanupFileBefore { ordinal: usize },
    CleanupFileAfter { ordinal: usize },
    CleanupAfter,
}

#[cfg(test)]
#[derive(Clone, Default)]
struct CheckpointHook(
    Option<std::sync::Arc<dyn Fn(Checkpoint) -> std::io::Result<()> + Send + Sync>>,
);

#[cfg(test)]
impl CheckpointHook {
    fn inactive() -> Self {
        Self::default()
    }

    fn new(hook: impl Fn(Checkpoint) -> std::io::Result<()> + Send + Sync + 'static) -> Self {
        Self(Some(std::sync::Arc::new(hook)))
    }

    fn run(&self, checkpoint: Checkpoint) -> std::io::Result<()> {
        match &self.0 {
            Some(hook) => hook(checkpoint),
            None => Ok(()),
        }
    }
}

#[cfg(not(test))]
#[derive(Clone, Default)]
struct CheckpointHook;

#[cfg(not(test))]
impl CheckpointHook {
    fn inactive() -> Self {
        Self
    }

    fn run(&self, _checkpoint: Checkpoint) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
