use super::*;

impl SessionState {
    #[cfg(test)]
    pub(in crate::session) fn set_analysis_test_gate(
        &mut self,
        gate: Option<Arc<TestAnalysisGate>>,
    ) {
        self.analysis_test_gate = gate;
    }

    pub(in crate::session) fn commit_diagnostic_reprojection(
        &mut self,
        request: &DiagnosticReprojectionRequest,
        projection: &DiagnosticReprojectionLease,
    ) -> Option<SnapshotContext> {
        self.commit_projected_context(request, projection.projected())
    }

    fn commit_projected_context(
        &mut self,
        request: &DiagnosticReprojectionRequest,
        projected: &Arc<DocumentAnalysisContext>,
    ) -> Option<SnapshotContext> {
        let uri = request.uri();
        let record = self.documents.get(uri)?;
        if record.epoch != request.document_epoch()
            || self.snapshot_generation != request.snapshot_generation()
            || self.diagnostic_generation != request.target_diagnostic_generation()
            || !self
                .analysis_executor
                .is_generation_current(uri, request.analysis_job_generation())
        {
            return None;
        }
        if projected.analysis_result_identity() != request.analysis_result_identity() {
            return None;
        }
        if projected.diagnostic_generation() != request.target_diagnostic_generation() {
            return None;
        }

        let context = SnapshotContext::with_analysis(
            Arc::clone(&projected.snapshot),
            Arc::clone(projected.diagnostic_round_trip()),
            request.snapshot_generation(),
            request.target_diagnostic_generation(),
            request.document_epoch(),
        );
        if let Some(authority) = request.cache_authority() {
            let _ = self.analysis_cache.promote(
                uri,
                authority,
                request.target_diagnostic_generation(),
                Arc::clone(projected),
            );
        }
        Some(context)
    }

    pub(in crate::session) fn diagnostic_context(&self, uri: &Uri) -> Option<DiagnosticContext> {
        self.documents.get(uri).map(|record| {
            DiagnosticContext::new(
                record.document.clone(),
                self.diagnostic_generation,
                record.epoch,
            )
        })
    }

    pub(in crate::session) fn is_diagnostic_context_current(
        &self,
        context: &DiagnosticContext,
    ) -> bool {
        self.diagnostic_generation == context.generation
            && self.is_document_epoch_current(&context.document.uri, context.document_epoch)
    }

    pub(in crate::session) fn acquired_snapshot_for_uri(
        &mut self,
        uri: &Uri,
    ) -> Option<AcquiredSnapshot> {
        let stamp = self.analysis_cache_stamp(uri)?;
        let cached = self.analysis_cache.lookup(uri, stamp)?;
        Some(AcquiredSnapshot::new(Arc::clone(cached.snapshot()), stamp))
    }

    fn analysis_cache_stamp(&self, uri: &Uri) -> Option<AnalysisCacheStamp> {
        Some(AnalysisCacheStamp {
            document_epoch: self.documents.get(uri)?.epoch,
            snapshot_generation: self.snapshot_generation,
            analysis_job_generation: self.analysis_executor.generation_for(uri),
        })
    }

    pub(in crate::session) fn snapshot_build_request_after_cache_miss(
        &self,
        uri: &Uri,
    ) -> Option<AnalysisBuildRequest> {
        let record = self.documents.get(uri)?;
        if record.document.is_analysis_unavailable() {
            return None;
        }
        let key = AnalysisBuildKey::new(
            record.document.uri.clone(),
            record.document.version,
            self.analysis_executor.generation_for(uri),
            self.snapshot_generation,
            record.epoch,
        );
        let request = AnalysisBuildRequest::new(
            key,
            Arc::clone(record.document.analysis_text()?),
            record.document.kind,
            self.analyzer.clone(),
        );
        #[cfg(test)]
        let request = match &self.analysis_test_gate {
            Some(gate) => request.with_test_gate(Arc::clone(gate)),
            None => request,
        };
        Some(request)
    }

    pub(in crate::session) fn commit_built_snapshot(
        &mut self,
        request: &AnalysisBuildRequest,
        analysis: &AnalysisExecutionLease,
    ) -> Option<AcquiredSnapshot> {
        self.commit_built_snapshot_inner(request, Arc::clone(analysis.snapshot()), || {
            analysis.claim_cache_admission()
        })
    }

    fn commit_built_snapshot_inner(
        &mut self,
        request: &AnalysisBuildRequest,
        snapshot: Arc<DocumentSnapshot>,
        claim_cache_admission: impl FnOnce() -> bool,
    ) -> Option<AcquiredSnapshot> {
        if self.snapshot_generation != request.snapshot_generation()
            || !self.is_document_epoch_current(request.uri(), request.document_epoch())
            || !self
                .analysis_executor
                .is_generation_current(request.uri(), request.analysis_job_generation())
        {
            return None;
        }
        let stamp = AnalysisCacheStamp {
            document_epoch: request.document_epoch(),
            snapshot_generation: request.snapshot_generation(),
            analysis_job_generation: request.analysis_job_generation(),
        };
        let cache_admission = claim_cache_admission();
        if let Some(current) = self
            .analysis_cache
            .current_without_touch(request.uri(), stamp)
        {
            return Some(AcquiredSnapshot::new(Arc::clone(current.snapshot()), stamp));
        }
        if cache_admission {
            let _ = self.analysis_cache.insert_snapshot(
                request.uri().clone(),
                stamp,
                Arc::clone(&snapshot),
            );
        }
        Some(AcquiredSnapshot::new(snapshot, stamp))
    }

    pub(in crate::session) fn projection_decision_for_snapshot(
        &self,
        snapshot: &AcquiredSnapshot,
    ) -> Option<ProjectionDecision> {
        if !self.is_acquired_snapshot_current(snapshot)
            || !self.analysis_executor.is_generation_current(
                snapshot.snapshot.uri(),
                snapshot.stamp.analysis_job_generation,
            )
        {
            return None;
        }
        if let Some(cached) = self
            .analysis_cache
            .current_without_touch(snapshot.snapshot.uri(), snapshot.stamp)
            .filter(|cached| Arc::ptr_eq(cached.snapshot(), &snapshot.snapshot))
        {
            if let Some(context) = cached.context()
                && context.diagnostic_generation() == self.diagnostic_generation
            {
                return Some(ProjectionDecision::Ready(Self::cached_snapshot_context(
                    context,
                    snapshot.stamp,
                )));
            }
            return Some(ProjectionDecision::Project(Box::new(
                self.diagnostic_reprojection_request_for_snapshot(
                    Arc::clone(cached.snapshot()),
                    snapshot.stamp,
                    Some(cached.authority()),
                ),
            )));
        }
        Some(ProjectionDecision::Project(Box::new(
            self.diagnostic_reprojection_request_for_snapshot(
                Arc::clone(&snapshot.snapshot),
                snapshot.stamp,
                None,
            ),
        )))
    }

    pub(in crate::session) fn is_acquired_snapshot_current(
        &self,
        snapshot: &AcquiredSnapshot,
    ) -> bool {
        self.snapshot_generation == snapshot.stamp.snapshot_generation
            && self
                .is_document_epoch_current(snapshot.snapshot.uri(), snapshot.stamp.document_epoch)
    }

    pub(in crate::session) fn is_snapshot_context_current(
        &self,
        context: &SnapshotContext,
    ) -> bool {
        self.snapshot_generation == context.generation
            && self.is_document_epoch_current(context.snapshot.uri(), context.document_epoch)
    }

    pub(in crate::session) fn is_analysis_context_current(
        &self,
        context: &SnapshotContext,
    ) -> bool {
        self.is_snapshot_context_current(context)
            && context.diagnostic_generation() == self.diagnostic_generation
    }

    pub(in crate::session) fn diagnostic_contexts_are_current(
        &self,
        diagnostic: &DiagnosticContext,
        analysis: Option<&SnapshotContext>,
    ) -> bool {
        self.is_diagnostic_context_current(diagnostic)
            && analysis.is_none_or(|context| self.is_analysis_context_current(context))
    }

    pub(in crate::session::documents) fn is_document_epoch_current(
        &self,
        uri: &Uri,
        document_epoch: DocumentEpoch,
    ) -> bool {
        self.documents
            .get(uri)
            .is_some_and(|record| record.epoch == document_epoch)
    }

    fn diagnostic_reprojection_request_for_snapshot(
        &self,
        snapshot: Arc<DocumentSnapshot>,
        stamp: AnalysisCacheStamp,
        cache_authority: Option<AnalysisCacheAuthority>,
    ) -> DiagnosticReprojectionRequest {
        let request = DiagnosticReprojectionRequest::new(
            self.analyzer.options().diagnostic_policy().clone(),
            self.diagnostic_reprojection_cancellation.clone(),
            self.diagnostic_generation,
            snapshot,
            stamp,
            cache_authority,
        );
        #[cfg(test)]
        let request = match &self.analysis_test_gate {
            Some(gate) => request.with_test_gate(Arc::clone(gate)),
            None => request,
        };
        request
    }

    pub(in crate::session) fn analysis_executor(&self) -> AnalysisExecutor {
        self.analysis_executor.clone()
    }

    pub(in crate::session::documents) fn diagnostic_uris(&self) -> Vec<Uri> {
        self.documents.keys().cloned().collect()
    }

    pub(in crate::session::documents) fn cached_snapshot_context(
        context: &Arc<DocumentAnalysisContext>,
        stamp: AnalysisCacheStamp,
    ) -> SnapshotContext {
        SnapshotContext::with_analysis(
            Arc::clone(&context.snapshot),
            Arc::clone(context.diagnostic_round_trip()),
            stamp.snapshot_generation,
            context.diagnostic_generation(),
            stamp.document_epoch,
        )
    }

    pub(in crate::session) fn diagnostic_state(
        &self,
        uri: &Uri,
    ) -> Option<DocumentDiagnosticState> {
        self.documents.get(uri).and_then(|record| {
            record.diagnostic_state.as_ref().and_then(|stored| {
                (stored.generation == self.diagnostic_generation
                    && stored.document_epoch == record.epoch)
                    .then(|| stored.state.clone())
            })
        })
    }

    pub(in crate::session) fn set_diagnostic_state_if_current(
        &mut self,
        context: &DiagnosticContext,
        state: DocumentDiagnosticState,
    ) -> bool {
        if !self.is_diagnostic_context_current(context) {
            return false;
        }

        let Some(record) = self.documents.get_mut(&context.document.uri) else {
            return false;
        };
        record.diagnostic_state = Some(StoredDiagnosticState {
            generation: context.generation,
            document_epoch: context.document_epoch,
            state,
        });
        true
    }
}

impl LanguageSession {
    pub(crate) async fn diagnostic_context(&self, uri: &Uri) -> Option<DiagnosticContext> {
        self.inner.state.lock().await.diagnostic_context(uri)
    }

    pub(crate) async fn diagnostic_uris(&self) -> Vec<Uri> {
        self.inner.state.lock().await.diagnostic_uris()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    const TEST_SOURCE: &str = "flowchart TD\nA-->B\n";

    fn test_uri(name: &str) -> Uri {
        Uri::from_str(&format!("file:///tmp/{name}.mmd")).expect("test URI must be valid")
    }

    fn state_with_cache_budget(cache_budget: usize) -> SessionState {
        let cancellation = AnalysisCancellationToken::new();
        SessionState::with_session_cancellation_and_cache_budget(cancellation, cache_budget)
    }

    fn open_test_document(state: &mut SessionState, uri: &Uri) {
        state.open_document_source(
            uri.clone(),
            1,
            DocumentSource::Available(Arc::from(TEST_SOURCE)),
            DocumentKind::Diagram,
        );
    }

    fn cache_contains(state: &SessionState, uri: &Uri) -> bool {
        let stamp = state
            .analysis_cache_stamp(uri)
            .expect("open test document must have a cache stamp");
        state
            .analysis_cache
            .current_without_touch(uri, stamp)
            .is_some()
    }

    fn exact_snapshot_cache_budget(uri: &Uri) -> usize {
        let mut state = state_with_cache_budget(DEFAULT_LSP_ANALYSIS_CACHE_BUDGET_BYTES);
        open_test_document(&mut state, uri);
        let request = state
            .snapshot_build_request_after_cache_miss(uri)
            .expect("available test document must produce a build request");
        let snapshot = request
            .build_cancellable(&AnalysisCancellationToken::new())
            .expect("test snapshot must build");
        state
            .commit_built_snapshot_inner(&request, snapshot, || true)
            .expect("current test snapshot must commit");
        state.analysis_cache.total_weight()
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn duplicate_build_waiter_cannot_resurrect_an_evicted_cache_entry() {
        let a = test_uri("duplicate-build-a");
        let b = test_uri("duplicate-build-b");
        let budget = exact_snapshot_cache_budget(&a);
        let mut state = state_with_cache_budget(budget);
        open_test_document(&mut state, &a);
        open_test_document(&mut state, &b);
        let executor = state.analysis_executor();

        let a_request = state
            .snapshot_build_request_after_cache_miss(&a)
            .expect("available test document must produce a build request");
        let (first, second) =
            tokio::join!(executor.execute(&a_request), executor.execute(&a_request));
        let first = first.expect("first shared build must complete");
        let second = second.expect("second shared build must complete");
        assert!(Arc::ptr_eq(first.snapshot(), second.snapshot()));

        state
            .commit_built_snapshot(&a_request, &first)
            .expect("the first waiter must admit the shared build");
        drop(first);
        assert!(cache_contains(&state, &a));

        let b_request = state
            .snapshot_build_request_after_cache_miss(&b)
            .expect("available test document must produce a build request");
        let b_analysis = executor
            .execute(&b_request)
            .await
            .expect("replacement build must complete");
        state
            .commit_built_snapshot(&b_request, &b_analysis)
            .expect("replacement build must commit");
        assert!(!cache_contains(&state, &a));
        assert!(cache_contains(&state, &b));
        let before_weight = state.analysis_cache.total_weight();
        let before_statistics = state.analysis_cache.statistics();

        let request_local = state
            .commit_built_snapshot(&a_request, &second)
            .expect("the late waiter must remain a request-local result");

        assert!(Arc::ptr_eq(&request_local.snapshot, second.snapshot()));
        assert!(!cache_contains(&state, &a));
        assert!(cache_contains(&state, &b));
        assert_eq!(state.analysis_cache.total_weight(), before_weight);
        assert_eq!(state.analysis_cache.statistics(), before_statistics);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cache_hit_consumes_a_new_build_outputs_admission_claim() {
        let a = test_uri("cache-hit-build-a");
        let b = test_uri("cache-hit-build-b");
        let budget = exact_snapshot_cache_budget(&a);
        let mut state = state_with_cache_budget(budget);
        open_test_document(&mut state, &a);
        open_test_document(&mut state, &b);
        let executor = state.analysis_executor();
        let a_request = state
            .snapshot_build_request_after_cache_miss(&a)
            .expect("available test document must produce a build request");

        let initial = executor
            .execute(&a_request)
            .await
            .expect("initial build must complete");
        let initial_snapshot = Arc::clone(initial.snapshot());
        state
            .commit_built_snapshot(&a_request, &initial)
            .expect("initial build must enter the cache");
        drop(initial);

        let (first, second) =
            tokio::join!(executor.execute(&a_request), executor.execute(&a_request));
        let first = first.expect("first replacement build waiter must complete");
        let second = second.expect("second replacement build waiter must complete");
        assert!(Arc::ptr_eq(first.snapshot(), second.snapshot()));
        assert!(!Arc::ptr_eq(&initial_snapshot, first.snapshot()));

        let cached = state
            .commit_built_snapshot(&a_request, &first)
            .expect("the new build must observe the resident snapshot");
        assert!(Arc::ptr_eq(&cached.snapshot, &initial_snapshot));
        drop(cached);
        drop(first);

        let b_request = state
            .snapshot_build_request_after_cache_miss(&b)
            .expect("available test document must produce a build request");
        let b_analysis = executor
            .execute(&b_request)
            .await
            .expect("replacement build must complete");
        state
            .commit_built_snapshot(&b_request, &b_analysis)
            .expect("replacement build must commit");
        assert!(!cache_contains(&state, &a));
        assert!(cache_contains(&state, &b));
        let before_weight = state.analysis_cache.total_weight();
        let before_statistics = state.analysis_cache.statistics();

        let request_local = state
            .commit_built_snapshot(&a_request, &second)
            .expect("the late waiter must remain a request-local result");

        assert!(Arc::ptr_eq(&request_local.snapshot, second.snapshot()));
        assert!(!cache_contains(&state, &a));
        assert!(cache_contains(&state, &b));
        assert_eq!(state.analysis_cache.total_weight(), before_weight);
        assert_eq!(state.analysis_cache.statistics(), before_statistics);
    }
}
