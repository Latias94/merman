use merman_core::preprocess::SourceConfigEvidence;

pub(crate) fn capture_source_config_evidence(source: &str) -> SourceConfigEvidence {
    let control = merman_core::ParseControl::new();
    match merman_core::Engine::new()
        .capture_diagram_snapshot_controlled_sync(source, &control)
        .expect("a private parse control cannot be cancelled")
    {
        merman_core::DiagramSnapshotCapture::Snapshot(Some(snapshot)) => {
            snapshot.source_config().clone()
        }
        merman_core::DiagramSnapshotCapture::Snapshot(None) => SourceConfigEvidence::default(),
        merman_core::DiagramSnapshotCapture::Failed { source_config, .. }
        | merman_core::DiagramSnapshotCapture::Panicked { source_config, .. } => source_config,
    }
}
