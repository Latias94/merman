use crate::session::analysis::request::{
    AnalysisBuildError, AnalysisBuildKey, AnalysisBuildRequest, AnalysisJobGeneration,
};
use crate::snapshot::{
    DiagnosticGeneration, DocumentAnalysisContext, DocumentEpoch, SnapshotGeneration,
};
use crate::sync::lock_recovering_poison;
use merman_analysis::{AnalysisCancellationToken, AnalysisCancelled, Analyzer};
use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};
use tokio::sync::{Notify, Semaphore};
use tower_lsp_server::ls_types::Uri;

/// Maximum number of document analyses that may consume CPU concurrently.
pub(in crate::session) const LSP_ANALYSIS_CONCURRENCY: usize = 2;
/// Maximum number of distinct analyses that may be running or waiting for CPU.
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

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct DiagnosticReprojectionKey {
    uri: Uri,
    analysis_job_generation: AnalysisJobGeneration,
    document_epoch: DocumentEpoch,
    snapshot_generation: SnapshotGeneration,
    source_diagnostic_generation: DiagnosticGeneration,
    target_diagnostic_generation: DiagnosticGeneration,
    source_identity: usize,
}

#[derive(Debug, Clone)]
pub(in crate::session) struct DiagnosticReprojectionRequest {
    analyzer: Analyzer,
    cancellation: AnalysisCancellationToken,
    target_diagnostic_generation: DiagnosticGeneration,
    uri: Uri,
    analysis_job_generation: AnalysisJobGeneration,
    document_epoch: DocumentEpoch,
    snapshot_generation: SnapshotGeneration,
    source_diagnostic_generation: DiagnosticGeneration,
    context: Arc<DocumentAnalysisContext>,
}

#[derive(Debug, Clone)]
struct DiagnosticReprojectionResult {
    uri: Uri,
    analysis_job_generation: AnalysisJobGeneration,
    document_epoch: DocumentEpoch,
    snapshot_generation: SnapshotGeneration,
    source_diagnostic_generation: DiagnosticGeneration,
    target_diagnostic_generation: DiagnosticGeneration,
    original: Arc<DocumentAnalysisContext>,
    projected: Arc<DocumentAnalysisContext>,
}

impl DiagnosticReprojectionRequest {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::session) fn new(
        analyzer: Analyzer,
        cancellation: AnalysisCancellationToken,
        target_diagnostic_generation: DiagnosticGeneration,
        uri: Uri,
        analysis_job_generation: AnalysisJobGeneration,
        document_epoch: DocumentEpoch,
        snapshot_generation: SnapshotGeneration,
        source_diagnostic_generation: DiagnosticGeneration,
        context: Arc<DocumentAnalysisContext>,
    ) -> Self {
        Self {
            analyzer,
            cancellation,
            target_diagnostic_generation,
            uri,
            analysis_job_generation,
            document_epoch,
            snapshot_generation,
            source_diagnostic_generation,
            context,
        }
    }

    fn key(&self) -> DiagnosticReprojectionKey {
        DiagnosticReprojectionKey {
            uri: self.uri.clone(),
            analysis_job_generation: self.analysis_job_generation,
            document_epoch: self.document_epoch,
            snapshot_generation: self.snapshot_generation,
            source_diagnostic_generation: self.source_diagnostic_generation,
            target_diagnostic_generation: self.target_diagnostic_generation,
            source_identity: Arc::as_ptr(&self.context) as usize,
        }
    }

    fn cancellation_child(&self) -> AnalysisCancellationToken {
        self.cancellation.child()
    }

    pub(in crate::session) fn uri(&self) -> &Uri {
        &self.uri
    }

    fn project_with_cancellation(
        self,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<DiagnosticReprojectionResult, AnalysisCancelled> {
        cancellation.checkpoint()?;
        let projected = Arc::new(
            self.context
                .reproject_cancellable(&self.analyzer, cancellation)?,
        );
        cancellation.checkpoint()?;
        Ok(DiagnosticReprojectionResult {
            uri: self.uri,
            analysis_job_generation: self.analysis_job_generation,
            document_epoch: self.document_epoch,
            snapshot_generation: self.snapshot_generation,
            source_diagnostic_generation: self.source_diagnostic_generation,
            target_diagnostic_generation: self.target_diagnostic_generation,
            original: self.context,
            projected,
        })
    }
}

impl DiagnosticReprojectionKey {
    fn uri(&self) -> &Uri {
        &self.uri
    }

    fn analysis_job_generation(&self) -> AnalysisJobGeneration {
        self.analysis_job_generation
    }
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
    Reproject(Arc<DiagnosticReprojectionResult>),
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

pub(in crate::session) struct AnalysisExecutionLease {
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
    pub(in crate::session) fn context(&self) -> &Arc<DocumentAnalysisContext> {
        &self.context
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
    pub(in crate::session) fn uri(&self) -> &Uri {
        &self.result.uri
    }

    pub(in crate::session) fn analysis_job_generation(&self) -> AnalysisJobGeneration {
        self.result.analysis_job_generation
    }

    pub(in crate::session) fn document_epoch(&self) -> DocumentEpoch {
        self.result.document_epoch
    }

    pub(in crate::session) fn snapshot_generation(&self) -> SnapshotGeneration {
        self.result.snapshot_generation
    }

    pub(in crate::session) fn source_diagnostic_generation(&self) -> DiagnosticGeneration {
        self.result.source_diagnostic_generation
    }

    pub(in crate::session) fn target_diagnostic_generation(&self) -> DiagnosticGeneration {
        self.result.target_diagnostic_generation
    }

    pub(in crate::session) fn original(&self) -> &Arc<DocumentAnalysisContext> {
        &self.result.original
    }

    pub(in crate::session) fn projected(&self) -> &Arc<DocumentAnalysisContext> {
        &self.result.projected
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
pub(in crate::session) struct AnalysisExecutionError {
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

    pub(in crate::session) fn is_stale(&self) -> bool {
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
    pub(in crate::session) fn new(cancellation_parent: AnalysisCancellationToken) -> Self {
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

    pub(in crate::session) async fn execute_diagnostic_reprojection(
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
            release_active_job(&inner, &job);
            if job.is_cancelled() || job.has_error() {
                remove_job_if_registered(&inner, &key, &job);
            }
        });
    }

    pub(in crate::session) fn invalidate(&self, uri: &Uri) {
        self.invalidate_uri(uri);
    }

    pub(in crate::session) fn forget(&self, uri: &Uri) {
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

    pub(in crate::session) fn invalidate_all(&self) {
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
    use merman_editor_core::DocumentKind;
    use std::str::FromStr;
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
                DiagnosticGeneration(1),
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
        let context = build.build().expect("test analysis should be ready");
        DiagnosticReprojectionRequest::new(
            analyzer.with_diagnostic_policy(analyzer.options().diagnostic_policy().clone()),
            AnalysisCancellationToken::new(),
            DiagnosticGeneration(2),
            uri,
            build.analysis_job_generation(),
            build.document_epoch(),
            build.snapshot_generation(),
            build.diagnostic_generation(),
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

        assert!(Arc::ptr_eq(first.context(), second.context()));
        assert!(Arc::ptr_eq(first.context(), third.context()));
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

        assert!(Arc::ptr_eq(first.context(), second.context()));
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
        assert_eq!(executor.execution_count(), 1);
        assert_eq!(executor.registry_state(), (0, 0, LSP_ANALYSIS_CONCURRENCY));
        gate.release();
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
