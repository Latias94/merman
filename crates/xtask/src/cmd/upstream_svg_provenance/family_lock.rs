use crate::XtaskError;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug)]
struct UpstreamSvgPathLock {
    file: fs::File,
    canonical_target: PathBuf,
}

impl Drop for UpstreamSvgPathLock {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.file);
    }
}

#[derive(Debug)]
pub(crate) struct UpstreamSvgFamilyLock {
    lock: UpstreamSvgPathLock,
}

#[derive(Debug)]
pub(crate) struct UpstreamSvgToolchainLock {
    _lock: UpstreamSvgPathLock,
}

fn canonical_lock_target(target: &Path) -> Result<PathBuf, XtaskError> {
    fs::canonicalize(target).map_err(|source| XtaskError::ReadFile {
        path: target.display().to_string(),
        source,
    })
}

fn acquire_canonical_upstream_svg_lock(
    canonical_target: &Path,
    timeout: Duration,
    description: &str,
) -> Result<UpstreamSvgPathLock, XtaskError> {
    let mut hasher = Sha256::new();
    hasher.update(canonical_target.as_os_str().as_encoded_bytes());
    let lock_root = std::env::temp_dir()
        .join("merman-xtask-locks")
        .join("upstream-svg");
    fs::create_dir_all(&lock_root).map_err(|source| XtaskError::WriteFile {
        path: lock_root.display().to_string(),
        source,
    })?;
    let lock_path = lock_root.join(format!("{:x}.lock", hasher.finalize()));
    let file = fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|source| XtaskError::WriteFile {
            path: lock_path.display().to_string(),
            source,
        })?;
    let started = Instant::now();
    loop {
        match fs2::FileExt::try_lock_exclusive(&file) {
            Ok(()) => {
                return Ok(UpstreamSvgPathLock {
                    file,
                    canonical_target: canonical_target.to_path_buf(),
                });
            }
            Err(err) if err.kind() == fs2::lock_contended_error().kind() => {
                if started.elapsed() >= timeout {
                    return Err(XtaskError::UpstreamSvgFailed(format!(
                        "timed out waiting for the {description} for {}",
                        canonical_target.display()
                    )));
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(source) => {
                return Err(XtaskError::WriteFile {
                    path: lock_path.display().to_string(),
                    source,
                });
            }
        }
    }
}

impl UpstreamSvgFamilyLock {
    pub(crate) fn validate_target(&self, out_dir: &Path) -> Result<(), XtaskError> {
        let canonical_out_dir = canonical_lock_target(out_dir)?;
        if canonical_out_dir == self.lock.canonical_target {
            return Ok(());
        }
        Err(XtaskError::UpstreamSvgFailed(format!(
            "upstream SVG family lock protects {}, not {}",
            self.lock.canonical_target.display(),
            canonical_out_dir.display()
        )))
    }
}

impl UpstreamSvgToolchainLock {
    pub(crate) fn validate_target(&self, tools_root: &Path) -> Result<(), XtaskError> {
        let canonical_tools_root = canonical_lock_target(tools_root)?;
        if canonical_tools_root == self._lock.canonical_target {
            return Ok(());
        }
        Err(XtaskError::UpstreamSvgFailed(format!(
            "upstream SVG toolchain lock protects {}, not {}",
            self._lock.canonical_target.display(),
            canonical_tools_root.display()
        )))
    }
}

pub(crate) fn acquire_upstream_svg_family_lock(
    out_dir: &Path,
) -> Result<UpstreamSvgFamilyLock, XtaskError> {
    acquire_upstream_svg_family_lock_with_timeout(out_dir, Duration::from_secs(30))
}

pub(crate) fn acquire_upstream_svg_family_lock_with_timeout(
    out_dir: &Path,
    timeout: Duration,
) -> Result<UpstreamSvgFamilyLock, XtaskError> {
    let canonical_out_dir = canonical_lock_target(out_dir)?;
    acquire_canonical_upstream_svg_lock(&canonical_out_dir, timeout, "upstream SVG family lock")
        .map(|lock| UpstreamSvgFamilyLock { lock })
}

pub(crate) fn acquire_upstream_svg_toolchain_lock(
    tools_root: &Path,
) -> Result<UpstreamSvgToolchainLock, XtaskError> {
    acquire_upstream_svg_toolchain_lock_with_timeout(tools_root, Duration::from_secs(30))
}

fn acquire_upstream_svg_toolchain_lock_with_timeout(
    tools_root: &Path,
    timeout: Duration,
) -> Result<UpstreamSvgToolchainLock, XtaskError> {
    let canonical_tools_root = canonical_lock_target(tools_root)?;
    acquire_canonical_upstream_svg_lock(
        &canonical_tools_root,
        timeout,
        "upstream SVG toolchain lock",
    )
    .map(|lock| UpstreamSvgToolchainLock { _lock: lock })
}

pub(crate) fn acquire_upstream_svg_family_locks(
    out_dirs: &[PathBuf],
) -> Result<Vec<UpstreamSvgFamilyLock>, XtaskError> {
    acquire_upstream_svg_family_locks_with_timeout(out_dirs, Duration::from_secs(30))
}

pub(super) fn acquire_upstream_svg_family_locks_with_timeout(
    out_dirs: &[PathBuf],
    timeout: Duration,
) -> Result<Vec<UpstreamSvgFamilyLock>, XtaskError> {
    let mut canonical_dirs = out_dirs
        .iter()
        .map(|out_dir| canonical_lock_target(out_dir))
        .collect::<Result<Vec<_>, _>>()?;
    canonical_dirs.sort();
    canonical_dirs.dedup();

    let started = Instant::now();
    canonical_dirs
        .iter()
        .map(|out_dir| {
            let remaining = timeout.saturating_sub(started.elapsed());
            acquire_canonical_upstream_svg_lock(out_dir, remaining, "upstream SVG family lock")
                .map(|lock| UpstreamSvgFamilyLock { lock })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_toolchain_dir() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "merman-xtask-toolchain-lock-{}-{nonce}",
            std::process::id()
        ))
    }

    #[test]
    fn toolchain_lock_serializes_install_and_render_users() {
        let tools_root = unique_toolchain_dir();
        fs::create_dir(&tools_root).expect("create toolchain directory");
        let first =
            acquire_upstream_svg_toolchain_lock(&tools_root).expect("acquire first toolchain lock");

        let blocked = acquire_upstream_svg_toolchain_lock_with_timeout(
            &tools_root,
            Duration::from_millis(50),
        )
        .expect_err("a second toolchain user must wait");
        assert!(blocked.to_string().contains("toolchain lock"));

        drop(first);
        acquire_upstream_svg_toolchain_lock_with_timeout(&tools_root, Duration::from_secs(1))
            .expect("released toolchain lock should be reusable");
        fs::remove_dir(&tools_root).expect("remove toolchain directory");
    }
}
