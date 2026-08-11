use super::*;

impl SessionState {
    #[cfg(test)]
    pub(in crate::session) fn analysis_cache_total_weight(&self) -> usize {
        self.analysis_cache.total_weight()
    }

    #[cfg(test)]
    pub(in crate::session) fn analysis_cache_len(&self) -> usize {
        self.analysis_cache.len()
    }

    #[cfg(test)]
    pub(in crate::session::documents) fn analysis_cache_statistics(
        &self,
    ) -> WeightedCacheStatistics {
        self.analysis_cache.statistics()
    }

    #[cfg(test)]
    pub(in crate::session::documents) fn estimated_analysis_cache_entry_weight(
        uri: &Uri,
        context: &DocumentAnalysisContext,
    ) -> usize {
        AnalysisCache::complete_entry_weight(uri, context)
    }

    #[cfg(test)]
    pub(in crate::session::documents) fn estimated_snapshot_cache_entry_weight(
        uri: &Uri,
        snapshot: &DocumentSnapshot,
    ) -> usize {
        AnalysisCache::snapshot_entry_weight(uri, snapshot)
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
        ticket: &DiagnosticProjectionTicket,
        projection: &DiagnosticReprojectionLease,
    ) -> Option<SnapshotContext> {
        if projection.key() != ticket.request.key_ref() {
            return None;
        }
        self.commit_projected_context(
            projection.key(),
            ticket.cache_authority,
            projection.projected(),
        )
    }

    #[cfg(test)]
    pub(in crate::session::documents) fn commit_diagnostic_reprojection_for_test(
        &mut self,
        ticket: &DiagnosticProjectionTicket,
        projected: &Arc<DocumentAnalysisContext>,
    ) -> Option<SnapshotContext> {
        self.commit_projected_context(ticket.request.key_ref(), ticket.cache_authority, projected)
    }

    fn commit_projected_context(
        &mut self,
        key: &DiagnosticReprojectionKey,
        cache_authority: Option<AnalysisCacheAuthority>,
        projected: &Arc<DocumentAnalysisContext>,
    ) -> Option<SnapshotContext> {
        let uri = key.uri();
        let record = self.documents.get(uri)?;
        if record.epoch != key.document_epoch()
            || self.snapshot_generation != key.snapshot_generation()
            || self.diagnostic_generation != key.target_diagnostic_generation()
            || !self
                .analysis_executor
                .is_generation_current(uri, key.analysis_job_generation())
        {
            return None;
        }
        if projected.analysis_result_identity() != key.analysis_result_identity() {
            return None;
        }
        if projected.diagnostic_generation() != key.target_diagnostic_generation() {
            return None;
        }

        let context = SnapshotContext::with_analysis(
            Arc::clone(&projected.snapshot),
            Arc::clone(&projected.payload),
            Arc::clone(projected.diagnostic_round_trip()),
            key.snapshot_generation(),
            key.target_diagnostic_generation(),
            key.document_epoch(),
        );
        if let Some(authority) = cache_authority {
            let _ = self.analysis_cache.promote(
                uri,
                authority,
                key.target_diagnostic_generation(),
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

    #[cfg(test)]
    pub(in crate::session) fn cached_snapshot_context_for_uri(
        &mut self,
        uri: &Uri,
    ) -> Option<SnapshotContext> {
        let stamp = self.analysis_cache_stamp(uri)?;
        let cached = self.analysis_cache.lookup(uri, stamp)?;
        let context = cached.context()?;
        Some(Self::cached_snapshot_context(context, stamp))
    }

    pub(in crate::session) fn snapshot_lease_for_uri(
        &mut self,
        uri: &Uri,
    ) -> Option<SnapshotLease> {
        let stamp = self.analysis_cache_stamp(uri)?;
        let cached = self.analysis_cache.lookup(uri, stamp)?;
        Some(SnapshotLease::new(
            Arc::clone(cached.snapshot()),
            cached.context().cloned(),
            stamp.snapshot_generation,
            stamp.document_epoch,
            stamp.analysis_job_generation,
            Some(cached.authority()),
        ))
    }

    #[cfg(test)]
    pub(in crate::session) fn cached_snapshot_for_probe(
        &self,
        uri: &Uri,
    ) -> Option<Arc<DocumentSnapshot>> {
        let stamp = self.analysis_cache_stamp(uri)?;
        let cached = self.analysis_cache.current_without_touch(uri, stamp)?;
        Some(Arc::clone(cached.snapshot()))
    }

    fn analysis_cache_stamp(&self, uri: &Uri) -> Option<AnalysisCacheStamp> {
        Some(AnalysisCacheStamp {
            document_epoch: self.documents.get(uri)?.epoch,
            snapshot_generation: self.snapshot_generation,
            analysis_job_generation: self.analysis_executor.generation_for(uri),
        })
    }

    pub(in crate::session) fn prepare_analysis_for_uri(
        &mut self,
        uri: &Uri,
    ) -> Option<AnalysisPreparation> {
        if self.documents.get(uri)?.document.is_analysis_unavailable() {
            return None;
        }
        let stamp = self.analysis_cache_stamp(uri)?;
        if let Some(cached) = self.analysis_cache.lookup(uri, stamp) {
            if let Some(context) = cached.context()
                && context.diagnostic_generation() == self.diagnostic_generation
            {
                return Some(AnalysisPreparation::Ready(Self::cached_snapshot_context(
                    context, stamp,
                )));
            }
            return Some(AnalysisPreparation::Project(
                self.diagnostic_reprojection_request_for_snapshot(
                    uri.clone(),
                    stamp.analysis_job_generation,
                    stamp.document_epoch,
                    stamp.snapshot_generation,
                    Arc::clone(cached.snapshot()),
                    Some(cached.authority()),
                ),
            ));
        }
        self.snapshot_build_request_without_cache_check(uri)
            .map(Box::new)
            .map(AnalysisPreparation::Build)
    }

    pub(in crate::session) fn snapshot_build_request(
        &self,
        uri: &Uri,
    ) -> Option<AnalysisBuildRequest> {
        if self.has_snapshot(uri) {
            return None;
        }
        self.snapshot_build_request_without_cache_check(uri)
    }

    fn snapshot_build_request_without_cache_check(
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
    ) -> Option<SnapshotLease> {
        self.commit_built_snapshot_inner(request, Arc::clone(analysis.snapshot()), || {
            analysis.claim_cache_admission()
        })
    }

    #[cfg(test)]
    pub(in crate::session::documents) fn commit_built_snapshot_direct_for_test(
        &mut self,
        request: &AnalysisBuildRequest,
        snapshot: Arc<DocumentSnapshot>,
    ) -> Option<SnapshotLease> {
        self.commit_built_snapshot_inner(request, snapshot, || true)
    }

    fn commit_built_snapshot_inner(
        &mut self,
        request: &AnalysisBuildRequest,
        snapshot: Arc<DocumentSnapshot>,
        claim_cache_admission: impl FnOnce() -> bool,
    ) -> Option<SnapshotLease> {
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
            return Some(SnapshotLease::new(
                Arc::clone(current.snapshot()),
                current.context().cloned(),
                stamp.snapshot_generation,
                stamp.document_epoch,
                stamp.analysis_job_generation,
                Some(current.authority()),
            ));
        }
        let cache_authority = cache_admission.then(|| {
            self.analysis_cache
                .insert_snapshot(request.uri().clone(), stamp, Arc::clone(&snapshot))
        });
        let cache_authority = cache_authority.flatten();
        Some(SnapshotLease::new(
            snapshot,
            None,
            request.snapshot_generation(),
            request.document_epoch(),
            request.analysis_job_generation(),
            cache_authority,
        ))
    }

    pub(in crate::session) fn prepare_diagnostic_projection_for_snapshot(
        &self,
        snapshot: &SnapshotLease,
    ) -> Option<DiagnosticProjectionPreparation> {
        if !self.is_snapshot_lease_current(snapshot)
            || !self
                .analysis_executor
                .is_generation_current(snapshot.snapshot.uri(), snapshot.analysis_job_generation)
        {
            return None;
        }
        if let Some(context) = &snapshot.context
            && context.diagnostic_generation() == self.diagnostic_generation
        {
            return Some(DiagnosticProjectionPreparation::Ready(
                Self::cached_snapshot_context(
                    context,
                    AnalysisCacheStamp {
                        document_epoch: snapshot.document_epoch,
                        snapshot_generation: snapshot.snapshot_generation,
                        analysis_job_generation: snapshot.analysis_job_generation,
                    },
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
                snapshot.cache_authority,
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
        self.analysis_cache_stamp(uri)
            .is_some_and(|stamp| self.analysis_cache.contains(uri, stamp))
    }

    #[cfg(test)]
    pub(in crate::session) fn has_analysis_payload(&self, uri: &Uri) -> bool {
        let Some(stamp) = self.analysis_cache_stamp(uri) else {
            return false;
        };
        self.analysis_cache
            .current_without_touch(uri, stamp)
            .is_some_and(|cached| {
                cached.context().is_some_and(|context| {
                    context.diagnostic_generation() == self.diagnostic_generation
                })
            })
    }

    #[cfg(test)]
    pub(in crate::session) fn diagnostic_reprojection_request(
        &self,
        uri: &Uri,
    ) -> Option<DiagnosticProjectionTicket> {
        let stamp = self.analysis_cache_stamp(uri)?;
        let cached = self.analysis_cache.current_without_touch(uri, stamp)?;
        if cached
            .context()
            .is_some_and(|context| context.diagnostic_generation() == self.diagnostic_generation)
        {
            return None;
        }
        Some(self.diagnostic_reprojection_request_for_snapshot(
            uri.clone(),
            stamp.analysis_job_generation,
            stamp.document_epoch,
            stamp.snapshot_generation,
            Arc::clone(cached.snapshot()),
            Some(cached.authority()),
        ))
    }

    pub(in crate::session) fn retry_diagnostic_reprojection_request(
        &self,
        ticket: &DiagnosticProjectionTicket,
    ) -> Option<DiagnosticProjectionPreparation> {
        let request = ticket.request();
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
        let stamp = AnalysisCacheStamp {
            document_epoch: record.epoch,
            snapshot_generation: self.snapshot_generation,
            analysis_job_generation: request.analysis_job_generation(),
        };
        if let Some(cached) = self.analysis_cache.current_without_touch(uri, stamp) {
            if let Some(context) = cached.context()
                && context.diagnostic_generation() == self.diagnostic_generation
            {
                return Some(DiagnosticProjectionPreparation::Ready(
                    Self::cached_snapshot_context(context, stamp),
                ));
            }
            return Some(DiagnosticProjectionPreparation::Project(
                self.diagnostic_reprojection_request_for_snapshot(
                    uri.clone(),
                    request.analysis_job_generation(),
                    record.epoch,
                    self.snapshot_generation,
                    Arc::clone(cached.snapshot()),
                    Some(cached.authority()),
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
                ticket.cache_authority,
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
        cache_authority: Option<AnalysisCacheAuthority>,
    ) -> DiagnosticProjectionTicket {
        DiagnosticProjectionTicket::new(
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
            ),
            cache_authority,
        )
    }

    #[cfg(test)]
    pub(in crate::session::documents) fn cached_analysis_generation(
        &self,
        uri: &Uri,
    ) -> Option<Arc<DocumentAnalysisContext>> {
        let stamp = self.analysis_cache_stamp(uri)?;
        self.analysis_cache
            .current_without_touch(uri, stamp)?
            .context()
            .cloned()
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
        context: &Arc<DocumentAnalysisContext>,
        stamp: AnalysisCacheStamp,
    ) -> SnapshotContext {
        SnapshotContext::with_analysis(
            Arc::clone(&context.snapshot),
            Arc::clone(&context.payload),
            Arc::clone(context.diagnostic_round_trip()),
            stamp.snapshot_generation,
            context.diagnostic_generation(),
            stamp.document_epoch,
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
                    && stored.state.result_id == previous_result_id)
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
