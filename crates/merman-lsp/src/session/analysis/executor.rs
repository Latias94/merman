use crate::session::analysis::request::{
    AnalysisBuildError, AnalysisBuildKey, AnalysisBuildRequest, AnalysisJobGeneration,
};
use crate::session::documents::{
    DiagnosticReprojectionBatch, DiagnosticReprojectionKey, DiagnosticReprojectionRequest,
};
use crate::snapshot::DocumentAnalysisContext;
use crate::sync::lock_recovering_poison;
use merman_analysis::AnalysisCancellationToken;
use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};
use tokio::sync::{Notify, Semaphore};
use tower_lsp_server::ls_types::Uri;

/// Maximum number of document analyses that may consume CPU concurrently.
pub(crate) const LSP_ANALYSIS_CONCURRENCY: usize = 2;
/// Maximum number of distinct analyses that may be running or waiting for CPU.
pub(crate) const LSP_ANALYSIS_IN_FLIGHT_LIMIT: usize = 8;

#[derive(Clone)]
pub(crate) struct AnalysisExecutor {
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
    capacity_changed: Notify,
    cancellation_parent: AnalysisCancellationToken,
    registry: Mutex<AnalysisRegistry>,
    #[cfg(test)]
    execution_count: AtomicUsize,
    #[cfg(test)]
    reprojection_count: AtomicUsize,
}

#[derive(Default)]
struct AnalysisRegistry {
    jobs: HashMap<AnalysisWorkKey, Arc<AnalysisJob>>,
    active_distinct: usize,
    next_generation: u64,
    document_generations: HashMap<Uri, AnalysisJobGeneration>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
enum AnalysisWorkKey {
    Build(AnalysisBuildKey),
    Reproject(DiagnosticReprojectionKey),
}

impl AnalysisWorkKey {
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
    Build(AnalysisBuildRequest),
    Reproject(DiagnosticReprojectionRequest),
}

#[derive(Clone)]
enum AnalysisWorkOutput {
    Build(Arc<DocumentAnalysisContext>),
    Reproject(Arc<DiagnosticReprojectionBatch>),
}

impl AnalysisRegistry {
    fn generation_for(&mut self, uri: &Uri) -> AnalysisJobGeneration {
        if let Some(generation) = self.document_generations.get(uri) {
            return *generation;
        }

        self.next_generation = self.next_generation.wrapping_add(1);
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
    active: AtomicBool,
}

impl AnalysisJob {
    fn new(cancellation: AnalysisCancellationToken) -> Self {
        Self {
            result: Mutex::new(None),
            ready: Notify::new(),
            cancellation,
            cancellation_signal: Notify::new(),
            waiters: AtomicUsize::new(0),
            active: AtomicBool::new(true),
        }
    }

    async fn wait(&self) -> Result<AnalysisWorkOutput, AnalysisExecutionError> {
        loop {
            let notified = self.ready.notified();
            if let Some(result) = lock_recovering_poison(&self.result).clone() {
                return result;
            }
            notified.await;
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
            if self.is_cancelled() {
                return;
            }
            notified.await;
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

pub(crate) struct AnalysisExecutionLease {
    context: Arc<DocumentAnalysisContext>,
    _waiter: AnalysisWaiter,
}

impl fmt::Debug for AnalysisExecutionLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AnalysisExecutionLease")
            .field("context", &self.context)
            .finish_non_exhaustive()
    }
}

impl AnalysisExecutionLease {
    pub(crate) fn context(&self) -> &Arc<DocumentAnalysisContext> {
        &self.context
    }
}

pub(crate) struct DiagnosticReprojectionLease {
    batch: Arc<DiagnosticReprojectionBatch>,
    _waiter: AnalysisWaiter,
}

impl fmt::Debug for DiagnosticReprojectionLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DiagnosticReprojectionLease")
            .field("batch", &self.batch)
            .finish_non_exhaustive()
    }
}

impl DiagnosticReprojectionLease {
    pub(crate) fn batch(&self) -> &Arc<DiagnosticReprojectionBatch> {
        &self.batch
    }
}

impl Drop for AnalysisWaiter {
    fn drop(&mut self) {
        let Some(inner) = self.inner.upgrade() else {
            return;
        };

        let (should_cancel, released_capacity) = {
            let mut registry = lock_recovering_poison(&inner.registry);
            let previous = self.job.waiters.fetch_sub(1, Ordering::Relaxed);
            debug_assert!(previous > 0, "analysis waiter count underflow");
            if previous == 1 {
                let mut removed = false;
                if registry
                    .jobs
                    .get(&self.key)
                    .is_some_and(|registered| Arc::ptr_eq(registered, &self.job))
                {
                    registry.jobs.remove(&self.key);
                    removed = true;
                }
                let should_cancel = !self.job.is_complete();
                let released_active =
                    should_cancel && self.job.active.swap(false, Ordering::AcqRel);
                if released_active {
                    registry.active_distinct = registry.active_distinct.saturating_sub(1);
                }
                (should_cancel, removed || released_active)
            } else {
                (false, false)
            }
        };

        if released_capacity {
            inner.capacity_changed.notify_waiters();
        }
        if should_cancel {
            self.job.cancel(AnalysisExecutionError::cancelled());
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct AnalysisExecutionError {
    message: Arc<str>,
    kind: AnalysisExecutionErrorKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnalysisExecutionErrorKind {
    Internal,
    Stale,
    Cancelled,
    ResourceRejected,
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
            kind: AnalysisExecutionErrorKind::Cancelled,
        }
    }

    fn resource_rejected(rejection: merman_analysis::AnalysisRejection) -> Self {
        Self {
            message: format!(
                "document analysis rejected after LSP preflight: source is {} bytes, exceeding max_source_bytes {}",
                rejection.source_len(),
                rejection.max_source_bytes()
            )
            .into(),
            kind: AnalysisExecutionErrorKind::ResourceRejected,
        }
    }

    pub(crate) fn is_stale(&self) -> bool {
        matches!(
            self.kind,
            AnalysisExecutionErrorKind::Stale | AnalysisExecutionErrorKind::Cancelled
        )
    }
}

impl fmt::Display for AnalysisExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for AnalysisExecutionError {}

impl AnalysisExecutor {
    pub(crate) fn new(cancellation_parent: AnalysisCancellationToken) -> Self {
        Self {
            inner: Arc::new(AnalysisExecutorInner {
                cpu_permits: Arc::new(Semaphore::new(LSP_ANALYSIS_CONCURRENCY)),
                capacity_changed: Notify::new(),
                cancellation_parent,
                registry: Mutex::new(AnalysisRegistry::default()),
                #[cfg(test)]
                execution_count: AtomicUsize::new(0),
                #[cfg(test)]
                reprojection_count: AtomicUsize::new(0),
            }),
        }
    }

    pub(crate) fn generation_for(&self, uri: &Uri) -> AnalysisJobGeneration {
        lock_recovering_poison(&self.inner.registry).generation_for(uri)
    }

    pub(crate) fn is_generation_current(
        &self,
        uri: &Uri,
        generation: AnalysisJobGeneration,
    ) -> bool {
        lock_recovering_poison(&self.inner.registry).current_generation_for(uri) == Some(generation)
    }

    pub(crate) async fn execute(
        &self,
        request: &AnalysisBuildRequest,
    ) -> Result<AnalysisExecutionLease, AnalysisExecutionError> {
        let key = AnalysisWorkKey::Build(request.key());
        let waiter = self
            .execute_work(
                key,
                AnalysisWork::Build(request.clone()),
                self.inner.cancellation_parent.child(),
            )
            .await?;
        let output = waiter.wait().await?;
        let AnalysisWorkOutput::Build(context) = output else {
            return Err(AnalysisExecutionError::new(
                "analysis executor returned a diagnostic projection for a build request",
            ));
        };
        Ok(AnalysisExecutionLease {
            context,
            _waiter: waiter,
        })
    }

    pub(crate) async fn execute_diagnostic_reprojection(
        &self,
        request: &DiagnosticReprojectionRequest,
    ) -> Result<DiagnosticReprojectionLease, AnalysisExecutionError> {
        let key = AnalysisWorkKey::Reproject(request.key());
        let waiter = self
            .execute_work(
                key,
                AnalysisWork::Reproject(request.clone()),
                request.cancellation_child(),
            )
            .await?;
        let output = waiter.wait().await?;
        let AnalysisWorkOutput::Reproject(batch) = output else {
            return Err(AnalysisExecutionError::new(
                "analysis executor returned a build result for a diagnostic projection request",
            ));
        };
        Ok(DiagnosticReprojectionLease {
            batch,
            _waiter: waiter,
        })
    }

    async fn execute_work(
        &self,
        key: AnalysisWorkKey,
        work: AnalysisWork,
        cancellation: AnalysisCancellationToken,
    ) -> Result<AnalysisWaiter, AnalysisExecutionError> {
        let mut work = Some(work);
        loop {
            let capacity_changed = self.inner.capacity_changed.notified();
            let admission = {
                let mut registry = lock_recovering_poison(&self.inner.registry);
                if Some(key.analysis_job_generation()) != registry.current_generation_for(key.uri())
                {
                    return Err(AnalysisExecutionError::stale());
                }
                if let Some(job) = registry.jobs.get(&key) {
                    Some((
                        AnalysisWaiter::new(&self.inner, key.clone(), Arc::clone(job)),
                        None,
                    ))
                } else if registry.active_distinct < LSP_ANALYSIS_IN_FLIGHT_LIMIT {
                    let job = Arc::new(AnalysisJob::new(cancellation.clone()));
                    let waiter = AnalysisWaiter::new(&self.inner, key.clone(), Arc::clone(&job));
                    registry.jobs.insert(key.clone(), Arc::clone(&job));
                    registry.active_distinct += 1;
                    Some((waiter, Some(job)))
                } else {
                    None
                }
            };

            let Some((waiter, start)) = admission else {
                capacity_changed.await;
                continue;
            };
            if let Some(job) = start {
                self.start(
                    key.clone(),
                    work.take()
                        .expect("newly admitted analysis work must retain its request"),
                    job,
                );
            }
            return Ok(waiter);
        }
    }

    fn start(&self, key: AnalysisWorkKey, work: AnalysisWork, job: Arc<AnalysisJob>) {
        let inner = Arc::clone(&self.inner);
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
                                .map(AnalysisWorkOutput::Build)
                                .map_err(AnalysisWorkError::Build),
                            AnalysisWork::Reproject(request) => request
                                .project_with_cancellation(&cancellation)
                                .map(|batch| AnalysisWorkOutput::Reproject(Arc::new(batch)))
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
            release_active_job(&inner, &job);
            if job.is_cancelled() || job.has_error() {
                remove_job_if_registered(&inner, &key, &job);
            }
        });
    }

    pub(crate) fn invalidate(&self, uri: &Uri) {
        self.invalidate_uri(uri);
    }

    pub(crate) fn forget(&self, uri: &Uri) {
        self.invalidate_uri(uri);
    }

    fn invalidate_uri(&self, uri: &Uri) {
        let cancelled = {
            let mut registry = lock_recovering_poison(&self.inner.registry);
            registry.document_generations.remove(uri);
            let mut cancelled = Vec::new();
            let mut released_active = 0usize;
            registry.jobs.retain(|key, job| {
                if key.uri() == uri {
                    if job.active.swap(false, Ordering::AcqRel) {
                        released_active += 1;
                    }
                    cancelled.push(Arc::clone(job));
                    false
                } else {
                    true
                }
            });
            registry.active_distinct = registry.active_distinct.saturating_sub(released_active);
            cancelled
        };
        for job in cancelled {
            job.cancel(AnalysisExecutionError::stale());
        }
        self.inner.capacity_changed.notify_waiters();
    }

    pub(crate) fn invalidate_all(&self) {
        let cancelled = {
            let mut registry = lock_recovering_poison(&self.inner.registry);
            registry.document_generations.clear();
            let cancelled = registry
                .jobs
                .drain()
                .map(|(_, job)| job)
                .collect::<Vec<_>>();
            for job in &cancelled {
                job.active.store(false, Ordering::Release);
            }
            registry.active_distinct = 0;
            cancelled
        };
        for job in cancelled {
            job.cancel(AnalysisExecutionError::stale());
        }
        self.inner.capacity_changed.notify_waiters();
    }

    #[cfg(test)]
    pub(crate) fn execution_count(&self) -> usize {
        self.inner.execution_count.load(Ordering::Relaxed)
    }

    #[cfg(test)]
    pub(crate) fn reprojection_count(&self) -> usize {
        self.inner.reprojection_count.load(Ordering::Relaxed)
    }

    #[cfg(test)]
    pub(crate) fn registry_state(&self) -> (usize, usize, usize) {
        let registry = lock_recovering_poison(&self.inner.registry);
        (
            registry.jobs.len(),
            registry.active_distinct,
            self.inner.cpu_permits.available_permits(),
        )
    }
}

fn release_active_job(inner: &AnalysisExecutorInner, job: &AnalysisJob) {
    if !job.active.swap(false, Ordering::AcqRel) {
        return;
    }
    let mut registry = lock_recovering_poison(&inner.registry);
    registry.active_distinct = registry.active_distinct.saturating_sub(1);
    drop(registry);
    inner.capacity_changed.notify_waiters();
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
        drop(registry);
        inner.capacity_changed.notify_waiters();
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
    use crate::session::documents::DocumentStore;
    use merman_analysis::{AnalysisRuleConfig, DiagnosticSeverity};
    use merman_editor_core::DocumentKind;
    use std::str::FromStr;
    use std::time::Duration;

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
            while gate.started() != expected {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("analysis workers did not reach the test gate");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn overlapping_identical_analysis_requests_share_one_cpu_execution() {
        let mut store = DocumentStore::new();
        let uri = Uri::from_str("file:///tmp/single-flight.mmd").unwrap();
        store.upsert_text(
            uri.clone(),
            1,
            "flowchart TD\nA-->B\n".to_string(),
            DocumentKind::Diagram,
        );
        let gate = Arc::new(TestAnalysisGate::default());
        let request = store
            .snapshot_build_request(&uri)
            .expect("expected analysis request")
            .with_test_gate(Arc::clone(&gate));
        let key = request.key();
        let executor = store.analysis_executor();

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

        assert!(Arc::ptr_eq(first.context(), second.context()));
        assert!(Arc::ptr_eq(first.context(), third.context()));
        assert_eq!(executor.execution_count(), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn completed_analysis_stays_single_flight_until_its_caller_commits() {
        let mut store = DocumentStore::new();
        let uri = Uri::from_str("file:///tmp/commit-lease.mmd").unwrap();
        store.upsert_text(
            uri.clone(),
            1,
            "flowchart TD\nA-->B\n".to_string(),
            DocumentKind::Diagram,
        );
        let request = store
            .snapshot_build_request(&uri)
            .expect("expected analysis request");
        let executor = store.analysis_executor();

        let first = executor.execute(&request).await.unwrap();
        let second = executor.execute(&request).await.unwrap();

        assert!(Arc::ptr_eq(first.context(), second.context()));
        assert_eq!(
            executor.execution_count(),
            1,
            "a completed job must remain joinable through the guarded commit window"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn completed_reprojection_stays_single_flight_and_commits_idempotently() {
        let mut store = DocumentStore::new();
        let uri = Uri::from_str("file:///tmp/reprojection-lease.mmd").unwrap();
        store.upsert_text(
            uri.clone(),
            1,
            "flowchart TD\nA-->B\n".to_string(),
            DocumentKind::Diagram,
        );
        let build = store
            .snapshot_build_request(&uri)
            .expect("expected analysis request");
        let executor = store.analysis_executor();
        let analysis = executor.execute(&build).await.unwrap();
        store
            .insert_built_analysis(&build, Arc::clone(analysis.context()))
            .expect("analysis should commit");
        drop(analysis);

        let options = store.analyzer_options().clone().with_rule_config(
            AnalysisRuleConfig::default()
                .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Hint)
                .unwrap(),
        );
        let (_, plan) = store.begin_analyzer_options(options);
        assert!(
            plan.is_some(),
            "diagnostic update should capture reprojection work"
        );
        let request = store
            .diagnostic_reprojection_request(&uri)
            .expect("expected request-local reprojection");

        let first = executor
            .execute_diagnostic_reprojection(&request)
            .await
            .unwrap();
        let second = executor
            .execute_diagnostic_reprojection(&request)
            .await
            .unwrap();

        assert!(Arc::ptr_eq(first.batch(), second.batch()));
        assert_eq!(executor.reprojection_count(), 1);

        let first_context = store
            .commit_diagnostic_reprojection_context(&uri, first.batch().as_ref().clone())
            .expect("first equivalent waiter should commit");
        let second_context = store
            .commit_diagnostic_reprojection_context(&uri, second.batch().as_ref().clone())
            .expect("second equivalent waiter should observe the committed context");
        assert!(Arc::ptr_eq(
            &first_context.snapshot,
            &second_context.snapshot
        ));
        assert_eq!(
            first_context.diagnostic_generation(),
            second_context.diagnostic_generation()
        );

        store.upsert_text(
            uri.clone(),
            2,
            "flowchart TD\nA-->C\n".to_string(),
            DocumentKind::Diagram,
        );
        let current_build = store
            .snapshot_build_request(&uri)
            .expect("expected replacement analysis request");
        let current = executor.execute(&current_build).await.unwrap();
        store
            .insert_built_analysis(&current_build, Arc::clone(current.context()))
            .expect("replacement analysis should commit");

        assert!(
            store
                .commit_diagnostic_reprojection_context(&uri, first.batch().as_ref().clone())
                .is_none(),
            "an old projection must not fall through to an unrelated current context"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cancelling_one_shared_waiter_does_not_cancel_the_analysis() {
        let mut store = DocumentStore::new();
        let uri = Uri::from_str("file:///tmp/shared-waiter-cancellation.mmd").unwrap();
        store.upsert_text(
            uri.clone(),
            1,
            "flowchart TD\nA-->B\n".to_string(),
            DocumentKind::Diagram,
        );
        let gate = Arc::new(TestAnalysisGate::default());
        let request = store
            .snapshot_build_request(&uri)
            .expect("expected analysis request")
            .with_test_gate(Arc::clone(&gate));
        let key = request.key();
        let executor = store.analysis_executor();

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
        assert!(
            store
                .insert_built_analysis(&request, Arc::clone(analysis.context()))
                .is_some()
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn session_cancellation_stops_running_analysis() {
        let cancellation = AnalysisCancellationToken::new();
        let mut store = DocumentStore::with_session_cancellation(cancellation.clone());
        let uri = Uri::from_str("file:///tmp/session-cancellation.mmd").unwrap();
        store.upsert_text(
            uri.clone(),
            1,
            "flowchart TD\nA-->B\n".to_string(),
            DocumentKind::Diagram,
        );
        let gate = Arc::new(TestAnalysisGate::default());
        let request = store
            .snapshot_build_request(&uri)
            .expect("expected analysis request")
            .with_test_gate(Arc::clone(&gate));
        let executor = store.analysis_executor();
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
        gate.release();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn committed_analysis_is_released_from_single_flight_registry() {
        let mut store = DocumentStore::new();
        let uri = Uri::from_str("file:///tmp/committed-single-flight.mmd").unwrap();
        store.upsert_text(
            uri.clone(),
            1,
            "flowchart TD\nA-->B\n".to_string(),
            DocumentKind::Diagram,
        );
        let request = store
            .snapshot_build_request(&uri)
            .expect("expected analysis request");
        let executor = store.analysis_executor();
        let analysis = executor.execute(&request).await.unwrap();

        assert!(
            !lock_recovering_poison(&executor.inner.registry)
                .jobs
                .is_empty(),
            "the execution lease must retain the completed single-flight job"
        );
        assert!(
            store
                .insert_built_analysis(&request, Arc::clone(analysis.context()))
                .is_some()
        );
        drop(analysis);
        assert!(
            lock_recovering_poison(&executor.inner.registry)
                .jobs
                .is_empty(),
            "the completed job should be released after guarded commit"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn document_epoch_change_invalidates_completed_single_flight_result() {
        let mut store = DocumentStore::new();
        let uri = Uri::from_str("file:///tmp/single-flight.mmd").unwrap();
        store.upsert_text(
            uri.clone(),
            1,
            "flowchart TD\nA-->B\n".to_string(),
            DocumentKind::Diagram,
        );
        let executor = store.analysis_executor();
        let first = store
            .snapshot_build_request(&uri)
            .expect("expected first analysis request");
        executor.execute(&first).await.unwrap();

        store.upsert_text(
            uri.clone(),
            2,
            "flowchart TD\nA-->C\n".to_string(),
            DocumentKind::Diagram,
        );
        let second = store
            .snapshot_build_request(&uri)
            .expect("expected second analysis request");
        executor.execute(&second).await.unwrap();

        assert_eq!(executor.execution_count(), 2);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn document_update_invalidates_running_analysis_for_the_same_uri() {
        let mut store = DocumentStore::new();
        let uri = Uri::from_str("file:///tmp/running-generation.mmd").unwrap();
        store.upsert_text(
            uri.clone(),
            1,
            "flowchart TD\nA-->B\n".to_string(),
            DocumentKind::Diagram,
        );
        let gate = Arc::new(TestAnalysisGate::default());
        let stale_request = store
            .snapshot_build_request(&uri)
            .expect("expected stale analysis request")
            .with_test_gate(Arc::clone(&gate));
        let executor = store.analysis_executor();
        let stale_executor = executor.clone();
        let stale = tokio::spawn(async move { stale_executor.execute(&stale_request).await });
        wait_for_gate_starts(&gate, 1).await;

        store.upsert_text(
            uri.clone(),
            2,
            "flowchart TD\nA-->C\n".to_string(),
            DocumentKind::Diagram,
        );
        assert!(stale.await.unwrap().unwrap_err().is_stale());

        let fresh_request = store
            .snapshot_build_request(&uri)
            .expect("expected fresh analysis request");
        let analysis =
            tokio::time::timeout(Duration::from_secs(1), executor.execute(&fresh_request))
                .await
                .expect("fresh generation remained blocked behind stale CPU work")
                .expect("fresh generation should succeed");
        gate.release();

        assert_eq!(executor.execution_count(), 2);
        assert!(
            store
                .insert_built_analysis(&fresh_request, Arc::clone(analysis.context()))
                .is_some()
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn request_invalidated_before_registration_is_rejected() {
        let mut store = DocumentStore::new();
        let uri = Uri::from_str("file:///tmp/stale-before-register.mmd").unwrap();
        store.upsert_text(
            uri.clone(),
            1,
            "flowchart TD\nA-->B\n".to_string(),
            DocumentKind::Diagram,
        );
        let request = store
            .snapshot_build_request(&uri)
            .expect("expected analysis request");
        let executor = store.analysis_executor();

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
    async fn closing_document_forgets_generation_without_reusing_it_on_reopen() {
        let mut store = DocumentStore::new();
        let uri = Uri::from_str("file:///tmp/reopened.mmd").unwrap();
        store.upsert_text(
            uri.clone(),
            1,
            "flowchart TD\nA-->B\n".to_string(),
            DocumentKind::Diagram,
        );
        let stale_request = store
            .snapshot_build_request(&uri)
            .expect("expected request before close");
        let stale_generation = stale_request.analysis_job_generation();
        let executor = store.analysis_executor();

        store.remove(&uri);

        assert!(
            lock_recovering_poison(&executor.inner.registry)
                .document_generations
                .is_empty(),
            "closed documents must not remain in the generation registry"
        );

        store.upsert_text(
            uri.clone(),
            1,
            "flowchart TD\nA-->B\n".to_string(),
            DocumentKind::Diagram,
        );
        let fresh_request = store
            .snapshot_build_request(&uri)
            .expect("expected request after reopen");
        assert_ne!(fresh_request.analysis_job_generation(), stale_generation);

        assert!(executor.execute(&stale_request).await.is_err());
        assert_eq!(executor.execution_count(), 0);
        executor.execute(&fresh_request).await.unwrap();
        assert_eq!(executor.execution_count(), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cancelled_queue_is_bounded_and_releases_capacity_for_the_next_job() {
        let mut store = DocumentStore::new();
        let executor = store.analysis_executor();
        let cpu_permits = executor
            .inner
            .cpu_permits
            .clone()
            .acquire_many_owned(LSP_ANALYSIS_CONCURRENCY as u32)
            .await
            .unwrap();

        let mut requests = (0..=LSP_ANALYSIS_IN_FLIGHT_LIMIT)
            .map(|index| {
                let uri = Uri::from_str(&format!("file:///tmp/cancel-queued-{index}.mmd")).unwrap();
                store.upsert_text(
                    uri.clone(),
                    1,
                    "flowchart TD\nA-->B\n".to_string(),
                    DocumentKind::Diagram,
                );
                store
                    .snapshot_build_request(&uri)
                    .expect("expected analysis request")
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
        wait_for_job_count(&executor, LSP_ANALYSIS_IN_FLIGHT_LIMIT).await;

        let ninth_executor = executor.clone();
        let duplicate_request = ninth_request.clone();
        let mut ninth = tokio::spawn(async move { ninth_executor.execute(&ninth_request).await });
        let duplicate_executor = executor.clone();
        let mut duplicate =
            tokio::spawn(async move { duplicate_executor.execute(&duplicate_request).await });
        assert!(
            tokio::time::timeout(Duration::from_millis(25), &mut ninth)
                .await
                .is_err(),
            "the ninth request must wait instead of being rejected"
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(25), &mut duplicate)
                .await
                .is_err(),
            "an identical ninth request must wait on the same bounded admission"
        );
        assert_eq!(
            lock_recovering_poison(&executor.inner.registry).jobs.len(),
            LSP_ANALYSIS_IN_FLIGHT_LIMIT,
            "the ninth distinct request must remain outside the job registry"
        );

        let first = executions.remove(0);
        first.abort();
        let _ = first.await;
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
        drop(cpu_permits);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn invalidated_running_analyses_release_cpu_for_the_latest_generation() {
        let mut store = DocumentStore::new();
        let executor = store.analysis_executor();
        let gate = Arc::new(TestAnalysisGate::default());

        let stale = (0..LSP_ANALYSIS_CONCURRENCY)
            .map(|index| {
                let uri = Uri::from_str(&format!("file:///tmp/stale-running-{index}.mmd")).unwrap();
                store.upsert_text(
                    uri.clone(),
                    1,
                    "flowchart TD\nA-->B\n".to_string(),
                    DocumentKind::Diagram,
                );
                let request = store
                    .snapshot_build_request(&uri)
                    .expect("expected stale analysis request")
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
        assert_eq!(executor.inner.cpu_permits.available_permits(), 0);

        for (uri, _) in &stale {
            executor.invalidate(uri);
        }

        let latest_uri = Uri::from_str("file:///tmp/latest-running.mmd").unwrap();
        store.upsert_text(
            latest_uri.clone(),
            1,
            "flowchart TD\nA-->C\n".to_string(),
            DocumentKind::Diagram,
        );
        let latest = store
            .snapshot_build_request(&latest_uri)
            .expect("expected latest analysis request");
        tokio::time::timeout(Duration::from_secs(1), executor.execute(&latest))
            .await
            .expect("latest analysis remained blocked behind stale CPU work")
            .expect("latest analysis should succeed");

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
        wait_for_job_count(&executor, 0).await;
        wait_for_available_cpu_permits(&executor, LSP_ANALYSIS_CONCURRENCY).await;
        assert_eq!(
            executor.inner.cpu_permits.available_permits(),
            LSP_ANALYSIS_CONCURRENCY
        );
    }
}
