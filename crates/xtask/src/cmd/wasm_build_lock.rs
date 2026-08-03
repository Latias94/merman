use crate::{XtaskError, cmd::paths};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const LOCK_DIRECTORY_NAME: &str = ".merman-wasm-build.lock";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(50);
const INCOMPLETE_OWNER_GRACE: Duration = Duration::from_secs(5);
static TOKEN_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Deserialize, Serialize)]
struct LockOwner {
    pid: u32,
    token: String,
}

#[derive(Debug)]
pub(crate) struct WorkspaceWasmBuildLock {
    directory: PathBuf,
    token: String,
}

impl WorkspaceWasmBuildLock {
    pub(crate) fn acquire() -> Result<Self, XtaskError> {
        Self::acquire_at(
            paths::target_root().join(LOCK_DIRECTORY_NAME),
            DEFAULT_TIMEOUT,
            DEFAULT_POLL_INTERVAL,
        )
    }

    fn acquire_at(
        directory: PathBuf,
        timeout: Duration,
        poll_interval: Duration,
    ) -> Result<Self, XtaskError> {
        if let Some(parent) = directory.parent() {
            fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
                path: parent.display().to_string(),
                source,
            })?;
        }

        let deadline = Instant::now() + timeout;
        loop {
            match fs::create_dir(&directory) {
                Ok(()) => return Self::claim(directory),
                Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {
                    if remove_stale_lock(&directory)? {
                        continue;
                    }
                }
                Err(source) => {
                    return Err(XtaskError::WriteFile {
                        path: directory.display().to_string(),
                        source,
                    });
                }
            }

            if Instant::now() >= deadline {
                return Err(XtaskError::WasmBuildLockFailed(format!(
                    "timed out waiting for the workspace WASM build lock: {}",
                    directory.display()
                )));
            }
            std::thread::sleep(
                poll_interval.min(deadline.saturating_duration_since(Instant::now())),
            );
        }
    }

    fn claim(directory: PathBuf) -> Result<Self, XtaskError> {
        let owner = LockOwner {
            pid: std::process::id(),
            token: lock_token(),
        };
        let owner_path = directory.join("owner.json");
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&owner_path)
                .map_err(|source| XtaskError::WriteFile {
                    path: owner_path.display().to_string(),
                    source,
                })?;
            let payload = serde_json::to_vec_pretty(&owner).map_err(XtaskError::Json)?;
            file.write_all(&payload)
                .and_then(|()| file.write_all(b"\n"))
                .map_err(|source| XtaskError::WriteFile {
                    path: owner_path.display().to_string(),
                    source,
                })?;
            Ok(Self {
                directory: directory.clone(),
                token: owner.token,
            })
        })();

        if result.is_err() {
            let _ = fs::remove_dir_all(&directory);
        }
        result
    }
}

impl Drop for WorkspaceWasmBuildLock {
    fn drop(&mut self) {
        let owner_path = self.directory.join("owner.json");
        let owner = fs::read(&owner_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<LockOwner>(&bytes).ok());
        if owner.as_ref().map(|owner| owner.token.as_str()) == Some(self.token.as_str()) {
            let _ = fs::remove_dir_all(&self.directory);
        }
    }
}

fn remove_stale_lock(directory: &Path) -> Result<bool, XtaskError> {
    let owner_path = directory.join("owner.json");
    match fs::read(&owner_path) {
        Ok(bytes) => match serde_json::from_slice::<LockOwner>(&bytes) {
            Ok(owner) if process_is_alive(owner.pid) => Ok(false),
            Ok(_) => quarantine_stale_lock(directory),
            Err(_) => recover_incomplete_lock(directory),
        },
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            recover_incomplete_lock(directory)
        }
        Err(source) => Err(XtaskError::ReadFile {
            path: owner_path.display().to_string(),
            source,
        }),
    }
}

fn recover_incomplete_lock(directory: &Path) -> Result<bool, XtaskError> {
    match lock_age(directory)? {
        Some(age) if age < INCOMPLETE_OWNER_GRACE => Ok(false),
        Some(_) => quarantine_stale_lock(directory),
        None => Ok(true),
    }
}

fn quarantine_stale_lock(directory: &Path) -> Result<bool, XtaskError> {
    let mut quarantine = directory.as_os_str().to_os_string();
    quarantine.push(".quarantine-");
    quarantine.push(lock_token());
    let quarantine = PathBuf::from(quarantine);

    match fs::rename(directory, &quarantine) {
        Ok(()) => {}
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(true),
        Err(source) => {
            return Err(XtaskError::WriteFile {
                path: directory.display().to_string(),
                source,
            });
        }
    }

    match fs::remove_dir_all(&quarantine) {
        Ok(()) => Ok(true),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(source) => Err(XtaskError::WriteFile {
            path: quarantine.display().to_string(),
            source,
        }),
    }
}

fn lock_age(directory: &Path) -> Result<Option<Duration>, XtaskError> {
    let modified = match fs::metadata(directory).and_then(|metadata| metadata.modified()) {
        Ok(modified) => modified,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(XtaskError::ReadFile {
                path: directory.display().to_string(),
                source,
            });
        }
    };
    Ok(Some(
        SystemTime::now()
            .duration_since(modified)
            .unwrap_or(Duration::ZERO),
    ))
}

fn epoch_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn lock_token() -> String {
    format!(
        "{}-{}-{}",
        std::process::id(),
        epoch_millis(),
        TOKEN_COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

#[cfg(unix)]
fn process_is_alive(pid: u32) -> bool {
    let result = unsafe { libc::kill(pid as libc::pid_t, 0) };
    result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(windows)]
fn process_is_alive(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_ACCESS_DENIED, GetLastError, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        return unsafe { GetLastError() } == ERROR_ACCESS_DENIED;
    }
    unsafe { CloseHandle(handle) };
    true
}

#[cfg(not(any(unix, windows)))]
fn process_is_alive(_pid: u32) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn workspace_lock_name_matches_the_web_build_protocol() {
        assert_eq!(LOCK_DIRECTORY_NAME, ".merman-wasm-build.lock");
    }

    #[test]
    fn two_contenders_are_serialized() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let directory = temp.path().join(LOCK_DIRECTORY_NAME);
        let first = WorkspaceWasmBuildLock::acquire_at(
            directory.clone(),
            Duration::from_secs(1),
            Duration::from_millis(5),
        )
        .expect("first lock");
        let (send, receive) = mpsc::channel();
        let contender = std::thread::spawn(move || {
            let lock = WorkspaceWasmBuildLock::acquire_at(
                directory,
                Duration::from_secs(1),
                Duration::from_millis(5),
            )
            .expect("second lock");
            send.send(()).expect("notify");
            drop(lock);
        });

        assert!(receive.recv_timeout(Duration::from_millis(50)).is_err());
        drop(first);
        receive
            .recv_timeout(Duration::from_secs(1))
            .expect("second lock acquired");
        contender.join().expect("contender thread");
    }

    #[test]
    fn dead_owner_is_recovered() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let directory = temp.path().join(LOCK_DIRECTORY_NAME);
        fs::create_dir(&directory).expect("lock directory");
        fs::write(
            directory.join("owner.json"),
            br#"{"pid":2147483647,"token":"dead"}"#,
        )
        .expect("dead owner");

        let lock = WorkspaceWasmBuildLock::acquire_at(
            directory.clone(),
            Duration::from_secs(1),
            Duration::from_millis(5),
        )
        .expect("recovered lock");
        drop(lock);

        assert!(!directory.exists());
    }
}
