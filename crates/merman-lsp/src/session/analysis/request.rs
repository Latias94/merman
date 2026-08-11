use super::AnalysisJobGeneration;
use crate::snapshot::{
    AnalysisResultIdentity, DiagnosticGeneration, DocumentAnalysisContext, DocumentEpoch,
    DocumentSnapshot, SnapshotGeneration,
};
#[cfg(test)]
use crate::sync::{lock_recovering_poison, recover_poison};
use merman_analysis::{
    AnalysisCancellationToken, AnalysisCancelled, AnalysisCaptureOutcome, AnalysisDiagnosticPolicy,
    AnalysisRejection, Analyzer, analyze_document_generation_shared_cancellable,
    source_descriptor_for_kind,
};
use merman_editor_core::DocumentKind;
use std::sync::Arc;
use tower_lsp_server::ls_types::Uri;

#[derive(Debug, Clone)]
pub(in crate::session) struct AnalysisBuildRequest {
    key: AnalysisBuildKey,
    text: Arc<str>,
    kind: DocumentKind,
    analyzer: Analyzer,
    #[cfg(test)]
    test_gate: Option<Arc<TestAnalysisGate>>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub(in crate::session) struct AnalysisBuildKey {
    uri: Uri,
    version: i32,
    analysis_job_generation: AnalysisJobGeneration,
    snapshot_generation: SnapshotGeneration,
    document_epoch: DocumentEpoch,
}

#[derive(Debug, Clone)]
pub(in crate::session) enum AnalysisBuildError {
    Cancelled(AnalysisCancelled),
    Rejected(AnalysisRejection),
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub(in crate::session) struct DiagnosticReprojectionKey {
    uri: Uri,
    analysis_job_generation: AnalysisJobGeneration,
    document_epoch: DocumentEpoch,
    snapshot_generation: SnapshotGeneration,
    target_diagnostic_generation: DiagnosticGeneration,
    analysis_result_identity: AnalysisResultIdentity,
}

#[derive(Debug, Clone)]
pub(in crate::session) struct DiagnosticReprojectionRequest {
    policy: AnalysisDiagnosticPolicy,
    cancellation: AnalysisCancellationToken,
    key: DiagnosticReprojectionKey,
    snapshot: Arc<DocumentSnapshot>,
    #[cfg(test)]
    test_gate: Option<Arc<TestAnalysisGate>>,
}

#[derive(Debug, Clone)]
pub(in crate::session) struct DiagnosticReprojectionResult {
    key: DiagnosticReprojectionKey,
    projected: Arc<DocumentAnalysisContext>,
}

impl AnalysisBuildKey {
    pub(in crate::session) fn new(
        uri: Uri,
        version: i32,
        analysis_job_generation: AnalysisJobGeneration,
        snapshot_generation: SnapshotGeneration,
        document_epoch: DocumentEpoch,
    ) -> Self {
        Self {
            uri,
            version,
            analysis_job_generation,
            snapshot_generation,
            document_epoch,
        }
    }

    pub(in crate::session) fn uri(&self) -> &Uri {
        &self.uri
    }

    pub(in crate::session) fn analysis_job_generation(&self) -> AnalysisJobGeneration {
        self.analysis_job_generation
    }
}

impl AnalysisBuildRequest {
    pub(in crate::session) fn new(
        key: AnalysisBuildKey,
        text: Arc<str>,
        kind: DocumentKind,
        analyzer: Analyzer,
    ) -> Self {
        Self {
            key,
            text,
            kind,
            analyzer,
            #[cfg(test)]
            test_gate: None,
        }
    }

    pub(in crate::session) fn uri(&self) -> &Uri {
        self.key.uri()
    }

    pub(in crate::session) fn key(&self) -> AnalysisBuildKey {
        self.key.clone()
    }

    pub(in crate::session) fn analysis_job_generation(&self) -> AnalysisJobGeneration {
        self.key.analysis_job_generation
    }

    pub(in crate::session) fn snapshot_generation(&self) -> SnapshotGeneration {
        self.key.snapshot_generation
    }

    pub(in crate::session) fn document_epoch(&self) -> DocumentEpoch {
        self.key.document_epoch
    }

    pub(in crate::session) fn build_cancellable(
        &self,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<Arc<DocumentSnapshot>, AnalysisBuildError> {
        cancellation
            .checkpoint()
            .map_err(AnalysisBuildError::Cancelled)?;
        #[cfg(test)]
        if let Some(gate) = &self.test_gate {
            gate.wait(cancellation)
                .map_err(AnalysisBuildError::Cancelled)?;
        }
        let source =
            source_descriptor_for_kind(Some(self.key.uri.as_str()), self.kind.source_kind());
        let capture = analyze_document_generation_shared_cancellable(
            Arc::clone(&self.text),
            &self.analyzer,
            source,
            cancellation,
        )
        .map_err(AnalysisBuildError::Cancelled)?;
        cancellation
            .checkpoint()
            .map_err(AnalysisBuildError::Cancelled)?;
        match capture {
            AnalysisCaptureOutcome::Ready(generation) => {
                let editor = merman_editor_core::DocumentSnapshot::try_from_analysis_generation(
                    self.key.version,
                    Arc::new(generation),
                )
                .expect("LSP analysis generation must preserve the requested document URI");
                Ok(Arc::new(DocumentSnapshot::try_from_editor(editor).expect(
                    "LSP analysis source must preserve its validated URI",
                )))
            }
            AnalysisCaptureOutcome::Rejected(rejection) => {
                Err(AnalysisBuildError::Rejected(rejection))
            }
        }
    }

    #[cfg(test)]
    pub(in crate::session) fn with_test_gate(mut self, gate: Arc<TestAnalysisGate>) -> Self {
        self.test_gate = Some(gate);
        self
    }
}

impl DiagnosticReprojectionKey {
    pub(in crate::session) fn new(
        uri: Uri,
        analysis_job_generation: AnalysisJobGeneration,
        document_epoch: DocumentEpoch,
        snapshot_generation: SnapshotGeneration,
        target_diagnostic_generation: DiagnosticGeneration,
        snapshot: &DocumentSnapshot,
    ) -> Self {
        Self {
            uri,
            analysis_job_generation,
            document_epoch,
            snapshot_generation,
            target_diagnostic_generation,
            analysis_result_identity: snapshot.analysis_result_identity(),
        }
    }

    pub(in crate::session) fn uri(&self) -> &Uri {
        &self.uri
    }

    pub(in crate::session) fn analysis_job_generation(&self) -> AnalysisJobGeneration {
        self.analysis_job_generation
    }

    pub(in crate::session) fn document_epoch(&self) -> DocumentEpoch {
        self.document_epoch
    }

    pub(in crate::session) fn snapshot_generation(&self) -> SnapshotGeneration {
        self.snapshot_generation
    }

    pub(in crate::session) fn target_diagnostic_generation(&self) -> DiagnosticGeneration {
        self.target_diagnostic_generation
    }

    pub(in crate::session) fn analysis_result_identity(&self) -> AnalysisResultIdentity {
        self.analysis_result_identity
    }
}

impl DiagnosticReprojectionRequest {
    pub(in crate::session) fn new(
        policy: AnalysisDiagnosticPolicy,
        cancellation: AnalysisCancellationToken,
        key: DiagnosticReprojectionKey,
        snapshot: Arc<DocumentSnapshot>,
    ) -> Self {
        Self {
            policy,
            cancellation,
            key,
            snapshot,
            #[cfg(test)]
            test_gate: None,
        }
    }

    pub(in crate::session) fn key(&self) -> DiagnosticReprojectionKey {
        self.key.clone()
    }

    pub(in crate::session) fn key_ref(&self) -> &DiagnosticReprojectionKey {
        &self.key
    }

    pub(in crate::session) fn cancellation_child(&self) -> AnalysisCancellationToken {
        self.cancellation.child()
    }

    pub(in crate::session) fn uri(&self) -> &Uri {
        self.key.uri()
    }

    pub(in crate::session) fn analysis_job_generation(&self) -> AnalysisJobGeneration {
        self.key.analysis_job_generation()
    }

    pub(in crate::session) fn document_epoch(&self) -> DocumentEpoch {
        self.key.document_epoch()
    }

    pub(in crate::session) fn snapshot_generation(&self) -> SnapshotGeneration {
        self.key.snapshot_generation()
    }

    pub(in crate::session) fn snapshot(&self) -> &Arc<DocumentSnapshot> {
        &self.snapshot
    }

    #[cfg(test)]
    pub(in crate::session) fn with_test_gate(mut self, gate: Arc<TestAnalysisGate>) -> Self {
        self.test_gate = Some(gate);
        self
    }

    pub(in crate::session) fn project_with_cancellation(
        self,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<DiagnosticReprojectionResult, AnalysisCancelled> {
        cancellation.checkpoint()?;
        #[cfg(test)]
        if let Some(gate) = &self.test_gate {
            gate.wait(cancellation)?;
        }
        let projected = Arc::new(DocumentAnalysisContext::project_cancellable(
            Arc::clone(&self.snapshot),
            &self.policy,
            self.key.document_epoch(),
            self.key.target_diagnostic_generation(),
            cancellation,
        )?);
        cancellation.checkpoint()?;
        Ok(DiagnosticReprojectionResult {
            key: self.key,
            projected,
        })
    }
}

impl DiagnosticReprojectionResult {
    pub(in crate::session) fn key(&self) -> &DiagnosticReprojectionKey {
        &self.key
    }

    pub(in crate::session) fn projected(&self) -> &Arc<DocumentAnalysisContext> {
        &self.projected
    }
}

#[cfg(test)]
#[derive(Debug, Default)]
pub(in crate::session) struct TestAnalysisGate {
    released: std::sync::Mutex<bool>,
    wake: std::sync::Condvar,
    started: std::sync::atomic::AtomicUsize,
}

#[cfg(test)]
impl TestAnalysisGate {
    pub(in crate::session) fn wait(
        &self,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<(), AnalysisCancelled> {
        use std::sync::atomic::Ordering;
        use std::time::Duration;

        self.started.fetch_add(1, Ordering::Release);
        let mut released = lock_recovering_poison(&self.released);
        while !*released {
            cancellation.checkpoint()?;
            let (next, _) =
                recover_poison(self.wake.wait_timeout(released, Duration::from_millis(5)));
            released = next;
        }
        cancellation.checkpoint()
    }

    pub(in crate::session) fn started(&self) -> usize {
        self.started.load(std::sync::atomic::Ordering::Acquire)
    }

    pub(in crate::session) fn release(&self) {
        *lock_recovering_poison(&self.released) = true;
        self.wake.notify_all();
    }
}
