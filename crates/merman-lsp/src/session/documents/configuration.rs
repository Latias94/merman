use super::*;

impl SnapshotConfigurationPlan {
    pub(in crate::session::documents) fn prepare(
        self,
    ) -> Result<SnapshotConfigurationBatch, AnalysisCancelled> {
        self.cancellation.checkpoint()?;
        let analyzer = self
            .base_analyzer
            .with_snapshot_policy(self.next_options.snapshot_policy().clone())
            .with_diagnostic_policy(self.next_options.diagnostic_policy().clone());
        self.cancellation.checkpoint()?;

        let resource_limits = self.next_options.resource_limits();
        let resource_rejections = self
            .documents
            .map(|documents| {
                let mut resource_rejections = HashMap::new();
                for document in documents {
                    self.cancellation.checkpoint()?;
                    let rejection = resource_limits.preflight_document_cancellable(
                        document.text.as_ref(),
                        &document.source,
                        &self.cancellation,
                    )?;
                    resource_rejections.insert(document.uri, rejection);
                }
                Ok(resource_rejections)
            })
            .transpose()?;
        self.cancellation.checkpoint()?;
        Ok(SnapshotConfigurationBatch {
            request: self.request,
            expected_configuration_revision: self.expected_configuration_revision,
            expected_documents_revision: self.expected_documents_revision,
            next_options: self.next_options,
            analyzer,
            resource_rejections,
        })
    }
}

impl SessionState {
    pub(in crate::session::documents) fn begin_analyzer_configuration_request(
        &mut self,
    ) -> ConfigurationRequestId {
        self.latest_configuration_request = ConfigurationRequestId(
            self.latest_configuration_request
                .0
                .checked_add(1)
                .expect("configuration request id exhausted"),
        );
        self.latest_configuration_request
    }

    pub(in crate::session::documents) fn is_analyzer_configuration_request_current(
        &self,
        request: ConfigurationRequestId,
    ) -> bool {
        request == self.latest_configuration_request
    }

    pub(in crate::session::documents) fn prepare_analyzer_options(
        &mut self,
        request: ConfigurationRequestId,
        options: AnalysisOptions,
    ) -> Option<AnalyzerOptionsPreparation> {
        if !self.is_analyzer_configuration_request_current(request) {
            return None;
        }
        let change = analyzer_configuration_change(self.analyzer.options(), &options);
        if change.affects_snapshots() {
            Some(AnalyzerOptionsPreparation::RequiresSnapshotPreparation(
                Box::new(self.prepare_snapshot_configuration_for(request, options)),
            ))
        } else {
            if !matches!(change, AnalyzerConfigurationChange::Unchanged) {
                self.apply_diagnostic_policy(options.diagnostic_policy().clone());
            }
            Some(AnalyzerOptionsPreparation::Applied(change))
        }
    }

    pub(in crate::session::documents) fn prepare_snapshot_configuration_for(
        &self,
        request: ConfigurationRequestId,
        next_options: AnalysisOptions,
    ) -> SnapshotConfigurationPlan {
        let resource_limits_changed =
            self.analyzer.options().resource_limits() != next_options.resource_limits();
        let documents = resource_limits_changed.then(|| {
            self.documents
                .iter()
                .filter_map(|(uri, record)| {
                    record
                        .document
                        .retained_text()
                        .map(|text| ResourceDocumentSnapshot {
                            uri: uri.clone(),
                            source: document_source_descriptor(uri, record.document.kind),
                            text: Arc::clone(text),
                        })
                })
                .collect()
        });
        SnapshotConfigurationPlan {
            cancellation: self.session_cancellation.child(),
            request,
            expected_configuration_revision: self.configuration_revision,
            expected_documents_revision: resource_limits_changed.then_some(self.documents_revision),
            base_analyzer: self.analyzer.clone(),
            next_options,
            documents,
        }
    }

    pub(in crate::session::documents) fn commit_snapshot_configuration(
        &mut self,
        batch: SnapshotConfigurationBatch,
    ) -> Option<AnalyzerConfigurationChange> {
        if !self.is_analyzer_configuration_request_current(batch.request)
            || self.configuration_revision != batch.expected_configuration_revision
            || batch
                .expected_documents_revision
                .is_some_and(|revision| self.documents_revision != revision)
        {
            return None;
        }

        let change = analyzer_configuration_change(self.analyzer.options(), &batch.next_options);
        if !change.affects_snapshots() {
            return None;
        }

        self.replace_analyzer(batch.analyzer, batch.resource_rejections.as_ref());
        Some(change)
    }

    fn apply_diagnostic_policy(&mut self, policy: AnalysisDiagnosticPolicy) {
        self.diagnostic_reprojection_cancellation.cancel();
        self.diagnostic_reprojection_cancellation = self.session_cancellation.child();
        self.analysis_executor.invalidate_reprojections();
        self.analyzer = self.analyzer.with_diagnostic_policy(policy);
        self.advance_configuration_revision();
        self.advance_diagnostic_generation();
        self.analysis_cache.downgrade_complete_entries();
    }

    pub(in crate::session::documents) fn replace_analyzer(
        &mut self,
        analyzer: Analyzer,
        resource_rejections: Option<&HashMap<Uri, Option<AnalysisRejection>>>,
    ) {
        self.diagnostic_reprojection_cancellation.cancel();
        self.diagnostic_reprojection_cancellation = self.session_cancellation.child();
        self.analyzer = analyzer;
        if let Some(resource_rejections) = resource_rejections {
            self.reclassify_documents_for_current_limits(resource_rejections);
        }
        self.advance_configuration_revision();
        self.advance_snapshot_generation();
        self.advance_diagnostic_generation();
        self.analysis_cache.clear();
        for record in self.documents.values_mut() {
            record.semantic_tokens_state = None;
        }
        self.analysis_executor.invalidate_all();
    }

    fn advance_snapshot_generation(&mut self) {
        self.snapshot_generation = SnapshotGeneration(
            self.snapshot_generation
                .0
                .checked_add(1)
                .expect("snapshot generation exhausted"),
        );
    }

    fn advance_diagnostic_generation(&mut self) {
        self.diagnostic_generation = DiagnosticGeneration(
            self.diagnostic_generation
                .0
                .checked_add(1)
                .expect("diagnostic generation exhausted"),
        );
        for record in self.documents.values_mut() {
            record.diagnostic_state = None;
        }
    }

    fn advance_configuration_revision(&mut self) {
        self.configuration_revision = ConfigurationRevision(
            self.configuration_revision
                .0
                .checked_add(1)
                .expect("configuration revision exhausted"),
        );
    }

    fn reclassify_documents_for_current_limits(
        &mut self,
        resource_rejections: &HashMap<Uri, Option<AnalysisRejection>>,
    ) {
        let max_source_bytes = self.analyzer.options().max_source_bytes();
        for (uri, record) in &mut self.documents {
            let next_source = match &record.document.source {
                DocumentSource::ResourceLimited(limit) => {
                    let source_len = limit.source_len;
                    let previous_max_source_bytes = limit.max_source_bytes;
                    let span = limit.span;
                    match max_source_bytes {
                        Some(max_source_bytes) if source_len > max_source_bytes => {
                            DocumentSource::ResourceLimited(DocumentResourceLimit {
                                source_len,
                                max_source_bytes,
                                span,
                            })
                        }
                        _ => DocumentSource::Discarded(DocumentDiscardedSource {
                            source_len,
                            previous_max_source_bytes,
                            span,
                        }),
                    }
                }
                DocumentSource::Discarded(discarded) => match max_source_bytes {
                    Some(max_source_bytes) if discarded.source_len > max_source_bytes => {
                        DocumentSource::ResourceLimited(DocumentResourceLimit {
                            source_len: discarded.source_len,
                            max_source_bytes,
                            span: discarded.span,
                        })
                    }
                    _ => DocumentSource::Discarded(*discarded),
                },
                DocumentSource::Available(text) | DocumentSource::AnalysisRejected { text, .. } => {
                    match resource_rejections
                        .get(uri)
                        .expect("resource reclassification must cover every retained document")
                    {
                        None => DocumentSource::Available(Arc::clone(text)),
                        Some(rejection) => {
                            document_source_from_rejection(Arc::clone(text), rejection.clone())
                        }
                    }
                }
                DocumentSource::SyncError(_) => continue,
            };
            record.document.source = next_source;
        }
    }

    #[cfg(test)]
    pub(in crate::session::documents) fn analyzer_options(&self) -> &AnalysisOptions {
        self.analyzer.options()
    }

    #[cfg(test)]
    pub(in crate::session::documents) fn analyzer_environment_identity(
        &self,
    ) -> &merman_analysis::AnalysisEnvironmentIdentity {
        self.analyzer.environment_identity()
    }
}

impl LanguageSession {
    pub(crate) async fn update_configuration(
        &self,
        options: AnalysisOptions,
    ) -> ConfigurationUpdateOutcome {
        let request = {
            let mut state = self.inner.state.lock().await;
            let Some(request) = self.commit_state_if_active(&mut state, |state| {
                state.begin_analyzer_configuration_request()
            }) else {
                return ConfigurationUpdateOutcome::Cancelled;
            };
            request
        };
        loop {
            let preparation = {
                let mut state = self.inner.state.lock().await;
                let Some(preparation) = self.commit_state_if_active(&mut state, |state| {
                    state.prepare_analyzer_options(request, options.clone())
                }) else {
                    return ConfigurationUpdateOutcome::Cancelled;
                };
                preparation
            };
            let Some(preparation) = preparation else {
                return ConfigurationUpdateOutcome::Superseded;
            };
            match preparation {
                AnalyzerOptionsPreparation::Applied(change) => {
                    return ConfigurationUpdateOutcome::applied(change);
                }
                AnalyzerOptionsPreparation::RequiresSnapshotPreparation(plan) => {
                    let batch = match tokio::task::spawn_blocking(move || plan.prepare()).await {
                        Ok(Ok(batch)) => batch,
                        Ok(Err(_)) => {
                            return ConfigurationUpdateOutcome::Cancelled;
                        }
                        Err(error) => {
                            tracing::error!(%error, "snapshot configuration preparation worker failed");
                            return ConfigurationUpdateOutcome::Failed;
                        }
                    };
                    let mut state = self.inner.state.lock().await;
                    let Some((change, request_is_current)) =
                        self.commit_state_if_active(&mut state, |state| {
                            let change = state.commit_snapshot_configuration(batch);
                            let request_is_current =
                                state.is_analyzer_configuration_request_current(request);
                            (change, request_is_current)
                        })
                    else {
                        return ConfigurationUpdateOutcome::Cancelled;
                    };
                    if let Some(change) = change {
                        return ConfigurationUpdateOutcome::applied(change);
                    }
                    if !request_is_current {
                        return ConfigurationUpdateOutcome::Superseded;
                    }
                }
            }
        }
    }
}
