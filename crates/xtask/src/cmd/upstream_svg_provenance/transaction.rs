use super::{MANIFEST_FILE_NAME, MANIFEST_SCHEMA_VERSION, UpstreamSvgManifest};
use crate::XtaskError;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static MANIFEST_WRITE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
struct PreparedManifestWrite {
    manifest_path: PathBuf,
    temp_path: PathBuf,
    backup_path: PathBuf,
}

#[derive(Debug)]
struct StagedManifestWrite {
    manifest_path: PathBuf,
    backup_path: Option<PathBuf>,
}

fn manifest_batch_error(primary: String, cleanup_errors: Vec<String>) -> XtaskError {
    if cleanup_errors.is_empty() {
        return XtaskError::UpstreamSvgFailed(primary);
    }
    XtaskError::UpstreamSvgFailed(format!(
        "{primary}; manifest rollback/cleanup errors: {}",
        cleanup_errors.join("; ")
    ))
}

fn cleanup_prepared_manifest_temps(prepared: &[PreparedManifestWrite]) -> Vec<String> {
    let mut errors = Vec::new();
    for entry in prepared {
        if entry.temp_path.exists()
            && let Err(err) = fs::remove_file(&entry.temp_path)
        {
            errors.push(format!(
                "failed to remove staged manifest {}: {err}",
                entry.temp_path.display()
            ));
        }
    }
    errors
}

fn rollback_manifest_batch(staged: &[StagedManifestWrite], installed: &[PathBuf]) -> Vec<String> {
    let mut errors = Vec::new();
    for manifest_path in installed.iter().rev() {
        if manifest_path.exists()
            && let Err(err) = fs::remove_file(manifest_path)
        {
            errors.push(format!(
                "failed to remove installed manifest {} during rollback: {err}",
                manifest_path.display()
            ));
        }
    }
    for entry in staged.iter().rev() {
        let Some(backup_path) = &entry.backup_path else {
            continue;
        };
        if let Err(err) = fs::rename(backup_path, &entry.manifest_path) {
            errors.push(format!(
                "failed to restore manifest {} from {}: {err}",
                entry.manifest_path.display(),
                backup_path.display()
            ));
        }
    }
    errors
}

fn write_manifest_batch_with_installer_and_validator<I, V>(
    manifests: &[(&Path, &UpstreamSvgManifest)],
    mut install: I,
    validate_after_install: V,
) -> Result<(), XtaskError>
where
    I: FnMut(&Path, &Path) -> std::io::Result<()>,
    V: FnOnce() -> Result<(), XtaskError>,
{
    let mut encoded_manifests = Vec::with_capacity(manifests.len());
    let mut manifest_paths = BTreeSet::new();
    for (out_dir, manifest) in manifests {
        let manifest_path = out_dir.join(MANIFEST_FILE_NAME);
        if !manifest_paths.insert(manifest_path.clone()) {
            return Err(XtaskError::UpstreamSvgFailed(format!(
                "duplicate upstream SVG provenance transaction target {}",
                manifest_path.display()
            )));
        }
        if manifest.schema_version != MANIFEST_SCHEMA_VERSION {
            return Err(XtaskError::UpstreamSvgFailed(format!(
                "refusing to write upstream SVG provenance schema {} to {}; expected schema {MANIFEST_SCHEMA_VERSION}",
                manifest.schema_version,
                manifest_path.display()
            )));
        }
        manifest.attestation.validate()?;
        if manifest_path.exists() && !manifest_path.is_file() {
            return Err(XtaskError::UpstreamSvgFailed(format!(
                "upstream SVG provenance target is not a file: {}",
                manifest_path.display()
            )));
        }
        let mut encoded = serde_json::to_string_pretty(manifest).map_err(|err| {
            XtaskError::UpstreamSvgFailed(format!(
                "failed to encode upstream SVG provenance {}: {err}",
                manifest_path.display()
            ))
        })?;
        encoded.push('\n');
        encoded_manifests.push(((*out_dir).to_path_buf(), manifest_path, encoded));
    }

    let mut prepared = Vec::with_capacity(encoded_manifests.len());
    for (out_dir, manifest_path, encoded) in encoded_manifests {
        if let Err(source) = fs::create_dir_all(&out_dir) {
            return Err(manifest_batch_error(
                format!(
                    "failed to create manifest directory {}: {source}",
                    out_dir.display()
                ),
                cleanup_prepared_manifest_temps(&prepared),
            ));
        }
        let sequence = MANIFEST_WRITE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let file_name = manifest_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(MANIFEST_FILE_NAME);
        let temp_path = manifest_path.with_file_name(format!(
            ".{file_name}.{}.{sequence}.tmp",
            std::process::id()
        ));
        let backup_path = manifest_path.with_file_name(format!(
            ".{file_name}.{}.{sequence}.backup",
            std::process::id()
        ));
        prepared.push(PreparedManifestWrite {
            manifest_path,
            temp_path,
            backup_path,
        });
        let entry = prepared.last().expect("prepared manifest entry");
        if let Err(source) = fs::write(&entry.temp_path, encoded) {
            return Err(manifest_batch_error(
                format!(
                    "failed to stage upstream SVG provenance {}: {source}",
                    entry.temp_path.display()
                ),
                cleanup_prepared_manifest_temps(&prepared),
            ));
        }
    }

    let mut staged = Vec::with_capacity(prepared.len());
    for entry in &prepared {
        let backup_path = if entry.manifest_path.is_file() {
            if let Err(source) = fs::rename(&entry.manifest_path, &entry.backup_path) {
                let mut cleanup_errors = rollback_manifest_batch(&staged, &[]);
                cleanup_errors.extend(cleanup_prepared_manifest_temps(&prepared));
                return Err(manifest_batch_error(
                    format!(
                        "failed to stage existing upstream SVG provenance {}: {source}",
                        entry.manifest_path.display()
                    ),
                    cleanup_errors,
                ));
            }
            Some(entry.backup_path.clone())
        } else {
            None
        };
        staged.push(StagedManifestWrite {
            manifest_path: entry.manifest_path.clone(),
            backup_path,
        });
    }

    let mut installed = Vec::with_capacity(prepared.len());
    for entry in &prepared {
        if let Err(source) = install(&entry.temp_path, &entry.manifest_path) {
            let mut cleanup_errors = rollback_manifest_batch(&staged, &installed);
            cleanup_errors.extend(cleanup_prepared_manifest_temps(&prepared));
            return Err(manifest_batch_error(
                format!(
                    "failed to atomically install upstream SVG provenance {}: {source}",
                    entry.manifest_path.display()
                ),
                cleanup_errors,
            ));
        }
        installed.push(entry.manifest_path.clone());
    }

    if let Err(err) = validate_after_install() {
        let mut cleanup_errors = rollback_manifest_batch(&staged, &installed);
        cleanup_errors.extend(cleanup_prepared_manifest_temps(&prepared));
        return Err(manifest_batch_error(
            format!("upstream SVG provenance post-install validation failed: {err}"),
            cleanup_errors,
        ));
    }

    for backup_path in staged.iter().filter_map(|entry| entry.backup_path.as_ref()) {
        if let Err(err) = fs::remove_file(backup_path) {
            eprintln!(
                "warning: failed to remove committed upstream SVG provenance backup {}: {err}",
                backup_path.display()
            );
        }
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn write_manifest_batch_with_installer<I>(
    manifests: &[(&Path, &UpstreamSvgManifest)],
    install: I,
) -> Result<(), XtaskError>
where
    I: FnMut(&Path, &Path) -> std::io::Result<()>,
{
    write_manifest_batch_with_installer_and_validator(manifests, install, || Ok(()))
}

pub(super) fn write_manifest_batch(
    manifests: &[(&Path, &UpstreamSvgManifest)],
) -> Result<(), XtaskError> {
    write_manifest_batch_with_installer_and_validator(
        manifests,
        |from, to| fs::rename(from, to),
        || Ok(()),
    )
}

#[cfg(test)]
pub(super) fn write_manifest(
    out_dir: &Path,
    manifest: &UpstreamSvgManifest,
) -> Result<(), XtaskError> {
    write_manifest_batch(&[(out_dir, manifest)])
}

pub(super) fn write_manifest_with_post_install_validator<V>(
    out_dir: &Path,
    manifest: &UpstreamSvgManifest,
    validate_after_install: V,
) -> Result<(), XtaskError>
where
    V: FnOnce() -> Result<(), XtaskError>,
{
    write_manifest_batch_with_installer_and_validator(
        &[(out_dir, manifest)],
        |from, to| fs::rename(from, to),
        validate_after_install,
    )
}
