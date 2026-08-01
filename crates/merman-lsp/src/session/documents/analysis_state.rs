use super::*;

impl SessionState {
    #[cfg(test)]
    pub(in crate::session) fn analysis_cache_total_weight(&self) -> usize {
        self.analysis_generations.total_weight()
    }

    #[cfg(test)]
    pub(in crate::session) fn analysis_cache_len(&self) -> usize {
        self.analysis_generations.len()
    }

    #[cfg(test)]
    pub(in crate::session::documents) fn analysis_cache_statistics(
        &self,
    ) -> WeightedCacheStatistics {
        self.analysis_generations.statistics()
    }

    #[cfg(test)]
    pub(in crate::session::documents) fn estimated_analysis_cache_entry_weight(
        uri: &Uri,
        context: &DocumentAnalysisContext,
    ) -> usize {
        cached_analysis_weight(uri, context)
    }

    #[cfg(test)]
    pub(in crate::session) fn set_analysis_test_gate(
        &mut self,
        gate: Option<Arc<TestAnalysisGate>>,
    ) {
        self.analysis_test_gate = gate;
    }

    pub(in crate::session) fn commit_diagnostic_reprojection_context(
        &mut self,
        projection: &DiagnosticReprojectionLease,
    ) -> Option<SnapshotContext> {
        let uri = projection.uri();
        let record = self.documents.get(uri)?;
        if record.epoch != projection.document_epoch()
            || self.snapshot_generation != projection.snapshot_generation()
            || self.diagnostic_generation != projection.target_diagnostic_generation()
            || !self
                .analysis_executor
                .is_generation_current(uri, projection.analysis_job_generation())
        {
            return None;
        }
        if projection.projected().generation_identity() != projection.generation_identity() {
            return None;
        }

        if let Some(cached) = self.analysis_generations.peek(uri)
            && cached.document_epoch == projection.document_epoch()
            && cached.snapshot_generation == projection.snapshot_generation()
            && cached.context.diagnostic_generation() == projection.target_diagnostic_generation()
        {
            return Some(Self::cached_snapshot_context(cached));
        }

        let context = Self::reprojected_snapshot_context(projection);
        let weight = cached_analysis_weight(uri, projection.projected());
        let replacement = WeightedReplacement {
            key: uri.clone(),
            value: CachedAnalysisGeneration {
                context: Arc::clone(projection.projected()),
                document_epoch: projection.document_epoch(),
                snapshot_generation: projection.snapshot_generation(),
            },
            weight,
        };
        let cached_matches_generation = self.analysis_generations.peek(uri).is_some_and(|cached| {
            cached.document_epoch == projection.document_epoch()
                && cached.snapshot_generation == projection.snapshot_generation()
                && cached.context.generation_identity() == projection.generation_identity()
        });
        if cached_matches_generation {
            self.analysis_generations
                .replace_batch_preserving_recency(vec![replacement]);
            return Some(context);
        }
        if self.analysis_generations.peek(uri).is_some() {
            return None;
        }
        if matches!(projection.origin(), DiagnosticProjectionOrigin::FreshBuild) {
            self.analysis_generations.insert(
                replacement.key,
                replacement.value,
                replacement.weight,
            );
        }
        Some(context)
    }

    pub(in crate::session::documents) fn reprojected_snapshot_context(
        projection: &DiagnosticReprojectionLease,
    ) -> SnapshotContext {
        SnapshotContext::with_analysis(
            Arc::clone(&projection.projected().snapshot),
            Arc::clone(&projection.projected().payload),
            projection.snapshot_generation(),
            projection.target_diagnostic_generation(),
            projection.document_epoch(),
        )
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

    pub(in crate::session) fn cached_snapshot_context_for_uri(
        &mut self,
        uri: &Uri,
    ) -> Option<SnapshotContext> {
        let document_epoch = self.documents.get(uri)?.epoch;
        let snapshot_generation = self.snapshot_generation;
        if let Some(cached) = self.analysis_generations.get_if(uri, |cached| {
            cached.document_epoch == document_epoch
                && cached.snapshot_generation == snapshot_generation
        }) {
            return Some(Self::cached_snapshot_context(cached));
        }
        self.analysis_generations.remove(uri);
        None
    }

    pub(in crate::session) fn snapshot_lease_for_uri(
        &mut self,
        uri: &Uri,
    ) -> Option<SnapshotLease> {
        if let Some(context) = self.cached_snapshot_context_for_uri(uri) {
            return Some(SnapshotLease::new(
                context.snapshot,
                context.generation,
                context.document_epoch,
                self.analysis_executor.generation_for(uri),
            ));
        }

        let captured = self.documents.get(uri)?.snapshot.clone()?;
        let current = captured.snapshot_generation == self.snapshot_generation
            && self.is_document_epoch_current(uri, captured.document_epoch)
            && self
                .analysis_executor
                .is_generation_current(uri, captured.analysis_job_generation);
        let snapshot = current.then(|| captured.snapshot.upgrade()).flatten();
        if let Some(snapshot) = snapshot {
            return Some(SnapshotLease::new(
                snapshot,
                captured.snapshot_generation,
                captured.document_epoch,
                captured.analysis_job_generation,
            ));
        }
        if let Some(record) = self.documents.get_mut(uri) {
            record.snapshot = None;
        }
        None
    }

    #[cfg(test)]
    pub(in crate::session) fn cached_snapshot_for_probe(
        &self,
        uri: &Uri,
    ) -> Option<Arc<DocumentSnapshot>> {
        let document_epoch = self.documents.get(uri)?.epoch;
        let cached = self.analysis_generations.peek(uri)?;
        (cached.document_epoch == document_epoch
            && cached.snapshot_generation == self.snapshot_generation)
            .then(|| Arc::clone(&cached.context.snapshot))
    }

    pub(in crate::session) fn snapshot_build_request(
        &self,
        uri: &Uri,
    ) -> Option<AnalysisBuildRequest> {
        let record = self.documents.get(uri)?;
        if record.document.is_analysis_unavailable() || self.has_snapshot(uri) {
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
        snapshot: Arc<DocumentSnapshot>,
    ) -> Option<SnapshotLease> {
        if self.snapshot_generation != request.snapshot_generation()
            || !self.is_document_epoch_current(request.uri(), request.document_epoch())
            || !self
                .analysis_executor
                .is_generation_current(request.uri(), request.analysis_job_generation())
        {
            return None;
        }
        if let Some(current) = self.snapshot_lease_for_uri(request.uri()) {
            return Some(current);
        }

        let record = self.documents.get_mut(request.uri())?;
        record.snapshot = Some(WeakDocumentSnapshot {
            snapshot: Arc::downgrade(&snapshot),
            snapshot_generation: request.snapshot_generation(),
            document_epoch: request.document_epoch(),
            analysis_job_generation: request.analysis_job_generation(),
        });
        Some(SnapshotLease::new(
            snapshot,
            request.snapshot_generation(),
            request.document_epoch(),
            request.analysis_job_generation(),
        ))
    }

    pub(in crate::session) fn prepare_diagnostic_projection_for_snapshot(
        &self,
        snapshot: &SnapshotLease,
        origin: DiagnosticProjectionOrigin,
    ) -> Option<DiagnosticProjectionPreparation> {
        if !self.is_snapshot_lease_current(snapshot)
            || !self
                .analysis_executor
                .is_generation_current(snapshot.snapshot.uri(), snapshot.analysis_job_generation)
        {
            return None;
        }
        if let Some(cached) = self.analysis_generations.peek(snapshot.snapshot.uri())
            && cached.document_epoch == snapshot.document_epoch
            && cached.snapshot_generation == snapshot.snapshot_generation
        {
            if cached.context.diagnostic_generation() == self.diagnostic_generation {
                return Some(DiagnosticProjectionPreparation::Ready(
                    Self::cached_snapshot_context(cached),
                ));
            }
            return Some(DiagnosticProjectionPreparation::Project(
                self.diagnostic_reprojection_request_for_snapshot(
                    snapshot.snapshot.uri().clone(),
                    snapshot.analysis_job_generation,
                    snapshot.document_epoch,
                    snapshot.snapshot_generation,
                    Arc::clone(&cached.context.snapshot),
                    DiagnosticProjectionOrigin::Cached,
                ),
            ));
        }
        Some(DiagnosticProjectionPreparation::Project(
            self.diagnostic_reprojection_request_for_snapshot(
                snapshot.snapshot.uri().clone(),
                snapshot.analysis_job_generation,
                snapshot.document_epoch,
                snapshot.snapshot_generation,
                Arc::clone(&snapshot.snapshot),
                origin,
            ),
        ))
    }

    pub(in crate::session) fn is_snapshot_lease_current(&self, snapshot: &SnapshotLease) -> bool {
        self.snapshot_generation == snapshot.snapshot_generation
            && self.is_document_epoch_current(snapshot.snapshot.uri(), snapshot.document_epoch)
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

    pub(in crate::session) fn has_snapshot(&self, uri: &Uri) -> bool {
        let Some(record) = self.documents.get(uri) else {
            return false;
        };
        self.analysis_generations.peek(uri).is_some_and(|cached| {
            cached.document_epoch == record.epoch
                && cached.snapshot_generation == self.snapshot_generation
        })
    }

    pub(in crate::session) fn has_analysis_payload(&self, uri: &Uri) -> bool {
        self.analysis_generations.peek(uri).is_some_and(|cached| {
            cached.context.diagnostic_generation() == self.diagnostic_generation
                && cached.snapshot_generation == self.snapshot_generation
                && self.is_document_epoch_current(uri, cached.document_epoch)
        })
    }

    pub(in crate::session) fn diagnostic_reprojection_request(
        &self,
        uri: &Uri,
    ) -> Option<DiagnosticReprojectionRequest> {
        let cached = self.analysis_generations.peek(uri)?;
        if cached.context.diagnostic_generation() == self.diagnostic_generation {
            return None;
        }
        let record = self.documents.get(uri)?;
        if cached.document_epoch != record.epoch
            || cached.snapshot_generation != self.snapshot_generation
        {
            return None;
        }
        Some(self.diagnostic_reprojection_request_for_snapshot(
            uri.clone(),
            self.analysis_executor.generation_for(uri),
            record.epoch,
            cached.snapshot_generation,
            Arc::clone(&cached.context.snapshot),
            DiagnosticProjectionOrigin::Cached,
        ))
    }

    pub(in crate::session) fn retry_diagnostic_reprojection_request(
        &self,
        request: &DiagnosticReprojectionRequest,
    ) -> Option<DiagnosticProjectionPreparation> {
        let uri = request.uri();
        let record = self.documents.get(uri)?;
        if record.epoch != request.document_epoch()
            || self.snapshot_generation != request.snapshot_generation()
            || !self
                .analysis_executor
                .is_generation_current(uri, request.analysis_job_generation())
        {
            return None;
        }
        if let Some(cached) = self.analysis_generations.peek(uri)
            && cached.document_epoch == record.epoch
            && cached.snapshot_generation == self.snapshot_generation
        {
            if cached.context.diagnostic_generation() == self.diagnostic_generation {
                return Some(DiagnosticProjectionPreparation::Ready(
                    Self::cached_snapshot_context(cached),
                ));
            }
            return Some(DiagnosticProjectionPreparation::Project(
                self.diagnostic_reprojection_request_for_snapshot(
                    uri.clone(),
                    request.analysis_job_generation(),
                    record.epoch,
                    self.snapshot_generation,
                    Arc::clone(&cached.context.snapshot),
                    DiagnosticProjectionOrigin::Cached,
                ),
            ));
        }
        Some(DiagnosticProjectionPreparation::Project(
            self.diagnostic_reprojection_request_for_snapshot(
                uri.clone(),
                request.analysis_job_generation(),
                record.epoch,
                self.snapshot_generation,
                Arc::clone(request.snapshot()),
                request.origin(),
            ),
        ))
    }

    fn diagnostic_reprojection_request_for_snapshot(
        &self,
        uri: Uri,
        analysis_job_generation: AnalysisJobGeneration,
        document_epoch: DocumentEpoch,
        snapshot_generation: SnapshotGeneration,
        snapshot: Arc<DocumentSnapshot>,
        origin: DiagnosticProjectionOrigin,
    ) -> DiagnosticReprojectionRequest {
        DiagnosticReprojectionRequest::new(
            self.analyzer.options().diagnostic_policy().clone(),
            self.diagnostic_reprojection_cancellation.clone(),
            DiagnosticReprojectionKey::new(
                uri,
                analysis_job_generation,
                document_epoch,
                snapshot_generation,
                self.diagnostic_generation,
                snapshot.as_ref(),
            ),
            snapshot,
            origin,
        )
    }

    #[cfg(test)]
    pub(in crate::session::documents) fn cached_analysis_generation(
        &self,
        uri: &Uri,
    ) -> Option<&Arc<DocumentAnalysisContext>> {
        self.analysis_generations
            .peek(uri)
            .map(|cached| &cached.context)
    }

    pub(in crate::session) fn analysis_executor(&self) -> AnalysisExecutor {
        self.analysis_executor.clone()
    }

    pub(in crate::session::documents) fn diagnostic_contexts(&self) -> Vec<DiagnosticContext> {
        self.documents
            .values()
            .map(|record| {
                DiagnosticContext::new(
                    record.document.clone(),
                    self.diagnostic_generation,
                    record.epoch,
                )
            })
            .collect()
    }

    pub(in crate::session::documents) fn cached_snapshot_context(
        cached: &CachedAnalysisGeneration,
    ) -> SnapshotContext {
        SnapshotContext::with_analysis(
            Arc::clone(&cached.context.snapshot),
            Arc::clone(&cached.context.payload),
            cached.snapshot_generation,
            cached.context.diagnostic_generation(),
            cached.document_epoch,
        )
    }

    #[cfg(test)]
    pub(in crate::session) fn semantic_tokens_state(
        &self,
        uri: &Uri,
    ) -> Option<&SemanticTokensState> {
        self.documents
            .get(uri)
            .and_then(|record| record.semantic_tokens_state.as_ref())
            .map(|stored| stored.state.as_ref())
    }

    pub(in crate::session) fn semantic_tokens_state_for_delta(
        &self,
        uri: &Uri,
        previous_result_id: &str,
    ) -> Option<Arc<SemanticTokensState>> {
        self.documents
            .get(uri)
            .and_then(|record| record.semantic_tokens_state.as_ref())
            .and_then(|stored| {
                (stored.snapshot_generation == self.snapshot_generation
                    && stored.state.result_id.as_deref() == Some(previous_result_id))
                .then(|| Arc::clone(&stored.state))
            })
    }

    #[cfg(test)]
    pub(in crate::session) fn set_semantic_tokens_state_if_current(
        &mut self,
        context: &SnapshotContext,
        state: SemanticTokensState,
    ) -> bool {
        if !self.is_snapshot_context_current(context) {
            return false;
        }

        let Some(record) = self.documents.get_mut(context.snapshot.uri()) else {
            return false;
        };
        record.semantic_tokens_state = Some(StoredSemanticTokensState {
            snapshot_generation: context.generation,
            state: Arc::new(state),
        });
        true
    }

    pub(in crate::session) fn set_semantic_tokens_state_if_snapshot_current(
        &mut self,
        snapshot: &SnapshotLease,
        state: SemanticTokensState,
    ) -> bool {
        if !self.is_snapshot_lease_current(snapshot) {
            return false;
        }

        let Some(record) = self.documents.get_mut(snapshot.snapshot.uri()) else {
            return false;
        };
        record.semantic_tokens_state = Some(StoredSemanticTokensState {
            snapshot_generation: snapshot.snapshot_generation,
            state: Arc::new(state),
        });
        true
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

    pub(crate) async fn diagnostic_contexts(&self) -> Vec<DiagnosticContext> {
        self.inner.state.lock().await.diagnostic_contexts()
    }
}
