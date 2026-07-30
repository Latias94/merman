use crate::snapshot::{
    DiagnosticGeneration, DocumentAnalysisContext, DocumentEpoch, SnapshotGeneration,
};
#[cfg(test)]
use crate::sync::{lock_recovering_poison, recover_poison};
use merman_analysis::{AnalysisCancellationToken, AnalysisCancelled, AnalysisRejection, Analyzer};
use merman_editor_core::{DocumentAnalysisOutcome, DocumentKind, DocumentWorkspace};
use std::sync::Arc;
use tower_lsp_server::ls_types::Uri;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub(in crate::session) struct AnalysisJobGeneration(pub(in crate::session) u64);

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
    diagnostic_generation: DiagnosticGeneration,
    document_epoch: DocumentEpoch,
}

#[derive(Debug, Clone)]
pub(in crate::session) enum AnalysisBuildError {
    Cancelled(AnalysisCancelled),
    Rejected(AnalysisRejection),
}

impl AnalysisBuildKey {
    pub(in crate::session) fn new(
        uri: Uri,
        version: i32,
        analysis_job_generation: AnalysisJobGeneration,
        snapshot_generation: SnapshotGeneration,
        diagnostic_generation: DiagnosticGeneration,
        document_epoch: DocumentEpoch,
    ) -> Self {
        Self {
            uri,
            version,
            analysis_job_generation,
            snapshot_generation,
            diagnostic_generation,
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

    pub(in crate::session) fn diagnostic_generation(&self) -> DiagnosticGeneration {
        self.key.diagnostic_generation
    }

    pub(in crate::session) fn document_epoch(&self) -> DocumentEpoch {
        self.key.document_epoch
    }

    pub(in crate::session) fn build_cancellable(
        &self,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<Arc<DocumentAnalysisContext>, AnalysisBuildError> {
        cancellation
            .checkpoint()
            .map_err(AnalysisBuildError::Cancelled)?;
        #[cfg(test)]
        if let Some(gate) = &self.test_gate {
            gate.wait(cancellation)
                .map_err(AnalysisBuildError::Cancelled)?;
        }
        let context = DocumentWorkspace::build_analysis_context_with_shared_text_cancellable(
            &self.analyzer,
            self.key.uri.as_str(),
            self.key.version,
            Arc::clone(&self.text),
            self.kind,
            cancellation,
        )
        .map_err(AnalysisBuildError::Cancelled)?;
        cancellation
            .checkpoint()
            .map_err(AnalysisBuildError::Cancelled)?;
        document_analysis_context(context, self.key.uri.clone())
            .map_err(AnalysisBuildError::Rejected)
    }

    #[cfg(test)]
    pub(in crate::session) fn with_test_gate(mut self, gate: Arc<TestAnalysisGate>) -> Self {
        self.test_gate = Some(gate);
        self
    }
}

fn document_analysis_context(
    outcome: DocumentAnalysisOutcome,
    uri: Uri,
) -> Result<Arc<DocumentAnalysisContext>, AnalysisRejection> {
    let context = outcome.into_ready()?;
    Ok(Arc::new(DocumentAnalysisContext::from_editor(context, uri)))
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
    fn wait(&self, cancellation: &AnalysisCancellationToken) -> Result<(), AnalysisCancelled> {
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
