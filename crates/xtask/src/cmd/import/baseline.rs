use crate::XtaskError;
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub(crate) struct ImportedFixtureWorkspaceLock {
    _lock: crate::cmd::UpstreamSvgFamilyLock,
}

#[derive(Debug)]
pub(crate) struct ImportedFixtureTransactionLocks<'workspace> {
    _workspace_lock: &'workspace ImportedFixtureWorkspaceLock,
    // Struct fields drop top-to-bottom, so declare owned guards in release order.
    family_lock: crate::cmd::UpstreamSvgFamilyLock,
    toolchain_lock: crate::cmd::UpstreamSvgToolchainLock,
}

#[derive(Debug)]
pub(crate) struct ImportedFixtureFamilyLocks<'workspace> {
    _workspace_lock: &'workspace ImportedFixtureWorkspaceLock,
    _family_locks: Vec<crate::cmd::UpstreamSvgFamilyLock>,
}

impl ImportedFixtureTransactionLocks<'_> {
    pub(crate) fn toolchain_lock(&self) -> &crate::cmd::UpstreamSvgToolchainLock {
        &self.toolchain_lock
    }

    pub(crate) fn family_lock(&self) -> &crate::cmd::UpstreamSvgFamilyLock {
        &self.family_lock
    }
}

pub(crate) fn acquire_imported_fixture_workspace_lock()
-> Result<ImportedFixtureWorkspaceLock, XtaskError> {
    let fixtures_root = crate::cmd::fixtures_root();
    acquire_imported_fixture_workspace_lock_in(&fixtures_root)
}

fn acquire_imported_fixture_workspace_lock_in(
    fixtures_root: &Path,
) -> Result<ImportedFixtureWorkspaceLock, XtaskError> {
    crate::cmd::acquire_upstream_svg_family_lock(fixtures_root)
        .map(|lock| ImportedFixtureWorkspaceLock { _lock: lock })
}

pub(crate) fn acquire_imported_fixture_family_locks<'workspace, S>(
    workspace_lock: &'workspace ImportedFixtureWorkspaceLock,
    diagram_dirs: impl IntoIterator<Item = S>,
) -> Result<ImportedFixtureFamilyLocks<'workspace>, XtaskError>
where
    S: AsRef<str>,
{
    acquire_imported_fixture_family_locks_in(
        workspace_lock,
        &crate::cmd::fixtures_root(),
        diagram_dirs,
    )
}

fn acquire_imported_fixture_family_locks_in<'workspace, S>(
    workspace_lock: &'workspace ImportedFixtureWorkspaceLock,
    fixtures_root: &Path,
    diagram_dirs: impl IntoIterator<Item = S>,
) -> Result<ImportedFixtureFamilyLocks<'workspace>, XtaskError>
where
    S: AsRef<str>,
{
    let family_dirs = diagram_dirs
        .into_iter()
        .map(|diagram_dir| {
            fixtures_root
                .join("upstream-svgs")
                .join(diagram_dir.as_ref())
        })
        .collect::<BTreeSet<_>>();
    for family_dir in &family_dirs {
        fs::create_dir_all(family_dir).map_err(|source| XtaskError::WriteFile {
            path: family_dir.display().to_string(),
            source,
        })?;
    }
    let family_dirs = family_dirs.into_iter().collect::<Vec<_>>();
    let family_locks = crate::cmd::acquire_upstream_svg_family_locks(&family_dirs)?;
    Ok(ImportedFixtureFamilyLocks {
        _workspace_lock: workspace_lock,
        _family_locks: family_locks,
    })
}

pub(crate) fn acquire_imported_fixture_transaction_locks<'workspace>(
    workspace_lock: &'workspace ImportedFixtureWorkspaceLock,
    diagram_dir: &str,
) -> Result<ImportedFixtureTransactionLocks<'workspace>, XtaskError> {
    let fixtures_root = crate::cmd::fixtures_root();
    let toolchain_lock =
        crate::cmd::acquire_upstream_svg_toolchain_lock(&crate::cmd::mermaid_cli_root())?;
    let upstream_family = fixtures_root.join("upstream-svgs").join(diagram_dir);
    fs::create_dir_all(&upstream_family).map_err(|source| XtaskError::WriteFile {
        path: upstream_family.display().to_string(),
        source,
    })?;
    let family_lock = crate::cmd::acquire_upstream_svg_family_lock(&upstream_family)?;
    Ok(ImportedFixtureTransactionLocks {
        _workspace_lock: workspace_lock,
        family_lock,
        toolchain_lock,
    })
}

fn load_existing_imported_fixtures_from_dirs(
    directories: impl IntoIterator<Item = PathBuf>,
    canonical_fixture_text: impl Fn(&str) -> String,
) -> HashMap<String, PathBuf> {
    let mut existing = HashMap::new();
    for directory in directories {
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|extension| extension == "mmd")
                && let Ok(text) = fs::read_to_string(&path)
            {
                existing
                    .entry(canonical_fixture_text(&text))
                    .or_insert(path);
            }
        }
    }
    existing
}

pub(crate) fn load_existing_imported_fixtures(
    _workspace_lock: &ImportedFixtureWorkspaceLock,
    fixtures_dir: &Path,
    diagram_dir: &str,
    canonical_fixture_text: impl Fn(&str) -> String,
) -> HashMap<String, PathBuf> {
    load_existing_imported_fixtures_from_dirs(
        [
            fixtures_dir.to_path_buf(),
            crate::cmd::fixtures_root()
                .join("_deferred")
                .join(diagram_dir),
        ],
        canonical_fixture_text,
    )
}

pub(crate) fn validate_exact_import_candidate_filter(
    diagram_dir: &str,
    stem: &str,
    fixture_path: &Path,
) -> Result<(), XtaskError> {
    let fixtures_dir = crate::cmd::fixtures_root().join(diagram_dir);
    let matches = crate::cmd::list_mmd_fixtures_in_dir(&fixtures_dir, Some(stem), false);
    if matches.len() == 1 && matches[0] == fixture_path {
        return Ok(());
    }
    Err(XtaskError::UpstreamSvgFailed(format!(
        "baseline import filter {stem:?} must select exactly {}, matched: {}",
        fixture_path.display(),
        matches
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    )))
}

// The generator currently erases stage information into UpstreamSvgFailed. Keep this whitelist
// narrow so family preconditions and infrastructure failures cannot become deferred fixtures.
fn is_candidate_upstream_svg_failure(message: &str, fixture_path: &Path) -> bool {
    if message.contains('\n') || message.contains("; failed to clean temporary upstream SVG ") {
        return false;
    }

    let fixture_path = fixture_path.display();
    let exit_prefix = format!("mmdc failed for {fixture_path} (exit=");
    if let Some(exit_code) = message
        .strip_prefix(&exit_prefix)
        .and_then(|rest| rest.strip_suffix(')'))
    {
        return exit_code
            .parse::<i32>()
            .is_ok_and(|exit_code| exit_code == 1);
    }

    let validation_prefix = format!("mmdc output validation failed for {fixture_path}:");
    let Some(validation) = message.strip_prefix(&validation_prefix) else {
        return false;
    };
    validation.contains("upstream renderer produced an empty temporary SVG")
        || validation.contains("upstream renderer output is not an SVG document")
}

pub(crate) fn candidate_upstream_svg_failure(
    error: XtaskError,
    fixture_path: &Path,
) -> Result<String, XtaskError> {
    match error {
        XtaskError::UpstreamSvgFailed(message)
            if is_candidate_upstream_svg_failure(&message, fixture_path) =>
        {
            Ok(message)
        }
        error => Err(error),
    }
}

pub(crate) fn candidate_snapshot_failure(
    error: XtaskError,
    fixture_path: &Path,
) -> Result<String, XtaskError> {
    let fixture_path = fixture_path.display();
    match error {
        XtaskError::SnapshotUpdateFailed(message)
            if !message.contains('\n')
                && (message == format!("no diagram detected in {fixture_path}")
                    || message.starts_with(&format!("parse failed for {fixture_path}:"))) =>
        {
            Ok(message)
        }
        XtaskError::LayoutSnapshotUpdateFailed(message)
            if !message.contains('\n')
                && (message == format!("no diagram detected in {fixture_path}")
                    || message.starts_with(&format!("parse failed for {fixture_path}:"))
                    || message.starts_with(&format!("layout failed for {fixture_path}:"))) =>
        {
            Ok(message)
        }
        error => Err(error),
    }
}

pub(crate) fn candidate_svg_compare_failure(
    error: XtaskError,
    fixture_path: &Path,
    stem: &str,
) -> Result<String, XtaskError> {
    let XtaskError::SvgCompareFailed(message) = error else {
        return Err(error);
    };

    const INFRASTRUCTURE_MARKERS: &[&str] = &[
        "provenance",
        "manifest",
        "family lock",
        "timed out acquiring",
        "missing upstream svg",
        "failed to read ",
        "failed to write ",
        "failed to create ",
        "no .mmd fixtures matched",
    ];
    if INFRASTRUCTURE_MARKERS
        .iter()
        .any(|marker| message.contains(marker))
    {
        return Err(XtaskError::SvgCompareFailed(message));
    }

    let fixture_path = fixture_path.display();
    let candidate_markers = [
        format!("dom mismatch for {stem}:"),
        format!("svg mismatch for {stem}"),
        format!("marker mismatch for {stem}:"),
        format!("parse failed for {fixture_path}:"),
        format!("layout failed for {fixture_path}:"),
        format!("render failed for {fixture_path}:"),
        format!("root parse failed for {stem}:"),
        format!("root parse failed for local {stem}:"),
        format!("label metric parse failed for {stem}:"),
    ];
    if candidate_markers
        .iter()
        .any(|marker| message.contains(marker))
    {
        Ok(message)
    } else {
        Err(XtaskError::SvgCompareFailed(message))
    }
}

fn restore_snapshot_errors(errors: Vec<String>) -> Result<(), XtaskError> {
    if errors.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::UpstreamSvgFailed(format!(
            "failed to restore imported fixture transaction: {}",
            errors.join("; ")
        )))
    }
}

pub(crate) fn restore_imported_fixture_snapshot(
    snapshot: &super::ImportedFixtureSnapshot,
) -> Result<(), XtaskError> {
    restore_snapshot_errors(snapshot.rollback())
}

pub(crate) fn restore_imported_fixture_snapshot_preserving_deferred(
    snapshot: &super::ImportedFixtureSnapshot,
) -> Result<(), XtaskError> {
    let errors = snapshot.rollback_preserving_deferred();
    if errors.is_empty() {
        return Ok(());
    }

    let mut errors = errors;
    errors.extend(snapshot.rollback());
    restore_snapshot_errors(errors)
}

// Batch importers pass snapshots newest-first so repeated paths and config updates unwind safely.
pub(crate) fn rollback_imported_fixture_snapshots<'a>(
    error: XtaskError,
    snapshots: impl IntoIterator<Item = &'a super::ImportedFixtureSnapshot>,
) -> XtaskError {
    let rollback_errors = snapshots
        .into_iter()
        .flat_map(|snapshot| snapshot.rollback())
        .collect::<Vec<_>>();
    if rollback_errors.is_empty() {
        return error;
    }

    let rollback_message = format!(
        "failed to roll back imported fixture files: {}",
        rollback_errors.join("; ")
    );
    match error {
        XtaskError::UpstreamSvgFailed(message) => {
            XtaskError::UpstreamSvgFailed(format!("{message}; {rollback_message}"))
        }
        error => XtaskError::UpstreamSvgFailed(format!("{error}; {rollback_message}")),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        acquire_imported_fixture_family_locks_in, acquire_imported_fixture_workspace_lock_in,
        candidate_snapshot_failure, candidate_svg_compare_failure, candidate_upstream_svg_failure,
        is_candidate_upstream_svg_failure, load_existing_imported_fixtures_from_dirs,
        rollback_imported_fixture_snapshots,
    };
    use crate::XtaskError;
    use crate::cmd::import::ImportedFixtureSnapshot;
    use std::fs;
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::mpsc;
    use std::time::Duration;

    static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn candidate_path() -> &'static Path {
        Path::new("fixtures/flowchart/candidate.mmd")
    }

    #[test]
    fn initial_dedup_cache_includes_active_and_deferred_fixture_bodies() {
        let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "merman-import-dedup-cache-{}-{sequence}",
            std::process::id()
        ));
        let active_dir = root.join("flowchart");
        let deferred_dir = root.join("_deferred").join("flowchart");
        fs::create_dir_all(&active_dir).expect("create active fixture directory");
        fs::create_dir_all(&deferred_dir).expect("create deferred fixture directory");

        let active_path = active_dir.join("active.mmd");
        let deferred_duplicate_path = deferred_dir.join("duplicate.mmd");
        let deferred_only_path = deferred_dir.join("deferred.mmd");
        fs::write(&active_path, "flowchart TD\r\n  A-->B\r\n").expect("write active fixture");
        fs::write(&deferred_duplicate_path, "flowchart TD\n  A-->B\n")
            .expect("write deferred duplicate fixture");
        fs::write(&deferred_only_path, "sequenceDiagram\n  A->>B: Hi\n")
            .expect("write deferred-only fixture");

        let existing =
            load_existing_imported_fixtures_from_dirs([active_dir, deferred_dir], |text| {
                format!("{}\n", text.replace("\r\n", "\n").trim_matches('\n'))
            });

        assert_eq!(
            existing.get("flowchart TD\n  A-->B\n"),
            Some(&active_path),
            "the active fixture should remain the stable duplicate target"
        );
        assert_eq!(
            existing.get("sequenceDiagram\n  A->>B: Hi\n"),
            Some(&deferred_only_path)
        );

        fs::remove_dir_all(root).expect("remove dedup cache test root");
    }

    #[test]
    fn non_baseline_batch_serializes_fixture_writes_with_family_generation() {
        let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "merman-import-family-interleaving-{}-{sequence}",
            std::process::id()
        ));
        let sequence_fixtures = root.join("sequence");
        let sequence_upstream = root.join("upstream-svgs").join("sequence");
        fs::create_dir_all(&sequence_fixtures).expect("create sequence fixtures");
        fs::create_dir_all(root.join("upstream-svgs").join("flowchart"))
            .expect("create flowchart upstream family");
        fs::create_dir_all(&sequence_upstream).expect("create sequence upstream family");
        fs::write(sequence_fixtures.join("existing.mmd"), "sequenceDiagram\n")
            .expect("write existing fixture");

        let generator_lock = crate::cmd::acquire_upstream_svg_family_lock(&sequence_upstream)
            .expect("hold generation family lock");
        let (workspace_ready_tx, workspace_ready_rx) = mpsc::channel();
        let (import_committed_tx, import_committed_rx) = mpsc::channel();
        std::thread::scope(|scope| {
            let writer_root = &root;
            scope.spawn(move || {
                let workspace_lock = acquire_imported_fixture_workspace_lock_in(writer_root)
                    .expect("acquire test workspace lock");
                workspace_ready_tx
                    .send(())
                    .expect("signal workspace acquisition");
                let family_locks = acquire_imported_fixture_family_locks_in(
                    &workspace_lock,
                    writer_root,
                    ["sequence", "flowchart"],
                )
                .expect("acquire non-baseline family locks");
                assert_eq!(family_locks._family_locks.len(), 2);

                fs::write(
                    writer_root.join("sequence").join("imported.mmd"),
                    "sequenceDiagram\n  A->>B: imported\n",
                )
                .expect("write imported fixture");
                let config_dir = writer_root.join("_config");
                fs::create_dir_all(&config_dir).expect("create config directory");
                fs::write(
                    config_dir.join("site_config_overrides.json"),
                    "{\"sequence/imported.mmd\":{}}\n",
                )
                .expect("commit imported site config");
                import_committed_tx
                    .send(())
                    .expect("signal imported fixture commit");
            });

            workspace_ready_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("writer acquires the disjoint workspace lock");
            assert!(
                import_committed_rx
                    .recv_timeout(Duration::from_millis(100))
                    .is_err(),
                "the family generator must block non-baseline fixture mutation"
            );
            fs::write(
                sequence_upstream.join("_baseline-manifest.json"),
                "generation-before-import\n",
            )
            .expect("commit simulated generation manifest");
            drop(generator_lock);
            import_committed_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("writer commits after generation releases the family");
        });

        let _next_generation_lock =
            crate::cmd::acquire_upstream_svg_family_lock(&sequence_upstream)
                .expect("acquire the next generation family lock");
        let mut fixture_names = fs::read_dir(&sequence_fixtures)
            .expect("enumerate committed fixture generation")
            .map(|entry| {
                entry
                    .expect("read committed fixture")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect::<Vec<_>>();
        fixture_names.sort();
        fs::write(
            sequence_upstream.join("_baseline-manifest.json"),
            fixture_names.join("\n"),
        )
        .expect("write simulated next-generation manifest");
        assert_eq!(fixture_names, ["existing.mmd", "imported.mmd"]);

        fs::remove_dir_all(root).expect("remove family interleaving test root");
    }

    #[test]
    fn classifies_only_current_candidate_renderer_failures_as_deferrable() {
        let path = candidate_path();

        assert!(is_candidate_upstream_svg_failure(
            "mmdc failed for fixtures/flowchart/candidate.mmd (exit=1)",
            path,
        ));
        assert!(is_candidate_upstream_svg_failure(
            "mmdc output validation failed for fixtures/flowchart/candidate.mmd: upstream renderer produced an empty temporary SVG",
            path,
        ));
        assert!(!is_candidate_upstream_svg_failure(
            "mmdc failed for fixtures/flowchart/other.mmd (exit=1)",
            path,
        ));
    }

    #[test]
    fn rejects_family_and_cleanup_failures_as_deferrable() {
        let path = candidate_path();

        assert!(!is_candidate_upstream_svg_failure(
            "partial upstream SVG provenance update requires an existing manifest",
            path,
        ));
        assert!(!is_candidate_upstream_svg_failure(
            "upstream SVG render environment probe failed (exit=1)",
            path,
        ));
        assert!(!is_candidate_upstream_svg_failure(
            "mmdc failed for fixtures/flowchart/candidate.mmd: process timed out",
            path,
        ));
        assert!(!is_candidate_upstream_svg_failure(
            "mmdc failed for fixtures/flowchart/candidate.mmd (exit=1); failed to clean temporary upstream SVG staging.svg: access denied",
            path,
        ));
        assert!(!is_candidate_upstream_svg_failure(
            "mmdc failed for fixtures/flowchart/candidate.mmd (exit=-1)",
            path,
        ));
        assert!(!is_candidate_upstream_svg_failure(
            "mmdc failed for fixtures/flowchart/candidate.mmd (exit=2)",
            path,
        ));
        assert!(!is_candidate_upstream_svg_failure(
            "mmdc failed for fixtures/flowchart/candidate.mmd (exit=137)",
            path,
        ));
        assert!(!is_candidate_upstream_svg_failure(
            "mmdc output validation failed for fixtures/flowchart/candidate.mmd: upstream renderer did not produce temporary SVG staging.svg: access denied",
            path,
        ));
    }

    #[test]
    fn snapshot_classification_rejects_io_and_other_fixture_failures() {
        let path = candidate_path();
        assert!(
            candidate_snapshot_failure(
                XtaskError::SnapshotUpdateFailed(format!(
                    "parse failed for {}: bad syntax",
                    path.display()
                )),
                path,
            )
            .is_ok()
        );
        assert!(
            candidate_snapshot_failure(
                XtaskError::SnapshotUpdateFailed(format!(
                    "failed to write {}: access denied",
                    path.with_extension("golden.json").display()
                )),
                path,
            )
            .is_err()
        );
        assert!(
            candidate_snapshot_failure(
                XtaskError::LayoutSnapshotUpdateFailed(
                    "layout failed for fixtures/flowchart/other.mmd: unsupported".to_string(),
                ),
                path,
            )
            .is_err()
        );
    }

    #[test]
    fn compare_classification_rejects_provenance_and_lock_failures() {
        let path = candidate_path();
        assert!(candidate_svg_compare_failure(
            XtaskError::SvgCompareFailed(
                "flowchart: svg compare failed:\ndom mismatch for candidate: upstream=x local=y"
                    .to_string(),
            ),
            path,
            "candidate",
        )
        .is_ok());
        assert!(
            candidate_svg_compare_failure(
                XtaskError::SvgCompareFailed(
                    "flowchart: upstream SVG provenance manifest drifted".to_string(),
                ),
                path,
                "candidate",
            )
            .is_err()
        );
        assert!(
            candidate_svg_compare_failure(
                XtaskError::UpstreamSvgFailed(
                    "timed out acquiring upstream SVG family lock".to_string(),
                ),
                path,
                "candidate",
            )
            .is_err()
        );
    }

    #[test]
    fn fatal_generation_error_restores_the_current_imported_fixture() {
        let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let fixture_path = std::env::temp_dir().join(format!(
            "merman-import-baseline-rollback-{}-{sequence}.mmd",
            std::process::id()
        ));
        fs::write(&fixture_path, "flowchart TD\n  original-->fixture\n")
            .expect("write original temporary fixture");
        let snapshot = ImportedFixtureSnapshot::capture(
            "test-import-baseline-rollback",
            "candidate",
            &fixture_path,
        )
        .expect("capture imported fixture state");
        fs::write(&fixture_path, "flowchart TD\n  replacement-->fixture\n")
            .expect("write replacement temporary fixture");

        let error = XtaskError::UpstreamSvgFailed(
            "partial upstream SVG provenance update requires an existing manifest".to_string(),
        );
        let error = candidate_upstream_svg_failure(error, &fixture_path)
            .expect_err("provenance failure must be fatal");
        let error = rollback_imported_fixture_snapshots(error, [&snapshot]);

        assert!(matches!(error, XtaskError::UpstreamSvgFailed(_)));
        assert_eq!(
            fs::read_to_string(&fixture_path).expect("read restored fixture"),
            "flowchart TD\n  original-->fixture\n"
        );
        fs::remove_file(&fixture_path).expect("remove temporary fixture");
    }

    #[test]
    fn fatal_generation_error_removes_a_new_imported_fixture() {
        let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let fixture_path = std::env::temp_dir().join(format!(
            "merman-import-baseline-cleanup-{}-{sequence}.mmd",
            std::process::id()
        ));
        let _ = fs::remove_file(&fixture_path);
        let snapshot = ImportedFixtureSnapshot::capture(
            "test-import-baseline-cleanup",
            "candidate",
            &fixture_path,
        )
        .expect("capture absent imported fixture state");
        fs::write(&fixture_path, "flowchart TD\n  new-->fixture\n")
            .expect("write new temporary fixture");

        let error = XtaskError::UpstreamSvgFailed(
            "partial upstream SVG provenance update requires an existing manifest".to_string(),
        );
        let error = candidate_upstream_svg_failure(error, &fixture_path)
            .expect_err("provenance failure must be fatal");
        let error = rollback_imported_fixture_snapshots(error, [&snapshot]);

        assert!(matches!(error, XtaskError::UpstreamSvgFailed(_)));
        assert!(!fixture_path.exists());
    }
}
