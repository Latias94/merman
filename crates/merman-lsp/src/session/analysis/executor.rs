use super::AnalysisJobGeneration;
use crate::session::analysis::request::{
    AnalysisBuildError, AnalysisBuildKey, AnalysisBuildRequest, DiagnosticReprojectionKey,
    DiagnosticReprojectionRequest, DiagnosticReprojectionResult,
};
#[cfg(test)]
use crate::snapshot::{DiagnosticGeneration, DocumentEpoch, SnapshotGeneration};
use crate::snapshot::{DocumentAnalysisContext, DocumentSnapshot};
use crate::sync::lock_recovering_poison;
use merman_analysis::AnalysisCancellationToken;
#[cfg(test)]
use merman_analysis::Analyzer;
use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};
use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore};
use tower_lsp_server::ls_types::Uri;

/// Maximum number of document analyses that may consume CPU concurrently.
pub(in crate::session) const LSP_ANALYSIS_CONCURRENCY: usize = 2;
/// Maximum number of physical analysis workers that may be running or waiting for CPU.
pub(in crate::session) const LSP_ANALYSIS_IN_FLIGHT_LIMIT: usize = 8;

#[derive(Clone)]
pub(in crate::session) struct AnalysisExecutor {
    inner: Arc<AnalysisExecutorInner>,
}

impl fmt::Debug for AnalysisExecutor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AnalysisExecutor")
            .field("concurrency", &LSP_ANALYSIS_CONCURRENCY)
            .field("in_flight_limit", &LSP_ANALYSIS_IN_FLIGHT_LIMIT)
            .finish_non_exhaustive()
    }
}

struct AnalysisExecutorInner {
    cpu_permits: Arc<Semaphore>,
    task_capacity: Arc<AnalysisTaskCapacity>,
    cancellation_parent: AnalysisCancellationToken,
    registry: Mutex<AnalysisRegistry>,
    #[cfg(test)]
    worker_exit_gate: Mutex<Option<Arc<TestWorkerExitGate>>>,
    #[cfg(test)]
    execution_count: AtomicUsize,
    #[cfg(test)]
    reprojection_count: AtomicUsize,
}

#[derive(Default)]
struct AnalysisRegistry {
    jobs: HashMap<AnalysisWorkKey, Arc<AnalysisJob>>,
    pending: Option<Arc<PendingAnalysis>>,
    next_generation: u64,
    document_generations: HashMap<Uri, AnalysisJobGeneration>,
}

/// Tracks physical worker lifetime independently from single-flight registry membership.
struct AnalysisTaskCapacity {
    slots: Arc<Semaphore>,
    running_workers: AtomicUsize,
    idle: Notify,
}

impl AnalysisTaskCapacity {
    fn new(limit: usize) -> Self {
        Self {
            slots: Arc::new(Semaphore::new(limit)),
            running_workers: AtomicUsize::new(0),
            idle: Notify::new(),
        }
    }

    async fn acquire(self: &Arc<Self>) -> Result<OwnedSemaphorePermit, tokio::sync::AcquireError> {
        Arc::clone(&self.slots).acquire_owned().await
    }

    fn try_acquire(self: &Arc<Self>) -> Result<OwnedSemaphorePermit, tokio::sync::TryAcquireError> {
        Arc::clone(&self.slots).try_acquire_owned()
    }

    fn start_worker(self: &Arc<Self>, task_permit: OwnedSemaphorePermit) -> AnalysisWorkerLease {
        let previous = self.running_workers.fetch_add(1, Ordering::AcqRel);
        debug_assert!(
            previous < LSP_ANALYSIS_IN_FLIGHT_LIMIT,
            "analysis task capacity admitted too many physical workers"
        );
        AnalysisWorkerLease {
            capacity: Arc::clone(self),
            task_permit: Some(task_permit),
        }
    }

    #[cfg(test)]
    fn running_workers(&self) -> usize {
        self.running_workers.load(Ordering::Acquire)
    }

    #[cfg(test)]
    fn available_slots(&self) -> usize {
        self.slots.available_permits()
    }

    #[cfg(test)]
    async fn wait_idle(&self) {
        loop {
            let idle = self.idle.notified();
            tokio::pin!(idle);
            let _ = idle.as_mut().enable();
            if self.running_workers() == 0 {
                return;
            }
            idle.as_mut().await;
        }
    }
}

/// Keeps one physical task slot until the spawned worker has completely unwound.
struct AnalysisWorkerLease {
    capacity: Arc<AnalysisTaskCapacity>,
    task_permit: Option<OwnedSemaphorePermit>,
}

impl Drop for AnalysisWorkerLease {
    fn drop(&mut self) {
        let previous = self.capacity.running_workers.fetch_sub(1, Ordering::AcqRel);
        debug_assert!(previous > 0, "analysis worker count underflow");
        drop(
            self.task_permit
                .take()
                .expect("analysis worker task permit must be released exactly once"),
        );
        if previous == 1 {
            self.capacity.idle.notify_waiters();
        }
    }
}

#[cfg(test)]
#[derive(Debug, Default)]
struct TestWorkerExitGate {
    released: AtomicBool,
    reached: AtomicUsize,
    reached_changed: Notify,
    release_changed: Notify,
}

#[cfg(test)]
impl TestWorkerExitGate {
    async fn wait(&self) {
        self.reached.fetch_add(1, Ordering::AcqRel);
        self.reached_changed.notify_waiters();
        loop {
            let released = self.release_changed.notified();
            tokio::pin!(released);
            let _ = released.as_mut().enable();
            if self.released.load(Ordering::Acquire) {
                return;
            }
            released.as_mut().await;
        }
    }

    async fn wait_for_workers(&self, expected: usize) {
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            loop {
                let reached = self.reached_changed.notified();
                tokio::pin!(reached);
                let _ = reached.as_mut().enable();
                if self.reached.load(Ordering::Acquire) >= expected {
                    return;
                }
                reached.as_mut().await;
            }
        })
        .await
        .expect("analysis workers did not reach the deterministic exit gate");
    }

    fn release(&self) {
        self.released.store(true, Ordering::Release);
        self.release_changed.notify_waiters();
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
enum AnalysisWorkKey {
    Build(AnalysisBuildKey),
    Reproject(DiagnosticReprojectionKey),
}

impl AnalysisWorkKey {
    fn is_reprojection(&self) -> bool {
        matches!(self, Self::Reproject(_))
    }

    fn uri(&self) -> &Uri {
        match self {
            Self::Build(key) => key.uri(),
            Self::Reproject(key) => key.uri(),
        }
    }

    fn analysis_job_generation(&self) -> AnalysisJobGeneration {
        match self {
            Self::Build(key) => key.analysis_job_generation(),
            Self::Reproject(key) => key.analysis_job_generation(),
        }
    }
}

enum AnalysisWork {
    Build(Box<AnalysisBuildRequest>),
    Reproject(Box<DiagnosticReprojectionRequest>),
}

/// One bounded admission rendezvous outside the eight physical worker slots.
///
/// This lets the ninth distinct key wait and coalesce duplicates without placing arbitrary
/// request payloads into the semaphore's unbounded waiter queue. A different key is rejected
/// while this slot is occupied; compliant transports never reach that path because they retain
/// at most eight handler futures.
struct PendingAnalysis {
    key: AnalysisWorkKey,
    job: Arc<AnalysisJob>,
}

#[derive(Clone)]
enum AnalysisWorkOutput {
    Build(Arc<AnalysisBuildOutput>),
    Reproject(Arc<DiagnosticReprojectionResult>),
}

struct AnalysisBuildOutput {
    snapshot: Arc<DocumentSnapshot>,
    cache_admission_claimed: AtomicBool,
}

impl AnalysisBuildOutput {
    fn new(snapshot: Arc<DocumentSnapshot>) -> Self {
        Self {
            snapshot,
            cache_admission_claimed: AtomicBool::new(false),
        }
    }

    fn claim_cache_admission(&self) -> bool {
        self.cache_admission_claimed
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
}

impl AnalysisRegistry {
    fn generation_for(&mut self, uri: &Uri) -> AnalysisJobGeneration {
        if let Some(generation) = self.document_generations.get(uri) {
            return *generation;
        }

        self.next_generation = self
            .next_generation
            .checked_add(1)
            .expect("analysis job generation sequence exhausted");
        let generation = AnalysisJobGeneration(self.next_generation);
        self.document_generations.insert(uri.clone(), generation);
        generation
    }

    fn current_generation_for(&self, uri: &Uri) -> Option<AnalysisJobGeneration> {
        self.document_generations.get(uri).copied()
    }
}

struct AnalysisJob {
    result: Mutex<Option<Result<AnalysisWorkOutput, AnalysisExecutionError>>>,
    ready: Notify,
    cancellation: AnalysisCancellationToken,
    cancellation_signal: Notify,
    waiters: AtomicUsize,
}

impl AnalysisJob {
    fn new(cancellation: AnalysisCancellationToken) -> Self {
        Self {
            result: Mutex::new(None),
            ready: Notify::new(),
            cancellation,
            cancellation_signal: Notify::new(),
            waiters: AtomicUsize::new(0),
        }
    }

    async fn wait(&self) -> Result<AnalysisWorkOutput, AnalysisExecutionError> {
        loop {
            let notified = self.ready.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if let Some(result) = lock_recovering_poison(&self.result).clone() {
                return result;
            }
            notified.as_mut().await;
        }
    }

    fn is_complete(&self) -> bool {
        lock_recovering_poison(&self.result).is_some()
    }

    fn has_error(&self) -> bool {
        matches!(&*lock_recovering_poison(&self.result), Some(Err(_)))
    }

    fn complete(&self, result: Result<AnalysisWorkOutput, AnalysisExecutionError>) {
        let mut stored = lock_recovering_poison(&self.result);
        if stored.is_none() {
            *stored = Some(result);
        }
        drop(stored);
        self.ready.notify_waiters();
    }

    fn cancel(&self, error: AnalysisExecutionError) {
        self.cancellation.cancel();
        self.complete(Err(error));
        self.cancellation_signal.notify_waiters();
    }

    fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    async fn cancelled(&self) {
        loop {
            let notified = self.cancellation_signal.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.is_cancelled() {
                return;
            }
            notified.as_mut().await;
        }
    }
}

struct AnalysisWaiter {
    inner: Weak<AnalysisExecutorInner>,
    key: AnalysisWorkKey,
    job: Arc<AnalysisJob>,
}

impl AnalysisWaiter {
    fn new(
        inner: &Arc<AnalysisExecutorInner>,
        key: AnalysisWorkKey,
        job: Arc<AnalysisJob>,
    ) -> Self {
        job.waiters.fetch_add(1, Ordering::Relaxed);
        Self {
            inner: Arc::downgrade(inner),
            key,
            job,
        }
    }

    async fn wait(&self) -> Result<AnalysisWorkOutput, AnalysisExecutionError> {
        self.job.wait().await
    }
}

pub(in crate::session) struct AnalysisExecutionLease {
    output: Arc<AnalysisBuildOutput>,
    _waiter: AnalysisWaiter,
}

impl fmt::Debug for AnalysisExecutionLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AnalysisExecutionLease")
            .field("snapshot", &self.output.snapshot)
            .finish_non_exhaustive()
    }
}

impl AnalysisExecutionLease {
    pub(in crate::session) fn snapshot(&self) -> &Arc<DocumentSnapshot> {
        &self.output.snapshot
    }

    pub(in crate::session) fn claim_cache_admission(&self) -> bool {
        self.output.claim_cache_admission()
    }
}

pub(in crate::session) struct DiagnosticReprojectionLease {
    result: Arc<DiagnosticReprojectionResult>,
    _waiter: AnalysisWaiter,
}

impl fmt::Debug for DiagnosticReprojectionLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DiagnosticReprojectionLease")
            .field("result", &self.result)
            .finish_non_exhaustive()
    }
}

impl DiagnosticReprojectionLease {
    pub(in crate::session) fn key(&self) -> &DiagnosticReprojectionKey {
        self.result.key()
    }

    pub(in crate::session) fn projected(&self) -> &Arc<DocumentAnalysisContext> {
        self.result.projected()
    }

    #[cfg(test)]
    pub(in crate::session) fn shares_result_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.result, &other.result)
    }
}

impl Drop for AnalysisWaiter {
    fn drop(&mut self) {
        let Some(inner) = self.inner.upgrade() else {
            return;
        };

        let should_cancel = {
            let mut registry = lock_recovering_poison(&inner.registry);
            let previous = self.job.waiters.fetch_sub(1, Ordering::Relaxed);
            debug_assert!(previous > 0, "analysis waiter count underflow");
            if previous == 1 {
                if registry
                    .jobs
                    .get(&self.key)
                    .is_some_and(|registered| Arc::ptr_eq(registered, &self.job))
                {
                    registry.jobs.remove(&self.key);
                } else if registry.pending.as_ref().is_some_and(|pending| {
                    pending.key == self.key && Arc::ptr_eq(&pending.job, &self.job)
                }) {
                    registry.pending = None;
                }
                !self.job.is_complete()
            } else {
                false
            }
        };

        if should_cancel {
            self.job.cancel(AnalysisExecutionError::cancelled());
        }
    }
}

#[derive(Debug, Clone)]
pub(in crate::session) struct AnalysisExecutionError {
    message: Arc<str>,
    kind: AnalysisExecutionErrorKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnalysisExecutionErrorKind {
    Internal,
    Stale,
    Overloaded,
}

impl AnalysisExecutionError {
    fn new(message: impl Into<Arc<str>>) -> Self {
        Self {
            message: message.into(),
            kind: AnalysisExecutionErrorKind::Internal,
        }
    }

    fn stale() -> Self {
        Self {
            message: "document analysis request was superseded".into(),
            kind: AnalysisExecutionErrorKind::Stale,
        }
    }

    fn cancelled() -> Self {
        Self {
            message: "document analysis request no longer has a waiter".into(),
            kind: AnalysisExecutionErrorKind::Stale,
        }
    }

    fn overloaded() -> Self {
        Self {
            message: "document analysis admission is saturated".into(),
            kind: AnalysisExecutionErrorKind::Overloaded,
        }
    }

    fn resource_rejected(rejection: merman_analysis::AnalysisRejection) -> Self {
        Self {
            message: format!(
                "document analysis rejected after LSP preflight: {}",
                rejection.resource_limit()
            )
            .into(),
            kind: AnalysisExecutionErrorKind::Internal,
        }
    }

    pub(in crate::session) fn is_stale(&self) -> bool {
        self.kind == AnalysisExecutionErrorKind::Stale
    }

    #[cfg(test)]
    fn is_overloaded(&self) -> bool {
        self.kind == AnalysisExecutionErrorKind::Overloaded
    }
}

impl fmt::Display for AnalysisExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for AnalysisExecutionError {}

impl AnalysisExecutor {
    pub(in crate::session) fn new(cancellation_parent: AnalysisCancellationToken) -> Self {
        Self {
            inner: Arc::new(AnalysisExecutorInner {
                cpu_permits: Arc::new(Semaphore::new(LSP_ANALYSIS_CONCURRENCY)),
                task_capacity: Arc::new(AnalysisTaskCapacity::new(LSP_ANALYSIS_IN_FLIGHT_LIMIT)),
                cancellation_parent,
                registry: Mutex::new(AnalysisRegistry::default()),
                #[cfg(test)]
                worker_exit_gate: Mutex::new(None),
                #[cfg(test)]
                execution_count: AtomicUsize::new(0),
                #[cfg(test)]
                reprojection_count: AtomicUsize::new(0),
            }),
        }
    }

    pub(in crate::session) fn generation_for(&self, uri: &Uri) -> AnalysisJobGeneration {
        lock_recovering_poison(&self.inner.registry).generation_for(uri)
    }

    pub(in crate::session) fn is_generation_current(
        &self,
        uri: &Uri,
        generation: AnalysisJobGeneration,
    ) -> bool {
        lock_recovering_poison(&self.inner.registry).current_generation_for(uri) == Some(generation)
    }

    pub(in crate::session) async fn execute(
        &self,
        request: &AnalysisBuildRequest,
    ) -> Result<AnalysisExecutionLease, AnalysisExecutionError> {
        let key = AnalysisWorkKey::Build(request.key());
        let waiter = self.execute_work(key, self.inner.cancellation_parent.child(), || {
            AnalysisWork::Build(Box::new(request.clone()))
        })?;
        let output = waiter.wait().await?;
        let AnalysisWorkOutput::Build(context) = output else {
            return Err(AnalysisExecutionError::new(
                "analysis executor returned a diagnostic projection for a build request",
            ));
        };
        Ok(AnalysisExecutionLease {
            output: context,
            _waiter: waiter,
        })
    }

    pub(in crate::session) async fn execute_diagnostic_reprojection(
        &self,
        request: &DiagnosticReprojectionRequest,
    ) -> Result<DiagnosticReprojectionLease, AnalysisExecutionError> {
        let key = AnalysisWorkKey::Reproject(request.key());
        let waiter = self.execute_work(key, request.cancellation_child(), || {
            AnalysisWork::Reproject(Box::new(request.clone()))
        })?;
        let output = waiter.wait().await?;
        let AnalysisWorkOutput::Reproject(result) = output else {
            return Err(AnalysisExecutionError::new(
                "analysis executor returned a build result for a diagnostic projection request",
            ));
        };
        Ok(DiagnosticReprojectionLease {
            result,
            _waiter: waiter,
        })
    }

    fn execute_work<F>(
        &self,
        key: AnalysisWorkKey,
        cancellation: AnalysisCancellationToken,
        make_work: F,
    ) -> Result<AnalysisWaiter, AnalysisExecutionError>
    where
        F: FnOnce() -> AnalysisWork,
    {
        if cancellation.is_cancelled() {
            return Err(AnalysisExecutionError::stale());
        }
        let mut make_work = Some(make_work);
        let existing = {
            let registry = lock_recovering_poison(&self.inner.registry);
            if Some(key.analysis_job_generation()) != registry.current_generation_for(key.uri()) {
                return Err(AnalysisExecutionError::stale());
            }
            if let Some(job) = registry.jobs.get(&key) {
                Some(AnalysisWaiter::new(
                    &self.inner,
                    key.clone(),
                    Arc::clone(job),
                ))
            } else if let Some(pending) = &registry.pending {
                if pending.key == key {
                    Some(AnalysisWaiter::new(
                        &self.inner,
                        key.clone(),
                        Arc::clone(&pending.job),
                    ))
                } else {
                    return Err(AnalysisExecutionError::overloaded());
                }
            } else {
                None
            }
        };
        if let Some(waiter) = existing {
            if cancellation.is_cancelled() {
                drop(waiter);
                return Err(AnalysisExecutionError::stale());
            }
            return Ok(waiter);
        }

        match self.inner.task_capacity.try_acquire() {
            Ok(task_permit) => {
                let mut task_permit = Some(task_permit);
                let (waiter, start) = {
                    let mut registry = lock_recovering_poison(&self.inner.registry);
                    if Some(key.analysis_job_generation())
                        != registry.current_generation_for(key.uri())
                    {
                        return Err(AnalysisExecutionError::stale());
                    }
                    if let Some(job) = registry.jobs.get(&key) {
                        (
                            AnalysisWaiter::new(&self.inner, key.clone(), Arc::clone(job)),
                            None,
                        )
                    } else if let Some(pending) = &registry.pending {
                        if pending.key != key {
                            return Err(AnalysisExecutionError::overloaded());
                        }
                        (
                            AnalysisWaiter::new(&self.inner, key.clone(), Arc::clone(&pending.job)),
                            None,
                        )
                    } else {
                        let job = Arc::new(AnalysisJob::new(cancellation.clone()));
                        let waiter =
                            AnalysisWaiter::new(&self.inner, key.clone(), Arc::clone(&job));
                        // Track the worker before publishing the job so shutdown cannot observe an
                        // empty worker set and then race with this already-admitted spawn.
                        let worker_lease = self.inner.task_capacity.start_worker(
                            task_permit
                                .take()
                                .expect("new analysis worker must retain its task permit"),
                        );
                        registry.jobs.insert(key.clone(), Arc::clone(&job));
                        let work = make_work
                            .take()
                            .expect("new analysis work must retain its factory")(
                        );
                        (waiter, Some((work, job, worker_lease)))
                    }
                };
                drop(task_permit);

                if let Some((work, job, worker_lease)) = start {
                    self.start(key, work, job, worker_lease);
                }
                if cancellation.is_cancelled() {
                    drop(waiter);
                    return Err(AnalysisExecutionError::stale());
                }
                Ok(waiter)
            }
            Err(tokio::sync::TryAcquireError::NoPermits) => {
                let (waiter, pending) = {
                    let mut registry = lock_recovering_poison(&self.inner.registry);
                    if Some(key.analysis_job_generation())
                        != registry.current_generation_for(key.uri())
                    {
                        return Err(AnalysisExecutionError::stale());
                    }
                    if let Some(job) = registry.jobs.get(&key) {
                        (
                            AnalysisWaiter::new(&self.inner, key.clone(), Arc::clone(job)),
                            None,
                        )
                    } else if let Some(pending) = &registry.pending {
                        if pending.key != key {
                            return Err(AnalysisExecutionError::overloaded());
                        }
                        (
                            AnalysisWaiter::new(&self.inner, key.clone(), Arc::clone(&pending.job)),
                            None,
                        )
                    } else {
                        let job = Arc::new(AnalysisJob::new(cancellation.clone()));
                        let waiter =
                            AnalysisWaiter::new(&self.inner, key.clone(), Arc::clone(&job));
                        let work = make_work
                            .take()
                            .expect("pending analysis work must retain its factory")(
                        );
                        let pending = Arc::new(PendingAnalysis {
                            key: key.clone(),
                            job,
                        });
                        registry.pending = Some(Arc::clone(&pending));
                        (waiter, Some((pending, work)))
                    }
                };

                if let Some((pending, work)) = pending {
                    self.start_pending(pending, work);
                }
                if cancellation.is_cancelled() {
                    drop(waiter);
                    return Err(AnalysisExecutionError::stale());
                }
                Ok(waiter)
            }
            Err(tokio::sync::TryAcquireError::Closed) => Err(AnalysisExecutionError::new(
                "document analysis task capacity closed",
            )),
        }
    }

    fn start_pending(&self, pending: Arc<PendingAnalysis>, work: AnalysisWork) {
        let executor = self.clone();
        tokio::spawn(async move {
            let task_permit = tokio::select! {
                _ = pending.job.cancelled() => return,
                permit = executor.inner.task_capacity.acquire() => match permit {
                    Ok(permit) => permit,
                    Err(error) => {
                        remove_pending_if_registered(&executor.inner, &pending);
                        pending.job.cancel(AnalysisExecutionError::new(format!(
                            "document analysis task capacity closed: {error}"
                        )));
                        return;
                    }
                },
            };
            if pending.job.is_cancelled() {
                return;
            }

            let mut task_permit = Some(task_permit);
            let (start, stale) = {
                let mut registry = lock_recovering_poison(&executor.inner.registry);
                let registered = registry
                    .pending
                    .as_ref()
                    .is_some_and(|registered| Arc::ptr_eq(registered, &pending));
                if !registered {
                    (None, false)
                } else {
                    registry.pending = None;
                    if Some(pending.key.analysis_job_generation())
                        != registry.current_generation_for(pending.key.uri())
                    {
                        (None, true)
                    } else {
                        let worker_lease = executor.inner.task_capacity.start_worker(
                            task_permit
                                .take()
                                .expect("promoted analysis must retain its task permit"),
                        );
                        let previous = registry
                            .jobs
                            .insert(pending.key.clone(), Arc::clone(&pending.job));
                        debug_assert!(previous.is_none(), "pending analysis key was started twice");
                        (Some((work, worker_lease)), false)
                    }
                }
            };
            drop(task_permit);

            if stale {
                pending.job.cancel(AnalysisExecutionError::stale());
                return;
            }
            if let Some((work, worker_lease)) = start {
                executor.start(
                    pending.key.clone(),
                    work,
                    Arc::clone(&pending.job),
                    worker_lease,
                );
            }
        });
    }

    fn start(
        &self,
        key: AnalysisWorkKey,
        work: AnalysisWork,
        job: Arc<AnalysisJob>,
        worker_lease: AnalysisWorkerLease,
    ) {
        let inner = Arc::clone(&self.inner);
        #[cfg(test)]
        let worker_exit_gate = lock_recovering_poison(&inner.worker_exit_gate).clone();
        tokio::spawn(async move {
            let result = tokio::select! {
                _ = job.cancelled() => Err(AnalysisExecutionError::cancelled()),
                permit = Arc::clone(&inner.cpu_permits).acquire_owned() => match permit {
                Ok(permit) if !job.is_cancelled() => {
                    #[cfg(test)]
                    match &work {
                        AnalysisWork::Build(_) => {
                            inner.execution_count.fetch_add(1, Ordering::Relaxed);
                        }
                        AnalysisWork::Reproject(_) => {
                            inner.reprojection_count.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    let cancellation = job.cancellation.clone();
                    tokio::task::spawn_blocking(move || {
                        let _permit = permit;
                        match work {
                            AnalysisWork::Build(request) => request
                                .build_cancellable(&cancellation)
                                .map(AnalysisBuildOutput::new)
                                .map(Arc::new)
                                .map(AnalysisWorkOutput::Build)
                                .map_err(AnalysisWorkError::Build),
                            AnalysisWork::Reproject(request) => request
                                .project_with_cancellation(&cancellation)
                                .map(|result| AnalysisWorkOutput::Reproject(Arc::new(result)))
                                .map_err(|_| AnalysisWorkError::Cancelled),
                        }
                    })
                    .await
                    .map_err(|error| {
                        AnalysisExecutionError::new(format!(
                            "document analysis worker failed: {error}"
                        ))
                    })
                    .and_then(|result| match result {
                        Ok(output) => Ok(output),
                        Err(AnalysisWorkError::Build(AnalysisBuildError::Cancelled(_)))
                        | Err(AnalysisWorkError::Cancelled) => {
                            Err(AnalysisExecutionError::cancelled())
                        }
                        Err(AnalysisWorkError::Build(AnalysisBuildError::Rejected(rejection))) => {
                            Err(AnalysisExecutionError::resource_rejected(rejection))
                        }
                    })
                }
                Ok(_) => Err(AnalysisExecutionError::cancelled()),
                Err(error) => Err(AnalysisExecutionError::new(format!(
                    "document analysis executor closed: {error}"
                ))),
                }
            };

            job.complete(result);
            if job.is_cancelled() || job.has_error() {
                remove_job_if_registered(&inner, &key, &job);
            }
            #[cfg(test)]
            if let Some(gate) = worker_exit_gate {
                gate.wait().await;
            }
            drop(worker_lease);
        });
    }

    pub(in crate::session) fn invalidate(&self, uri: &Uri) {
        self.invalidate_uri(uri);
    }

    pub(in crate::session) fn forget(&self, uri: &Uri) {
        self.invalidate_uri(uri);
    }

    /// Drops every diagnostic-only projection from admission without disturbing
    /// the canonical snapshot builds that may still be needed by a later policy.
    pub(in crate::session) fn invalidate_reprojections(&self) {
        let cancelled = {
            let mut registry = lock_recovering_poison(&self.inner.registry);
            let mut cancelled = Vec::new();
            if registry
                .pending
                .as_ref()
                .is_some_and(|pending| pending.key.is_reprojection())
            {
                let pending = registry
                    .pending
                    .take()
                    .expect("checked pending analysis must remain registered");
                cancelled.push(Arc::clone(&pending.job));
            }
            registry.jobs.retain(|key, job| {
                if !key.is_reprojection() {
                    return true;
                }

                cancelled.push(Arc::clone(job));
                false
            });
            cancelled
        };

        for job in cancelled {
            job.cancel(AnalysisExecutionError::stale());
        }
    }

    fn invalidate_uri(&self, uri: &Uri) {
        let cancelled = {
            let mut registry = lock_recovering_poison(&self.inner.registry);
            registry.document_generations.remove(uri);
            let mut cancelled = Vec::new();
            if registry
                .pending
                .as_ref()
                .is_some_and(|pending| pending.key.uri() == uri)
            {
                let pending = registry
                    .pending
                    .take()
                    .expect("checked pending analysis must remain registered");
                cancelled.push(Arc::clone(&pending.job));
            }
            registry.jobs.retain(|key, job| {
                if key.uri() == uri {
                    cancelled.push(Arc::clone(job));
                    false
                } else {
                    true
                }
            });
            cancelled
        };
        for job in cancelled {
            job.cancel(AnalysisExecutionError::stale());
        }
    }

    pub(in crate::session) fn invalidate_all(&self) {
        let cancelled = {
            let mut registry = lock_recovering_poison(&self.inner.registry);
            registry.document_generations.clear();
            let mut cancelled = registry
                .jobs
                .drain()
                .map(|(_, job)| job)
                .collect::<Vec<_>>();
            if let Some(pending) = registry.pending.take() {
                cancelled.push(Arc::clone(&pending.job));
            }
            cancelled
        };
        for job in cancelled {
            job.cancel(AnalysisExecutionError::stale());
        }
    }

    #[cfg(test)]
    pub(in crate::session) fn execution_count(&self) -> usize {
        self.inner.execution_count.load(Ordering::Relaxed)
    }

    #[cfg(test)]
    pub(in crate::session) fn reprojection_count(&self) -> usize {
        self.inner.reprojection_count.load(Ordering::Relaxed)
    }

    #[cfg(test)]
    pub(in crate::session) fn registry_state(&self) -> (usize, usize, usize) {
        let registry = lock_recovering_poison(&self.inner.registry);
        (
            registry.jobs.len(),
            self.inner.task_capacity.running_workers(),
            self.inner.cpu_permits.available_permits(),
        )
    }

    #[cfg(test)]
    fn pending_admission_count(&self) -> usize {
        usize::from(
            lock_recovering_poison(&self.inner.registry)
                .pending
                .is_some(),
        )
    }

    #[cfg(test)]
    pub(in crate::session) fn running_worker_count(&self) -> usize {
        self.inner.task_capacity.running_workers()
    }

    #[cfg(test)]
    fn available_task_slots(&self) -> usize {
        self.inner.task_capacity.available_slots()
    }

    #[cfg(test)]
    fn set_worker_exit_gate(&self, gate: Arc<TestWorkerExitGate>) {
        *lock_recovering_poison(&self.inner.worker_exit_gate) = Some(gate);
    }

    #[cfg(test)]
    pub(in crate::session) async fn wait_idle(&self) {
        loop {
            self.inner.task_capacity.wait_idle().await;
            if self.inner.task_capacity.running_workers() == 0
                && lock_recovering_poison(&self.inner.registry)
                    .pending
                    .is_none()
            {
                return;
            }
            tokio::task::yield_now().await;
        }
    }

    #[cfg(test)]
    pub(in crate::session) fn waiter_count_for_uri(&self, uri: &Uri) -> usize {
        let registry = lock_recovering_poison(&self.inner.registry);
        let registered = registry
            .jobs
            .iter()
            .filter(|(key, _)| key.uri() == uri)
            .map(|(_, job)| job.waiters.load(Ordering::Acquire))
            .sum::<usize>();
        let pending = registry
            .pending
            .as_ref()
            .filter(|pending| pending.key.uri() == uri)
            .map_or(0, |pending| pending.job.waiters.load(Ordering::Acquire));
        registered.saturating_add(pending)
    }
}

fn remove_pending_if_registered(inner: &AnalysisExecutorInner, pending: &Arc<PendingAnalysis>) {
    let mut registry = lock_recovering_poison(&inner.registry);
    if registry
        .pending
        .as_ref()
        .is_some_and(|registered| Arc::ptr_eq(registered, pending))
    {
        registry.pending = None;
    }
}

fn remove_job_if_registered(
    inner: &AnalysisExecutorInner,
    key: &AnalysisWorkKey,
    job: &Arc<AnalysisJob>,
) {
    let mut registry = lock_recovering_poison(&inner.registry);
    if registry
        .jobs
        .get(key)
        .is_some_and(|registered| Arc::ptr_eq(registered, job))
    {
        registry.jobs.remove(key);
    }
}

enum AnalysisWorkError {
    Build(AnalysisBuildError),
    Cancelled,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::analysis::request::{AnalysisBuildKey, TestAnalysisGate};
    use merman_editor_core::DocumentKind;
    use std::str::FromStr;
    use std::task::Poll;
    use std::time::Duration;

    const TEST_SOURCE: &str = "flowchart TD\nA-->B\n";

    fn test_executor() -> AnalysisExecutor {
        AnalysisExecutor::new(AnalysisCancellationToken::new())
    }

    fn test_uri(name: &str) -> Uri {
        Uri::from_str(&format!("file:///tmp/{name}.mmd")).unwrap()
    }

    fn build_request(executor: &AnalysisExecutor, name: &str) -> AnalysisBuildRequest {
        build_request_for_uri(executor, test_uri(name), 1, DocumentEpoch(1), TEST_SOURCE)
    }

    fn build_request_for_uri(
        executor: &AnalysisExecutor,
        uri: Uri,
        version: i32,
        document_epoch: DocumentEpoch,
        source: &str,
    ) -> AnalysisBuildRequest {
        build_request_with_analyzer(
            executor,
            uri,
            version,
            document_epoch,
            source,
            Analyzer::new(),
        )
    }

    fn build_request_with_analyzer(
        executor: &AnalysisExecutor,
        uri: Uri,
        version: i32,
        document_epoch: DocumentEpoch,
        source: &str,
        analyzer: Analyzer,
    ) -> AnalysisBuildRequest {
        let analysis_job_generation = executor.generation_for(&uri);
        AnalysisBuildRequest::new(
            AnalysisBuildKey::new(
                uri,
                version,
                analysis_job_generation,
                SnapshotGeneration(1),
                document_epoch,
            ),
            Arc::<str>::from(source),
            DocumentKind::Diagram,
            analyzer,
        )
    }

    fn diagnostic_reprojection_request(
        executor: &AnalysisExecutor,
        name: &str,
    ) -> DiagnosticReprojectionRequest {
        let analyzer = Analyzer::new();
        let uri = test_uri(name);
        let build = build_request_with_analyzer(
            executor,
            uri.clone(),
            1,
            DocumentEpoch(1),
            TEST_SOURCE,
            analyzer.clone(),
        );
        let context = build
            .build_cancellable(&AnalysisCancellationToken::new())
            .expect("test analysis should be ready");
        DiagnosticReprojectionRequest::new(
            analyzer.options().diagnostic_policy().clone(),
            AnalysisCancellationToken::new(),
            DiagnosticReprojectionKey::new(
                uri,
                build.analysis_job_generation(),
                build.document_epoch(),
                build.snapshot_generation(),
                DiagnosticGeneration(2),
                context.as_ref(),
            ),
            context,
        )
    }

    async fn wait_for_job_count(executor: &AnalysisExecutor, expected: usize) {
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if lock_recovering_poison(&executor.inner.registry).jobs.len() == expected {
                    return;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("analysis job registry did not reach the expected size");
    }

    async fn wait_for_registered_job(executor: &AnalysisExecutor, key: &AnalysisBuildKey) {
        let key = AnalysisWorkKey::Build(key.clone());
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if lock_recovering_poison(&executor.inner.registry)
                    .jobs
                    .contains_key(&key)
                {
                    return;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("analysis job was not registered");
    }

    async fn wait_for_available_cpu_permits(executor: &AnalysisExecutor, expected: usize) {
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if executor.inner.cpu_permits.available_permits() == expected {
                    return;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("analysis CPU permits were not restored");
    }

    async fn wait_for_running_workers(executor: &AnalysisExecutor, expected: usize) {
        tokio::time::timeout(Duration::from_secs(1), async {
            while executor.running_worker_count() != expected {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("analysis executor did not reach the expected physical worker count");
    }

    async fn wait_for_registry_state(executor: &AnalysisExecutor, expected: (usize, usize, usize)) {
        tokio::time::timeout(Duration::from_secs(1), async {
            while executor.registry_state() != expected {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("analysis registry did not reach the expected state");
    }

    async fn wait_for_waiter_count(
        executor: &AnalysisExecutor,
        key: &AnalysisBuildKey,
        expected: usize,
    ) {
        let key = AnalysisWorkKey::Build(key.clone());
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                let waiters = lock_recovering_poison(&executor.inner.registry)
                    .jobs
                    .get(&key)
                    .map(|job| job.waiters.load(std::sync::atomic::Ordering::Acquire));
                if waiters == Some(expected) {
                    return;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("analysis waiter count did not reach the expected value");
    }

    async fn wait_for_gate_starts(gate: &TestAnalysisGate, expected: usize) {
        tokio::time::timeout(Duration::from_secs(1), async {
            while gate.started() < expected {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("analysis workers did not reach the test gate");
    }

    async fn admit_build(
        executor: &AnalysisExecutor,
        request: &AnalysisBuildRequest,
    ) -> AnalysisWaiter {
        executor
            .execute_work(
                AnalysisWorkKey::Build(request.key()),
                executor.inner.cancellation_parent.child(),
                || AnalysisWork::Build(Box::new(request.clone())),
            )
            .expect("analysis work should be admitted")
    }

    async fn fill_task_capacity_waiting_for_cpu(
        executor: &AnalysisExecutor,
        name: &str,
    ) -> (tokio::sync::OwnedSemaphorePermit, Vec<AnalysisWaiter>) {
        let cpu_permits = executor
            .inner
            .cpu_permits
            .clone()
            .acquire_many_owned(LSP_ANALYSIS_CONCURRENCY as u32)
            .await
            .expect("test should reserve every CPU permit");
        let mut waiters = Vec::with_capacity(LSP_ANALYSIS_IN_FLIGHT_LIMIT);
        for index in 0..LSP_ANALYSIS_IN_FLIGHT_LIMIT {
            let request = build_request(executor, &format!("{name}-{index}"));
            waiters.push(admit_build(executor, &request).await);
        }
        tokio::task::yield_now().await;
        assert_eq!(
            executor.registry_state(),
            (
                LSP_ANALYSIS_IN_FLIGHT_LIMIT,
                LSP_ANALYSIS_IN_FLIGHT_LIMIT,
                0
            )
        );
        assert_eq!(executor.available_task_slots(), 0);
        (cpu_permits, waiters)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn overlapping_identical_analysis_requests_share_one_cpu_execution() {
        let executor = test_executor();
        let gate = Arc::new(TestAnalysisGate::default());
        let request = build_request(&executor, "single-flight").with_test_gate(Arc::clone(&gate));
        let key = request.key();

        let spawn_execution = || {
            let executor = executor.clone();
            let request = request.clone();
            tokio::spawn(async move { executor.execute(&request).await })
        };
        let first = spawn_execution();
        wait_for_gate_starts(&gate, 1).await;
        let second = spawn_execution();
        let third = spawn_execution();
        wait_for_waiter_count(&executor, &key, 3).await;
        gate.release();

        let (first, second, third) = tokio::join!(first, second, third);
        let first = first.unwrap().unwrap();
        let second = second.unwrap().unwrap();
        let third = third.unwrap().unwrap();

        assert!(Arc::ptr_eq(first.snapshot(), second.snapshot()));
        assert!(Arc::ptr_eq(first.snapshot(), third.snapshot()));
        assert_eq!(executor.execution_count(), 1);
        drop((first, second, third));
        wait_for_registry_state(&executor, (0, 0, LSP_ANALYSIS_CONCURRENCY)).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn completed_analysis_stays_single_flight_until_all_leases_drop() {
        let executor = test_executor();
        let request = build_request(&executor, "completed-single-flight");

        let first = executor.execute(&request).await.unwrap();
        let second = executor.execute(&request).await.unwrap();

        assert!(Arc::ptr_eq(first.snapshot(), second.snapshot()));
        assert_eq!(
            executor.execution_count(),
            1,
            "a completed job must remain joinable while a caller retains its lease"
        );
        wait_for_registry_state(&executor, (1, 0, LSP_ANALYSIS_CONCURRENCY)).await;
        assert_eq!(executor.registry_state(), (1, 0, LSP_ANALYSIS_CONCURRENCY));

        drop(first);
        assert_eq!(executor.registry_state().0, 1);
        drop(second);
        assert_eq!(executor.registry_state().0, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn completed_reprojection_stays_single_flight_until_all_leases_drop() {
        let executor = test_executor();
        let request = diagnostic_reprojection_request(&executor, "reprojection-single-flight");

        let first = executor
            .execute_diagnostic_reprojection(&request)
            .await
            .unwrap();
        let second = executor
            .execute_diagnostic_reprojection(&request)
            .await
            .unwrap();

        assert!(first.shares_result_with(&second));
        assert_eq!(executor.reprojection_count(), 1);
        wait_for_registry_state(&executor, (1, 0, LSP_ANALYSIS_CONCURRENCY)).await;
        assert_eq!(executor.registry_state(), (1, 0, LSP_ANALYSIS_CONCURRENCY));

        drop(first);
        assert_eq!(executor.registry_state().0, 1);
        drop(second);
        assert_eq!(executor.registry_state().0, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cancelling_one_shared_waiter_does_not_cancel_the_analysis() {
        let executor = test_executor();
        let gate = Arc::new(TestAnalysisGate::default());
        let request = build_request(&executor, "shared-waiter-cancellation")
            .with_test_gate(Arc::clone(&gate));
        let key = request.key();

        let first_executor = executor.clone();
        let first_request = request.clone();
        let first = tokio::spawn(async move { first_executor.execute(&first_request).await });
        wait_for_gate_starts(&gate, 1).await;

        let second_executor = executor.clone();
        let second_request = request.clone();
        let second = tokio::spawn(async move { second_executor.execute(&second_request).await });
        wait_for_waiter_count(&executor, &key, 2).await;

        first.abort();
        let _ = first.await;
        wait_for_waiter_count(&executor, &key, 1).await;
        assert!(!second.is_finished());

        gate.release();
        let analysis = second.await.unwrap().unwrap();
        assert_eq!(executor.execution_count(), 1);
        drop(analysis);
        wait_for_registry_state(&executor, (0, 0, LSP_ANALYSIS_CONCURRENCY)).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn dropping_the_last_waiter_cancels_unfinished_work_and_releases_capacity() {
        let executor = test_executor();
        let gate = Arc::new(TestAnalysisGate::default());
        let request =
            build_request(&executor, "last-waiter-cancellation").with_test_gate(Arc::clone(&gate));
        let execution = tokio::spawn({
            let executor = executor.clone();
            async move { executor.execute(&request).await }
        });
        wait_for_gate_starts(&gate, 1).await;
        assert_eq!(
            executor.registry_state(),
            (1, 1, LSP_ANALYSIS_CONCURRENCY - 1)
        );

        execution.abort();
        let _ = execution.await;

        wait_for_job_count(&executor, 0).await;
        wait_for_available_cpu_permits(&executor, LSP_ANALYSIS_CONCURRENCY).await;
        executor.wait_idle().await;
        assert_eq!(executor.execution_count(), 1);
        assert_eq!(executor.registry_state(), (0, 0, LSP_ANALYSIS_CONCURRENCY));
        gate.release();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn last_waiters_cancel_jobs_without_releasing_task_capacity_before_worker_exit() {
        let executor = test_executor();
        let exit_gate = Arc::new(TestWorkerExitGate::default());
        executor.set_worker_exit_gate(Arc::clone(&exit_gate));
        let (cpu_permits, waiters) =
            fill_task_capacity_waiting_for_cpu(&executor, "last-waiter-capacity").await;
        let jobs = waiters
            .iter()
            .map(|waiter| Arc::clone(&waiter.job))
            .collect::<Vec<_>>();

        drop(waiters);

        assert!(
            lock_recovering_poison(&executor.inner.registry)
                .jobs
                .is_empty(),
            "the final waiter should remove each job from the single-flight registry immediately"
        );
        assert!(
            jobs.iter()
                .all(|job| job.is_cancelled() && job.is_complete())
        );
        exit_gate
            .wait_for_workers(LSP_ANALYSIS_IN_FLIGHT_LIMIT)
            .await;
        assert_eq!(
            executor.running_worker_count(),
            LSP_ANALYSIS_IN_FLIGHT_LIMIT
        );
        assert_eq!(executor.available_task_slots(), 0);

        let next = build_request(&executor, "after-last-waiter-capacity");
        let next_executor = executor.clone();
        let mut next = Box::pin(async move { next_executor.execute(&next).await });
        assert!(
            matches!(futures::poll!(&mut next), Poll::Pending),
            "a ninth physical worker must wait while cancelled workers have not exited"
        );
        assert_eq!(executor.execution_count(), 0);

        let next = tokio::spawn(next);
        exit_gate.release();
        wait_for_job_count(&executor, 1).await;
        wait_for_running_workers(&executor, 1).await;
        drop(cpu_permits);
        let next = next
            .await
            .expect("next analysis task should not panic")
            .expect("next analysis should enter after an old worker exits");
        drop(next);
        executor.wait_idle().await;
        wait_for_registry_state(&executor, (0, 0, LSP_ANALYSIS_CONCURRENCY)).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn session_cancellation_stops_running_analysis() {
        let cancellation = AnalysisCancellationToken::new();
        let executor = AnalysisExecutor::new(cancellation.clone());
        let gate = Arc::new(TestAnalysisGate::default());
        let request =
            build_request(&executor, "session-cancellation").with_test_gate(Arc::clone(&gate));
        let execution = tokio::spawn({
            let executor = executor.clone();
            async move { executor.execute(&request).await }
        });
        wait_for_gate_starts(&gate, 1).await;

        cancellation.cancel();

        let error = tokio::time::timeout(Duration::from_secs(1), execution)
            .await
            .expect("session cancellation did not stop running analysis")
            .expect("analysis task should not panic")
            .expect_err("session cancellation must reject analysis");
        assert!(error.is_stale());
        wait_for_registry_state(&executor, (0, 0, LSP_ANALYSIS_CONCURRENCY)).await;
        gate.release();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn invalidation_rejects_a_completed_old_generation() {
        let executor = test_executor();
        let uri = test_uri("completed-generation-invalidation");
        let first = build_request_for_uri(&executor, uri.clone(), 1, DocumentEpoch(1), TEST_SOURCE);
        let first_generation = first.analysis_job_generation();
        let first_lease = executor.execute(&first).await.unwrap();

        executor.invalidate(&uri);
        assert!(executor.execute(&first).await.unwrap_err().is_stale());

        let second =
            build_request_for_uri(&executor, uri, 2, DocumentEpoch(2), "flowchart TD\nA-->C\n");
        assert_ne!(second.analysis_job_generation(), first_generation);
        let second_lease = executor.execute(&second).await.unwrap();

        assert_eq!(executor.execution_count(), 2);
        drop((first_lease, second_lease));
        wait_for_registry_state(&executor, (0, 0, LSP_ANALYSIS_CONCURRENCY)).await;
    }

    #[test]
    #[should_panic(expected = "analysis job generation sequence exhausted")]
    fn document_generation_overflow_panics_instead_of_reusing_an_old_generation() {
        let mut registry = AnalysisRegistry {
            next_generation: u64::MAX,
            ..AnalysisRegistry::default()
        };

        registry.generation_for(&test_uri("generation-overflow"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn invalidation_wakes_waiters_but_retains_task_capacity_until_workers_exit() {
        let executor = test_executor();
        let exit_gate = Arc::new(TestWorkerExitGate::default());
        executor.set_worker_exit_gate(Arc::clone(&exit_gate));
        let (cpu_permits, waiters) =
            fill_task_capacity_waiting_for_cpu(&executor, "invalidated-capacity").await;

        executor.invalidate_all();

        assert!(
            lock_recovering_poison(&executor.inner.registry)
                .jobs
                .is_empty(),
            "invalidation should clear the single-flight registry synchronously"
        );
        for waiter in &waiters {
            let mut result = Box::pin(waiter.wait());
            assert!(
                matches!(futures::poll!(&mut result), Poll::Ready(Err(error)) if error.is_stale()),
                "invalidation should make every waiter ready synchronously"
            );
        }
        exit_gate
            .wait_for_workers(LSP_ANALYSIS_IN_FLIGHT_LIMIT)
            .await;
        assert_eq!(
            executor.running_worker_count(),
            LSP_ANALYSIS_IN_FLIGHT_LIMIT
        );
        assert_eq!(executor.available_task_slots(), 0);

        let next = build_request(&executor, "after-invalidated-capacity");
        let next_executor = executor.clone();
        let mut next = Box::pin(async move { next_executor.execute(&next).await });
        assert!(
            matches!(futures::poll!(&mut next), Poll::Pending),
            "the ninth physical worker must not spawn while invalidated workers retain all slots"
        );
        assert_eq!(executor.execution_count(), 0);

        drop(waiters);
        let next = tokio::spawn(next);
        exit_gate.release();
        wait_for_job_count(&executor, 1).await;
        wait_for_running_workers(&executor, 1).await;
        drop(cpu_permits);
        let next = next
            .await
            .expect("next analysis task should not panic")
            .expect("next analysis should enter after an invalidated worker exits");
        drop(next);
        executor.wait_idle().await;
        wait_for_registry_state(&executor, (0, 0, LSP_ANALYSIS_CONCURRENCY)).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn only_two_distinct_jobs_may_enter_cpu_work_concurrently() {
        let executor = test_executor();
        let gate = Arc::new(TestAnalysisGate::default());
        let executions = (0..LSP_ANALYSIS_CONCURRENCY + 1)
            .map(|index| {
                let request = build_request(&executor, &format!("cpu-bound-{index}"))
                    .with_test_gate(Arc::clone(&gate));
                let executor = executor.clone();
                tokio::spawn(async move { executor.execute(&request).await })
            })
            .collect::<Vec<_>>();

        wait_for_job_count(&executor, LSP_ANALYSIS_CONCURRENCY + 1).await;
        wait_for_gate_starts(&gate, LSP_ANALYSIS_CONCURRENCY).await;
        assert_eq!(gate.started(), LSP_ANALYSIS_CONCURRENCY);
        assert_eq!(executor.execution_count(), LSP_ANALYSIS_CONCURRENCY);
        assert_eq!(
            executor.registry_state(),
            (
                LSP_ANALYSIS_CONCURRENCY + 1,
                LSP_ANALYSIS_CONCURRENCY + 1,
                0
            )
        );

        gate.release();
        let mut leases = Vec::with_capacity(executions.len());
        for execution in executions {
            leases.push(execution.await.unwrap().unwrap());
        }
        assert_eq!(executor.execution_count(), LSP_ANALYSIS_CONCURRENCY + 1);
        wait_for_registry_state(
            &executor,
            (LSP_ANALYSIS_CONCURRENCY + 1, 0, LSP_ANALYSIS_CONCURRENCY),
        )
        .await;
        assert_eq!(
            executor.registry_state(),
            (LSP_ANALYSIS_CONCURRENCY + 1, 0, LSP_ANALYSIS_CONCURRENCY)
        );

        drop(leases);
        wait_for_registry_state(&executor, (0, 0, LSP_ANALYSIS_CONCURRENCY)).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn completed_leases_do_not_consume_distinct_job_capacity() {
        let executor = test_executor();
        let mut leases = Vec::with_capacity(LSP_ANALYSIS_IN_FLIGHT_LIMIT);
        for index in 0..LSP_ANALYSIS_IN_FLIGHT_LIMIT {
            let request = build_request(&executor, &format!("completed-capacity-{index}"));
            leases.push(executor.execute(&request).await.unwrap());
        }
        wait_for_registry_state(
            &executor,
            (LSP_ANALYSIS_IN_FLIGHT_LIMIT, 0, LSP_ANALYSIS_CONCURRENCY),
        )
        .await;
        assert_eq!(
            executor.registry_state(),
            (LSP_ANALYSIS_IN_FLIGHT_LIMIT, 0, LSP_ANALYSIS_CONCURRENCY)
        );

        let ninth = build_request(&executor, "completed-capacity-next");
        let ninth = tokio::time::timeout(Duration::from_secs(1), executor.execute(&ninth))
            .await
            .expect("completed leases must not block a new distinct job")
            .expect("the new distinct job should succeed");
        wait_for_registry_state(
            &executor,
            (
                LSP_ANALYSIS_IN_FLIGHT_LIMIT + 1,
                0,
                LSP_ANALYSIS_CONCURRENCY,
            ),
        )
        .await;
        assert_eq!(
            executor.registry_state(),
            (
                LSP_ANALYSIS_IN_FLIGHT_LIMIT + 1,
                0,
                LSP_ANALYSIS_CONCURRENCY
            )
        );

        drop(ninth);
        drop(leases);
        wait_for_registry_state(&executor, (0, 0, LSP_ANALYSIS_CONCURRENCY)).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn invalidating_running_analysis_releases_the_uri_for_a_fresh_generation() {
        let executor = test_executor();
        let uri = test_uri("running-generation");
        let gate = Arc::new(TestAnalysisGate::default());
        let stale_request =
            build_request_for_uri(&executor, uri.clone(), 1, DocumentEpoch(1), TEST_SOURCE)
                .with_test_gate(Arc::clone(&gate));
        let stale_executor = executor.clone();
        let stale = tokio::spawn(async move { stale_executor.execute(&stale_request).await });
        wait_for_gate_starts(&gate, 1).await;

        executor.invalidate(&uri);
        let fresh_request = build_request_for_uri(
            &executor,
            uri.clone(),
            2,
            DocumentEpoch(2),
            "flowchart TD\nA-->C\n",
        );
        assert!(stale.await.unwrap().unwrap_err().is_stale());

        let analysis =
            tokio::time::timeout(Duration::from_secs(1), executor.execute(&fresh_request))
                .await
                .expect("fresh generation remained blocked behind stale CPU work")
                .expect("fresh generation should succeed");
        gate.release();

        assert_eq!(executor.execution_count(), 2);
        drop(analysis);
        wait_for_registry_state(&executor, (0, 0, LSP_ANALYSIS_CONCURRENCY)).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn request_invalidated_before_registration_is_rejected() {
        let executor = test_executor();
        let uri = test_uri("stale-before-register");
        let request =
            build_request_for_uri(&executor, uri.clone(), 1, DocumentEpoch(1), TEST_SOURCE);

        executor.invalidate(&uri);

        assert!(executor.execute(&request).await.is_err());
        assert_eq!(executor.execution_count(), 0);
        assert!(
            lock_recovering_poison(&executor.inner.registry)
                .jobs
                .is_empty()
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn invalidate_all_rejects_every_old_generation_and_allocates_fresh_ones() {
        let executor = test_executor();
        let first_uri = test_uri("invalidate-all-first");
        let second_uri = test_uri("invalidate-all-second");
        let first = build_request_for_uri(
            &executor,
            first_uri.clone(),
            1,
            DocumentEpoch(1),
            TEST_SOURCE,
        );
        let second = build_request_for_uri(
            &executor,
            second_uri.clone(),
            1,
            DocumentEpoch(1),
            TEST_SOURCE,
        );
        let old_generations = (
            first.analysis_job_generation(),
            second.analysis_job_generation(),
        );

        executor.invalidate_all();

        assert!(executor.execute(&first).await.unwrap_err().is_stale());
        assert!(executor.execute(&second).await.unwrap_err().is_stale());
        assert_eq!(executor.execution_count(), 0);
        assert!(
            lock_recovering_poison(&executor.inner.registry)
                .document_generations
                .is_empty()
        );

        let fresh_first =
            build_request_for_uri(&executor, first_uri, 2, DocumentEpoch(2), TEST_SOURCE);
        let fresh_second =
            build_request_for_uri(&executor, second_uri, 2, DocumentEpoch(2), TEST_SOURCE);
        assert_ne!(fresh_first.analysis_job_generation(), old_generations.0);
        assert_ne!(fresh_second.analysis_job_generation(), old_generations.1);

        let (fresh_first, fresh_second) = tokio::join!(
            executor.execute(&fresh_first),
            executor.execute(&fresh_second),
        );
        let fresh_first = fresh_first.unwrap();
        let fresh_second = fresh_second.unwrap();
        assert_eq!(executor.execution_count(), 2);
        drop((fresh_first, fresh_second));
        wait_for_registry_state(&executor, (0, 0, LSP_ANALYSIS_CONCURRENCY)).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn forgetting_a_uri_does_not_reuse_its_generation() {
        let executor = test_executor();
        let uri = test_uri("reopened");
        let stale_request =
            build_request_for_uri(&executor, uri.clone(), 1, DocumentEpoch(1), TEST_SOURCE);
        let stale_generation = stale_request.analysis_job_generation();

        executor.forget(&uri);

        assert!(
            lock_recovering_poison(&executor.inner.registry)
                .document_generations
                .is_empty(),
            "closed documents must not remain in the generation registry"
        );

        let fresh_request = build_request_for_uri(&executor, uri, 1, DocumentEpoch(2), TEST_SOURCE);
        assert_ne!(fresh_request.analysis_job_generation(), stale_generation);

        assert!(executor.execute(&stale_request).await.is_err());
        assert_eq!(executor.execution_count(), 0);
        let fresh = executor.execute(&fresh_request).await.unwrap();
        assert_eq!(executor.execution_count(), 1);
        drop(fresh);
        wait_for_registry_state(&executor, (0, 0, LSP_ANALYSIS_CONCURRENCY)).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn mixed_cancelled_queue_is_bounded_and_releases_capacity_for_the_next_job() {
        let executor = test_executor();
        let cpu_permits = executor
            .inner
            .cpu_permits
            .clone()
            .acquire_many_owned(LSP_ANALYSIS_CONCURRENCY as u32)
            .await
            .unwrap();

        let mut requests = (0..LSP_ANALYSIS_IN_FLIGHT_LIMIT)
            .map(|index| {
                build_request_for_uri(
                    &executor,
                    test_uri(&format!("cancel-queued-{index}")),
                    1,
                    DocumentEpoch(1),
                    TEST_SOURCE,
                )
            })
            .collect::<Vec<_>>();

        let ninth_request = requests.pop().expect("expected ninth request");
        let ninth_key = ninth_request.key();

        let mut executions = requests
            .into_iter()
            .map(|request| {
                let executor = executor.clone();
                tokio::spawn(async move { executor.execute(&request).await })
            })
            .collect::<Vec<_>>();
        let projection_request =
            diagnostic_reprojection_request(&executor, "cancel-queued-reprojection");
        let projection_executor = executor.clone();
        let projection = tokio::spawn(async move {
            projection_executor
                .execute_diagnostic_reprojection(&projection_request)
                .await
        });
        wait_for_job_count(&executor, LSP_ANALYSIS_IN_FLIGHT_LIMIT).await;
        assert_eq!(
            executor.registry_state(),
            (
                LSP_ANALYSIS_IN_FLIGHT_LIMIT,
                LSP_ANALYSIS_IN_FLIGHT_LIMIT,
                0
            )
        );

        let ninth_executor = executor.clone();
        let duplicate_request = ninth_request.clone();
        let mut ninth_future =
            Box::pin(async move { ninth_executor.execute(&ninth_request).await });
        let duplicate_executor = executor.clone();
        let mut duplicate_future =
            Box::pin(async move { duplicate_executor.execute(&duplicate_request).await });
        assert!(
            futures::poll!(&mut ninth_future).is_pending(),
            "the ninth request must wait instead of being rejected"
        );
        assert!(
            futures::poll!(&mut duplicate_future).is_pending(),
            "an identical ninth request must wait on the same bounded admission"
        );
        assert_eq!(
            executor.pending_admission_count(),
            1,
            "the ninth key and its duplicate must share one bounded rendezvous"
        );
        for index in 0..32 {
            let extra = build_request(&executor, &format!("saturated-extra-{index}"));
            let error = tokio::time::timeout(Duration::from_secs(1), executor.execute(&extra))
                .await
                .expect("a different saturated key must fail without entering a queue")
                .expect_err("a second pending key must be rejected");
            assert!(error.is_overloaded());
        }
        assert_eq!(executor.pending_admission_count(), 1);
        let ninth = tokio::spawn(ninth_future);
        let duplicate = tokio::spawn(duplicate_future);
        assert_eq!(
            lock_recovering_poison(&executor.inner.registry).jobs.len(),
            LSP_ANALYSIS_IN_FLIGHT_LIMIT,
            "the ninth distinct request must remain outside the job registry"
        );

        projection.abort();
        let _ = projection.await;
        wait_for_registered_job(&executor, &ninth_key).await;
        wait_for_waiter_count(&executor, &ninth_key, 2).await;
        assert!(
            !ninth.is_finished(),
            "the ninth request should continue once queue capacity is released"
        );

        for execution in &executions {
            execution.abort();
        }
        ninth.abort();
        duplicate.abort();
        for execution in executions.drain(..) {
            let _ = execution.await;
        }
        let _ = ninth.await;
        let _ = duplicate.await;
        wait_for_job_count(&executor, 0).await;

        assert_eq!(executor.execution_count(), 0);
        assert_eq!(executor.reprojection_count(), 0);
        drop(cpu_permits);
        wait_for_available_cpu_permits(&executor, LSP_ANALYSIS_CONCURRENCY).await;
        assert_eq!(executor.registry_state(), (0, 0, LSP_ANALYSIS_CONCURRENCY));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn invalidating_reprojections_releases_admission_without_cancelling_builds() {
        let executor = test_executor();
        let gate = Arc::new(TestAnalysisGate::default());
        let preserved_build =
            build_request(&executor, "preserved-build").with_test_gate(Arc::clone(&gate));
        let preserved_executor = executor.clone();
        let preserved =
            tokio::spawn(async move { preserved_executor.execute(&preserved_build).await });
        wait_for_gate_starts(&gate, 1).await;
        let cpu_permit = executor
            .inner
            .cpu_permits
            .clone()
            .acquire_owned()
            .await
            .unwrap();

        let executions = (0..LSP_ANALYSIS_IN_FLIGHT_LIMIT - 1)
            .map(|index| {
                let request = diagnostic_reprojection_request(
                    &executor,
                    &format!("invalidate-reprojection-{index}"),
                );
                let executor = executor.clone();
                tokio::spawn(
                    async move { executor.execute_diagnostic_reprojection(&request).await },
                )
            })
            .collect::<Vec<_>>();

        wait_for_registry_state(
            &executor,
            (
                LSP_ANALYSIS_IN_FLIGHT_LIMIT,
                LSP_ANALYSIS_IN_FLIGHT_LIMIT,
                0,
            ),
        )
        .await;

        executor.invalidate_reprojections();

        for execution in executions {
            assert!(
                execution
                    .await
                    .expect("diagnostic projection task should not panic")
                    .expect_err("invalidated diagnostic projection must be rejected")
                    .is_stale()
            );
        }
        wait_for_registry_state(&executor, (1, 1, 0)).await;
        assert!(
            !preserved.is_finished(),
            "diagnostic invalidation must not cancel the canonical snapshot build"
        );

        let build = build_request(&executor, "build-after-reprojection-invalidation");
        let build_executor = executor.clone();
        let build = tokio::spawn(async move { build_executor.execute(&build).await });
        wait_for_registry_state(&executor, (2, 2, 0)).await;
        assert!(
            !build.is_finished(),
            "the fresh build should be admitted even while CPU remains occupied"
        );

        gate.release();
        let preserved = preserved
            .await
            .expect("preserved build task should not panic")
            .expect("preserved build should survive diagnostic invalidation");
        let build = build
            .await
            .expect("fresh build task should not panic")
            .expect("fresh build should finish after capacity becomes available");
        drop((preserved, build));
        drop(cpu_permit);
        wait_for_available_cpu_permits(&executor, LSP_ANALYSIS_CONCURRENCY).await;
        wait_for_registry_state(&executor, (0, 0, LSP_ANALYSIS_CONCURRENCY)).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn invalidating_a_running_reprojection_retains_capacity_until_its_worker_unwinds() {
        let executor = test_executor();
        let gate = Arc::new(TestAnalysisGate::default());
        let exit_gate = Arc::new(TestWorkerExitGate::default());
        executor.set_worker_exit_gate(Arc::clone(&exit_gate));
        let reprojection = diagnostic_reprojection_request(&executor, "running-reprojection")
            .with_test_gate(Arc::clone(&gate));
        let running_executor = executor.clone();
        let running = tokio::spawn(async move {
            running_executor
                .execute_diagnostic_reprojection(&reprojection)
                .await
        });
        wait_for_gate_starts(&gate, 1).await;
        assert_eq!(
            executor.registry_state(),
            (1, 1, LSP_ANALYSIS_CONCURRENCY - 1)
        );

        executor.invalidate_reprojections();
        exit_gate.wait_for_workers(1).await;

        assert!(
            running
                .await
                .expect("running reprojection task should not panic")
                .expect_err("invalidated running reprojection must be rejected")
                .is_stale()
        );
        assert_eq!(
            executor.registry_state().0,
            0,
            "the invalidated reprojection must leave the registry before its worker returns"
        );
        assert_eq!(
            executor.running_worker_count(),
            1,
            "the invalidated reprojection must retain task capacity until its worker returns"
        );
        assert_eq!(
            executor.available_task_slots(),
            LSP_ANALYSIS_IN_FLIGHT_LIMIT - 1
        );
        assert_eq!(
            executor.registry_state(),
            (0, 1, LSP_ANALYSIS_CONCURRENCY),
            "the spawn-blocking work has unwound, but the physical worker is still held at exit"
        );

        let mut idle = Box::pin(executor.wait_idle());
        assert!(
            matches!(futures::poll!(&mut idle), Poll::Pending),
            "worker-idle observation must include the retained task-capacity lease"
        );

        exit_gate.release();
        idle.await;
        wait_for_registry_state(&executor, (0, 0, LSP_ANALYSIS_CONCURRENCY)).await;
        assert_eq!(
            executor.available_task_slots(),
            LSP_ANALYSIS_IN_FLIGHT_LIMIT
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn invalidated_running_analyses_release_cpu_for_the_latest_generation() {
        let executor = test_executor();
        let gate = Arc::new(TestAnalysisGate::default());

        let stale = (0..LSP_ANALYSIS_CONCURRENCY)
            .map(|index| {
                let uri = test_uri(&format!("stale-running-{index}"));
                let request =
                    build_request_for_uri(&executor, uri.clone(), 1, DocumentEpoch(1), TEST_SOURCE)
                        .with_test_gate(Arc::clone(&gate));
                (uri, request)
            })
            .collect::<Vec<_>>();

        let stale_executions = stale
            .iter()
            .map(|(_, request)| {
                let executor = executor.clone();
                let request = request.clone();
                tokio::spawn(async move { executor.execute(&request).await })
            })
            .collect::<Vec<_>>();
        wait_for_gate_starts(&gate, LSP_ANALYSIS_CONCURRENCY).await;
        assert_eq!(
            executor.registry_state(),
            (LSP_ANALYSIS_CONCURRENCY, LSP_ANALYSIS_CONCURRENCY, 0)
        );

        for (uri, _) in &stale {
            executor.invalidate(uri);
        }

        let latest = build_request_for_uri(
            &executor,
            test_uri("latest-running"),
            1,
            DocumentEpoch(1),
            "flowchart TD\nA-->C\n",
        );
        let latest = tokio::time::timeout(Duration::from_secs(1), executor.execute(&latest))
            .await
            .expect("latest analysis remained blocked behind stale CPU work")
            .expect("latest analysis should succeed");
        drop(latest);

        for execution in stale_executions {
            assert!(
                execution
                    .await
                    .expect("stale analysis task should not panic")
                    .expect_err("stale analysis must be cancelled")
                    .is_stale()
            );
        }
        gate.release();
        wait_for_registry_state(&executor, (0, 0, LSP_ANALYSIS_CONCURRENCY)).await;
        assert_eq!(
            executor.inner.cpu_permits.available_permits(),
            LSP_ANALYSIS_CONCURRENCY
        );
    }
}
