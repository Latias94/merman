use merman_core::preprocess::SourceConfigEvidence;

pub(crate) fn capture_source_config_evidence(source: &str) -> SourceConfigEvidence {
    let control = merman_core::OperationControl::new();
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

pub(crate) fn capture_source_metadata(source: &str) -> Option<merman_core::SemanticMetadata> {
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
