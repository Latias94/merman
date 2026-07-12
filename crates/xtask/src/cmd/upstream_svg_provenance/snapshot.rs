use super::{fixture_stem, hash_bytes, hash_file, upstream_svg_fixture_exclusion_reason};
use crate::XtaskError;
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static SNAPSHOT_RUN_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CapturedUpstreamSvgFixture {
    stem: String,
    live_path: PathBuf,
    snapshot_path: PathBuf,
    input_sha256: String,
}

impl CapturedUpstreamSvgFixture {
    pub(crate) fn stem(&self) -> &str {
        &self.stem
    }

    pub(crate) fn live_path(&self) -> &Path {
        &self.live_path
    }

    pub(crate) fn snapshot_path(&self) -> &Path {
        &self.snapshot_path
    }

    pub(crate) fn input_sha256(&self) -> &str {
        &self.input_sha256
    }

    fn validate_path_hash(&self, path: &Path, kind: &str) -> Result<(), XtaskError> {
        let actual = hash_file(path)?;
        if actual == self.input_sha256 {
            return Ok(());
        }
        Err(XtaskError::UpstreamSvgFailed(format!(
            "upstream SVG {kind} changed after snapshot capture for {}: {}; rerun generation",
            self.stem,
            path.display()
        )))
    }

    pub(crate) fn validate_captured_hashes(&self) -> Result<(), XtaskError> {
        self.validate_path_hash(&self.snapshot_path, "fixture snapshot")?;
        self.validate_path_hash(&self.live_path, "fixture")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CapturedUpstreamSvgExclusion {
    fixture: CapturedUpstreamSvgFixture,
    reason: String,
}

impl CapturedUpstreamSvgExclusion {
    pub(crate) fn fixture(&self) -> &CapturedUpstreamSvgFixture {
        &self.fixture
    }

    pub(crate) fn reason(&self) -> &str {
        &self.reason
    }
}

#[derive(Debug)]
pub(crate) struct UpstreamSvgFixtureSnapshots {
    run_root: Option<PathBuf>,
    staging_family_root: PathBuf,
    diagram: String,
    fixtures_dir: PathBuf,
    filter: Option<String>,
    renderable: Vec<CapturedUpstreamSvgFixture>,
    excluded: Vec<CapturedUpstreamSvgExclusion>,
}

impl UpstreamSvgFixtureSnapshots {
    pub(crate) fn renderable(&self) -> &[CapturedUpstreamSvgFixture] {
        &self.renderable
    }

    pub(crate) fn excluded(&self) -> &[CapturedUpstreamSvgExclusion] {
        &self.excluded
    }

    pub(crate) fn validate_live_selection_and_hashes(&self) -> Result<(), XtaskError> {
        let selected =
            crate::cmd::list_mmd_fixtures_in_dir(&self.fixtures_dir, self.filter.as_deref(), false);
        let mut current = BTreeMap::new();
        for live_path in selected {
            let stem = fixture_stem(&live_path)?.to_string();
            let reason = upstream_svg_fixture_exclusion_reason(&self.diagram, &live_path)?;
            if current.insert(stem.clone(), (live_path, reason)).is_some() {
                return Err(XtaskError::UpstreamSvgFailed(format!(
                    "duplicate fixture stem while validating upstream SVG snapshots for {}: {stem}",
                    self.diagram
                )));
            }
        }

        let mut captured = BTreeMap::new();
        for fixture in &self.renderable {
            captured.insert(
                fixture.stem.clone(),
                (fixture.live_path.clone(), None::<String>),
            );
        }
        for exclusion in &self.excluded {
            captured.insert(
                exclusion.fixture.stem.clone(),
                (
                    exclusion.fixture.live_path.clone(),
                    Some(exclusion.reason.clone()),
                ),
            );
        }
        if current != captured {
            return Err(XtaskError::UpstreamSvgFailed(format!(
                "upstream SVG fixture selection changed after snapshot capture for {}; rerun generation",
                self.diagram
            )));
        }

        for fixture in &self.renderable {
            fixture.validate_captured_hashes()?;
        }
        for exclusion in &self.excluded {
            exclusion.fixture.validate_captured_hashes()?;
        }
        Ok(())
    }

    pub(crate) fn cleanup(&mut self) -> Result<(), String> {
        let Some(run_root) = self.run_root.as_ref() else {
            return Ok(());
        };
        let canonical_staging = fs::canonicalize(&self.staging_family_root).map_err(|err| {
            format!(
                "failed to validate upstream SVG snapshot staging root {}: {err}",
                self.staging_family_root.display()
            )
        })?;
        let canonical_run = fs::canonicalize(run_root).map_err(|err| {
            format!(
                "failed to validate upstream SVG snapshot run root {}: {err}",
                run_root.display()
            )
        })?;
        if canonical_run == canonical_staging || !canonical_run.starts_with(&canonical_staging) {
            return Err(format!(
                "refusing to remove unsafe upstream SVG snapshot path {} outside {}",
                canonical_run.display(),
                canonical_staging.display()
            ));
        }
        fs::remove_dir_all(&canonical_run).map_err(|err| {
            format!(
                "failed to remove upstream SVG snapshot workspace {}: {err}",
                canonical_run.display()
            )
        })?;
        self.run_root = None;
        Ok(())
    }
}

impl Drop for UpstreamSvgFixtureSnapshots {
    fn drop(&mut self) {
        if let Err(err) = self.cleanup() {
            eprintln!("warning: {err}");
        }
    }
}

fn create_snapshot_run_root(staging_family_root: &Path) -> Result<PathBuf, XtaskError> {
    fs::create_dir_all(staging_family_root).map_err(|source| XtaskError::WriteFile {
        path: staging_family_root.display().to_string(),
        source,
    })?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    for _ in 0..128 {
        let sequence = SNAPSHOT_RUN_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let run_root =
            staging_family_root.join(format!("run-{}-{timestamp}-{sequence}", std::process::id()));
        match fs::create_dir(&run_root) {
            Ok(()) => return Ok(run_root),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(source) => {
                return Err(XtaskError::WriteFile {
                    path: run_root.display().to_string(),
                    source,
                });
            }
        }
    }
    Err(XtaskError::UpstreamSvgFailed(format!(
        "failed to allocate an upstream SVG snapshot run under {}",
        staging_family_root.display()
    )))
}

fn capture_fixture(
    live_path: PathBuf,
    inputs_dir: &Path,
) -> Result<CapturedUpstreamSvgFixture, XtaskError> {
    let stem = fixture_stem(&live_path)?.to_string();
    let bytes = fs::read(&live_path).map_err(|source| XtaskError::ReadFile {
        path: live_path.display().to_string(),
        source,
    })?;
    let file_name = live_path.file_name().ok_or_else(|| {
        XtaskError::UpstreamSvgFailed(format!(
            "invalid upstream SVG fixture filename {}",
            live_path.display()
        ))
    })?;
    let snapshot_path = inputs_dir.join(file_name);
    let mut snapshot = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&snapshot_path)
        .map_err(|source| XtaskError::WriteFile {
            path: snapshot_path.display().to_string(),
            source,
        })?;
    snapshot
        .write_all(&bytes)
        .map_err(|source| XtaskError::WriteFile {
            path: snapshot_path.display().to_string(),
            source,
        })?;
    drop(snapshot);

    Ok(CapturedUpstreamSvgFixture {
        stem,
        live_path,
        snapshot_path,
        input_sha256: hash_bytes(&bytes),
    })
}

pub(crate) fn capture_upstream_svg_fixture_selection(
    staging_parent: &Path,
    diagram: &str,
    fixtures_dir: &Path,
    filter: Option<&str>,
) -> Result<UpstreamSvgFixtureSnapshots, XtaskError> {
    let staging_family_root = staging_parent.join(diagram);
    let run_root = create_snapshot_run_root(&staging_family_root)?;
    let inputs_dir = run_root.join("inputs");
    if let Err(source) = fs::create_dir(&inputs_dir) {
        let _ = fs::remove_dir(&run_root);
        return Err(XtaskError::WriteFile {
            path: inputs_dir.display().to_string(),
            source,
        });
    }

    let mut snapshots = UpstreamSvgFixtureSnapshots {
        run_root: Some(run_root),
        staging_family_root,
        diagram: diagram.to_string(),
        fixtures_dir: fixtures_dir.to_path_buf(),
        filter: filter.map(str::to_string),
        renderable: Vec::new(),
        excluded: Vec::new(),
    };
    let mut selected = crate::cmd::list_mmd_fixtures_in_dir(fixtures_dir, filter, false);
    selected.sort();
    for live_path in selected {
        let reason = upstream_svg_fixture_exclusion_reason(diagram, &live_path)?;
        let fixture = capture_fixture(live_path, &inputs_dir)?;
        if let Some(reason) = reason {
            snapshots
                .excluded
                .push(CapturedUpstreamSvgExclusion { fixture, reason });
        } else {
            snapshots.renderable.push(fixture);
        }
    }
    Ok(snapshots)
}
