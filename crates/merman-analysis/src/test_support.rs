use merman_core::preprocess::SourceConfigEvidence;

pub(crate) fn capture_source_config_evidence(source: &str) -> SourceConfigEvidence {
    let control = merman_model::OperationControl::new();
    match merman_core::SemanticParser::new()
        .capture_diagram_snapshot_controlled_sync(source, &control)
        .expect("a private parse control cannot be cancelled")
    {
        merman_core::SemanticSnapshotCapture::Snapshot(snapshot) => {
            snapshot.source_config().clone()
        }
        merman_core::SemanticSnapshotCapture::Failed { source_config, .. }
        | merman_core::SemanticSnapshotCapture::Panicked { source_config, .. } => source_config,
    }
}

pub(crate) fn capture_source_metadata(source: &str) -> Option<merman_model::SemanticMetadata> {
    match merman_core::SemanticParser::new().capture_diagram_snapshot_sync(source) {
        merman_core::SemanticSnapshotCapture::Snapshot(snapshot)
            if !snapshot.recovered_incomplete_directive() =>
        {
            Some(snapshot.into_parts().0)
        }
        merman_core::SemanticSnapshotCapture::Snapshot(_)
        | merman_core::SemanticSnapshotCapture::Failed { .. }
        | merman_core::SemanticSnapshotCapture::Panicked { .. } => None,
    }
}

pub(crate) fn diagnostic_policy(
    rule_config: crate::AnalysisRuleConfig,
) -> crate::AnalysisDiagnosticPolicy {
    let policy = crate::AnalysisDiagnosticPolicy {
        rule_config,
        ..Default::default()
    };
    #[cfg(feature = "fixes")]
    let policy = policy.with_fix_generation(crate::FixGeneration::Generate);
    policy
}

#[cfg(feature = "fixes")]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct FixWork {
    pub(crate) materializations: usize,
    pub(crate) serializations: usize,
}

#[cfg(feature = "fixes")]
thread_local! {
    static FIX_WORK: std::cell::Cell<FixWork> = const { std::cell::Cell::new(FixWork { materializations: 0, serializations: 0 }) };
}

#[cfg(feature = "fixes")]
pub(crate) fn reset_fix_work() {
    FIX_WORK.set(FixWork::default());
}

#[cfg(feature = "fixes")]
pub(crate) fn fix_work() -> FixWork {
    FIX_WORK.get()
}

#[cfg(feature = "fixes")]
pub(crate) fn record_fix_materialization() {
    FIX_WORK.set(FixWork {
        materializations: FIX_WORK.get().materializations + 1,
        ..FIX_WORK.get()
    });
}

#[cfg(feature = "fixes")]
pub(crate) fn record_fix_serialization() {
    FIX_WORK.set(FixWork {
        serializations: FIX_WORK.get().serializations + 1,
        ..FIX_WORK.get()
    });
}
