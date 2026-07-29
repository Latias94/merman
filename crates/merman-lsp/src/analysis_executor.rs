use crate::analysis_request::{
    AnalysisBuildError, AnalysisBuildKey, AnalysisBuildRequest, AnalysisJobGeneration,
};
use crate::snapshot::DocumentAnalysisContext;
use crate::sync::lock_recovering_poison;
use merman_analysis::AnalysisCancellationToken;
use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};
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
}

#[derive(Default)]
struct AnalysisRegistry {
    jobs: HashMap<AnalysisBuildKey, Arc<AnalysisJob>>,
    next_generation: u64,
    document_generations: HashMap<Uri, AnalysisJobGeneration>,
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
    result: Mutex<Option<Result<Arc<DocumentAnalysisContext>, AnalysisExecutionError>>>,
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

    async fn wait(&self) -> Result<Arc<DocumentAnalysisContext>, AnalysisExecutionError> {
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

    fn complete(&self, result: Result<Arc<DocumentAnalysisContext>, AnalysisExecutionError>) {
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
    key: AnalysisBuildKey,
    job: Arc<AnalysisJob>,
}

impl AnalysisWaiter {
    fn new(
        inner: &Arc<AnalysisExecutorInner>,
        key: AnalysisBuildKey,
        job: Arc<AnalysisJob>,
    ) -> Self {
        job.waiters.fetch_add(1, Ordering::Relaxed);
        Self {
            inner: Arc::downgrade(inner),
            key,
            job,
        }
    }

    async fn wait(self) -> Result<Arc<DocumentAnalysisContext>, AnalysisExecutionError> {
        self.job.wait().await
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
                (!self.job.is_complete(), removed)
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
    ) -> Result<Arc<DocumentAnalysisContext>, AnalysisExecutionError> {
        let key = request.key();
        loop {
            let capacity_changed = self.inner.capacity_changed.notified();
            let admission = {
                let mut registry = lock_recovering_poison(&self.inner.registry);
                if Some(request.analysis_job_generation())
                    != registry.current_generation_for(request.uri())
                {
                    return Err(AnalysisExecutionError::stale());
                }
                if let Some(job) = registry.jobs.get(&key) {
                    Some((
                        AnalysisWaiter::new(&self.inner, key.clone(), Arc::clone(job)),
                        None,
                    ))
                } else if registry.jobs.len() < LSP_ANALYSIS_IN_FLIGHT_LIMIT {
                    let job = Arc::new(AnalysisJob::new(self.inner.cancellation_parent.child()));
                    let waiter = AnalysisWaiter::new(&self.inner, key.clone(), Arc::clone(&job));
                    registry.jobs.insert(key.clone(), Arc::clone(&job));
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
                self.start(key.clone(), request.clone(), job);
            }
            return waiter.wait().await;
        }
    }

    fn start(&self, key: AnalysisBuildKey, request: AnalysisBuildRequest, job: Arc<AnalysisJob>) {
        let inner = Arc::clone(&self.inner);
        tokio::spawn(async move {
            let result = tokio::select! {
                _ = job.cancelled() => Err(AnalysisExecutionError::cancelled()),
                permit = Arc::clone(&inner.cpu_permits).acquire_owned() => match permit {
                Ok(permit) if !job.is_cancelled() => {
                    #[cfg(test)]
                    inner
                        .execution_count
                        .fetch_add(1, Ordering::Relaxed);
                    let cancellation = job.cancellation.clone();
                    tokio::task::spawn_blocking(move || {
                        let _permit = permit;
                        request.build_cancellable(&cancellation)
                    })
                    .await
                    .map_err(|error| {
                        AnalysisExecutionError::new(format!(
                            "document analysis worker failed: {error}"
                        ))
                    })
                    .and_then(|result| match result {
                        Ok(context) => Ok(context),
                        Err(AnalysisBuildError::Cancelled(_)) => {
                            Err(AnalysisExecutionError::cancelled())
                        }
                        Err(AnalysisBuildError::Rejected(rejection)) => {
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
        self.inner.capacity_changed.notify_waiters();
    }

    pub(crate) fn invalidate_all(&self) {
        let cancelled = {
            let mut registry = lock_recovering_poison(&self.inner.registry);
            registry.document_generations.clear();
            registry
                .jobs
                .drain()
                .map(|(_, job)| job)
                .collect::<Vec<_>>()
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
}

fn remove_job_if_registered(
    inner: &AnalysisExecutorInner,
    key: &AnalysisBuildKey,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_request::{AnalysisBuildKey, TestAnalysisGate};
    use crate::document_store::DocumentStore;
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
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if lock_recovering_poison(&executor.inner.registry)
                    .jobs
                    .contains_key(key)
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
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                let waiters = lock_recovering_poison(&executor.inner.registry)
                    .jobs
                    .get(key)
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

        assert!(Arc::ptr_eq(&first, &second));
        assert!(Arc::ptr_eq(&first, &third));
        assert_eq!(executor.execution_count(), 1);
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
        assert!(store.insert_built_analysis(&request, analysis).is_some());
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
    async fn completed_analysis_is_released_from_single_flight_registry() {
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
            lock_recovering_poison(&executor.inner.registry)
                .jobs
                .is_empty(),
            "the final waiter should release a completed single-flight job"
        );
        assert!(store.insert_built_analysis(&request, analysis).is_some());
        assert!(
            lock_recovering_poison(&executor.inner.registry)
                .jobs
                .is_empty(),
            "committed contexts should be owned only by the document store"
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
                .insert_built_analysis(&fresh_request, analysis)
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
